//! Port of `report.go`: renders GRAPH_REPORT.md.

use std::collections::BTreeMap;

use crate::model::Graph;

/// Go `Generate`: builds the GRAPH_REPORT.md body. A commit is shortened to 8 bytes; `""` omits
/// the freshness section.
pub fn generate(
    g: &Graph,
    c: &BTreeMap<i64, Vec<String>>,
    root: &str,
    built_at_commit: &str,
) -> String {
    todo!("graphify-rs-ihc.11")
}
