//! graphify-go `internal/extract/javascript_test.go`. (`javascript_alias_test.go` tests the `@/`
//! alias of `resolveModulePath`, `resolve.go:1877`, and is ported with the resolve tests,
//! graphify-rs-ihc.18.)

use std::collections::BTreeMap;

use crate::extract::{Result, file_from_bytes, resolve};
use crate::idutil::make_id;

// A `// NOTE:` comment becomes a rationale node with an edge to the file, and an
// `ADR-0001` reference in a comment becomes a normalized doc_ref node cited by
// the file.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_extract_js_rationale() {
    crate::extract::kinds::register_kinds();
    let src = concat!(
        "// NOTE: debounce to avoid thrash\n",
        "// See ADR-0001 for the retry policy.\n",
        "export const boot = () => {\n",
        "  return 1;\n",
        "};\n"
    );
    let res = file_from_bytes("web/app.js", src.as_bytes());

    let file_id = make_id(&["web/app.js"]);

    let mut label: BTreeMap<&str, &str> = BTreeMap::new();
    let mut ftype: BTreeMap<&str, &str> = BTreeMap::new();
    for n in &res.nodes {
        label.insert(&n.id, &n.label);
        ftype.insert(&n.id, &n.file_type);
    }
    let label_of = |id: &str| label.get(id).copied().unwrap_or("");
    let ftype_of = |id: &str| ftype.get(id).copied().unwrap_or("");

    // NOTE comment -> rationale node with a rationale_for edge to the file.
    let rationale_ok = res.edges.iter().any(|e| {
        e.relation.as_str() == "rationale_for"
            && e.target == file_id
            && ftype_of(&e.source) == "rationale"
            && label_of(&e.source).starts_with("NOTE: debounce")
    });
    assert!(
        rationale_ok,
        "expected rationale_for edge from NOTE comment to file {file_id}"
    );

    // ADR-0001 comment -> normalized doc_ref node + cites edge from the file.
    let doc_ref_id = make_id(&["docref", "ADR-0001"]);
    assert!(
        ftype_of(&doc_ref_id) == "doc_ref" && label_of(&doc_ref_id) == "ADR-0001",
        "expected doc_ref node {doc_ref_id} labeled ADR-0001, got type={:?} label={:?}",
        ftype_of(&doc_ref_id),
        label_of(&doc_ref_id)
    );
    let cites_ok = res
        .edges
        .iter()
        .any(|e| e.relation.as_str() == "cites" && e.source == file_id && e.target == doc_ref_id);
    assert!(cites_ok, "expected cites edge {file_id} -> {doc_ref_id}");
}

/// Go `importEdges` (`local_imports_test.go:11`) with no go.mod table: extracts each source,
/// resolves the corpus, and returns the sorted import edges.
fn import_edges(srcs: &[(&str, &str)]) -> Vec<String> {
    let mut results = vec![Result::default()];
    let mut files: Vec<String> = Vec::new();
    for (f, s) in srcs {
        results.push(file_from_bytes(f, s.as_bytes()));
        files.push((*f).to_string());
    }
    files.sort();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut edges: Vec<String> = resolve(&results, &files, workers)
        .edges
        .iter()
        .filter(|e| matches!(e.relation.as_str(), "imports" | "imports_from"))
        .map(|e| format!("{} {} {}", e.source, e.relation.as_str(), e.target))
        .collect();
    edges.sort();
    edges
}

// CommonJS require and re-exports each link to the target file.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_import_forms_resolve_to_files() {
    crate::extract::kinds::register_kinds();
    const CASES: [(&str, &str); 4] = [
        ("require", "const lib = require('./lib');\n"),
        (
            "nested require",
            "function f() {\n  return require(\"./lib\").x;\n}\n",
        ),
        ("export star", "export * from './lib';\n"),
        ("export named", "export { thing } from './lib';\n"),
    ];
    for (name, src) in CASES {
        let got = import_edges(&[
            ("src/a.ts", src),
            ("src/lib.ts", "export const thing = 1;\n"),
        ]);
        let want = vec!["src_a_ts imports_from src_lib_ts".to_string()];
        assert_eq!(got, want, "{name}");
    }
    // Not imports: a computed spec, a method named require, a plain export.
    let src = "const a = require(name);\nconst b = obj.require('./lib');\nexport { b };\n";
    let got = import_edges(&[("src/a.js", src), ("src/lib.js", "")]);
    assert!(got.is_empty(), "non-imports: got {got:?}, want none");
}

// `export type { T } from` is type-only, like `import type`.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_re_export_type_only() {
    crate::extract::kinds::register_kinds();
    const CASES: [(&str, bool); 2] = [
        ("export type { T } from './lib';\n", true),
        ("export { T } from './lib';\n", false),
    ];
    for (src, want) in CASES {
        let imps = file_from_bytes("a.ts", src.as_bytes()).imps;
        assert!(
            imps.len() == 1 && imps[0].type_only == want,
            "{src:?}: got {imps:?}, want one import with TypeOnly={want}"
        );
    }
}
