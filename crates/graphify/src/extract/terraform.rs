//! Port of `terraform.go`. Owner: the gostd stream (graphify-rs-ihc.31).

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::sync::{LazyLock, OnceLock};

use gositter::Node;
use gositter_grammars::hcl;
use gostd::regexp::Regexp;
use gostd::strings::trim_space;

use super::builder::{line, node_text, parse_root, walk, walk_kinds};
use super::extract::{LabelInputs, ModInvoke, ModRef, NullLabelRef, Result, SegVal};
use super::nulllabel::{
    SegState, classify_list, classify_scalar, compose_id, expr_var_name, null_label_inputs,
    string_lit_text,
};
use crate::idutil::make_id;
use crate::model::{self, Confidence, Edge, File, Loc, Relation, file_of};
use crate::security::sanitize_label;

/// Go `tfAttrKinds` (`terraform.go:18`): the mask of `attribute`.
static TF_ATTR_KINDS: OnceLock<u32> = OnceLock::new();
/// Go `tfVarKinds` (`terraform.go:19`): the mask of `variable_expr`.
static TF_VAR_KINDS: OnceLock<u32> = OnceLock::new();

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    TF_ATTR_KINDS.get_or_init(|| hcl::language().kind_mask(&["attribute"]));
    TF_VAR_KINDS.get_or_init(|| hcl::language().kind_mask(&["variable_expr"]));
}

fn mask(m: &OnceLock<u32>) -> u32 {
    *m.get().expect("kinds::register_kinds was not called")
}

/// Go `extractTerraform`'s per-file state: the result, the ids seen, and what `defNode` and
/// `refsFrom` close over.
struct Tf<'a> {
    rel: &'a str,
    src: &'a [u8],
    scope: String,
    file_id: String,
    rel_f: File,
    res: Result,
    seen: HashSet<String>,
}

impl Tf<'_> {
    /// Go `defNode`: adds a block node (once) with an optional computed name, plus a contains
    /// edge from the file.
    fn def_node(&mut self, addr: &str, label: &str, loc: Loc, computed: &str) -> String {
        let id = make_id(&[&self.scope, addr]);
        if self.seen.insert(id.clone()) {
            self.res.nodes.push(model::Node {
                id: id.clone(),
                label: label.to_string(),
                file_type: "code".to_string(),
                source_file: self.rel.to_string(),
                source_location: loc.to_string(),
                computed_name: computed.to_string(),
                ..model::Node::default()
            });
            self.res.edges.push(Edge {
                source: self.file_id.clone(),
                target: id.clone(),
                relation: Relation::CONTAINS,
                confidence: Confidence::EXTRACTED,
                source_file: self.rel_f,
                source_location: loc,
                ..Edge::default()
            });
        }
        id
    }

    /// Go `def`: [`Tf::def_node`] with no computed name.
    fn def(&mut self, addr: &str, label: &str, loc: Loc) -> String {
        self.def_node(addr, label, loc, "")
    }

    /// Go `refsFrom`: an edge from `src_id` to every address interpolated within `node`.
    fn refs_from(&mut self, src_id: &str, node: Option<Node<'_>>) {
        let src = self.src;
        let scope = &self.scope;
        let rel_f = self.rel_f;
        let edges = &mut self.res.edges;
        let var_kinds = mask(&TF_VAR_KINDS);
        walk_kinds(node, mask(&TF_ATTR_KINDS), |c| {
            let rel2 = match &*tf_child(c, "identifier", src) {
                "depends_on" => Relation::DEPENDS_ON,
                "context" => model::rel("inherits_context"),
                _ => Relation::REFERENCES,
            };
            walk_kinds(Some(c), var_kinds, |v| {
                let addr = tf_ref_address(v, src);
                if !addr.is_empty() {
                    let tgt = make_id(&[scope, &addr]);
                    if tgt != src_id {
                        edges.push(Edge {
                            source: src_id.to_string(),
                            target: tgt,
                            relation: rel2,
                            confidence: Confidence::EXTRACTED,
                            source_file: rel_f,
                            source_location: line(c),
                            ..Edge::default()
                        });
                    }
                }
                true
            });
            true
        });
    }
}

