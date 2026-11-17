//! Port of `csharp_resolve.go`: binds the receiver codes `csharp.rs` writes.
//!
//! The functions `resolve.rs` calls ([`new_cs_index`], [`CsIndex::bind`], [`CsMemo`]) are
//! shared API (graphify-rs-ihc.18). Their signatures are proposed here, Go-shaped; the resolve
//! owner may change them.
//!
//! Go's maps become `HashMap`/`HashSet` here: no iteration of one reaches the output (the one
//! range, over `byName` at `csharp_resolve.go:208`, fills a first-wins map whose keys differ per
//! name). Go's `*csTwin` pointers are indices into [`CsIndex`]'s twin store.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, OnceLock};

use gostd::path;
use gostd::regexp::Regexp;
use gostd::strings::{equal_fold, to_lower, trim_space};
use gostd::unicode::{is_letter, is_number};

use super::extract::Result;
use super::resolve::{contains, keep};
use crate::model::{Confidence, Edge, Relation};

/// Go `csTwin`: one of the generic twins upstream keeps apart, its own methods and base names,
/// kept for the one a type name stands for.
struct CsTwin {
    kept: bool,
    methods: Vec<String>,
    bases: Vec<String>,
}

/// A twin: an index into `CsIndex::twin_store` (Go `*csTwin`).
type Twin = Option<usize>;

/// Go `csIndex`: binds the receiver codes `csCalls` writes: a C# member call on its receiver's
/// type, a `new X()` on type X. It is read by every bind run, so nothing in it changes once made
/// (`foreign` is filled on first use, once).
///
/// Shared with resolve.rs (graphify-rs-ihc.18): signature proposed here; the resolve owner may change it.
pub(crate) struct CsIndex<'a> {
    /// top-level type id -> namespace
    ns: HashMap<String, String>,
    /// type name -> type ids, by file, once per upstream node
    by_name: HashMap<String, Vec<String>>,
    /// name\0namespace -> first such id of by_name
    by_ns: HashMap<String, String>,
    /// cs_key of type name -> type ids
    by_key: HashMap<String, Vec<String>>,
    /// type id\0cs_key of method name -> method ids
    mkey: HashMap<String, Vec<String>>,
    /// type id\0field -> type id\0field type, as self_target reads
    fields: HashMap<String, Vec<String>>,
    /// type id -> the namespaces its usings bring in
    usings: HashMap<String, Vec<String>>,
    /// type id\0alias -> the names its usings alias
    alias: HashMap<String, Vec<String>>,
    /// file\0name a using of the file aliases
    aliased: HashSet<String>,
    /// file\0alias a using of the file declares, and file
    alias_in: HashSet<String>,
    /// type id\0name of a type nested in it
    nested: HashSet<String>,
    /// name -> files with a nested type of it
    nested_in: HashMap<String, Vec<String>>,
    /// generic type ids an in-file stub hides
    shadow: HashSet<String>,
    /// file, a name its type refs make a stub of
    stubs: HashSet<[String; 2]>,
    /// source\0name of a C# type ref
    named: HashSet<String>,
    /// source\0target of a C# type ref edge
    refs: HashSet<String>,
    /// method -> its type
    owner: &'a HashMap<String, String>,
    local: &'a HashMap<[String; 2], Vec<String>>,
    id_file: &'a HashMap<String, String>,
    methods: &'a HashMap<String, Vec<String>>,
    supers: &'a HashMap<String, Vec<String>>,
    open_super: &'a HashSet<String>,
    /// upstream's C# resolvers need a .cs file
    on: bool,
    /// type id -> its same-file generic twins
    twins: HashMap<String, Vec<usize>>,
    twin_store: Vec<CsTwin>,
    /// types of .CS files, upstream's index skips them
    upper: HashMap<String, bool>,
    /// method id -> its name
    mname: HashMap<String, String>,
    /// cs_key -> other languages' types upstream counts (Go `sync.OnceValue`)
    foreign: OnceLock<HashMap<String, usize>>,
    results: &'a [Result],
    /// test files, by upstream's rule
    test: HashMap<String, bool>,
}

/// `m[k]` of a Go `map[string][]string`: nil (empty) when absent.
fn list<'m>(m: &'m HashMap<String, Vec<String>>, k: &str) -> &'m [String] {
    m.get(k).map_or(&[], Vec::as_slice)
}

