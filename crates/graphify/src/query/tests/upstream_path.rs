//! Upstream graphify v0.9.107 tests of `path` and its endpoint resolution, as graphify-go
//! v0.12.0 answers them: `tests/test_path_confidence.py` (4 of 12) and
//! `tests/test_path_type_member.py` (8 of 15).
//!
//! Go's `path` prints the hops `path_edges` returns (`renderPathChain`, graphify-rs-ihc.22) and
//! no confidence summary line (B62); its endpoints resolve with `Graph::resolve` (id, `::`,
//! file path, exact label, substring), with no `Type.Member` tier (B09). The rows pin the hops
//! and the resolved nodes Go gives.

use super::load_str;
use crate::query::{Error, Graph, PathEdge, path_edges};

/// Go `PathEdge{relation, confidence, forward: true}`.
fn fwd(relation: &str, confidence: &str) -> PathEdge {
    PathEdge {
        relation: relation.to_string(),
        confidence: confidence.to_string(),
        forward: true,
    }
}

/// Checks `path_edges(g, from, to, 0, false)`: the node ids and the hops.
fn assert_path(name: &str, g: &Graph, from: &str, to: &str, ids: &[&str], hops: &[PathEdge]) {
    let res = path_edges(g, from, to, 0, false).unwrap_or_else(|e| panic!("{name}: {e}"));
    let got: Vec<&str> = res.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(got, ids, "{name}: path nodes");
    assert_eq!(res.edges, hops, "{name}: path hops");
}

// ---- tests/test_path_confidence.py ----

/// Upstream `_report_graph` (Report.cs).
fn report_graph() -> Graph {
    load_str(
        r#"{"directed": true, "multigraph": false, "graph": {}, "nodes": [
 {"id": "report_exporter_run", "label": ".Run()", "file_type": "code", "source_file": "Report.cs"},
 {"id": "report_ireport_build", "label": ".Build()", "file_type": "code", "source_file": "Report.cs"},
 {"id": "report_pdfreport_build", "label": ".Build()", "file_type": "code", "source_file": "Report.cs"}],
"links": [
 {"source": "report_exporter_run", "target": "report_ireport_build", "relation": "calls", "confidence": "INFERRED", "confidence_score": 0.85, "source_file": "Report.cs", "source_location": "L7"},
 {"source": "report_ireport_build", "target": "report_pdfreport_build", "relation": "dispatches_to", "confidence": "INFERRED", "confidence_score": 0.75, "source_file": "Report.cs", "source_location": "L2"}]}"#,
    )
}

// pins Go v0.12.0 (the two hops, no summary line); upstream 0.9.107 ends with "Path confidence:
// INFERRED (weakest hop 2: dispatches_to, score 0.75)" -> P3 (B62)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_path_confidence() {
    let g = report_graph();
    let ids = [
        "report_exporter_run",
        "report_ireport_build",
        "report_pdfreport_build",
    ];
    let hops = [fwd("calls", "INFERRED"), fwd("dispatches_to", "INFERRED")];
    for name in [
        "test_cli_path_prints_weakest_hop",
        "test_mcp_shortest_path_prints_weakest_hop",
    ] {
        assert_path(
            name,
            &g,
            "report_exporter_run",
            "report_pdfreport_build",
            &ids,
            &hops,
        );
    }

    let g = load_str(
        r#"{"directed": true, "multigraph": false, "graph": {}, "nodes": [
 {"id": "report_exporter", "label": "Exporter", "file_type": "code", "source_file": "Report.cs"},
 {"id": "report_exporter_run", "label": ".Run()", "file_type": "code", "source_file": "Report.cs"}],
"links": [
 {"source": "report_exporter", "target": "report_exporter_run", "relation": "method", "confidence": "EXTRACTED", "confidence_score": 1.0, "source_file": "Report.cs", "source_location": "L7"}]}"#,
    );
    assert_path(
        "test_extracted_path_from_built_graph_prints_no_score",
        &g,
        "report_exporter",
        "report_exporter_run",
        &["report_exporter", "report_exporter_run"],
        &[fwd("method", "EXTRACTED")],
    );

    // Two 2-hop routes A -> C. Go's BFS visits neighbours in id order, so B1 (strong) wins.
    // pins Go v0.12.0 (A -> B1 -> C, both EXTRACTED); upstream 0.9.107 reports the weakest of
    // the equal-length routes (AMBIGUOUS, via B2) -> P3 (B62)
    let g = load_str(
        r#"{"directed": true, "multigraph": false, "graph": {}, "nodes": [
 {"id": "A", "label": "A", "file_type": "code", "source_file": "m.py"},
 {"id": "B1", "label": "B1", "file_type": "code", "source_file": "m.py"},
 {"id": "C", "label": "C", "file_type": "code", "source_file": "m.py"},
 {"id": "B2", "label": "B2", "file_type": "code", "source_file": "m.py"}],
