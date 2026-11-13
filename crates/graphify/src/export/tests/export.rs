//! Port of `export_test.go` (9 tests), then the upstream cases of `test_export.py`,
//! `test_hypergraph.py` that hold for graphify-go v0.12.0, then the added tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use gostd::json::{Value, unmarshal};

use super::to_json_at;
use crate::export::export::{JsonGraph, norm_label, to_json_link};
use crate::export::{Error, check_shrink};
use crate::model::{Confidence, Edge, Graph, Node, Relation, line};
use crate::testutil::TempDir;

/// A `code` node in `source_file`, label = id.
fn node(id: &str, source_file: &str) -> Node {
    Node {
        id: id.to_string(),
        label: id.to_string(),
        file_type: "code".to_string(),
        source_file: source_file.to_string(),
        ..Node::default()
    }
}

/// Go `graphOfSize`: a graph with n distinct nodes and no edges.
fn graph_of_size(n: usize) -> Graph {
    let mut g = Graph::new();
    for i in 0..n {
        g.add_node(node(&format!("n{i}"), "pkg/a.go"));
    }
    g
}

/// Go `graphWith`: a graph of exactly these nodes.
fn graph_with(nodes: &[&Node]) -> Graph {
    let mut g = Graph::new();
    for n in nodes {
        g.add_node((*n).clone());
    }
    g
}

/// Go `json.Unmarshal(data, &jsonGraph)`.
fn read_graph(path: &Path) -> JsonGraph {
    let data = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    unmarshal::<JsonGraph>(&data).unwrap_or_else(|e| panic!("unmarshal {}: {e}", path.display()))
}

/// Go `nodeCount`: reads a node-link graph.json and returns its node count.
fn node_count(path: &Path) -> usize {
    read_graph(path).nodes.map_or(0, |n| n.len())
}

/// Go `CheckShrink(path, g, skipped, root, force)`; `root` is a directory made by `TempDir`.
fn check(
    path: &Path,
    g: &Graph,
    skipped: &[&str],
    root: &TempDir,
    force: bool,
) -> Result<(), Error> {
    let skipped: BTreeSet<String> = skipped.iter().map(|s| (*s).to_string()).collect();
    let root = root.path().to_str().expect("temp dir is UTF-8");
    check_shrink(path, g, &skipped, root, force)
}

