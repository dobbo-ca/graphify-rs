//! Upstream graphify v0.9.107 `tests/test_validate.py` (15 of 18) and
//! `tests/test_corrupt_graph_json.py` (2 of 4), as graphify-go v0.12.0 answers them.
//!
//! Upstream's `validate_extraction` checks an extraction (`nodes`, `edges`, the `file_type` and
//! `confidence` enums, required fields); Go's `validate` checks a graph.json (`nodes`, `links`):
//! dangling endpoints, duplicate and empty ids, and the decode error of anything that is not a
//! `query.Graph`. Each row is upstream's input with `edges` renamed `links` (Go's key; an `edges`
//! key is ignored). Rows marked "pins Go v0.12.0" assert Go's value where upstream asserts
//! another. The JSON error texts follow gostd's Go 1.27 rules (see `added.rs`).

use super::write_graph_json;
use crate::query::{load, validate};

/// What Go's `validate` returns for a row: its issues, or its error after `parsing <path>: `.
enum Want {
    Issues(&'static [&'static str]),
    Err(&'static str),
}

/// Upstream `VALID`.
const VALID: &str = r#"{"nodes":[
 {"id":"n1","label":"Foo","file_type":"code","source_file":"foo.py"},
 {"id":"n2","label":"Bar","file_type":"document","source_file":"bar.md"}],
"links":[{"source":"n1","target":"n2","relation":"references","confidence":"EXTRACTED","source_file":"foo.py","weight":1.0}]}"#;

/// `VALID` with `nodes[0].file_type` set to `v`.
macro_rules! file_type {
    ($v:literal) => {
        concat!(
            r#"{"nodes":[
 {"id":"n1","label":"Foo","file_type":"#,
            $v,
            r#","source_file":"foo.py"},
 {"id":"n2","label":"Bar","file_type":"document","source_file":"bar.md"}],
"links":[{"source":"n1","target":"n2","relation":"references","confidence":"EXTRACTED","source_file":"foo.py","weight":1.0}]}"#
        )
    };
}

