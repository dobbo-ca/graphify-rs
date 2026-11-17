//! Port of `golang.go`. Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.24.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, OnceLock};

use gositter::Node;

use super::builder::{
    Builder, bind_type, line, node_text, parse_root, recv_text, walk, walk_kinds,
};
use super::extract::Result;
use super::lua::first_named;
use crate::model::Loc;

/// Go `goTypeKinds`, set by `register_kinds`.
static GO_TYPE_KINDS: OnceLock<u32> = OnceLock::new();
/// Go `goCallKinds`, set by `register_kinds`.
static GO_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `goPredeclared` holds the predeclared types, which are no corpus symbols.
static GO_PREDECLARED: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    super::python::set(&[
        "bool",
        "byte",
        "complex64",
        "complex128",
        "error",
        "float32",
        "float64",
        "int",
        "int8",
        "int16",
        "int32",
        "int64",
        "rune",
        "string",
        "uint",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "uintptr",
        "any",
        "comparable",
    ])
});

/// Go `extractGo` pulls functions, methods, type declarations, imports, and call
/// edges out of a .go file. `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_go(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::golang::language(), |root| {
        let mut b = Builder::new(rel);

        // Single named return type of each function in this file.
        let mut rets: HashMap<String, String> = HashMap::new();
        for i in 0..root.child_count() {
            if let Some(n) = root.child(i)
                && n.kind() == "function_declaration"
            {
                rets.insert(
                    field_text(n, "name", src),
                    go_type_name(n.child_by_field_name("result"), src),
                );
            }
        }

        // Go `b.goPkgs`: per-file state, kept here rather than on `Builder`.
        let pkgs = go_pkg_paths(root, src);
        for i in 0..root.child_count() {
            let Some(n) = root.child(i) else { continue };
            match n.kind() {
                "function_declaration" => b.go_func(n, src, &rets, &pkgs),
                "method_declaration" => b.go_method(n, src, &rets, &pkgs),
                "type_declaration" => b.go_types(n, src, &pkgs),
                "import_declaration" => b.go_imports(n, src),
                _ => {}
            }
        }
        b.res
    })
}

