//! Port of `semantic_test.go` (9 tests), then the upstream case of `test_chunking.py` that holds
//! for graphify-go v0.12.0.

use std::collections::BTreeSet;

use super::{FakeBackend, base_extraction, edge, node, notes};
use crate::model::{Confidence, Edge, File, Loc, Relation, file_of, line, rel};
use crate::semantic::{Cache, Config, MAX_NOTE_BYTES, Note, enrich};

#[test]
fn test_enrich_is_additive() {
    let base = base_extraction();
    let be = FakeBackend::with(|n: &Note| {
        (
            vec![node("appconfig", "AppConfig", "concept")],
            vec![Edge {
                source: n.id.clone(),
                target: "appconfig".to_string(),
                relation: rel("conceptually_related_to"),
                confidence: Confidence::INFERRED,
                confidence_score: 0.8,
                ..Edge::default()
            }],
        )
    });

    let cfg = Config { backend: Some(&be) };
    let (out, _) = enrich(&cfg, base.clone(), &notes(), Cache::new()).expect("Enrich");

    // Every deterministic node and edge must survive unchanged.
    assert!(
        out.nodes.len() >= base.nodes.len(),
        "nodes shrank: got {}, base had {}",
        out.nodes.len(),
        base.nodes.len()
    );
    for want in &base.edges {
        let found = out.edges.iter().any(|got| got == want);
        assert!(found, "deterministic edge {want:?} was mutated or dropped");
    }

    // The new semantic edge must be present.
    let sem = out
        .edges
        .iter()
        .filter(|e| e.relation == rel("conceptually_related_to"))
        .count();
    assert_eq!(sem, 1, "want 1 semantic edge, got {sem}");
}

#[test]
fn test_enrich_never_duplicates_a_concept_node() {
    let base = base_extraction(); // already has the appconfig concept node
    let be = FakeBackend::with(|_n: &Note| {
        // Backend re-emits an existing node id with a different label; the merge
        // must not overwrite or duplicate the deterministic node.
        (vec![node("appconfig", "DIFFERENT", "concept")], Vec::new())
    });
    let cfg = Config { backend: Some(&be) };
    let (out, _) = enrich(&cfg, base, &notes(), Cache::new()).expect("Enrich");
    let mut count = 0;
    for n in &out.nodes {
        if n.id == "appconfig" {
            count += 1;
            assert_eq!(
                n.label, "AppConfig",
                "deterministic node label overwritten: got {:?}",
                n.label
            );
        }
    }
    assert_eq!(
        count, 1,
        "appconfig node count = {count}, want 1 (no duplicate)"
    );
}

#[test]
fn test_enrich_cache_skips_unchanged_notes() {
    let base = base_extraction();
    let be = FakeBackend::with(|n: &Note| {
        (
            Vec::new(),
            vec![edge(
                &n.id,
                "appconfig",
                Relation::CITES,
                Confidence::INFERRED,
            )],
        )
    });
    let cfg = Config { backend: Some(&be) };

    // First run: cold cache, backend is called once and a cache is returned.
    let (_, c1) = enrich(&cfg, base.clone(), &notes(), Cache::new()).expect("first Enrich");
    assert_eq!(
        be.call_cnt(),
        1,
        "cold run: backend called {} times, want 1",
        be.call_cnt()
    );

    // Second run with the same content + warm cache: backend not called again.
    let (out2, _) = enrich(&cfg, base, &notes(), c1).expect("second Enrich");
    assert_eq!(
        be.call_cnt(),
        1,
        "warm run re-paid tokens: backend called {} times total, want 1",
        be.call_cnt()
    );
    // The cached edge must still appear in the output.
    let got = out2
        .edges
        .iter()
        .filter(|e| e.relation == Relation::CITES)
        .count();
    assert_eq!(
        got, 1,
        "cached semantic edge missing: got {got} cites edges"
    );
}

#[test]
fn test_enrich_cache_repays_changed_note() {
    let base = base_extraction();
    let be = FakeBackend::with(|_n: &Note| (Vec::new(), Vec::new()));
    let cfg = Config { backend: Some(&be) };
    let (_, c1) = enrich(&cfg, base.clone(), &notes(), Cache::new()).expect("Enrich");

    let mut changed = notes();
    changed[0].content = "completely different prose now".to_string();
    enrich(&cfg, base, &changed, c1).expect("Enrich");
    assert_eq!(
        be.call_cnt(),
        2,
        "changed note should re-pay tokens: backend called {} times, want 2",
        be.call_cnt()
    );
}

#[test]
fn test_enrich_skips_sensitive_and_oversized_notes() {
    let base = base_extraction();
    let be = FakeBackend::with(|_n: &Note| (Vec::new(), Vec::new()));
    let ns = vec![
        Note {
            id: "secret".to_string(),
            file: "secrets/creds.md".to_string(),
            content: "token=abc".to_string(),
        },
        Note {
            id: "huge".to_string(),
            file: "learnings/huge.md".to_string(),
            content: "x".repeat(MAX_NOTE_BYTES + 1),
        },
        Note {
            id: "ok".to_string(),
            file: "learnings/ok.md".to_string(),
            content: "fine".to_string(),
        },
    ];
    let cfg = Config { backend: Some(&be) };
    enrich(&cfg, base, &ns, Cache::new()).expect("Enrich");
    let calls = be.calls.borrow();
    assert!(
        calls.len() == 1 && calls[0] == "ok",
        "sensitive/oversized notes were sent to the backend: calls={calls:?}"
    );
}

