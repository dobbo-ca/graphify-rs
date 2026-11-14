//! The encode side of Go `encoding/json` (GOEXPERIMENT=jsonv2): `Marshal`, `MarshalIndent`
//! (`v2_encode.go`) and the `Encoder` (`v2_stream.go`); the reflection encoders still cite v1
//! `encode.go`.

use std::collections::{BTreeMap, HashMap, btree_map};

use super::indent::indent as append_indent;
use super::jsontext::{MAX_NESTING_DEPTH, Opts, append_format};
use super::{Error, RawMessage, Value};
use crate::utf8::{RUNE_ERROR, decode_rune};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Go `encodeState` (`encode.go:~300`): the output buffer and the HTML-escape option. `depth`
/// is the number of open arrays and objects, Go's `len(Tokens.Stack)`.
#[derive(Debug, Clone, Default)]
pub struct EncodeState {
    pub buf: Vec<u8>,
    pub escape_html: bool,
    depth: usize,
    /// The depths of the objects that [`write_field`] opened and that are still open: only
    /// these does a hand-written `write_raw(b"}")` close (see [`EncodeState::write_raw`]).
    field_objects: Vec<usize>,
    /// Lone `{` written by `write_raw` and not yet closed by a lone `}`: those braces are the
    /// caller's own and never touch `depth`.
    raw_open: usize,
}

impl EncodeState {
    /// Writes `[` or `{` and enters it. Go's `pushArray`/`pushObject` (jsontext/state.go:308,343)
    /// refuse the 10001st open container with `errMaxDepth`; `WriteToken`
    /// (jsontext/encode.go:356,393-424) wraps it at the offset before the delimiter,
    /// `wrapSyntacticError` (jsontext/errors.go:102-115) adds one, and `transformMarshalError`
    /// (v2_inject.go:66) makes it a legacy `SyntaxError` (v2_scanner.go:67-71). An empty array or
    /// object skips `WriteToken` (v2/arshal_any.go:118-131,234-247, v2/arshal_default.go:824-836,
    /// 1504-1516), so callers write `[]` and `{}` without this check. graphify-go's values are
    /// trees, so Go's cycle check past depth 1000 (`startDetectingCyclesAfter`,
    /// v2/arshal_default.go:50) never fires and has no counterpart.
    fn open(&mut self, c: u8) -> Result<(), Error> {
        if self.depth == MAX_NESTING_DEPTH {
            return Err(Error::Syntax {
                msg: "exceeded max depth".to_string(),
                offset: self.buf.len() as i64 + 1,
            });
        }
        self.buf.push(c);
        self.depth += 1;
        Ok(())
    }

    /// Writes `]` or `}` and leaves the container [`open`](Self::open) entered.
    fn close(&mut self, c: u8) {
        self.buf.push(c);
        self.depth -= 1;
    }

    /// Ends an object whose fields went through [`write_field`] (a hand-written `Marshal` impl,
    /// a `json_fields!` struct, a map): `{}` if no field was written (`first` still true), else
    /// `}`, which leaves the depth level the first field entered. This is the way to end a
    /// hand-written object:
    ///
    /// ```ignore
    /// let mut first = true;
    /// write_field(e, &mut first, "a", &self.a, false)?;
    /// e.end_object(first);
    /// ```
    pub fn end_object(&mut self, first: bool) {
        if first {
            self.buf.extend_from_slice(b"{}");
        } else {
            if self.field_objects.last() == Some(&self.depth) {
                self.field_objects.pop();
            }
            self.close(b'}');
        }
    }

