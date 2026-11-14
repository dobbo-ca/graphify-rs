//! Port of `python.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.25.

use super::extract::Result;

/// Go `extractPython`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_python(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.25")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.25.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/pycalls.rs"]
mod pycalls_tests;

#[cfg(test)]
#[path = "tests/python_nested.rs"]
mod python_nested_tests;

#[cfg(test)]
#[path = "tests/python.rs"]
mod python_tests;
