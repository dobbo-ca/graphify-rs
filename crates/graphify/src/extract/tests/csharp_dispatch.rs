//! graphify-go `internal/extract/csharp_dispatch_test.go` (handoff branch
//! `handoff-csharp-resolve-tests-6f1d`). These test `resolveCSharpDispatch` (`dispatch.go`),
//! which `Resolve` calls (`resolve.go:1044`), with hand-made `Result`s: they belong to the
//! resolve stream (graphify-rs-ihc.18) and are attached from `tests/mod.rs`. They parse C# (and,
//! in one, Java), so they pass once the C# extractor lands: graphify-rs-ihc.28.

use std::collections::BTreeMap;

use crate::extract::csharp::extract_csharp;
use crate::extract::java::extract_java;
use crate::extract::{Result, kinds, resolve};
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Node, Relation, file_of};

const IFACE_SRC: &str = "interface IReport {\n  string Build();\n}\n";

const REPORT_SRC: &str = "class Report {\n  public string Build() { return \"x\"; }\n}\n";

const AUDIT_SRC: &str = "class Audit {\n  public string Build() { return \"y\"; }\n}\n";

/// Go `implementsEdge` stands in for the inherits/implements pass: `implements` edges are not
/// emitted yet, so the dispatch pass is fed the edge it will walk.
fn implements_edge(impl_file: &str, impl_type: &str) -> Edge {
    Edge {
        source: make_id(&[impl_file, impl_type]),
        target: make_id(&["ireport.cs", "IReport"]),
        relation: Relation::IMPLEMENTS,
        confidence: Confidence::EXTRACTED,
        source_file: file_of(impl_file),
        ..Edge::default()
    }
}

/// Go `dispatchTargets`: the `dispatches_to` edges of `Resolve` as source label to target label.
fn dispatch_targets(results: &[Result], files: &[&str]) -> BTreeMap<String, String> {
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(results, &files, workers);
    let label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let of = |id: &str| label.get(id).copied().unwrap_or("").to_string();
    let mut got = BTreeMap::new();
    for e in &ext.edges {
        if e.relation == Relation::DISPATCHES_TO {
            got.insert(of(&e.source), of(&e.target));
        }
    }
    got
}

fn csharp(rel: &str, src: &str) -> Result {
    extract_csharp(rel, src.as_bytes()).expect("parse budget")
}

/// One implementer owning one same-named method: the interface method dispatches to the
/// implementation.
#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_c_sharp_dispatch_single_implementer() {
    kinds::register_kinds();
    let files = ["IReport.cs", "Report.cs"];
    let mut results = vec![
        csharp("IReport.cs", IFACE_SRC),
        csharp("Report.cs", REPORT_SRC),
    ];
    results[1]
        .edges
        .push(implements_edge("Report.cs", "Report"));

    let got = dispatch_targets(&results, &files);
    assert_eq!(
        got.get("IReport.Build()").map(String::as_str),
        Some("Report.Build()"),
        "expected IReport.Build --dispatches_to--> Report.Build, got {got:?}"
    );
}

/// Two implementers: the runtime target is ambiguous, so nothing is emitted.
#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_c_sharp_dispatch_two_implementers() {
    kinds::register_kinds();
    let files = ["IReport.cs", "Report.cs", "Audit.cs"];
    let mut results = vec![
        csharp("IReport.cs", IFACE_SRC),
        csharp("Report.cs", REPORT_SRC),
        csharp("Audit.cs", AUDIT_SRC),
    ];
    results[1]
        .edges
        .push(implements_edge("Report.cs", "Report"));
    results[2].edges.push(implements_edge("Audit.cs", "Audit"));

    let got = dispatch_targets(&results, &files);
    assert!(
        got.is_empty(),
        "expected no dispatches_to with two implementers, got {got:?}"
    );
}

/// A non-C# implementer is a name collision, not an implementation. Parses Java too
/// (graphify-rs-ihc.27).
#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_c_sharp_dispatch_cross_language() {
    kinds::register_kinds();
    let files = ["IReport.cs", "Report.java"];
    let java_src = "class Report {\n  public String Build() { return \"x\"; }\n}\n";
    let mut results = vec![
        csharp("IReport.cs", IFACE_SRC),
        extract_java("Report.java", java_src.as_bytes()).expect("parse budget"),
    ];
    results[1]
        .edges
        .push(implements_edge("Report.java", "Report"));

    let got = dispatch_targets(&results, &files);
    assert!(
        got.is_empty(),
        "expected no dispatches_to across a language boundary, got {got:?}"
    );
}

/// A later node of another language under the implementing method's id takes the id, as in the
/// graph, so the method is no C# candidate any more.
#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_c_sharp_dispatch_id_taken_by_other_language() {
    kinds::register_kinds();
    let files = ["IReport.cs", "Report.cs", "report.ts"];
    let mut results = vec![
        csharp("IReport.cs", IFACE_SRC),
        csharp("Report.cs", REPORT_SRC),
        Result::default(),
    ];
    results[1]
        .edges
        .push(implements_edge("Report.cs", "Report"));
    let shadow: Vec<Node> = results[1]
        .nodes
        .iter()
        .filter(|n| n.label == "Report.Build()")
        .map(|n| Node {
            id: n.id.clone(),
            label: n.label.clone(),
            source_file: "report.ts".to_string(),
            ..Node::default()
        })
        .collect();
    assert_eq!(shadow.len(), 1, "no Report.Build() node to shadow");
    results[2].nodes = shadow;

    let got = dispatch_targets(&results, &files);
    assert!(got.is_empty(), "expected no dispatch edge, got {got:?}");
}
