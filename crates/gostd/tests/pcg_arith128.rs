//! Ported from golang.org/x/exp/rand/arith128_test.go (x/exp v0.0.0-20260603202125-055de637280b,
//! BSD-3-Clause, Copyright 2017 The Go Authors).
//! Go checks the unexported `add` and `multiply` against math/big mod 2^128; here the reference is
//! `u128` wrapping arithmetic. `PcgSource::from_state(high, low)` and `state()` stand for the Go
//! test's direct access to the `low` and `high` fields.
//! TestPCGMultiplyLong uses a local xorshift64 instead of Go's unseeded math/rand.

use gostd::pcg::PcgSource;

const MAX: u64 = u64::MAX;
const INCREMENT: u128 = 117397592171526113268558934119004209487;
const MULTIPLIER: u128 = 47026247687942121848144207491837523525;

/// `arithTests` (xHi, xLo).
const ARITH_TESTS: [(u64, u64); 11] = [
    (0, 0),
    (0, 1),
    (1, 0),
    (0, MAX),
    (MAX, 0),
    (MAX, MAX),
    // Randomly generated 64-bit integers.
    (3757956613005209672, 17983933746665545631),
    (511324141977587414, 5626651684620191081),
    (1534313104606153588, 2415006486399353367),
    (6873586429837825902, 13854394671140464137),
    (6617134480561088940, 18421520694158684312),
];

fn big(hi: u64, lo: u64) -> u128 {
    (u128::from(hi) << 64) | u128::from(lo)
}

fn split(b: u128) -> (u64, u64) {
    ((b >> 64) as u64, b as u64)
}

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_pcg_add() {
    for (i, (hi, lo)) in ARITH_TESTS.into_iter().enumerate() {
        let mut p = PcgSource::from_state(hi, lo);
        p.add();
        let expect = split(big(hi, lo).wrapping_add(INCREMENT));
        assert_eq!(p.state(), expect, "{i}: hi,lo");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_pcg_multiply() {
    for (i, (hi, lo)) in ARITH_TESTS.into_iter().enumerate() {
        let mut p = PcgSource::from_state(hi, lo);
        p.multiply();
        let expect = split(big(hi, lo).wrapping_mul(MULTIPLIER));
        assert_eq!(p.state(), expect, "{i}: hi,lo");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_pcg_multiply_long() {
    let mut x = 0x9e3779b97f4a7c15u64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for i in 0..1_000_000 {
        let low = next();
        let high = next();
        let mut p = PcgSource::from_state(high, low);
        p.multiply();
        let expect = split(big(high, low).wrapping_mul(MULTIPLIER));
        assert_eq!(p.state(), expect, "{i}: ({high},{low})");
    }
}
