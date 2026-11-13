//! Port of `lua.go`. Owner: the gostd stream; filled by graphify-rs-ihc.39.

use super::extract::Result;

/// Go `extractLua`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_lua(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.39")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.39.
pub(super) fn register_kinds() {}
