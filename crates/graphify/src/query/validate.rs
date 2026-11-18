//! Port of `validate.go`: the structural problems of a graph.json.

use std::collections::BTreeMap;
use std::path::Path;

use gostd::json;
use gostd::strconv::quote;

use super::query::{Error, Graph, parsing, read_file};
use crate::security::check_graph_file_size;

/// Go `Validate`: the sorted issues (dangling endpoints, duplicate ids, empty ids), the node and
/// link counts, and the unclassified-files line. It decodes the file as is: unlike
/// [`load`](super::load) it does not strip a BOM (plan Review Focus 3).
pub fn validate(path: &Path) -> Result<(Vec<String>, i64, i64, String), Error> {
    check_graph_file_size(path).map_err(|e| Error::Other(e.to_string()))?;
    let data = read_file(path)?;
    let g: Graph = json::unmarshal(&data).map_err(|e| parsing(path, &e))?;

    let mut issues = Vec::new();
    let mut seen: BTreeMap<&str, i64> = BTreeMap::new();
    for n in &g.nodes {
        if n.id.is_empty() {
            issues.push(format!(
                "node with empty id (label {})",
                quote(n.label.as_bytes())
            ));
            continue;
        }
        *seen.entry(n.id.as_str()).or_default() += 1;
    }
    for (id, &count) in &seen {
        if count > 1 {
            issues.push(format!(
                "duplicate node id {} ({count} times)",
                quote(id.as_bytes())
            ));
        }
    }
    for l in &g.links {
        if !seen.contains_key(l.source.as_str()) {
            issues.push(format!(
                "edge {} --{}--> {}: source node missing",
                l.source, l.relation, l.target
            ));
        }
        if !seen.contains_key(l.target.as_str()) {
            issues.push(format!(
                "edge {} --{}--> {}: target node missing",
                l.source, l.relation, l.target
            ));
        }
    }
    // sort.Strings: equal strings are indistinguishable, so any sort gives Go's order.
    issues.sort();
    Ok((
        issues,
        g.nodes.len() as i64,
        g.links.len() as i64,
        g.unclassified_summary(),
    ))
}
