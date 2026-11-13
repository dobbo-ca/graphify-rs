//! Go 1.27.4 `encoding/json`, decode half. Go 1.27.4 builds it with GOEXPERIMENT=jsonv2 on, so
//! `Unmarshal` is `jsonv2.Unmarshal(data, v, DefaultOptionsV1())` (`v2_decode.go:110`) with its
//! errors mapped by `transformUnmarshalError` (`v2_inject.go:69-159`). The v1 flags that shape
//! decoding (`internal/jsonflags/flags.go:51-72`): `AllowDuplicateNames` (a repeated key is
//! decoded again), `AllowInvalidUTF8` (bad bytes become U+FFFD), `MatchCaseInsensitiveNames` with
//! `MatchCaseSensitiveDelimiter` (`strings.EqualFold` key match), `MergeWithLegacySemantics`
//! (`null` leaves scalars and structs alone, decoding merges into what is there) and
//! `ReportErrorsWithLegacySemantics` (the whole value is validated first, `v2/arshal.go:455-464`;
//! a type error does not stop decoding, only a syntax error is fatal, `v2/errors.go:62-65`).

use std::collections::{BTreeMap, HashMap};

use super::scanner::check_valid;
use super::{Error, RawMessage, UnmarshalTypeError, Value};
use crate::strings::equal_fold;
use crate::utf8::decode_rune;

/// One JSON Pointer token of the value being decoded.
enum Seg {
    /// An object name: the byte range of its quoted text in the input, unquoted on demand.
    Key(usize, usize),
    /// An array index.
    Index(usize),
}

/// Go's `jsontext.Decoder` state as `arshal_default.go` drives it: a cursor over input that
/// `check_valid` already accepted, the first saved error and the error context.
///
/// The error context follows v2 (`v2_inject.go:137-146`): `struct_name` holds the root type's Go
/// `Name()`, fixed for the whole decode (the root's `GO_TYPE` without its package qualifier, e.g.
/// `json.PP` gives `PP`; empty for an unnamed root: `struct { ... }`, a map, a slice, an `Option`
/// (Go pointer)), and `path` is the JSON Pointer of the current value
/// (`jsontext/state.go:181-218`): every object name (struct field and map key alike) and every
/// array index is pushed while its value decodes. A saved error gets the path joined with `.`,
/// and `struct_name` only when the pointer is non-empty.
///
/// Reading methods never fail: the input is valid. Input of the wrong kind is a *type* error: it
/// is recorded (the first one is kept), the value is skipped and `None` is returned, as Go keeps
/// decoding after a semantic error (`v2/errors.go:62-65`). `null` is never a type error: every
/// reader consumes it and returns `None` (each Go arshaler accepts `null`).
pub struct DecodeState<'a> {
    data: &'a [u8],
    off: usize,
    saved_error: Option<Error>,
    struct_name: String,
    path: Vec<Seg>,
    /// `go_type` of the value [`value`](Self::value) is decoding; the Go type of a type error
    /// raised by a reader that takes no explicit type.
    cur_type: fn() -> String,
}

impl<'a> DecodeState<'a> {
    /// A state over `data`, already validated by `check_valid`; `struct_name` is the root's Go
    /// `Name()`.
    fn new(data: &'a [u8], struct_name: String) -> Self {
        DecodeState {
            data,
            off: 0,
            saved_error: None,
            struct_name,
            path: Vec::new(),
            cur_type: String::new,
        }
    }

