//! Ported from BurntSushi/toml v1.4.0 decode_test.go (MIT, Copyright (c) 2013 TOML authors, see
//! `testdata/toml/COPYING`). graphify-go decodes into `map[string]any`, so each test is expressed
//! through `decode` into `Table` / `Value`; where Go's assertion is about a struct destination
//! (`int8`, `float32`, `time.Time`) the any-destination analogue is given instead.
//! - TestDecodeFile (decode_test.go:40): ported, adapted (`decode` of the file bytes; the Go test
//!   writes `a = 42` to a temp file; `MetaData` is not reached)
//! - TestDecodeBOM (85): ported
//! - TestDecodeErrors (157): ported, adapted (rows verbatim; struct destinations give no error
//!   into `any`, so each row checks that `decode` succeeds and that `ParseError` prints Go's text)
//! - TestDecodeBadDatetime (305): ported, adapted (`T = 123` is an integer into `any`)
//! - TestDecodeIntOverflow (327): ported, adapted (`value = 500` is an integer into `any`)
//! - TestDecodeFloatOverflow (337): ported, adapted (rows verbatim; into `any` an overflow row
//!   decodes to the float or integer as written)
//! - TestDecodeSignbit (381): ported, adapted (`z1`, `z2` are integers into `any`; the encoder half
//!   is not reached)
//! - TestDecodeDatetime (718): ported, adapted (`time.Time` as `Datetime` fields; `LocalTime`
//!   date fields are not compared)
//! - array_of_tables_is_distinct: added (Review Focus 3, no Go source)
//!
//! Not ported (19 of the 27 `func Test*`; graphify-go decodes into `map[string]any` only):
//! - TestDecodeReader (27): `DecodeReader`, not reached
//! - TestDecodeFS (66): `DecodeFS`, not reached
//! - TestDecodeEmbedded (104), TestDecodeIgnoreFields (240), TestDecodePointers (256),
//!   TestDecodeArrayWrongSize (320), TestDecodeSizedInts (449), TestDecodeTypes (491),
//!   TestDecodeDoubleTags (1043): struct destinations
//! - TestUnmarshaler (541), TestCustomDecode (1019): `Unmarshaler`
//! - TestDecodePrimitive (659): `Primitive`
//! - TestDecodeTextUnmarshaler (760), TestDecodeDuration (808), TestDecodeJSONNumber (862):
//!   `TextUnmarshaler`, `time.Duration`, `json.Number` destinations
//! - TestMetaDotConflict (896), TestMetaKeys (1065): `MetaData`
//! - TestCustomEncode (993): `Marshaler`, encoder
//! - TestDecodeParallel (1095): Go reflect cache under goroutines; no shared state in `decode`

use gostd::toml::{Datetime, DatetimeKind, ParseError, Table, Value, decode};

fn table(src: &[u8]) -> Table {
    decode(src).unwrap_or_else(|e| panic!("{}: {e}", String::from_utf8_lossy(src)))
}

#[test]
fn test_decode_file() {
    let t = table(b"a = 42");
    assert_eq!(t, Table::from([("a".to_string(), Value::Integer(42))]));
}

#[test]
fn test_decode_bom() {
    for tt in [
        &b"\xff\xfea = \"b\""[..],
        &b"\xfe\xffa = \"b\""[..],
        &b"\xef\xbb\xbfa = \"b\""[..],
    ] {
        assert_eq!(
            table(tt),
            Table::from([("a".to_string(), Value::String("b".to_string()))]),
            "{tt:?}"
        );
    }
}

// (toml, last key, message, wantErr); the struct destination of each Go row is not expressible.
const DECODE_ERRORS: [(&str, &str, &str, &str); 12] = [
    (
        "V = 999",
        "V",
        "999 is out of range for int8",
        "toml: line 1 (last key \"V\"): 999 is out of range for int8",
    ),
    (
        "V = 999999999999999",
        "V",
        "999999999999999 is out of the safe float32 range",
        "toml: line 1 (last key \"V\"): 999999999999999 is out of the safe float32 range",
    ),
    (
        "V = 5",
        "V",
        "incompatible types: TOML value has type int64; destination has type string",
        "toml: line 1 (last key \"V\"): incompatible types: TOML value has type int64; destination has type string",
    ),
    (
        "V = 999",
        "V",
        "unsupported type interface { ASD() }",
        "toml: line 1 (last key \"V\"): unsupported type interface { ASD() }",
    ),
    (
        "V = 999",
        "V",
        "type mismatch for struct { V int }: expected table but found int64",
        "toml: line 1 (last key \"V\"): type mismatch for struct { V int }: expected table but found int64",
    ),
    (
        "V = [1,2,3]",
        "V",
        "expected array length 1; got TOML array of length 3",
        "toml: line 1 (last key \"V\"): expected array length 1; got TOML array of length 3",
    ),
    (
        "V.N = 999",
        "V.N",
        "999 is out of range for int8",
        "toml: line 1 (last key \"V.N\"): 999 is out of range for int8",
    ),
    (
        "V.N = 999999999999999",
        "V.N",
        "999999999999999 is out of the safe float32 range",
        "toml: line 1 (last key \"V.N\"): 999999999999999 is out of the safe float32 range",
    ),
    (
        "V.N = 5",
        "V.N",
        "incompatible types: TOML value has type int64; destination has type string",
        "toml: line 1 (last key \"V.N\"): incompatible types: TOML value has type int64; destination has type string",
    ),
    (
        "V.N = 999",
        "V.N",
        "unsupported type interface { ASD() }",
        "toml: line 1 (last key \"V.N\"): unsupported type interface { ASD() }",
    ),
    (
        "V.N = 999",
        "V.N",
        "type mismatch for struct { V int }: expected table but found int64",
        "toml: line 1 (last key \"V.N\"): type mismatch for struct { V int }: expected table but found int64",
    ),
    (
        "V.N = [1,2,3]",
        "V.N",
        "expected array length 1; got TOML array of length 3",
        "toml: line 1 (last key \"V.N\"): expected array length 1; got TOML array of length 3",
    ),
];

