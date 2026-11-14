//! Port of `report_test.go` (6 tests), then the upstream cases of `test_report.py` that hold for
//! graphify-go v0.12.0.

use std::collections::BTreeMap;

use super::{edge, node, report_graph};
use crate::cluster::cluster;
use crate::model::{Confidence, Graph, Relation, file_of, rel};
use crate::report::generate;

// The freshness line cuts the commit with `short`, which goes through `detect::go_lossy` (R3).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_generate_sections() {
    let g = report_graph();
    let communities = cluster(&g);
    let out = generate(&g, &communities, "testroot", "abcdef1234567");

    for want in [
        "# Graph Report - testroot",
        "## Summary",
        "## God Nodes",
        "## Surprising Connections",
        "## Import Cycles",
        "## Communities",
        "## Graph Freshness",
        "Built from commit: `abcdef12`", // commit truncated to 8 chars
    ] {
        assert!(out.contains(want), "report missing section/line {want:?}");
    }
}

#[test]
fn test_generate_omits_freshness_without_commit() {
    let g = report_graph();
    let communities = cluster(&g);
    let out = generate(&g, &communities, "testroot", "");
    assert!(
        !out.contains("## Graph Freshness"),
        "freshness section should be omitted when no commit is provided"
    );
    assert!(
        out.contains("nodes ·"),
        "summary line should always be present"
    );
}

#[test]
fn test_generate_omits_gap_sections_when_clean() {
    // A single fully-connected community of real nodes: no ambiguous edges, no
    // isolated nodes, no thin communities.
    let mut g = Graph::new();
    g.add_node(node("fc", "c.go", "pkg/c.go"));
    for id in ["c1", "c2", "c3"] {
        g.add_node(node(id, &format!("{id}()"), "pkg/c.go"));
        g.add_edge(edge("fc", id, Relation::CONTAINS, Confidence::EXTRACTED));
    }
    g.add_edge(edge("c1", "c2", Relation::CALLS, Confidence::EXTRACTED));
    g.add_edge(edge("c2", "c3", Relation::CALLS, Confidence::EXTRACTED));
    g.add_edge(edge("c3", "c1", Relation::CALLS, Confidence::EXTRACTED));

    let out = generate(&g, &cluster(&g), "testroot", "");
    for unwanted in ["## Ambiguous Edges", "## Knowledge Gaps"] {
        assert!(
            !out.contains(unwanted),
            "clean graph should omit {unwanted:?}"
        );
    }
}

#[test]
fn test_generate_ambiguous_and_gap_sections() {
    let mut g = report_graph();
    // An AMBIGUOUS edge between two real nodes.
    let mut uses = edge("a3", "b3", rel("uses"), Confidence::AMBIGUOUS);
    uses.source_file = file_of("pkg/a.go");
    g.add_edge(uses);
    // An isolated real node (degree 1, real source file) hanging off its file hub.
    g.add_node(node("lonely", "lonely()", "pkg/a.go"));
    g.add_edge(edge(
        "fa",
        "lonely",
        Relation::CONTAINS,
        Confidence::EXTRACTED,
    ));

    let out = generate(&g, &cluster(&g), "testroot", "");

    for want in [
        "## Ambiguous Edges - Review These",
        "`a3()` --uses--> `b3()`  [AMBIGUOUS]",
        "pkg/a.go · relation: uses",
        "## Knowledge Gaps",
        "isolated node(s):",
        "`lonely()`",
    ] {
        assert!(out.contains(want), "report missing {want:?}");
    }
}

#[test]
fn test_generate_high_ambiguity_warning() {
    let mut g = Graph::new();
    g.add_node(node("x", "x()", "pkg/a.go"));
    g.add_node(node("y", "y()", "pkg/a.go"));
    let mut calls = edge("x", "y", Relation::CALLS, Confidence::AMBIGUOUS);
    calls.source_file = file_of("pkg/a.go");
    g.add_edge(calls);

    let out = generate(&g, &cluster(&g), "testroot", "");
    assert!(
        out.contains("High ambiguity: 100% of edges are AMBIGUOUS."),
        "expected high-ambiguity warning, got:\n{out}"
    );
}

#[test]
fn test_generate_over_budget_line() {
    let mut g = report_graph();
    let communities = cluster(&g);
    let base = generate(&g, &communities, "testroot", "");
    const LINE: &str = "- 2 file(s) skipped: parser made no progress (GRAPHIFY_PARSE_BUDGET)\n";
    assert!(
        !base.contains("skipped"),
        "report without stopped files mentions skipped files"
    );
    g.over_budget = 2;
    let out = generate(&g, &communities, "testroot", "");
    assert!(
        out.replacen(LINE, "", 1) == base,
        "report with 2 stopped files is not the base plus one line:\n{out}"
    );
    assert!(
        out.contains(&format!("{LINE}\n## ")),
        "the line is not the last of the Summary block"
    );
}

// Upstream `test_report.py`. Python builds its graph from `fixtures/extraction.json` and passes
// labels, scores and token counts; Go's `Generate` takes the graph and communities, so these use
// the Go `report_graph` (plus an AMBIGUOUS edge where the section needs one).

#[test]
fn test_report_contains_header() {
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(report.contains("# Graph Report"));
}

#[test]
fn test_report_contains_god_nodes() {
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(report.contains("## God Nodes"));
}

#[test]
fn test_report_contains_surprising_connections() {
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(report.contains("## Surprising Connections"));
}

#[test]
fn test_report_contains_communities() {
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(report.contains("## Communities"));
}

#[test]
fn test_report_contains_ambiguous_section() {
    let mut g = report_graph();
    g.add_edge(edge("a3", "b3", rel("uses"), Confidence::AMBIGUOUS));
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(report.contains("## Ambiguous Edges"));
}

#[test]
fn test_report_shows_raw_cohesion_scores() {
    // Python passes `min_community_size=1`; Go's minimum is 3, so each community here holds three
    // real nodes (the file hub is not one).
    let g = report_graph();
    let communities: BTreeMap<i64, Vec<String>> = BTreeMap::from([
        (0, ["fa", "a1", "a2", "a3"].map(String::from).to_vec()),
        (1, ["fb", "b1", "b2", "b3"].map(String::from).to_vec()),
    ]);
    let report = generate(&g, &communities, "./project", "");
    assert!(report.contains("Cohesion:"));
    assert!(!report.contains("✓"));
    assert!(!report.contains("⚠"));
}

#[test]
fn test_import_cycles_section_present_for_code_corpus() {
    // #1657: the fixture is a code corpus, so the Import Cycles section shows.
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(report.contains("## Import Cycles"));
}

#[test]
fn test_report_omits_unclassified_line_when_none() {
    // Backward compatible: no unclassified files, no new line.
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(!report.contains("Unclassified:"));
}

#[test]
fn test_report_work_memory_section_absent_without_overlay() {
    // No learning input => no section (without the python test's `before == empty` half: Go's
    // `Generate` has no `learning` argument).
    let g = report_graph();
    let report = generate(&g, &cluster(&g), "./project", "");
    assert!(!report.contains("## Work-memory lessons"));
}
