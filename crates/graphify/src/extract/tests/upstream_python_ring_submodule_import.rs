//! graphify v0.9.107 `test_type_only_import_cycles.py`,
//! `test_python_ring_through_a_submodule_import_and_a_guard_keeps_its_cycle` (3 parametrized
//! cases: undirected, directed, no-cluster raw): not ported.
//!
//! Not ported, every case, for this reason: it asserts the cycles `_cycles` finds in the built
//! graph (`build_from_json`, directed or not, or the raw node list), that is the analyze stage
//! (graphify-rs-ihc.10), not resolve. The stamp it depends on (`from . import b` plus a guarded
//! `from .b import B`) is in tests/python_imports.rs (`type checking`) and the Python stamp cases
//! of tests/python_type_only.rs.