"links": [
 {"source": "A", "target": "B1", "relation": "calls", "confidence": "EXTRACTED", "source_file": "m.py", "source_location": "L1"},
 {"source": "B1", "target": "C", "relation": "calls", "confidence": "EXTRACTED", "source_file": "m.py", "source_location": "L2"},
 {"source": "A", "target": "B2", "relation": "calls", "confidence": "EXTRACTED", "source_file": "m.py", "source_location": "L3"},
 {"source": "B2", "target": "C", "relation": "dispatches_to", "confidence": "AMBIGUOUS", "confidence_score": 0.3, "source_file": "m.py", "source_location": "L4"}]}"#,
    );
    assert_path(
        "test_multiple_routes_report_the_weakest_route",
        &g,
        "A",
        "C",
        &["A", "B1", "C"],
        &[fwd("calls", "EXTRACTED"), fwd("calls", "EXTRACTED")],
    );
}

// ---- tests/test_path_type_member.py ----

/// Upstream `_write_nodes`: `(id, label, source_file, source_location)` nodes and
/// `(source, target, relation)` EXTRACTED links, undirected.
fn write_nodes(nodes: &[(&str, &str, &str, &str)], links: &[(&str, &str, &str)]) -> Graph {
    let nodes: Vec<String> = nodes
        .iter()
        .map(|(i, l, sf, loc)| {
            format!(
                r#"{{"id": "{i}", "label": "{l}", "source_file": "{sf}", "source_location": "{loc}"}}"#
            )
        })
        .collect();
    let links: Vec<String> = links
        .iter()
        .map(|(a, b, rel)| {
            format!(
                r#"{{"source": "{a}", "target": "{b}", "relation": "{rel}", "confidence": "EXTRACTED"}}"#
            )
        })
        .collect();
    load_str(&format!(
        r#"{{"directed": false, "multigraph": false, "graph": {{}}, "nodes": [{}], "links": [{}]}}"#,
        nodes.join(", "),
        links.join(", ")
    ))
}

/// What Go's `Graph::resolve` gives a query.
fn resolved(g: &Graph, q: &str) -> Option<String> {
    match g.resolve(q) {
        Ok(n) => n.map(|n| n.id.clone()),
        Err(e) => panic!("resolve({q:?}): {e:?}"),
    }
}

/// Upstream `_nested_graph`.
fn nested_graph() -> Graph {
    write_nodes(
        &[
            ("a", "A.cs", "A.cs", "L1"),
            ("ns", "Acme.Io", "A.cs", "L1"),
            ("outer", "Outer", "A.cs", "L2"),
            ("outer_go", ".Go()", "A.cs", "L3"),
            ("inner", "Inner", "A.cs", "L4"),
            ("inner_go", ".Go()", "A.cs", "L5"),
        ],
        &[
            ("a", "ns", "contains"),
            ("a", "outer", "contains"),
            ("outer", "inner", "contains"),
            ("outer", "outer_go", "method"),
            ("inner", "inner_go", "method"),
        ],
    )
}

/// Upstream `_file_name_graph`.
fn file_name_graph() -> Graph {
    write_nodes(
        &[
            ("src_utils", "src/utils.py", "src/utils.py", "L1"),
            ("src_utils_helper", "helper()", "src/utils.py", "L3"),
            ("utils", "Utils", "Utils.cs", "L1"),
            ("utils_run", ".Run()", "Utils.cs", "L2"),
            ("config_port", "port", "config.json", "L2"),
            ("cfg", "Config", "Config.cs", "L1"),
            ("cfg_json", ".Json()", "Config.cs", "L2"),
        ],
        &[
            ("src_utils", "src_utils_helper", "contains"),
            ("utils", "utils_run", "method"),
            ("cfg", "cfg_json", "method"),
        ],
    )
}

