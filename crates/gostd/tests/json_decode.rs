//! Ported from encoding/json/v2_decode_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2010 The Go
//! Authors). Go 1.27.4 builds encoding/json with GOEXPERIMENT=jsonv2 on, so v2_decode_test.go
//! (`//go:build goexperiment.jsonv2`) is the active file; decode_test.go is excluded and not cited.
//! v2_decode_test.go has 35 Test funcs: 17 ported (several adapted or partial, noted per entry),
//! 18 not ported. Go structs are Rust structs with `gostd::json_fields!(decode ...)`, or a hand-written
//! `Marshal` and `Unmarshal` where Go has `UnmarshalJSON` or `UnmarshalText`. A Go nil slice, map or
//! pointer is an `Option`; `any` is `Value`. Every test is `#[ignore]`d until graphify-rs-aa6.10.
//!
//! Types the API lacks: `Unmarshal` exists for `bool`, `i64`, `i32`, `u64`, `u32`, `usize`, `f64`,
//! `String`, `Value`, `RawMessage`, `Option`, `Vec` and string-keyed maps only, so Go `int8`, `int16`,
//! `uint8`, `uint16` and `float32` fields are `i64`, `u64` and `f64` (the values are the same).
//! `Option<T>` and `Vec<T>` need `T: Default`, so every struct derives it. A Go `*T` to a recursive
//! struct needs a `Box`, which has no `Unmarshal`; `PVOuter` below wraps it by hand.
//!
//! v2_decode_test.go:
//! - TestMarshal (:1338): not ported: already in json_encode.rs (`test_marshal`)
//! - TestMarshalInvalidUTF8 (:1360): not ported: already in json_encode.rs
//! - TestMarshalNumberZeroVal (:1383): not ported: `json.Number` has no counterpart
//! - TestMarshalEmbeds (:1395): not ported: embedded struct promotion
//! - TestUnmarshal (:1463): ported, 100 of 178 `unmarshalTests` rows (see below). The Go test decodes with
//!   a `Decoder`, which has no counterpart, so `unmarshal_into` stands in (an "unexpected end of JSON
//!   input" row is a `Syntax` error here, where Go expects `io.EOF` or `io.ErrUnexpectedEOF` from the
//!   stream, :1502-1508). Each row that decodes without error also marshals the result and decodes that
//!   again (`golden` rows must marshal back to the input text)
//! - TestUnmarshalMarshal (:1550): ported, adapted (`jsonBig` is 10,000 random leaves from `math/rand`; a
//!   seeded xorshift generator stands in, same shape as `genValue`)
//! - TestNumberAccessors (:1568): not ported: `json.Number`
//! - TestLargeByteSlice (:1601): not ported: `[]byte` is base64 in Go
//! - TestUnmarshalInterface (:1624): ported, adapted (an `any` holding a `*Xint` cannot be expressed; the
//!   decode goes straight into the struct)
//! - TestUnmarshalPtrPtr (:1635): not ported: decoding through a `**Xint` target; `Option` cannot alias
//!   the pointee
//! - TestEscape (:1646): ported (`marshal`)
//! - TestErrorMessageFromMisusedString (:1660): not ported: the `,string` option (graphify-go has no
//!   `,string` tag)
//! - TestRefUnmarshal (:2012): not ported: pointer- versus value-receiver `UnmarshalJSON`/`UnmarshalText`
//!   method sets, which Rust traits do not have
//! - TestEmptyString (:2040): ported, adapted (no `,string`: `Number1` is `1` and `Number2` is `""`)
//! - TestNullString (:2058): ported, adapted (no `,string`: `A` is `1` instead of `"1"`)
//! - TestInterfaceSet (:2083): ported, 11 of 41 rows (see below)
//! - TestUnmarshalNulls (:2190): ported, adapted (the `time.Time`, `big.Int`, `MustNotUnmarshalText`
//!   and `bytes.Buffer` fields are not expressible: std only, and no `TextUnmarshaler` target; the input
//!   still carries their `null` keys, which a struct skips as unknown keys)
//! - TestStringKind (:2325): not ported: a named string key type, a Rust map key is always `String`
//! - TestByteKind (:2345): not ported: `[]byte` base64
//! - TestSliceOfCustomByte (:2364): not ported: `[]Uint8` is base64 in Go
//! - TestUnmarshalTypeError (:2381): ported, 1 of 6 rows (the 5 `new(error)` rows decode into an
//!   interface type other than `any`)
//! - TestUnmarshalTypeErrorMessage (:2405): ported, 7 of 7 rows (`Error::UnmarshalType` `Display` only;
//!   not ignored, it needs no decoder)
//! - TestUnmarshalSyntax (:2433): ported, 8 of 8 rows
//! - TestUnmarshalUnexported (:2469): not ported: unexported fields
//! - TestUnmarshalJSONLiteralError (:2499): not ported: the expected "range" text comes from
//!   `time.Parse`; custom-unmarshaler errors are covered by the `MustNotUnmarshalJSON` row of TestUnmarshal
//! - TestSkipArrayObjects (:2512): ported, adapted (`[0]any` is a hand-written `Unmarshal` that skips
//!   every element; a fixed-size array has no counterpart)
//! - TestPrefilled (:2525): ported, 6 of 8 rows (the 2 `[...]int` rows are fixed-size arrays)
//! - TestInvalidUnmarshal (:2590): not ported: `InvalidUnmarshalError` cannot occur in Rust (the target is
//!   always a `&mut T`) and has no `Error` variant
//! - TestInvalidStringOption (:2619): not ported: the `,string` option
//! - TestUnmarshalEmbeddedUnexported (:2651): not ported: embedded structs
//! - TestUnmarshalErrorAfterMultipleJSON (:2797): ported, adapted (the Go test reads several documents
//!   from one `Decoder`; there is none, so each document is decoded alone and the failing one is a suffix
//!   of the stream: its error offset plus the suffix start is the offset Go reports), 5 of 5 rows
//! - TestUnmarshalPanic (:2842): ported (a panicking `Unmarshal` impl, `catch_unwind` and the payload)
//! - TestUnmarshalMapWithTextUnmarshalerStringKey (:2861): not ported: `TextUnmarshaler` map keys
//! - TestUnmarshalRescanLiteralMangledUnquote (:2872): ported, adapted, 2 of 3 parts (the `,string` part is
//!   left out; the map keys are plain strings, so `textUnmarshalerString` lowercasing is not applied)
//! - TestUnmarshalMaxDepth (:2917): ported, 24 of 24 cases (6 documents by 4 targets), on a thread with a
//!   large stack (9,999 levels of nesting decode recursively)
//!
//! The `UnmarshalTypeError` rows follow v2 (`v2_inject.go:69-159`): `Struct` is the root type's name
//! (empty for an unnamed root) and `Field` is the JSON Pointer of the value joined with `.`, array
//! indices and map keys included. An error returned by a hand-written `Unmarshal` (the `W` and `TOuter`
//! rows) comes back verbatim with no context added (`v2_inject.go:70-71`).
//!
//! Rows of TestUnmarshal left out (78 of 178), by class:
//! - `Number` and `UseNumber` (17): the two `Number` target rows, the `any` and `V` rows with `UseNumber`,
//!   `V` with `F3 Number`, the `ifaceNumAsNumber` row, and the 11 `#14702` rows (`Number` targets)
//! - `,string` (15): `B`, the quoted int/uint/float rows and `QuotedNull`
//! - non-string map keys (18): 11 integer-keyed rows (5 maps, 6 errors), 7 rows keyed by a
//!   `TextUnmarshaler` (`unmarshalerText`, `u8marshal`, `time.Time`) or a struct (`Point`, `unmarshaler`)
//! - embedded and anonymous structs (8): `Top` (4 rows), `S5` (2), `S10` (2)
//! - `DisallowUnknownFields` (4): the rows that differ only by that decoder option
//! - `[]byte` base64 (4): the `"AQID"` rows of the `byteWith*` types
//! - fixed-size arrays (3): `[3]int`, `[1]int`, `[5]int`
//! - unexported fields (2): `tx`
//! - `MustNotUnmarshalText` array and object errors (2, :1069-1080): the expected offset and the `Err`
//!   text "JSON value must be string type" come from v2's `TextUnmarshaler` path, which a hand-written
//!   `Unmarshal` cannot reproduce
//! - the dash tag (2): `json:"-,"`, `json_fields!` treats the key `"-"` as skip the field
//! - interface, channel, complex and function targets (3): `error`, `chan int`, the `UnsupportedTypes` row
//!
//! Rows of TestInterfaceSet left out (30 of 41): every row whose `pre` is a pointer (`*int`, `**int`,
//! `***int`, typed nil or not) in the interface; a `Value` cannot hold a typed pointer, and the decode
//! through it (`*int` becomes `*int(2)`) has no counterpart. 11 kept: the first 6 (string, map, slice),
//! `any(nil)` with `null`, `2` and `{}`, and the `(int)(1)` rows for `2` and `{}` (as `Value::Number`).
//!
//! Added: v1_compat_rules (the plan's Task 10 case, renamed from v1_rules when its BOM and depth cases
//! moved to the jsontext texts). Each case cites its Go row or is marked "traced, no Go row" with the
//! Go source it is derived from.

