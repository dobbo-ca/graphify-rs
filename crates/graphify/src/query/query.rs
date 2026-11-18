//! Port of `query.go`: load a graph.json, find nodes by name, explain a node and its neighbours,
//! and find the shortest dependency path between two nodes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

// graphify-rs-ihc.13: stubs until the code PR

/// Go `ErrNoDirectedPath`: a directed search found no route ([`Error::NoDirectedPath`] starts
/// with it); callers add their own "retry undirected" hint.
pub const ERR_NO_DIRECTED_PATH: &str = "no directed path";

/// The errors of this module. `Display` is Go's text byte for byte (plan R6).
#[allow(dead_code, unused_variables)]
#[derive(Debug)]
pub enum Error {
    /// Go `*SameNodeError`.
    SameNode(SameNodeError),
    /// Go `*MaxHopsError`.
    MaxHops(MaxHopsError),
    /// Go `*AmbiguousError`.
    Ambiguous(AmbiguousError),
    /// Go `fmt.Errorf("%w between %q and %q", ErrNoDirectedPath, ...)`: the whole text
    /// (`errors.Is(err, ErrNoDirectedPath)` is a match on this variant).
    NoDirectedPath(String),
    /// Any other Go error text: `parsing <path>: <json err>`, `open <path>: <errno>`, the
    /// security size cap, `no node matching "x"`, a regexp compile error, the merge caps.
    Other(String),
}

#[allow(dead_code, unused_variables)]
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-ihc.13")
    }
}

impl std::error::Error for Error {}

/// Go `Graph`: a loaded graph.json (`query.go:20-33`), with the lookup tables `load` builds.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// Go `Attrs` (json `graph`).
    pub attrs: GraphAttrs,
    pub built_at_commit: String,
    /// Go `Path`: the file `load` read, for git lookups.
    pub path: PathBuf,
    /// Go `byID`: node id to its index in `nodes` (the last node with an id wins, as in Go).
    pub(super) by_id: BTreeMap<String, usize>,
    /// Go `adj`: undirected adjacency of every link's endpoints.
    pub(super) adj: BTreeMap<String, BTreeSet<String>>,
    /// Go `edge`: directed (source, target) to the index in `links` of its first link.
    pub(super) edge: BTreeMap<(String, String), usize>,
}

/// Go `GraphAttrs`: graph.json's graph-level counters.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphAttrs {
    pub unclassified_files: i64,
    pub unclassified_exts: BTreeMap<String, i64>,
}

/// Go `Node`: a graph.json node.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub file_type: String,
    pub source_file: String,
    pub source_location: String,
    /// Go `*int`: `None` for a missing or `null` community.
    pub community: Option<i64>,
    pub norm_label: String,
    pub computed_name: String,
    pub attributes: BTreeMap<String, String>,
}

/// Go `Link`: a graph.json edge.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    pub source_file: String,
    pub source_location: String,
}

impl Graph {
    /// Go `Graph.UnclassifiedSummary`: the files no extractor handled, with the three biggest
    /// extensions; `""` when there were none.
    #[allow(dead_code, unused_variables)]
    pub fn unclassified_summary(&self) -> String {
        todo!("graphify-rs-ihc.13")
    }

    /// Go `Graph.resolve`: by exact id, then `path::Symbol`, then source file path, then exact
    /// (case-insensitive) label, then label or id substring. More than one match at a tier is
    /// [`Error::Ambiguous`]; no match is `Ok(None)`.
    #[allow(dead_code, unused_variables)]
    pub(super) fn resolve(&self, s: &str) -> Result<Option<&Node>, Error> {
        todo!("graphify-rs-ihc.13")
    }
}

/// Go `Load`: reads and indexes the graph.json at `path` (size cap, BOM strip, `parsing <path>:
/// <err>`).
#[allow(dead_code, unused_variables)]
pub fn load(path: &Path) -> Result<Graph, Error> {
    todo!("graphify-rs-ihc.13")
}

/// Go `Match`: a node found by [`query`].
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub id: String,
    pub label: String,
    pub location: String,
}

/// Go `Query`: the nodes whose label, id or norm_label match `(?i)` + `pattern`, sorted by label
/// (`sort.Slice`: `gostd::sort::slice`).
#[allow(dead_code, unused_variables)]
pub fn query(g: &Graph, pattern: &str) -> Result<Vec<Match>, Error> {
    todo!("graphify-rs-ihc.13")
}

/// Go `Neighbor`: an adjacent node in one direction; `degree` is the neighbour's own undirected
/// degree and `file` its source file.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Neighbor {
    pub id: String,
    pub label: String,
    pub relation: String,
    pub direction: String,
    pub location: String,
    pub file: String,
    pub degree: i64,
}

/// Go `Explanation`: a node plus its neighbours.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, PartialEq)]
pub struct Explanation {
    pub node: Node,
    pub neighbors: Vec<Neighbor>,
}

/// Go `Explain`: the node `id` resolves to and its neighbours, most connected first.
#[allow(dead_code, unused_variables)]
pub fn explain(g: &Graph, id: &str) -> Result<Explanation, Error> {
    todo!("graphify-rs-ihc.13")
}

/// Go `PathEdge`: the edge traversed to reach a path step.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathEdge {
    pub relation: String,
    pub confidence: String,
    pub forward: bool,
}

/// Go `PathResult`: `edges[i]` joins `nodes[i]` to `nodes[i + 1]`.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<PathEdge>,
}

/// Go `SameNodeError`.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, PartialEq)]
pub struct SameNodeError {
    pub from: String,
    pub to: String,
    pub id: String,
}

#[allow(dead_code, unused_variables)]
impl fmt::Display for SameNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-ihc.13")
    }
}

/// Go `MaxHopsError`.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, PartialEq)]
pub struct MaxHopsError {
    pub max_hops: i64,
    pub hops: i64,
}

#[allow(dead_code, unused_variables)]
impl fmt::Display for MaxHopsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-ihc.13")
    }
}

/// Go `AmbiguousError`: `candidates` is `"label (file:line)"` per match, capped at 10, `more`
/// the matches left out. Query and candidates are already sanitized.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, PartialEq)]
pub struct AmbiguousError {
    pub query: String,
    pub candidates: Vec<String>,
    pub more: i64,
}

#[allow(dead_code, unused_variables)]
impl fmt::Display for AmbiguousError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-ihc.13")
    }
}

/// Go `PathEdges`: the shortest path from `from` to `to`, following edge direction unless
/// `undirected` (a `contains` edge is walkable upward); `max_hops > 0` caps it.
#[allow(dead_code, unused_variables)]
pub fn path_edges(
    g: &Graph,
    from: &str,
    to: &str,
    max_hops: i64,
    undirected: bool,
) -> Result<PathResult, Error> {
    todo!("graphify-rs-ihc.13")
}