/// Go `seed`: writes an on-disk graph.json of the given nodes and returns its path. The
/// directory lives in `dirs` until the test ends, as Go's `t.TempDir()` does.
fn seed(dirs: &mut Vec<TempDir>, nodes: &[&Node]) -> PathBuf {
    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&graph_with(nodes), &BTreeMap::new(), &path, "seed", true).expect("seed");
    dirs.push(dir);
    path
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_anti_shrink() {
    let comm = BTreeMap::new();
    let dir = TempDir::new();
    let path = dir.join("graph.json");

    // Seed disk with a 5-node graph.
    to_json_at(&graph_of_size(5), &comm, &path, "c1", false).expect("seed write");
    assert_eq!(node_count(&path), 5, "seed node count");

    // A smaller (2-node) graph is refused and the 5-node file is left intact.
    let err = to_json_at(&graph_of_size(2), &comm, &path, "c2", false);
    assert!(
        matches!(err, Err(Error::GraphShrink(_))),
        "shrink write err = {err:?}, want ErrGraphShrink"
    );
    assert_eq!(
        node_count(&path),
        5,
        "after refused shrink node count, want 5 (file must be unchanged)"
    );

    // force=true overwrites even though it shrinks.
    to_json_at(&graph_of_size(2), &comm, &path, "c3", true).expect("forced shrink write");
    assert_eq!(node_count(&path), 2, "after forced shrink node count");

    // A larger graph is never refused.
    to_json_at(&graph_of_size(9), &comm, &path, "c4", false).expect("growth write");
    assert_eq!(node_count(&path), 9, "after growth node count");
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_check_shrink() {
    let n_a = node("a", "pkg/a.go");
    let n_b = node("b", "pkg/b.go");
    let n_c = node("c", "pkg/c.go");
    let mut dirs = Vec::new();

    // root has none of the pkg/*.go source files on disk, so a lost node's source
    // reads as deleted (not excluded) unless a subtest creates it.
    let root = TempDir::new();

    // Growth (or same size) is never a shrink, regardless of skipped.
    let r = check(
        &seed(&mut dirs, &[&n_a, &n_b]),
        &graph_with(&[&n_a, &n_b, &n_c]),
        &["pkg/b.go"],
        &root,
        false,
    );
    assert!(r.is_ok(), "growth: {r:?}, want nil");

    // Fail closed (#1795): b left the corpus but its source still exists on disk,
    // so it was excluded (e.g. a new ignore rule), not deleted: refuse the shrink.
    let excluded_root = TempDir::new();
    excluded_root.write("pkg/b.go", b"x");
    let r = check(
        &seed(&mut dirs, &[&n_a, &n_b]),
        &graph_with(&[&n_a]),
        &[],
        &excluded_root,
        false,
    );
    assert!(
        matches!(r, Err(Error::GraphShrink(_))),
        "excluded-but-present source err = {r:?}, want ErrGraphShrink"
    );

    // b's file left the corpus and is absent from root's disk: a deletion, allowed.
    let r = check(
        &seed(&mut dirs, &[&n_a, &n_b]),
        &graph_with(&[&n_a]),
        &[],
        &root,
        false,
    );
    assert!(
        r.is_ok(),
        "deletion (source gone from disk): {r:?}, want nil"
    );

    // Deletion: b's file no longer current, so it never appears in skipped: allowed.
    let r = check(
        &seed(&mut dirs, &[&n_a, &n_b]),
        &graph_with(&[&n_a]),
        &["pkg/z.go"],
        &root,
        false,
    );
    assert!(r.is_ok(), "deletion shrink: {r:?}, want nil");

    // Silent loss: b's source file is still present but was skipped this run: refuse.
    let r = check(
        &seed(&mut dirs, &[&n_a, &n_b]),
        &graph_with(&[&n_a]),
        &["pkg/b.go"],
        &root,
        false,
    );
    assert!(
        matches!(r, Err(Error::GraphShrink(_))),
        "skipped-file shrink err = {r:?}, want ErrGraphShrink"
    );

    // force bypasses even a skipped-file shrink.
    let r = check(
        &seed(&mut dirs, &[&n_a, &n_b]),
        &graph_with(&[&n_a]),
        &["pkg/b.go"],
        &root,
        true,
    );
    assert!(r.is_ok(), "force bypass: {r:?}, want nil");

    // Absent target: nothing to lose.
    let absent = TempDir::new();
    let r = check(
        &absent.join("graph.json"),
        &graph_with(&[&n_a]),
        &[],
        &root,
        false,
    );
    assert!(r.is_ok(), "absent target: {r:?}, want nil");

    // Empty (whitespace-only) target: proceed even with a would-be skip set.
    let empty = TempDir::new();
    let empty_path = empty.write("graph.json", b"  \n");
    let r = check(
        &empty_path,
        &graph_with(&[&n_a]),
        &["pkg/b.go"],
        &root,
        false,
    );
    assert!(r.is_ok(), "empty target: {r:?}, want nil");

    // Unparseable existing graph: fail safe.
    let corrupt = TempDir::new();
    let corrupt_path = corrupt.write("graph.json", b"{ not json");
    let r = check(&corrupt_path, &graph_with(&[&n_a]), &[], &root, false);
    assert!(
        matches!(r, Err(Error::GraphUnverifiable(_))),
        "unparseable err = {r:?}, want ErrGraphUnverifiable"
    );
}

// A node lost to an in-place rebuild (its source is still in the new graph) is a
// normal refactor and must be allowed even when the source file exists on disk:
// the currentSources carve-out must short-circuit before the on-disk check.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_check_shrink_allows_rebuild_in_place() {
    let a1 = node("a1", "pkg/a.go");
    let a2 = node("a2", "pkg/a.go");
    let mut dirs = Vec::new();
    let path = seed(&mut dirs, &[&a1, &a2]);
    let root = TempDir::new();
    root.write("pkg/a.go", b"package p");
    // New graph dropped a2 but pkg/a.go is still built (a1 present): in-place
    // refactor. Must be allowed despite pkg/a.go existing on disk.
    let r = check(&path, &graph_with(&[&a1]), &[], &root, false);
    assert!(
        r.is_ok(),
        "rebuild-in-place shrink must be allowed with source on disk, got {r:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_empty_and_absent_target_writes() {
    let comm = BTreeMap::new();

    // Absent target: writes normally.
    let dir = TempDir::new();
    let absent = dir.join("graph.json");
    to_json_at(&graph_of_size(3), &comm, &absent, "c1", false).expect("absent-target write");
    assert_eq!(node_count(&absent), 3, "absent-target node count");

    // Empty (whitespace-only) target: writes normally, no shrink refusal.
    let dir = TempDir::new();
    let empty = dir.write("graph.json", b"   \n");
    to_json_at(&graph_of_size(1), &comm, &empty, "c2", false).expect("empty-target write");
    assert_eq!(node_count(&empty), 1, "empty-target node count");
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_unparseable_existing_refused() {
    let comm = BTreeMap::new();
    let dir = TempDir::new();
    let path = dir.write("graph.json", b"{ this is not json");

    let err = to_json_at(&graph_of_size(3), &comm, &path, "c1", false);
    assert!(
        matches!(err, Err(Error::GraphUnverifiable(_))),
        "unparseable-target err = {err:?}, want ErrGraphUnverifiable"
    );
    // File is left untouched (still the corrupt bytes).
    let data = fs::read(&path).expect("read");
    assert_eq!(data, b"{ this is not json", "corrupt file was modified");

    // force overwrites the corrupt file.
    to_json_at(&graph_of_size(3), &comm, &path, "c2", true)
        .expect("forced overwrite of corrupt file");
    assert_eq!(node_count(&path), 3, "after forced overwrite node count");
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json() {
    let mut g = Graph::new();
    g.add_node(Node {
        id: "fa".to_string(),
        label: "a.go".to_string(),
        file_type: "code".to_string(),
        source_file: "pkg/a.go".to_string(),
        source_location: "L1".to_string(),
        ..Node::default()
    });
    g.add_node(Node {
        id: "a1".to_string(),
        label: "Add()".to_string(),
        file_type: "code".to_string(),
        source_file: "pkg/a.go".to_string(),
        source_location: "L3".to_string(),
        ..Node::default()
    });
    g.add_edge(Edge {
        source: "fa".to_string(),
        target: "a1".to_string(),
        relation: Relation::CONTAINS,
        confidence: Confidence::EXTRACTED,
        ..Edge::default()
    });
    g.add_edge(Edge {
        source: "a1".to_string(),
        target: "fa".to_string(),
        relation: Relation::CALLS,
        confidence: Confidence::INFERRED,
        ..Edge::default()
    });
    let communities = BTreeMap::from([(0, vec!["a1".to_string(), "fa".to_string()])]);

    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&g, &communities, &path, "deadbeefcafe", false).expect("ToJSON");
    let out = read_graph(&path);

    assert!(
        !out.directed && !out.multigraph,
        "node-link graph should be undirected/simple, got directed={} multigraph={}",
        out.directed,
        out.multigraph
    );
    assert_eq!(out.built_at_commit, "deadbeefcafe", "built_at_commit");
    assert!(
        out.hyperedges.as_ref().is_some_and(Vec::is_empty),
        "hyperedges should serialize as an empty array, got {:?}",
        out.hyperedges
    );
    let nodes = out.nodes.unwrap_or_default();
    let links = out.links.unwrap_or_default();
    assert!(
        nodes.len() == 2 && links.len() == 2,
        "got {} nodes / {} links, want 2 / 2",
        nodes.len(),
        links.len()
    );

    // Every node carries its community and a norm_label.
    for n in &nodes {
        assert_eq!(
            n.community,
            Some(0),
            "node {:?} community = {:?}, want 0",
            n.id,
            n.community
        );
        assert!(
            !n.norm_label.is_empty(),
            "node {:?} has empty norm_label",
            n.id
        );
    }
    // confidence_score is derived from confidence when unset on the edge.
    for l in &links {
        match l.confidence.as_str() {
            "EXTRACTED" => assert!(
                l.confidence_score == 1.0,
                "EXTRACTED link score = {}, want 1.0",
                l.confidence_score
            ),
            "INFERRED" => assert!(
                l.confidence_score == 0.5,
                "INFERRED link score = {}, want 0.5",
                l.confidence_score
            ),
            _ => {}
        }
    }
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_edge_weight() {
    let mut g = Graph::new();
    g.add_node(node("a", "pkg/a.go"));
    g.add_node(node("b", "pkg/b.go"));
    // One edge carries a non-zero weight, the other leaves it unset.
    g.add_edge(Edge {
        source: "a".to_string(),
        target: "b".to_string(),
        relation: Relation::CALLS,
        confidence: Confidence::EXTRACTED,
        weight: 3.5,
        ..Edge::default()
    });
    g.add_edge(Edge {
        source: "b".to_string(),
        target: "a".to_string(),
        relation: Relation::CALLS,
        confidence: Confidence::EXTRACTED,
        ..Edge::default()
    });

    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&g, &BTreeMap::new(), &path, "c1", false).expect("ToJSON");
    let data = fs::read(&path).expect("read");

    // A weighted edge serializes its weight; an unweighted edge omits the field.
    let out = unmarshal::<JsonGraph>(&data).expect("unmarshal");
    let links = out.links.unwrap_or_default();
    assert_eq!(links.len(), 2, "got {} links, want 2", links.len());
    for l in &links {
        let want = if l.source == "a" { 3.5 } else { 0.0 };
        assert!(
            l.weight == want,
            "link {}->{} weight = {}, want {want}",
            l.source,
            l.target,
            l.weight
        );
    }

    // omitempty: the unweighted edge must not emit a "weight" key at all, so
    // existing consumers and round-trips are unaffected.
    for l in raw_links(&data) {
        let has_weight = l.contains_key("weight");
        let source = l.get("source");
        if source == Some(&Value::String("a".to_string())) && !has_weight {
            panic!("weighted edge is missing the weight key: {l:?}");
        }
        if source == Some(&Value::String("b".to_string())) && has_weight {
            panic!("unweighted edge must omit the weight key: {l:?}");
        }
    }
}

/// Go `struct{ Links []map[string]any }`: the `links` of a graph.json as raw objects.
fn raw_links(data: &[u8]) -> Vec<BTreeMap<String, Value>> {
    let doc = unmarshal::<Value>(data).expect("unmarshal raw");
    let Value::Object(top) = &doc else {
        panic!("graph.json is not an object");
    };
    let Some(Value::Array(links)) = top.get("links") else {
        return Vec::new();
    };
    links
        .iter()
        .map(|l| match l {
            Value::Object(m) => m.clone(),
            other => panic!("link is not an object: {other:?}"),
        })
        .collect()
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_computed_name() {
    let mut g = Graph::new();
    g.add_node(Node {
        id: "m1".to_string(),
        label: "module.this [null-label]".to_string(),
        file_type: "code".to_string(),
        source_file: "main.tf".to_string(),
        source_location: "L1".to_string(),
        computed_name: "eg-prod-app".to_string(),
        ..Node::default()
    });
    let communities = BTreeMap::from([(0, vec!["m1".to_string()])]);

    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&g, &communities, &path, "deadbeefcafe", false).expect("ToJSON");
    let nodes = read_graph(&path).nodes.unwrap_or_default();
    assert_eq!(nodes.len(), 1, "want 1 node, got {}", nodes.len());
    let n = &nodes[0];
    assert_eq!(n.computed_name, "eg-prod-app", "computed_name");
    assert!(
        n.norm_label.contains("eg-prod-app"),
        "norm_label = {:?}, want it to contain eg-prod-app",
        n.norm_label
    );
}

// TestToJSONSanitizesComputedName checks that an oversized, control-char-laden
// computed name (e.g. a null-label module with a huge literal namespace) is
// capped and stripped on the way into graph.json.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_sanitizes_computed_name() {
    let mut g = Graph::new();
    g.add_node(Node {
        id: "n0".to_string(),
        label: "module.big".to_string(),
        file_type: "code".to_string(),
        source_file: "main.tf".to_string(),
        computed_name: format!("{}\u{0}bad\u{7}", "a".repeat(5000)),
        ..Node::default()
    });
    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&g, &BTreeMap::new(), &path, "", false).expect("ToJSON");
    let nodes = read_graph(&path).nodes.unwrap_or_default();
    let got = &nodes[0].computed_name;
    assert_eq!(got.len(), 256, "computed_name length");
    assert!(
        !got.contains(['\u{0}', '\u{7}']),
        "computed_name retained control chars: {got:?}"
    );
    let nl = &nodes[0].norm_label;
    assert!(
        !nl.contains(['\u{0}', '\u{7}']) && nl.len() <= 300,
        "norm_label not sanitized: {} bytes",
        nl.len()
    );
}

