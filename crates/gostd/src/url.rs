//! Go 1.27.4 `net/url`: `Parse`, `PathUnescape`, `URL.Hostname` and `URL.Port`. Go strings can hold
//! invalid UTF-8 (`%ff` in a host or path), so decoded fields are bytes. Call sites: graphify-go
//! `resolve.go:1367` (`PathUnescape`) and `security.go:66` (`Parse`, `Scheme`, `Hostname`).
#![allow(unused_variables)] // stubs; removed in PR B

use crate::strconv::quote;
use std::fmt;

/// Go `url.URL`, minus `OmitHost` (only `String` reads it) and the methods graphify-go never calls.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Url {
    pub scheme: String,
    pub opaque: Vec<u8>,
    pub user: Option<Userinfo>,
    pub host: Vec<u8>,
    pub path: Vec<u8>,
    pub raw_path: String,
    pub force_query: bool,
    pub raw_query: String,
    pub fragment: Vec<u8>,
    pub raw_fragment: String,
}

/// Go `url.Userinfo`; `password: None` is Go's `passwordSet == false`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Userinfo {
    pub username: Vec<u8>,
    pub password: Option<Vec<u8>>,
}

/// Go `url.Error`: `parse "<url>": <err>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub op: &'static str,
    pub url: String,
    pub err: ErrorKind,
}

impl fmt::Display for Error {
    /// Go `fmt.Sprintf("%s %q: %s", e.Op, e.URL, e.Err)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.op,
            quote(self.url.as_bytes()),
            self.err
        )
    }
}

/// Go `url.EscapeError`: the offending `%` escape, up to 3 bytes. Bytes, because Go's string can
/// hold a cut multi-byte character (`"%aé"` gives `EscapeError("%a\xc3")`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapeError(pub Vec<u8>);

impl fmt::Display for EscapeError {
    /// `invalid URL escape "%zz"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URL escape {}", quote(&self.0))
    }
}

/// Go `url.InvalidHostError`: the offending byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidHostError(pub String);

impl fmt::Display for InvalidHostError {
    /// `invalid character " " in host name`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid character {} in host name",
            quote(self.0.as_bytes())
        )
    }
}

/// Every error value `Parse` can return (`getScheme`, `parse`, `parseAuthority`, `parseHost`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// `missing protocol scheme`
    MissingScheme,
    /// `net/url: invalid control character in URL`
    InvalidControlCharacter,
    /// `first path segment in URL cannot contain colon`
    FirstSegmentColon,
    /// `net/url: invalid userinfo`
    InvalidUserinfo,
    /// `invalid IP-literal`
    InvalidIpLiteral,
    /// `missing ']' in host`
    MissingBracket,
    /// `invalid port ":x" after host`; holds the port with its colon.
    InvalidPort(String),
    /// `invalid host: <netip.ParseAddr error text>`
    InvalidHost(String),
    Escape(EscapeError),
    InvalidHostChar(InvalidHostError),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::MissingScheme => f.write_str("missing protocol scheme"),
            ErrorKind::InvalidControlCharacter => {
                f.write_str("net/url: invalid control character in URL")
            }
            ErrorKind::FirstSegmentColon => {
                f.write_str("first path segment in URL cannot contain colon")
            }
            ErrorKind::InvalidUserinfo => f.write_str("net/url: invalid userinfo"),
            ErrorKind::InvalidIpLiteral => f.write_str("invalid IP-literal"),
            ErrorKind::MissingBracket => f.write_str("missing ']' in host"),
            ErrorKind::InvalidPort(p) => {
                write!(f, "invalid port {} after host", quote(p.as_bytes()))
            }
            ErrorKind::InvalidHost(e) => write!(f, "invalid host: {e}"),
            ErrorKind::Escape(e) => fmt::Display::fmt(e, f),
            ErrorKind::InvalidHostChar(e) => fmt::Display::fmt(e, f),
        }
    }
}

/// Go `url.PathUnescape`: `%XX` to a byte, `+` kept; the result can be invalid UTF-8.
pub fn path_unescape(s: &str) -> Result<Vec<u8>, EscapeError> {
    todo!("graphify-rs-aa6.14")
}

/// Go `url.Parse`.
pub fn parse(raw: &str) -> Result<Url, Error> {
    todo!("graphify-rs-aa6.14")
}

impl Url {
    /// Go `URL.Hostname`: the host without port, and without `[` `]` around an IPv6 literal.
    pub fn hostname(&self) -> &[u8] {
        todo!("graphify-rs-aa6.14")
    }

    /// Go `URL.Port`: the numeric port without the colon, or empty.
    pub fn port(&self) -> &[u8] {
        todo!("graphify-rs-aa6.14")
    }
}