    /// Go `jsonwire.ConsumeWhitespace` (internal/jsonwire/decode.go:33).
    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\r' | b'\n') = self.data.get(self.off) {
            self.off += 1;
        }
    }

    /// The first byte of the next value (Go `PeekKind`).
    fn peek(&mut self) -> u8 {
        self.ws();
        self.data[self.off]
    }

    /// The end of the string whose opening quote is at `pos`.
    fn string_end(&self, pos: usize) -> usize {
        let mut i = pos + 1;
        loop {
            match self.data[i] {
                b'"' => return i + 1,
                b'\\' => i += 2,
                _ => i += 1,
            }
        }
    }

    /// Go `SkipValue`: past the next value, without recursion (it may be 10000 levels deep).
    fn skip(&mut self) {
        self.ws();
        let mut depth = 0usize;
        loop {
            match self.data[self.off] {
                b'"' => self.off = self.string_end(self.off),
                b'{' | b'[' => {
                    depth += 1;
                    self.off += 1;
                }
                b'}' | b']' => {
                    depth -= 1;
                    self.off += 1;
                }
                // Separators and whitespace occur only inside a container.
                b',' | b':' | b' ' | b'\t' | b'\r' | b'\n' => self.off += 1,
                b't' | b'n' => self.off += 4,
                b'f' => self.off += 5,
                _ => {
                    self.off += 1;
                    while let Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') =
                        self.data.get(self.off)
                    {
                        self.off += 1;
                    }
                }
            }
            if depth == 0 {
                return;
            }
        }
    }

    /// Records a type error for the next value and skips it. Go `newUnmarshalErrorAfter`
    /// (`v2/errors.go:164-178`): the offset is past the value, or past only its `[` or `{`; the
    /// value text is the kind (`v2_inject.go:103-113`). `go_type` `None` is the current type.
    fn mismatch(&mut self, go_type: Option<&str>) {
        let start = self.off;
        let c = self.data[start];
        self.skip();
        if self.saved_error.is_some() {
            return;
        }
        let value = match c {
            b'"' => "string",
            b't' | b'f' => "bool",
            b'[' => "array",
            b'{' => "object",
            b'n' => "null",
            _ => "number",
        };
        let offset = if matches!(c, b'[' | b'{') {
            start + 1
        } else {
            self.off
        };
        let go_type = go_type.map_or_else(self.cur_type, str::to_string);
        self.record(value.to_string(), go_type, offset);
    }

    /// Saves a type error unless one is saved, as Go's `cmp.Or(errUnmarshal, err)` keeps the
    /// first (`v2/arshal_default.go:1411`). `Struct` and `Field` per `v2_inject.go:137-146`.
    fn record(&mut self, value: String, go_type: String, offset: usize) {
        if self.saved_error.is_some() {
            return;
        }
        let field = self
            .path
            .iter()
            .map(|s| match *s {
                Seg::Index(i) => i.to_string(),
                // Go `appendEscapePointerName` (jsontext/state.go:205-218).
                Seg::Key(a, b) => unquote(&self.data[a..b])
                    .replace('~', "~0")
                    .replace('/', "~1"),
            })
            .collect::<Vec<_>>()
            .join(".");
        let struct_name = if self.path.is_empty() {
            String::new()
        } else {
            self.struct_name.clone()
        };
        self.saved_error = Some(Error::UnmarshalType(Box::new(UnmarshalTypeError {
            value,
            go_type,
            offset: offset as i64,
            struct_name,
            field,
            err: None,
        })));
    }

    /// Decodes the next value into `v` with `T`'s arshaler, as Go's container arshalers call
    /// the element's `unmarshal`. An `Err` from `v` is Go's error from an `UnmarshalJSON`
    /// method: returned verbatim (`v2/arshal_methods.go:347-351`) but not fatal
    /// (`v2/errors.go:62-65`), so it is saved if it is the first error, the rest of the value
    /// is skipped and `Ok` is returned.
    pub fn value<T: Unmarshal>(&mut self, v: &mut T) -> Result<(), Error> {
        self.ws();
        let (start, depth, outer) = (self.off, self.path.len(), self.cur_type);
        self.cur_type = T::go_type;
        let r = v.unmarshal_json(self);
        self.cur_type = outer;
        if let Err(e) = r {
            self.path.truncate(depth);
            self.off = start;
            self.skip();
            if self.saved_error.is_none() {
                self.saved_error = Some(e);
            }
        }
        Ok(())
    }

    /// Whether the next value is the literal `null`. Does not consume it: an impl calls
    /// [`skip_value`](Self::skip_value) after deciding, e.g. `Option<T>` becomes `None`.
    pub fn peek_null(&mut self) -> bool {
        self.peek() == b'n'
    }

    /// Reads a JSON string as Go's string arshaler (`v2/arshal_default.go:257-303`): unquoted by
    /// `jsonwire.AppendUnquote` with invalid UTF-8 and lone surrogates as U+FFFD. `null` yields
    /// `None`; another kind records a type error (`"number"`, `"bool"`, `"array"`, ...) against
    /// the current type and yields `None`.
    pub fn read_string(&mut self) -> Result<Option<String>, Error> {
        Ok(match self.peek() {
            b'"' => {
                let start = self.off;
                self.off = self.string_end(start);
                Some(unquote(&self.data[start..self.off]))
            }
            b'n' => {
                self.off += 4;
                None
            }
            _ => {
                self.mismatch(None);
                None
            }
        })
    }

    /// The literal text of the next number; `null` and other kinds as in `read_string`.
    fn number(&mut self, go_type: Option<&str>) -> Option<&'a str> {
        match self.peek() {
            b'-' | b'0'..=b'9' => {
                let start = self.off;
                self.skip();
                let data: &'a [u8] = self.data;
                Some(std::str::from_utf8(&data[start..self.off]).expect("a JSON number is ASCII"))
            }
            b'n' => {
                self.off += 4;
                None
            }
            _ => {
                self.mismatch(go_type);
                None
            }
        }
    }

    /// Reads a JSON number and returns its literal text, e.g. `"1e400"`. `null` yields `None`;
    /// another kind records a type error against the current type and yields `None`.
    pub fn read_number_literal(&mut self) -> Result<Option<&'a str>, Error> {
        Ok(self.number(None))
    }

    /// Reads a JSON number as `f64`, Go's float arshaler (`v2/arshal_default.go:698-774`):
    /// `strconv.ParseFloat`; out of range (`1e400`) records `"number 1e400"` against `go_type`
    /// (`v2_inject.go:114-136`) and yields `None`, as does another kind (`"string"`, ...).
    pub fn read_f64(&mut self, go_type: &str) -> Result<Option<f64>, Error> {
        let Some(lit) = self.number(Some(go_type)) else {
            return Ok(None);
        };
        match crate::strconv::parse_float(lit) {
            Ok(f) => Ok(Some(f)),
            Err(_) => {
                self.record(format!("number {lit}"), go_type.to_string(), self.off);
                Ok(None)
            }
        }
    }

    /// Reads a JSON integer as `i64`, Go's int arshaler (`v2/arshal_default.go:492-563`): a
    /// fraction, exponent or more than `bits` bits records `"number <literal>"` against
    /// `go_type` and yields `None`.
    pub fn read_i64(&mut self, go_type: &str, bits: u32) -> Result<Option<i64>, Error> {
        let Some(lit) = self.number(Some(go_type)) else {
            return Ok(None);
        };
        let neg = lit.starts_with('-');
        let (n, ok) = parse_uint(&lit.as_bytes()[usize::from(neg)..]);
        let max_int = 1u64 << (bits - 1);
        let overflow = (neg && n > max_int) || (!neg && n > max_int - 1);
        if !ok || overflow {
            self.record(format!("number {lit}"), go_type.to_string(), self.off);
            return Ok(None);
        }
        // Go `int64(-n)` wraps for n = 1<<63.
        Ok(Some(if neg {
            n.wrapping_neg() as i64
        } else {
            n as i64
        }))
    }

    /// As [`read_i64`](Self::read_i64) for unsigned integers, Go's uint arshaler
    /// (`v2/arshal_default.go:591-653`): a minus sign is a syntax error (`"number -5"`).
    pub fn read_u64(&mut self, go_type: &str, bits: u32) -> Result<Option<u64>, Error> {
        let Some(lit) = self.number(Some(go_type)) else {
            return Ok(None);
        };
        let (n, ok) = parse_uint(lit.as_bytes());
        // Go `uint64(1) << 64` is 0, and 0 - 1 wraps to the maximum.
        let max_uint = 1u64.checked_shl(bits).unwrap_or(0);
        if !ok || n > max_uint.wrapping_sub(1) {
            self.record(format!("number {lit}"), go_type.to_string(), self.off);
            return Ok(None);
        }
        Ok(Some(n))
    }

    /// Reads `true` or `false`, Go's bool arshaler (`v2/arshal_default.go:160-206`). `null`
    /// yields `None`; another kind records a type error against the current type.
    pub fn read_bool(&mut self) -> Result<Option<bool>, Error> {
        Ok(match self.peek() {
            b't' => {
                self.off += 4;
                Some(true)
            }
            b'f' => {
                self.off += 5;
                Some(false)
            }
            b'n' => {
                self.off += 4;
                None
            }
            _ => {
                self.mismatch(None);
                None
            }
        })
    }

    /// Iterates the members of the next JSON object (Go's map arshaler loop,
    /// `v2/arshal_default.go:1019-1077`), calling `f(self, name)` once per member in document
    /// order, duplicates included; `f` must consume the value (decode or
    /// [`skip_value`](Self::skip_value)). Names are unquoted as in
    /// [`read_string`](Self::read_string). The name is on the field path while `f` runs
    /// (`data.test2`, `v2_decode_test.go:1058`). `null` calls `f` zero times; another kind
    /// records a type error (`"object"`) against the current type and calls `f` zero times.
    pub fn read_object(
        &mut self,
        mut f: impl FnMut(&mut DecodeState<'a>, String) -> Result<(), Error>,
    ) -> Result<(), Error> {
        match self.peek() {
            b'{' => self.off += 1,
            b'n' => {
                self.off += 4;
                return Ok(());
            }
            _ => {
                self.mismatch(None);
                return Ok(());
            }
        }
        if self.peek() == b'}' {
            self.off += 1;
            return Ok(());
        }
        loop {
            let start = self.off;
            let end = self.string_end(start);
            self.off = end;
            let name = unquote(&self.data[start..end]);
            self.ws();
            self.off += 1; // ':'
            self.path.push(Seg::Key(start, end));
            let r = f(self, name);
            self.path.pop();
            r?;
            let c = self.peek();
            self.off += 1;
            if c == b'}' {
                return Ok(());
            }
            self.ws();
        }
    }

    /// As [`read_object`](Self::read_object) for a struct whose fields have the JSON names
    /// `keys` (a `"-"` key is no field). Go's struct arshaler (`v2/arshal_default.go:1288-1419`)
    /// under the v1 flags: a name with an exact match takes that field, else the first field (in
    /// `keys` order) whose name is `strings.EqualFold` to it (`v2/arshal_default.go:1326-1337`,
    /// `v2/fields.go:380-386`; `MatchCaseSensitiveDelimiter` keeps only `EqualFold` matches of
    /// the `foldName` candidates, and `EqualFold` implies equal `foldName`s). `f(self, key)` gets
    /// the matched entry of `keys`; an unknown name's value is skipped. A repeated name decodes
    /// into the field again, so the last one wins (`AllowDuplicateNames`).
    pub fn read_struct(
        &mut self,
        keys: &[&str],
        mut f: impl FnMut(&mut DecodeState<'a>, &str) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.read_object(|d, name| {
            let field = keys
                .iter()
                .find(|k| **k != "-" && **k == name.as_str())
                .or_else(|| keys.iter().find(|k| **k != "-" && equal_fold(&name, k)));
            match field {
                Some(k) => f(d, k),
                None => d.skip_value(),
            }
        })
    }

    /// Iterates the elements of the next JSON array (Go's slice arshaler loop,
    /// `v2/arshal_default.go:1577-1596`), calling `f(self, index)` per element; `f` must consume
    /// the element. The index is on the field path while `f` runs (`Ts.2.Y`,
    /// `v2_decode_test.go:1102`). `null` calls `f` zero times; another kind records a type error
    /// (`"array"`) against the current type and calls `f` zero times.
    pub fn read_array(
        &mut self,
        mut f: impl FnMut(&mut DecodeState<'a>, usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        match self.peek() {
            b'[' => self.off += 1,
            b'n' => {
                self.off += 4;
                return Ok(());
            }
            _ => {
                self.mismatch(None);
                return Ok(());
            }
        }
        if self.peek() == b']' {
            self.off += 1;
            return Ok(());
        }
        let mut i = 0;
        loop {
            self.path.push(Seg::Index(i));
            let r = f(self, i);
            self.path.pop();
            r?;
            i += 1;
            let c = self.peek();
            self.off += 1;
            if c == b']' {
                return Ok(());
            }
        }
    }

    /// Skips the next value of any kind (Go `SkipValue`), e.g. an unknown struct key.
    pub fn skip_value(&mut self) -> Result<(), Error> {
        self.skip();
        Ok(())
    }

    /// Takes the raw bytes of the next value and consumes it: Go `Decoder.ReadValue`, the input
    /// text without the leading whitespace, escapes and inner whitespace kept, as passed to
    /// `UnmarshalJSON` (`v2/arshal_methods.go:342`). The slice is a view of the input.
    pub fn raw_value(&mut self) -> Result<&'a [u8], Error> {
        self.ws();
        let start = self.off;
        self.skip();
        let data: &'a [u8] = self.data;
        Ok(&data[start..self.off])
    }

    /// Records a type error at the bytes read so far, with the root name and field path
    /// described on [`DecodeState`] and `err: None`; only the first error is kept. `value` is
    /// Go's description (`"bool"`, `"array"`, `"number -5"`); `go_type` is the target's Go type
    /// string. Decoding continues and [`unmarshal_into`] returns the saved error at the end.
    pub fn save_type_error(&mut self, value: &str, go_type: &str) {
        self.record(value.to_string(), go_type.to_string(), self.off);
    }
}

