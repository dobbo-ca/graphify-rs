//! Shared test helpers of the extract tests: Go's `extractAll` and `wantEdges`
//! (`inherits_test.go:6-46`), used by the inherits, java_chain, python and resolve tests. Owner:
//! the extractors-and-CLI stream. Use as `crate::extract::tests::util::{extract_all, want_edges}`.

use crate::extract::{file_from_bytes, resolve};

/// One resolved edge as labels: (source label, relation, target label).
pub(crate) type LabelEdge = (String, String, String);

/// Go `extractAll`: extracts every `(rel, src)`, resolves them together, and returns the edges
/// by node label. (Go ranges a map, so its file order is random; the edges asserted on do not
/// depend on it.)
pub(crate) fn extract_all(srcs: &[(&str, &str)]) -> Vec<LabelEdge> {
    let results: Vec<_> = srcs
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    let label = |id: &str| {
        ext.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.label.clone())
            .unwrap_or_default()
    };
    ext.edges
        .iter()
        .map(|e| {
            (
                label(&e.source),
                e.relation.as_str().to_string(),
                label(&e.target),
            )
        })
        .collect()
}

/// Go `wantEdges`: every `want` edge is there and no `unwanted` one is.
pub(crate) fn want_edges(srcs: &[(&str, &str)], want: &[[&str; 3]], unwanted: &[[&str; 3]]) {
    let got = extract_all(srcs);
    let has = |w: &[&str; 3]| {
        got.iter()
            .any(|(s, r, t)| s == w[0] && r == w[1] && t == w[2])
    };
    for w in want {
        assert!(
            has(w),
            "missing edge {} --{}--> {}; got {got:?}",
            w[0],
            w[1],
            w[2]
        );
    }
    for w in unwanted {
        assert!(!has(w), "unexpected edge {} --{}--> {}", w[0], w[1], w[2]);
    }
}
