//! Port of `cluster.go`: Louvain, oversized-community split, and stable community ids.

use std::collections::BTreeMap;

use crate::model::Graph;

/// Go `Cluster`: partitions `g` into communities, `communityID -> sorted node IDs`.
pub fn cluster(g: &Graph) -> BTreeMap<i64, Vec<String>> {
    todo!("graphify-rs-ihc.9")
}

/// Go `louvain`: Louvain over the induced subgraph on `node_set`, as string-ID groups.
pub(super) fn louvain(g: &Graph, node_set: &[String]) -> Vec<Vec<String>> {
    todo!("graphify-rs-ihc.9")
}

/// Go `RemapToPrevious` (dead code in Go, ported for its tests): relabels `communities` so their
/// ids stay stable against `prev_node_community`.
pub fn remap_to_previous(
    communities: &BTreeMap<i64, Vec<String>>,
    prev_node_community: &BTreeMap<String, i64>,
) -> BTreeMap<i64, Vec<String>> {
    todo!("graphify-rs-ihc.9")
}

/// Go `Cohesion`: actual intra-community edges over the maximum possible.
pub fn cohesion(g: &Graph, nodes: &[String]) -> f64 {
    todo!("graphify-rs-ihc.9")
}

/// Go `NodeCommunity`: inverts communities into node ID -> community ID.
pub fn node_community(c: &BTreeMap<i64, Vec<String>>) -> BTreeMap<String, i64> {
    todo!("graphify-rs-ihc.9")
}

/// Go `Scores`: each community's cohesion.
pub fn scores(g: &Graph, c: &BTreeMap<i64, Vec<String>>) -> BTreeMap<i64, f64> {
    todo!("graphify-rs-ihc.9")
}
