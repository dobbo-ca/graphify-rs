//! graphify-go `internal/extract/dispatch_test.go`.

use std::collections::BTreeSet;

use crate::extract::tests::util::extract_bytes;

/// A .mts file routes through the TypeScript extractor: the TS-only `interface` and
/// type-annotated function parse (the plain JS grammar would not), and their definitions surface
/// as nodes.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_file_from_bytes_mts_as_type_script() {
    let src = concat!(
        "export interface Widget { id: number; }\n",
        "export function build(w: Widget): string {\n",
        "  return w.id.toString();\n",
        "}\n"
    );
    let res = extract_bytes("web/util.mts", src.as_bytes());

    let labels: BTreeSet<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in ["Widget", "build()"] {
        assert!(
            labels.contains(want),
            "missing node label {want:?} (mts not extracted as TypeScript): {labels:?}"
        );
    }
}

/// An extensionless script whose shebang names bash routes through the bash extractor: its
/// function definition surfaces as a `name()` node.
#[test]
#[ignore = "graphify-rs-ihc.35"]
fn test_file_from_bytes_shebang_bash() {
    let src = concat!("#!/usr/bin/env bash\n", "greet() {\n", "  echo hi\n", "}\n");
    let res = extract_bytes("scripts/deploy", src.as_bytes());

    assert!(
        res.nodes.iter().any(|n| n.label == "greet()"),
        "expected greet() node from extensionless bash shebang script"
    );
}
