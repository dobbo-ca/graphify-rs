//! Ported from encoding/json/stream_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2010 The Go
//! Authors). stream_test.go has 13 Test funcs: 6 ported, 7 not ported. Go 1.27.4 runs v2_stream_test.go
//! (GOEXPERIMENT=jsonv2): the same encoder rows, plus TestDecoderMaxBytesError (Decoder, not ported).
//! - TestEncoder (stream_test.go:72): ported (`Value` for streamTest, `Encoder` over a `Vec<u8>`)
//! - TestEncoderErrorAndReuseEncodeState (stream_test.go:92): ported, adapted (the pointer cycle is not
//!   expressible; a NaN `UnsupportedValue` error stands in, then the same Data is encoded; the
//!   Unmarshal round trip is Task 10, "decode: graphify-rs-aa6.10")
//! - TestEncoderIndent (stream_test.go:146): ported
//! - TestEncoderSetEscapeHTML (stream_test.go:171): ported (5 of 6 rows; each phase uses a fresh `Encoder`
//!   because the writer is borrowed, `set_escape_html(false)` is set before the second encode)
//! - TestRawMessage (stream_test.go:308): ported (marshal half, then the Unmarshal half as
//!   `test_raw_message_decode`, ignored until graphify-rs-aa6.10, from the active v2_stream_test.go:290,
//!   identical; `float32` Y is an `f64`, no `Marshal for f32`)
//! - TestNullRawMessage (stream_test.go:332): ported (marshal half, then `test_null_raw_message_decode`,
//!   ignored until graphify-rs-aa6.10, from the active v2_stream_test.go:314, identical; same reasons)
//! - TestDecoder (stream_test.go:240): not ported: `Decoder` is not used by graphify-go
//! - TestDecoderBuffered (stream_test.go:272): not ported: `Decoder`/`Buffered` not used by graphify-go
//! - TestBlocking (stream_test.go:359): not ported: `Decoder` over `net.Pipe`, not used by graphify-go
//! - TestDecodeInStream (stream_test.go:388): not ported: `Decoder.Token`, not used by graphify-go
//! - TestHTTPDecoding (stream_test.go:505): not ported: `Decoder` over HTTP, not used by graphify-go
//! - TestTokenError (stream_test.go:545): not ported: `Decoder.Token`, not used by graphify-go
//! - TestDecoderInputOffset (stream_test.go:592): not ported: `Decoder.InputOffset`, not used by graphify-go
//! - marshal_indent_prefix: added (no Go source), the plan's Task 9 case; expected strings checked against
//!   `appendIndent` (v2_indent.go:91, `jsontext` `AppendIndent`): the prefix after every `\n`, never on the first line
//!
//! streamTest has 8 values (stream_test.go:51): 8 kept, 0 left out. streamEncoded and streamEncodedIndent
//! are kept whole. TestEncoderSetEscapeHTML has 6 rows (stream_test.go:198-215): 5 kept, 1 left out:
//! - stringOption (`json:"bar,string"`): the `,string` tag is not reached
//!
//! The tagStruct `Invalid` field has the tag `json:"\\"`, which Go rejects as a key (`isValidTag`), so its key
//! is the field name `Invalid`; that is the key given to `json_fields!`.

use gostd::json::{
    EncodeState, Encoder, Error, Marshal, RawMessage, Value, marshal, marshal_indent, unmarshal,
};
use std::collections::BTreeMap;

// Go streamTest (stream_test.go:51); the map keys are "K" (U+212A KELVIN SIGN) and "ß".
// 3.14 is Go's value, not an approximation of PI.
#[allow(clippy::approx_constant)]
fn stream_test() -> Vec<Value> {
    vec![
        Value::Number(0.1),
        Value::String("hello".into()),
        Value::Null,
        Value::Bool(true),
        Value::Bool(false),
        Value::Array(vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]),
        Value::Object(BTreeMap::from([
            ("\u{212A}".to_string(), Value::String("Kelvin".into())),
            ("ß".to_string(), Value::String("long s".into())),
        ])),
        Value::Number(3.14),
    ]
}

const STREAM_ENCODED: &str = "0.1\n\"hello\"\nnull\ntrue\nfalse\n[\"a\",\"b\",\"c\"]\n{\"ß\":\"long s\",\"\u{212A}\":\"Kelvin\"}\n3.14\n";

const STREAM_ENCODED_INDENT: &str = "0.1\n\"hello\"\nnull\ntrue\nfalse\n[\n>.\"a\",\n>.\"b\",\n>.\"c\"\n>]\n{\n>.\"ß\": \"long s\",\n>.\"\u{212A}\": \"Kelvin\"\n>}\n3.14\n";

// Go nlines (stream_test.go:294): the first n lines of s.
fn nlines(s: &str, n: usize) -> String {
    s.split_inclusive('\n').take(n).collect()
}

