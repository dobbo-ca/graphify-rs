//! Port of `rust.go`: functions, types (struct/enum/union/trait/type alias), inherent/trait impl
//! methods, `use` imports, and call edges out of a .rs file. Items nested in `mod` blocks are
//! descended into so module-organised crates still surface their definitions.
//!
//! Go keeps `rsLocal` and `rsTParams` on its builder; here they sit beside a [`Builder`].

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk, walk_kinds};
use super::extract::{Def, Result};
use crate::model::{Confidence, Edge, Loc, Relation};

/// Go `rustCallKinds`, named by [`register_kinds`].
static CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    CALL_KINDS.get_or_init(|| {
        gositter_grammars::rust::language().kind_mask(&["call_expression", "macro_invocation"])
    });
}

/// Go `extractRust`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_rust(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::rust::language(), |root| {
        let mut ex = Ex {
            b: Builder::new(rel),
            src,
            local: HashSet::new(),
            tparams: HashSet::new(),
        };
        walk(Some(root), |cv| {
            if matches!(cv.kind(), "struct_item" | "enum_item" | "trait_item") {
                ex.local.insert(field_text(cv, "name", src).into_owned());
            }
            true
        });
        ex.rust_items(root);
        // An impl of a type declared elsewhere has no node to start from.
        let b = &mut ex.b;
        b.res.type_refs.retain(|r| b.seen.contains(&r.from_id));
        ex.b.res
    })
}

/// Go `rustBuiltin`: the prelude and std names that name no type of the corpus.
fn rust_builtin(n: &str) -> bool {
    matches!(
        n,
        "String"
            | "str"
            | "Option"
            | "Result"
            | "Vec"
            | "VecDeque"
            | "Box"
            | "Rc"
            | "Arc"
            | "Weak"
            | "RefCell"
            | "Cell"
            | "Cow"
            | "Pin"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "BinaryHeap"
            | "Path"
            | "PathBuf"
            | "OsStr"
            | "OsString"
            | "CStr"
            | "CString"
            | "Duration"
            | "Instant"
            | "SystemTime"
            | "Ordering"
            | "Range"
            | "RangeInclusive"
            | "RangeFrom"
            | "RangeTo"
            | "RangeFull"
            | "PhantomData"
            | "ManuallyDrop"
            | "NonZeroU8"
            | "NonZeroU16"
            | "NonZeroU32"
            | "NonZeroU64"
            | "NonZeroUsize"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "Self"
            | "Default"
            | "Clone"
            | "Copy"
            | "Debug"
            | "Display"
            | "Error"
            | "From"
            | "Into"
            | "TryFrom"
            | "TryInto"
            | "AsRef"
            | "AsMut"
            | "Iterator"
            | "IntoIterator"
            | "Extend"
            | "PartialEq"
            | "Eq"
            | "PartialOrd"
            | "Ord"
            | "Hash"
            | "Send"
            | "Sync"
            | "Sized"
            | "Drop"
            | "Deref"
            | "DerefMut"
            | "Future"
            | "Fn"
            | "FnMut"
            | "FnOnce"
    )
}

/// Go `fieldText`: the text of field `field` of `n`, `""` when absent.
fn field_text<'s>(n: Node<'_>, field: &str, src: &'s [u8]) -> Cow<'s, str> {
    match n.child_by_field_name(field) {
        Some(c) => node_text(c, src),
        None => Cow::Borrowed(""),
    }
}

/// Go `rustTParams`: `outer` plus the type parameter names of the type_parameters node `n`.
fn rust_t_params(n: Option<Node<'_>>, outer: &HashSet<String>, src: &[u8]) -> HashSet<String> {
    let mut out = outer.clone();
    let Some(n) = n else { return out };
    for i in 0..n.named_child_count() {
        let Some(p) = n.named_child(i) else { continue };
        if !matches!(
            p.kind(),
            "type_parameter" | "constrained_type_parameter" | "optional_type_parameter"
        ) {
            continue;
        }
        if let Some(name) = p.child_by_field_name("name") {
            out.insert(node_text(name, src).into_owned());
        } else if let Some(left) = p.child_by_field_name("left") {
            out.insert(node_text(left, src).into_owned());
        }
    }
    out
}

