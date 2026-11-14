//! Port of `lua.go`. Owner: the gostd stream; filled by graphify-rs-ihc.39.

use super::extract::Result;

/// Go `extractLua`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_lua(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.39")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.39.
pub(super) fn register_kinds() {}

// Shared tree helpers, used by c/golang/java/js/cpp; filled by the runtime stream.

/// Go `childByKind` (`lua.go:223`): the first direct child of `n` with the given kind.
pub(crate) fn child_by_kind<'t>(n: gositter::Node<'t>, kind: &str) -> Option<gositter::Node<'t>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.kind() == kind)
}

/// Go `firstNamed` (`lua.go:233`): the first named child of `n`.
pub(crate) fn first_named(n: gositter::Node<'_>) -> Option<gositter::Node<'_>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.is_named())
}

#[cfg(test)]
#[path = "tests/lua.rs"]
mod lua_tests;

#[cfg(test)]
#[path = "tests/lua_helpers.rs"]
mod lua_helpers_tests;