#[test]
fn test_decode_errors() {
    for (toml, last_key, message, want_err) in DECODE_ERRORS {
        let err = ParseError {
            message: message.to_string(),
            line: 1,
            last_key: last_key.to_string(),
        };
        assert_eq!(err.to_string(), want_err);
        table(toml.as_bytes());
    }
    // No last key: no "(last key ...)" part.
    let err = ParseError {
        message: "m".to_string(),
        line: 3,
        last_key: String::new(),
    };
    assert_eq!(err.to_string(), "toml: line 3: m");
}

#[test]
fn test_decode_bad_datetime() {
    for s in ["123", "1230"] {
        let t = table(format!("T = {s}").as_bytes());
        assert_eq!(t["T"], Value::Integer(s.parse().unwrap()), "T = {s}");
    }
}

#[test]
fn test_decode_int_overflow() {
    assert_eq!(table(b"value = 500")["value"], Value::Integer(500));
}

const MAX_SAFE_FLOAT32_INT: i64 = 16777215; // 2^24-1
const MAX_SAFE_FLOAT64_INT: i64 = 9007199254740991; // 2^53-1

#[test]
fn test_decode_float_overflow() {
    let max64 = f64::MAX;
    let max32 = f64::from(f32::MAX);
    // (input, overflow); `%f` is Rust `{:.6}`. Go computes `MaxFloat32*1.1` exactly before
    // rounding; here it is rounded twice, which can move the last digit of the input.
    let tests = [
        (format!("F32 = {max64:.6}"), true),
        (format!("F32 = {:.6}", -max64), true),
        (format!("F32 = {:.6}", max32 * 1.1), true),
        (format!("F32 = {:.6}", -max32 * 1.1), true),
        (format!("F32 = {}", MAX_SAFE_FLOAT32_INT + 1), true),
        (format!("F32 = {}", -MAX_SAFE_FLOAT32_INT - 1), true),
        (format!("F64 = {}", MAX_SAFE_FLOAT64_INT + 1), true),
        (format!("F64 = {}", -MAX_SAFE_FLOAT64_INT - 1), true),
        (format!("F32 = {max32:.6}"), false),
        (format!("F32 = {:.6}", -max32), false),
        (format!("F32 = {MAX_SAFE_FLOAT32_INT}"), false),
        (format!("F32 = {}", -MAX_SAFE_FLOAT32_INT), false),
        (format!("F64 = {max64:.6}"), false),
        (format!("F64 = {:.6}", -max64), false),
        (format!("F64 = {max32:.6}"), false),
        (format!("F64 = {:.6}", -max32), false),
        (format!("F64 = {MAX_SAFE_FLOAT64_INT}"), false),
        (format!("F64 = {}", -MAX_SAFE_FLOAT64_INT), false),
    ];
    for (value, _overflow) in tests {
        let t = table(value.as_bytes());
        let (key, rhs) = value.split_once(" = ").unwrap();
        let want = if rhs.contains('.') {
            Value::Float(rhs.parse().unwrap())
        } else {
            Value::Integer(rhs.parse().unwrap())
        };
        assert_eq!(t[key], want, "{value}");
    }
}

