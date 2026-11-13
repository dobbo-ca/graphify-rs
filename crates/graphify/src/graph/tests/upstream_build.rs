//! graphify v0.9.107 upstream tests that hold for graphify-go's `Build`/`BuildParts`
//! (`build.go`), from `test_build.py`, `test_build_located_semantic_identity.py`,
//! `test_relation_collapse_precedence.py` and `test_external_stub_endpoints.py`.
//!
//! 19 of the 223 upstream functions are ported. Each is a case whose assertion was traced
//! through `build.go`, `model.go` (`AddEdgesAt`: one edge per source, target, relation; both
//! directions kept), `langfamily.go` and `idutil.go` (`NormalizeID`). Python calls adapt as
//! `build_from_json(ext)` -> `build(ext)`, `build([e1, e2])` -> `build_parts`; the `dedup`,
//! `directed` and `root` arguments, `_origin`, and `is_directed()` have no Go counterpart and
//! are dropped. The `fixtures/extraction.json` of `test_build.py` is inlined.
//!
//! # Not ported (204)
//!
//! Vacuous in Go (removed in review): `test_build.py`
//! `build_from_json_ambiguous_old_stem_alias_stays_dangling`,
//! `build_from_json_ambiguous_alias_detected_despite_header_impl_salting`,
//! `build_from_json_unambiguous_old_stem_alias_still_needs_language_match`. They assert that an
//! edge to a bare stem stays dangling; Go has no old-stem alias, so they would pass only because
//! Go drops every dangling edge, and test nothing of Go's.
//!
//! No Go function (Python-only API):
//! - `test_build.py` (46): `dedupe_edges_collapses_exact_parallels`,
//!   `dedupe_edges_is_idempotent`, `dedupe_nodes_collapses_by_id_last_wins`;
//!   `build_merge_preserves_call_edge_direction`,
//!   `build_merge_directed_edge_direction_survives_round_trip`,
//!   `build_merge_inherits_directed_flag_from_disk`, `build_merge_fresh_graph_defaults_undirected`,
//!   `build_merge_explicit_directed_overrides_disk_flag`,
//!   `build_merge_prune_absolute_paths_match_relative_nodes`,
//!   `build_merge_prune_windows_backslash_paths`, `build_merge_replaces_changed_file_stale_edges`,
//!   `build_merge_semantic_reextract_preserves_ast_layer`,
//!   `build_merge_ast_reextract_preserves_semantic_layer`,
//!   `build_merge_root_collapses_convention_drift`, `build_merge_rejects_oversized_existing_graph`,
//!   `build_merge_sln_stub_does_not_replace_referenced_csproj`,
//!   `build_merge_project_reference_stub_does_not_replace_referenced_project`,
//!   `build_merge_explicit_ast_sources_argument`; `merge_raw_extraction_tier_scoped`,
//!   `is_ast_tier_legacy_fallback`, `merge_raw_extraction_cross_file_stub_parity`;
//!   `edge_data_simple_graph`, `edge_datas_simple_graph_returns_singleton_list`,
//!   `edge_data_multigraph_with_parallel_edges`, `edge_datas_multigraph_returns_all_parallel_edges`,
//!   `edge_data_multidigraph`, `edge_data_node_link_multigraph_roundtrip`;
//!   `has_parallel_links_detects_repeated_pairs`, `has_parallel_links_false_without_repeats`,
//!   `has_parallel_links_antiparallel_is_not_parallel_when_directed`;
//!   `graph_has_legacy_ids_detects_old_scheme`, `graph_has_legacy_ids_honors_the_scan_root_prefix`,
//!   `legacy_id_source_prefix_reads_the_relative_marker`, `legacy_id_source_prefix_without_marker`,
//!   `graph_has_legacy_ids_ignores_global_mcp_ids`,
//!   `graph_has_legacy_ids_still_checks_file_scoped_mcp_nodes`,
//!   `semantic_rekey_relative_vs_absolute_source_file`,
//!   `semantic_rekey_skips_absolute_from_either_platform`,
//!   `graph_has_legacy_ids_skips_absolute_from_either_platform`,
//!   `norm_source_file_relativizes_a_posix_absolute_path`,
//!   `derive_prune_root_recovers_root_from_posix_absolute_prune_sources`,
//!   `build_annotations_all_resolve`, `build_from_json_prunes_dangling_hyperedge_members`
//!   (no hyperedges), `build_from_json_relativizes_absolute_source_file`,
//!   `build_relativizes_absolute_source_file`, `build_from_json_relativizes_definition_file`
//!   (no `root`).
//! - `test_build_located_semantic_identity.py` (2): `empty_no_dedup_merge_keeps_located_graph`,
//!   `fresh_merge_honors_disabled_semantic_dedup` (`build_merge`).
//! - `test_build_merge_dedup_scope.py` (8): all (`build_merge`, `deduplicate_entities`).
//! - `test_build_merge_hyperedges_and_prune.py` (20): all (`build_merge`, hyperedges, prune).
//! - `test_build_merge_shrink_guard.py` (12): all (`build_merge`, shrink guard).
//! - `test_carried_hyperedge_remap.py` (6): all (`build_merge`, hyperedges).
//! - `test_multigraph_diagnostics.py` (28): all (the `diagnostics` module and its CLI).
//! - `test_external_stub_endpoints.py` (4): `mint_external_stubs_in_data_is_idempotent`,
//!   `external_stub_unifies_across_repos_in_merge` (`prefix_graph_for_global`),
//!   `backfill_origin_leaves_external_stubs_unstamped` (`_backfill_origin`),
//!   `external_stub_makes_serialized_graph_self_consistent` (checks stub minting through
//!   `node_link_data`; Go drops the dangling import edge, so it would hold only vacuously).
//!
//! Go behaves differently:
//! - Go keeps one edge per (source, target, relation) and both directions; upstream keeps one
//!   edge per pair and picks the survivor. `test_build.py`:
//!   `build_from_json_preserves_first_direction_on_bidirectional_pair` (asserts 1 edge).
//!   `test_relation_collapse_precedence.py` (16 of 18): `generic_never_overwrites_specific`,
//!   `the_reported_case_keeps_calls`, `calls_survives_for_callflow`,
//!   `specific_still_overwrites_generic`, `two_generic_relations_keep_previous_behaviour`,
//!   `two_specific_relations_keep_previous_behaviour`, `collapse_still_yields_exactly_one_edge`,
//!   `edge_count_is_unchanged_by_the_fix`, `reverse_direction_guard_still_holds`,
//!   `direction_metadata_survives_the_demotion`, `directed_graphs_get_the_same_protection`,
//!   `specific_edge_numeric_metadata_survives_the_demotion`,
//!   `unknown_relation_is_treated_as_specific`, `structural_membership_beats_an_opposite_call`,
//!   `same_direction_method_and_calls_stay_unranked`; and
//!   `same_relation_collision_preserves_extracted_over_inferred` (Go keeps the first of two
//!   same-relation edges, with no EXTRACTED preference).
//! - Go has no ghost or twin merge: `test_build.py` `alias_node_ghost_merges_into_ast_twin`,
//!   `ghost_merge_unique_located_node_still_merges`, `ghost_merge_uses_source_file_not_basename`,
//!   `ghost_merge_non_ast_same_file_still_merges`, `ghost_merge_wrong_source_file_resolved_by_label`,
//!   `label_only_file_fold_preserves_distinct_semantic_evidence`,
//!   `markdown_doc_twin_merges_into_semantic_doc_node`, `method_ghost_dedupe_and_alias_resolution_3705`;
//!   `test_build_located_semantic_identity.py` `ast_twin_remains_canonical_despite_semantic_location_drift`.
//!   `marked_cross_document_stubs_survive_issue_4738_collisions`: a repeated id keeps the last
//!   node (3 nodes, not 6).
//! - Go does not re-key ids from `source_file`, and has no old-stem alias: `test_build.py`
//!   `absolute_derived_semantic_ids_rekeyed`, `absolute_derived_semantic_ids_rekeyed_backslash`,
//!   `build_from_json_relative_source_file_unchanged` (asserts id `src_foo_bar`),
//!   `build_from_json_unambiguous_old_stem_alias_still_resolves`.
//! - Go builds no external stubs (it drops the dangling edge): `test_external_stub_endpoints.py`
//!   `build_from_json_mints_external_import_stub`.
//! - Go does not validate, coerce, alias-fold or normalise fields, and the Rust `Edge` is typed
//!   (no `from`/`to`/`type` keys, no null weight, no list ids): `test_build.py`
//!   `null_weight_edge_builds_and_clusters`, `malformed_weights_normalize`,
//!   `legacy_node_source_canonicalized`, `legacy_edge_from_to_canonicalized`,
//!   `legacy_node_name_path_aliases_folded`, `legacy_edge_type_confidence_score_aliases_folded`,
//!   `legacy_numeric_confidence_normalized_to_inferred`,
//!   `legacy_numeric_confidence_existing_score_wins`,
//!   `legacy_numeric_confidence_links_spelling_reload`,
//!   `legacy_numeric_confidence_normalization_is_idempotent`, `node_alias_canonical_field_wins`,
//!   `alias_node_gets_nonempty_norm_label`, `extraction_warning_breakdown_by_cause`,
//!   `build_drops_edge_still_missing_relation` (Go keeps an edge with no relation),
//!   `none_file_type_defaults_to_concept`, `missing_file_type_defaults_to_concept`,
//!   `real_invalid_file_type_coerced_to_concept`, `file_type_synonym_mapping`,
//!   `source_file_backslash_normalized`, `edge_missing_source_file_backfilled_from_node`,
//!   `build_from_json_skips_non_hashable_node_id`,
//!   `build_from_json_skips_edge_with_non_hashable_endpoint`.
//!
//! Needs the whole pipeline (extract and the watch rebuild): `test_4978_reconcile_cost.py` (15),
//! `test_build.py` `subfolder_build_from_parent_is_not_flagged_legacy` and
//! `fresh_mcp_graph_is_not_flagged_legacy`, `test_external_stub_endpoints.py`
//! `no_cluster_update_leaves_no_undeclared_endpoint`, `external_stub_is_dropped_with_its_last_import`,
//! `rebuild_writes_external_stubs_like_a_fresh_build`, `rebuild_heals_stub_left_by_an_earlier_version`.

