//! graphify-go `internal/extract/python_import_guided_test.go`.

use crate::extract::{file_from_bytes, resolve};
use crate::model::{Extraction, Relation};

// resolvePy extracts each in-memory Python file and stitches them with Resolve.
pub(super) fn resolve_py(srcs: &[(&str, &str)]) -> Extraction {
    crate::extract::kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(f, s)| file_from_bytes(f, s.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(f, _)| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

// callConfidence returns the confidence of a src->tgt calls edge, by node label.
fn call_confidence(ext: &Extraction, src_label: &str, tgt_label: &str) -> Option<&'static str> {
    let label = |id: &str| {
        ext.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.label.as_str())
            .unwrap_or("")
    };
    ext.edges
        .iter()
        .find(|e| {
            e.relation == Relation::CALLS
                && label(&e.source) == src_label
                && label(&e.target) == tgt_label
        })
        .map(|e| e.confidence.as_str())
}

// countCalls returns the number of calls edges in an extraction.
fn count_calls(ext: &Extraction) -> usize {
    ext.edges
        .iter()
        .filter(|e| e.relation == Relation::CALLS)
        .count()
}

// TestImportGuidedResolvesAmbiguousAlias checks that `from M import N as L`
// evidence resolves an otherwise-ambiguous bare call L() to the unique
// (module_stem, N) definition with EXTRACTED confidence — a case the generic
// name pass cannot resolve (the name N is defined in two files).
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_import_guided_resolves_ambiguous_alias() {
    let ext = resolve_py(&[
        ("helper.py", "def transform(x):\n    return x\n"),
        // decoy: same name, different module
        ("a/dup.py", "def transform(y):\n    return y\n"),
        (
            "main.py",
            "from helper import transform as tx\n\n\ndef run():\n    return tx(1)\n",
        ),
    ]);

    let conf = call_confidence(&ext, "run()", "transform()")
        .expect("expected run --calls--> transform via import evidence");
    assert_eq!(
        conf, "EXTRACTED",
        "import-guided edge confidence = {conf:?}, want EXTRACTED"
    );
    // The decoy a/dup.py:transform shares the bare name, so the generic name
    // pass could not have resolved this; exactly one calls edge must exist.
    let got = count_calls(&ext);
    assert_eq!(
        got, 1,
        "calls edge count = {got}, want 1 (ambiguous name resolved only by import)"
    );
}

// TestImportGuidedSkipsMemberCall checks that a member call obj.tx() never
// resolves via import evidence even though tx is an imported alias.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_import_guided_skips_member_call() {
    let ext = resolve_py(&[
        ("helper.py", "def transform(x):\n    return x\n"),
        ("a/dup.py", "def transform(y):\n    return y\n"),
        (
            "main.py",
            "from helper import transform as tx\n\n\ndef run(obj):\n    return obj.tx(1)\n",
        ),
    ]);
    assert!(
        call_confidence(&ext, "run()", "transform()").is_none(),
        "member call obj.tx() must not resolve via import evidence"
    );
}

// TestImportGuidedSkipsSelfEdge checks that import evidence never emits a
// self-edge when the imported symbol resolves back to the caller.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_import_guided_skips_self_edge() {
    // run imports itself by name from its own module; the unique (main, run)
    // def is run itself, so no edge may be emitted.
    let ext = resolve_py(&[(
        "main.py",
        "from main import run\n\n\ndef run():\n    return run()\n",
    )]);
    for e in &ext.edges {
        assert!(
            !(e.relation == Relation::CALLS && e.source == e.target),
            "unexpected self calls edge {}->{}",
            e.source,
            e.target
        );
    }
}

// TestImportGuidedNonAliased checks the plain `from M import N` form (local name
// equals imported name) resolves with EXTRACTED confidence.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_import_guided_non_aliased() {
    let ext = resolve_py(&[
        ("helper.py", "def transform(x):\n    return x\n"),
        ("a/dup.py", "def transform(y):\n    return y\n"),
        (
            "main.py",
            "from helper import transform\n\n\ndef run():\n    return transform(1)\n",
        ),
    ]);
    let conf = call_confidence(&ext, "run()", "transform()");
    assert_eq!(
        conf,
        Some("EXTRACTED"),
        "plain `from helper import transform` should resolve EXTRACTED, got {conf:?}"
    );
}
