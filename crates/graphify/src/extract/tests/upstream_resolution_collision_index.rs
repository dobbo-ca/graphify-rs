//! graphify v0.9.107 `test_resolution_collision_index.py` (2 test functions): none ported.
//!
//! Not ported, every case, for this reason: it tests the Python `_collision_aware` pass over salted sibling node ids (`m_base_ab12cd`) against a copy of the old loop; graphify-go node ids have no salted siblings.
//!
//! Cases:
//! - `test_same_caller_as_the_old_scan`
//! - `test_passes_over_nodes_do_not_grow_with_uses`
