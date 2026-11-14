//! Upstream graphify v0.9.107 `tests/test_type_only_import_cycles.py`, the five TypeScript stamp
//! cases (#3123): a whole-statement `import type` / `export type ... from` is type-only (Go
//! `jsTypeOnlyImport`, `javascript.go:400`); a mixed specifier list and a default binding named
//! `type` are runtime imports. The cycle cases of that file belong to analyze.

use crate::extract::{file_from_bytes, resolve};
use crate::model::{Edge, Extraction, Relation};

/// Upstream `_extract`: every file extracted and resolved together.
fn extract_ts(srcs: &[(&str, &str)]) -> Extraction {
    crate::extract::kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(f, s)| file_from_bytes(f, s.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(f, _)| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

/// The edges of `relations` whose source file ends in `importer`.
fn edges_of<'a>(ext: &'a Extraction, importer: &str, relations: &[Relation]) -> Vec<&'a Edge> {
    ext.edges
        .iter()
        .filter(|e| e.source_file.as_str().ends_with(importer) && relations.contains(&e.relation))
        .collect()
}

#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_import_type_statement_is_stamped() {
    let ext = extract_ts(&[
        (
            "types.ts",
            "export type Variables = { requestId: string };\n",
        ),
        (
            "v1.ts",
            "import type { Variables } from \"./types.js\";\nexport function makeRouter() { return {} as unknown; }\n",
        ),
    ]);
    let edges = edges_of(&ext, "v1.ts", &[Relation::IMPORTS_FROM]);
    assert!(
        !edges.is_empty() && edges.iter().all(|e| e.type_only()),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_export_type_reexport_is_stamped() {
    let ext = extract_ts(&[
        ("v1.ts", "export type RouterType = { v: number };\n"),
        ("types.ts", "export type { RouterType } from \"./v1.js\";\n"),
    ]);
    let edges = edges_of(
        &ext,
        "types.ts",
        &[Relation::IMPORTS_FROM, Relation::RE_EXPORTS],
    );
    assert!(
        !edges.is_empty() && edges.iter().all(|e| e.type_only()),
        "{edges:?}"
    );
}

/// `import type from './x.js'` imports a value whose name is `type`: erased by nothing.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_a_default_binding_named_type_is_a_runtime_import() {
    let ext = extract_ts(&[
        ("x.ts", "const type = 1;\nexport default type;\n"),
        (
            "user.ts",
            "import type from \"./x.js\";\nexport const y = type;\n",
        ),
    ]);
    let edges = edges_of(&ext, "user.ts", &[Relation::IMPORTS_FROM]);
    assert!(
        !edges.is_empty() && !edges.iter().any(|e| e.type_only()),
        "{edges:?}"
    );
}

/// `import { type B, C }` still imports C at runtime.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_a_mixed_specifier_list_stays_a_runtime_edge() {
    let ext = extract_ts(&[
        ("b.ts", "export type B = number;\nexport const C = 1;\n"),
        (
            "user.ts",
            "import { type B, C } from \"./b.js\";\nexport const y = C;\n",
        ),
    ]);
    let edges = edges_of(&ext, "user.ts", &[Relation::IMPORTS_FROM]);
    assert!(
        !edges.is_empty() && !edges.iter().any(|e| e.type_only()),
        "{edges:?}"
    );
}

/// Upstream checks that the key is absent; here the flag is a bit, so: not set.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_plain_imports_are_untouched() {
    let ext = extract_ts(&[
        ("b.ts", "export const C = 1;\n"),
        (
            "user.ts",
            "import { C } from \"./b.js\";\nexport const y = C;\n",
        ),
    ]);
    let edges = edges_of(&ext, "user.ts", &[Relation::IMPORTS_FROM]);
    assert!(
        !edges.is_empty() && !edges.iter().any(|e| e.type_only()),
        "{edges:?}"
    );
}
