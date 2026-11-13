//! Ported from encoding/json/encode_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2011 The Go
//! Authors), plus `TestMarshal` and `TestMarshalInvalidUTF8` from decode_test.go. encode_test.go has
//! 34 Test funcs: 14 ported (several adapted or partial, noted per entry), 20 not ported. All Go structs are Rust structs
//! with `gostd::json_fields!`, or a hand-written `Marshal` where Go has MarshalJSON/MarshalText.
//! A Go nil slice, map or pointer is an `Option`.
//!
//! encode_test.go:
//! - TestOmitEmpty (:52): ported (`Str`/`Sto` struct{} fields are the `Empty` type)
//! - TestOmitZero (:135): not ported: `omitzero`, `time.Time`, `IsZero` and interface fields are reflection-only
//! - TestOmitZeroMap (:177): not ported: `omitzero`
//! - TestOmitEmptyZero (:233): not ported: `omitzero`
//! - TestRoundtripStringTag (:267): not ported: the `,string` option
//! - TestEncodeRenamedByteSlice (:330): not ported: `[]byte` base64 and renamed byte-slice types
//! - TestSamePointerNoCycle (:393): not ported: pointer-cycle detection, no shared pointers in the Rust API
//! - TestSliceNoCycle (:399): not ported: slice-cycle detection
//! - TestUnsupportedValues (:405): ported, 3 of 8 rows (NaN, -Inf, +Inf); the 5 cycle rows are pointer/map/slice cycles
//! - TestUnsupportedValueErrorValue (:433): not ported: `Error::UnsupportedValue` holds the text, not a
//!   `reflect.Value`, so `.Value` and its named float types do not exist
//! - TestMarshalTextFloatMap (:470): not ported: float map keys through `MarshalText` (non-string map keys)
//! - TestRefValMarshal (:523): not ported: pointer- versus value-receiver Marshaler method sets
//! - TestMarshalerEscaping (:567): ported
//! - TestAnonymousFields (:589): not ported: embedded/anonymous struct promotion
//! - TestNilMarshal (:811): ported, 12 of 13 rows (`[]byte(nil)` is base64 `[]byte`); a typed nil
//!   pointer behind an interface is `NilJsonMarshaler(None)`/`NilTextMarshaler(None)`
//! - TestEmbeddedBug (:844): not ported: embedded struct promotion
//! - TestTaggedFieldDominates (:884): not ported: embedded struct promotion
//! - TestDuplicatedFieldDisappears (:907): not ported: embedded struct promotion
//! - TestIssue10281 (:927): not ported: `json.Number` has no counterpart in the stub API
//! - TestMarshalErrorAndReuseEncodeState (:938): ported, adapted (the pointer cycle is not expressible, a NaN
//!   `UnsupportedValue` error stands in; the Unmarshal round trip is Task 10, so the second Marshal is
//!   compared as text)
//! - TestHTMLEscape (:973): ported (`html_escape`)
//! - TestEncodePointerString (:984): not ported: the `,string` option
//! - TestEncodeString (:1045): ported, 32 of 32 rows (encodeStringTests)
//! - TestEncodeBytekind (:1086): not ported: byte-kind slices and arrays of Marshaler bytes (`[]byte` base64)
//! - TestTextMarshalerMapKeysAreSorted (:1120): ported, adapted (Go keys are `unmarshalerText` structs; there
//!   is no map-key trait, so the keys are the `MarshalText` strings in a `HashMap<String, _>`), 1 of 1
//! - TestNilMarshalerTextMapKey (:1137): not ported: pointer map keys
//! - TestStringKindTextMarshalerMapKey (:1164): ported, adapted (string key used directly, value through
//!   `MarshalText`)
//! - TestMarshalFloat (:1199): ported, the 64-bit half of the sweep (`float32`, bits == 32, is not reached);
//!   the 15 `badFloatREs` that apply to 64 bits are hand-coded (std only, no regex)
//! - TestMarshalRawMessageValue (:1282): ported, 36 of 54 rows; 18 `rawEmpty` rows left out (see the gap in
//!   `test_marshal_raw_message_value`)
//! - TestMarshalPanic (:1390): ported, adapted (a panicking `Marshal` impl, `catch_unwind` and the payload)
//! - TestMarshalUncommonFieldNames (:1400): ported
//! - TestMarshalerError (:1415): ported, 2 of 2 rows (the `Error` Display texts)
//! - TestIssue63379 (:1450): ported (a MarshalJSON result with trailing `<`, `>`, `&`, U+2028, U+2029
//!   after the value is an error: the marshaler path validates before it compacts)
//! - TestSynctestMarshal (:1473): not ported: `testing/synctest` bubbles
//!
//! decode_test.go:
//! - TestMarshal (:1336): ported, adapted (`All` without `IntStr`, `UintptrStr` and `ByteSlice`, which need
//!   `,string` and `[]byte` base64; Int8, Int16 and Uint8, Uint16 are `i64`/`u64`, Float32 is an `f64`, since
//!   the stub has no `Marshal` for `i8`, `i16`, `u8`, `u16`, `f32`; the expected text drops the same three
//!   keys), 2 of 2 values (allValue, pallValue)
//! - TestMarshalInvalidUTF8 (:1358): ported through `append_string` with HTML escape on (a Rust `str` cannot
//!   hold invalid UTF-8, so `marshal` cannot take these inputs), 6 of 6 rows
//!
//! Added (no Go source): invalid_utf8_per_byte, float_format (the plan's Task 9 cases).

