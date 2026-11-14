//! Port of `python.go`: functions, classes (+ methods), imports, and call edges out of a .py
//! file. Top-level functions/classes become definitions; methods are nested under their class.
//! Calls are recorded by the called name and resolved against the corpus by `resolve`.
//!
//! Go keeps `pyMods`, `pyTypeVars`, `pyOwn`, `pyBodies` and `jsLinked` on its one `builder`;
//! here they are the fields of [`Py`], beside a `Builder`.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, OnceLock};

use gositter::Node;
use gostd::regexp::Regexp;
use gostd::strings::{bytes_trim_space, trim_space};

use super::builder::{
    Builder, base_type_name, bind_type, go_text, line, node_text, parse_root, recv_text, walk_kinds,
};
use super::extract::{Def, ImportAlias, Result};
use super::golang::field_text;
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Loc, Relation};

/// Go `extractPython`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_python(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::python::language(), |root| {
        let mut p = Py {
            b: Builder::new(rel),
            py_mods: py_modules(root, src),
            py_type_vars: py_type_var_names(root, src),
            py_own: HashSet::new(),
            py_bodies: Vec::new(),
            js_linked: HashSet::new(),
        };

        for i in 0..root.child_count() {
            if let Some(n) = root.child(i) {
                p.py_statement(n, src);
            }
        }
        // The names a module-level def, class or from-import of the file binds.
        for d in &p.b.res.defs {
            if d.id == p.b.id(&[&d.name]) {
                p.py_own.insert(d.name.clone());
            }
        }
        for a in &p.b.res.import_aliases {
            if !a.in_func {
                p.py_own.insert(a.local.clone());
            }
        }
        let dyn_ = py_dynamic(root, src);
        for i in 0..p.py_bodies.len() {
            let (func, id, method) = (
                p.py_bodies[i].func,
                p.py_bodies[i].id.clone(),
                p.py_bodies[i].method,
            );
            p.py_calls(func, &id, !dyn_ && !method, src);
        }
        py_rationale(&mut p.b, root, src);
        p.b.res
    })
}

/// Go `pyScope`: the function or class the pyCalls walk is inside. A class adds to the ids
/// below it alone: `func` and the body are those of the function around. `outer` is the function
/// a def is made in, which runs its defaults; `local` marks a method of a class made in a
/// function, where self is not that of `func`.
#[derive(Clone)]
struct PyScope {
    id: String,
    /// Go `fn`.
    func: String,
    outer: String,
    end: u32,
    body_lo: u32,
    body_hi: u32,
    local: bool,
}

/// Go `pyBody`: a function whose calls pyCalls records once the file's own definitions have
/// their ids.
struct PyBody<'t> {
    /// Go `fn`.
    func: Node<'t>,
    id: String,
    method: bool,
}

/// The Go `builder` fields python.go reads, beside the [`Builder`].
struct Py<'t> {
    b: Builder,
    /// Go `pyMods`: `None` is Go's nil map (a star import).
    py_mods: Option<HashMap<String, String>>,
    py_type_vars: HashSet<String>,
    py_own: HashSet<String>,
    py_bodies: Vec<PyBody<'t>>,
    /// Go `jsLinked`: the `calls` edges of `javascript::js_nested_call` already made.
    js_linked: HashSet<(String, String)>,
}

impl<'t> Py<'t> {
    /// Go `pyStatement`: handles one module-level statement, unwrapping decorators first.
    fn py_statement(&mut self, n: Node<'t>, src: &[u8]) {
        match n.kind() {
            "decorated_definition" => match n.child_by_field_name("definition") {
                Some(d) if d.kind() == "function_definition" => self.py_func(d, Some(n), src),
                Some(d) if d.kind() == "class_definition" => self.py_class(d, Some(n), src),
                _ => {}
            },
            "function_definition" => self.py_func(n, None, src),
            "class_definition" => self.py_class(n, None, src),
            "import_statement"
            | "import_from_statement"
            | "if_statement"
            | "try_statement"
            | "with_statement"
            | "for_statement"
            | "while_statement"
            | "match_statement" => self.py_nested_imports(n, src, false),
            _ => {}
        }
    }

