//! graphify-go `internal/extract/julia_test.go`. There is no upstream Julia test file.

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
#[ignore = "graphify-rs-ihc.38"]
fn test_extract_julia() {
    let ext = extract_proj("juliaproj", &["math.jl", "geometry.jl"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.jl",
        "geometry.jl",
        "MathUtils",
        "MathUtils.square()",
        "cube()",
        "Circle",
        "Shapes",
        "Shapes.area()",
        "Shapes.scale()",
        "LinearAlgebra",
        "Printf",
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

    // Module function scoped under its module, with a contains edge.
    assert!(
        has("Shapes", "contains", "Shapes.area()"),
        "expected Shapes --contains--> Shapes.area()"
    );
    // Same-file call: Shapes.area -> Shapes.scale.
    assert!(
        has("Shapes.area()", "calls", "Shapes.scale()"),
        "expected Shapes.area --calls--> Shapes.scale"
    );
    // Cross-file call: Shapes.scale -> MathUtils.square (unique global square).
    assert!(
        has("Shapes.scale()", "calls", "MathUtils.square()"),
        "expected cross-file Shapes.scale --calls--> MathUtils.square"
    );

    // `using LinearAlgebra` records an external import.
    let has_import = |spec: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == "imports" && id2label.get(e.target.as_str()) == Some(&spec)
        })
    };
    assert!(
        has_import("LinearAlgebra"),
        "expected import of LinearAlgebra"
    );
}
