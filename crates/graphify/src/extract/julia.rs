//! Port of `julia.go`: functions, types (struct/abstract/primitive), modules with their
//! contained functions (treated as methods), imports (`import`/`using`), and call edges out of a
//! .jl file. Julia organises code in modules rather than classes, so a `module` becomes a
//! type-like node and the `function`s defined directly inside it are scoped under it as methods.

use std::sync::OnceLock;

use gositter::Node;
use gositter_grammars::julia;

use super::builder::{Builder, line, node_text, parse_root, walk, walk_kinds};
use super::extract::{Def, Result};
use super::golang::field_text;
use crate::model::{Confidence, Edge, Relation};

/// Go `juliaCallKinds` (`julia.go:237`), named by [`register_kinds`].
static CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    CALL_KINDS.get_or_init(|| julia::language().kind_mask(&["call_expression"]));
}

/// Go `extractJulia`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_julia(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, julia::language(), |root| {
        let mut b = Builder::new(rel);
        julia_items(&mut b, root, src);
        b.res
    })
}

/// Go `juliaItem` over the root's children, with `juliaModule` folded in and its recursion on
/// module nesting made a stack (Go's visit order). Each entry carries the index in `mods` of
/// the module whose body holds it (`None` at the top level): a function directly in a module
/// body is a method of it; every other statement is handled as at the top level.
fn julia_items(b: &mut Builder, root: Node<'_>, src: &[u8]) {
    // (module name, module id)
    let mut mods: Vec<(String, String)> = Vec::new();
    let mut buf: Vec<Node<'_>> = Vec::new();
    root.children(&mut buf);
    let mut stack: Vec<(Node<'_>, Option<usize>)> = buf.iter().rev().map(|&c| (c, None)).collect();
    while let Some((n, module)) = stack.pop() {
        match (n.kind(), module) {
            ("function_definition" | "macro_definition", Some(mi)) => {
                let (name, mod_id) = &mods[mi];
                julia_method(b, n, name, mod_id, src);
            }
            ("function_definition" | "macro_definition", None) => julia_func(b, n, src),
            ("struct_definition" | "abstract_definition" | "primitive_definition", _) => {
                julia_type(b, n, src);
            }
            ("module_definition", _) => {
                let name = field_text(n, "name", src);
                if name.is_empty() {
                    continue;
                }
                let mod_id = b.id(&[&name]);
                b.def(&mod_id, &name, &name, line(n));
                let Some(body) = julia_body(n) else { continue };
                mods.push((name, mod_id));
                let idx = mods.len() - 1;
                buf.clear();
                body.children(&mut buf);
                stack.extend(buf.iter().rev().map(|&c| (c, Some(idx))));
            }
            ("import_statement" | "using_statement", _) => julia_imports(b, n, src),
            // Short function form: `f(x) = expr`.
            ("assignment", _) => julia_short_func(b, n, src),
            _ => {}
        }
    }
}

/// Go `juliaFunc`: a top-level function/macro definition and the calls in its body.
fn julia_func(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = julia_func_name(n, src);
    if name.is_empty() {
        return;
    }
    let id = b.id(&[&name]);
    b.def(&id, &name, &format!("{name}()"), line(n));
    julia_calls(b, julia_body(n), &id, src);
}

/// The function arm of Go `juliaModule`'s body loop: a function defined directly in module
/// `name` (id `mod_id`) becomes `name.fn()` with a contains edge from the module.
fn julia_method(b: &mut Builder, m: Node<'_>, name: &str, mod_id: &str, src: &[u8]) {
    let mname = julia_func_name(m, src);
    if mname.is_empty() {
        return;
    }
    let mid = b.id(&[name, &mname]);
    b.add_node(&mid, &format!("{name}.{mname}()"), line(m));
    b.res.edges.push(Edge {
        source: mod_id.to_string(),
        target: mid.clone(),
        relation: Relation::CONTAINS,
        confidence: Confidence::EXTRACTED,
        source_file: b.src,
        source_location: line(m),
        ..Edge::default()
    });
    b.res.defs.push(Def {
        id: mid.clone(),
        name: mname,
        file: b.file.clone(),
        ..Def::default()
    });
    julia_calls(b, julia_body(m), &mid, src);
}

/// Go `juliaShortFunc`: `f(x) = expr`, where the assignment's left side is a call_expression.
/// Other assignments are ignored.
fn julia_short_func(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let Some(lhs) = n.named_child(0) else { return };
    if lhs.kind() != "call_expression" {
        return;
    }
    let name = julia_callee_name(lhs.named_child(0), src);
    if name.is_empty() {
        return;
    }
    let id = b.id(&[&name]);
    b.def(&id, &name, &format!("{name}()"), line(n));
    // Record calls on the right-hand side of the definition.
    julia_calls(b, n.named_child(1), &id, src);
}

/// Go `juliaType`: a struct/abstract/primitive type definition.
fn julia_type(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    let name = julia_type_name(n, src);
    if name.is_empty() {
        return;
    }
    let id = b.id(&[&name]);
    b.def(&id, &name, &name, line(n));
}

/// Go `juliaImports`: each `import`/`using` path. `using A`, `import A.B`, and `using A: x` all
/// record the leading module path (A or A.B).
fn julia_imports(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    for i in 0..n.named_child_count() {
        let Some(c) = n.named_child(i) else { continue };
        match c.kind() {
            "identifier" | "import_path" => b.imp(&node_text(c, src), line(n)),
            // `import A as B` / `import A.B as C`: the source path.
            "import_alias" => {
                if let Some(p) = c.named_child(0) {
                    b.imp(&node_text(p, src), line(n));
                }
            }
            // `using A: x, y`: the leading module path only.
            "selected_import" => {
                if let Some(p) = c.named_child(0)
                    && matches!(p.kind(), "import_path" | "identifier")
                {
                    b.imp(&node_text(p, src), line(n));
                }
            }
            _ => {}
        }
    }
}

/// Go `juliaBody`: the `block` child of a definition node.
fn julia_body(n: Node<'_>) -> Option<Node<'_>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.kind() == "block")
}

