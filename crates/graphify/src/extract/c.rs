//! Port of `c.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.29.

use super::extract::Result;

/// Go `extractC`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_c(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.29")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.29.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/c.rs"]
mod c_tests;
