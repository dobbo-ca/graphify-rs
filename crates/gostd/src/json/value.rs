//! `any` and `RawMessage`.

use std::collections::{BTreeMap, btree_map};
use std::fmt;

use super::encode::Frame;

/// Go `any` holding decoded JSON (`map[string]any` keys sort bytewise, as `BTreeMap` does).
///
/// A decoded `Value` can be 9,999 levels deep (Go's goroutine stacks grow), so `Drop`, `Clone`,
/// `PartialEq`, `Debug` and `Marshal` walk it with an explicit stack instead of one call per
/// level; they behave as the derived impls would.
#[derive(Default)]
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

/// A container being cloned: the source elements left and the copy so far (for an object, with
/// the key of the element being cloned).
enum CloneFrame<'a> {
    Array(std::slice::Iter<'a, Value>, Vec<Value>),
    Object(
        btree_map::Iter<'a, String, Value>,
        BTreeMap<String, Value>,
        String,
    ),
}

impl Clone for Value {
    fn clone(&self) -> Self {
        let mut stack = Vec::new();
        let mut next = self;
        loop {
            // A scalar is copied at once; a container is copied when its last element is.
            let mut done = match next {
                Value::Null => Some(Value::Null),
                Value::Bool(b) => Some(Value::Bool(*b)),
                Value::Number(f) => Some(Value::Number(*f)),
                Value::String(s) => Some(Value::String(s.clone())),
                Value::Array(a) => {
                    stack.push(CloneFrame::Array(a.iter(), Vec::with_capacity(a.len())));
                    None
                }
                Value::Object(m) => {
                    stack.push(CloneFrame::Object(m.iter(), BTreeMap::new(), String::new()));
                    None
                }
            };
            next = loop {
                let child = match stack.last_mut() {
                    // The root is done (a container is pushed before its value is).
                    None => return done.unwrap_or_default(),
                    Some(CloneFrame::Array(it, out)) => {
                        out.extend(done.take());
                        it.next()
                    }
                    Some(CloneFrame::Object(it, out, key)) => {
                        if let Some(v) = done.take() {
                            out.insert(std::mem::take(key), v);
                        }
                        it.next().map(|(k, v)| {
                            key.clone_from(k);
                            v
                        })
                    }
                };
                if let Some(c) = child {
                    break c;
                }
                done = stack.pop().map(|f| match f {
                    CloneFrame::Array(_, out) => Value::Array(out),
                    CloneFrame::Object(_, out, _) => Value::Object(out),
                });
            };
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let mut stack = vec![(self, other)];
        while let Some(pair) = stack.pop() {
            let same = match pair {
                (Value::Null, Value::Null) => true,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (Value::Number(a), Value::Number(b)) => a == b,
                (Value::String(a), Value::String(b)) => a == b,
                (Value::Array(a), Value::Array(b)) => {
                    stack.extend(a.iter().zip(b));
                    a.len() == b.len()
                }
                (Value::Object(a), Value::Object(b)) => {
                    stack.extend(a.values().zip(b.values()));
                    a.len() == b.len() && a.keys().eq(b.keys())
                }
                _ => false,
            };
            if !same {
                return false;
            }
        }
        true
    }
}

/// The derived format, `Array([Number(1.0), Object({"k": Null})])`; `{:#?}` prints it the same.
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut stack = Vec::new();
        let mut next = Some(self);
        loop {
            match next.take() {
                Some(Value::Null) => f.write_str("Null")?,
                Some(Value::Bool(b)) => write!(f, "Bool({b:?})")?,
                Some(Value::Number(n)) => write!(f, "Number({n:?})")?,
                Some(Value::String(s)) => write!(f, "String({s:?})")?,
                Some(Value::Array(a)) => {
                    f.write_str("Array([")?;
                    stack.push(Frame::Array(a.iter(), true));
                }
                Some(Value::Object(m)) => {
                    f.write_str("Object({")?;
                    stack.push(Frame::Object(m.iter(), true));
                }
                None => {}
            }
            match stack.last_mut() {
                None => return Ok(()),
                Some(Frame::Array(it, first)) => match it.next() {
                    Some(v) => {
                        if !std::mem::take(first) {
                            f.write_str(", ")?;
                        }
                        next = Some(v);
                    }
                    None => {
                        f.write_str("])")?;
                        stack.pop();
                    }
                },
                Some(Frame::Object(it, first)) => match it.next() {
                    Some((k, v)) => {
                        if !std::mem::take(first) {
                            f.write_str(", ")?;
                        }
                        write!(f, "{k:?}: ")?;
                        next = Some(v);
                    }
                    None => {
                        f.write_str("})")?;
                        stack.pop();
                    }
                },
            }
        }
    }
}

/// Go `RawMessage` (`v2_stream.go:190`, `jsontext.Value`): pre-encoded JSON, validated and
/// compacted on marshal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawMessage(pub Vec<u8>);
