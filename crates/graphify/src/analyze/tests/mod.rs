//! Shared helpers (Go: the test files of package analyze share one namespace).

use std::collections::BTreeMap;

use crate::analyze::Cycle;
use crate::model::{Confidence, Edge, Node, Relation};

mod analyze;
mod cycles;
mod typeonly_cycle;

/// Go `model.Node{ID: id, Label: label, SourceFile: source_file}`.
fn node(id: &str, label: &str, source_file: &str) -> Node {
    Node {
        id: id.to_string(),
        label: label.to_string(),
        source_file: source_file.to_string(),
        ..Node::default()
    }
}

/// Go `model.Node{ID: id, Label: label, FileType: file_type, SourceFile: source_file}`.
fn node_ft(id: &str, label: &str, file_type: &str, source_file: &str) -> Node {
    Node {
        file_type: file_type.to_string(),
        ..node(id, label, source_file)
    }
}

/// Go `model.Edge{Source: s, Target: t, Relation: relation, Confidence: confidence}`.
fn edge(s: &str, t: &str, relation: Relation, confidence: Confidence) -> Edge {
    Edge {
        source: s.to_string(),
        target: t.to_string(),
        relation,
        confidence,
        ..Edge::default()
    }
}

/// `communityID -> members` from literals (Go `map[int][]string{...}`).
fn communities(entries: &[(i64, &[&str])]) -> BTreeMap<i64, Vec<String>> {
    entries
        .iter()
        .map(|(cid, nodes)| (*cid, nodes.iter().map(|s| (*s).to_string()).collect()))
        .collect()
}

/// Python `_names`: each cycle as its sorted file base names, the cycles sorted.
fn names(cycles: &[Cycle]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = cycles
        .iter()
        .map(|c| {
            let mut v: Vec<String> = c
                .files
                .iter()
                .map(|f| f.rsplit('/').next().unwrap_or(f).to_string())
                .collect();
            v.sort();
            v
        })
        .collect();
    out.sort();
    out
}
