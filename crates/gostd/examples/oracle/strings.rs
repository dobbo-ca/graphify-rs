//! `strings`: gostd::strings and gostd::utf8 against Go strings, bytes and unicode/utf8 (see
//! parity/go/oracle/strings.go).

use super::Op;
use gostd::strings as s;
use gostd::unicode::simple_fold;
use gostd::utf8::{decode_last_rune, decode_rune, rune_count};
use std::fmt::Write;

fn decode_all(b: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let (r, w) = decode_rune(&b[i..]);
        write!(out, "{:04X}/{w} ", r as u32).unwrap();
        i += w;
    }
    out.push('|');
    let mut j = b.len();
    while j > 0 {
        let (r, w) = decode_last_rune(&b[..j]);
        write!(out, " {:04X}/{w}", r as u32).unwrap();
        j -= w;
    }
    writeln!(out, " | {}", rune_count(b)).unwrap();
    out
}

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let mut ops: Vec<Op> = vec![
        (
            "to_valid_utf8",
            Ok(s::to_valid_utf8(input, "\u{FFFD}").into_bytes()),
        ),
        (
            "to_valid_utf8_empty",
            Ok(s::to_valid_utf8(input, "").into_bytes()),
        ),
        ("bytes_trim_space", Ok(s::bytes_trim_space(input).to_vec())),
        ("decode", Ok(decode_all(input).into_bytes())),
    ];
    let Ok(text) = std::str::from_utf8(input) else {
        return ops;
    };
    let upper = s::to_upper(text);
    let lower = s::to_lower(text);
    let folded = s::map(text, |c| Some(simple_fold(c)));
    let folds = [&upper, &lower, &folded].map(String::as_str);
    let eq: Vec<u8> = folds
        .into_iter()
        .chain(["k", "K", "\u{212a}", "ß", "SS", "ss", "σ", "ς", "Σ", ""])
        .map(|t| if s::equal_fold(text, t) { b'1' } else { b'0' })
        .collect();
    ops.extend([
        ("to_lower", Ok(lower.into_bytes())),
        ("to_upper", Ok(upper.into_bytes())),
        ("trim_space", Ok(s::trim_space(text).as_bytes().to_vec())),
        ("fields", Ok(s::fields(text).join("\x00").into_bytes())),
        ("equal_fold", Ok(eq)),
    ]);
    ops
}
