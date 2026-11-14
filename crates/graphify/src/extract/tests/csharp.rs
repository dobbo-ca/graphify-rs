//! graphify-go `internal/extract/csharp_test.go`, and the helpers the C# upstream test files
//! (`upstream_csharp_*.rs`) share. The upstream cases of graphify v0.9.107 are in those files; the
//! `extract-csharp` block of `parity/README.md` says which hold for graphify-go v0.12.0.

use std::collections::{BTreeMap, BTreeSet};

use super::extract_csharp;
use crate::extract::{file_from_bytes, kinds, resolve};
use crate::model::Extraction;
use crate::testutil::testdata;

/// Go `FileFromBytes` on each `(rel, src)`, then `Resolve` (GOMAXPROCS workers).
pub(super) fn run(srcs: &[(&str, &str)]) -> Extraction {
    kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

pub(super) fn label_set(ext: &Extraction) -> BTreeSet<&str> {
    ext.nodes.iter().map(|n| n.label.as_str()).collect()
}

pub(super) fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// (source label, target label) of every edge of `relation`; a missing node is "".
pub(super) fn pairs(ext: &Extraction, relation: &str) -> BTreeSet<(String, String)> {
    let label = id2label(ext);
    let of = |id: &str| label.get(id).copied().unwrap_or("").to_string();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .map(|e| (of(&e.source), of(&e.target)))
        .collect()
}

pub(super) fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    pairs(ext, relation).contains(&(src.to_string(), tgt.to_string()))
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_extract_c_sharp() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("csharpproj");
    let files = ["util/math.cs", "web/server.cs"];

    // .cs is not yet wired into File(); call the extractor directly per file.
    let mut results = Vec::new();
    for f in files {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("ReadFile({f}): {e}"));
        results.push(extract_csharp(f, &src).expect("parse budget"));
    }
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels = label_set(&ext);
    for want in [
        "math.cs",
        "server.cs",
        "Add()",
        "Server",
        "Server.Start()",
        "Boot()",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    // Method scoped under its type, with a contains edge.
    assert!(
        has_edge(&ext, "Server", "contains", "Server.Start()"),
        "expected Server --contains--> Server.Start"
    );
    // Same-file call: Server.Start -> Boot.
    assert!(
        has_edge(&ext, "Server.Start()", "calls", "Boot()"),
        "expected Server.Start --calls--> Boot"
    );
    // Cross-file call: Boot -> Add (unique global definition).
    assert!(
        has_edge(&ext, "Boot()", "calls", "Add()"),
        "expected cross-file Boot --calls--> Add"
    );

    // External import edge for System.Text.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports > 0,
        "no external import edges (expected System.Text)"
    );
}