/// `m[k]` of a Go `map[string]string`: "" when absent.
fn text<'m>(m: &'m HashMap<String, String>, k: &str) -> &'m str {
    m.get(k).map_or("", String::as_str)
}

/// Go `unique` (`resolve.go:1653`): the only id matching `pred`, or "" if zero or more than one
/// do. resolve.rs has no port of it yet (graphify-rs-ihc.18); this private copy can go once it
/// does.
fn unique(ids: &[String], pred: impl Fn(&str) -> bool) -> String {
    let mut found = "";
    for id in ids {
        if pred(id) {
            if !found.is_empty() {
                return String::new();
            }
            found = id;
        }
    }
    found.to_string()
}

/// Go `newCSIndex` (`csharp_resolve.go:55`). Go passes the shared maps by reference; so does
/// this.
///
/// Shared with resolve.rs (graphify-rs-ihc.18): signature proposed here; the resolve owner may change it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_cs_index<'a>(
    results: &'a [Result],
    inherit: &[Edge],
    owner: &'a HashMap<String, String>,
    local: &'a HashMap<[String; 2], Vec<String>>,
    id_file: &'a HashMap<String, String>,
    methods: &'a HashMap<String, Vec<String>>,
    supers: &'a HashMap<String, Vec<String>>,
    open_super: &'a HashSet<String>,
) -> CsIndex<'a> {
    let mut x = CsIndex {
        ns: HashMap::new(),
        by_name: HashMap::new(),
        by_ns: HashMap::new(),
        by_key: HashMap::new(),
        mkey: HashMap::new(),
        fields: HashMap::new(),
        usings: HashMap::new(),
        alias: HashMap::new(),
        aliased: HashSet::new(),
        alias_in: HashSet::new(),
        nested: HashSet::new(),
        nested_in: HashMap::new(),
        shadow: HashSet::new(),
        stubs: HashSet::new(),
        named: HashSet::new(),
        refs: HashSet::new(),
        owner,
        local,
        id_file,
        methods,
        supers,
        open_super,
        on: false,
        twins: HashMap::new(),
        twin_store: Vec::new(),
        upper: HashMap::new(),
        mname: HashMap::new(),
        foreign: OnceLock::new(),
        results,
        test: HashMap::new(),
    };
    // One upstream node per namespace, arity.
    let mut decls: HashMap<String, usize> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    for r in results {
        let ext = path::ext(&r.file);
        x.on = x.on || ext == ".cs";
        if !equal_fold(ext, ".cs") {
            continue;
        }
        // Upstream skips .CS in index, usings, bases.
        let upper = ext != ".cs";
        x.test.insert(r.file.clone(), cs_test_path(&r.file));
        for e in &r.cs_types {
            let parts: Vec<&str> = e.split(' ').collect();
            let id = parts[0];
            // Go indexes parts[1]; every entry holds "id ns".
            let ns = parts.get(1).copied().unwrap_or("");
            x.ns.insert(id.to_string(), ns.to_string());
            x.upper.insert(id.to_string(), upper);
            let mut arity = "";
            let mut tw: Twin = None;
            for &f in parts.iter().skip(2) {
                let Some(&c) = f.as_bytes().first() else {
                    continue;
                };
                // Go's f[1:]; read only after an ASCII first byte (a field may start with any rune).
                let rest = f.get(1..).unwrap_or("");
                match c {
                    b'+' => {
                        if !upper {
                            x.usings
                                .entry(id.to_string())
                                .or_default()
                                .push(rest.to_string());
                        }
                        continue;
                    }
                    b'&' => {
                        if upper {
                            continue;
                        }
                        let (a, t) = rest.split_once('=').unwrap_or((rest, ""));
                        x.alias
                            .entry(format!("{id}\0{a}"))
                            .or_default()
                            .push(t.to_string());
                        x.aliased.insert(format!("{}\0{t}", r.file));
                        x.alias_in.insert(format!("{}\0{a}", r.file));
                        x.alias_in.insert(r.file.clone());
                        continue;
                    }
                    b'^' => {
                        x.nested.insert(format!("{id}\0{rest}"));
                        let files = x.nested_in.entry(rest.to_string()).or_default();
                        if !contains(files, &r.file) {
                            // One upstream node per file.
                            files.push(r.file.clone());
                        }
                        continue;
                    }
                    b'`' => {
                        arity = f;
                        continue;
                    }
                    b'~' => {
                        x.shadow.insert(id.to_string());
                        continue;
                    }
                    b'*' => {
                        x.twin_store.push(CsTwin {
                            kept: f == "*!",
                            methods: Vec::new(),
                            bases: Vec::new(),
                        });
                        let i = x.twin_store.len() - 1;
                        tw = Some(i);
                        x.twins.entry(id.to_string()).or_default().push(i);
                        continue;
                    }
                    b'.' => {
                        // csTwins writes `*` before any `.`; Go would panic on a nil twin.
                        if let Some(i) = tw {
                            x.twin_store[i].methods.push(cs_key(rest));
                        }
                        continue;
                    }
                    b':' => {
                        if let Some(i) = tw {
                            x.twin_store[i].bases.push(rest.to_string());
                        }
                        continue;
                    }
                    _ => {}
                }
                let (mut name, typ) = f.split_once('=').unwrap_or((f, ""));
                let mut val = format!("{id}\0{typ}");
                if let Some(base) = name.strip_suffix("[]") {
                    // Upstream skips list fields here.
                    name = base;
                    val = format!("{id}\0[]");
                }
                let vals = x.fields.entry(format!("{id}\0{name}")).or_default();
                if !contains(vals, &val) {
                    vals.push(val);
                }
            }
            if seen.insert(format!("{id} {ns}{arity}")) {
                *decls.entry(id.to_string()).or_default() += 1;
            }
        }
        for d in &r.defs {
            let o = text(owner, &d.id);
            if !o.is_empty() {
                let key = cs_key(&d.name);
                x.mname.insert(d.id.clone(), key.clone());
                let ids = x.mkey.entry(format!("{o}\0{key}")).or_default();
                if !contains(ids, &d.id) {
                    ids.push(d.id.clone());
                }
            }
            let n = decls.get(&d.id).copied().unwrap_or(0);
            if n > 0 && !contains(list(&x.by_name, &d.name), &d.id) {
                let key = cs_key(&d.name);
                for _ in 0..n {
                    x.by_name
                        .entry(d.name.clone())
                        .or_default()
                        .push(d.id.clone());
                    x.by_key.entry(key.clone()).or_default().push(d.id.clone());
                }
            }
        }
        for t in &r.type_refs {
            if !t.arg {
                x.stubs.insert([r.file.clone(), t.name.clone()]);
            }
            x.named.insert(format!("{}\0{}", t.from_id, t.name));
        }
    }
    for (name, ids) in &mut x.by_name {
        // Go `slices.SortStableFunc` by file.
        ids.sort_by_key(|a| text(id_file, a));
        for id in ids.iter() {
            let k = format!("{name}\0{}", text(&x.ns, id));
            if !x.upper.get(id).copied().unwrap_or(false) && !x.by_ns.contains_key(&k) {
                x.by_ns.insert(k, id.clone());
            }
        }
    }
    for e in inherit {
        if equal_fold(path::ext(e.source_file.as_str()), ".cs") {
            x.refs.insert(format!("{}\0{}", e.source, e.target));
        }
    }
    x
}

