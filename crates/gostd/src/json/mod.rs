//! Go 1.27.4 `encoding/json`, encode half. Go 1.27.4 builds it with GOEXPERIMENT=jsonv2 on, so
//! the active code is the `v2_*.go` files over `encoding/json/jsontext` and
//! `internal/jsonwire` (`v2_encode.go`, `v2_indent.go`, `v2_scanner.go`, `v2_stream.go`,
//! `tags.go`). The decode half is API stubs until graphify-rs-aa6.10.
mod decode;
mod encode;
mod indent;
mod jsontext;
mod scanner;
mod value;

use std::fmt;

pub use decode::{DecodeState, Unmarshal, touch_field, unmarshal, unmarshal_into};
pub use encode::{
    EncodeState, Encoder, Marshal, append_float, append_string, marshal, marshal_indent,
    write_field,
};
pub use indent::{compact, html_escape, indent};
pub use scanner::valid;
pub use value::{RawMessage, Value};

/// The encode-side errors of Go `encoding/json`; `Display` is Go's `Error()` text.
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
    /// Go `UnmarshalTypeError` (`v2_decode.go:123`), as `transformUnmarshalError` builds it
    /// (`v2_inject.go:69-159`). `value` is Go's `Value` (`"bool"`, `"number -5"`), `go_type` is
    /// `Type.String()`, `offset` is bytes read. `struct_name` is Go's `Struct`: the `Name()` of the
    /// root type (empty for an unnamed type), set only when `field` is non-empty
    /// (`v2_inject.go:137-143`). `field` is Go's `Field`: the JSON Pointer of the value without its
    /// leading `/`, every `/` turned into `.`, so object keys, map keys and array indices all
    /// appear (`Ts.2.Y`; `v2_inject.go:144-146`). `err` is the text of Go's `Err`, `None` for nil.
    /// Go `InvalidUnmarshalError` (`v2_decode.go:178`) cannot occur in Rust and has no variant.
    /// Boxed: the payload would make every `Result<_, Error>` large (`clippy::result_large_err`).
    UnmarshalType(Box<UnmarshalTypeError>),
    /// A write error from the `io.Writer` of an [`Encoder`]; holds the error's text.
    Io(String),
}

impl fmt::Display for Error {
    /// Go `SyntaxError.Error` (`v2_scanner.go:52`), `UnsupportedValueError.Error`
    /// (`v2_encode.go:225`), `UnsupportedTypeError.Error` (`v2_encode.go:214`),
    /// `MarshalerError.Error` (`v2_encode.go:251`), `UnmarshalTypeError.Error` (`v2_decode.go:132`).
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
            Error::UnmarshalType(e) => {
                let UnmarshalTypeError {
                    value,
                    go_type,
                    struct_name,
                    field,
                    err,
                    ..
                } = &**e;
                if !struct_name.is_empty() || !field.is_empty() {
                    // A path ending in an array index drops "Go struct field ".
                    let last = &field[field.rfind('.').map_or(0, |i| i + 1)..];
                    let into = if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
                        ""
                    } else {
                        "Go struct field "
                    };
                    write!(
                        f,
                        "json: cannot unmarshal {value} into {into}{struct_name}.{field} of type {go_type}"
                    )?;
                } else {
                    write!(
                        f,
                        "json: cannot unmarshal {value} into Go value of type {go_type}"
                    )?;
                }
                match err {
                    Some(err) => write!(f, ": {err}"),
                    None => Ok(()),
                }
            }
            Error::Io(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

/// The fields of Go `UnmarshalTypeError`; see [`Error::UnmarshalType`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmarshalTypeError {
    pub value: String,
    pub go_type: String,
    pub offset: i64,
    pub struct_name: String,
    pub field: String,
    pub err: Option<String>,
}

/// Implements [`Marshal`] for a struct as Go's `structEncoder` (`encode.go:716`) does for a
/// struct with `json` tags: fields in declaration order, key from the tag, `omitempty`
/// (`isEmptyValue`, `encode.go:351`, here [`Marshal::is_empty_value`]) skips empty fields, and
/// the key `"-"` skips the field.
///
/// Syntax: `json_fields!(Type, "go.Type", { field => "key" [omitempty], ... })`. The second
/// argument is the Go type name used in error text.
///
/// With the leading keyword `decode`, `json_fields!(decode Type, "go.Type", { ... })` also
/// implements [`Unmarshal`] (Go's `d.object` into a struct, `decode.go:588`; the key `"-"` is not
/// decoded) and requires every field type to implement [`Unmarshal`]. The keyword is opt-in
/// because many structs have fields that are `Marshal` only (a hand-written `MarshalJSON`
/// stand-in), and an unconditional `Unmarshal` impl would stop them compiling.
///
/// ```ignore
/// struct Node { id: String, community: Option<i64>, path: String }
/// gostd::json_fields!(decode Node, "model.Node", {
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
    (decode $ty:ty, $go_type:literal, { $($field:ident => $key:literal $($oe:ident)?),* $(,)? }) => {
        $crate::json_fields!($ty, $go_type, { $($field => $key $($oe)?),* });
        impl $crate::json::Unmarshal for $ty {
            const GO_TYPE: &'static str = $go_type;
            /// Go `d.object` into a struct (`decode.go:588`). Every field is touched, key `"-"`
            /// included, so the caller's crate gets no dead_code warning.
            fn unmarshal_json(
                &mut self,
                d: &mut $crate::json::DecodeState<'_>,
            ) -> ::core::result::Result<(), $crate::json::Error> {
                let _ = &d;
                $( $crate::json::touch_field(&mut self.$field); )*
                todo!("graphify-rs-aa6.10")
            }
        }
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