// ---- Upstream (graphify v0.9.107) cases that hold for graphify-go v0.12.0 ----
//
// `tests/fixtures/extraction.json` and `cluster()` are replaced by a small graph and hand-made
// communities: cluster is its own bead, and the assertions do not depend on the partition.

/// Three nodes in two files and two edges, in communities 0 and 1.
fn upstream_graph() -> (Graph, BTreeMap<i64, Vec<String>>) {
    let mut g = Graph::new();
    g.add_node(node("n_transformer", "model.py"));
    g.add_node(node("n_attention", "model.py"));
    g.add_node(node("n_loss", "train.py"));
    g.add_edge(Edge {
        source: "n_transformer".to_string(),
        target: "n_attention".to_string(),
        relation: Relation::CONTAINS,
        confidence: Confidence::EXTRACTED,
        ..Edge::default()
    });
    g.add_edge(Edge {
        source: "n_loss".to_string(),
        target: "n_transformer".to_string(),
        relation: Relation::CALLS,
        confidence: Confidence::INFERRED,
        ..Edge::default()
    });
    let communities = BTreeMap::from([
        (
            0,
            vec!["n_transformer".to_string(), "n_attention".to_string()],
        ),
        (1, vec!["n_loss".to_string()]),
    ]);
    (g, communities)
}