/// Go `csMemo`: caches the `new X()` work of one bind run.
///
/// Shared with resolve.rs (graphify-rs-ihc.18): signature proposed here; the resolve owner may change it.
#[derive(Default)]
pub(crate) struct CsMemo {
    /// file, name, qualifier -> its pick
    picks: HashMap<[String; 3], CsPick>,
    /// name -> its candidates, indexed
    cands: HashMap<String, CsCands>,
}

/// Go `csPick`: a cached choice among the types of one name.
#[derive(Clone, Default)]
struct CsPick {
    tgt: String,
    done: bool,
}

impl CsIndex<'_> {
    /// Go `foreign()`: upstream's unique-name fallback counts every language's types. Built on
    /// first use: it walks every non-C# file. Go scans the files on GOMAXPROCS goroutines and
    /// merges in file order; the result is the same one file at a time.
    fn foreign(&self) -> &HashMap<String, usize> {
        self.foreign.get_or_init(|| {
            let mut foreign: HashMap<String, usize> = HashMap::new();
            if !self.on {
                return foreign;
            }
            // ponytail: one thread; fan out over `workers` if this scan shows in profiles.
            let mut counted: HashSet<&str> = HashSet::new();
            for r in self.results {
                if equal_fold(path::ext(&r.file), ".cs") {
                    continue;
                }
                let mut contained: Option<HashSet<&str>> = None;
                for nd in &r.nodes {
                    let l = trim_space(&nd.label);
                    if nd.source_file.is_empty()
                        || nd.file_type != "code"
                        || l.is_empty()
                        || l.ends_with(')')
                        || l.contains('.')
                    {
                        continue;
                    }
                    let contained = contained.get_or_insert_with(|| {
                        r.edges
                            .iter()
                            .filter(|e| e.relation == Relation::CONTAINS)
                            .map(|e| e.target.as_str())
                            .collect()
                    });
                    if contained.contains(nd.id.as_str()) && counted.insert(nd.id.as_str()) {
                        *foreign.entry(cs_key(l)).or_default() += 1;
                    }
                }
            }
            foreign
        })
    }

    /// Go `resolve`: binds a type name seen from type `from` the way upstream's namespace
    /// resolver does: an alias of the name in from's file decides alone, by the ones in from's
    /// scope, else from's namespace, the global one and its usings (one hit, or none when
    /// several), then the parent namespaces, then a name unique in the corpus: "^" when that is a
    /// nested type, which has no node.
    // ponytail: no `using static`; aliases are seen only through types.
    fn resolve(&self, name: &str, from: &str, unique: bool) -> String {
        let in_ns = |name: &str, ns: &str| text(&self.by_ns, &format!("{name}\0{ns}")).to_string();
        let ns = text(&self.ns, from);
        let mut keys: Vec<(String, String)> = Vec::new();
        let aliases = list(&self.alias, &format!("{from}\0{name}"));
        let aliased = self
            .alias_in
            .contains(&format!("{}\0{name}", text(self.id_file, from)));
        for a in aliases {
            let Some(i) = a.rfind('.') else {
                // Upstream never resolves a dotless alias.
                continue;
            };
            keys.push((a[i + 1..].to_string(), a[..i].to_string()));
        }
        if !aliased {
            keys.push((name.to_string(), ns.to_string()));
            keys.push((name.to_string(), String::new()));
            for s in list(&self.usings, from) {
                keys.push((name.to_string(), s.clone()));
            }
        }
        let mut hit = String::new();
        for (k0, k1) in &keys {
            let id = in_ns(k0, k1);
            if !id.is_empty() && id != hit {
                if !hit.is_empty() {
                    return String::new();
                }
                hit = id;
            }
        }
        if !hit.is_empty() || aliased {
            return hit;
        }
        let mut n = ns;
        while let Some(i) = n.rfind('.') {
            n = &n[..i];
            let id = in_ns(name, n);
            if !id.is_empty() {
                return id;
            }
        }
        let name = cs_key(name);
        let ids = list(&self.by_key, &name);
        if !unique
            || ids.len()
                + list(&self.nested_in, &name).len()
                + self.foreign().get(&name).copied().unwrap_or(0)
                != 1
        {
            return String::new();
        }
        if ids.is_empty() {
            return "^".to_string();
        }
        ids[0].clone()
    }

    /// Go `bind` (`csharp_resolve.go:309`): the target of a C# call with a receiver code made in
    /// `file`, and its confidence.
    ///
    /// Shared with resolve.rs (graphify-rs-ihc.18): signature proposed here; the resolve owner may change it.
    pub(crate) fn bind(
        &self,
        m: &mut CsMemo,
        file: &str,
        caller: &str,
        callee: &str,
        recv: &str,
    ) -> (String, Confidence) {
        let enc = text(self.owner, caller);
        let mut t = String::new();
        let mut conf = Confidence::INFERRED;
        let mut tw: Twin = None;
        if recv == "new" || recv.starts_with("new ") {
            let prefix = recv.strip_prefix("new").unwrap_or(recv);
            let prefix = prefix.strip_prefix(' ').unwrap_or(prefix);
            let (t, conf) = self.bind_new(m, file, caller, callee, prefix, enc);
            if t == enc {
                // Upstream's contains edge wins.
                return (String::new(), conf);
            }
            return (t, conf);
        } else if !self.on {
            // Only member calls need a .cs file.
        } else if recv == "this" {
            t = enc.to_string();
            conf = Confidence::EXTRACTED;
            tw = self.twin_of(enc, caller);
        } else if recv == "base" {
            if let Some(bs) = self.bases(enc, self.twin_of(enc, caller))
                && bs.len() == 1
            {
                t = bs[0].clone();
                conf = Confidence::EXTRACTED;
            }
        } else if let Some(r) = recv.strip_prefix('!') {
            let (u, typ) = r.split_once(' ').unwrap_or(("", r));
            t = self.resolve(u, enc, true);
            if t.is_empty() {
                t = self.resolve(typ, enc, true);
            }
            conf = Confidence::EXTRACTED;
        } else if let Some(r) = recv.strip_prefix('?') {
            let (u, f, chain) = match r.split_once(' ') {
                Some((u, f)) => (u, f, true),
                None => (r, "", false),
            };
            t = self.resolve(u, enc, true);
            if !t.is_empty() && t != "^" && chain {
                let hit = self.lookup(&t, None, f, &self.fields);
                let (o, typ) = hit.split_once('\0').unwrap_or((&hit, ""));
                t = self.resolve(typ, o, true);
            }
        }
        if t.is_empty() || t == "^" {
            return (String::new(), conf);
        }
        (self.lookup(&t, tw, &cs_key(callee), &self.mkey), conf)
    }

    /// Go `lookup`: upstream's search for `key` on type `t` and its bases: a branch stops where
    /// it is declared, one distinct hit wins, and an open base the walk reaches spoils it. A
    /// twinned type is its twin `d`, else the kept one.
    fn lookup(&self, t: &str, d: Twin, key: &str, table: &HashMap<String, Vec<String>>) -> String {
        let mut hits: Vec<String> = Vec::new();
        let mut seen: HashSet<(String, Twin)> = HashSet::new();
        let mut q: Vec<(String, Twin)> = vec![(t.to_string(), d)];
        while let Some((id, mut d)) = q.pop() {
            if d.is_none() {
                d = self.twin_of(&id, "");
            }
            if !seen.insert((id.clone(), d)) {
                continue;
            }
            let ids = list(table, &format!("{id}\0{key}"));
            if !ids.is_empty() && d.is_none_or(|i| contains(&self.twin_store[i].methods, key)) {
                for h in ids {
                    if !contains(&hits, h) {
                        hits.push(h.clone());
                    }
                }
                continue;
            }
            let Some(bs) = self.bases(&id, d) else {
                return String::new();
            };
            for b in bs {
                q.push((b, None));
            }
        }
        if hits.len() != 1 {
            return String::new();
        }
        hits.swap_remove(0)
    }

    /// Go `twinOf`: the twin of type `t` that declares method `caller`, else the kept one, `None`
    /// for a type with no twins.
    fn twin_of(&self, t: &str, caller: &str) -> Twin {
        let mut kept: Twin = None;
        let mut own: Twin = None;
        let name = text(&self.mname, caller);
        for &i in self.twins.get(t).map_or(&[][..], Vec::as_slice) {
            let tw = &self.twin_store[i];
            if tw.kept {
                kept = Some(i);
            }
            if own.is_none() && contains(&tw.methods, name) {
                own = Some(i);
            }
        }
        if own.is_none() || kept.is_some_and(|k| contains(&self.twin_store[k].methods, name)) {
            return kept;
        }
        own
    }

    /// Go `bases`: the base ids of type `t`, as its twin `d` when it has twins; `None` (Go's
    /// `ok` false) when one is outside the corpus.
    fn bases(&self, t: &str, d: Twin) -> Option<Vec<String>> {
        let d = d.or_else(|| self.twin_of(t, ""));
        if self.upper.get(t).copied().unwrap_or(false) {
            return Some(Vec::new());
        }
        let supers = list(self.supers, t);
        let Some(d) = d else {
            return (!self.open_super.contains(t)).then(|| supers.to_vec());
        };
        let mut ids = Vec::new();
        for b in &self.twin_store[d].bases {
            let named = list(&self.by_name, b);
            if contains(named, t) {
                // A twin's base binds the kept twin.
                ids.push(t.to_string());
                continue;
            }
            let i = supers.iter().position(|s| contains(named, s))?;
            ids.push(supers[i].clone());
        }
        Some(ids)
    }

    /// Go `bindNew`: binds `new X()`, qualified by `prefix`, to the type X.
    fn bind_new(
        &self,
        m: &mut CsMemo,
        file: &str,
        caller: &str,
        name: &str,
        prefix: &str,
        enc: &str,
    ) -> (String, Confidence) {
        let own: &[String] = self
            .local
            .get(&[file.to_string(), name.to_string()])
            .map_or(&[], Vec::as_slice);
        let types = keep(own, |id| self.ns.contains_key(id));
        let lexical = self.own_method(enc, name, file);
        let shadow = !types.is_empty() && self.shadow.contains(&types[0]);
        // Methods or a stub defer upstream.
        if !types.is_empty()
            && text(self.owner, &own[own.len() - 1]).is_empty()
            && !lexical
            && !shadow
        {
            return (types[0].clone(), Confidence::EXTRACTED);
        }
        let cands = list(&self.by_name, name);
        let in_ns = |id: &str| text(&self.ns, id) == prefix;
        let last = prefix.rfind('.').map_or(prefix, |i| &prefix[i + 1..]);
        if !prefix.is_empty()
            && list(&self.by_name, last)
                .iter()
                .any(|id| self.nested.contains(&format!("{id}\0{name}")))
        {
            // Nested: no node; namespace pass binds.
            return (unique(cands, in_ns), Confidence::EXTRACTED);
        }
        // Upstream's import evidence: a file alias.
        let evidence = |id: &str| {
            self.aliased
                .contains(&format!("{file}\0{}.{name}", text(&self.ns, id)))
        };
        let via = prefix.is_empty()
            && !lexical
            && (types.is_empty() || shadow)
            && self.stubs.contains(&[file.to_string(), name.to_string()]);
        let mut tgt = String::new();
        if via {
            // Stub of X: scope or evidence.
            tgt = self.resolve(name, enc, false);
            if !contains(cands, &tgt) {
                tgt = String::new();
            }
        }
        let scoped = !tgt.is_empty();
        if !scoped {
            let k = [file.to_string(), name.to_string(), prefix.to_string()];
            let p = match m.picks.get(&k) {
                Some(p) => p.clone(),
                None => {
                    let mut p = CsPick::default();
                    let ev = if self.alias_in.contains(file) {
                        keep(cands, evidence)
                    } else {
                        Vec::new()
                    };
                    let all = m
                        .cands
                        .entry(name.to_string())
                        .or_insert_with(|| self.cands_of(name));
                    if all.all.n == 1 {
                        p.tgt = all.all.id.clone();
                    } else if ev.len() == 1 {
                        p.tgt = ev[0].clone();
                    } else if prefix.is_empty() {
                        p.tgt = all.tie_break(file, self.test.get(file).copied().unwrap_or(false));
                    } else {
                        p.tgt = unique(cands, in_ns);
                        p.done = !p.tgt.is_empty();
                    }
                    m.picks.insert(k, p.clone());
                    p
                }
            };
            tgt = p.tgt;
            if p.done {
                return (tgt, Confidence::EXTRACTED);
            }
        }
        if tgt.starts_with('\0') {
            return (String::new(), Confidence::INFERRED);
        }
        let mut conf = Confidence::INFERRED;
        if !tgt.is_empty() && evidence(&tgt) {
            conf = Confidence::EXTRACTED;
        }
        if via && !scoped && conf != Confidence::EXTRACTED
            || self.refs.contains(&format!("{caller}\0{tgt}"))
            || via && self.named.contains(&format!("{caller}\0{name}"))
        {
            // Unscoped stub, or references hold pair.
            return (String::new(), Confidence::INFERRED);
        }
        (tgt, conf)
    }

    /// Go `ownMethod`: whether the type `enc` or a base of it in `file` declares a method `name`,
    /// as upstream's lexical lookup finds first.
    fn own_method(&self, enc: &str, name: &str, file: &str) -> bool {
        let mut seen: HashSet<String> = HashSet::new();
        let mut q: Vec<String> = vec![enc.to_string()];
        let mut i = 0;
        while i < q.len() {
            let h = q[i].clone();
            i += 1;
            if seen.contains(&h) || text(self.id_file, &h) != file {
                continue;
            }
            seen.insert(h.clone());
            if !list(self.methods, &format!("{h}\0{name}")).is_empty() {
                return true;
            }
            q.extend(list(self.supers, &h).iter().cloned());
        }
        false
    }

    /// Go `fileOf`: the file of a candidate, nested ones "\0file".
    fn file_of(&self, id: &str) -> String {
        match id.strip_prefix('\0') {
            Some(f) => f.to_string(),
            None => text(self.id_file, id).to_string(),
        }
    }

    /// Go `candsOf`.
    fn cands_of(&self, name: &str) -> CsCands {
        let mut c = CsCands::default();
        let mut ids: Vec<String> = list(&self.by_name, name).to_vec();
        // Nested types count, with no node.
        for f in list(&self.nested_in, name) {
            ids.push(format!("\0{f}"));
        }
        for id in &ids {
            c.all.add(id);
            let f = self.file_of(id);
            let g = if self.test.get(&f).copied().unwrap_or(false) {
                &mut c.tests
            } else {
                &mut c.rest
            };
            g.hit.add(id);
            g.file.entry(f.clone()).or_default().add(id);
            g.dir.entry(path::dir(&f)).or_default().add(id);
            let mut d = path::dir(&f);
            while d != "." && d != "/" {
                g.under.entry(d.clone()).or_default().add(id);
                d = path::dir(&d);
            }
        }
        c
    }
}

