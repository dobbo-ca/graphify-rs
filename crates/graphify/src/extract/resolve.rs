//! Port of `resolve.go`: stitches the per-file results into one extraction. Filled by
//! graphify-rs-ihc.18 (plan Task 17); the signatures are shared API.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::idutil::make_id;
use crate::model::{Edge, Extraction, Node};

use super::extract::{Imp, Result};

/// Go's `corpus map[string]bool` (only ever `true`): the slash-relative paths of the corpus.
pub(crate) type Corpus = HashSet<String>;

/// Go `extract.Resolve`: the call, import and reference edges that need the whole corpus.
/// `files` are the slash-relative paths of the corpus; `workers` is Go's GOMAXPROCS (plan R5).
pub fn resolve(results: &[Result], files: &[String], workers: usize) -> Extraction {
    todo!("graphify-rs-ihc.18")
}

/// Go `extract.ResolveParts`: [`resolve`] without the join, the parts in the order `resolve`
/// joins them (`resolve.go:922-1044`).
pub fn resolve_parts(
    results: &[Result],
    files: &[String],
    workers: usize,
) -> (Vec<Vec<Node>>, Vec<Vec<Edge>>) {
    todo!("graphify-rs-ihc.18")
}

/// Go `extract.Join`: the parts copied into one extraction.
pub fn join(nodes: Vec<Vec<Node>>, edges: Vec<Vec<Edge>>) -> Extraction {
    todo!("graphify-rs-ihc.18")
}

/// Go `keep` (`resolve.go:1309`): the ids `pred` holds for, in order.
pub(crate) fn keep(ids: &[String], pred: impl Fn(&str) -> bool) -> Vec<String> {
    ids.iter().filter(|id| pred(id)).cloned().collect()
}

/// Go `contains` (`resolve.go:1643`): whether `ids` holds `id`.
pub(crate) fn contains(ids: &[String], id: &str) -> bool {
    ids.iter().any(|x| x == id)
}

/// Go `typeDefs` (`resolve.go:1570`): the ids that are `make_id(file of id, name)`, i.e. a type
/// `name` defined at the top of its file, in order. An id with no file is `make_id("", name)`.
pub(crate) fn type_defs(
    ids: &[String],
    name: &str,
    id_file: &HashMap<String, String>,
) -> Vec<String> {
    keep(ids, |id| {
        let file = id_file.get(id).map_or("", String::as_str);
        make_id(&[file, name]) == id
    })
}

/// Go `importTargets` (`resolve.go:1686`): the corpus files `im` imports, in order, without the
/// importing file and without repeats. Go passes nil maps where a caller has no Go data; pass
/// empty ones (`&Default::default()`). `go_targets` is Go's `goTargets`, a cache the call fills
/// (`resolve.go:1707`), so it is `&mut`.
pub(crate) fn import_targets(
    im: &Imp,
    corpus: &Corpus,
    go_pkgs: &HashMap<String, Vec<String>>,
    go_mods: &BTreeMap<String, String>,
    go_targets: &mut HashMap<[String; 2], Vec<String>>,
) -> Vec<String> {
    todo!("graphify-rs-ihc.18")
}

/// Go `impPart` (`resolve.go:1048`): how many import edges [`resolve_parts`] puts in one part.
pub(crate) const IMP_PART: usize = 4096;

/// Go `recvTail` (`resolve.go:1198`): the last identifier of a receiver (`a.b` is `b`); a
/// receiver ending in anything else (`f()`, `a[0]`) has none. Go scans bytes and reads an invalid
/// UTF-8 byte as U+FFFD, which is no letter or number; a `&str` has none to read.
pub(crate) fn recv_tail(s: &str) -> &str {
    todo!("graphify-rs-ihc.18")
}

/// Go `selfTarget` (`resolve.go:1269`): binds a self/this call made from a method of type `cls`:
/// the method of that name on `cls`, else on the nearest ancestor reached over resolved inherits
/// edges. A super call starts at the parents. It returns "" on a tie or when the chain passes a
/// base outside the corpus. `methods` is keyed `class + "\0" + name`.
pub(crate) fn self_target(
    cls: &str,
    name: &str,
    super_call: bool,
    methods: &HashMap<String, Vec<String>>,
    supers: &HashMap<String, Vec<String>>,
    open_super: &HashSet<String>,
) -> String {
    todo!("graphify-rs-ihc.18")
}

/// Go `goModDir` (`resolve.go:1667`): the directory of the nearest go.mod above file `f`, or "".
pub(crate) fn go_mod_dir(go_mods: &BTreeMap<String, String>, f: &str) -> String {
    todo!("graphify-rs-ihc.18")
}

/// Go `resolveModulePath` (`resolve.go:1877`): the corpus file a JS/TS module path names (the
/// path itself, then the extensions, then a directory index), or "" when nothing matches.
pub(crate) fn resolve_module_path(base: &str, corpus: &Corpus) -> String {
    todo!("graphify-rs-ihc.18")
}
