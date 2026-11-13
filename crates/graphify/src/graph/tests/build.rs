//! Port of graphify-go `internal/graph/build_test.go`.

use crate::graph::build;
use crate::model::{Confidence, Edge, Extraction, Node, Relation};

fn node(id: &str, label: &str, file_type: &str, source_file: &str) -> Node {
    Node {
        id: id.into(),
        label: label.into(),
        file_type: file_type.into(),
        source_file: source_file.into(),
        ..Node::default()
    }
}

fn edge(source: &str, target: &str, relation: Relation, confidence: Confidence) -> Edge {
    Edge {
        source: source.into(),
        target: target.into(),
        relation,
        confidence,
        ..Edge::default()
    }
}

#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_drops_dangling_and_cross_lang_calls() {
    let ext = Extraction {
        nodes: vec![
            node("a", "a()", "code", "x.go"),
            node("b", "b()", "code", "y.ts"),
            node("c", "c()", "code", "z.go"),
        ],
        edges: vec![
            edge("a", "c", Relation::CALLS, Confidence::INFERRED), // same family, kept
            edge("a", "b", Relation::CALLS, Confidence::INFERRED), // cross-language, dropped
            edge("a", "ghost", Relation::CALLS, Confidence::EXTRACTED), // dangling, dropped
        ],
    };
    let g = build(ext);
    assert_eq!(g.num_nodes(), 3);
    assert!(
        g.has_edge("a", "c"),
        "same-language inferred call a->c was dropped"
    );
    assert!(
        !g.has_edge("a", "b"),
        "cross-language inferred call a->b should be dropped"
    );
    assert_eq!(g.num_edges(), 1, "only a->c");
}

#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_drops_cross_lang_imports_keeps_concept() {
    let ext = Extraction {
        nodes: vec![
            node("worker", "worker.rb", "code", "backend/worker.rb"),
            node("time_ts", "time.ts", "code", "src/time.ts"),
            node("util_ts", "util.ts", "code", "src/util.ts"),
            node("pkg", "package.json", "concept", "package.json"),
        ],
        edges: vec![
            // cross-language (ruby -> jsts), EXTRACTED: phantom, dropped
            edge(
                "worker",
                "time_ts",
                Relation::IMPORTS_FROM,
                Confidence::EXTRACTED,
            ),
            // same family (jsts -> jsts): kept
            edge(
                "time_ts",
                "util_ts",
                Relation::IMPORTS,
                Confidence::EXTRACTED,
            ),
            // concept (empty family) -> code: kept
            edge(
                "pkg",
                "time_ts",
                Relation::REFERENCES,
                Confidence::EXTRACTED,
            ),
        ],
    };
    let g = build(ext);
    assert!(
        !g.has_edge("worker", "time_ts"),
        "cross-language imports_from should be dropped"
    );
    assert!(
        g.has_edge("time_ts", "util_ts"),
        "same-family import was dropped"
    );
    assert!(
        g.has_edge("pkg", "time_ts"),
        "concept->code reference should be kept"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_keeps_both_directions() {
    let ext = Extraction {
        nodes: vec![node("a", "a()", "", "x.go"), node("b", "b()", "", "x.go")],
        edges: vec![
            edge("a", "b", Relation::CALLS, Confidence::INFERRED),
            edge("b", "a", Relation::CALLS, Confidence::INFERRED),
        ],
    };
    assert_eq!(build(ext).num_edges(), 2, "both directions kept");
}

#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_keeps_provenance_on_id_collision() {
    let mut doc = node("model", "Model Doc", "concept", "model.md");
    doc.source_location = "L1".into();
    let ext = Extraction {
        // the external concept node for `import model` comes after the per-file nodes
        nodes: vec![doc, node("model", "model", "concept", "")],
        edges: vec![],
    };
    let g = build(ext);
    let n = g.node("model").unwrap();
    assert!(
        n.source_file == "model.md" && n.label == "Model Doc" && n.source_location == "L1",
        "collision erased provenance: {n:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_fills_missing_attributes_from_same_file() {
    let mut first = node("a", "a()", "code", "x.go");
    first.source_location = "L7".into();
    let ext = Extraction {
        nodes: vec![first, node("a", "a()", "", "x.go")],
        edges: vec![],
    };
    let g = build(ext);
    let n = g.node("a").unwrap();
    assert!(
        n.file_type == "code" && n.source_location == "L7",
        "same-file re-add lost attributes: {n:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_keeps_component_edges() {
    let ext = Extraction {
        nodes: vec![
            node("app", "App.vue", "code", "App.vue"),
            node("util", "util.py", "code", "util.py"),
        ],
        edges: vec![edge(
            "app",
            "util",
            Relation::IMPORTS_FROM,
            Confidence::EXTRACTED,
        )],
    };
    assert!(
        build(ext).has_edge("app", "util"),
        "component -> python import was dropped"
    );
}