/// Go `csTestDirs`.
fn cs_test_dir(s: &str) -> bool {
    matches!(s, "tests" | "test" | "spec" | "specs" | "__tests__")
}

/// Go `csTestName`.
static CS_TEST_NAME: LazyLock<Regexp> = LazyLock::new(|| {
    Regexp::must_compile(r"(?i)^test_|_test\..|\.test\..|\.spec\..|_spec\..|\.tests\.ps1$")
});

/// Go `csTestCase`.
static CS_TEST_CASE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"Tests?\.java$|Tests\.cs$"));

/// Go `csTestPath`: upstream's `_is_test_path`.
fn cs_test_path(p: &str) -> bool {
    let segs: Vec<&str> = p.split('/').collect();
    if segs.iter().any(|s| cs_test_dir(&to_lower(s))) {
        return true;
    }
    let name = segs[segs.len() - 1];
    CS_TEST_NAME.match_string(name) || CS_TEST_CASE.match_string(name)
}

/// Go `csKey` (`csharp_resolve.go:279`): upstream's bare-name key, no non-word runes.
pub(crate) fn cs_key(s: &str) -> String {
    if s.bytes()
        .all(|c| c == b'_' || c.is_ascii_digit() || c.is_ascii_alphabetic())
    {
        return s.to_string();
    }
    gostd::strings::map(s, |r| {
        (r == '_' || is_letter(r) || is_number(r)).then_some(r)
    })
}

