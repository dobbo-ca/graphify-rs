//! Port of `ruby.go`: top-level methods, classes/modules (+ their methods), require-style
//! imports and call edges out of a .rb (or .rake) file. Top-level methods and classes/modules
//! become definitions; methods defined inside a class/module are nested under it. Calls are
//! recorded by the called method name and resolved against the corpus by `resolve`.
//!
//! Go keeps `rubyMethods` on its one `builder`; here it is a field of [`Rb`], beside a `Builder`.

use std::collections::HashSet;
use std::sync::OnceLock;

use gositter::Node;
use gositter_grammars::ruby;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk, walk_kinds};
use super::extract::{Def, Result};
use super::golang::field_text;
use super::lua::first_named;
use crate::model::{Confidence, Edge, Relation};

/// Go `rubyMethodKinds` (`ruby.go:168`): the mask of `method`.
static RUBY_METHOD_KINDS: OnceLock<u32> = OnceLock::new();
/// Go `rubyLocalKinds` (`ruby.go:169`): the nodes that bind a local.
static RUBY_LOCAL_KINDS: OnceLock<u32> = OnceLock::new();
/// Go `rubyCallKinds` (`ruby.go:171`): `call` and `identifier`.
static RUBY_CALL_KINDS: OnceLock<u32> = OnceLock::new();

fn mask(m: &OnceLock<u32>) -> u32 {
    *m.get().expect("kinds::register_kinds was not called")
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let lang = ruby::language();
    RUBY_METHOD_KINDS.get_or_init(|| lang.kind_mask(&["method"]));
    RUBY_LOCAL_KINDS.get_or_init(|| {
        lang.kind_mask(&[
            "assignment",
            "operator_assignment",
            "left_assignment_list",
            "block_parameters",
            "lambda_parameters",
            "exception_variable",
            "for",
        ])
    });
    RUBY_CALL_KINDS.get_or_init(|| lang.kind_mask(&["call", "identifier"]));
}

/// Go `rubyRequires`: the method names that act like imports; their first string argument
/// names the required file/library.
fn ruby_requires(name: &str) -> bool {
    matches!(name, "require" | "require_relative" | "load" | "autoload")
}

/// Go `extractRuby`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_ruby(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, ruby::language(), |root| {
        let mut ruby_methods = HashSet::new();
        walk_kinds(Some(root), mask(&RUBY_METHOD_KINDS), |nv| {
            ruby_methods.insert(field_text(nv, "name", src));
            true
        });
        let mut rb = Rb {
            b: Builder::new(rel),
            ruby_methods,
        };
        for i in 0..root.child_count() {
            if let Some(n) = root.child(i) {
                rb.ruby_statement(n, src);
            }
        }
        rb.b.res
    })
}

/// Go's `builder` with its Ruby field.
struct Rb {
    b: Builder,
    /// Go `rubyMethods`: method names defined in the file (bare self-sends).
    ruby_methods: HashSet<String>,
}

impl Rb {
    /// Go `rubyStatement`: one top-level statement.
    fn ruby_statement(&mut self, n: Node<'_>, src: &[u8]) {
        match n.kind() {
            "method" => self.ruby_method(n, src),
            "class" | "module" => self.ruby_type(n, src),
            "call" => ruby_require(&mut self.b, n, "", src),
            _ => {}
        }
    }

