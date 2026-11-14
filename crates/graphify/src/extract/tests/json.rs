//! graphify-go `internal/extract/json_test.go`, and the plan's Task 15 added test
//! `deep_json_nesting`. A test that parses a `.json` file goes through `ts::parse` (ihc.15) and
//! `extractJSON` (`json.go`, ihc.16), so it is ignored on ihc.16, the later of the two.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::Result;
use crate::extract::tests::util::extract_bytes;
use crate::graph;
use crate::idutil::make_id;
use crate::model::{Extraction, rel};
use crate::testutil::on_worker_stack;

/// Go `jsonNodes`: the labels, and each node's label by id.
fn json_nodes(res: &Result) -> (BTreeSet<&str>, BTreeMap<&str, &str>) {
    let labels = res.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label = res
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    (labels, id2label)
}

/// Go `jsonHasEdge`: matches by source label and target ID. extends/imports targets get a
/// "concept" node (mirrors upstream #1764) so the edge survives graph.Build; $ref/references
/// targets stay edge-only (dangling, per upstream). The target is matched by its computed ID, not
/// a node label.
fn json_has_edge(
    res: &Result,
    id2label: &BTreeMap<&str, &str>,
    src_label: &str,
    relation: &str,
    tgt_id: &str,
) -> bool {
    res.edges.iter().any(|e| {
        e.relation.as_str() == relation
            && id2label.get(e.source.as_str()) == Some(&src_label)
            && e.target == tgt_id
    })
}

#[test]
fn test_extract_json_package_manifest() {
    let src = r#"{
  "name": "demo",
  "dependencies": {
    "left-pad": "^1.0.0"
  },
  "devDependencies": {
    "jest": "^29.0.0"
  }
}"#;
    let res = extract_bytes("package.json", src.as_bytes());

    let (labels, id2label) = json_nodes(&res);
    for want in [
        "package.json",
        "name",
        "dependencies",
        "left-pad",
        "devDependencies",
        "jest",
    ] {
        assert!(labels.contains(want), "missing node {want:?}");
    }
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "package.json",
            "contains",
            &make_id(&["package.json", "dependencies"])
        ),
        "expected package.json --contains--> dependencies"
    );
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "dependencies",
            "contains",
            &make_id(&["package.json", "dependencies", "left-pad"])
        ),
        "expected dependencies --contains--> left-pad"
    );
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "left-pad",
            "imports",
            &make_id(&["left-pad"])
        ),
        "expected dep key --imports--> dep node"
    );
}

#[test]
fn test_extract_json_tsconfig_extends_and_ref() {
    let src = r#"{
  "extends": "./base.json",
  "compilerOptions": {
    "strict": true
  }
}"#;
    let res = extract_bytes("tsconfig.json", src.as_bytes());

    let (_, id2label) = json_nodes(&res);
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "tsconfig.json",
            "extends",
            &make_id(&["ref", "./base.json"])
        ),
        "expected tsconfig.json --extends--> ./base.json ref"
    );
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "tsconfig.json",
            "contains",
            &make_id(&["tsconfig.json", "compilerOptions"])
        ),
        "expected tsconfig.json --contains--> compilerOptions"
    );
}

/// Verifies the #1764 fix: the "extends" target gets a "concept" node so graph.Build no longer
/// drops the edge as dangling. The $ref/references target still has no node (stays dangling per
/// parity).
#[test]
fn test_extract_json_extends_target_survives_build() {
    let res = extract_bytes(
        "tsconfig.json",
        br#"{
  "extends": "./base.json"
}"#,
    );

    let ref_id = make_id(&["ref", "./base.json"]);
    let concept = res
        .nodes
        .iter()
        .find(|n| n.id == ref_id)
        .unwrap_or_else(|| {
            panic!("expected concept target node for extends ./base.json (id {ref_id:?})")
        });
    assert_eq!(
        concept.file_type, "concept",
        "extends target file_type = {:?}, want \"concept\"",
        concept.file_type
    );
    assert_eq!(
        concept.label, "./base.json",
        "extends target label = {:?}, want \"./base.json\"",
        concept.label
    );

    let g = graph::build(Extraction {
        nodes: res.nodes.clone(),
        edges: res.edges.clone(),
    });
    let file_id = make_id(&["tsconfig.json"]);
    let survived = g
        .edges()
        .iter()
        .any(|e| e.relation == rel("extends") && e.source == file_id && e.target == ref_id);
    assert!(
        survived,
        "expected tsconfig.json --extends--> ./base.json edge to survive graph.Build"
    );
}

