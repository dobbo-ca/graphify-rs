# P2 test port map (appendix to `2026-11-13-p2-graphify.md`)

Generated from graphify-go v0.12.0 (35fcd9e) `*_test.go` files and the upstream inventory (`inv/upstream_rows.json`, graphify v0.9.107) with the corrections of `inv/verify.md` applied. Task keys are the plan's task keys.

Column "grammars named" lists the grammars whose file extensions appear as string literals in the Go test file (`.h` is shown as `c/cpp`). It is the first guess for the `#[ignore]` bead of each test: the tests PR reads each test and sets its ignore to the bead of the task whose code PR makes it pass (see the plan, "Ignore rule"). "needs": `git` (the test runs the git binary), `workers` (it sets GOMAXPROCS), `testdata` (it reads `internal/extract/testdata` or `internal/cluster/testdata`).

Totals: 604 `func Test`, 3 `func Fuzz`, 21 `t.Run` call sites, in 119 files. Upstream: 367 files (5938 tests) where Go has the behaviour in full (`yes`) or in part (`partial`).

## Summary

| Task | Go files | Test | Fuzz | t.Run | Upstream files (yes / partial) | Upstream tests (yes / partial) |
|---|---|---|---|---|---|---|
| `fsutil` | 1 | 2 | 0 | 0 | 0 / 3 | 0 / 32 |
| `idutil` | 1 | 4 | 1 | 0 | 0 / 1 | 0 / 4 |
| `langfamily` | 2 | 3 | 0 | 0 | 0 / 0 | 0 / 0 |
| `security` | 1 | 5 | 0 | 1 | 0 / 2 | 0 / 72 |
| `model` | 2 | 8 | 0 | 0 | 0 / 1 | 0 / 7 |
| `graph` | 2 | 8 | 0 | 0 | 0 / 10 | 0 / 223 |
| `cluster` | 2 | 13 | 0 | 0 | 0 / 4 | 0 / 36 |
| `analyze` | 3 | 16 | 0 | 0 | 2 / 3 | 27 / 73 |
| `report` | 1 | 6 | 0 | 0 | 1 / 4 | 22 / 65 |
| `export` | 5 | 21 | 1 | 0 | 0 / 13 | 0 / 207 |
| `query` | 7 | 60 | 0 | 0 | 3 / 15 | 25 / 140 |
| `detect` | 7 | 39 | 0 | 0 | 0 / 5 | 0 / 315 |
| `ts` | 1 | 1 | 0 | 0 | 0 / 0 | 0 / 0 |
| `extract-core` | 8 | 27 | 0 | 2 | 1 / 2 | 40 / 24 |
| `extract-nogrammar` | 5 | 34 | 0 | 3 | 3 / 2 | 35 / 75 |
| `extract-resolve` | 6 | 48 | 1 | 2 | 0 / 24 | 0 / 1108 |
| `cache` | 2 | 12 | 0 | 0 | 0 / 9 | 0 / 133 |
| `semantic` | 2 | 13 | 0 | 0 | 1 / 10 | 10 / 119 |
| `cli-core` | 17 | 59 | 0 | 4 | 11 / 26 | 120 / 344 |
| `cli-maint` | 6 | 20 | 0 | 0 | 0 / 21 | 0 / 476 |
| `cli-serve` | 1 | 34 | 0 | 0 | 0 / 3 | 0 / 173 |
| `extract-golang` | 2 | 3 | 0 | 0 | 0 / 14 | 0 / 131 |
| `extract-python` | 5 | 15 | 0 | 5 | 3 / 27 | 33 / 199 |
| `extract-js` | 7 | 76 | 0 | 0 | 2 / 54 | 103 / 433 |
| `extract-java` | 2 | 14 | 0 | 0 | 0 / 5 | 0 / 62 |
| `extract-csharp` | 3 | 7 | 0 | 2 | 1 / 23 | 18 / 408 |
| `extract-c` | 1 | 1 | 0 | 0 | 1 / 0 | 13 / 0 |
| `extract-cpp` | 2 | 10 | 0 | 0 | 1 / 18 | 5 / 207 |
| `extract-hcl` | 2 | 21 | 0 | 1 | 1 / 1 | 25 / 18 |
| `extract-rust` | 1 | 1 | 0 | 0 | 0 / 11 | 0 / 107 |
| `extract-ruby` | 3 | 4 | 0 | 0 | 0 / 2 | 0 / 76 |
| `extract-php` | 1 | 2 | 0 | 0 | 0 / 9 | 0 / 78 |
| `extract-bash` | 2 | 10 | 0 | 1 | 0 / 0 | 0 / 0 |
| `extract-scala` | 1 | 1 | 0 | 0 | 0 / 8 | 0 / 67 |
| `extract-kotlin` | 1 | 2 | 0 | 0 | 1 / 3 | 29 / 8 |
| `extract-julia` | 1 | 1 | 0 | 0 | 0 / 0 | 0 / 0 |
| `extract-lua` | 1 | 1 | 0 | 0 | 2 / 0 | 13 / 0 |
| `extract-zig` | 1 | 1 | 0 | 0 | 0 / 0 | 0 / 0 |
| `extract-verilog` | 1 | 1 | 0 | 0 | 0 / 0 | 0 / 0 |

## `fsutil`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/fsutil/fsutil_test.go` | 2 | 0 | 0 | n/a | - | `crates/graphify/src/fsutil/tests/fsutil.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_atomic_version_stamp.py` | 5 | partial | internal/fsutil/fsutil_test.go |
| `test_atomic_writes.py` | 18 | partial | internal/fsutil/fsutil_test.go |
| `tests/test_paths.py` | 9 | partial | internal/fsutil/fsutil_test.go, cmd/graphify/outdir_test.go |

## `idutil`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/idutil/idutil_test.go` | 4 | 1 | 0 | n/a | - | `crates/graphify/src/idutil/tests/idutil.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_node_id_canonical.py` | 4 | partial | internal/idutil/idutil_test.go, extract/nodeid_test.go |

## `langfamily`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/langfamily/langfamily_rake_test.go` | 1 | 0 | 0 | n/a | - | `crates/graphify/src/langfamily/tests/langfamily_rake.rs` |
| `internal/langfamily/langfamily_test.go` | 2 | 0 | 0 | n/a | - | `crates/graphify/src/langfamily/tests/langfamily.rs` |

## `security`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/security/security_test.go` | 5 | 0 | 1 | n/a | - | `crates/graphify/src/security/tests/security.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_security.py` | 69 | partial | internal/security/security_test.go |
| `tests/test_frontmatter_and_ssrf_hardening.py` | 3 | partial | internal/security/security_test.go |

## `model`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/model/code_test.go` | 5 | 0 | 0 | n/a | - | `crates/graphify/src/model/tests/code.rs` |
| `internal/model/model_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/model/tests/model.rs` |

