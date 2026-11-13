//! Go `encode.go` and the `Encoder` of `stream.go`.

use std::collections::{BTreeMap, HashMap};

use super::indent::{compact, indent as append_indent};
use super::{Error, RawMessage, Value};
use crate::utf8::{RUNE_ERROR, decode_rune};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Go `encodeState` (`encode.go:~300`): the output buffer and the HTML-escape option.
#[derive(Debug, Clone, Default)]
pub struct EncodeState {
    pub buf: Vec<u8>,
    pub escape_html: bool,
}

impl EncodeState {
    /// Appends `b` verbatim; the caller guarantees it is valid JSON.
    pub fn write_raw(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// Go `appendString` (`encode.go:998`) with the current `escape_html`; quotes included.
    pub fn write_string(&mut self, s: &[u8]) {
        append_string(&mut self.buf, s, self.escape_html);
    }

    /// Go `floatEncoder.encode` (`encode.go:570`).
    pub fn write_float(&mut self, f: f64) -> Result<(), Error> {
        append_float(&mut self.buf, f)
    }

    /// Go `intEncoder` (`encode.go:552`).
    pub fn write_int(&mut self, i: i64) {
        self.buf.extend_from_slice(i.to_string().as_bytes());
    }

    /// Go `uintEncoder` (`encode.go:560`).
    pub fn write_uint(&mut self, u: u64) {
        self.buf.extend_from_slice(u.to_string().as_bytes());
    }

    /// Go `boolEncoder` (`encode.go:544`).
    pub fn write_bool(&mut self, b: bool) {
        self.buf
            .extend_from_slice(if b { b"true" } else { b"false" });
    }

    /// Writes `null`.
    pub fn write_null(&mut self) {
        self.buf.extend_from_slice(b"null");
    }

    /// Go `marshalerEncoder` (`encode.go:472`): the result of a `MarshalJSON` is validated and
    /// compacted with `escape_html`; a syntax error becomes `Error::Marshaler` for `go_type`.
    pub fn write_marshaler_output(
        &mut self,
        go_type: &'static str,
        json: &[u8],
    ) -> Result<(), Error> {
        // Go's appendCompact runs the scanner over `json` itself, so it also does what `checkValid`
        // would; the syntax error is wrapped as Go's `MarshalerError` and the buffer is left as it was.
        compact(&mut self.buf, json, self.escape_html).map_err(|err| Error::Marshaler {
            go_type: go_type.to_string(),
            source_func: "MarshalJSON",
            err: err.to_string(),
        })
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

    /// Whether this is an empty Go slice or map. `Option<T>` uses it so that `Option<Vec<_>>` and
    /// `Option<BTreeMap<_, _>>`, which stand for Go's nil-able slices and maps, are empty for
    /// `omitempty` when `None` or when the container is empty, as Go's `isEmptyValue` treats a
    /// nil and an empty slice alike. graphify-go has no pointers to slices or maps, so an
    /// `Option` of a container is always a Go slice or map. Every other type keeps `false`, so
    /// `Option<T>` stays Go's pointer: empty only when `None`.
    #[doc(hidden)]
    fn is_empty_container(&self) -> bool {
        false
    }
}

impl Marshal for bool {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_bool(*self);
        Ok(())
    }
    fn is_empty_value(&self) -> bool {
        !*self
    }
}

/// Signed integers, Go `intEncoder` (`encode.go:552`).
macro_rules! impl_int {
    ($($t:ty),*) => {$(
        impl Marshal for $t {
            fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
                e.write_int(i64::from(*self));
                Ok(())
            }
            fn is_empty_value(&self) -> bool {
                *self == 0
            }
        }
    )*};
}
impl_int!(i64, i32);

/// Unsigned integers, Go `uintEncoder` (`encode.go:560`).
macro_rules! impl_uint {
    ($($t:ty),*) => {$(
        impl Marshal for $t {
            fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
                e.write_uint(u64::from(*self));
                Ok(())
            }
            fn is_empty_value(&self) -> bool {
                *self == 0
            }
        }
    )*};
}
impl_uint!(u64, u32);

