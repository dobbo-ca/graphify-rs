//! Port of `verilog.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.41.

use std::sync::OnceLock;

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, walk, walk_kinds};
use super::extract::{Def, Result};
use crate::model::{Confidence, Edge, Relation};

/// Go `verilogCallKinds`, set by `register_kinds`.
static VERILOG_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `extractVerilog` pulls modules/programs/interfaces/packages (as types), classes
/// (+ methods), top-level functions/tasks, `include` directives (as imports), and
/// call edges out of a Verilog / SystemVerilog file. The grammar exposes no field
/// names, so names are read from the first identifier descendant of each
/// declaration; calls are recorded by the called name and resolved by Resolve.
/// `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_verilog(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::verilog::language(), |root| {
        let mut b = Builder::new(rel);

        b.verilog_items(root, src);
        b.res
    })
}

impl Builder {
    /// Go `verilogItems` handles each item directly under n (a source_file, a package
    /// body, or a wrapper node), unwrapping package/item wrappers so the declarations
    /// they hold still surface.
    fn verilog_items(&mut self, n: Node<'_>, src: &[u8]) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            match c.kind() {
                "module_declaration" | "program_declaration" | "interface_declaration" => {
                    self.verilog_module(c, src)
                }
                "package_declaration" => self.verilog_package(c, src),
                "class_declaration" => self.verilog_class(c, src),
                "function_declaration" | "task_declaration" => self.verilog_subroutine(c, src),
                "include_compiler_directive" => self.verilog_include(c, src),
                // Top-level functions/tasks/classes are wrapped in this node.
                "package_or_generate_item_declaration" | "package_export_declaration" => {
                    self.verilog_items(c, src)
                }
                _ => {}
            }
        }
    }

    /// Go `verilogModule` records a module/program/interface as a type definition. Its
    /// name is the first simple_identifier in its header. Nested functions/tasks and
    /// call sites inside the body are recorded under the module.
    fn verilog_module(&mut self, n: Node<'_>, src: &[u8]) {
        let name = header_name(n, src);
        if name.is_empty() {
            return;
        }
        let id = self.id(&[&name]);
        self.def(&id, &name, &name, line(n));
        self.verilog_calls(Some(n), &id, src);
    }

    /// Go `verilogPackage` records a package as a type definition (its name is the first
    /// package_identifier) and descends into its items so packaged functions, tasks
    /// and classes surface.
    fn verilog_package(&mut self, n: Node<'_>, src: &[u8]) {
        let name = first_ident_in(n, "package_identifier", src);
        if name.is_empty() {
            return;
        }
        let id = self.id(&[&name]);
        self.def(&id, &name, &name, line(n));
        self.verilog_items(n, src);
    }

    /// Go `verilogClass` records a class as a type definition and its function/task
    /// methods scoped under the class name, each with a contains edge.
    fn verilog_class(&mut self, n: Node<'_>, src: &[u8]) {
        let name = first_ident_in(n, "class_identifier", src);
        if name.is_empty() {
            return;
        }
        let class_id = self.id(&[&name]);
        self.def(&class_id, &name, &name, line(n));

        for i in 0..n.child_count() {
            let Some(item) = n.child(i) else { continue };
            if item.kind() != "class_item" {
                continue;
            }
            for j in 0..item.child_count() {
                let Some(cm) = item.child(j) else { continue };
                if cm.kind() != "class_method" {
                    continue;
                }
                self.verilog_method(cm, &name, &class_id, src);
            }
        }
    }

    /// Go `verilogMethod` records a single function/task method of a class.
    fn verilog_method(&mut self, cm: Node<'_>, type_name: &str, class_id: &str, src: &[u8]) {
        for k in 0..cm.child_count() {
            let Some(sub) = cm.child(k) else { continue };
            if sub.kind() != "function_declaration" && sub.kind() != "task_declaration" {
                continue;
            }
            let mname = subroutine_name(sub, src);
            if mname.is_empty() {
                continue;
            }
            let mid = self.id(&[type_name, &mname]);
            self.add_node(&mid, &format!("{type_name}.{mname}()"), line(sub));
            self.res.edges.push(Edge {
                source: class_id.to_string(),
                target: mid.clone(),
                relation: Relation::CONTAINS,
                confidence: Confidence::EXTRACTED,
                source_file: self.src,
                source_location: line(sub),
                ..Edge::default()
            });
            self.res.defs.push(Def {
                id: mid.clone(),
                name: mname,
                file: self.file.clone(),
                ..Def::default()
            });
            self.verilog_calls(Some(sub), &mid, src);
        }
    }

    /// Go `verilogSubroutine` records a top-level function/task as a definition and its
    /// internal call sites.
    fn verilog_subroutine(&mut self, n: Node<'_>, src: &[u8]) {
        let name = subroutine_name(n, src);
        if name.is_empty() {
            return;
        }
        let id = self.id(&[&name]);
        self.def(&id, &name, &format!("{name}()"), line(n));
        self.verilog_calls(Some(n), &id, src);
    }

    /// Go `verilogInclude` records the path of an `include "x.svh"` directive as an
    /// import. The quotes are stripped so the spec is a plain path (an external
    /// dependency, since it is not a relative ./ path).
    fn verilog_include(&mut self, n: Node<'_>, src: &[u8]) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            if c.kind() == "double_quoted_string" {
                let spec = node_text(c, src).trim_matches('"').to_string();
                self.imp(&spec, line(n));
                return;
            }
        }
    }

    /// Go `verilogCalls` walks a declaration body and records each user call site. Direct
    /// task/function calls (`f(...)`) come from tf_call; method calls (`x.f(...)`)
    /// come from method_call. System tasks (`$display`) are skipped as builtins.
    fn verilog_calls(&mut self, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
        if body.is_none() {
            return;
        }
        walk_kinds(
            body,
            *VERILOG_CALL_KINDS.get().expect("register_kinds"),
            |c| {
                match c.kind() {
                    "tf_call" => {
                        let name = first_child_ident(c, src);
                        if !name.is_empty() {
                            self.call(caller_id, &name, line(c));
                        }
                    }
                    "method_call" => {
                        if let Some(mb) = child_of_kind(c, "method_call_body")
                            && let Some(mi) = child_of_kind(mb, "method_identifier")
                        {
                            self.call(caller_id, &first_simple_ident(Some(mi), src), line(c));
                        }
                    }
                    _ => {}
                }
                true
            },
        );
    }
}

