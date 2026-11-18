//! Port of `validate.go`: the structural problems of a graph.json.

use std::path::Path;

use super::query::Error;

// graphify-rs-ihc.13: stubs until the code PR

/// Go `Validate`: the sorted issues (dangling endpoints, duplicate ids, empty ids), the node and
/// link counts, and the unclassified-files line. It decodes the file as is: unlike
/// [`load`](super::load) it does not strip a BOM (plan Review Focus 3).
#[allow(dead_code, unused_variables)]
pub fn validate(path: &Path) -> Result<(Vec<String>, i64, i64, String), Error> {
    todo!("graphify-rs-ihc.13")
}