    /// Go `pyNestedImports`: records the imports under n, including ones guarded by an
    /// if/try/with block. Imports in the body of `if TYPE_CHECKING:` (or
    /// `<x>.TYPE_CHECKING`) never run, so they are type-only; else branches are not.
    fn py_nested_imports(&mut self, n: Node<'t>, src: &[u8], type_only: bool) {
        let mut guard = false;
        match n.kind() {
            "import_statement" | "import_from_statement" => {
                self.py_imports(n, src, type_only);
                return;
            }
            "function_definition" | "class_definition" | "decorated_definition" => return,
            "if_statement" | "elif_clause" => {
                let mut cond = n.child_by_field_name("condition");
                if let Some(c) = cond.filter(|c| c.kind() == "attribute") {
                    cond = c.child_by_field_name("attribute");
                }
                guard = cond.is_some_and(|c| {
                    c.kind() == "identifier" && node_text(c, src) == "TYPE_CHECKING"
                });
            }
            _ => {}
        }
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            let consequence = n.field_name_for_child(i) == Some("consequence");
            self.py_nested_imports(c, src, type_only || (guard && consequence));
        }
    }

    /// Go `pyFunc`: takes the decorated_definition around n, or `None`.
    fn py_func(&mut self, n: Node<'t>, deco: Option<Node<'t>>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &format!("{name}()"), line(n));
        self.py_refs(&id, n, deco, src);
        self.py_bodies.push(PyBody {
            func: n,
            id,
            method: false,
        });
    }

    /// Go `pyClass`: takes the decorated_definition around n, or `None`.
    fn py_class(&mut self, n: Node<'t>, deco: Option<Node<'t>>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let class_id = self.b.id(&[&name]);
        self.b.def(&class_id, &name, &name, line(n));
        self.py_decorators(&class_id, deco, src);
        self.py_superclasses(&class_id, n, src);

        let Some(body) = n.child_by_field_name("body") else {
            return;
        };
        for i in 0..body.child_count() {
            let Some(mut m) = body.child(i) else { continue };
            let mut mdeco = None;
            if m.kind() == "decorated_definition" {
                mdeco = Some(m);
                let Some(d) = m.child_by_field_name("definition") else {
                    continue;
                };
                m = d;
            }
            if m.kind() != "function_definition" {
                py_import_names(m, src, &mut |local, val| {
                    py_in_func_alias(&mut self.b, local, val, line(m))
                });
                continue;
            }
            let mname = field_text(m, "name", src);
            if mname.is_empty() {
                continue;
            }
            let mid = self.b.id(&[&name, &mname]);
            self.b.add_node(&mid, &format!("{name}.{mname}()"), line(m));
            self.b.res.edges.push(Edge {
                source: class_id.clone(),
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
            self.py_refs(&mid, m, mdeco, src);
            self.py_bodies.push(PyBody {
                func: m,
                id: mid,
                method: true,
            });
        }
    }

    /// Go `pyImports`: records each imported module. `import a.b` and `from a.b import c`
    /// both record the module path a.b; a from-import also keeps its imported names, since
    /// `from . import b` names a module. Resolve binds them to corpus files.
    fn py_imports(&mut self, n: Node<'t>, src: &[u8], type_only: bool) {
        match n.kind() {
            "import_statement" => {
                for i in 0..n.child_count() {
                    let Some(c) = n.child(i) else { continue };
                    match c.kind() {
                        "dotted_name" => self.b.imp_typed(&node_text(c, src), line(n), type_only),
                        "aliased_import" => {
                            if let Some(name) = c.child_by_field_name("name") {
                                self.b.imp_typed(&node_text(name, src), line(n), type_only);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "import_from_statement" => {
                let Some(module) = n.child_by_field_name("module_name") else {
                    return;
                };
                self.b
                    .imp_typed(&node_text(module, src), line(n), type_only);
                if let Some(imp) = self.b.res.imps.last_mut() {
                    for i in 0..n.child_count() {
                        if n.field_name_for_child(i) != Some("name") {
                            continue;
                        }
                        let Some(mut c) = n.child(i) else { continue };
                        if c.kind() == "aliased_import" {
                            let Some(name) = c.child_by_field_name("name") else {
                                continue;
                            };
                            c = name;
                        }
                        imp.names.push(node_text(c, src).into_owned());
                    }
                }
                self.py_import_aliases(n, module, src);
            }
            _ => {}
        }
    }

    /// Go `pyImportAliases`: captures `from M import N [as L]` evidence for the import-guided
    /// call resolver. stem is the final component of the module name; each imported name
    /// records local -> imported under that stem. `import *` (wildcard_import, no name field)
    /// carries no usable alias and is skipped.
    fn py_import_aliases(&mut self, n: Node<'t>, module: Node<'t>, src: &[u8]) {
        let stem = py_module_stem(module, src); // "" for `from . import N`
        let loc = line(n);
        for i in 0..n.child_count() {
            if n.field_name_for_child(i) != Some("name") {
                continue;
            }
            let Some(c) = n.child(i) else { continue };
            let (local, imported);
            match c.kind() {
                "dotted_name" => {
                    imported = node_text(c, src).into_owned();
                    local = imported.clone();
                }
                "aliased_import" => {
                    let (Some(name), Some(alias)) = (
                        c.child_by_field_name("name"),
                        c.child_by_field_name("alias"),
                    ) else {
                        continue;
                    };
                    imported = node_text(name, src).into_owned();
                    local = node_text(alias, src).into_owned();
                }
                _ => continue,
            }
            if local.is_empty() || imported.is_empty() {
                continue;
            }
            self.b.res.import_aliases.push(ImportAlias {
                local,
                imported,
                module_stem: stem.clone(),
                spec: node_text(module, src).into_owned(),
                loc,
                ..ImportAlias::default()
            });
        }
    }

    /// Go `pyCalls`: records each call site in the body of function `func`. Direct calls
    /// (`f()`) record the identifier; attribute calls (`x.f()`) record the attribute name, with
    /// the receiver's class in place of a typed local. One walk of `func` collects the local
    /// types, the bound names and the call sites. A def inside `func` is a node the function
    /// around it contains, and the caller of the calls in its own body. `ext` is false when no
    /// call of `func` can name an external module: upstream gives that edge to a function of the
    /// file alone, and to no function of a file pyDynamic holds for.
    // ponytail: the types and bound names are one flat scope for fn and the defs
    // inside it; add scopes if a repo shows wrong edges.
    fn py_calls(&mut self, func: Node<'t>, caller_id: &str, ext: bool, src: &[u8]) {
        let Some(body) = func.child_by_field_name("body") else {
            return;
        };
        let params = func.child_by_field_name("parameters");
        fn within(c: Node<'_>, of: Option<Node<'_>>) -> bool {
            of.is_some_and(|of| c.start_byte() >= of.start_byte() && c.end_byte() <= of.end_byte())
        }
        let mut types: HashMap<String, String> = HashMap::new();
        let mut bound: HashSet<String> = HashSet::new();
        // function, name -> what an import inside it binds
        let mut mods: HashMap<(String, String), String> = HashMap::new();
        // ids of the defs inside fn
        let mut made: HashSet<String> = HashSet::new();
        // The defs inside fn by function and name, and the function around each. They are of
        // this walk alone: a later function can have the id of fn.
        let mut nested: HashMap<(String, String), String> = HashMap::new();
        let mut parent: HashMap<String, String> = HashMap::new();
        // Types and bound names cover the whole function, so a receiver is resolved only after
        // the walk.
        struct Site {
            caller: String,
            callee: String,
            recv: String,
            loc: Loc,
            member: bool,
            local: bool,
        }
        let mut sites: Vec<Site> = Vec::new();
        let mut scopes = vec![PyScope {
            id: caller_id.to_string(),
            func: caller_id.to_string(),
            outer: String::new(),
            end: func.end_byte(),
            body_lo: body.start_byte(),
            body_hi: body.end_byte(),
            local: false,
        }];
        let mask = *PY_CALL_KINDS.get().expect("register_kinds");
        walk_kinds(Some(func), mask, |c| {
            py_local_typer(c, &mut types, src);
            while scopes.len() > 1 && c.start_byte() >= scopes[scopes.len() - 1].end {
                scopes.pop();
            }
            let inn = scopes[scopes.len() - 1].clone();
            if within(c, Some(body))
                && py_import_names(c, src, &mut |local, val| {
                    py_in_func_alias(&mut self.b, local, val, line(c));
                    // A name of a class body is no name of the functions.
                    if inn.id != inn.func {
                        return;
                    }
                    let key = (inn.func.clone(), local.to_string());
                    let mut val = val.to_string();
                    if mods.get(&key).is_some_and(|old| *old != val) {
                        val.clear();
                    }
                    mods.insert(key, val);
                })
            {
                return true;
            }
            if within(c, Some(body)) || within(c, params) {
                py_bind(c, src, true, &mut bound);
            }
            if matches!(c.kind(), "function_definition" | "class_definition") {
                let name = field_text(c, "name", src);
                let cbody = c.child_by_field_name("body");
                let Some(cbody) =
                    cbody.filter(|_| c.start_byte() != func.start_byte() && !name.is_empty())
                else {
                    return true;
                };
                let mut sc = inn.clone();
                sc.id = make_id(&[&inn.id, &name]);
                sc.end = c.end_byte();
                if c.kind() == "function_definition" {
                    // ponytail: one more part keeps the def off the id of a
                    // module-level def or method `outer_name`; a def of that
                    // longer name still collides.
                    if self.b.seen.contains(&sc.id) && !made.contains(&sc.id) {
                        sc.id = make_id(&[&inn.id, &name, "nested"]);
                    }
                    sc.func = sc.id.clone();
                    sc.outer = inn.func.clone();
                    sc.body_lo = cbody.start_byte();
                    sc.body_hi = cbody.end_byte();
                    sc.local = inn.local || inn.id != inn.func;
                    if !self.b.seen.contains(&sc.id) {
                        made.insert(sc.id.clone());
                        self.b.add_node(&sc.id, &format!("{name}()"), line(c));
                        self.b.res.edges.push(Edge {
                            source: inn.func.clone(),
                            target: sc.id.clone(),
                            relation: Relation::CONTAINS,
                            confidence: Confidence::EXTRACTED,
                            source_file: self.b.src,
                            source_location: line(c),
                            ..Edge::default()
                        });
                    }
                    // A method of a local class is no name of the scope around it. Two names
                    // of one id (`_f`, `f`) both reach its node.
                    parent.insert(sc.id.clone(), inn.func.clone());
                    if inn.id == inn.func {
                        nested.insert((inn.func.clone(), name), sc.id.clone());
                    }
                }
                scopes.push(sc);
                return true;
            }
            if c.kind() != "call" {
                return true;
            }
            let mut caller = inn.func.clone();
            if c.start_byte() < inn.body_lo || c.end_byte() > inn.body_hi {
                // A default or an annotation of a def runs in the function the def is made in;
                // fn itself has none here.
                caller = inn.outer.clone();
                if caller.is_empty() {
                    return true;
                }
            }
            let Some(f) = c.child_by_field_name("function") else {
                return true;
            };
            match f.kind() {
                "identifier" => sites.push(Site {
                    caller,
                    callee: node_text(f, src).into_owned(),
                    recv: String::new(),
                    loc: line(c),
                    member: false,
                    local: false,
                }),
                "attribute" => {
                    if let Some(a) = f.child_by_field_name("attribute") {
                        sites.push(Site {
                            caller,
                            callee: node_text(a, src).into_owned(),
                            recv: recv_text(Some(f), Some(a), src),
                            loc: line(c),
                            member: true,
                            local: inn.local,
                        });
                    }
                }
                _ => {}
            }
            true
        });
        // imported is what name names by an import no other binding hides: one inside the
        // caller or a function around it before one of the module.
        let imported = |caller: &str, name: &str| -> (String, bool) {
            if bound.contains(name) {
                return (String::new(), false);
            }
            for s in py_chain(&parent, caller) {
                if let Some(val) = mods.get(&(s.to_string(), name.to_string())) {
                    return (val.clone(), true);
                }
            }
            let val = self
                .py_mods
                .as_ref()
                .and_then(|m| m.get(name))
                .cloned()
                .unwrap_or_default();
            (val, false)
        };
        'sites: for c in &sites {
            let n = self.b.res.calls.len();
            let mut modname;
            if !c.member {
                // A def of an enclosing function hides every other of its name.
                for s in py_chain(&parent, &c.caller) {
                    if let Some(id) = nested.get(&(s.to_string(), c.callee.clone())) {
                        super::javascript::js_nested_call(
                            &mut self.b,
                            &mut self.js_linked,
                            &c.caller,
                            id,
                            c.loc,
                        );
                        continue 'sites;
                    }
                }
                let (val, in_fn) = imported(&c.caller, &c.callee);
                // A builtin names nothing of another file unless an import or a def of this
                // file has its name. A star import can bind any name.
                if val.is_empty()
                    && PY_BUILTINS.contains(c.callee.as_str())
                    && self.py_mods.is_some()
                    && !self.py_own.contains(&c.callee)
                {
                    // An import of a function around the call can name it even when another
                    // binding hides which one does.
                    let own = py_chain(&parent, &c.caller)
                        .any(|s| mods.contains_key(&(s.to_string(), c.callee.clone())));
                    if !own {
                        continue;
                    }
                }
                self.b.call(&c.caller, &c.callee, c.loc);
                modname = if val.contains(':') {
                    val
                } else {
                    String::new()
                };
                // An import in the function binds the callee to that and to nothing else, even
                // when it names no definition.
                if in_fn {
                    modname.push(':');
                }
            } else {
                let mut recv = c.recv.clone();
                // The self of a local class has no node to bind on.
                if is_self_recv(&recv) && c.local {
                    continue;
                }
                let (val, in_fn) = imported(&c.caller, &recv);
                // Upstream gives a call on an external module to a top-level function, and to a
                // plain module-level import, alone.
                modname = val.clone();
                if !modname.is_empty()
                    && (!ext || in_fn || c.caller != caller_id || val.contains(':'))
                {
                    modname.push(':');
                }
                if let Some(t) = types.get(&recv).filter(|t| !t.is_empty()) {
                    recv = t.clone();
                }
                self.b.call_recv(&c.caller, &c.callee, &recv, c.loc);
            }
            if self.b.res.calls.len() > n && !modname.is_empty() {
                while self.b.res.call_mods.len() < n {
                    self.b.res.call_mods.push(String::new());
                }
                self.b.res.call_mods.push(modname);
            }
        }
    }

    /// Go `pySuperclasses`: records an inherits edge per positional base class. Keyword
    /// arguments (`metaclass=M`) are not bases and are skipped.
    fn py_superclasses(&mut self, class_id: &str, n: Node<'t>, src: &[u8]) {
        let Some(args) = n.child_by_field_name("superclasses") else {
            return;
        };
        for i in 0..args.named_child_count() {
            let Some(arg) = args.named_child(i) else {
                continue;
            };
            if arg.kind() == "keyword_argument" {
                continue;
            }
            // Only a plain name gets a stub when no definition binds it.
            if arg.kind() == "identifier" {
                self.b
                    .ref_(class_id, &node_text(arg, src), "inherits", false, line(n));
                continue;
            }
            let before = self.b.res.type_refs.len();
            self.b.type_ref(
                class_id,
                &base_type_name(&node_text(arg, src)),
                "inherits",
                line(n),
            );
            py_qual(&mut self.b, before, arg, src);
        }
    }

    /// Go `pyRefs`: records a references edge from the function or method id to each type its
    /// parameter and return annotations name, and to its decorators.
    fn py_refs(&mut self, id: &str, func: Node<'t>, deco: Option<Node<'t>>, src: &[u8]) {
        self.py_decorators(id, deco, src);
        if let Some(params) = func.child_by_field_name("parameters") {
            for i in 0..params.named_child_count() {
                if let Some(p) = params.named_child(i)
                    && matches!(p.kind(), "typed_parameter" | "typed_default_parameter")
                {
                    self.py_type_names(id, p.child_by_field_name("type"), src, line(func));
                }
            }
        }
        self.py_type_names(id, func.child_by_field_name("return_type"), src, line(func));
    }

    /// Go `pyTypeNames`: walks an annotation. A string annotation names nothing.
    fn py_type_names(&mut self, id: &str, n: Option<Node<'t>>, src: &[u8], loc: Loc) {
        let Some(n) = n else { return };
        match n.kind() {
            "identifier" | "attribute" => {
                // A module variable made by TypeVar is a type parameter, not a type.
                let name = base_type_name(&node_text(n, src));
                if !PY_TYPE_SKIP.contains(name.as_str()) && !self.py_type_vars.contains(&name) {
                    let before = self.b.res.type_refs.len();
                    self.b.ref_(id, &name, "references", false, loc);
                    py_qual(&mut self.b, before, n, src);
                }
                return;
            }
            "keyword_argument" => {
                // Annotated[int, Field(gt=0)]: gt is no type.
                self.py_type_names(id, n.child_by_field_name("value"), src, loc);
                return;
            }
            _ => {}
        }
        for i in 0..n.named_child_count() {
            self.py_type_names(id, n.named_child(i), src, loc);
        }
    }

    /// Go `pyDecorators`: records a references edge from id to each decorator of the
    /// decorated_definition n.
    fn py_decorators(&mut self, id: &str, n: Option<Node<'t>>, src: &[u8]) {
        let Some(n) = n else { return };
        for i in 0..n.named_child_count() {
            let Some(d) = n.named_child(i) else { continue };
            if d.kind() != "decorator" || d.named_child_count() == 0 {
                continue;
            }
            let Some(mut expr) = d.named_child(0) else {
                continue;
            };
            // @x.setter is the property protocol.
            let text = node_text(expr, src);
            if text.ends_with(".setter") || text.ends_with(".getter") || text.ends_with(".deleter")
            {
                continue;
            }
            if expr.kind() == "call"
                && let Some(f) = expr.child_by_field_name("function")
            {
                expr = f;
            }
            let attr = expr;
            let mut name_node = Some(expr);
            if expr.kind() == "attribute" {
                name_node = expr.child_by_field_name("attribute");
            }
            let Some(name_node) = name_node.filter(|e| e.kind() == "identifier") else {
                continue;
            };
            let name = node_text(name_node, src);
            if !PY_DECO_SKIP.contains(&*name) {
                let before = self.b.res.type_refs.len();
                self.b.ref_(id, &name, "references", true, line(d));
                py_qual(&mut self.b, before, attr, src);
            }
        }
    }
}

/// Go `(*builder).pyRationale` (`python.go:721`), a function of the `Builder` alone: a
/// deterministic post-pass mirroring upstream `_extract_python_rationale`: it captures
/// module/class/function docstrings and `# NOTE:`-style comments as "rationale" nodes, each with a
/// rationale_for edge to the file or the enclosing definition. No LLM is involved.
pub(crate) fn py_rationale(b: &mut Builder, root: Node<'_>, src: &[u8]) {
    let file_id = b.file_id.clone();
    // Module docstring: skipped for auto-generated files (Alembic, Django migrations, protobuf
    // stubs) whose module docstring is a revision annotation, not architectural rationale.
    if !is_autogenerated_python(src)
        && let Some((text, ln)) = py_docstring(Some(root), src)
    {
        b.add_rationale(&text, ln, &file_id);
    }
    // Class and function docstrings.
    py_walk_docstrings(b, root, &file_id, src);

    // Rationale comments (# NOTE:, # IMPORTANT:, ...).
    for (idx, raw) in src.split(|&c| c == b'\n').enumerate() {
        let t = bytes_trim_space(raw);
        if t.first() == Some(&b'#') {
            let stripped = go_text(t);
            if PY_RATIONALE_PREFIXES
                .iter()
                .any(|p| stripped.starts_with(*p))
            {
                b.add_rationale(&stripped, idx as i64 + 1, &file_id);
            }
        }
    }
}

/// Go `pyWalkDocstrings`: recurses the AST attaching each class/function docstring to its
/// definition node (rationale_for), mirroring upstream walk_docstrings. It descends into class
/// bodies but not function bodies, so nested definitions inside a method are not visited
/// (matching upstream).
fn py_walk_docstrings(b: &mut Builder, n: Node<'_>, parent_id: &str, src: &[u8]) {
    let mask = *PY_DEF_KINDS.get().expect("register_kinds");
    walk_kinds(Some(n), mask, |n| {
        match n.kind() {
            "class_definition" => {
                let name = field_text(n, "name", src);
                let body = n.child_by_field_name("body");
                if let Some(body) = body.filter(|_| !name.is_empty()) {
                    let nid = b.id(&[&name]);
                    if let Some((text, ln)) = py_docstring(Some(body), src) {
                        b.add_rationale(&text, ln, &nid);
                    }
                    for i in 0..body.child_count() {
                        if let Some(c) = body.child(i) {
                            py_walk_docstrings(b, c, &nid, src);
                        }
                    }
                }
                return false;
            }
            "function_definition" => {
                let name = field_text(n, "name", src);
                let body = n.child_by_field_name("body");
                if let Some(body) = body.filter(|_| !name.is_empty()) {
                    let mut nid = b.id(&[&name]);
                    if parent_id != b.file_id {
                        nid = make_id(&[parent_id, &name]);
                    }
                    if let Some((text, ln)) = py_docstring(Some(body), src) {
                        b.add_rationale(&text, ln, &nid);
                    }
                }
                return false;
            }
            _ => {}
        }
        true
    });
}

/// Go `pyModuleStem`: the final component of a `from ... import` module name, mirroring upstream
/// `_module_stem`: `util.math` -> `math`, `.helper` -> `helper`.
fn py_module_stem(module: Node<'_>, src: &[u8]) -> String {
    py_spec_stem(&node_text(module, src))
}

/// Go `pySpecStem`.
fn py_spec_stem(spec: &str) -> String {
    let text = spec.trim_matches('.');
    match text.rfind('.') {
        Some(i) => text[i + 1..].to_string(),
        None => text.to_string(),
    }
}

/// Go `pyBuiltins`: the functions and types of the Python builtins module. A bare call of one
/// names the builtin unless its file defines or imports the name. exit and quit are those of
/// the site module.
static PY_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    set(&[
        "abs",
        "aiter",
        "all",
        "anext",
        "any",
        "ascii",
        "bin",
        "bool",
        "breakpoint",
        "bytearray",
        "bytes",
        "callable",
        "chr",
        "classmethod",
        "compile",
        "complex",
        "delattr",
        "dict",
        "dir",
        "divmod",
        "enumerate",
        "eval",
        "exec",
        "filter",
        "float",
        "format",
        "frozenset",
        "getattr",
        "globals",
        "hasattr",
        "hash",
        "help",
        "hex",
        "id",
        "input",
        "int",
        "isinstance",
        "issubclass",
        "iter",
        "len",
        "list",
        "locals",
        "map",
        "max",
        "memoryview",
        "min",
        "next",
        "object",
        "oct",
        "open",
        "ord",
        "pow",
        "print",
        "property",
        "range",
        "repr",
        "reversed",
        "round",
        "set",
        "setattr",
        "slice",
        "sorted",
        "staticmethod",
        "str",
        "sum",
        "super",
        "tuple",
        "type",
        "vars",
        "zip",
        "__import__",
        "exit",
        "quit",
    ])
});

/// Go `pyClassName`: matches a bare capitalized name, the only annotation or callee taken as a
/// class.
static PY_CLASS_NAME: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"^[A-Z]\w*$"));

/// Go `pyLocalTyper` (`python.go:259`), as a function of the node: run over every node of a
/// function, it maps the locals to their class in `types`, from a parameter annotation
/// (`c: Client`) or a constructor binding (`c = Client()`). A local also bound by anything else
/// (assignment, unpacking, for, with, except, `:=`) maps to "".
fn py_local_typer(c: Node<'_>, types: &mut HashMap<String, String>, src: &[u8]) {
    let mut name: Option<Node<'_>> = None;
    match c.kind() {
        "typed_parameter" => name = c.named_child(0),
        "typed_default_parameter" => name = c.child_by_field_name("name"),
        "assignment" => {
            name = c.child_by_field_name("left");
            if let Some(nm) = name.filter(|nm| nm.kind() != "identifier") {
                py_untype(Some(nm), types, src);
            }
        }
        "for_statement" | "for_in_clause" => {
            py_untype(c.child_by_field_name("left"), types, src);
        }
        "as_pattern_target" => py_untype(Some(c), types, src),
        "named_expression" => py_untype(c.child_by_field_name("name"), types, src),
        _ => {}
    }
    let Some(name) = name.filter(|nm| nm.kind() == "identifier") else {
        return;
    };
    let mut typ = field_text(c, "type", src);
    if let Some(r) = c.child_by_field_name("right").filter(|_| typ.is_empty()) {
        if r.kind() == "none" {
            return;
        }
        if r.kind() == "call" {
            typ = field_text(r, "function", src);
        }
    }
    if !PY_CLASS_NAME.match_string(&typ) {
        typ = String::new();
    }
    bind_type(types, &node_text(name, src), &typ);
}

/// Go `untype` (inside `pyLocalTyper`): clears every name a binding target rebinds.
fn py_untype(n: Option<Node<'_>>, types: &mut HashMap<String, String>, src: &[u8]) {
    let Some(n) = n else { return };
    match n.kind() {
        "identifier" => bind_type(types, &node_text(n, src), ""),
        "pattern_list" | "tuple_pattern" | "list_pattern" | "list_splat_pattern"
        | "as_pattern_target" => {
            for i in 0..n.named_child_count() {
                py_untype(n.named_child(i), types, src);
            }
        }
        _ => {}
    }
}

/// Go `pyModules`: maps each local name bound by exactly one module-level import, and bound
/// nowhere else at module scope, to what it names (`py_import_names`). `None` is Go's nil map.
fn py_modules(root: Node<'_>, src: &[u8]) -> Option<HashMap<String, String>> {
    let mut mods: HashMap<String, String> = HashMap::new();
    let mut bound: HashSet<String> = HashSet::new();
    for i in 0..root.child_count() {
        let Some(st) = root.child(i) else { continue };
        let named = py_import_names(st, src, &mut |local, val| {
            if mods.contains_key(local) || val.is_empty() {
                bound.insert(local.to_string());
            }
            mods.insert(local.to_string(), val.to_string());
        });
        if !named {
            py_binds(st, src, false, &mut bound);
        }
    }
    // `from m import *` can bind any name.
    if bound.contains("*") {
        return None;
    }
    for name in &bound {
        mods.remove(name);
    }
    Some(mods)
}

/// Go `pyImportNames`: calls `f` for each name the import statement st binds, with what it
/// names: "a.b" for the module of `import a.b as x`, "a.b:n" for the n of `from a.b import n`,
/// and "" for `import a.b`, which binds a alone. It returns false, and calls nothing, for
/// another statement or a star import.
fn py_import_names(st: Node<'_>, src: &[u8], f: &mut impl FnMut(&str, &str)) -> bool {
    let mut from = String::new();
    match st.kind() {
        "import_statement" => {}
        "import_from_statement" => {
            from = field_text(st, "module_name", src) + ":";
            for i in 0..st.child_count() {
                if st.child(i).is_some_and(|c| c.kind() == "wildcard_import") {
                    return false;
                }
            }
        }
        _ => return false,
    }
    for i in 0..st.child_count() {
        if st.field_name_for_child(i) != Some("name") {
            continue;
        }
        let Some(c) = st.child(i) else { continue };
        match c.kind() {
            "dotted_name" => {
                // `import a.b` binds a, which is not the module imported.
                let mut local = c
                    .named_child(0)
                    .map(|x| node_text(x, src).into_owned())
                    .unwrap_or_default();
                let mut val = String::new();
                if c.named_child_count() == 1 || !from.is_empty() {
                    local = node_text(c, src).into_owned();
                    val = format!("{from}{local}");
                }
                f(&local, &val);
            }
            "aliased_import" => f(
                &field_text(c, "alias", src),
                &format!("{from}{}", field_text(c, "name", src)),
            ),
            _ => {}
        }
    }
    true
}

// The kinds pyBind acts on, then those of the pyCalls walk: pyBind's, the local typer's and the
// call itself.
static PY_GLOBAL_KINDS: OnceLock<u32> = OnceLock::new();
static PY_BIND_KINDS: OnceLock<u32> = OnceLock::new();
static PY_CALL_KINDS: OnceLock<u32> = OnceLock::new();
static PY_DEF_KINDS: OnceLock<u32> = OnceLock::new();
static PY_DYN_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `pyBinds`: adds the names bound under n to bound. Unless deep, the body of a def, class or
/// lambda is skipped: it is another scope, and only a `global` reaches out of it.
fn py_binds(n: Node<'_>, src: &[u8], deep: bool, bound: &mut HashSet<String>) {
    let mask = *PY_BIND_KINDS.get().expect("register_kinds");
    walk_kinds(Some(n), mask, |c| py_bind(c, src, deep, bound));
}

/// Go `pyBind`: `py_binds` for the single node c; false means skip its children.
fn py_bind(c: Node<'_>, src: &[u8], deep: bool, bound: &mut HashSet<String>) -> bool {
    match c.kind() {
        "function_definition" | "class_definition" | "lambda" => {
            py_targets(c.child_by_field_name("name"), src, bound);
            if !deep {
                let mask = *PY_GLOBAL_KINDS.get().expect("register_kinds");
                walk_kinds(Some(c), mask, |gv| {
                    py_targets(Some(gv), src, bound);
                    true
                });
                return false;
            }
        }
        "parameters" | "lambda_parameters" => {
            for i in 0..c.named_child_count() {
                let Some(p) = c.named_child(i) else { continue };
                let p = if let Some(name) = p.child_by_field_name("name") {
                    Some(name)
                } else if p.kind() == "typed_parameter" {
                    p.named_child(0)
                } else {
                    Some(p)
                };
                py_targets(p, src, bound);
            }
        }
        "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause" => {
            py_targets(c.child_by_field_name("left"), src, bound);
        }
        "named_expression" => py_targets(c.child_by_field_name("name"), src, bound),
        "as_pattern" => py_targets(c.child_by_field_name("alias"), src, bound),
        "global_statement" | "nonlocal_statement" | "delete_statement" | "case_pattern" => {
            py_targets(Some(c), src, bound);
        }
        "import_statement" | "import_from_statement" => {
            for i in 0..c.child_count() {
                if c.field_name_for_child(i) != Some("name") {
                    continue;
                }
                // `import a.b` binds a alone.
                let Some(name) = c.child(i) else { continue };
                let name = match name.child_by_field_name("alias") {
                    Some(alias) => Some(alias),
                    None => name.named_child(0),
                };
                py_targets(name, src, bound);
            }
        }
        "wildcard_import" => {
            bound.insert("*".to_string());
        }
        _ => {}
    }
    true
}

/// Go `pyTargets`: adds the names an assignment target t rebinds or mutates: the identifiers of
/// a pattern, and the root object of an attribute or subscript.
fn py_targets(t: Option<Node<'_>>, src: &[u8], bound: &mut HashSet<String>) {
    let Some(t) = t else { return };
    match t.kind() {
        "identifier" => {
            bound.insert(node_text(t, src).into_owned());
        }
        "attribute" => py_targets(t.child_by_field_name("object"), src, bound),
        "subscript" => py_targets(t.child_by_field_name("value"), src, bound),
        _ => {
            for i in 0..t.named_child_count() {
                py_targets(t.named_child(i), src, bound);
            }
        }
    }
}

/// Go `pyDynNames`: the identifiers upstream takes as a way to rebind a module name without an
/// assignment (`_python_receiver_shadow_checker`).
static PY_DYN_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    set(&[
        "exec", "eval", "globals", "locals", "vars", "setattr", "delattr",
    ])
});