impl Marshal for usize {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_uint(*self as u64);
        Ok(())
    }
    fn is_empty_value(&self) -> bool {
        *self == 0
    }
}
impl Marshal for f64 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_float(*self)
    }
    fn is_empty_value(&self) -> bool {
        *self == 0.0
    }
}
impl Marshal for str {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_string(self.as_bytes());
        Ok(())
    }
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
}
impl Marshal for String {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.as_str().marshal_json(e)
    }
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
}
/// Go pointer: `None` is a nil pointer, `null`, and empty for `omitempty`.
impl<T: Marshal> Marshal for Option<T> {
    /// Go `ptrEncoder.encode` (`encode.go:903`).
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        match self {
            None => {
                e.write_null();
                Ok(())
            }
            Some(v) => v.marshal_json(e),
        }
    }
    fn is_empty_value(&self) -> bool {
        self.as_ref().is_none_or(Marshal::is_empty_container)
    }
}
impl<T: Marshal> Marshal for Vec<T> {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.as_slice().marshal_json(e)
    }
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
    fn is_empty_container(&self) -> bool {
        self.is_empty()
    }
}
impl<T: Marshal> Marshal for [T] {
    /// Go `sliceEncoder.encode` (`encode.go:842`) and `arrayEncoder.encode` (`encode.go:882`).
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        e.write_raw(b"[");
        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                e.write_raw(b",");
            }
            v.marshal_json(e)?;
        }
        e.write_raw(b"]");
        Ok(())
    }
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
    fn is_empty_container(&self) -> bool {
        self.is_empty()
    }
}
impl<T: Marshal + ?Sized> Marshal for &T {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        (**self).marshal_json(e)
    }
    fn is_empty_value(&self) -> bool {
        (**self).is_empty_value()
    }
    fn is_empty_container(&self) -> bool {
        (**self).is_empty_container()
    }
}

/// The body of Go `mapEncoder.encode` (`encode.go:764`) for entries already sorted by key.
fn write_map<'a, T: Marshal + 'a>(
    e: &mut EncodeState,
    entries: impl Iterator<Item = (&'a String, &'a T)>,
) -> Result<(), Error> {
    e.write_raw(b"{");
    for (i, (k, v)) in entries.enumerate() {
        if i > 0 {
            e.write_raw(b",");
        }
        e.write_string(k.as_bytes());
        e.write_raw(b":");
        v.marshal_json(e)?;
    }
    e.write_raw(b"}");
    Ok(())
}

impl<T: Marshal> Marshal for BTreeMap<String, T> {
    /// `BTreeMap<String, _>` iterates in byte order of the keys, Go's `strings.Compare` order.
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        write_map(e, self.iter())
    }
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
    fn is_empty_container(&self) -> bool {
        self.is_empty()
    }
}
/// Keys are sorted on encode, as Go's `mapEncoder` does (`encode.go:764`).
impl<T: Marshal> Marshal for HashMap<String, T> {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        let mut entries: Vec<_> = self.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        write_map(e, entries.into_iter())
    }
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
    fn is_empty_container(&self) -> bool {
        self.is_empty()
    }
}
/// A Go `any` holding decoded JSON; `Null` is the nil interface.
impl Marshal for Value {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        match self {
            Value::Null => {
                e.write_null();
                Ok(())
            }
            Value::Bool(b) => {
                e.write_bool(*b);
                Ok(())
            }
            Value::Number(f) => e.write_float(*f),
            Value::String(s) => s.marshal_json(e),
            Value::Array(a) => a.marshal_json(e),
            Value::Object(m) => m.marshal_json(e),
        }
    }
    fn is_empty_value(&self) -> bool {
        matches!(self, Value::Null)
    }
}
/// Go `RawMessage.MarshalJSON` (`stream.go:276`) through `marshalerEncoder`. An empty
/// `RawMessage` is Go's nil one, `null`.
impl Marshal for RawMessage {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        if self.0.is_empty() {
            return e.write_marshaler_output("json.RawMessage", b"null");
        }
        e.write_marshaler_output("json.RawMessage", &self.0)
    }
    fn is_empty_value(&self) -> bool {
        self.0.is_empty()
    }
}

/// Go `structEncoder.encode` (`encode.go:716`) for one field, used by `json_fields!`. `first` is
/// true until a field has been written (Go's `next == '{'`); the key `"-"` and a `skip`
/// (`omitempty` on an empty value) write nothing.
#[doc(hidden)]
pub fn write_field<T: Marshal + ?Sized>(
    e: &mut EncodeState,
    first: &mut bool,
    key: &str,
    v: &T,
    skip: bool,
) -> Result<(), Error> {
    if skip || key == "-" {
        return Ok(());
    }
    e.write_raw(if *first { b"{" } else { b"," });
    *first = false;
    e.write_string(key.as_bytes());
    e.write_raw(b":");
    v.marshal_json(e)
}

/// Go `Marshal` (`encode.go:204`): HTML escaping on.
pub fn marshal<T: Marshal + ?Sized>(v: &T) -> Result<Vec<u8>, Error> {
    let mut e = EncodeState {
        buf: Vec::new(),
        escape_html: true,
    };
    v.marshal_json(&mut e)?;
    Ok(e.buf)
}

/// Go `MarshalIndent` (`encode.go:220`).
pub fn marshal_indent<T: Marshal + ?Sized>(
    v: &T,
    prefix: &str,
    indent: &str,
) -> Result<Vec<u8>, Error> {
    let b = marshal(v)?;
    let mut b2 = Vec::with_capacity(2 * b.len());
    append_indent(&mut b2, &b, prefix, indent)?;
    Ok(b2)
}