use crate::graph::{build, build_parts};
use crate::model::{Confidence, Edge, Extraction, Graph, Node, Relation, loc_of, rel};

fn node(id: &str, label: &str, file_type: &str, source_file: &str) -> Node {
    Node {
        id: id.into(),
        label: label.into(),
        file_type: file_type.into(),
        source_file: source_file.into(),
        ..Node::default()
    }
}

fn located(id: &str, label: &str, file_type: &str, source_file: &str, location: &str) -> Node {
    let mut n = node(id, label, file_type, source_file);
    n.source_location = location.into();
    n
}

fn edge(source: &str, target: &str, relation: Relation, confidence: Confidence) -> Edge {
    Edge {
        source: source.into(),
        target: target.into(),
        relation,
        confidence,
        ..Edge::default()
    }
}

/// Node ids, sorted: Python's `set(G.nodes)` / `sorted(G.nodes())`.
fn sorted_ids(g: &Graph) -> Vec<String> {
    let mut ids = g.node_ids().to_vec();
    ids.sort();
    ids
}

fn sorted(ids: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}

fn find<'a>(g: &'a Graph, source: &str, target: &str) -> &'a Edge {
    g.edges()
        .iter()
        .find(|e| e.source == source && e.target == target)
        .unwrap_or_else(|| panic!("no edge {source} -> {target}"))
}

