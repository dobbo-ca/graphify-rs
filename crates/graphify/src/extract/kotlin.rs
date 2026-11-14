//! Port of `kotlin.go`. Owner: the gostd stream; filled by graphify-rs-ihc.37.

use super::extract::Result;

/// Go `extractKotlin`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_kotlin(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.37")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.37.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/kotlin.rs"]
mod kotlin_tests;
