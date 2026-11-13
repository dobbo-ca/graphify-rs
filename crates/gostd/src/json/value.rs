//! `any` and `RawMessage`.

use std::collections::BTreeMap;

/// Go `any` holding decoded JSON (`map[string]any` keys sort bytewise, as `BTreeMap` does).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

/// Drops nested arrays and objects with an explicit stack: the derived drop recurses once per
/// nesting level, and a decoded `Value` can be 9,999 levels deep (Go's GC frees it without
/// recursion). Each popped child is dropped with its own children already moved out.
impl Drop for Value {
    fn drop(&mut self) {
        let mut stack = match self {
            Value::Array(a) => std::mem::take(a),
            Value::Object(m) => std::mem::take(m).into_values().collect(),
            _ => return,
        };
        while let Some(mut v) = stack.pop() {
            match &mut v {
                Value::Array(a) => stack.append(a),
                Value::Object(m) => stack.extend(std::mem::take(m).into_values()),
                _ => {}
            }
        }
    }
}

/// Go `RawMessage` (`v2_stream.go:190`, `jsontext.Value`): pre-encoded JSON, validated and
/// compacted on marshal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawMessage(pub Vec<u8>);
