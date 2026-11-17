//! Port of `csharp.go`: top-level functions, types (class/struct/interface/enum/record and their
//! methods), `using` imports and call edges out of a .cs file. Declarations nested inside
//! (file-scoped) namespaces are descended into so namespaced source still surfaces its
//! definitions.
//!
//! Go keeps `csNested`, `csFields`, `csSupers`, `csOwnNested`, `csGen` and `csDecls` on its
//! builder; here they sit beside a [`Builder`] in [`Ex`].

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use gositter::Node;
use gostd::unicode::is_upper;

use super::builder::{Builder, line, node_text, parse_root, walk_kinds};
use super::extract::{Def, Result};
use super::golang::field_text;
use crate::model::{Confidence, Edge, Loc, Relation};

/// Go `csCallKinds`, named by [`register_kinds`].
static CALL_KINDS: OnceLock<u32> = OnceLock::new();
/// Go `csInterfaceKinds`, named by [`register_kinds`].
static INTERFACE_KINDS: OnceLock<u32> = OnceLock::new();

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`):
/// `csCallKinds` (`csharp.go:433`), then `csInterfaceKinds` (`csharp.go:956`).
pub(super) fn register_kinds() {
    let lang = gositter_grammars::csharp::language();
    let _ =
        CALL_KINDS.set(lang.kind_mask(&["invocation_expression", "object_creation_expression"]));
    let _ = INTERFACE_KINDS.set(lang.kind_mask(&["interface_declaration"]));
}

/// Go `extractCSharp`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_csharp(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::csharp::language(), |root| {
        let mut ex = Ex {
            b: Builder::new(rel),
            src,
            nested: HashSet::new(),
            fields: HashMap::new(),
            supers: HashMap::new(),
            own_nested: HashSet::new(),
            gens: Vec::new(),
            decls: Vec::new(),
        };
        let ifaces = cs_interface_names(root, src);
        ex.cs_collect(root, &ifaces);
        // A nested namesake shares this node upstream.
        for key in ex.fields.keys() {
            let name = key.split_once('`').map_or(key.as_str(), |(n, _)| n);
            ex.nested.remove(name);
        }
        ex.cs_items(root, &ifaces, String::new(), Vec::new());
        // A later ref's stub hides it upstream.
        let res = &mut ex.b.res;
        for g in &ex.gens {
            let id = cut_before(&res.cs_types[g.entry], " ").to_string();
            let is_ref = |t: &super::extract::TypeRef| {
                t.name == g.name && t.relation == "references" && !t.arg
            };
            let prefix = format!("{id} ");
            let plain = res
                .cs_types
                .iter()
                .any(|e| e.starts_with(&prefix) && !e.contains(" `"));
            if !plain
                && !res.type_refs[..g.refs].iter().any(is_ref)
                && res.type_refs[g.refs..].iter().any(is_ref)
            {
                res.cs_types[g.entry].push_str(" ~");
            }
        }
        ex.cs_twins();
        ex.b.res
    })
}

/// Go `strings.Cut(s, sep)`'s first result: `s` before the first `sep`, all of `s` without one.
fn cut_before<'s>(s: &'s str, sep: &str) -> &'s str {
    s.split_once(sep).map_or(s, |(a, _)| a)
}

/// The children of `n`.
fn kids<'t>(n: Node<'t>) -> impl Iterator<Item = Node<'t>> {
    (0..n.child_count()).filter_map(move |i| n.child(i))
}

/// The named children of `n`.
fn named<'t>(n: Node<'t>) -> impl Iterator<Item = Node<'t>> {
    (0..n.named_child_count()).filter_map(move |i| n.named_child(i))
}

/// The five type declaration kinds `csharp.go` handles alike.
fn is_type_decl(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
    )
}

/// Go `csGen`: a generic type, its `CsTypes` entry and the `TypeRefs` made before it.
struct CsGen {
    entry: usize,
    refs: usize,
    name: String,
}

/// Go `csDecl`: a type declaration, its `CsTypes` entry and line, its own methods and bases.
#[derive(Clone)]
struct CsDecl {
    entry: usize,
    loc: String,
    methods: Vec<String>,
    bases: Vec<String>,
}

/// Go `builder` plus the fields `csharp.go` adds to it.
struct Ex<'a> {
    b: Builder,
    src: &'a [u8],
    /// Go `csNested`: the names of the types nested in a type of the file.
    nested: HashSet<String>,
    /// Go `csFields`: the field table of each top-level type, keyed by `cs_arity_key`.
    fields: HashMap<String, HashMap<String, String>>,
    /// Go `csSupers`: the inherits bases of each top-level type, keyed by `cs_arity_key`.
    supers: HashMap<String, Vec<String>>,
    /// Go `csOwnNested`: the nested types of the type whose methods are walked.
    own_nested: HashSet<String>,
    /// Go `csGen`.
    gens: Vec<CsGen>,
    /// Go `csDecls`.
    decls: Vec<CsDecl>,
}