#[test]
fn test_encoder() {
    let values = stream_test();
    for i in 0..=values.len() {
        let mut buf = Vec::new();
        {
            let mut enc = Encoder::new(&mut buf);
            // Check that set_indent("", "") turns off indentation.
            enc.set_indent(">", ".");
            enc.set_indent("", "");
            for (j, v) in values[..i].iter().enumerate() {
                enc.encode(v)
                    .unwrap_or_else(|e| panic!("#{i}.{j} Encode error: {e}"));
            }
        }
        let got = String::from_utf8(buf).unwrap();
        assert_eq!(got, nlines(STREAM_ENCODED, i), "encoding {i} items");
    }
}

struct Data {
    a: String,
    i: i64,
}
gostd::json_fields!(Data, "json.Data", { a => "A", i => "I" });

#[test]
fn test_encoder_error_and_reuse_encode_state() {
    let mut buf = Vec::new();
    {
        let mut enc = Encoder::new(&mut buf);
        let err = enc
            .encode(&f64::NAN)
            .expect_err("Encode(NaN) error: got nil, want non-nil");
        assert_eq!(err, Error::UnsupportedValue("NaN".into()));
        let want = Data {
            a: "a".into(),
            i: 1,
        };
        enc.encode(&want).expect("Marshal error");
    }
    // Go compares the Unmarshal round trip (Task 10, "decode: graphify-rs-aa6.10"); a failed
    // Encode must leave no bytes in the writer, and the next Encode must be clean.
    assert_eq!(String::from_utf8(buf).unwrap(), "{\"A\":\"a\",\"I\":1}\n");
}

#[test]
fn test_encoder_indent() {
    let mut buf = Vec::new();
    {
        let mut enc = Encoder::new(&mut buf);
        enc.set_indent(">", ".");
        for v in &stream_test() {
            let _ = enc.encode(v);
        }
    }
    assert_eq!(String::from_utf8(buf).unwrap(), STREAM_ENCODED_INDENT);
}

// Go C (encode_test.go:554): MarshalJSON returns `"<&>"`.
struct C;
impl Marshal for C {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_marshaler_output("json.C", b"\"<&>\"")
    }
}

// Go CText (encode_test.go:561): MarshalText returns `"<&>"`, encoded as a JSON string.
struct CText;
impl Marshal for CText {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_string(b"\"<&>\"");
        Ok(())
    }
}

// Go strMarshaler and strPtrMarshaler (stream_test.go:159, 165): MarshalJSON returns the string as is.
struct StrMarshaler(&'static str);
impl Marshal for StrMarshaler {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_marshaler_output("json.strMarshaler", self.0.as_bytes())
    }
}
struct StrPtrMarshaler(&'static str);
impl Marshal for StrPtrMarshaler {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_marshaler_output("json.strPtrMarshaler", self.0.as_bytes())
    }
}

struct TagStruct {
    valid: i64,
    invalid: i64,
}
gostd::json_fields!(TagStruct, "struct", { valid => "<>&#! ", invalid => "Invalid" });

struct MarshalerStruct {
    non_ptr: StrMarshaler,
    ptr: StrPtrMarshaler,
}
gostd::json_fields!(MarshalerStruct, "struct", { non_ptr => "NonPtr", ptr => "Ptr" });

