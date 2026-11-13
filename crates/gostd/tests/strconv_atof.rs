//! Port of internal/strconv/atof_test.go (GOSRC/internal/strconv/atof_test.go, Go 1.27.4).
//!
//! TestParseFloatPrefix: ported (reduced): parseFloatPrefix is not in the API, so the suffix and consumed-length checks are dropped; every non-error row must parse whole through parse_float
//! TestAtof: ported (64-bit half of testAtof only: the ParseFloat(s, 32) checks and the atof32tests table are not in the API)
//! TestAtofSlow: ported (SetOptimize is not in the API, so it runs the same checks as TestAtof)
//! TestAtofRandom: ported (xorshift64 with a fixed seed instead of math/rand, 10000 iterations)
//! TestRoundTrip: ported (SetOptimize is not in the API, so one pass instead of two)
//! TestRoundTrip32: not ported: bit size 32 is not in the API
//! TestParseFloatIncorrectBitSize: not ported: bit size is not in the API (always 64)
//!
//! atoftests: 264 rows kept of 264 (none left out). atof32tests: 0 of 58 kept (float32 only).
//! Go's internal/strconv returns the bare ErrSyntax / ErrRange; here each is the NumError that
//! parse_float returns for the row's input. On a Go error the float returned beside it is not part
//! of the API, so only the error is checked on those rows.

use gostd::strconv::{NumError, NumErrorKind, format_float, parse_float};

struct AtofTest {
    input: String,
    out: &'static str,
    err: Option<NumErrorKind>,
}

fn t(input: impl Into<String>, out: &'static str, err: Option<NumErrorKind>) -> AtofTest {
    AtofTest {
        input: input.into(),
        out,
        err,
    }
}

