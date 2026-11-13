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
#[ignore = "graphify-rs-aa6.9"]
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
#[ignore = "graphify-rs-aa6.9"]
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
#[ignore = "graphify-rs-aa6.9"]
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
#[ignore = "graphify-rs-aa6.9"]
fn test_compact_big() {
    let big = json_big();
    let mut b = Vec::new();
    compact(&mut b, &big, false).unwrap();
    assert!(b == big, "Compact: {}", diff(&b, &big));
}

#[test]
#[ignore = "graphify-rs-aa6.9"]
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
#[ignore = "graphify-rs-aa6.9"]
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
