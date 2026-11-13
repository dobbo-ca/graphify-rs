//! Port of `formats.go`: GraphML, DOT and CSV from a graph.json.

use std::path::Path;

use super::export::Error;

/// Go `GraphMLFromJSON` (`formats.go:69`).
pub fn graphml_from_json(json_path: &Path, out_path: &Path) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}

/// Go `DOTFromJSON` (`formats.go:113`).
pub fn dot_from_json(json_path: &Path, out_path: &Path) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}

/// Go `CSVFromJSON` (`formats.go:141`).
pub fn csv_from_json(json_path: &Path, nodes_path: &Path, edges_path: &Path) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}
