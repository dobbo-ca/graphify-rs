//! BurntSushi/toml v1.4.0, TOML 1.0 only (`BURNTSUSHI_TOML_110` is never set). Call site:
//! graphify-go `cargo.go:187` `toml.DecodeFile(path, &data)` into `map[string]any`. The caller
//! reads the file, so there is no `decode_file`.

mod lex;
mod parse;

use crate::strconv;
use std::collections::BTreeMap;
use std::fmt;

/// Go `map[string]any` (keys sort bytewise, as `BTreeMap` does).
pub type Table = BTreeMap<String, Value>;

/// Go `any` holding a decoded TOML value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Datetime(Datetime),
    /// Go `[]any`, including an inline array of inline tables.
    Array(Vec<Value>),
    Table(Table),
    /// Go `[]map[string]any`, from `[[x]]`. Distinct from `Array`: graphify asserts `.([]any)`.
    ArrayOfTables(Vec<Table>),
}

/// Drops nested arrays and tables with a work list, not one call per level: a value can be
/// nested as deep as its input is long (graphify-rs-aa6.24), and Go frees it with no stack at all.
impl Drop for Value {
    fn drop(&mut self) {
        let mut rest = Vec::new();
        take_children(self, &mut rest);
        while let Some(mut v) = rest.pop() {
            take_children(&mut v, &mut rest);
        }
    }
}

/// Moves the direct children of `v` to `rest`, leaving `v` with none.
fn take_children(v: &mut Value, rest: &mut Vec<Value>) {
    match v {
        Value::Array(a) => rest.append(a),
        Value::Table(t) => rest.extend(std::mem::take(t).into_values()),
        Value::ArrayOfTables(ts) => {
            for t in ts.drain(..) {
                rest.extend(t.into_values());
            }
        }
        _ => {}
    }
}

/// Go `time.Time` as BurntSushi builds it. Fields the kind does not carry are 0 (a `LocalDate`
/// has zero time fields, a `LocalTime` zero date fields); `offset_seconds` is `Some` only for
/// `OffsetDateTime` (`Some(0)` for `Z`).
#[derive(Debug, Clone, PartialEq)]
pub struct Datetime {
    pub kind: DatetimeKind,
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
    pub offset_seconds: Option<i32>,
}

/// The four `internal.Local*` locations of BurntSushi plus any other (an offset datetime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatetimeKind {
    OffsetDateTime,
    LocalDateTime,
    LocalDate,
    LocalTime,
}

/// Go `ParseError` (`error.go:61`), reduced to the fields `Error()` prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    /// Go `Position.Line`, starting at 1.
    pub line: usize,
    /// Go `LastKey`; empty when there is none.
    pub last_key: String,
}

impl fmt::Display for ParseError {
    /// Go `ParseError.Error()` (`error.go:73`): `toml: line N (last key "k"): msg`, the key part
    /// only when `last_key` is not empty, the key quoted as `%q`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.last_key.is_empty() {
            return write!(f, "toml: line {}: {}", self.line, self.message);
        }
        write!(
            f,
            "toml: line {} (last key {}): {}",
            self.line,
            strconv::quote(self.last_key.as_bytes()),
            self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Go `toml.Decode` into `map[string]any` (`decode.go`, `parse.go`). `data` need not be UTF-8:
/// invalid bytes are a `ParseError`.
pub fn decode(data: &[u8]) -> Result<Table, ParseError> {
    parse::parse(data)
}
