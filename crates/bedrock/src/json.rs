//! A minimal JSON encoder and decoder, private to this crate. It swaps for `gostd::json` when that
//! lands; other modules use only the items below.

use std::ops::Range;

#[cfg(test)]
mod tests;

/// Appends `s` as a JSON string with Go's encoding (anthropic-sdk-go internal/encoding/json with
/// HTML escaping on): `<` `>` `&` as `\u003c` `\u003e` `\u0026`, U+2028/U+2029 escaped, and each
/// byte of invalid UTF-8 as `\ufffd`.
pub(crate) fn write_string(out: &mut String, s: &[u8]) {
    todo!("graphify-rs-u2o.4")
}

/// A parsed JSON value and the byte range of its text in the source.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Value {
    pub kind: Kind,
    pub span: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Null,
    Bool(bool),
    /// The number's text, as written.
    Number(String),
    String(String),
    Array(Vec<Value>),
    /// Entries in source order; duplicate keys are kept.
    Object(Vec<(String, Value)>),
}

/// Parses one JSON value; only whitespace may follow it.
pub(crate) fn parse(src: &str) -> Result<Value, String> {
    todo!("graphify-rs-u2o.4")
}

impl Value {
    /// The exact source text of this value (Go `respjson.Field.Raw()`).
    pub(crate) fn raw<'s>(&self, src: &'s str) -> &'s str {
        &src[self.span.clone()]
    }

    /// The first entry named exactly `key` (gjson lookup, used for API responses).
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        todo!("graphify-rs-u2o.4")
    }

    /// Go `encoding/json` v1 struct-field rule: the last entry whose key equals `key` ignoring
    /// case wins (used for `credential_process` output).
    pub(crate) fn get_go(&self, key: &str) -> Option<&Value> {
        todo!("graphify-rs-u2o.4")
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match &self.kind {
            Kind::String(s) => Some(s),
            _ => None,
        }
    }
}
