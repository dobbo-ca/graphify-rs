//! Upstream graphify v0.9.107 cases of the command line that hold for graphify-go v0.12.0 and run
//! the binary as upstream does (`python -m graphify <args>` with a working directory):
//! `test_cargo_missing_manifest.py` (2 of 2), `test_cli_export.py` (8 of 63),
//! `test_update_incremental.py` (2 of 17: the `_cli` cases; the rest are unit tests in
//! `src/cli/tests/upstream_update.rs`) and `tests/test_no_cluster_run_only_keys.py` (1 of 3).
//! Upstream `extract . --code-only` is Go's `build .`.

mod common;

use std::path::Path;

use common::{TempDir, ok_in, read, run_in};

/// The (id, label, source_file) nodes and (source, target, relation) links of a graph.json,
/// sorted (upstream `_graph_shape`).
fn graph_shape(root: &Path) -> (Vec<[String; 3]>, Vec<[String; 3]>) {
    let data = std::fs::read(root.join("graphify-out/graph.json")).expect("read graph.json");
    let v: gostd::json::Value = gostd::json::unmarshal(&data).expect("unmarshal graph.json");
    let gostd::json::Value::Object(g) = &v else {
        panic!("graph.json is not an object");
    };
    let rows = |key: &str, fields: [&str; 3]| -> Vec<[String; 3]> {
        let Some(gostd::json::Value::Array(a)) = g.get(key) else {
            return Vec::new();
        };
        let mut out: Vec<[String; 3]> = a
            .iter()
            .filter_map(|x| match x {
                gostd::json::Value::Object(o) => Some(fields.map(|f| match o.get(f) {
                    Some(gostd::json::Value::String(s)) => s.clone(),
                    _ => String::new(),
                })),
                _ => None,
            })
            .collect();
        out.sort();
        out
    };
    (
        rows("nodes", ["id", "label", "source_file"]),
        rows("links", ["source", "target", "relation"]),
    )
}

/// The top-level keys of graph.json and whether every node's `community` is null.
fn keys_and_null_communities(root: &Path) -> (Vec<String>, bool) {
    let data = std::fs::read(root.join("graphify-out/graph.json")).expect("read graph.json");
    let v: gostd::json::Value = gostd::json::unmarshal(&data).expect("unmarshal graph.json");
    let gostd::json::Value::Object(g) = &v else {
        panic!("graph.json is not an object");
    };
    let all_null = match g.get("nodes") {
        Some(gostd::json::Value::Array(a)) => a.iter().all(|n| match n {
            gostd::json::Value::Object(o) => {
                matches!(o.get("community"), Some(gostd::json::Value::Null))
            }
            _ => false,
        }),
        _ => false,
    };
    (g.keys().cloned().collect(), all_null)
}

// --- test_cargo_missing_manifest.py ---

// pins Go v0.12.0 (`build --cargo` without a root Cargo.toml fails: `graphify: open Cargo.toml:
// no such file or directory`, exit 1, no graph.json); upstream 0.9.107 reports the skip and still
// writes the graph -> P3
#[test]
fn test_cargo_flag_without_a_manifest_still_writes_the_graph() {
    let repo = TempDir::new();
    repo.write("app.py", "def hello():\n    return 1\n");
    let r = run_in(repo.path(), &["build", ".", "--cargo"], &[]);
    assert_eq!(r.code, Some(1), "stdout {:?}", r.stdout);
    assert_eq!(
        r.stderr,
        "graphify: open Cargo.toml: no such file or directory\n"
    );
    assert!(
        !repo.join("graphify-out/graph.json").exists(),
        "graph.json written after the cargo error"
    );
}

#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_cargo_flag_with_a_manifest_still_adds_crate_nodes() {
    let repo = TempDir::new();
    repo.write("app.py", "def hello():\n    return 1\n");
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    ok_in(repo.path(), &["build", ".", "--cargo"], &[]);
    let (nodes, _) = graph_shape(repo.path());
    assert!(
        nodes.iter().any(|n| n[0].contains("app")),
        "a real Cargo.toml should still contribute a crate node; got {nodes:?}"
    );
}

// --- test_cli_export.py ---

/// A small graph at graphify-out/graph.json (upstream builds its fixture with the Python
/// pipeline; the commands below only need a loadable graph with a `Transformer` node).
const SMALL_GRAPH: &str = r#"{"directed":false,"multigraph":false,"graph":{},"nodes":[{"id":"transformer","label":"Transformer","file_type":"code","source_file":"model.py","source_location":"L1","community":0,"norm_label":"transformer"},{"id":"attention","label":"Attention","file_type":"code","source_file":"model.py","source_location":"L9","community":0,"norm_label":"attention"}],"links":[{"source":"transformer","target":"attention","relation":"calls","confidence":"EXTRACTED"}]}"#;

#[test]
fn test_export_graphml_creates_file() {
    let dir = TempDir::new();
    dir.write("graphify-out/graph.json", SMALL_GRAPH);
    ok_in(dir.path(), &["export", "graphml"], &[]);
    let gml = read(&dir.join("graphify-out/graph.graphml"));
    assert!(gml.contains("<graphml"), "{gml}");
}