fn atoftests() -> Vec<AtofTest> {
    vec![
        t("", "0", Some(NumErrorKind::Syntax)),
        t("1", "1", None),
        t("+1", "1", None),
        t("1x", "0", Some(NumErrorKind::Syntax)),
        t("1.1.", "0", Some(NumErrorKind::Syntax)),
        t("1e23", "1e+23", None),
        t("1E23", "1e+23", None),
        t("100000000000000000000000", "1e+23", None),
        t("1e-100", "1e-100", None),
        t("123456700", "1.234567e+08", None),
        t("99999999999999974834176", "9.999999999999997e+22", None),
        t("100000000000000000000001", "1.0000000000000001e+23", None),
        t("100000000000000008388608", "1.0000000000000001e+23", None),
        t("100000000000000016777215", "1.0000000000000001e+23", None),
        t("100000000000000016777216", "1.0000000000000003e+23", None),
        t("-1", "-1", None),
        t("-0.1", "-0.1", None),
        t("-0", "-0", None),
        t("1e-20", "1e-20", None),
        t("625e-3", "0.625", None),
        // Hexadecimal floating-point.
        t("0x1p0", "1", None),
        t("0x1p1", "2", None),
        t("0x1p-1", "0.5", None),
        t("0x1ep-1", "15", None),
        t("-0x1ep-1", "-15", None),
        t("-0x1_ep-1", "-15", None),
        t("0x1p-200", "6.223015277861142e-61", None),
        t("0x1p200", "1.6069380442589903e+60", None),
        t("0x1fFe2.p0", "131042", None),
        t("0x1fFe2.P0", "131042", None),
        t("-0x2p3", "-16", None),
        t("0x0.fp4", "15", None),
        t("0x0.fp0", "0.9375", None),
        t("0x1e2", "0", Some(NumErrorKind::Syntax)),
        t("1p2", "0", Some(NumErrorKind::Syntax)),
        // zeros
        t("0", "0", None),
        t("0e0", "0", None),
        t("-0e0", "-0", None),
        t("+0e0", "0", None),
        t("0e-0", "0", None),
        t("-0e-0", "-0", None),
        t("+0e-0", "0", None),
        t("0e+0", "0", None),
        t("-0e+0", "-0", None),
        t("+0e+0", "0", None),
        t("0e+01234567890123456789", "0", None),
        t("0.00e-01234567890123456789", "0", None),
        t("-0e+01234567890123456789", "-0", None),
        t("-0.00e-01234567890123456789", "-0", None),
        t("0x0p+01234567890123456789", "0", None),
        t("0x0.00p-01234567890123456789", "0", None),
        t("-0x0p+01234567890123456789", "-0", None),
        t("-0x0.00p-01234567890123456789", "-0", None),
        t("0e291", "0", None), // issue 15364
        t("0e292", "0", None), // issue 15364
        t("0e347", "0", None), // issue 15364
        t("0e348", "0", None), // issue 15364
        t("-0e291", "-0", None),
        t("-0e292", "-0", None),
        t("-0e347", "-0", None),
        t("-0e348", "-0", None),
        t("0x0p126", "0", None),
        t("0x0p127", "0", None),
        t("0x0p128", "0", None),
        t("0x0p129", "0", None),
        t("0x0p130", "0", None),
        t("0x0p1022", "0", None),
        t("0x0p1023", "0", None),
        t("0x0p1024", "0", None),
        t("0x0p1025", "0", None),
        t("0x0p1026", "0", None),
        t("-0x0p126", "-0", None),
        t("-0x0p127", "-0", None),
        t("-0x0p128", "-0", None),
        t("-0x0p129", "-0", None),
        t("-0x0p130", "-0", None),
        t("-0x0p1022", "-0", None),
        t("-0x0p1023", "-0", None),
        t("-0x0p1024", "-0", None),
        t("-0x0p1025", "-0", None),
        t("-0x0p1026", "-0", None),
        // NaNs
        t("nan", "NaN", None),
        t("NaN", "NaN", None),
        t("NAN", "NaN", None),
        // Infs
        t("inf", "+Inf", None),
        t("-Inf", "-Inf", None),
        t("+INF", "+Inf", None),
        t("-Infinity", "-Inf", None),
        t("+INFINITY", "+Inf", None),
        t("Infinity", "+Inf", None),
        // largest float64
        t("1.7976931348623157e308", "1.7976931348623157e+308", None),
        t("-1.7976931348623157e308", "-1.7976931348623157e+308", None),
        t("0x1.fffffffffffffp1023", "1.7976931348623157e+308", None),
        t("-0x1.fffffffffffffp1023", "-1.7976931348623157e+308", None),
        t("0x1fffffffffffffp+971", "1.7976931348623157e+308", None),
        t("-0x1fffffffffffffp+971", "-1.7976931348623157e+308", None),
        t("0x.1fffffffffffffp1027", "1.7976931348623157e+308", None),
        t("-0x.1fffffffffffffp1027", "-1.7976931348623157e+308", None),
        // next float64 - too large
        t("1.7976931348623159e308", "+Inf", Some(NumErrorKind::Range)),
        t("-1.7976931348623159e308", "-Inf", Some(NumErrorKind::Range)),
        t("0x1p1024", "+Inf", Some(NumErrorKind::Range)),
        t("-0x1p1024", "-Inf", Some(NumErrorKind::Range)),
        t("0x2p1023", "+Inf", Some(NumErrorKind::Range)),
        t("-0x2p1023", "-Inf", Some(NumErrorKind::Range)),
        t("0x.1p1028", "+Inf", Some(NumErrorKind::Range)),
        t("-0x.1p1028", "-Inf", Some(NumErrorKind::Range)),
        t("0x.2p1027", "+Inf", Some(NumErrorKind::Range)),
        t("-0x.2p1027", "-Inf", Some(NumErrorKind::Range)),
        // the border is ...158079
        // borderline - okay
        t("1.7976931348623158e308", "1.7976931348623157e+308", None),
        t("-1.7976931348623158e308", "-1.7976931348623157e+308", None),
        t(
            "0x1.fffffffffffff7fffp1023",
            "1.7976931348623157e+308",
            None,
        ),
        t(
            "-0x1.fffffffffffff7fffp1023",
            "-1.7976931348623157e+308",
            None,
        ),
        // borderline - too large
        t(
            "1.797693134862315808e308",
            "+Inf",
            Some(NumErrorKind::Range),
        ),
        t(
            "-1.797693134862315808e308",
            "-Inf",
            Some(NumErrorKind::Range),
        ),
        t("0x1.fffffffffffff8p1023", "+Inf", Some(NumErrorKind::Range)),
        t(
            "-0x1.fffffffffffff8p1023",
            "-Inf",
            Some(NumErrorKind::Range),
        ),
        t("0x1fffffffffffff.8p+971", "+Inf", Some(NumErrorKind::Range)),
        t("-0x1fffffffffffff8p+967", "-Inf", Some(NumErrorKind::Range)),
        t("0x.1fffffffffffff8p1027", "+Inf", Some(NumErrorKind::Range)),
        t(
            "-0x.1fffffffffffff9p1027",
            "-Inf",
            Some(NumErrorKind::Range),
        ),
        // a little too large
        t("1e308", "1e+308", None),
        t("2e308", "+Inf", Some(NumErrorKind::Range)),
        t("1e309", "+Inf", Some(NumErrorKind::Range)),
        t("0x1p1025", "+Inf", Some(NumErrorKind::Range)),
        // way too large
        t("1e310", "+Inf", Some(NumErrorKind::Range)),
        t("-1e310", "-Inf", Some(NumErrorKind::Range)),
        t("1e400", "+Inf", Some(NumErrorKind::Range)),
        t("-1e400", "-Inf", Some(NumErrorKind::Range)),
        t("1e400000", "+Inf", Some(NumErrorKind::Range)),
        t("-1e400000", "-Inf", Some(NumErrorKind::Range)),
        t("0x1p1030", "+Inf", Some(NumErrorKind::Range)),
        t("0x1p2000", "+Inf", Some(NumErrorKind::Range)),
        t("0x1p2000000000", "+Inf", Some(NumErrorKind::Range)),
        t("-0x1p1030", "-Inf", Some(NumErrorKind::Range)),
        t("-0x1p2000", "-Inf", Some(NumErrorKind::Range)),
        t("-0x1p2000000000", "-Inf", Some(NumErrorKind::Range)),
        // denormalized
        t("1e-305", "1e-305", None),
        t("1e-306", "1e-306", None),
        t("1e-307", "1e-307", None),
        t("1e-308", "1e-308", None),
        t("1e-309", "1e-309", None),
        t("1e-310", "1e-310", None),
        t("1e-322", "1e-322", None),
        // smallest denormal
        t("5e-324", "5e-324", None),
        t("4e-324", "5e-324", None),
        t("3e-324", "5e-324", None),
        // too small
        t("2e-324", "0", None),
        // way too small
        t("1e-350", "0", None),
        t("1e-400000", "0", None),
        t("1e-345", "0", None), // picked off in atof64
        t("1e-343", "0", None), // large c.s in parseFloat64
        t("9.999999999999999999e-343", "0", None),
        // Near denormals and denormals.
        t("0x2.00000000000000p-1010", "1.8227805048890994e-304", None), // 0x00e0000000000000
        t("0x1.fffffffffffff0p-1010", "1.8227805048890992e-304", None), // 0x00dfffffffffffff
        t("0x1.fffffffffffff7p-1010", "1.8227805048890992e-304", None), // rounded down
        t("0x1.fffffffffffff8p-1010", "1.8227805048890994e-304", None), // rounded up
        t("0x1.fffffffffffff9p-1010", "1.8227805048890994e-304", None), // rounded up
        t("0x2.00000000000000p-1022", "4.450147717014403e-308", None),  // 0x0020000000000000
        t("0x1.fffffffffffff0p-1022", "4.4501477170144023e-308", None), // 0x001fffffffffffff
        t("0x1.fffffffffffff7p-1022", "4.4501477170144023e-308", None), // rounded down
        t("0x1.fffffffffffff8p-1022", "4.450147717014403e-308", None),  // rounded up
        t("0x1.fffffffffffff9p-1022", "4.450147717014403e-308", None),  // rounded up
        t("0x1.00000000000000p-1022", "2.2250738585072014e-308", None), // 0x0010000000000000
        t("0x0.fffffffffffff0p-1022", "2.225073858507201e-308", None),  // 0x000fffffffffffff
        t("0x0.ffffffffffffe0p-1022", "2.2250738585072004e-308", None), // 0x000ffffffffffffe
        t("0x0.ffffffffffffe7p-1022", "2.2250738585072004e-308", None), // rounded down
        t("0x1.ffffffffffffe8p-1023", "2.225073858507201e-308", None),  // rounded up
        t("0x1.ffffffffffffe9p-1023", "2.225073858507201e-308", None),  // rounded up
        t("0x0.00000003fffff0p-1022", "2.072261e-317", None),           // 0x00000000003fffff
        t("0x0.00000003456780p-1022", "1.694649e-317", None),           // 0x0000000000345678
        t("0x0.00000003456787p-1022", "1.694649e-317", None),           // rounded down
        t("0x0.00000003456788p-1022", "1.694649e-317", None), // rounded down (half to even)
        t("0x0.00000003456790p-1022", "1.6946496e-317", None), // 0x0000000000345679
        t("0x0.00000003456789p-1022", "1.6946496e-317", None), // rounded up
        t(
            "0x0.0000000345678800000000000000000000000001p-1022",
            "1.6946496e-317",
            None,
        ), // rounded up
        t("0x0.000000000000f0p-1022", "7.4e-323", None),      // 0x000000000000000f
        t("0x0.00000000000060p-1022", "3e-323", None),        // 0x0000000000000006
        t("0x0.00000000000058p-1022", "3e-323", None),        // rounded up
        t("0x0.00000000000057p-1022", "2.5e-323", None),      // rounded down
        t("0x0.00000000000050p-1022", "2.5e-323", None),      // 0x0000000000000005
        t("0x0.00000000000010p-1022", "5e-324", None),        // 0x0000000000000001
        t("0x0.000000000000081p-1022", "5e-324", None),       // rounded up
        t("0x0.00000000000008p-1022", "0", None),             // rounded down
        t("0x0.00000000000007fp-1022", "0", None),            // rounded down
        // try to overflow exponent
        t("1e-4294967296", "0", None),
        t("1e+4294967296", "+Inf", Some(NumErrorKind::Range)),
        t("1e-18446744073709551616", "0", None),
        t("1e+18446744073709551616", "+Inf", Some(NumErrorKind::Range)),
        t("0x1p-4294967296", "0", None),
        t("0x1p+4294967296", "+Inf", Some(NumErrorKind::Range)),
        t("0x1p-18446744073709551616", "0", None),
        t(
            "0x1p+18446744073709551616",
            "+Inf",
            Some(NumErrorKind::Range),
        ),
        // Parse errors
        t("1e", "0", Some(NumErrorKind::Syntax)),
        t("1e-", "0", Some(NumErrorKind::Syntax)),
        t(".e-1", "0", Some(NumErrorKind::Syntax)),
        t("1\x00.2", "0", Some(NumErrorKind::Syntax)),
        t("0x", "0", Some(NumErrorKind::Syntax)),
        t("0x.", "0", Some(NumErrorKind::Syntax)),
        t("0x1", "0", Some(NumErrorKind::Syntax)),
        t("0x.1", "0", Some(NumErrorKind::Syntax)),
        t("0x1p", "0", Some(NumErrorKind::Syntax)),
        t("0x.1p", "0", Some(NumErrorKind::Syntax)),
        t("0x1p+", "0", Some(NumErrorKind::Syntax)),
        t("0x.1p+", "0", Some(NumErrorKind::Syntax)),
        t("0x1p-", "0", Some(NumErrorKind::Syntax)),
        t("0x.1p-", "0", Some(NumErrorKind::Syntax)),
        t("0x1p+2", "4", None),
        t("0x.1p+2", "0.25", None),
        t("0x1p-2", "0.25", None),
        t("0x.1p-2", "0.015625", None),
        // https://www.exploringbinary.com/java-hangs-when-converting-2-2250738585072012e-308/
        t("2.2250738585072012e-308", "2.2250738585072014e-308", None),
        // https://www.exploringbinary.com/php-hangs-on-numeric-value-2-2250738585072011e-308/
        t("2.2250738585072011e-308", "2.225073858507201e-308", None),
        // A very large number (initially wrongly parsed by the fast algorithm).
        t("4.630813248087435e+307", "4.630813248087435e+307", None),
        // A different kind of very large number.
        t("22.222222222222222", "22.22222222222222", None),
        t(
            format!("2.{}e+1", "2".repeat(4000)),
            "22.22222222222222",
            None,
        ),
        t("0x1.1111111111111p222", "7.18931911124017e+66", None),
        t("0x2.2222222222222p221", "7.18931911124017e+66", None),
        t(
            format!("0x2.{}p221", "2".repeat(4000)),
            "7.18931911124017e+66",
            None,
        ),
        // Exactly halfway between 1 and math.Nextafter(1, 2).
        // Round to even (down).
        t(
            "1.00000000000000011102230246251565404236316680908203125",
            "1",
            None,
        ),
        t("0x1.00000000000008p0", "1", None),
        // Slightly lower; still round down.
        t(
            "1.00000000000000011102230246251565404236316680908203124",
            "1",
            None,
        ),
        t("0x1.00000000000007Fp0", "1", None),
        // Slightly higher; round up.
        t(
            "1.00000000000000011102230246251565404236316680908203126",
            "1.0000000000000002",
            None,
        ),
        t("0x1.000000000000081p0", "1.0000000000000002", None),
        t("0x1.00000000000009p0", "1.0000000000000002", None),
        // Slightly higher, but you have to read all the way to the end.
        t(
            format!(
                "1.00000000000000011102230246251565404236316680908203125{}1",
                "0".repeat(10000)
            ),
            "1.0000000000000002",
            None,
        ),
        t(
            format!("0x1.00000000000008{}1p0", "0".repeat(10000)),
            "1.0000000000000002",
            None,
        ),
        // Halfway between x := math.Nextafter(1, 2) and math.Nextafter(x, 2)
        // Round to even (up).
        t(
            "1.00000000000000033306690738754696212708950042724609375",
            "1.0000000000000004",
            None,
        ),
        t("0x1.00000000000018p0", "1.0000000000000004", None),
        // Halfway between 1090544144181609278303144771584 and 1090544144181609419040633126912
        // (15497564393479157p+46, should round to even 15497564393479156p+46, issue 36657)
        t(
            "1090544144181609348671888949248",
            "1.0905441441816093e+30",
            None,
        ),
        // slightly above, rounds up
        t(
            "1090544144181609348835077142190",
            "1.0905441441816094e+30",
            None,
        ),
        // Underscores.
        t("1_23.50_0_0e+1_2", "1.235e+14", None),
        t("-_123.5e+12", "0", Some(NumErrorKind::Syntax)),
        t("+_123.5e+12", "0", Some(NumErrorKind::Syntax)),
        t("_123.5e+12", "0", Some(NumErrorKind::Syntax)),
        t("1__23.5e+12", "0", Some(NumErrorKind::Syntax)),
        t("123_.5e+12", "0", Some(NumErrorKind::Syntax)),
        t("123._5e+12", "0", Some(NumErrorKind::Syntax)),
        t("123.5_e+12", "0", Some(NumErrorKind::Syntax)),
        t("123.5__0e+12", "0", Some(NumErrorKind::Syntax)),
        t("123.5e_+12", "0", Some(NumErrorKind::Syntax)),
        t("123.5e+_12", "0", Some(NumErrorKind::Syntax)),
        t("123.5e_-12", "0", Some(NumErrorKind::Syntax)),
        t("123.5e-_12", "0", Some(NumErrorKind::Syntax)),
        t("123.5e+1__2", "0", Some(NumErrorKind::Syntax)),
        t("123.5e+12_", "0", Some(NumErrorKind::Syntax)),
        t("0x_1_2.3_4_5p+1_2", "74565", None),
        t("-_0x12.345p+12", "0", Some(NumErrorKind::Syntax)),
        t("+_0x12.345p+12", "0", Some(NumErrorKind::Syntax)),
        t("_0x12.345p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x__12.345p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x1__2.345p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x12_.345p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x12._345p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.3__45p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345_p+12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345p_+12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345p+_12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345p_-12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345p-_12", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345p+1__2", "0", Some(NumErrorKind::Syntax)),
        t("0x12.345p+12_", "0", Some(NumErrorKind::Syntax)),
        t("1e100x", "0", Some(NumErrorKind::Syntax)),
        t("1e1000x", "0", Some(NumErrorKind::Syntax)),
    ]
}

fn run_atof() {
    for test in atoftests() {
        let got = parse_float(&test.input);
        match test.err {
            Some(kind) => {
                let want = NumError {
                    func: "ParseFloat",
                    num: test.input.clone(),
                    err: kind,
                };
                // Go's value beside the error (0, +Inf, -Inf) is not part of our API.
                assert_eq!(got, Err(want), "ParseFloat({:?}, 64)", test.input);
            }
            None => {
                let v = got.unwrap_or_else(|e| {
                    panic!(
                        "ParseFloat({:?}, 64) = {e:?}, want {}",
                        test.input, test.out
                    )
                });
                assert_eq!(
                    format_float(v, b'g'),
                    test.out,
                    "ParseFloat({:?}, 64)",
                    test.input
                );
            }
        }
    }
}

#[test]
fn test_parse_float_prefix() {
    for test in atoftests() {
        if test.err.is_some() {
            continue;
        }
        // Go also appends suffixes and checks the consumed length; parse_float has no prefix mode.
        assert!(
            parse_float(&test.input).is_ok(),
            "ParseFloat({:?}, 64): want no error",
            test.input
        );
    }
}

#[test]
fn test_atof() {
    run_atof();
}

#[test]
fn test_atof_slow() {
    run_atof();
}

struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn test_atof_random() {
    let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
    for _ in 0..10000 {
        let want = f64::from_bits(rng.next_u64());
        let s = format_float(want, b'g');
        // Go ignores the error here.
        let x = parse_float(&s).unwrap_or(0.0);
        assert!(
            x == want || (want.is_nan() && x.is_nan()),
            "number {s} badly parsed as {:b} (expected {:b})",
            x.to_bits(),
            want.to_bits()
        );
    }
}

#[test]
fn test_round_trip() {
    // Issue 2917. Go's untyped constants 8865794286000691 << 39 and ...692 << 39 are exact in f64.
    let two39 = (1u64 << 39) as f64;
    let cases = [
        (8865794286000691.0_f64 * two39, "4.87402195346389e+27"),
        (8865794286000692.0_f64 * two39, "4.8740219534638903e+27"),
    ];
    for (f, s) in cases {
        assert_eq!(format_float(f, b'g'), s, "FormatFloat({:b})", f.to_bits());
        assert_eq!(parse_float(s), Ok(f), "ParseFloat({s})");
    }
}
