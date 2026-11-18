//! Port of `query.go`: load a graph.json, find nodes by name, explain a node and its neighbours,
//! and find the shortest dependency path between two nodes.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use gostd::json::{self, DecodeState, Error as JsonError, Unmarshal};
use gostd::regexp::Regexp;
use gostd::strconv::quote;
use gostd::strings::{equal_fold, to_lower};

use super::affected::normalize_seed;
use crate::security::{check_graph_file_size, sanitize_label};

/// Go `ErrNoDirectedPath`: a directed search found no route ([`Error::NoDirectedPath`] starts
/// with it); callers add their own "retry undirected" hint.
pub const ERR_NO_DIRECTED_PATH: &str = "no directed path";

/// The errors of this module. `Display` is Go's text byte for byte (plan R6).
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SameNode(e) => e.fmt(f),
            Error::MaxHops(e) => e.fmt(f),
            Error::Ambiguous(e) => e.fmt(f),
            Error::NoDirectedPath(s) | Error::Other(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

/// Go `Graph`: a loaded graph.json (`query.go:20-33`), with the lookup tables `load` builds.
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

/// Go's struct decoding of `Graph` (json `nodes`, `links`, `graph`, `built_at_commit`; `Path`
/// is `json:"-"` and the lookup tables are unexported).
impl Unmarshal for Graph {
    const GO_TYPE: &'static str = "query.Graph";

    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), JsonError> {
        d.read_struct(
            &["nodes", "links", "graph", "built_at_commit", "-"],
            |d, key| match key {
                "nodes" => d.value(&mut self.nodes),
                "links" => d.value(&mut self.links),
                "graph" => d.value(&mut self.attrs),
                _ => d.value(&mut self.built_at_commit),
            },
        )
    }
}

/// Go `GraphAttrs`: graph.json's graph-level counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphAttrs {
    pub unclassified_files: i64,
    pub unclassified_exts: BTreeMap<String, i64>,
}

gostd::json_fields!(decode GraphAttrs, "query.GraphAttrs", {
    unclassified_files => "unclassified_files",
    unclassified_exts => "unclassified_extensions",
});

/// Go `Node`: a graph.json node.
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

gostd::json_fields!(decode Node, "query.Node", {
    id => "id",
    label => "label",
    file_type => "file_type",
    source_file => "source_file",
    source_location => "source_location",
    community => "community",
    norm_label => "norm_label",
    computed_name => "computed_name",
    attributes => "attributes",
});

/// Go `Link`: a graph.json edge.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    pub source_file: String,
    pub source_location: String,
}

gostd::json_fields!(decode Link, "query.Link", {
    source => "source",
    target => "target",
    relation => "relation",
    confidence => "confidence",
    source_file => "source_file",
    source_location => "source_location",
});

impl Graph {
    /// Go `Graph.UnclassifiedSummary`: the files no extractor handled, with the three biggest
    /// extensions; `""` when there were none.
    pub fn unclassified_summary(&self) -> String {
        if self.attrs.unclassified_files == 0 {
            return String::new();
        }
        let counts = &self.attrs.unclassified_exts;
        let mut exts: Vec<&String> = counts.keys().collect();
        gostd::sort::slice(&mut exts, |a, b| {
            let (x, y) = (counts[*a], counts[*b]);
            if x != y {
                return x > y;
            }
            a < b
        });
        exts.truncate(3);
        let line = format!(
            "Unclassified: {} file(s) no extractor handles",
            self.attrs.unclassified_files
        );
        if exts.is_empty() {
            return line;
        }
        let parts: Vec<String> = exts
            .iter()
            .map(|e| format!("{} {}", sanitize_label(e), counts[*e]))
            .collect();
        format!("{line} ({})", parts.join(", "))
    }

    /// Go `g.byID[id]`.
    pub(super) fn node(&self, id: &str) -> Option<&Node> {
        self.by_id.get(id).map(|&i| &self.nodes[i])
    }

