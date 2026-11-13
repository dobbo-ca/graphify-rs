//! Port of `analyze_test.go` (10 tests), then the upstream cases of `test_analyze.py` and
//! `test_god_nodes_exclude_hubs.py` that hold for graphify-go v0.12.0.

use std::collections::BTreeSet;

use super::{communities, edge, node, node_ft};
use crate::analyze::analyze::{is_concept_node, is_file_node, is_json_key_node};
use crate::analyze::{GodNode, god_nodes, import_cycles, surprising};
use crate::model::{Confidence, Graph, Relation, conf, rel};

use crate::cluster::tests::gorand::GoRand;

/// Go `sampleGraph`: two files (a.go, b.go) each with three functions, file hubs that "contain"
/// their functions, a cross-file inferred call a1->b1, and a mutual file-level import cycle
/// a.go <-> b.go.
fn sample_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(node("fa", "a.go", "pkg/a.go"));
    g.add_node(node("fb", "b.go", "pkg/b.go"));
    for id in ["a1", "a2", "a3"] {
        g.add_node(node(id, &format!("{id}()"), "pkg/a.go"));
    }
    for id in ["b1", "b2", "b3"] {
        g.add_node(node(id, &format!("{id}()"), "pkg/b.go"));
    }

    for id in ["a1", "a2", "a3"] {
        g.add_edge(edge("fa", id, Relation::CONTAINS, Confidence::EXTRACTED));
    }
    for id in ["b1", "b2", "b3"] {
        g.add_edge(edge("fb", id, Relation::CONTAINS, Confidence::EXTRACTED));
    }

    let calls = |g: &mut Graph, s: &str, t: &str, c: &str| {
        g.add_edge(edge(s, t, Relation::CALLS, conf(c)));
    };
    calls(&mut g, "a1", "a2", "EXTRACTED");
    calls(&mut g, "a1", "a3", "EXTRACTED");
    calls(&mut g, "b1", "b2", "EXTRACTED");
    calls(&mut g, "b1", "b3", "EXTRACTED");
    calls(&mut g, "a1", "b1", "INFERRED"); // cross-file: surprising

    // File-level import cycle.
    g.add_edge(edge(
        "fa",
        "fb",
        Relation::IMPORTS_FROM,
        Confidence::EXTRACTED,
    ));
    g.add_edge(edge(
        "fb",
        "fa",
        Relation::IMPORTS_FROM,
        Confidence::EXTRACTED,
    ));
    g
}

#[test]
fn test_god_nodes_exclude_file_hubs() {
    let g = sample_graph();
    let gods = god_nodes(&g, 10, 0);
    assert!(!gods.is_empty(), "expected at least one god node");
    for n in &gods {
        assert!(
            n.label != "a.go" && n.label != "b.go",
            "god nodes should exclude file hubs, got {:?}",
            n.label
        );
    }
    // Results are degree-sorted descending.
    for w in gods.windows(2) {
        assert!(
            w[0].degree >= w[1].degree,
            "god nodes not sorted by descending degree: {} before {}",
            w[0].degree,
            w[1].degree
        );
    }
    let got = god_nodes(&g, 2, 0);
    assert_eq!(
        got.len(),
        2,
        "GodNodes(topN=2) returned {}, want 2",
        got.len()
    );
}

#[test]
fn test_surprising_finds_cross_file_call() {
    let g = sample_graph();
    let comm = communities(&[
        (0, &["a1", "a2", "a3", "fa"]),
        (1, &["b1", "b2", "b3", "fb"]),
    ]);
    let surprises = surprising(&g, &comm, 5);
    let mut found = false;
    for s in &surprises {
        if s.source == "a1()" && s.target == "b1()" && s.relation == "calls" {
            found = true;
            assert!(
                !s.note.is_empty(),
                "expected a community-bridging note on a1->b1"
            );
        }
        assert!(
            s.source_files[0] != s.source_files[1],
            "surprising connection should be cross-file, got {:?}",
            s.source_files
        );
    }
    assert!(found, "expected surprising connection a1() --calls--> b1()");
}

