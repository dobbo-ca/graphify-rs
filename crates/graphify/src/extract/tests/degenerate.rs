//! graphify-go `internal/extract/degenerate_test.go`.

use std::collections::BTreeSet;

use crate::extract::{Result, file, kinds};
use crate::testutil::TempDir;

/// Go `labelSet` (`rake_skill_test.go:9`).
fn label_set(r: &Result) -> BTreeSet<&str> {
    r.nodes.iter().map(|n| n.label.as_str()).collect()
}

/// A JS symbol whose name normalizes to an empty ID (a minified `$`) must not produce a node; it
/// would collapse to the bare file stem and be pure noise (#1899). A real function alongside it
/// must still be extracted.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_extract_js_skips_degenerate_name() {
    kinds::register_kinds();
    let root = TempDir::new();
    root.write(
        "vendor.js",
        b"function $(){ return 1 }\nfunction real(){ return 2 }\n",
    );
    let r = file(root.path(), "vendor.js").unwrap_or_else(|e| panic!("File(vendor.js): {e}"));
    let labels = label_set(&r);
    assert!(
        !labels.contains("$()"),
        "degenerate `$` function must not produce a node, got labels {labels:?}"
    );
    assert!(
        labels.contains("real()"),
        "real function should still be extracted, got {labels:?}"
    );
}

/// A JSONC `"//"` comment key normalizes to an empty ID and must not produce a node; real config
/// keys alongside it must still be extracted (#1899).
#[test]
fn test_extract_json_skips_comment_key() {
    kinds::register_kinds();
    let root = TempDir::new();
    root.write(
        "tsconfig.json",
        br#"{"//": "a note", "compilerOptions": {"strict": true}}"#,
    );
    let r =
        file(root.path(), "tsconfig.json").unwrap_or_else(|e| panic!("File(tsconfig.json): {e}"));
    let labels = label_set(&r);
    assert!(
        !labels.contains("//"),
        "JSONC `//` comment key must not produce a node, got labels {labels:?}"
    );
    assert!(
        labels.contains("compilerOptions"),
        "real config key should still be extracted, got {labels:?}"
    );
}
