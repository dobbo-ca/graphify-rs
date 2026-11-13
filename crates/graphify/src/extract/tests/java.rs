//! graphify-go `internal/extract/java_test.go`.

use std::collections::{BTreeMap, BTreeSet};

use super::extract_java;
use crate::extract::resolve;
use crate::testutil::testdata;

#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_extract_java() {
    crate::extract::kinds::register_kinds();
    let root = testdata().join("extract").join("javaproj");
    let files = ["util/Math.java", "web/Server.java"];

    // Dispatch for .java is not wired into File() yet, so call extractJava
    // directly. Reading + extracting here mirrors what File() will do once wired.
    let mut results = Vec::new();
    for f in files {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
        results.push(extract_java(f, &src).expect("parse budget"));
    }
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "Math.java",
        "Server.java",
        "Math",
        "Math.add()",
        "Server",
        "Server.start()",
        "Server.boot()",
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
    // Method scoped under its type, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "Server.boot()"),
        "expected Server.start --calls--> Server.boot"
    );
    // Cross-file call: Server.boot -> add (unique global definition in Math).
    assert!(
        has("Server.boot()", "calls", "Math.add()"),
        "expected cross-file Server.boot --calls--> Math.add"
    );

    // External import edge for util.Math.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports != 0,
        "no external import edges (expected util.Math)"
    );
}

// TestExtractJavaEnum checks that enum constants become nodes linked to the enum
// type by a `case_of` edge, and that a constant's anonymous class body still
// contributes its methods (attached to the constant).
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_extract_java_enum() {
    crate::extract::kinds::register_kinds();
    let src = b"enum Color {
  RED,
  GREEN {
    void greet() {}
  };
}";
    let res = extract_java("Color.java", src).expect("parse budget");

    let id2label: BTreeMap<&str, &str> = res
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let labels: BTreeSet<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in ["Color", "RED", "GREEN", "GREEN.greet()"] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        res.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };
    assert!(
        has("Color", "case_of", "RED"),
        "expected Color --case_of--> RED"
    );
    assert!(
        has("Color", "case_of", "GREEN"),
        "expected Color --case_of--> GREEN"
    );
    // Anonymous-body method attaches to the constant, not the enum type.
    assert!(
        has("GREEN", "contains", "GREEN.greet()"),
        "expected GREEN --contains--> GREEN.greet"
    );
}