// (name, value, wantEscape, want)
type EscapeRow = (&'static str, Box<dyn Marshal>, &'static str, &'static str);

#[test]
fn test_encoder_set_escape_html() {
    let tests: Vec<EscapeRow> = vec![
        ("c", Box::new(C), r#""\u003c\u0026\u003e""#, r#""<&>""#),
        (
            "ct",
            Box::new(CText),
            r#""\"\u003c\u0026\u003e\"""#,
            r#""\"<&>\"""#,
        ),
        (
            "\"<&>\"",
            Box::new("<&>".to_string()),
            r#""\u003c\u0026\u003e""#,
            r#""<&>""#,
        ),
        (
            "tagStruct",
            Box::new(TagStruct {
                valid: 0,
                invalid: 0,
            }),
            r#"{"\u003c\u003e\u0026#! ":0,"Invalid":0}"#,
            r#"{"<>&#! ":0,"Invalid":0}"#,
        ),
        (
            "\"<str>\"",
            Box::new(MarshalerStruct {
                non_ptr: StrMarshaler("\"<str>\""),
                ptr: StrPtrMarshaler("\"<str>\""),
            }),
            r#"{"NonPtr":"\u003cstr\u003e","Ptr":"\u003cstr\u003e"}"#,
            r#"{"NonPtr":"<str>","Ptr":"<str>"}"#,
        ),
    ];
    for (name, v, want_escape, want) in &tests {
        let mut buf = Vec::new();
        Encoder::new(&mut buf)
            .encode(&**v)
            .unwrap_or_else(|e| panic!("Encode({name}) error: {e}"));
        let got = String::from_utf8(buf).unwrap();
        assert_eq!(got.trim(), *want_escape, "Encode({name})");

        let mut buf = Vec::new();
        let mut enc = Encoder::new(&mut buf);
        enc.set_escape_html(false);
        enc.encode(&**v)
            .unwrap_or_else(|e| panic!("SetEscapeHTML(false) Encode({name}) error: {e}"));
        drop(enc);
        let got = String::from_utf8(buf).unwrap();
        assert_eq!(got.trim(), *want, "SetEscapeHTML(false) Encode({name})");
    }
}

struct RawData {
    x: f64,
    id: RawMessage,
    y: f64,
}
gostd::json_fields!(RawData, "struct", { x => "X", id => "Id", y => "Y" });

#[test]
fn test_raw_message() {
    let data = RawData {
        x: 0.1,
        id: RawMessage(br#"["\u0056",null]"#.to_vec()),
        y: 0.2,
    };
    let got = marshal(&data).expect("Marshal error");
    assert_eq!(
        String::from_utf8(got).unwrap(),
        r#"{"X":0.1,"Id":["\u0056",null],"Y":0.2}"#
    );
}

struct NullRawData {
    x: f64,
    id: RawMessage,
    id_ptr: Option<RawMessage>,
    y: f64,
}
gostd::json_fields!(NullRawData, "struct", { x => "X", id => "Id", id_ptr => "IdPtr", y => "Y" });

#[test]
fn test_null_raw_message() {
    let data = NullRawData {
        x: 0.1,
        id: RawMessage(b"null".to_vec()),
        id_ptr: None,
        y: 0.2,
    };
    let got = marshal(&data).expect("Marshal error");
    assert_eq!(
        String::from_utf8(got).unwrap(),
        r#"{"X":0.1,"Id":null,"IdPtr":null,"Y":0.2}"#
    );
}

#[test]
fn marshal_indent_prefix() {
    let m = BTreeMap::from([
        ("a".to_string(), "1".to_string()),
        ("b".to_string(), "2".to_string()),
    ]);
    let got = marshal_indent(&m, "      ", "  ").unwrap();
    assert_eq!(
        String::from_utf8(got).unwrap(),
        "{\n        \"a\": \"1\",\n        \"b\": \"2\"\n      }"
    );
    let empty = BTreeMap::<String, String>::new();
    let got = marshal_indent(&empty, "      ", "  ").unwrap();
    assert_eq!(String::from_utf8(got).unwrap(), "{}");
    let got = marshal_indent(&Vec::<String>::new(), "      ", "  ").unwrap();
    assert_eq!(String::from_utf8(got).unwrap(), "[]");
}

#[derive(Default)]
struct RawDecodeData {
    x: f64,
    id: RawMessage,
    y: f64,
}
gostd::json_fields!(decode RawDecodeData, "struct", { x => "X", id => "Id", y => "Y" });

/// TestRawMessage (v2_stream_test.go:290), Unmarshal half.
#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_raw_message_decode() {
    const RAW: &str = r#"["\u0056",null]"#;
    const WANT: &str = r#"{"X":0.1,"Id":["\u0056",null],"Y":0.2}"#;
    let data: RawDecodeData = unmarshal(WANT.as_bytes()).expect("Unmarshal error");
    assert_eq!(
        String::from_utf8(data.id.0.clone()).unwrap(),
        RAW,
        "Unmarshal Id"
    );
    let got = marshal(&data).expect("Marshal error");
    assert_eq!(String::from_utf8(got).unwrap(), WANT, "Marshal");
}

#[derive(Default)]
struct NullRawDecodeData {
    x: f64,
    id: RawMessage,
    id_ptr: Option<RawMessage>,
    y: f64,
}
gostd::json_fields!(decode NullRawDecodeData, "struct", { x => "X", id => "Id", id_ptr => "IdPtr", y => "Y" });

/// TestNullRawMessage (v2_stream_test.go:314), Unmarshal half.
#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_null_raw_message_decode() {
    const WANT: &str = r#"{"X":0.1,"Id":null,"IdPtr":null,"Y":0.2}"#;
    let data: NullRawDecodeData = unmarshal(WANT.as_bytes()).expect("Unmarshal error");
    assert_eq!(
        String::from_utf8(data.id.0.clone()).unwrap(),
        "null",
        "Unmarshal Id"
    );
    assert!(
        data.id_ptr.is_none(),
        "pointer mismatch: got non-nil, want nil"
    );
    let got = marshal(&data).expect("Marshal error");
    assert_eq!(String::from_utf8(got).unwrap(), WANT, "Marshal");
}
