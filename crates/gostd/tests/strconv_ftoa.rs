//! Ported from Go 1.27.4 `src/internal/strconv/ftoa_test.go` (GOSRC).
//!
//! Go Test funcs in that file:
//! - TestFtoa: ported (only rows with prec -1, fmt e/E/f/g/G, 64-bit; see the table note)
//! - TestFtoaPowersOfTwo: ported (64-bit half only)
//! - TestFtoaRandom: ported (only the shortest 'g' / -1 part; the fast-vs-slow comparison and the
//!   fixed precision `%e` part are not expressible, so the kept assertion is the round trip)
//! - TestFormatFloatInvalidBitSize: not ported: gostd has no bit size argument (always 64)
#![allow(clippy::excessive_precision)]

use gostd::strconv::{append_float, format_float, parse_float};

// ftoatests: 148 Go rows, 37 kept, 111 left out: 98 with prec != -1, and 13 prec -1 rows with
// fmt 'x'/'X' (10), 'b' (1), and the invalid 'y' and '?' (2; ours panics).
// The float32 half of TestFtoa (bit size 32) is not ported.
const FTOA_TESTS: &[(f64, u8, &str)] = &[
    (1.0, b'g', "1"),
    (20.0, b'g', "20"),
    (1234567.8, b'g', "1.2345678e+06"),
    (200000.0, b'g', "200000"),
    (2000000.0, b'g', "2e+06"),
    (1e10, b'g', "1e+10"),
    (0.0, b'g', "0"),
    (-1.0, b'g', "-1"),
    (12.0, b'g', "12"),
    (123456700.0, b'g', "1.234567e+08"),
    (108678236358137.625, b'g', "1.0867823635813762e+14"),
    (1e23, b'e', "1e+23"),
    (1e23, b'f', "100000000000000000000000"),
    (1e23, b'g', "1e+23"),
    (99999999999999974834176.0, b'e', "9.999999999999997e+22"),
    (99999999999999974834176.0, b'f', "99999999999999970000000"),
    (99999999999999974834176.0, b'g', "9.999999999999997e+22"),
    (100000000000000008388608.0, b'e', "1.0000000000000001e+23"),
    (100000000000000008388608.0, b'f', "100000000000000010000000"),
    (100000000000000008388608.0, b'g', "1.0000000000000001e+23"),
    // Go: fdiv(5e-304, 1e20) "avoid constant arithmetic"; Rust const float division is IEEE
    // (unlike Go untyped constants), so this gives the same 5e-324.
    (5e-304 / 1e20, b'g', "5e-324"),
    (-5e-304 / 1e20, b'g', "-5e-324"),
    (5e-304 / 1e20, b'e', "5e-324"),
    (32.0, b'g', "32"),
    (f64::NAN, b'g', "NaN"),
    (-f64::NAN, b'g', "NaN"),
    (f64::INFINITY, b'g', "+Inf"),
    (f64::NEG_INFINITY, b'g', "-Inf"),
    (-f64::INFINITY, b'g', "-Inf"),
    (2.2250738585072012e-308, b'g', "2.2250738585072014e-308"),
    (2.2250738585072011e-308, b'g', "2.225073858507201e-308"),
    (383260575764816448.0, b'g', "3.8326057576481645e+17"),
    (498484681984085570.0, b'f', "498484681984085570"),
    (-5.8339553793802237e+23, b'g', "-5.8339553793802237e+23"),
    (1.801439850948199e+16, b'g', "1.801439850948199e+16"),
    (5.960464477539063e-08, b'g', "5.960464477539063e-08"),
    (1.012e-320, b'g', "1.012e-320"),
];

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_ftoa() {
    for &(f, fmt, want) in FTOA_TESTS {
        let s = format_float(f, fmt);
        assert_eq!(s, want, "FormatFloat {f:?} {}", fmt as char);
        let mut x = b"abc".to_vec();
        append_float(&mut x, f, fmt);
        assert_eq!(
            String::from_utf8(x).unwrap(),
            format!("abc{want}"),
            "AppendFloat {f:?} {}",
            fmt as char
        );
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_ftoa_powers_of_two() {
    // Go: math.Ldexp(1, exp) for exp in -2048..=2048, skipping +Inf. Built from bits here.
    for exp in -2048i32..=2048 {
        let f = if exp > 1023 {
            continue; // +Inf
        } else if exp >= -1022 {
            f64::from_bits(((exp + 1023) as u64) << 52)
        } else if exp >= -1074 {
            f64::from_bits(1u64 << (exp + 1074))
        } else {
            0.0 // Ldexp underflows to 0 (2^-1075 ties to even)
        };
        let s = format_float(f, b'e');
        let x = parse_float(&s).unwrap_or(f64::NAN);
        assert_eq!(x, f, "failed roundtrip {f:?} => {s} => {x:?}");
    }
    // Go's float32 half (bit size 32) is not ported.
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_ftoa_random() {
    // Go compares the fast and slow (SetOptimize) FormatFloat; that switch is not in the API, so
    // only the shortest round trip is kept. Go uses math/rand, here a local xorshift64.
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..10_000 {
        let x = f64::from_bits(next());
        let s = format_float(x, b'g');
        let y = parse_float(&s).unwrap_or_else(|e| panic!("{x:?} printed as {s}: {e}"));
        if x.is_nan() {
            assert!(y.is_nan(), "{x:?} printed as {s}, parsed {y:?}");
        } else {
            assert_eq!(y, x, "{x:?} printed as {s}, parsed {y:?}");
        }
    }
    // Go's fixed precision `%e` comparison (prec 5..=16) is not ported.
}