/// Go `extractTerraform`: `None` when the parse budget stops the parse (see `builder`).
/// Terraform/HCL blocks and the references between them. Nodes: resources, data sources,
/// modules, variables, outputs, providers, and locals. Edges: contains (file -> block),
/// references (block -> the addresses it interpolates), and depends_on.
///
/// IDs are scoped by the parent DIRECTORY, not the file stem: Terraform addresses are
/// module(directory)-scoped, so cross-file references resolve once per-file extractions merge.
pub(crate) fn extract_terraform(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, hcl::language(), |root| {
        let file_id = make_id(&[rel]);
        let mut t = Tf {
            rel,
            src,
            scope: dir_scope(rel),
            file_id: file_id.clone(),
            rel_f: file_of(rel),
            res: Result {
                nodes: vec![model::Node {
                    id: file_id.clone(),
                    label: gostd::filepath::base(rel),
                    file_type: "code".to_string(),
                    source_file: rel.to_string(),
                    source_location: "L1".to_string(),
                    ..model::Node::default()
                }],
                ..Result::default()
            },
            seen: HashSet::from([file_id]),
        };

        let Some(body) = tf_child_node(root, "body") else {
            return t.res;
        };
        for i in 0..body.child_count() {
            let Some(b) = body.child(i) else {
                continue;
            };
            if b.kind() != "block" {
                continue;
            }
            let (btype, labels, bbody) = tf_block(b, src);
            let loc = line(b);
            let mut owner = String::new();
            match btype.as_str() {
                "resource" if labels.len() >= 2 => {
                    let a = format!("{}.{}", labels[0], labels[1]);
                    owner = t.def(&a, &a, loc);
                    t.refs_from(&owner, bbody);
                }
                "data" if labels.len() >= 2 => {
                    let a = format!("data.{}.{}", labels[0], labels[1]);
                    owner = t.def(&a, &a, loc);
                    t.refs_from(&owner, bbody);
                }
                "module" if !labels.is_empty() => {
                    let addr = format!("module.{}", labels[0]);
                    let s = tf_attr_string(bbody, "source", src);
                    let mut label = addr.clone();
                    let mut computed = String::new();
                    let mut inputs = LabelInputs::default();
                    if is_null_label(&s) {
                        label = format!("{addr} [null-label]");
                        inputs = null_label_inputs(bbody, src);
                        computed = compose_id(&inputs);
                    }
                    let id = t.def_node(&addr, &label, loc, &computed);
                    owner = id.clone();
                    t.refs_from(&id, bbody);
                    if !s.is_empty() {
                        t.res.mod_refs.push(ModRef {
                            from_id: id.clone(),
                            source: s.clone(),
                            file: rel.to_string(),
                            loc,
                        });
                    }
                    // Stage C carry: capture every invocation's args so a local wrapper chain
                    // can be followed, and the null-label inputs so its partial id can be
                    // completed from the caller.
                    let (args, arg_var_refs) = module_args(bbody, src);
                    t.res.mod_invokes.push(ModInvoke {
                        node_id: id.clone(),
                        dir: t.scope.clone(),
                        args,
                        arg_var_refs,
                    });
                    if is_null_label(&s) {
                        t.res.null_labels.push(NullLabelRef {
                            node_id: id,
                            scope: t.scope.clone(),
                            inputs,
                        });
                    }
                }
                "variable" if !labels.is_empty() => {
                    let a = format!("var.{}", labels[0]);
                    owner = t.def(&a, &a, loc);
                }
                "output" if !labels.is_empty() => {
                    let a = format!("output.{}", labels[0]);
                    owner = t.def(&a, &a, loc);
                    t.refs_from(&owner, bbody);
                }
                "provider" if !labels.is_empty() => {
                    let a = format!("provider.{}", labels[0]);
                    owner = t.def(&a, &a, loc);
                }
                "locals" => {
                    let Some(bb) = bbody else {
                        continue;
                    };
                    for j in 0..bb.child_count() {
                        let Some(a) = bb.child(j) else {
                            continue;
                        };
                        if a.kind() != "attribute" {
                            continue;
                        }
                        let key = tf_child(a, "identifier", src);
                        if key.is_empty() {
                            continue;
                        }
                        let k = format!("local.{key}");
                        let id = t.def(&k, &k, line(a));
                        t.refs_from(&id, Some(a));
                    }
                }
                _ => {}
            }
            let attrs = block_attributes(bbody, src, &labels);
            if !owner.is_empty()
                && !attrs.is_empty()
                && let Some(n) = t.res.nodes.iter_mut().find(|n| n.id == owner)
            {
                n.attributes = attrs;
            }
        }
        t.res
    })
}

