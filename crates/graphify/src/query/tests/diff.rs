//! Port of `diff_test.go` (4 tests).

use super::load_json;
use crate::query::{DiffEdge, DiffNode, diff};

/// Go `oldDiffJSON`: util.go defines add(); math.go defines compute() which calls add().
const OLD_DIFF_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"add","label":"add()","file_type":"code","source_file":"util.go","source_location":"L1"},
 {"id":"compute","label":"compute()","file_type":"code","source_file":"math.go","source_location":"L1"}
],
"links":[
 {"source":"compute","target":"add","relation":"calls","confidence":"INFERRED"}
],
"hyperedges":[]}"#;

/// Go `newDiffJSON`: add() removed, sub() added; compute() now calls sub() instead.
const NEW_DIFF_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"sub","label":"sub()","file_type":"code","source_file":"util.go","source_location":"L5"},
 {"id":"compute","label":"compute()","file_type":"code","source_file":"math.go","source_location":"L1"}
],
"links":[
 {"source":"compute","target":"sub","relation":"calls","confidence":"INFERRED"}
],
"hyperedges":[]}"#;

/// Go `DiffNode{ID: id, Label: label}`.
fn dn(id: &str, label: &str) -> DiffNode {
    DiffNode {
        id: id.to_string(),
        label: label.to_string(),
    }
}

/// Go `DiffEdge{Source, Target, Relation}`.
fn de(source: &str, target: &str, relation: &str) -> DiffEdge {
    DiffEdge {
        source: source.to_string(),
        target: target.to_string(),
        relation: relation.to_string(),
    }
}

#[test]
fn test_diff() {
    let old_g = load_json(OLD_DIFF_JSON);
    let new_g = load_json(NEW_DIFF_JSON);
    let res = diff(&old_g, &new_g);

    assert_eq!(res.new_nodes, [dn("sub", "sub()")], "new nodes");
    assert_eq!(res.removed_nodes, [dn("add", "add()")], "removed nodes");
    assert_eq!(
        res.new_edges,
        [de("compute", "sub", "calls")],
        "new edges: want [compute->sub]"
    );
    assert_eq!(
        res.removed_edges,
        [de("compute", "add", "calls")],
        "removed edges: want [compute->add]"
    );
    assert_eq!(
        res.summary, "1 new node, 1 new edge, 1 node removed, 1 edge removed",
        "summary"
    );
}

#[test]
fn test_diff_no_changes() {
    let g1 = load_json(OLD_DIFF_JSON);
    let g2 = load_json(OLD_DIFF_JSON);
    let res = diff(&g1, &g2);
    assert_eq!(res.summary, "no changes", "summary");
    assert_eq!(
        res.new_nodes.len()
            + res.removed_nodes.len()
            + res.new_edges.len()
            + res.removed_edges.len(),
        0,
        "expected empty delta, got {res:?}"
    );
}

/// Go `TestDiffEdgeDirectionAware`: the graph is directed, so reversing an edge's endpoints
/// between snapshots registers as one removed (x->y) and one added (y->x).
#[test]
fn test_diff_edge_direction_aware() {
    let a = r#"{"directed":true,"nodes":[
 {"id":"x","label":"x()","source_file":"a.go","source_location":"L1"},
 {"id":"y","label":"y()","source_file":"b.go","source_location":"L1"}
],"links":[{"source":"x","target":"y","relation":"calls","confidence":"INFERRED"}]}"#;
    let b = r#"{"directed":true,"nodes":[
 {"id":"x","label":"x()","source_file":"a.go","source_location":"L1"},
 {"id":"y","label":"y()","source_file":"b.go","source_location":"L1"}
],"links":[{"source":"y","target":"x","relation":"calls","confidence":"INFERRED"}]}"#;
    let res = diff(&load_json(a), &load_json(b));
    assert_eq!(
        res.new_edges,
        [de("y", "x", "calls")],
        "new edges: want [y->x]"
    );
    assert_eq!(
        res.removed_edges,
        [de("x", "y", "calls")],
        "removed edges: want [x->y]"
    );
    assert_eq!(res.summary, "1 new edge, 1 edge removed", "summary");
}

/// Go `TestDiffMutualRecursionEdgeRemoved`: when a node pair has edges in both directions
/// (A->B and B->A) and one is removed, the removal must be reported. A direction-insensitive key
/// would collapse both edges and hide the removal.
#[test]
fn test_diff_mutual_recursion_edge_removed() {
    let a = r#"{"directed":true,"nodes":[
 {"id":"A","label":"A()","source_file":"a.go","source_location":"L1"},
 {"id":"B","label":"B()","source_file":"b.go","source_location":"L1"}
],"links":[
 {"source":"A","target":"B","relation":"calls","confidence":"INFERRED"},
 {"source":"B","target":"A","relation":"calls","confidence":"INFERRED"}
]}"#;
    let b = r#"{"directed":true,"nodes":[
 {"id":"A","label":"A()","source_file":"a.go","source_location":"L1"},
 {"id":"B","label":"B()","source_file":"b.go","source_location":"L1"}
],"links":[
 {"source":"A","target":"B","relation":"calls","confidence":"INFERRED"}
]}"#;
    let res = diff(&load_json(a), &load_json(b));
    assert!(
        res.new_edges.is_empty(),
        "new edges: got {:?}, want none",
        res.new_edges
    );
    assert_eq!(
        res.removed_edges,
        [de("B", "A", "calls")],
        "removed edges: want [B->A]"
    );
    assert_eq!(res.summary, "1 edge removed", "summary");
}