impl Ex<'_> {
    /// Go `csTwins`: marks each of the types of one name and different arity in this file as a
    /// twin (`*`, `*!` for the one upstream keeps: first by "L<line>" text, then by id) with its
    /// own methods (`.M`) and bases (`:B`), and drops their fields: upstream binds no field table
    /// to a label twice in a file.
    fn cs_twins(&mut self) {
        let cs_types = &mut self.b.res.cs_types;
        let mut by_id: HashMap<String, Vec<CsDecl>> = HashMap::new();
        let mut ids: Vec<String> = Vec::new();
        for d in &self.decls {
            let id = cut_before(&cs_types[d.entry], " ").to_string();
            let group = by_id.entry(id.clone()).or_default();
            if group.is_empty() {
                ids.push(id);
            }
            group.push(d.clone());
        }
        let arity = |cs_types: &[String], d: &CsDecl| -> String {
            let a = cs_types[d.entry].split_once(" `").map_or("", |(_, a)| a);
            cut_before(a, " ").to_string()
        };
        for id in &ids {
            let ds = &by_id[id];
            let view: &[String] = cs_types;
            let seen: HashSet<String> = ds.iter().map(|d| arity(view, d)).collect();
            if ds.len() < 2 || seen.len() < ds.len() {
                // Partial halves merge upstream.
                continue;
            }
            // Ids sort the bare name before name_1. Go `slices.MinFunc`: the first least one.
            let mut kept = &ds[0];
            for d in &ds[1..] {
                let ord = d
                    .loc
                    .cmp(&kept.loc)
                    .then_with(|| arity(view, d).cmp(&arity(view, kept)));
                if ord.is_lt() {
                    kept = d;
                }
            }
            let kept_entry = kept.entry;
            for d in ds {
                // Go drops the field codes: non-empty, not an alias (`&`), holding `=`.
                let mut codes: Vec<String> = cs_types[d.entry]
                    .split(' ')
                    .filter(|f| f.is_empty() || f.starts_with('&') || !f.contains('='))
                    .map(str::to_string)
                    .collect();
                codes.push(if d.entry == kept_entry { "*!" } else { "*" }.to_string());
                for m in &d.methods {
                    codes.push(format!(".{m}"));
                }
                for s in &d.bases {
                    codes.push(format!(":{}", s.replace(' ', "\t")));
                }
                cs_types[d.entry] = codes.join(" ");
            }
        }
    }

    /// Go `csCollect`: records the nested type names and the field table and inherits bases of
    /// each top-level type, before any call is typed: a base may come later in the file.
    fn cs_collect(&mut self, n: Node<'_>, ifaces: &HashSet<String>) {
        let src = self.src;
        for c in kids(n) {
            match c.kind() {
                k if is_type_decl(k) => {
                    cs_nested_types(c, src, &mut self.nested);
                    let tps = cs_type_params(c, &HashSet::new(), src);
                    let name = cs_arity_key(&field_text(c, "name", src), tps.len());
                    let table = self.fields.entry(name.clone()).or_default();
                    for l in kids(c) {
                        if l.kind() != "parameter_list" {
                            continue;
                        }
                        // A primary constructor's parameters are fields.
                        if c.kind() == "interface_declaration" {
                            continue;
                        }
                        for p in named(l) {
                            let t = cs_recv_type(p.child_by_field_name("type"), src);
                            let pn = field_text(p, "name", src);
                            if p.kind() == "parameter"
                                && !t.is_empty()
                                && !tps.contains(&t)
                                && !pn.is_empty()
                            {
                                table.insert(pn, t);
                            }
                        }
                    }
                    for bn in cs_base_types(c) {
                        let base = cs_base_name(Some(bn), src);
                        if !tps.contains(&base)
                            && (c.kind() == "interface_declaration"
                                || !cs_is_interface_name(&base, ifaces))
                        {
                            let mut an = bn;
                            if bn.kind() == "qualified_name"
                                && let Some(q) = bn.child_by_field_name("name")
                            {
                                an = q;
                            }
                            self.supers
                                .entry(name.clone())
                                .or_default()
                                .push(cs_arity_key(&base, cs_type_arg_list(an).len()));
                        }
                    }
                    if let Some(body) = c.child_by_field_name("body") {
                        let table = self.fields.entry(name).or_default();
                        for m in kids(body) {
                            cs_field(table, m, &tps, src);
                        }
                    }
                }
                "namespace_declaration" => {
                    if let Some(body) = c.child_by_field_name("body") {
                        self.cs_collect(body, ifaces);
                    }
                }
                "file_scoped_namespace_declaration" => self.cs_collect(c, ifaces),
                _ => {}
            }
        }
    }

    /// Go `csFieldsUp`: merges the field tables of type `name` and its bases in this file,
    /// nearest first.
    fn cs_fields_up(&self, name: &str) -> HashMap<String, String> {
        let mut merged: HashMap<String, String> = HashMap::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: Vec<String> = vec![name.to_string()];
        let mut i = 0;
        while i < queue.len() {
            let q = queue[i].clone();
            i += 1;
            if !seen.insert(q.clone()) {
                continue;
            }
            if let Some(t) = self.fields.get(&q) {
                for (k, v) in t {
                    merged.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            if let Some(s) = self.supers.get(&q) {
                queue.extend(s.iter().cloned());
            }
        }
        merged
    }

    /// Go `csItems`: handles each declaration directly under `n` (a compilation_unit, a
    /// namespace's declaration_list, or a file_scoped_namespace_declaration), recursing into
    /// nested namespaces. `uses` are the usings in scope.
    fn cs_items(
        &mut self,
        n: Node<'_>,
        ifaces: &HashSet<String>,
        mut ns: String,
        mut uses: Vec<String>,
    ) {
        let src = self.src;
        for mut c in kids(n) {
            // Top-level statements wrap their inner statement in a global_statement.
            if c.kind() == "global_statement"
                && c.named_child_count() > 0
                && let Some(inner) = c.named_child(0)
            {
                c = inner;
            }
            match c.kind() {
                k if is_type_decl(k) => self.cs_type(c, ifaces, &ns, &uses),
                "local_function_statement" => self.cs_func(c),
                "using_directive" => {
                    self.cs_using(c);
                    let u = cs_use_code(&node_text(c, src));
                    if !u.is_empty() {
                        uses.push(u);
                    }
                }
                "namespace_declaration" => {
                    if let Some(body) = c.child_by_field_name("body") {
                        self.cs_items(body, ifaces, cs_namespace(&ns, c, src), uses.clone());
                    }
                }
                "file_scoped_namespace_declaration" => {
                    // Members follow as siblings, not children.
                    ns = cs_namespace(&ns, c, src);
                    self.cs_items(c, ifaces, ns.clone(), uses.clone());
                }
                _ => {}
            }
        }
    }

    /// Go `csFunc`: records a top-level (local) function and its call sites.
    fn cs_func(&mut self, n: Node<'_>) {
        let src = self.src;
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &format!("{name}()"), line(n));
        self.own_nested = HashSet::new();
        let tps = cs_type_params(n, &HashSet::new(), src);
        self.cs_calls(n, &id, &HashMap::new(), &tps);
    }

    /// Go `csType`: records a type definition and the methods declared in its body, each scoped
    /// under the type's name.
    fn cs_type(&mut self, n: Node<'_>, ifaces: &HashSet<String>, ns: &str, uses: &[String]) {
        let src = self.src;
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let type_id = self.b.id(&[&name]);
        self.b.def(&type_id, &name, &name, line(n));
        let tps = cs_type_params(n, &HashSet::new(), src);
        let key = cs_arity_key(&name, tps.len());
        let fields = self.cs_fields_up(&key);
        let mut nested = HashSet::new();
        cs_nested_types(n, src, &mut nested);
        let skip = cs_type_params(n, &nested, src);
        nested.retain(|k| self.nested.contains(k));
        self.own_nested = nested.clone();
        let mut entry = format!("{type_id} {ns}");
        let empty = HashMap::new();
        let own = self.fields.get(&key).unwrap_or(&empty);
        let mut keys: Vec<String> = own
            .iter()
            .filter(|(k, v)| !k.ends_with("{}") && !nested.contains(*v))
            .map(|(k, _)| k.clone())
            .collect();
        keys.extend(nested.iter().map(|k| format!("^{k}")));
        keys.sort();
        for k in &keys {
            if k.starts_with('^') {
                entry.push(' ');
                entry.push_str(k);
            } else {
                entry.push_str(&format!(" {k}={}", own[k]));
            }
        }
        for u in uses {
            entry.push(' ');
            entry.push_str(u);
        }
        if !tps.is_empty() {
            entry.push_str(&format!(" `{}", tps.len()));
            self.gens.push(CsGen {
                entry: self.b.res.cs_types.len(),
                refs: self.b.res.type_refs.len(),
                name: name.clone(),
            });
        }
        self.b.res.cs_types.push(entry);
        let di = self.decls.len();
        self.decls.push(CsDecl {
            entry: self.b.res.cs_types.len() - 1,
            loc: line(n).to_string(),
            methods: Vec::new(),
            bases: Vec::new(),
        });
        for bn in cs_base_types(n) {
            self.decls[di].bases.push(cs_base_name(Some(bn), src));
        }
        cs_bases(&mut self.b, &type_id, n, src, ifaces, &skip);
        // A record or class with a primary constructor takes its parameters as members.
        for ps in kids(n) {
            if ps.kind() == "parameter_list" {
                cs_params(&mut self.b, &type_id, ps, &skip, src);
            }
        }

        let Some(body) = n.child_by_field_name("body") else {
            return;
        };
        for m in kids(body) {
            match m.kind() {
                "field_declaration" => {
                    for v in named(m) {
                        if v.kind() == "variable_declaration" {
                            cs_type_refs(
                                &mut self.b,
                                &type_id,
                                v.child_by_field_name("type"),
                                &skip,
                                src,
                            );
                        }
                    }
                    continue;
                }
                "property_declaration" => {
                    cs_type_refs(
                        &mut self.b,
                        &type_id,
                        m.child_by_field_name("type"),
                        &skip,
                        src,
                    );
                    continue;
                }
                "method_declaration" => {}
                _ => continue,
            }
            let mname = field_text(m, "name", src);
            if mname.is_empty() {
                continue;
            }
            let mid = self.b.id(&[&name, &mname]);
            self.decls[di].methods.push(mname.clone());
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
            // Register under the bare method name so `x.Method()` call sites resolve.
            self.b.res.defs.push(Def {
                id: mid.clone(),
                name: mname,
                file: self.b.file.clone(),
                ..Def::default()
            });
            cs_signature(&mut self.b, &mid, m, &skip, src);
            let mtps = cs_type_params(m, &tps, src);
            self.cs_calls(m, &mid, &fields, &mtps);
        }
    }

    /// Go `csUsing`: records the namespace path of a `using` directive as an import. The imported
    /// namespace is the directive's `qualified_name`/`identifier` child; aliased usings
    /// (`using Foo = Bar;`) keep the aliased path.
    fn cs_using(&mut self, n: Node<'_>) {
        for c in kids(n) {
            if matches!(c.kind(), "qualified_name" | "identifier") {
                self.b.imp(&node_text(c, self.src), line(n));
                return;
            }
        }
    }

    /// Go `csCalls`: walks the body of the method or local function `fn_node` and records each
    /// call site. A bare call `f()` has no receiver. A member call records the receiver's type,
    /// as a code the resolver reads for .cs files alone:
    ///
    /// ```text
    /// this, base         the caller's own type, its base
    /// !U T               T named in source (a type, a cast, a Pascal-case member), U its declared type
    /// ?U                 a local, parameter or field of type U, an element of a U list, a new U
    /// ?U F               field F of a U
    /// new, new A.B       a construction of the callee type, qualified by A.B
    /// ```
    ///
    /// A member call whose receiver has no type is not recorded.
    fn cs_calls(
        &mut self,
        fn_node: Node<'_>,
        caller_id: &str,
        fields: &HashMap<String, String>,
        tps: &HashSet<String>,
    ) {
        let Some(body) = fn_node.child_by_field_name("body") else {
            return;
        };
        let src = self.src;
        let mut skip = tps.clone();
        skip.extend(self.own_nested.iter().cloned());
        let mask = *CALL_KINDS
            .get()
            .expect("kinds::register_kinds not called before extract_csharp");
        let b = &mut self.b;
        let cs_nested = &self.nested;
        let mut scope: Option<CsScope> = None;
        walk_kinds(Some(body), mask, |c| {
            if c.kind() == "object_creation_expression" {
                let (tail, _, prefix) = cs_read_type_name(c.child_by_field_name("type"), src);
                if !tail.is_empty() && !cs_nested.contains(&tail) && !cs_builtin(&tail) {
                    let mut recv = "new".to_string();
                    if !prefix.is_empty() {
                        recv.push(' ');
                        recv.push_str(&prefix);
                    }
                    b.call_recv(caller_id, &tail, &recv, line(c));
                }
                return true;
            }
            let Some(f) = c.child_by_field_name("function") else {
                return true;
            };
            let mut name = None;
            let mut recv = None;
            match f.kind() {
                "identifier" => b.call(caller_id, &node_text(f, src), line(c)),
                "generic_name" => cs_targs(b, caller_id, f, &skip, src),
                "member_access_expression" => {
                    name = f.child_by_field_name("name");
                    recv = f.child_by_field_name("expression");
                }
                "conditional_access_expression" => {
                    if let Some(mb) = named(f).find(|mb| mb.kind() == "member_binding_expression") {
                        name = mb.child_by_field_name("name");
                        recv = f.child_by_field_name("condition");
                    }
                }
                _ => {}
            }
            let (Some(name), Some(recv)) = (name, recv) else {
                return true;
            };
            let mut callee = node_text(name, src).into_owned();
            if name.kind() == "generic_name" {
                cs_targs(b, caller_id, name, &skip, src);
                if let Some(id) = named(name).find(|id| id.kind() == "identifier") {
                    callee = node_text(id, src).into_owned();
                }
            }
            let mut type_at = |n: &str, at: u32| -> String {
                scope
                    .get_or_insert_with(|| cs_scope_of(fn_node, fields, tps, src))
                    .type_at(n, at)
            };
            let code = cs_recv(recv, c.start_byte(), &mut type_at, src);
            if !code.is_empty() {
                b.call_recv(caller_id, &callee, &code, line(c));
            }
            true
        });
    }
}