    /// Go `g.edge[[2]string{a, b}]`.
    fn edge_of(&self, a: &str, b: &str) -> Option<&Link> {
        self.edge
            .get(&(a.to_string(), b.to_string()))
            .map(|&i| &self.links[i])
    }

    /// Go `Graph.resolve`: by exact id, then `path::Symbol`, then source file path, then exact
    /// (case-insensitive) label, then label or id substring. More than one match at a tier is
    /// [`Error::Ambiguous`]; no match is `Ok(None)`.
    pub(super) fn resolve(&self, s: &str) -> Result<Option<&Node>, Error> {
        if let Some(n) = self.node(s) {
            return Ok(Some(n));
        }
        if let Some(i) = s.rfind("::")
            && i > 0
            && i + 2 < s.len()
        {
            let (path, sym) = (to_lower(&s[..i]), to_lower(&s[i + 2..]));
            let suffix = format!("/{path}");
            let hits: Vec<&Node> = self
                .nodes
                .iter()
                .filter(|n| {
                    let sf = to_lower(&n.source_file);
                    to_lower(&n.label) == sym && (sf == path || sf.ends_with(&suffix))
                })
                .collect();
            if !hits.is_empty() {
                if hits.len() > 1 {
                    return Err(Error::Ambiguous(ambiguous(s, &hits, hits.len())));
                }
                return Ok(Some(hits[0]));
            }
        }
        let low = to_lower(s);
        if low.is_empty() {
            return Ok(None);
        }
        let p = to_lower(&normalize_seed(s));
        if !p.is_empty() {
            let suffix = format!("/{p}");
            let (mut exact_files, mut suffix_files) = (Vec::new(), Vec::new());
            for n in &self.nodes {
                let sf = to_lower(&n.source_file);
                if sf == p {
                    exact_files.push(n);
                } else if p.contains('/') && sf.ends_with(&suffix) {
                    suffix_files.push(n);
                }
            }
            let mut files = file_nodes(&exact_files);
            if files.is_empty() {
                files = file_nodes(&suffix_files);
            }
            if !files.is_empty() {
                if files.len() > 1 {
                    return Err(Error::Ambiguous(ambiguous(s, &files, files.len())));
                }
                return Ok(Some(files[0]));
            }
        }
        let exact: Vec<&Node> = self
            .nodes
            .iter()
            .filter(|n| to_lower(&n.label) == low)
            .collect();
        if !exact.is_empty() {
            if exact.len() > 1 {
                return Err(Error::Ambiguous(ambiguous(s, &exact, exact.len())));
            }
            return Ok(Some(exact[0]));
        }
        let mut subs = Vec::new();
        let mut total = 0;
        for n in &self.nodes {
            if to_lower(&n.label).contains(&low) || to_lower(&n.id).contains(&low) {
                total += 1;
                if subs.len() < MAX_CANDIDATES {
                    subs.push(n);
                }
            }
        }
        match total {
            0 => Ok(None),
            1 => Ok(Some(subs[0])),
            _ => Err(Error::Ambiguous(ambiguous(s, &subs, total))),
        }
    }

    /// Go `Graph.bfsPath`: the node ids of a shortest path from `a_id` to `b_id` (both
    /// included), following edge direction unless `undirected`.
    fn bfs_path(&self, a_id: &str, b_id: &str, undirected: bool) -> Option<Vec<String>> {
        let mut prev: HashMap<&str, &str> = HashMap::from([(a_id, "")]);
        let mut queue = VecDeque::from([a_id]);
        while let Some(cur) = queue.pop_front() {
            if cur == b_id {
                break;
            }
            // BTreeSet: already in sort.Strings order.
            for nb in self.adj.get(cur).into_iter().flatten() {
                if !undirected && self.edge_of(cur, nb).is_none() {
                    // contains is walkable upward so symbols reach their file
                    match self.edge_of(nb, cur) {
                        Some(l) if l.relation == "contains" => {}
                        _ => continue,
                    }
                }
                if !prev.contains_key(nb.as_str()) {
                    prev.insert(nb, cur);
                    queue.push_back(nb);
                }
            }
        }
        if !prev.contains_key(b_id) {
            return None;
        }
        let mut ids = Vec::new();
        let mut cur = b_id;
        while !cur.is_empty() {
            ids.push(cur.to_string());
            cur = prev.get(cur).copied().unwrap_or("");
        }
        ids.reverse();
        Some(ids)
    }
}