/// Go `maxAttributes`, `maxListItems`: how many attributes one block contributes and how many
/// items one list keeps. Value length is capped by `sanitize_label`.
const MAX_ATTRIBUTES: usize = 100;
const MAX_LIST_ITEMS: usize = 50;

/// Go `sensitiveKeyRe` (`terraform.go:197`): attribute keys whose VALUE must never reach
/// graph.json.
static SENSITIVE_KEY_RE: LazyLock<Regexp> = LazyLock::new(|| {
    Regexp::must_compile(
        r"(?i)(password|passwd|secret|token|api[-_]?key|access[-_]?key|private[-_]?key|credential|client[-_]?secret|connection[-_]?string|sas[-_]?token|auth|bearer|passphrase)",
    )
});

/// Go `redactedValue` (`terraform.go:199`): what a sensitive attribute's value becomes.
pub(super) const REDACTED_VALUE: &str = "[redacted]";

/// Go `urlCredRe` (`terraform.go:202`): user:pass@ credentials embedded in a URL.
static URL_CRED_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"://[^/@\s]*:[^@\s]+@"));

/// Go `bareRefRe` (`terraform.go:205`): a plain reference (var.x, data.a.b.id), never a secret.
static BARE_REF_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"^[A-Za-z_][\w.\[\]*-]*$"));

/// Go `scriptKeys`: bootstrap-script attributes, dropped outright.
const SCRIPT_KEYS: [&str; 5] = [
    "user_data",
    "user_data_base64",
    "user_data_replace_on_change",
    "custom_data",
    "metadata_startup_script",
];

/// Go `blockAttributes`: a block body's direct attributes as a flat key -> value map (empty
/// where Go returns nil). Nested objects are dropped; sensitive keys keep the key and redact the
/// value. A block is sensitive as a whole when one of its labels reads as a secret name or when
/// it declares `sensitive = true`.
fn block_attributes(
    body: Option<Node<'_>>,
    src: &[u8],
    labels: &[String],
) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let Some(body) = body else {
        return attrs;
    };
    let mut block_sensitive = labels.iter().any(|l| SENSITIVE_KEY_RE.match_string(l));
    if !block_sensitive {
        block_sensitive = (0..body.child_count())
            .filter_map(|i| body.child(i))
            .any(|a| {
                a.kind() == "attribute"
                    && a.named_child_count() >= 2
                    && tf_child(a, "identifier", src) == "sensitive"
                    && attr_value(attr_value_node(a), src) == "true"
            });
    }
    for a in (0..body.child_count()).filter_map(|i| body.child(i)) {
        if a.kind() != "attribute" {
            continue;
        }
        let key = tf_child(a, "identifier", src);
        if key.is_empty() || a.named_child_count() < 2 {
            continue;
        }
        if SCRIPT_KEYS.contains(&&*key) {
            continue;
        }
        if (block_sensitive && key != "sensitive") || SENSITIVE_KEY_RE.match_string(&key) {
            attrs.insert(key.into_owned(), REDACTED_VALUE.to_string());
        } else {
            let v = attr_value(attr_value_node(a), src);
            if v.is_empty() {
                continue;
            }
            attrs.insert(key.into_owned(), v);
        }
        if attrs.len() >= MAX_ATTRIBUTES {
            break;
        }
    }
    attrs
}

