//! Port of `terraform.go`. Owner: the gostd stream; filled by graphify-rs-ihc.31.

use super::extract::Result;

/// Go `extractTerraform`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_terraform(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.31")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.31.
pub(super) fn register_kinds() {}
