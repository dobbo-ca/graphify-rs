//! Port of graphify-go `internal/model`: nodes, edges, the graph, and their JSON.

mod code;
mod jsonenc;
#[allow(clippy::module_inception)]
mod model;

pub use code::{Confidence, File, Loc, Relation, conf, file_of, line, loc_of, rel};
pub use jsonenc::{append_json_float, append_json_string};
pub use model::{Edge, Extraction, Graph, NO_NODE, Node};

#[cfg(test)]
mod tests;