/// Go `targs` in `csCalls`: the type arguments of the generic_name `g`, as `Arg` refs.
fn cs_targs(b: &mut Builder, caller_id: &str, g: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
    let n = b.res.type_refs.len();
    cs_type_args(b, caller_id, g, skip, src);
    for t in &mut b.res.type_refs[n..] {
        t.arg = true;
    }
}

/// Go `csField`: adds a field or property declaration `m` to the table: name -> type,
/// name[] -> element type, name{} (and .Keys{}, .Values{}) -> loop element.
fn cs_field(table: &mut HashMap<String, String>, m: Node<'_>, tps: &HashSet<String>, src: &[u8]) {
    let mut t = None;
    let mut names: Vec<String> = Vec::new();
    match m.kind() {
        "field_declaration" => {
            for v in named(m) {
                if v.kind() != "variable_declaration" {
                    continue;
                }
                t = v.child_by_field_name("type");
                for d in named(v) {
                    if d.kind() == "variable_declarator" {
                        names.push(cs_decl_name(d, src));
                    }
                }
            }
            // Properties keep type parameters upstream.
            let (tail, _, _) = cs_read_type_name(t, src);
            if tail.is_empty() || tps.contains(&tail) {
                return;
            }
        }
        "property_declaration" => {
            t = m.child_by_field_name("type");
            names = vec![field_text(m, "name", src)];
        }
        _ => return,
    }
    let typ = cs_recv_type(t, src);
    if typ.is_empty() {
        return;
    }
    let elem = cs_elem_type(t, tps, src);
    let loops = cs_loop_elems(t, tps, src);
    for name in &names {
        if name.is_empty() {
            continue;
        }
        table.insert(name.clone(), typ.clone());
        if !elem.is_empty() {
            table.insert(format!("{name}[]"), elem.clone());
        }
        for (suffix, e) in &loops {
            table.insert(format!("{name}{suffix}{{}}"), e.clone());
        }
    }
}

