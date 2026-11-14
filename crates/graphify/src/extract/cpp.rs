//! Port of `cpp.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.30.

use super::extract::Result;

/// Go `extractCpp`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_cpp(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.30")
}

/// Go `blankCppExportMacros` (`cpp.go:36`): the ALL-CAPS export macros between `class`/`struct`
/// and the type name become spaces, newlines kept.
pub(crate) fn blank_cpp_export_macros(src: &[u8]) -> Vec<u8> {
    todo!("graphify-rs-ihc.30")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.30.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/cpp.rs"]
mod cpp_tests;

#[cfg(test)]
#[path = "tests/preproc.rs"]
mod preproc_tests;
