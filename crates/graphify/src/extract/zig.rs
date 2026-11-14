//! Port of `zig.go`: functions, container types (struct/enum/union/opaque) plus their
//! methods, `@import` dependencies, and call edges out of a .zig file.
//!
//! Zig has no dedicated class/import statements: types are values bound by a `const`, e.g.
//! `const Foo = struct { ... }`, and imports are `const std = @import("std")`. So a top-level
//! definition is a variable_declaration whose value is a container declaration or an @import
//! builtin, and a method is a function_declaration nested inside a container body.

use std::sync::OnceLock;

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk, walk_kinds};
use super::extract::{Def, Result};
use super::golang::{field_text, unquote};
use crate::model::{Confidence, Edge, Relation};

/// Go `extractZig`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_zig(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::zig::language(), |root| {
        let mut b = Builder::new(rel);

        for i in 0..root.child_count() {
            let Some(n) = root.child(i) else { continue };
            match n.kind() {
                "function_declaration" => zig_func(&mut b, n, src),
                "variable_declaration" => zig_var_decl(&mut b, n, src),
                _ => {}
            }
        }
        b.res
    })
}

/// Go `zigFunc`: records a top-level function definition and the calls in its body.
fn zig_func(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = field_text(n, "name", src);
    if name.is_empty() {
        return;
    }
    let id = b.id(&[&name]);
    b.def(&id, &name, &format!("{name}()"), line(n));
    zig_calls(b, n.child_by_field_name("body"), &id, src);
}

/// Go `zigVarDecl`: handles a `const`/`var` binding. If its value is a container declaration
/// it becomes a type (with nested methods); if its value is an `@import("...")` it becomes an
/// import.
fn zig_var_decl(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = zig_var_name(n, src);
    if name.is_empty() {
        return;
    }
    let Some(val) = zig_var_value(n) else {
        return;
    };
    match val.kind() {
        "struct_declaration" | "enum_declaration" | "union_declaration" | "opaque_declaration" => {
            zig_type(b, &name, val, src)
        }
        "builtin_function" => {
            let spec = zig_import_spec(val, src);
            if !spec.is_empty() {
                b.imp(&spec, line(n));
            }
        }
        _ => {}
    }
}

/// Go `zigType`: records a container type definition and each method (a function_declaration
/// directly inside the container body) scoped under it.
fn zig_type(b: &mut Builder, name: &str, container: Node<'_>, src: &[u8]) {
    let type_id = b.id(&[name]);
    b.def(&type_id, name, name, line(container));

    for i in 0..container.child_count() {
        let Some(m) = container.child(i) else {
            continue;
        };
        if m.kind() != "function_declaration" {
            continue;
        }
        let mname = field_text(m, "name", src);
        if mname.is_empty() {
            continue;
        }
        let mid = b.id(&[name, &mname]);
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
        // Register under the bare method name so `x.method()` call sites resolve.
        b.res.defs.push(Def {
            id: mid.clone(),
            name: mname,
            file: b.file.clone(),
            ..Def::default()
        });
        zig_calls(b, m.child_by_field_name("body"), &mid, src);
    }
}

/// Go `zigVarName`: the bound name of a variable_declaration: the leading `identifier` child
/// (the `:` type and any alignment clauses come after it).
fn zig_var_name(n: Node<'_>, src: &[u8]) -> String {
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if c.kind() == "identifier" {
            return node_text(c, src).into_owned();
        }
    }
    String::new()
}

/// Go `zigVarValue`: the value expression of a variable_declaration (the node after `=`),
/// skipping the name identifier, the `type` field, and the alignment/section metadata that can
/// precede it.
fn zig_var_value(n: Node<'_>) -> Option<Node<'_>> {
    let type_field = n.child_by_field_name("type");
    let mut seen_name = false;
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if !c.is_named() {
            continue;
        }
        if type_field.is_some_and(|t| c.equals(t)) {
            continue;
        }
        match c.kind() {
            "identifier" => {
                if !seen_name {
                    seen_name = true;
                    continue;
                }
                return Some(c);
            }
            "byte_alignment" | "address_space" | "link_section" | "string" => continue,
            _ => {}
        }
        return Some(c);
    }
    None
}

/// Go `zigImportSpec`: the imported path for an `@import("path")` builtin, or "" if the
/// builtin is something else.
fn zig_import_spec(bf: Node<'_>, src: &[u8]) -> String {
    let mut ident = None;
    let mut args = None;
    for i in 0..bf.child_count() {
        let Some(c) = bf.child(i) else { continue };
        match c.kind() {
            "builtin_identifier" => ident = Some(c),
            "arguments" => args = Some(c),
            _ => {}
        }
    }
    let (Some(ident), Some(args)) = (ident, args) else {
        return String::new();
    };
    if node_text(ident, src) != "@import" {
        return String::new();
    }
    let mut spec = String::new();
    walk(Some(args), |c| {
        if !spec.is_empty() {
            return false;
        }
        if c.kind() == "string" {
            spec = unquote(&node_text(c, src));
            return false;
        }
        true
    });
    spec
}

/// Go `zigCallKinds`.
static ZIG_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `zigCalls`: walks a function body and records each call site, attributing it to
/// `caller_id`. Direct calls (`f()`) record the identifier; field/method calls (`x.f()`)
/// record the trailing member name.
fn zig_calls(b: &mut Builder, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
    if body.is_none() {
        return;
    }
    walk_kinds(body, *ZIG_CALL_KINDS.get().expect("register_kinds"), |c| {
        let Some(func) = c.child_by_field_name("function") else {
            return true;
        };
        match func.kind() {
            "identifier" => b.call(caller_id, &node_text(func, src), line(c)),
            "field_expression" => {
                if let Some(m) = func.child_by_field_name("member") {
                    b.call_recv(
                        caller_id,
                        &node_text(m, src),
                        &recv_text(Some(func), Some(m), src),
                        line(c),
                    );
                }
            }
            _ => {}
        }
        true
    });
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let _ = ZIG_CALL_KINDS.set(gositter_grammars::zig::language().kind_mask(&["call_expression"]));
}

#[cfg(test)]
#[path = "tests/zig.rs"]
mod zig_tests;