/// Go `juliaFuncName`: the defined name, from the definition's `signature` child.
fn julia_func_name(n: Node<'_>, src: &[u8]) -> String {
    match (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.kind() == "signature")
    {
        Some(sig) => julia_signature_name(sig, src),
        None => String::new(),
    }
}

/// Go `juliaSignatureName`: unwraps a signature (call_expression, typed_expression,
/// where_expression, or bare identifier) down to the called identifier.
fn julia_signature_name(sig: Node<'_>, src: &[u8]) -> String {
    let mut name = String::new();
    walk(Some(sig), |c| {
        if !name.is_empty() {
            return false;
        }
        match c.kind() {
            "call_expression" => {
                name = julia_callee_name(c.named_child(0), src);
                false
            }
            "identifier" => {
                name = node_text(c, src).into_owned();
                false
            }
            _ => true,
        }
    });
    name
}

/// Go `juliaTypeName`: the first identifier of the `type_head` child (the name, before any
/// `<:` supertype or `{T}` params).
fn julia_type_name(n: Node<'_>, src: &[u8]) -> String {
    let Some(head) = (0..n.child_count())
        .filter_map(|i| n.child(i))
        .find(|c| c.kind() == "type_head")
    else {
        return String::new();
    };
    let mut name = String::new();
    walk(Some(head), |d| {
        if !name.is_empty() {
            return false;
        }
        if d.kind() == "identifier" {
            name = node_text(d, src).into_owned();
            return false;
        }
        true
    });
    name
}

/// Go `juliaCalleeName`: a bare identifier, or the trailing field of a `Mod.f`
/// field_expression.
fn julia_callee_name(f: Option<Node<'_>>, src: &[u8]) -> String {
    let Some(f) = f else { return String::new() };
    match f.kind() {
        "identifier" => node_text(f, src).into_owned(),
        // Trailing name is the last identifier child (`x.f` -> "f").
        "field_expression" => {
            let mut name = String::new();
            for i in 0..f.named_child_count() {
                if let Some(c) = f.named_child(i)
                    && c.kind() == "identifier"
                {
                    name = node_text(c, src).into_owned();
                }
            }
            name
        }
        _ => String::new(),
    }
}

/// Go `juliaCalls`: each call site in `body` by its simple callee name. Macro invocations
/// (`@m ...`) are not call edges.
fn julia_calls(b: &mut Builder, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
    let mask = *CALL_KINDS
        .get()
        .expect("kinds::register_kinds was not called");
    walk_kinds(body, mask, |c| {
        let f = c.named_child(0);
        let name = julia_callee_name(f, src);
        if !name.is_empty() {
            let recv = f
                .filter(|f| f.kind() == "field_expression")
                .and_then(|f| f.named_child(0))
                .map_or_else(String::new, |r| node_text(r, src).into_owned());
            b.call_recv(caller_id, &name, &recv, line(c));
        }
        true
    });
}

#[cfg(test)]
#[path = "tests/julia.rs"]
mod julia_tests;
