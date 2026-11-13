//! `pcg`: gostd::pcg against golang.org/x/exp/rand (see parity/go/oracle/pcg.go).

use super::Op;
use gostd::pcg::{PcgSource, Rand};
use std::fmt::Write;

const BOUNDS: [u64; 10] = [
    1,
    2,
    3,
    5,
    7,
    10,
    1 << 32,
    (1 << 32) + 1,
    (1 << 63) + 1,
    u64::MAX,
];

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    let seed: u64 = std::str::from_utf8(input).unwrap().parse().unwrap();
    let mut u = String::new();
    let mut r = Rand::new(PcgSource::new(seed));
    for _ in 0..10000 {
        writeln!(u, "{}", r.uint64()).unwrap();
    }
    let mut un = String::new();
    let mut r = Rand::new(PcgSource::new(seed));
    for n in BOUNDS {
        for _ in 0..1000 {
            writeln!(un, "{n} {}", r.uint64n(n)).unwrap();
        }
    }
    let mut ops: Vec<Op> = vec![
        ("uint64", Ok(u.into_bytes())),
        ("uint64n", Ok(un.into_bytes())),
    ];
    if name == "s42" {
        let mut l = String::new();
        for n in 2..=1000usize {
            let mut r = Rand::new(PcgSource::new(seed));
            let mut order: Vec<usize> = (0..n).collect();
            for i in 0..n - 1 {
                let j = i + r.intn((n - i) as i64) as usize;
                order.swap(i, j);
            }
            let line: Vec<String> = order.iter().map(usize::to_string).collect();
            writeln!(l, "{}", line.join(" ")).unwrap();
        }
        ops.push(("louvain", Ok(l.into_bytes())));
    }
    ops
}
