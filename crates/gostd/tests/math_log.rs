//! Ported from go1.27.4 `math/all_test.go` (BSD-3-Clause, Copyright 2009 The Go Authors).
//! TestLog is ported whole: the `vf`/`log` rows (exact `!=`, as Go), `Log(10) == Ln10`, and the
//! `vflogSC`/`logSC` rows compared with Go's `alike`. TestLog uses neither `close` nor
//! `veryclose`, so they are not ported. Go runs it on amd64 against the assembly `archLog`,
//! which `gostd::math::log` ports. Added: `special_values`.
// The Go values are copied verbatim.
#![allow(clippy::excessive_precision)]

use gostd::math::log;
use std::f64::consts::{LN_10, PI};

const VF: [f64; 10] = [
    4.9790119248836735e+00,
    7.7388724745781045e+00,
    -2.7688005719200159e-01,
    -5.0106036182710749e+00,
    9.6362937071984173e+00,
    2.9263772392439646e+00,
    5.2290834314593066e+00,
    2.7279399104360102e+00,
    1.8253080916808550e+00,
    -8.6859247685756013e+00,
];

const LOG: [f64; 10] = [
    1.605231462693062999102599e+00,
    2.0462560018708770653153909e+00,
    -1.2841708730962657801275038e+00,
    1.6115563905281545116286206e+00,
    2.2655365644872016636317461e+00,
    1.0737652208918379856272735e+00,
    1.6542360106073546632707956e+00,
    1.0035467127723465801264487e+00,
    6.0174879014578057187016475e-01,
    2.161703872847352815363655e+00,
];

const VFLOG_SC: [f64; 7] = [
    f64::NEG_INFINITY,
    -PI,
    -0.0,
    0.0,
    1.0,
    f64::INFINITY,
    f64::NAN,
];
const LOG_SC: [f64; 7] = [
    f64::NAN,
    f64::NAN,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
    0.0,
    f64::INFINITY,
    f64::NAN,
];

/// Go `alike`: both NaN, or equal with the same sign bit.
fn alike(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    a == b && a.is_sign_negative() == b.is_sign_negative()
}

#[test]
fn test_log() {
    for (i, &v) in VF.iter().enumerate() {
        let a = v.abs();
        let f = log(a);
        assert!(LOG[i] == f, "Log({a:e}) = {f:e}, want {:e}", LOG[i]);
    }
    let f = log(10.0);
    assert!(f == LN_10, "Log(10) = {f:e}, want {LN_10:e}");
    for (i, &v) in VFLOG_SC.iter().enumerate() {
        let f = log(v);
        assert!(
            alike(LOG_SC[i], f),
            "Log({v:e}) = {f:e}, want {:e}",
            LOG_SC[i]
        );
    }
}

#[test]
fn special_values() {
    assert_eq!(log(1.0).to_bits(), 0.0f64.to_bits());
    assert_eq!(log(0.0), f64::NEG_INFINITY);
    assert!(log(-1.0).is_nan());
    assert_eq!(log(f64::INFINITY), f64::INFINITY);
}