Note, `code_test.go`: asserts Go struct sizes with unsafe.Sizeof; ported as behaviour checks of the code tables only.

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_confidence.py` | 7 | partial | internal/model/model_test.go |

## `graph`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/graph/build_test.go` | 6 | 0 | 0 | n/a | - | `crates/graphify/src/graph/tests/build.rs` |
| `internal/graph/order_test.go` | 2 | 0 | 0 | n/a | - | `crates/graphify/src/graph/tests/order.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_4978_reconcile_cost.py` | 15 | partial | internal/graph/build_test.go |
| `test_build.py` | 100 | partial | internal/graph/build_test.go, order_test.go |
| `test_build_located_semantic_identity.py` | 6 | partial | internal/graph/build_test.go |
| `test_build_merge_dedup_scope.py` | 8 | partial | internal/query/merge_test.go |
| `test_build_merge_hyperedges_and_prune.py` | 20 | partial | internal/query/merge_test.go |
| `test_build_merge_shrink_guard.py` | 12 | partial | internal/query/merge_test.go, cmd/graphify/update_test.go |
| `test_carried_hyperedge_remap.py` | 6 | partial | internal/query/merge_test.go |
| `test_relation_collapse_precedence.py` | 18 | partial | internal/graph/build_test.go |
| `tests/test_external_stub_endpoints.py` | 10 | partial | internal/graph/build_test.go |
| `tests/test_multigraph_diagnostics.py` | 28 | partial | internal/graph/build_test.go |

## `cluster`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/cluster/cluster_test.go` | 10 | 0 | 0 | n/a | - | `crates/graphify/src/cluster/tests/cluster.rs` |
| `internal/cluster/louvain_test.go` | 3 | 0 | 0 | n/a | testdata | `crates/graphify/src/cluster/tests/louvain.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_cluster.py` | 18 | partial | internal/cluster/cluster_test.go, louvain_test.go |
| `test_cluster_ambiguous_scale.py` | 7 | partial | internal/cluster/cluster_test.go |
| `test_cluster_exclude_hubs.py` | 5 | partial | cmd/graphify/godnodes_test.go, cluster_test.go |
| `tests/test_graph_invariance.py` | 6 | partial | internal/graph/build_test.go, internal/extract/order_test.go |

## `analyze`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/analyze/analyze_test.go` | 10 | 0 | 0 | n/a | - | `crates/graphify/src/analyze/tests/analyze.rs` |
| `internal/analyze/cycles_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/analyze/tests/cycles.rs` |
| `internal/analyze/typeonly_cycle_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/analyze/tests/typeonly_cycle.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_type_only_import_cycles.py` | 18 | yes | internal/analyze/typeonly_cycle_test.go |
| `test_undirected_two_file_import_cycle.py` | 9 | yes | internal/analyze/cycles_test.go |
| `test_analyze.py` | 58 | partial | internal/analyze/analyze_test.go, cycles_test.go, typeonly_cycle_test.go |
| `test_semantic_similarity.py` | 8 | partial | - |
| `tests/test_god_nodes_exclude_hubs.py` | 7 | partial | internal/analyze/analyze_test.go, cmd/graphify/godnodes_test.go |

## `report`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/report/report_test.go` | 6 | 0 | 0 | n/a | - | `crates/graphify/src/report/tests/report.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_report.py` | 22 | yes | internal/report/report_test.go |
| `test_community_hub_labels.py` | 8 | partial | internal/report/report_test.go |
| `test_confidence_score_output.py` | 5 | partial | internal/report/report_test.go |
| `test_report_gap_thresholds.py` | 13 | partial | internal/report/report_test.go |
| `tests/test_labeling.py` | 39 | partial | internal/cluster/cluster_test.go, report_test.go |

## `export`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/export/export_test.go` | 9 | 0 | 0 | n/a | - | `crates/graphify/src/export/tests/export.rs` |
| `internal/export/formats_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/export/tests/formats.rs` |
| `internal/export/okf_test.go` | 5 | 0 | 0 | n/a | - | `crates/graphify/src/export/tests/okf.rs` |
| `internal/export/oldnodes_test.go` | 3 | 1 | 0 | n/a | - | `crates/graphify/src/export/tests/oldnodes.rs` |
| `internal/export/stream_test.go` | 1 | 0 | 0 | n/a | - | `crates/graphify/src/export/tests/stream.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_compact_graph_json.py` | 9 | partial | internal/export/export_test.go |
| `test_definition_file_portability.py` | 4 | partial | internal/extract/nodeid_test.go |
| `test_wiki.py` | 38 | partial | internal/export/formats_test.go |
| `test_wiki_case_only_relabel.py` | 5 | partial | internal/export/formats_test.go |
| `test_wiki_link_filename_parity.py` | 8 | partial | internal/export/formats_test.go |
| `test_wiki_truncation_indicator.py` | 3 | partial | internal/export/formats_test.go |
| `tests/test_export.py` | 80 | partial | internal/export/export_test.go, oldnodes_test.go, order_test.go |
| `tests/test_export_control_characters.py` | 17 | partial | internal/export/export_test.go |
| `tests/test_export_idempotent_writes.py` | 5 | partial | internal/export/export_test.go, cmd/graphify/outfp_test.go |
| `tests/test_file_label_disambiguation.py` | 6 | partial | internal/extract/nodeid_test.go, internal/idutil/idutil_test.go |
| `tests/test_hyperedge_member_shapes.py` | 6 | partial | internal/export/export_test.go |
| `tests/test_hyperedge_roundtrip.py` | 3 | partial | internal/export/export_test.go, oldnodes_test.go |
| `tests/test_hypergraph.py` | 23 | partial | internal/export/export_test.go |

## `query`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/query/affected_test.go` | 8 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/affected.rs` |
| `internal/query/ask_test.go` | 22 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/ask.rs` |
| `internal/query/diff_test.go` | 4 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/diff.rs` |
| `internal/query/merge_test.go` | 7 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/merge.rs` |
| `internal/query/query_test.go` | 14 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/query.rs` |
| `internal/query/unclassified_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/unclassified.rs` |
| `internal/query/validate_test.go` | 2 | 0 | 0 | n/a | - | `crates/graphify/src/query/tests/validate.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_validate.py` | 18 | yes | internal/query/validate_test.go |
| `tests/test_query_deterministic_order.py` | 2 | yes | internal/query/query_test.go |
| `tests/test_query_seed_order.py` | 5 | yes | internal/query/query_test.go |
| `test_affected_defines.py` | 10 | partial | internal/query/affected_test.go |
| `test_affected_member_seed.py` | 5 | partial | internal/query/affected_test.go |
| `test_affected_overrides.py` | 11 | partial | internal/query/affected_test.go |
| `test_affected_path_file_node.py` | 6 | partial | internal/query/affected_test.go |
| `test_affected_seed_reference_forms.py` | 8 | partial | internal/query/affected_test.go |
| `test_corrupt_graph_json.py` | 4 | partial | internal/query/validate_test.go |
| `test_truncation_full_answer.py` | 4 | partial | internal/query/query_test.go, cmd/graphify/budget_test.go |
| `tests/test_merge_chunks_validation.py` | 10 | partial | internal/query/merge_test.go, validate_test.go |
| `tests/test_merge_graphs_cli.py` | 15 | partial | internal/query/merge_test.go |
| `tests/test_path_confidence.py` | 12 | partial | internal/query/query_test.go |
| `tests/test_path_type_member.py` | 15 | partial | cmd/graphify/explain_path_test.go |
| `tests/test_query_fallback_edges.py` | 6 | partial | internal/query/query_test.go |
| `tests/test_query_induced_edges.py` | 11 | partial | internal/query/query_test.go |
| `tests/test_query_seeds.py` | 13 | partial | internal/query/query_test.go |
| `tests/test_query_source_first.py` | 10 | partial | internal/query/query_test.go |