/// `VALID` with `links[0].confidence` set to `v`.
macro_rules! confidence {
    ($v:literal) => {
        concat!(
            r#"{"nodes":[
 {"id":"n1","label":"Foo","file_type":"code","source_file":"foo.py"},
 {"id":"n2","label":"Bar","file_type":"document","source_file":"bar.md"}],
"links":[{"source":"n1","target":"n2","relation":"references","confidence":"#,
            $v,
            r#","source_file":"foo.py","weight":1.0}]}"#
        )
    };
}

const ROWS: &[(&str, &str, Want)] = &[
    ("test_valid_passes", VALID, Want::Issues(&[])),
    // pins Go v0.12.0 (a decode error: Go's file_type and confidence are strings); upstream
    // 0.9.107 reports "invalid file_type" / "invalid confidence" as an issue -> P3
    (
        "test_structured_enum_values_reported_not_raised[nodes-file_type-[]]",
        file_type!("[]"),
        Want::Err(
            "json: cannot unmarshal array into Go struct field Graph.nodes.0.file_type of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[nodes-file_type-{}]",
        file_type!("{}"),
        Want::Err(
            "json: cannot unmarshal object into Go struct field Graph.nodes.0.file_type of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[nodes-file_type-[code]]",
        file_type!(r#"["code"]"#),
        Want::Err(
            "json: cannot unmarshal array into Go struct field Graph.nodes.0.file_type of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[nodes-file_type-{value:code}]",
        file_type!(r#"{"value":"code"}"#),
        Want::Err(
            "json: cannot unmarshal object into Go struct field Graph.nodes.0.file_type of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[edges-confidence-[]]",
        confidence!("[]"),
        Want::Err(
            "json: cannot unmarshal array into Go struct field Graph.links.0.confidence of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[edges-confidence-{}]",
        confidence!("{}"),
        Want::Err(
            "json: cannot unmarshal object into Go struct field Graph.links.0.confidence of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[edges-confidence-[code]]",
        confidence!(r#"["code"]"#),
        Want::Err(
            "json: cannot unmarshal array into Go struct field Graph.links.0.confidence of type string",
        ),
    ),
    (
        "test_structured_enum_values_reported_not_raised[edges-confidence-{value:code}]",
        confidence!(r#"{"value":"code"}"#),
        Want::Err(
            "json: cannot unmarshal object into Go struct field Graph.links.0.confidence of type string",
        ),
    ),
    // pins Go v0.12.0 (no issue: Go requires no key); upstream 0.9.107 reports the missing
    // "nodes" / "edges" key -> P3
    (
        "test_missing_nodes_key",
        r#"{"links":[]}"#,
        Want::Issues(&[]),
    ),
    (
        "test_missing_edges_key",
        r#"{"nodes":[]}"#,
        Want::Issues(&[]),
    ),
    // pins Go v0.12.0 (a decode error); upstream 0.9.107 returns one error string -> P3
    (
        "test_not_a_dict",
        "[]",
        Want::Err("json: cannot unmarshal array into Go value of type query.Graph"),
    ),
    // pins Go v0.12.0 (no issue: Go checks no enum); upstream 0.9.107 reports the value -> P3
    (
        "test_invalid_file_type",
        r#"{"nodes":[{"id":"n1","label":"X","file_type":"video","source_file":"x.mp4"}],"links":[]}"#,
        Want::Issues(&[]),
    ),
    (
        "test_invalid_confidence",
        r#"{"nodes":[
 {"id":"n1","label":"A","file_type":"code","source_file":"a.py"},
 {"id":"n2","label":"B","file_type":"code","source_file":"b.py"}],
"links":[{"source":"n1","target":"n2","relation":"calls","confidence":"CERTAIN","source_file":"a.py"}]}"#,
        Want::Issues(&[]),
    ),
    (
        "test_dangling_edge_source",
        r#"{"nodes":[{"id":"n1","label":"A","file_type":"code","source_file":"a.py"}],
"links":[{"source":"missing_id","target":"n1","relation":"calls","confidence":"EXTRACTED","source_file":"a.py"}]}"#,
        Want::Issues(&["edge missing_id --calls--> n1: source node missing"]),
    ),
    (
        "test_dangling_edge_target",
        r#"{"nodes":[{"id":"n1","label":"A","file_type":"code","source_file":"a.py"}],
"links":[{"source":"n1","target":"ghost","relation":"calls","confidence":"EXTRACTED","source_file":"a.py"}]}"#,
        Want::Issues(&["edge n1 --calls--> ghost: target node missing"]),
    ),
    // pins Go v0.12.0 (no issue: Go requires no field); upstream 0.9.107 reports file_type -> P3
    (
        "test_missing_node_field",
        r#"{"nodes":[{"id":"n1","label":"A","source_file":"a.py"}],"links":[]}"#,
        Want::Issues(&[]),
    ),
    // Upstream raises; Go fails too, with its decode error (the later "nodes" key wins in the
    // Python dict literal).
    (
        "test_assert_valid_raises_on_errors",
        r#"{"nodes":"bad","links":[],"oops":true}"#,
        Want::Err(
            "json: cannot unmarshal string into Go struct field Graph.nodes of type []query.Node",
        ),
    ),
    (
        "test_assert_valid_passes_silently",
        VALID,
        Want::Issues(&[]),
    ),
    // pins Go v0.12.0 (a decode error: an id is a string); upstream 0.9.107 reports
    // "non-hashable id" as an issue, and still flags the dangling `ghost` edge -> P3
    (
        "test_non_hashable_node_id_reported_not_raised",
        r#"{"nodes":[
 {"id":"n1","label":"A","file_type":"code","source_file":"a.py"},
 {"id":["x","y"],"label":"B","file_type":"code","source_file":"b.py"}],"links":[]}"#,
        Want::Err(
            "json: cannot unmarshal array into Go struct field Graph.nodes.1.id of type string",
        ),
    ),
    (
        "test_non_hashable_edge_endpoint_reported_not_raised",
        r#"{"nodes":[
 {"id":"n1","label":"A","file_type":"code","source_file":"a.py"},
 {"id":"n2","label":"B","file_type":"code","source_file":"b.py"}],
"links":[{"source":"n1","target":["n2","n3"],"relation":"calls","confidence":"INFERRED","source_file":"a.py"}]}"#,
        Want::Err(
            "json: cannot unmarshal array into Go struct field Graph.links.0.target of type string",
        ),
    ),
    (
        "test_non_hashable_node_id_does_not_mask_valid_ids",
        r#"{"nodes":[
 {"id":"n1","label":"A","file_type":"code","source_file":"a.py"},
 {"id":{"oops":1},"label":"B","file_type":"code","source_file":"b.py"}],
"links":[{"source":"n1","target":"ghost","relation":"calls","confidence":"EXTRACTED","source_file":"a.py"}]}"#,
        Want::Err(
            "json: cannot unmarshal object into Go struct field Graph.nodes.1.id of type string",
        ),
    ),
];

/// `tests/test_validate.py`, one row per upstream case (pytest parameters in brackets).
#[test]
fn upstream_test_validate() {
    for (name, body, want) in ROWS {
        let (_d, p) = write_graph_json(body.as_bytes());
        match (validate(&p), want) {
            (Ok((issues, ..)), Want::Issues(w)) => assert_eq!(issues, *w, "{name}: issues"),
            (Err(e), Want::Err(w)) => assert_eq!(
                e.to_string(),
                format!("parsing {}: {w}", p.display()),
                "{name}: error"
            ),
            (Ok(r), Want::Err(w)) => panic!("{name}: validate = {r:?}, want error {w:?}"),
            (Err(e), Want::Issues(w)) => panic!("{name}: validate error {e}, want issues {w:?}"),
        }
    }
}

/// Upstream `_CORRUPT`: truncated mid-object.
const CORRUPT: &str = r#"{"nodes": [{"id": "a", "labe"#;

/// `tests/test_corrupt_graph_json.py` `test_affected_load_graph_corrupt_raises_runtimeerror`:
/// Go's `affected` loads with `load`.
// pins Go v0.12.0 ("parsing <path>: unexpected end of JSON input"); upstream 0.9.107 raises
// "Cannot read graph file ... regenerate" -> P3
#[test]
fn upstream_test_affected_load_graph_corrupt_raises_runtimeerror() {
    let (_d, p) = write_graph_json(CORRUPT.as_bytes());
    match load(&p) {
        Err(e) => assert_eq!(
            e.to_string(),
            format!("parsing {}: unexpected end of JSON input", p.display())
        ),
        Ok(g) => panic!("load of a truncated graph = {g:?}"),
    }
}

/// `tests/test_corrupt_graph_json.py` `test_valid_graph_still_loads` (the `load_graph` half: Go
/// has no `build_merge` over graph.json in `query`, and no `diagnose`).
#[test]
fn upstream_test_valid_graph_still_loads() {
    let (_d, p) = write_graph_json(
        br#"{"nodes": [{"id": "a", "label": "a", "file_type": "code"}], "edges": []}"#,
    );
    let g = load(&p).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(g.nodes.len(), 1, "nodes");
}
