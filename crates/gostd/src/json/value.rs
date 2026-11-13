//! `any` and `RawMessage`.

use std::collections::BTreeMap;

/// Go `any` holding decoded JSON (`map[string]any` keys sort bytewise, as `BTreeMap` does).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

/// Go `RawMessage` (`v2_stream.go:190`, `jsontext.Value`): pre-encoded JSON, validated and
/// compacted on marshal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawMessage(pub Vec<u8>);