use gostd::json::{
    DecodeState, EncodeState, Error, Marshal, RawMessage, Unmarshal, UnmarshalTypeError, Value,
    marshal, unmarshal, unmarshal_into,
};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

fn len64(s: &str) -> i64 {
    s.len() as i64
}

fn strip_whitespace(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '\n' | '\r' | '\t'))
        .collect()
}

/// Go `&SyntaxError{msg, Offset}`.
fn syntax(msg: &str, offset: i64) -> Error {
    Error::Syntax {
        msg: msg.into(),
        offset,
    }
}

/// Go `&UnmarshalTypeError{Value, Type, Offset, Struct, Field, nil}`, `go_type` being
/// `Type.String()`. No kept row has a non-nil `Err`.
fn type_err(value: &str, go_type: &str, offset: i64, struct_name: &str, field: &str) -> Error {
    Error::UnmarshalType(Box::new(UnmarshalTypeError {
        value: value.into(),
        go_type: go_type.into(),
        offset,
        struct_name: struct_name.into(),
        field: field.into(),
        err: None,
    }))
}

/// Go `errors.New` returned by a custom unmarshaler. `Error` has no variant for an arbitrary error;
/// `Io` carries the text and passes through the decoder unchanged.
fn custom_err(text: &str) -> Error {
    Error::Io(text.into())
}

/// One `unmarshalTests` row: decode `input` into a fresh `D`, compare the error and the value (`out`
/// of `None` is the zero value, as Go's nil `out` after an error), then for a row without an error
/// marshal the result (it must equal `input` for a `golden` row) and decode that again.
fn check<D>(input: impl AsRef<[u8]>, out: Option<D>, err: Option<Error>, golden: bool)
where
    D: Unmarshal + Marshal + Default + PartialEq + Debug,
{
    let input = input.as_ref();
    let shown = String::from_utf8_lossy(input);
    let mut v = D::default();
    let got = unmarshal_into(input, &mut v).err();
    let no_error = err.is_none();
    assert_eq!(got, err, "Decode error for {shown:?}");
    assert_eq!(v, out.unwrap_or_default(), "Decode of {shown:?}");
    if no_error {
        let enc = marshal(&v).expect("Marshal error after roundtrip");
        if golden {
            assert_eq!(String::from_utf8_lossy(&enc), shown, "Marshal of {shown:?}");
        }
        let mut vv = D::default();
        unmarshal_into(&enc, &mut vv).expect("Decode error after roundtrip");
        assert_eq!(v, vv, "Decode after roundtrip of {shown:?}");
    }
}

fn ok<D>(input: impl AsRef<[u8]>, out: D)
where
    D: Unmarshal + Marshal + Default + PartialEq + Debug,
{
    check(input, Some(out), None, false);
}

fn gold<D>(input: impl AsRef<[u8]>, out: D)
where
    D: Unmarshal + Marshal + Default + PartialEq + Debug,
{
    check(input, Some(out), None, true);
}

fn fail<D>(input: impl AsRef<[u8]>, out: Option<D>, err: Error)
where
    D: Unmarshal + Marshal + Default + PartialEq + Debug,
{
    check(input, out, Some(err), false);
}

fn string(s: &str) -> Value {
    Value::String(s.into())
}

fn num(f: f64) -> Value {
    Value::Number(f)
}

fn obj<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// Go `struct{}`.
#[derive(Debug, Default, PartialEq)]
struct Empty;
gostd::json_fields!(decode Empty, "struct {}", {});

#[derive(Debug, Default, PartialEq)]
struct T {
    x: String,
    y: i64,
    z: i64,
}
gostd::json_fields!(decode T, "json.T", { x => "X", y => "Y", z => "-" });

#[derive(Debug, Default, PartialEq)]
struct U {
    alphabet: String,
}
gostd::json_fields!(decode U, "json.U", { alphabet => "alpha" });

/// Go `SS`: its `UnmarshalJSON` always returns an `UnmarshalTypeError{Value: "number"}`.
#[derive(Debug, Default, PartialEq)]
struct Ss(String);

impl Marshal for Ss {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.0.marshal_json(e)
    }
}

impl Unmarshal for Ss {
    const GO_TYPE: &'static str = "json.SS";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        d.raw_value()?;
        Err(type_err("number", Self::GO_TYPE, 0, "", ""))
    }
}

#[derive(Debug, Default, PartialEq)]
struct W {
    s: Ss,
}
gostd::json_fields!(decode W, "json.W", { s => "S" });

/// Go `TAlias`: `UnmarshalJSON` decodes into a `T` and converts.
#[derive(Debug, Default, PartialEq)]
struct TAlias(T);

impl Marshal for TAlias {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.0.marshal_json(e)
    }
}

impl Unmarshal for TAlias {
    const GO_TYPE: &'static str = "json.TAlias";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        let raw = d.raw_value()?;
        let mut t = T::default();
        unmarshal_into(raw, &mut t)?;
        self.0 = t;
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq)]
struct TOuter {
    t: TAlias,
}
gostd::json_fields!(decode TOuter, "json.TOuter", { t => "T" });

// Go `V` without `F3 Number`.
#[derive(Debug, Default, PartialEq)]
struct V {
    f1: Value,
    f2: i32,
    f4: Option<PVOuter>,
}
gostd::json_fields!(decode V, "json.V", { f1 => "F1", f2 => "F2", f4 => "F4" });

#[derive(Debug, Default, PartialEq)]
struct VOuter {
    v: V,
}
gostd::json_fields!(decode VOuter, "json.VOuter", { v => "V" });

/// Go `*VOuter` inside `V`: a boxed `VOuter`.
#[derive(Debug, Default, PartialEq)]
struct PVOuter(Box<VOuter>);