#[test]
fn test_decode_signbit() {
    let m = table(
        b"
n1 = nan
n2 = -nan
i1 = inf
i2 = -inf
z1 = 0
z2 = -0
zf1 = 0.0
zf2 = -0.0
",
    );
    let float = |k: &str| match m[k] {
        Value::Float(f) => f,
        ref v => panic!("{k}: not a float: {v:?}"),
    };
    assert!(float("n1").is_nan() && !float("n1").is_sign_negative());
    assert!(float("i1") == f64::INFINITY);
    assert!(float("zf1") == 0.0 && !float("zf1").is_sign_negative());

    assert!(float("n2").is_nan() && float("n2").is_sign_negative());
    assert!(float("i2") == f64::NEG_INFINITY);
    assert!(float("zf2") == 0.0 && float("zf2").is_sign_negative());

    // Integers into `any`; Go converts them to float64 and -0 is the same as 0.
    assert_eq!(m["z1"], Value::Integer(0));
    assert_eq!(m["z2"], Value::Integer(0));
}

fn dt(
    kind: DatetimeKind,
    (year, month, day): (i32, u8, u8),
    (hour, minute, second, nanosecond): (u8, u8, u8, u32),
    offset_seconds: Option<i32>,
) -> Datetime {
    Datetime {
        kind,
        year,
        month,
        day,
        hour,
        minute,
        second,
        nanosecond,
        offset_seconds,
    }
}

#[test]
fn test_decode_datetime() {
    use DatetimeKind::{LocalDate, LocalDateTime, LocalTime, OffsetDateTime};
    let tz7 = Some(-3600 * 7);
    let utc = Some(0);
    let tests = [
        // Offset datetime
        (
            "1979-05-27T07:32:00Z",
            dt(OffsetDateTime, (1979, 5, 27), (7, 32, 0, 0), utc),
        ),
        (
            "1979-05-27T07:32:00.999999Z",
            dt(OffsetDateTime, (1979, 5, 27), (7, 32, 0, 999999000), utc),
        ),
        (
            "1979-05-27T00:32:00-07:00",
            dt(OffsetDateTime, (1979, 5, 27), (0, 32, 0, 0), tz7),
        ),
        (
            "1979-05-27T00:32:00.999999-07:00",
            dt(OffsetDateTime, (1979, 5, 27), (0, 32, 0, 999999000), tz7),
        ),
        (
            "1979-05-27T00:32:00.24-07:00",
            dt(OffsetDateTime, (1979, 5, 27), (0, 32, 0, 240000000), tz7),
        ),
        (
            "1979-05-27 07:32:00Z",
            dt(OffsetDateTime, (1979, 5, 27), (7, 32, 0, 0), utc),
        ),
        (
            "1979-05-27t07:32:00z",
            dt(OffsetDateTime, (1979, 5, 27), (7, 32, 0, 0), utc),
        ),
        // Make sure the space between the datetime and "#" isn't lexed.
        (
            "1979-05-27T07:32:12-07:00  # c",
            dt(OffsetDateTime, (1979, 5, 27), (7, 32, 12, 0), tz7),
        ),
        // Local times.
        (
            "1979-05-27T07:32:00",
            dt(LocalDateTime, (1979, 5, 27), (7, 32, 0, 0), None),
        ),
        (
            "1979-05-27T07:32:00.999999",
            dt(LocalDateTime, (1979, 5, 27), (7, 32, 0, 999999000), None),
        ),
        (
            "1979-05-27T07:32:00.25",
            dt(LocalDateTime, (1979, 5, 27), (7, 32, 0, 250000000), None),
        ),
        (
            "1979-05-27",
            dt(LocalDate, (1979, 5, 27), (0, 0, 0, 0), None),
        ),
        ("07:32:00", dt(LocalTime, (0, 1, 1), (7, 32, 0, 0), None)),
        (
            "07:32:00.999999",
            dt(LocalTime, (0, 1, 1), (7, 32, 0, 999999000), None),
        ),
    ];
    for (input, mut want) in tests {
        let t = table(format!("d = {input}").as_bytes());
        let Value::Datetime(have) = &t["d"] else {
            panic!("{input}: not a datetime: {:?}", t["d"]);
        };
        if want.kind == LocalTime {
            (want.year, want.month, want.day) = (have.year, have.month, have.day);
        }
        assert_eq!(*have, want, "{input}");
    }
}

#[test]
fn array_of_tables_is_distinct() {
    // Review Focus 3: `[[x]]` is Go `[]map[string]any`, an inline array is `[]any`.
    let t = table(b"[[workspace.members]]\nname = 'a'\n");
    let Value::Table(ws) = &t["workspace"] else {
        panic!("workspace is not a table: {:?}", t["workspace"]);
    };
    assert_eq!(
        ws["members"],
        Value::ArrayOfTables(vec![Table::from([(
            "name".to_string(),
            Value::String("a".to_string())
        )])])
    );

    let t = table(b"m = [{x = 1}]\n");
    assert_eq!(
        t["m"],
        Value::Array(vec![Value::Table(Table::from([(
            "x".to_string(),
            Value::Integer(1)
        )]))])
    );

    let t = table(b"members = ['a', 1]\n");
    assert_eq!(
        t["members"],
        Value::Array(vec![Value::String("a".to_string()), Value::Integer(1)])
    );
}
