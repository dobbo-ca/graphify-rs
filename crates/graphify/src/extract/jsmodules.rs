//! Port of `jsmodules.go`: resolves what the import and export statements of JS/TS files name.
//!
//! Helpers only. The functions `resolve.rs` calls are shared API: their signatures are
//! stubbed by the extractors-and-CLI stream with the resolve tests PR (graphify-rs-ihc.18).
//!
//! Go's `edges(fc *fileMemo)` takes the `fileMemo` of `resolve.go:1417`; here it takes the
//! closure `of` that memo's `of` method is (a path to its `File`), so this file does not
//! depend on a type `resolve.rs` has not got yet.

use std::collections::{HashMap, HashSet};

use super::extract::{Imp, Result, TypeRef};
use super::resolve::{Corpus, import_targets, type_defs};
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, File, Relation};

/// Go `jsLike`: reports whether a file's imports are JS/TS ones (see `js_imp`).
pub(crate) fn js_like(file: &str) -> bool {
    crate::langfamily::of(file) == "jsts"
}

/// Go `jsRe`: one `export ... from` of a file, or an export of an imported name: it exports
/// `name` of `target` as `as_`. Both are empty for `export *`.
#[derive(Clone, Default)]
struct JsRe {
    target: String,
    name: String,
    /// Go `as`.
    as_: String,
}

/// Go `jsMods`: resolves what the import and export statements of JS/TS files name.
pub(crate) struct JsMods<'a> {
    results: &'a [Result],
    corpus: &'a Corpus,
    local: &'a HashMap<[String; 2], Vec<String>>,
    id_file: &'a HashMap<String, String>,
    /// File -> local name of its default export.
    defaults: HashMap<String, String>,
    /// File -> its re-exports.
    reex: HashMap<String, Vec<JsRe>>,
    /// File -> index in `results`, made by `is_fn`.
    by_file: Option<HashMap<String, usize>>,
    /// File -> its exports of imported names, no `names`.
    exp: HashMap<String, Vec<Imp>>,
    memo: HashMap<[String; 2], String>,
}

