//! Port of `nulllabel.go`. Owner: the gostd stream (graphify-rs-ihc.31).
//!
//! Helpers only. The functions `resolve.rs` calls are shared API: their signatures are
//! stubbed by the extractors-and-CLI stream with the resolve tests PR (graphify-rs-ihc.18).
//!
//! Go's `segVal` and `labelInputs` are `SegVal` and `LabelInputs` in `extract.rs` (they ride in
//! `Result`); their fields are the Go fields, in snake case.

use std::collections::{BTreeMap, HashSet};

use gositter::Node;
use gostd::strings::{to_lower, to_upper};

use super::builder::node_text;
use super::extract::{LabelInputs, SegVal};
use super::terraform::{tf_child, tf_ref_address};

/// Go `segState`: how a captured label segment reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SegState {
    /// Go `segUnknown`: the value cannot be read from local literals.
    #[default]
    Unknown,
    /// Go `segKnown`: a definite literal value (possibly empty string).
    Known,
    /// Go `segEmpty`: explicitly null / unset, dropped from the id.
    Empty,
}

/// Go `labelScalars`.
pub(super) const LABEL_SCALARS: [&str; 5] = ["namespace", "tenant", "environment", "stage", "name"];
/// Go `defaultLabelOrder`.
const DEFAULT_LABEL_ORDER: [&str; 5] = ["namespace", "environment", "stage", "name", "attributes"];

fn seg(state: SegState) -> SegVal {
    SegVal {
        val: String::new(),
        state,
    }
}

/// Go `nullLabelInputs`: the null-label fields of a module body. Only literal values are read;
/// anything dynamic becomes `Unknown`, and a non-literal knob marks the whole shape unresolved.
pub(super) fn null_label_inputs(body: Option<Node<'_>>, src: &[u8]) -> LabelInputs {
    let mut exprs = BTreeMap::new();
    if let Some(body) = body {
        for a in (0..body.named_child_count()).filter_map(|i| body.named_child(i)) {
            if a.kind() != "attribute" {
                continue;
            }
            let key = tf_child(a, "identifier", src);
            if !key.is_empty()
                && a.named_child_count() >= 2
                && let Some(e) = a.named_child(1)
            {
                exprs.insert(key.into_owned(), e); // the value expression
            }
        }
    }

    let mut inp = LabelInputs {
        delimiter: "-".to_string(),
        value_case: "lower".to_string(),
        label_order: DEFAULT_LABEL_ORDER.map(String::from).to_vec(),
        has_context: exprs.contains_key("context"),
        ..LabelInputs::default()
    };

    for k in LABEL_SCALARS {
        // absent: dropped here; if a context attr exists, has_context marks the id partial so
        // Stage C fills it
        let Some(&e) = exprs.get(k) else {
            inp.scalars.insert(k.to_string(), seg(SegState::Empty));
            continue;
        };
        let (v, st) = classify_scalar(e, src);
        let sv = match st {
            SegState::Known if !v.is_empty() => SegVal {
                val: v,
                state: SegState::Known,
            },
            // literal empty string, or explicit null -> dropped
            SegState::Known | SegState::Empty => seg(SegState::Empty),
            // explicit var.X / expression -> referenced but unresolved
            SegState::Unknown => {
                let vn = expr_var_name(e, src); // Stage C: record `var.<name>`
                if !vn.is_empty() {
                    inp.var_refs.insert(k.to_string(), vn);
                }
                seg(SegState::Unknown)
            }
        };
        inp.scalars.insert(k.to_string(), sv);
    }

    // attributes (list): absent -> empty (has_context marks the id partial)
    match exprs.get("attributes") {
        None => inp.attr_state = SegState::Empty,
        Some(&e) => {
            (inp.attrs, inp.attr_state) = classify_list(e, src);
            if inp.attr_state == SegState::Unknown {
                let vn = expr_var_name(e, src); // Stage C: `attributes = var.<name>`
                if !vn.is_empty() {
                    inp.var_refs.insert("attributes".to_string(), vn);
                }
            }
        }
    }

    // context: record the address it points at so Stage C can inherit segments.
    if let Some(&e) = exprs.get("context") {
        inp.context_ref = expr_ref_address(e, src);
    }

    // knobs
    if let Some(&e) = exprs.get("delimiter") {
        let (v, st) = classify_scalar(e, src);
        match st {
            SegState::Known => inp.delimiter = v, // includes explicit "" (no delimiter)
            SegState::Empty => inp.delimiter = "-".to_string(), // null -> default
            SegState::Unknown => inp.unresolved = true,
        }
    }
    if let Some(&e) = exprs.get("label_value_case") {
        let (v, st) = classify_scalar(e, src);
        match st {
            SegState::Known if !v.is_empty() => inp.value_case = to_lower(&v),
            SegState::Known | SegState::Empty => inp.value_case = "lower".to_string(),
            SegState::Unknown => inp.unresolved = true,
        }
    }
    if let Some(&e) = exprs.get("label_order") {
        let (vals, st) = classify_list(e, src);
        match st {
            SegState::Known if !vals.is_empty() => inp.label_order = vals,
            // null/[] -> keep default order
            SegState::Known | SegState::Empty => {}
            SegState::Unknown => inp.unresolved = true,
        }
    }
    inp
}

