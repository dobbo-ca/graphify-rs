//! Port of `validate_test.go` (2 tests).

use super::{AFFECTED_JSON, write_graph_json};
use crate::query::validate;

#[test]
fn test_validate_clean() {
    let (_d, p) = write_graph_json(AFFECTED_JSON.as_bytes());
    let (issues, nodes, links, _) = validate(&p).unwrap_or_else(|e| panic!("{e}"));
    assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    assert!(
        nodes == 4 && links == 2,
        "counts: got {nodes} nodes / {links} links, want 4 / 2"
    );
}

const BROKEN_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"a","label":"a()","file_type":"code","source_file":"x.go"},
 {"id":"a","label":"a-dup()","file_type":"code","source_file":"y.go"},
 {"id":"","label":"empty","file_type":"code","source_file":"z.go"}
],
"links":[
 {"source":"a","target":"ghost","relation":"calls","confidence":"INFERRED"}
],
"hyperedges":[]}"#;

#[test]
fn test_validate_finds_problems() {
    let (_d, p) = write_graph_json(BROKEN_JSON.as_bytes());
    let (issues, ..) = validate(&p).unwrap_or_else(|e| panic!("{e}"));
    let joined = issues.join("\n");
    for want in ["duplicate node id", "empty id", "target node missing"] {
        assert!(
            joined.contains(want),
            "expected an issue mentioning {want:?}, got:\n{joined}"
        );
    }
}
