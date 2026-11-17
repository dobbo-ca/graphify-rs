//! Port of `merge.go`: the git merge driver for graph.json, the node and edge union of two files.

use std::path::Path;

use super::query::Error;

// graphify-rs-ihc.13: stubs until the code PR

/// Go `mergeMaxNodes`.
#[allow(dead_code, unused_variables)]
pub(super) const MERGE_MAX_NODES: usize = 100_000;

/// Go `Merge`: writes the union of `current` and `other` to `current` (nodes by id, current
/// wins; edges by (source, target, relation)), or fails on corrupt or oversized input.
#[allow(dead_code, unused_variables)]
pub fn merge(current: &Path, other: &Path) -> Result<(), Error> {
    todo!("graphify-rs-ihc.13")
}