/// Go `pyDynamic`: whether the file has a global or nonlocal statement or an identifier of
/// `PY_DYN_NAMES`, anywhere. Upstream then gives no call the node of an external module.
fn py_dynamic(root: Node<'_>, src: &[u8]) -> bool {
    let mut found = false;
    let mask = *PY_DYN_KINDS.get().expect("register_kinds");
    walk_kinds(Some(root), mask, |c| {
        found = found || c.kind() != "identifier" || PY_DYN_NAMES.contains(&*node_text(c, src));
        !found
    });
    found
}

/// Go `pyInFuncAlias`: records the name a from-import inside a function or class binds, for the
/// edge from the file to what it names.
fn py_in_func_alias(b: &mut Builder, local: &str, val: &str, loc: Loc) {
    if let Some((module, name)) = val.split_once(':') {
        b.res.import_aliases.push(ImportAlias {
            local: local.to_string(),
            imported: name.to_string(),
            spec: module.to_string(),
            loc,
            in_func: true,
            ..ImportAlias::default()
        });
    }
}

/// Go's `for s, ok := start, true; ok; s, ok = parent[s]` walk up the enclosing functions.
fn py_chain<'a>(
    parent: &'a HashMap<String, String>,
    start: &'a str,
) -> impl Iterator<Item = &'a str> {
    std::iter::successors(Some(start), move |s| parent.get(*s).map(String::as_str))
}

