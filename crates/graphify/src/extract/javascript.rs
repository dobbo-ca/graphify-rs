//! Port of `javascript.go`: functions, classes (+ methods), interfaces, type aliases, enums,
//! imports, and call edges out of a JS/TS/TSX file. The same walker serves all three because
//! TypeScript's grammar is a superset of JavaScript's.
//!
//! Go keeps `jsExp`, `jsBare`, `jsExpObjs`, `jsProto`, `jsOwners`, `jsBodySeen`, `jsBodies`,
//! `jsNested`, `jsParent`, `jsLinked` and `jsOwned` on its one `builder`; here they are the
//! fields of [`Js`], beside a `Builder`. Go's `jsChild` is `lua::child_by_kind`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{LazyLock, OnceLock};

use gositter::{Language, Node};
use gostd::regexp::Regexp;
use gostd::strings::{bytes_trim_space, fields, to_upper};

use super::builder::{Builder, go_text, line, node_text, parse_root, recv_text, walk, walk_kinds};
use super::extract::{DYN_IMPORT, Def, Imp, Result};
use super::golang::{field_text, unquote};
use super::lua::{child_by_kind, first_named};
use super::python::set;
use crate::idutil::{make_id, normalize_id};
use crate::model::{Confidence, Edge, Loc, Node as ModelNode, Relation, line as model_line};

/// Go `init()` (`javascript.go:22`): the masks of `call_expression`, `new_expression`,
/// `method_definition` and `export_statement` for js, ts and tsx, in that order.
static JS_MASKS: OnceLock<[[u32; 4]; 3]> = OnceLock::new();

/// The masks of the language `lang` (Go `langPtr.KindMask(...)`, named by `register_kinds`):
/// call, new, method, export.
fn js_masks(lang: &'static Language) -> [u32; 4] {
    let m = JS_MASKS.get().expect("register_kinds");
    if std::ptr::eq(lang, gositter_grammars::typescript::language()) {
        m[1]
    } else if std::ptr::eq(lang, gositter_grammars::tsx::language()) {
        m[2]
    } else {
        m[0]
    }
}

/// Go `jsPrimitive`: the type names that are no symbols.
static JS_PRIMITIVE: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    set(&[
        "string",
        "number",
        "boolean",
        "any",
        "unknown",
        "void",
        "never",
        "object",
        "null",
        "undefined",
        "bigint",
        "symbol",
        "this",
    ])
});

/// Go `jsNewBuiltins`: the names upstream never binds a `new` to: its one builtin list for
/// every language, Swift types included.
static JS_NEW_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    fields(
        "String Number Boolean Object Array Symbol BigInt Date RegExp Error
		TypeError RangeError SyntaxError ReferenceError EvalError URIError Promise Map Set WeakMap
		WeakSet JSON Math Reflect Proxy Intl parseInt parseFloat isNaN isFinite encodeURIComponent
		decodeURIComponent encodeURI decodeURI URL URLSearchParams FormData Blob File Headers
		Request Response AbortController AbortSignal TextEncoder TextDecoder console str int float
		bool list dict set tuple bytes len range enumerate zip map filter sum min max print open
		isinstance type super sorted reversed any all abs round next iter hash id repr callable
		getattr setattr hasattr delattr vars dir Int Int8 Int16 Int32 Int64 UInt UInt8 UInt16
		UInt32 UInt64 Double Float Bool Character Sendable Codable Decodable Encodable Equatable
		Hashable Identifiable Comparable CaseIterable RawRepresentable CustomStringConvertible
		CustomDebugStringConvertible AnyObject LocalizedError Data UUID Decimal Calendar Locale
		TimeZone Bundle IndexPath IndexSet NotificationCenter UserDefaults FileManager URLSession
		URLRequest URLComponents JSONDecoder JSONEncoder DateFormatter NumberFormatter
		ISO8601DateFormatter NSObject NSString NSError NSLock NSAttributedString DispatchQueue
		DispatchGroup OperationQueue RunLoop View Color Font",
    )
    .into_iter()
    .collect()
});

/// Go `jsNewRecv`: marks the call of a `new X()`, which binds only with import evidence.
const JS_NEW_RECV: &str = "\x00new";

/// Go `jsRationalePrefixes`: the leading comment tokens that mark an explanatory comment worth
/// capturing as a rationale node (mirrors upstream `_JS_RATIONALE_PREFIXES`; covers `//` line
/// comments and `*`-prefixed lines inside block comments).
const JS_RATIONALE_PREFIXES: [&str; 14] = [
    "// NOTE:",
    "// IMPORTANT:",
    "// HACK:",
    "// WHY:",
    "// RATIONALE:",
    "// TODO:",
    "// FIXME:",
    "* NOTE:",
    "* IMPORTANT:",
    "* HACK:",
    "* WHY:",
    "* RATIONALE:",
    "* TODO:",
    "* FIXME:",
];

/// Go `jsCommentLineRe`: a line that begins (after optional indent) with a comment marker, so
/// doc references are only harvested from comments.
static JS_COMMENT_LINE_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"^\s*(//|/\*|\*)"));

/// Go `jsDocRefRe`: architecture-decision / RFC references (ADR-0011, RFC 793) in a comment.
static JS_DOC_REF_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"(?i)\b(ADR[- ]?\d{1,5}|RFC[- ]?\d{1,5})\b"));

/// Go `jsDocRefParseRe`: splits a matched token into its kind and number.
static JS_DOC_REF_PARSE_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"([A-Za-z]+)[- ]?(\d+)"));

/// Go `jsProto`: a prototype member; its owner may be declared further down.
#[derive(Default)]
struct JsProto {
    id: String,
    owner: String,
    name: String,
    oid: String,
    loc: Loc,
}

/// Go `jsBody`: a function body whose calls belong to `caller_id`.
#[derive(Clone, Default)]
struct JsBody {
    start: u32,
    end: u32,
    caller_id: String,
    /// Namespaces around the body.
    ns: Vec<String>,
}

/// The Go `builder` fields javascript.go reads, beside the [`Builder`].
struct Js {
    b: Builder,
    /// Go `jsExp`: the ids of the exported declarations of a namespace.
    js_exp: HashSet<String>,
    /// Go `jsBare`: the ids of the exported variables of a namespace.
    js_bare: HashSet<String>,
    /// Go `jsExpObjs`: `None` is Go's nil map.
    js_exp_objs: Option<HashSet<String>>,
    js_proto: Vec<JsProto>,
    js_owners: Vec<JsProto>,
    js_body_seen: HashSet<u32>,
    js_bodies: Vec<JsBody>,
    /// Go `jsNested`: (parent, name) to the id of the function declared inside parent.
    js_nested: HashMap<(String, String), String>,
    js_parent: HashMap<String, String>,
    /// Go `jsLinked`: the `calls` edges of [`js_nested_call`] already made.
    js_linked: HashSet<(String, String)>,
    /// Go `jsOwned`: `None` until first asked.
    js_owned: Option<HashSet<String>>,
}

/// Go `extractJS`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_js(rel: &str, src: &[u8], lang: &'static Language) -> Option<Result> {
    parse_root(src, lang, |root| {
        let mut js = Js {
            b: Builder::new(rel),
            js_exp: HashSet::new(),
            js_bare: HashSet::new(),
            js_exp_objs: None,
            js_proto: Vec::new(),
            js_owners: Vec::new(),
            js_body_seen: HashSet::new(),
            js_bodies: Vec::new(),
            js_nested: HashMap::new(),
            js_parent: HashMap::new(),
            js_linked: HashSet::new(),
            js_owned: None,
        };
        let [call, new, method, export] = js_masks(lang);

        for i in 0..root.child_count() {
            if let Some(n) = root.child(i) {
                js.js_statement(n, src, "");
            }
        }
        // One walk serves the import calls and every body jsCalls registered. The
        // bodies are in source order and nest, so a stack holds the open ones and
        // its top is the innermost body of the call.
        js.js_bodies.sort_by_key(|b| b.start);
        let mut next = 0;
        let mut open: Vec<JsBody> = Vec::new();
        // Open method_definition spans.
        let mut meths: Vec<(u32, u32)> = Vec::new();
        let mask = call | new | method;
        walk_kinds(Some(root), mask, |c| {
            while meths.last().is_some_and(|m| c.start_byte() >= m.1) {
                meths.pop();
            }
            if c.kind() == "method_definition" {
                meths.push((c.start_byte(), c.end_byte()));
                return true;
            }
            while open.last().is_some_and(|o| c.start_byte() >= o.end) {
                open.pop();
            }
            while next < js.js_bodies.len() && c.start_byte() >= js.js_bodies[next].start {
                if c.start_byte() < js.js_bodies[next].end {
                    open.push(js.js_bodies[next].clone());
                }
                next += 1;
            }
            let body = open.last().cloned().unwrap_or_default();
            js.js_call_import(c, src, body.caller_id.clone(), &body);
            if !open.is_empty() {
                let in_method = meths
                    .last()
                    .is_some_and(|m| m.0 >= body.start && *m != (body.start, body.end));
                js.js_call(c, &body, src, in_method);
            }
            true
        });
        // An exported namespace member is no call target of plain code when the file has a plain one.
        for d in std::mem::take(&mut js.b.js_ns_defs) {
            if js.js_exp.contains(&d.id) && !js.b.res.defs.iter().any(|p| p.name == d.name) {
                js.b.res.defs.push(d);
            }
        }
        js.js_export_orphans(root, src, export);
        // An owner node never takes the id of a declaration.
        for o in std::mem::take(&mut js.js_owners) {
            if !js.b.seen.contains(&o.id) {
                js.const_node(&o.id, &o.owner, o.loc);
            }
        }
        js.js_proto_owners();
        js_rationale(&mut js.b, src);
        js.b.res
    })
}

