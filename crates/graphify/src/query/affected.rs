//! Port of `affected.go`: the blast radius of a change, walking dependency edges backwards from
//! the nodes of the changed files.

use super::query::{Graph, Node};

// graphify-rs-ihc.13: stubs until the code PR

/// Go `depRelations`: the relations that mean "source depends on target".
#[allow(dead_code, unused_variables)]
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
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffectedResult {
    pub changed: Vec<Node>,
    pub impacted: Vec<Node>,
}

/// Go `AffectedOptions`: `depth <= 0` is unbounded; empty `relations` is [`DEP_RELATIONS`].
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffectedOptions {
    pub depth: i64,
    pub relations: Vec<String>,
}

/// Go `Affected`: the nodes defined in `changed_files` and every node that transitively depends
/// on them, each list sorted by (source file, label) with Go's `sort.Slice`
/// (`gostd::sort::slice`). A seed is normalised as Go's `normalizeSeed` does (`./` stripped, an
/// absolute path under the working directory made relative).
#[allow(dead_code, unused_variables)]
pub fn affected(g: &Graph, changed_files: &[String], opts: &AffectedOptions) -> AffectedResult {
    todo!("graphify-rs-ihc.13")
}
