//! Go 1.27.4 `encoding/json`, decode half. Go 1.27.4 builds it with GOEXPERIMENT=jsonv2 on, so
//! `Unmarshal` is `jsonv2.Unmarshal(data, v, DefaultOptionsV1())` (`v2_decode.go:110`) with its
//! errors mapped by `transformUnmarshalError` (`v2_inject.go:69`). The `decode.go:NNN` citations
//! below are to the excluded v1 file and describe the intended behaviour only where the v2 tests
//! agree. API stubs; bodies arrive with graphify-rs-aa6.10.

// graphify-rs-aa6.10: stubs until the code PR
#![allow(unused_variables)]
// graphify-rs-aa6.10: stubs until the code PR
#![allow(dead_code)]
// graphify-rs-aa6.10: stubs until the code PR
#![allow(clippy::ptr_arg)]

use std::collections::{BTreeMap, HashMap};

use super::{Error, RawMessage, Value};

/// Go `decodeState` (`decode.go:200`): a cursor over validated input, the first saved type error
/// and the error context. Opaque; only [`Unmarshal`] impls drive it.
///
/// The error context follows v2 (`v2_inject.go:137-146`), not v1's `errorContext`:
/// `struct_name` holds the root type's Go `Name()`, fixed for the whole decode (the root's
/// `GO_TYPE` without its package qualifier, e.g. `json.PP` gives `PP`; empty for an unnamed
/// root: `struct { ... }`, a map, a slice, an `Option` (Go pointer)), and `field_stack` is the
/// JSON Pointer of the current value: every object key (struct field and map key alike) and
/// every array index is pushed while its value decodes. A saved error gets `field_stack`
/// joined with `.`, and `struct_name` only when that path is non-empty.
///
/// Reading methods return `Err` only for a failure that aborts the decode. Input of the wrong
/// kind is a *type* error: it is recorded with [`save_type_error`](Self::save_type_error), the
/// value is skipped and `Ok(None)` is returned, as Go keeps decoding after an
/// `UnmarshalTypeError` (`v2_decode_test.go:1197-1203`). An error an `Unmarshal` impl returns
/// itself is returned verbatim, with no context added (`v2_inject.go:70-71`).
pub struct DecodeState<'a> {
    data: &'a [u8],
    off: usize,
    saved_error: Option<Error>,
    struct_name: String,
    field_stack: Vec<String>,
}

impl<'a> DecodeState<'a> {
    /// A state over `data`, already validated by `check_valid` (`decode.go:93`).
    pub(super) fn new(data: &'a [u8]) -> Self {
        DecodeState {
            data,
            off: 0,
            saved_error: None,
            struct_name: String::new(),
            field_stack: Vec::new(),
        }
    }

    /// Whether the next value is the literal `null` (`decode.go:430` `literalStore` null case).
    /// Does not consume it: an impl calls [`skip_value`](Self::skip_value) after deciding, e.g.
    /// `Option<T>` becomes `None`, a scalar stays unchanged.
    pub fn peek_null(&mut self) -> bool {
        todo!("graphify-rs-aa6.10")
    }