/// Go `csCands`: the candidates of one name, nested ones "\0file", split by upstream's test rule
/// and indexed by file, dir and ancestor dir. Go embeds a `csGroup` whose maps stay nil; only
/// its count is read, here `all`.
#[derive(Default)]
struct CsCands {
    all: CsHit,
    tests: CsGroup,
    rest: CsGroup,
}

/// Go `csGroup`: counts candidates, and keeps one, overall and per key.
#[derive(Default)]
struct CsGroup {
    hit: CsHit,
    file: HashMap<String, CsHit>,
    dir: HashMap<String, CsHit>,
    under: HashMap<String, CsHit>,
}

/// Go `csHit`.
#[derive(Clone, Default)]
struct CsHit {
    n: usize,
    id: String,
}

impl CsHit {
    fn add(&mut self, id: &str) {
        self.n += 1;
        if self.n == 1 {
            self.id = id.to_string();
        }
    }

    /// Go `only`: the one candidate counted, else "".
    fn only(&self) -> String {
        if self.n == 1 {
            return self.id.clone();
        }
        String::new()
    }
}

/// `m[k]` of a Go `map[string]csHit`: the zero hit when absent.
fn hit<'m>(m: &'m HashMap<String, CsHit>, k: &str) -> Option<&'m CsHit> {
    m.get(k).filter(|h| h.n > 0)
}

impl CsCands {
    /// Go `tieBreak`: upstream's pick among several candidates for a caller in `file`: test ones
    /// by the caller's kind, then the same file, the same dir, the longest shared dir.
    fn tie_break(&self, file: &str, test: bool) -> String {
        let mut g = &self.rest;
        if test && self.tests.hit.n > 0 {
            let id = self
                .tests
                .file
                .get(file)
                .map(CsHit::only)
                .unwrap_or_default();
            if !id.is_empty() {
                return id;
            }
            g = &self.tests;
        }
        if g.hit.n == 1 {
            return g.hit.id.clone();
        }
        if let Some(h) = hit(&g.file, file) {
            return h.only();
        }
        let dir = path::dir(file);
        if let Some(h) = hit(&g.dir, &dir) {
            return h.only();
        }
        let mut d = dir;
        while d != "." && d != "/" {
            if let Some(h) = hit(&g.under, &d) {
                return h.only();
            }
            d = path::dir(&d);
        }
        String::new()
    }
}