/// Go `headerName` returns the first simple_identifier inside the *_header child of a
/// module/program/interface declaration (its declared name).
fn header_name(n: Node<'_>, src: &[u8]) -> String {
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if c.kind().ends_with("_header") {
            return first_simple_ident(Some(c), src);
        }
    }
    String::new()
}

/// Go `subroutineName` returns the declared name of a function/task by digging into
/// its *_identifier node (function_identifier / task_identifier), skipping any
/// leading return-type identifier.
fn subroutine_name(n: Node<'_>, src: &[u8]) -> String {
    let mut name = String::new();
    walk(Some(n), |c| {
        if !name.is_empty() {
            return false;
        }
        if c.kind() == "function_identifier" || c.kind() == "task_identifier" {
            name = first_simple_ident(Some(c), src);
            return false;
        }
        true
    });
    name
}

/// Go `firstIdentIn` returns the simple_identifier text of the first descendant of n
/// whose kind is wrapperKind (e.g. package_identifier, class_identifier).
fn first_ident_in(n: Node<'_>, wrapper_kind: &str, src: &[u8]) -> String {
    let mut name = String::new();
    walk(Some(n), |c| {
        if !name.is_empty() {
            return false;
        }
        if c.kind() == wrapper_kind {
            name = first_simple_ident(Some(c), src);
            return false;
        }
        true
    });
    name
}

/// Go `firstSimpleIdent` returns the text of the first simple_identifier at or below n.
fn first_simple_ident(n: Option<Node<'_>>, src: &[u8]) -> String {
    if n.is_none() {
        return String::new();
    }
    let mut name = String::new();
    walk(n, |c| {
        if !name.is_empty() {
            return false;
        }
        if c.kind() == "simple_identifier" {
            name = node_text(c, src).into_owned();
            return false;
        }
        true
    });
    name
}

/// Go `firstChildIdent` returns the text of the first direct simple_identifier child
/// of n (the callee name of a tf_call, before its argument list).
fn first_child_ident(n: Node<'_>, src: &[u8]) -> String {
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if c.kind() == "simple_identifier" {
            return node_text(c, src).into_owned();
        }
    }
    String::new()
}

/// Go `childOfKind` returns the first direct child of n with the given kind, or None.
fn child_of_kind<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    for i in 0..n.child_count() {
        if let Some(c) = n.child(i)
            && c.kind() == kind
        {
            return Some(c);
        }
    }
    None
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let mask = gositter_grammars::verilog::language().kind_mask(&["tf_call", "method_call"]);
    let _ = VERILOG_CALL_KINDS.set(mask);
}

#[cfg(test)]
#[path = "tests/verilog.rs"]
mod verilog_tests;