/// Go `jsonwire.ParseUint` (internal/jsonwire/decode.go:593-606): `ok` false with the maximum
/// for an overflow, with 0 for anything but plain digits. The digit loop wraps as Go's does.
fn parse_uint(b: &[u8]) -> (u64, bool) {
    let n = b.iter().take_while(|c| c.is_ascii_digit()).count();
    let v = b[..n].iter().fold(0u64, |v, &c| {
        v.wrapping_mul(10).wrapping_add(u64::from(c - b'0'))
    });
    if n == 0 || b.len() != n || (b[0] == b'0' && b != b"0") {
        return (0, false);
    }
    if n >= 20 && (b[0] != b'1' || v < 10_000_000_000_000_000_000 || n > 20) {
        return (u64::MAX, false);
    }
    (v, true)
}

/// The value of 4 hex digits.
fn hex4(b: &[u8]) -> u32 {
    b.iter().fold(0, |v, &c| {
        (v << 4) | char::from(c).to_digit(16).unwrap_or(0)
    })
}

/// Go `jsonwire.AppendUnquote` (internal/jsonwire/decode.go:257-385) of a valid quoted string,
/// keeping going past the errors `AllowInvalidUTF8` tolerates: an invalid UTF-8 byte is one
/// U+FFFD (`utf8.DecodeRune` width 1), and a surrogate escape not followed by its pair is
/// U+FFFD with the next escape read on its own.
fn unquote(q: &[u8]) -> String {
    let s = &q[1..q.len() - 1];
    if !s.contains(&b'\\')
        && let Ok(t) = std::str::from_utf8(s)
    {
        return t.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut n = 0;
    while n < s.len() {
        if s[n] != b'\\' {
            let (r, w) = decode_rune(&s[n..]);
            out.push(r);
            n += w;
            continue;
        }
        match s[n + 1] {
            b'u' => {
                let v1 = hex4(&s[n + 2..n + 6]);
                n += 6;
                let r = if (0xD800..0xE000).contains(&v1) {
                    // utf16.DecodeRune: a high half then a low half, else U+FFFD.
                    let v2 = (s.get(n) == Some(&b'\\') && s.get(n + 1) == Some(&b'u'))
                        .then(|| hex4(&s[n + 2..n + 6]));
                    match v2 {
                        Some(v2)
                            if (0xD800..0xDC00).contains(&v1) && (0xDC00..0xE000).contains(&v2) =>
                        {
                            n += 6;
                            0x10000 + ((v1 - 0xD800) << 10) + (v2 - 0xDC00)
                        }
                        _ => 0xFFFD,
                    }
                } else {
                    v1
                };
                out.push(char::from_u32(r).unwrap_or('\u{FFFD}'));
            }
            c => {
                out.push(match c {
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    _ => char::from(c), // '"', '\\', '/'
                });
                n += 2;
            }
        }
    }
    out
}

/// Go `reflect.Type.Name()` from a type string: the name after the package qualifier of a
/// defined type (`json.PP` gives `PP`, `int` gives `int`), empty for a type literal (`[]T`,
/// `map[K]V`, `*T`, `struct { ... }`, `interface {}`).
fn go_name(t: &str) -> &str {
    if t.contains([' ', '[', '*', '{', '(']) {
        ""
    } else {
        t.rsplit_once('.').map_or(t, |(_, n)| n)
    }
}

/// Decodes JSON into an existing Go value, as Go's arshaler for the type does under the v1
/// flags: `null` leaves a scalar or struct unchanged and sets an `Option`, `Vec`, map or
/// [`Value`] to `None`/empty/`Null`; objects merge into existing maps and structs; arrays refill
/// a slice from index 0, decoding into the elements already there. Go `Unmarshaler`.
///
/// `GO_TYPE` is the Go type string used in error text (`reflect.Type.String()`). For the generic
/// containers it is only a placeholder prefix (`"[]"`, `"*"`, `"map[string]"`) because an
/// associated const cannot embed `T::GO_TYPE`; use [`go_type`](Self::go_type) for error text.
///
/// An `Err` returned by an impl is Go's error from an `UnmarshalJSON` method: it comes back from
/// [`unmarshal_into`] verbatim if it is the first error (see [`DecodeState::value`]).
pub trait Unmarshal {
    /// The Go type string, e.g. `"int"`, `"main.Node"`.
    const GO_TYPE: &'static str;

    /// Go's `reflect.Type.String()` of the target, used in `UnmarshalTypeError`. Defaults to
    /// `GO_TYPE`; the container impls compose their element's.
    fn go_type() -> String {
        Self::GO_TYPE.to_string()
    }

    /// Decodes the next value of `d` into `self`. A nested value is decoded with
    /// [`DecodeState::value`].
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error>;
}

/// Go's bool arshaler (`v2/arshal_default.go:160-206`).
impl Unmarshal for bool {
    const GO_TYPE: &'static str = "bool";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(b) = d.read_bool()? {
            *self = b;
        }
        Ok(())
    }
}

