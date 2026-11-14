//! Port of `php.go`. Owner: the gostd stream; filled by graphify-rs-ihc.34.

use super::extract::Result;

/// Go `extractPHP`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_php(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.34")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.34.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/php.rs"]
mod php_tests;