impl<'a> JsMods<'a> {
    /// Go `newJSMods`.
    pub(crate) fn new(
        results: &'a [Result],
        corpus: &'a Corpus,
        local: &'a HashMap<[String; 2], Vec<String>>,
        id_file: &'a HashMap<String, String>,
    ) -> JsMods<'a> {
        let mut m = JsMods {
            results,
            corpus,
            local,
            id_file,
            defaults: HashMap::new(),
            reex: HashMap::new(),
            by_file: None,
            exp: HashMap::new(),
            memo: HashMap::new(),
        };
        for r in results {
            if !js_like(&r.file) {
                continue;
            }
            if !r.default.is_empty() {
                m.defaults.insert(r.file.clone(), r.default.clone());
            }
            for im in &r.imps {
                if !im.export {
                    continue;
                }
                let t = m.targets(im);
                if t.is_empty() {
                    continue;
                }
                if im.names.is_empty() {
                    m.reex.entry(r.file.clone()).or_default().push(JsRe {
                        target: t[0].clone(),
                        ..JsRe::default()
                    });
                }
                for n in &im.names {
                    let (name, as_) = n.split_once(" as ").unwrap_or((n, ""));
                    let as_ = if as_.is_empty() { name } else { as_ };
                    m.reex.entry(r.file.clone()).or_default().push(JsRe {
                        target: t[0].clone(),
                        name: name.to_string(),
                        as_: as_.to_string(),
                    });
                }
            }
            m.exports(r);
        }
        m
    }

    /// Go's `importTargets(im, m.corpus, nil, nil, nil)`.
    fn targets(&self, im: &Imp) -> Vec<String> {
        import_targets(
            im,
            self.corpus,
            &Default::default(),
            &Default::default(),
            &mut Default::default(),
        )
    }

    /// Go `m.local[[2]string{file, name}]`: nil when there is none.
    fn local_of(&self, file: &str, name: &str) -> &'a [String] {
        self.local
            .get(&[file.to_string(), name.to_string()])
            .map_or(&[][..], Vec::as_slice)
    }

    /// Go `exports`: finds the names `export { x as y }` of `r` takes from its imports and files
    /// them as re-exports of the imported module.
    fn exports(&mut self, r: &Result) {
        for e in &r.exports {
            let (local, as_) = e.split_once(" as ").unwrap_or((e, ""));
            let as_ = if as_.is_empty() { local } else { as_ };
            for im in &r.imps {
                for n in &im.names {
                    let (name, l) = n.split_once(" as ").unwrap_or((n, ""));
                    let l = if l.is_empty() { name } else { l };
                    if im.export || l != local || name == "*" {
                        continue;
                    }
                    let t = self.targets(im);
                    if t.is_empty() {
                        continue;
                    }
                    self.reex.entry(r.file.clone()).or_default().push(JsRe {
                        target: t[0].clone(),
                        name: name.to_string(),
                        as_: as_.to_string(),
                    });
                    let mut ex = im.clone();
                    ex.export = true;
                    ex.names = Vec::new();
                    self.exp.entry(r.file.clone()).or_default().push(ex);
                }
            }
        }
    }

    /// Go `def`: the top-level definition of `name` that `file` exports: its own, else one a
    /// chain of re-exports leads to. "default" is the file's own default export.
    pub(crate) fn def(&mut self, file: &str, name: &str) -> String {
        let key = [file.to_string(), name.to_string()];
        if let Some(id) = self.memo.get(&key) {
            return id.clone();
        }
        let id = self.find(file, name, &mut HashSet::new());
        self.memo.insert(key, id.clone());
        id
    }

    /// Go `find`: `def` without the memo. `seen` stops a cycle of `export *` barrels.
    fn find(&self, file: &str, name: &str, seen: &mut HashSet<[String; 2]>) -> String {
        if !seen.insert([file.to_string(), name.to_string()]) {
            return String::new();
        }
        let own = if name == "default" {
            self.defaults.get(file).cloned().unwrap_or_default()
        } else {
            name.to_string()
        };
        let ids = type_defs(self.local_of(file, &own), &own, self.id_file);
        if !own.is_empty() && !ids.is_empty() {
            return ids[0].clone();
        }
        for re in self.reex.get(file).map_or(&[][..], Vec::as_slice) {
            let from = if re.as_.is_empty() && name != "default" {
                name
            } else if re.as_ != name {
                continue;
            } else {
                re.name.as_str()
            };
            let id = self.find(&re.target, from, seen);
            if !id.is_empty() {
                return id;
            }
        }
        String::new()
    }

    /// Go `each`: calls `f` for every name an import or re-export of `r` binds to a definition:
    /// the import (or re-export), the local name, the definition id and the target file. A
    /// re-export of a whole module has no name, so `f` gets an empty id and the target file.
    pub(crate) fn each(&mut self, r: &Result, f: &mut impl FnMut(&Imp, &str, &str, &str)) {
        for im in &r.imps {
            if !im.export && im.names.is_empty() {
                continue;
            }
            let t = self.targets(im);
            if t.is_empty() {
                continue;
            }
            if im.export {
                f(im, "", "", &t[0]);
            }
            for n in &im.names {
                let (name, local) = n.split_once(" as ").unwrap_or((n, ""));
                let local = if local.is_empty() { name } else { local };
                if name == "*" || (im.export && name == "default") {
                    continue;
                }
                let id = self.def(&t[0], name);
                if !id.is_empty() {
                    f(im, local, &id, &t[0]);
                }
            }
        }
    }

    /// Go `edges`: the imports and re_exports edges of the JS/TS files. `of` is the `fileMemo`
    /// of Go's `fc`: the `File` of a path.
    pub(crate) fn edges(&mut self, of: &mut impl FnMut(&str) -> File) -> Vec<Edge> {
        let mut out = Vec::new();
        let results = self.results;
        for r in results {
            if !js_like(&r.file) {
                continue;
            }
            self.each(r, &mut |im, _, id, file| {
                let mut e = Edge {
                    source: im.file_id.clone(),
                    target: id.to_string(),
                    relation: Relation::IMPORTS,
                    confidence: Confidence::EXTRACTED,
                    source_file: of(&im.file),
                    source_location: im.loc,
                    ..Edge::default()
                };
                if im.export && id.is_empty() {
                    e.target = make_id(&[file]);
                    e.relation = Relation::RE_EXPORTS;
                } else if im.export {
                    e.relation = Relation::RE_EXPORTS;
                }
                out.push(e);
            });
            for im in self.exp.get(&r.file).map_or(&[][..], Vec::as_slice) {
                out.push(Edge {
                    source: im.file_id.clone(),
                    target: make_id(&[&self.targets(im)[0]]),
                    relation: Relation::RE_EXPORTS,
                    confidence: Confidence::EXTRACTED,
                    source_file: of(&im.file),
                    source_location: im.loc,
                    ..Edge::default()
                });
            }
        }
        out
    }

    /// Go `bound`: maps the local names `r`'s named imports bind to their definitions.
    pub(crate) fn bound(&mut self, r: &Result) -> HashMap<String, String> {
        let mut out = HashMap::new();
        self.each(r, &mut |im, local, id, _| {
            if !im.export && !id.is_empty() {
                out.insert(local.to_string(), id.to_string());
            }
        });
        out
    }

    /// Go `aliases`: maps (file, local name) to the definition of every name a file imports: a
    /// barrel can export it under a name no definition has.
    pub(crate) fn aliases(&mut self) -> HashMap<[String; 2], String> {
        let mut out = HashMap::new();
        let results = self.results;
        for r in results {
            if js_like(&r.file) {
                self.each(r, &mut |im, local, id, _| {
                    if !im.export && !id.is_empty() {
                        out.insert([r.file.clone(), local.to_string()], id.to_string());
                    }
                });
            }
        }
        out
    }

    /// Go `supertype`: the id a heritage clause in `r` names. Upstream binds a name declared in
    /// the file or imported by name, nothing else, and a function is no base. A type inside a
    /// namespace is no Def, so the file's nodes are read.
    pub(crate) fn supertype(
        &mut self,
        t: &TypeRef,
        r: &Result,
        bound: &HashMap<String, String>,
    ) -> String {
        let ids = type_defs(self.local_of(&t.file, &t.name), &t.name, self.id_file);
        let id = if !ids.is_empty() {
            ids[0].clone()
        } else {
            let mut id = bound.get(&t.name).cloned().unwrap_or_default();
            if id.is_empty()
                && let Some(n) = r
                    .nodes
                    .iter()
                    .find(|n| n.label == t.name && n.id != t.from_id)
            {
                id = n.id.clone();
            }
            id
        };
        if id.is_empty() || self.is_fn(&id) {
            return String::new();
        }
        id
    }

    /// Go `isFn`: reports whether the node of `id` is labelled as a function.
    fn is_fn(&mut self, id: &str) -> bool {
        let results = self.results;
        let by_file = self.by_file.get_or_insert_with(|| {
            results
                .iter()
                .enumerate()
                .map(|(i, r)| (r.file.clone(), i))
                .collect()
        });
        let file = self.id_file.get(id).map_or("", String::as_str);
        let i = by_file.get(file).copied().unwrap_or(0);
        for n in &results[i].nodes {
            if n.id == id {
                return n.label.ends_with("()");
            }
        }
        false
    }
}
