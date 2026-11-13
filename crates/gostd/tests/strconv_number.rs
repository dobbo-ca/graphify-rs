//! Port of strconv/number_test.go and TestErrorPrefixes of strconv/strconv_test.go (Go 1.27.4,
//! GOSRC = /opt/homebrew/Cellar/go/1.27.4/libexec/src).
//!
//! number_test.go:
//! TestParseBool: not ported: Bool is not in the gostd API (not reached by graphify-go)
//! TestFormatBool: not ported: Bool is not in the gostd API
//! TestAppendBool: not ported: Bool is not in the gostd API
//! TestParseComplex: not ported: Complex is not in the gostd API
//! TestParseComplexIncorrectBitSize: not ported: Complex is not in the gostd API
//! TestAtof: ported (bit size 64 only; the ParseFloat(s, 32) half is not ported: no float32)
//! TestParseUint32: ported
//! TestParseUint64: ported
//! TestParseUint64Base: ported
//! TestParseInt32: ported
//! TestParseInt64: ported
//! TestParseInt64Base: ported
//! TestParseUint: ported (the IntSize 64 arm)
//! TestParseInt: ported (the IntSize 64 arm)
//! TestAtoi: ported (the IntSize 64 arm)
//! TestParseIntBitSize: ported (the -1 row left out: bit size is u32)
//! TestParseUintBitSize: ported (the -1 row left out: bit size is u32)
//! TestParseIntBase: ported (the -1 row left out: base is u32)
//! TestParseUintBase: ported (the -1 row left out: base is u32)
//! TestNumError: ported (Go's errors.New("failed") is not a NumErrorKind; NumErrorKind::Syntax
//!   stands in, so "failed" becomes "invalid syntax" in the expected text)
//! TestNumErrorUnwrap: not ported: checks Go errors.Is(err, ErrSyntax); Rust has no error
//!   unwrap chain here, the kind is a plain field compared in every other test
//! TestFormatComplex: not ported: Complex is not in the gostd API
//! TestFormatComplexInvalidBitSize: not ported: Complex is not in the gostd API
//! TestItoa: ported as std checks (format!("{}") equals Go's string; base 10 rows only, Go
//!   FormatInt with another base is not called by graphify-go; the illegal-base panic is not
//!   ported for the same reason)
//! TestUitoa: ported as std checks (base 10 rows only)
//! TestFormatUintVarlen: ported as std checks
//!
//! strconv_test.go:
//! TestCountMallocs: not ported: Go allocation counts (testing.AllocsPerRun)
//! TestAllocationsFromBytes: not ported: Go allocation counts
//! TestErrorPrefixes: ported (Atoi, ParseFloat, ParseInt, ParseUint; the ParseBool part is
//!   not ported: Bool is not in the gostd API)
//!
//! On a Go error the value Go returns beside it is not part of the gostd API, so the tables keep
//! it (verbatim) but the checks skip it.

use gostd::strconv::*;
use std::io::Write;

use gostd::strconv::NumErrorKind::{Range, Syntax};

fn num_err(func: &'static str, num: &str, err: NumErrorKind) -> NumError {
    NumError {
        func,
        num: num.into(),
        err,
    }
}

fn check_uint(
    got: Result<u64, NumError>,
    func: &'static str,
    num: &str,
    out: u64,
    err: Option<NumErrorKind>,
) {
    match err {
        // Go's value beside an error is not part of the API.
        Some(kind) => assert_eq!(got, Err(num_err(func, num, kind)), "{num:?}"),
        None => assert_eq!(got, Ok(out), "{num:?}"),
    }
}

fn check_int(
    got: Result<i64, NumError>,
    func: &'static str,
    num: &str,
    out: i64,
    err: Option<NumErrorKind>,
) {
    match err {
        Some(kind) => assert_eq!(got, Err(num_err(func, num, kind)), "{num:?}"),
        None => assert_eq!(got, Ok(out), "{num:?}"),
    }
}