/// Whether `recv` is a key of Go `selfRecv` (`resolve.go:1187`): a receiver that names the
/// caller's own instance or type, or its parent.
fn is_self_recv(recv: &str) -> bool {
    matches!(
        recv,
        "self"
            | "this"
            | "cls"
            | "$this"
            | "@self"
            | "Self"
            | "static"
            | "super"
            | "super()"
            | "parent"
            | "base"
    )
}

/// Go `pyRationalePrefixes`: the leading comment tokens that mark an explanatory comment worth
/// capturing as a rationale node (mirrors upstream `_RATIONALE_PREFIXES`).
const PY_RATIONALE_PREFIXES: [&str; 7] = [
    "# NOTE:",
    "# IMPORTANT:",
    "# HACK:",
    "# WHY:",
    "# RATIONALE:",
    "# TODO:",
    "# FIXME:",
];

/// Go `pyRevisionRe`: matches an Alembic/Flask-Migrate `revision = "..."` header line.
static PY_REVISION_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"(?m)^revision\s*[:=]"));

/// Go `pyDocstring`: a module/class/function body's leading docstring and its 1-based line. It
/// mirrors upstream `_get_docstring`: only the first statement is inspected, it must be a bare
/// string expression, and the unquoted text must exceed 20 characters.
fn py_docstring(body: Option<Node<'_>>, src: &[u8]) -> Option<(String, i64)> {
    let body = body?;
    // Find the first non-comment statement. tree-sitter includes comment nodes as body
    // children, whereas upstream's AST body[0] omits them; skip leading comments so a comment
    // before the docstring does not hide it.
    let mut stmt = None;
    for i in 0..body.child_count() {
        let Some(child) = body.child(i) else { continue };
        if child.kind() == "comment" {
            continue;
        }
        stmt = Some(child);
        break;
    }
    let stmt = stmt.filter(|s| s.kind() == "expression_statement")?;
    for j in 0..stmt.child_count() {
        let Some(sub) = stmt.child(j) else { continue };
        if matches!(sub.kind(), "string" | "concatenated_string") {
            let text = strip_docstring(&node_text(sub, src));
            if text.chars().count() > 20 {
                return Some((text, i64::from(stmt.start_position().row) + 1));
            }
        }
    }
    None
}

