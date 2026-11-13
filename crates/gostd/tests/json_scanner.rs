//! Ported from encoding/json/scanner_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2010 The Go Authors).
//! - TestValid (scanner_test.go:31): ported (6 of 6 rows)
//! - TestCompactAndIndent (scanner_test.go:53): ported (10 of 10 rows; the 4 checks per row are kept)
//! - TestCompactSeparators (scanner_test.go:113): ported (2 of 2 rows)
//! - TestCompactBig (scanner_test.go:137): ported, adapted: Go builds `jsonBig` with the unseeded global
//!   `math/rand` (`genValue(10000)`), which cannot be reproduced; the generator is ported with the same
//!   structure (genValue/genString/genArray/genMap) on a fixed-seed local xorshift64* and Box-Muller
//!   normals, and the check shape is Go's (Compact of compact output is the identity)
//! - TestIndentBig (scanner_test.go:151): ported, adapted the same way (expands, idempotent, Compact
//!   round-trips)
//! - TestIndentErrors (scanner_test.go:190): ported (2 of 2 rows); Go's `reflect.DeepEqual` on the
//!   `SyntaxError` is the `Error::Syntax` equality (msg and offset) plus the `Display` text. Go passes
//!   silently if Indent returns no error; here an error is required.
//!
//! Rows kept: 20 of 20 (6 + 10 + 2 + 2), none left out. Go's `Indent`/`Compact` write to a `bytes.Buffer`;
//! here `indent`/`compact` append to a `Vec<u8>` (`Compact` is `compact(.., false)`, no HTML escaping).
//!
//! Go 1.27.4 runs v2_scanner_test.go (GOEXPERIMENT=jsonv2); its rows and expectations are the
//! same as scanner_test.go (only a `%v` in a failure message differs, v2_scanner_test.go:205),
//! so no row changed. Added: indent_close_after_top_level_value, compact_and_indent_errors (rows from
//! v2_decode_test.go:480-496 and jsontext/encode_test.go:393, the rest traced), max_depth,
//! indent_placeholder_patch. The decoder texts of `Valid` are tested in src/json/scanner.rs.

use gostd::json::{Error, Value, compact, indent, marshal, valid};
use std::collections::BTreeMap;

fn compact_str(s: &str) -> Result<String, Error> {
    let mut buf = Vec::new();
    compact(&mut buf, s.as_bytes(), false)?;
    Ok(String::from_utf8(buf).unwrap())
}

fn indent_str(s: &str, prefix: &str, ind: &str) -> Result<String, Error> {
    let mut buf = Vec::new();
    indent(&mut buf, s.as_bytes(), prefix, ind)?;
    Ok(String::from_utf8(buf).unwrap())
}