/// Go's int arshaler (`v2/arshal_default.go:492-563`) for `int` (64 bits).
impl Unmarshal for i64 {
    const GO_TYPE: &'static str = "int";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(n) = d.read_i64(Self::GO_TYPE, 64)? {
            *self = n;
        }
        Ok(())
    }
}

/// Go's int arshaler for `int32`.
impl Unmarshal for i32 {
    const GO_TYPE: &'static str = "int32";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(n) = d.read_i64(Self::GO_TYPE, 32)? {
            *self = n as i32;
        }
        Ok(())
    }
}

/// Go's uint arshaler (`v2/arshal_default.go:591-653`) for `uint64`.
impl Unmarshal for u64 {
    const GO_TYPE: &'static str = "uint64";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(n) = d.read_u64(Self::GO_TYPE, 64)? {
            *self = n;
        }
        Ok(())
    }
}

/// Go's uint arshaler for `uint32`.
impl Unmarshal for u32 {
    const GO_TYPE: &'static str = "uint32";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(n) = d.read_u64(Self::GO_TYPE, 32)? {
            *self = n as u32;
        }
        Ok(())
    }
}

/// Go's uint arshaler for `uint` (64 bits).
impl Unmarshal for usize {
    const GO_TYPE: &'static str = "uint";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(n) = d.read_u64(Self::GO_TYPE, 64)? {
            *self = n as usize;
        }
        Ok(())
    }
}

