//! Port of `ruby.go`. Owner: the gostd stream; filled by graphify-rs-ihc.33.

use super::extract::Result;

/// Go `extractRuby`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_ruby(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.33")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.33.
pub(super) fn register_kinds() {}