/// Go `extractJSFile`: [`extract_js`] plus the dynamic_import edges, found by regex as upstream
/// does. `None` when the parse budget stops the parse.
pub(crate) fn extract_js_file(rel: &str, src: &[u8], lang: &'static Language) -> Option<Result> {
    let mut res = extract_js(rel, src, lang)?;
    let file_id = make_id(&[rel]);
    let is_word = |c: u8| c == b'_' || c.is_ascii_alphanumeric() || c >= 0x80;
    let skip_space = |mut i: usize| {
        while i < src.len() && b" \t\n\r\x0c\x0b".contains(&src[i]) {
            i += 1;
        }
        i
    };
    // slash: // seen on this line
    let (mut line_no, mut prev, mut slash) = (1i64, 0usize, false);
    let mut i = 0;
    while let Some(j) = src[i..].windows(6).position(|w| w == b"import") {
        let at = i + j;
        i = at + "import".len();
        if at > 0 && is_word(src[at - 1]) {
            continue;
        }
        let mut k = skip_space(i);
        if k >= src.len() || src[k] != b'(' {
            continue;
        }
        k = skip_space(k + 1);
        if k >= src.len() || (src[k] != b'\'' && src[k] != b'"' && src[k] != b'`') {
            continue;
        }
        let q = src[k];
        let mut end = k + 1;
        while end < src.len()
            && src[end] != q
            && src[end] != b'\n'
            && (q != b'`' || src[end] != b'$')
        {
            end += 1;
        }
        if end == k + 1 || end >= src.len() || src[end] != q {
            continue;
        }
        let c = skip_space(end + 1);
        if c >= src.len() || src[c] != b')' {
            continue;
        }
        // incremental, keeps it linear
        let mut seg = &src[prev..at];
        prev = at;
        let n = seg.iter().filter(|&&c| c == b'\n').count();
        if n > 0 {
            line_no += n as i64;
            slash = false;
            if let Some(p) = seg.iter().rposition(|&c| c == b'\n') {
                seg = &seg[p + 1..];
            }
        }
        slash = slash || seg.windows(2).any(|w| w == b"//");
        if slash {
            continue;
        }
        // upstream drops an empty package name
        if src[k + 1] == b'/' {
            continue;
        }
        res.imps.push(Imp {
            file_id: file_id.clone(),
            file: rel.to_string(),
            spec: go_text(&src[k + 1..end]).into_owned(),
            loc: model_line(line_no),
            dynamic: DYN_IMPORT,
            ..Imp::default()
        });
    }
    Some(res)
}

/// Go `degenerateName`: whether a symbol name carries no identifier signal; it normalizes to
/// an empty ID (a minified `$`, a JSONC `//` comment key). MakeID would collapse such a name to
/// the bare file stem, producing a label-only noise node that can also collide with an
/// extensionless sibling file. Skip it (#1899).
pub(super) fn degenerate_name(name: &str) -> bool {
    normalize_id(name).is_empty()
}

/// The `contains` edge literal Go writes out in `jsClass`, `jsMethodNode`, `jsMemberScan` and
/// `jsNestedFuncs`: `source` contains `target`.
fn push_contains(b: &mut Builder, source: &str, target: &str, loc: Loc) {
    b.res.edges.push(Edge {
        source: source.to_string(),
        target: target.to_string(),
        relation: Relation::CONTAINS,
        confidence: Confidence::EXTRACTED,
        source_file: b.src,
        source_location: loc,
        ..Edge::default()
    });
}

impl Js {
    /// Go `jsExportOrphans`: gives every exported declarator that has no node yet a node with no
    /// edge, as upstream's export facts do. Any export statement counts, nested or under a parse
    /// error. A const or let with an identifier value also names its whole pattern.
    fn js_export_orphans(&mut self, root: Node<'_>, src: &[u8], mask: u32) {
        walk_kinds(Some(root), mask, |ev| {
            let Some(d) = ev.child_by_field_name("declaration") else {
                return true;
            };
            if d.kind() != "lexical_declaration" && d.kind() != "variable_declaration" {
                return true;
            }
            for i in 0..d.named_child_count() {
                let Some(dc) = d.named_child(i) else { continue };
                let nm = dc.child_by_field_name("name");
                let val = dc.child_by_field_name("value");
                let Some(nm) = nm.filter(|_| dc.kind() == "variable_declarator") else {
                    continue;
                };
                let alias = val.is_some_and(|v| {
                    d.kind() == "lexical_declaration"
                        && (v.kind() == "identifier" || v.kind() == "type_identifier")
                });
                if !alias && nm.kind() != "identifier" {
                    continue;
                }
                let name = node_text(nm, src).into_owned();
                let id = make_id(&[&self.b.stem, &name]);
                if !degenerate_name(&name) && !self.b.seen.contains(&id) {
                    self.b.add_node(&id, &name, line(ev));
                }
            }
            true
        });
    }

    /// Go `jsStatement`: handles one node, unwrapping `export ...` first, and recurses into
    /// anything it has no case for: a declaration in an IIFE, a callback or an `if` is a
    /// file-level one. `owner` is the caller of the closures met on the way, empty for none.
    fn js_statement(&mut self, n: Node<'_>, src: &[u8], owner: &str) {
        match n.kind() {
            "export_statement" => {
                let d = n.child_by_field_name("declaration");
                let s = n.child_by_field_name("source");
                if js_has_default(n) {
                    self.js_default(n, d, src);
                }
                if let Some(d) = d {
                    self.js_statement(d, src, owner);
                    if !self.b.js_ns.is_empty() {
                        let id = self.b.id(&[&field_text(d, "name", src)]);
                        self.js_exp.insert(id);
                    }
                } else if s.is_none() {
                    self.js_children(n, src, owner);
                }
                if let Some(s) = s {
                    self.js_imp(n, &unquote(&node_text(s, src)), src, true);
                } else if let Some(c) = child_by_kind(n, "export_clause") {
                    for i in 0..c.named_child_count() {
                        let Some(sp) = c.named_child(i) else { continue };
                        let mut e = field_text(sp, "name", src);
                        let a = field_text(sp, "alias", src);
                        if a == "default" {
                            // `export { Foo as default }`
                            self.b.res.default = e.clone();
                        }
                        if !a.is_empty() && a != e {
                            e.push_str(" as ");
                            e.push_str(&a);
                        }
                        self.b.res.exports.push(e);
                    }
                }
            }
            "import_statement" => {
                if let Some(s) = n.child_by_field_name("source") {
                    self.js_imp(n, &unquote(&node_text(s, src)), src, false);
                }
            }
            "function_declaration" | "generator_function_declaration" => self.js_func(n, src),
            "class_declaration" | "abstract_class_declaration" => self.js_class(n, src),
            "interface_declaration" | "type_alias_declaration" | "enum_declaration" => {
                self.js_named_type(n, src)
            }
            "lexical_declaration" | "variable_declaration" => self.js_var_funcs(n, src, owner),
            "expression_statement" => {
                if !self.js_member_assign(n, src) {
                    // Only a closure in a call statement is the file's call.
                    let mut owner = owner.to_string();
                    for i in 0..n.child_count() {
                        let Some(c) = n.child(i) else { continue };
                        if c.kind() == "call_expression" || c.kind() == "new_expression" {
                            owner = self.b.file_id.clone();
                            // Its closures' members belong to the file.
                            let mut cs = Vec::new();
                            js_top_closures(c, &mut cs);
                            for cl in cs {
                                let fid = self.b.file_id.clone();
                                self.js_member_scan(
                                    cl.child_by_field_name("body"),
                                    &fid,
                                    "",
                                    &fid,
                                    src,
                                );
                            }
                        }
                    }
                    self.js_children(n, src, &owner);
                }
            }
            "internal_module" | "module" => self.js_namespace(n, src, owner),
            "method_definition" | "method_signature" => self.js_lit_method(n, src),
            "arrow_function" | "function_expression" | "function" | "generator_function" => {
                if !owner.is_empty() {
                    self.js_calls(n.child_by_field_name("body"), owner);
                }
                self.js_children(n, src, owner);
            }
            _ => self.js_children(n, src, owner),
        }
    }