/// Go's float arshaler (`v2/arshal_default.go:698-774`); out of range leaves the value as is.
impl Unmarshal for f64 {
    const GO_TYPE: &'static str = "float64";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(f) = d.read_f64(Self::GO_TYPE)? {
            *self = f;
        }
        Ok(())
    }
}

/// Go's string arshaler (`v2/arshal_default.go:257-303`).
impl Unmarshal for String {
    const GO_TYPE: &'static str = "string";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(s) = d.read_string()? {
            *self = s;
        }
        Ok(())
    }
}

/// Go's interface arshaler for `any` (`v2/arshal_default.go:1860-1965`) under
/// `MergeWithLegacySemantics`: the old value is dropped and a fresh `bool`, `string`, `float64`,
/// `map[string]any` or `[]any` decoded; `null` is `Null`. A number out of range is a type error
/// against `float64` and leaves `Number(0)`, the fresh `float64` (`v2_decode_test.go:2570-2575`).
impl Unmarshal for Value {
    const GO_TYPE: &'static str = "interface {}";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        *self = match d.peek() {
            b'n' => {
                d.skip();
                Value::Null
            }
            b't' | b'f' => Value::Bool(d.read_bool()?.unwrap_or_default()),
            b'"' => Value::String(d.read_string()?.unwrap_or_default()),
            b'[' => {
                let mut a = Vec::new();
                d.read_array(|d, _| {
                    let mut v = Value::Null;
                    d.value(&mut v)?;
                    a.push(v);
                    Ok(())
                })?;
                Value::Array(a)
            }
            b'{' => {
                let mut m = BTreeMap::new();
                d.read_object(|d, k| {
                    let mut v = Value::Null;
                    d.value(&mut v)?;
                    m.insert(k, v);
                    Ok(())
                })?;
                Value::Object(m)
            }
            _ => Value::Number(d.read_f64("float64")?.unwrap_or(0.0)),
        };
        Ok(())
    }
}