// atoftests: 20 rows, 20 kept, 0 left out. The out column is the FormatFloat(out, 'g', -1, 64)
// text, checked only when there is no error.
const ATOF_TESTS: &[(&str, &str, Option<NumErrorKind>)] = &[
    ("", "0", Some(Syntax)),
    ("1.25", "1.25", None),
    ("+1", "1", None),
    ("1x", "0", Some(Syntax)),
    ("1.1.", "0", Some(Syntax)),
    ("1e23", "1e+23", None),
    ("1E23", "1e+23", None),
    ("0x1fFe2.p0", "131042", None),
    ("0x1fFe2.P0", "131042", None),
    ("-0x2p3", "-16", None),
    ("0x0.fp4", "15", None),
    ("0x0.fp0", "0.9375", None),
    ("0x1e2", "0", Some(Syntax)),
    ("1p2", "0", Some(Syntax)),
    ("0x1p1024", "+Inf", Some(Range)),
    ("-0x1p1024", "-Inf", Some(Range)),
    (
        "0x1.fffffffffffff7fffp1023",
        "1.7976931348623157e+308",
        None,
    ),
    (
        "-0x1.fffffffffffff7fffp1023",
        "-1.7976931348623157e+308",
        None,
    ),
    ("1.797693134862315808e308", "+Inf", Some(Range)),
    ("-1.797693134862315808e308", "-Inf", Some(Range)),
];

// parseUint64Tests: 7 rows, 7 kept, 0 left out.
const PARSE_UINT64_TESTS: &[(&str, u64, Option<NumErrorKind>)] = &[
    ("", 0, Some(Syntax)),
    ("0", 0, None),
    ("1", 1, None),
    ("12345", 12345, None),
    ("012345", 12345, None),
    ("18446744073709551616", u64::MAX, Some(Range)),
    ("-1", 0, Some(Syntax)),
];

// parseUint64BaseTests: 10 rows, 10 kept, 0 left out.
const PARSE_UINT64_BASE_TESTS: &[(&str, u32, u64, Option<NumErrorKind>)] = &[
    ("", 0, 0, Some(Syntax)),
    ("0", 0, 0, None),
    ("1", 0, 1, None),
    ("-1", 0, 0, Some(Syntax)),
    ("12345", 0, 12345, None),
    ("012345", 0, 0o12345, None),
    ("18446744073709551616", 0, u64::MAX, Some(Range)),
    ("0b", 0, 0, Some(Syntax)),
    ("101", 2, 5, None),
    ("101_", 2, 0, Some(Syntax)),
];

// parseInt64Tests: 7 rows, 7 kept, 0 left out.
const PARSE_INT64_TESTS: &[(&str, i64, Option<NumErrorKind>)] = &[
    ("", 0, Some(Syntax)),
    ("0", 0, None),
    ("1", 1, None),
    ("-1", -1, None),
    ("12345", 12345, None),
    ("9223372036854775808", i64::MAX, Some(Range)),
    ("123%45", 0, Some(Syntax)),
];

// parseInt64BaseTests: 11 rows, 11 kept, 0 left out.
const PARSE_INT64_BASE_TESTS: &[(&str, u32, i64, Option<NumErrorKind>)] = &[
    ("", 0, 0, Some(Syntax)),
    ("0", 0, 0, None),
    ("1", 0, 1, None),
    ("-1", 0, -1, None),
    ("12345", 0, 12345, None),
    ("12345", 9, 8303, None),
    ("012345", 0, 0o12345, None),
    ("9223372036854775808", 10, i64::MAX, Some(Range)),
    ("0b", 0, 0, Some(Syntax)),
    ("101", 2, 5, None),
    ("101_", 2, 0, Some(Syntax)),
];

// parseUint32Tests: 9 rows, 9 kept, 0 left out.
const PARSE_UINT32_TESTS: &[(&str, u64, Option<NumErrorKind>)] = &[
    ("", 0, Some(Syntax)),
    ("0", 0, None),
    ("1", 1, None),
    ("12345", 12345, None),
    ("12345x", 0, Some(Syntax)),
    ("987654321", 987654321, None),
    ("4294967296", u32::MAX as u64, Some(Range)),
    ("1_2_3_4_5", 0, Some(Syntax)), // base=10 so no underscores allowed
    ("12345_", 0, Some(Syntax)),
];