    /// Go `jsNamespace`: makes a node for a namespace or module and walks its body with the
    /// name in member ids, so equal names in two namespaces both survive.
    fn js_namespace(&mut self, n: Node<'_>, src: &[u8], owner: &str) {
        let name = unquote(&field_text(n, "name", src));
        if !degenerate_name(&name) {
            let id = self.b.id(&[&name]);
            self.const_node(&id, &name, line(n));
        }
        self.b.js_ns.push(name);
        self.js_children(n, src, owner);
        self.b.js_ns.pop();
    }

    /// Go `jsChildren`.
    fn js_children(&mut self, n: Node<'_>, src: &[u8], owner: &str) {
        for i in 0..n.child_count() {
            if let Some(c) = n.child(i) {
                self.js_statement(c, src, owner);
            }
        }
    }

    /// Go `jsMaskedImport`: reports an import() in the type arguments of a generic call that
    /// tree-sitter takes for `a < b > (c)`. Upstream blanks it before parsing.
    fn js_masked_import(&self, n: Node<'_>, src: &[u8]) -> bool {
        let e = gostd::path::ext(&self.b.file);
        if e == ".js" || e == ".jsx" || e == ".mjs" || e == ".cjs" {
            return false;
        }
        let mut bin: Option<Node<'_>> = None;
        let mut parens = 0i32;
        let (mut p, mut q) = (n, n.parent());
        while let Some(qq) = q {
            let k = qq.kind();
            if k == "binary_expression" {
                let r = qq.child_by_field_name("right");
                let op = qq.child_by_field_name("operator");
                if let (Some(r), Some(op)) = (r, op)
                    && op.kind() == "<"
                    && r.start_byte() == p.start_byte()
                    && r.end_byte() == p.end_byte()
                {
                    bin = Some(qq);
                }
                break;
            }
            if k == "parenthesized_expression" {
                parens += 1;
            }
            if k != "unary_expression"
                && k != "member_expression"
                && k != "subscript_expression"
                && k != "parenthesized_expression"
                && k != "array"
            {
                break;
            }
            (p, q) = (qq, qq.parent());
        }
        let Some(bin) = bin else {
            return false;
        };
        let mut end = n.end_byte() as usize;
        while end < src.len()
            && src[end] != b'>'
            && ((src[end] == b')' && parens > 0) || !b"();{}".contains(&src[end]))
        {
            if src[end] == b')' {
                parens -= 1;
            }
            end += 1;
        }
        if end >= src.len() || src[end] != b'>' {
            return false;
        }
        end += 1;
        // only failed generics leave ERROR
        let mut a = bin;
        while let Some(par) = a.parent() {
            let mut kids = Vec::new();
            par.children(&mut kids);
            for ch in kids {
                if ch.kind() == "ERROR"
                    && ch.start_byte() as usize <= end + 2
                    && ch.end_byte() as usize + 1 >= end
                {
                    return true;
                }
            }
            a = par;
        }
        false
    }

    /// Go `jsCallImport`: records `require("m")` and `import("m")` as imports; `caller` is the
    /// function holding the call. A computed spec names no file.
    fn js_call_import(&mut self, n: Node<'_>, src: &[u8], mut caller: String, body: &JsBody) {
        if n.kind() != "call_expression" {
            return;
        }
        let (Some(f), Some(args)) = (
            n.child_by_field_name("function"),
            n.child_by_field_name("arguments"),
        ) else {
            return;
        };
        if args.named_child_count() == 0 {
            return;
        }
        // The one argument, when it is the only one and a string.
        let one = args
            .named_child(0)
            .filter(|a| args.named_child_count() == 1 && a.kind() == "string");
        if f.kind() == "identifier" && node_text(f, src) == "require" {
            if let Some(a) = one {
                self.b.imp(&unquote(&node_text(a, src)), line(n));
            }
        } else if f.kind() == "import" {
            // TS import-equals parses as a call
            let between = go_text(&src[f.end_byte() as usize..args.start_byte() as usize]);
            if !gostd::strings::trim_space(&between).is_empty()
                && let Some(a) = one
            {
                self.b.imp(&unquote(&node_text(a, src)), line(n));
            }
            if !caller.is_empty() && (js_in_signature(n, body) || self.js_masked_import(n, src)) {
                caller.clear();
            }
            self.b
                .imp_deferred(&js_import_spec(args, src), line(n), &caller);
        }
    }

    /// Go `jsDefault`: records the local name `export default` exports: a declaration's name or
    /// a bare identifier. An anonymous default has none.
    fn js_default(&mut self, n: Node<'_>, d: Option<Node<'_>>, src: &[u8]) {
        if let Some(d) = d {
            self.b.res.default = field_text(d, "name", src);
        } else if let Some(v) = n.child_by_field_name("value")
            && v.kind() == "identifier"
        {
            self.b.res.default = node_text(v, src).into_owned();
        }
    }

