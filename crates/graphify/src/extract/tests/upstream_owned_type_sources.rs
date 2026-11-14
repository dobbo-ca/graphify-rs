//! graphify v0.9.107 `test_owned_type_sources.py` (3 test functions): none ported.
//!
//! Not ported, every case, for this reason: the cases assert upstream node ids (`local_visible_method`), which differ by design (GOALS.md:83); how graphify-go handles a class local to a function or callback is not traced.
//!
//! Cases:
//! - `test_callback_local_method_preserved_named_function_local_omitted`
//! - `test_same_basename_sources_do_not_suppress_other_callers`
//! - `test_abstract_method_omissions_do_not_drop_concrete_method`