/// Go `csDeclName`: the name of a variable_declarator.
fn cs_decl_name(d: Node<'_>, src: &[u8]) -> String {
    if let Some(name) = d.child_by_field_name("name") {
        return node_text(name, src).into_owned();
    }
    for c in named(d) {
        if c.kind() == "identifier" {
            return node_text(c, src).into_owned();
        }
    }
    String::new()
}

/// Go `csArityKey`: keys the tables apart for generic twins.
fn cs_arity_key(name: &str, arity: usize) -> String {
    if arity == 0 {
        return name.to_string();
    }
    format!("{name}`{arity}")
}

/// Go `csNamespace`: the namespace `n` declares inside `ns`.
fn cs_namespace(ns: &str, n: Node<'_>, src: &[u8]) -> String {
    // Spaces split CsTypes codes.
    let name = field_text(n, "name", src).replace(' ', "\t");
    if ns.is_empty() {
        return name;
    }
    format!("{ns}.{name}")
}

/// Go `csUseCode`: the CsTypes code of a using directive: `+A.B` for a namespace, `&X=A.B` for
/// an alias, "" for `using static`.
fn cs_use_code(text: &str) -> String {
    use gostd::strings::trim_space;
    let t0 = trim_space(text);
    let t0 = t0.strip_suffix(';').unwrap_or(t0);
    let t0 = t0.strip_prefix("global ").unwrap_or(t0);
    let body = trim_space(t0);
    let Some(body) = body.strip_prefix("using") else {
        return String::new();
    };
    if body.starts_with(" static ") {
        return String::new();
    }
    let mut code = format!("+{}", trim_space(body));
    if let Some((a, t)) = body.split_once('=') {
        // Upstream strips only a trailing <...>.
        let mut t = trim_space(t);
        if t.ends_with('>') {
            let bs = t.as_bytes();
            let mut depth: i64 = 0;
            for i in (0..bs.len()).rev() {
                if bs[i] == b'>' {
                    depth += 1;
                } else if bs[i] == b'<' {
                    depth -= 1;
                    if depth == 0 {
                        t = &t[..i];
                        break;
                    }
                }
            }
        }
        code = format!("&{}={}", trim_space(a), trim_space(t));
    }
    // Spaces split CsTypes codes.
    code.replace(' ', "\t")
}

/// Go `csRecv`: the receiver code of a member call made at byte `at`.
fn cs_recv(
    recv: Node<'_>,
    at: u32,
    type_at: &mut dyn FnMut(&str, u32) -> String,
    src: &[u8],
) -> String {
    match recv.kind() {
        "identifier" => return cs_named(&node_text(recv, src), at, type_at),
        "this" | "this_expression" => return "this".to_string(),
        "base" | "base_expression" => return "base".to_string(),
        "member_access_expression" => {
            let inner = recv.child_by_field_name("expression");
            let f = recv.child_by_field_name("name");
            let (Some(inner), Some(f)) = (inner, f) else {
                return String::new();
            };
            if f.kind() != "identifier" {
                return String::new();
            }
            match inner.kind() {
                "this" | "this_expression" => {
                    return cs_named(&node_text(f, src), at, type_at);
                }
                "identifier" => {
                    let t = type_at(&node_text(inner, src), at);
                    if !t.is_empty() {
                        return format!("?{t} {}", node_text(f, src));
                    }
                }
                _ => {}
            }
        }
        "element_access_expression" => {
            let inner = recv.child_by_field_name("expression");
            if let Some(inner) = inner
                && inner.kind() == "identifier"
            {
                let t = type_at(&format!("{}[]", node_text(inner, src)), at);
                if !t.is_empty() {
                    return format!("?{t}");
                }
            }
        }
        "object_creation_expression" => {
            // Upstream skips the Pascal-case check here.
            let t = cs_type_tail(recv.child_by_field_name("type"), src);
            if !t.is_empty() {
                return format!("?{t}");
            }
        }
        "parenthesized_expression" => {
            let Some(inner) = recv.named_child(0) else {
                return String::new();
            };
            let t = match inner.kind() {
                "cast_expression" => inner.child_by_field_name("type"),
                "as_expression" => inner.child_by_field_name("right"),
                _ => None,
            };
            let (tail, _, _) = cs_read_type_name(t, src);
            return cs_named(&tail, at, type_at);
        }
        _ => {}
    }
    String::new()
}

