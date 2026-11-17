//! Port of graphify-go `cmd/graphify/explain_path_test.go`: the 6 tests that change the working
//! directory run the binary there (`TestExplainLinesCapsAndGroups` and
//! `TestExplainLinesShortListUngrouped` take `[]query.Neighbor`, which main does not stub yet:
//! they land with the query tests PR, graphify-rs-ihc.13). Then upstream graphify v0.9.107
//! `test_path_cli.py` cases that hold for Go (4 of 22).

mod common;

use std::path::Path;

use common::{Run, TempDir, run_in, s, write};

fn graph(dir: &TempDir, rel: &str, body: &str) {
    write(&dir.join(rel), body);
}

fn ok(r: &Run, what: &str) {
    assert_eq!(
        r.code,
        Some(0),
        "{what}: want success, stderr {:?}",
        r.stderr
    );
}

fn fails(r: &Run, what: &str) {
    assert_ne!(
        r.code,
        Some(0),
        "{what}: want an error, stdout {:?}",
        r.stdout
    );
}

fn go(cwd: &Path, args: &[&str]) -> Run {
    run_in(cwd, args, &[])
}

// --graph makes explain and path load an alternate graph.json (whose nodes exist only there),
// while the default graphify-out/graph.json backs the commands without --graph.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_cmd_explain_path_graph_override() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        r#"{"nodes":[{"id":"alpha","label":"alpha"}],"links":[]}"#,
    );
    graph(
        &dir,
        "graphify-out/alt.json",
        r#"{"nodes":[{"id":"beta","label":"beta"},{"id":"gamma","label":"gamma"}],"links":[{"source":"beta","target":"gamma","relation":"calls"}]}"#,
    );
    let cwd = dir.path();
    let alt = cwd.join("graphify-out/alt.json");
    let alt_eq = format!("--graph={}", s(&alt));

    ok(
        &go(cwd, &["explain", "beta", "--graph", s(&alt)]),
        "explain beta --graph alt",
    );
    ok(
        &go(cwd, &["explain", "beta", &alt_eq]),
        "explain beta --graph=alt",
    );
    fails(
        &go(cwd, &["explain", "alpha", "--graph", s(&alt)]),
        "explain alpha --graph alt",
    );
    ok(
        &go(cwd, &["path", "beta", "gamma", "--graph", s(&alt)]),
        "path beta gamma --graph alt",
    );
    ok(
        &go(cwd, &["explain", "alpha"]),
        "explain alpha (default graph)",
    );
    fails(
        &go(cwd, &["explain", "beta"]),
        "explain beta (default graph)",
    );
}

// The --graph override on explain and path is contained: a path escaping graphify-out is
// rejected.
#[test]
fn test_cmd_explain_path_graph_traversal() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        r#"{"nodes":[{"id":"alpha","label":"alpha"}],"links":[]}"#,
    );
    let secret = dir.write("secrets.json", r#"{"nodes":[],"links":[]}"#);
    let cwd = dir.path();
    let in_cwd = cwd.join("secrets.json");
    for p in [s(&secret), "../secrets.json", s(&in_cwd)] {
        fails(
            &go(cwd, &["explain", "alpha", "--graph", p]),
            &format!("explain --graph {p}"),
        );
        fails(
            &go(cwd, &["path", "alpha", "alpha", "--graph", p]),
            &format!("path --graph {p}"),
        );
    }
}

// path follows edge direction by default and only traverses backwards with --undirected,
// pointing the user at the flag.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_cmd_path_undirected() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        r#"{"nodes":[{"id":"beta","label":"beta"},{"id":"gamma","label":"gamma"}],"links":[{"source":"beta","target":"gamma","relation":"calls"}]}"#,
    );
    let r = go(dir.path(), &["path", "gamma", "beta"]);
    assert!(
        r.code != Some(0) && r.stderr.contains("--undirected"),
        "path gamma beta: stderr {:?}, want no-directed-path error hinting --undirected",
        r.stderr
    );
    ok(
        &go(dir.path(), &["path", "gamma", "beta", "--undirected"]),
        "path gamma beta --undirected",
    );
}