use gostd::json::{
    EncodeState, Error, Marshal, RawMessage, Value, append_float, append_string, html_escape,
    marshal, marshal_indent,
};
use std::collections::{BTreeMap, HashMap};
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

fn strip_whitespace(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '\n' | '\r' | '\t'))
        .collect()
}

fn text<T: Marshal + ?Sized>(v: &T) -> String {
    String::from_utf8(marshal(v).expect("Marshal error")).expect("Marshal output is UTF-8")
}

// ---- TestOmitEmpty ----

/// Go `struct{}`.
struct Empty;

impl Marshal for Empty {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_raw(b"{}");
        Ok(())
    }
}

struct OptionalsEmpty {
    sr: String,
    so: String,
    sw: String,
    ir: i64,
    io: i64,
    slr: Option<Vec<String>>,
    slo: Option<Vec<String>>,
    mr: BTreeMap<String, Value>,
    mo: BTreeMap<String, Value>,
    fr: f64,
    fo: f64,
    br: bool,
    bo: bool,
    ur: u64,
    uo: u64,
    str_: Empty,
    sto: Empty,
}

gostd::json_fields!(OptionalsEmpty, "json.OptionalsEmpty", {
    sr => "sr",
    so => "so" omitempty,
    sw => "-",
    ir => "omitempty",
    io => "io" omitempty,
    slr => "slr",
    slo => "slo" omitempty,
    mr => "mr",
    mo => "Mo" omitempty,
    fr => "fr",
    fo => "fo" omitempty,
    br => "br",
    bo => "bo" omitempty,
    ur => "ur",
    uo => "uo" omitempty,
    str_ => "str",
    sto => "sto" omitempty,
});

#[test]
fn test_omit_empty() {
    const WANT: &str = r#"{
 "sr": "",
 "omitempty": 0,
 "slr": null,
 "mr": {},
 "fr": 0,
 "br": false,
 "ur": 0,
 "str": {},
 "sto": {}
}"#;
    let o = OptionalsEmpty {
        sr: String::new(),
        so: String::new(),
        sw: "something".into(),
        ir: 0,
        io: 0,
        slr: None,
        slo: None,
        mr: BTreeMap::new(),
        mo: BTreeMap::new(),
        fr: 0.0,
        fo: 0.0,
        br: false,
        bo: false,
        ur: 0,
        uo: 0,
        str_: Empty,
        sto: Empty,
    };
    let got = marshal_indent(&o, "", " ").expect("MarshalIndent error");
    assert_eq!(String::from_utf8(got).unwrap(), WANT);
}

// ---- TestUnsupportedValues ----

#[test]
fn test_unsupported_values() {
    // 3 of 8 rows; pointerCycle, pointerCycleIndirect, mapCycle, sliceCycle, recursiveSliceCycle are cycles
    for f in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
        match marshal(&f) {
            Err(Error::UnsupportedValue(_)) => {}
            Err(e) => panic!("Marshal({f}) error: got {e:?}, want UnsupportedValue"),
            Ok(b) => panic!("Marshal({f}) error: got nil, want non-nil ({b:?})"),
        }
    }
}

// ---- TestMarshalerEscaping ----

/// Go `C`: MarshalJSON returns unescaped JSON.
struct C;

impl Marshal for C {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_marshaler_output("json.C", br#""<&>""#)
    }
}

/// Go `CText`: MarshalText returns unescaped text.
struct CText;

impl Marshal for CText {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_string(br#""<&>""#);
        Ok(())
    }
}

#[test]
fn test_marshaler_escaping() {
    assert_eq!(text(&C), r#""\u003c\u0026\u003e""#);
    assert_eq!(text(&CText), r#""\"\u003c\u0026\u003e\"""#);
}

// ---- TestNilMarshal ----

/// Go `nilJSONMarshaler`: MarshalJSON is called even on a nil receiver (`None`).
struct NilJsonMarshaler(Option<String>);

impl Marshal for NilJsonMarshaler {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        let b = match &self.0 {
            None => marshal("0zenil0")?,
            Some(s) => marshal(&format!("zenil:{s}"))?,
        };
        e.write_marshaler_output("json.nilJSONMarshaler", &b)
    }
}

/// Go `nilTextMarshaler`: MarshalText is called even on a nil receiver (`None`).
struct NilTextMarshaler(Option<String>);

impl Marshal for NilTextMarshaler {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        match &self.0 {
            None => e.write_string(b"0zenil0"),
            Some(s) => e.write_string(format!("zenil:{s}").as_bytes()),
        }
        Ok(())
    }
}

struct MString {
    m: String,
}
gostd::json_fields!(MString, "struct { M string }", { m => "M" });

struct MJson {
    m: NilJsonMarshaler,
}
gostd::json_fields!(MJson, "struct { M json.Marshaler }", { m => "M" });

struct MOptJson {
    m: Option<NilJsonMarshaler>,
}
gostd::json_fields!(MOptJson, "struct { M json.Marshaler }", { m => "M" });

struct MText {
    m: NilTextMarshaler,
}
gostd::json_fields!(MText, "struct { M encoding.TextMarshaler }", { m => "M" });