/// Go `Load`: reads and indexes the graph.json at `path` (size cap, BOM strip, `parsing <path>:
/// <err>`).
pub fn load(path: &Path) -> Result<Graph, Error> {
    check_graph_file_size(path).map_err(|e| Error::Other(e.to_string()))?;
    let data = read_file(path)?;
    // Strip a UTF-8 BOM: some editors/Windows tooling prepend one and encoding/json rejects it
    // outright.
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&data);
    let mut g: Graph = json::unmarshal(data).map_err(|e| parsing(path, &e))?;
    g.path = path.to_path_buf();
    for (i, n) in g.nodes.iter().enumerate() {
        g.by_id.insert(n.id.clone(), i);
    }
    for (i, l) in g.links.iter().enumerate() {
        let (s, t) = (l.source.clone(), l.target.clone());
        g.adj.entry(s.clone()).or_default().insert(t.clone());
        g.adj.entry(t.clone()).or_default().insert(s.clone());
        g.edge.entry((s, t)).or_insert(i);
    }
    Ok(g)
}

/// Go `os.ReadFile`, its error as Go's `*PathError` text.
pub(super) fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path)
        .map_err(|e| Error::Other(crate::goerr::path_error("open", path, &e).to_string()))
}

/// Go `fmt.Errorf("parsing %s: %w", path, err)`.
pub(super) fn parsing(path: &Path, e: &JsonError) -> Error {
    Error::Other(format!("parsing {}: {e}", path.display()))
}

/// Go `Match`: a node found by [`query`].
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub id: String,
    pub label: String,
    pub location: String,
}

/// Go `Query`: the nodes whose label, id or norm_label match `(?i)` + `pattern`, sorted by label
/// (`sort.Slice`: `gostd::sort::slice`).
pub fn query(g: &Graph, pattern: &str) -> Result<Vec<Match>, Error> {
    let re = Regexp::compile(&format!("(?i){pattern}")).map_err(|e| Error::Other(e.to_string()))?;
    let mut out: Vec<Match> = g
        .nodes
        .iter()
        .filter(|n| {
            re.match_string(&n.label) || re.match_string(&n.id) || re.match_string(&n.norm_label)
        })
        .map(|n| Match {
            id: n.id.clone(),
            label: n.label.clone(),
            location: loc(n),
        })
        .collect();
    gostd::sort::slice(&mut out, |a, b| a.label < b.label);
    Ok(out)
}

/// Go `Neighbor`: an adjacent node in one direction; `degree` is the neighbour's own undirected
/// degree and `file` its source file.
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
#[derive(Clone, Debug, PartialEq)]
pub struct Explanation {
    pub node: Node,
    pub neighbors: Vec<Neighbor>,
}