// path prints each hop's relation and confidence, and flips the arrow when the stored edge runs
// against the direction of travel.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_cmd_path_annotates_hops() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        concat!(
            r#"{"nodes":[{"id":"a","label":"a()"},{"id":"b","label":"b()"},{"id":"c","label":"c()"}],"#,
            r#""links":[{"source":"a","target":"b","relation":"calls","confidence":"INFERRED"},"#,
            r#"{"source":"c","target":"b","relation":"imports","confidence":"EXTRACTED"}]}"#,
        ),
    );
    let r = go(dir.path(), &["path", "a()", "c()", "--undirected"]);
    ok(&r, "path a c --undirected");
    assert_eq!(
        r.stdout,
        "a() --calls [INFERRED]--> b() <--imports [EXTRACTED]-- c()\n"
    );
}

// explain and path accept a repo-relative file path, with or without a leading "./".
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_cmd_explain_path_source_file() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        concat!(
            r#"{"nodes":["#,
            r#"{"id":"pkg_a_py","label":"a.py","source_file":"pkg/a.py"},"#,
            r#"{"id":"pkg_b_py","label":"b.py","source_file":"pkg/b.py"},"#,
            r#"{"id":"pkg_b_py_foo","label":"foo()","source_file":"pkg/b.py"},"#,
            r#"{"id":"o_b_py","label":"b.py","source_file":"other/pkg/b.py"},"#,
            r#"{"id":"top_py","label":"top.py","source_file":"top.py"},"#,
            r#"{"id":"sub_top_py","label":"top.py","source_file":"sub/top.py"}],"#,
            r#""links":[{"source":"pkg_a_py","target":"pkg_b_py","relation":"imports"}]}"#,
        ),
    );
    for p in ["pkg/b.py", "./pkg/b.py", "./top.py", "sub/top.py"] {
        ok(&go(dir.path(), &["explain", p]), &format!("explain {p}"));
    }
    ok(
        &go(dir.path(), &["path", "pkg/a.py", "./pkg/b.py"]),
        "path by file path",
    );
}

// A directed path may step from a symbol back out to its containing file over a contains edge.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_cmd_path_directed_contains_reverse() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        concat!(
            r#"{"nodes":[{"id":"run","label":"run()"},{"id":"helper","label":"helper()"},{"id":"b","label":"b.py","source_file":"b.py"}],"#,
            r#""links":[{"source":"run","target":"helper","relation":"calls","confidence":"EXTRACTED"},"#,
            r#"{"source":"b","target":"helper","relation":"contains","confidence":"EXTRACTED"}]}"#,
        ),
    );
    let r = go(dir.path(), &["path", "run()", "b.py"]);
    ok(&r, "path run() b.py");
    assert_eq!(
        r.stdout,
        "run() --calls [EXTRACTED]--> helper() <--contains [EXTRACTED]-- b.py\n"
    );
}

// Upstream `test_path_cli.py` `_write_graph`, at graphify-out/graph.json (Go's --graph must stay
// inside graphify-out, so the upstream `--graph <tmp>/graph.json` is the default graph).
const PATCH_GRAPH: &str = concat!(
    r#"{"directed": false, "multigraph": false, "graph": {}, "nodes": ["#,
    r#"{"id": "create_patch", "label": "createPatchHandler()", "source_file": "server/create-patch-handler.ts", "community": 0},"#,
    r#"{"id": "validate", "label": "validateSanitySession()", "source_file": "server/sanity-validate-session.ts", "community": 0}],"#,
    r#""links": [{"source": "create_patch", "target": "validate", "relation": "calls", "confidence": "EXTRACTED"}]}"#,
);

