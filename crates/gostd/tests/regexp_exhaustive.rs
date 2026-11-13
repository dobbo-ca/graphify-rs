//! Go 1.27.4 regexp/exec2_test.go
//!
//! - TestRE2Exhaustive: ported (runs the testRE2 checker from exec_test.go over
//!   `$GOSTD_GOTEST/regexp/re2-exhaustive.txt`, the decompressed form of Go's
//!   `testdata/re2-exhaustive.txt.bz2`; the Go `testing.Short` skip and `!race` build tag are
//!   the slow-test ignore the code PR applies).
//!
//! testRE2 is copied here, not shared. The `[full,longest]` and `[longest]` runs are not ported:
//! `Regexp::Longest` is not reached by graphify-go, so the Rust API has no longest mode. Only
//! the first two of the four `;`-separated results on each line are compared; the count of four
//! is still checked. Patterns that are not valid UTF-8 cannot be passed to `Regexp::compile`
//! and are reported as compile failures, as Go reports them.
//!
//! Go calls `FindStringSubmatchIndex` (not reached by graphify-go); the port calls the
//! equivalent byte API `find_submatch_index`. `MatchString` takes a `&str`, so it is skipped for
//! a text that is not valid UTF-8 (re2-exhaustive.txt has none). As in Go, failure messages show
//! the partial pattern for every run, with the run's suffix.

use gostd::regexp::Regexp;
use std::io::{BufRead, BufReader};

/// Go `strconv.Unquote` for a double-quoted literal, returning the bytes it denotes.
fn unquote(line: &str) -> Result<Vec<u8>, String> {
    let b = line.as_bytes();
    if b.len() < 2 || b[0] != b'"' || b[b.len() - 1] != b'"' {
        return Err("invalid syntax".to_string());
    }
    let b = &b[1..b.len() - 1];
    let mut out = Vec::new();
    let mut i = 0;
    let hex = |s: &[u8]| -> Result<u32, String> {
        let t = std::str::from_utf8(s).map_err(|e| e.to_string())?;
        u32::from_str_radix(t, 16).map_err(|e| e.to_string())
    };
    while i < b.len() {
        let c = b[i];
        i += 1;
        if c != b'\\' {
            out.push(c);
            continue;
        }
        let Some(&e) = b.get(i) else {
            return Err("invalid syntax".to_string());
        };
        i += 1;
        match e {
            b'a' => out.push(7),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'v' => out.push(11),
            b'\\' => out.push(b'\\'),
            b'"' => out.push(b'"'),
            b'x' | b'u' | b'U' => {
                let n = match e {
                    b'x' => 2,
                    b'u' => 4,
                    _ => 8,
                };
                let digits = b.get(i..i + n).ok_or("invalid syntax")?;
                let v = hex(digits)?;
                i += n;
                if e == b'x' {
                    out.push(v as u8);
                } else {
                    let ch = char::from_u32(v).ok_or("invalid syntax")?;
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
            }
            b'0'..=b'7' => {
                let digits = b.get(i..i + 2).ok_or("invalid syntax")?;
                let t = std::str::from_utf8(&[e, digits[0], digits[1]])
                    .map_err(|e| e.to_string())?
                    .to_string();
                let v = u32::from_str_radix(&t, 8).map_err(|e| e.to_string())?;
                if v > 255 {
                    return Err("invalid syntax".to_string());
                }
                i += 2;
                out.push(v as u8);
            }
            _ => return Err("invalid syntax".to_string()),
        }
    }
    Ok(out)
}

type Match = Option<Vec<Option<(usize, usize)>>>;

/// Go `parseResult`: `-` is no match, else space-separated `lo-hi` pairs, `-` for no submatch.
fn parse_result(file: &str, lineno: usize, res: &str) -> Match {
    if res == "-" {
        return None;
    }
    let mut out = Vec::new();
    for pair in res.split(' ') {
        if pair == "-" {
            out.push(None);
        } else {
            let (lo, hi) = pair.split_once('-').unwrap_or((pair, ""));
            match (lo.parse::<usize>(), hi.parse::<usize>()) {
                (Ok(lo), Ok(hi)) if lo <= hi => out.push(Some((lo, hi))),
                _ => panic!("{file}:{lineno}: invalid pair {pair}"),
            }
        }
    }
    Some(out)
}

/// Go `tryCompile`: a panic during compile becomes an error.
fn try_compile(s: &str) -> Result<Regexp, String> {
    match std::panic::catch_unwind(|| Regexp::compile(s)) {
        Ok(Ok(re)) => Ok(re),
        Ok(Err(e)) => Err(e.to_string()),
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("panic: {msg}"))
        }
    }
}