/// Go `csNamed`: the receiver code of a name: a Pascal-case one names a type in source unless
/// its declared type says otherwise, a lowercase one needs a declared type.
fn cs_named(name: &str, at: u32, type_at: &mut dyn FnMut(&str, u32) -> String) -> String {
    if name.is_empty() {
        return String::new();
    }
    let u = type_at(name, at);
    if !first_is_upper(name) {
        if u.is_empty() {
            return String::new();
        }
        return format!("?{u}");
    }
    if u.is_empty() || u == name {
        return format!("!{name}");
    }
    format!("!{u} {name}")
}

/// Go `r, _ := utf8.DecodeRuneInString(s); unicode.IsUpper(r)`: "" decodes to `RuneError`, which
/// is no upper-case letter.
fn first_is_upper(s: &str) -> bool {
    s.chars().next().is_some_and(is_upper)
}

/// Go `csBind`: a name declared over the byte range `[lo, hi)`, with its type ("" for none).
struct CsBind {
    lo: u32,
    hi: u32,
    typ: String,
}

/// Go `csScope`: types the receivers of one method: its declarations by range, and the field
/// table of its type for a name none of them covers or poisons.
struct CsScope<'f> {
    /// Sorted by `lo`.
    binds: HashMap<String, Vec<CsBind>>,
    fields: &'f HashMap<String, String>,
    poison: HashSet<String>,
}

impl CsScope<'_> {
    /// Go `typeAt`: the type of `name` at byte `at`: the innermost declaration covering it, else
    /// the field. A tie or an untyped declaration gives "".
    fn type_at(&self, name: &str, at: u32) -> String {
        let bs: &[CsBind] = self.binds.get(name).map_or(&[], Vec::as_slice);
        // Go `slices.BinarySearchFunc(bs, at+1, ...)`: the first bind with lo > at.
        let mut j = bs.partition_point(|d| d.lo <= at) as isize - 1;
        while j >= 0 && bs[j as usize].hi <= at {
            j -= 1;
        }
        if j < 0 {
            if self.poison.contains(name) {
                return String::new();
            }
            return self.fields.get(name).cloned().unwrap_or_default();
        }
        let j = j as usize;
        // Scopes nest: an earlier start covering at is wider.
        let (mut typ, mut size, mut n) = ("", 0u32, 0);
        let mut k = j as isize;
        while k >= 0 && bs[k as usize].lo == bs[j].lo {
            let d = &bs[k as usize];
            let w = d.hi.wrapping_sub(d.lo);
            // Go's first case, `d.hi <= at`, does nothing.
            if d.hi > at {
                if n == 0 || w < size {
                    (typ, size, n) = (d.typ.as_str(), w, 1);
                } else if w == size {
                    n += 1;
                }
            }
            k -= 1;
        }
        if n > 1 {
            return String::new();
        }
        typ.to_string()
    }
}

/// A Go `foreach` whose element type waits for the field table (`csScopeOf`'s `loop`).
struct Loop<'t> {
    name: String,
    field: String,
    suffix: String,
    n: Node<'t>,
}

/// Go `csScopeOf`: collects the declarations of `fn_node`. A declaration whose type differs from
/// a same-named field hides that field in all of `fn_node`. Go's `visit` recurses on the tree;
/// this walks it with a stack, in the same preorder.
fn cs_scope_of<'f>(
    fn_node: Node<'_>,
    fields: &'f HashMap<String, String>,
    tps: &HashSet<String>,
    src: &[u8],
) -> CsScope<'f> {
    let mut s = CsScope {
        binds: HashMap::new(),
        fields,
        poison: HashSet::new(),
    };
    let bind = |s: &mut CsScope, name: &str, typ: &str, scope: Node<'_>, elem: &str| {
        if name.is_empty() {
            return;
        }
        let elem_name = format!("{name}[]");
        for (k, v) in [(name, typ), (elem_name.as_str(), elem)] {
            if let Some(f) = fields.get(k)
                && f != v
            {
                s.poison.insert(k.to_string());
            }
            s.binds.entry(k.to_string()).or_default().push(CsBind {
                lo: scope.start_byte(),
                hi: scope.end_byte(),
                typ: v.to_string(),
            });
        }
    };
    let bind_params = |s: &mut CsScope, ps: Option<Node<'_>>, scope: Node<'_>| {
        let Some(ps) = ps else { return };
        for p in kids(ps) {
            match p.kind() {
                "parameter" => {
                    let t = p.child_by_field_name("type");
                    bind(
                        s,
                        &field_text(p, "name", src),
                        &cs_recv_type(t, src),
                        scope,
                        &cs_elem_type(t, tps, src),
                    );
                }
                "implicit_parameter" => bind(s, &node_text(p, src), "", scope, ""),
                _ => {}
            }
        }
    };
    let mut loops: Vec<Loop<'_>> = Vec::new();
    let Some(body) = fn_node.child_by_field_name("body") else {
        return s;
    };
    bind_params(&mut s, fn_node.child_by_field_name("parameters"), body);
    // (node, scope), popped in preorder.
    let mut stack: Vec<(Node<'_>, Node<'_>)> = kids(body).map(|c| (c, body)).collect();
    stack.reverse();
    while let Some((n, mut scope)) = stack.pop() {
        match n.kind() {
            k if is_type_decl(k) => continue,
            "lambda_expression" => {
                let ps = n.child_by_field_name("parameters");
                match ps {
                    Some(ps) if ps.kind() == "implicit_parameter" => {
                        bind(&mut s, &node_text(ps, src), "", n, "");
                    }
                    _ => bind_params(&mut s, ps, n),
                }
            }
            "local_function_statement" => {
                bind_params(&mut s, n.child_by_field_name("parameters"), n);
            }
            "local_declaration_statement" => {
                if let Some(vd) = named(n).find(|vd| vd.kind() == "variable_declaration") {
                    let t = vd.child_by_field_name("type");
                    let declared = cs_recv_type(t, src);
                    let elem = cs_elem_type(t, tps, src);
                    for d in named(vd) {
                        if d.kind() != "variable_declarator" {
                            continue;
                        }
                        let mut typ = declared.clone();
                        // ponytail: var v = M() untyped (byx.46)
                        for g in kids(d) {
                            if !typ.is_empty() {
                                break;
                            }
                            if g.kind() == "object_creation_expression" {
                                typ = cs_recv_type(g.child_by_field_name("type"), src);
                            } else if g.kind() == "invocation_expression" {
                                break;
                            }
                        }
                        bind(&mut s, &cs_decl_name(d, src), &typ, scope, &elem);
                    }
                }
            }
            "foreach_statement" => {
                if let Some(left) = n.child_by_field_name("left")
                    && left.kind() == "identifier"
                {
                    let typ = cs_recv_type(n.child_by_field_name("type"), src);
                    let mut source = None;
                    if typ.is_empty() {
                        source = cs_loop_source(n.child_by_field_name("right"), src);
                    }
                    match source {
                        Some((field, suffix)) => loops.push(Loop {
                            name: node_text(left, src).into_owned(),
                            field,
                            suffix,
                            n,
                        }),
                        None => bind(&mut s, &node_text(left, src), &typ, n, ""),
                    }
                }
            }
            "declaration_expression" | "declaration_pattern" => {
                if let Some(name) = n.child_by_field_name("name")
                    && name.kind() == "identifier"
                {
                    let typ = cs_recv_type(n.child_by_field_name("type"), src);
                    bind(&mut s, &node_text(name, src), &typ, scope, "");
                }
            }
            _ => {}
        }
        if matches!(
            n.kind(),
            "block" | "lambda_expression" | "local_function_statement"
        ) {
            scope = n;
        }
        let at = stack.len();
        stack.extend(kids(n).map(|c| (c, scope)));
        stack[at..].reverse();
    }
    // Hiding declarations are all known now.
    for l in &loops {
        let mut typ = fields
            .get(&format!("{}{}{{}}", l.field, l.suffix))
            .cloned()
            .unwrap_or_default();
        let start = l.n.start_byte();
        if let Some(bs) = s.binds.get(&l.field) {
            for d in bs {
                if d.lo <= start && start < d.hi {
                    typ = String::new();
                }
            }
        }
        bind(&mut s, &l.name, &typ, l.n, "");
    }
    for k in &s.poison {
        s.binds.remove(k);
    }
    for bs in s.binds.values_mut() {
        bs.sort_by_key(|d| d.lo);
    }
    s
}

