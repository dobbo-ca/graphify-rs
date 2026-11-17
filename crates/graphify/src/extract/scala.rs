//! Port of `scala.go`. Owner: the gostd stream; filled by graphify-rs-ihc.36.
//!
//! Go recurses on input depth in `scalaItem` (braced packages), `scalaNested`, `scalaTypeParams`,
//! `scalaBounds` and `scalaTypeRefs`; each is ported with an explicit stack in the same visit
//! order (plan hazard "Stack depth").

use std::collections::HashSet;
use std::sync::OnceLock;

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk_kinds};
use super::extract::{Def, Result};
use super::golang::field_text;
use crate::model::{Confidence, Edge, Relation};

/// Go `scalaCallKinds`, set by `register_kinds`.
static SCALA_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `extractScala` pulls top-level functions, types (class/object/trait/enum) and their
/// methods, imports, and call edges out of a .scala/.sc file. `None` when the parse budget stops
/// the parse (see `builder`).
pub(crate) fn extract_scala(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::scala::language(), |root| {
        let mut b = Builder::new(rel);
        for i in 0..root.child_count() {
            if let Some(n) = root.child(i) {
                b.scala_item(n, src);
            }
        }
        b.res
    })
}

impl Builder {
    /// Go `scalaItem` handles one compilation-unit-level item, descending into braced package
    /// clauses so package-organised sources still surface their definitions.
    fn scala_item(&mut self, n: Node<'_>, src: &[u8]) {
        let mut stack = vec![n];
        while let Some(n) = stack.pop() {
            match n.kind() {
                "function_definition" => self.scala_func(n, src),
                "class_definition" | "object_definition" | "trait_definition"
                | "enum_definition" | "package_object" => self.scala_type(n, src),
                "import_declaration" => self.scala_import(n, src),
                "package_clause" => {
                    // Braced `package foo { ... }` nests definitions in a template body.
                    if let Some(body) = n.child_by_field_name("body") {
                        for i in (0..body.child_count()).rev() {
                            if let Some(c) = body.child(i) {
                                stack.push(c);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Go `scalaFunc`.
    fn scala_func(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let id = self.id(&[&name]);
        self.def(&id, &name, &format!("{name}()"), line(n));
        self.scala_signature(&id, n, &HashSet::new(), src);
        self.scala_calls(n.child_by_field_name("body"), &id, src);
    }

    /// Go `scalaType` records a class/object/trait/enum and the methods in its template body,
    /// scoping each method under the type's name.
    fn scala_type(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let type_id = self.id(&[&name]);
        self.def(&type_id, &name, &name, line(n));

        let body = n.child_by_field_name("body");
        let skip = scala_type_params(n, &scala_nested(body, src), src);
        self.scala_bases(&type_id, n, src);
        self.scala_class_refs(&type_id, n, &skip, src);
        let Some(body) = body else {
            return;
        };
        for i in 0..body.child_count() {
            let Some(m) = body.child(i) else { continue };
            match m.kind() {
                "val_definition" | "var_definition" => {
                    self.scala_type_refs(&type_id, m.child_by_field_name("type"), &skip, src);
                }
                "type_definition" => {
                    // `type F[A] = Map[A, B]`: A is the alias's own parameter.
                    let own = scala_type_params(m, &skip, src);
                    self.scala_type_refs(&type_id, m.child_by_field_name("type"), &own, src);
                }
                _ => {}
            }
            if m.kind() != "function_definition" && m.kind() != "function_declaration" {
                continue;
            }
            let mname = field_text(m, "name", src);
            if mname.is_empty() {
                continue;
            }
            let mid = self.id(&[&name, &mname]);
            self.add_node(&mid, &format!("{name}.{mname}()"), line(m));
            self.res.edges.push(Edge {
                source: type_id.clone(),
                target: mid.clone(),
                relation: Relation::CONTAINS,
                confidence: Confidence::EXTRACTED,
                source_file: self.src,
                source_location: line(m),
                ..Edge::default()
            });
            // Register under the bare method name so `x.method()` call sites resolve.
            self.res.defs.push(Def {
                id: mid.clone(),
                name: mname,
                file: self.file.clone(),
                is_abstract: m.kind() == "function_declaration",
                ..Def::default()
            });
            self.scala_signature(&mid, m, &skip, src);
            self.scala_calls(m.child_by_field_name("body"), &mid, src);
        }
    }

    /// Go `scalaImport` records the dotted path of an `import` declaration, e.g.
    /// `scala.collection.mutable.Map` for `import scala.collection.mutable.Map`, or `a.b` for
    /// `import a.b.{c, d}`. Selector groups and wildcards are dropped. Path segments and their
    /// `.` separators are anonymous/named children, so raw children are iterated.
    fn scala_import(&mut self, n: Node<'_>, src: &[u8]) {
        let mut path = String::new();
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            match c.kind() {
                "identifier" | "operator_identifier" | "." => path.push_str(&node_text(c, src)),
                "namespace_selectors" | "namespace_wildcard" | "as_renamed_identifier" => {
                    // Stop at the first selector clause; everything before it is the path.
                    self.imp(trim_trailing_dot(&path), line(n));
                    return;
                }
                _ => {}
            }
        }
        self.imp(trim_trailing_dot(&path), line(n));
    }

    /// Go `scalaCalls` walks a definition body and records each call site. Direct calls (`f()`)
    /// record the identifier; field calls (`x.f()`) record the trailing field name.
    fn scala_calls(&mut self, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
        if body.is_none() {
            return;
        }
        let mask = *SCALA_CALL_KINDS.get().expect("register_kinds");
        walk_kinds(body, mask, |c| {
            let Some(mut f) = c.child_by_field_name("function") else {
                return true;
            };
            // `f[T]()` wraps the callee in a generic_function.
            if f.kind() == "generic_function"
                && let Some(g) = f.child_by_field_name("function")
            {
                f = g;
            }
            match f.kind() {
                "identifier" => self.call(caller_id, &node_text(f, src), line(c)),
                "field_expression" => {
                    if let Some(name) = f.child_by_field_name("field") {
                        let recv = recv_text(Some(f), Some(name), src);
                        self.call_recv(caller_id, &node_text(name, src), &recv, line(c));
                    }
                }
                _ => {}
            }
            true
        });
    }

    /// Go `scalaBases` records the first base as inherits and the rest as mixes_in.
    fn scala_bases(&mut self, type_id: &str, n: Node<'_>, src: &[u8]) {
        let Some(ext) = n.child_by_field_name("extend") else {
            return;
        };
        let mut rel = "inherits";
        for i in 0..ext.named_child_count() {
            let Some(mut c) = ext.named_child(i) else {
                continue;
            };
            if c.kind() == "generic_type" {
                match c.child_by_field_name("type") {
                    Some(t) => c = t,
                    None => continue,
                }
            }
            let mut name = String::new();
            match c.kind() {
                "type_identifier" => name = node_text(c, src).into_owned(),
                "stable_type_identifier" => {
                    for j in 0..c.named_child_count() {
                        if let Some(s) = c.named_child(j) {
                            name = node_text(s, src).into_owned();
                        }
                    }
                }
                _ => {}
            }
            if !name.is_empty() {
                self.ref_(type_id, &name, rel, false, line(c));
                rel = "mixes_in";
            }
        }
    }

    /// Go `scalaClassRefs` records the constructor parameter types, context bounds and derives
    /// clause of a type.
    fn scala_class_refs(&mut self, type_id: &str, n: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        for i in 0..n.named_child_count() {
            let Some(c) = n.named_child(i) else { continue };
            match c.kind() {
                "class_parameters" => {
                    for j in 0..c.named_child_count() {
                        if let Some(p) = c.named_child(j)
                            && p.kind() == "class_parameter"
                        {
                            self.scala_type_refs(type_id, p.child_by_field_name("type"), skip, src);
                        }
                    }
                }
                "type_parameters" => self.scala_bounds(type_id, c, skip, src),
                "derives_clause" => {
                    for j in 0..c.named_child_count() {
                        let Some(mut d) = c.named_child(j) else {
                            continue;
                        };
                        if d.kind() != "type_identifier"
                            && d.named_child_count() > 0
                            && let Some(last) = d.named_child(d.named_child_count() - 1)
                        {
                            d = last;
                        }
                        if d.kind() == "type_identifier" {
                            self.ref_(type_id, &node_text(d, src), "references", false, line(d));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Go `scalaSignature` records the types of the first parameter list, the return type and
    /// the context bounds of a function.
    fn scala_signature(&mut self, id: &str, n: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        let skip = scala_type_params(n, skip, src);
        let mut params = false;
        for i in 0..n.named_child_count() {
            let Some(c) = n.named_child(i) else { continue };
            match c.kind() {
                "parameters" => {
                    if params {
                        continue;
                    }
                    params = true;
                    for j in 0..c.named_child_count() {
                        if let Some(p) = c.named_child(j)
                            && p.kind() == "parameter"
                        {
                            self.scala_type_refs(id, p.child_by_field_name("type"), &skip, src);
                        }
                    }
                }
                "type_parameters" => self.scala_bounds(id, c, &skip, src),
                _ => {}
            }
        }
        self.scala_type_refs(id, n.child_by_field_name("return_type"), &skip, src);
    }

    /// Go `scalaBounds` records the context bounds (`[A: Ordering]`) of a type parameter list,
    /// nested lists included.
    fn scala_bounds(&mut self, id: &str, list: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        let mut stack = Vec::new();
        push_named_rev(&mut stack, list);
        while let Some(c) = stack.pop() {
            match c.kind() {
                "type_parameters" => push_named_rev(&mut stack, c),
                "context_bound" => {
                    self.scala_type_refs(id, c.child_by_field_name("type"), skip, src);
                }
                _ => {}
            }
        }
    }

    /// Go `scalaTypeRefs` records a reference from id to each type t names, generic arguments
    /// included. Names in skip give nothing.
    fn scala_type_refs(
        &mut self,
        id: &str,
        t: Option<Node<'_>>,
        skip: &HashSet<String>,
        src: &[u8],
    ) {
        let Some(t) = t else { return };
        let mut stack = vec![t];
        while let Some(t) = stack.pop() {
            match t.kind() {
                "type_identifier" => {
                    let name = node_text(t, src);
                    if !skip.contains(&*name) {
                        self.ref_(id, &name, "references", false, line(t));
                    }
                }
                "generic_type" => {
                    // Go visits the base, then each type argument, in order.
                    let mut next = Vec::new();
                    if let Some(base) = t.child_by_field_name("type")
                        && base.kind() == "type_identifier"
                    {
                        next.push(base);
                    }
                    for i in 0..t.named_child_count() {
                        if let Some(args) = t.named_child(i)
                            && args.kind() == "type_arguments"
                        {
                            for j in 0..args.named_child_count() {
                                if let Some(a) = args.named_child(j) {
                                    next.push(a);
                                }
                            }
                        }
                    }
                    stack.extend(next.into_iter().rev());
                }
                "compound_type" | "infix_type" | "function_type" | "tuple_type"
                | "annotated_type" | "projected_type" | "match_type" | "type_case_clause" => {
                    push_named_rev(&mut stack, t);
                }
                _ => {}
            }
        }
    }
}

/// Pushes the named children of `n` in reverse, so popping visits them in order.
fn push_named_rev<'t>(stack: &mut Vec<Node<'t>>, n: Node<'t>) {
    for i in (0..n.named_child_count()).rev() {
        if let Some(c) = n.named_child(i) {
            stack.push(c);
        }
    }
}

/// Go `trimTrailingDot` (scala.go).
fn trim_trailing_dot(s: &str) -> &str {
    s.strip_suffix('.').unwrap_or(s)
}

/// Go `scalaNested` returns the names of the types nested in the template body. They have no
/// node, so a ref to one must not bind to a namesake.
fn scala_nested(body: Option<Node<'_>>, src: &[u8]) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack: Vec<Node<'_>> = body.into_iter().collect();
    while let Some(body) = stack.pop() {
        for i in 0..body.named_child_count() {
            let Some(c) = body.named_child(i) else {
                continue;
            };
            if matches!(
                c.kind(),
                "class_definition" | "object_definition" | "trait_definition" | "enum_definition"
            ) {
                out.insert(field_text(c, "name", src));
                stack.extend(c.child_by_field_name("body"));
            }
        }
    }
    out
}

/// Go `scalaTypeParams` returns skip plus the type parameters n declares.
fn scala_type_params(n: Node<'_>, skip: &HashSet<String>, src: &[u8]) -> HashSet<String> {
    let mut out = skip.clone();
    let mut stack = Vec::new();
    for i in 0..n.named_child_count() {
        if let Some(c) = n.named_child(i)
            && c.kind() == "type_parameters"
        {
            stack.push(c);
        }
    }
    while let Some(list) = stack.pop() {
        for i in 0..list.named_child_count() {
            let Some(c) = list.named_child(i) else {
                continue;
            };
            match c.kind() {
                "identifier" => {
                    out.insert(node_text(c, src).into_owned());
                }
                "covariant_type_parameter" | "contravariant_type_parameter" => {
                    out.insert(field_text(c, "name", src));
                }
                "type_parameters" => stack.push(c),
                _ => {}
            }
        }
    }
    out
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    SCALA_CALL_KINDS
        .get_or_init(|| gositter_grammars::scala::language().kind_mask(&["call_expression"]));
}

#[cfg(test)]
#[path = "tests/scala.rs"]
mod scala_tests;