#[test]
fn test_missing_graph_fails() {
    // test_query_missing_graph_fails, test_path_missing_graph_fails,
    // test_explain_missing_graph_fails
    const CASES: [&[&str]; 3] = [
        &["query", "anything"],
        &["path", "a", "b"],
        &["explain", "anything"],
    ];
    for a in CASES {
        let dir = TempDir::new();
        assert_ne!(
            run_in(dir.path(), a, &[]).code,
            Some(0),
            "{a:?} without a graph"
        );
    }
}

#[test]
fn test_uses_graphify_out_env() {
    // test_query_uses_graphify_out_env, test_explain_uses_graphify_out_env: the graph lives in
    // `custom-graph`, named by a relative GRAPHIFY_OUT.
    for a in [&["query", "Transformer"][..], &["explain", "Transformer"]] {
        let dir = TempDir::new();
        dir.write("custom-graph/graph.json", SMALL_GRAPH);
        let r = ok_in(dir.path(), a, &[("GRAPHIFY_OUT", "custom-graph")]);
        assert!(!r.stdout.is_empty(), "{a:?}: no output");
    }
}

#[test]
fn test_export_unknown_format_fails() {
    let dir = TempDir::new();
    assert_ne!(run_in(dir.path(), &["export", "pdf"], &[]).code, Some(0));
}

// pins Go v0.12.0 (every node has `"community": null`); upstream 0.9.107 omits the key -> P3
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_update_no_cluster_writes_raw_graph() {
    let dir = TempDir::new();
    dir.write("sample.py", "def f():\n    return 1\n");
    ok_in(dir.path(), &["update", ".", "--no-cluster"], &[]);
    let (keys, all_null) = keys_and_null_communities(dir.path());
    assert!(
        keys.iter().any(|k| k == "nodes") && keys.iter().any(|k| k == "links"),
        "{keys:?}"
    );
    assert!(all_null, "a node has a community");
}

// --- test_update_incremental.py (the `_cli` cases) ---

fn corpus(root: &Path) {
    for name in ["a", "b", "c"] {
        common::write(
            &root.join(format!("src/{name}.py")),
            &format!("def f{name}():\n    return 1\n"),
        );
    }
    common::write(
        &root.join("src/uses.py"),
        "from src.a import fa\n\ndef use():\n    return fa()\n",
    );
}

// pins Go v0.12.0 (the first update prints `(4 reparsed, 0 reused, 0 removed)`, the second
// `(1 reparsed, 3 reused, 0 removed)`); upstream 0.9.107 prints `Incremental: re-extracting 1 of
// 4 scanned files` -> P3
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_cli_update_re_extracts_only_changed_files() {
    let tmp = TempDir::new();
    let root = tmp.join("tree");
    corpus(&root);
    let first = ok_in(&root, &["update", "."], &[]);
    assert!(
        first.stdout.contains("(4 reparsed, 0 reused, 0 removed)"),
        "{:?}",
        first.stdout
    );

    common::write(&root.join("src/a.py"), "def renamed_fa():\n    return 11\n");
    let second = ok_in(&root, &["update", "."], &[]);
    assert!(
        second.stdout.contains("(1 reparsed, 3 reused, 0 removed)"),
        "{:?}",
        second.stdout
    );

    // And the result is the graph a full rebuild of the same tree produces.
    let other = tmp.join("other");
    corpus(&other);
    common::write(
        &other.join("src/a.py"),
        "def renamed_fa():\n    return 11\n",
    );
    ok_in(&other, &["update", "."], &[]);
    assert_eq!(graph_shape(&root), graph_shape(&other));
}

// pins Go v0.12.0 (`(4 reparsed, 0 reused, 0 removed)`); upstream 0.9.107 asserts no
// `Incremental:` line -> P3
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_cli_update_force_keeps_the_full_path() {
    let tmp = TempDir::new();
    let root = tmp.join("tree");
    corpus(&root);
    ok_in(&root, &["update", "."], &[]);
    let forced = ok_in(&root, &["update", ".", "--force"], &[]);
    assert!(
        forced.stdout.contains("(4 reparsed, 0 reused, 0 removed)"),
        "{:?}",
        forced.stdout
    );
}

// --- tests/test_no_cluster_run_only_keys.py ---

// Go never writes upstream's run-only keys (`extracted_sources`, `failed_sources`) nor the
// checkout path into graph.json; a first `update . --no-cluster` has the keys of a forced
// rebuild.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_update_no_cluster_first_build_matches_a_rebuild() {
    let tmp = TempDir::new();
    let root = tmp.join("alice_home/proj");
    common::write(
        &root.join("app.py"),
        "import os\n\n\ndef home():\n    return os.getcwd()\n",
    );
    common::write(
        &root.join("util.py"),
        "from app import home\n\n\ndef where():\n    return home()\n",
    );
    let run = |extra: &[&str]| {
        let mut a = vec!["update", ".", "--no-cluster"];
        a.extend_from_slice(extra);
        ok_in(&root, &a, &[]);
        let text = read(&root.join("graphify-out/graph.json"));
        assert!(
            !text.contains("alice_home"),
            "graph.json embeds the checkout path"
        );
        keys_and_null_communities(&root).0
    };
    let first = run(&[]);
    for k in ["extracted_sources", "failed_sources"] {
        assert!(
            !first.iter().any(|f| f == k),
            "graph.json has the run-only key {k}"
        );
    }
    let rebuilt = run(&["--force"]);
    assert_eq!(first, rebuilt);
}
