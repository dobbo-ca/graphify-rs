//! Port of `affected.go`: the blast radius of a change, walking dependency edges backwards from
//! the nodes of the changed files.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use gostd::filepath::{abs, is_abs};

use super::query::{Graph, Node};
use crate::detect::go_lossy;

/// Go `depRelations`: the relations that mean "source depends on target".
pub(super) const DEP_RELATIONS: &[&str] = &[
    "calls",
    "indirect_call",
    "references",
    "imports",
    "imports_from",
    "dynamic_import",
    "re_exports",
    "inherits",
    "extends",
    "implements",
    "uses",
    "mixes_in",
    "embeds",
    "defines",
    "requires",
    "depends_on",
    "inherits_context",
];

/// Go `AffectedResult`: the nodes of the changed files and the nodes that depend on them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffectedResult {
    pub changed: Vec<Node>,
    pub impacted: Vec<Node>,
}

/// Go `AffectedOptions`: `depth <= 0` is unbounded; empty `relations` is [`DEP_RELATIONS`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffectedOptions {
    pub depth: i64,
    pub relations: Vec<String>,
}

/// Go `Affected`: the nodes defined in `changed_files` and every node that transitively depends
/// on them, each list sorted by (source file, label) with Go's `sort.Slice`
/// (`gostd::sort::slice`). A seed is normalised as Go's `normalizeSeed` does (`./` stripped, an
/// absolute path under the working directory made relative).
pub fn affected(g: &Graph, changed_files: &[String], opts: &AffectedOptions) -> AffectedResult {
    let want: HashSet<String> = changed_files.iter().map(|f| normalize_seed(f)).collect();
    let rel_set: HashSet<&str> = if opts.relations.is_empty() {
        DEP_RELATIONS.iter().copied().collect()
    } else {
        opts.relations.iter().map(String::as_str).collect()
    };

    // Reverse dependency adjacency: for "A depends on B" (A->B), record B->A so a changed B
    // reaches its dependent A.
    let mut rev: HashMap<&str, Vec<&str>> = HashMap::new();
    for l in &g.links {
        if rel_set.contains(l.relation.as_str()) {
            rev.entry(l.target.as_str())
                .or_default()
                .push(l.source.as_str());
        }
    }

    let mut changed_set: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<(&str, i64)> = VecDeque::new();
    for n in &g.nodes {
        if !n.source_file.is_empty() && want.contains(&normalize_seed(&n.source_file)) {
            changed_set.insert(n.id.as_str());
            queue.push_back((n.id.as_str(), 0));
        }
    }

    // BFS backwards from the changed nodes (depth 0); depth <= 0 leaves the walk unbounded.
    let bounded = opts.depth > 0;
    let mut impacted_set: HashSet<&str> = HashSet::new();
    while let Some((id, depth)) = queue.pop_front() {
        if bounded && depth >= opts.depth {
            continue;
        }
        for &dep in rev.get(id).into_iter().flatten() {
            if changed_set.contains(dep) || impacted_set.contains(dep) {
                continue;
            }
            impacted_set.insert(dep);
            queue.push_back((dep, depth + 1));
        }
    }

    AffectedResult {
        changed: collect(g, &changed_set),
        impacted: collect(g, &impacted_set),
    }
}

/// Go `normalizeSeed`: a file seed in the repo-relative, slash-separated form of
/// `Node::source_file` (`./` stripped, an absolute path under the working directory made
/// relative); anything else unchanged.
pub(super) fn normalize_seed(s: &str) -> String {
    let s = if is_abs(s) {
        let Ok(cwd) = abs(Path::new(".")) else {
            return s.to_string();
        };
        match gostd::filepath::rel(&go_lossy(cwd.as_os_str().as_encoded_bytes()), s) {
            Ok(r) if r != ".." && !r.starts_with("../") => r,
            _ => return s.to_string(),
        }
    } else {
        s.to_string()
    };
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// Go `Graph.collect`: the nodes whose ids are in `set`, sorted by source file then label.
fn collect(g: &Graph, set: &HashSet<&str>) -> Vec<Node> {
    let mut out: Vec<Node> = g
        .nodes
        .iter()
        .filter(|n| set.contains(n.id.as_str()))
        .cloned()
        .collect();
    gostd::sort::slice(&mut out, |a, b| {
        if a.source_file != b.source_file {
            return a.source_file < b.source_file;
        }
        a.label < b.label
    });
    out
}