/// Go `composeID`: the cloudposse id from captured inputs. "" when the shape is unresolved or
/// nothing is known; a partial id (with "{seg}" sentinels and a " (partial)" suffix) when some
/// segments are unknown; or the exact id.
pub(super) fn compose_id(inp: &LabelInputs) -> String {
    if inp.unresolved {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut known = 0;
    let mut partial = inp.has_context; // context may add inherited segments not visible here
    for key in &inp.label_order {
        if key == "attributes" {
            match inp.attr_state {
                SegState::Unknown => {
                    parts.push("{attributes}".to_string());
                    partial = true;
                }
                SegState::Known => {
                    let mut norm: Vec<String> = Vec::new();
                    let mut seen = HashSet::new();
                    for a in &inp.attrs {
                        let v = normalize_seg(a, &inp.value_case);
                        if v.is_empty() || !seen.insert(v.clone()) {
                            continue;
                        }
                        norm.push(v);
                    }
                    if !norm.is_empty() {
                        parts.push(norm.join(&inp.delimiter));
                        known += 1;
                    }
                }
                SegState::Empty => {}
            }
            continue;
        }
        let Some(sv) = inp.scalars.get(key) else {
            continue; // an unrecognised label_order entry
        };
        match sv.state {
            SegState::Known => {
                let v = normalize_seg(&sv.val, &inp.value_case);
                if v.is_empty() {
                    continue;
                }
                parts.push(v);
                known += 1;
            }
            SegState::Unknown => {
                parts.push(format!("{{{key}}}"));
                partial = true;
            }
            SegState::Empty => {}
        }
    }
    if known == 0 {
        return String::new();
    }
    let mut id = parts.join(&inp.delimiter);
    if partial {
        id.push_str(" (partial)");
    }
    id
}

/// Go `normalizeSeg`: strips chars outside [-a-zA-Z0-9] then applies the case mode, mirroring
/// cloudposse's regex_replace_chars + label_value_case.
fn normalize_seg(v: &str, case_mode: &str) -> String {
    let s: String = v
        .chars()
        .filter(|&r| r == '-' || r.is_ascii_alphanumeric())
        .collect();
    match case_mode {
        "none" => s,
        "upper" => to_upper(&s),
        "title" => title_case(&s),
        _ => to_lower(&s), // "lower" and any unrecognised value
    }
}

/// Go `titleCase`: Terraform title(lower(v)) of a single word: the first rune upper-cased.
fn title_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut r: Vec<char> = to_lower(s).chars().collect();
    r[0] = gostd::unicode::to_upper(r[0]);
    r.into_iter().collect()
}

/// Go `classifyScalar`: a pure literal string -> (text, Known) where text may be ""; a null
/// literal -> ("", Empty); anything dynamic or a non-string literal -> ("", Unknown).
pub(super) fn classify_scalar(expr: Node<'_>, src: &[u8]) -> (String, SegState) {
    let Some(lv) = tf_named_child(expr, "literal_value") else {
        return (String::new(), SegState::Unknown); // variable_expr, template_expr, ...
    };
    if let Some(sl) = tf_named_child(lv, "string_lit") {
        return (string_lit_text(sl, src), SegState::Known);
    }
    if tf_named_child(lv, "null_lit").is_some() {
        return (String::new(), SegState::Empty);
    }
    (String::new(), SegState::Unknown) // numeric_lit / bool_lit are not string labels
}

/// Go `classifyList`: a literal tuple of strings -> (vals, Known) (vals may be empty for []);
/// a null -> empty; anything else (or a tuple with a non-literal element) -> Unknown.
pub(super) fn classify_list(expr: Node<'_>, src: &[u8]) -> (Vec<String>, SegState) {
    let Some(cv) = tf_named_child(expr, "collection_value") else {
        if let Some(lv) = tf_named_child(expr, "literal_value")
            && tf_named_child(lv, "null_lit").is_some()
        {
            return (Vec::new(), SegState::Empty);
        }
        return (Vec::new(), SegState::Unknown);
    };
    let Some(tup) = tf_named_child(cv, "tuple") else {
        return (Vec::new(), SegState::Unknown);
    };
    let mut out = Vec::new();
    for el in (0..tup.named_child_count()).filter_map(|i| tup.named_child(i)) {
        if el.kind() != "expression" {
            continue;
        }
        let (v, st) = classify_scalar(el, src);
        if st == SegState::Unknown {
            return (Vec::new(), SegState::Unknown);
        }
        if st == SegState::Known && !v.is_empty() {
            out.push(v);
        }
    }
    (out, SegState::Known)
}

/// Go `stringLitText`: the literal text of a string_lit (its template_literal children
/// concatenated); "" for an empty string "".
pub(super) fn string_lit_text(sl: Node<'_>, src: &[u8]) -> String {
    let mut b = String::new();
    for c in (0..sl.named_child_count()).filter_map(|i| sl.named_child(i)) {
        if c.kind() == "template_literal" {
            b.push_str(&node_text(c, src));
        }
    }
    b
}

/// Go `exprRefAddress`: the canonical reference address of a value expression that is exactly a
/// single reference (`var.context` -> "var.context", `module.x.context` -> "module.x"), or "".
fn expr_ref_address(expr: Node<'_>, src: &[u8]) -> String {
    match tf_named_child(expr, "variable_expr") {
        Some(ve) => tf_ref_address(ve, src),
        None => String::new(),
    }
}

/// Go `exprVarName`: the bare variable name when `expr` is exactly `var.<name>`, else "".
pub(super) fn expr_var_name(expr: Node<'_>, src: &[u8]) -> String {
    let addr = expr_ref_address(expr, src);
    match addr.strip_prefix("var.") {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => String::new(),
    }
}

/// Go `tfNamedChild`: `n`'s first named child of the given kind.
fn tf_named_child<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    (0..n.named_child_count())
        .filter_map(|i| n.named_child(i))
        .find(|c| c.kind() == kind)
}

#[cfg(test)]
#[path = "tests/nulllabel.rs"]
mod nulllabel_tests;