/// Go `attrValueNode`: an attribute's value expression, its first named child after the
/// identifier that is not a comment.
fn attr_value_node(a: Node<'_>) -> Option<Node<'_>> {
    (1..a.named_child_count())
        .filter_map(|i| a.named_child(i))
        .find(|c| c.kind() != "comment")
}

/// Go `attrValue`: a literal string as its text, a literal tuple as its elements joined by
/// ", ", an object as "" (dropped by the caller), anything else as its raw source text.
/// Recursive on tuple nesting, as Go.
fn attr_value(e: Option<Node<'_>>, src: &[u8]) -> String {
    let mut cur = e;
    while let Some(c) = cur {
        if c.named_child_count() != 1 {
            break;
        }
        match c.kind() {
            "expression" | "literal_value" | "collection_value" => cur = c.named_child(0),
            _ => break,
        }
    }
    let Some(cur) = cur else {
        return String::new();
    };
    match cur.kind() {
        "string_lit" => {
            let t = string_lit_text(cur, src);
            if URL_CRED_RE.match_string(&t)
                || (SENSITIVE_KEY_RE.match_string(&t) && !BARE_REF_RE.match_string(&t))
            {
                return REDACTED_VALUE.to_string();
            }
            sanitize_label(&t)
        }
        "object" => String::new(),
        "tuple" => {
            let mut items: Vec<String> = Vec::new();
            let mut i = 0;
            while i < cur.named_child_count() && items.len() < MAX_LIST_ITEMS {
                let el = cur.named_child(i);
                i += 1;
                // skips tuple_start/tuple_end
                let Some(el) = el.filter(|el| el.kind() == "expression") else {
                    continue;
                };
                let mut v = attr_value(Some(el), src);
                if !v.is_empty() {
                    if SENSITIVE_KEY_RE.match_string(&v) && !BARE_REF_RE.match_string(&v) {
                        v = REDACTED_VALUE.to_string();
                    }
                    items.push(v);
                }
            }
            items.join(", ")
        }
        _ => {
            let text = node_text(cur, src);
            let raw = trim_space(&text);
            if !BARE_REF_RE.match_string(raw)
                && (SENSITIVE_KEY_RE.match_string(raw) || URL_CRED_RE.match_string(raw))
            {
                return REDACTED_VALUE.to_string();
            }
            sanitize_label(raw)
        }
    }
}

/// Go `listSep`: joins/splits a list value carried inside a `SegVal`.
pub(super) const LIST_SEP: &str = "\x00";

/// Go `moduleArgs`: a module block body as Stage C carry data, the literal scalar and list
/// arguments (`args`) and the bare `arg = var.<name>` pass-throughs (`var_refs`).
fn module_args(
    body: Option<Node<'_>>,
    src: &[u8],
) -> (BTreeMap<String, SegVal>, BTreeMap<String, String>) {
    let mut args = BTreeMap::new();
    let mut var_refs = BTreeMap::new();
    let Some(body) = body else {
        return (args, var_refs);
    };
    for i in 0..body.named_child_count() {
        let Some(a) = body.named_child(i) else {
            continue;
        };
        if a.kind() != "attribute" {
            continue;
        }
        let key = tf_child(a, "identifier", src);
        if key.is_empty() || a.named_child_count() < 2 {
            continue;
        }
        let Some(e) = a.named_child(1) else {
            continue;
        };
        if key == "source" || key == "version" {
            continue;
        }
        let vn = expr_var_name(e, src);
        if !vn.is_empty() {
            var_refs.insert(key.into_owned(), vn);
            continue;
        }
        // literal scalar?
        let (v, st) = classify_scalar(e, src);
        if st != SegState::Unknown {
            let sv = if st == SegState::Known && !v.is_empty() {
                SegVal {
                    val: v,
                    state: SegState::Known,
                }
            } else {
                SegVal {
                    val: String::new(),
                    state: SegState::Empty,
                }
            };
            args.insert(key.into_owned(), sv);
            continue;
        }
        // literal list (e.g. attributes = ["1","2"])?
        let (vals, st) = classify_list(e, src);
        if st != SegState::Unknown {
            args.insert(
                key.into_owned(),
                SegVal {
                    val: vals.join(LIST_SEP),
                    state: SegState::Known,
                },
            );
        }
    }
    (args, var_refs)
}

/// Go `isNullLabel`: the cloudposse null-label module source, registry or git form.
fn is_null_label(source: &str) -> bool {
    source.contains("cloudposse/label/null") || source.contains("terraform-null-label")
}

/// Go `tfBlock`: a block's type identifier, its quoted labels, and its body.
fn tf_block<'t>(n: Node<'t>, src: &[u8]) -> (String, Vec<String>, Option<Node<'t>>) {
    let mut btype = String::new();
    let mut labels = Vec::new();
    let mut body = None;
    for c in (0..n.child_count()).filter_map(|i| n.child(i)) {
        match c.kind() {
            "identifier" if btype.is_empty() => btype = node_text(c, src).into_owned(),
            "string_lit" => labels.push(node_text(c, src).trim_matches('"').to_string()),
            "body" => body = Some(c),
            _ => {}
        }
    }
    (btype, labels, body)
}