impl Marshal for PVOuter {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.0.marshal_json(e)
    }
}

impl Unmarshal for PVOuter {
    const GO_TYPE: &'static str = "json.VOuter";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        self.0.unmarshal_json(d)
    }
}

#[derive(Debug, Default, PartialEq)]
struct PP {
    t: T,
    ts: Vec<T>,
}
gostd::json_fields!(decode PP, "json.PP", { t => "T", ts => "Ts" });

#[derive(Debug, Default, PartialEq)]
struct P {
    pp: PP,
}
gostd::json_fields!(decode P, "json.P", { pp => "PP" });

/// Go `Ambig`: given "hello", the first match wins.
#[derive(Debug, Default, PartialEq)]
struct Ambig {
    first: i64,
    second: i64,
}
gostd::json_fields!(decode Ambig, "json.Ambig", { first => "HELLO", second => "Hello" });

#[derive(Debug, Default, PartialEq)]
struct DoublePtr {
    i: Option<Option<i64>>,
    j: Option<Option<i64>>,
}
gostd::json_fields!(decode DoublePtr, "json.DoublePtr", { i => "I", j => "J" });

#[derive(Debug, Default, PartialEq)]
struct MapStringToStringData {
    data: BTreeMap<String, String>,
}
gostd::json_fields!(decode MapStringToStringData, "json.mapStringToStringData", { data => "data" });

/// Go `struct{ V int }`, the anonymous type of `NestedUnamed.F` (its `Name()` is empty).
#[derive(Debug, Default, PartialEq)]
struct NestedF {
    v: i64,
}
gostd::json_fields!(decode NestedF, "struct { V int }", { v => "V" });

#[derive(Debug, Default, PartialEq)]
struct NestedUnamed {
    f: NestedF,
}
gostd::json_fields!(decode NestedUnamed, "json.NestedUnamed", { f => "F" });

/// Go `unmarshaler`: `UnmarshalJSON` only sets `T` to true.
#[derive(Debug, Default, PartialEq)]
struct Unmarshaler {
    t: bool,
}
gostd::json_fields!(Unmarshaler, "json.unmarshaler", { t => "T" });

impl Unmarshal for Unmarshaler {
    const GO_TYPE: &'static str = "json.unmarshaler";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        d.skip_value()?;
        *self = Unmarshaler { t: true };
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq)]
struct UStruct {
    m: Unmarshaler,
}
gostd::json_fields!(decode UStruct, "json.ustruct", { m => "M" });

/// Go `unmarshalerText`: `MarshalText` is `A:B`, `UnmarshalText` splits at the first `:`.
#[derive(Debug, Default, PartialEq)]
struct UnmarshalerText {
    a: String,
    b: String,
}

impl Marshal for UnmarshalerText {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_string(format!("{}:{}", self.a, self.b).as_bytes());
        Ok(())
    }
}

impl Unmarshal for UnmarshalerText {
    const GO_TYPE: &'static str = "json.unmarshalerText";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(s) = d.read_string()? {
            let Some((a, b)) = s.split_once(':') else {
                return Err(custom_err("missing separator"));
            };
            self.a = a.to_string();
            self.b = b.to_string();
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq)]
struct UStructText {
    m: UnmarshalerText,
}
gostd::json_fields!(decode UStructText, "json.ustructText", { m => "M" });

/// Go `MustNotUnmarshalJSON`.
#[derive(Debug, Default, PartialEq)]
struct MustNotUnmarshalJson;

impl Marshal for MustNotUnmarshalJson {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_raw(b"{}");
        Ok(())
    }
}

impl Unmarshal for MustNotUnmarshalJson {
    const GO_TYPE: &'static str = "json.MustNotUnmarshalJSON";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        d.raw_value()?;
        Err(custom_err("MustNotUnmarshalJSON was used"))
    }
}

// Go `byteWith*` and `intWith*`: `"Zxx"` through `MarshalJSON`/`UnmarshalJSON` or `MarshalText`/
// `UnmarshalText`. A pointer receiver (`*Ptr*`) makes no difference to a Rust trait impl.
macro_rules! z_json {
    ($name:ident, $go:literal, $int:ty) => {
        #[derive(Debug, Default, PartialEq)]
        struct $name($int);

        impl Marshal for $name {
            fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
                e.write_marshaler_output($go, format!("\"Z{:02x}\"", self.0).as_bytes())
            }
        }

        impl Unmarshal for $name {
            const GO_TYPE: &'static str = $go;
            fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
                let b = d.raw_value()?;
                if b.len() != 5 || b[0] != b'"' || b[1] != b'Z' || b[4] != b'"' {
                    return Err(custom_err("bad quoted string"));
                }
                let hex = std::str::from_utf8(&b[2..4]).map_err(|_| custom_err("bad hex"))?;
                self.0 = <$int>::from_str_radix(hex, 16).map_err(|_| custom_err("bad hex"))?;
                Ok(())
            }
        }
    };
}

macro_rules! z_text {
    ($name:ident, $go:literal, $int:ty) => {
        #[derive(Debug, Default, PartialEq)]
        struct $name($int);

        impl Marshal for $name {
            fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
                e.write_string(format!("Z{:02x}", self.0).as_bytes());
                Ok(())
            }
        }

        impl Unmarshal for $name {
            const GO_TYPE: &'static str = $go;
            fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
                let Some(s) = d.read_string()? else {
                    return Ok(());
                };
                if s.len() != 3 || !s.starts_with('Z') {
                    return Err(custom_err("bad quoted string"));
                }
                self.0 = <$int>::from_str_radix(&s[1..3], 16).map_err(|_| custom_err("bad hex"))?;
                Ok(())
            }
        }
    };
}

z_json!(ByteWithMarshalJson, "json.byteWithMarshalJSON", u8);
z_json!(ByteWithPtrMarshalJson, "json.byteWithPtrMarshalJSON", u8);
z_text!(ByteWithMarshalText, "json.byteWithMarshalText", u8);
z_text!(ByteWithPtrMarshalText, "json.byteWithPtrMarshalText", u8);
z_json!(IntWithMarshalJson, "json.intWithMarshalJSON", i64);
z_json!(IntWithPtrMarshalJson, "json.intWithPtrMarshalJSON", i64);
z_text!(IntWithMarshalText, "json.intWithMarshalText", i64);
z_text!(IntWithPtrMarshalText, "json.intWithPtrMarshalText", i64);

// ---- All, Small ----

#[derive(Clone, Debug, Default, PartialEq)]
struct Small {
    tag: String,
}
gostd::json_fields!(decode Small, "json.Small", { tag => "Tag" });

// Go `All` without IntStr, UintptrStr (the `,string` option) and ByteSlice (`[]byte` base64). Int8, Int16,
// Uint8, Uint16 are 64-bit and Float32 is an f64: the API has no Marshal or Unmarshal for the narrow types and the
// values print the same. A Go nil map, slice or pointer is `None`; `*map` and `*[]T` are `Option<map>`
// and `Option<Vec>`; `**Small` is `Option<Option<Small>>`; `any` is `Value`.
#[derive(Clone, Debug, Default, PartialEq)]
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

