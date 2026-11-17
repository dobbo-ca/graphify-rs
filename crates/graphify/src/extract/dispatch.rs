//! Port of `dispatch.go`: C# interface dispatch (`dispatches_to` edges).
//!
//! [`resolve_c_sharp_dispatch`] is shared API (graphify-rs-ihc.18): its signature is proposed
//! here, Go-shaped; the resolve owner may change it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::resolve::contains;
use crate::model::{Confidence, Edge, Node, Relation, file_of, loc_of};

/// Go `resolveCSharpDispatch` (`dispatch.go:27`): links a C# interface's method to the
/// implementing method when the interface has exactly one implementer owning exactly one
/// same-named method.
///
/// A call through a constructor-injected dependency (`_report.Build()` where `_report` is an
/// `IReport`) resolves to the interface's method node, because that is what the call site names.
/// Nothing joins it to `Report.Build()`, so a directed walk stops at the interface. One
/// `dispatches_to` edge per method reconnects every call that reaches it.
///
/// Single-owner only: walking `implements` to the one type that can serve the call mirrors the
/// runtime, guessing among several implementers would not. Purely additive: the call edge to the
/// interface method stays. Confidence is INFERRED. It only reads nodes and edges and returns the
/// edges it adds.
///
/// Go ranges over two maps (`dispatch.go:76`, `:89`), so its edge order is random; `build_parts`
/// sorts the edges later. Here the maps are `BTreeMap`s: the order is by interface id, then by
/// method name.
///
/// Shared with resolve.rs (graphify-rs-ihc.18): signature proposed here; the resolve owner may change it.
pub(crate) fn resolve_c_sharp_dispatch(nodes: &[Vec<Node>], edges: &[Vec<Edge>]) -> Vec<Edge> {
    // Both ends of every edge made here are C# nodes.
    if !nodes.iter().flatten().any(|n| is_c_sharp_node(Some(n))) {
        return Vec::new();
    }
    // The last node of an id, kept only when it is a C# one: every use below asks for that.
    let mut node: HashMap<&str, &Node> = HashMap::new();
    for n in nodes.iter().flatten() {
        if is_c_sharp_node(Some(n)) {
            node.insert(&n.id, n);
        } else if !node.is_empty() {
            node.remove(n.id.as_str());
        }
    }

    // interface id -> implementer ids
    let mut implementers: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    // owner id -> bare name -> method ids
    let mut methods: BTreeMap<&str, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for e in edges.iter().flatten() {
        if e.relation == Relation::IMPLEMENTS {
            implementers.entry(&e.target).or_default().insert(&e.source);
        } else if e.relation == Relation::CONTAINS {
            let Some(m) = node.get(e.target.as_str()) else {
                continue;
            };
            let name = bare_method_name(&m.label);
            if name.is_empty() {
                continue;
            }
            let ids = methods
                .entry(&e.source)
                .or_default()
                .entry(name.to_string())
                .or_default();
            if !contains(ids, &e.target) {
                ids.push(e.target.clone());
            }
        }
    }

    let mut made = Vec::new();
    for (iface_id, impls) in &implementers {
        if impls.len() != 1 {
            continue;
        }
        let Some(&impl_id) = impls.first() else {
            continue;
        };
        // Both ends must be C# declarations: `implements` is resolved by name, so a
        // cross-language pair is a name collision, not an implementation.
        if !is_c_sharp_node(node.get(iface_id).copied())
            || !is_c_sharp_node(node.get(impl_id).copied())
        {
            continue;
        }
        let Some(declared_by_name) = methods.get(iface_id) else {
            continue;
        };
        for (name, declared) in declared_by_name {
            let candidates: &[String] = methods
                .get(impl_id)
                .and_then(|m| m.get(name))
                .map_or(&[], Vec::as_slice);
            if declared.len() != 1 || candidates.len() != 1 || declared[0] == candidates[0] {
                continue;
            }
            let imp = node[candidates[0].as_str()];
            made.push(Edge {
                source: declared[0].clone(),
                target: candidates[0].clone(),
                relation: Relation::DISPATCHES_TO,
                confidence: Confidence::INFERRED,
                source_file: file_of(&imp.source_file),
                source_location: loc_of(&imp.source_location),
                ..Edge::default()
            });
        }
    }
    made
}

/// Go `isCSharpNode`.
fn is_c_sharp_node(n: Option<&Node>) -> bool {
    n.is_some_and(|n| n.source_file.ends_with(".cs"))
}

/// Go `bareMethodName`: a method node's own name: `Report.Build()` -> `Build`. Case is kept: C#
/// is case sensitive. The name is cut at the first parenthesis rather than by stripping a
/// trailing `()`, so a label that ever carried a signature still matches.
fn bare_method_name(label: &str) -> &str {
    let label = label.find('(').map_or(label, |i| &label[..i]);
    label.rfind('.').map_or(label, |i| &label[i + 1..])
}