    /// Appends `b` verbatim; the caller guarantees it is valid JSON.
    ///
    /// Kept for hand-written impls written before [`end_object`](Self::end_object): a lone `}`
    /// that ends the innermost open object [`write_field`] opened is the same as
    /// `end_object(false)`, so the object does not leak a depth level toward the 10000 limit.
    /// A lone `}` that matches a lone raw `{` (an object the caller writes with raw braces)
    /// leaves the depth alone.
    pub fn write_raw(&mut self, b: &[u8]) {
        match b {
            b"{" => self.raw_open += 1,
            b"}" if self.raw_open > 0 => self.raw_open -= 1,
            b"}" if self.field_objects.last() == Some(&self.depth) => {
                self.field_objects.pop();
                self.close(b'}');
                return;
            }
            _ => {}
        }
        self.buf.extend_from_slice(b);
    }

    /// [`append_string`] with the current `escape_html`; quotes included.
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

    /// The result of a `MarshalJSON` goes through Go's `enc.WriteValue(val)`
    /// (`v2/arshal_methods.go:248`): validated and compacted, with `EscapeForHTML` as
    /// `escape_html` and `EscapeForJS` always on (v1 defaults,
    /// `internal/jsonflags/flags.go:51-72`).
    /// A syntax error becomes `Error::Marshaler` for `go_type` (`v2_inject.go:25`) with the legacy
    /// text of `transformMarshalError` (`v2_inject.go:63`), and the buffer is left as it was.
    pub fn write_marshaler_output(
        &mut self,
        go_type: &'static str,
        json: &[u8],
    ) -> Result<(), Error> {
        let o = Opts {
            decoder: false,
            indent: None,
            escape_html: self.escape_html,
            escape_js: true,
        };
        append_format(&mut self.buf, json, &o).map_err(|err| Error::Marshaler {
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
        if self.is_empty() {
            e.write_raw(b"[]");
            return Ok(());
        }
        e.open(b'[')?;
        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                e.write_raw(b",");
            }
            v.marshal_json(e)?;
        }
        e.close(b']');
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
    let mut first = true;
    for (k, v) in entries {
        if first {
            e.open(b'{')?;
        } else {
            e.write_raw(b",");
        }
        first = false;
        e.write_string(k.as_bytes());
        e.write_raw(b":");
        v.marshal_json(e)?;
    }
    e.end_object(first);
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
/// An open array or object of a [`Value`] walk and whether its next element is the first.
pub(super) enum Frame<'a> {
    Array(std::slice::Iter<'a, Value>, bool),
    Object(btree_map::Iter<'a, String, Value>, bool),
}

/// A Go `any` holding decoded JSON; `Null` is the nil interface.
impl Marshal for Value {
    /// Go `marshalValueAny` (v2/arshal_any.go:32-58) with an explicit stack instead of one call
    /// per level: a decoded `Value` can be 10000 levels deep and must encode on any thread.
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        let mut stack = Vec::new();
        let mut next = Some(self);
        loop {
            match next.take() {
                Some(Value::Array(a)) if !a.is_empty() => {
                    e.open(b'[')?;
                    stack.push(Frame::Array(a.iter(), true));
                }
                Some(Value::Object(m)) if !m.is_empty() => {
                    e.open(b'{')?;
                    stack.push(Frame::Object(m.iter(), true));
                }
                Some(Value::Array(_)) => e.write_raw(b"[]"),
                Some(Value::Object(_)) => e.write_raw(b"{}"),
                Some(Value::Null) => e.write_null(),
                Some(Value::Bool(b)) => e.write_bool(*b),
                Some(Value::Number(f)) => e.write_float(*f)?,
                Some(Value::String(s)) => e.write_string(s.as_bytes()),
                None => {}
            }
            match stack.last_mut() {
                None => return Ok(()),
                Some(Frame::Array(it, first)) => match it.next() {
                    Some(v) => {
                        if !std::mem::take(first) {
                            e.write_raw(b",");
                        }
                        next = Some(v);
                    }
                    None => {
                        e.close(b']');
                        stack.pop();
                    }
                },
                Some(Frame::Object(it, first)) => match it.next() {
                    Some((k, v)) => {
                        if !std::mem::take(first) {
                            e.write_raw(b",");
                        }
                        e.write_string(k.as_bytes());
                        e.write_raw(b":");
                        next = Some(v);
                    }
                    None => {
                        e.close(b'}');
                        stack.pop();
                    }
                },
            }
        }
    }
    fn is_empty_value(&self) -> bool {
        matches!(self, Value::Null)
    }
}
/// Go `RawMessage.MarshalJSON` (`jsontext/value.go:234`) through `WriteValue`. An empty
/// `RawMessage` is Go's nil one, `null`. Invalid JSON is a `MarshalerError` whose type is
/// `reflect.TypeOf(va.Addr().Interface())` (v2/arshal_methods.go:248-251, v2_inject.go:25-27):
/// `RawMessage` is an alias of `jsontext.Value` (v2_stream.go:190), so `*jsontext.Value`.
impl Marshal for RawMessage {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        if self.0.is_empty() {
            return e.write_marshaler_output("*jsontext.Value", b"null");
        }
        e.write_marshaler_output("*jsontext.Value", &self.0)
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
    if *first {
        e.open(b'{')?;
        e.field_objects.push(e.depth);
    } else {
        e.write_raw(b",");
    }
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
        depth: 0,
        field_objects: Vec::new(),
        raw_open: 0,
    };
    v.marshal_json(&mut e)?;
    Ok(e.buf)
}

/// Go `MarshalIndent` (`v2_encode.go:189`): `Marshal`, then `appendIndent`.
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

/// Go `jsonwire.AppendQuote` (`internal/jsonwire/encode.go:65`) with `EscapeForJS` on and
/// `EscapeForHTML` as `escape_html`; quotes included, each invalid UTF-8 byte becomes the raw
/// bytes of U+FFFD (EF BF BD), not v1's six-byte escape text.
pub fn append_string(dst: &mut Vec<u8>, s: &[u8], escape_html: bool) {
    dst.push(b'"');
    let mut start = 0;
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        if b < 0x80 {
            // Go's `escapeASCII` (`internal/jsonwire/encode.go:20`): printable ASCII (DEL
            // included) except `"` and `\`, and, with escape_html, `<`, `>` and `&`.
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
            dst.extend_from_slice("\u{FFFD}".as_bytes());
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

/// Go `Encoder` (`v2_stream.go:112`).
pub struct Encoder<W: std::io::Write> {
    w: W,
    err: Option<Error>,
    prefix: String,
    indent: String,
    escape_html: bool,
}

impl<W: std::io::Write> Encoder<W> {
    /// Go `NewEncoder` (`v2_stream.go:124`).
    pub fn new(w: W) -> Self {
        Encoder {
            w,
            err: None,
            prefix: String::new(),
            indent: String::new(),
            escape_html: true,
        }
    }

    /// Go `Encoder.SetIndent` (`v2_stream.go:166`).
    pub fn set_indent(&mut self, prefix: &str, indent: &str) {
        self.prefix = prefix.to_string();
        self.indent = indent.to_string();
    }

    /// Go `Encoder.SetEscapeHTML` (`v2_stream.go:181`).
    pub fn set_escape_html(&mut self, on: bool) {
        self.escape_html = on;
    }

    /// Go `Encoder.Encode` (`v2_stream.go:136`): the value, indented if set, then `\n` (added
    /// after indenting, so the prefix never follows it). A write error is sticky.
    pub fn encode<T: Marshal + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        if let Some(err) = &self.err {
            return Err(err.clone());
        }
        let mut e = EncodeState {
            buf: Vec::new(),
            escape_html: self.escape_html,
            depth: 0,
            field_objects: Vec::new(),
            raw_open: 0,
        };
        v.marshal_json(&mut e)?;
        let mut b = if self.prefix.is_empty() && self.indent.is_empty() {
            e.buf
        } else {
            let mut out = Vec::new();
            append_indent(&mut out, &e.buf, &self.prefix, &self.indent)?;
            out
        };
        b.push(b'\n');
        self.w.write_all(&b).map_err(|err| {
            let err = Error::Io(err.to_string());
            self.err = Some(err.clone());
            err
        })
    }
}