#[test]
fn test_import_cycles_detects_mutual_import() {
    let g = sample_graph();
    let cycles = import_cycles(&g, 5, 20);
    assert_eq!(
        cycles.len(),
        1,
        "expected exactly 1 cycle, got {}: {cycles:?}",
        cycles.len()
    );
    assert_eq!(
        cycles[0].files.len(),
        2,
        "expected a 2-file cycle, got {:?}",
        cycles[0].files
    );
    let files: BTreeSet<&str> = cycles[0].files.iter().map(String::as_str).collect();
    assert!(
        files.contains("pkg/a.go") && files.contains("pkg/b.go"),
        "cycle should contain both files, got {:?}",
        cycles[0].files
    );
}

#[test]
fn test_import_cycles_none_when_acyclic() {
    let mut g = Graph::new();
    g.add_node(node("fa", "a.go", "pkg/a.go"));
    g.add_node(node("fb", "b.go", "pkg/b.go"));
    g.add_edge(edge(
        "fa",
        "fb",
        Relation::IMPORTS_FROM,
        Confidence::EXTRACTED,
    ));
    let cycles = import_cycles(&g, 5, 20);
    assert!(
        cycles.is_empty(),
        "expected no cycles in acyclic graph, got {cycles:?}"
    );
}

/// Go `semanticGraph`: two prose notes that both link to one LLM-derived concept hub (file_type
/// semantic_concept, no source file), plus a second concept. The concept hub should surface as a
/// god node, and a note->concept edge should be eligible as a surprising connection.
fn semantic_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(node_ft("note_a", "Note A", "document", "learnings/a.md"));
    g.add_node(node_ft("note_b", "Note B", "document", "learnings/b.md"));
    g.add_node(node_ft(
        "concept_appconfig",
        "AppConfig",
        "semantic_concept",
        "",
    ));
    g.add_node(node_ft("concept_flux", "Flux", "semantic_concept", ""));
    let sem = |g: &mut Graph, s: &str, t: &str| {
        let mut e = edge(s, t, Relation::CITES, Confidence::INFERRED);
        e.confidence_score = 0.8;
        g.add_edge(e);
    };
    sem(&mut g, "note_a", "concept_appconfig");
    sem(&mut g, "note_b", "concept_appconfig");
    sem(&mut g, "note_a", "concept_flux");
    g
}

#[test]
fn test_god_nodes_includes_semantic_concepts() {
    let gods = god_nodes(&semantic_graph(), 10, 0);
    let mut found = false;
    for n in &gods {
        if n.id == "concept_appconfig" {
            found = true;
            assert_eq!(
                n.degree, 2,
                "AppConfig concept degree = {}, want 2",
                n.degree
            );
        }
    }
    assert!(
        found,
        "semantic concept hub not surfaced as a god node; got {gods:?}"
    );
}

#[test]
fn test_surprising_includes_note_to_concept() {
    let g = semantic_graph();
    // Each note in its own community so a note->concept edge bridges them.
    let comm = communities(&[
        (0, &["note_a", "concept_flux"]),
        (1, &["note_b"]),
        (2, &["concept_appconfig"]),
    ]);
    let s = surprising(&g, &comm, 5);
    assert!(
        !s.is_empty(),
        "no surprising connections surfaced for a semantically enriched graph"
    );
}

/// Go `fileHubNoteGraph`: mirrors the wiki corpus: prose-note nodes whose label is the file
/// basename (so they look like file hubs) link to a shared concept. The note->concept semantic
/// edge is the insight and must still surface as a surprising connection even though the note is
/// a "file hub".
fn file_hub_note_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(node_ft("na", "a.md", "document", "learnings/a.md"));
    g.add_node(node_ft("nb", "b.md", "document", "learnings/b.md"));
    g.add_node(node_ft("c1", "Shared Concept", "semantic_concept", ""));
    for s in ["na", "nb"] {
        let mut e = edge(
            s,
            "c1",
            rel("conceptually_related_to"),
            Confidence::INFERRED,
        );
        e.confidence_score = 0.9;
        g.add_edge(e);
    }
    g
}

