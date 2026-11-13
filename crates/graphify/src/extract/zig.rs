//! Port of `zig.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.40.

use super::extract::Result;

/// Go `extractZig`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_zig(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.40")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.40.
pub(super) fn register_kinds() {}
