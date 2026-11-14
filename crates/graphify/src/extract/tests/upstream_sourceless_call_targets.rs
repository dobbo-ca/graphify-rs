//! graphify v0.9.107 `test_sourceless_call_targets.py` (6 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases assert upstream edge attributes (`context == return_type`) and Python module layouts; the Go counterparts are `TestReferencesBindOrStub` and `gotargets_test.go`, ported in tests/references.rs and tests/gotargets.rs. Each case is not traced through graphify-go's extractor, so it is not ported.
//!
//! Cases:
//! - `test_sourceless_external_call_target_is_not_emitted`
//! - `test_source_backed_exception_call_is_preserved`
//! - `test_constructor_call_survives_same_file_annotation_stub`
//! - `test_csharp_new_survives_generic_arg_stub_without_binding_members`
//! - `test_stub_rescued_call_needs_import_or_scope_evidence`
//! - `test_csharp_stub_rescued_call_follows_using`
