//! graphify v0.9.107 `test_import_self_loops.py` (3 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases check a self loop in the Python `build_from_json` graph and the `imports` relation of upstream's resolution; graphify-go's relation for a corpus file is `imports_from` and the Go builds are not traced. The Go counterparts are the 'go external test package skips itself' case of tests/local_imports.rs and `TestJSImportJsSelfSkip` of tests/handoff_js_resolve.rs.
//!
//! Cases:
//! - `test_python_external_import_matching_current_basename_has_no_self_loop`
//! - `test_rust_import_matching_current_basename_has_no_self_loop`
//! - `test_recursive_call_self_loop_is_preserved`
