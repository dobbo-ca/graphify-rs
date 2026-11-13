//! Go `indent.go`.

use super::Error;

/// Go `appendHTMLEscape` (`indent.go:21`); Go `HTMLEscape` (`indent.go:16`) is not reached,
/// this exists for the Go tests that call it.
pub fn html_escape(dst: &mut Vec<u8>, src: &[u8]) {
    todo!("graphify-rs-aa6.9")
}

/// Go `appendCompact` (`indent.go:51`); `escape_html` is its `escape` argument.
pub fn compact(dst: &mut Vec<u8>, src: &[u8], escape_html: bool) -> Result<(), Error> {
    todo!("graphify-rs-aa6.9")
}

/// Go `appendIndent` (`indent.go:128`).
pub fn indent(dst: &mut Vec<u8>, src: &[u8], prefix: &str, indent: &str) -> Result<(), Error> {
    todo!("graphify-rs-aa6.9")
}
