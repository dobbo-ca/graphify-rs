//! graphify-go `internal/extract/zig_test.go`.

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
#[ignore = "graphify-rs-ihc.40"]
fn test_extract_zig() {
    let ext = extract_proj("zigproj", &["util/math.zig", "web/server.zig"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.zig",
        "server.zig",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
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
    // Method scoped under its container type, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "boot()"),
        "expected Server.start --calls--> boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );

    // External import edge for std (@import("std")).
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(imports > 0, "no external import edges (expected std)");
}