/// `fixtures/extraction.json` of `test_build.py`.
fn fixture() -> Extraction {
    Extraction {
        nodes: vec![
            located("n_transformer", "Transformer", "code", "model.py", "L1"),
            located(
                "n_attention",
                "MultiHeadAttention",
                "code",
                "model.py",
                "L10",
            ),
            located("n_layernorm", "LayerNorm", "code", "model.py", "L20"),
            located(
                "n_concept_attn",
                "attention mechanism",
                "document",
                "paper.md",
                "§3.1",
            ),
        ],
        edges: vec![
            edge(
                "n_transformer",
                "n_attention",
                Relation::CONTAINS,
                Confidence::EXTRACTED,
            ),
            edge(
                "n_transformer",
                "n_layernorm",
                Relation::CONTAINS,
                Confidence::EXTRACTED,
            ),
            edge(
                "n_attention",
                "n_concept_attn",
                Relation::IMPLEMENTS,
                Confidence::INFERRED,
            ),
            edge(
                "n_layernorm",
                "n_concept_attn",
                rel("referenced"),
                Confidence::AMBIGUOUS,
            ),
        ],
    }
}

/// `_located_nodes()` of `test_build_located_semantic_identity.py`.
fn located_nodes() -> Vec<Node> {
    vec![
        located("pkg_name", "name", "code", "pkg/plugin.json", "L2"),
        located("pkg_author_name", "name", "code", "pkg/plugin.json", "L5"),
        located("doc_s1_notes", "Notas", "document", "docs/plan.md", "L10"),
        located("doc_s2_notes", "Notas", "document", "docs/plan.md", "L80"),
        located("other_name", "name", "code", "pkg/other.json", "L2"),
    ]
}

