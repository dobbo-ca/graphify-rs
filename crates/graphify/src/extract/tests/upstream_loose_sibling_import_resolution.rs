//! graphify v0.9.107 `test_loose_sibling_import_resolution.py` (8 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases assert upstream node ids (`scripts_main`) and the `imports` relation for a resolved sibling module; node ids differ by design (GOALS.md:83) and graphify-go gives `imports_from` for a corpus file (tests/python_imports.rs).
//!
//! Cases:
//! - `test_primary_regression_loose_sibling_import_and_call`
//! - `test_scan_root_parity`
//! - `test_same_name_modules_in_separate_loose_directories`
//! - `test_package_isolation_pep328`
//! - `test_no_cross_directory_leakage`
//! - `test_aliased_import_resolves_member_call`
//! - `test_bare_stdlib_import_does_not_bind_to_same_stem_sibling`
//! - `test_non_stdlib_sibling_still_repoints_control`
