//! graphify v0.9.107 `test_phantom_external_import.py` (7 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases assert upstream's `ref`-prefixed id of an unresolved bare import (`_resolve_js_import_target`) and graph shapes of the Python `build_from_json`; graphify-go names an external import by `make_id(spec)`, so the `ref` assertions fail, and the rest are not traced.
//!
//! Cases:
//! - `test_unresolved_bare_import_is_ref_namespaced`
//! - `test_scoped_package_import_is_ref_namespaced`
//! - `test_package_subpath_import_resolves_to_the_same_target_as_a_bare_import`
//! - `test_scoped_package_subpath_import_resolves_to_the_scope_and_package_only`
//! - `test_no_phantom_edge_from_tsx_to_unrelated_python_file`
//! - `test_multiple_tsx_files_do_not_all_alias_onto_one_python_file`
//! - `test_subpath_import_does_not_dangle_like_a_bare_import`