/// `test_build.py::test_build_from_json_node_count`
#[test]
fn build_from_json_node_count() {
    assert_eq!(build(fixture()).num_nodes(), 4);
}

/// `test_build.py::test_build_from_json_edge_count`
#[test]
fn build_from_json_edge_count() {
    assert_eq!(build(fixture()).num_edges(), 4);
}

/// `test_build.py::test_nodes_have_label`
#[test]
fn nodes_have_label() {
    let g = build(fixture());
    assert_eq!(g.node("n_transformer").unwrap().label, "Transformer");
}

/// `test_build.py::test_edges_have_confidence`
#[test]
fn edges_have_confidence() {
    let g = build(fixture());
    assert!(find(&g, "n_attention", "n_concept_attn").confidence == Confidence::INFERRED);
}

/// `test_build.py::test_ambiguous_edge_preserved`
#[test]
fn ambiguous_edge_preserved() {
    let g = build(fixture());
    assert!(find(&g, "n_layernorm", "n_concept_attn").confidence == Confidence::AMBIGUOUS);
}

/// `test_build.py::test_build_merges_multiple_extractions`: `build([ext1, ext2])` is
/// `build_parts` with one part per extraction.
#[test]
fn build_merges_multiple_extractions() {
    let g = build_parts(
        vec![
            vec![node("n1", "A", "code", "a.py")],
            vec![node("n2", "B", "document", "b.md")],
        ],
        vec![
            vec![],
            vec![edge("n1", "n2", Relation::REFERENCES, Confidence::INFERRED)],
        ],
        1,
    );
    assert_eq!(g.num_nodes(), 2);
    assert_eq!(g.num_edges(), 1);
}

/// `test_build.py::test_ghost_merge_not_across_directories_same_basename`
#[test]
fn ghost_merge_not_across_directories_same_basename() {
    let ext = Extraction {
        nodes: vec![
            located(
                "docs_a_index",
                "Quickstart",
                "document",
                "docs/product_a/index.md",
                "L1",
            ),
            node(
                "docs_b_index",
                "Quickstart",
                "document",
                "docs/product_b/index.md",
            ),
            located("docs_hub", "Docs", "concept", "docs/hub.md", "L1"),
        ],
        edges: vec![edge(
            "docs_hub",
            "docs_b_index",
            rel("links_to"),
            Confidence::INFERRED,
        )],
    };
    let g = build(ext);
    assert!(g.node("docs_a_index").is_some() && g.node("docs_b_index").is_some());
    assert!(g.has_edge("docs_hub", "docs_b_index"));
    assert!(!g.has_edge("docs_hub", "docs_a_index"));
}

/// `test_build.py::test_ghost_merge_non_ast_different_files_both_survive`
#[test]
fn ghost_merge_non_ast_different_files_both_survive() {
    let ext = Extraction {
        nodes: vec![
            located(
                "dir_a_update_build_merge",
                "build_merge() function",
                "concept",
                "dir_a/update.md",
                "L10",
            ),
            located(
                "dir_b_update_build_merge",
                "build_merge() function",
                "concept",
                "dir_b/update.md",
                "L12",
            ),
        ],
        edges: vec![],
    };
    assert_eq!(
        sorted_ids(&build(ext)),
        sorted(&["dir_a_update_build_merge", "dir_b_update_build_merge"])
    );
}

