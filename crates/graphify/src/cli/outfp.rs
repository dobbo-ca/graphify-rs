//! Port of `outfp.go`: the fingerprint of a build's outputs, so a no-change `update` skips the
//! rewrite. Filled by graphify-rs-ihc.20.

use std::path::Path;

use crate::detect::WalkReport;

/// Go `binID`: `<size>/<mtime ns>` of `std::env::current_exe()`, or `noexe`; tells builds apart
/// when the version string is the same.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn bin_id() -> &'static str {
    todo!("graphify-rs-ihc.20")
}

/// Go `outFP`: hashes every input of graph.json and GRAPH_REPORT.md other than file contents.
/// It is [`out_fp_with`] with [`bin_id`].
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn out_fp(root: &str, walk: &WalkReport, commit: &str, flags: &str) -> String {
    todo!("graphify-rs-ihc.20")
}

/// [`out_fp`] with the binary identity given (Go's test swaps the `binID` var).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn out_fp_with(
    bin_id: &str,
    root: &str,
    walk: &WalkReport,
    commit: &str,
    flags: &str,
) -> String {
    todo!("graphify-rs-ihc.20")
}

/// Go `sigOf`: `<size>/<mtime ns>` of `path`, or `""` when it cannot be stat'ed.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn sig_of(path: &Path) -> String {
    todo!("graphify-rs-ihc.20")
}

#[cfg(test)]
#[path = "tests/outfp.rs"]
mod outfp_tests;