struct MOptText {
    m: Option<NilTextMarshaler>,
}
gostd::json_fields!(MOptText, "struct { M encoding.TextMarshaler }", { m => "M" });

#[test]
fn test_nil_marshal() {
    // 12 of 13 rows; []byte(nil) is left out (base64 []byte)
    assert_eq!(text(&None::<i64>), "null"); // nil
    assert_eq!(text(&Some(0.0_f64)), "0"); // new(float64)
    assert_eq!(text(&None::<Vec<Value>>), "null"); // []any(nil)
    assert_eq!(text(&None::<Vec<String>>), "null"); // []string(nil)
    assert_eq!(text(&None::<BTreeMap<String, String>>), "null"); // map[string]string(nil)
    assert_eq!(text(&MString { m: "gopher".into() }), r#"{"M":"gopher"}"#);
    assert_eq!(text(&MOptJson { m: None }), r#"{"M":null}"#); // struct{ M Marshaler }{}
    assert_eq!(
        text(&MJson {
            m: NilJsonMarshaler(None)
        }),
        r#"{"M":"0zenil0"}"#
    ); // struct{ M Marshaler }{(*nilJSONMarshaler)(nil)}
    assert_eq!(text(&MOptJson { m: None }), r#"{"M":null}"#); // struct{ M any }{(*nilJSONMarshaler)(nil)}
    assert_eq!(text(&MOptText { m: None }), r#"{"M":null}"#); // struct{ M encoding.TextMarshaler }{}
    assert_eq!(
        text(&MText {
            m: NilTextMarshaler(None)
        }),
        r#"{"M":"0zenil0"}"#
    ); // struct{ M encoding.TextMarshaler }{(*nilTextMarshaler)(nil)}
    assert_eq!(text(&MOptText { m: None }), r#"{"M":null}"#); // struct{ M any }{(*nilTextMarshaler)(nil)}
}

// ---- TestMarshalErrorAndReuseEncodeState ----

struct Data {
    a: String,
    i: i64,
}
gostd::json_fields!(Data, "json.Data", { a => "A", i => "I" });

#[test]
fn test_marshal_error_and_reuse_encode_state() {
    // Go triggers the error with cyclic data; a NaN stands in.
    assert!(
        marshal(&f64::NAN).is_err(),
        "Marshal error: got nil, want non-nil"
    );
    let want = Data {
        a: "a".into(),
        i: 1,
    };
    assert_eq!(text(&want), r#"{"A":"a","I":1}"#);
}

// ---- TestHTMLEscape ----

#[test]
fn test_html_escape() {
    let m = "{\"M\":\"<html>foo &\u{2028} \u{2029}</html>\"}";
    let want = r#"{"M":"\u003chtml\u003efoo \u0026\u2028 \u2029\u003c/html\u003e"}"#;
    let mut b = Vec::new();
    html_escape(&mut b, m.as_bytes());
    assert_eq!(String::from_utf8(b).unwrap(), want);
}

// ---- TestEncodeString ----

const ENCODE_STRING_TESTS: &[(&str, &str)] = &[
    ("\x00", r#""\u0000""#),
    ("\x01", r#""\u0001""#),
    ("\x02", r#""\u0002""#),
    ("\x03", r#""\u0003""#),
    ("\x04", r#""\u0004""#),
    ("\x05", r#""\u0005""#),
    ("\x06", r#""\u0006""#),
    ("\x07", r#""\u0007""#),
    ("\x08", r#""\b""#),
    ("\x09", r#""\t""#),
    ("\x0a", r#""\n""#),
    ("\x0b", r#""\u000b""#),
    ("\x0c", r#""\f""#),
    ("\x0d", r#""\r""#),
    ("\x0e", r#""\u000e""#),
    ("\x0f", r#""\u000f""#),
    ("\x10", r#""\u0010""#),
    ("\x11", r#""\u0011""#),
    ("\x12", r#""\u0012""#),
    ("\x13", r#""\u0013""#),
    ("\x14", r#""\u0014""#),
    ("\x15", r#""\u0015""#),
    ("\x16", r#""\u0016""#),
    ("\x17", r#""\u0017""#),
    ("\x18", r#""\u0018""#),
    ("\x19", r#""\u0019""#),
    ("\x1a", r#""\u001a""#),
    ("\x1b", r#""\u001b""#),
    ("\x1c", r#""\u001c""#),
    ("\x1d", r#""\u001d""#),
    ("\x1e", r#""\u001e""#),
    ("\x1f", r#""\u001f""#),
];

#[test]
fn test_encode_string() {
    for &(inp, out) in ENCODE_STRING_TESTS {
        let b = marshal(inp).unwrap_or_else(|e| panic!("Marshal({inp:?}) error: {e}"));
        assert_eq!(String::from_utf8(b).unwrap(), out, "Marshal({inp:?})");
    }
}

// ---- TestTextMarshalerMapKeysAreSorted, TestStringKindTextMarshalerMapKey ----

#[test]
fn test_text_marshaler_map_keys_are_sorted() {
    // Go keys are unmarshalerText{A, B}, MarshalText is A + ":" + B
    let m: HashMap<String, i64> = HashMap::from([
        ("x:y".to_string(), 1),
        ("y:x".to_string(), 2),
        ("a:z".to_string(), 3),
        ("z:a".to_string(), 4),
    ]);
    assert_eq!(text(&m), r#"{"a:z":3,"x:y":1,"y:x":2,"z:a":4}"#);
}

/// Go `textMarshalerString`: MarshalText is "X_" + s.
struct TextMarshalerString(String);

impl Marshal for TextMarshalerString {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_string(format!("X_{}", self.0).as_bytes());
        Ok(())
    }
}

#[test]
fn test_string_kind_text_marshaler_map_key() {
    let m: HashMap<String, TextMarshalerString> =
        HashMap::from([("foo".to_string(), TextMarshalerString("bar".into()))]);
    assert_eq!(text(&m), r#"{"foo":"X_bar"}"#);
}

// ---- TestMarshalFloat ----

/// The `badFloatREs` that apply to 64-bit floats (the first 15 of Go's 17); returns the regexp source
/// of the first one that matches.
fn bad_float(out: &str) -> Option<&'static str> {
    let b = out.as_bytes();
    let t = out.strip_prefix('-').unwrap_or(out).as_bytes(); // the `^-?` prefix
    let dig = |c: u8| c.is_ascii_digit();
    let nz = |c: u8| (b'1'..=b'9').contains(&c);
    if b.contains(&b'p') {
        return Some("p");
    }
    if b.first() == Some(&b'+') {
        return Some(r"^\+");
    }
    if t.len() >= 2 && t[0] == b'0' && t[1] != b'.' {
        return Some("^-?0[^.]");
    }
    if t.first() == Some(&b'.') {
        return Some(r"^-?\.");
    }
    for (i, _) in b.iter().enumerate().filter(|&(_, &c)| c == b'.') {
        // \.(e|$)
        if b.get(i + 1).is_none_or(|&c| c == b'e') {
            return Some(r"\.(e|$)");
        }
        // \.[0-9]+0(e|$): the digit run after the point ends in 0 and is followed by e or the end
        let j = i + 1 + b[i + 1..].iter().take_while(|&&c| dig(c)).count();
        if j - (i + 1) >= 2 && b[j - 1] == b'0' && b.get(j).is_none_or(|&c| c == b'e') {
            return Some(r"\.[0-9]+0(e|$)");
        }
    }
    // ^-?(0|[0-9]{2,})\..*e
    let n = t.iter().take_while(|&&c| dig(c)).count();
    if n < t.len()
        && t[n] == b'.'
        && (n >= 2 || (n == 1 && t[0] == b'0'))
        && t[n + 1..].contains(&b'e')
    {
        return Some(r"^-?(0|[0-9]{2,})\..*e");
    }
    if b.windows(2).any(|w| w[0] == b'e' && dig(w[1])) {
        return Some("e[0-9]");
    }
    if b.windows(3)
        .any(|w| w[0] == b'e' && (w[1] == b'+' || w[1] == b'-') && w[2] == b'0')
    {
        return Some("e[+-]0");
    }
    if b.len() >= 3
        && b[b.len() - 3..b.len() - 1] == *b"e-"
        && (b'1'..=b'6').contains(&b[b.len() - 1])
    {
        return Some("e-[1-6]$");
    }
    // e+(.|1.|20)$ as written in Go: `e+` is one or more `e`, not an escaped plus
    for (i, _) in b.iter().enumerate().filter(|&(_, &c)| c == b'e') {
        let rest = &b[i + 1..];
        if rest.len() == 1 || (rest.len() == 2 && rest[0] == b'1') || rest == b"20" {
            return Some("e+(.|1.|20)$");
        }
    }
    if t.starts_with(b"0.0000000") {
        return Some(r"^-?0\.0000000");
    }
    if t.len() >= 22 && t[..22].iter().all(|&c| dig(c)) {
        return Some("^-?[0-9]{22}");
    }
    if b.windows(18)
        .any(|w| nz(w[0]) && w[1..17].iter().all(|&c| dig(c)) && nz(w[17]))
    {
        return Some("[1-9][0-9]{16}[1-9]");
    }
    if b.windows(19)
        .any(|w| nz(w[0]) && w[1..18].iter().all(|&c| dig(c) || c == b'.') && nz(w[18]))
    {
        return Some("[1-9][0-9.]{17}[1-9]");
    }
    None
}

/// Go `math.Nextafter(f, +Inf)` for finite `f`.
fn next_up(f: f64) -> f64 {
    if f == 0.0 {
        f64::from_bits(1)
    } else if f > 0.0 {
        f64::from_bits(f.to_bits() + 1)
    } else {
        f64::from_bits(f.to_bits() - 1)
    }
}

/// One call of Go's `test` closure in TestMarshalFloat (64 bits); failures are collected.
fn check_float(f: f64, fails: &mut Vec<String>) {
    let bout = match marshal(&f) {
        Ok(b) => b,
        Err(e) => {
            fails.push(format!("Marshal({f:e}) error: {e}"));
            return;
        }
    };
    let out = String::from_utf8(bout.clone()).unwrap();

    // append_float is the same encoder
    let mut d = Vec::new();
    append_float(&mut d, f).unwrap();
    if d != bout {
        fails.push(format!("append_float({f:e}) = {d:?}, marshal = {out:?}"));
        return;
    }

    // result must convert back to the same float (bits compare handles +-0)
    match out.parse::<f64>() {
        Err(e) => {
            fails.push(format!("ParseFloat({out:?}) error: {e}"));
            return;
        }
        Ok(g) if g.to_bits() != f.to_bits() => {
            fails.push(format!("ParseFloat({out:?}): got {g:e}, want {f:e}"));
            return;
        }
        Ok(_) => {}
    }

    if let Some(re) = bad_float(&out) {
        fails.push(format!("Marshal({f:e}) = {out:?}; must not match /{re}/"));
    }
}

#[test]
fn test_marshal_float() {
    let mut fails: Vec<String> = Vec::new();
    let digits = "1.2345678901234567890123";
    'sweep: for i in (2..=digits.len()).rev() {
        for exp in -30..=30 {
            for sign in ['+', '-'] {
                let s = format!("{sign}{}e{exp}", &digits[..i]);
                let f: f64 = s.parse().unwrap();
                check_float(f, &mut fails);
                check_float(next_up(f), &mut fails);
                check_float(-next_up(-f), &mut fails);
                if fails.len() > 50 {
                    break 'sweep;
                }
            }
        }
    }
    check_float(0.0, &mut fails);
    check_float(-0.0, &mut fails);
    assert!(
        fails.is_empty(),
        "{} failures:\n{}",
        fails.len(),
        fails.join("\n")
    );
}

// ---- TestMarshalRawMessageValue ----

struct S1 {
    m: RawMessage,
}
gostd::json_fields!(S1, "struct { M json.RawMessage }", { m => "M" });

struct S2 {
    m: Option<RawMessage>,
}
gostd::json_fields!(S2, "struct { M *json.RawMessage }", { m => "M" });

struct T1 {
    m: RawMessage,
}
gostd::json_fields!(T1, "json.T1", { m => "M" omitempty });

struct T2 {
    m: Option<RawMessage>,
}
gostd::json_fields!(T2, "json.T2", { m => "M" omitempty });

/// A Go value row and its `&value` row.
fn check_and_ptr<T: Marshal>(v: &T, want: &str) {
    assert_eq!(text(v), want);
    let p = &v;
    assert_eq!(text(p), want);
}

/// The 18 rows of one Go RawMessage group: `[]any{x}` is `Vec<RawMessage>`, `[]any{&x}` is
/// `Vec<&RawMessage>`, `*RawMessage` fields are `Option<RawMessage>`, `map[string]any` is a map of
/// `RawMessage` or `&RawMessage`.
fn raw_rows(raw: &RawMessage, scalar: &str, arr: &str, obj: &str, t1: &str, t2: &str) {
    check_and_ptr(raw, scalar); // x, &x
    check_and_ptr(&vec![raw.clone()], arr); // []any{x}, &[]any{x}
    check_and_ptr(&vec![raw], arr); // []any{&x}, &[]any{&x}
    check_and_ptr(&S1 { m: raw.clone() }, obj); // struct{ M RawMessage }, &struct
    check_and_ptr(
        &S2 {
            m: Some(raw.clone()),
        },
        obj,
    ); // struct{ M *RawMessage }{&x}, &struct
    check_and_ptr(&BTreeMap::from([("M".to_string(), raw.clone())]), obj); // map[string]any{"M": x}
    check_and_ptr(&BTreeMap::from([("M".to_string(), raw)]), obj); // map[string]any{"M": &x}
    check_and_ptr(&T1 { m: raw.clone() }, t1); // T1{x}, &T1{x}
    check_and_ptr(
        &T2 {
            m: Some(raw.clone()),
        },
        t2,
    ); // T2{&x}, &T2{&x}
}

#[test]
fn test_marshal_raw_message_value() {
    // Gap: Go's rawNil (`RawMessage(nil)`, marshals as `null`) and rawEmpty (`RawMessage([]byte{})`, an
    // error) are the same `RawMessage(vec![])` here, so the 18 rawEmpty rows (all `ok == false`, or `{}`
    // for T1) are left out. rawNil is the empty value; 18 + 18 of 54 rows are kept.
    raw_rows(
        &RawMessage(Vec::new()),
        "null",
        "[null]",
        r#"{"M":null}"#,
        "{}",
        r#"{"M":null}"#,
    );
    // Issue6458 rows are in this group
    raw_rows(
        &RawMessage(br#""foo""#.to_vec()),
        r#""foo""#,
        r#"["foo"]"#,
        r#"{"M":"foo"}"#,
        r#"{"M":"foo"}"#,
        r#"{"M":"foo"}"#,
    );
}

// ---- TestMarshalPanic ----

struct MarshalPanic;

impl Marshal for MarshalPanic {
    fn marshal_json(&self, _e: &mut EncodeState) -> Result<(), Error> {
        panic_any(0xdead_i32)
    }
}

#[test]
fn test_marshal_panic() {
    match catch_unwind(AssertUnwindSafe(|| marshal(&MarshalPanic))) {
        Err(got) => assert_eq!(got.downcast_ref::<i32>(), Some(&0xdead), "panic() payload"),
        Ok(_) => panic!("Marshal should have panicked"),
    }
}

// ---- TestMarshalUncommonFieldNames ----

struct Uncommon {
    a0: i64,
    a_grave: i64,
    a_beta: i64,
}
gostd::json_fields!(Uncommon, "struct { A0, À, Aβ int }", {
    a0 => "A0",
    a_grave => "À",
    a_beta => "Aβ",
});

#[test]
fn test_marshal_uncommon_field_names() {
    let v = Uncommon {
        a0: 0,
        a_grave: 0,
        a_beta: 0,
    };
    assert_eq!(text(&v), r#"{"A0":0,"À":0,"Aβ":0}"#);
}

// ---- TestMarshalerError ----

#[test]
fn test_marshaler_error() {
    const ERR_TEXT: &str = "json: test error";
    // reflect.TypeOf("test variable").String() is "string"
    let tests = [
        (
            Error::Marshaler {
                go_type: "string".into(),
                source_func: "",
                err: ERR_TEXT.into(),
            },
            format!("json: error calling MarshalJSON for type string: {ERR_TEXT}"),
        ),
        (
            Error::Marshaler {
                go_type: "string".into(),
                source_func: "TestMarshalerError",
                err: ERR_TEXT.into(),
            },
            format!("json: error calling TestMarshalerError for type string: {ERR_TEXT}"),
        ),
    ];
    for (err, want) in tests {
        assert_eq!(err.to_string(), want);
    }
}

// ---- decode_test.go: TestMarshal, TestMarshalInvalidUTF8 ----

#[derive(Clone)]
struct Small {
    tag: String,
}
gostd::json_fields!(Small, "json.Small", { tag => "Tag" });

// Go `All` without IntStr, UintptrStr (the `,string` option) and ByteSlice (`[]byte` base64). Int8, Int16,
// Uint8, Uint16 are 64-bit and Float32 is an f64: the stub has no Marshal for the narrow types and the
// values print the same. A Go nil map, slice or pointer is `None`; `*map` and `*[]T` are `Option<map>`
// and `Option<Vec>`; `**Small` is `Option<Option<Small>>`; `any` is `Value`.
struct All {
    bool: bool,
    int: i64,
    int8: i64,
    int16: i64,
    int32: i32,
    int64: i64,
    uint: u64,
    uint8: u64,
    uint16: u64,
    uint32: u32,
    uint64: u64,
    uintptr: usize,
    float32: f64,
    float64: f64,
    foo: String,
    foo2: String,
    pbool: Option<bool>,
    pint: Option<i64>,
    pint8: Option<i64>,
    pint16: Option<i64>,
    pint32: Option<i32>,
    pint64: Option<i64>,
    puint: Option<u64>,
    puint8: Option<u64>,
    puint16: Option<u64>,
    puint32: Option<u32>,
    puint64: Option<u64>,
    puintptr: Option<usize>,
    pfloat32: Option<f64>,
    pfloat64: Option<f64>,
    string: String,
    pstring: Option<String>,
    map: Option<BTreeMap<String, Small>>,
    map_p: Option<BTreeMap<String, Option<Small>>>,
    pmap: Option<BTreeMap<String, Small>>,
    pmap_p: Option<BTreeMap<String, Option<Small>>>,
    empty_map: Option<BTreeMap<String, Small>>,
    nil_map: Option<BTreeMap<String, Small>>,
    slice: Option<Vec<Small>>,
    slice_p: Option<Vec<Option<Small>>>,
    pslice: Option<Vec<Small>>,
    pslice_p: Option<Vec<Option<Small>>>,
    empty_slice: Option<Vec<Small>>,
    nil_slice: Option<Vec<Small>>,
    string_slice: Option<Vec<String>>,
    small: Small,
    psmall: Option<Small>,
    ppsmall: Option<Option<Small>>,
    interface: Value,
    pinterface: Option<Value>,
    invalid_embed: i64,
}

gostd::json_fields!(All, "json.All", {
    bool => "Bool",
    int => "Int",
    int8 => "Int8",
    int16 => "Int16",
    int32 => "Int32",
    int64 => "Int64",
    uint => "Uint",
    uint8 => "Uint8",
    uint16 => "Uint16",
    uint32 => "Uint32",
    uint64 => "Uint64",
    uintptr => "Uintptr",
    float32 => "Float32",
    float64 => "Float64",
    foo => "bar",
    foo2 => "bar2",
    pbool => "PBool",
    pint => "PInt",
    pint8 => "PInt8",
    pint16 => "PInt16",
    pint32 => "PInt32",
    pint64 => "PInt64",
    puint => "PUint",
    puint8 => "PUint8",
    puint16 => "PUint16",
    puint32 => "PUint32",
    puint64 => "PUint64",
    puintptr => "PUintptr",
    pfloat32 => "PFloat32",
    pfloat64 => "PFloat64",
    string => "String",
    pstring => "PString",
    map => "Map",
    map_p => "MapP",
    pmap => "PMap",
    pmap_p => "PMapP",
    empty_map => "EmptyMap",
    nil_map => "NilMap",
    slice => "Slice",
    slice_p => "SliceP",
    pslice => "PSlice",
    pslice_p => "PSliceP",
    empty_slice => "EmptySlice",
    nil_slice => "NilSlice",
    string_slice => "StringSlice",
    small => "Small",
    psmall => "PSmall",
    ppsmall => "PPSmall",
    interface => "Interface",
    pinterface => "PInterface",
    invalid_embed => "InvalidEmbed",
});

fn small(tag: &str) -> Small {
    Small { tag: tag.into() }
}

fn all_map() -> BTreeMap<String, Small> {
    BTreeMap::from([
        ("17".to_string(), small("tag17")),
        ("18".to_string(), small("tag18")),
    ])
}

fn all_map_p() -> BTreeMap<String, Option<Small>> {
    BTreeMap::from([
        ("19".to_string(), Some(small("tag19"))),
        ("20".to_string(), None),
    ])
}

fn all_slice() -> Vec<Small> {
    vec![small("tag20"), small("tag21")]
}

fn all_slice_p() -> Vec<Option<Small>> {
    vec![Some(small("tag22")), None, Some(small("tag23"))]
}

/// Go's zero `All`.
fn blank() -> All {
    All {
        bool: false,
        int: 0,
        int8: 0,
        int16: 0,
        int32: 0,
        int64: 0,
        uint: 0,
        uint8: 0,
        uint16: 0,
        uint32: 0,
        uint64: 0,
        uintptr: 0,
        float32: 0.0,
        float64: 0.0,
        foo: String::new(),
        foo2: String::new(),
        pbool: None,
        pint: None,
        pint8: None,
        pint16: None,
        pint32: None,
        pint64: None,
        puint: None,
        puint8: None,
        puint16: None,
        puint32: None,
        puint64: None,
        puintptr: None,
        pfloat32: None,
        pfloat64: None,
        string: String::new(),
        pstring: None,
        map: None,
        map_p: None,
        pmap: None,
        pmap_p: None,
        empty_map: None,
        nil_map: None,
        slice: None,
        slice_p: None,
        pslice: None,
        pslice_p: None,
        empty_slice: None,
        nil_slice: None,
        string_slice: None,
        small: small(""),
        psmall: None,
        ppsmall: None,
        interface: Value::Null,
        pinterface: None,
        invalid_embed: 0,
    }
}

fn all_value() -> All {
    All {
        bool: true,
        int: 2,
        int8: 3,
        int16: 4,
        int32: 5,
        int64: 6,
        uint: 7,
        uint8: 8,
        uint16: 9,
        uint32: 10,
        uint64: 11,
        uintptr: 12,
        float32: 14.1,
        float64: 15.1,
        foo: "foo".into(),
        foo2: "foo2".into(),
        string: "16".into(),
        map: Some(all_map()),
        map_p: Some(all_map_p()),
        empty_map: Some(BTreeMap::new()),
        slice: Some(all_slice()),
        slice_p: Some(all_slice_p()),
        empty_slice: Some(Vec::new()),
        string_slice: Some(vec!["str24".into(), "str25".into(), "str26".into()]),
        small: small("tag30"),
        psmall: Some(small("tag31")),
        interface: Value::Number(5.2),
        invalid_embed: 123,
        ..blank()
    }
}

/// Go `pallValue`: every pointer set to the matching `allValue` field.
fn pall_value() -> All {
    All {
        pbool: Some(true),
        pint: Some(2),
        pint8: Some(3),
        pint16: Some(4),
        pint32: Some(5),
        pint64: Some(6),
        puint: Some(7),
        puint8: Some(8),
        puint16: Some(9),
        puint32: Some(10),
        puint64: Some(11),
        puintptr: Some(12),
        pfloat32: Some(14.1),
        pfloat64: Some(15.1),
        pstring: Some("16".into()),
        pmap: Some(all_map()),
        pmap_p: Some(all_map_p()),
        pslice: Some(all_slice()),
        pslice_p: Some(all_slice_p()),
        ppsmall: Some(Some(small("tag31"))),
        pinterface: Some(Value::Number(5.2)),
        ..blank()
    }
}

const ALL_VALUE_INDENT: &str = r#"{
	"Bool": true,
	"Int": 2,
	"Int8": 3,
	"Int16": 4,
	"Int32": 5,
	"Int64": 6,
	"Uint": 7,
	"Uint8": 8,
	"Uint16": 9,
	"Uint32": 10,
	"Uint64": 11,
	"Uintptr": 12,
	"Float32": 14.1,
	"Float64": 15.1,
	"bar": "foo",
	"bar2": "foo2",
	"PBool": null,
	"PInt": null,
	"PInt8": null,
	"PInt16": null,
	"PInt32": null,
	"PInt64": null,
	"PUint": null,
	"PUint8": null,
	"PUint16": null,
	"PUint32": null,
	"PUint64": null,
	"PUintptr": null,
	"PFloat32": null,
	"PFloat64": null,
	"String": "16",
	"PString": null,
	"Map": {
		"17": {
			"Tag": "tag17"
		},
		"18": {
			"Tag": "tag18"
		}
	},
	"MapP": {
		"19": {
			"Tag": "tag19"
		},
		"20": null
	},
	"PMap": null,
	"PMapP": null,
	"EmptyMap": {},
	"NilMap": null,
	"Slice": [
		{
			"Tag": "tag20"
		},
		{
			"Tag": "tag21"
		}
	],
	"SliceP": [
		{
			"Tag": "tag22"
		},
		null,
		{
			"Tag": "tag23"
		}
	],
	"PSlice": null,
	"PSliceP": null,
	"EmptySlice": [],
	"NilSlice": null,
	"StringSlice": [
		"str24",
		"str25",
		"str26"
	],
	"Small": {
		"Tag": "tag30"
	},
	"PSmall": {
		"Tag": "tag31"
	},
	"PPSmall": null,
	"Interface": 5.2,
	"PInterface": null,
	"InvalidEmbed": 123
}"#;

const PALL_VALUE_INDENT: &str = r#"{
	"Bool": false,
	"Int": 0,
	"Int8": 0,
	"Int16": 0,
	"Int32": 0,
	"Int64": 0,
	"Uint": 0,
	"Uint8": 0,
	"Uint16": 0,
	"Uint32": 0,
	"Uint64": 0,
	"Uintptr": 0,
	"Float32": 0,
	"Float64": 0,
	"bar": "",
	"bar2": "",
	"PBool": true,
	"PInt": 2,
	"PInt8": 3,
	"PInt16": 4,
	"PInt32": 5,
	"PInt64": 6,
	"PUint": 7,
	"PUint8": 8,
	"PUint16": 9,
	"PUint32": 10,
	"PUint64": 11,
	"PUintptr": 12,
	"PFloat32": 14.1,
	"PFloat64": 15.1,
	"String": "",
	"PString": "16",
	"Map": null,
	"MapP": null,
	"PMap": {
		"17": {
			"Tag": "tag17"
		},
		"18": {
			"Tag": "tag18"
		}
	},
	"PMapP": {
		"19": {
			"Tag": "tag19"
		},
		"20": null
	},
	"EmptyMap": null,
	"NilMap": null,
	"Slice": null,
	"SliceP": null,
	"PSlice": [
		{
			"Tag": "tag20"
		},
		{
			"Tag": "tag21"
		}
	],
	"PSliceP": [
		{
			"Tag": "tag22"
		},
		null,
		{
			"Tag": "tag23"
		}
	],
	"EmptySlice": null,
	"NilSlice": null,
	"StringSlice": null,
	"Small": {
		"Tag": ""
	},
	"PSmall": null,
	"PPSmall": {
		"Tag": "tag31"
	},
	"Interface": null,
	"PInterface": 5.2,
	"InvalidEmbed": 0
}"#;

#[test]
fn test_marshal() {
    let want = strip_whitespace(ALL_VALUE_INDENT);
    assert_eq!(text(&all_value()), want, "Marshal(allValue)");
    let want = strip_whitespace(PALL_VALUE_INDENT);
    assert_eq!(text(&pall_value()), want, "Marshal(pallValue)");
}

#[test]
fn test_marshal_invalid_utf8() {
    // (in, want); `Marshal` of a string is appendString with HTML escape on
    let tests: &[(&[u8], &str)] = &[
        (b"hello\xffworld", r#""hello\ufffdworld""#),
        (b"", r#""""#),
        (b"\xff", r#""\ufffd""#),
        (b"\xff\xff", r#""\ufffd\ufffd""#),
        (b"a\xffb", r#""a\ufffdb""#),
        (
            b"\xe6\x97\xa5\xe6\x9c\xac\xff\xaa\x9e",
            r#""日本\ufffd\ufffd\ufffd""#,
        ),
    ];
    for &(inp, want) in tests {
        let mut got = Vec::new();
        append_string(&mut got, inp, true);
        assert_eq!(String::from_utf8(got).unwrap(), want, "Marshal({inp:?})");
    }
}

// ---- added ----

#[test]
fn invalid_utf8_per_byte() {
    // Go's DecodeRune gives (U+FFFD, 1) for each bad byte: E2 and 82 are two errors, not one.
    let mut got = Vec::new();
    append_string(&mut got, b"\xE2\x82<", true);
    assert_eq!(String::from_utf8(got).unwrap(), r#""\ufffd\ufffd\u003c""#);

    // U+2028 is escaped whether or not HTML escaping is on (appendString).
    for escape_html in [true, false] {
        let mut got = Vec::new();
        append_string(&mut got, "\u{2028}".as_bytes(), escape_html);
        assert_eq!(
            String::from_utf8(got).unwrap(),
            r#""\u2028""#,
            "escape_html {escape_html}"
        );
    }
}

#[test]
fn float_format() {
    // Checked against floatEncoder.encode (encode.go:570): 'e' format only when abs < 1e-6 or abs >= 1e21,
    // then `e-09` becomes `e-9`; `-0` takes the 'f' branch (abs == 0).
    let tests: &[(f64, &str)] = &[
        (1e-6, "0.000001"),
        (9.999999999999997e-7, "9.999999999999997e-7"),
        (1e21, "1e+21"),
        (1e-9, "1e-9"),
        (-0.0, "-0"),
    ];
    for &(f, want) in tests {
        let mut got = Vec::new();
        append_float(&mut got, f).unwrap();
        assert_eq!(String::from_utf8(got).unwrap(), want, "append_float({f:e})");
    }
    let mut got = Vec::new();
    assert_eq!(
        append_float(&mut got, f64::NAN),
        Err(Error::UnsupportedValue("NaN".into()))
    );
}

// ---- TestIssue63379 ----

/// Go `marshaledValue` (encode_test.go): a MarshalJSON that returns its own text verbatim.
struct MarshaledValue(&'static str);

impl Marshal for MarshaledValue {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_marshaler_output("json.marshaledValue", self.0.as_bytes())
    }
}

#[test]
fn test_issue63379() {
    for v in [
        "[]<",
        "[]>",
        "[]&",
        "[]\u{2028}",
        "[]\u{2029}",
        "{}<",
        "{}>",
        "{}&",
        "{}\u{2028}",
        "{}\u{2029}",
    ] {
        assert!(
            marshal(&MarshaledValue(v)).is_err(),
            "expected error for {v:?}"
        );
    }
}
