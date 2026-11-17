//! Port of `cpp.go`: free functions, types (class/struct/enum + methods), #includes /
//! using-declarations, and call edges out of a C++ file. Items nested in `namespace` blocks are
//! descended into so namespaced code still surfaces its definitions. Methods are scoped under
//! their enclosing type, whether defined inline in the class body or out-of-line as
//! `Type::method`.

use std::collections::HashSet;
use std::sync::{LazyLock, OnceLock};

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk_kinds};
use super::c::sig_types;
use super::extract::{Def, Result};
use super::lua::first_named;
use crate::model::{Confidence, Edge, Loc, Relation};

/// The Go `builder` plus its `cppTParams` (the template parameters in scope).
struct Cpp {
    b: Builder,
    /// Go `cppTParams`.
    t_params: HashSet<String>,
}

/// Go `extractCpp`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_cpp(rel: &str, src: &[u8]) -> Option<Result> {
    let src = blank_cpp_export_macros(src);
    let src = &src[..];
    parse_root(src, gositter_grammars::cpp::language(), |root| {
        let mut c = Cpp {
            b: Builder::new(rel),
            t_params: HashSet::new(),
        };

        c.cpp_items(root, src);
        c.b.res
    })
}

/// Go `cppExportMacroRe` (`cpp.go:28`). Anchored: it runs only where a keyword starts, not over
/// the whole file.
static CPP_EXPORT_MACRO_RE: LazyLock<gostd::regexp::Regexp> = LazyLock::new(|| {
    gostd::regexp::Regexp::must_compile(
        r"^(?:class|struct)\s+((?:[A-Z][A-Z0-9_]*\s+)+)([A-Za-z_]\w*(?:\s+final)?\s*)([:{])",
    )
});

/// Go `blankCppExportMacros` (`cpp.go:36`): the ALL-CAPS export macros between `class`/`struct`
/// and the type name (`class Q_CORE_EXPORT Widget`) become spaces: the grammar otherwise reads
/// the macro as the type and the class as a function. Newlines are kept so offsets and line
/// numbers hold.
pub(crate) fn blank_cpp_export_macros(src: &[u8]) -> Vec<u8> {
    let mut out: Option<Vec<u8>> = None;
    let next = |kw: &[u8], from: usize| -> usize {
        match src[from..].windows(kw.len()).position(|w| w == kw) {
            Some(i) => from + i,
            None => src.len(),
        }
    };
    let (mut ci, mut si) = (next(b"class", 0), next(b"struct", 0));
    let mut pos = 0;
    loop {
        if ci < pos {
            ci = next(b"class", pos);
        }
        if si < pos {
            si = next(b"struct", pos);
        }
        let p = ci.min(si);
        if p == src.len() {
            break;
        }
        pos = p + 1;
        // The \b the unanchored pattern began with.
        if p > 0 && (src[p - 1] == b'_' || src[p - 1].is_ascii_alphanumeric()) {
            continue;
        }
        let Some(m) = CPP_EXPORT_MACRO_RE.find_submatch_index(&src[p..]) else {
            continue;
        };
        let g = |i: usize| m[i].expect("group");
        let (m0, m1) = (g(0).0 + p, g(0).1 + p);
        let (m2, m3) = (g(1).0 + p, g(1).1 + p);
        let (m4, m5) = (g(2).0 + p, g(2).1 + p);
        let (m6, m7) = (g(3).0 + p, g(3).1 + p);
        pos = m1;
        // `for (class API v : items)` is a variable, not a type.
        if gostd::strings::bytes_trim_space(&src[..m0]).ends_with(b"(") {
            continue;
        }
        // `class API v{1};` is a brace initialiser.
        let rest = gostd::strings::bytes_trim_space(&src[m7..]);
        if src[m6] == b'{' && !rest.is_empty() && b"0123456789\"'-".contains(&rest[0]) {
            continue;
        }
        let mut end = m3;
        // `class API WIDGET final :` has no lookahead: WIDGET is the name.
        if gostd::strings::bytes_trim_space(&src[m4..m5]) == b"final" {
            let t = gostd::strings::bytes_trim_space(&src[m2..m3]);
            end = m2
                + t.iter()
                    .rposition(|c| b" \t\r\n".contains(c))
                    .map_or(0, |i| i + 1);
        }
        let out = out.get_or_insert_with(|| src.to_vec());
        for c in &mut out[m2..end] {
            if *c != b'\r' && *c != b'\n' {
                *c = b' ';
            }
        }
    }
    out.unwrap_or_else(|| src.to_vec())
}

