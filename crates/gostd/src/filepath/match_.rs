//! Go 1.27.4 `path/filepath/match.go` (unix): `Match` and `Glob`. Call site: graphify-go
//! `cargo.go:223` (`Glob`). Patterns and names are matched as bytes, so non-UTF-8 file names
//! survive; an invalid byte decodes as U+FFFD of width 1, as in Go.

use super::{join_bytes, path_of};
use crate::utf8::{RUNE_ERROR, decode_rune};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

/// Go `filepath.ErrBadPattern`: `syntax error in pattern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadPattern;

impl fmt::Display for BadPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("syntax error in pattern")
    }
}

impl std::error::Error for BadPattern {}

/// Go `filepath.Match`.
pub fn match_pattern(pattern: &str, name: &str) -> Result<bool, BadPattern> {
    match_bytes(pattern.as_bytes(), name.as_bytes())
}

/// match.go:46 `Match` on bytes.
fn match_bytes(mut pattern: &[u8], mut name: &[u8]) -> Result<bool, BadPattern> {
    'pattern: while !pattern.is_empty() {
        let (star, chunk, rest) = scan_chunk(pattern);
        pattern = rest;
        if star && chunk.is_empty() {
            // Trailing * matches rest of string unless it has a /.
            return Ok(!name.contains(&b'/'));
        }
        // Look for match at current position. If we're the last chunk, make sure we've exhausted
        // the name, otherwise we'll give a false result even if we could still match using the
        // star.
        match match_chunk(chunk, name) {
            Ok(Some(t)) if t.is_empty() || !pattern.is_empty() => {
                name = t;
                continue;
            }
            Err(e) => return Err(e),
            _ => {}
        }
        if star {
            // Look for match skipping i+1 bytes. Cannot skip /.
            let mut i = 0;
            while i < name.len() && name[i] != b'/' {
                match match_chunk(chunk, &name[i + 1..]) {
                    Ok(Some(t)) => {
                        // if we're the last chunk, make sure we exhausted the name
                        if !(pattern.is_empty() && !t.is_empty()) {
                            name = t;
                            continue 'pattern;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => return Err(e),
                }
                i += 1;
            }
        }
        return Ok(false);
    }
    Ok(name.is_empty())
}

/// match.go:93 `scanChunk`: the next segment of pattern, a non-star string possibly preceded by a
/// star.
fn scan_chunk(mut pattern: &[u8]) -> (bool, &[u8], &[u8]) {
    let mut star = false;
    while let [b'*', rest @ ..] = pattern {
        pattern = rest;
        star = true;
    }
    let mut inrange = false;
    let mut i = 0;
    while i < pattern.len() {
        match pattern[i] {
            // error check handled in match_chunk: bad pattern.
            b'\\' => {
                if i + 1 < pattern.len() {
                    i += 1;
                }
            }
            b'[' => inrange = true,
            b']' => inrange = false,
            b'*' if !inrange => return (star, &pattern[..i], &pattern[i..]),
            _ => {}
        }
        i += 1;
    }
    (star, pattern, &pattern[pattern.len()..])
}

/// match.go:122 `matchChunk`: whether chunk (literals, classes, `?`) matches the beginning of s;
/// `Some(rest of s)` if so. After the match fails the loop keeps checking that the chunk is
/// well-formed but no longer reads s.
fn match_chunk<'a>(mut chunk: &[u8], mut s: &'a [u8]) -> Result<Option<&'a [u8]>, BadPattern> {
    let mut failed = false;
    while !chunk.is_empty() {
        failed = failed || s.is_empty();
        match chunk[0] {
            b'[' => {
                // character class
                let mut r = '\0';
                if !failed {
                    let (c, n) = decode_rune(s);
                    r = c;
                    s = &s[n..];
                }
                chunk = &chunk[1..];
                // possibly negated
                let negated = chunk.first() == Some(&b'^');
                if negated {
                    chunk = &chunk[1..];
                }
                // parse all ranges
                let mut matched = false;
                let mut nrange = 0;
                loop {
                    if chunk.first() == Some(&b']') && nrange > 0 {
                        chunk = &chunk[1..];
                        break;
                    }
                    let (lo, rest) = get_esc(chunk)?;
                    chunk = rest;
                    let mut hi = lo;
                    if chunk.first() == Some(&b'-') {
                        let (h, rest) = get_esc(&chunk[1..])?;
                        hi = h;
                        chunk = rest;
                    }
                    matched = matched || (lo <= r && r <= hi);
                    nrange += 1;
                }
                failed = failed || matched == negated;
            }
            b'?' => {
                if !failed {
                    failed = s[0] == b'/';
                    let (_, n) = decode_rune(s);
                    s = &s[n..];
                }
                chunk = &chunk[1..];
            }
            c => {
                if c == b'\\' {
                    // match.go:158 (non-Windows) then falls through to the literal
                    chunk = &chunk[1..];
                    if chunk.is_empty() {
                        return Err(BadPattern);
                    }
                }
                if !failed {
                    failed = chunk[0] != s[0];
                    s = &s[1..];
                }
                chunk = &chunk[1..];
            }
        }
    }
    if failed { Ok(None) } else { Ok(Some(s)) }
}

