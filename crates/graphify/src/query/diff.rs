//! Port of `diff.go`: the nodes and edges added or removed between two graph snapshots.

use super::query::Graph;

// graphify-rs-ihc.13: stubs until the code PR

/// Go `DiffNode` (json `id`, `label`).
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffNode {
    pub id: String,
    pub label: String,
}

/// Go `DiffEdge` (json `source`, `target`, `relation`).
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

/// Go `DiffResult` (json `new_nodes`, `removed_nodes`, `new_edges`, `removed_edges`, `summary`).
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffResult {
    pub new_nodes: Vec<DiffNode>,
    pub removed_nodes: Vec<DiffNode>,
    pub new_edges: Vec<DiffEdge>,
    pub removed_edges: Vec<DiffEdge>,
    pub summary: String,
}

/// Go `Diff`: edges compare by (source, target, relation), so a reversed edge is one removed and
/// one added.
#[allow(dead_code, unused_variables)]
pub fn diff(old: &Graph, new: &Graph) -> DiffResult {
    todo!("graphify-rs-ihc.13")
}