    /// Go `jsImp`: records an import or `export ... from` and the names it binds. An import
    /// entry is "name" or "name as local" (name "default" for the default clause); a re-export
    /// entry is "name" or "name as exported", name being the one in the source module, "*" for
    /// `export * as ns`. A plain `export *` has none.
    fn js_imp(&mut self, n: Node<'_>, spec: &str, src: &[u8], export: bool) {
        self.b.imp_typed(spec, line(n), js_type_only_import(n));
        if spec.is_empty() {
            return;
        }
        let mut names: Vec<String> = Vec::new();
        if child_by_kind(n, "namespace_export").is_some() {
            names = vec!["*".to_string()];
        }
        if let Some(c) = child_by_kind(n, "export_clause") {
            for i in 0..c.named_child_count() {
                if let Some(sp) = c.named_child(i) {
                    js_imp_spec(sp, src, &mut names);
                }
            }
        }
        if let Some(c) = child_by_kind(n, "import_clause") {
            for i in 0..c.named_child_count() {
                let Some(k) = c.named_child(i) else { continue };
                match k.kind() {
                    "identifier" => names.push(format!("default as {}", node_text(k, src))),
                    "named_imports" => {
                        for j in 0..k.named_child_count() {
                            if let Some(sp) = k.named_child(j) {
                                js_imp_spec(sp, src, &mut names);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(im) = self.b.res.imps.last_mut() {
            im.export = export;
            im.names.extend(names);
        }
    }

    /// Go `jsHeritage`: records the bases of a class or interface from its extends, implements
    /// or bare JS heritage clause. Only a plain or generic name is bound: a qualified name or a
    /// call names no declaration.
    fn js_heritage(&mut self, from_id: &str, clause: Node<'_>, src: &[u8]) {
        let rel = if clause.kind() == "implements_clause" {
            "implements"
        } else {
            "inherits"
        };
        for i in 0..clause.named_child_count() {
            let Some(mut c) = clause.named_child(i) else {
                continue;
            };
            if c.kind() == "generic_type" {
                let Some(g) = c.named_child(0) else { continue };
                c = g;
            }
            if c.kind() == "identifier" || c.kind() == "type_identifier" {
                self.b
                    .type_ref(from_id, &node_text(c, src), rel, line(clause));
            }
        }
    }

    /// Go `jsFunc`.
    fn js_func(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if degenerate_name(&name) {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &format!("{name}()"), line(n));
        self.js_fn_body(n.child_by_field_name("body"), &id, src);
        self.js_member_scan(n.child_by_field_name("body"), &id, &name, &id, src);
    }

    /// Go `jsClass`.
    fn js_class(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if degenerate_name(&name) {
            return;
        }
        let class_id = self.b.id(&[&name]);
        self.b.def(&class_id, &name, &name, line(n));
        if let Some(h) = child_by_kind(n, "class_heritage") {
            self.js_heritage(&class_id, h, src);
            for i in 0..h.named_child_count() {
                if let Some(c) = h.named_child(i)
                    && (c.kind() == "extends_clause" || c.kind() == "implements_clause")
                {
                    self.js_heritage(&class_id, c, src);
                }
            }
        }

        // `@D export class X {}` hangs the decorator on the export statement.
        self.js_decorators(&class_id, n, src);
        if let Some(p) = n.parent()
            && p.kind() == "export_statement"
        {
            self.js_decorators(&class_id, p, src);
        }

        let Some(body) = n.child_by_field_name("body") else {
            return;
        };
        // Decorators written before the next member.
        let mut pending: Vec<Node<'_>> = Vec::new();
        // Method bodies to scan once every member has its node.
        let mut scans: Vec<(Option<Node<'_>>, String)> = Vec::new();
        for i in 0..body.child_count() {
            let Some(m) = body.child(i) else { continue };
            if m.kind() == "class_static_block" {
                self.js_children(m, src, "");
                continue;
            }
            if m.kind() == "decorator" {
                pending.push(m);
                continue;
            }
            // A comment or a `;` does not part a decorator from its member.
            if !m.is_named() || m.kind() == "comment" {
                continue;
            }
            let mut decos = std::mem::take(&mut pending);
            // The node with the parameters and the body; `field_fn` is Go's `fn != m`.
            let mut func = m;
            let mut field_fn = false;
            if m.kind() == "public_field_definition" || m.kind() == "field_definition" {
                // A field is no node: its type and its decorators are the class's.
                self.js_type_names(&class_id, m.child_by_field_name("type"), src, line(m));
                self.js_decorators(&class_id, m, src);
                for d in &decos {
                    self.js_decorator(&class_id, *d, src);
                }
                // A function value makes the field a method too.
                match m.child_by_field_name("value") {
                    Some(v) if js_func_value(v) => {
                        func = v;
                        field_fn = true;
                    }
                    _ => continue,
                }
                decos.clear();
            } else if m.kind() != "method_definition"
                && m.kind() != "abstract_method_signature"
                && m.kind() != "method_signature"
            {
                continue;
            }
            let mut mname = field_text(m, "name", src);
            if mname.is_empty() {
                mname = field_text(m, "property", src);
            }
            // `override abstract h()` parses as name "abstract" then ERROR "h".
            if mname == "abstract"
                && let Some(e) = child_by_kind(m, "ERROR")
                && e.named_child_count() == 1
                && let Some(c) = e.named_child(0)
                && c.kind() == "identifier"
            {
                mname = node_text(c, src).into_owned();
            }
            if degenerate_name(&mname) {
                continue;
            }
            let mid = self.b.id(&[&name, &mname]);
            // The class owns an id another symbol took first too, as upstream.
            push_contains(&mut self.b, &class_id, &mid, line(m));
            // An overload's node sits at its first signature.
            if !self.b.seen.contains(&mid) {
                self.b.add_node(&mid, &format!("{name}.{mname}()"), line(m));
                self.b.res.defs.push(Def {
                    id: mid.clone(),
                    name: mname.clone(),
                    file: self.b.file.clone(),
                    is_abstract: m.kind() != "method_definition" && !field_fn,
                    ..Def::default()
                });
            } else if m.kind() == "method_definition" || field_fn {
                // The implementation after overload signatures is no abstract method.
                if let Some(d) = self.b.res.defs.iter_mut().rev().find(|d| d.id == mid) {
                    d.is_abstract = false;
                }
            }
            if !field_fn {
                self.js_decorators(&mid, m, src);
            }
            for d in &decos {
                self.js_decorator(&mid, *d, src);
            }
            if !field_fn {
                self.js_signature(&mid, func, src);
                self.js_fn_body(func.child_by_field_name("body"), &mid, src);
                scans.push((func.child_by_field_name("body"), mid));
            } else {
                // Upstream scans no declarations in a field's function.
                self.js_calls(func.child_by_field_name("body"), &mid);
            }
        }
        // `this.x = fn` is the class's; a local object stays the method's.
        for (bd, id) in scans {
            self.js_member_scan(bd, &class_id, &name, &id, src);
        }
    }

    /// Go `jsSignature`: records the types the parameters and the return type of the method
    /// `m` name, and the decorators of its parameters.
    fn js_signature(&mut self, id: &str, m: Node<'_>, src: &[u8]) {
        if let Some(ps) = m.child_by_field_name("parameters") {
            for i in 0..ps.named_child_count() {
                let Some(p) = ps.named_child(i) else { continue };
                self.js_type_names(id, p.child_by_field_name("type"), src, line(m));
                self.js_decorators(id, p, src);
            }
        }
        self.js_type_names(id, m.child_by_field_name("return_type"), src, line(m));
    }

    /// Go `jsTypeNames`: records each name under the type annotation `n` as a type reference of
    /// `id`. Resolve keeps only the names the file imports.
    fn js_type_names(&mut self, id: &str, n: Option<Node<'_>>, src: &[u8], loc: Loc) {
        walk(n, |c| match c.kind() {
            "nested_type_identifier" => {
                let name = field_text(c, "name", src);
                if !JS_PRIMITIVE.contains(name.as_str()) {
                    self.b.type_ref(id, &name, "references", loc);
                }
                false
            }
            "type_identifier" | "identifier" => {
                let name = node_text(c, src);
                if !JS_PRIMITIVE.contains(&*name) {
                    self.b.type_ref(id, &name, "references", loc);
                }
                true
            }
            _ => true,
        });
    }

    /// Go `jsDecorators`: records the decorators that are children of `n`.
    fn js_decorators(&mut self, id: &str, n: Node<'_>, src: &[u8]) {
        for i in 0..n.child_count() {
            if let Some(d) = n.child(i)
                && d.kind() == "decorator"
            {
                self.js_decorator(id, d, src);
            }
        }
    }

    /// Go `jsDecorator`: records a references edge from `id` to the name of decorator `d`:
    /// `@D`, `@D(x)`, and the last name of `@ns.D`.
    fn js_decorator(&mut self, id: &str, d: Node<'_>, src: &[u8]) {
        let mut e = first_named(d);
        if let Some(x) = e
            && x.kind() == "call_expression"
        {
            e = x.child_by_field_name("function");
        }
        if let Some(x) = e
            && x.kind() == "member_expression"
        {
            e = x.child_by_field_name("property");
        }
        if let Some(x) = e
            && (x.kind() == "identifier" || x.kind() == "property_identifier")
        {
            self.b
                .ref_(id, &node_text(x, src), "references", true, line(d));
        }
    }

    /// Go `jsNamedType`.
    fn js_named_type(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if degenerate_name(&name) {
            return;
        }
        let id = self.b.id(&[&name]);
        self.b.def(&id, &name, &name, line(n));
        if let Some(c) = child_by_kind(n, "extends_type_clause") {
            self.js_heritage(&id, c, src);
        }
        // An interface references its property types; its method signatures
        // wait for their nodes (graphify-go-vi5.9).
        if let Some(body) = n.child_by_field_name("body")
            && n.kind() == "interface_declaration"
        {
            for i in 0..body.named_child_count() {
                if let Some(m) = body.named_child(i)
                    && m.kind() == "property_signature"
                {
                    self.js_type_names(&id, m.child_by_field_name("type"), src, line(m));
                }
            }
        }
    }

    /// Go `jsVarFuncs`: captures `const foo = () => {}` and `const foo = function() {}`, the
    /// dominant way functions are defined in modern JS/TS. Below module level it makes no node
    /// and only walks the value, as upstream's #1077 guard.
    fn js_var_funcs(&mut self, n: Node<'_>, src: &[u8], owner: &str) {
        let mut p = n.parent();
        let exported = p.is_some_and(|x| x.kind() == "export_statement");
        if exported {
            p = p.and_then(|x| x.parent());
        }
        let top = p.is_some_and(|x| x.kind() == "program");
        // An exported variable of a namespace body is an orphan node.
        let mut in_ns = false;
        if exported
            && let Some(pp) = p
            && pp.kind() == "statement_block"
            && let Some(g) = pp.parent()
        {
            in_ns = g.kind() == "internal_module" || g.kind() == "module";
        }
        for i in 0..n.child_count() {
            let Some(d) = n.child(i) else { continue };
            if d.kind() != "variable_declarator" {
                continue;
            }
            if let Some(nm) = d.child_by_field_name("name")
                && in_ns
                && nm.kind() == "identifier"
                && !degenerate_name(&node_text(nm, src))
            {
                let t = node_text(nm, src).into_owned();
                let id = self.b.id(&[&t]);
                self.b.add_node(&id, &t, line(d));
                self.js_bare.insert(id);
            }
            let val = d.child_by_field_name("value");
            // A type literal's method signatures are the walk's too, where no
            // value takes the declarator.
            if let Some(t) = d.child_by_field_name("type") {
                let skip = val.is_some_and(|v| {
                    js_func_value(v)
                        || (top
                            && n.kind() == "lexical_declaration"
                            && js_const_value(d, v, exported))
                });
                if !skip {
                    self.js_statement(t, src, owner);
                }
            }
            let Some(val) = val else { continue };
            let func = val.kind() == "arrow_function"
                || val.kind() == "function_expression"
                || val.kind() == "function"
                || (val.kind() == "generator_function" && n.kind() == "lexical_declaration");
            if top
                && !func
                && n.kind() == "lexical_declaration"
                && self.js_const(d, val, exported, src)
            {
                continue;
            }
            if !top || !func {
                self.js_statement(val, src, owner);
                continue;
            }
            let name = field_text(d, "name", src);
            if degenerate_name(&name) {
                continue;
            }
            let id = self.b.id(&[&name]);
            self.b.def(&id, &name, &format!("{name}()"), line(d));
            self.js_fn_body(val.child_by_field_name("body"), &id, src);
            if n.kind() == "lexical_declaration" {
                self.js_member_scan(val.child_by_field_name("body"), &id, &name, &id, src);
            }
        }
    }

    /// Go `jsConst`: makes a node for a module-level const or let that is no function: an
    /// object, array, cast, call or new value, or any value of an exported plain name. A closure
    /// in a call is called by the const. It reports whether the value was taken.
    fn js_const(&mut self, d: Node<'_>, val: Node<'_>, exported: bool, src: &[u8]) -> bool {
        let Some(nm) = d.child_by_field_name("name") else {
            return false;
        };
        if !js_const_value(d, val, exported) {
            return false;
        }
        if nm.kind() == "object_pattern" && exported {
            for i in 0..nm.named_child_count() {
                let Some(pr) = nm.named_child(i) else {
                    continue;
                };
                let mut name = String::new();
                match pr.kind() {
                    "shorthand_property_identifier_pattern" => {
                        name = node_text(pr, src).into_owned();
                    }
                    "pair_pattern" => {
                        if let Some(k) = pr.child_by_field_name("key") {
                            name = node_text(k, src).into_owned();
                        }
                    }
                    _ => {}
                }
                if !degenerate_name(&name) {
                    let id = self.b.id(&[&name]);
                    self.const_def(&id, &name, line(d));
                }
            }
            return true;
        }
        let name = node_text(nm, src).into_owned();
        if degenerate_name(&name) {
            return true;
        }
        let id = self.b.id(&[&name]);
        self.const_def(&id, &name, line(d));
        let mut inner = val;
        while inner.kind() == "as_expression" || inner.kind() == "satisfies_expression" {
            let Some(c) = inner.named_child(0) else {
                return true;
            };
            inner = c;
        }
        if inner.kind() == "call_expression" || inner.kind() == "new_expression" {
            self.js_closures(inner, &id);
        }
        true
    }

    /// Go `constNode`: adds a node the file contains, with no Def: a namespace marker is no call
    /// target.
    fn const_node(&mut self, id: &str, name: &str, loc: Loc) {
        self.b.add_node(id, name, loc);
        self.b.contains(id, loc);
    }

    /// Go `constDef`: `const_node` for a const, which a call can bind.
    fn const_def(&mut self, id: &str, name: &str, loc: Loc) {
        self.const_node(id, name, loc);
        // Abstract: a function of the same name wins the call.
        self.b.res.defs.push(Def {
            id: id.to_string(),
            name: name.to_string(),
            file: self.b.file.clone(),
            is_abstract: true,
            value: true,
            ..Def::default()
        });
    }

    /// Go `jsClosures`: registers the outermost closures under `n` as bodies of `id`.
    fn js_closures(&mut self, n: Node<'_>, id: &str) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            match c.kind() {
                "arrow_function" | "function_expression" | "function" | "generator_function" => {
                    self.js_calls(c.child_by_field_name("body"), id);
                }
                _ => self.js_closures(c, id),
            }
        }
    }

    /// Go `jsMemberAssign`: captures `exports.x = fn`, `module.exports.x = fn` as a file
    /// function, `Foo.prototype.x = fn` as a method of Foo and `o.x = fn` as a method of o when
    /// the file exports o. Any other receiver names no owner: no node. It reports whether the
    /// statement was one.
    fn js_member_assign(&mut self, n: Node<'_>, src: &[u8]) -> bool {
        let Some(a) = first_named(n) else {
            return false;
        };
        if a.kind() != "assignment_expression" {
            return false;
        }
        let (Some(l), Some(r)) = (
            a.child_by_field_name("left"),
            a.child_by_field_name("right"),
        ) else {
            return false;
        };
        if l.kind() != "member_expression" || !js_func_value(r) {
            return false;
        }
        let name = field_text(l, "property", src);
        let Some(o) = l.child_by_field_name("object") else {
            return false;
        };
        if degenerate_name(&name) {
            return false;
        }
        let (kind, owner) = js_receiver(o, src);
        let id;
        if kind == "exports" {
            id = self.b.id(&[&name]);
            self.b.def(&id, &name, &format!("{name}()"), line(n));
        } else if kind == "prototype" {
            id = self.b.id(&[&owner, &name]);
            let oid = self.b.id(&[&owner]);
            self.js_proto.push(JsProto {
                id: id.clone(),
                owner,
                name: name.clone(),
                loc: line(n),
                oid,
            });
            self.b.res.defs.push(Def {
                id: id.clone(),
                name,
                file: self.b.file.clone(),
                ..Def::default()
            });
        } else if kind == "object"
            && let Some(prog) = n.parent()
            && prog.kind() == "program"
            && self.js_exported_obj(prog, &owner, src)
        {
            let oid = self.b.id(&[&owner]);
            self.js_owners.push(JsProto {
                id: oid.clone(),
                owner: owner.clone(),
                loc: line(n),
                ..JsProto::default()
            });
            id = self.b.id(&[&owner, &name]);
            self.js_method_node(&oid, &id, &owner, &name, line(n));
            self.b.res.defs.push(Def {
                id: id.clone(),
                name,
                file: self.b.file.clone(),
                export: true,
                ..Def::default()
            });
        } else {
            return false;
        }
        self.js_calls(r.child_by_field_name("body"), &id);
        true
    }

    /// Go `jsProtoOwners`: hangs each prototype member on its owner, or on the file when the
    /// file declares no such owner.
    fn js_proto_owners(&mut self) {
        for p in std::mem::take(&mut self.js_proto) {
            // The namespace's own owner first, then the file's.
            for oid in [p.oid.clone(), make_id(&[&self.b.stem, &p.owner])] {
                if self.b.seen.contains(&oid) {
                    self.js_method_node(&oid, &p.id, &p.owner, &p.name, p.loc);
                    break;
                }
            }
            if self.b.seen.contains(&p.id) {
                continue;
            }
            self.b
                .add_node(&p.id, &format!("{}.{}()", p.owner, p.name), p.loc);
            self.b.contains(&p.id, p.loc);
            // No call target: a bare name never reaches a prototype member.
            self.b.res.defs.retain(|d| d.id != p.id);
        }
    }

    /// Go `jsMethodNode`: adds the method node `id` of the owner node `owner_id`.
    fn js_method_node(&mut self, owner_id: &str, id: &str, owner: &str, name: &str, loc: Loc) {
        if self.b.seen.contains(id) {
            return;
        }
        self.b.add_node(id, &format!("{owner}.{name}()"), loc);
        push_contains(&mut self.b, owner_id, id, loc);
    }

    /// Go `jsExportedObj`: reports whether a module-level statement of `prog` exports `name`, as
    /// upstream's `_js_find_exported_objects` reads them.
    fn js_exported_obj(&mut self, prog: Node<'_>, name: &str, src: &[u8]) -> bool {
        if let Some(objs) = &self.js_exp_objs {
            return objs.contains(name);
        }
        let mut objs: HashSet<String> = HashSet::new();
        fn add(objs: &mut HashSet<String>, n: Option<Node<'_>>, src: &[u8]) {
            if let Some(n) = n
                && n.kind() == "identifier"
            {
                objs.insert(node_text(n, src).into_owned());
            }
        }
        fn is_exports(t: &str) -> bool {
            t == "module.exports"
                || t == "exports"
                || t.starts_with("module.exports.")
                || t.starts_with("exports.")
        }
        for i in 0..prog.named_child_count() {
            let Some(st) = prog.named_child(i) else {
                continue;
            };
            match st.kind() {
                "export_statement" => {
                    for j in 0..st.child_count() {
                        let Some(c) = st.child(j) else { continue };
                        match c.kind() {
                            "identifier" => add(&mut objs, Some(c), src),
                            "export_clause" => {
                                for k in 0..c.named_child_count() {
                                    let f = c
                                        .named_child(k)
                                        .and_then(|x| x.child_by_field_name("name"));
                                    add(&mut objs, f, src);
                                }
                            }
                            "lexical_declaration" | "variable_declaration" => {
                                for k in 0..c.named_child_count() {
                                    let f = c
                                        .named_child(k)
                                        .and_then(|x| x.child_by_field_name("name"));
                                    add(&mut objs, f, src);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    // `var x = module.exports = ...` or `var x = exports`.
                    for k in 0..st.named_child_count() {
                        let Some(d) = st.named_child(k) else { continue };
                        let (Some(nm), Some(v)) = (
                            d.child_by_field_name("name"),
                            d.child_by_field_name("value"),
                        ) else {
                            continue;
                        };
                        if nm.kind() != "identifier" {
                            continue;
                        }
                        let mut cur = Some(v);
                        while let Some(vv) = cur
                            && vv.kind() == "assignment_expression"
                        {
                            if let Some(l) = vv.child_by_field_name("left")
                                && is_exports(&node_text(l, src))
                            {
                                add(&mut objs, Some(nm), src);
                                break;
                            }
                            cur = vv.child_by_field_name("right");
                        }
                        if let Some(vv) = cur {
                            let t = node_text(vv, src);
                            if vv.kind() != "assignment_expression"
                                && (t == "module.exports" || t == "exports")
                            {
                                add(&mut objs, Some(nm), src);
                            }
                        }
                    }
                }
                "expression_statement" => {
                    let Some(a) = first_named(st) else { continue };
                    if a.kind() != "assignment_expression" {
                        continue;
                    }
                    let Some(l) = a.child_by_field_name("left") else {
                        continue;
                    };
                    if !is_exports(&node_text(l, src)) {
                        continue;
                    }
                    let mut v = a.child_by_field_name("right");
                    while let Some(vv) = v
                        && vv.kind() == "assignment_expression"
                    {
                        add(&mut objs, vv.child_by_field_name("left"), src);
                        v = vv.child_by_field_name("right");
                    }
                    if let Some(v) = v
                        && v.kind() == "identifier"
                    {
                        add(&mut objs, Some(v), src);
                    } else if let Some(v) = v
                        && v.kind() == "object"
                    {
                        for k in 0..v.named_child_count() {
                            let Some(pr) = v.named_child(k) else { continue };
                            match pr.kind() {
                                "shorthand_property_identifier" => {
                                    objs.insert(node_text(pr, src).into_owned());
                                }
                                "pair" => add(&mut objs, pr.child_by_field_name("value"), src),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let found = objs.contains(name);
        self.js_exp_objs = Some(objs);
        found
    }

    /// Go `jsMemberScan`: makes the members a function body assigns in its own statements:
    /// `this.x = fn` a method of `this_id` (`owner` its label, "" for the file: then x() is a
    /// file function), `o.x = fn` a method of o when o is an object literal the body declares, o
    /// itself under `scope`.
    fn js_member_scan(
        &mut self,
        body: Option<Node<'_>>,
        this_id: &str,
        owner: &str,
        scope: &str,
        src: &[u8],
    ) {
        let Some(body) = body else {
            return;
        };
        let mut objs: HashMap<String, Node<'_>> = HashMap::new();
        for i in 0..body.named_child_count() {
            let Some(st) = body.named_child(i) else {
                continue;
            };
            if st.kind() != "lexical_declaration" && st.kind() != "variable_declaration" {
                continue;
            }
            for j in 0..st.named_child_count() {
                let Some(d) = st.named_child(j) else { continue };
                let nm = d.child_by_field_name("name");
                let v = d.child_by_field_name("value");
                if d.kind() == "variable_declarator"
                    && let (Some(nm), Some(v)) = (nm, v)
                    && nm.kind() == "identifier"
                    && v.kind() == "object"
                {
                    objs.insert(node_text(nm, src).into_owned(), d);
                }
            }
        }
        for i in 0..body.named_child_count() {
            let Some(st) = body.named_child(i) else {
                continue;
            };
            let a = first_named(st);
            let Some(a) = a.filter(|a| {
                st.kind() == "expression_statement" && a.kind() == "assignment_expression"
            }) else {
                continue;
            };
            let (Some(l), Some(r)) = (
                a.child_by_field_name("left"),
                a.child_by_field_name("right"),
            ) else {
                continue;
            };
            if l.kind() != "member_expression" || !js_func_value(r) {
                continue;
            }
            let name = field_text(l, "property", src);
            let Some(o) = l.child_by_field_name("object") else {
                continue;
            };
            if degenerate_name(&name) {
                continue;
            }
            let (kind, on) = js_receiver(o, src);
            let id;
            if kind == "this" && owner.is_empty() {
                id = make_id(&[this_id, &name]);
                self.b.add_node(&id, &format!("{name}()"), line(st));
                push_contains(&mut self.b, this_id, &id, line(st));
            } else if kind == "this" {
                id = make_id(&[this_id, &name]);
                self.js_method_node(this_id, &id, owner, &name, line(st));
            } else if kind == "object"
                && let Some(od) = objs.get(&on).copied()
            {
                let oid = make_id(&[scope, &on]);
                if !self.b.seen.contains(&oid) {
                    self.b.add_node(&oid, &on, line(od));
                    push_contains(&mut self.b, scope, &oid, line(od));
                }
                id = make_id(&[&oid, &name]);
                self.js_method_node(&oid, &id, &on, &name, line(st));
            } else {
                continue;
            }
            self.js_calls(r.child_by_field_name("body"), &id);
        }
    }

    /// Go `jsLitMethod`: makes a node for a method the walk meets outside a class declaration:
    /// one of an object literal, a class expression or a type literal. Its owner is the
    /// module-level name the value is bound to, else the class expression's name, else the file.
    fn js_lit_method(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if degenerate_name(&name) {
            return;
        }
        // No Def: a literal's method is no call target of other files.
        let (mut owner, at) = js_lit_owner(n, src);
        let (id, this_id) = if owner.is_empty() {
            let id = self.b.id(&[&name]);
            if !self.b.seen.contains(&id) {
                self.b.add_node(&id, &format!("{name}()"), line(n));
                self.b.contains(&id, line(n));
            }
            (id.clone(), id)
        } else {
            let oid = self.b.id(&[&owner]);
            self.js_owners.push(JsProto {
                id: oid.clone(),
                owner: owner.clone(),
                loc: at,
                ..JsProto::default()
            });
            let id = self.b.id(&[&owner, &name]);
            self.js_method_node(&oid, &id, &owner, &name, line(n));
            (id, oid)
        };
        let body = n.child_by_field_name("body");
        self.js_fn_body(body, &id, src);
        if owner.is_empty() {
            owner = name;
        }
        self.js_member_scan(body, &this_id, &owner, &id, src);
    }

    /// Go `jsFnBody`: registers a function body and the functions declared inside it.
    fn js_fn_body(&mut self, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
        self.js_calls(body, caller_id);
        self.js_nested_funcs(body, caller_id, src);
    }

    /// Go `jsNestedFuncs`: makes a node for each function declared inside `n`, under `parent`;
    /// a closure passes through, so its declarations keep the parent. A method of a class or
    /// object met here is no tracked body: its calls drop.
    fn js_nested_funcs(&mut self, n: Option<Node<'_>>, parent: &str, src: &[u8]) {
        let Some(n) = n else {
            return;
        };
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            match c.kind() {
                "function_declaration" | "generator_function_declaration" => {
                    let name = field_text(c, "name", src);
                    if degenerate_name(&name) {
                        continue;
                    }
                    let id = make_id(&[parent, &name]);
                    if !self.b.seen.contains(&id) {
                        self.b.add_node(&id, &format!("{name}()"), line(c));
                        push_contains(&mut self.b, parent, &id, line(c));
                        self.js_scope(parent, &name, &id);
                    }
                    self.js_fn_body(c.child_by_field_name("body"), &id, src);
                }
                "method_definition" => {
                    self.js_calls(c.child_by_field_name("body"), "");
                    self.js_nested_funcs(Some(c), parent, src);
                }
                _ => self.js_nested_funcs(Some(c), parent, src),
            }
        }
    }

    /// Go `jsScope`: records that `id` is the function `name` declared inside `parent`. Such a
    /// function stays out of the global index: only calls from inside `parent` bind to it.
    fn js_scope(&mut self, parent: &str, name: &str, id: &str) {
        self.js_nested
            .insert((parent.to_string(), name.to_string()), id.to_string());
        self.js_parent.insert(id.to_string(), parent.to_string());
    }

    /// Go `jsCalls`: registers a function body; [`extract_js`] records its call sites in the one
    /// walk it makes over the file.
    fn js_calls(&mut self, body: Option<Node<'_>>, caller_id: &str) {
        let Some(body) = body else {
            return;
        };
        // The first owner of a body keeps it: a member scan runs before the walk.
        if !self.js_body_seen.insert(body.start_byte()) {
            return;
        }
        self.js_bodies.push(JsBody {
            start: body.start_byte(),
            end: body.end_byte(),
            caller_id: caller_id.to_string(),
            ns: self.b.js_ns.clone(),
        });
    }

    /// Go `jsCall`: records the call site `c`, a call_expression or new_expression, for the
    /// body's caller.
    fn js_call(&mut self, c: Node<'_>, body: &JsBody, src: &[u8], in_method: bool) {
        let caller_id = &body.caller_id;
        let is_new = c.kind() == "new_expression";
        let f = if is_new {
            c.child_by_field_name("constructor")
        } else {
            c.child_by_field_name("function")
        };
        let Some(f) = f else {
            return;
        };
        if caller_id.is_empty() {
            return;
        }
        // A method body is a boundary: its calls are no caller's.
        if in_method {
            return;
        }
        match f.kind() {
            "identifier" => {
                let name = node_text(f, src).into_owned();
                // new binds in the resolver
                let mut s = caller_id.clone();
                while !s.is_empty() && !is_new {
                    if let Some(id) = self.js_nested.get(&(s.clone(), name.clone())).cloned() {
                        js_nested_call(&mut self.b, &mut self.js_linked, caller_id, &id, line(c));
                        return;
                    }
                    s = self.js_parent.get(&s).cloned().unwrap_or_default();
                }
                if is_new && JS_NEW_BUILTINS.contains(name.as_str()) {
                    return;
                }
                // Innermost namespace first; a name in none is a plain global call.
                for k in (1..=body.ns.len()).rev() {
                    let mut parts: Vec<&str> = vec![self.b.stem.as_str(), "ns"];
                    parts.extend(body.ns[..k].iter().map(String::as_str));
                    parts.push(&name);
                    let id = make_id(&parts);
                    if self.b.seen.contains(&id) && !self.js_bare.contains(&id) {
                        // Own-member new X() makes no edge
                        if !is_new
                            || !self.b.res.edges.iter().any(|e| {
                                e.source == id
                                    && e.target == *caller_id
                                    && e.relation == Relation::CONTAINS
                            })
                        {
                            js_nested_call(
                                &mut self.b,
                                &mut self.js_linked,
                                caller_id,
                                &id,
                                line(c),
                            );
                        }
                        return;
                    }
                }
                if is_new {
                    // contains edge already links the pair
                    if *caller_id != self.b.file_id || !self.js_file_owns(&name) {
                        self.b.call_recv(caller_id, &name, JS_NEW_RECV, line(c));
                    }
                    return;
                }
                // The file's contains edge to its own function hides this call upstream.
                if *caller_id == self.b.file_id && self.js_file_owns(&name) {
                    return;
                }
                self.b.call(caller_id, &name, line(c));
            }
            "member_expression" => {
                if let Some(p) = f.child_by_field_name("property")
                    && !is_new
                {
                    self.b.call_recv(
                        caller_id,
                        &node_text(p, src),
                        &recv_text(Some(f), Some(p), src),
                        line(c),
                    );
                }
            }
            _ => {}
        }
    }

    /// Go `jsFileOwns`: reports whether the file contains a definition called `name`.
    fn js_file_owns(&mut self, name: &str) -> bool {
        if self.js_owned.is_none() {
            let own: HashSet<&str> = self
                .b
                .res
                .edges
                .iter()
                .filter(|e| e.source == self.b.file_id && e.relation == Relation::CONTAINS)
                .map(|e| e.target.as_str())
                .collect();
            let mut owned = HashSet::new();
            for ds in [&self.b.res.defs, &self.b.js_ns_defs] {
                for d in ds {
                    if own.contains(d.id.as_str()) {
                        owned.insert(d.name.clone());
                    }
                }
            }
            self.js_owned = Some(owned);
        }
        self.js_owned.as_ref().is_some_and(|o| o.contains(name))
    }
}

/// Go `jsInSignature`: reports whether `n` lies in the params or type of a nested callable,
/// which upstream's closure walk skips.
fn js_in_signature(n: Node<'_>, body: &JsBody) -> bool {
    let (mut c, mut p) = (n, Some(n));
    while let Some(pp) = p {
        if pp.start_byte() == body.start && pp.end_byte() == body.end {
            break;
        }
        match pp.kind() {
            "method_definition" => return true,
            "function_declaration" | "generator_function_declaration" => {
                match pp.child_by_field_name("body") {
                    Some(bd)
                        if bd.start_byte() == c.start_byte() && bd.end_byte() == c.end_byte() => {}
                    _ => return true,
                }
            }
            _ => {}
        }
        (c, p) = (pp, pp.parent());
    }
    false
}

/// Go `jsImportSpec`: the first literal argument of an import(), "" if computed.
fn js_import_spec(args: Node<'_>, src: &[u8]) -> String {
    for i in 0..args.named_child_count() {
        let Some(a) = args.named_child(i) else {
            continue;
        };
        match a.kind() {
            "string" => return node_text(a, src).trim_matches(['\'', '"', ' ']).to_string(),
            "template_string" => {
                for j in 0..a.named_child_count() {
                    if a.named_child(j)
                        .is_some_and(|c| c.kind() == "template_substitution")
                    {
                        return String::new();
                    }
                }
                return node_text(a, src).trim_matches('`').to_string();
            }
            _ => {}
        }
    }
    String::new()
}

/// Go `jsTypeOnlyImport`: whether an import or re-export statement is a whole-statement
/// `import type { T } from "m"`. The `type` keyword is a direct child only in that form; for a
/// mixed `import { type A, B }` tree-sitter nests it inside the specifier, so a direct-child
/// check keeps mixed imports as value imports.
fn js_type_only_import(n: Node<'_>) -> bool {
    (0..n.child_count()).any(|i| n.child(i).is_some_and(|c| c.kind() == "type"))
}

/// Go `jsHasDefault`.
fn js_has_default(n: Node<'_>) -> bool {
    child_by_kind(n, "default").is_some()
}

/// The `spec1` closure of Go `jsImp`: the entry one specifier `sp` adds to `names`.
fn js_imp_spec(sp: Node<'_>, src: &[u8], names: &mut Vec<String>) {
    let Some(name) = sp.child_by_field_name("name") else {
        return;
    };
    if name.kind() == "string" {
        return;
    }
    let mut e = node_text(name, src).into_owned();
    let a = field_text(sp, "alias", src);
    if !a.is_empty() && a != e {
        e.push_str(" as ");
        e.push_str(&a);
    }
    names.push(e);
}

/// Go `jsFuncValue`: whether `n` is a function value.
fn js_func_value(n: Node<'_>) -> bool {
    matches!(
        n.kind(),
        "arrow_function" | "function_expression" | "function" | "generator_function"
    )
}

/// Go `jsConstValue`: whether jsConst takes the declarator `d`.
fn js_const_value(d: Node<'_>, val: Node<'_>, exported: bool) -> bool {
    if matches!(
        val.kind(),
        "object"
            | "array"
            | "as_expression"
            | "satisfies_expression"
            | "call_expression"
            | "new_expression"
    ) {
        return d.child_by_field_name("name").is_some();
    }
    exported
        && d.child_by_field_name("name")
            .is_some_and(|nm| nm.kind() == "identifier")
}

/// Go `jsReceiver`: classifies the object of a member assignment: "this", "exports" (also
/// module.exports), "prototype" with the owner of Foo.prototype, "object" with an identifier's
/// name, or "" for any other.
fn js_receiver(o: Node<'_>, src: &[u8]) -> (&'static str, String) {
    match o.kind() {
        "this" => ("this", String::new()),
        "identifier" => {
            let t = node_text(o, src);
            if t != "exports" {
                ("object", t.into_owned())
            } else {
                ("exports", String::new())
            }
        }
        "member_expression" => {
            let io = o.child_by_field_name("object");
            let p = field_text(o, "property", src);
            let Some(io) = io.filter(|x| x.kind() == "identifier") else {
                return ("", String::new());
            };
            let t = node_text(io, src);
            if t == "module" && p == "exports" {
                ("exports", String::new())
            } else if p == "prototype" {
                ("prototype", t.into_owned())
            } else {
                ("", String::new())
            }
        }
        _ => ("", String::new()),
    }
}

/// Go `jsTopClosures`: collects the outermost closures under `n`.
fn js_top_closures<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if js_func_value(c) {
            out.push(c);
        } else {
            js_top_closures(c, out);
        }
    }
}

/// Go `jsLitOwner`: names the owner of the method `n` and the line of its binding. Only a
/// module-level binding owns: a local one is not seen from outside.
fn js_lit_owner(n: Node<'_>, src: &[u8]) -> (String, Loc) {
    let Some(mut cur) = n.parent() else {
        return (String::new(), Loc::default());
    };
    let mut cls = String::new();
    if cur.kind() == "class_body" {
        let Some(g) = cur.parent() else {
            return (String::new(), Loc::default());
        };
        cur = g;
        cls = field_text(cur, "name", src);
    }
    let mut p = cur.parent();
    while let Some(pp) = p {
        match pp.kind() {
            "pair"
            | "object"
            | "array"
            | "parenthesized_expression"
            | "as_expression"
            | "satisfies_expression"
            | "ternary_expression"
            | "type_annotation"
            | "object_type" => {
                (cur, p) = (pp, pp.parent());
                continue;
            }
            "variable_declarator" => {
                let mut d = pp.parent();
                if let Some(dd) = d
                    && let Some(g) = dd.parent()
                    && g.kind() == "export_statement"
                {
                    d = Some(g);
                }
                if let Some(nm) = pp.child_by_field_name("name")
                    && nm.kind() == "identifier"
                    && d.and_then(|x| x.parent())
                        .is_some_and(|g| g.kind() == "program")
                {
                    return (node_text(nm, src).into_owned(), line(pp));
                }
            }
            "assignment_expression" => {
                let l = pp.child_by_field_name("left");
                let st = pp.parent();
                if let Some(l) = l
                    && let Some(st) = st
                    && st.kind() == "expression_statement"
                    && st.parent().is_some_and(|g| g.kind() == "program")
                {
                    if l.kind() == "identifier" {
                        return (node_text(l, src).into_owned(), line(pp));
                    }
                    if let Some(o) = l.child_by_field_name("object")
                        && l.kind() == "member_expression"
                    {
                        match js_receiver(o, src) {
                            ("exports", _) => {
                                return (field_text(l, "property", src), line(pp));
                            }
                            ("prototype", on) => return (on, line(pp)),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        break;
    }
    if !degenerate_name(&cls) {
        return (cls, line(cur));
    }
    (String::new(), Loc::default())
}

/// Go `(*builder).jsRationale`: a deterministic post-pass mirroring upstream
/// `_extract_js_rationale`: `// NOTE:`-style comments become "rationale" nodes (rationale_for
/// edge to the file) and ADR/RFC tokens in comments become "doc_ref" nodes (cites edge from the
/// file). No LLM is involved.
pub(crate) fn js_rationale(b: &mut Builder, src: &[u8]) {
    let mut seen_doc_refs = BTreeSet::new();
    for (i, text) in src.split(|&c| c == b'\n').enumerate() {
        let line_num = i as i64 + 1;
        // Every prefix and comment marker starts with / or * after the indent.
        let t = bytes_trim_space(text);
        if !t.is_empty() && (t[0] == b'/' || t[0] == b'*') {
            let stripped = go_text(t);
            for p in JS_RATIONALE_PREFIXES {
                if stripped.starts_with(p) {
                    let file_id = b.file_id.clone();
                    b.add_rationale(
                        stripped.trim_start_matches(['/', '*', ' ']),
                        line_num,
                        &file_id,
                    );
                    break;
                }
            }
            if may_cite_doc(&stripped) && JS_COMMENT_LINE_RE.match_string(&go_text(text)) {
                for m in JS_DOC_REF_RE.find_all_string_submatch(&stripped, -1) {
                    add_doc_ref(b, m[1].unwrap_or(""), line_num, &mut seen_doc_refs);
                }
            }
        }
    }
}

/// Go `mayCiteDoc`: reports whether `s` holds "adr" or "rfc" in any case, as every jsDocRefRe
/// match does.
fn may_cite_doc(s: &str) -> bool {
    let s = s.as_bytes();
    for i in 1..s.len().saturating_sub(1) {
        match s[i] | 0x20 {
            b'd' if s[i - 1] | 0x20 == b'a' && s[i + 1] | 0x20 == b'r' => return true,
            b'f' if s[i - 1] | 0x20 == b'r' && s[i + 1] | 0x20 == b'c' => return true,
            _ => {}
        }
    }
    false
}

/// Go `(*builder).addDocRef`: records a "doc_ref" node for a normalized ADR/RFC reference and a
/// cites edge from the file to it. Labels are normalized (ADR-0011, RFC-793) and deduped per
/// file so repeated citations collapse to one node. Mirrors upstream `_add_doc_ref`.
pub(crate) fn add_doc_ref(
    b: &mut Builder,
    token: &str,
    line_num: i64,
    seen_doc_refs: &mut BTreeSet<String>,
) {
    let Some(m) = JS_DOC_REF_PARSE_RE.find_string_submatch(token) else {
        return;
    };
    let (kind, num) = (to_upper(m[1].unwrap_or("")), m[2].unwrap_or(""));
    let mut label = format!("{kind}-{num}");
    if kind == "ADR" {
        label = format!("{kind}-{}", zfill(num, 4));
    }
    if !seen_doc_refs.insert(label.clone()) {
        return;
    }
    let rid = make_id(&["docref", &label]);
    let loc = model_line(line_num);
    if b.seen.insert(rid.clone()) {
        b.res.nodes.push(ModelNode {
            id: rid.clone(),
            label,
            file_type: "doc_ref".to_string(),
            source_file: b.file.clone(),
            source_location: loc.to_string(),
            ..ModelNode::default()
        });
    }
    b.res.edges.push(Edge {
        source: b.file_id.clone(),
        target: rid,
        relation: Relation::CITES,
        confidence: Confidence::EXTRACTED,
        source_file: b.src,
        source_location: loc,
        ..Edge::default()
    });
}

/// Go `zfill`: left-pads `s` with zeros to at least `width` characters (Python `str.zfill` for
/// unsigned numeric strings).
fn zfill(s: &str, width: usize) -> String {
    if s.len() >= width {
        return s.to_string();
    }
    "0".repeat(width - s.len()) + s
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let names = [
        "call_expression",
        "new_expression",
        "method_definition",
        "export_statement",
    ];
    let masks = [
        gositter_grammars::javascript::language(),
        gositter_grammars::typescript::language(),
        gositter_grammars::tsx::language(),
    ]
    .map(|l| names.map(|k| l.kind_mask(&[k])));
    let _ = JS_MASKS.set(masks);
}

/// Go `jsNestedCall` (`javascript.go:1401`): binds a call to a nested function seen from its
/// scope, once per (caller, callee). Go keeps `jsLinked` on the builder; here each language
/// passes its own set. Shared with python.go.
pub(crate) fn js_nested_call(
    b: &mut super::builder::Builder,
    linked: &mut std::collections::HashSet<(String, String)>,
    caller_id: &str,
    id: &str,
    loc: crate::model::Loc,
) {
    if caller_id == id || !linked.insert((caller_id.to_string(), id.to_string())) {
        return;
    }
    b.res.edges.push(crate::model::Edge {
        source: caller_id.to_string(),
        target: id.to_string(),
        relation: crate::model::Relation::CALLS,
        confidence: crate::model::Confidence::EXTRACTED,
        source_file: b.src,
        source_location: loc,
        ..crate::model::Edge::default()
    });
}

#[cfg(test)]
#[path = "tests/jsts_parity.rs"]
mod jsts_parity_tests;

#[cfg(test)]
#[path = "tests/javascript.rs"]
mod javascript_tests;

#[cfg(test)]
#[path = "tests/jscalls.rs"]
mod jscalls_tests;

#[cfg(test)]
#[path = "tests/jsts_imports.rs"]
mod jsts_imports_tests;

#[cfg(test)]
#[path = "tests/js_type_only.rs"]
mod js_type_only_tests;
