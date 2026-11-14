//! Shared test helpers of the extract tests: Go's `extractAll` and `wantEdges`
//! (`inherits_test.go:6-46`), used by the inherits, java_chain, python and resolve tests. Owner:
//! the extractors-and-CLI stream. Use as `crate::extract::tests::util::{extract_all, want_edges}`.

use std::collections::{BTreeMap, BTreeSet};

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

/// `file_from_bytes` after `register_kinds`, as every test that parses must (plan hazard
/// "KindMask registration order").
pub(crate) fn extract_bytes(rel: &str, src: &[u8]) -> crate::extract::Result {
    crate::extract::kinds::register_kinds();
    file_from_bytes(rel, src)
}

/// Go `resolveSrc` (`nodeid_test.go:6`): extracts each in-memory file, resolves the set, and
/// returns the nodes as `[source file, label]` and the edges as `[source file:label, relation,
/// target file:label]`.
pub(crate) fn resolve_src(srcs: &[(&str, &str)]) -> (BTreeSet<[String; 2]>, BTreeSet<[String; 3]>) {
    crate::extract::kinds::register_kinds();
    let mut results = Vec::new();
    let mut files = Vec::new();
    for (rel, src) in srcs {
        // "go.mod" names the module of the corpus root.
        if *rel == "go.mod" {
            results.push(crate::extract::Result {
                go_mods: BTreeMap::from([(".".to_string(), (*src).to_string())]),
                ..crate::extract::Result::default()
            });
            continue;
        }
        results.push(file_from_bytes(rel, src.as_bytes()));
        files.push((*rel).to_string());
    }
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    let mut nodes = BTreeSet::new();
    let mut at: BTreeMap<&str, [String; 2]> = BTreeMap::new(); // id -> source file, label
    for n in &ext.nodes {
        let k = [n.source_file.clone(), n.label.clone()];
        nodes.insert(k.clone());
        at.insert(n.id.as_str(), k);
    }
    let none = [String::new(), String::new()];
    let mut edges = BTreeSet::new();
    for e in &ext.edges {
        let s = at.get(e.source.as_str()).unwrap_or(&none);
        let d = at.get(e.target.as_str()).unwrap_or(&none);
        edges.insert([
            format!("{}:{}", s[0], s[1]),
            e.relation.as_str().to_string(),
            format!("{}:{}", d[0], d[1]),
        ]);
    }
    (nodes, edges)
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