#[test]
fn test_surprising_includes_semantic_edge_from_file_hub_note() {
    let g = file_hub_note_graph();
    let comm = communities(&[(0, &["na"]), (1, &["nb", "c1"])]);
    let s = surprising(&g, &comm, 5);
    assert!(
        !s.is_empty(),
        "note->concept semantic edge not surfaced as surprising"
    );
}

#[test]
fn test_god_nodes_exclude_noise_labels() {
    let mut g = sample_graph();
    // A mechanical hub out-degreeing every real entity.
    g.add_node(node("dep", "dependencies", "package.json"));
    for id in ["a1", "a2", "a3", "b1", "b2", "b3"] {
        g.add_edge(edge("dep", id, Relation::CALLS, Confidence::EXTRACTED));
    }
    for n in god_nodes(&g, 10, 0) {
        assert!(
            n.label != "dependencies",
            "god nodes should exclude noise label {:?}",
            n.label
        );
    }
    // A JSON key that isn't generic noise still ranks.
    g.add_node(node("sched", "scheduler", "package.json"));
    g.add_edge(edge("sched", "a1", Relation::CALLS, Confidence::EXTRACTED));
    let found = god_nodes(&g, 20, 0).iter().any(|n| n.label == "scheduler");
    assert!(found, "non-noise JSON key should still rank as a god node");
}

/// Go `hubGraph`: one mega-hub (degree 40), two mid symbols, and a tail of leaves.
fn hub_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(node("hub", "Registry", "u.go"));
    for i in 0..40 {
        let id = format!("leaf{i}");
        g.add_node(node(&id, &id, &format!("l{i}.go")));
        g.add_edge(edge("hub", &id, Relation::CALLS, Confidence::EXTRACTED));
    }
    for (name, deg) in [("core", 6), ("svc", 4)] {
        g.add_node(node(name, name, &format!("{name}.go")));
        for i in 0..deg {
            g.add_edge(edge(
                name,
                &format!("leaf{i}"),
                Relation::CALLS,
                Confidence::EXTRACTED,
            ));
        }
    }
    g
}

#[test]
fn test_god_nodes_exclude_hubs_percentile() {
    let g = hub_graph();

    // Zero keeps the historical ranking: the hub still wins.
    let got = god_nodes(&g, 3, 0);
    assert!(
        !got.is_empty() && got[0].label == "Registry",
        "GodNodes(topN=3, pct=0) = {got:?}, want Registry first"
    );

    // The threshold must match cluster()'s formula: degrees sorted ascending,
    // idx = max(0, len*pct/100 - 1), anything with a greater degree is a hub.
    let mut degrees: Vec<usize> = g.node_ids().iter().map(|id| g.degree(id)).collect();
    degrees.sort();
    let threshold = degrees[degrees.len() * 90 / 100 - 1] as i64;

    let gods = god_nodes(&g, 100, 90);
    assert!(
        !gods.is_empty(),
        "suppressing the hub must not empty the ranking"
    );
    for n in &gods {
        assert!(
            n.degree <= threshold,
            "god node {:?} has degree {}, above the p90 threshold {threshold}",
            n.label,
            n.degree
        );
    }

    // 100 excludes nothing: the threshold is the maximum degree.
    let got = god_nodes(&g, 3, 100);
    assert!(
        !got.is_empty() && got[0].label == "Registry",
        "GodNodes(topN=3, pct=100) = {got:?}, want Registry first"
    );
}

