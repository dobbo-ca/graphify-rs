//! Port of `component.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.26.

use super::extract::Result;

/// Go `extractComponent`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_component(rel: &str, src: &[u8], is_astro: bool) -> Option<Result> {
    todo!("graphify-rs-ihc.26")
}

#[cfg(test)]
#[path = "tests/component.rs"]
mod component_tests;
