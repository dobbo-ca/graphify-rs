//! Port of graphify-go `cmd/graphify/godnodes_test.go` (3 tests; they change the working
//! directory, so they run the binary there), then upstream graphify v0.9.107
//! `tests/test_god_nodes_cli.py` (4 of 5; Go has no `god_nodes` spelling).

mod common;

use common::{Run, TempDir, run_in, s, write};

/// Go `godTestGraph`: hub is connected to three leaves, spoke to one, so hub outranks spoke;
/// file.go is a file node and must be excluded from the ranking.
const GOD_TEST_GRAPH: &str = r#"{"nodes":[
  {"id":"hub","label":"hub","file_type":"go","source_file":"file.go","source_location":"L1"},
  {"id":"spoke","label":"spoke","file_type":"go","source_file":"file.go","source_location":"L2"},
  {"id":"a","label":"a","file_type":"go","source_file":"file.go","source_location":"L3"},
  {"id":"b","label":"b","file_type":"go","source_file":"file.go","source_location":"L4"},
  {"id":"file.go","label":"file.go","file_type":"go","source_file":"file.go","source_location":"file"}],
 "links":[
  {"source":"hub","target":"spoke","relation":"calls"},
  {"source":"hub","target":"a","relation":"calls"},
  {"source":"hub","target":"b","relation":"calls"},
  {"source":"file.go","target":"hub","relation":"contains"},
  {"source":"file.go","target":"spoke","relation":"contains"},
  {"source":"file.go","target":"a","relation":"contains"},
  {"source":"file.go","target":"b","relation":"contains"}]}"#;

fn god(dir: &TempDir, extra: &[&str]) -> Run {
    let mut a = vec!["god-nodes"];
    a.extend_from_slice(extra);
    run_in(dir.path(), &a, &[])
}

