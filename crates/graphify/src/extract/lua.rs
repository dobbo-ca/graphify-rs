//! Port of `lua.go`. Owner: the gostd stream; filled by graphify-rs-ihc.39.
//!
//! Functions, module/class tables (+ their methods), requires, and call edges
//! out of a .lua file. Top-level `function f()` declarations become definitions; `function
//! M.f()` / `function M:f()` are scoped under the table M. `local M = {}` registers M as a type.
//! `require "m"` records an import. Calls are recorded by the called name and resolved by
//! `resolve`.

use std::sync::OnceLock;

use gositter::Node;
use gositter_grammars::lua;

use super::builder::{Builder, line, node_text, parse_root, walk, walk_kinds};
use super::extract::{Def, Result};
use crate::model::{Confidence, Edge, Loc, Relation};

/// Go `luaCallKinds` (`lua.go:137`): the mask of `function_call`.
static LUA_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `extractLua`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_lua(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, lua::language(), |root| {
        let mut b = Builder::new(rel);
        for i in 0..root.child_count() {
            if let Some(n) = root.child(i) {
                lua_statement(&mut b, n, src);
            }
        }
        b.res
    })
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    LUA_CALL_KINDS.get_or_init(|| lua::language().kind_mask(&["function_call"]));
}

/// Go `luaStatement`: one chunk-level statement.
fn lua_statement(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    match n.kind() {
        "function_declaration" => lua_func(b, n, src),
        "variable_declaration" | "assignment_statement" => {
            lua_assign(b, n, src);
            lua_top_calls(b, n, src);
        }
        // Chunk-level expression statements may carry top-level require() calls.
        _ => lua_top_calls(b, n, src),
    }
}

/// Go `luaFunc`: a `function ...() ... end` declaration. A plain identifier name is a top-level
/// function; a dotted (`M.f`) or method (`M:f`) name is a method scoped under the table M.
fn lua_func(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let Some(name) = n.child_by_field_name("name") else {
        return;
    };
    let body = n.child_by_field_name("body");

    match name.kind() {
        "identifier" => {
            let fn_ = node_text(name, src);
            let id = b.id(&[&fn_]);
            b.def(&id, &fn_, &format!("{fn_}()"), line(n));
            lua_calls(b, body, &id, src);
        }
        "dot_index_expression" => lua_method(
            b,
            name.child_by_field_name("table"),
            name.child_by_field_name("field"),
            body,
            n,
            src,
        ),
        "method_index_expression" => lua_method(
            b,
            name.child_by_field_name("table"),
            name.child_by_field_name("method"),
            body,
            n,
            src,
        ),
        _ => {}
    }
}

/// Go `luaMethod`: scopes a method under its receiver table, creating the type node on demand
/// (Lua tables are usually declared as `local M = {}` but a method may be the first thing seen
/// for M).
fn lua_method(
    b: &mut Builder,
    table: Option<Node<'_>>,
    field: Option<Node<'_>>,
    body: Option<Node<'_>>,
    decl: Node<'_>,
    src: &[u8],
) {
    let type_name = lua_simple_name(table, src);
    let mname = lua_simple_name(field, src);
    if type_name.is_empty() || mname.is_empty() {
        return;
    }
    lua_ensure_type(b, &type_name, line(decl));

    let mid = b.id(&[&type_name, &mname]);
    b.add_node(&mid, &format!("{type_name}.{mname}()"), line(decl));
    let type_id = b.id(&[&type_name]);
    b.res.edges.push(Edge {
        source: type_id,
        target: mid.clone(),
        relation: Relation::CONTAINS,
        confidence: Confidence::EXTRACTED,
        source_file: b.src,
        source_location: line(decl),
        ..Edge::default()
    });
    // Register under the bare method name so `x.method()` call sites resolve.
    b.res.defs.push(Def {
        id: mid.clone(),
        name: mname,
        file: b.file.clone(),
        ..Def::default()
    });
    lua_calls(b, body, &mid, src);
}

/// Go `luaEnsureType`: registers a table name as a type definition if not already seen.
fn lua_ensure_type(b: &mut Builder, name: &str, loc: Loc) {
    let id = b.id(&[name]);
    if b.seen.contains(&id) {
        return;
    }
    b.def(&id, name, name, loc);
}

