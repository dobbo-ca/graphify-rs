//! `echo`: the output is the input. It proves the harness.

use super::Op;

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    vec![("echo", Ok(input.to_vec()))]
}
