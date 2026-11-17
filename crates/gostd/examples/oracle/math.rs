//! `math`: gostd::math::log against Go math.Log on amd64 (see parity/go/oracle/math.go).

use super::Op;
use std::fmt::Write;

/// One 16-digit hex bit pattern per line in, the bits of its log per line out.
pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let mut b = String::new();
    for l in std::str::from_utf8(input).unwrap().lines() {
        let x = f64::from_bits(u64::from_str_radix(l, 16).unwrap());
        writeln!(b, "{:016x}", gostd::math::log(x).to_bits()).unwrap();
    }
    vec![("log", Ok(b.into_bytes()))]
}
