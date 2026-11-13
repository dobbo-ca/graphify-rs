//! Port of `analyze.go`: god nodes, surprising cross-file connections, and file-level import
//! cycles.

use std::collections::BTreeMap;

use crate::model::Graph;

/// Go `GodNode`: a highly-connected core abstraction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GodNode {
    pub id: String,
    pub label: String,
    pub degree: i64,
}

/// Go `Surprise`: a non-obvious cross-file connection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surprise {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    pub source_files: [String; 2],
    pub note: String,
}

/// Go `Cycle`: a circular file-level import dependency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cycle {
    pub files: Vec<String>,
}

/// Go `GodNodes`: the `top_n` most-connected real entities (file hubs, concept nodes and generic
/// JSON keys excluded), optionally without nodes above the degree percentile
/// `exclude_hubs_percentile` (1-100; 0 keeps the historical ranking).
pub fn god_nodes(g: &Graph, top_n: i64, exclude_hubs_percentile: i64) -> Vec<GodNode> {
    todo!("graphify-rs-ihc.10")
}

/// Go `Surprising`: up to `top_n` cross-file edges between real entities, ranked by how
/// non-obvious they are. A node with no community is community 0 (`analyze.go:147`).
pub fn surprising(g: &Graph, c: &BTreeMap<i64, Vec<String>>, top_n: i64) -> Vec<Surprise> {
    todo!("graphify-rs-ihc.10")
}

/// Go `ImportCycles`: circular dependencies in the file-level import graph, at most `max_len`
/// files long, deduplicated by rotation, shortest first, at most `top_n`.
pub fn import_cycles(g: &Graph, max_len: i64, top_n: i64) -> Vec<Cycle> {
    todo!("graphify-rs-ihc.10")
}

/// Go `isFileNode`.
pub(super) fn is_file_node(g: &Graph, id: &str) -> bool {
    todo!("graphify-rs-ihc.10")
}

/// Go `isConceptNode`.
pub(super) fn is_concept_node(g: &Graph, id: &str) -> bool {
    todo!("graphify-rs-ihc.10")
}

/// Go `isJSONKeyNode`.
pub(super) fn is_json_key_node(g: &Graph, id: &str) -> bool {
    todo!("graphify-rs-ihc.10")
}