// pins Go v0.12.0 (the chain line only); upstream 0.9.107 also prints `Shortest path (1 hops):`
// -> P3
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_forward_arrow() {
    let dir = TempDir::new();
    graph(&dir, "graphify-out/graph.json", PATCH_GRAPH);
    let r = go(
        dir.path(),
        &["path", "createPatchHandler", "validateSanitySession"],
    );
    ok(&r, "path");
    assert_eq!(
        r.stdout,
        "createPatchHandler() --calls [EXTRACTED]--> validateSanitySession()\n"
    );
}

// pins Go v0.12.0 (the chain line only); upstream 0.9.107 also prints `Shortest path (1 hops):`
// -> P3
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_reverse_arrow() {
    let dir = TempDir::new();
    graph(&dir, "graphify-out/graph.json", PATCH_GRAPH);
    let r = go(
        dir.path(),
        &[
            "path",
            "validateSanitySession",
            "createPatchHandler",
            "--undirected",
        ],
    );
    ok(&r, "path --undirected");
    assert_eq!(
        r.stdout,
        "validateSanitySession() <--calls [EXTRACTED]-- createPatchHandler()\n"
    );
}

// Upstream varies PYTHONHASHSEED over 8 runs; Go's BFS visits neighbours in sorted order, so 8
// runs of the binary give one route over the diamond.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_path_deterministic_across_hash_seeds() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        concat!(
            r#"{"directed": false, "multigraph": false, "graph": {}, "nodes": ["#,
            r#"{"id": "a", "label": "Alpha", "source_file": "a.py"},"#,
            r#"{"id": "p", "label": "Pmid", "source_file": "p.py"},"#,
            r#"{"id": "q", "label": "Qmid", "source_file": "q.py"},"#,
            r#"{"id": "b", "label": "Beta", "source_file": "b.py"}],"#,
            r#""links": [{"source": "a", "target": "p", "relation": "calls", "confidence": "EXTRACTED"},"#,
            r#"{"source": "p", "target": "b", "relation": "calls", "confidence": "EXTRACTED"},"#,
            r#"{"source": "a", "target": "q", "relation": "calls", "confidence": "EXTRACTED"},"#,
            r#"{"source": "q", "target": "b", "relation": "calls", "confidence": "EXTRACTED"}]}"#,
        ),
    );
    let routes: std::collections::BTreeSet<String> = (0..8)
        .map(|_| {
            let r = go(dir.path(), &["path", "Alpha", "Beta"]);
            ok(&r, "path Alpha Beta");
            r.stdout
        })
        .collect();
    assert_eq!(routes.len(), 1, "routes {routes:?}");
}

// A file-to-file dependency that only closes through a contained symbol still resolves.
// pins Go v0.12.0 (the chain line only); upstream 0.9.107 also prints `Shortest path (2 hops):`
// -> P3
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_path_routes_through_a_contains_edge_to_reach_the_file() {
    let dir = TempDir::new();
    graph(
        &dir,
        "graphify-out/graph.json",
        concat!(
            r#"{"directed": false, "multigraph": false, "graph": {}, "nodes": ["#,
            r#"{"id": "a", "label": "a.py", "source_file": "a.py"},"#,
            r#"{"id": "b", "label": "b.py", "source_file": "b.py"},"#,
            r#"{"id": "helper", "label": "helper()", "source_file": "b.py"}],"#,
            r#""links": [{"source": "a", "target": "helper", "relation": "imports", "confidence": "EXTRACTED"},"#,
            r#"{"source": "b", "target": "helper", "relation": "contains", "confidence": "EXTRACTED"}]}"#,
        ),
    );
    let r = go(dir.path(), &["path", "a.py", "b.py"]);
    ok(&r, "path a.py b.py");
    assert_eq!(
        r.stdout,
        "a.py --imports [EXTRACTED]--> helper() <--contains [EXTRACTED]-- b.py\n"
    );
}