// parseInt32Tests: 9 rows, 9 kept, 0 left out.
const PARSE_INT32_TESTS: &[(&str, i64, Option<NumErrorKind>)] = &[
    ("", 0, Some(Syntax)),
    ("0", 0, None),
    ("-0", 0, None),
    ("1", 1, None),
    ("-1", -1, None),
    ("12345", 12345, None),
    ("-12345", -12345, None),
    ("2147483648", i32::MAX as i64, Some(Range)),
    ("12345_", 0, Some(Syntax)),
];

// parseBitSizeTests: 4 rows, 3 kept, 1 left out (arg -1: bit size is u32, no negative value).
// (arg, is_error)
const PARSE_BIT_SIZE_TESTS: &[(u32, bool)] = &[(0, false), (64, false), (65, true)];

// parseBaseTests: 6 rows, 5 kept, 1 left out (arg -1: base is u32, no negative value).
const PARSE_BASE_TESTS: &[(u32, bool)] =
    &[(0, false), (1, true), (2, false), (36, false), (37, true)];

// numErrorTests: 3 rows, 3 kept, 0 left out. Go's err text "failed" becomes "invalid syntax".
const NUM_ERROR_TESTS: &[(&str, &str)] = &[
    ("0", r#"strconv.ParseFloat: parsing "0": invalid syntax"#),
    ("`", "strconv.ParseFloat: parsing \"`\": invalid syntax"),
    (
        "1\x00.2",
        r#"strconv.ParseFloat: parsing "1\x00.2": invalid syntax"#,
    ),
];

// itob64tests: 8 rows, 5 kept (base 10), 3 left out (bases 17, 25, 36: no non-decimal format).
const ITOB64_TESTS: &[(i64, &str)] = &[
    (0, "0"),
    (1, "1"),
    (-1, "-1"),
    (12345678, "12345678"),
    (-1 << 63, "-9223372036854775808"),
];

// uitob64tests: 6 rows, 5 kept (base 10), 1 left out (base 2).
const UITOB64_TESTS: &[(u64, &str)] = &[
    ((1 << 63) - 1, "9223372036854775807"),
    (1 << 63, "9223372036854775808"),
    ((1 << 63) + 1, "9223372036854775809"),
    (u64::MAX - 1, "18446744073709551614"),
    (u64::MAX, "18446744073709551615"),
];

// varlenUints: 20 rows, 20 kept, 0 left out.
const VARLEN_UINTS: &[(u64, &str)] = &[
    (1, "1"),
    (12, "12"),
    (123, "123"),
    (1234, "1234"),
    (12345, "12345"),
    (123456, "123456"),
    (1234567, "1234567"),
    (12345678, "12345678"),
    (123456789, "123456789"),
    (1234567890, "1234567890"),
    (12345678901, "12345678901"),
    (123456789012, "123456789012"),
    (1234567890123, "1234567890123"),
    (12345678901234, "12345678901234"),
    (123456789012345, "123456789012345"),
    (1234567890123456, "1234567890123456"),
    (12345678901234567, "12345678901234567"),
    (123456789012345678, "123456789012345678"),
    (1234567890123456789, "1234567890123456789"),
    (12345678901234567890, "12345678901234567890"),
];

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_atof() {
    for &(input, want, err) in ATOF_TESTS {
        let got = parse_float(input);
        match err {
            Some(kind) => assert_eq!(got, Err(num_err("ParseFloat", input, kind)), "{input:?}"),
            None => {
                let out = got.unwrap_or_else(|e| panic!("{input:?}: {e}"));
                assert_eq!(format_float(out, b'g'), want, "{input:?}");
            }
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_uint32() {
    for &(input, out, err) in PARSE_UINT32_TESTS {
        check_uint(parse_uint(input, 10, 32), "ParseUint", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_uint64() {
    for &(input, out, err) in PARSE_UINT64_TESTS {
        check_uint(parse_uint(input, 10, 64), "ParseUint", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_uint64_base() {
    for &(input, base, out, err) in PARSE_UINT64_BASE_TESTS {
        check_uint(parse_uint(input, base, 64), "ParseUint", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_int32() {
    for &(input, out, err) in PARSE_INT32_TESTS {
        check_int(parse_int(input, 10, 32), "ParseInt", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_int64() {
    for &(input, out, err) in PARSE_INT64_TESTS {
        check_int(parse_int(input, 10, 64), "ParseInt", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_int64_base() {
    for &(input, base, out, err) in PARSE_INT64_BASE_TESTS {
        check_int(parse_int(input, base, 64), "ParseInt", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_uint() {
    for &(input, out, err) in PARSE_UINT64_TESTS {
        check_uint(parse_uint(input, 10, 0), "ParseUint", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_int() {
    for &(input, out, err) in PARSE_INT64_TESTS {
        check_int(parse_int(input, 10, 0), "ParseInt", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_atoi() {
    for &(input, out, err) in PARSE_INT64_TESTS {
        check_int(atoi(input), "Atoi", input, out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_int_bit_size() {
    for &(arg, is_err) in PARSE_BIT_SIZE_TESTS {
        let want = if is_err {
            Err(num_err("ParseInt", "0", NumErrorKind::BitSize(arg)))
        } else {
            Ok(0)
        };
        assert_eq!(parse_int("0", 0, arg), want, "bit size {arg}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_uint_bit_size() {
    for &(arg, is_err) in PARSE_BIT_SIZE_TESTS {
        let want = if is_err {
            Err(num_err("ParseUint", "0", NumErrorKind::BitSize(arg)))
        } else {
            Ok(0)
        };
        assert_eq!(parse_uint("0", 0, arg), want, "bit size {arg}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_int_base() {
    for &(arg, is_err) in PARSE_BASE_TESTS {
        let want = if is_err {
            Err(num_err("ParseInt", "0", NumErrorKind::Base(arg)))
        } else {
            Ok(0)
        };
        assert_eq!(parse_int("0", arg, 0), want, "base {arg}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_parse_uint_base() {
    for &(arg, is_err) in PARSE_BASE_TESTS {
        let want = if is_err {
            Err(num_err("ParseUint", "0", NumErrorKind::Base(arg)))
        } else {
            Ok(0)
        };
        assert_eq!(parse_uint("0", arg, 0), want, "base {arg}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_num_error() {
    for &(num, want) in NUM_ERROR_TESTS {
        let err = num_err("ParseFloat", num, Syntax);
        assert_eq!(err.to_string(), want, "{num:?}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_itoa() {
    for &(n, out) in ITOB64_TESTS {
        assert_eq!(format!("{n}"), out);
        let mut x = b"abc".to_vec();
        write!(x, "{n}").unwrap();
        assert_eq!(x, format!("abc{out}").into_bytes());
        if n >= 0 {
            assert_eq!(format!("{}", n as u64), out);
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_uitoa() {
    for &(n, out) in UITOB64_TESTS {
        assert_eq!(format!("{n}"), out);
        let mut x = b"abc".to_vec();
        write!(x, "{n}").unwrap();
        assert_eq!(x, format!("abc{out}").into_bytes());
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_format_uint_varlen() {
    for &(n, out) in VARLEN_UINTS {
        assert_eq!(format!("{n}"), out);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_error_prefixes() {
    let vectors = [
        (atoi("INVALID").unwrap_err(), "Atoi"),
        (parse_float("INVALID").unwrap_err(), "ParseFloat"),
        (parse_int("INVALID", 10, 64).unwrap_err(), "ParseInt"),
        (parse_uint("INVALID", 10, 64).unwrap_err(), "ParseUint"),
    ];
    for (err, want) in vectors {
        assert_eq!(err.func, want);
    }
}
