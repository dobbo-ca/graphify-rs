//! Port of `diff.go`: the nodes and edges added or removed between two graph snapshots.

use std::collections::BTreeMap;

use gostd::json::{EncodeState, Error as JsonError, Marshal};

use super::query::Graph;

/// Go `DiffNode` (json `id`, `label`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffNode {
    pub id: String,
    pub label: String,
}

gostd::json_fields!(DiffNode, "query.DiffNode", { id => "id", label => "label" });

/// Go `DiffEdge` (json `source`, `target`, `relation`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

gostd::json_fields!(DiffEdge, "query.DiffEdge", {
    source => "source",
    target => "target",
    relation => "relation",
});

/// Go `DiffResult` (json `new_nodes`, `removed_nodes`, `new_edges`, `removed_edges`, `summary`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffResult {
    pub new_nodes: Vec<DiffNode>,
    pub removed_nodes: Vec<DiffNode>,
    pub new_edges: Vec<DiffEdge>,
    pub removed_edges: Vec<DiffEdge>,
    pub summary: String,
}

/// `DiffResult` as Go encodes it: `Diff` appends to nil slices, so an empty list is `null`.
struct DiffJson<'a> {
    new_nodes: Option<&'a [DiffNode]>,
    removed_nodes: Option<&'a [DiffNode]>,
    new_edges: Option<&'a [DiffEdge]>,
    removed_edges: Option<&'a [DiffEdge]>,
    summary: &'a str,
}

gostd::json_fields!(DiffJson<'_>, "query.DiffResult", {
    new_nodes => "new_nodes",
    removed_nodes => "removed_nodes",
    new_edges => "new_edges",
    removed_edges => "removed_edges",
    summary => "summary",
});

/// Go's nil-or-not slice.
fn nil<T>(v: &[T]) -> Option<&[T]> {
    (!v.is_empty()).then_some(v)
}

impl Marshal for DiffResult {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), JsonError> {
        DiffJson {
            new_nodes: nil(&self.new_nodes),
            removed_nodes: nil(&self.removed_nodes),
            new_edges: nil(&self.new_edges),
            removed_edges: nil(&self.removed_edges),
            summary: &self.summary,
        }
        .marshal_json(e)
    }
}

/// Go `Diff`: edges compare by (source, target, relation), so a reversed edge is one removed and
/// one added.
pub fn diff(old: &Graph, new: &Graph) -> DiffResult {
    let old_nodes = node_labels(old);
    let new_nodes = node_labels(new);

    // Go ranges over maps here; the sorts below are total orders, so the result is the same.
    let mut res = DiffResult::default();
    for (id, label) in &new_nodes {
        if !old_nodes.contains_key(id) {
            res.new_nodes.push(diff_node(id, label));
        }
    }
    for (id, label) in &old_nodes {
        if !new_nodes.contains_key(id) {
            res.removed_nodes.push(diff_node(id, label));
        }
    }

    let old_edges = edge_keys(old);
    let new_edges = edge_keys(new);
    for (k, e) in &new_edges {
        if !old_edges.contains_key(k) {
            res.new_edges.push(e.clone());
        }
    }
    for (k, e) in &old_edges {
        if !new_edges.contains_key(k) {
            res.removed_edges.push(e.clone());
        }
    }

    sort_nodes(&mut res.new_nodes);
    sort_nodes(&mut res.removed_nodes);
    sort_edges(&mut res.new_edges);
    sort_edges(&mut res.removed_edges);
    res.summary = diff_summary(&res);
    res
}

fn diff_node(id: &str, label: &str) -> DiffNode {
    DiffNode {
        id: id.to_string(),
        label: label.to_string(),
    }
}

/// Go `nodeLabels`: id to label (the id when the label is empty); the last node wins.
fn node_labels(g: &Graph) -> BTreeMap<&str, &str> {
    g.nodes
        .iter()
        .map(|n| {
            let label = if n.label.is_empty() { &n.id } else { &n.label };
            (n.id.as_str(), label.as_str())
        })
        .collect()
}

/// Go `edgeKeys`: the first link per `edgeKey` (source, target and relation joined by NUL, so
/// ids holding NUL can share a key as in Go).
fn edge_keys(g: &Graph) -> BTreeMap<String, DiffEdge> {
    let mut m = BTreeMap::new();
    for l in &g.links {
        m.entry(format!("{}\0{}\0{}", l.source, l.target, l.relation))
            .or_insert_with(|| DiffEdge {
                source: l.source.clone(),
                target: l.target.clone(),
                relation: l.relation.clone(),
            });
    }
    m
}

/// Go `sortNodes`.
fn sort_nodes(ns: &mut [DiffNode]) {
    gostd::sort::slice(ns, |a, b| {
        if a.label != b.label {
            return a.label < b.label;
        }
        a.id < b.id
    });
}

/// Go `sortEdges`.
fn sort_edges(es: &mut [DiffEdge]) {
    gostd::sort::slice(es, |a, b| {
        if a.source != b.source {
            return a.source < b.source;
        }
        if a.target != b.target {
            return a.target < b.target;
        }
        a.relation < b.relation
    });
}

/// Go `diffSummary`.
fn diff_summary(r: &DiffResult) -> String {
    let mut parts = Vec::new();
    let n = r.new_nodes.len();
    if n > 0 {
        parts.push(format!("{n} new {}", plural(n, "node")));
    }
    let n = r.new_edges.len();
    if n > 0 {
        parts.push(format!("{n} new {}", plural(n, "edge")));
    }
    let n = r.removed_nodes.len();
    if n > 0 {
        parts.push(format!("{n} {} removed", plural(n, "node")));
    }
    let n = r.removed_edges.len();
    if n > 0 {
        parts.push(format!("{n} {} removed", plural(n, "edge")));
    }
    if parts.is_empty() {
        return "no changes".to_string();
    }
    parts.join(", ")
}

/// Go `plural`.
fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}
