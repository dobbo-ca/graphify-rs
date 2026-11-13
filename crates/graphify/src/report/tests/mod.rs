//! Shared helpers (Go: the test file of package report holds `reportGraph`).

use crate::model::{Confidence, Edge, Graph, Node, Relation};

mod report;

/// A node with the given ID, label and source file.
fn node(id: &str, label: &str, source_file: &str) -> Node {
    Node {
        id: id.to_string(),
        label: label.to_string(),
        source_file: source_file.to_string(),
        ..Node::default()
    }
}

/// Go `model.Edge{Source: s, Target: t, Relation: r, Confidence: c}`.
fn edge(s: &str, t: &str, relation: Relation, confidence: Confidence) -> Edge {
    Edge {
        source: s.to_string(),
        target: t.to_string(),
        relation,
        confidence,
        ..Edge::default()
    }
}

/// Go `reportGraph`: two files (a.go, b.go) with three functions each, calls inside each file,
/// one INFERRED call across, and a two-file import cycle.
fn report_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(node("fa", "a.go", "pkg/a.go"));
    g.add_node(node("fb", "b.go", "pkg/b.go"));
    for id in ["a1", "a2", "a3"] {
        g.add_node(node(id, &format!("{id}()"), "pkg/a.go"));
        g.add_edge(edge("fa", id, Relation::CONTAINS, Confidence::EXTRACTED));
    }
    for id in ["b1", "b2", "b3"] {
        g.add_node(node(id, &format!("{id}()"), "pkg/b.go"));
        g.add_edge(edge("fb", id, Relation::CONTAINS, Confidence::EXTRACTED));
    }
    g.add_edge(edge("a1", "a2", Relation::CALLS, Confidence::EXTRACTED));
    g.add_edge(edge("b1", "b2", Relation::CALLS, Confidence::EXTRACTED));
    g.add_edge(edge("a1", "b1", Relation::CALLS, Confidence::INFERRED));
    g.add_edge(edge(
        "fa",
        "fb",
        Relation::IMPORTS_FROM,
        Confidence::EXTRACTED,
    ));
    g.add_edge(edge(
        "fb",
        "fa",
        Relation::IMPORTS_FROM,
        Confidence::EXTRACTED,
    ));
    g
}