/// Go `RawMessage` (`jsontext.Value`) `UnmarshalJSON` (jsontext/value.go:244-251): a copy of the
/// raw value text, `null` included. `RawMessage` is an alias of `jsontext.Value`
/// (`v2_stream.go:190`), so error texts name that type.
impl Unmarshal for RawMessage {
    const GO_TYPE: &'static str = "jsontext.Value";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        let raw = d.raw_value()?;
        self.0.clear();
        self.0.extend_from_slice(raw);
        Ok(())
    }
}

/// Go's pointer arshaler (`v2/arshal_default.go:1755-1795`): `null` sets `None`, else allocates a
/// `T::default()` when `None` and decodes into it.
impl<T: Unmarshal + Default> Unmarshal for Option<T> {
    const GO_TYPE: &'static str = "*";
    fn go_type() -> String {
        format!("*{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if d.peek_null() {
            d.skip();
            *self = None;
            return Ok(());
        }
        // `Option<Vec<_>>` and `Option<map>` stand for Go's nil-able slice and map: their
        // arshalers (`v2/arshal_default.go:1541-1611`, `:958-1069`) check the token kind before
        // they allocate, so a value of the wrong kind leaves a nil slice or map nil. The
        // temporary records the type error and skips the value.
        let opener = match T::GO_TYPE {
            "[]" => Some(b'['),
            "map[string]" => Some(b'{'),
            _ => None,
        };
        if self.is_none() && opener.is_some_and(|o| d.peek() != o) {
            return d.value(&mut T::default());
        }
        d.value(self.get_or_insert_with(T::default))
    }
}

