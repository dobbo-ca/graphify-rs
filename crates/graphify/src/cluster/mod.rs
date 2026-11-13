//! Port of graphify-go `internal/cluster`. Filled by graphify-rs-ihc.9 (plan Task 8).

#[allow(clippy::module_inception)]
mod cluster;
mod louvain;

pub use cluster::{cluster, cohesion, node_community, remap_to_previous, scores};

#[cfg(test)]
pub(crate) mod tests;
