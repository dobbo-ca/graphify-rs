//! `strconv`: gostd::strconv against Go strconv (see parity/go/oracle/strconv.go).

use super::Op;
use gostd::strconv as sc;
use std::fmt::Display;
use std::fmt::Write;

fn lines(input: &[u8]) -> Vec<&str> {
    let s = std::str::from_utf8(input).expect("strconv inputs are UTF-8");
    s.strip_suffix('\n').unwrap_or(s).split('\n').collect()
}

/// One output line per input line: the value, or the error text.
fn each<T: Display, E: Display>(input: &[u8], f: impl Fn(&str) -> Result<T, E>) -> Op2 {
    let mut b = String::new();
    for l in lines(input) {
        match f(l) {
            Ok(v) => writeln!(b, "{v}").unwrap(),
            Err(e) => writeln!(b, "{e}").unwrap(),
        }
    }
    Ok(b.into_bytes())
}

type Op2 = Result<Vec<u8>, String>;

fn floats(input: &[u8]) -> Vec<Op> {
    let bits: Vec<f64> = lines(input)
        .into_iter()
        .map(|l| f64::from_bits(u64::from_str_radix(l, 16).unwrap()))
        .collect();
    [
        ("g", b'g'),
        ("e", b'e'),
        ("f", b'f'),
        ("upper_e", b'E'),
        ("upper_g", b'G'),
    ]
    .into_iter()
    .map(|(op, fmt)| {
        let mut b = Vec::new();
        for &f in &bits {
            sc::append_float(&mut b, f, fmt);
            b.push(b'\n');
        }
        (op, Ok(b))
    })
    .collect()
}

fn ints(input: &[u8]) -> Vec<Op> {
    vec![
        ("atoi", each(input, sc::atoi)),
        ("parse_int_0_64", each(input, |s| sc::parse_int(s, 0, 64))),
        ("parse_int_10_32", each(input, |s| sc::parse_int(s, 10, 32))),
        (
            "parse_uint_10_32",
            each(input, |s| sc::parse_uint(s, 10, 32)),
        ),
        ("parse_uint_0_64", each(input, |s| sc::parse_uint(s, 0, 64))),
    ]
}

fn quotes(mut input: &[u8]) -> Vec<Op> {
    let mut b = Vec::new();
    while let Some((len, rest)) = input.split_first_chunk::<4>() {
        let (s, rest) = rest.split_at(u32::from_be_bytes(*len) as usize);
        sc::append_quote(&mut b, s);
        b.push(b'\n');
        input = rest;
    }
    vec![("quote", Ok(b))]
}

fn sweep() -> Vec<Op> {
    let mut b = Vec::new();
    for r in (0..=0x10FFFF).filter_map(char::from_u32) {
        sc::append_quote(&mut b, r.encode_utf8(&mut [0; 4]).as_bytes());
        b.push(b'\n');
    }
    vec![("quote_runes", Ok(b))]
}

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    if name == "floats" {
        floats(input)
    } else if name.starts_with("parse-") {
        let parse = |s: &str| sc::parse_float(s).map(|f| format!("{:016x}", f.to_bits()));
        vec![("parse_float", each(input, parse))]
    } else if name == "ints" {
        ints(input)
    } else if name.starts_with("quote-") {
        quotes(input)
    } else if name == "sweep" {
        sweep()
    } else {
        Vec::new()
    }
}