/// Go `testRE2`: checks the Regexp API against RE2 logged results.
fn test_re2(file: &str) {
    let f = std::fs::File::open(file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let file = file.strip_suffix(".bz2").unwrap_or(file);
    let mut str_list: Vec<Vec<u8>> = Vec::new();
    let mut input: Vec<Vec<u8>> = Vec::new();
    let mut in_strings = false;
    let mut re: Option<Regexp> = None;
    let mut refull: Option<Regexp> = None;
    let mut nfail = 0;
    let mut ncase = 0;
    let mut lineno = 0;
    // Records a failure; stops after 100, like Go.
    let mut fail = |msg: String| {
        eprintln!("{msg}");
        nfail += 1;
        if nfail >= 100 {
            panic!("stopping after {nfail} errors");
        }
    };
    for line in BufReader::new(f).lines() {
        lineno += 1;
        let line = line.unwrap_or_else(|e| panic!("{file}:{lineno}: {e}"));
        let first = match line.bytes().next() {
            Some(c) => c,
            None => panic!("{file}:{lineno}: unexpected blank line"),
        };
        if first == b'#' || first.is_ascii_uppercase() {
            // Comment or test name.
            continue;
        } else if line == "strings" {
            str_list.clear();
            in_strings = true;
        } else if line == "regexps" {
            in_strings = false;
        } else if first == b'"' {
            // Fatal because we'll get out of sync.
            let q =
                unquote(&line).unwrap_or_else(|e| panic!("{file}:{lineno}: unquote {line}: {e}"));
            if in_strings {
                str_list.push(q);
                continue;
            }
            // Is a regexp.
            if !input.is_empty() {
                panic!(
                    "{file}:{lineno}: out of sync: have {} strings left before {line}",
                    input.len()
                );
            }
            let compiled = match String::from_utf8(q) {
                Ok(q) => try_compile(&q).map(|r| (q, r)),
                Err(_) => Err("invalid UTF-8".to_string()),
            };
            match compiled {
                Err(err) => {
                    re = None;
                    if err == "error parsing regexp: invalid escape sequence: `\\C`" {
                        // We don't and likely never will support \C; keep going.
                        continue;
                    }
                    fail(format!("{file}:{lineno}: compile {line}: {err}"));
                }
                Ok((q, r)) => {
                    re = Some(r);
                    let full = format!(r"\A(?:{q})\z");
                    refull =
                        Some(try_compile(&full).unwrap_or_else(|e| {
                            panic!("{file}:{lineno}: compile full {full}: {e}")
                        }));
                    input = str_list.clone();
                }
            }
        } else if first == b'-' || first.is_ascii_digit() {
            // A sequence of match results.
            ncase += 1;
            let (Some(re), Some(refull)) = (&re, &refull) else {
                // Failed to compile: skip results.
                continue;
            };
            if input.is_empty() {
                panic!("{file}:{lineno}: out of sync: no input remaining");
            }
            let text = input.remove(0);
            let single_bytes = text.is_ascii();
            if !single_bytes && re.as_str().contains(r"\B") {
                // RE2's \B considers every byte position,
                // so it sees 'not word boundary' in the
                // middle of UTF-8 sequences. This package
                // only considers the positions between runes,
                // so it disagrees. Skip those cases.
                continue;
            }
            let res: Vec<&str> = line.split(';').collect();
            if res.len() != 4 {
                panic!("{file}:{lineno}: have {} test results, want 4", res.len());
            }
            let text_str = std::str::from_utf8(&text).ok();
            // [full] and [partial]; the two longest runs are not ported.
            for (i, (r, suffix)) in [(refull, "[full]"), (re, "")].into_iter().enumerate() {
                let have = r.find_submatch_index(&text);
                let want = parse_result(file, lineno, res[i]);
                let shown = String::from_utf8_lossy(&text);
                if have != want {
                    fail(format!(
                        "{file}:{lineno}: {:?}{suffix}.FindSubmatchIndex({shown:?}) = {have:?}, want {want:?}",
                        re.as_str()
                    ));
                    continue;
                }
                // MatchString takes a &str; a text with invalid UTF-8 has only the byte API.
                if let Some(s) = text_str {
                    let b = r.match_string(s);
                    if b != want.is_some() {
                        fail(format!(
                            "{file}:{lineno}: {:?}{suffix}.MatchString({shown:?}) = {b}, want {}",
                            re.as_str(),
                            !b
                        ));
                    }
                }
            }
        } else {
            panic!("{file}:{lineno}: out of sync: {line}");
        }
    }
    if !input.is_empty() {
        panic!(
            "{file}:{lineno}: out of sync: have {} strings left at EOF",
            input.len()
        );
    }
    assert_eq!(nfail, 0, "{nfail} failures in {file}");
    eprintln!("{ncase} cases tested");
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_re2_exhaustive() {
    let dir = std::env::var("GOSTD_GOTEST").expect(
        "GOSTD_GOTEST is not set: the test stage bind-mounts the testinputs stage at /gotest",
    );
    test_re2(&format!("{dir}/regexp/re2-exhaustive.txt"));
}
