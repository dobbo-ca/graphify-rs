//! graphify-go `internal/extract/verilog_test.go`.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file, kinds, resolve};
use crate::model::Extraction;
use crate::testutil::testdata;

/// Go `File` on each file of `testdata/extract/<proj>`, then `Resolve` (GOMAXPROCS workers).
fn extract_proj(proj: &str, files: &[&str]) -> Extraction {
    kinds::register_kinds();
    let root = testdata().join("extract").join(proj);
    let results: Vec<_> = files
        .iter()
        .map(|f| file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

#[test]
#[ignore = "graphify-rs-ihc.41"]
fn test_extract_verilog() {
    let ext = extract_proj("verilogproj", &["rtl/alu.sv"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "alu.sv",
        "add()",
        "compute()",
        "Counter",
        "Counter.step()",
        "alu",
        "defs.svh",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };
    // Class method scoped under its type, with a contains edge.
    assert!(
        has("Counter", "contains", "Counter.step()"),
        "expected Counter --contains--> Counter.step()"
    );
    // Same-file call inside a function body: compute -> add.
    assert!(
        has("compute()", "calls", "add()"),
        "expected compute --calls--> add"
    );
    // Same-file call inside a method body: Counter.step -> compute.
    assert!(
        has("Counter.step()", "calls", "compute()"),
        "expected Counter.step --calls--> compute"
    );
    // `include "defs.svh"` recorded as an external import.
    assert!(
        has("alu.sv", "imports", "defs.svh"),
        "expected alu.sv --imports--> defs.svh"
    );
}