    /// Reads a JSON string as Go `literalStore` (`decode.go:866`) does into a `string`: unquoted
    /// (`unquote`, `decode.go:1300`), invalid UTF-8 and lone surrogates become U+FFFD. A non-string
    /// value records a type error (`"number"`, `"bool"`, `"array"`, ...) and yields `Ok(None)`.
    pub fn read_string(&mut self) -> Result<Option<String>, Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Reads a JSON number and returns its literal text, e.g. `"1e400"` (Go `d.convertNumber`
    /// input, `decode.go:834`). A non-number records a type error and yields `Ok(None)`.
    pub fn read_number_literal(&mut self) -> Result<Option<&'a str>, Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Reads a JSON number as `f64` (Go `literalStore` float case, `decode.go:985`). A non-number
    /// or an out-of-range value (`1e400`, text `"number 1e400"`) records a type error with
    /// `go_type` and yields `Ok(None)`.
    pub fn read_f64(&mut self, go_type: &str) -> Result<Option<f64>, Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Reads a JSON integer as `i64`, rejecting values that overflow `bits` bits or are not
    /// integers (Go `literalStore` int case, `decode.go:965`); on failure records a type error
    /// with `value` text `"number <literal>"` and `go_type`, yielding `Ok(None)`.
    pub fn read_i64(&mut self, go_type: &str, bits: u32) -> Result<Option<i64>, Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// As [`read_i64`](Self::read_i64) for unsigned integers (Go `literalStore` uint case,
    /// `decode.go:975`).
    pub fn read_u64(&mut self, go_type: &str, bits: u32) -> Result<Option<u64>, Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Reads `true` or `false` (Go `literalStore` bool case, `decode.go:920`). A non-bool records
    /// a type error (`"string"`, `"number"`, ...) and yields `Ok(None)`.
    pub fn read_bool(&mut self) -> Result<Option<bool>, Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Iterates the entries of the next JSON object (Go `d.object`, `decode.go:588`), calling
    /// `f(self, key)` once per entry in document order, duplicates included; `f` must consume the
    /// value (decode or [`skip_value`](Self::skip_value)). Keys are unquoted as in
    /// [`read_string`](Self::read_string). Callback style is chosen over an iterator because the
    /// callback borrows the state mutably for the value. A non-object records a type error
    /// (`"object"`, `decode.go:614`), skips the value and calls `f` zero times. Use for maps and
    /// [`Value`]; a struct uses [`read_struct`](Self::read_struct). The key is on the field path
    /// while `f` runs (`data.test2`, `v2_decode_test.go:1058`).
    pub fn read_object(
        &mut self,
        f: impl FnMut(&mut DecodeState<'a>, &str) -> Result<(), Error>,
    ) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// As [`read_object`](Self::read_object) for a struct (Go `d.object` with a struct target).
    /// Each key is pushed on the field path while `f` runs and popped after, as in
    /// [`read_object`](Self::read_object). `struct_name` does not reach the error: v2 names the
    /// root type, not the innermost struct (`v2_decode_test.go:1088`, `Struct: "P"` for an error
    /// inside `T`).
    pub fn read_struct(
        &mut self,
        struct_name: &str,
        f: impl FnMut(&mut DecodeState<'a>, &str) -> Result<(), Error>,
    ) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Iterates the elements of the next JSON array (Go `d.array`, `decode.go:500`), calling
    /// `f(self, index)` per element; `f` must consume the element. A non-array records a type
    /// error (`"array"`, `decode.go:517`), skips the value and calls `f` zero times. The index is
    /// on the field path while `f` runs (`Ts.2.Y`, `v2_decode_test.go:1102`).
    pub fn read_array(
        &mut self,
        f: impl FnMut(&mut DecodeState<'a>, usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Skips the next value of any kind (Go `d.skip`, `decode.go:276`), e.g. an unknown struct key.
    pub fn skip_value(&mut self) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Takes the raw bytes of the next value and consumes it (Go `RawMessage.UnmarshalJSON`,
    /// `stream.go:285`, fed by `d.data[start:d.readIndex()]`, `decode.go:422`). The slice is a
    /// view of the input: copy it to keep it.
    pub fn raw_value(&mut self) -> Result<&'a [u8], Error> {
        todo!("graphify-rs-aa6.10")
    }

    /// Records a type error: only the first is kept, with the root name and field path described
    /// on [`DecodeState`] and `err: None`. The offset is the bytes read so far: past the value,
    /// or past only its opening `[` or `{` (`v2/errors.go:164-174`). `value` is Go's description
    /// (`"bool"`, `"array"`, `"number -5"`, `v2_inject.go:103-133`); `go_type` is the target's Go
    /// type string. The decode continues and the saved error is returned at the end by
    /// [`unmarshal_into`].
    pub fn save_type_error(&mut self, value: &str, go_type: &str) {
        todo!("graphify-rs-aa6.10")
    }
}

/// Decodes JSON into an existing Go value, as Go `d.value` does (`decode.go:384`): `null` leaves
/// a scalar unchanged and sets an `Option`, `Vec` or map to `None`/empty; objects merge into
/// existing maps (keys are kept); arrays refill a slice from index 0. Go `Unmarshaler`
/// (`decode.go:118`).
///
/// `GO_TYPE` is the Go type string used in error text (`reflect.Type.String()`). For the generic
/// containers it is only a placeholder prefix (`"[]"`, `"*"`, `"map[string]"`) because an
/// associated const cannot embed `T::GO_TYPE`; use [`go_type`](Self::go_type) for error text.
pub trait Unmarshal {
    /// The Go type string, e.g. `"int"`, `"main.Node"`.
    const GO_TYPE: &'static str;

    /// Go's `reflect.Type.String()` of the target, used in `UnmarshalTypeError`
    /// (`decode.go:136`). Defaults to `GO_TYPE`; the container impls compose their element's.
    fn go_type() -> String {
        Self::GO_TYPE.to_string()
    }

    /// Decodes the next value of `d` into `self`.
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error>;
}

macro_rules! stub_unmarshal {
    ($($t:ty => $go:literal),* $(,)?) => {$(
        /// Go `literalStore` (`decode.go:866`) for this scalar.
        impl Unmarshal for $t {
            const GO_TYPE: &'static str = $go;
            fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
                todo!("graphify-rs-aa6.10")
            }
        }
    )*};
}
stub_unmarshal! {
    bool => "bool",
    i64 => "int",
    i32 => "int32",
    u64 => "uint64",
    u32 => "uint32",
    usize => "uint",
    f64 => "float64",
    String => "string",
    Value => "interface {}",
    RawMessage => "json.RawMessage",
}

/// Go pointer target (`indirect`, `decode.go:430`): `null` sets `None`, else allocates a
/// `T::default()` when `None` and decodes into it.
impl<T: Unmarshal + Default> Unmarshal for Option<T> {
    const GO_TYPE: &'static str = "*";
    fn go_type() -> String {
        format!("*{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }
}

/// Go `d.array` (`decode.go:500`) into a slice: `null` empties it, elements are decoded into
/// `T::default()` slots.
impl<T: Unmarshal + Default> Unmarshal for Vec<T> {
    const GO_TYPE: &'static str = "[]";
    fn go_type() -> String {
        format!("[]{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }
}

/// Go `d.object` (`decode.go:588`) into a `map[string]T`: existing keys are kept.
impl<T: Unmarshal + Default> Unmarshal for BTreeMap<String, T> {
    const GO_TYPE: &'static str = "map[string]";
    fn go_type() -> String {
        format!("map[string]{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }
}

/// As the `BTreeMap` impl, for a hash map.
impl<T: Unmarshal + Default> Unmarshal for HashMap<String, T> {
    const GO_TYPE: &'static str = "map[string]";
    fn go_type() -> String {
        format!("map[string]{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        todo!("graphify-rs-aa6.10")
    }
}

/// Go `Unmarshal` (`v2_decode.go:110`) into a fresh `T::default()`.
pub fn unmarshal<T: Unmarshal + Default>(data: &[u8]) -> Result<T, Error> {
    todo!("graphify-rs-aa6.10")
}

/// Go `Unmarshal` (`v2_decode.go:110`) into `v`: `check_valid` first (syntax error wins, as the
/// legacy `CheckNextValue` in `v2/arshal.go:456-464`), then decode; the first saved type error is
/// returned after decoding finishes. The root name for the error context comes from `T`.
pub fn unmarshal_into<T: Unmarshal>(data: &[u8], v: &mut T) -> Result<(), Error> {
    todo!("graphify-rs-aa6.10")
}

/// Used by `json_fields!`: touches a field with a compile-time `Unmarshal` check.
#[doc(hidden)]
pub fn touch_field<T: Unmarshal>(v: &mut T) {}