## `detect`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/detect/detect_parity_test.go` | 6 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/detect_parity.rs` |
| `internal/detect/detect_parity_unix_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/detect_parity_unix.rs` |
| `internal/detect/detect_test.go` | 15 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/detect.rs` |
| `internal/detect/gitignore_infoexclude_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/gitignore_infoexclude.rs` |
| `internal/detect/gitignore_parity_test.go` | 8 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/gitignore_parity.rs` |
| `internal/detect/gitignore_test.go` | 3 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/gitignore.rs` |
| `internal/detect/walk_test.go` | 1 | 0 | 0 | n/a | - | `crates/graphify/src/detect/tests/walk.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_detect.py` | 273 | partial | internal/detect/detect_test.go, detect_parity_test.go, detect_parity_unix_test.go, gitignore*_test.go, walk_test.go |
| `test_source_file_encoding.py` | 16 | partial | internal/extract/degenerate_test.go |
| `tests/test_ignore_file_encoding.py` | 12 | partial | internal/detect/gitignore_test.go, gitignore_parity_test.go |
| `tests/test_msbuild_dir_evidence.py` | 5 | partial | cmd/graphify/outdir_test.go, internal/detect/detect_test.go |
| `tests/test_non_regular_files.py` | 9 | partial | internal/detect/detect_parity_unix_test.go |

## `ts`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/ts/ts_test.go` | 1 | 0 | 0 | n/a | - | `crates/graphify/src/ts/tests/ts.rs` |

## `extract-core`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/budget_test.go` | 3 | 0 | 0 | golang, julia, kotlin | - | `crates/graphify/src/extract/tests/budget.rs` |
| `internal/extract/degenerate_test.go` | 2 | 0 | 0 | js, json | - | `crates/graphify/src/extract/tests/degenerate.rs` |
| `internal/extract/dispatch_test.go` | 2 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/dispatch.rs` |
| `internal/extract/extract_test.go` | 2 | 0 | 0 | golang, js, python | testdata | `crates/graphify/src/extract/tests/extract.rs` |
| `internal/extract/json_test.go` | 11 | 0 | 0 | json | - | `crates/graphify/src/extract/tests/json.rs` |
| `internal/extract/nodeid_test.go` | 5 | 0 | 2 | c, c/cpp, cpp, golang, php, python, ruby | - | `crates/graphify/src/extract/tests/nodeid.rs` |
| `internal/extract/rationale_scan_test.go` | 1 | 0 | 0 | js, python | - | `crates/graphify/src/extract/tests/rationale_scan.rs` |
| `internal/extract/resultjson_test.go` | 1 | 0 | 0 | golang | - | `crates/graphify/src/extract/tests/resultjson.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_rationale.py` | 40 | yes | internal/extract/rationale_scan_test.go |
| `test_workers_by_work_size.py` | 10 | partial | internal/extract/budget_test.go |
| `tests/test_json_schema_config.py` | 14 | partial | internal/extract/json_test.go, mcpconfig_test.go |

## `extract-nogrammar`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/cargo_rename_test.go` | 5 | 0 | 0 | - | - | `crates/graphify/src/extract/tests/cargo_rename.rs` |
| `internal/extract/cargo_test.go` | 9 | 0 | 0 | - | - | `crates/graphify/src/extract/tests/cargo.rs` |
| `internal/extract/manifest_test.go` | 6 | 0 | 0 | - | - | `crates/graphify/src/extract/tests/manifest.rs` |
| `internal/extract/markdown_test.go` | 7 | 0 | 3 | golang | testdata | `crates/graphify/src/extract/tests/markdown.rs` |
| `internal/extract/mcpconfig_test.go` | 7 | 0 | 0 | golang, json | - | `crates/graphify/src/extract/tests/mcpconfig.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_cargo_introspect.py` | 16 | yes | internal/extract/cargo_test.go, cargo_rename_test.go, cmd/graphify/cargo_test.go |
| `test_cargo_missing_manifest.py` | 2 | yes | internal/extract/cargo_test.go |
| `tests/test_manifest_ingest.py` | 17 | yes | internal/extract/manifest_test.go, cargo_test.go |
| `tests/test_markdown_code_spans.py` | 46 | partial | internal/extract/markdown_test.go |
| `tests/test_mcp_ingest.py` | 29 | partial | internal/extract/mcpconfig_test.go |

