//! Port of `merge_test.go` (7 tests).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::query::merge::MERGE_MAX_NODES;
use crate::query::{Graph, load, merge};
use crate::testutil::TempDir;

/// Go `writeFile`: `content` at `<tmp>/g.json`; the temp dir is kept alive by the caller.
fn write_file(content: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new();
    let p = dir.write("g.json", content.as_bytes());
    (dir, p)
}

/// Go `loadMerged`: reads back the JSON Merge wrote to current. Go decodes it with
/// `json.Unmarshal` into a `Graph`; `load` does that plus the size check and the indexes.
fn load_merged(path: &Path) -> Graph {
    load(path).unwrap_or_else(|e| panic!("merged output is not valid graph.json: {e}"))
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_unions_nodes_and_edges() {
    let (_c, current) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"a","label":"a()"},
 {"id":"b","label":"b()"}
],
"links":[
 {"source":"a","target":"b","relation":"calls","confidence":"INFERRED"}
]}"#,
    );
    let (_o, other) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"b","label":"b()"},
 {"id":"c","label":"c()"}
],
"links":[
 {"source":"b","target":"c","relation":"calls","confidence":"INFERRED"}
]}"#,
    );

    merge(&current, &other).unwrap_or_else(|e| panic!("{e}"));
    let g = load_merged(&current);
    assert_eq!(g.nodes.len(), 3, "nodes, want 3 (a, b deduped, c)");
    assert_eq!(g.links.len(), 2, "links, want 2");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_keeps_both_directions_and_relations() {
    // a->b calls and b->a calls are distinct edges; a->b calls and a->b imports are also
    // distinct. None should collapse.
    let (_c, current) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[{"id":"a","label":"a()"},{"id":"b","label":"b()"}],
"links":[{"source":"a","target":"b","relation":"calls","confidence":"INFERRED"}]}"#,
    );
    let (_o, other) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[{"id":"a","label":"a()"},{"id":"b","label":"b()"}],
"links":[
 {"source":"b","target":"a","relation":"calls","confidence":"INFERRED"},
 {"source":"a","target":"b","relation":"imports","confidence":"INFERRED"},
 {"source":"a","target":"b","relation":"calls","confidence":"INFERRED"}
]}"#,
    );

    merge(&current, &other).unwrap_or_else(|e| panic!("{e}"));
    let g = load_merged(&current);
    // a->b calls (deduped), b->a calls, a->b imports = 3.
    assert_eq!(
        g.links.len(),
        3,
        "links, want 3 (direction- and relation-aware dedup)"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_preserves_node_attributes() {
    let (_c, current) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[{"id":"a","label":"a()","community":2,"confidence_score":0.5}],
"links":[]}"#,
    );
    let (_o, other) =
        write_file(r#"{"directed":true,"multigraph":false,"graph":{},"nodes":[],"links":[]}"#);
    merge(&current, &other).unwrap_or_else(|e| panic!("{e}"));
    let data = fs::read_to_string(&current).expect("read merged");
    for want in [r#""community": 2"#, r#""confidence_score": 0.5"#] {
        assert!(
            data.contains(want),
            "merged output lost attribute {want:?}:\n{data}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_preserves_provenance_and_hyperedges() {
    // built_at_commit and hyperedges are top-level fields the canonical writer emits and the
    // spec promises to preserve. They must survive a merge.
    let (_c, current) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[{"id":"a","label":"a()"}],"links":[],
"hyperedges":[{"id":"h1"}],"built_at_commit":"abc123"}"#,
    );
    let (_o, other) = write_file(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[{"id":"b","label":"b()"}],"links":[],
"hyperedges":[],"built_at_commit":"def456"}"#,
    );
    merge(&current, &other).unwrap_or_else(|e| panic!("{e}"));
    let data = fs::read_to_string(&current).expect("read merged");
    for want in [
        r#""hyperedges": ["#,
        r#""id": "h1""#,
        r#""built_at_commit": "abc123""#,
    ] {
        assert!(
            data.contains(want),
            "merged output lost top-level field {want:?}:\n{data}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_rejects_corrupt_input() {
    let (_c, current) = write_file(r#"{"directed":true,"nodes":[],"links":[]}"#);
    let (_o, other) = write_file("{not json");
    assert!(
        merge(&current, &other).is_err(),
        "expected error on corrupt other graph, got nil"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_missing_file() {
    let (_c, current) = write_file(r#"{"directed":true,"nodes":[],"links":[]}"#);
    let missing = TempDir::new();
    assert!(
        merge(&current, &missing.join("nope.json")).is_err(),
        "expected error on missing other graph, got nil"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_merge_rejects_too_many_nodes() {
    let mut b = String::from(r#"{"directed":true,"multigraph":false,"graph":{},"nodes":["#);
    for i in 0..=MERGE_MAX_NODES {
        if i > 0 {
            b.push(',');
        }
        write!(b, r#"{{"id":"n{i}","label":"n"}}"#).expect("write to String");
    }
    b.push_str(r#"],"links":[]}"#);
    let (_c, current) = write_file(&b);
    let (_o, other) = write_file(r#"{"directed":true,"nodes":[],"links":[]}"#);
    match merge(&current, &other) {
        Err(e) if e.to_string().contains("node cap") => {}
        r => panic!("expected node-cap error, got {r:?}"),
    }
}