/// `test_build.py::test_ghost_merge_wrong_source_file_ambiguous_basename_left_alone`
#[test]
fn ghost_merge_wrong_source_file_ambiguous_basename_left_alone() {
    let ext = Extraction {
        nodes: vec![
            located(
                "apps_admin_web_index",
                "index.ts",
                "code",
                "apps/admin-web/src/lib/index.ts",
                "L1",
            ),
            located(
                "apps_api_index",
                "index.ts",
                "code",
                "apps/api/src/lib/index.ts",
                "L1",
            ),
            node(
                "phantom_index",
                "index.ts",
                "code",
                "graphify-out/memory/query_fake2.md",
            ),
        ],
        edges: vec![],
    };
    let g = build(ext);
    assert!(g.node("phantom_index").is_some());
    assert!(g.node("apps_admin_web_index").is_some() && g.node("apps_api_index").is_some());
}

/// `test_build.py::test_cross_language_imports_references_are_dropped`
#[test]
fn cross_language_imports_references_are_dropped() {
    let ext = Extraction {
        nodes: vec![
            located(
                "backend_worker_py",
                "worker.py",
                "code",
                "backend/worker.py",
                "L1",
            ),
            located("src_time_ts", "time.ts", "code", "src/time.ts", "L1"),
            located("src_util_ts", "util.ts", "code", "src/util.ts", "L1"),
        ],
        edges: vec![
            // phantom: Python file importing a TS file (cross-language)
            edge(
                "backend_worker_py",
                "src_time_ts",
                Relation::IMPORTS,
                Confidence::EXTRACTED,
            ),
            // legit: TS importing TS (same family) must survive
            edge(
                "src_time_ts",
                "src_util_ts",
                Relation::IMPORTS,
                Confidence::EXTRACTED,
            ),
        ],
    };
    let g = build(ext);
    assert!(
        !g.has_edge("backend_worker_py", "src_time_ts"),
        "cross-language import must be dropped"
    );
    assert!(
        g.has_edge("src_time_ts", "src_util_ts"),
        "same-family (TS->TS) import must survive"
    );
}

/// `test_build.py::test_cross_family_reference_to_unknown_ext_is_kept`
#[test]
fn cross_family_reference_to_unknown_ext_is_kept() {
    let ext = Extraction {
        nodes: vec![
            located("pkg_json", "package.json", "code", "package.json", "L1"),
            located("src_app_ts", "app.ts", "code", "src/app.ts", "L1"),
        ],
        edges: vec![edge(
            "pkg_json",
            "src_app_ts",
            Relation::REFERENCES,
            Confidence::EXTRACTED,
        )],
    };
    assert!(
        build(ext).has_edge("pkg_json", "src_app_ts"),
        "config->code reference (unknown ext) must be kept"
    );
}

/// `test_build.py::test_doc_twin_merge_does_not_touch_code_symbols`
#[test]
fn doc_twin_merge_does_not_touch_code_symbols() {
    let ext = Extraction {
        nodes: vec![
            located("m_foo", "foo", "code", "m.py", "L1"),
            located("m_foo_doc", "foo rationale", "rationale", "m.py", "L2"),
        ],
        edges: vec![],
    };
    let g = build(ext);
    assert!(g.node("m_foo").is_some() && g.node("m_foo_doc").is_some());
}

/// `test_build.py::test_method_ghost_ambiguous_same_name_in_one_file_is_not_merged_3705`
#[test]
fn method_ghost_ambiguous_same_name_in_one_file_is_not_merged_3705() {
    let ext = Extraction {
        nodes: vec![
            located("svc_a_save_state", ".save_state()", "code", "svc.py", "L10"),
            located("svc_b_save_state", ".save_state()", "code", "svc.py", "L40"),
            node("svc_save_state", "save_state()", "code", "svc.py"),
            node("doc", "Doc", "document", "d.md"),
        ],
        edges: vec![edge(
            "doc",
            "svc_save_state",
            Relation::REFERENCES,
            Confidence::INFERRED,
        )],
    };
    let g = build(ext);
    assert!(g.node("svc_a_save_state").is_some() && g.node("svc_b_save_state").is_some());
    assert!(
        g.node("svc_save_state").is_some(),
        "ambiguous ghost must be left intact, not remapped"
    );
    assert!(g.has_edge("doc", "svc_save_state"));
    assert!(!g.has_edge("doc", "svc_a_save_state"));
    assert!(!g.has_edge("doc", "svc_b_save_state"));
}

