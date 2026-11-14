//! graphify v0.9.107 `test_symbol_resolution.py` (42 test functions): none ported.
//!
//! Not ported, every case, for this reason: it tests the Python module `graphify.symbol_resolution` (label index, `raw_calls` dicts, bash source edges) through its internal API; graphify-go has no such API. Its behaviour is covered by tests/resolve.rs and tests/python_import_guided.rs.
//!
//! Cases:
//! - `test_normalise_callable_label_strips_function_punctuation`
//! - `test_node_is_resolvable_symbol_skips_rationale_and_doc_tags`
//! - `test_build_label_index_collects_unique_symbols`
//! - `test_resolve_cross_file_raw_calls_emits_unique_unqualified_call`
//! - `test_resolve_cross_file_raw_calls_skips_member_calls`
//! - `test_resolve_cross_file_raw_calls_skips_ambiguous_duplicate_labels`
//! - `test_resolve_cross_file_raw_calls_real_edge_survives_test_mock`
//! - `test_resolve_cross_file_raw_calls_n_mock_scale`
//! - `test_resolve_cross_file_raw_calls_call_site_is_test_prefers_test_local`
//! - `test_resolve_cross_file_raw_calls_skips_existing_pair`
//! - `test_parse_python_import_aliases_supports_from_import_alias`
//! - `test_build_python_symbol_index_uses_module_stem_and_label`
//! - `test_find_unique_python_symbol_returns_none_when_ambiguous`
//! - `test_resolve_python_import_guided_calls_emits_extracted_edge`
//! - `test_bash_call_resolver_emits_source_edges`
//! - `test_bash_call_resolver_emits_call_edges_from_sourced_files`
//! - `test_bash_call_resolver_skips_existing_pair`
//! - `test_bash_call_resolver_skips_ambiguous_multiple_candidates`
//! - `test_bash_call_resolver_skips_non_bash_raw_calls`
//! - `test_bash_make_id_identical_to_make_id`
//! - `test_bash_make_id_unicode_matches_make_id`
//! - `test_parse_python_import_aliases_skips_function_local_imports`
//! - `test_parse_python_import_aliases_accepts_top_level_import`
//! - `test_node_is_resolvable_symbol_requires_code_file_type`
//! - `test_build_label_index_excludes_non_code_nodes`
//! - `test_resolve_bash_source_edges_skips_malformed_source`
//! - `test_resolve_bash_source_edges_skips_bash_function_node_missing_id`
//! - `test_resolve_bash_source_edges_skips_raw_call_missing_caller_nid`
//! - `test_resolve_bash_source_edges_accepts_none_per_file_entries`
//! - `test_resolve_bash_source_edges_skips_non_dict_lists`
//! - `test_resolve_bash_source_edges_relative_path_resolves_against_source_dir`
//! - `test_iter_raw_calls_skips_non_dict_per_file_entries`
//! - `test_iter_raw_calls_skips_non_list_raw_calls`
//! - `test_iter_raw_calls_drops_non_dict_items_in_list`
//! - `test_resolve_cross_file_raw_calls_survives_malformed_raw_calls`
//! - `test_resolve_python_import_guided_calls_survives_malformed_raw_calls`
//! - `test_resolve_bash_source_edges_skips_unhashable_callee`
//! - `test_resolve_python_import_guided_calls_non_dict_per_file_slot`
//! - `test_resolve_python_import_guided_calls_per_file_shorter_than_paths`
//! - `test_resolve_python_import_guided_calls_per_file_none_slot`
//! - `test_resolve_python_import_guided_calls_metadata_is_sanitized`
//! - `test_resolve_python_import_guided_calls_metadata_sanitizes_hostile_alias`
