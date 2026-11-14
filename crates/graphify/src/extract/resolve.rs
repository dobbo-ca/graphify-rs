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
/// empty ones (`&Default::default()`).
pub(crate) fn import_targets(
    im: &Imp,
    corpus: &Corpus,
    go_pkgs: &HashMap<String, Vec<String>>,
    go_mods: &BTreeMap<String, String>,
    go_targets: &HashMap<[String; 2], Vec<String>>,
) -> Vec<String> {
    todo!("graphify-rs-ihc.18")
}
