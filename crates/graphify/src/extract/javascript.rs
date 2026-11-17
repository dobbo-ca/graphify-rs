//! Port of `javascript.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.26.

use super::extract::Result;

/// Go `extractJSFile`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_js_file(
    rel: &str,
    src: &[u8],
    lang: &'static gositter::Language,
) -> Option<Result> {
    todo!("graphify-rs-ihc.26")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`). Empty
/// until graphify-rs-ihc.26.
pub(super) fn register_kinds() {}

/// Go `jsNestedCall` (`javascript.go:1401`): binds a call to a nested function seen from its
/// scope, once per (caller, callee). Go keeps `jsLinked` on the builder; here each language
/// passes its own set. Shared with python.go.
pub(crate) fn js_nested_call(
    b: &mut super::builder::Builder,
    linked: &mut std::collections::HashSet<(String, String)>,
    caller_id: &str,
    id: &str,
    loc: crate::model::Loc,
) {
    if caller_id == id || !linked.insert((caller_id.to_string(), id.to_string())) {
        return;
    }
    b.res.edges.push(crate::model::Edge {
        source: caller_id.to_string(),
        target: id.to_string(),
        relation: crate::model::Relation::CALLS,
        confidence: crate::model::Confidence::EXTRACTED,
        source_file: b.src,
        source_location: loc,
        ..crate::model::Edge::default()
    });
}

#[cfg(test)]
#[path = "tests/jsts_parity.rs"]
mod jsts_parity_tests;

#[cfg(test)]
#[path = "tests/javascript.rs"]
mod javascript_tests;

#[cfg(test)]
#[path = "tests/jscalls.rs"]
mod jscalls_tests;

#[cfg(test)]
#[path = "tests/jsts_imports.rs"]
mod jsts_imports_tests;

#[cfg(test)]
#[path = "tests/js_type_only.rs"]
mod js_type_only_tests;