/// Go `luaAssign`: `local M = {}` / `M = {}` (an assignment whose value is a table constructor)
/// is a module/class table definition.
fn lua_assign(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let assign = if n.kind() == "variable_declaration" {
        match child_by_kind(n, "assignment_statement") {
            Some(a) => a,
            None => return,
        }
    } else {
        n
    };
    let (Some(vars), Some(vals)) = (
        child_by_kind(assign, "variable_list"),
        child_by_kind(assign, "expression_list"),
    ) else {
        return;
    };
    // Pair the first variable with the first value; only a bare identifier assigned a table
    // constructor counts as a type.
    let name = lua_simple_name(first_named(vars), src);
    let val = first_named(vals);
    if name.is_empty() || val.is_none_or(|v| v.kind() != "table_constructor") {
        return;
    }
    lua_ensure_type(b, &name, line(n));
}

/// Go `luaSimpleName`: the bare identifier name of a variable / identifier node, unwrapping the
/// `variable` wrapper (Go's recursion, as a loop). Non-identifier targets return "".
fn lua_simple_name(mut n: Option<Node<'_>>, src: &[u8]) -> String {
    while let Some(c) = n {
        match c.kind() {
            "identifier" => return node_text(c, src).into_owned(),
            "variable" => n = first_named(c),
            _ => break,
        }
    }
    String::new()
}

/// Go `luaTopCalls`: records require()/other call sites that sit directly in a statement at
/// chunk level (e.g. `local x = require "m"`), attributing them to the file node so module-level
/// requires are captured even outside a function.
fn lua_top_calls(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let file_id = b.file_id.clone();
    lua_calls(b, Some(n), &file_id, src);
}

/// Go `luaCalls` (and the walk of `luaTopCalls`): records each call site under `body`,
/// attributing them to `caller_id`.
fn lua_calls(b: &mut Builder, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
    let mask = *LUA_CALL_KINDS
        .get()
        .expect("kinds::register_kinds was not called");
    walk_kinds(body, mask, |cv| {
        lua_call(b, cv, caller_id, src);
        true
    });
}

/// Go `luaCall`: one function_call. `require "m"` / `require("m")` is an import; everything else
/// records the trailing callee name (identifier for `f()`, field for `m.f()`, method for
/// `o:f()`).
fn lua_call(b: &mut Builder, c: Node<'_>, caller_id: &str, src: &[u8]) {
    let Some(name) = c.child_by_field_name("name") else {
        return;
    };
    let callee = lua_callee_name(name, src);
    if callee == "require" {
        let spec = lua_require_spec(c, src);
        if !spec.is_empty() {
            b.imp(&spec, line(c));
        }
        return;
    }
    let recv = name
        .child_by_field_name("table")
        .map_or_else(String::new, |t| node_text(t, src).into_owned());
    b.call_recv(caller_id, &callee, &recv, line(c));
}

/// Go `luaCalleeName`: the trailing simple name of a call target.
fn lua_callee_name(name: Node<'_>, src: &[u8]) -> String {
    let f = match name.kind() {
        "variable" | "identifier" => return lua_simple_name(Some(name), src),
        "dot_index_expression" => name.child_by_field_name("field"),
        "method_index_expression" => name.child_by_field_name("method"),
        _ => None,
    };
    f.map_or_else(String::new, |f| node_text(f, src).into_owned())
}

/// Go `luaRequireSpec`: the string literal passed to require, for both `require("m")` and
/// `require "m"`.
fn lua_require_spec(c: Node<'_>, src: &[u8]) -> String {
    let mut spec = String::new();
    walk(c.child_by_field_name("arguments"), |n| {
        if !spec.is_empty() {
            return false;
        }
        if n.kind() == "string_content" {
            spec = node_text(n, src).into_owned();
            return false;
        }
        true
    });
    spec
}

// Shared tree helpers, used by c/golang/java/js/cpp; filled by the runtime stream.

/// Go `childByKind` (`lua.go:223`): the first direct child of `n` with the given kind.
pub(crate) fn child_by_kind<'t>(n: gositter::Node<'t>, kind: &str) -> Option<gositter::Node<'t>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.kind() == kind)
}

/// Go `firstNamed` (`lua.go:233`): the first named child of `n`.
pub(crate) fn first_named(n: gositter::Node<'_>) -> Option<gositter::Node<'_>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.is_named())
}

#[cfg(test)]
#[path = "tests/lua.rs"]
mod lua_tests;

#[cfg(test)]
#[path = "tests/lua_helpers.rs"]
mod lua_helpers_tests;
