//! Shared helpers (Go: the test files of package export share one namespace).

use std::collections::BTreeMap;
use std::path::Path;

use crate::export::{Error, to_json};
use crate::model::Graph;

mod export;
mod formats;
mod okf;
mod oldnodes;
mod stream;

/// The encoder fan-out the tests give `to_json` (Go: `runtime.GOMAXPROCS`).
const WORKERS: usize = 4;

/// Go `ToJSON(g, communities, path, commit, force)`.
fn to_json_at(
    g: &Graph,
    communities: &BTreeMap<i64, Vec<String>>,
    path: &Path,
    commit: &str,
    force: bool,
) -> Result<(), Error> {
    to_json(g, communities, path, commit, force, WORKERS)
}