/// `test_type_member_in_two_files_is_refused` and `test_overloaded_member_is_refused_with_ids`:
/// `PdfReport.Build` matches no tier, so `path` fails before it searches.
// pins Go v0.12.0 (`no node matching "PdfReport.Build"`); upstream 0.9.107 exits 3 with
// "Ambiguous: 'PdfReport.Build' matches 2 nodes ..." -> P3 (B09)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_type_member_is_refused() {
    let two_files = write_nodes(
        &[
            ("a_run", ".Run()", "a/Exporter.cs", "L3"),
            ("a_pdf", "PdfReport", "a/Pdf.cs", "L1"),
            ("a_pdf_build", ".Build()", "a/Pdf.cs", "L2"),
            ("b_pdf", "PdfReport", "b/Pdf.cs", "L1"),
            ("b_pdf_build", ".Build()", "b/Pdf.cs", "L2"),
        ],
        &[
            ("a_pdf", "a_pdf_build", "method"),
            ("b_pdf", "b_pdf_build", "method"),
            ("a_run", "a_pdf_build", "calls"),
        ],
    );
    let overloaded = write_nodes(
        &[
            ("pdf", "PdfReport", "Pdf.cs", "L1"),
            ("pdf_build", ".Build()", "Pdf.cs", "L2"),
            ("pdf_build_1", ".Build()", "Pdf.cs", "L3"),
            ("other_build", ".Build()", "Other.cs", "L1"),
        ],
        &[
            ("pdf", "pdf_build", "method"),
            ("pdf", "pdf_build_1", "method"),
        ],
    );
    for (name, g, from, to) in [
        (
            "test_type_member_in_two_files_is_refused",
            &two_files,
            "Run",
            "PdfReport.Build",
        ),
        (
            "test_overloaded_member_is_refused_with_ids",
            &overloaded,
            "PdfReport.Build",
            "pdf",
        ),
    ] {
        match path_edges(g, from, to, 0, false) {
            Err(Error::Other(e)) => {
                assert_eq!(e, r#"no node matching "PdfReport.Build""#, "{name}")
            }
            r => panic!("{name}: path = {r:?}, want no node matching"),
        }
    }
}

/// `test_enclosing_types_qualify_a_nested_member` and
/// `test_namespace_qualifier_is_refused_with_the_bare_type_members`: no dotted query matches a
/// label or id, so none resolves. `Inner.Outer` and `Acme.Io.Outer.Go` resolve to nothing
/// upstream too.
// pins Go v0.12.0 (no match for Outer.Inner.Go, Inner.Go, Outer.Go, Outer.Inner); upstream
// 0.9.107 resolves them through the types' membership edges -> P3 (B09)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_nested_type_members() {
    let g = nested_graph();
    for q in [
        "Outer.Inner.Go",
        "Inner.Go",
        "Outer.Go",
        "Outer.Inner",
        "Inner.Outer",
        "Acme.Io.Outer.Go",
    ] {
        assert_eq!(resolved(&g, q), None, "resolve({q:?})");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_dotted_label_and_non_type_owner_keep_their_old_resolution() {
    let g = write_nodes(
        &[
            ("cfg", "Config", "app.ts", "L1"),
            ("cfg_load", ".load()", "app.ts", "L2"),
            ("literal", "Config.load", "routes.ts", "L9"),
            ("helper", "release notes helper", "notes.md", "L1"),
        ],
        &[("cfg", "cfg_load", "method")],
    );
    // A node literally labelled `Config.load` is the one named.
    assert_eq!(resolved(&g, "Config.load").as_deref(), Some("literal"));
    // pins Go v0.12.0 (no match: Go has no token scoring in `resolve`); upstream 0.9.107 scores
    // `release.notes` to helper -> P3 (B09)
    assert_eq!(resolved(&g, "release.notes"), None);
}

/// `test_file_name_endpoint_is_not_refused_as_a_missing_member`: `utils.py` is a substring of
/// the file node's label, `helper()` its exact label; the path is the one `contains` hop.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_file_name_endpoint_is_not_refused_as_a_missing_member() {
    let g = file_name_graph();
    assert_eq!(resolved(&g, "utils.py").as_deref(), Some("src_utils"));
    assert_path(
        "test_file_name_endpoint_is_not_refused_as_a_missing_member",
        &g,
        "utils.py",
        "helper()",
        &["src_utils", "src_utils_helper"],
        &[fwd("contains", "EXTRACTED")],
    );
}

/// `test_file_name_beats_a_same_shaped_member`: config.json has no file node (`port` is on L2)
/// and `Utils.Run` matches no tier.
// pins Go v0.12.0 (no match for config.json, Utils.Run); upstream 0.9.107 resolves config_port
// and utils_run -> P3 (B09)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_file_name_beats_a_same_shaped_member() {
    let g = file_name_graph();
    assert_eq!(resolved(&g, "config.json"), None);
    assert_eq!(resolved(&g, "Utils.Run"), None);
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_exact_dotted_node_id_is_not_refused_as_a_missing_member() {
    let g = write_nodes(
        &[
            ("Report.Build", "build step", "ci.yml", "L4"),
            ("ci_deploy", "deploy step", "ci.yml", "L9"),
            ("report", "Report", "Report.cs", "L1"),
            ("report_render", ".Render()", "Report.cs", "L2"),
        ],
        &[
            ("Report.Build", "ci_deploy", "calls"),
            ("report", "report_render", "method"),
        ],
    );
    assert_eq!(
        resolved(&g, "Report.Build").as_deref(),
        Some("Report.Build")
    );
    assert_path(
        "test_exact_dotted_node_id_is_not_refused_as_a_missing_member",
        &g,
        "Report.Build",
        "ci_deploy",
        &["Report.Build", "ci_deploy"],
        &[fwd("calls", "EXTRACTED")],
    );
}