/// Go `csLoopSource`: the field and suffix ("", ".Keys", ".Values") of a foreach over `f`,
/// `this.f`, `f.Keys` or `f.Values`; `None` for anything else.
fn cs_loop_source(e: Option<Node<'_>>, src: &[u8]) -> Option<(String, String)> {
    let e = e?;
    if e.kind() == "identifier" {
        return Some((node_text(e, src).into_owned(), String::new()));
    }
    let inner = e.child_by_field_name("expression");
    let name = e.child_by_field_name("name");
    let (Some(inner), Some(name)) = (inner, name) else {
        return None;
    };
    if e.kind() != "member_access_expression" || name.kind() != "identifier" {
        return None;
    }
    let member = node_text(name, src);
    if matches!(inner.kind(), "this" | "this_expression") {
        return Some((member.into_owned(), String::new()));
    }
    if member == "Values" || member == "Keys" {
        // Go recurses once per `.Keys`/`.Values`; only a bare field may sit under one.
        if let Some((f, sfx)) = cs_loop_source(Some(inner), src)
            && sfx.is_empty()
        {
            return Some((f, format!(".{member}")));
        }
    }
    None
}

/// Go `csReadTypeName`: the last name of a type, generic arguments cut, and the qualifier it was
/// written with: (tail, qualified, prefix).
fn cs_read_type_name(n: Option<Node<'_>>, src: &[u8]) -> (String, bool, String) {
    let (tail, qualified, prefix, _) = cs_type_name(n, src);
    // Spaces split CsTypes and receiver codes.
    (
        tail.replace(' ', "\t"),
        qualified,
        prefix.replace(' ', "\t"),
    )
}

/// Go `csBaseName`: a base_list entry's name, read as upstream does.
fn cs_base_name(n: Option<Node<'_>>, src: &[u8]) -> String {
    cs_type_name(n, src).0
}

/// Go `csTypeName`: (tail, qualified, prefix, ok).
fn cs_type_name(n: Option<Node<'_>>, src: &[u8]) -> (String, bool, String, bool) {
    let Some(n) = n else {
        return (String::new(), false, String::new(), false);
    };
    match n.kind() {
        "identifier" | "predefined_type" => {
            return (node_text(n, src).into_owned(), false, String::new(), true);
        }
        "qualified_name" => {
            let text = node_text(n, src);
            let (tail_from, prefix) = match text.rfind('.') {
                Some(i) => (&text[i + 1..], &text[..i]),
                None => (&text[..], ""),
            };
            let tail = cut_before(tail_from, "<");
            return (tail.to_string(), true, prefix.to_string(), true);
        }
        _ => {}
    }
    for c in named(n) {
        let r = cs_type_name(Some(c), src);
        if r.3 {
            return r;
        }
    }
    (String::new(), false, String::new(), false)
}

/// Go `csRecvType`: the type `t` names when a receiver of it can own a method: Pascal case.
fn cs_recv_type(t: Option<Node<'_>>, src: &[u8]) -> String {
    let (tail, _, _) = cs_read_type_name(t, src);
    if !first_is_upper(&tail) {
        return String::new();
    }
    tail
}

/// Go `csLists`.
fn cs_list(n: &str) -> bool {
    matches!(n, "List" | "IList" | "IReadOnlyList")
}

/// Go `csNameKinds`.
fn cs_name_kind(k: &str) -> bool {
    matches!(k, "identifier" | "qualified_name" | "generic_name")
}

/// Go `csSeqs`.
fn cs_seq(n: &str) -> bool {
    matches!(
        n,
        "List"
            | "IList"
            | "IReadOnlyList"
            | "IEnumerable"
            | "ICollection"
            | "IReadOnlyCollection"
            | "HashSet"
            | "ISet"
            | "SortedSet"
            | "Queue"
            | "Stack"
            | "LinkedList"
    )
}

/// Go `csDicts`.
fn cs_dict(n: &str) -> bool {
    matches!(
        n,
        "Dictionary"
            | "IDictionary"
            | "IReadOnlyDictionary"
            | "SortedDictionary"
            | "SortedList"
            | "ConcurrentDictionary"
    )
}