/// Go `stripDocstring`: peels surrounding quote characters and whitespace off a raw tree-sitter
/// string node, mirroring upstream's strip chain.
fn strip_docstring(s: &str) -> String {
    let s = s.trim_matches(['"', '\'']);
    let s = s.trim_matches('"');
    let s = s.trim_matches('\'');
    trim_space(s).to_string()
}

/// Go `isAutogeneratedPython`: whether a file's first 2 KiB marks it as generated
/// (protobuf/gRPC/OpenAPI) or a migration/revision (Alembic, Django), whose module docstring is
/// boilerplate rather than rationale. Mirrors upstream `_is_autogenerated_python`.
fn is_autogenerated_python(src: &[u8]) -> bool {
    let head = &src[..src.len().min(2048)];
    let h = go_text(head);
    for m in [
        "DO NOT EDIT",
        "@generated",
        "Generated by the protocol buffer",
    ] {
        if h.contains(m) {
            return true;
        }
    }
    if h.contains("def upgrade(") && h.contains("down_revision") && PY_REVISION_RE.match_string(&h)
    {
        return true;
    }
    h.contains("class Migration(migrations.Migration)") && h.contains("operations")
}

/// Go `pyTypeSkip`: the names an annotation mentions that are not corpus symbols: typing
/// containers, scalar builtins and unittest.mock.
static PY_TYPE_SKIP: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    set(&[
        "list",
        "dict",
        "set",
        "tuple",
        "frozenset",
        "type",
        "List",
        "Dict",
        "Set",
        "Tuple",
        "FrozenSet",
        "Type",
        "Optional",
        "Union",
        "Sequence",
        "Iterable",
        "Mapping",
        "MutableMapping",
        "Iterator",
        "Callable",
        "Awaitable",
        "AsyncIterable",
        "AsyncIterator",
        "Coroutine",
        "Generator",
        "AsyncGenerator",
        "ContextManager",
        "AsyncContextManager",
        "Annotated",
        "ClassVar",
        "Final",
        "Literal",
        "Concatenate",
        "ParamSpec",
        "TypeVar",
        "None",
        "Ellipsis",
        "Self",
        "str",
        "int",
        "float",
        "bool",
        "bytes",
        "bytearray",
        "complex",
        "object",
        "True",
        "False",
        "MagicMock",
        "Mock",
        "AsyncMock",
        "NonCallableMock",
        "NonCallableMagicMock",
        "PropertyMock",
        "patch",
        "sentinel",
    ])
});