/// The `[{"id", "label", "degree"}]` of `god-nodes --json`.
fn gods(out: &str) -> Vec<(String, String, f64)> {
    let v: gostd::json::Value =
        gostd::json::unmarshal(out.as_bytes()).unwrap_or_else(|e| panic!("{out:?}: {e}"));
    let gostd::json::Value::Array(a) = &v else {
        panic!("god-nodes --json output {out:?} is not an array");
    };
    a.iter()
        .map(|g| {
            let gostd::json::Value::Object(o) = g else {
                panic!("{out:?}: not an object");
            };
            let st = |k: &str| match o.get(k) {
                Some(gostd::json::Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let deg = match o.get("degree") {
                Some(gostd::json::Value::Number(n)) => *n,
                _ => -1.0,
            };
            (st("id"), st("label"), deg)
        })
        .collect()
}

#[test]
fn test_cmd_god_nodes() {
    let dir = TempDir::new();
    write(&dir.join("graphify-out/graph.json"), GOD_TEST_GRAPH);

    let r = god(&dir, &[]);
    assert_eq!(r.code, Some(0), "god-nodes: {}", r.stderr);
    assert!(
        r.stdout.contains("hub"),
        "god-nodes output = {:?}, want the hub node",
        r.stdout
    );
    assert!(
        !r.stdout.contains("file.go"),
        "god-nodes output = {:?}, want file hub excluded",
        r.stdout
    );

    // --json emits a parseable array, and --top caps it.
    let r = god(&dir, &["--json", "--top", "1"]);
    assert_eq!(r.code, Some(0), "god-nodes --json --top 1: {}", r.stderr);
    let g = gods(&r.stdout);
    assert!(
        g.len() == 1 && g[0].0 == "hub" && g[0].2 == 4.0,
        "god-nodes --json --top 1 = {g:?}, want one hub node with degree 4"
    );

    // --top=N form is equivalent; bad values and stray args are rejected.
    let r = god(&dir, &["--top=2", "--json"]);
    assert_eq!(r.code, Some(0), "god-nodes --top=2: {}", r.stderr);
    for a in [&["--top", "0"][..], &["--top", "x"], &["bogus"]] {
        assert_ne!(
            god(&dir, a).code,
            Some(0),
            "god-nodes {a:?} = ok, want error"
        );
    }
}

// --graph selects an alternate graph and stays contained to graphify-out.
#[test]
fn test_cmd_god_nodes_graph_override() {
    let dir = TempDir::new();
    write(
        &dir.join("graphify-out/graph.json"),
        r#"{"nodes":[],"links":[]}"#,
    );
    write(&dir.join("graphify-out/alt.json"), GOD_TEST_GRAPH);
    dir.write("secrets.json", r#"{"nodes":[],"links":[]}"#);
    let alt = dir.join("graphify-out/alt.json");
    let r = god(&dir, &["--graph", s(&alt)]);
    assert!(
        r.code == Some(0) && r.stdout.contains("hub"),
        "god-nodes --graph alt = {:?}, {:?}; want the alt graph's hub",
        r.stdout,
        r.stderr
    );
    assert_ne!(
        god(&dir, &["--graph", "../secrets.json"]).code,
        Some(0),
        "god-nodes --graph ../secrets.json = ok, want containment error"
    );
}

// A graph.json may carry any number of distinct confidence values.
#[test]
fn test_cmd_god_nodes_many_confidences() {
    let mut sb = String::from(
        r#"{"nodes":[{"id":"hub","label":"hub","file_type":"go","source_file":"f.go","source_location":"L1"},{"id":"leaf","label":"leaf","file_type":"go","source_file":"f.go","source_location":"L2"}],"links":["#,
    );
    for i in 0..70000 {
        if i > 0 {
            sb.push(',');
        }
        sb.push_str(&format!(
            r#"{{"source":"hub","target":"leaf","relation":"calls","confidence":"c{i}"}}"#
        ));
    }
    sb.push_str("]}");
    let dir = TempDir::new();
    write(&dir.join("graphify-out/graph.json"), &sb);
    let r = god(&dir, &[]);
    assert!(
        r.code == Some(0) && r.stdout.contains("hub"),
        "god-nodes = {:?}, {:?}, want the hub node",
        r.stdout,
        r.stderr
    );
}

/// Upstream `_write_graph`: a hub (`Auth`) called from four files and its file node `auth.py`,
/// at graphify-out/graph.json (Go's --graph must stay inside graphify-out).
const AUTH_GRAPH: &str = concat!(
    r#"{"directed": true, "multigraph": false, "graph": {}, "nodes": ["#,
    r#"{"id": "hub", "label": "Auth", "file_type": "code", "source_file": "auth.py", "source_location": "L1"},"#,
    r#"{"id": "f", "label": "auth.py", "file_type": "code", "source_file": "auth.py", "source_location": null},"#,
    r#"{"id": "caller0", "label": "c0()", "file_type": "code", "source_file": "m0.py", "source_location": "L1"},"#,
    r#"{"id": "caller1", "label": "c1()", "file_type": "code", "source_file": "m1.py", "source_location": "L1"},"#,
    r#"{"id": "caller2", "label": "c2()", "file_type": "code", "source_file": "m2.py", "source_location": "L1"},"#,
    r#"{"id": "caller3", "label": "c3()", "file_type": "code", "source_file": "m3.py", "source_location": "L1"}],"#,
    r#""links": [{"source": "caller0", "target": "hub", "relation": "calls", "confidence": "EXTRACTED"},"#,
    r#"{"source": "caller1", "target": "hub", "relation": "calls", "confidence": "EXTRACTED"},"#,
    r#"{"source": "caller2", "target": "hub", "relation": "calls", "confidence": "EXTRACTED"},"#,
    r#"{"source": "caller3", "target": "hub", "relation": "calls", "confidence": "EXTRACTED"},"#,
    r#"{"source": "f", "target": "hub", "relation": "contains", "confidence": "EXTRACTED"}]}"#,
);

fn auth_dir() -> TempDir {
    let dir = TempDir::new();
    write(&dir.join("graphify-out/graph.json"), AUTH_GRAPH);
    dir
}

#[test]
fn test_god_nodes_cli_text_output() {
    let dir = auth_dir();
    let r = god(&dir, &["--graph", "graphify-out/graph.json"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(
        r.stdout.contains("God nodes (most connected):"),
        "{:?}",
        r.stdout
    );
    assert!(r.stdout.contains("Auth"), "{:?}", r.stdout);
    assert!(r.stdout.contains("edges"), "{:?}", r.stdout);
    assert!(
        !r.stdout.contains("auth.py"),
        "file node ranked: {:?}",
        r.stdout
    );
}

#[test]
fn test_god_nodes_cli_top_limits() {
    let dir = auth_dir();
    let r = god(&dir, &["--graph", "graphify-out/graph.json", "--top", "1"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(r.stdout.matches(" edges").count(), 1, "{:?}", r.stdout);
}

#[test]
fn test_god_nodes_cli_json() {
    let dir = auth_dir();
    let r = god(&dir, &["--graph", "graphify-out/graph.json", "--json"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let g = gods(&r.stdout);
    assert!(!g.is_empty(), "{:?}", r.stdout);
    assert_eq!(g[0].1, "Auth");
    // Each element carries the three keys.
    assert!(
        r.stdout.contains("\"id\"")
            && r.stdout.contains("\"label\"")
            && r.stdout.contains("\"degree\""),
        "{:?}",
        r.stdout
    );
}

// Go's text is `graph file not found: <abs path>` (security.ValidateGraphPath), exit 1.
#[test]
fn test_god_nodes_cli_missing_graph_errors() {
    let dir = auth_dir();
    let r = god(&dir, &["--graph", "graphify-out/nope.json"]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("graph file not found"), "{:?}", r.stderr);
}
