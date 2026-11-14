//! Port of `c.go`: functions, aggregate types (struct/union/enum), their function-pointer
//! "methods", #include imports, and call edges out of a C file.
//!
//! C has no methods, so a function-pointer field of a struct/union is treated as a method of that
//! aggregate (the common C idiom for behaviour on a type).

use std::sync::OnceLock;

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk, walk_kinds};
use super::extract::{Def, Result};
use super::golang::{field_text, unquote};
use super::lua::first_named;
use crate::model::{Confidence, Edge, Relation};

/// Go `extractC`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_c(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::c::language(), |root| {
        let mut b = Builder::new(rel);

        c_items(&mut b, root, src);
        b.res
    })
}

/// Go `cItems`: handles each item directly under n, and the items behind a preprocessor
/// conditional or inside an `extern "C"` block.
fn c_items(b: &mut Builder, root: Node<'_>, src: &[u8]) {
    for i in 0..root.child_count() {
        let Some(n) = root.child(i) else { continue };
        match n.kind() {
            "preproc_if"
            | "preproc_ifdef"
            | "preproc_else"
            | "preproc_elif"
            | "preproc_elifdef"
            | "linkage_specification"
            | "declaration_list"
            | "ERROR" => c_items(b, n, src),
            "function_definition" => c_func(b, n, src),
            "struct_specifier" | "union_specifier" | "enum_specifier" => c_type(b, n, src),
            "preproc_include" => c_include(b, n, src),
            "declaration" | "type_definition" => {
                // A struct/union/enum can be introduced inside a declaration
                // (`struct S {...} x;`) or a typedef (`typedef struct {...} S;`).
                if let Some(spec) = n.child_by_field_name("type") {
                    // `typedef struct _kv kv;`: kv names the tag _kv. Any other
                    // type has no tag.
                    let mut tag = String::new();
                    if matches!(
                        spec.kind(),
                        "struct_specifier" | "union_specifier" | "enum_specifier"
                    ) {
                        c_type(b, spec, src);
                        tag = field_text(spec, "name", src);
                    }
                    if n.kind() == "type_definition" {
                        for j in 0..n.child_count() {
                            let Some(d) = n.child(j) else { continue };
                            if n.field_name_for_child(j) == Some("declarator")
                                && d.kind() == "type_identifier"
                            {
                                b.res
                                    .type_aliases
                                    .push(format!("{} {}", node_text(d, src), tag));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Go `cFunc`.
fn c_func(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = c_decl_name(n.child_by_field_name("declarator"), src);
    if name.is_empty() {
        return;
    }
    let id = b.id(&[&name]);
    b.def(&id, &name, &format!("{name}()"), line(n));
    c_refs(b, &id, n, src);
    c_calls(b, n.child_by_field_name("body"), &id, src);
}

/// Go `cRefs`: records a references edge from the function id to its return type and to each
/// parameter type that is a typedef name. `struct S` names a tag and a primitive type names
/// nothing.
fn c_refs(b: &mut Builder, id: &str, func: Node<'_>, src: &[u8]) {
    sig_types(func, |t| {
        if t.kind() == "type_identifier" {
            b.ref_(id, &node_text(t, src), "references", false, line(func));
        }
    });
}

/// Go `sigTypes`: calls `r` with the return type node of the C or C++ function definition `func`
/// and with the type node of each of its parameters.
pub(crate) fn sig_types<'t>(func: Node<'t>, mut r: impl FnMut(Node<'t>)) {
    if let Some(t) = func.child_by_field_name("type") {
        r(t);
    }
    walk(func.child_by_field_name("declarator"), |c| {
        if c.kind() != "function_declarator" {
            return true;
        }
        // `T (*f(int n))(P)`: the parameters of f sit beside its name, in
        // the declarator behind the pointer. `T NAME(f)(P)` has none there.
        let mut d = c.child_by_field_name("declarator");
        let mut ptr = false;
        while let Some(mut cur) = d {
            if cur.kind() == "parenthesized_declarator" {
                match first_named(cur) {
                    Some(x) => cur = x,
                    None => return false,
                }
            }
            if cur.kind() == "function_declarator" && ptr {
                return true;
            }
            ptr = ptr || cur.kind() == "pointer_declarator";
            d = cur.child_by_field_name("declarator");
        }
        if let Some(ps) = c.child_by_field_name("parameters") {
            for i in 0..ps.named_child_count() {
                if let Some(t) = ps
                    .named_child(i)
                    .and_then(|p| p.child_by_field_name("type"))
                {
                    r(t);
                }
            }
        }
        false
    });
}

/// Go `cType`: records a struct/union/enum definition and any function-pointer fields in its body
/// as methods scoped under the type.
fn c_type(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = field_text(n, "name", src);
    if name.is_empty() {
        return;
    }
    // `struct S;` and `struct S v;` name a type, they do not define it.
    let Some(body) = n.child_by_field_name("body") else {
        return;
    };
    let type_id = b.id(&[&name]);
    b.def(&type_id, &name, &name, line(n));

    for i in 0..body.child_count() {
        let Some(f) = body.child(i) else { continue };
        if f.kind() != "field_declaration" {
            continue;
        }
        // A function-pointer field declares a callable member; its declarator
        // unwraps to a function_declarator over a field_identifier.
        let decl = f.child_by_field_name("declarator");
        if !c_is_func_pointer(decl) {
            continue;
        }
        let mname = c_decl_name(decl, src);
        if mname.is_empty() {
            continue;
        }
        let mid = b.id(&[&name, &mname]);
        b.add_node(&mid, &format!("{name}.{mname}()"), line(f));
        b.res.edges.push(Edge {
            source: type_id.clone(),
            target: mid.clone(),
            relation: Relation::CONTAINS,
            confidence: Confidence::EXTRACTED,
            source_file: b.src,
            source_location: line(f),
            ..Edge::default()
        });
        b.res.defs.push(Def {
            id: mid,
            name: mname,
            file: b.file.clone(),
            ..Def::default()
        });
    }
}

/// Go `cInclude`: records the included header path as an import. `<stdio.h>` and `"util.h"` both
/// record the header path; only the quoted form is looked up beside the including file.
fn c_include(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let Some(p) = n.child_by_field_name("path") else {
        return;
    };
    match p.kind() {
        "system_lib_string" => b.imp(&trim_angle(&node_text(p, src)), line(n)),
        "string_literal" => b.imp_rel(&c_string_content(p, src), line(n)),
        _ => {}
    }
}

/// Go `cCallKinds` (`c.go:171`).
static C_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `cCalls`: walks a function body and records each call site. Direct calls (`f()`) record the
/// identifier; member calls (`x.f()`, `x->f()`) record the field name.
fn c_calls(b: &mut Builder, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
    if body.is_none() {
        return;
    }
    let mask = *C_CALL_KINDS.get().expect("register_kinds");
    walk_kinds(body, mask, |c| {
        let Some(func) = c.child_by_field_name("function") else {
            return true;
        };
        match func.kind() {
            "identifier" => b.call(caller_id, &node_text(func, src), line(c)),
            "field_expression" => {
                if let Some(f) = func.child_by_field_name("field") {
                    b.call_recv(
                        caller_id,
                        &node_text(f, src),
                        &recv_text(Some(func), Some(f), src),
                        line(c),
                    );
                }
            }
            _ => {}
        }
        true
    });
}

/// Go `cDeclName`: unwraps pointer/parenthesized/function declarators to the innermost identifier
/// (or field_identifier) that names a function or pointer field.
fn c_decl_name(n: Option<Node<'_>>, src: &[u8]) -> String {
    let mut n = n;
    while let Some(cur) = n {
        match cur.kind() {
            "identifier" | "field_identifier" => return node_text(cur, src).into_owned(),
            "pointer_declarator"
            | "function_declarator"
            | "array_declarator"
            | "init_declarator" => n = cur.child_by_field_name("declarator"),
            _ => {
                // parenthesized_declarator and anything unexpected: fall back to
                // the first identifier descendant. For `int (*start)(...)` the
                // parenthesized subtree excludes the parameter list, so the first
                // identifier is the field name, not a parameter.
                let mut name = String::new();
                walk(Some(cur), |c| {
                    if !name.is_empty() {
                        return false;
                    }
                    if c.kind() == "identifier" || c.kind() == "field_identifier" {
                        name = node_text(c, src).into_owned();
                        return false;
                    }
                    true
                });
                return name;
            }
        }
    }
    String::new()
}

/// Go `cIsFuncPointer`: reports whether a field declarator is a function pointer, i.e. it
/// contains a function_declarator (e.g. `int (*start)(struct S *s)`).
fn c_is_func_pointer(n: Option<Node<'_>>) -> bool {
    let mut found = false;
    walk(n, |c| {
        if c.kind() == "function_declarator" {
            found = true;
            return false;
        }
        true
    });
    found
}

/// Go `trimAngle`: strips the surrounding < > from a system include path.
fn trim_angle(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('<') && s.ends_with('>') {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Go `cStringContent`: the inner text of a "..." string_literal include path, reading its
/// string_content child and falling back to a quote-trimmed literal.
fn c_string_content(p: Node<'_>, src: &[u8]) -> String {
    for i in 0..p.child_count() {
        if let Some(c) = p.child(i)
            && c.kind() == "string_content"
        {
            return node_text(c, src).into_owned();
        }
    }
    unquote(&node_text(p, src))
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let _ = C_CALL_KINDS.set(gositter_grammars::c::language().kind_mask(&["call_expression"]));
}

#[cfg(test)]
#[path = "tests/c.rs"]
mod c_tests;
