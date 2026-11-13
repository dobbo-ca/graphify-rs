//! Port of `java.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.27.

use super::extract::Result;

/// Go `extractJava`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_java(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.27")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.27.
pub(super) fn register_kinds() {}