/// Go `godNodesBySort`: GodNodes as it was, a stable sort of every node by degree.
fn god_nodes_by_sort(g: &Graph, top_n: i64, pct: i64) -> Vec<GodNode> {
    let mut ids: Vec<String> = g.node_ids().to_vec();
    let hub_threshold = if pct > 0 && !ids.is_empty() {
        let mut degrees: Vec<usize> = ids.iter().map(|id| g.degree(id)).collect();
        degrees.sort();
        let len = degrees.len() as i64;
        let idx = (len * pct / 100 - 1).clamp(0, len - 1);
        Some(degrees[idx as usize])
    } else {
        None
    };
    ids.sort_by_key(|a| std::cmp::Reverse(g.degree(a)));
    let mut out = Vec::new();
    for id in &ids {
        if hub_threshold.is_some_and(|t| g.degree(id) > t) {
            continue;
        }
        if is_file_node(g, id) || is_concept_node(g, id) || is_json_key_node(g, id) {
            continue;
        }
        out.push(GodNode {
            id: id.clone(),
            label: g.node(id).map(|n| n.label.clone()).unwrap_or_default(),
            degree: g.degree(id) as i64,
        });
        if out.len() as i64 >= top_n {
            break;
        }
    }
    out
}

#[test]
fn test_god_nodes_matches_stable_sort() {
    let mut rng = GoRand::new(1);
    for run in 0..300 {
        let mut g = Graph::new();
        let n = 1 + rng.intn(60);
        for i in 0..n {
            let mut nd = node(
                &format!("n{:02}", rng.intn(1000) * 100 + i),
                &format!("L{i}"),
                "a/x.go",
            );
            match rng.intn(6) {
                0 => nd.label = "x.go".to_string(), // file hub
                1 => nd.source_file.clear(),        // concept
                2 => nd.label.push_str("()"),
                _ => {}
            }
            g.add_node(nd);
        }
        let ids: Vec<String> = g.node_ids().to_vec();
        for _ in 0..rng.intn(3 * n) {
            g.add_edge(edge(
                &ids[rng.intn(n)],
                &ids[rng.intn(n)],
                Relation::CALLS,
                Confidence::EXTRACTED,
            ));
        }
        for top_n in [0, 1, 3, 10, 100] {
            for pct in [0, 50, 90] {
                let got = god_nodes(&g, top_n, pct);
                let want = god_nodes_by_sort(&g, top_n, pct);
                assert_eq!(
                    got, want,
                    "run {run} topN={top_n} pct={pct}:\n got {got:?}\nwant {want:?}"
                );
            }
        }
    }
}

// ---- Upstream: test_analyze.py (the cases that hold for graphify-go v0.12.0) ----

#[test]
fn test_is_concept_node_empty_source() {
    let mut g = Graph::new();
    g.add_node(node("c1", "", ""));
    assert!(is_concept_node(&g, "c1"));
}

#[test]
fn test_is_concept_node_real_file() {
    let mut g = Graph::new();
    g.add_node(node("n1", "", "model.py"));
    assert!(!is_concept_node(&g, "n1"));
}

#[test]
fn test_is_json_key_node_noise_label() {
    let mut g = Graph::new();
    g.add_node(node("j1", "name", "schema.json"));
    assert!(is_json_key_node(&g, "j1"));
}

#[test]
fn test_is_json_key_node_non_json_file() {
    let mut g = Graph::new();
    g.add_node(node("n1", "name", "model.py"));
    assert!(!is_json_key_node(&g, "n1"));
}

#[test]
fn test_is_json_key_node_real_label() {
    let mut g = Graph::new();
    g.add_node(node("j2", "UserProfile", "schema.json"));
    assert!(!is_json_key_node(&g, "j2"));
}

