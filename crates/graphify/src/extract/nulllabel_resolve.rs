//! Port of `nulllabel_resolve.go`. Owner: the gostd stream (graphify-rs-ihc.31).
//!
//! `resolve.go` calls one function of this file, `resolveNullLabels` (`resolve.go:1036`): it is
//! shared API, its signature proposed here (see [`resolve_null_labels`]). The rest are helpers.

use std::collections::BTreeMap;

use super::extract::{LabelInputs, ModInvoke, NullLabelRef, Result, SegVal};
use super::nulllabel::{LABEL_SCALARS, SegState, compose_id};
use super::terraform::LIST_SEP;
use crate::idutil::make_id;
use crate::model::{Extraction, Relation};

/// Go `maxChainDepth`: how many local wrapper hops a var-ref or context is followed.
const MAX_CHAIN_DEPTH: usize = 8;

/// The callers of each wrapper directory, in edge order (Go `callersByDir`).
type CallersByDir<'r> = BTreeMap<String, Vec<&'r ModInvoke>>;

/// Go `in.scalars[k]`: the zero `SegVal` (Unknown) when absent.
fn scalar(inp: &LabelInputs, k: &str) -> SegVal {
    inp.scalars.get(k).cloned().unwrap_or_default()
}

/// Go `resolveNullLabels`: completes partial cloudposse null-label ids by following LOCAL
/// wrapper-module chains, from the single local caller that invoked the wrapper directory. For
/// multi-caller dirs, remote/registry hops, or anything unresolvable it leaves the Stage B
/// (partial) id untouched, never fabricating an exact id.
///
/// Shared with resolve.rs (graphify-rs-ihc.18): signature proposed here; the resolve owner may change it.
pub(crate) fn resolve_null_labels(results: &[Result], out: &mut Extraction) {
    // Without a null-label the indexes below serve nothing.
    if !results.iter().any(|r| !r.null_labels.is_empty()) {
        return;
    }
    // Index invocations by their module node id.
    let mut inv_by_node: BTreeMap<&str, &ModInvoke> = BTreeMap::new();
    for r in results {
        for m in &r.mod_invokes {
            inv_by_node.insert(&m.node_id, m);
        }
    }

    // node_label maps a node id to its label (the last node of an id wins, as Go's map).
    let mut node_label: BTreeMap<&str, &str> = BTreeMap::new();
    for n in &out.nodes {
        node_label.insert(&n.id, &n.label);
    }

    // callers_by_dir maps a wrapper directory to the invocations that called it, from the
    // module-source pass's `references` edges into tfmodule dir nodes (Label is the dir path).
    let mut dir_label: BTreeMap<&str, &str> = BTreeMap::new(); // dir-node id -> dir path label
    for (&id, &label) in &node_label {
        if make_id(&["tfmodule", label]) == id {
            dir_label.insert(id, label);
        }
    }
    let mut callers_by_dir: CallersByDir<'_> = BTreeMap::new();
    for e in &out.edges {
        if e.relation != Relation::REFERENCES {
            continue;
        }
        let Some(dir) = dir_label.get(e.target.as_str()) else {
            continue;
        };
        if let Some(m) = inv_by_node.get(e.source.as_str()) {
            callers_by_dir
                .entry((*dir).to_string())
                .or_default()
                .push(*m);
        }
    }

    // Index null-label refs (by node) so a `module.Y.context` reference can inherit from
    // another null-label module in the same corpus.
    let mut nl_by_node: BTreeMap<&str, &NullLabelRef> = BTreeMap::new();
    for r in results {
        for nl in &r.null_labels {
            nl_by_node.insert(&nl.node_id, nl);
        }
    }
    // nl_by_addr: key "<dir>\x00module.<y>". Go ranges a map here (`nulllabel_resolve.go:80`),
    // so two refs with one key would make the winner random; Terraform's duplicate-name rule
    // rules that out, and the id order here is fixed.
    let mut nl_by_addr: BTreeMap<String, &NullLabelRef> = BTreeMap::new();
    for nl in nl_by_node.values() {
        let label = node_label.get(nl.node_id.as_str()).copied().unwrap_or("");
        let addr = label.strip_suffix(" [null-label]").unwrap_or(label);
        nl_by_addr.insert(format!("{}\x00{addr}", nl.scope), nl);
    }

    for r in results {
        for nl in &r.null_labels {
            let before = compose_id(&nl.inputs);
            let before_exact =
                !before.is_empty() && !before.contains('{') && !before.ends_with("(partial)");
            if before_exact {
                continue; // already EXACT
            }
            let resolved = resolve_one_null_label(nl, &callers_by_dir, &nl_by_addr, 0);
            let after = compose_id(&resolved);
            if after.is_empty() || after == before {
                continue;
            }
            let after_exact = !after.contains('{') && !after.ends_with("(partial)");
            // Replace only when the new id is a strict improvement: it became fully exact, or
            // it carries more concrete segments than before.
            if (!after_exact || before_exact) && exact_count(&after) <= exact_count(&before) {
                continue;
            }
            set_computed(out, &nl.node_id, &after);
        }
    }
}

