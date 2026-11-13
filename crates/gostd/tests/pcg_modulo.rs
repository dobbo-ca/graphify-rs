//! Ported from golang.org/x/exp/rand/modulo_test.go (x/exp v0.0.0-20260603202125-055de637280b,
//! BSD-3-Clause, Copyright 2017 The Go Authors).
//! Go drives `Uint64n` with a `modSource` that walks [MaxUint64-15, MaxUint64]; the Rust `Rand`
//! owns a `PcgSource`, so the same probe is `Rand::uint64n_with`, which takes the draw function.

use gostd::pcg::Rand;

const MOD_EDGE: u64 = 15;

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_uint64_modulo() {
    let mut m = 0u64;
    let mut src = move || {
        if m > MOD_EDGE {
            m = 0;
        }
        let r = u64::MAX - m;
        m += 1;
        r
    };
    let mut result = [0u64; 10];
    for _ in 0..100 {
        result[Rand::uint64n_with(&mut src, 10) as usize] += 1;
    }
    assert_eq!(result, [10; 10]);
}
