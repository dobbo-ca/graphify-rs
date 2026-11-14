//! graphify v0.9.107 `test_cross_repo_member_calls.py` (33 test functions): none ported.
//!
//! Not ported, every case, for this reason: it tests `link_cross_repo_member_calls` and `merge-graphs` (cross-repo linking); graphify-go has no cross-repo pass.
//!
//! Cases:
//! - `test_a_parked_call_binds_to_the_one_declaration_in_another_repo`
//! - `test_edge_source_file_is_the_parked_call_site_not_the_caller_node_file`
//! - `test_a_pre_3152_parked_entry_without_file_falls_back_to_the_caller_file`
//! - `test_two_repos_declaring_the_same_name_bind_nothing`
//! - `test_a_declaration_in_the_callers_own_repo_binds_nothing`
//! - `test_a_declaration_in_another_language_binds_nothing`
//! - `test_a_cpp_call_does_not_bind_to_a_csharp_declaration`
//! - `test_a_php_call_binds_to_a_php_declaration_in_another_repo`
//! - `test_a_php_call_does_not_bind_to_a_java_declaration`
//! - `test_a_cpp_header_declaration_answers_through_defines`
//! - `test_a_defines_member_does_not_answer_a_java_call`
//! - `test_a_typescript_call_binds_across_the_whole_js_family`
//! - `test_a_typescript_call_does_not_bind_to_a_python_declaration`
//! - `test_an_objc_selector_answers_through_either_sigil`
//! - `test_an_objc_callee_that_already_carries_a_sigil_still_binds`
//! - `test_a_type_declaring_both_objc_sigils_binds_nothing`
//! - `test_an_objc_call_does_not_bind_to_a_cpp_member_of_a_shared_header`
//! - `test_an_objc_protocol_does_not_answer_a_parked_call`
//! - `test_an_objc_class_split_decl_impl_across_repos_binds_to_the_definition`
//! - `test_two_objc_definitions_of_the_same_class_still_bind_nothing`
//! - `test_the_definition_answers_before_a_same_named_declaration`
//! - `test_a_sourceless_stub_does_not_answer_a_parked_call`
//! - `test_a_declaration_without_that_method_binds_nothing`
//! - `test_an_unknown_language_binds_nothing`
//! - `test_running_twice_does_not_duplicate_the_edge`
//! - `test_a_repo_that_stops_declaring_the_type_loses_the_edge`
//! - `test_a_go_call_does_not_bind_to_a_java_declaration`
//! - `test_a_go_call_does_not_bind_to_a_differently_cased_method`
//! - `test_merge_graphs_cli_finishes_the_parked_call`
//! - `test_a_java_build_parks_the_call_and_the_merge_finishes_it`
//! - `test_each_language_parks_the_call_and_the_merge_finishes_it`
//! - `test_cpp_call_conditions_survive_parking_and_merge`
//! - `test_cpp_condition_metadata_is_copied_selectively_and_relinks_stably`
