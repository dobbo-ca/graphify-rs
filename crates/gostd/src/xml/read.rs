//! Reader side: `Unmarshal` into the `xmlElem` shape plus a test-only token API.

use std::fmt;

/// Go `xml.Name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub space: String,
    pub local: String,
}

/// Go `xmlElem{XMLName; Chardata ",chardata"; Children ",any"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: Name,
    pub chardata: String,
    pub children: Vec<Element>,
}

/// Go `*xml.SyntaxError`; Display is `XML syntax error on line N: msg`.
///
/// Errors that are not `*SyntaxError` in Go (`errUnmarshalDepth`, the
/// CharsetReader-nil error at xml.go:646) are carried here with `line == 0`
/// and `msg` holding Go's `err.Error()` text. Display must then print just
/// `msg`, without the "XML syntax error on line N:" prefix (implemented in PR B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub msg: String,
    pub line: usize,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-aa6.12")
    }
}

impl std::error::Error for SyntaxError {}

/// Go `xml.Unmarshal(data, &xmlElem)`: first root element only.
pub fn unmarshal_element(data: &[u8]) -> Result<Element, SyntaxError> {
    todo!("graphify-rs-aa6.12")
}

// ---- Test-only token API (Go `Decoder`, Strict = true, no CharsetReader) ----

/// Go `xml.Attr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: Name,
    pub value: String,
}

/// Go `xml.Token` (`StartElement`, `EndElement`, `CharData`, `Comment`,
/// `ProcInst`, `Directive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    StartElement { name: Name, attr: Vec<Attr> },
    EndElement { name: Name },
    CharData(Vec<u8>),
    Comment(Vec<u8>),
    ProcInst { target: String, inst: Vec<u8> },
    Directive(Vec<u8>),
}

/// All tokens from `Decoder.Token()` until EOF (namespace-translated, start
/// and end tags matched, `Entity` empty). Err on the first syntax error.
#[doc(hidden)]
pub fn tokens(data: &[u8]) -> Result<Vec<Token>, SyntaxError> {
    todo!("graphify-rs-aa6.12")
}

/// As `tokens`, with `Decoder.Entity` set to the given (name, value) pairs.
#[doc(hidden)]
pub fn tokens_with_entity(data: &[u8], entity: &[(&str, &str)]) -> Result<Vec<Token>, SyntaxError> {
    todo!("graphify-rs-aa6.12")
}

/// Tokens read before the first error, and that error (None on clean EOF).
/// `raw` selects `RawToken` (no namespace translation or tag matching) over `Token`.
#[doc(hidden)]
pub fn tokens_partial(data: &[u8], raw: bool) -> (Vec<Token>, Option<SyntaxError>) {
    todo!("graphify-rs-aa6.12")
}

/// All tokens from `Decoder.RawToken()` until EOF (no namespace translation,
/// no start/end matching).
#[doc(hidden)]
pub fn raw_tokens(data: &[u8]) -> Result<Vec<Token>, SyntaxError> {
    todo!("graphify-rs-aa6.12")
}

/// `RawToken` stream (with `Decoder.Entity` set to `entity`) with `(token, start, end)` where start/end are
/// `Decoder.InputOffset()` before and after each call (Go `testRawToken`).
#[doc(hidden)]
pub fn raw_token_spans(
    data: &[u8],
    entity: &[(&str, &str)],
) -> Result<Vec<(Token, i64, i64)>, SyntaxError> {
    todo!("graphify-rs-aa6.12")
}

/// `Decoder.InputPos()` (line, column; both 1-based) after each `Token()` call
/// (Go `TestInputLinePos`).
#[doc(hidden)]
pub fn input_pos_after_tokens(data: &[u8]) -> Result<Vec<(usize, usize)>, SyntaxError> {
    todo!("graphify-rs-aa6.12")
}

/// Go `procInst(param, s)`: the value of `param` in a `<?xml ...?>` instruction.
#[doc(hidden)]
pub fn proc_inst(param: &str, s: &str) -> String {
    todo!("graphify-rs-aa6.12")
}

/// Go `isInCharacterRange(r)`; takes a rune value (may exceed char range).
#[doc(hidden)]
pub fn is_in_character_range(r: u32) -> bool {
    todo!("graphify-rs-aa6.12")
}