impl Cpp {
    /// Go `cppItems`: handles each item directly under n (a translation_unit or a namespace's
    /// declaration_list), recursing into nested namespaces.
    fn cpp_items(&mut self, n: Node<'_>, src: &[u8]) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            match c.kind() {
                "function_definition" => self.cpp_func(c, src),
                "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier" => {
                    self.cpp_type(c, src)
                }
                "preproc_include" => self.cpp_include(c, src),
                "using_declaration" => self.cpp_using(c, src),
                "namespace_definition" => {
                    if let Some(body) = c.child_by_field_name("body") {
                        self.cpp_items(body, src);
                    }
                }
                "template_declaration" => {
                    // Unwrap `template<...>` and re-dispatch on the templated item.
                    // Its parameters name no type while it lasts.
                    let inner = cpp_template_params(c, &self.t_params, src);
                    let outer = std::mem::replace(&mut self.t_params, inner);
                    self.cpp_items(c, src);
                    self.t_params = outer;
                }
                "preproc_if"
                | "preproc_ifdef"
                | "preproc_else"
                | "preproc_elif"
                | "preproc_elifdef"
                | "linkage_specification"
                | "declaration_list"
                | "ERROR" => self.cpp_items(c, src),
                "type_definition" => {
                    // `typedef struct S {...} T;` defines S, and T names the tag S.
                    let mut tag = String::new();
                    if let Some(spec) = c.child_by_field_name("type")
                        && matches!(
                            spec.kind(),
                            "struct_specifier" | "union_specifier" | "enum_specifier"
                        )
                    {
                        if spec.child_by_field_name("body").is_some() {
                            self.cpp_type(spec, src);
                        }
                        tag = cpp_name_text(spec.child_by_field_name("name"), src);
                    }
                    for j in 0..c.child_count() {
                        let Some(d) = c.child(j) else { continue };
                        if c.field_name_for_child(j) == Some("declarator")
                            && d.kind() == "type_identifier"
                        {
                            self.b
                                .res
                                .type_aliases
                                .push(format!("{} {}", node_text(d, src), tag));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Go `cppFunc`: records a free function (or an out-of-line method definition such as
    /// `Server::start`). Out-of-line definitions whose declarator names a scope are scoped under
    /// that type so they share the type's method node.
    fn cpp_func(&mut self, n: Node<'_>, src: &[u8]) {
        let (scope, name) = cpp_declarator_name(n.child_by_field_name("declarator"), src);
        if name.is_empty() {
            return;
        }
        if !scope.is_empty() {
            let mid = self.b.id(&[&scope, &name]);
            self.b.add_node(&mid, &format!("{scope}.{name}()"), line(n));
            let type_id = self.b.id(&[&scope]);
            if !self.b.seen.contains(&type_id) {
                // class is in a header: Resolve finds it
                self.b.contains(&mid, line(n));
                self.b.res.defs.push(Def {
                    id: mid.clone(),
                    name,
                    file: self.b.file.clone(),
                    owner: scope,
                    ..Def::default()
                });
                self.cpp_signature(&mid, n, src);
                self.cpp_calls(n.child_by_field_name("body"), &mid, src);
                return;
            }
            self.b.res.edges.push(Edge {
                source: type_id,
                target: mid.clone(),
                relation: Relation::CONTAINS,
                confidence: Confidence::EXTRACTED,
                source_file: self.b.src,
                source_location: line(n),
                ..Edge::default()
            });
            self.b.res.defs.push(Def {
                id: mid.clone(),
                name,
                file: self.b.file.clone(),
                ..Def::default()
            });
            self.cpp_signature(&mid, n, src);
            self.cpp_calls(n.child_by_field_name("body"), &mid, src);
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &format!("{name}()"), line(n));
        self.cpp_signature(&id, n, src);
        self.cpp_calls(n.child_by_field_name("body"), &id, src);
    }

    /// Go `cppSignature`: records a references edge from the function id to the types its
    /// return type and its parameters name.
    fn cpp_signature(&mut self, id: &str, func: Node<'_>, src: &[u8]) {
        sig_types(func, |t| self.cpp_type_refs(id, Some(t), src, line(func)));
    }

    /// Go `cppTypeRefs`: records a references edge from id to the type t names and to its
    /// template arguments. `ns::T` gives T, `struct S` and a primitive type give nothing, and a
    /// template parameter in scope names no type.
    fn cpp_type_refs(&mut self, id: &str, t: Option<Node<'_>>, src: &[u8], loc: Loc) {
        let Some(t) = t else { return };
        match t.kind() {
            "type_identifier" => {
                let name = node_text(t, src);
                if !self.t_params.contains(&*name) {
                    self.b.ref_(id, &name, "references", false, loc);
                }
            }
            "qualified_identifier" => {
                self.cpp_type_refs(id, t.child_by_field_name("name"), src, loc);
            }
            "template_type" => {
                self.cpp_type_refs(id, t.child_by_field_name("name"), src, loc);
                self.cpp_template_args(id, t, src, loc);
            }
            _ => {}
        }
    }

    /// Go `cppTemplateArgs`: records the types in the template arguments of t.
    fn cpp_template_args(&mut self, id: &str, t: Node<'_>, src: &[u8], loc: Loc) {
        let Some(args) = t.child_by_field_name("arguments") else {
            return;
        };
        for i in 0..args.named_child_count() {
            if let Some(a) = args.named_child(i)
                && a.kind() == "type_descriptor"
            {
                self.cpp_type_refs(id, a.child_by_field_name("type"), src, loc);
            }
        }
    }

    /// Go `cppType`: records a class/struct/union/enum and the methods inside its body. Inline
    /// methods appear as function_definition children of the field_declaration_list and are
    /// scoped under the type's name.
    fn cpp_type(&mut self, n: Node<'_>, src: &[u8]) {
        let name = cpp_name_text(n.child_by_field_name("name"), src);
        if name.is_empty() {
            return;
        }
        let type_id = self.b.id(&[&name]);
        self.b.def(&type_id, &name, &name, line(n));

        for i in 0..n.named_child_count() {
            let Some(c) = n.named_child(i) else { continue };
            if c.kind() != "base_class_clause" {
                continue;
            }
            for j in 0..c.named_child_count() {
                let Some(mut base) = c.named_child(j) else {
                    continue;
                };
                if base.kind() == "access_specifier" {
                    continue;
                }
                // `: Base<Dep>` inherits Base and references Dep.
                let bname = cpp_name_text(Some(base), src);
                if !self.t_params.contains(&bname) {
                    self.b.ref_(&type_id, &bname, "inherits", false, line(n));
                }
                if base.kind() == "qualified_identifier" {
                    match base.child_by_field_name("name") {
                        Some(x) => base = x,
                        None => continue,
                    }
                }
                if base.kind() == "template_type" {
                    self.cpp_template_args(&type_id, base, src, line(n));
                }
            }
        }

        if let Some(body) = n.child_by_field_name("body") {
            self.cpp_members(body, &type_id, &name, src);
        }
    }

    /// Go `cppMembers`: records the inline methods directly under body, and those behind a
    /// preprocessor conditional in it.
    fn cpp_members(&mut self, body: Node<'_>, type_id: &str, name: &str, src: &[u8]) {
        for i in 0..body.child_count() {
            let Some(m) = body.child(i) else { continue };
            if m.kind().starts_with("preproc_") {
                self.cpp_members(m, type_id, name, src);
                continue;
            }
            if m.kind() == "field_declaration"
                && !cpp_is_method_decl(m.child_by_field_name("declarator"))
            {
                self.cpp_type_refs(type_id, m.child_by_field_name("type"), src, line(m));
            }
            if m.kind() != "function_definition" {
                continue;
            }
            let (_, mname) = cpp_declarator_name(m.child_by_field_name("declarator"), src);
            if mname.is_empty() {
                continue;
            }
            let mid = self.b.id(&[name, &mname]);
            self.b.add_node(&mid, &format!("{name}.{mname}()"), line(m));
            self.b.res.edges.push(Edge {
                source: type_id.to_string(),
                target: mid.clone(),
                relation: Relation::CONTAINS,
                confidence: Confidence::EXTRACTED,
                source_file: self.b.src,
                source_location: line(m),
                ..Edge::default()
            });
            // Register under the bare method name so `x.method()` sites resolve.
            self.b.res.defs.push(Def {
                id: mid.clone(),
                name: mname,
                file: self.b.file.clone(),
                ..Def::default()
            });
            self.cpp_signature(&mid, m, src);
            self.cpp_calls(m.child_by_field_name("body"), &mid, src);
        }
    }

    /// Go `cppInclude`: records the included header. `#include "x.h"` records the path without
    /// quotes; `#include <x>` records the bare header name.
    fn cpp_include(&mut self, n: Node<'_>, src: &[u8]) {
        let Some(p) = n.child_by_field_name("path") else {
            return;
        };
        let text = node_text(p, src);
        let mut spec: &str = &text;
        match p.kind() {
            "string_literal" => {
                self.b.imp_rel(trim_delims(spec, b'"', b'"'), line(n));
                return;
            }
            "system_lib_string" => spec = trim_delims(spec, b'<', b'>'),
            _ => {}
        }
        self.b.imp(spec, line(n));
    }

    /// Go `cppUsing`: records a `using` declaration's qualified path as an import.
    fn cpp_using(&mut self, n: Node<'_>, src: &[u8]) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            if c.kind() == "identifier" || c.kind() == "qualified_identifier" {
                self.b.imp(&node_text(c, src), line(n));
                return;
            }
        }
    }

    /// Go `cppCalls`: walks a function body and records each call site. Direct calls (`f()`),
    /// member calls (`x.f()` / `p->f()`), and scoped calls (`N::f()`) all record the trailing
    /// called name.
    fn cpp_calls(&mut self, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
        if body.is_none() {
            return;
        }
        let mask = *CPP_CALL_KINDS.get().expect("register_kinds");
        walk_kinds(body, mask, |c| {
            let Some(func) = c.child_by_field_name("function") else {
                return true;
            };
            match func.kind() {
                "identifier" => self.b.call(caller_id, &node_text(func, src), line(c)),
                "field_expression" => {
                    if let Some(f) = func.child_by_field_name("field") {
                        self.b.call_recv(
                            caller_id,
                            &cpp_name_text(Some(f), src),
                            &recv_text(Some(func), Some(f), src),
                            line(c),
                        );
                    }
                }
                "qualified_identifier" => {
                    let mut nm = func.child_by_field_name("name");
                    while let Some(x) = nm
                        && x.kind() == "qualified_identifier"
                    {
                        nm = x.child_by_field_name("name");
                    }
                    if let Some(nm) = nm {
                        self.b.call_recv(
                            caller_id,
                            &cpp_name_text(Some(nm), src),
                            &recv_text(Some(func), Some(nm), src),
                            line(c),
                        );
                    }
                }
                "template_function" => self.b.call(
                    caller_id,
                    &cpp_name_text(func.child_by_field_name("name"), src),
                    line(c),
                ),
                _ => {}
            }
            true
        });
    }
}

/// Go `cppTemplateParams`: returns outer plus the type parameter names of the
/// template_declaration n.
fn cpp_template_params(n: Node<'_>, outer: &HashSet<String>, src: &[u8]) -> HashSet<String> {
    let mut out = outer.clone();
    let Some(list) = n.child_by_field_name("parameters") else {
        return out;
    };
    for i in 0..list.named_child_count() {
        let Some(mut p) = list.named_child(i) else {
            continue;
        };
        // `int N`: a value, but `v<T, N>` reads it as a type name.
        if let Some(d) = p.child_by_field_name("declarator")
            && d.kind() == "identifier"
        {
            out.insert(node_text(d, src).into_owned());
            continue;
        }
        // `template <class> class TT`: the name is in the last child.
        if p.kind() == "template_template_parameter_declaration"
            && p.named_child_count() > 0
            && let Some(last) = p.named_child(p.named_child_count() - 1)
        {
            p = last;
        }
        if !p.kind().ends_with("type_parameter_declaration") {
            continue;
        }
        // `class T = Default`: the first type name is the parameter.
        for j in 0..p.named_child_count() {
            if let Some(c) = p.named_child(j)
                && c.kind() == "type_identifier"
            {
                out.insert(node_text(c, src).into_owned());
                break;
            }
        }
    }
    out
}

/// Go `cppIsMethodDecl`: reports whether the declarator of a class member declares a method
/// (`T* m(int);`) and not a field or a function pointer (`T (*cb)(int);`).
fn cpp_is_method_decl(d: Option<Node<'_>>) -> bool {
    let mut d = d;
    while let Some(cur) = d
        && (cur.kind() == "pointer_declarator" || cur.kind() == "reference_declarator")
    {
        d = first_named(cur);
    }
    let Some(d) = d else { return false };
    if d.kind() != "function_declarator" {
        return false;
    }
    let inner = d.child_by_field_name("declarator");
    inner.is_none_or(|i| i.kind() != "parenthesized_declarator")
}

/// Go `cppDeclaratorName`: descends a function_definition's declarator chain to its innermost
/// name, returning (scope, name). scope is non-empty only for out-of-line `Scope::name`
/// definitions.
fn cpp_declarator_name(n: Option<Node<'_>>, src: &[u8]) -> (String, String) {
    let mut n = n;
    while let Some(cur) = n {
        match cur.kind() {
            "function_declarator"
            | "reference_declarator"
            | "pointer_declarator"
            | "parenthesized_declarator" => n = cur.child_by_field_name("declarator"),
            "qualified_identifier" => {
                return (
                    cpp_name_text(cur.child_by_field_name("scope"), src),
                    cpp_name_text(cur.child_by_field_name("name"), src),
                );
            }
            "identifier" | "field_identifier" | "type_identifier" | "destructor_name"
            | "operator_name" => return (String::new(), node_text(cur, src).into_owned()),
            _ => return (String::new(), String::new()),
        }
    }
    (String::new(), String::new())
}

/// Go `cppNameText`: returns the simple text of a name node, unwrapping a nested
/// qualified_identifier down to its trailing component.
fn cpp_name_text(n: Option<Node<'_>>, src: &[u8]) -> String {
    let Some(n) = n else { return String::new() };
    match n.kind() {
        "qualified_identifier" | "template_type" | "template_function" | "template_method" => {
            cpp_name_text(n.child_by_field_name("name"), src)
        }
        _ => node_text(n, src).into_owned(),
    }
}

/// Go `cppCallKinds` (`cpp.go:399`).
static CPP_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `trimDelims`: strips a single leading open and trailing close delimiter if both are
/// present (e.g. `"foo.h"` -> foo.h, `<vector>` -> vector).
fn trim_delims(s: &str, open: u8, close: u8) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && b[0] == open && b[b.len() - 1] == close {
        return &s[1..s.len() - 1];
    }
    s
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let _ = CPP_CALL_KINDS.set(gositter_grammars::cpp::language().kind_mask(&["call_expression"]));
}

#[cfg(test)]
#[path = "tests/cpp.rs"]
mod cpp_tests;

#[cfg(test)]
#[path = "tests/preproc.rs"]
mod preproc_tests;