/// Go `appendString` (`encode.go:998`); quotes included, invalid UTF-8 becomes `�` per byte.
pub fn append_string(dst: &mut Vec<u8>, s: &[u8], escape_html: bool) {
    dst.push(b'"');
    let mut start = 0;
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        if b < 0x80 {
            // Go's htmlSafeSet / safeSet (`tables.go`): printable ASCII (DEL included) except `"`
            // and `\`, and, with escape_html, `<`, `>` and `&`.
            if b >= 0x20
                && b != b'"'
                && b != b'\\'
                && !(escape_html && matches!(b, b'<' | b'>' | b'&'))
            {
                i += 1;
                continue;
            }
            dst.extend_from_slice(&s[start..i]);
            match b {
                b'\\' | b'"' => dst.extend_from_slice(&[b'\\', b]),
                b'\x08' => dst.extend_from_slice(b"\\b"),
                b'\x0c' => dst.extend_from_slice(b"\\f"),
                b'\n' => dst.extend_from_slice(b"\\n"),
                b'\r' => dst.extend_from_slice(b"\\r"),
                b'\t' => dst.extend_from_slice(b"\\t"),
                // bytes < 0x20 except \b, \f, \n, \r and \t, and, with escape_html, <, > and &
                _ => dst.extend_from_slice(&[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[usize::from(b >> 4)],
                    HEX[usize::from(b & 0xF)],
                ]),
            }
            i += 1;
            start = i;
            continue;
        }
        let (c, size) = decode_rune(&s[i..]);
        if c == RUNE_ERROR && size == 1 {
            dst.extend_from_slice(&s[start..i]);
            dst.extend_from_slice(b"\\ufffd");
            i += size;
            start = i;
            continue;
        }
        // U+2028 and U+2029 are escaped unconditionally (JSONP safety).
        if c == '\u{2028}' || c == '\u{2029}' {
            dst.extend_from_slice(&s[start..i]);
            dst.extend_from_slice(b"\\u202");
            dst.push(HEX[(c as usize) & 0xF]);
            i += size;
            start = i;
            continue;
        }
        i += size;
    }
    dst.extend_from_slice(&s[start..]);
    dst.push(b'"');
}

/// Go `floatEncoder.encode` for 64 bits (`encode.go:570`); NaN and the infinities give
/// `Error::UnsupportedValue`.
pub fn append_float(dst: &mut Vec<u8>, f: f64) -> Result<(), Error> {
    if f.is_infinite() || f.is_nan() {
        return Err(Error::UnsupportedValue(crate::strconv::format_float(
            f, b'g',
        )));
    }
    // Convert as if by ES6 number to string conversion: like fmt %g, but the exponent cutoffs are
    // different and exponents themselves are not padded to two digits.
    let abs = f.abs();
    let fmt = if abs != 0.0 && !(1e-6..1e21).contains(&abs) {
        b'e'
    } else {
        b'f'
    };
    crate::strconv::append_float(dst, f, fmt);
    if fmt == b'e' {
        // clean up e-09 to e-9
        let n = dst.len();
        if n >= 4 && dst[n - 4] == b'e' && dst[n - 3] == b'-' && dst[n - 2] == b'0' {
            dst[n - 2] = dst[n - 1];
            dst.truncate(n - 1);
        }
    }
    Ok(())
}

/// Go `Encoder` (`stream.go:195`).
pub struct Encoder<W: std::io::Write> {
    w: W,
    err: Option<Error>,
    prefix: String,
    indent: String,
    escape_html: bool,
}

impl<W: std::io::Write> Encoder<W> {
    /// Go `NewEncoder` (`stream.go:206`).
    pub fn new(w: W) -> Self {
        Encoder {
            w,
            err: None,
            prefix: String::new(),
            indent: String::new(),
            escape_html: true,
        }
    }

    /// Go `Encoder.SetIndent` (`stream.go:254`).
    pub fn set_indent(&mut self, prefix: &str, indent: &str) {
        self.prefix = prefix.to_string();
        self.indent = indent.to_string();
    }

    /// Go `Encoder.SetEscapeHTML` (`stream.go:266`).
    pub fn set_escape_html(&mut self, on: bool) {
        self.escape_html = on;
    }

    /// Go `Encoder.Encode` (`stream.go:216`): value, then `\n`; the newline is added before
    /// indenting, so the indented text keeps it as trailing whitespace. A write error is sticky.
    pub fn encode<T: Marshal + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        if let Some(err) = &self.err {
            return Err(err.clone());
        }
        let mut e = EncodeState {
            buf: Vec::new(),
            escape_html: self.escape_html,
        };
        v.marshal_json(&mut e)?;
        // Terminate each value with a newline.
        e.buf.push(b'\n');
        let b = if self.prefix.is_empty() && self.indent.is_empty() {
            e.buf
        } else {
            let mut out = Vec::new();
            append_indent(&mut out, &e.buf, &self.prefix, &self.indent)?;
            out
        };
        self.w.write_all(&b).map_err(|err| {
            let err = Error::Io(err.to_string());
            self.err = Some(err.clone());
            err
        })
    }
}
