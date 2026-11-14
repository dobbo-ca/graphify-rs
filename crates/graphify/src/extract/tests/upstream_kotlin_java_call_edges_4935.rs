//! graphify v0.9.107 `test_kotlin_java_call_edges_4935.py` (17 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases assert `File:label` pairs and overload node ids of upstream's Kotlin and Java member-call resolvers; each is not traced through graphify-go's Kotlin and Java extractors. Go's Kotlin tests are in tests/kotlin.rs and tests/java_chain.rs.
//!
//! Cases:
//! - `test_kotlin_call_to_a_non_overloaded_java_method_resolves`
//! - `test_a_property_initializer_call_still_resolves`
//! - `test_two_classes_with_the_same_name_stay_ambiguous`
//! - `test_inherited_in_repo_members_still_resolve`
//! - `test_kotlin_call_to_an_overloaded_java_method_resolves`
//! - `test_kotlin_call_to_an_overloaded_kotlin_object_method_resolves`
//! - `test_an_init_block_call_attaches_to_its_class`
//! - `test_an_init_block_call_to_an_overloaded_method_resolves`
//! - `test_a_class_with_a_property_and_an_init_block_gets_both_edges`
//! - `test_an_init_block_is_attributed_like_a_property_initializer_in_a_nested_class`
//! - `test_object_and_companion_init_blocks_produce_an_edge`
//! - `test_an_external_base_does_not_bind_a_bare_call_to_an_unrelated_class`
//! - `test_a_java_class_with_an_external_base_gets_the_same_guard`
//! - `test_an_in_repo_base_that_extends_an_external_class_still_taints_its_children`
//! - `test_a_top_level_function_is_still_called_from_a_class_with_an_external_base`
//! - `test_a_member_imported_by_name_survives_an_external_base_in_kotlin`
//! - `test_a_member_imported_by_name_survives_an_external_base_in_java`
