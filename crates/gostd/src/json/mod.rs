//! Go 1.27.4 `encoding/json` v1, encode half (`encode.go`, `indent.go`, `scanner.go`,
//! `stream.go`, `tags.go`). The decode half arrives with graphify-rs-aa6.10.
// graphify-rs-aa6.9: stubs until the code PR
#![allow(clippy::ptr_arg)]
// graphify-rs-aa6.9: stubs until the code PR
#![allow(unused_variables)]
// graphify-rs-aa6.9: stubs until the code PR
#![allow(dead_code)]

mod encode;
mod indent;
mod scanner;
mod value;

use std::fmt;

pub use encode::{
    EncodeState, Encoder, Marshal, append_float, append_string, marshal, marshal_indent,
};
pub use indent::{compact, html_escape, indent};
pub use scanner::valid;
pub use value::{RawMessage, Value};

/// The encode-side errors of Go `encoding/json`; `Display` is Go's `Error()` text.
/// Decode variants (`UnmarshalTypeError` etc.) arrive with graphify-rs-aa6.10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Go `SyntaxError` (`scanner.go:49`); `Error()` is `msg`. `offset` is bytes read before the error.
    Syntax { msg: String, offset: i64 },
    /// Go `UnsupportedValueError` (`encode.go:251`); holds Go's `Str`, e.g. `"NaN"`.
    UnsupportedValue(String),
    /// Go `UnsupportedTypeError` (`encode.go:241`); holds the Go type string.
    UnsupportedType(String),
    /// Go `MarshalerError` (`encode.go:276`). `source_func` is `"MarshalJSON"` or
    /// `"MarshalText"`; empty means `"MarshalJSON"`, as in Go.
    Marshaler {
        go_type: String,
        source_func: &'static str,
        err: String,
    },
    /// A write error from the `io.Writer` of an [`Encoder`]; holds the error's text.
    Io(String),
}

impl fmt::Display for Error {
    /// Go `SyntaxError.Error` (`scanner.go:54`), `UnsupportedValueError.Error`
    /// (`encode.go:256`), `UnsupportedTypeError.Error` (`encode.go:245`),
    /// `MarshalerError.Error` (`encode.go:282`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax { msg, .. } => f.write_str(msg),
            Error::UnsupportedValue(s) => write!(f, "json: unsupported value: {s}"),
            Error::UnsupportedType(t) => write!(f, "json: unsupported type: {t}"),
            Error::Marshaler {
                go_type,
                source_func,
                err,
            } => {
                let src = if source_func.is_empty() {
                    "MarshalJSON"
                } else {
                    source_func
                };
                write!(f, "json: error calling {src} for type {go_type}: {err}")
            }
            Error::Io(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

/// Implements [`Marshal`] for a struct as Go's `structEncoder` (`encode.go:716`) does for a
/// struct with `json` tags: fields in declaration order, key from the tag, `omitempty`
/// (`isEmptyValue`, `encode.go:351`, here [`Marshal::is_empty_value`]) skips empty fields, and
/// the key `"-"` skips the field. Task 10 extends this macro with `Unmarshal`.
///
/// Syntax: `json_fields!(Type, "go.Type", { field => "key" [omitempty], ... })`. The second
/// argument is the Go type name used in error text.
///
/// ```ignore
/// struct Node { id: String, community: Option<i64>, path: String }
/// gostd::json_fields!(Node, "model.Node", {
///     id => "id",
///     community => "community" omitempty,
///     path => "-",
/// });
/// ```
#[macro_export]
macro_rules! json_fields {
    ($ty:ty, $go_type:literal, { $($field:ident => $key:literal $(omitempty)?),* $(,)? }) => {
        impl $crate::json::Marshal for $ty {
            fn marshal_json(
                &self,
                e: &mut $crate::json::EncodeState,
            ) -> ::core::result::Result<(), $crate::json::Error> {
                // graphify-rs-aa6.9 stub: touch every field and `e` so the caller's crate gets no
                // dead_code / unused_variables warning before the real body lands.
                let _ = (&e, $(&self.$field),*);
                todo!("graphify-rs-aa6.9")
            }
        }
    };
}