impl Builder {
    /// Go `goFunc`.
    fn go_func(
        &mut self,
        n: Node<'_>,
        src: &[u8],
        rets: &HashMap<String, String>,
        pkgs: &HashMap<String, String>,
    ) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let id = self.id(&[&name]);
        self.def(&id, &name, &format!("{name}()"), line(n));
        let skip = go_type_params(n.child_by_field_name("type_parameters"), "identifier", src);
        self.go_signature(&id, n, &skip, src, pkgs);
        self.go_calls(n, &id, "", src, rets);
    }

    /// Go `goMethod`.
    fn go_method(
        &mut self,
        n: Node<'_>,
        src: &[u8],
        rets: &HashMap<String, String>,
        pkgs: &HashMap<String, String>,
    ) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let recv = go_receiver_type(n, src);
        let id = self.id(&[&recv, &name]);
        let label = if recv.is_empty() {
            format!("{name}()")
        } else {
            format!("{recv}.{name}()")
        };
        // Register under the bare method name so `x.Method()` call sites resolve.
        self.def(&id, &name, &label, line(n));
        if let Some(d) = self.res.defs.last_mut() {
            d.owner = recv;
        }
        let receiver = n.child_by_field_name("receiver");
        let self_name = match receiver.and_then(first_named) {
            Some(p) => field_text(p, "name", src),
            None => String::new(),
        };
        // `func (g *Gen[T]) M()`: T is the receiver's type parameter.
        let mut skip = HashSet::new();
        walk(receiver, |cv| {
            if cv.kind() == "type_arguments" {
                skip = go_type_params(Some(cv), "type_identifier", src);
            }
            true
        });
        self.go_signature(&id, n, &skip, src, pkgs);
        self.go_calls(n, &id, &self_name, src, rets);
    }

    /// Go `goTypes`.
    fn go_types(&mut self, n: Node<'_>, src: &[u8], pkgs: &HashMap<String, String>) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            if c.kind() != "type_spec" {
                continue;
            }
            let name = field_text(c, "name", src);
            if name.is_empty() {
                continue;
            }
            let id = self.id(&[&name]);
            self.def(&id, &name, &name, line(c));
            let skip = go_type_params(c.child_by_field_name("type_parameters"), "identifier", src);
            let Some(t) = c.child_by_field_name("type") else {
                continue;
            };
            match t.kind() {
                "struct_type" => {
                    walk(first_named(t), |f| {
                        if f.kind() != "field_declaration" {
                            return f.kind() == "field_declaration_list";
                        }
                        // An unnamed field embeds its type (graphify-go-vi5.11) and
                        // references only that type's arguments.
                        if f.child_by_field_name("name").is_some() {
                            self.go_refs(
                                &id,
                                f.child_by_field_name("type"),
                                &skip,
                                src,
                                line(f),
                                pkgs,
                            );
                        } else {
                            self.go_type_args(&id, f, &skip, src, pkgs);
                        }
                        false
                    });
                }
                "interface_type" => {
                    // `~int | Early` and `~Early` are type sets; a lone type embeds.
                    for j in 0..t.named_child_count() {
                        let Some(e) = t.named_child(j) else { continue };
                        if e.kind() != "type_elem" {
                            continue;
                        }
                        if e.named_child_count() > 1
                            || e.named_child(0).is_some_and(|c| c.kind() == "negated_type")
                        {
                            self.go_refs(&id, Some(e), &skip, src, line(e), pkgs);
                        } else {
                            self.go_type_args(&id, e, &skip, src, pkgs);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Go `goTypeArgs` records the types in the type arguments under n.
    fn go_type_args(
        &mut self,
        id: &str,
        n: Node<'_>,
        skip: &HashSet<String>,
        src: &[u8],
        pkgs: &HashMap<String, String>,
    ) {
        walk(Some(n), |cv| {
            if cv.kind() != "type_arguments" {
                return true;
            }
            self.go_refs(id, Some(cv), skip, src, line(n), pkgs);
            false
        });
    }

    /// Go `goSignature` records the types the parameters and results of fn name.
    fn go_signature(
        &mut self,
        id: &str,
        func: Node<'_>,
        skip: &HashSet<String>,
        src: &[u8],
        pkgs: &HashMap<String, String>,
    ) {
        self.go_refs(
            id,
            func.child_by_field_name("parameters"),
            skip,
            src,
            line(func),
            pkgs,
        );
        self.go_refs(
            id,
            func.child_by_field_name("result"),
            skip,
            src,
            line(func),
            pkgs,
        );
    }

    /// Go `goRefs` records a references edge from id to each type named under n, but
    /// for the predeclared types and the type parameters in skip. `pkg.T` carries
    /// the import path of pkg as its qualifier.
    fn go_refs(
        &mut self,
        id: &str,
        n: Option<Node<'_>>,
        skip: &HashSet<String>,
        src: &[u8],
        loc: Loc,
        pkgs: &HashMap<String, String>,
    ) {
        let mask = *GO_TYPE_KINDS.get().expect("register_kinds");
        walk_kinds(n, mask, |c| {
            if c.kind() == "qualified_type" {
                let mut pkg = field_text(c, "package", src);
                let before = self.res.type_refs.len();
                self.ref_(id, &field_text(c, "name", src), "references", false, loc);
                if self.res.type_refs.len() > before && !pkg.is_empty() {
                    if let Some(p) = pkgs.get(&pkg).filter(|p| !p.is_empty()) {
                        pkg = p.clone();
                    }
                    self.res.type_refs[before].qual = pkg;
                }
                return false;
            }
            let name = node_text(c, src);
            if !GO_PREDECLARED.contains(&*name) && !skip.contains(&*name) {
                self.ref_(id, &name, "references", false, loc);
            }
            true
        });
    }

    /// Go `goImports`.
    fn go_imports(&mut self, n: Node<'_>, src: &[u8]) {
        walk(Some(n), |c| {
            if (c.kind() == "import_spec"
                || (c.kind() == "import_declaration" && c.child_by_field_name("path").is_some()))
                && let Some(path) = c.child_by_field_name("path")
            {
                self.imp(&unquote(&node_text(path, src)), line(c));
            }
            true
        });
    }

    /// Go `goCalls` records each call site in the body of function fn, attributing it to
    /// callerID. Both direct calls (`f()`) and method/selector calls (`x.f()`) are
    /// recorded by the called name. self is the enclosing method's receiver name; a
    /// call on it is recorded with the receiver "self". A call on a typed local is
    /// recorded with its type as the receiver.
    ///
    /// The same walk maps the locals of fn to their type, from a parameter
    /// or `var` (`x Foo`, `x *Foo`, `x pkg.Foo`), a composite literal (`x := Foo{}`,
    /// `x := &Foo{}`) or a call to a function in rets (`x := newFoo()`). A local also
    /// declared with anything else, range and type switch included, maps to "".
    // ponytail: one flat scope per function, and rets only knows this file, so
    // `x := pkg.New()` is untyped; that needs return types across files.
    fn go_calls(
        &mut self,
        func: Node<'_>,
        caller_id: &str,
        self_name: &str,
        src: &[u8],
        rets: &HashMap<String, String>,
    ) {
        // Calls count only inside the body, which is the last child of fn.
        let body_start = func.child_by_field_name("body").map(|b| b.start_byte());
        // The scope is flat, so a receiver is typed only once every local is seen.
        struct Site {
            callee: String,
            recv: String,
            loc: Loc,
            member: bool,
        }
        let mut sites: Vec<Site> = Vec::new();
        let mut types: HashMap<String, String> = HashMap::new();
        let mask = *GO_CALL_KINDS.get().expect("register_kinds");
        walk_kinds(Some(func), mask, |c| {
            match c.kind() {
                "call_expression" => {
                    let Some(f) = c.child_by_field_name("function") else {
                        return true;
                    };
                    if body_start.is_none_or(|s| c.start_byte() < s) {
                        return true;
                    }
                    match f.kind() {
                        "identifier" => sites.push(Site {
                            callee: node_text(f, src).into_owned(),
                            recv: String::new(),
                            loc: line(c),
                            member: false,
                        }),
                        "selector_expression" => {
                            if let Some(name) = f.child_by_field_name("field") {
                                sites.push(Site {
                                    callee: node_text(name, src).into_owned(),
                                    recv: recv_text(Some(f), Some(name), src),
                                    loc: line(c),
                                    member: true,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                "parameter_declaration" | "var_spec" => {
                    let typ = go_type_name(c.child_by_field_name("type"), src);
                    for i in 0..c.named_child_count() {
                        if let Some(p) = c.named_child(i)
                            && p.kind() == "identifier"
                        {
                            bind_type(&mut types, &node_text(p, src), &typ);
                        }
                    }
                }
                "range_clause" | "type_switch_statement" => {
                    // A range or type-switch variable shadows a typed local.
                    let l = if c.kind() == "type_switch_statement" {
                        c.child_by_field_name("alias")
                    } else {
                        c.child_by_field_name("left")
                    };
                    if let Some(l) = l {
                        for i in 0..l.named_child_count() {
                            if let Some(v) = l.named_child(i)
                                && v.kind() == "identifier"
                            {
                                bind_type(&mut types, &node_text(v, src), "");
                            }
                        }
                    }
                }
                "short_var_declaration" => {
                    let (Some(l), Some(r)) = (
                        c.child_by_field_name("left"),
                        c.child_by_field_name("right"),
                    ) else {
                        return true;
                    };
                    for i in 0..l.named_child_count() {
                        let Some(v) = l.named_child(i) else { continue };
                        if v.kind() != "identifier" {
                            continue;
                        }
                        let mut typ = String::new();
                        if l.named_child_count() == r.named_child_count() {
                            let mut e = r.named_child(i);
                            if let Some(u) = e
                                && u.kind() == "unary_expression"
                                && field_text(u, "operator", src) == "&"
                            {
                                e = u.child_by_field_name("operand");
                            }
                            match e {
                                Some(e) if e.kind() == "composite_literal" => {
                                    typ = go_type_name(e.child_by_field_name("type"), src);
                                }
                                Some(e) if e.kind() == "call_expression" => {
                                    typ = rets
                                        .get(&field_text(e, "function", src))
                                        .cloned()
                                        .unwrap_or_default();
                                }
                                _ => {}
                            }
                        }
                        bind_type(&mut types, &node_text(v, src), &typ);
                    }
                }
                _ => {}
            }
            true
        });
        for c in sites {
            if !c.member {
                self.call(caller_id, &c.callee, c.loc);
                continue;
            }
            let mut recv = c.recv;
            if recv == self_name {
                recv = "self".to_string();
            } else if let Some(t) = types.get(&recv).filter(|t| !t.is_empty()) {
                recv = t.clone();
            }
            self.call_recv(caller_id, &c.callee, &recv, c.loc);
        }
    }
}

/// Go `goReceiverType` returns the bare type name of a method receiver, e.g. "Server"
/// for `func (s *Server) Handle()`.
fn go_receiver_type(n: Node<'_>, src: &[u8]) -> String {
    let mut name = String::new();
    walk(n.child_by_field_name("receiver"), |c| {
        if !name.is_empty() {
            return false;
        }
        if c.kind() == "type_identifier" {
            name = node_text(c, src).into_owned();
            return false;
        }
        true
    });
    name
}

/// Go `goTypeParams` returns the names of kind under a type parameter list or a
/// receiver's type arguments.
fn go_type_params(n: Option<Node<'_>>, kind: &str, src: &[u8]) -> HashSet<String> {
    let mut out = HashSet::new();
    walk(n, |cv| {
        if cv.kind() == kind {
            out.insert(node_text(cv, src).into_owned());
        }
        true
    });
    out
}

/// Go `goPkgPaths` maps the name each import binds in the file to its import path.
fn go_pkg_paths(root: Node<'_>, src: &[u8]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for i in 0..root.child_count() {
        let Some(d) = root.child(i) else { continue };
        if d.kind() != "import_declaration" {
            continue;
        }
        walk(Some(d), |c| {
            if c.kind() != "import_spec" {
                return true;
            }
            let p = unquote(&field_text(c, "path", src));
            let mut name = field_text(c, "name", src);
            if name.is_empty() {
                // The package of example.com/m/v2 is m by convention.
                let seg: Vec<&str> = p.split('/').collect();
                name = seg[seg.len() - 1].to_string();
                if seg.len() > 1
                    && name.len() > 1
                    && name.as_bytes()[0] == b'v'
                    && name[1..]
                        .trim_matches(|c: char| c.is_ascii_digit())
                        .is_empty()
                {
                    name = seg[seg.len() - 2].to_string();
                }
            }
            out.insert(name, p);
            false
        });
    }
    out
}

/// Go `goTypeName` returns the name of a plain or pointer named type, else "". A
/// package-qualified type yields its bare name, which is what a method's owner
/// is keyed by.
// ponytail: the package is dropped, so `r *http.Request` can bind to a corpus
// Request when it is the only one with that method; carry the package in Recv
// if that shows up.
fn go_type_name(t: Option<Node<'_>>, src: &[u8]) -> String {
    let mut t = t;
    if let Some(p) = t
        && p.kind() == "pointer_type"
    {
        t = p.named_child(0);
    }
    if let Some(q) = t
        && q.kind() == "qualified_type"
    {
        t = q.child_by_field_name("name");
    }
    match t {
        Some(t) if t.kind() == "type_identifier" => node_text(t, src).into_owned(),
        _ => String::new(),
    }
}

// Go fieldText (golang.go:388)
pub(crate) fn field_text(n: Node<'_>, field: &str, src: &[u8]) -> String {
    match n.child_by_field_name(field) {
        Some(c) => node_text(c, src).into_owned(),
        None => String::new(),
    }
}

// Go unquote (golang.go:394)
pub(crate) fn unquote(s: &str) -> String {
    s.trim_matches(['`', '"', '\'']).to_string()
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let lang = gositter_grammars::golang::language();
    let _ = GO_TYPE_KINDS.set(lang.kind_mask(&["type_identifier", "qualified_type"]));
    let _ = GO_CALL_KINDS.set(lang.kind_mask(&[
        "call_expression",
        "parameter_declaration",
        "var_spec",
        "range_clause",
        "type_switch_statement",
        "short_var_declaration",
    ]));
}

#[cfg(test)]
#[path = "tests/gocalls.rs"]
mod gocalls_tests;