// test_export.py::test_to_json_creates_file
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_creates_file() {
    let (g, communities) = upstream_graph();
    let dir = TempDir::new();
    let out = dir.join("graph.json");
    to_json_at(&g, &communities, &out, "", false).expect("ToJSON");
    assert!(out.exists(), "graph.json was not written");
}

// test_export.py::test_to_json_valid_json
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_valid_json() {
    let (g, communities) = upstream_graph();
    let dir = TempDir::new();
    let out = dir.join("graph.json");
    to_json_at(&g, &communities, &out, "", false).expect("ToJSON");
    let doc = unmarshal::<Value>(&fs::read(&out).expect("read")).expect("graph.json is valid JSON");
    let Value::Object(top) = &doc else {
        panic!("graph.json is not an object");
    };
    assert!(top.contains_key("nodes"), "no nodes key");
    assert!(top.contains_key("links"), "no links key");
}

// test_export.py::test_to_json_nodes_have_community. Go writes `"community": null` for a node
// in no community, so the key is always there.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_nodes_have_community() {
    let (g, communities) = upstream_graph();
    let dir = TempDir::new();
    let out = dir.join("graph.json");
    to_json_at(&g, &communities, &out, "", false).expect("ToJSON");
    let doc = unmarshal::<Value>(&fs::read(&out).expect("read")).expect("decode");
    let Value::Object(top) = &doc else {
        panic!("graph.json is not an object");
    };
    let Some(Value::Array(nodes)) = top.get("nodes") else {
        panic!("nodes is not an array");
    };
    assert_eq!(nodes.len(), 3, "node count");
    for n in nodes {
        let Value::Object(n) = n else {
            panic!("node is not an object: {n:?}");
        };
        assert!(n.contains_key("community"), "node without community: {n:?}");
    }
}