/// Go `Explain`: the node `id` resolves to and its neighbours, most connected first.
pub fn explain(g: &Graph, id: &str) -> Result<Explanation, Error> {
    let Some(n) = g.resolve(id)? else {
        return Err(no_node(id));
    };
    let mut nbrs = Vec::new();
    for l in &g.links {
        let (other, dir) = if n.id == l.source {
            (&l.target, "->")
        } else if n.id == l.target {
            (&l.source, "<-")
        } else {
            continue;
        };
        let (mut label, mut location, mut file) = (other.clone(), String::new(), String::new());
        if let Some(o) = g.node(other) {
            (label, location, file) = (o.label.clone(), loc(o), o.source_file.clone());
        }
        // Prefer the traversed edge's own call site over the neighbour's definition line: "who
        // calls this and where" wants the call site.
        let el = edge_loc(l);
        if !el.is_empty() {
            location = el;
        }
        nbrs.push(Neighbor {
            id: other.clone(),
            label,
            relation: l.relation.clone(),
            direction: dir.to_string(),
            location,
            file,
            degree: g.adj.get(other).map_or(0, |s| s.len() as i64),
        });
    }
    // Most-connected neighbours first; (relation, label, direction) breaks ties.
    gostd::sort::slice(&mut nbrs, |a, b| {
        if a.degree != b.degree {
            a.degree > b.degree
        } else if a.relation != b.relation {
            a.relation < b.relation
        } else if a.label != b.label {
            a.label < b.label
        } else {
            a.direction < b.direction
        }
    });
    Ok(Explanation {
        node: n.clone(),
        neighbors: nbrs,
    })
}

/// Go `fmt.Errorf("no node matching %q", s)`.
fn no_node(s: &str) -> Error {
    Error::Other(format!("no node matching {}", quote(s.as_bytes())))
}

/// Go `PathEdge`: the edge traversed to reach a path step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathEdge {
    pub relation: String,
    pub confidence: String,
    pub forward: bool,
}

/// Go `PathResult`: `edges[i]` joins `nodes[i]` to `nodes[i + 1]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<PathEdge>,
}

/// Go `SameNodeError`.
#[derive(Clone, Debug, PartialEq)]
pub struct SameNodeError {
    pub from: String,
    pub to: String,
    pub id: String,
}

impl fmt::Display for SameNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} and {} both resolved to the same node {}; use a more specific label or the exact node ID",
            quote(self.from.as_bytes()),
            quote(self.to.as_bytes()),
            quote(self.id.as_bytes())
        )
    }
}

/// Go `MaxHopsError`.
#[derive(Clone, Debug, PartialEq)]
pub struct MaxHopsError {
    pub max_hops: i64,
    pub hops: i64,
}

impl fmt::Display for MaxHopsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "path exceeds max_hops={} ({} hops found)",
            self.max_hops, self.hops
        )
    }
}

/// Go `AmbiguousError`: `candidates` is `"label (file:line)"` per match, capped at 10, `more`
/// the matches left out. Query and candidates are already sanitized.
#[derive(Clone, Debug, PartialEq)]
pub struct AmbiguousError {
    pub query: String,
    pub candidates: Vec<String>,
    pub more: i64,
}

impl fmt::Display for AmbiguousError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is ambiguous, matches: {}",
            quote(self.query.as_bytes()),
            self.candidates.join(", ")
        )?;
        if self.more > 0 {
            write!(f, " (and {} more)", self.more)?;
        }
        f.write_str("; disambiguate with path/to/file::Symbol or the exact node ID")
    }
}

/// Go `maxCandidates`.
const MAX_CANDIDATES: usize = 10;

/// Go `ambiguous`: `total` is the number of matches before capping.
fn ambiguous(s: &str, hits: &[&Node], total: usize) -> AmbiguousError {
    let mut e = AmbiguousError {
        query: sanitize_label(s),
        candidates: Vec::new(),
        more: 0,
    };
    for n in hits {
        if e.candidates.len() == MAX_CANDIDATES {
            break;
        }
        let mut c = n.label.clone();
        let l = loc(n);
        if !l.is_empty() {
            c = format!("{c} ({l})");
        }
        e.candidates.push(sanitize_label(&c));
    }
    if total > MAX_CANDIDATES {
        e.more = (total - MAX_CANDIDATES) as i64;
    }
    e
}