/// Go `rustTypeName`: a bare type name for impl targets, ignoring generic arguments and
/// references (e.g. "Server" for `impl<T> Server<T>`).
fn rust_type_name(n: Option<Node<'_>>, src: &[u8]) -> String {
    let Some(mut n) = n else {
        return String::new();
    };
    loop {
        let next = match n.kind() {
            "type_identifier" | "identifier" => return node_text(n, src).into_owned(),
            "generic_type" | "reference_type" => n.child_by_field_name("type"),
            "scoped_type_identifier" => n.child_by_field_name("name"),
            _ => break,
        };
        match next {
            Some(c) => n = c,
            None => return String::new(),
        }
    }
    // Fall back to the first type/identifier descendant.
    let mut name = String::new();
    walk(Some(n), |c| {
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

/// Go `builder` plus the fields `rust.go` adds to it.
struct Ex<'a> {
    b: Builder,
    src: &'a [u8],
    /// Go `rsLocal`: the struct, enum and trait names of the file.
    local: HashSet<String>,
    /// Go `rsTParams`: the type parameters in scope.
    tparams: HashSet<String>,
}

impl Ex<'_> {
    /// Go `rustTypeRefs`: a reference from `id` to the types `t` names and to its generic
    /// arguments. `rel` applies to the outermost name only: a supertrait is inherited, what it
    /// takes as arguments is referenced. A primitive, a prelude name, a type parameter and an
    /// associated type name no type.
    fn rust_type_refs(&mut self, id: &str, t: Option<Node<'_>>, rel: &str) {
        let Some(t) = t else { return };
        let src = self.src;
        match t.kind() {
            "primitive_type" => {}
            "type_identifier" => self.rust_ref(id, &node_text(t, src), rel, line(t)),
            "scoped_type_identifier" => {
                if let Some(p) = t.child_by_field_name("path")
                    && p.kind() != "bracketed_type"
                {
                    let text = node_text(p, src);
                    let first = text.split_once("::").map_or(&*text, |(f, _)| f);
                    if first != "Self" && !self.tparams.contains(first) {
                        self.rust_ref(id, &field_text(t, "name", src), rel, line(t));
                    }
                }
            }
            "generic_type" => {
                self.rust_type_refs(id, t.child_by_field_name("type"), rel);
                self.rust_type_refs(id, t.child_by_field_name("type_arguments"), "references");
            }
            "ordered_field_declaration_list" => {
                // A tuple field is a type, a path, a reference, a tuple or an array.
                for i in 0..t.named_child_count() {
                    let Some(c) = t.named_child(i) else { continue };
                    if matches!(
                        c.kind(),
                        "type_identifier"
                            | "generic_type"
                            | "scoped_type_identifier"
                            | "reference_type"
                            | "tuple_type"
                            | "array_type"
                    ) {
                        self.rust_type_refs(id, Some(c), "references");
                    }
                }
            }
            // `Item = X` names the associated type, not a type.
            "type_binding" => self.rust_type_refs(id, t.child_by_field_name("type"), "references"),
            _ => {
                for i in 0..t.named_child_count() {
                    self.rust_type_refs(id, t.named_child(i), "references");
                }
            }
        }
    }

    fn rust_ref(&mut self, id: &str, name: &str, rel: &str, loc: Loc) {
        if !self.tparams.contains(name) && (!rust_builtin(name) || self.local.contains(name)) {
            self.b.ref_(id, name, rel, false, loc);
        }
    }

    /// Go `rustSignature`: the types the parameters and result of `fn_node` name.
    fn rust_signature(&mut self, id: &str, fn_node: Node<'_>) {
        let new = rust_t_params(
            fn_node.child_by_field_name("type_parameters"),
            &self.tparams,
            self.src,
        );
        let outer = std::mem::replace(&mut self.tparams, new);
        if let Some(ps) = fn_node.child_by_field_name("parameters") {
            for i in 0..ps.named_child_count() {
                if let Some(p) = ps.named_child(i)
                    && p.kind() == "parameter"
                {
                    self.rust_type_refs(id, p.child_by_field_name("type"), "references");
                }
            }
        }
        self.rust_type_refs(id, fn_node.child_by_field_name("return_type"), "references");
        self.tparams = outer;
    }

    /// Go `rustItems`: each item directly under `n` (a source_file or a mod's declaration_list),
    /// recursing into nested modules.
    fn rust_items(&mut self, n: Node<'_>) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            match c.kind() {
                "function_item" => self.rust_func(c),
                "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" => {
                    self.rust_type(c)
                }
                "impl_item" => self.rust_impl(c),
                "use_declaration" => self.rust_use(c),
                "mod_item" => {
                    if let Some(body) = c.child_by_field_name("body") {
                        self.rust_items(body);
                    } else {
                        let name = field_text(c, "name", self.src);
                        if !name.is_empty() {
                            // `mod x;` loads x.rs, so the file depends on it.
                            self.b.imp(&format!("self::{name}"), line(c));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn rust_func(&mut self, n: Node<'_>) {
        let name = field_text(n, "name", self.src);
        if name.is_empty() {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &format!("{name}()"), line(n));
        self.rust_signature(&id, n);
        self.rust_calls(n.child_by_field_name("body"), &id);
    }

    fn rust_type(&mut self, n: Node<'_>) {
        let name = field_text(n, "name", self.src);
        if name.is_empty() {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &name, line(n));
        let new = rust_t_params(
            n.child_by_field_name("type_parameters"),
            &self.tparams,
            self.src,
        );
        let outer = std::mem::replace(&mut self.tparams, new);
        if n.kind() != "trait_item" {
            self.rust_type_refs(&id, n.child_by_field_name("body"), "references");
        } else if let Some(bounds) = n.child_by_field_name("bounds") {
            for i in 0..bounds.named_child_count() {
                self.rust_type_refs(&id, bounds.named_child(i), "inherits");
            }
        }
        self.tparams = outer;
    }

    /// Go `rustImpl`: the methods of an `impl Type { ... }` (or `impl Trait for Type`) block,
    /// scoped under the implemented type's name.
    fn rust_impl(&mut self, n: Node<'_>) {
        let type_name = rust_type_name(n.child_by_field_name("type"), self.src);
        if type_name.is_empty() {
            return;
        }
        let new = rust_t_params(
            n.child_by_field_name("type_parameters"),
            &self.tparams,
            self.src,
        );
        let outer = std::mem::replace(&mut self.tparams, new);
        self.rust_impl_body(n, &type_name);
        self.tparams = outer;
    }

    fn rust_impl_body(&mut self, n: Node<'_>, type_name: &str) {
        let type_id = self.b.id(&[type_name]);
        self.rust_type_refs(&type_id, n.child_by_field_name("trait"), "implements");
        let Some(body) = n.child_by_field_name("body") else {
            return;
        };
        for i in 0..body.child_count() {
            let Some(m) = body.child(i) else { continue };
            if m.kind() != "function_item" {
                continue;
            }
            let mname = field_text(m, "name", self.src);
            if mname.is_empty() {
                continue;
            }
            let mid = self.b.id(&[type_name, &mname]);
            self.b
                .add_node(&mid, &format!("{type_name}.{mname}()"), line(m));
            self.b.res.edges.push(Edge {
                source: type_id.clone(),
                target: mid.clone(),
                relation: Relation::CONTAINS,
                confidence: Confidence::EXTRACTED,
                source_file: self.b.src,
                source_location: line(m),
                ..Edge::default()
            });
            // Register under the bare method name so `x.method()` call sites resolve.
            self.b.res.defs.push(Def {
                id: mid.clone(),
                name: mname.into_owned(),
                file: self.b.file.clone(),
                ..Def::default()
            });
            self.rust_signature(&mid, m);
            self.rust_calls(m.child_by_field_name("body"), &mid);
        }
    }

    /// Go `rustUse`: the module path of a `use` declaration as an import. Grouped
    /// (`use a::{b, c}`) and aliased uses record the leading path; the names bound under it are
    /// kept too, since `use crate::a` names a module.
    fn rust_use(&mut self, n: Node<'_>) {
        let src = self.src;
        let Some(arg) = n.child_by_field_name("argument") else {
            return;
        };
        let Some(p) = arg.child_by_field_name("path") else {
            self.b.imp(&node_text(arg, src), line(n));
            return;
        };
        self.b.imp(&node_text(p, src), line(n));
        // Go indexes `Imps[len-1]`, the previous one if `imp` took an empty spec.
        let Some(imp) = self.b.res.imps.last_mut() else {
            return;
        };
        match arg.kind() {
            "scoped_identifier" => imp.names = vec![field_text(arg, "name", src).into_owned()],
            "scoped_use_list" => {
                if let Some(list) = arg.child_by_field_name("list") {
                    for i in 0..list.child_count() {
                        if let Some(c) = list.child(i)
                            && matches!(c.kind(), "identifier" | "scoped_identifier")
                        {
                            imp.names.push(node_text(c, src).into_owned());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Go `rustCalls`: walks a function body and records each call site. Direct calls (`f()`),
    /// path calls (`mod::f()`), and method calls (`x.f()`) all record the trailing name; macro
    /// invocations (`f!()`) record the macro name.
    fn rust_calls(&mut self, body: Option<Node<'_>>, caller_id: &str) {
        let src = self.src;
        let mask = *CALL_KINDS
            .get()
            .expect("kinds::register_kinds not called before extract_rust");
        walk_kinds(body, mask, |c| {
            match c.kind() {
                "call_expression" => {
                    let Some(f) = c.child_by_field_name("function") else {
                        return true;
                    };
                    match f.kind() {
                        "identifier" => self.b.call(caller_id, &node_text(f, src), line(c)),
                        "scoped_identifier" => {
                            if let Some(name) = f.child_by_field_name("name") {
                                let recv = recv_text(Some(f), Some(name), src);
                                self.b
                                    .call_recv(caller_id, &node_text(name, src), &recv, line(c));
                            }
                        }
                        "field_expression" => {
                            if let Some(fld) = f.child_by_field_name("field") {
                                let recv = recv_text(Some(f), Some(fld), src);
                                self.b
                                    .call_recv(caller_id, &node_text(fld, src), &recv, line(c));
                            }
                        }
                        _ => {}
                    }
                }
                "macro_invocation" => {
                    let Some(mut m) = c.child_by_field_name("macro") else {
                        return true;
                    };
                    if m.kind() == "scoped_identifier" {
                        match m.child_by_field_name("name") {
                            Some(x) => m = x,
                            None => return true,
                        }
                    }
                    self.b.call(caller_id, &node_text(m, src), line(c));
                }
                _ => {}
            }
            true
        });
    }
}

#[cfg(test)]
#[path = "tests/rust.rs"]
mod rust_tests;
