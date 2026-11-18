//! Port of `merge.go`: the git merge driver for graph.json, the node and edge union of two files.

use std::collections::HashSet;
use std::path::Path;

use gostd::json::{self, RawMessage};

use super::query::{Error, parsing, read_file};
use crate::goerr::path_error;

/// Go `mergeMaxBytes`.
const MERGE_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Go `mergeMaxNodes`.
pub(super) const MERGE_MAX_NODES: usize = 100_000;

/// Go `rawGraph`: a graph.json with its nodes and links left as raw JSON. A nil-able Go slice
/// is an `Option`.
#[derive(Default)]
struct RawGraph {
    directed: bool,
    multigraph: bool,
    graph: RawMessage,
    nodes: Option<Vec<RawMessage>>,
    links: Option<Vec<RawMessage>>,
    hyperedges: RawMessage,
    built_at_commit: RawMessage,
}

gostd::json_fields!(decode RawGraph, "query.rawGraph", {
    directed => "directed",
    multigraph => "multigraph",
    graph => "graph",
    nodes => "nodes",
    links => "links",
    hyperedges => "hyperedges" omitempty,
    built_at_commit => "built_at_commit" omitempty,
});

/// Go `nodeKey`.
#[derive(Default)]
struct NodeKey {
    id: String,
}

gostd::json_fields!(decode NodeKey, "query.nodeKey", { id => "id" });

/// Go `linkKey`.
#[derive(Default)]
struct LinkKey {
    source: String,
    target: String,
    relation: String,
}

gostd::json_fields!(decode LinkKey, "query.linkKey", {
    source => "source",
    target => "target",
    relation => "relation",
});

/// Go `Merge`: writes the union of `current` and `other` to `current` (nodes by id, current
/// wins; edges by (source, target, relation)), or fails on corrupt or oversized input.
pub fn merge(current: &Path, other: &Path) -> Result<(), Error> {
    let cur = load_raw_graph(current)?;
    let oth = load_raw_graph(other)?;

    let mut nodes = Vec::new();
    let mut seen_node = HashSet::new();
    for raw in [&cur.nodes, &oth.nodes].into_iter().flatten().flatten() {
        let k: NodeKey =
            json::unmarshal(&raw.0).map_err(|e| Error::Other(format!("parsing node: {e}")))?;
        if seen_node.insert(k.id) {
            nodes.push(raw.clone());
        }
    }
    if nodes.len() > MERGE_MAX_NODES {
        return Err(Error::Other(format!(
            "merged graph has {} nodes, exceeds {MERGE_MAX_NODES}-node cap",
            nodes.len()
        )));
    }

    let mut links = Vec::new();
    let mut seen_link = HashSet::new();
    for raw in [&cur.links, &oth.links].into_iter().flatten().flatten() {
        let k: LinkKey =
            json::unmarshal(&raw.0).map_err(|e| Error::Other(format!("parsing link: {e}")))?;
        if seen_link.insert((k.source, k.target, k.relation)) {
            links.push(raw.clone());
        }
    }

    let out = RawGraph {
        directed: cur.directed,
        multigraph: cur.multigraph,
        graph: cur.graph,
        // Go appends to nil slices: none is `null`.
        nodes: (!nodes.is_empty()).then_some(nodes),
        links: (!links.is_empty()).then_some(links),
        hyperedges: cur.hyperedges,
        built_at_commit: cur.built_at_commit,
    };
    let data = json::marshal_indent(&out, "", "  ").map_err(|e| Error::Other(e.to_string()))?;
    std::fs::write(current, data)
        .map_err(|e| Error::Other(path_error("open", current, &e).to_string()))
}

/// Go `loadRawGraph`: rejects a file over the byte cap before parsing it.
fn load_raw_graph(path: &Path) -> Result<RawGraph, Error> {
    let size = std::fs::metadata(path)
        .map_err(|e| {
            Error::Other(format!(
                "cannot stat {}: {}",
                path.display(),
                path_error("stat", path, &e)
            ))
        })?
        .len();
    if size > MERGE_MAX_BYTES {
        return Err(Error::Other(format!(
            "graph file {} is {size} bytes, exceeds {MERGE_MAX_BYTES}-byte cap",
            path.display()
        )));
    }
    let data = read_file(path)?;
    json::unmarshal(&data).map_err(|e| parsing(path, &e))
}