/// Go `csElemType`: the element type of an array or list type `t`.
fn cs_elem_type(t: Option<Node<'_>>, tps: &HashSet<String>, src: &[u8]) -> String {
    let Some(t) = t else {
        return String::new();
    };
    let mut elem = None;
    match t.kind() {
        "array_type" => elem = t.child_by_field_name("type"),
        "generic_name" => {
            let args = cs_type_arg_list(t);
            if cs_list(&cs_type_tail(Some(t), src)) && args.len() == 1 {
                elem = Some(args[0]);
            }
        }
        _ => {}
    }
    let Some(elem) = elem else {
        return String::new();
    };
    if !cs_name_kind(elem.kind()) {
        return String::new();
    }
    let name = cs_recv_type(Some(elem), src);
    if !cs_list(&name) && !tps.contains(&name) {
        return name;
    }
    String::new()
}

/// Go `csLoopElems`: maps what a foreach over a field of type `t` iterates ("" for the field,
/// ".Keys", ".Values" for a dictionary's) to the element type.
fn cs_loop_elems(
    t: Option<Node<'_>>,
    tps: &HashSet<String>,
    src: &[u8],
) -> HashMap<&'static str, String> {
    let mut args: Vec<Option<Node<'_>>> = Vec::new();
    let mut suffixes: &[&'static str] = &[];
    if let Some(t) = t {
        match t.kind() {
            "array_type" => {
                args = vec![t.child_by_field_name("type")];
                suffixes = &[""];
            }
            "generic_name" => {
                args = cs_type_arg_list(t).into_iter().map(Some).collect();
                let tail = cs_type_tail(Some(t), src);
                if cs_seq(&tail) && args.len() == 1 {
                    suffixes = &[""];
                } else if cs_dict(&tail) && args.len() == 2 {
                    suffixes = &[".Keys", ".Values"];
                }
            }
            _ => {}
        }
    }
    let mut out = HashMap::new();
    for (i, sfx) in suffixes.iter().enumerate() {
        if let Some(a) = args[i]
            && cs_name_kind(a.kind())
        {
            let name = cs_recv_type(Some(a), src);
            if !name.is_empty() && !tps.contains(&name) && !cs_seq(&name) && !cs_dict(&name) {
                out.insert(*sfx, name);
            }
        }
    }
    out
}

/// Go `csTypeTail`.
fn cs_type_tail(t: Option<Node<'_>>, src: &[u8]) -> String {
    cs_read_type_name(t, src).0
}

/// Go `csTypeArgList`: the type arguments of the generic_name `t`.
fn cs_type_arg_list(t: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    for args in named(t) {
        if args.kind() == "type_argument_list" {
            out.extend(named(args));
        }
    }
    out
}

/// Go `csBuiltins`: upstream's builtin globals; `new X()` of one is no call.
fn cs_builtin(n: &str) -> bool {
    matches!(
        n,
        "String"
            | "Number"
            | "Boolean"
            | "Object"
            | "Array"
            | "Symbol"
            | "BigInt"
            | "Date"
            | "RegExp"
            | "Error"
            | "TypeError"
            | "RangeError"
            | "SyntaxError"
            | "ReferenceError"
            | "EvalError"
            | "URIError"
            | "Promise"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "JSON"
            | "Math"
            | "Reflect"
            | "Proxy"
            | "Intl"
            | "parseInt"
            | "parseFloat"
            | "isNaN"
            | "isFinite"
            | "encodeURIComponent"
            | "decodeURIComponent"
            | "encodeURI"
            | "decodeURI"
            | "URL"
            | "URLSearchParams"
            | "FormData"
            | "Blob"
            | "File"
            | "Headers"
            | "Request"
            | "Response"
            | "AbortController"
            | "AbortSignal"
            | "TextEncoder"
            | "TextDecoder"
            | "console"
            | "str"
            | "int"
            | "float"
            | "bool"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "bytes"
            | "len"
            | "range"
            | "enumerate"
            | "zip"
            | "map"
            | "filter"
            | "sum"
            | "min"
            | "max"
            | "print"
            | "open"
            | "isinstance"
            | "type"
            | "super"
            | "sorted"
            | "reversed"
            | "any"
            | "all"
            | "abs"
            | "round"
            | "next"
            | "iter"
            | "hash"
            | "id"
            | "repr"
            | "callable"
            | "getattr"
            | "setattr"
            | "hasattr"
            | "delattr"
            | "vars"
            | "dir"
            | "Int"
            | "Int8"
            | "Int16"
            | "Int32"
            | "Int64"
            | "UInt"
            | "UInt8"
            | "UInt16"
            | "UInt32"
            | "UInt64"
            | "Double"
            | "Float"
            | "Bool"
            | "Character"
            | "Sendable"
            | "Codable"
            | "Decodable"
            | "Encodable"
            | "Equatable"
            | "Hashable"
            | "Identifiable"
            | "Comparable"
            | "CaseIterable"
            | "RawRepresentable"
            | "CustomStringConvertible"
            | "CustomDebugStringConvertible"
            | "AnyObject"
            | "LocalizedError"
            | "Data"
            | "UUID"
            | "Decimal"
            | "Calendar"
            | "Locale"
            | "TimeZone"
            | "Bundle"
            | "IndexPath"
            | "IndexSet"
            | "NotificationCenter"
            | "UserDefaults"
            | "FileManager"
            | "URLSession"
            | "URLRequest"
            | "URLComponents"
            | "JSONDecoder"
            | "JSONEncoder"
            | "DateFormatter"
            | "NumberFormatter"
            | "ISO8601DateFormatter"
            | "NSObject"
            | "NSString"
            | "NSError"
            | "NSLock"
            | "NSAttributedString"
            | "DispatchQueue"
            | "DispatchGroup"
            | "OperationQueue"
            | "RunLoop"
            | "View"
            | "Color"
            | "Font"
    )
}

/// Go `csInterfaceNames`: the interfaces declared anywhere in the file, so a base can be
/// classified as implements rather than inherits.
fn cs_interface_names(root: Node<'_>, src: &[u8]) -> HashSet<String> {
    let mask = *INTERFACE_KINDS
        .get()
        .expect("kinds::register_kinds not called before extract_csharp");
    let mut names = HashSet::new();
    walk_kinds(Some(root), mask, |nv| {
        let name = field_text(nv, "name", src);
        if !name.is_empty() {
            names.insert(name);
        }
        true
    });
    names
}

