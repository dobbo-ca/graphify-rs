//! Go 1.27.4 regexp/exec_test.go
//!
//! TestRE2Search: ported (testdata/regexp/re2-search.txt; the [full] and [partial] results of each
//!   row are checked, the [full,longest] and [longest] results are left out: no `Longest`)
//! TestFowler: ported in part (basic.dat, nullsubexpr.dat, repetition.dat; Go compiles with
//!   `syntax.POSIX|ClassNL` and leftmost-longest, which the public API cannot express, so a row
//!   checks that the pattern compiles or not, whether it matches and where the match starts)
//! TestLongest: not ported: `Longest` is not in the API
//! TestProgramTooLongForBacktrack: not ported: the backtrack engine is deferred (graphify-rs-aa6.8)
//!
//! BenchmarkMatch and BenchmarkMatch_onepass_regex are benchmarks, not tests; makeText is theirs.
//!
//! TestRE2Exhaustive is not in Go 1.27.4's exec_test.go; it lives in tests/regexp_exhaustive.rs.
//!
//! Go's `strconv.Unquote` is ported as `unquote` (gostd has no unquote), returning bytes because
//! a Go string may hold invalid UTF-8. A text that is not UTF-8 goes only to the byte API
//! (`find_submatch_index`); a pattern that is not UTF-8 cannot be passed to `Regexp::compile` and
//! is left out. Go's `tryCompile` recovers a compile panic; here a panic fails the test.

use std::collections::VecDeque;
use std::fs;

use gostd::regexp::{Regexp, quote_meta};

type Groups = Vec<Option<(usize, usize)>>;

const MAX_FAILS: usize = 100;

fn fail(fails: &mut Vec<String>, msg: String) {
    fails.push(msg);
    assert!(
        fails.len() < MAX_FAILS,
        "stopping after {MAX_FAILS} errors:\n{}",
        fails.join("\n")
    );
}