/// Go's slice arshaler (`v2/arshal_default.go:1541-1611`) under `MergeWithLegacySemantics`:
/// `null` empties it; element `i` decodes into the element already there (a new one is
/// `T::default()`), then the slice is cut to the array's length.
impl<T: Unmarshal + Default> Unmarshal for Vec<T> {
    const GO_TYPE: &'static str = "[]";
    fn go_type() -> String {
        format!("[]{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        let kind = d.peek();
        if kind == b'n' {
            d.skip();
            self.clear();
            return Ok(());
        }
        let mut n = 0;
        d.read_array(|d, i| {
            if i == self.len() {
                self.push(T::default());
            }
            n = i + 1;
            d.value(&mut self[i])
        })?;
        if kind == b'[' {
            self.truncate(n);
        }
        Ok(())
    }
}

/// Go's map arshaler (`v2/arshal_default.go:958-1069`) into a `map[string]T`: `null` empties it;
/// existing keys are kept, and each member decodes into a fresh `T::default()` (legacy merge
/// zeroes the old value, :1052-1056) that is stored even when it fails (:1062-1063).
impl<T: Unmarshal + Default> Unmarshal for BTreeMap<String, T> {
    const GO_TYPE: &'static str = "map[string]";
    fn go_type() -> String {
        format!("map[string]{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if d.peek_null() {
            d.skip();
            self.clear();
            return Ok(());
        }
        d.read_object(|d, k| {
            let mut v = T::default();
            d.value(&mut v)?;
            self.insert(k, v);
            Ok(())
        })
    }
}

/// As the `BTreeMap` impl, for a hash map.
impl<T: Unmarshal + Default> Unmarshal for HashMap<String, T> {
    const GO_TYPE: &'static str = "map[string]";
    fn go_type() -> String {
        format!("map[string]{}", T::go_type())
    }
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if d.peek_null() {
            d.skip();
            self.clear();
            return Ok(());
        }
        d.read_object(|d, k| {
            let mut v = T::default();
            d.value(&mut v)?;
            self.insert(k, v);
            Ok(())
        })
    }
}

