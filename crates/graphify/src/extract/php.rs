//! Port of `php.go`: functions, types (class/interface/trait/enum + methods), `use` imports, and
//! call edges out of a .php file. Top-level functions and types become definitions; methods are
//! nested under their type. Calls are recorded by the called name and resolved against the corpus
//! by `resolve`.

use std::sync::OnceLock;

use gositter::Node;
use gostd::strings::to_lower;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk_kinds};
use super::extract::{Def, Result};
use super::golang::field_text;
use crate::model::{Confidence, Edge, Relation};

/// Go `extractPHP`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_php(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::php::language(), |root| {
        let mut b = Builder::new(rel);
        php_items(&mut b, root, src);
        b.res
    })
}

/// Go `phpItems`: handles each statement directly under `n` (the program root or a
/// namespace_definition body), going into braced namespace blocks. Go recurses into a namespace
/// body in the middle of its loop; the explicit stack keeps that order.
fn php_items(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let mut stack: Vec<(Node<'_>, u32)> = vec![(n, 0)];
    while let Some(top) = stack.last_mut() {
        let (p, i) = *top;
        if i >= p.child_count() {
            stack.pop();
            continue;
        }
        top.1 += 1;
        let Some(c) = p.child(i) else { continue };
        match c.kind() {
            "function_definition" => php_func(b, c, src),
            "class_declaration"
            | "interface_declaration"
            | "trait_declaration"
            | "enum_declaration" => php_type(b, c, src),
            "namespace_use_declaration" => php_use(b, c, src),
            "namespace_definition" => {
                if let Some(body) = c.child_by_field_name("body") {
                    stack.push((body, 0));
                }
            }
            _ => {}
        }
    }
}

/// Go `phpFunc`.
fn php_func(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = field_text(n, "name", src);
    if name.is_empty() {
        return;
    }
    let id = b.id(&[&name]);
    b.def(&id, &name, &format!("{name}()"), line(n));
    php_signature(b, &id, "", n, src);
    php_calls(b, n.child_by_field_name("body"), &id, src);
}

/// Go `phpType`: records a class/interface/trait/enum and the methods in its body, scoping each
/// method under the type's name.
fn php_type(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = field_text(n, "name", src);
    if name.is_empty() {
        return;
    }
    let type_id = b.id(&[&name]);
    b.def(&type_id, &name, &name, line(n));
    php_bases(b, &type_id, n, src);

    let Some(body) = n.child_by_field_name("body") else {
        return;
    };
    for i in 0..body.child_count() {
        let Some(m) = body.child(i) else { continue };
        if m.kind() == "property_declaration" {
            php_type_refs(b, &type_id, m.child_by_field_name("type"), src);
        }
        if m.kind() != "method_declaration" {
            continue;
        }
        let mname = field_text(m, "name", src);
        if mname.is_empty() {
            continue;
        }
        let mid = b.id(&[&name, &mname]);
        b.add_node(&mid, &format!("{name}.{mname}()"), line(m));
        b.res.edges.push(Edge {
            source: type_id.clone(),
            target: mid.clone(),
            relation: Relation::CONTAINS,
            confidence: Confidence::EXTRACTED,
            source_file: b.src,
            source_location: line(m),
            ..Edge::default()
        });
        // Register under the bare method name so `$x->method()` call sites resolve.
        b.res.defs.push(Def {
            id: mid.clone(),
            name: mname,
            file: b.file.clone(),
            ..Def::default()
        });
        php_signature(b, &mid, &type_id, m, src);
        php_calls(b, m.child_by_field_name("body"), &mid, src);
    }
}

/// Go `phpUse`: records the qualified path of each clause in a `use` declaration as an import
/// (e.g. `use App\Models\User;` records App\Models\User).
fn php_use(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if c.kind() != "namespace_use_clause" {
            continue;
        }
        for j in 0..c.child_count() {
            let Some(nm) = c.child(j) else { continue };
            if nm.kind() == "qualified_name" || nm.kind() == "name" {
                b.imp(&node_text(nm, src), line(n));
                break;
            }
        }
    }
}

/// Go `phpConstructs`: language constructs that parse like calls.
fn php_construct(name: &str) -> bool {
    matches!(
        name,
        "array" | "die" | "empty" | "eval" | "exit" | "isset" | "list" | "unset"
    )
}

/// Go `phpCallKinds` (`php.go:117`).
static PHP_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `phpCalls`: walks a function/method body and records each call site. Direct calls (`f()`),
/// method calls (`$x->f()`, `$x?->f()`), and static calls (`C::f()`) all record the trailing
/// simple name.
fn php_calls(b: &mut Builder, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
    if body.is_none() {
        return;
    }
    let mask = *PHP_CALL_KINDS.get().expect("register_kinds");
    walk_kinds(body, mask, |c| {
        match c.kind() {
            "function_call_expression" => {
                if let Some(f) = c.child_by_field_name("function") {
                    let name = php_name(f, src);
                    if !php_construct(&to_lower(&name)) {
                        b.call(caller_id, &name, line(c));
                    }
                }
            }
            "member_call_expression"
            | "nullsafe_member_call_expression"
            | "scoped_call_expression" => {
                if let Some(nm) = c.child_by_field_name("name")
                    && nm.kind() == "name"
                {
                    b.call_recv(
                        caller_id,
                        &node_text(nm, src),
                        &recv_text(Some(c), Some(nm), src),
                        line(c),
                    );
                }
            }
            _ => {}
        }
        true
    });
}