gostd::json_fields!(decode All, "json.All", {
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

// ---- TestUnmarshal (:1463), rows from unmarshalTests (:425) ----

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal() {
    // basic types
    ok("true", true);
    ok("1", 1_i64);
    ok("1.2", 1.2_f64);
    fail(
        "1e1000",
        None::<f64>,
        type_err("number 1e1000", "float64", len64("1e1000"), "", ""),
    );
    ok("-5", -5_i32); // int16
    ok("2", num(2.0));
    ok(r#""a\u1234""#, "a\u{1234}".to_string());
    ok(r#""http:\/\/""#, "http://".to_string());
    ok(r#""g-clef: \uD834\uDD1E""#, "g-clef: \u{1D11E}".to_string());
    ok(
        r#""invalid: \uD834x\uDD1E""#,
        "invalid: \u{FFFD}x\u{FFFD}".to_string(),
    );
    ok("null", Value::Null);
    fail(
        r#"{"X": [1,2,3], "Y": 4}"#,
        Some(T {
            y: 4,
            ..T::default()
        }),
        type_err("array", "string", len64(r#"{"X": ["#), "T", "X"),
    );
    fail::<T>(
        r#"{"X": 23}"#,
        None,
        type_err("number", "string", len64(r#"{"X": 23"#), "T", "X"),
    );
    // :455, `SS.UnmarshalJSON`'s error comes back verbatim
    fail::<W>(
        r#"{"S": 23}"#,
        None,
        type_err("number", "json.SS", 0, "", ""),
    );
    // :456, the error of the inner `Unmarshal` into `T` comes back verbatim (its offset, 8, is past
    // the `23` of the inner text `{"X": 23}`)
    fail::<TOuter>(
        r#"{"T": {"X": 23}}"#,
        None,
        type_err("number", "string", len64(r#"{"T": {""#), "T", "X"),
    );
    ok(
        r#"{"k1":1,"k2":"s","k3":[1,2.0,3e-3],"k4":{"kk1":"s","kk2":2}}"#,
        obj([
            ("k1", num(1.0)),
            ("k2", string("s")),
            ("k3", Value::Array(vec![num(1.0), num(2.0), num(3e-3)])),
            ("k4", obj([("kk1", string("s")), ("kk2", num(2.0))])),
        ]),
    );

    // raw values with whitespace
    ok("\n true ", true);
    ok("\t 1 ", 1_i64);
    ok("\r 1.2 ", 1.2_f64);
    ok("\t -5 \n", -5_i32); // int16
    ok("\t \"a\\u1234\" \n", "a\u{1234}".to_string());

    // Z has a "-" tag.
    ok(
        r#"{"Y": 1, "Z": 2}"#,
        T {
            y: 1,
            ..T::default()
        },
    );

    let abc = || U {
        alphabet: "abc".into(),
    };
    ok(r#"{"alpha": "abc", "alphabet": "xyz"}"#, abc());
    ok(r#"{"alpha": "abc"}"#, abc());
    ok(r#"{"alphabet": "xyz"}"#, U::default());

    // syntax errors
    fail::<Value>("", None, syntax("unexpected end of JSON input", 0));
    fail::<Value>(
        " \n\r\t",
        None,
        syntax("unexpected end of JSON input", len64(" \n\r\t")),
    );
    fail::<Value>(
        "[2, 3",
        None,
        syntax("unexpected end of JSON input", len64("[2, 3")),
    );
    // ptr is nil in these rows: only the syntax check applies
    fail::<Value>(
        r#"{"X": "foo", "Y"}"#,
        None,
        syntax(
            "invalid character '}' after object key",
            len64(r#"{"X": "foo", "Y"}"#),
        ),
    );
    fail::<Value>(
        "[1, 2, 3+]",
        None,
        syntax(
            "invalid character '+' after array element",
            len64("[1, 2, 3+"),
        ),
    );
    fail::<Value>(
        r#"{"X":12x}"#,
        None,
        syntax(
            "invalid character 'x' after object key:value pair",
            len64(r#"{"X":12x"#),
        ),
    );
    fail::<V>(
        r#"{"F3": -}"#,
        None,
        syntax(
            "invalid character '}' in numeric literal",
            len64(r#"{"F3": -}"#),
        ),
    );

    // raw value errors
    let begin = "invalid character '\\x01' looking for beginning of value";
    let after = "invalid character '\\x01' after top-level value";
    fail::<Value>("\x01 42", None, syntax(begin, len64("\x01")));
    fail::<Value>(" 42 \x01", None, syntax(after, len64(" 42 \x01")));
    fail::<Value>("\x01 true", None, syntax(begin, len64("\x01")));
    fail::<Value>(" false \x01", None, syntax(after, len64(" false \x01")));
    fail::<Value>("\x01 1.2", None, syntax(begin, len64("\x01")));
    fail::<Value>(" 3.4 \x01", None, syntax(after, len64(" 3.4 \x01")));
    fail::<Value>("\x01 \"string\"", None, syntax(begin, len64("\x01")));
    fail::<Value>(
        " \"string\" \x01",
        None,
        syntax(after, len64(" \"string\" \x01")),
    );

    // array tests
    fail::<MustNotUnmarshalJson>(
        "[1, 2, 3]",
        None,
        custom_err("MustNotUnmarshalJSON was used"),
    );

    // empty array to interface test
    ok("[]", Vec::<Value>::new());
    ok("null", Vec::<Value>::new());
    ok(
        r#"{"T":[]}"#,
        BTreeMap::from([("T".to_string(), Value::Array(vec![]))]),
    );
    ok(
        r#"{"T":null}"#,
        BTreeMap::from([("T".to_string(), Value::Null)]),
    );

    // composite tests
    let all_compact = strip_whitespace(ALL_VALUE_INDENT);
    let pall_compact = strip_whitespace(PALL_VALUE_INDENT);
    ok(ALL_VALUE_INDENT, all_value());
    ok(&all_compact, all_value());
    ok(ALL_VALUE_INDENT, Some(all_value()));
    ok(&all_compact, Some(all_value()));
    ok(PALL_VALUE_INDENT, pall_value());
    ok(&pall_compact, pall_value());
    ok(PALL_VALUE_INDENT, Some(pall_value()));
    ok(&pall_compact, Some(pall_value()));

    // unmarshal interface test
    let umtrue = || Unmarshaler { t: true };
    // use "false" so test will fail if custom unmarshaler is not called
    ok(r#"{"T":false}"#, umtrue());
    ok(r#"{"T":false}"#, Some(umtrue()));
    ok(r#"[{"T":false}]"#, vec![umtrue()]);
    ok(r#"[{"T":false}]"#, Some(vec![umtrue()]));
    ok(r#"{"M":{"T":"x:y"}}"#, UStruct { m: umtrue() });

    // UnmarshalText interface test
    let xy = || UnmarshalerText {
        a: "x".into(),
        b: "y".into(),
    };
    ok(r#""x:y""#, xy());
    ok(r#""x:y""#, Some(xy()));
    ok(r#"["x:y"]"#, vec![xy()]);
    ok(r#"["x:y"]"#, Some(vec![xy()]));
    ok(r#"{"M":"x:y"}"#, UStructText { m: xy() });

    ok(
        r#"{"hello": 1}"#,
        Ambig {
            first: 1,
            second: 0,
        },
    );

    ok(
        r#"{"I": 0, "I": null, "J": null}"#,
        DoublePtr { i: None, j: None },
    );

    // invalid UTF-8 is coerced to valid UTF-8.
    ok(b"\"hello\xffworld\"", "hello\u{fffd}world".to_string());
    ok(
        b"\"hello\xc2\xc2world\"",
        "hello\u{fffd}\u{fffd}world".to_string(),
    );
    ok(
        b"\"hello\xc2\xffworld\"",
        "hello\u{fffd}\u{fffd}world".to_string(),
    );
    ok(b"\"hello\\ud800world\"", "hello\u{fffd}world".to_string());
    ok(
        b"\"hello\\ud800\\ud800world\"",
        "hello\u{fffd}\u{fffd}world".to_string(),
    );
    ok(
        b"\"hello\\ud800\\ud800world\"",
        "hello\u{fffd}\u{fffd}world".to_string(),
    );
    ok(
        b"\"hello\xed\xa0\x80\xed\xb0\x80world\"",
        "hello\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}world".to_string(),
    );

    // related to issue 13783: the `["Z01","Z02","Z03"]` form of the byte and int types
    let zs = r#"["Z01","Z02","Z03"]"#;
    gold(
        zs,
        vec![
            ByteWithMarshalJson(1),
            ByteWithMarshalJson(2),
            ByteWithMarshalJson(3),
        ],
    );
    gold(
        zs,
        vec![
            ByteWithMarshalText(1),
            ByteWithMarshalText(2),
            ByteWithMarshalText(3),
        ],
    );
    gold(
        zs,
        vec![
            ByteWithPtrMarshalJson(1),
            ByteWithPtrMarshalJson(2),
            ByteWithPtrMarshalJson(3),
        ],
    );
    gold(
        zs,
        vec![
            ByteWithPtrMarshalText(1),
            ByteWithPtrMarshalText(2),
            ByteWithPtrMarshalText(3),
        ],
    );

    // ints work with the marshaler but not the base64 []byte case
    gold(
        zs,
        vec![
            IntWithMarshalJson(1),
            IntWithMarshalJson(2),
            IntWithMarshalJson(3),
        ],
    );
    gold(
        zs,
        vec![
            IntWithMarshalText(1),
            IntWithMarshalText(2),
            IntWithMarshalText(3),
        ],
    );
    gold(
        zs,
        vec![
            IntWithPtrMarshalJson(1),
            IntWithPtrMarshalJson(2),
            IntWithPtrMarshalJson(3),
        ],
    );
    gold(
        zs,
        vec![
            IntWithPtrMarshalText(1),
            IntWithPtrMarshalText(2),
            IntWithPtrMarshalText(3),
        ],
    );

    gold("0.000001", 0.000001_f64);
    gold("1e-7", 1e-7_f64);
    gold("100000000000000000000", 1e20_f64);
    gold("1e+21", 1e21_f64);
    gold("-0.000001", -0.000001_f64);
    gold("-1e-7", -1e-7_f64);
    gold("-100000000000000000000", -1e20_f64);
    gold("-1e+21", -1e21_f64);
    gold("999999999999999900000", 9.999999999999999e20_f64);
    gold("9007199254740992", 9007199254740992.0_f64);
    ok("9007199254740993", 9007199254740992.0_f64);

    // :899-910 and :911-923, `Struct` is the root `VOuter`
    fail::<VOuter>(
        r#"{"V": {"F2": "hello"}}"#,
        None,
        type_err(
            "string",
            "int32",
            len64(r#"{"V": {"F2": "hello""#),
            "VOuter",
            "V.F2",
        ),
    );
    fail(
        r#"{"V": {"F4": {}, "F2": "hello"}}"#,
        Some(VOuter {
            v: V {
                f4: Some(PVOuter(Box::default())),
                ..V::default()
            },
        }),
        type_err(
            "string",
            "int32",
            len64(r#"{"V": {"F4": {}, "F2": "hello""#),
            "VOuter",
            "V.F2",
        ),
    );

    // issue 26444: UnmarshalTypeError without field & struct values (:1053-1066; the map key is
    // on the path)
    fail(
        r#"{"data":{"test1": "bob", "test2": 123}}"#,
        Some(MapStringToStringData {
            data: BTreeMap::from([
                ("test1".to_string(), "bob".to_string()),
                ("test2".to_string(), String::new()),
            ]),
        }),
        type_err(
            "number",
            "string",
            len64(r#"{"data":{"test1": "bob", "test2": 123"#),
            "mapStringToStringData",
            "data.test2",
        ),
    );
    fail(
        r#"{"data":{"test1": 123, "test2": "bob"}}"#,
        Some(MapStringToStringData {
            data: BTreeMap::from([
                ("test1".to_string(), String::new()),
                ("test2".to_string(), "bob".to_string()),
            ]),
        }),
        type_err(
            "number",
            "string",
            len64(r#"{"data":{"test1": 123"#),
            "mapStringToStringData",
            "data.test1",
        ),
    );

    // #22369 (:1081-1105; `Struct` is the root, the array index is on the path)
    fail::<P>(
        r#"{"PP": {"T": {"Y": "bad-type"}}}"#,
        None,
        type_err(
            "string",
            "int",
            len64(r#"{"PP": {"T": {"Y": "bad-type""#),
            "P",
            "PP.T.Y",
        ),
    );
    let y = |y| T { y, ..T::default() };
    fail(
        r#"{"Ts": [{"Y": 1}, {"Y": 2}, {"Y": "bad-type"}]}"#,
        Some(PP {
            ts: vec![y(1), y(2), y(0)],
            ..PP::default()
        }),
        type_err(
            "string",
            "int",
            len64(r#"{"Ts": [{"Y": 1}, {"Y": 2}, {"Y": "bad-type""#),
            "PP",
            "Ts.2.Y",
        ),
    );

    // Verify that syntactic errors are immediately fatal, while semantic errors are lazily reported
    // (i.e., allow processing to continue).
    fail::<Vec<i64>>(
        "[1,2,true,4,5}",
        None,
        syntax(
            "invalid character '}' after array element",
            len64("[1,2,true,4,5}"),
        ),
    );
    // :1197-1203, the index is the path; an unnamed root (`[]int`) leaves `Struct` empty
    fail(
        "[1,2,true,4,5]",
        Some(vec![1_i64, 2, 0, 4, 5]),
        type_err("bool", "int", len64("[1,2,true"), "", "2"),
    );

    // :1226-1232, `Struct` is the root `NestedUnamed`
    fail::<NestedUnamed>(
        r#"{"F":{"V":"s"}}"#,
        None,
        type_err(
            "string",
            "int",
            len64(r#"{"F":{"V":"s""#),
            "NestedUnamed",
            "F.V",
        ),
    );
}

// ---- TestUnmarshalMarshal (:1550) ----

/// xorshift64*, a stand-in for `math/rand`.
struct Rng(u64);

impl Rng {
    fn step(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.step() % n
    }

    fn uniform(&mut self) -> f64 {
        (self.step() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Approximately normal (Irwin-Hall), for `rand.NormFloat64`.
    fn norm(&mut self) -> f64 {
        (0..12).map(|_| self.uniform()).sum::<f64>() - 6.0
    }
}

fn gen_value(r: &mut Rng, n: usize) -> Value {
    if n > 1 {
        return if r.below(2) == 0 {
            gen_array(r, n)
        } else {
            gen_map(r, n)
        };
    }
    match r.below(3) {
        0 => Value::Bool(r.below(2) == 0),
        1 => Value::Number(r.norm()),
        _ => Value::String(gen_string(r, 30.0)),
    }
}

fn gen_string(r: &mut Rng, stddev: f64) -> String {
    let n = (r.norm() * stddev + stddev / 2.0).abs() as usize;
    (0..n)
        .map(|_| {
            let f = (r.norm() * 64.0 + 32.0).abs().min(f64::from(0x10ffff_u32));
            char::from_u32(f as u32).unwrap_or('\u{fffd}')
        })
        .collect()
}

fn gen_array(r: &mut Rng, n: usize) -> Value {
    let f = ((r.norm().abs() * f64::min(10.0, (n / 2) as f64)) as usize)
        .min(n)
        .max(1);
    Value::Array(
        (0..f)
            .map(|i| gen_value(r, ((i + 1) * n) / f - (i * n) / f))
            .collect(),
    )
}

fn gen_map(r: &mut Rng, n: usize) -> Value {
    let mut f = ((r.norm().abs() * f64::min(10.0, (n / 2) as f64)) as usize).min(n);
    if n > 0 && f == 0 {
        f = 1;
    }
    let mut x = BTreeMap::new();
    for i in 0..f {
        let k = gen_string(r, 10.0);
        let v = gen_value(r, ((i + 1) * n) / f - (i * n) / f);
        x.insert(k, v);
    }
    Value::Object(x)
}

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_marshal() {
    let json_big = marshal(&gen_value(&mut Rng(0x9E37_79B9_7F4A_7C15), 10000)).expect("Marshal");
    let v: Value = unmarshal(&json_big).expect("Unmarshal error");
    let b = marshal(&v).expect("Marshal error");
    assert!(b == json_big, "Marshal differs from the input");
}

// ---- TestUnmarshalInterface (:1624) ----

#[derive(Debug, Default, PartialEq)]
struct Xint {
    x: i64,
}
gostd::json_fields!(decode Xint, "json.Xint", { x => "X" });

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_interface() {
    let mut xint = Xint::default();
    unmarshal_into(br#"{"X":1}"#, &mut xint).expect("Unmarshal error");
    assert_eq!(xint.x, 1);
}

// ---- TestEscape (:1646) ----

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_escape() {
    let input = "\"foobar\"<html> [\u{2028} \u{2029}]";
    let want = r#""\"foobar\"\u003chtml\u003e [\u2028 \u2029]""#;
    let got = marshal(input).expect("Marshal error");
    assert_eq!(String::from_utf8(got).unwrap(), want);
}

// ---- TestEmptyString (:2040), TestNullString (:2058) ----

#[derive(Debug, Default)]
struct T2 {
    number1: i64,
    number2: i64,
}
gostd::json_fields!(decode T2, "json.T2", { number1 => "Number1", number2 => "Number2" });

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_empty_string() {
    // Go: `{"Number1":"1", "Number2":""}` with `,string`
    let data = r#"{"Number1":1, "Number2":""}"#;
    let mut got = T2::default();
    let err = unmarshal_into(data.as_bytes(), &mut got);
    assert!(err.is_err(), "Decode error: got nil, want non-nil");
    assert_eq!(got.number1, 1, "Decode: got.Number1");
    assert_eq!(got.number2, 0);
}

#[derive(Debug, Default)]
struct NullString {
    a: i64,
    b: i64,
    c: Option<i64>,
}
gostd::json_fields!(decode NullString, "json.T", { a => "A", b => "B", c => "C" });

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_null_string() {
    // Go: `{"A": "1", "B": null, "C": null}` with `,string`
    let data = br#"{"A": 1, "B": null, "C": null}"#;
    let mut s = NullString {
        a: 0,
        b: 1,
        c: Some(2),
    };
    unmarshal_into(data, &mut s).expect("Unmarshal error");
    assert_eq!(s.a, 1);
    assert_eq!(s.b, 1, "Unmarshal: s.B");
    assert_eq!(s.c, None, "Unmarshal: s.C");
}

// ---- TestInterfaceSet (:2083) ----

#[derive(Debug, Default, PartialEq)]
struct Holder {
    x: Value,
}
gostd::json_fields!(decode Holder, "struct { X interface {} }", { x => "X" });

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_interface_set() {
    // (pre, json, post)
    let tests: Vec<(Value, &str, Value)> = vec![
        (string("foo"), r#""bar""#, string("bar")),
        (string("foo"), "2", num(2.0)),
        (string("foo"), "true", Value::Bool(true)),
        (string("foo"), "null", Value::Null),
        (obj([]), "true", Value::Bool(true)), // map[string]any{}
        (Value::Array(vec![]), "true", Value::Bool(true)), // []string{}
        (Value::Null, "null", Value::Null),
        (Value::Null, "2", num(2.0)),
        (num(1.0), "2", num(2.0)), // (int)(1)
        (Value::Null, "{}", obj([])),
        (num(1.0), "{}", obj([])), // (int)(1)
    ];
    for (pre, json, post) in tests {
        let mut b = Holder { x: pre };
        let blob = format!(r#"{{"X":{json}}}"#);
        unmarshal_into(blob.as_bytes(), &mut b)
            .unwrap_or_else(|e| panic!("Unmarshal({blob}): {e}"));
        assert_eq!(b.x, post, "Unmarshal({blob})");
    }
}

// ---- TestUnmarshalNulls (:2190) ----

#[derive(Debug, Default)]
struct NullTest {
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
    float32: f64,
    float64: f64,
    string: String,
    pbool: Option<bool>,
    map: Option<BTreeMap<String, String>>,
    slice: Option<Vec<String>>,
    interface: Value,
    praw: Option<RawMessage>,
    pstruct: Option<Empty>,
    raw: RawMessage,
    struct_: Empty,
}
gostd::json_fields!(decode NullTest, "json.NullTest", {
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
    float32 => "Float32",
    float64 => "Float64",
    string => "String",
    pbool => "PBool",
    map => "Map",
    slice => "Slice",
    interface => "Interface",
    praw => "PRaw",
    pstruct => "PStruct",
    raw => "Raw",
    struct_ => "Struct",
});

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_nulls() {
    let json_data = br#"{
				"Bool"    : null,
				"Int"     : null,
				"Int8"    : null,
				"Int16"   : null,
				"Int32"   : null,
				"Int64"   : null,
				"Uint"    : null,
				"Uint8"   : null,
				"Uint16"  : null,
				"Uint32"  : null,
				"Uint64"  : null,
				"Float32" : null,
				"Float64" : null,
				"String"  : null,
				"PBool": null,
				"Map": null,
				"Slice": null,
				"Interface": null,
				"PRaw": null,
				"PTime": null,
				"PBigInt": null,
				"PText": null,
				"PBuffer": null,
				"PStruct": null,
				"Raw": null,
				"Time": null,
				"BigInt": null,
				"Text": null,
				"Buffer": null,
				"Struct": null
			}"#;
    let mut nulls = NullTest {
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
        float32: 12.1,
        float64: 13.1,
        string: "14".into(),
        pbool: Some(false),
        map: Some(BTreeMap::new()),
        slice: Some(Vec::new()),
        interface: Value::Bool(true),
        praw: Some(RawMessage::default()),
        pstruct: Some(Empty),
        raw: RawMessage(b"123".to_vec()),
        struct_: Empty,
    };

    unmarshal_into(json_data, &mut nulls).expect("Unmarshal of null values failed");
    assert!(
        nulls.bool
            && nulls.int == 2
            && nulls.int8 == 3
            && nulls.int16 == 4
            && nulls.int32 == 5
            && nulls.int64 == 6
            && nulls.uint == 7
            && nulls.uint8 == 8
            && nulls.uint16 == 9
            && nulls.uint32 == 10
            && nulls.uint64 == 11
            && nulls.float32 == 12.1
            && nulls.float64 == 13.1
            && nulls.string == "14",
        "Unmarshal of null values affected primitives"
    );
    assert!(nulls.pbool.is_none(), "did not clear nulls.PBool");
    assert!(nulls.map.is_none(), "did not clear nulls.Map");
    assert!(nulls.slice.is_none(), "did not clear nulls.Slice");
    assert_eq!(
        nulls.interface,
        Value::Null,
        "did not clear nulls.Interface"
    );
    assert!(nulls.praw.is_none(), "did not clear nulls.PRaw");
    assert!(nulls.pstruct.is_none(), "did not clear nulls.PStruct");
    assert_eq!(
        String::from_utf8_lossy(&nulls.raw.0),
        "null",
        "Unmarshal of RawMessage null did not record null"
    );
    assert_eq!(nulls.struct_, Empty);
}

// ---- TestUnmarshalTypeError (:2381), TestUnmarshalTypeErrorMessage (:2405),
// TestUnmarshalSyntax (:2433) ----

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_type_error() {
    // issue 4628; the 5 `new(error)` rows (issue 4222) are left out
    let err = unmarshal::<String>(br#"{"user": "name"}"#);
    assert!(
        matches!(err, Err(Error::UnmarshalType(_))),
        "Unmarshal(string): got {err:?}, want UnmarshalType"
    );
}

#[test]
fn test_unmarshal_type_error_message() {
    for (field, want) in [
        (
            "",
            "json: cannot unmarshal number 5 into Go struct field Root. of type int",
        ),
        (
            "1",
            "json: cannot unmarshal number 5 into Root.1 of type int",
        ),
        (
            "foo",
            "json: cannot unmarshal number 5 into Go struct field Root.foo of type int",
        ),
        (
            "foo.1",
            "json: cannot unmarshal number 5 into Root.foo.1 of type int",
        ),
        (
            "foo.bar",
            "json: cannot unmarshal number 5 into Go struct field Root.foo.bar of type int",
        ),
        (
            "foo.bar.1",
            "json: cannot unmarshal number 5 into Root.foo.bar.1 of type int",
        ),
        (
            "foo.bar.baz",
            "json: cannot unmarshal number 5 into Go struct field Root.foo.bar.baz of type int",
        ),
    ] {
        let got = type_err("number 5", "int", 1234, "Root", field).to_string();
        assert_eq!(got, want, "Error");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_syntax() {
    for input in [
        "tru",
        "fals",
        "nul",
        "123e",
        r#""hello"#,
        "[1,2,3",
        r#"{"key":1"#,
        r#"{"key":1,"#,
    ] {
        let err = unmarshal::<Value>(input.as_bytes());
        assert!(
            matches!(err, Err(Error::Syntax { .. })),
            "Unmarshal({input:?}, any): got {err:?}, want Syntax"
        );
    }
}

// ---- TestSkipArrayObjects (:2512) ----

/// Go `[0]any`: every element is dropped.
#[derive(Default)]
struct ZeroArray;

impl Unmarshal for ZeroArray {
    const GO_TYPE: &'static str = "[0]interface {}";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        d.read_array(|d, _| d.skip_value())
    }
}

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_skip_array_objects() {
    let mut dest = ZeroArray;
    unmarshal_into(b"[{}]", &mut dest).expect("Unmarshal error");
}

// ---- TestPrefilled (:2525) ----

#[derive(Debug, Default, PartialEq)]
struct Xyz {
    x: Value,
    y: Value,
    z: Value,
}
gostd::json_fields!(decode Xyz, "json.XYZ", { x => "X", y => "Y", z => "Z" });

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_prefilled() {
    // `float32(3)` and `int16(4)` in an `any` are `Value::Number`
    let mut xyz = Xyz {
        x: num(3.0),
        y: num(4.0),
        z: num(1.5),
    };
    unmarshal_into(br#"{"X": 1, "Y": 2}"#, &mut xyz).expect("Unmarshal error");
    assert_eq!(
        xyz,
        Xyz {
            x: num(1.0),
            y: num(2.0),
            z: num(1.5)
        }
    );

    let mut m = BTreeMap::from([
        ("X".to_string(), num(3.0)),
        ("Y".to_string(), num(4.0)),
        ("Z".to_string(), num(1.5)),
    ]);
    unmarshal_into(br#"{"X": 1, "Y": 2}"#, &mut m).expect("Unmarshal error");
    assert_eq!(
        m,
        BTreeMap::from([
            ("X".to_string(), num(1.0)),
            ("Y".to_string(), num(2.0)),
            ("Z".to_string(), num(1.5)),
        ])
    );

    let mut s = vec![1_i64];
    unmarshal_into(b"[2]", &mut s).expect("Unmarshal error");
    assert_eq!(s, [2]);

    let mut s = vec![1_i64];
    unmarshal_into(b"[2, 3]", &mut s).expect("Unmarshal error");
    assert_eq!(s, [2, 3]);

    let mut f = std::f64::consts::PI;
    let err = unmarshal_into(b"1e1000", &mut f);
    assert_eq!(f, std::f64::consts::PI);
    assert_eq!(
        err,
        Err(type_err(
            "number 1e1000",
            "float64",
            len64("1e1000"),
            "",
            ""
        ))
    );

    // :2570-2575, the same offset as the float64 target
    let mut v = num(0.0);
    let err = unmarshal_into(b"1e1000", &mut v);
    assert_eq!(v, num(0.0));
    assert_eq!(
        err,
        Err(type_err(
            "number 1e1000",
            "float64",
            len64("1e1000"),
            "",
            ""
        ))
    );
}

// ---- TestUnmarshalErrorAfterMultipleJSON (:2797) ----

/// (stream, the valid documents before the failing one, the failing document at the end of the stream,
/// Go's message, Go's offset in the stream)
type StreamRow = (
    &'static str,
    &'static [&'static str],
    &'static str,
    &'static str,
    i64,
);

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_error_after_multiple_json() {
    let tests: [StreamRow; 5] = [
        (
            "1 false null :",
            &["1", "false", "null"],
            ":",
            "invalid character ':' looking for beginning of value",
            len64("1 false null :"),
        ),
        (
            "1 [] [,]",
            &["1", "[]"],
            "[,]",
            "invalid character ',' looking for beginning of value",
            len64("1 [] [,"),
        ),
        (
            "1 [] [true:]",
            &["1", "[]"],
            "[true:]",
            "invalid character ':' after array element",
            len64("1 [] [true:"),
        ),
        (
            r#"1  {}    {"x"=}"#,
            &["1", "{}"],
            r#"{"x"=}"#,
            "invalid character '=' after object key",
            len64(r#"1  {}    {"x"="#),
        ),
        (
            "falsetruenul#",
            &["false", "true"],
            "nul#",
            "invalid character '#' in literal null (expecting 'l')",
            len64("falsetruenul#"),
        ),
    ];
    for (stream, before, last, msg, offset) in tests {
        for doc in before {
            unmarshal::<Value>(doc.as_bytes())
                .unwrap_or_else(|e| panic!("Decode({doc:?}) of {stream:?}: {e}"));
        }
        let start = len64(stream) - len64(last);
        assert_eq!(
            unmarshal::<Value>(last.as_bytes()),
            Err(syntax(msg, offset - start)),
            "Decode error of {stream:?}"
        );
    }
}

// ---- TestUnmarshalPanic (:2842) ----

struct UnmarshalPanic;

impl Unmarshal for UnmarshalPanic {
    const GO_TYPE: &'static str = "json.unmarshalPanic";
    fn unmarshal_json(&mut self, _d: &mut DecodeState<'_>) -> Result<(), Error> {
        panic_any(0xdead_i32)
    }
}

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_panic() {
    match catch_unwind(AssertUnwindSafe(|| {
        unmarshal_into(b"{}", &mut UnmarshalPanic)
    })) {
        Err(got) => assert_eq!(got.downcast_ref::<i32>(), Some(&0xdead), "panic() payload"),
        Ok(_) => panic!("Unmarshal should have panicked"),
    }
}

// ---- TestUnmarshalRescanLiteralMangledUnquote (:2872) ----

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_rescan_literal_mangled_unquote() {
    // See golang.org/issues/38105.
    let p: BTreeMap<String, String> =
        unmarshal("{\"开源\":\"12345开源\"}".as_bytes()).expect("Unmarshal error");
    assert!(
        p.contains_key("开源"),
        r#"key "开源" missing in map: {p:?}"#
    );

    // See golang.org/issues/39555, with plain string keys.
    let input = BTreeMap::from([
        ("FOO".to_string(), String::new()),
        ("\"".to_string(), String::new()),
    ]);
    let encoded = marshal(&input).expect("Marshal error");
    let got: BTreeMap<String, String> = unmarshal(&encoded).expect("Unmarshal error");
    assert_eq!(got, input, "Marshal/Unmarshal roundtrip");
}

// ---- TestUnmarshalMaxDepth (:2917) ----

#[derive(Debug, Default)]
struct NamedA {
    a: Value,
}
gostd::json_fields!(decode NamedA, "struct { A interface {} }", { a => "a" });

#[derive(Debug, Default)]
struct NamedB {
    b: Value,
}
gostd::json_fields!(decode NamedB, "struct { B interface {} }", { b => "b" });

fn decode_into<D: Unmarshal + Default>(data: &[u8]) -> Result<(), Error> {
    unmarshal::<D>(data).map(drop)
}

type Target = (&'static str, fn(&[u8]) -> Result<(), Error>);

fn max_depth() {
    let arrays = |n: usize| format!("{{\"a\":{}{}}}", "[".repeat(n), "]".repeat(n));
    let objects = |n: usize| format!("{{\"a\":{}0{}}}", "{\"a\":".repeat(n), "}".repeat(n));
    let tests: [(&str, String, bool); 6] = [
        ("ArrayUnderMaxNestingDepth", arrays(10000 - 1), false),
        ("ArrayOverMaxNestingDepth", arrays(10000), true),
        ("ArrayOverStackDepth", arrays(3000000), true),
        ("ObjectUnderMaxNestingDepth", objects(10000 - 1), false),
        ("ObjectOverMaxNestingDepth", objects(10000), true),
        ("ObjectOverStackDepth", objects(3000000), true),
    ];
    let targets: [Target; 4] = [
        ("unstructured", decode_into::<Value>),
        ("typed named field", decode_into::<NamedA>),
        ("typed missing field", decode_into::<NamedB>),
        ("custom unmarshaler", decode_into::<Unmarshaler>),
    ];
    for (name, data, err_max_depth) in &tests {
        for (target, decode) in &targets {
            let err = decode(data.as_bytes());
            if *err_max_depth {
                let found = err
                    .as_ref()
                    .is_err_and(|e| e.to_string().contains("exceeded max depth"));
                assert!(
                    found,
                    "{target}-{name}: Unmarshal error: got {err:?}, want exceeded max depth"
                );
            } else {
                assert!(err.is_ok(), "{target}-{name}: Unmarshal error: {err:?}");
            }
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn test_unmarshal_max_depth() {
    // 9,999 levels of nesting decode recursively: run on a thread with a large stack
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(max_depth)
        .expect("spawn")
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e));
}

// ---- Added: v1_compat_rules ----

#[derive(Debug, Default, PartialEq)]
struct IdOnly {
    id: String,
}
gostd::json_fields!(decode IdOnly, "json.IdOnly", { id => "id" });

/// An unnamed root struct, as graphify-go decodes graph.json in export.go:181.
#[derive(Debug, Default, PartialEq)]
struct AnonNodes {
    nodes: Vec<IdOnly>,
}
gostd::json_fields!(decode AnonNodes, r#"struct { Nodes []json.IdOnly "json:\"nodes\"" }"#, {
    nodes => "nodes",
});

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn v1_compat_rules() {
    // Traced, no Go row: DefaultV1Flags sets AllowDuplicateNames, MatchCaseInsensitiveNames and
    // MatchCaseSensitiveDelimiter (internal/jsonflags/flags.go:51-72). A duplicate key is decoded
    // again, so the last one wins; a key with no exact match takes the first field whose name is
    // strings.EqualFold to it (v2/arshal_default.go:1328-1337, v2/fields.go:380-386).
    let got: IdOnly = unmarshal(br#"{"ID":"a","id":"b"}"#).unwrap();
    assert_eq!(got.id, "b");
    let got: IdOnly = unmarshal(br#"{"id":"a","id":"b"}"#).unwrap();
    assert_eq!(got.id, "b");
    let got: IdOnly = unmarshal(br#"{"Id":"x"}"#).unwrap();
    assert_eq!(got.id, "x");

    // A lone surrogate escape and an invalid UTF-8 byte decode to U+FFFD (the unmarshalTests rows at
    // v2_decode_test.go:730-735 and :748-753, here as a whole string).
    assert_eq!(unmarshal::<String>(br#""\ud800""#).unwrap(), "\u{fffd}");
    assert_eq!(unmarshal::<String>(b"\"\xff\"").unwrap(), "\u{fffd}");

    // Traced, no Go row: a BOM is one rune U+FEFF, quoted by strconv.QuoteRune
    // (internal/jsonwire/wire.go:62-68) and counted whole in the offset (jsontext/errors.go:102-107);
    // Unmarshal validates first (v2/arshal.go:456-464).
    assert_eq!(
        unmarshal::<Value>(b"\xef\xbb\xbf{}"),
        Err(syntax(
            "invalid character '\\ufeff' looking for beginning of value",
            3
        ))
    );

    // Traced, no Go row: the 10,001st `[` exceeds maxNestingDepth (jsontext/state.go:53), a
    // structural error one byte past its start (jsontext/errors.go:102-115). TestUnmarshalMaxDepth
    // (v2_decode_test.go:2917) checks only that the text contains "exceeded max depth".
    assert_eq!(
        unmarshal::<Value>("[".repeat(10001).as_bytes()),
        Err(syntax("exceeded max depth", 10001))
    );

    // Traced, no Go row: 1e400 overflows float64 (strconv.ErrRange, v2/arshal_default.go:765-767),
    // which v1 reports as "number <literal>" with no Err (v2_inject.go:114-136); the offset is past
    // the value (v2/errors.go:164-174).
    assert_eq!(
        unmarshal::<f64>(b"1e400"),
        Err(type_err("number 1e400", "float64", 5, "", ""))
    );

    // Traced, no Go row: an unnamed root leaves `Struct` empty (v2_inject.go:137-143) while the path
    // runs from the root through the array index (v2_inject.go:144-146), so the text has
    // "Go struct field .nodes.1.id" (graphify-go wraps such text into a user error at export.go:187).
    let mut v = AnonNodes::default();
    let err = unmarshal_into(br#"{"nodes":[{"id":"a"},{"id":1}]}"#, &mut v).unwrap_err();
    assert_eq!(
        err,
        type_err(
            "number",
            "string",
            len64(r#"{"nodes":[{"id":"a"},{"id":1"#),
            "",
            "nodes.1.id"
        )
    );
    assert_eq!(
        err.to_string(),
        "json: cannot unmarshal number into Go struct field .nodes.1.id of type string"
    );
}