/// `test_build_located_semantic_identity.py::test_distinct_same_file_locations_preserve_ids_and_edges`
/// (rows `reverse=False` and `reverse=True`)
#[test]
fn distinct_same_file_locations_preserve_ids_and_edges() {
    for reverse in [false, true] {
        let mut nodes = located_nodes();
        if reverse {
            nodes.reverse();
        }
        let want = sorted(&nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>());
        let g = build(Extraction {
            nodes,
            edges: vec![
                edge(
                    "pkg_name",
                    "doc_s1_notes",
                    Relation::REFERENCES,
                    Confidence::default(),
                ),
                edge(
                    "pkg_author_name",
                    "doc_s2_notes",
                    Relation::REFERENCES,
                    Confidence::default(),
                ),
            ],
        });
        assert_eq!(sorted_ids(&g), want, "reverse={reverse}");
        assert!(g.has_edge("pkg_name", "doc_s1_notes"), "reverse={reverse}");
        assert!(
            g.has_edge("pkg_author_name", "doc_s2_notes"),
            "reverse={reverse}"
        );
    }
}

/// `test_build_located_semantic_identity.py::test_disabled_dedup_keeps_distinct_semantic_ids_at_identical_locations`
#[test]
fn disabled_dedup_keeps_distinct_semantic_ids_at_identical_locations() {
    let nodes: Vec<Node> = [
        ("a_first", "L2"),
        ("b_second", "L5"),
        ("z_first", "L2"),
        ("z_second", "L5"),
    ]
    .iter()
    .map(|(id, loc)| located(id, "Notes", "document", "plan.md", loc))
    .collect();
    let g = build(Extraction {
        nodes,
        edges: vec![edge(
            "z_first",
            "z_second",
            Relation::REFERENCES,
            Confidence::default(),
        )],
    });
    assert_eq!(
        sorted_ids(&g),
        sorted(&["a_first", "b_second", "z_first", "z_second"])
    );
    assert!(g.has_edge("z_first", "z_second"));
}

/// `test_build_located_semantic_identity.py::test_full_build_honors_disabled_semantic_dedup`
#[test]
fn full_build_honors_disabled_semantic_dedup() {
    let nodes = located_nodes();
    let want = sorted(&nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>());
    let g = build(Extraction {
        nodes,
        edges: vec![],
    });
    assert_eq!(sorted_ids(&g), want);
}

fn pair_extraction(edges: Vec<Edge>) -> Extraction {
    Extraction {
        nodes: vec![
            node("a", "a()", "code", "a.py"),
            node("b", "b()", "code", "b.py"),
        ],
        edges,
    }
}

/// `test_relation_collapse_precedence.py::test_a_lone_generic_edge_is_kept`
#[test]
fn a_lone_generic_edge_is_kept() {
    let g = build(pair_extraction(vec![edge(
        "a",
        "b",
        Relation::REFERENCES,
        Confidence::EXTRACTED,
    )]));
    assert!(find(&g, "a", "b").relation == Relation::REFERENCES);
    assert_eq!(g.num_edges(), 1);
}

/// `test_relation_collapse_precedence.py::test_directed_graph_keeps_method_and_the_reverse_call`
/// (Go always keeps both arcs; `is_directed()` is dropped)
#[test]
fn directed_graph_keeps_method_and_the_reverse_call() {
    let mut method = edge("a", "b", rel("method"), Confidence::EXTRACTED);
    method.source_location = loc_of("L4");
    let mut call = edge("b", "a", Relation::CALLS, Confidence::EXTRACTED);
    call.source_location = loc_of("L5");
    let g = build(pair_extraction(vec![method, call]));
    assert_eq!(g.num_edges(), 2);
    let ab = find(&g, "a", "b");
    assert!(ab.relation == rel("method"));
    let ba = find(&g, "b", "a");
    assert!(ba.relation == Relation::CALLS);
    assert_eq!(ba.loc(), loc_of("L5"));
}

/// `test_external_stub_endpoints.py::test_non_import_dangling_edge_is_still_dropped`
/// (`links` is `edges`)
#[test]
fn non_import_dangling_edge_is_still_dropped() {
    let g = build(Extraction {
        nodes: vec![node("pkg_a", "a.py", "code", "pkg/a.py")],
        edges: vec![edge(
            "pkg_a",
            "some_external_fn",
            Relation::CALLS,
            Confidence::EXTRACTED,
        )],
    });
    assert!(g.node("some_external_fn").is_none());
    assert!(!g.has_edge("pkg_a", "some_external_fn"));
}