/// Go `pyDecoSkip`: the builtin and stdlib decorators.
static PY_DECO_SKIP: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    set(&[
        "property",
        "staticmethod",
        "classmethod",
        "abstractmethod",
        "abstractproperty",
        "cached_property",
        "wraps",
        "lru_cache",
        "cache",
        "singledispatch",
        "singledispatchmethod",
        "total_ordering",
        "contextmanager",
        "asynccontextmanager",
        "overload",
        "override",
        "final",
        "no_type_check",
        "runtime_checkable",
        "dataclass",
    ])
});

/// Go `set` (`python.go:901`): a lookup set of `names` (Go's `map[string]bool` of trues), shared
/// by golang.go and javascript.go.
pub(crate) fn set(names: &[&'static str]) -> HashSet<&'static str> {
    names.iter().copied().collect()
}

/// Go `pyQual`: marks the type reference added since `before` as qualified when n is `q.Name`,
/// `q.Name[...]` or `q.Name(...)`.
fn py_qual(b: &mut Builder, before: usize, n: Node<'_>, src: &[u8]) {
    let n = match n.kind() {
        "subscript" => n.child_by_field_name("value"),
        "call" => n.child_by_field_name("function"),
        _ => Some(n),
    };
    let Some(n) = n.filter(|n| n.kind() == "attribute") else {
        return;
    };
    if b.res.type_refs.len() == before {
        return;
    }
    let Some(o) = n.child_by_field_name("object") else {
        return;
    };
    if let Some(last) = b.res.type_refs.last_mut() {
        last.qual = node_text(o, src).into_owned();
    }
}

/// Go `pyTypeVarNames`: the module-level names bound by `T = TypeVar(...)`, ParamSpec or
/// TypeVarTuple.
fn py_type_var_names(root: Node<'_>, src: &[u8]) -> HashSet<String> {
    let mut out = HashSet::new();
    for i in 0..root.named_child_count() {
        let Some(a) = root.named_child(i) else {
            continue;
        };
        if a.kind() != "expression_statement" || a.named_child_count() != 1 {
            continue;
        }
        let Some(a) = a.named_child(0) else { continue };
        let (Some(l), Some(r)) = (
            a.child_by_field_name("left"),
            a.child_by_field_name("right"),
        ) else {
            continue;
        };
        if a.kind() != "assignment" || l.kind() != "identifier" || r.kind() != "call" {
            continue;
        }
        if matches!(
            base_type_name(&field_text(r, "function", src)).as_str(),
            "TypeVar" | "ParamSpec" | "TypeVarTuple"
        ) {
            out.insert(node_text(l, src).into_owned());
        }
    }
    out
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let lang = gositter_grammars::python::language();
    let _ = PY_GLOBAL_KINDS.set(lang.kind_mask(&["global_statement"]));
    let bind = lang.kind_mask(&[
        "function_definition",
        "class_definition",
        "lambda",
        "parameters",
        "lambda_parameters",
        "assignment",
        "augmented_assignment",
        "for_statement",
        "for_in_clause",
        "named_expression",
        "as_pattern",
        "global_statement",
        "nonlocal_statement",
        "delete_statement",
        "case_pattern",
        "import_statement",
        "import_from_statement",
        "wildcard_import",
    ]);
    let _ = PY_BIND_KINDS.set(bind);
    let _ = PY_CALL_KINDS.set(
        bind | lang.kind_mask(&[
            "typed_parameter",
            "typed_default_parameter",
            "as_pattern_target",
            "call",
        ]),
    );
    let _ = PY_DEF_KINDS.set(lang.kind_mask(&["function_definition", "class_definition"]));
    let _ =
        PY_DYN_KINDS.set(lang.kind_mask(&["global_statement", "nonlocal_statement", "identifier"]));
}

#[cfg(test)]
#[path = "tests/pycalls.rs"]
mod pycalls_tests;

#[cfg(test)]
#[path = "tests/python_nested.rs"]
mod python_nested_tests;

#[cfg(test)]
#[path = "tests/python.rs"]
mod python_tests;

#[cfg(test)]
#[path = "tests/python_type_only.rs"]
mod python_type_only_tests;