    /// Go `rubyMethod`.
    fn ruby_method(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &format!("{name}()"), line(n));
        self.ruby_calls(n.child_by_field_name("body"), &id, src);
    }

    /// Go `rubyType`: a class or module and the methods defined directly in its body, each
    /// method scoped under the type's bare name.
    fn ruby_type(&mut self, n: Node<'_>, src: &[u8]) {
        let name = ruby_const_name(n.child_by_field_name("name"), src);
        if name.is_empty() {
            return;
        }
        let type_id = self.b.id(&[&name]);
        self.b.def(&type_id, &name, &name, line(n));
        // The superclass lets an inherited self-send walk the chain.
        if let Some(sup) = n.child_by_field_name("superclass") {
            let sup_name = ruby_const_name(first_named(sup), src);
            self.b.ref_(&type_id, &sup_name, "inherits", false, line(n));
        }

        let Some(body) = n.child_by_field_name("body") else {
            return;
        };
        for i in 0..body.child_count() {
            let Some(m) = body.child(i) else {
                continue;
            };
            match m.kind() {
                "method" => {
                    let mname = field_text(m, "name", src);
                    if mname.is_empty() {
                        continue;
                    }
                    let mid = self.b.id(&[&name, &mname]);
                    self.b.add_node(&mid, &format!("{name}.{mname}()"), line(m));
                    self.b.res.edges.push(Edge {
                        source: type_id.clone(),
                        target: mid.clone(),
                        relation: Relation::CONTAINS,
                        confidence: Confidence::EXTRACTED,
                        source_file: self.b.src,
                        source_location: line(m),
                        ..Edge::default()
                    });
                    self.b.res.defs.push(Def {
                        id: mid.clone(),
                        name: mname,
                        file: self.b.file.clone(),
                        ..Def::default()
                    });
                    self.ruby_calls(m.child_by_field_name("body"), &mid, src);
                }
                // require/include inside a class body still counts as a file import.
                "call" => ruby_require(&mut self.b, m, &type_id, src),
                _ => {}
            }
        }
    }

    /// Go `rubyCalls`: walks a method body and records each call site.
    fn ruby_calls(&mut self, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
        let Some(body) = body else {
            return;
        };
        // Parameters and assigned locals shadow same-named methods.
        let mut locals: HashSet<String> = HashSet::new();
        if let Some(m) = body.parent() {
            add_idents(m.child_by_field_name("parameters"), src, &mut locals);
        }
        walk_kinds(Some(body), mask(&RUBY_LOCAL_KINDS), |c| {
            match c.kind() {
                "assignment" | "operator_assignment" => {
                    if let Some(l) = c.child_by_field_name("left")
                        && l.kind() == "identifier"
                    {
                        locals.insert(node_text(l, src).into_owned());
                    }
                }
                "left_assignment_list"
                | "block_parameters"
                | "lambda_parameters"
                | "exception_variable" => add_idents(Some(c), src, &mut locals),
                "for" => add_idents(c.child_by_field_name("pattern"), src, &mut locals),
                _ => {}
            }
            true
        });
        let b = &mut self.b;
        let ruby_methods = &self.ruby_methods;
        walk_kinds(Some(body), mask(&RUBY_CALL_KINDS), |c| {
            match c.kind() {
                "call" => ruby_record_call(b, c, caller_id, src),
                // Paren-less self-send parses as a bare identifier statement.
                "identifier" if c.parent().is_some_and(|p| p.kind() == "body_statement") => {
                    let name = node_text(c, src);
                    if ruby_methods.contains(&*name) && !locals.contains(&*name) {
                        // Inside a class it binds to that class, not by bare name.
                        let in_type = body
                            .parent()
                            .and_then(|m| m.parent())
                            .is_some_and(|p| p.kind() != "program");
                        if in_type {
                            b.call_recv(caller_id, &name, "self", line(c));
                        } else {
                            b.call(caller_id, &name, line(c));
                        }
                    }
                }
                _ => {}
            }
            true
        });
    }
}

/// Go `rubyCalls`' `addIdents` (and its parameters walk): every identifier under `n`.
fn add_idents(n: Option<Node<'_>>, src: &[u8], locals: &mut HashSet<String>) {
    walk(n, |c| {
        if c.kind() == "identifier" {
            locals.insert(node_text(c, src).into_owned());
        }
        true
    });
}

/// Go `rubyRequire`: `require 'x'` / `require_relative 'x'` record the quoted path as the import
/// spec. If `caller_id` is non-empty, a non-require call is a call site (a call in a class body).
fn ruby_require(b: &mut Builder, n: Node<'_>, caller_id: &str, src: &[u8]) {
    let Some(method) = n.child_by_field_name("method") else {
        return;
    };
    let name = node_text(method, src);
    if ruby_requires(&name) {
        let spec = ruby_first_string_arg(n, src);
        if name == "require_relative" {
            b.imp_rel(&spec, line(n));
        } else {
            b.imp(&spec, line(n));
        }
        return;
    }
    if !caller_id.is_empty() {
        ruby_record_call(b, n, caller_id, src);
    }
}

/// Go `rubyConstName`: the bare name of a class/module's name node, the trailing constant of a
/// `scope_resolution` (`Bar` for `Foo::Bar`).
fn ruby_const_name(n: Option<Node<'_>>, src: &[u8]) -> String {
    let Some(n) = n else {
        return String::new();
    };
    match n.kind() {
        "constant" => node_text(n, src).into_owned(),
        "scope_resolution" => n
            .child_by_field_name("name")
            .map_or_else(String::new, |name| node_text(name, src).into_owned()),
        _ => String::new(),
    }
}

/// Go `rubyFirstStringArg`: the text of the first string-literal argument of a call, without
/// its quotes.
fn ruby_first_string_arg(n: Node<'_>, src: &[u8]) -> String {
    let Some(args) = n.child_by_field_name("arguments") else {
        return String::new();
    };
    for i in 0..args.child_count() {
        let Some(a) = args.child(i) else {
            continue;
        };
        if a.kind() != "string" {
            continue;
        }
        for j in 0..a.child_count() {
            if let Some(c) = a.child(j)
                && c.kind() == "string_content"
            {
                return node_text(c, src).into_owned();
            }
        }
        // Empty string literal.
        return String::new();
    }
    String::new()
}

/// Go `rubyRecordCall`: a `call` node's callee. Both bare calls (`f(...)`) and method calls
/// (`x.f(...)`) record the method name from the `method` field.
fn ruby_record_call(b: &mut Builder, c: Node<'_>, caller_id: &str, src: &[u8]) {
    let Some(method) = c.child_by_field_name("method") else {
        return;
    };
    if matches!(method.kind(), "identifier" | "constant") {
        let recv = recv_text(Some(c), Some(method), src);
        b.call_recv(caller_id, &node_text(method, src), &recv, line(c));
    }
}

#[cfg(test)]
#[path = "tests/ruby.rs"]
mod ruby_tests;

#[cfg(test)]
#[path = "tests/rake_resolve.rs"]
mod rake_resolve_tests;

#[cfg(test)]
#[path = "tests/rake_skill.rs"]
mod rake_skill_tests;
