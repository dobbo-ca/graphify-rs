//! Go 1.27.4 `strconv` as graphify-go calls it: `Quote` (through `fmt %q`), float formatting
//! with precision -1 (`FormatFloat(f, 'g', -1, 64)`, `AppendFloat(b, f, 'f', -1, 64)`, the JSON
//! float encoder), `ParseFloat(s, 64)`, `ParseInt`, `ParseUint`, `Atoi` and `NumError`.
//! `Itoa`, `AppendInt` and `AppendUint` are Rust's `{}` (the same digits), so they have no
//! function here.

mod atof;
mod atoi;
mod ftoa;
mod quote;

use std::fmt;

/// The error kind of a `NumError` (Go `ErrSyntax`, `ErrRange`, and the base and bit size errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumErrorKind {
    /// `invalid syntax`
    Syntax,
    /// `value out of range`
    Range,
    /// `invalid base N`
    Base(u32),
    /// `invalid bit size N`
    BitSize(u32),
}

/// Go `*strconv.NumError`. `func` is the Go function name (`ParseInt`, `ParseUint`,
/// `ParseFloat`, `Atoi`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumError {
    pub func: &'static str,
    pub num: String,
    pub err: NumErrorKind,
}

impl fmt::Display for NumError {
    /// Go: `"strconv." + Func + ": parsing " + Quote(Num) + ": " + Err.Error()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "strconv.{}: parsing {}: ",
            self.func,
            quote(self.num.as_bytes())
        )?;
        match self.err {
            NumErrorKind::Syntax => f.write_str("invalid syntax"),
            NumErrorKind::Range => f.write_str("value out of range"),
            NumErrorKind::Base(n) => write!(f, "invalid base {n}"),
            NumErrorKind::BitSize(n) => write!(f, "invalid bit size {n}"),
        }
    }
}

impl std::error::Error for NumError {}

fn num_error(func: &'static str, num: &str, err: NumErrorKind) -> NumError {
    NumError {
        func,
        num: num.to_owned(),
        err,
    }
}

/// Go `strconv.Quote` (and `fmt %q` on a string): a double-quoted Go string literal. Invalid
/// UTF-8 bytes become `\xNN`, non-printable runes `\uNNNN` / `\UNNNNNNNN`.
pub fn quote(s: &[u8]) -> String {
    let mut buf = Vec::with_capacity(3 * s.len() / 2 + 2);
    append_quote(&mut buf, s);
    String::from_utf8(buf).expect("quote output is valid UTF-8")
}

/// Go `strconv.AppendQuote`.
pub fn append_quote(dst: &mut Vec<u8>, s: &[u8]) {
    quote::append_quote(dst, s);
}

/// Go `strconv.IsPrint` (the same set as `unicode::is_print`).
pub fn is_print(r: char) -> bool {
    crate::unicode::is_print(r)
}

/// Go `strconv.FormatFloat(f, fmt, -1, 64)`: the shortest digits that round-trip. `fmt` is one
/// of `b'e'`, `b'E'`, `b'f'`, `b'g'`, `b'G'`; any other value panics.
pub fn format_float(f: f64, fmt: u8) -> String {
    let mut buf = Vec::new();
    append_float(&mut buf, f, fmt);
    String::from_utf8(buf).expect("float text is ASCII")
}

/// Go `strconv.AppendFloat(dst, f, fmt, -1, 64)`.
pub fn append_float(dst: &mut Vec<u8>, f: f64, fmt: u8) {
    assert!(
        matches!(fmt, b'e' | b'E' | b'f' | b'g' | b'G'),
        "strconv: illegal float format {:?}",
        fmt as char
    );
    ftoa::append_float(dst, f, fmt);
}

/// Go `strconv.ParseFloat(s, 64)`: decimal and hex forms, `inf`/`infinity`/`nan` in any case,
/// underscores where Go's `underscoreOK` allows them. Out of range is `NumErrorKind::Range`.
pub fn parse_float(s: &str) -> Result<f64, NumError> {
    atof::parse_float(s).map_err(|e| num_error("ParseFloat", s, e))
}

/// Go `strconv.ParseInt(s, base, bit_size)` (base 0 reads a `0x`, `0o`, `0b` or `0` prefix and
/// allows underscores; bit size 0 means 64).
pub fn parse_int(s: &str, base: u32, bit_size: u32) -> Result<i64, NumError> {
    atoi::parse_int(s.as_bytes(), base, bit_size).map_err(|e| num_error("ParseInt", s, e))
}

/// Go `strconv.ParseUint(s, base, bit_size)`.
pub fn parse_uint(s: &str, base: u32, bit_size: u32) -> Result<u64, NumError> {
    atoi::parse_uint(s.as_bytes(), base, bit_size).map_err(|e| num_error("ParseUint", s, e))
}

/// Go `strconv.Atoi(s)` (Go `int` is 64-bit on graphify's targets).
pub fn atoi(s: &str) -> Result<i64, NumError> {
    atoi::parse_int(s.as_bytes(), 10, 0).map_err(|e| num_error("Atoi", s, e))
}