## `extract-resolve`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/gaps_test.go` | 9 | 0 | 0 | csharp, js, ruby, scala | - | `crates/graphify/src/extract/tests/gaps.rs` |
| `internal/extract/gotargets_test.go` | 1 | 0 | 0 | golang | - | `crates/graphify/src/extract/tests/gotargets.rs` |
| `internal/extract/inherits_test.go` | 6 | 0 | 0 | csharp, java, python | - | `crates/graphify/src/extract/tests/inherits.rs` |
| `internal/extract/local_imports_test.go` | 1 | 0 | 1 | c, c/cpp, cpp, golang, ruby, rust | - | `crates/graphify/src/extract/tests/local_imports.rs` |
| `internal/extract/recvtail_test.go` | 1 | 1 | 0 | - | - | `crates/graphify/src/extract/tests/recvtail.rs` |
| `internal/extract/references_test.go` | 19 | 0 | 0 | c, c/cpp, cpp, csharp, golang, java, js, kotlin, php, python, ruby, rust, scala | - | `crates/graphify/src/extract/tests/references.rs` |
| `internal/extract/resolve_test.go` | 12 | 0 | 1 | csharp, golang, hcl, java, js, lua, python, ruby, rust | workers | `crates/graphify/src/extract/tests/resolve.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_4909_reference_binding_scale.py` | 1 | partial | internal/extract/references_test.go |
| `test_alias_folding.py` | 19 | partial | internal/extract/csharp_calls_test.go, resolve_test.go |
| `test_case_sensitive_resolution.py` | 4 | partial | internal/extract/resolve_test.go |
| `test_cross_language_call_resolution.py` | 4 | partial | internal/extract/resolve_test.go |
| `test_cross_repo_external_call_guards.py` | 2 | partial | internal/extract/resolve_test.go |
| `test_cross_repo_member_calls.py` | 33 | partial | internal/extract/resolve_test.go, csharp_calls_test.go |
| `test_cross_repo_shared_types.py` | 7 | partial | internal/extract/resolve_test.go |
| `test_resolution_collision_index.py` | 2 | partial | internal/extract/resolve_test.go |
| `test_sourceless_call_targets.py` | 6 | partial | internal/extract/resolve_test.go, gotargets_test.go |
| `test_symbol_resolution.py` | 42 | partial | internal/extract/resolve_test.go |
| `tests/test_extract.py` | 318 | partial | internal/extract/*_test.go (56 files) |
| `tests/test_extractor_scan_root_context.py` | 4 | partial | internal/extract/extract_test.go |
| `tests/test_import_self_loops.py` | 3 | partial | internal/extract/jsts_imports_test.go, typeonly_cycle_test.go |
| `tests/test_inherited_field_receivers.py` | 9 | partial | internal/extract/inherits_test.go, recvtail_test.go |
| `tests/test_issue_3472_source_file_collision.py` | 5 | partial | internal/extract/nodeid_test.go |
| `tests/test_kotlin_java_call_edges_4935.py` | 17 | partial | internal/extract/kotlin_test.go, java_chain_test.go |
| `tests/test_language_resolvers.py` | 5 | partial | internal/extract/resolve_test.go, langfamily tests |
| `tests/test_languages.py` | 505 | partial | internal/extract/{c,cpp,csharp,java,ruby,rust,scala,php,lua,zig,julia,verilog,bash,kotlin}_test.go |
| `tests/test_loose_sibling_import_resolution.py` | 8 | partial | internal/extract/local_imports_test.go |
| `tests/test_multilang.py` | 98 | partial | internal/extract/*_test.go |
| `tests/test_owned_type_sources.py` | 3 | partial | internal/extract/resolve_test.go |
| `tests/test_parked_call_site_file.py` | 1 | partial | internal/extract/pycalls_test.go |
| `tests/test_phantom_cross_package_call.py` | 5 | partial | internal/extract/gocalls_test.go, resolve_test.go |
| `tests/test_phantom_external_import.py` | 7 | partial | internal/extract/local_imports_test.go |

## `cache`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/cache/cache_test.go` | 8 | 0 | 0 | golang, hcl, json | - | `crates/graphify/src/cache/tests/cache.rs` |
| `internal/cache/stream_test.go` | 4 | 0 | 0 | c, c/cpp, golang, js, python | - | `crates/graphify/src/cache/tests/stream.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_cache.py` | 86 | partial | internal/cache/cache_test.go, stream_test.go |
| `test_cache_long_paths.py` | 1 | partial | internal/cache/cache_test.go |
| `test_cache_stale_import_target.py` | 4 | partial | internal/cache/cache_test.go |
| `test_semantic_cache_out_root.py` | 7 | partial | internal/cache/cache_test.go, cmd/graphify/outdir_test.go |
| `test_stat_index_portability.py` | 9 | partial | - |
| `test_zero_node_no_cache.py` | 3 | partial | internal/cache/cache_test.go, cmd/graphify/update_test.go |
| `tests/test_extract_cache_location.py` | 5 | partial | internal/cache/cache_test.go, cmd/graphify/outdir_test.go |
| `tests/test_long_path_hashing.py` | 4 | partial | internal/cache/cache_test.go, fsutil_test.go |
| `tests/test_partial_cache.py` | 14 | partial | internal/cache/cache_test.go |

## `semantic`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/semantic/bedrock_test.go` | 4 | 0 | 0 | n/a | - | `crates/graphify/src/semantic/tests/bedrock.rs` |
| `internal/semantic/semantic_test.go` | 9 | 0 | 0 | n/a | - | `crates/graphify/src/semantic/tests/semantic.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_llm_parser.py` | 10 | yes | internal/semantic/semantic_test.go, extract/resultjson_test.go |
| `test_chunking.py` | 33 | partial | internal/semantic/semantic_test.go, cmd/graphify/budget_test.go, internal/extract/budget_test.go |
| `test_semantic_cache_basename_recovery.py` | 4 | partial | internal/cache/cache_test.go, internal/semantic/semantic_test.go |
| `test_semantic_cleanup.py` | 26 | partial | internal/semantic/semantic_test.go |
| `test_semantic_fragment_sanitize.py` | 5 | partial | internal/semantic/semantic_test.go |
| `test_semantic_id_remap_root.py` | 7 | partial | internal/semantic/semantic_test.go |
| `test_semantic_origin_stamp.py` | 8 | partial | internal/semantic/semantic_test.go |
| `tests/test_llm_null_token_counts.py` | 4 | partial | internal/semantic/semantic_test.go |
| `tests/test_llm_parser_reasoning.py` | 12 | partial | internal/semantic/semantic_test.go |
| `tests/test_oversized_document_slicing.py` | 7 | partial | internal/semantic/semantic_test.go |
| `tests/test_partial_extraction_warning.py` | 13 | partial | internal/semantic/semantic_test.go, extract/degenerate_test.go |

## `cli-core`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `cmd/graphify/affected_stdin_test.go` | 1 | 0 | 0 | golang | - | `crates/graphify/src/cli/tests/affected_stdin.rs` |
| `cmd/graphify/ask_test.go` | 2 | 0 | 0 | json | - | `crates/graphify/src/cli/tests/ask.rs` |
| `cmd/graphify/brokenpipe_test.go` | 2 | 0 | 0 | golang, json | - | `crates/graphify/src/cli/tests/brokenpipe.rs` |
| `cmd/graphify/budget_test.go` | 2 | 0 | 0 | golang, json, julia | workers | `crates/graphify/src/cli/tests/budget.rs` |
| `cmd/graphify/cargo_test.go` | 2 | 0 | 0 | json, rust | - | `crates/graphify/src/cli/tests/cargo.rs` |
| `cmd/graphify/explain_error_test.go` | 2 | 0 | 0 | golang, json | - | `crates/graphify/src/cli/tests/explain_error.rs` |
| `cmd/graphify/explain_path_test.go` | 8 | 0 | 0 | golang, json, python | - | `crates/graphify/src/cli/tests/explain_path.rs` |
| `cmd/graphify/githead_test.go` | 4 | 0 | 2 | - | git | `crates/graphify/src/cli/tests/githead.rs` |
| `cmd/graphify/godnodes_test.go` | 3 | 0 | 0 | golang, json | - | `crates/graphify/src/cli/tests/godnodes.rs` |
| `cmd/graphify/largefirst_test.go` | 2 | 0 | 0 | golang | - | `crates/graphify/src/cli/tests/largefirst.rs` |
| `cmd/graphify/nocluster_test.go` | 1 | 0 | 0 | golang, json | - | `crates/graphify/src/cli/tests/nocluster.rs` |
| `cmd/graphify/outdir_test.go` | 9 | 0 | 0 | golang, json | - | `crates/graphify/src/cli/tests/outdir.rs` |
| `cmd/graphify/outfp_test.go` | 5 | 0 | 1 | golang, json, julia | - | `crates/graphify/src/cli/tests/outfp.rs` |
| `cmd/graphify/prof_test.go` | 3 | 0 | 0 | - | - | `crates/graphify/src/cli/tests/prof.rs` |
| `cmd/graphify/semantic_test.go` | 2 | 0 | 0 | json | - | `crates/graphify/src/cli/tests/semantic.rs` |
| `cmd/graphify/unclassified_test.go` | 4 | 0 | 0 | golang, json | - | `crates/graphify/src/cli/tests/unclassified.rs` |
| `cmd/graphify/update_test.go` | 7 | 0 | 1 | golang, json | - | `crates/graphify/src/cli/tests/update.rs` |

Note, `prof_test.go`: TestProfCPU*/TestProfMem*: Go pprof output. Ported as D-005 checks (no file written, exit 0).

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_affected_cli.py` | 19 | yes | cmd/graphify/affected_stdin_test.go, internal/query/affected_test.go |
| `test_cli_broken_pipe.py` | 2 | yes | cmd/graphify/brokenpipe_test.go |
| `test_update_incremental.py` | 17 | yes | cmd/graphify/update_test.go |
| `tests/test_git_head_stdin.py` | 1 | yes | cmd/graphify/githead_test.go, affected_stdin_test.go |
| `tests/test_god_nodes_cli.py` | 5 | yes | cmd/graphify/godnodes_test.go |
| `tests/test_incremental.py` | 12 | yes | cmd/graphify/update_test.go, internal/cache/cache_test.go |
| `tests/test_no_cluster_run_only_keys.py` | 3 | yes | cmd/graphify/nocluster_test.go |
| `tests/test_no_cluster_undeclared_endpoints.py` | 9 | yes | cmd/graphify/nocluster_test.go |
| `tests/test_out_dir_evidence.py` | 6 | yes | cmd/graphify/outdir_test.go |
| `tests/test_path_cli.py` | 22 | yes | cmd/graphify/explain_path_test.go |
| `tests/test_query_cli.py` | 24 | yes | internal/query/query_test.go, cmd/graphify/ask_test.go |
| `(non-test: __init__, bench_extract, bench_query_scoring, conftest, csharp_extension_harness, pi.test.ts, fixtures/**)` | 0 | partial | - |
| `test_cli_export.py` | 63 | partial | internal/export/export_test.go, formats_test.go, okf_test.go, stream_test.go |
| `test_cli_help.py` | 3 | partial | cmd/graphify/*_test.go |
| `test_code_only_scope_4923.py` | 14 | partial | cmd/graphify/update_test.go |
| `test_shrink_guard_dedup_merge_4940.py` | 17 | partial | cmd/graphify/update_test.go, internal/query/merge_test.go |
| `test_shrink_guard_replaced_sources.py` | 5 | partial | cmd/graphify/update_test.go |
| `test_stale_prune.py` | 5 | partial | cmd/graphify/update_test.go |
| `test_unverified_semantic_shrink.py` | 13 | partial | cmd/graphify/update_test.go, semantic_test.go |
| `test_update_js_resolution_inputs.py` | 2 | partial | cmd/graphify/update_test.go |
| `test_update_manifest_preservation.py` | 1 | partial | cmd/graphify/update_test.go |
| `test_update_missing_graph.py` | 1 | partial | cmd/graphify/update_test.go |
| `test_update_no_change_gate.py` | 17 | partial | cmd/graphify/update_test.go |
| `test_update_partial_manifest.py` | 3 | partial | cmd/graphify/update_test.go |
| `tests/test_explain_cli.py` | 19 | partial | cmd/graphify/explain_path_test.go, explain_error_test.go |
| `tests/test_explain_same_file_tie.py` | 6 | partial | cmd/graphify/explain_path_test.go |
| `tests/test_extract_code_only_cli.py` | 13 | partial | cmd/graphify/update_test.go |
| `tests/test_incomplete_build_guard.py` | 9 | partial | cmd/graphify/update_test.go |
| `tests/test_incremental_unscanned_placeholders.py` | 5 | partial | cmd/graphify/update_test.go |
| `tests/test_linked_worktree_reconcile.py` | 2 | partial | cmd/graphify/githead_test.go, update_test.go |
| `tests/test_lookup_exit_codes.py` | 15 | partial | cmd/graphify/explain_error_test.go, explain_path_test.go |
| `tests/test_lost_saved_semantic_nodes.py` | 23 | partial | cmd/graphify/update_test.go, semantic_test.go |
| `tests/test_merge_sweeps_reextract_orphans.py` | 9 | partial | cmd/graphify/update_test.go |
| `tests/test_pipeline.py` | 8 | partial | cmd/graphify/update_test.go, extract/extract_test.go |
| `tests/test_prs.py` | 72 | partial | various internal/extract tests |
| `tests/test_prune_sweeps_orphans.py` | 10 | partial | cmd/graphify/update_test.go |
| `tests/test_query_names_its_graph.py` | 9 | partial | cmd/graphify/ask_test.go |

## `cli-maint`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `cmd/graphify/hook_mergedriver_status_test.go` | 6 | 0 | 0 | - | git | `crates/graphify/src/cli/tests/hook_mergedriver_status.rs` |
| `cmd/graphify/hook_mergedriver_test.go` | 2 | 0 | 0 | - | git | `crates/graphify/src/cli/tests/hook_mergedriver.rs` |
| `cmd/graphify/hook_postcheckout_test.go` | 1 | 0 | 0 | - | git | `crates/graphify/src/cli/tests/hook_postcheckout.rs` |
| `cmd/graphify/install_test.go` | 2 | 0 | 0 | - | - | `crates/graphify/src/cli/tests/install.rs` |
| `cmd/graphify/maintenance_test.go` | 6 | 0 | 0 | golang | - | `crates/graphify/src/cli/tests/maintenance.rs` |
| `cmd/graphify/save_result_test.go` | 3 | 0 | 0 | golang | - | `crates/graphify/src/cli/tests/save_result.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_agents_platform.py` | 12 | partial | cmd/graphify/install_test.go |
| `test_claude_md.py` | 18 | partial | cmd/graphify/install_test.go |
| `test_reflect.py` | 61 | partial | - |
| `test_replace_or_append_section.py` | 5 | partial | cmd/graphify/install_test.go |
| `test_settings_merge.py` | 8 | partial | cmd/graphify/install_test.go |
| `test_uninstall_global_registration.py` | 17 | partial | cmd/graphify/install_test.go |
| `test_uninstall_purge_safety.py` | 17 | partial | cmd/graphify/install_test.go, maintenance_test.go |
| `test_uninstall_scope.py` | 6 | partial | cmd/graphify/install_test.go |
| `test_uninstall_symlink.py` | 2 | partial | cmd/graphify/install_test.go |
| `tests/test_hook_chain_survives_skip.py` | 7 | partial | cmd/graphify/hook_mergedriver_test.go, hook_postcheckout_test.go |
| `tests/test_hooks.py` | 118 | partial | cmd/graphify/hook_mergedriver_test.go, hook_postcheckout_test.go, hook_mergedriver_status_test.go |
| `tests/test_install.py` | 119 | partial | cmd/graphify/install_test.go |
| `tests/test_install_hook_identity.py` | 6 | partial | cmd/graphify/install_test.go |
| `tests/test_install_line_endings.py` | 6 | partial | cmd/graphify/install_test.go |
| `tests/test_install_references.py` | 18 | partial | cmd/graphify/install_test.go |
| `tests/test_install_roundtrip.py` | 9 | partial | cmd/graphify/install_test.go |
| `tests/test_install_strings.py` | 7 | partial | cmd/graphify/install_test.go |
| `tests/test_install_upgrade.py` | 8 | partial | cmd/graphify/install_test.go |
| `tests/test_install_version_stamp.py` | 2 | partial | cmd/graphify/install_test.go |
| `tests/test_install_version_warning.py` | 6 | partial | cmd/graphify/install_test.go |
| `tests/test_opencode_plugin_refresh.py` | 24 | partial | cmd/graphify/install_test.go |

## `cli-serve`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `cmd/graphify/serve_test.go` | 34 | 0 | 0 | golang, json | git | `crates/graphify/src/cli/tests/serve.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_serve.py` | 161 | partial | cmd/graphify/serve_test.go, internal/query/query_test.go |
| `tests/test_mcp_graph_view_cache.py` | 6 | partial | cmd/graphify/serve_test.go |
| `tests/test_query_mcp_direction.py` | 6 | partial | cmd/graphify/serve_test.go |

## `extract-golang`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/gocalls_test.go` | 2 | 0 | 0 | golang | - | `crates/graphify/src/extract/tests/gocalls.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_go_builtin_call_targets.py` | 6 | partial | internal/extract/gotargets_test.go, gocalls_test.go |
| `tests/test_go_generic_receiver_methods.py` | 4 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_generic_type_params.py` | 4 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_import_repoint.py` | 3 | partial | internal/extract/modulepath_test.go, local_imports_test.go |
| `tests/test_go_interface_dispatch.py` | 10 | partial | internal/extract/dispatch_test.go, gocalls_test.go |
| `tests/test_go_interface_methods.py` | 5 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_method_value_indirect_calls.py` | 7 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_package_type_fold.py` | 6 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_package_var_calls.py` | 3 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_package_var_type_refs.py` | 4 | partial | internal/extract/gocalls_test.go |
| `tests/test_go_qualified_resolution.py` | 17 | partial | internal/extract/gotargets_test.go, resolve_test.go |
| `tests/test_go_receiver_member_calls.py` | 30 | partial | internal/extract/gocalls_test.go, recvtail_test.go |
| `tests/test_go_same_file_receiver_calls.py` | 22 | partial | internal/extract/gocalls_test.go, recvtail_test.go |
| `tests/test_go_type_alias.py` | 10 | partial | internal/extract/gocalls_test.go |

## `extract-python`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/pycalls_test.go` | 1 | 0 | 0 | python | - | `crates/graphify/src/extract/tests/pycalls.rs` |
| `internal/extract/python_import_guided_test.go` | 4 | 0 | 0 | python | - | `crates/graphify/src/extract/tests/python_import_guided.rs` |
| `internal/extract/python_imports_test.go` | 1 | 0 | 0 | python | - | `crates/graphify/src/extract/tests/python_imports.rs` |
| `internal/extract/python_nested_test.go` | 5 | 0 | 2 | python | - | `crates/graphify/src/extract/tests/python_nested.rs` |
| `internal/extract/python_test.go` | 4 | 0 | 3 | python | testdata | `crates/graphify/src/extract/tests/python.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_python_import_never_resolves_to_itself.py` | 5 | yes | internal/extract/python_imports_test.go |
| `tests/test_python_import_resolution.py` | 24 | yes | internal/extract/python_imports_test.go, python_import_guided_test.go |
| `tests/test_python_nested_class_identity.py` | 4 | yes | internal/extract/python_nested_test.go |
| `test_src_layout_import_resolution.py` | 13 | partial | internal/extract/python_imports_test.go, modulepath_test.go |
| `test_trailing_newline_not_a_syntax_error.py` | 6 | partial | internal/extract/degenerate_test.go |
| `tests/test_duplicate_annotation_edges.py` | 4 | partial | internal/extract/python_test.go |
| `tests/test_indirect_dispatch_getattr.py` | 8 | partial | internal/extract/pycalls_test.go |
| `tests/test_indirect_dispatch_module_attr.py` | 10 | partial | internal/extract/pycalls_test.go, python_imports_test.go |
| `tests/test_issue_3405_python_resolution.py` | 8 | partial | internal/extract/python_import_guided_test.go, python_imports_test.go |
| `tests/test_module_level_underscore_decl_ids.py` | 4 | partial | internal/extract/nodeid_test.go, python_test.go |
| `tests/test_python_bare_call_not_enclosing_method.py` | 6 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_builtin_call_shadowing_4989.py` | 7 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_decorators.py` | 13 | partial | internal/extract/python_test.go |
| `tests/test_python_forward_ref_file_id.py` | 4 | partial | internal/extract/python_test.go |
| `tests/test_python_imported_call_caller_collision.py` | 4 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_inherited_method_calls.py` | 8 | partial | internal/extract/pycalls_test.go, inherits_test.go |
| `tests/test_python_injected_factory_calls.py` | 9 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_member_call_language_gate.py` | 11 | partial | internal/extract/pycalls_test.go, langfamily_test.go |
| `tests/test_python_member_collisions.py` | 5 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_module_call_exact_name.py` | 5 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_module_instance_calls.py` | 7 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_property_reads.py` | 9 | partial | internal/extract/python_test.go |
| `tests/test_python_pytest_fixture_receivers.py` | 9 | partial | internal/extract/pycalls_test.go |
| `tests/test_python_reexport_type_binding.py` | 2 | partial | internal/extract/python_imports_test.go |
| `tests/test_python_self_type_ref.py` | 4 | partial | internal/extract/python_test.go |
| `tests/test_python_subscript_type_refs.py` | 3 | partial | internal/extract/references_test.go |
| `tests/test_python_type_alias.py` | 8 | partial | internal/extract/python_test.go |
| `tests/test_python_typed_receiver_calls.py` | 11 | partial | internal/extract/pycalls_test.go, recvtail_test.go |
| `tests/test_python_underscore_resolution.py` | 19 | partial | internal/extract/python_test.go, nodeid_test.go |
| `tests/test_qualified_annotation_stub_bind.py` | 2 | partial | internal/extract/references_test.go |

## `extract-js`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/component_test.go` | 1 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/component.rs` |
| `internal/extract/javascript_alias_test.go` | 1 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/javascript_alias.rs` |
| `internal/extract/javascript_test.go` | 3 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/javascript.rs` |
| `internal/extract/jscalls_test.go` | 1 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/jscalls.rs` |
| `internal/extract/jsts_imports_test.go` | 30 | 0 | 0 | js, python | - | `crates/graphify/src/extract/tests/jsts_imports.rs` |
| `internal/extract/jsts_parity_test.go` | 39 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/jsts_parity.rs` |
| `internal/extract/modulepath_test.go` | 1 | 0 | 0 | js | - | `crates/graphify/src/extract/tests/modulepath.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_js_import_resolution.py` | 90 | yes | internal/extract/jsts_imports_test.go, jsts_parity_test.go, javascript_alias_test.go |
| `tests/test_jsconfig_baseurl.py` | 13 | yes | internal/extract/javascript_alias_test.go |
| `test_4992_ts_import_types.py` | 3 | partial | internal/extract/jsts_imports_test.go |
| `test_astro_extraction.py` | 10 | partial | - |
| `test_builtin_global_type_refs.py` | 3 | partial | internal/extract/jsts_parity_test.go |
| `test_cjs_module_extension.py` | 6 | partial | internal/extract/javascript_test.go |
| `test_cross_extension_reexport_self_cycle.py` | 10 | partial | internal/extract/jsts_imports_test.go, jsts_parity_test.go |
| `test_read_path_imports.py` | 4 | partial | internal/extract/jsts_imports_test.go |
| `test_svelte_extraction.py` | 11 | partial | - |
| `test_ts_abstract_methods.py` | 4 | partial | internal/extract/javascript_test.go, jsts_parity_test.go |
| `test_ts_decorator_chains.py` | 5 | partial | internal/extract/jsts_parity_test.go |
| `test_ts_decorators.py` | 9 | partial | internal/extract/jsts_parity_test.go |
| `test_ts_export_type_star.py` | 5 | partial | internal/extract/jsts_imports_test.go |
| `test_ts_generators.py` | 5 | partial | internal/extract/javascript_test.go |
| `test_ts_import_require.py` | 5 | partial | internal/extract/jsts_imports_test.go |
| `test_ts_import_type_arguments.py` | 14 | partial | internal/extract/jsts_imports_test.go |
| `test_ts_inheritance.py` | 10 | partial | internal/extract/inherits_test.go |
| `test_ts_namespace.py` | 6 | partial | internal/extract/javascript_test.go |
| `test_ts_new_expression_calls.py` | 5 | partial | internal/extract/jscalls_test.go |
| `test_ts_parse_warning.py` | 6 | partial | internal/extract/degenerate_test.go |
| `test_ts_receiver_member_calls.py` | 38 | partial | internal/extract/jscalls_test.go, recvtail_test.go |
| `test_ts_typed_template_and_annotation.py` | 8 | partial | internal/extract/jsts_parity_test.go |
| `test_typescript_enum_members.py` | 12 | partial | internal/extract/javascript_test.go |
| `test_typescript_module_extensions.py` | 7 | partial | internal/extract/javascript_alias_test.go, detect_test.go |
| `test_unmapped_at_alias_resolution.py` | 9 | partial | internal/extract/javascript_alias_test.go |
| `tests/test_forwarding_review_findings.py` | 5 | partial | internal/extract/jsts_parity_test.go |
| `tests/test_import_extension_resolution.py` | 45 | partial | internal/extract/jsts_imports_test.go, javascript_alias_test.go, local_imports_test.go |
| `tests/test_imported_export_forwarding.py` | 14 | partial | internal/extract/jsts_parity_test.go |
| `tests/test_indirect_call_arrow_single_param_shadow.py` | 5 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_block_scoped_shadow.py` | 8 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_catch_binding_shadow.py` | 5 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_external_import_shadow.py` | 8 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_file_scope_callback_shadow.py` | 8 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_for_of_binding_shadow.py` | 3 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_function_expression_shadow.py` | 13 | partial | internal/extract/jscalls_test.go |
| `tests/test_indirect_call_nested_closure_shadow.py` | 6 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_callback_calls.py` | 8 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_cjs_barrel_require_call.py` | 1 | partial | internal/extract/jsts_parity_test.go |
| `tests/test_js_destructured_export.py` | 5 | partial | internal/extract/jsts_parity_test.go |
| `tests/test_js_dynamic_import_affected.py` | 7 | partial | internal/extract/jsts_imports_test.go, cmd/graphify/affected_stdin_test.go |
| `tests/test_js_dynamic_imports.py` | 17 | partial | internal/extract/jsts_imports_test.go |
| `tests/test_js_exported_fn_call_resolution.py` | 5 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_exported_scalar_bindings.py` | 3 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_extends_require_4688.py` | 10 | partial | internal/extract/javascript_test.go, inherits_test.go |
| `tests/test_js_member_call_bare_name_fallback.py` | 6 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_member_collisions.py` | 12 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_module_level_anonymous_calls.py` | 4 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_namespace_import_call_edges.py` | 5 | partial | internal/extract/jsts_imports_test.go |
| `tests/test_js_nested_function_scope.py` | 5 | partial | internal/extract/javascript_test.go |
| `tests/test_js_require_binding_shadow.py` | 1 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_require_external_package_call.py` | 2 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_this_member_expressions.py` | 5 | partial | internal/extract/jscalls_test.go |
| `tests/test_js_walk_order.py` | 2 | partial | internal/extract/javascript_test.go |
| `tests/test_jsx_component_usage_calls.py` | 8 | partial | internal/extract/component_test.go, jscalls_test.go |
| `tests/test_package_json_subpath_imports.py` | 8 | partial | internal/extract/jsts_imports_test.go |
| `tests/test_pnpm_workspace_globs.py` | 4 | partial | internal/extract/jsts_imports_test.go |

## `extract-java`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/java_chain_test.go` | 12 | 0 | 0 | java, python | - | `crates/graphify/src/extract/tests/java_chain.rs` |
| `internal/extract/java_test.go` | 2 | 0 | 0 | java | testdata | `crates/graphify/src/extract/tests/java.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_4874_jvm_resolution.py` | 9 | partial | internal/extract/java_test.go, java_chain_test.go |
| `tests/test_java_anonymous_class.py` | 3 | partial | internal/extract/java_test.go |
| `tests/test_java_enum_members.py` | 3 | partial | internal/extract/java_test.go |
| `tests/test_java_member_calls.py` | 32 | partial | internal/extract/java_chain_test.go, java_test.go |
| `tests/test_java_type_resolution.py` | 15 | partial | internal/extract/java_test.go, java_chain_test.go |

## `extract-csharp`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/csharp_calls_test.go` | 2 | 0 | 2 | csharp, python | - | `crates/graphify/src/extract/tests/csharp_calls.rs` |
| `internal/extract/csharp_dispatch_test.go` | 4 | 0 | 0 | csharp, java, js | - | `crates/graphify/src/extract/tests/csharp_dispatch.rs` |
| `internal/extract/csharp_test.go` | 1 | 0 | 0 | csharp | testdata | `crates/graphify/src/extract/tests/csharp.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_csharp_interface_dispatch.py` | 18 | yes | internal/extract/csharp_dispatch_test.go |
| `test_csharp_call_site_generic_args.py` | 7 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_delegate_declarations.py` | 2 | partial | internal/extract/csharp_test.go |
| `test_csharp_enum_members.py` | 12 | partial | internal/extract/csharp_test.go |
| `test_csharp_extension_calls.py` | 13 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_extension_metadata.py` | 9 | partial | internal/extract/csharp_test.go |
| `test_csharp_extension_resolution.py` | 16 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_field_generic_args.py` | 8 | partial | internal/extract/csharp_test.go |
| `test_csharp_foreach_loop_variable.py` | 7 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_generic_callsites.py` | 5 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_member_calls.py` | 50 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_member_chain_calls.py` | 15 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_member_nodes.py` | 18 | partial | internal/extract/csharp_test.go |
| `test_csharp_method_owner_labels.py` | 12 | partial | internal/extract/csharp_test.go |
| `test_csharp_object_creation.py` | 30 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_overloads.py` | 13 | partial | internal/extract/csharp_test.go |
| `test_csharp_partial_classes.py` | 29 | partial | internal/extract/csharp_test.go |
| `test_csharp_tuple_type_refs.py` | 2 | partial | internal/extract/csharp_test.go |
| `test_csharp_type_resolution.py` | 60 | partial | internal/extract/csharp_test.go, resolve_test.go |
| `test_csharp_var_call_return_type.py` | 9 | partial | internal/extract/csharp_calls_test.go |
| `test_csharp_verbatim_interpolation.py` | 8 | partial | internal/extract/csharp_test.go |
| `tests/test_dotnet.py` | 48 | partial | internal/extract/csharp_test.go, csharp_calls_test.go, csharp_dispatch_test.go |
| `tests/test_indirect_dispatch.py` | 28 | partial | internal/extract/dispatch_test.go, pycalls_test.go |
| `tests/test_indirect_dispatch_assign_return.py` | 7 | partial | internal/extract/dispatch_test.go |

## `extract-c`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/c_test.go` | 1 | 0 | 0 | c | testdata | `crates/graphify/src/extract/tests/c.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_c_include_resolution.py` | 13 | yes | internal/extract/c_test.go |

## `extract-cpp`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/cpp_test.go` | 4 | 0 | 0 | c/cpp, cpp | testdata | `crates/graphify/src/extract/tests/cpp.rs` |
| `internal/extract/preproc_test.go` | 6 | 0 | 0 | c, c/cpp, cpp, scala | - | `crates/graphify/src/extract/tests/preproc.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_cpp_preprocess.py` | 5 | yes | internal/extract/preproc_test.go |
| `test_cpp_alias_member_resolution.py` | 29 | partial | internal/extract/cpp_test.go |
| `test_cpp_anonymous_namespace_receivers.py` | 8 | partial | internal/extract/cpp_test.go |
| `test_cpp_conditional_initializers.py` | 20 | partial | internal/extract/cpp_test.go |
| `test_cpp_forward_declarations.py` | 14 | partial | internal/extract/cpp_test.go |
| `test_cpp_forward_receiver_completion.py` | 13 | partial | internal/extract/cpp_test.go |
| `test_cpp_incremental_type_resolution.py` | 2 | partial | internal/extract/cpp_test.go |
| `test_cpp_inherited_member_resolution.py` | 24 | partial | internal/extract/cpp_test.go, inherits_test.go |
| `test_cpp_local_callable_shadowing.py` | 3 | partial | internal/extract/cpp_test.go |
| `test_cpp_member_call_by_receiver_type.py` | 5 | partial | internal/extract/cpp_test.go, recvtail_test.go |
| `test_cpp_member_declaration_identity.py` | 9 | partial | internal/extract/cpp_test.go |
| `test_cpp_method_declarations.py` | 4 | partial | internal/extract/cpp_test.go |
| `test_cpp_nested_and_cli.py` | 20 | partial | internal/extract/cpp_test.go |
| `test_cpp_receiver_chains.py` | 12 | partial | internal/extract/cpp_test.go, recvtail_test.go |
| `test_cpp_receiver_parameters.py` | 6 | partial | internal/extract/cpp_test.go |
| `test_cpp_split_header_owner.py` | 4 | partial | internal/extract/cpp_test.go |
| `test_cpp_template_members.py` | 21 | partial | internal/extract/cpp_test.go |
| `test_cpp_template_type_params.py` | 2 | partial | internal/extract/cpp_test.go |
| `test_cpp_type_alias.py` | 11 | partial | internal/extract/cpp_test.go |

## `extract-hcl`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/nulllabel_test.go` | 11 | 0 | 1 | hcl | testdata | `crates/graphify/src/extract/tests/nulllabel.rs` |
| `internal/extract/terraform_test.go` | 10 | 0 | 0 | hcl | testdata | `crates/graphify/src/extract/tests/terraform.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_terraform.py` | 25 | yes | internal/extract/terraform_test.go |
| `test_terraform_modules.py` | 18 | partial | internal/extract/terraform_test.go |

## `extract-rust`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/rust_test.go` | 1 | 0 | 0 | rust | testdata | `crates/graphify/src/extract/tests/rust.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_rust_anonymous_const.py` | 1 | partial | internal/extract/rust_test.go |
| `test_rust_builtin_type_hubs.py` | 5 | partial | internal/extract/rust_test.go |
| `test_rust_case_collisions.py` | 8 | partial | internal/extract/rust_test.go, nodeid_test.go |
| `test_rust_crate_use_scope.py` | 18 | partial | internal/extract/rust_test.go, cargo_test.go |
| `test_rust_in_file_call_binding.py` | 7 | partial | internal/extract/rust_test.go |
| `test_rust_macro_argument_calls.py` | 9 | partial | internal/extract/rust_test.go |
| `test_rust_macro_rules.py` | 4 | partial | internal/extract/rust_test.go |
| `test_rust_scoped_call_resolution.py` | 15 | partial | internal/extract/rust_test.go |
| `test_rust_self_member_calls.py` | 16 | partial | internal/extract/rust_test.go |
| `test_rust_type_alias_union.py` | 6 | partial | internal/extract/rust_test.go |
| `test_rust_use_trees.py` | 18 | partial | internal/extract/rust_test.go |

## `extract-ruby`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/rake_resolve_test.go` | 1 | 0 | 0 | ruby | - | `crates/graphify/src/extract/tests/rake_resolve.rs` |
| `internal/extract/rake_skill_test.go` | 2 | 0 | 0 | ruby | - | `crates/graphify/src/extract/tests/rake_skill.rs` |
| `internal/extract/ruby_test.go` | 1 | 0 | 0 | ruby | testdata | `crates/graphify/src/extract/tests/ruby.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_ruby_bare_self_send_calls.py` | 8 | partial | internal/extract/ruby_test.go |
| `test_ruby_resolution.py` | 68 | partial | internal/extract/ruby_test.go, rake_resolve_test.go |

## `extract-php`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/php_test.go` | 2 | 0 | 0 | php | testdata | `crates/graphify/src/extract/tests/php.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_extract_php_closures.py` | 9 | partial | internal/extract/php_test.go |
| `tests/test_php_class_const_initializer_refs.py` | 7 | partial | internal/extract/php_test.go |
| `tests/test_php_language_construct_calls.py` | 12 | partial | internal/extract/php_test.go |
| `tests/test_php_member_collisions.py` | 10 | partial | internal/extract/php_test.go |
| `tests/test_php_object_creation.py` | 6 | partial | internal/extract/php_test.go |
| `tests/test_php_receiver_member_calls.py` | 12 | partial | internal/extract/php_test.go |
| `tests/test_php_static_call_resolution.py` | 9 | partial | internal/extract/php_test.go |
| `tests/test_php_type_resolution.py` | 8 | partial | internal/extract/php_test.go |
| `tests/test_php_unparsed_casts.py` | 5 | partial | internal/extract/php_test.go |

## `extract-bash`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/bash_scriptinvocation_test.go` | 9 | 0 | 1 | bash, js, php, python | - | `crates/graphify/src/extract/tests/bash_scriptinvocation.rs` |
| `internal/extract/bash_test.go` | 1 | 0 | 0 | bash | testdata | `crates/graphify/src/extract/tests/bash.rs` |

## `extract-scala`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/scala_test.go` | 1 | 0 | 0 | scala | testdata | `crates/graphify/src/extract/tests/scala.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `test_scala_abstract_methods.py` | 4 | partial | internal/extract/scala_test.go |
| `test_scala_context_bounds.py` | 22 | partial | internal/extract/scala_test.go |
| `test_scala_extension_methods.py` | 8 | partial | internal/extract/scala_test.go |
| `test_scala_generic_type_params.py` | 2 | partial | internal/extract/scala_test.go |
| `test_scala_given_instances.py` | 5 | partial | internal/extract/scala_test.go |
| `test_scala_self_type.py` | 8 | partial | internal/extract/scala_test.go |
| `test_scala_top_level_binding.py` | 8 | partial | internal/extract/scala_test.go |
| `test_scala_type_definition.py` | 10 | partial | internal/extract/scala_test.go |

## `extract-kotlin`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/kotlin_test.go` | 2 | 0 | 0 | kotlin | testdata | `crates/graphify/src/extract/tests/kotlin.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_kotlin_grammar.py` | 29 | yes | internal/extract/kotlin_test.go |
| `tests/test_kotlin_generic_type_params.py` | 2 | partial | internal/extract/kotlin_test.go |
| `tests/test_kotlin_object_literal.py` | 5 | partial | internal/extract/kotlin_test.go |
| `tests/test_kotlin_typealias_declarations.py` | 1 | partial | internal/extract/kotlin_test.go |

## `extract-julia`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/julia_test.go` | 1 | 0 | 0 | julia | testdata | `crates/graphify/src/extract/tests/julia.rs` |

## `extract-lua`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/lua_test.go` | 1 | 0 | 0 | lua | testdata | `crates/graphify/src/extract/tests/lua.rs` |

Upstream (port only the cases whose assertion holds for graphify-go v0.12.0):

| Upstream file | Tests | Go has | Go tests cited |
|---|---|---|---|
| `tests/test_lua_import.py` | 8 | yes | internal/extract/lua_test.go |
| `tests/test_lua_method_calls.py` | 5 | yes | internal/extract/lua_test.go |

## `extract-zig`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/zig_test.go` | 1 | 0 | 0 | zig | testdata | `crates/graphify/src/extract/tests/zig.rs` |

## `extract-verilog`

| Go file | Test | Fuzz | t.Run | Grammars named | Needs | Rust file |
|---|---|---|---|---|---|---|
| `internal/extract/verilog_test.go` | 1 | 0 | 0 | verilog | testdata | `crates/graphify/src/extract/tests/verilog.rs` |
