use super::*;
use crate::testutil::testdata;

fn enc(s: &[u8]) -> String {
    let mut out = String::new();
    write_string(&mut out, s);
    out
}

fn str_of(src: &str) -> String {
    let v = parse(src).expect("parse");
    v.as_str().expect("string value").to_string()
}

// New: golden body for client::request_body (plan Task 3 Step 1; graphify-go bedrock.go:27-64).
#[test]
fn request_body_golden() {
    let want = std::fs::read(testdata("request_body.json")).expect("read golden");
    let got = crate::client::request_body("docs/a.md", "hello <world> & \"q\"\n".as_bytes());
    assert_eq!(
        got.as_bytes(),
        want.as_slice(),
        "request body differs from testdata/request_body.json"
    );
}

// New: cases from appendString (anthropic-sdk-go v1.51.0 internal/encoding/json/encode.go:1028-1097, escapeHTML on).
// The SDK ships no encode_test.go; each row below follows one branch of appendString.
#[test]
fn string_escapes() {
    // (case name, input, expected)
    let cases: &[(&str, &str, &str)] = &[
        ("quote", "\"", r#""\"""#),
        ("backslash", "\\", r#""\\""#),
        ("backspace", "\u{8}", r#""\b""#),
        ("form feed", "\u{c}", r#""\f""#),
        ("newline", "\n", r#""\n""#),
        ("carriage return", "\r", r#""\r""#),
        ("tab", "\t", r#""\t""#),
        ("nul", "\u{0}", r#""\u0000""#),
        ("0x1f", "\u{1f}", r#""\u001f""#),
        ("0x0b", "\u{b}", r#""\u000b""#),
        ("0x1b lowercase hex", "\u{1b}", r#""\u001b""#),
        ("less than", "<", r#""\u003c""#),
        ("greater than", ">", r#""\u003e""#),
        ("ampersand", "&", r#""\u0026""#),
        ("line separator", "\u{2028}", r#""\u2028""#),
        ("paragraph separator", "\u{2029}", r#""\u2029""#),
        ("DEL raw", "\u{7f}", "\"\u{7f}\""),
        ("slash raw", "/", "\"/\""),
        ("e acute raw", "é", "\"é\""),
        ("em dash raw", "—", "\"—\""),
        ("astral raw", "😀", "\"😀\""),
        ("plain", "abc 123", "\"abc 123\""),
        ("empty", "", "\"\""),
        ("mixed", "a<b>&\"c\"\n", r#""a\u003cb\u003e\u0026\"c\"\n""#),
    ];
    for (name, input, want) in cases {
        assert_eq!(enc(input.as_bytes()), *want, "case {name}");
    }
}

// New: encode.go:1072-1077 emits one \ufffd per invalid byte (utf8.DecodeRuneInString size 1).
#[test]
fn string_invalid_utf8_one_fffd_per_byte() {
    assert_eq!(enc(b"a\xffb\xe2\x80"), r#""a\ufffdb\ufffd\ufffd""#);
    assert_eq!(
        enc(b"\xc0\xaf"),
        r#""\ufffd\ufffd""#,
        "overlong two-byte form is two bad bytes"
    );
    assert_eq!(
        enc(b"\xed\xa0\x80"),
        r#""\ufffd\ufffd\ufffd""#,
        "encoded surrogate is three bad bytes"
    );
}

// New: write_string appends to existing output.
#[test]
fn string_appends() {
    let mut out = String::from("x:");
    write_string(&mut out, b"y");
    assert_eq!(out, "x:\"y\"");
}

// New: Value::raw returns the exact source text, whitespace inside included.
#[test]
fn parse_spans_raw_keeps_text() {
    let src = r#"{"content":[{"type":"tool_use","name":"record_concepts","input": {"concepts" : [ ]} }]}"#;
    let root = parse(src).expect("parse");
    assert_eq!(root.raw(src), src, "root span covers the whole text");
    let content = root.get("content").expect("content");
    let Kind::Array(items) = &content.kind else {
        panic!("content is not an array: {content:?}")
    };
    assert_eq!(items.len(), 1);
    let input = items[0].get("input").expect("input");
    assert_eq!(input.raw(src), r#"{"concepts" : [ ]}"#);
    assert_eq!(
        items[0].get("name").and_then(Value::as_str),
        Some("record_concepts")
    );
}

// New: leading and trailing whitespace is allowed, and is not part of the span.
#[test]
fn parse_span_excludes_outer_whitespace() {
    let src = " \n[1, 2.5e3, -0, true, false, null]\t ";
    let v = parse(src).expect("parse");
    assert_eq!(v.raw(src), "[1, 2.5e3, -0, true, false, null]");
    let Kind::Array(items) = &v.kind else {
        panic!("not an array: {v:?}")
    };
    let kinds: Vec<Kind> = items.iter().map(|i| i.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            Kind::Number("1".into()),
            Kind::Number("2.5e3".into()),
            Kind::Number("-0".into()),
            Kind::Bool(true),
            Kind::Bool(false),
            Kind::Null,
        ]
    );
}

// New: plan Task 3 parse_rejects. The plan's `""` is read as empty input.
#[test]
fn parse_rejects() {
    for (name, src) in [
        ("not json", "not json"),
        ("empty input", ""),
        ("whitespace only", "  \n"),
        ("unterminated object", r#"{"a":1"#),
        ("trailing garbage", "{} x"),
        ("two values", "1 2"),
        ("unterminated string", r#""abc"#),
        ("trailing comma", "[1,]"),
        ("bare key", "{a:1}"),
        ("leading zero", "01"),
        ("raw newline in string", "\"a\nb\""),
        ("bad escape", r#""\x""#),
        ("short unicode escape", r#""\u12""#),
    ] {
        assert!(
            parse(src).is_err(),
            "case {name}: {src:?} should be rejected"
        );
    }
}

// New: Value::get returns the first exact match; duplicate keys are kept in order.
#[test]
fn get_first_match() {
    let v = parse(r#"{"k":"first","K":"upper","k":"second"}"#).expect("parse");
    assert_eq!(v.get("k").and_then(Value::as_str), Some("first"));
    assert_eq!(v.get("K").and_then(Value::as_str), Some("upper"));
    assert!(v.get("missing").is_none());
    let Kind::Object(entries) = &v.kind else {
        panic!("not an object: {v:?}")
    };
    assert_eq!(entries.len(), 3, "duplicate keys are kept");
    let scalar = parse("1").expect("parse");
    assert!(scalar.get("k").is_none(), "get on a non-object is None");
}

// New: Go encoding/json v1 decode assigns every matching key in input order, so the last
// case-insensitive match wins (credential_process output).
#[test]
fn get_go_rules() {
    let cases: &[(&str, &str, &str)] = &[
        (
            r#"{"accesskeyid":"a","AccessKeyId":"b"}"#,
            "AccessKeyId",
            "b",
        ),
        (
            r#"{"AccessKeyId":"b","accesskeyid":"a"}"#,
            "AccessKeyId",
            "a",
        ),
        (
            r#"{"ACCESSKEYID":"a","accessKeyID":"b"}"#,
            "AccessKeyId",
            "b",
        ),
        (r#"{"AccessKeyId":"only"}"#, "AccessKeyId", "only"),
        (r#"{"x":"1","sessiontoken":"t"}"#, "SessionToken", "t"),
    ];
    for (src, key, want) in cases {
        let v = parse(src).expect("parse");
        assert_eq!(
            v.get_go(key).and_then(Value::as_str),
            Some(*want),
            "src {src} key {key}"
        );
    }
    let v = parse(r#"{"AccessKeyId":"a"}"#).expect("parse");
    assert!(v.get_go("SecretAccessKey").is_none(), "absent key");
}

// New: plan Task 3 parse_strings (Go decode rules for surrogates).
#[test]
fn parse_strings() {
    assert_eq!(str_of(r#""\ud83d\ude00""#), "\u{1F600}", "surrogate pair");
    assert_eq!(str_of("\"\u{1F600}\""), "\u{1F600}", "raw astral");
    assert_eq!(str_of(r#""\ud800""#), "\u{FFFD}", "lone high surrogate");
    assert_eq!(
        str_of(r#""\ud800x""#),
        "\u{FFFD}x",
        "high surrogate then text"
    );
    assert_eq!(str_of(r#""\udc00""#), "\u{FFFD}", "lone low surrogate");
    assert_eq!(
        str_of(r#""\ud800\u0041""#),
        "\u{FFFD}A",
        "high surrogate then non-surrogate escape"
    );
    assert_eq!(
        str_of(r#""a\"\\\/\b\f\n\r\tb""#),
        "a\"\\/\u{8}\u{c}\n\r\tb",
        "simple escapes"
    );
    assert_eq!(str_of(r#""\u00e9\u2014""#), "é—", "bmp escapes");
}

#[test]
fn parse_first_ignores_trailing() {
    // Go json.Decoder.Decode reads one value and leaves the rest (requestconfig.go:560).
    let v = parse_first(" {\"a\":1} trailing").expect("first value");
    assert_eq!(
        v.get("a").map(|n| n.kind.clone()),
        Some(Kind::Number("1".into()))
    );
    assert!(parse("{\"a\":1} trailing").is_err(), "parse stays strict");
}
