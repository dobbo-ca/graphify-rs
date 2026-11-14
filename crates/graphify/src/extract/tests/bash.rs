//! graphify-go `internal/extract/bash_test.go`.

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
fn test_extract_bash() {
    let ext = extract_proj("bashproj", &["util/math.sh", "app/main.sh"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in ["math.sh", "main.sh", "add()", "double()", "boot()"] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };

    // File contains its function definition.
    assert!(
        has("math.sh", "contains", "add()"),
        "expected math.sh --contains--> add()"
    );
    // Same-file call: double -> add.
    assert!(
        has("double()", "calls", "add()"),
        "expected double --calls--> add"
    );
    // Cross-file call: boot (app/main.sh) -> add (util/math.sh), unique global.
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );

    // `source "../util/math.sh"` resolves to a corpus file (imports_from).
    assert!(
        has("main.sh", "imports_from", "math.sh"),
        "expected main.sh --imports_from--> math.sh"
    );

    // `. /etc/profile` is an unresolved (external) include.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports > 0,
        "no external import edges (expected /etc/profile)"
    );
}
