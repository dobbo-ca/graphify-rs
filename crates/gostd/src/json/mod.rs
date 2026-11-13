//! Go 1.27.4 `encoding/json`, encode half. Go 1.27.4 builds it with GOEXPERIMENT=jsonv2 on, so
//! the active code is the `v2_*.go` files over `encoding/json/jsontext` and
//! `internal/jsonwire` (`v2_encode.go`, `v2_indent.go`, `v2_scanner.go`, `v2_stream.go`,
//! `tags.go`). The decode half arrives with graphify-rs-aa6.10.
mod encode;
mod indent;
mod jsontext;
mod scanner;
mod value;

use std::fmt;

pub use encode::{
    EncodeState, Encoder, Marshal, append_float, append_string, marshal, marshal_indent,
    write_field,
};
pub use indent::{compact, html_escape, indent};
pub use scanner::valid;
pub use value::{RawMessage, Value};

/// The encode-side errors of Go `encoding/json`; `Display` is Go's `Error()` text.
/// Decode variants (`UnmarshalTypeError` etc.) arrive with graphify-rs-aa6.10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Go `SyntaxError` (`v2_scanner.go:47`); `Error()` is `msg`. `offset` is Go's legacy
    /// `Offset` (`jsontext/errors.go:102-116`): past the invalid text.
    Syntax { msg: String, offset: i64 },
    /// Go `UnsupportedValueError` (`v2_encode.go:220`); holds Go's `Str`, e.g. `"NaN"`.
    UnsupportedValue(String),
    /// Go `UnsupportedTypeError` (`v2_encode.go:210`); holds the Go type string.
    UnsupportedType(String),
    /// Go `MarshalerError` (`v2_encode.go:245`). `source_func` is `"MarshalJSON"` or
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
    /// Go `SyntaxError.Error` (`v2_scanner.go:52`), `UnsupportedValueError.Error`
    /// (`v2_encode.go:225`), `UnsupportedTypeError.Error` (`v2_encode.go:214`),
    /// `MarshalerError.Error` (`v2_encode.go:251`).
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
    (@skip $v:expr, omitempty) => {
        $crate::json::Marshal::is_empty_value(&$v)
    };
    (@skip $v:expr,) => {
        false
    };
    ($ty:ty, $go_type:literal, { $($field:ident => $key:literal $($oe:ident)?),* $(,)? }) => {
        impl $crate::json::Marshal for $ty {
            /// Go `structEncoder.encode` (`encode.go:716`). Every field is read, key `"-"`
            /// included, so the caller's crate gets no dead_code warning.
            fn marshal_json(
                &self,
                e: &mut $crate::json::EncodeState,
            ) -> ::core::result::Result<(), $crate::json::Error> {
                #[allow(unused_mut)]
                let mut first = true;
                $(
                    $crate::json::write_field(
                        e,
                        &mut first,
                        $key,
                        &self.$field,
                        $crate::json_fields!(@skip self.$field, $($oe)?),
                    )?;
                )*
                e.write_raw(if first { &b"{}"[..] } else { &b"}"[..] });
                ::core::result::Result::Ok(())
            }
        }
    };
}