// test_export.py::test_to_json_refuses_shrink
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_refuses_shrink() {
    let dir = TempDir::new();
    let nodes: Vec<String> = (0..5).map(|i| format!(r#"{{"id": "n{i}"}}"#)).collect();
    let p = dir.write(
        "graph.json",
        format!(r#"{{"nodes": [{}]}}"#, nodes.join(", ")).as_bytes(),
    );
    let comm = BTreeMap::new();
    let r = to_json_at(&graph_of_size(2), &comm, &p, "", false);
    assert!(
        matches!(r, Err(Error::GraphShrink(_))),
        "shrink err = {r:?}, want ErrGraphShrink"
    );
    // force overrides
    to_json_at(&graph_of_size(2), &comm, &p, "", true).expect("forced write");
    assert_eq!(node_count(&p), 2, "node count after force");
}

// test_export.py::test_to_json_fails_safe_on_corrupt_existing
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_fails_safe_on_corrupt_existing() {
    let dir = TempDir::new();
    let p = dir.write("graph.json", b"{ this has content but is not valid json");
    let comm = BTreeMap::new();
    let r = to_json_at(&graph_of_size(10), &comm, &p, "", false);
    assert!(
        matches!(r, Err(Error::GraphUnverifiable(_))),
        "corrupt existing err = {r:?}, want ErrGraphUnverifiable"
    );
    to_json_at(&graph_of_size(10), &comm, &p, "", true).expect("forced write");
}

// test_export.py::test_to_json_proceeds_on_empty_existing
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_proceeds_on_empty_existing() {
    let dir = TempDir::new();
    let p = dir.write("graph.json", b"");
    to_json_at(&graph_of_size(3), &BTreeMap::new(), &p, "", false).expect("empty-target write");
    assert_eq!(node_count(&p), 3, "node count");
}

// test_export.py::test_existing_graph_node_count. Go has no counting helper: each state is
// seen through what `to_json` does with it (a 1-node graph written over it, force false).
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_existing_graph_node_state() {
    let g = graph_of_size(1);
    let comm = BTreeMap::new();
    let dir = TempDir::new();
    let state = |name: &str, content: Option<&[u8]>| -> Result<(), Error> {
        let p = dir.join(name);
        if let Some(c) = content {
            dir.write(name, c);
        }
        to_json_at(&g, &comm, &p, "", false)
    };
    // absent and empty: nothing to protect
    assert!(state("absent.json", None).is_ok(), "absent");
    assert!(state("empty.json", Some(b"")).is_ok(), "empty");
    // non-empty but unparseable, or structurally wrong: fail closed
    let r = state("malformed.json", Some(b"{not json"));
    assert!(
        matches!(r, Err(Error::GraphUnverifiable(_))),
        "malformed: {r:?}"
    );
    let r = state("notalist.json", Some(br#"{"nodes": "notalist"}"#));
    assert!(
        matches!(r, Err(Error::GraphUnverifiable(_))),
        "nodes not a list: {r:?}"
    );
    // valid, 2 nodes: the 1-node graph is a shrink
    let r = state(
        "two.json",
        Some(br#"{"nodes": [{"id": "a"}, {"id": "b"}], "links": []}"#),
    );
    assert!(matches!(r, Err(Error::GraphShrink(_))), "two nodes: {r:?}");
}

// test_hypergraph.py::test_to_json_hyperedges_empty_when_none. Go has no hyperedges: the key is
// always an empty array.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_json_hyperedges_empty_when_none() {
    let (g, communities) = upstream_graph();
    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&g, &communities, &path, "", false).expect("ToJSON");
    let data = fs::read(&path).expect("read");
    let doc = unmarshal::<Value>(&data).expect("decode");
    let Value::Object(top) = &doc else {
        panic!("graph.json is not an object");
    };
    assert_eq!(
        top.get("hyperedges"),
        Some(&Value::Array(Vec::new())),
        "hyperedges"
    );
}

// ---- Added tests ----

// The texts of the two guard errors (plan R6; they reach stderr through cli::path_error).
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn shrink_error_texts() {
    let comm = BTreeMap::new();
    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&graph_of_size(5), &comm, &path, "c1", false).expect("seed write");

    // export.go:434, the ToJSON guard (`pass force`, not `--force`).
    let err = to_json_at(&graph_of_size(2), &comm, &path, "c2", false).expect_err("shrink");
    assert_eq!(
        err.to_string(),
        "refusing to overwrite graph.json with a smaller graph: existing has 5 nodes, new has 2 (net -3); pass force to override"
    );

    let bad = dir.write("bad.json", b"{ not json");
    let err = to_json_at(&graph_of_size(2), &comm, &bad, "c3", false).expect_err("unverifiable");
    let text = err.to_string();
    let want_head = format!(
        "refusing to overwrite unparseable graph.json {} to verify the new graph is not smaller (",
        bad.display()
    );
    assert!(text.starts_with(&want_head), "unverifiable text: {text}");
    assert!(
        text.ends_with("); pass force to override"),
        "unverifiable text: {text}"
    );

    // export.go:370, :397, :407: the CheckShrink texts (`pass --force`).
    let n_a = node("a", "pkg/a.go");
    let n_b = node("b", "pkg/b.go");
    let mut dirs = Vec::new();
    let old = seed(&mut dirs, &[&n_a, &n_b]);
    let root = TempDir::new();
    let err = check(&old, &graph_with(&[&n_a]), &["pkg/b.go"], &root, false).expect_err("skipped");
    assert_eq!(
        err.to_string(),
        "refusing to overwrite graph.json with a smaller graph: existing has 2 nodes, new has 1 (net -1); a lost node belongs to pkg/b.go, which failed to process this run; pass --force to override"
    );
    let excluded = TempDir::new();
    excluded.write("pkg/b.go", b"x");
    let err = check(&old, &graph_with(&[&n_a]), &[], &excluded, false).expect_err("excluded");
    assert_eq!(
        err.to_string(),
        "refusing to overwrite graph.json with a smaller graph: existing has 2 nodes, new has 1 (net -1); a lost node belongs to pkg/b.go, which still exists on disk but left the corpus (excluded, not deleted); pass --force to override"
    );
    let err = check(&bad, &graph_with(&[&n_a]), &[], &root, false).expect_err("unverifiable");
    let text = err.to_string();
    assert!(text.starts_with(&want_head), "unverifiable text: {text}");
    assert!(
        text.ends_with("); pass --force to override"),
        "unverifiable text: {text}"
    );
}

// Review Focus 4. x/text NFKD (Unicode 15) is stream-safe: after 30 non-starters (ccc != 0) it
// inserts U+034F COMBINING GRAPHEME JOINER before the 31st, and the buffer is flushed there
// (`composition.go` `streamSafe.next`, `decomposeSegment`). `norm_label` = NFKD, drop
// `unicode.Mn` (Unicode 17), `strings.ToLower`.
//
// U+034F has ccc 0 and general category Mn, so `norm_label` always drops it. Dropping it hides
// the CGJ itself, so the rule only shows in a result when it changes how the marks around it
// are ordered: canonical ordering sorts non-starters by ccc inside one buffer, and the flush at
// the CGJ ends the buffer. The marks must survive the Mn drop, so they must not be Mn.
// Expected values below are derived by hand from these code points (UnicodeData.txt):
//   U+0301 COMBINING ACUTE ACCENT          Mn, ccc 230
//   U+302E HANGUL SINGLE DOT TONE MARK     Mc, ccc 224   (not dropped)
//   U+1D165 MUSICAL SYMBOL COMBINING STEM  Mc, ccc 216   (not dropped)
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn norm_label_cgj_run() {
    // 1. "e" + 31 x U+0301. NFKD gives "e" + 30 x U+0301 + U+034F + U+0301; every mark is Mn
    //    (U+034F too), so only "e" is left. The CGJ does not change this result: this case
    //    holds with or without the stream-safe rule (it pins that the CGJ is dropped, not kept).
    let label = format!("e{}", "\u{301}".repeat(31));
    assert_eq!(norm_label(&label), "e", "31 x U+0301");

    // 2. "E" + 30 x U+302E + U+1D165: the 31st non-starter has the lower ccc (216 < 224).
    //    With the stream-safe rule NFKD is "E" + 30 x U+302E + U+034F + U+1D165: the CGJ
    //    (ccc 0) ends the buffer, so U+1D165 stays after the 30 marks. Without it canonical
    //    ordering would move U+1D165 in front of them ("E" + U+1D165 + 30 x U+302E). The Mc
    //    marks are kept, the CGJ is dropped, and ToLower turns "E" into "e".
    let label = format!("E{}\u{1D165}", "\u{302E}".repeat(30));
    let want = format!("e{}\u{1D165}", "\u{302E}".repeat(30));
    assert_eq!(norm_label(&label), want, "30 x U+302E then U+1D165");
    let naive = format!("e\u{1D165}{}", "\u{302E}".repeat(30));
    assert_ne!(
        norm_label(&label),
        naive,
        "the stream-safe rule must not be skipped"
    );

    // 3. 29 x U+302E + U+1D165 is a run of 30: no CGJ, so canonical ordering does move the
    //    lower-ccc mark to the front.
    let label = format!("e{}\u{1D165}", "\u{302E}".repeat(29));
    let want = format!("e\u{1D165}{}", "\u{302E}".repeat(29));
    assert_eq!(norm_label(&label), want, "29 x U+302E then U+1D165");
}

// Go `toJSONLink` (export.go:315) writes `e.Loc()`, the location without the type-only and
// deferred flag bits (bits 30 and 29), never the raw `e.SourceLocation`.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn link_source_location_strips_flag_bits() {
    let mut e = Edge {
        source: "a".to_string(),
        target: "b".to_string(),
        relation: Relation::CALLS,
        confidence: Confidence::EXTRACTED,
        source_location: line(12),
        ..Edge::default()
    };
    e.set_type_only(true);
    e.set_deferred(true);
    assert_ne!(
        e.source_location,
        line(12),
        "the flag bits are set on the raw field"
    );
    assert_eq!(to_json_link(&e).source_location, line(12));

    let mut g = Graph::new();
    g.add_node(node("a", "pkg/a.go"));
    g.add_node(node("b", "pkg/b.go"));
    g.add_edge(e);
    let dir = TempDir::new();
    let path = dir.join("graph.json");
    to_json_at(&g, &BTreeMap::new(), &path, "c1", false).expect("ToJSON");
    let data = fs::read(&path).expect("read");
    let text = String::from_utf8(data.clone()).expect("graph.json is UTF-8");
    assert!(
        text.contains("\"source_location\": \"L12\""),
        "graph.json link has no source_location L12:\n{text}"
    );
    let out = unmarshal::<JsonGraph>(&data).expect("unmarshal");
    let links = out.links.unwrap_or_default();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].source_location, line(12));
}
