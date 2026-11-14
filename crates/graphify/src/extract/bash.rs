//! Port of `bash.go`. Owner: the gostd stream; filled by graphify-rs-ihc.35.

use super::extract::Result;

/// Go `extractBash`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_bash(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.35")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.35.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/bash.rs"]
mod bash_tests;

#[cfg(test)]
#[path = "tests/bash_scriptinvocation.rs"]
mod bash_scriptinvocation_tests;
