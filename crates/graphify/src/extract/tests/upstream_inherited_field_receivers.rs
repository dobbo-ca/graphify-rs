//! graphify v0.9.107 `test_inherited_field_receivers.py` (9 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases assert upstream labels (`.run()`, `.greet()`); graphify-go labels carry the owner (`Pair.run()`), so the assertion differs, and the field-table chain is not traced through the Go Java and C# extractors.
//!
//! Cases:
//! - `test_java_same_file_inherited_field_types_the_receiver`
//! - `test_java_cross_file_inherited_field_types_the_receiver`
//! - `test_java_grandparent_field_is_reachable_through_the_chain`
//! - `test_java_a_redeclared_field_uses_the_nearest_declaration`
//! - `test_java_untyped_bare_receiver_still_produces_nothing`
//! - `test_csharp_same_file_inherited_field_types_the_receiver`
//! - `test_objc_property_on_the_superclass_types_self_field`
//! - `test_binder_matches_by_label_and_disambiguates_by_file`
//! - `test_binder_skips_what_it_cannot_disambiguate`