/// Go `strconv.Unquote` for a double-quoted string.
fn unquote(s: &str) -> Result<Vec<u8>, String> {
    let bad = || format!("invalid syntax: {s}");
    let inner = s
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .ok_or_else(bad)?;
    let mut out = Vec::new();
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        match c {
            '"' | '\n' => return Err(bad()),
            '\\' => match it.next().ok_or_else(bad)? {
                'a' => out.push(7),
                'b' => out.push(8),
                'f' => out.push(12),
                'n' => out.push(b'\n'),
                'r' => out.push(b'\r'),
                't' => out.push(b'\t'),
                'v' => out.push(11),
                '\\' => out.push(b'\\'),
                '"' => out.push(b'"'),
                e @ ('x' | 'u' | 'U') => {
                    let n = match e {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    let mut v: u32 = 0;
                    for _ in 0..n {
                        let d = it.next().and_then(|h| h.to_digit(16)).ok_or_else(bad)?;
                        v = v * 16 + d;
                    }
                    if e == 'x' {
                        out.push(v as u8);
                    } else {
                        let ch = char::from_u32(v).ok_or_else(bad)?;
                        out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                    }
                }
                e @ '0'..='7' => {
                    let mut v = e as u32 - '0' as u32;
                    for _ in 0..2 {
                        let d = it.next().and_then(|h| h.to_digit(8)).ok_or_else(bad)?;
                        v = v * 8 + d;
                    }
                    if v > 255 {
                        return Err(bad());
                    }
                    out.push(v as u8);
                }
                _ => return Err(bad()),
            },
            _ => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }
    Ok(out)
}

fn is_single_bytes(s: &[u8]) -> bool {
    s.iter().all(|&b| b < 0x80)
}

fn parse_result(file: &str, lineno: usize, res: &str) -> Option<Groups> {
    // A single - indicates no match.
    if res == "-" {
        return None;
    }
    // Otherwise, a space-separated list of pairs. - means no submatch.
    let mut out = Vec::new();
    for pair in res.split(' ') {
        if pair == "-" {
            out.push(None);
            continue;
        }
        let (lo, hi) = pair.split_once('-').unwrap_or((pair, ""));
        match (lo.parse::<usize>(), hi.parse::<usize>()) {
            (Ok(lo), Ok(hi)) if lo <= hi => out.push(Some((lo, hi))),
            _ => panic!("{file}:{lineno}: invalid pair {pair}"),
        }
    }
    Some(out)
}

fn test_re2(file: &str) {
    let txt = fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let mut strs: Vec<Vec<u8>> = Vec::new();
    let mut input: VecDeque<Vec<u8>> = VecDeque::new();
    let mut in_strings = false;
    let mut re: Option<Regexp> = None;
    let mut refull: Option<Regexp> = None;
    let mut fails: Vec<String> = Vec::new();
    let mut ncase = 0;
    let mut non_utf8_patterns = 0;
    for (i, line) in txt.lines().enumerate() {
        let lineno = i + 1;
        let Some(&first) = line.as_bytes().first() else {
            panic!("{file}:{lineno}: unexpected blank line");
        };
        if first == b'#' || first.is_ascii_uppercase() {
            // Comment, or a test name.
            continue;
        } else if line == "strings" {
            strs.clear();
            in_strings = true;
        } else if line == "regexps" {
            in_strings = false;
        } else if first == b'"' {
            let q =
                unquote(line).unwrap_or_else(|e| panic!("{file}:{lineno}: unquote {line}: {e}"));
            if in_strings {
                strs.push(q);
                continue;
            }
            // Is a regexp.
            assert!(
                input.is_empty(),
                "{file}:{lineno}: out of sync: have {} strings left before {q:?}",
                input.len()
            );
            re = None;
            refull = None;
            let Ok(q) = String::from_utf8(q) else {
                non_utf8_patterns += 1;
                continue;
            };
            match Regexp::compile(&q) {
                Err(e) => {
                    if e.to_string() == r"error parsing regexp: invalid escape sequence: `\C`" {
                        // We don't and likely never will support \C; keep going.
                        continue;
                    }
                    fail(&mut fails, format!("{file}:{lineno}: compile {q:?}: {e}"));
                    continue;
                }
                Ok(r) => re = Some(r),
            }
            let full = format!(r"\A(?:{q})\z");
            // Fatal because q worked, so this should always work.
            refull = Some(
                Regexp::compile(&full)
                    .unwrap_or_else(|e| panic!("{file}:{lineno}: compile full {full:?}: {e}")),
            );
            input = strs.iter().cloned().collect();
        } else if first == b'-' || first.is_ascii_digit() {
            // A sequence of match results.
            ncase += 1;
            let (Some(re), Some(refull)) = (&re, &refull) else {
                // Failed to compile: skip results.
                continue;
            };
            let text = input
                .pop_front()
                .unwrap_or_else(|| panic!("{file}:{lineno}: out of sync: no input remaining"));
            if !is_single_bytes(&text) && re.as_str().contains(r"\B") {
                // RE2's \B considers every byte position, so it sees 'not word boundary' in
                // the middle of UTF-8 sequences. This package only considers the positions
                // between runes, so it disagrees. Skip those cases.
                continue;
            }
            let res: Vec<&str> = line.split(';').collect();
            assert!(
                res.len() == 4,
                "{file}:{lineno}: have {} test results, want 4",
                res.len()
            );
            // Go runs four: full, partial, full longest, partial longest. Longest is not in the API.
            for (i, r) in res.iter().take(2).enumerate() {
                let (rx, suffix) = if i == 0 { (refull, "[full]") } else { (re, "") };
                let have = rx.find_submatch_index(&text);
                let want = parse_result(file, lineno, r);
                if have != want {
                    fail(
                        &mut fails,
                        format!(
                            "{file}:{lineno}: {:?}{suffix}.FindSubmatchIndex({:?}) = {have:?}, want {want:?}",
                            re.as_str(),
                            String::from_utf8_lossy(&text)
                        ),
                    );
                    continue;
                }
                // match_string and find_string_index take &str; a non-UTF-8 text has only the
                // byte API, whose result was compared above.
                let Ok(s) = std::str::from_utf8(&text) else {
                    continue;
                };
                let b = rx.match_string(s);
                if b != want.is_some() {
                    fail(
                        &mut fails,
                        format!(
                            "{file}:{lineno}: {:?}{suffix}.MatchString({s:?}) = {b}, want {}",
                            re.as_str(),
                            !b
                        ),
                    );
                    continue;
                }
                let idx = rx.find_string_index(s);
                let want_idx = want.as_ref().and_then(|w| w[0]);
                if idx != want_idx {
                    fail(
                        &mut fails,
                        format!(
                            "{file}:{lineno}: {:?}{suffix}.FindStringIndex({s:?}) = {idx:?}, want {want_idx:?}",
                            re.as_str()
                        ),
                    );
                }
            }
        } else {
            panic!("{file}:{lineno}: out of sync: {line}");
        }
    }
    assert!(
        input.is_empty(),
        "{file}: out of sync: have {} strings left at EOF",
        input.len()
    );
    println!("{ncase} cases tested, {non_utf8_patterns} non-UTF-8 patterns left out");
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_re2_search() {
    test_re2(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/testdata/regexp/re2-search.txt"
    ));
}

/// Go `parseFowlerResult`: (ok, compiled, matched, pos); -1 is an unset endpoint (`?`).
fn parse_fowler_result(s: &str) -> (bool, bool, bool, Vec<i64>) {
    const BAD: (bool, bool, bool, Vec<i64>) = (false, false, false, Vec::new());
    // Match with no position information.
    if s.is_empty() {
        return (true, true, true, Vec::new());
    }
    // Match failure.
    if s == "NOMATCH" {
        return (true, true, false, Vec::new());
    }
    // All the other error codes are compile errors.
    if s.as_bytes()[0].is_ascii_uppercase() {
        return (true, false, false, Vec::new());
    }
    let mut s = s;
    let mut x: Vec<i64> = Vec::new();
    while !s.is_empty() {
        let mut end = ')';
        if x.len().is_multiple_of(2) {
            let Some(rest) = s.strip_prefix('(') else {
                return BAD;
            };
            s = rest;
            end = ',';
        }
        let i = s.find(end).unwrap_or(s.len());
        if i == 0 || i == s.len() {
            return BAD;
        }
        let tok = &s[..i];
        let v = if tok == "?" {
            -1
        } else {
            match tok.parse::<i64>() {
                Ok(v) => v,
                Err(_) => return BAD,
            }
        };
        x.push(v);
        s = &s[i + 1..];
    }
    if !x.len().is_multiple_of(2) {
        return BAD;
    }
    (true, true, true, x)
}

fn test_fowler_file(file: &str) {
    let txt = fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let mut fails: Vec<String> = Vec::new();
    let mut last_regexp: Vec<u8> = Vec::new();
    let (mut rows, mut non_utf8) = (0, 0);
    'reading: for (i, seg) in txt.split_inclusive('\n').enumerate() {
        let lineno = i + 1;
        // Go's ReadString drops a final line with no newline.
        let Some(line) = seg.strip_suffix('\n') else {
            break;
        };
        // Input lines may be blank, a comment beginning with #, or a test specification.
        // A specification is five fields separated by one or more tabs. NULL denotes the
        // empty string and NIL denotes the 0 pointer.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut field: Vec<String> = line
            .split('\t')
            .filter(|f| !f.is_empty())
            .map(String::from)
            .collect();
        for f in field.iter_mut() {
            if *f == "NULL" {
                f.clear();
            }
            if *f == "NIL" {
                continue 'reading;
            }
        }
        if field.is_empty() {
            continue;
        }

        // Field 1: the regex(3) flags, or a global control line.
        let mut flag = field[0].clone();
        match flag.as_bytes()[0] {
            b'?' | b'&' | b'|' | b';' | b'{' | b'}' => {
                // Ignore all the control operators. Just run everything.
                flag.remove(0);
                if flag.is_empty() {
                    continue;
                }
            }
            b':' => match flag[1..].split_once(':') {
                Some((_, f)) => flag = f.to_string(),
                None => continue,
            },
            b'C' | b'N' | b'T' | b'0'..=b'9' => continue,
            _ => {}
        }

        // Can check field count now that we've handled the myriad comment formats.
        if field.len() < 4 {
            fail(
                &mut fails,
                format!("{file}:{lineno}: too few fields: {line}"),
            );
            continue;
        }

        // Expand C escapes (a.k.a. Go escapes).
        let mut pat = field[1].clone().into_bytes();
        let mut text = field[2].clone().into_bytes();
        if flag.contains('$') {
            for (f, dst) in [(&field[1], &mut pat), (&field[2], &mut text)] {
                let q = format!("\"{f}\"");
                match unquote(&q) {
                    Ok(b) => *dst = b,
                    Err(_) => {
                        fail(&mut fails, format!("{file}:{lineno}: cannot unquote {q}"));
                        dst.clear();
                    }
                }
            }
        }

        // Field 2: the regular expression pattern; SAME uses the previous one.
        if pat == b"SAME" {
            pat = last_regexp.clone();
        }
        last_regexp = pat.clone();

        // Field 4: the test outcome.
        let (ok, should_compile, should_match, pos) = parse_fowler_result(&field[3]);
        if !ok {
            fail(
                &mut fails,
                format!("{file}:{lineno}: cannot parse result {:?}", field[3]),
            );
            continue;
        }

        let Ok(pat) = String::from_utf8(pat) else {
            non_utf8 += 1;
            continue;
        };
        rows += 1;

        // Run test once for each specified capital letter mode that we support.
        'testing: for c in flag.chars() {
            let mut pattern = pat.clone();
            match c {
                'E' => {} // extended regexp (what we support)
                'L' => pattern = quote_meta(&pattern),
                _ => continue 'testing,
            }
            // Go parses with syntax.POSIX|ClassNL: ^ and $ match at lines, as (?m) does, and
            // 'i' adds FoldCase.
            let fold = if flag.contains('i') { "(?i)" } else { "" };
            let pattern = format!("(?m){fold}{pattern}");

            let re = match Regexp::compile(&pattern) {
                Err(_) => {
                    if should_compile {
                        fail(
                            &mut fails,
                            format!("{file}:{lineno}: {pattern:?} did not compile"),
                        );
                    }
                    continue 'testing;
                }
                Ok(re) => re,
            };
            if !should_compile {
                fail(
                    &mut fails,
                    format!("{file}:{lineno}: {pattern:?} should not compile"),
                );
                continue 'testing;
            }
            let have = re.find_submatch_index(&text);
            let matched = match std::str::from_utf8(&text) {
                Ok(s) => re.match_string(s),
                Err(_) => have.is_some(),
            };
            if matched != should_match {
                fail(
                    &mut fails,
                    format!(
                        "{file}:{lineno}: {pattern:?}.Match({:?}) = {matched}, want {should_match}",
                        String::from_utf8_lossy(&text)
                    ),
                );
                continue 'testing;
            }
            if have.is_some() != matched {
                fail(
                    &mut fails,
                    format!(
                        "{file}:{lineno}: {pattern:?}.Match({:?}) = {matched}, but FindSubmatchIndex = {have:?}",
                        String::from_utf8_lossy(&text)
                    ),
                );
                continue 'testing;
            }
            // Go compares every endpoint; leftmost-longest differs from leftmost-first in the
            // end of the match and in the groups, but not in where the leftmost match starts.
            if let (Some(have), Some(&start)) = (&have, pos.first()) {
                let start_ok =
                    have.first().copied().flatten().map(|(s, _)| s as i64) == Some(start);
                if !start_ok || have.len() < pos.len() / 2 {
                    fail(
                        &mut fails,
                        format!(
                            "{file}:{lineno}: {pattern:?}.FindSubmatchIndex({:?}) = {have:?}, want start {start} and {} groups",
                            String::from_utf8_lossy(&text),
                            pos.len() / 2
                        ),
                    );
                }
            }
        }
    }
    println!("{file}: {rows} rows run, {non_utf8} left out (pattern not UTF-8)");
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_fowler() {
    for name in ["basic.dat", "nullsubexpr.dat", "repetition.dat"] {
        test_fowler_file(&format!(
            "{}/testdata/regexp/{name}",
            env!("CARGO_MANIFEST_DIR")
        ));
    }
}
