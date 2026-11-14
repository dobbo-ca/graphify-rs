//! Port of `resolve.go`: stitches the per-file results into one extraction. Filled by
//! graphify-rs-ihc.18 (plan Task 17); the signatures are shared API.

use crate::model::{Edge, Extraction, Node};

use super::extract::Result;

/// Go `extract.Resolve`: the call, import and reference edges that need the whole corpus.
/// `files` are the slash-relative paths of the corpus; `workers` is Go's GOMAXPROCS (plan R5).
pub fn resolve(results: &[Result], files: &[String], workers: usize) -> Extraction {
    todo!("graphify-rs-ihc.18")
}

/// Go `extract.ResolveParts`: [`resolve`] without the join, the parts in the order `resolve`
/// joins them (`resolve.go:922-1044`).
pub fn resolve_parts(
    results: &[Result],
    files: &[String],
    workers: usize,
) -> (Vec<Vec<Node>>, Vec<Vec<Edge>>) {
    todo!("graphify-rs-ihc.18")
}

/// Go `extract.Join`: the parts copied into one extraction.
pub fn join(nodes: Vec<Vec<Node>>, edges: Vec<Vec<Edge>>) -> Extraction {
    todo!("graphify-rs-ihc.18")
}

#[cfg(test)]
#[path = "tests/csharp_dispatch.rs"]
mod csharp_dispatch_tests;