/// npm dep-block keys of a package.json are filtered from the god nodes even at the highest
/// degree, and the real-domain node stays.
#[test]
fn test_god_nodes_excludes_npm_dep_block_keys() {
    const DEP_KEYS: [&str; 5] = [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
        "bundledDependencies",
    ];
    for dep_key in DEP_KEYS {
        let mut g = Graph::new();
        // Real-domain node with a realistic source file.
        g.add_node(node_ft("real_node", "AuthService", "code", "src/auth.py"));
        // npm dep-block key node, sourced from a JSON file so is_json_key_node fires.
        g.add_node(node_ft(
            "dep_node",
            dep_key,
            "code",
            "frontend/package.json",
        ));
        // Enough edges that dep_node would be a god node without the filter.
        for i in 0..20 {
            let peer = format!("pkg_{i}");
            g.add_node(node_ft(
                &peer,
                &format!("package-{i}"),
                "code",
                "frontend/package.json",
            ));
            g.add_edge(edge(
                "dep_node",
                &peer,
                Relation::CONTAINS,
                Confidence::EXTRACTED,
            ));
        }
        // Give real_node an edge too.
        g.add_edge(edge(
            "real_node",
            "dep_node",
            Relation::IMPORTS,
            Confidence::EXTRACTED,
        ));

        let result = god_nodes(&g, 10, 0);
        let ids: Vec<&str> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(
            !ids.contains(&"dep_node"),
            "{dep_key}: god nodes should filter the npm dep-block key, got {result:?}"
        );
        assert!(
            ids.contains(&"real_node"),
            "{dep_key}: god nodes should include AuthService, got {result:?}"
        );
    }
}

/// god nodes must not return generic JSON key nodes like `name` or `id`.
#[test]
fn test_god_nodes_excludes_json_noise() {
    let mut g = Graph::new();
    g.add_node(node("real", "AuthService", "src/auth.py"));
    // A noisy JSON key node with high degree.
    g.add_node(node("json_name", "name", "schema.json"));
    for i in 0..8 {
        let n = format!("peer{i}");
        g.add_node(node(&n, &format!("Peer{i}"), &format!("src/peer{i}.py")));
        g.add_edge(edge(
            "json_name",
            &n,
            Relation::default(),
            Confidence::default(),
        ));
        g.add_edge(edge("real", &n, Relation::default(), Confidence::default()));
    }
    let result = god_nodes(&g, 10, 0);
    let labels: Vec<&str> = result.iter().map(|r| r.label.as_str()).collect();
    assert!(!labels.contains(&"name"), "labels = {labels:?}");
    assert!(labels.contains(&"AuthService"), "labels = {labels:?}");
}

/// The JSON-key filter matches regardless of label casing.
#[test]
fn test_god_nodes_filter_is_case_insensitive() {
    let mut g = Graph::new();
    g.add_node(node("real", "RealAbstraction", "libs/real.py"));
    for i in 0..3 {
        let p = format!("peer{i}");
        g.add_node(node(&p, &format!("P{i}"), &format!("src/p{i}.py")));
        g.add_edge(edge("real", &p, Relation::default(), Confidence::default()));
    }
    for variant in ["Start", "START", "Name", "ID"] {
        let nid = format!("json_{}", variant.to_ascii_lowercase());
        g.add_node(node(&nid, variant, "testhelpers/data.json"));
        for i in 0..15 {
            let t = format!("{nid}_t{i}");
            g.add_node(node(&t, &format!("X{i}"), "testhelpers/data.json"));
            g.add_edge(edge(&t, &nid, Relation::default(), Confidence::default()));
        }
    }
    let result = god_nodes(&g, 10, 0);
    let labels: Vec<&str> = result.iter().map(|r| r.label.as_str()).collect();
    for variant in ["Start", "START", "Name", "ID"] {
        assert!(
            !labels.contains(&variant),
            "`{variant}` should be filtered as JSON-key noise"
        );
    }
}

// ---- Upstream: test_god_nodes_exclude_hubs.py ----

/// With `exclude_hubs_percentile=90` and `top_n=10` the hub is suppressed and the ranking is not
/// emptied (the other tests of the file are `test_god_nodes_exclude_hubs_percentile`).
#[test]
fn test_the_percentile_suppresses_the_hub() {
    let gods = god_nodes(&hub_graph(), 10, 90);
    let labels: Vec<&str> = gods.iter().map(|g| g.label.as_str()).collect();
    assert!(!labels.contains(&"Registry"), "labels = {labels:?}");
    assert!(
        !gods.is_empty(),
        "suppressing the hub must not empty the ranking"
    );
    assert!(gods.iter().all(|g| g.degree < 40), "gods = {gods:?}");
}
