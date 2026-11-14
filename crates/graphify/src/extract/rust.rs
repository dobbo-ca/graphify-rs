//! Port of `rust.go`. Owner: the gostd stream; filled by graphify-rs-ihc.32.

use super::extract::Result;

/// Go `extractRust`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_rust(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.32")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.32.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/rust.rs"]
mod rust_tests;
