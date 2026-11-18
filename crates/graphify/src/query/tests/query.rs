//! Port of `query_test.go` (14 tests).

use super::load_str;
use crate::query::{Error, Graph, explain, path_edges, query};

const SAMPLE_GRAPH: &str = r#"{
  "directed": false, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"x_a","label":"a()","file_type":"code","source_file":"x.go","source_location":"L1","community":0,"norm_label":"a()"},
    {"id":"x_b","label":"b()","file_type":"code","source_file":"x.go","source_location":"L5","community":0,"norm_label":"b()"},
    {"id":"y_c","label":"c()","file_type":"code","source_file":"y.go","source_location":"L1","community":1,"norm_label":"c()"}
  ],
  "links": [
    {"source":"x_a","target":"x_b","relation":"calls","confidence":"INFERRED"},
    {"source":"x_b","target":"y_c","relation":"calls","confidence":"INFERRED"}
  ]
}"#;

/// Go `loadSample`.
fn load_sample() -> Graph {
    load_str(SAMPLE_GRAPH)
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_query_matches() {
    let g = load_sample();
    let m = query(&g, "a").unwrap_or_else(|e| panic!("{e}"));
    assert!(
        m.len() == 1 && m[0].label == "a()",
        "query a = {m:?}, want single a()"
    );
    assert_eq!(m[0].location, "x.go:1", "location");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_explain_neighbors() {
    let g = load_sample();
    let ex = explain(&g, "b()").unwrap_or_else(|e| panic!("{e}"));
    // b is called by a (incoming) and calls c (outgoing).
    let in_a = ex
        .neighbors
        .iter()
        .any(|n| n.label == "a()" && n.direction == "<-");
    let out_c = ex
        .neighbors
        .iter()
        .any(|n| n.label == "c()" && n.direction == "->");
    assert!(in_a && out_c, "neighbors wrong: {:?}", ex.neighbors);
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_path_edges() {
    let g = load_sample();
    let res = path_edges(&g, "a()", "c()", 8, false).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        res.nodes.len() == 3 && res.edges.len() == 2,
        "got {} nodes / {} edges, want 3 / 2",
        res.nodes.len(),
        res.edges.len()
    );
    for (i, e) in res.edges.iter().enumerate() {
        assert!(
            e.relation == "calls" && e.confidence == "INFERRED" && e.forward,
            "edge {i} = {e:?}, want forward calls/INFERRED"
        );
    }
    // Reversed query orients both hops backwards against the stored edges.
    let rev = path_edges(&g, "c()", "a()", 8, true).unwrap_or_else(|e| panic!("{e}"));
    for (i, e) in rev.edges.iter().enumerate() {
        assert!(!e.forward, "edge {i} should be backward: {e:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_path_directed_by_default() {
    let g = load_sample();
    // c -> a only exists against the direction of the stored call edges.
    let r = path_edges(&g, "c()", "a()", 8, false);
    assert!(
        matches!(r, Err(Error::NoDirectedPath(_))),
        "err = {r:?}, want ErrNoDirectedPath"
    );
    let res = path_edges(&g, "c()", "a()", 8, true).unwrap_or_else(|e| panic!("undirected: {e}"));
    assert!(
        res.nodes.len() == 3 && res.nodes[0].label == "c()" && res.nodes[2].label == "a()",
        "undirected path = {:?}, want c->b->a",
        res.nodes
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_path_edges_same_node() {
    let g = load_sample();
    match path_edges(&g, "a()", "a()", 8, false) {
        Err(Error::SameNode(same)) => assert_eq!(same.id, "x_a", "resolved ID"),
        r => panic!("err = {r:?}, want SameNodeError"),
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_path_edges_max_hops() {
    let g = load_sample();
    match path_edges(&g, "a()", "c()", 1, false) {
        Err(Error::MaxHops(over)) => assert!(
            over.max_hops == 1 && over.hops == 2,
            "got max={} hops={}, want max=1 hops=2",
            over.max_hops,
            over.hops
        ),
        r => panic!("err = {r:?}, want MaxHopsError"),
    }
}

const EDGE_LOC_GRAPH: &str = r#"{
  "directed": false, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"a_caller","label":"zorkcaller()","file_type":"code","source_file":"a.go","source_location":"L3","community":0,"norm_label":"zorkcaller()"},
    {"id":"a_target","label":"zorktarget()","file_type":"code","source_file":"a.go","source_location":"L12","community":0,"norm_label":"zorktarget()"},
    {"id":"a_other","label":"zorkother()","file_type":"code","source_file":"a.go","source_location":"L20","community":0,"norm_label":"zorkother()"}
  ],
  "links": [
    {"source":"a_caller","target":"a_target","relation":"calls","confidence":"EXTRACTED","source_file":"a.go","source_location":"L8"},
    {"source":"a_other","target":"a_target","relation":"calls","confidence":"INFERRED"}
  ]
}"#;

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_explain_uses_edge_call_site() {
    let g = load_str(EDGE_LOC_GRAPH);
    let ex = explain(&g, "zorktarget()").unwrap_or_else(|e| panic!("{e}"));
    let loc_of = |label: &str| {
        ex.neighbors
            .iter()
            .rfind(|n| n.label == label)
            .map_or("", |n| n.location.as_str())
    };
    // Edge carries the call site: cite it, not the caller's definition line (L3).
    assert_eq!(loc_of("zorkcaller()"), "a.go:8", "caller location");
    // Edge has no location: fall back to the neighbour node's own line.
    assert_eq!(loc_of("zorkother()"), "a.go:20", "fallback location");
}

/// Go `TestExplainNeighborsDegreeOrdered`: Explain returns the most connected neighbours first,
/// so a caller capping the list keeps the hubs.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_explain_neighbors_degree_ordered() {
    let g = load_sample();
    let ex = explain(&g, "b()").unwrap_or_else(|e| panic!("{e}"));
    for w in ex.neighbors.windows(2) {
        assert!(
            w[0].degree >= w[1].degree,
            "neighbors not degree-descending: {:?}",
            ex.neighbors
        );
    }
    for n in &ex.neighbors {
        assert_ne!(n.degree, 0, "neighbor {:?} has zero degree", n.label);
    }
}

const DUP_GRAPH: &str = r#"{
  "directed": false, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"p_foo","label":"Foo()","file_type":"code","source_file":"pkg/p.go","source_location":"L3","norm_label":"Foo()"},
    {"id":"q_foo","label":"Foo()","file_type":"code","source_file":"pkg/q.go","source_location":"L9","norm_label":"Foo()"}
  ],
  "links": []
}"#;

/// Go `loadDup`.
fn load_dup() -> Graph {
    load_str(DUP_GRAPH)
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_explain_ambiguous_label() {
    let g = load_dup();
    let err = match explain(&g, "Foo()") {
        Err(e) => e,
        Ok(ex) => panic!("got {ex:?}, want AmbiguousError"),
    };
    let Error::Ambiguous(amb) = &err else {
        panic!("err = {err:?}, want AmbiguousError");
    };
    assert_eq!(amb.candidates.len(), 2, "candidates = {:?}", amb.candidates);
    for want in ["pkg/p.go:3", "pkg/q.go:9"] {
        assert!(
            err.to_string().contains(want),
            "error {:?} missing candidate {want}",
            err.to_string()
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_explain_path_qualifier_selects_one() {
    let g = load_dup();
    let ex = explain(&g, "pkg/q.go::Foo()").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ex.node.id, "q_foo", "node");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_path_edges_ambiguous() {
    let g = load_dup();
    let r = path_edges(&g, "Foo()", "pkg/q.go::Foo()", 0, false);
    assert!(
        matches!(r, Err(Error::Ambiguous(_))),
        "err = {r:?}, want AmbiguousError"
    );
}

const SUFFIX_GRAPH: &str = r#"{
  "directed": false, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"h_new","label":"New()","file_type":"code","source_file":"pkg/x/req_handler.go","source_location":"L3","norm_label":"New()"}
  ],
  "links": []
}"#;

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_resolve_empty_query_no_match() {
    let g = load_sample();
    let r = g.resolve("");
    assert!(
        matches!(r, Ok(None)),
        "resolve(\"\") = {r:?}, want (nil, nil)"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_explain_path_qualifier_needs_boundary() {
    let g = load_str(SUFFIX_GRAPH);
    // "handler.go" is not a path-boundary suffix of "pkg/x/req_handler.go".
    assert!(
        explain(&g, "handler.go::New()").is_err(),
        "err = nil, want no-match error"
    );
    if let Err(e) = explain(&g, "x/req_handler.go::New()") {
        panic!("err = {e}, want match");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_resolve_file_path_tier() {
    let body = r#"{"nodes":[
 {"id":"claude","label":"CLAUDE.md","file_type":"document","source_file":"CLAUDE.md","source_location":"L1"},
 {"id":"claude_claude_md","label":"CLAUDE.md","file_type":"heading","source_file":"CLAUDE.md","source_location":"L1"},
 {"id":"gloss","label":"Business Glossary","file_type":"glossary","source_file":"d/glossary.md","source_location":"L1"},
 {"id":"gloss_h","label":"Business Glossary","file_type":"heading","source_file":"d/glossary.md","source_location":"L8"},
 {"id":"top_py","label":"top.py","file_type":"code","source_file":"top.py","source_location":"L1"},
 {"id":"sub_top_py","label":"top.py","file_type":"code","source_file":"sub/top.py","source_location":"L1"}],"links":[]}"#;
    let g = load_str(body);
    const CASES: &[(&str, &str)] = &[
        ("./CLAUDE.md", "claude"),
        ("CLAUDE.md", "claude"),
        ("d/glossary.md", "gloss"),
        ("top.py", "top_py"),
        ("./top.py", "top_py"),
        ("sub/top.py", "sub_top_py"),
    ];
    for &(q, want) in CASES {
        let r = g.resolve(q);
        assert!(
            matches!(r, Ok(Some(n)) if n.id == want),
            "resolve({q:?}) = {r:?}; want {want}"
        );
    }
}