/// Go `csBases`: records the entries of a type's base_list. C# does not distinguish a base class
/// from an implemented interface syntactically, so a base is an implements target when it is an
/// interface declared in this file or follows the IPascalCase convention, and an inherits target
/// otherwise. Every base of an interface is interface inheritance.
fn cs_bases(
    b: &mut Builder,
    type_id: &str,
    n: Node<'_>,
    src: &[u8],
    ifaces: &HashSet<String>,
    skip: &HashSet<String>,
) {
    let is_interface = n.kind() == "interface_declaration";
    for c in cs_base_types(n) {
        let base = cs_base_name(Some(c), src);
        let mut relation = "inherits";
        if !is_interface && cs_is_interface_name(&base, ifaces) {
            relation = "implements";
        }
        match c.kind() {
            "identifier" | "generic_name" | "qualified_name" => {
                if skip.contains(&base) {
                    continue;
                }
                b.ref_(type_id, &base, relation, false, line(n));
                if c.kind() == "generic_name" {
                    cs_type_args(b, type_id, c, skip, src);
                }
            }
            _ => b.type_ref(type_id, &base, relation, line(n)),
        }
    }
}

/// Go `csBaseTypes`: the base_list entries of `n` upstream reads.
fn cs_base_types(n: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    for bl in kids(n) {
        if bl.kind() != "base_list" {
            continue;
        }
        // Constructor args; upstream skips `B(x)` record bases.
        out.extend(named(bl).filter(|c| {
            c.kind() != "argument_list" && c.kind() != "primary_constructor_base_type"
        }));
    }
    out
}

/// Go `csIsInterfaceName`: whether `base` names an interface: declared as one in this file, or
/// named IPascalCase by convention.
fn cs_is_interface_name(base: &str, ifaces: &HashSet<String>) -> bool {
    if ifaces.contains(base) {
        return true;
    }
    let mut r = base.chars();
    r.next() == Some('I') && r.next().is_some_and(is_upper)
}

/// Go `csTypeParams`: `skip` plus the type parameters `n` declares.
fn cs_type_params(n: Node<'_>, skip: &HashSet<String>, src: &[u8]) -> HashSet<String> {
    list_type_params(n, "type_parameter_list", skip, src)
}

/// Go `listTypeParams` (`csharp.go:1031`): `skip` plus the names in the `list_kind` child of `n`.
fn list_type_params(
    n: Node<'_>,
    list_kind: &str,
    skip: &HashSet<String>,
    src: &[u8],
) -> HashSet<String> {
    let mut out = skip.clone();
    for list in kids(n) {
        if list.kind() != list_kind {
            continue;
        }
        for p in named(list) {
            if let Some(id) = named(p).find(|id| id.kind() == "identifier") {
                out.insert(node_text(id, src).into_owned());
            }
        }
    }
    out
}

/// Go `csNestedTypes`: adds the names of the types nested in `n` to `out`. They have no node, so
/// a ref to one must not bind to a namesake elsewhere.
fn cs_nested_types(n: Node<'_>, src: &[u8], out: &mut HashSet<String>) {
    let Some(body) = n.child_by_field_name("body") else {
        return;
    };
    for c in named(body) {
        if is_type_decl(c.kind()) {
            out.insert(field_text(c, "name", src));
            cs_nested_types(c, src, out);
        }
    }
}

/// Go `csRef`.
fn cs_ref(b: &mut Builder, id: &str, name: &str, skip: &HashSet<String>, loc: Loc) {
    if !skip.contains(name) {
        b.ref_(id, name, "references", false, loc);
    }
}

/// Go `text[strings.LastIndex(text, ".")+1:]`.
fn after_last_dot(text: &str) -> &str {
    text.rfind('.').map_or(text, |i| &text[i + 1..])
}

/// Go `csTypeRefs`: a reference from `id` to each type `t` names, generic arguments included.
/// Keyword types and names in `skip` give nothing; a qualified name keeps its last segment.
fn cs_type_refs(
    b: &mut Builder,
    id: &str,
    t: Option<Node<'_>>,
    skip: &HashSet<String>,
    src: &[u8],
) {
    let Some(t) = t else { return };
    match t.kind() {
        "predefined_type" => {}
        "identifier" => cs_ref(b, id, &node_text(t, src), skip, line(t)),
        "qualified_name" => {
            let text = node_text(t, src);
            let name = cut_before(after_last_dot(&text), "<");
            cs_ref(b, id, name, skip, line(t));
        }
        "generic_name" => {
            if let Some(c) = named(t).find(|c| matches!(c.kind(), "identifier" | "qualified_name"))
            {
                let text = node_text(c, src);
                cs_ref(b, id, after_last_dot(&text), skip, line(c));
            }
            cs_type_args(b, id, t, skip, src);
        }
        "tuple_type" => {
            for el in named(t) {
                if el.kind() == "tuple_element" {
                    cs_type_refs(b, id, el.child_by_field_name("type"), skip, src);
                }
            }
        }
        _ => {
            for c in named(t) {
                cs_type_refs(b, id, Some(c), skip, src);
            }
        }
    }
}

/// Go `csTypeArgs`: the types in the type_argument_list of the generic_name `t`.
fn cs_type_args(b: &mut Builder, id: &str, t: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
    for args in named(t) {
        if args.kind() == "type_argument_list" {
            for a in named(args) {
                cs_type_refs(b, id, Some(a), skip, src);
            }
        }
    }
}

/// Go `csParams`: the type of each parameter of the parameter_list `ps`.
fn cs_params(b: &mut Builder, id: &str, ps: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
    for p in named(ps) {
        if p.kind() == "parameter" {
            cs_type_refs(b, id, p.child_by_field_name("type"), skip, src);
        }
    }
}

/// Go `csSignature`: the parameter and return types and the attributes of method `m`. Upstream
/// reads attributes from methods only.
fn cs_signature(b: &mut Builder, id: &str, m: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
    let skip = cs_type_params(m, skip, src);
    if let Some(ps) = m.child_by_field_name("parameters") {
        cs_params(b, id, ps, &skip, src);
    }
    cs_type_refs(b, id, m.child_by_field_name("returns"), &skip, src);
    for al in named(m) {
        if al.kind() != "attribute_list" {
            continue;
        }
        for a in named(al) {
            if a.kind() == "attribute"
                && let Some(nm) = a.child_by_field_name("name")
            {
                let text = node_text(nm, src);
                cs_ref(b, id, after_last_dot(&text), &skip, line(a));
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/csharp.rs"]
mod csharp_tests;

#[cfg(test)]
#[path = "tests/csharp_calls.rs"]
mod csharp_calls_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_calls.rs"]
mod upstream_csharp_calls_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_types.rs"]
mod upstream_csharp_types_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_members.rs"]
mod upstream_csharp_members_tests;

#[cfg(test)]
#[path = "tests/upstream_csharp_extension.rs"]
mod upstream_csharp_extension_tests;