/// Go `fileNodes`: the file-level node of nodes sharing a source file (labelled with the
/// basename, non-heading preferred), else the lone non-heading node at L1.
fn file_nodes<'g>(in_: &[&'g Node]) -> Vec<&'g Node> {
    let mut lab: Vec<&Node> = Vec::new();
    let mut top: Vec<&Node> = Vec::new();
    for &n in in_ {
        if equal_fold(&n.label, &gostd::path::base(&n.source_file)) {
            lab.push(n);
        }
        if n.file_type != "heading" && n.source_location == "L1" {
            top.push(n);
        }
    }
    if lab.len() > 1 {
        let non_head: Vec<&Node> = lab
            .iter()
            .copied()
            .filter(|n| n.file_type != "heading")
            .collect();
        if !non_head.is_empty() {
            lab = non_head;
        }
    }
    if lab.is_empty() && top.len() == 1 {
        return top;
    }
    lab
}

/// Go `PathEdges`: the shortest path from `from` to `to`, following edge direction unless
/// `undirected` (a `contains` edge is walkable upward); `max_hops > 0` caps it.
pub fn path_edges(
    g: &Graph,
    from: &str,
    to: &str,
    max_hops: i64,
    undirected: bool,
) -> Result<PathResult, Error> {
    let Some(a) = g.resolve(from)? else {
        return Err(no_node(from));
    };
    let Some(b) = g.resolve(to)? else {
        return Err(no_node(to));
    };
    if a.id == b.id {
        return Err(Error::SameNode(SameNodeError {
            from: from.to_string(),
            to: to.to_string(),
            id: a.id.clone(),
        }));
    }
    let Some(ids) = g.bfs_path(&a.id, &b.id, undirected) else {
        return Err(no_path_err(a, b, undirected));
    };
    let hops = ids.len() as i64 - 1;
    if max_hops > 0 && hops > max_hops {
        return Err(Error::MaxHops(MaxHopsError { max_hops, hops }));
    }
    let nodes = ids
        .iter()
        .map(|id| {
            g.node(id).cloned().unwrap_or_else(|| Node {
                id: id.clone(),
                ..Node::default()
            })
        })
        .collect();
    let edges = ids
        .windows(2)
        .map(|w| {
            let (u, v) = (&w[0], &w[1]);
            if let Some(l) = g.edge_of(u, v) {
                path_edge(l, true)
            } else if let Some(l) = g.edge_of(v, u) {
                path_edge(l, false)
            } else {
                PathEdge::default()
            }
        })
        .collect();
    Ok(PathResult { nodes, edges })
}

fn path_edge(l: &Link, forward: bool) -> PathEdge {
    PathEdge {
        relation: l.relation.clone(),
        confidence: l.confidence.clone(),
        forward,
    }
}

/// Go `noPathErr`.
fn no_path_err(a: &Node, b: &Node, undirected: bool) -> Error {
    let (qa, qb) = (quote(a.label.as_bytes()), quote(b.label.as_bytes()));
    if undirected {
        return Error::Other(format!("no path between {qa} and {qb}"));
    }
    Error::NoDirectedPath(format!("{ERR_NO_DIRECTED_PATH} between {qa} and {qb}"))
}

/// Go `edgeLoc`: the call site an edge was extracted from, if it has one.
fn edge_loc(l: &Link) -> String {
    if l.source_location.is_empty() || l.source_file.is_empty() {
        return String::new();
    }
    let line = l
        .source_location
        .strip_prefix('L')
        .unwrap_or(&l.source_location);
    format!("{}:{line}", l.source_file)
}

/// Go `loc`.
pub(super) fn loc(n: &Node) -> String {
    if n.source_file.is_empty() {
        return String::new();
    }
    if n.source_location.is_empty() {
        return n.source_file.clone();
    }
    let line = n
        .source_location
        .strip_prefix('L')
        .unwrap_or(&n.source_location);
    format!("{}:{line}", n.source_file)
}
