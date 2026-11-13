//! Go `encode.go` and the `Encoder` of `stream.go`.

use std::collections::{BTreeMap, HashMap};

use super::{Error, RawMessage, Value};

/// Go `encodeState` (`encode.go:~300`): the output buffer and the HTML-escape option.
#[derive(Debug, Clone, Default)]
pub struct EncodeState {
    pub buf: Vec<u8>,
    pub escape_html: bool,
}

impl EncodeState {
    /// Appends `b` verbatim; the caller guarantees it is valid JSON.
    pub fn write_raw(&mut self, b: &[u8]) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `appendString` (`encode.go:998`) with the current `escape_html`; quotes included.
    pub fn write_string(&mut self, s: &[u8]) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `floatEncoder.encode` (`encode.go:570`).
    pub fn write_float(&mut self, f: f64) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `intEncoder` (`encode.go:552`).
    pub fn write_int(&mut self, i: i64) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `uintEncoder` (`encode.go:560`).
    pub fn write_uint(&mut self, u: u64) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `boolEncoder` (`encode.go:544`).
    pub fn write_bool(&mut self, b: bool) {
        todo!("graphify-rs-aa6.9")
    }

    /// Writes `null`.
    pub fn write_null(&mut self) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `marshalerEncoder` (`encode.go:472`): the result of a `MarshalJSON` is validated and
    /// compacted with `escape_html`; a syntax error becomes `Error::Marshaler` for `go_type`.
    pub fn write_marshaler_output(
        &mut self,
        go_type: &'static str,
        json: &[u8],
    ) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}

/// What Go's `json.Marshaler` or a `reflect` encoder does for a type.
pub trait Marshal {
    /// Appends this value's JSON to `e`.
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error>;

    /// Go `isEmptyValue` (`encode.go:351`): `omitempty` skips the field when true.
    fn is_empty_value(&self) -> bool {
        false
    }
}

impl Marshal for bool {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for i64 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for i32 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for u64 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for u32 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for usize {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for f64 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for str {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for String {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
/// Go pointer: `None` is a nil pointer, `null`, and empty for `omitempty`.
impl<T: Marshal> Marshal for Option<T> {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl<T: Marshal> Marshal for Vec<T> {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl<T: Marshal> Marshal for [T] {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl<T: Marshal + ?Sized> Marshal for &T {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl<T: Marshal> Marshal for BTreeMap<String, T> {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
/// Keys are sorted on encode, as Go's `mapEncoder` does (`encode.go:764`).
impl<T: Marshal> Marshal for HashMap<String, T> {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
impl Marshal for Value {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
/// Go `RawMessage.MarshalJSON` (`stream.go:276`) through `marshalerEncoder`.
impl Marshal for RawMessage {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}

/// Go `Marshal` (`encode.go:204`): HTML escaping on.
pub fn marshal<T: Marshal + ?Sized>(v: &T) -> Result<Vec<u8>, Error> {
    todo!("graphify-rs-aa6.9")
}

/// Go `MarshalIndent` (`encode.go:220`).
pub fn marshal_indent<T: Marshal + ?Sized>(
    v: &T,
    prefix: &str,
    indent: &str,
) -> Result<Vec<u8>, Error> {
    todo!("graphify-rs-aa6.9")
}

/// Go `appendString` (`encode.go:998`); quotes included, invalid UTF-8 becomes `�` per byte.
pub fn append_string(dst: &mut Vec<u8>, s: &[u8], escape_html: bool) {
    todo!("graphify-rs-aa6.9")
}

/// Go `floatEncoder.encode` for 64 bits (`encode.go:570`); NaN and the infinities give
/// `Error::UnsupportedValue`.
pub fn append_float(dst: &mut Vec<u8>, f: f64) -> Result<(), Error> {
    todo!("graphify-rs-aa6.9")
}

/// Go `Encoder` (`stream.go:195`).
pub struct Encoder<W: std::io::Write> {
    w: W,
    prefix: String,
    indent: String,
    escape_html: bool,
}

impl<W: std::io::Write> Encoder<W> {
    /// Go `NewEncoder` (`stream.go:206`).
    pub fn new(w: W) -> Self {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `Encoder.SetIndent` (`stream.go:254`).
    pub fn set_indent(&mut self, prefix: &str, indent: &str) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `Encoder.SetEscapeHTML` (`stream.go:266`).
    pub fn set_escape_html(&mut self, on: bool) {
        todo!("graphify-rs-aa6.9")
    }

    /// Go `Encoder.Encode` (`stream.go:216`): value, then `\n`.
    pub fn encode<T: Marshal + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        todo!("graphify-rs-aa6.9")
    }
}
