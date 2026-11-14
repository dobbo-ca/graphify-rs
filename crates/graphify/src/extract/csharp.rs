//! Port of `csharp.go`. Owner: the gostd stream; filled by graphify-rs-ihc.28.

use super::extract::Result;

/// Go `extractCSharp`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_csharp(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.28")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.28.
pub(super) fn register_kinds() {}

#[cfg(test)]
#[path = "tests/csharp.rs"]
mod csharp_tests;

#[cfg(test)]
#[path = "tests/csharp_calls.rs"]
mod csharp_calls_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_calls.rs"]
mod upstream_csharp_calls_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_types.rs"]
mod upstream_csharp_types_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_members.rs"]
mod upstream_csharp_members_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_extension.rs"]
mod upstream_csharp_extension_tests;
