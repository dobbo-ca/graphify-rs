//! Port of graphify-go `internal/query`. Filled by graphify-rs-ihc.13 (plan Task 12).

mod affected;
mod ask;
mod diff;
mod merge;
#[allow(clippy::module_inception)]
mod query;
mod validate;

pub use affected::{AffectedOptions, AffectedResult, affected};
pub use ask::ask;
pub use diff::{DiffEdge, DiffNode, DiffResult, diff};
pub use merge::merge;
pub use query::{
    AmbiguousError, ERR_NO_DIRECTED_PATH, Error, Explanation, Graph, GraphAttrs, Link, Match,
    MaxHopsError, Neighbor, Node, PathEdge, PathResult, SameNodeError, explain, load, path_edges,
    query,
};
pub use validate::validate;

#[cfg(test)]
mod tests;