/// match.go:200 `getEsc`: a possibly-escaped character from chunk, for a character class.
fn get_esc(mut chunk: &[u8]) -> Result<(char, &[u8]), BadPattern> {
    if chunk.is_empty() || chunk[0] == b'-' || chunk[0] == b']' {
        return Err(BadPattern);
    }
    if chunk[0] == b'\\' {
        chunk = &chunk[1..];
        if chunk.is_empty() {
            return Err(BadPattern);
        }
    }
    let (r, n) = decode_rune(chunk);
    if r == RUNE_ERROR && n == 1 {
        return Err(BadPattern);
    }
    let nchunk = &chunk[n..];
    if nchunk.is_empty() {
        return Err(BadPattern);
    }
    Ok((r, nchunk))
}

/// Go `filepath.Glob`: names are matched as bytes, results are sorted by bytes.
pub fn glob(pattern: &Path) -> Result<Vec<PathBuf>, BadPattern> {
    // match.go:237: limit used to prevent stack exhaustion issues. See CVE-2022-30632.
    const PATH_SEPARATORS_LIMIT: usize = 10000;
    // Go's globWithLimit recurses once per trailing path element of `dir`; the recursion is
    // unrolled here (a Rust test thread has a 2 MiB stack, 10,000 frames would not fit). `files`
    // holds each level's last element, outermost first, and is applied innermost first on the way
    // back, which is the order of Go's returns.
    let mut pattern: Vec<u8> = pattern.as_os_str().as_bytes().to_vec();
    let mut files: Vec<Vec<u8>> = Vec::new();
    let mut depth = 0;
    let mut matches: Vec<Vec<u8>> = loop {
        if depth == PATH_SEPARATORS_LIMIT {
            return Err(BadPattern);
        }
        // Check pattern is well-formed.
        match_bytes(&pattern, b"")?;
        if !has_meta(&pattern) {
            let exists = fs::symlink_metadata(Path::new(OsStr::from_bytes(&pattern))).is_ok();
            break if exists { vec![pattern] } else { Vec::new() };
        }
        let (dir, file) = match pattern.iter().rposition(|&c| c == b'/') {
            Some(i) => (&pattern[..=i], &pattern[i + 1..]),
            None => (&pattern[..0], &pattern[..]),
        };
        let dir = clean_glob_path(dir);
        if !has_meta(&dir) {
            break glob_dir(&dir, file, Vec::new())?;
        }
        // Prevent infinite recursion. See issue 15879.
        if dir == pattern {
            return Err(BadPattern);
        }
        files.push(file.to_vec());
        pattern = dir;
        depth += 1;
    };
    for file in files.iter().rev() {
        let mut next = Vec::new();
        for d in &matches {
            next = glob_dir(d, file, next)?;
        }
        matches = next;
    }
    Ok(matches.into_iter().map(path_of).collect())
}

/// match.go:285 `cleanGlobPath`: prepares path for glob matching.
fn clean_glob_path(path: &[u8]) -> Vec<u8> {
    match path {
        [] => b".".to_vec(),
        // do nothing to the path
        b"/" => path.to_vec(),
        // chop off trailing separator
        _ => path[..path.len() - 1].to_vec(),
    }
}

/// match.go:320 `glob`: searches for files matching pattern in the directory dir and appends them
/// to matches. I/O errors are ignored (the existing matches are returned). New matches are added
/// in byte order.
fn glob_dir(dir: &[u8], pattern: &[u8], mut m: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, BadPattern> {
    let dp = Path::new(OsStr::from_bytes(dir));
    if !fs::metadata(dp).is_ok_and(|fi| fi.is_dir()) {
        return Ok(m);
    }
    let Ok(rd) = fs::read_dir(dp) else {
        return Ok(m);
    };
    // `names, _ := d.Readdirnames(-1)` keeps the names read before an error.
    let mut names: Vec<Vec<u8>> = rd
        .map_while(Result::ok)
        .map(|e| e.file_name().into_vec())
        .collect();
    names.sort();
    for n in names {
        if match_bytes(pattern, &n)? {
            m.push(join_bytes(&[dir, &n]));
        }
    }
    Ok(m)
}

/// match.go:352 `hasMeta` (non-Windows magic characters `*?[\`).
fn has_meta(path: &[u8]) -> bool {
    path.iter().any(|c| matches!(c, b'*' | b'?' | b'[' | b'\\'))
}
