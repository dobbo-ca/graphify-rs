//! graphify v0.9.107 `test_issue_3472_source_file_collision.py` (5 test functions): none ported.
//!
//! Not ported, every case, for this reason: it tests the merge of semantic and AST nodes whose `source_file` is an absolute path in the Python `build`; not a resolve behaviour.
//!
//! Cases:
//! - `test_semantic_absolute_path_same_file_retains_attributes`
//! - `test_semantic_absolute_path_inverse_arrival_order`
//! - `test_build_merge_infers_root_for_semantic_absolute_path`
//! - `test_genuine_cross_file_collision_remains_isolated`
//! - `test_definition_file_normalized_before_dedup`