/// Go `tfAttrString`: the first string literal of the attribute named `key` in `body` (its
/// quotes trimmed), or "" if absent.
fn tf_attr_string(body: Option<Node<'_>>, key: &str, src: &[u8]) -> String {
    let Some(body) = body else {
        return String::new();
    };
    for a in (0..body.child_count()).filter_map(|i| body.child(i)) {
        if a.kind() != "attribute" || tf_child(a, "identifier", src) != key {
            continue;
        }
        let mut out = String::new();
        walk(Some(a), |n| {
            if !out.is_empty() {
                return false;
            }
            if n.kind() == "string_lit" {
                out = node_text(n, src).trim_matches('"').to_string();
                return false;
            }
            true
        });
        return out;
    }
    String::new()
}

/// Go `tfRefAddress`: a variable_expr (plus its trailing get_attr chain) as a canonical block
/// address, or "" for meta references (var/each/count/etc.).
pub(super) fn tf_ref_address(var_expr: Node<'_>, src: &[u8]) -> String {
    let mut parts = vec![first_ident(var_expr, src)];
    if let Some(parent) = var_expr.parent() {
        let mut after = false;
        for c in (0..parent.child_count()).filter_map(|i| parent.child(i)) {
            if c.equals(var_expr) {
                after = true;
                continue;
            }
            if !after {
                continue;
            }
            if c.kind() == "get_attr" {
                parts.push(first_ident(c, src));
            } else {
                break;
            }
        }
    }
    if parts[0].is_empty() {
        return String::new();
    }
    match &*parts[0] {
        "var" | "local" | "module" if parts.len() >= 2 => format!("{}.{}", parts[0], parts[1]),
        "data" if parts.len() >= 3 => format!("data.{}.{}", parts[1], parts[2]),
        "var" | "local" | "module" | "data" | "each" | "count" | "self" | "path" | "terraform" => {
            String::new()
        }
        // resource reference: <type>.<name>
        _ if parts.len() >= 2 => format!("{}.{}", parts[0], parts[1]),
        _ => String::new(),
    }
}

/// Go `firstIdent`.
fn first_ident<'s>(n: Node<'_>, src: &'s [u8]) -> Cow<'s, str> {
    tf_child(n, "identifier", src)
}

/// Go `tfChild`: the text of `n`'s first child of the given kind.
pub(super) fn tf_child<'s>(n: Node<'_>, kind: &str, src: &'s [u8]) -> Cow<'s, str> {
    tf_child_node(n, kind).map_or(Cow::Borrowed(""), |c| node_text(c, src))
}

/// Go `tfChildNode`.
fn tf_child_node<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.kind() == kind)
}

/// Go `dirScope`: the file's full directory path (Go `filepath.ToSlash(filepath.Dir(rel))`;
/// `ToSlash` is the identity on the `/`-separated platforms), or "tf" at the root.
fn dir_scope(rel: &str) -> String {
    let d = gostd::filepath::dir(rel);
    if d != "." && d != "/" && !d.is_empty() {
        return d;
    }
    "tf".to_string()
}

#[cfg(test)]
#[path = "tests/terraform.rs"]
mod terraform_tests;

#[cfg(test)]
#[path = "tests/upstream_terraform.rs"]
mod upstream_terraform_tests;

#[cfg(test)]
#[path = "tests/upstream_terraform_modules.rs"]
mod upstream_terraform_modules_tests;
