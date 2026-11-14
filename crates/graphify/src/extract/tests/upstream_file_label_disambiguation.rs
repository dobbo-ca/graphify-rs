//! graphify v0.9.107 `test_file_label_disambiguation.py` (6 test functions): none ported.
//!
//! Not ported, every case, for this reason: it tests `build.py` relabelling file nodes that share
//! a basename (`_disambiguate_file_node_labels`, `disambiguate_file_labels_in_nodes`,
//! `_shortest_unique_suffix`) and `serve._find_node`. graphify-go v0.12.0 has no such
//! relabelling (no function of `internal` does it), so the assertions do not hold for it.
//!
//! Cases:
//! - `test_disambiguate_raw_node_list_for_no_cluster_path`
//! - `test_is_file_node_label_and_suffix_helpers`
//! - `test_colliding_file_labels_are_qualified_uniques_left_bare`
//! - `test_disambiguation_is_idempotent`
//! - `test_three_way_collision_grows_suffix_until_unique`
//! - `test_end_to_end_build_and_lookup`
