//! Ported from golang.org/x/exp/rand/regress_test.go (x/exp v0.0.0-20260603202125-055de637280b,
//! BSD-3-Clause, Copyright 2014 The Go Authors).
//! TestSourceRegress is ported whole. TestRegress is ported for the Intn, Uint64n and Uint64 rows
//! only (the methods graphify-go reaches); the Go test drives each method by reflection with
//! `r := New(NewSource(0))`, `r.Seed(0)` before each method and 20 repeats, the argument being
//! `int64s[repeat%13]` (Intn) or `uint64s[repeat%13]` (Uint64n).

use gostd::pcg::{PcgSource, Rand};

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_source_regress() {
    let mut src = PcgSource::new(1);
    let mut got = [0u64; 20];
    for g in &mut got {
        *g = src.uint64();
    }
    let want: [u64; 20] = [
        0x34e936394905d167,
        0x817c0ef62fe4c731,
        0x937987e6e24f5a40,
        0x0c0a8307fe226199,
        0xf96363568d8bab56,
        0xbaef3af36bd02620,
        0x8f18e416eb6b936b,
        0x05a43fc149f3a67a,
        0xdab012eb3ce01697,
        0xf76c495a133c6aa9,
        0x304b24c5040ce457,
        0x47d77e0abb413159,
        0x52a810fa9e452f04,
        0x2d24b66380cf4780,
        0x5ec7691b92018ef5,
        0x5076dfa749261ea0,
        0xac8f11ad3941d213,
        0x13fa8d67de91db25,
        0xb50883a9893274eb,
        0xeb8f59263f9109ac,
    ];
    assert_eq!(got, want);
}

/// (argument, golden) for `r.Intn(arg)`, 20 repeats, seed 0.
const INTN: [(i64, i64); 20] = [
    (1, 0),
    (10, 6),
    (32, 17),
    (1048576, 1000595),
    (1048577, 424333),
    (1000000000, 382438494),
    (1073741824, 902738458),
    (2147483646, 1204933878),
    (2147483647, 1376191263),
    (1000000000000000000, 502116868085730778),
    (1152921504606846976, 144894195020570665),
    (9223372036854775806, 6715870808026712034),
    (9223372036854775807, 528819992478005418),
    (1, 0),
    (10, 0),
    (32, 20),
    (1048576, 854710),
    (1048577, 649893),
    (1000000000, 687244206),
    (1073741824, 836883386),
];

/// (argument, golden) for `r.Uint64n(arg)`, 20 repeats, seed 0.
const UINT64N: [(u64, u64); 20] = [
    (1, 0),
    (10, 6),
    (32, 17),
    (1048576, 1000595),
    (1048577, 424333),
    (1000000000, 382438494),
    (1073741824, 902738458),
    (2147483646, 1204933878),
    (2147483647, 1376191263),
    (1000000000000000000, 502116868085730778),
    (1152921504606846976, 144894195020570665),
    (18446744073709551614, 6715870808026712034),
    (18446744073709551615, 528819992478005418),
    (1, 0),
    (10, 0),
    (32, 20),
    (1048576, 854710),
    (1048577, 649893),
    (1000000000, 687244206),
    (1073741824, 836883386),
];

/// Golden for `r.Uint64()`, 20 repeats, seed 0.
const UINT64: [u64; 20] = [
    14697929703826476783,
    5591422465364813936,
    74029666500212977,
    8088122161323000979,
    16521829690994476282,
    10814004662382438494,
    9052198920789078554,
    7381380909356947872,
    10961594741481288303,
    12502116868085730778,
    16285795259516428329,
    6715870808026712034,
    528819992478005418,
    2284534088986354339,
    10169200759946765890,
    3813019469742317492,
    10592760183762258614,
    7367238674766648970,
    8217673022687244206,
    3185531743396549562,
];

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_regress_intn() {
    let mut r = Rand::new(PcgSource::new(0));
    for (n, want) in INTN {
        assert_eq!(r.intn(n), want, "r.Intn({n})");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_regress_uint64n() {
    let mut r = Rand::new(PcgSource::new(0));
    for (n, want) in UINT64N {
        assert_eq!(r.uint64n(n), want, "r.Uint64n({n})");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.17"]
fn test_regress_uint64() {
    let mut r = Rand::new(PcgSource::new(0));
    for want in UINT64 {
        assert_eq!(r.uint64(), want, "r.Uint64()");
    }
}
