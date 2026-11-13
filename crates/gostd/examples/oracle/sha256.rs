//! `sha256`: gostd::sha256 against Go crypto/sha256 (see parity/go/oracle/sha256.go).

use super::Op;
use gostd::sha256::{Digest, sum256};
use std::fmt::Write;

fn hex(b: &[u8]) -> Vec<u8> {
    b.iter()
        .fold(String::new(), |mut s, x| {
            write!(s, "{x:02x}").unwrap();
            s
        })
        .into_bytes()
}

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    if !name.starts_with("split") {
        return vec![("sum", Ok(hex(&sum256(input))))];
    }
    let nl = input.iter().position(|&b| b == b'\n').unwrap();
    let (line, data) = (&input[..nl], &input[nl + 1..]);
    let mut d = Digest::new();
    let mut prev = 0;
    if !line.is_empty() {
        for f in line.split(|&b| b == b',') {
            let off: usize = std::str::from_utf8(f).unwrap().parse().unwrap();
            d.write(&data[prev..off]);
            prev = off;
        }
    }
    d.write(&data[prev..]);
    vec![
        ("sum", Ok(hex(&sum256(data)))),
        ("split", Ok(hex(&d.sum()))),
    ]
}