#[test]
fn test_extract_json_extends_array() {
    let src = r#"{
  "extends": ["eslint:recommended", "plugin:react/recommended"]
}"#;
    let res = extract_bytes(".eslintrc.json", src.as_bytes());

    let (_, id2label) = json_nodes(&res);
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "extends",
            "extends",
            &make_id(&["ref", "eslint:recommended"])
        ),
        "expected extends key --extends--> eslint:recommended ref"
    );
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "extends",
            "extends",
            &make_id(&["ref", "plugin:react/recommended"])
        ),
        "expected extends key --extends--> plugin:react/recommended ref"
    );
}

#[test]
fn test_extract_json_schema_ref() {
    // Recognized via the top-level $schema key probe (arbitrary basename).
    let src = r##"{
  "$schema": "https://example.com/schema.json",
  "thing": {
    "$ref": "#/defs/widget"
  }
}"##;
    let res = extract_bytes("config/whatever.json", src.as_bytes());

    let (_, id2label) = json_nodes(&res);
    assert!(
        json_has_edge(
            &res,
            &id2label,
            "thing",
            "references",
            &make_id(&["ref", "#/defs/widget"])
        ),
        "expected $ref parent --references--> ref node"
    );
}

#[test]
fn test_extract_json_skips_data_json() {
    // No recognized basename, no config key probe hit => data JSON, skipped.
    let src = r#"{"users": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]}"#;
    let res = extract_bytes("fixtures/users.json", src.as_bytes());
    assert!(
        res.nodes.is_empty() && res.edges.is_empty(),
        "data json should be skipped, got {} nodes / {} edges",
        res.nodes.len(),
        res.edges.len()
    );
}

#[test]
fn test_extract_json_non_object_root_skipped() {
    let res = extract_bytes("data.json", b"[1, 2, 3]");
    assert!(
        res.nodes.is_empty() && res.edges.is_empty(),
        "top-level array should be skipped, got {} nodes",
        res.nodes.len()
    );
}

#[test]
fn test_extract_json_malformed_skipped() {
    let res = extract_bytes("package.json", b"{not valid json");
    // tree-sitter is error-tolerant; a malformed config still must not panic and must not emit a
    // non-file node from garbage. At minimum it stays bounded.
    assert!(
        !(res.nodes.len() > 1 && res.nodes[0].label != "package.json"),
        "unexpected nodes from malformed json: {}",
        res.nodes.len()
    );
}

/// `.mcp.json` must reach `extractMCPConfig` (server nodes), NOT `extractJSON` (config-key
/// nodes). Guards against the general extractor stealing it. It never parses with a grammar, so
/// it waits for the extractor of `mcpconfig.go` (ihc.17).
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_json_mcp_config_still_routed_to_mcp_extractor() {
    let src = r#"{"mcpServers": {"fs": {"command": "npx", "args": ["-y", "@scope/server-fs"]}}}"#;
    let res = extract_bytes(".mcp.json", src.as_bytes());
    let (labels, _) = json_nodes(&res);
    assert!(
        labels.contains("fs"),
        "expected mcp_server node 'fs' from extractMCPConfig"
    );
    assert!(
        !labels.contains("mcpServers"),
        ".mcp.json was AST-walked by extractJSON instead of routed to extractMCPConfig"
    );
}

#[test]
fn test_extract_json_schema_document_skipped() {
    let src = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://example.com/x.json",
  "definitions": {"a": {"type": "string"}},
  "properties": {"dependencies": {"type": "object"}}
}"#;
    let res = extract_bytes("contracts/x.json", src.as_bytes());
    assert!(
        res.nodes.is_empty() && res.edges.is_empty(),
        "schema doc should be skipped, got {} nodes / {} edges",
        res.nodes.len(),
        res.edges.len()
    );
}

#[test]
fn test_extract_json_schema_alone_still_walked() {
    let src = r#"{"$schema": "https://example.com/s.json", "thing": {"a": 1}}"#;
    let res = extract_bytes("config/whatever.json", src.as_bytes());
    let (labels, _) = json_nodes(&res);
    assert!(
        labels.contains("thing"),
        "$schema-only config should still be walked"
    );
}

/// Hazard "Stack depth": `extractJSON` and the parser recurse on input depth. 100,000 nested `[`
/// in a `.json` file must not overflow the stack; the top level is not an object, so nothing but
/// (at most) the file node of a stopped parse comes back.
#[test]
fn deep_json_nesting() {
    let src = "[".repeat(100_000);
    let res = on_worker_stack(|| extract_bytes("data.json", src.as_bytes()));
    assert!(
        res.edges.is_empty() && res.nodes.len() <= 1,
        "deep nesting: {} nodes, {} edges; want none (or only the file node of a stopped parse)",
        res.nodes.len(),
        res.edges.len()
    );
}
