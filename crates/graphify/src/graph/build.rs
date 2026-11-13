//! Port of `build.go`: deterministic edge order, id-normalisation of edge endpoints, dangling
//! and cross-language phantom edges dropped, provenance kept on an id collision. Filled by
//! graphify-rs-ihc.8 (plan Task 7).

use crate::model::{Edge, Extraction, Graph, Node};

/// Go `graph.Build`.
pub fn build(ext: Extraction) -> Graph {
    todo!("graphify-rs-ihc.8")
}

/// Go `graph.BuildParts`: [`build`] for an extraction left in parts (`extract::resolve_parts`);
/// `workers` is Go's GOMAXPROCS (plan R5).
pub fn build_parts(nodes: Vec<Vec<Node>>, parts: Vec<Vec<Edge>>, workers: usize) -> Graph {
    todo!("graphify-rs-ihc.8")
}

/// Go `edgeOrder`: the indexes of `edges` in the order a stable sort by (source, target,
/// relation text) leaves them.
pub(super) fn edge_order(edges: &[Edge], workers: usize) -> Vec<i32> {
    todo!("graphify-rs-ihc.8")
}