#[test]
fn test_enrich_drops_edges_with_unknown_endpoints() {
    let base = base_extraction();
    let be = FakeBackend::with(|n: &Note| {
        // Edge to a target that is neither a deterministic node nor an emitted
        // concept node: must be dropped so it can't dangle.
        (
            Vec::new(),
            vec![edge(
                &n.id,
                "ghost_node",
                Relation::CITES,
                Confidence::INFERRED,
            )],
        )
    });
    let cfg = Config { backend: Some(&be) };
    let (out, _) = enrich(&cfg, base, &notes(), Cache::new()).expect("Enrich");
    for e in &out.edges {
        assert!(
            e.target != "ghost_node",
            "dangling edge to ghost_node was kept"
        );
    }
}

#[test]
fn test_enrich_forces_inferred_confidence() {
    let base = base_extraction();
    let be = FakeBackend::with(|n: &Note| {
        // A backend that wrongly claims EXTRACTED must be downgraded: semantic
        // edges are never EXTRACTED.
        (
            Vec::new(),
            vec![edge(
                &n.id,
                "appconfig",
                Relation::CITES,
                Confidence::EXTRACTED,
            )],
        )
    });
    let cfg = Config { backend: Some(&be) };
    let (out, _) = enrich(&cfg, base, &notes(), Cache::new()).expect("Enrich");
    for e in &out.edges {
        assert!(
            !(e.relation == Relation::CITES && e.confidence == Confidence::EXTRACTED),
            "semantic edge kept EXTRACTED confidence; must be INFERRED or AMBIGUOUS"
        );
    }
}

#[test]
fn test_enrich_keeps_type_only_without_location() {
    let be = FakeBackend::with(|n: &Note| {
        let mut e = Edge {
            source: n.id.clone(),
            target: "appconfig".to_string(),
            relation: Relation::CITES,
            source_file: file_of("invented.md"),
            source_location: line(7),
            ..Edge::default()
        };
        e.set_type_only(true);
        (Vec::new(), vec![e])
    });
    let cfg = Config { backend: Some(&be) };
    let (out, _) = enrich(&cfg, base_extraction(), &notes(), Cache::new()).expect("Enrich");
    let e = out.edges.last().expect("no edges");
    assert!(
        e.relation == Relation::CITES
            && e.type_only()
            && e.loc() == Loc::default()
            && e.source_file == File::default(),
        "got {e:?}, want the cites edge type-only with no file or location"
    );
}

#[test]
fn test_backend_error_is_tolerated() {
    let base = base_extraction();
    let be = FakeBackend {
        fail_on: BTreeSet::from(["notes_appconfig_bake".to_string()]),
        ..FakeBackend::default()
    };
    // A backend failure on one note must not abort the whole enrichment or
    // corrupt the deterministic core.
    let cfg = Config { backend: Some(&be) };
    let (out, _) = enrich(&cfg, base.clone(), &notes(), Cache::new())
        .unwrap_or_else(|e| panic!("a per-note backend error must not fail Enrich: {e}"));
    assert_eq!(
        out.nodes.len(),
        base.nodes.len(),
        "node count changed after backend error: got {}, want {}",
        out.nodes.len(),
        base.nodes.len()
    );
}

// Upstream `test_corpus_parallel_continues_after_chunk_failure` (test_chunking.py:253): of four
// notes, the second fails; the other three still contribute. The stderr text is not asserted
// (Go prints `  semantic: skipped <file> (<err>)` to the process stderr).
#[test]
fn test_enrich_continues_after_note_failure() {
    let be = FakeBackend {
        fail_on: BTreeSet::from(["f1".to_string()]),
        result: Some(Box::new(|n: &Note| {
            (
                vec![node(&format!("concept_{}", n.id), "C", "concept")],
                Vec::new(),
            )
        })),
        ..FakeBackend::default()
    };
    let ns: Vec<Note> = (0..4)
        .map(|i| Note {
            id: format!("f{i}"),
            file: format!("learnings/f{i}.md"),
            content: "x".to_string(),
        })
        .collect();
    let base = base_extraction();
    let cfg = Config { backend: Some(&be) };
    let (out, cache) = enrich(&cfg, base.clone(), &ns, Cache::new()).expect("Enrich");
    // 4 notes dispatched, 1 failed: 3 concept nodes added and 3 cache entries (none for f1).
    assert_eq!(out.nodes.len(), base.nodes.len() + 3);
    assert_eq!(
        cache.keys().map(String::as_str).collect::<Vec<_>>(),
        ["f0", "f2", "f3"]
    );
    assert_eq!(*be.calls.borrow(), ["f0", "f1", "f2", "f3"]);
}

/// Added (R6): Go `Enrich` with a nil backend returns `semantic: no backend configured`
/// (`semantic.go:101`); no Go test pins the text.
#[test]
fn test_enrich_no_backend_error_text() {
    let err = enrich(
        &Config::default(),
        base_extraction(),
        &notes(),
        Cache::new(),
    )
    .map(|_| ())
    .expect_err("no backend must fail");
    assert_eq!(err.to_string(), "semantic: no backend configured");
}