/// Go `resolveOneNullLabel`: a copy of `nl.inputs` with var-ref and context fields filled in
/// where possible. With exactly one local caller of the wrapper dir, `var.X` fields are filled
/// from that caller's args; a `context =` module ref inherits from its sibling. Bounded by
/// [`MAX_CHAIN_DEPTH`].
fn resolve_one_null_label(
    nl: &NullLabelRef,
    callers_by_dir: &CallersByDir<'_>,
    nl_by_addr: &BTreeMap<String, &NullLabelRef>,
    depth: usize,
) -> LabelInputs {
    let m = match callers_by_dir.get(&nl.scope) {
        Some(callers) if callers.len() == 1 => Some(callers[0]),
        _ => None,
    };

    let mut res = nl.inputs.clone(); // Go cloneInputs

    // Fill scalar fields.
    for k in LABEL_SCALARS {
        if scalar(&res, k).state == SegState::Known {
            continue;
        }
        if let Some(vname) = nl.inputs.var_refs.get(k)
            && let Some(m) = m
        {
            let (v, st) = resolve_arg(m, vname, callers_by_dir, depth);
            if st == SegState::Known && !v.is_empty() {
                res.scalars.insert(
                    k.to_string(),
                    SegVal {
                        val: v,
                        state: SegState::Known,
                    },
                );
            }
        }
    }

    // Fill attributes from a var-ref to a list argument.
    if res.attr_state != SegState::Known
        && let Some(vname) = nl.inputs.var_refs.get("attributes")
        && let Some(m) = m
    {
        let (v, st) = resolve_arg(m, vname, callers_by_dir, depth);
        if st == SegState::Known {
            res.attrs = if v.is_empty() {
                Vec::new()
            } else {
                v.split(LIST_SEP).map(String::from).collect()
            };
            res.attr_state = SegState::Known;
        }
    }

    // Context inheritance: a `context = module.<y>` (sibling null-label in the same dir)
    // contributes fields the child did not set (override-else-inherit).
    if !nl.inputs.context_ref.is_empty()
        && depth < MAX_CHAIN_DEPTH
        && let Some(parent) = nl_by_addr.get(&format!("{}\x00{}", nl.scope, nl.inputs.context_ref))
    {
        let pin = resolve_one_null_label(parent, callers_by_dir, nl_by_addr, depth + 1);
        inherit_context(&mut res, &pin);
    }

    res
}

/// Go `resolveArg`: a wrapper argument as a literal, from the caller's `args` or a transitive
/// pass-through via the wrapper's own single caller; ("", Unknown) otherwise.
fn resolve_arg(
    m: &ModInvoke,
    vname: &str,
    callers_by_dir: &CallersByDir<'_>,
    depth: usize,
) -> (String, SegState) {
    if let Some(sv) = m.args.get(vname) {
        return (sv.val.clone(), sv.state);
    }
    if let Some(outer_var) = m.arg_var_refs.get(vname)
        && depth < MAX_CHAIN_DEPTH
        && let Some(callers) = callers_by_dir.get(&m.dir)
        && callers.len() == 1
    {
        return resolve_arg(callers[0], outer_var, callers_by_dir, depth + 1);
    }
    (String::new(), SegState::Unknown)
}

/// Go `inheritContext`: copies parent fields into `child` where the child left them unset
/// (Unknown/Empty), mirroring cloudposse override-else-inherit. Attributes merge parent-first,
/// then dedup in `compose_id`.
fn inherit_context(child: &mut LabelInputs, parent: &LabelInputs) {
    for k in LABEL_SCALARS {
        if scalar(child, k).state == SegState::Known {
            continue; // child overrides the inherited value
        }
        let p = scalar(parent, k);
        match p.state {
            SegState::Known => {
                child.scalars.insert(k.to_string(), p);
            }
            // The parent contributes this field but its value is unknown; the child's
            // effective value is unknown too, so it renders as a {seg} placeholder.
            SegState::Unknown => {
                child.scalars.insert(k.to_string(), SegVal::default());
            }
            SegState::Empty => {}
        }
    }
    if parent.attr_state == SegState::Known {
        let mut merged = parent.attrs.clone();
        if child.attr_state == SegState::Known {
            merged.extend(child.attrs.iter().cloned());
        }
        child.attrs = merged;
        child.attr_state = SegState::Known;
    }
    // Clear the child's "pending inheritance" markers ONLY when the resolved parent is itself
    // fully exact; a partial parent keeps the child partial (a gap is safer than a wrong name).
    let pid = compose_id(parent);
    let parent_exact = !pid.is_empty() && !pid.contains('{') && !pid.ends_with(" (partial)");
    if parent_exact {
        child.has_context = false;
        child.context_ref.clear();
    }
}

/// Go `exactCount`: the concrete (non-"{seg}") segments in a composed id.
fn exact_count(id: &str) -> usize {
    let id = id.strip_suffix(" (partial)").unwrap_or(id);
    id.split('-')
        .filter(|seg| !seg.is_empty() && !seg.starts_with('{'))
        .count()
}

/// Go `setComputed`: the first node of `id` gets `name` as its computed name.
fn set_computed(out: &mut Extraction, id: &str, name: &str) {
    if let Some(n) = out.nodes.iter_mut().find(|n| n.id == id) {
        n.computed_name = name.to_string();
    }
}