/// Go `phpName`: the simple callee name for a function call target, taking the trailing segment
/// of qualified/relative names (e.g. "add" for `App\Math\add`).
fn php_name(n: Node<'_>, src: &[u8]) -> String {
    match n.kind() {
        "name" => node_text(n, src).into_owned(),
        "qualified_name" | "relative_name" => {
            let mut last = String::new();
            for i in 0..n.child_count() {
                if let Some(c) = n.child(i)
                    && c.kind() == "name"
                {
                    last = node_text(c, src).into_owned();
                }
            }
            last
        }
        _ => String::new(),
    }
}

/// Go `phpBases`: records extends as inherits, implements, and trait use as mixes_in.
fn php_bases(b: &mut Builder, type_id: &str, n: Node<'_>, src: &[u8]) {
    for i in 0..n.named_child_count() {
        let Some(c) = n.named_child(i) else { continue };
        match c.kind() {
            "base_clause" => php_base(b, type_id, c, "inherits", src),
            "class_interface_clause" => php_base(b, type_id, c, "implements", src),
            _ => {}
        }
    }
    if let Some(body) = n.child_by_field_name("body") {
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i)
                && c.kind() == "use_declaration"
            {
                php_base(b, type_id, c, "mixes_in", src);
            }
        }
    }
}

/// The `base` closure of Go `phpBases`: each name in `list` is a `rel` of `type_id`.
fn php_base(b: &mut Builder, type_id: &str, list: Node<'_>, rel: &str, src: &[u8]) {
    for i in 0..list.named_child_count() {
        if let Some(c) = list.named_child(i)
            && (c.kind() == "name" || c.kind() == "qualified_name")
        {
            b.ref_q(
                type_id,
                &php_last(c, src),
                rel,
                &php_qual(c, src),
                line(list),
            );
        }
    }
}

/// Go `phpSignature`: records the parameter and return types of a function. A promoted
/// constructor parameter is also a field of `owner_id`.
fn php_signature(b: &mut Builder, id: &str, owner_id: &str, n: Node<'_>, src: &[u8]) {
    if let Some(params) = n.child_by_field_name("parameters") {
        for i in 0..params.named_child_count() {
            let Some(p) = params.named_child(i) else {
                continue;
            };
            match p.kind() {
                "simple_parameter" => {}
                "property_promotion_parameter" => {
                    php_type_refs(b, owner_id, p.child_by_field_name("type"), src)
                }
                _ => continue,
            }
            php_type_refs(b, id, p.child_by_field_name("type"), src);
        }
    }
    php_type_refs(b, id, n.child_by_field_name("return_type"), src);
}

/// Go `phpTypeRefs`: records a reference from `id` to each class `t` names. Builtin types and
/// the self, static and parent keywords give nothing. Go recurses into the named children; the
/// explicit stack visits them in the same preorder.
fn php_type_refs(b: &mut Builder, id: &str, t: Option<Node<'_>>, src: &[u8]) {
    let Some(t) = t else { return };
    if id.is_empty() {
        return;
    }
    let mut stack = vec![t];
    while let Some(t) = stack.pop() {
        match t.kind() {
            "primitive_type" => {}
            "name" | "qualified_name" => {
                let name = php_last(t, src);
                if !matches!(to_lower(&name).as_str(), "self" | "static" | "parent") {
                    b.ref_q(id, &name, "references", &php_qual(t, src), line(t));
                }
            }
            _ => {
                for i in (0..t.named_child_count()).rev() {
                    if let Some(c) = t.named_child(i) {
                        stack.push(c);
                    }
                }
            }
        }
    }
}

/// Go `phpQual`: the namespace a qualified name is written with, or "".
fn php_qual(n: Node<'_>, src: &[u8]) -> String {
    let t = node_text(n, src);
    let end = t.rfind('\\').map_or(0, |i| i + 1);
    t[..end].trim_matches('\\').to_string()
}

/// Go `phpLast`: the last segment of a possibly qualified name.
fn php_last(n: Node<'_>, src: &[u8]) -> String {
    let t = node_text(n, src);
    let start = t.rfind('\\').map_or(0, |i| i + 1);
    t[start..].to_string()
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let _ = PHP_CALL_KINDS.set(gositter_grammars::php::language().kind_mask(&[
        "function_call_expression",
        "member_call_expression",
        "nullsafe_member_call_expression",
        "scoped_call_expression",
    ]));
}

#[cfg(test)]
#[path = "tests/php.rs"]
mod php_tests;
