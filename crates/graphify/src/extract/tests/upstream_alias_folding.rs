//! graphify v0.9.107 `test_alias_folding.py` (19 test functions): none ported.
//!
//! Not ported, every case, for this reason: it tests the Python folding of drifted edge and node fields (`relationship`, `file`, numeric or lowercase confidence) in `build_from_json` and its validation; `resolve.go` has no alias folding and the Go model decodes fixed keys. Not a resolve behaviour.
//!
//! Cases:
//! - `test_edge_relationship_folds_to_relation`
//! - `test_edge_relation_wins_over_relationship`
//! - `test_node_file_folds_to_source_file`
//! - `test_node_path_folds_to_source_file`
//! - `test_node_filepath_folds_to_source_file`
//! - `test_node_src_folds_to_source_file`
//! - `test_node_source_file_wins_over_alias`
//! - `test_numeric_confidence_folds_to_inferred_and_backfills_score`
//! - `test_numeric_confidence_does_not_override_an_existing_score`
//! - `test_model_drifted_string_confidence_folds_to_inferred`
//! - `test_numeric_looking_string_confidence_backfills_score`
//! - `test_lowercase_confidence_is_case_corrected_not_treated_as_drift`
//! - `test_valid_enum_confidences_are_left_untouched`
//! - `test_hyperedge_confidence_folds_the_same_way`
//! - `test_build_from_json_normalizes_drifted_confidence_without_warning`
//! - `test_build_from_json_normalizes_without_validation_warnings`
//! - `test_node_confidence_folds_the_same_way`
//! - `test_node_without_confidence_is_left_untouched`
//! - `test_build_from_json_normalizes_node_confidence`
