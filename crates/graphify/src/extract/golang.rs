//! Port of `golang.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.24.

use gositter::Node;

use super::builder::node_text;
use super::extract::Result;

/// Go `extractGo`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_go(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.24")
}

// Go fieldText (golang.go:388)
pub(crate) fn field_text(n: Node<'_>, field: &str, src: &[u8]) -> String {
    match n.child_by_field_name(field) {
        Some(c) => node_text(c, src).into_owned(),
        None => String::new(),
    }
}

// Go unquote (golang.go:394)
pub(crate) fn unquote(s: &str) -> String {
    s.trim_matches(['`', '"', '\'']).to_string()
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.24.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/gocalls.rs"]
mod gocalls_tests;
