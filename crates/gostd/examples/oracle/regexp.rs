//! `regexp`: gostd::regexp against Go regexp (see parity/go/oracle/regexp.go, which states the
//! input framing and the output encoding; the two files write the same bytes).

use super::Op;
use gostd::regexp::{Regexp, quote_meta};
use std::fmt::Write;

/// The records of an input: 4-byte big-endian length, then the bytes.
fn unframe(mut b: &[u8]) -> Vec<&[u8]> {
    let mut recs = Vec::new();
    while !b.is_empty() {
        let (len, rest) = b.split_at(4);
        let (rec, rest) = rest.split_at(u32::from_be_bytes(len.try_into().unwrap()) as usize);
        recs.push(rec);
        b = rest;
    }
    recs
}

/// The bytes in double quotes, each byte outside 0x20..0x7e and each `"` and `\` as `\xHH`.
fn q(b: &[u8]) -> String {
    let mut s = String::from("\"");
    for &c in b {
        if (c.is_ascii_graphic() || c == b' ') && c != b'"' && c != b'\\' {
            s.push(c as char);
        } else {
            write!(s, "\\x{c:02x}").unwrap();
        }
    }
    s.push('"');
    s
}

fn groups(g: &[Option<&str>]) -> String {
    let parts: Vec<String> = g
        .iter()
        .map(|m| m.map_or("unmatched".to_string(), |t| q(t.as_bytes())))
        .collect();
    format!("[{}]", parts.join(" "))
}

fn spans(g: &[Option<(usize, usize)>]) -> String {
    let parts: Vec<String> = g
        .iter()
        .map(|m| m.map_or("unmatched".to_string(), |(lo, hi)| format!("{lo},{hi}")))
        .collect();
    format!("[{}]", parts.join(" "))
}

/// The number of items and then each item.
fn count(items: impl IntoIterator<Item = String>) -> String {
    let items: Vec<String> = items.into_iter().collect();
    let mut s = items.len().to_string();
    for it in items {
        s.push(' ');
        s.push_str(&it);
    }
    s
}

fn list(n: isize, items: impl IntoIterator<Item = String>) -> String {
    format!("n={n} {}", count(items))
}

fn quoted(l: Vec<&str>) -> Vec<String> {
    l.into_iter().map(|s| q(s.as_bytes())).collect()
}

/// One line per subject; "-" for a subject that is not valid UTF-8.
fn str_op(subs: &[&[u8]], f: impl Fn(&str) -> String) -> Result<Vec<u8>, String> {
    let mut out = String::new();
    for b in subs {
        match std::str::from_utf8(b) {
            Ok(s) => out.push_str(&f(s)),
            Err(_) => out.push('-'),
        }
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// One line per subject.
fn byte_op(subs: &[&[u8]], f: impl Fn(&[u8]) -> String) -> Result<Vec<u8>, String> {
    let mut out = String::new();
    for b in subs {
        out.push_str(&f(b));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    let recs = unframe(input);
    if name == "quote_meta" {
        let out: String = recs
            .iter()
            .map(|r| q(quote_meta(std::str::from_utf8(r).unwrap()).as_bytes()) + "\n")
            .collect();
        return vec![("quote_meta", Ok(out.into_bytes()))];
    }
    let re = match Regexp::compile(std::str::from_utf8(recs[0]).unwrap()) {
        Ok(re) => re,
        Err(e) => return vec![("compile", Err(e.to_string()))],
    };
    let subs = &recs[1..];
    vec![
        ("compile", Ok(b"ok\n".to_vec())),
        (
            "match",
            str_op(subs, |s| u8::from(re.match_string(s)).to_string()),
        ),
        (
            "find_index",
            str_op(subs, |s| {
                re.find_string_index(s)
                    .map_or("none".to_string(), |(lo, hi)| format!("{lo},{hi}"))
            }),
        ),
        (
            "find_submatch",
            str_op(subs, |s| {
                re.find_string_submatch(s)
                    .map_or("none".to_string(), |g| groups(&g))
            }),
        ),
        (
            "find_all",
            str_op(subs, |s| {
                [-1, 0, 2]
                    .map(|n| list(n, quoted(re.find_all_string(s, n))))
                    .join(" ; ")
            }),
        ),
        (
            "find_all_index",
            str_op(subs, |s| {
                count(
                    re.find_all_string_index(s, -1)
                        .into_iter()
                        .map(|(lo, hi)| format!("{lo},{hi}")),
                )
            }),
        ),
        (
            "find_all_submatch",
            str_op(subs, |s| {
                count(
                    re.find_all_string_submatch(s, -1)
                        .into_iter()
                        .map(|g| groups(&g)),
                )
            }),
        ),
        (
            "bytes_submatch_index",
            byte_op(subs, |b| {
                re.find_submatch_index(b)
                    .map_or("none".to_string(), |g| spans(&g))
            }),
        ),
        (
            "bytes_all_submatch_index",
            byte_op(subs, |b| {
                count(
                    re.find_all_submatch_index(b, -1)
                        .into_iter()
                        .map(|g| spans(&g)),
                )
            }),
        ),
        (
            "replace",
            str_op(subs, |s| {
                ["_", "", "$1x", "${1}", "$$", "${name}"]
                    .map(|repl| q(re.replace_all_string(s, repl).as_bytes()))
                    .join(" ")
            }),
        ),
        (
            "replace_func",
            str_op(subs, |s| {
                q(re.replace_all_string_func(s, |m| format!("<{m}>"))
                    .as_bytes())
            }),
        ),
        (
            "split",
            str_op(subs, |s| {
                [-1, 0, 3]
                    .map(|n| list(n, quoted(re.split(s, n))))
                    .join(" ; ")
            }),
        ),
    ]
}