/// Go `Unmarshal` (`v2_decode.go:110`) into a fresh `T::default()`.
pub fn unmarshal<T: Unmarshal + Default + Send>(data: &[u8]) -> Result<T, Error> {
    let mut v = T::default();
    unmarshal_into(data, &mut v)?;
    Ok(v)
}

/// Nesting depth above which decoding moves to its own thread (see [`unmarshal_into`]).
const DEEP: usize = 256;

/// Stack of that thread. Go's goroutine stacks grow to 1 GB; the decoder recurses once per
/// nesting level and the syntax check caps nesting at 10000 (`jsontext/state.go:46`). The
/// memory is reserved, not committed.
const DEEP_STACK: usize = 1 << 30;

/// The deepest nesting of `data`, which `check_valid` has accepted (strings are skipped).
fn max_depth(data: &[u8]) -> usize {
    let (mut depth, mut max, mut in_str, mut esc) = (0usize, 0, false, false);
    for &b in data {
        if in_str {
            match b {
                _ if esc => esc = false,
                b'\\' => esc = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => {
                depth += 1;
                max = max.max(depth);
            }
            b']' | b'}' => depth -= 1,
            _ => {}
        }
    }
    max
}

/// Go `Unmarshal` (`v2_decode.go:110`) into `v`: the whole input is validated first, so a syntax
/// error wins and nothing is written (`v2/arshal.go:455-464`), then decoded; the first saved
/// error is returned after decoding finishes. The root name for the error context is `T`'s
/// (`v2_inject.go:137-143`).
///
/// The syntax check rejects nesting over 10000 levels with Go's "exceeded max depth" error.
/// Below that, a document nested more than [`DEEP`] levels decodes on a scoped thread with a
/// [`DEEP_STACK`] stack, so the recursion never depends on the caller's stack size (a spawned
/// Rust thread has 2 MiB). A panic in an `Unmarshal` impl still reaches the caller.
pub fn unmarshal_into<T: Unmarshal + Send>(data: &[u8], v: &mut T) -> Result<(), Error> {
    check_valid(data)?;
    if max_depth(data) > DEEP {
        return std::thread::scope(|s| {
            match std::thread::Builder::new()
                .stack_size(DEEP_STACK)
                .spawn_scoped(s, || decode_valid(data, v))
                .expect("json: spawn the decode thread")
                .join()
            {
                Ok(r) => r,
                Err(p) => std::panic::resume_unwind(p),
            }
        });
    }
    decode_valid(data, v)
}

/// The decode half of [`unmarshal_into`], after the syntax check.
fn decode_valid<T: Unmarshal>(data: &[u8], v: &mut T) -> Result<(), Error> {
    let root = T::go_type();
    let mut d = DecodeState::new(data, go_name(&root).to_string());
    d.value(v)?;
    match d.saved_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
