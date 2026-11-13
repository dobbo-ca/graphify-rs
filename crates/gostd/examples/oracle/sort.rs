//! `sort`: gostd::sort::{slice, sort_func} and std `sort_by` against Go sort.Slice,
//! slices.SortFunc and sort.SliceStable (see parity/go/oracle/sort.go).

use super::Op;
use gostd::sort::{slice, sort_func};
use std::fmt::Write;

/// One decimal key per line; each element is (key, original index).
fn parse(input: &[u8]) -> Vec<(i64, usize)> {
    std::str::from_utf8(input)
        .unwrap()
        .split_whitespace()
        .enumerate()
        .map(|(i, f)| (f.parse().unwrap(), i))
        .collect()
}

/// The output permutation on one line, then one `i j` line per call.
fn out(x: &[(i64, usize)], trace: &[(usize, usize)]) -> Vec<u8> {
    let mut b = x
        .iter()
        .map(|e| e.1.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    b.push('\n');
    for (i, j) in trace {
        writeln!(b, "{i} {j}").unwrap();
    }
    b.into_bytes()
}

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let mut a = parse(input);
    let mut ta = Vec::new();
    slice(&mut a, |p, q| {
        ta.push((p.1, q.1));
        p.0 < q.0
    });
    let mut f = parse(input);
    let mut tf = Vec::new();
    sort_func(&mut f, |p, q| {
        tf.push((p.1, q.1));
        p.0.cmp(&q.0)
    });
    let mut s = parse(input);
    s.sort_by_key(|p| p.0);
    vec![
        ("slice", Ok(out(&a, &ta))),
        ("sort_func", Ok(out(&f, &tf))),
        ("stable", Ok(out(&s, &[]))),
    ]
}