// (data, ok): Go TestValid tests
const VALID_TESTS: &[(&str, bool)] = &[
    ("foo", false),
    ("}{", false),
    ("{]", false),
    ("{}", true),
    (r#"{"foo":"bar"}"#, true),
    (r#"{"foo":"bar","bar":{"baz":["qux"]}}"#, true),
];

#[test]
fn test_valid() {
    for (data, ok) in VALID_TESTS {
        assert_eq!(valid(data.as_bytes()), *ok, "Valid(`{data}`)");
    }
}

// (compact, indent): Go TestCompactAndIndent tests
const COMPACT_AND_INDENT: &[(&str, &str)] = &[
    ("1", "1"),
    ("{}", "{}"),
    ("[]", "[]"),
    (r#"{"":2}"#, "{\n\t\"\": 2\n}"),
    ("[3]", "[\n\t3\n]"),
    ("[1,2,3]", "[\n\t1,\n\t2,\n\t3\n]"),
    (r#"{"x":1}"#, "{\n\t\"x\": 1\n}"),
    (
        r#"[true,false,null,"x",1,1.5,0,-5e+2]"#,
        "[\n\ttrue,\n\tfalse,\n\tnull,\n\t\"x\",\n\t1,\n\t1.5,\n\t0,\n\t-5e+2\n]",
    ),
    (
        "{\"\":\"<>&\u{2028}\u{2029}\"}",
        "{\n\t\"\": \"<>&\u{2028}\u{2029}\"\n}",
    ), // See golang.org/issue/34070
    ("null", "null \n\r\t"), // See golang.org/issue/13520 and golang.org/issue/74806
];

#[test]
fn test_compact_and_indent() {
    for (i, (c, ind)) in COMPACT_AND_INDENT.iter().enumerate() {
        assert_eq!(
            compact_str(c).as_deref(),
            Ok(*c),
            "row {i}: Compact(compact)"
        );
        assert_eq!(
            compact_str(ind).as_deref(),
            Ok(*c),
            "row {i}: Compact(indent)"
        );
        assert_eq!(
            indent_str(ind, "", "\t").as_deref(),
            Ok(*ind),
            "row {i}: Indent(indent)"
        );
        assert_eq!(
            indent_str(c, "", "\t").as_deref(),
            Ok(ind.trim_end_matches([' ', '\n', '\r', '\t'])),
            "row {i}: Indent(compact)"
        );
    }
}

// (in, compact): Go TestCompactSeparators tests. U+2028 and U+2029 should be escaped inside
// strings. They should not appear outside strings.
const COMPACT_SEPARATORS: &[(&str, &str)] = &[
    ("{\"\u{2028}\": 1}", "{\"\u{2028}\":1}"),
    ("{\"\u{2029}\" :2}", "{\"\u{2029}\":2}"),
];

#[test]
fn test_compact_separators() {
    for (i, (input, want)) in COMPACT_SEPARATORS.iter().enumerate() {
        assert_eq!(compact_str(input).as_deref(), Ok(*want), "row {i}");
    }
}

// Tests of a large random structure.

/// Fixed-seed xorshift64* standing in for Go's global `math/rand`.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Go `rand.Intn`.
    fn intn(&mut self, n: u64) -> u64 {
        (self.next() >> 11) % n
    }

    /// Go `rand.NormFloat64` (Box-Muller; Go uses the ziggurat, same distribution).
    fn norm(&mut self) -> f64 {
        let u1 = 1.0 - (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        let u2 = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    // Go genValue
    fn value(&mut self, n: usize) -> Value {
        if n > 1 {
            match self.intn(2) {
                0 => return self.array(n),
                _ => return self.map(n),
            }
        }
        match self.intn(3) {
            0 => Value::Bool(self.intn(2) == 0),
            1 => Value::Number(self.norm()),
            _ => Value::String(self.string(30.0)),
        }
    }

    // Go genString
    fn string(&mut self, stddev: f64) -> String {
        let n = (self.norm() * stddev + stddev / 2.0).abs() as usize;
        (0..n)
            .map(|_| {
                let f = (self.norm() * 64.0 + 32.0).abs().min(0x10ffff as f64);
                char::from_u32(f as u32).unwrap_or('\u{FFFD}')
            })
            .collect()
    }

    // Go genArray
    fn array(&mut self, n: usize) -> Value {
        let f = ((self.norm().abs() * 10f64.min((n / 2) as f64)) as usize).clamp(1, n);
        let mut x = Vec::with_capacity(f);
        for i in 0..f {
            x.push(self.value(((i + 1) * n) / f - (i * n) / f));
        }
        Value::Array(x)
    }

    // Go genMap
    fn map(&mut self, n: usize) -> Value {
        let mut f = (self.norm().abs() * 10f64.min((n / 2) as f64)) as usize;
        f = f.min(n);
        if n > 0 && f == 0 {
            f = 1;
        }
        let mut x = BTreeMap::new();
        for i in 0..f {
            let k = self.string(10.0);
            x.insert(k, self.value(((i + 1) * n) / f - (i * n) / f));
        }
        Value::Object(x)
    }
}

// Go initBig (n = 10000, not -short)
fn json_big() -> Vec<u8> {
    marshal(&Gen(0x9e37_79b9_7f4a_7c15).value(10000)).unwrap()
}

// Go diff: first divergence
fn diff(a: &[u8], b: &[u8]) -> String {
    let i = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let j = i.saturating_sub(10);
    let t = |s: &[u8]| String::from_utf8_lossy(&s[j..s.len().min(j + 20)]).into_owned();
    format!("diverge at {i}: «{}» vs «{}»", t(a), t(b))
}

#[test]
fn test_compact_big() {
    let big = json_big();
    let mut b = Vec::new();
    compact(&mut b, &big, false).unwrap();
    assert!(b == big, "Compact: {}", diff(&b, &big));
}

#[test]
fn test_indent_big() {
    let big = json_big();
    let mut b = Vec::new();
    indent(&mut b, &big, "", "\t").unwrap();
    // jsonBig is compact (no unnecessary spaces); indenting should make it bigger
    assert_ne!(b.len(), big.len(), "Indent did not expand the input");

    // should be idempotent
    let mut b1 = Vec::new();
    indent(&mut b1, &b, "", "\t").unwrap();
    assert!(
        b1 == b,
        "Indent(Indent(jsonBig)) != Indent(jsonBig): {}",
        diff(&b1, &b)
    );

    // should get back to original
    let mut b1 = Vec::new();
    compact(&mut b1, &b, false).unwrap();
    assert!(
        b1 == big,
        "Compact(Indent(jsonBig)) != jsonBig: {}",
        diff(&b1, &big)
    );
}

// (in, error message, offset): Go TestIndentErrors tests; offset is Go's len64 of the text read
const INDENT_ERRORS: &[(&str, &str, i64)] = &[
    (
        r#"{"X": "foo", "Y"}"#,
        "invalid character '}' after object key",
        r#"{"X": "foo", "Y"}"#.len() as i64,
    ),
    (
        r#"{"X": "foo" "Y": "bar"}"#,
        "invalid character '\"' after object key:value pair",
        r#"{"X": "foo" ""#.len() as i64,
    ),
];

#[test]
fn test_indent_errors() {
    for (input, msg, offset) in INDENT_ERRORS {
        let want = Error::Syntax {
            msg: msg.to_string(),
            offset: *offset,
        };
        let err = indent_str(input, "", "").unwrap_err();
        assert_eq!(err, want, "Indent({input})");
        assert_eq!(err.to_string(), *msg, "Indent({input}) text");
    }
}

/// Added (not a Go test): a `}` or `]` after a complete top-level value is "after top-level
/// value" (jsontext/encode.go:548-552) and `dst` is left as it was (v2_indent.go:123-125).
#[test]
fn indent_close_after_top_level_value() {
    for (src, c) in [("{}}", '}'), ("1]", ']'), ("[]]", ']')] {
        let mut dst = b"x".to_vec();
        let err = gostd::json::indent(&mut dst, src.as_bytes(), "", "\t").unwrap_err();
        assert_eq!(
            err,
            gostd::json::Error::Syntax {
                msg: format!("invalid character '{c}' after top-level value"),
                offset: src.len() as i64,
            }
        );
        assert_eq!(dst, b"x");
    }
}

/// Syntax errors of `Compact` and `Indent`, which run Go's `jsontext` encoder
/// (v2_indent.go:49-131). The texts follow `transformSyntacticError` (v2_scanner.go:56-91); the offset is past the
/// invalid text (jsontext/errors.go:102-116). Rows from v2_decode_test.go:480-496 (the decoder
/// gives the encoder's texts there); the array row from jsontext/encode_test.go:393 with the
/// legacy offset; the rest added (no Go row), each traced through jsonwire/decode.go.
#[test]
fn compact_and_indent_errors() {
    let rows: &[(&[u8], &str, i64)] = &[
        (b"", "unexpected end of JSON input", 0),
        (b" \n\r\t", "unexpected end of JSON input", 4),
        (b"[2, 3", "unexpected end of JSON input", 5),
        (
            br#"{"X":12x}"#,
            "invalid character 'x' after object key:value pair",
            8,
        ),
        (
            br#"{"F3": -}"#,
            "invalid character '}' in numeric literal",
            9,
        ),
        (
            b"\x01 42",
            "invalid character '\\x01' looking for beginning of value",
            1,
        ),
        (
            b" 42 \x01",
            "invalid character '\\x01' after top-level value",
            5,
        ),
        // jsontext/encode_test.go:393: the encoder says "after array value" where the decoder
        // says "after array element" (jsontext/encode.go:891, jsontext/decode.go:1127).
        (
            br#" [ "fizz" "buzz" ] "#,
            "invalid character '\"' after array value",
            11,
        ),
        // Added: jsonwire.ConsumeNumber (internal/jsonwire/decode.go:532, 554) reports "in number".
        (b"1.x", "invalid character 'x' in numeric literal", 3),
        (b"1e+]", "invalid character ']' in numeric literal", 4),
        // Added: a truncated number is reported where its last part starts (the encoder's
        // `resumeOffset`), with nothing added for unexpected EOF.
        (b"[1.", "unexpected end of JSON input", 2),
        // Added: escape sequences (internal/jsonwire/decode.go:182-230, wire.go:134); the offset
        // is past the sequence.
        (br#"["\x"]"#, "invalid escape sequence `\\x` in string", 4),
        (
            br#""\u12g4""#,
            "invalid escape sequence `\\u12g4` in string",
            7,
        ),
        // Added: a control byte in a string (internal/jsonwire/decode.go:243-245).
        (b"\"a\x1fb\"", "invalid character '\\x1f' in string", 3),
        // Added: a byte-order mark is one rune, quoted by strconv.QuoteRune and counted whole
        // (wire.go:62, jsontext/errors.go:106).
        (
            b"\xef\xbb\xbf{}",
            "invalid character '\\ufeff' looking for beginning of value",
            3,
        ),
        // Added: an invalid byte is quoted as '\xNN' (wire.go:64).
        (
            b"[\xff]",
            "invalid character '\\xff' looking for beginning of value",
            2,
        ),
        // Added: a literal keeps its "(expecting ...)" (v2_scanner.go:68).
        (
            b"[nul,1]",
            "invalid character ',' in literal null (expecting 'l')",
            5,
        ),
    ];
    for &(input, msg, offset) in rows {
        let want = Err(Error::Syntax {
            msg: msg.to_string(),
            offset,
        });
        let mut dst = b"x".to_vec();
        assert_eq!(compact(&mut dst, input, false), want, "Compact({input:?})");
        assert_eq!(dst, b"x", "Compact({input:?}) output");
        assert_eq!(
            indent(&mut dst, input, ">", "\t"),
            want,
            "Indent({input:?})"
        );
        assert_eq!(dst, b"x", "Indent({input:?}) output");
    }
}

/// Max depth 10000 (jsontext/state.go:53): the 10001st open bracket is "exceeded max depth",
/// one past the bracket (a structural error, jsontext/errors.go:114). Go's Unmarshal checks the
/// text in v2_decode_test.go:2990; here through Compact and Valid.
#[test]
fn max_depth() {
    let ok = format!("{}{}", "[".repeat(10000), "]".repeat(10000));
    let mut dst = Vec::new();
    assert_eq!(compact(&mut dst, ok.as_bytes(), false), Ok(()));
    assert!(valid(ok.as_bytes()));

    let deep = "[".repeat(10001);
    let mut dst = Vec::new();
    assert_eq!(
        compact(&mut dst, deep.as_bytes(), false),
        Err(Error::Syntax {
            msg: "exceeded max depth".to_string(),
            offset: 10001,
        })
    );
    assert!(!valid(deep.as_bytes()));
}

/// Added (no Go row): with a prefix or indent that is not all spaces and tabs, Go formats with
/// spaces and then overwrites every run of spaces after a newline, the trailing whitespace of
/// the input included (v2_indent.go:94-111, 127-129).
#[test]
fn indent_placeholder_patch() {
    let mut dst = Vec::new();
    indent(&mut dst, b"[1,{\"a\":2}] \n  ", ">", "\t").unwrap();
    assert_eq!(
        String::from_utf8(dst).unwrap(),
        "[\n>\t1,\n>\t{\n>\t\t\"a\": 2\n>\t}\n>] \n>\t"
    );
}
