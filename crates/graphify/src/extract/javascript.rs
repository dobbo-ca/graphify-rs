//! Port of `javascript.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.26.

use super::extract::Result;

/// Go `extractJSFile`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_js_file(
    rel: &str,
    src: &[u8],
    lang: &'static gositter::Language,
) -> Option<Result> {
    todo!("graphify-rs-ihc.26")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.26.
pub(super) fn register_kinds() {}
