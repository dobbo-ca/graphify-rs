//! Port of `ask.go`: answer a question by TF-IDF seeding and a bounded BFS or DFS, rendered as a
//! token-budgeted text block. The IDF uses `gostd::math::log` (plan R10, Go's amd64 `archLog`).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

use gostd::regexp::Regexp;
use gostd::strings::{fields, to_lower};

use super::query::{Graph, Link, Node};
use crate::security::sanitize_label;

/// Go `map[string]map[string]bool` adjacency: node id to its neighbour ids.
pub(super) type Adj = BTreeMap<String, BTreeSet<String>>;

/// What `bfs_traverse` and `dfs_traverse` return: the visited node set and the traversed
/// (then induced) edges, in Go's order.
pub(super) type Traversal = (BTreeSet<String>, Vec<(String, String)>);

/// Go `Ask`: the answer to `question` (`depth` hops, `token_budget` tokens, edges limited to
/// `relations` when it is not empty).
pub fn ask(
    g: &Graph,
    question: &str,
    dfs: bool,
    depth: i64,
    token_budget: i64,
    relations: &[String],
) -> String {
    let terms = query_terms(question);
    let scored = score_nodes(g, &terms);
    let seeds = pick_seeds(g, &scored, 3, 0.2);
    if seeds.is_empty() {
        return "No matching nodes found.".to_string();
    }
    let want = relation_set(relations);
    let (adj, deg) = relation_adj(g, want.as_ref());
    let (mode, (nodes, edges)) = if dfs {
        ("DFS", dfs_traverse(g, &adj, &deg, &seeds, depth))
    } else {
        ("BFS", bfs_traverse(g, &adj, &deg, &seeds, depth))
    };
    let seed_labels: Vec<&str> = seeds.iter().map(|id| node_label(g, id)).collect();
    let header = format!(
        "Traversal: {mode} depth={depth} | Start: [{}] | {} nodes found\n\n",
        seed_labels.join(", "),
        nodes.len()
    );
    // only the subgraph's links, to keep big graphs cheap
    let mut pairs: HashMap<(&str, &str), Vec<&Link>> = HashMap::new();
    for l in &g.links {
        if want.as_ref().is_none_or(|w| w.contains(&l.relation))
            && nodes.contains(&l.source)
            && nodes.contains(&l.target)
        {
            pairs
                .entry(pair_key(&l.source, &l.target))
                .or_default()
                .push(l);
        }
    }
    header + &subgraph_to_text(g, &adj, &deg, &pairs, &nodes, &edges, token_budget, &seeds)
}

/// Go `wordRe`: Unicode-aware, as Python's `\w+`.
static WORD_RE: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"[\p{L}\p{N}_]+"));

/// Go `queryStopwords`: question and filler words dropped from the query terms.
const QUERY_STOPWORDS: &[&str] = &[
    // English
    "how",
    "what",
    "why",
    "when",
    "where",
    "which",
    "who",
    "whom",
    "whose",
    "does",
    "did",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "can",
    "could",
    "should",
    "would",
    "will",
    "shall",
    "may",
    "might",
    "must",
    "has",
    "have",
    "had",
    "the",
    "and",
    "but",
    "not",
    "for",
    "from",
    "with",
    "without",
    "into",
    "onto",
    "off",
    "that",
    "this",
    "these",
    "those",
    "there",
    "here",
    "its",
    "their",
    "them",
    "they",
    "about",
    "any",
    "all",
    "some",
    "work",
    "works",
    "working",
    // German
    "der",
    "die",
    "das",
    "den",
    "dem",
    "ein",
    "eine",
    "und",
    "oder",
    "nicht",
    "wie",
    "wer",
    "wann",
    "wo",
    "warum",
    "wieso",
    "welche",
    "welcher",
    "welches",
    "ist",
    "sind",
    "wird",
    "wurde",
    "hat",
    "haben",
    "kann",
    "koennen",
    "können",
    "soll",
    "muss",
    "sich",
    "bei",
    "mit",
    "von",
    "fuer",
    "für",
    "ueber",
    "über",
    "nach",
    "aus",
    "gibt",
    "es",
    "funktioniert",
    "geaendert",
    "geändert",
    "aendert",
    "ändert",
    // French
    "pourquoi",
    "quand",
    "quel",
    "quelle",
    "quels",
    "quelles",
    "quoi",
    "qui",
    "que",
    "est",
    "sont",
    "fonctionne",
    "cette",
    "dans",
    "avec",
    "où",
    // Spanish
    "cómo",
    "como",
    "qué",
    "cuál",
    "cuáles",
    "cuándo",
    "dónde",
    "donde",
    "porque",
    "por",
    "para",
    "funciona",
    "está",
    "están",
    "hay",
    // Portuguese
    "qual",
    "quais",
    "quando",
    "onde",
    "são",
    "estão",
    "tem",
    "uma",
    "não",
    // Italian
    "perché",
    "cosa",
    "quale",
    "quali",
    "dove",
    "funziona",
    "sono",
    "che",
    "della",
];

/// Go `queryTerms`: the searchable lowercase tokens of a question without stopwords (all of them
/// when every token is a stopword).
pub(super) fn query_terms(question: &str) -> Vec<String> {
    let mut searchable = Vec::new();
    for raw in fields(question) {
        for tok in WORD_RE.find_all_string(&to_lower(raw), -1) {
            if is_searchable(tok) {
                searchable.push(tok.to_string());
            }
        }
    }
    let terms: Vec<String> = searchable
        .iter()
        .filter(|t| !QUERY_STOPWORDS.contains(&t.as_str()))
        .cloned()
        .collect();
    if terms.is_empty() {
        return searchable; // all-stopword query: keep seeding on something
    }
    terms
}

/// Go `isSearchable`: an all-ASCII-letter word must be longer than two chars.
fn is_searchable(term: &str) -> bool {
    if term.chars().any(|ch| !ch.is_ascii_lowercase()) {
        return true;
    }
    term.len() > 2
}

const EXACT_MATCH_BONUS: f64 = 1000.0;
const PREFIX_MATCH_BONUS: f64 = 100.0;
const SUBSTRING_MATCH_BONUS: f64 = 1.0;
const SOURCE_MATCH_BONUS: f64 = 0.5;

/// Go `computeIDF`: IDF weights for the query terms.
fn compute_idf<'t>(g: &Graph, terms: &'t [String]) -> HashMap<&'t str, f64> {
    let mut n = g.nodes.len() as f64;
    if n == 0.0 {
        n = 1.0;
    }
    let mut df: HashMap<&str, i64> = terms.iter().map(|t| (t.as_str(), 0)).collect();
    for nd in &g.nodes {
        let norm = norm_label(nd);
        for (t, d) in df.iter_mut() {
            if norm.contains(*t) {
                *d += 1;
            }
        }
    }
    df.into_iter()
        .map(|(t, d)| (t, gostd::math::log(1.0 + n / (1 + d) as f64)))
        .collect()
}

/// Go `scoredNode`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScoredNode {
    pub score: f64,
    pub id: String,
    pub label_len: i64,
}

/// Go `scoreNodes`: the nodes with a positive score for `terms`, best first (ties: shorter
/// label, then id).
pub(super) fn score_nodes(g: &Graph, terms: &[String]) -> Vec<ScoredNode> {
    let idf = compute_idf(g, terms);
    let joined = terms.join(" ");
    let mut joined_w = 1.0;
    for t in terms {
        let w = idf[t.as_str()];
        if w > joined_w {
            joined_w = w;
        }
    }
    let mut out = Vec::new();
    for nd in &g.nodes {
        let norm = norm_label(nd);
        let bare = norm.trim_end_matches(['(', ')']);
        let source = to_lower(&nd.source_file);
        let attrs = attributes_text(nd);
        let mut score = 0.0;
        if !joined.is_empty() {
            let nid_lower = to_lower(&nd.id);
            if joined == norm || joined == bare || joined == nid_lower {
                score += EXACT_MATCH_BONUS * 10.0 * joined_w;
            } else if norm.starts_with(&joined) || bare.starts_with(&joined) {
                score += PREFIX_MATCH_BONUS * 10.0 * joined_w;
            }
        }
        for t in terms {
            let t = t.as_str();
            let w = idf[t];
            if t == norm || t == bare {
                score += EXACT_MATCH_BONUS * w;
            } else if norm.starts_with(t) || bare.starts_with(t) {
                score += PREFIX_MATCH_BONUS * w;
            } else if norm.contains(t) {
                score += SUBSTRING_MATCH_BONUS * w;
            }
            if source.contains(t) {
                score += SOURCE_MATCH_BONUS * w;
            }
            if !attrs.is_empty() && attrs.contains(t) {
                score += SOURCE_MATCH_BONUS * w;
            }
        }
        if score > 0.0 {
            out.push(ScoredNode {
                score,
                id: nd.id.clone(),
                label_len: label_of(nd).len() as i64,
            });
        }
    }
    gostd::sort::slice(&mut out, |a, b| {
        if a.score != b.score {
            return a.score > b.score;
        }
        if a.label_len != b.label_len {
            return a.label_len < b.label_len;
        }
        a.id < b.id
    });
    out
}

/// Go `pickSeeds`: up to `max_k` seeds, stopping below `gap_ratio` of the top score, one per
/// normalised label.
pub(super) fn pick_seeds(
    g: &Graph,
    scored: &[ScoredNode],
    max_k: usize,
    gap_ratio: f64,
) -> Vec<String> {
    let Some(first) = scored.first() else {
        return Vec::new();
    };
    let top = first.score;
    let mut seeds = Vec::new();
    let mut seen_label = HashSet::new();
    for s in scored {
        if seeds.len() >= max_k {
            break;
        }
        if !seeds.is_empty() && s.score < top * gap_ratio {
            break;
        }
        // Key on the normalised label, but keep label-less nodes distinct by the id.
        let mut label = s.id.clone();
        if let Some(nd) = g.node(&s.id) {
            let nl = norm_label(nd);
            if !nl.is_empty() {
                label = nl;
            }
        }
        if !seen_label.insert(label) {
            continue; // a homonym already holds a seed slot
        }
        seeds.push(s.id.clone());
    }
    seeds
}

/// Go `relationSet`: the relations to keep, `None` (Go `nil`) for all of them.
fn relation_set(relations: &[String]) -> Option<BTreeSet<String>> {
    if relations.is_empty() {
        return None;
    }
    Some(relations.iter().cloned().collect())
}

/// Go `relationAdj`: the adjacency and degree to traverse, the whole graph's when `want` is
/// `None` (Go `nil`), otherwise only the links whose relation is in `want`.
pub(super) fn relation_adj<'g>(
    g: &'g Graph,
    want: Option<&BTreeSet<String>>,
) -> (Cow<'g, Adj>, BTreeMap<String, i64>) {
    let mut adj = match want {
        None => Cow::Borrowed(&g.adj),
        Some(_) => Cow::Owned(Adj::new()),
    };
    let mut deg: BTreeMap<String, i64> = BTreeMap::new();
    for l in &g.links {
        if let Some(want) = want {
            if !want.contains(&l.relation) {
                continue;
            }
            let adj = adj.to_mut();
            adj.entry(l.source.clone())
                .or_default()
                .insert(l.target.clone());
            adj.entry(l.target.clone())
                .or_default()
                .insert(l.source.clone());
        }
        *deg.entry(l.source.clone()).or_default() += 1;
        *deg.entry(l.target.clone()).or_default() += 1;
    }
    (adj, deg)
}

/// Go `pairKey`.
fn pair_key<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if b < a { (b, a) } else { (a, b) }
}

/// Go `deg[id]`.
fn deg_of(deg: &BTreeMap<String, i64>, id: &str) -> i64 {
    deg.get(id).copied().unwrap_or(0)
}

/// Go `hubThreshold`: the p99 of the degrees of every node, floored at 50.
fn hub_threshold(g: &Graph, deg: &BTreeMap<String, i64>) -> i64 {
    let mut degrees: Vec<i64> = g.by_id.keys().map(|id| deg_of(deg, id)).collect();
    for id in g.adj.keys() {
        if !g.by_id.contains_key(id) {
            // dangling endpoints are nodes upstream
            degrees.push(deg_of(deg, id));
        }
    }
    if degrees.is_empty() {
        return 50;
    }
    degrees.sort_unstable();
    let p99 = ((degrees.len() as f64 * 0.99) as usize).min(degrees.len() - 1);
    degrees[p99].max(50)
}

/// Go `bfsTraverse`.
pub(super) fn bfs_traverse(
    g: &Graph,
    adj: &Adj,
    deg: &BTreeMap<String, i64>,
    seeds: &[String],
    depth: i64,
) -> Traversal {
    let hub = hub_threshold(g, deg);
    let seed_set: HashSet<&str> = seeds.iter().map(String::as_str).collect();
    let mut visited: BTreeSet<String> = seeds.iter().cloned().collect();
    let mut frontier: Vec<String> = seeds.to_vec();
    let mut edges = Vec::new();
    for _ in 0..depth {
        let mut next: Vec<String> = Vec::new();
        let mut next_set = HashSet::new();
        for n in &frontier {
            if !seed_set.contains(n.as_str()) && deg_of(deg, n) >= hub {
                continue;
            }
            for nb in neighbors(adj, n) {
                if !visited.contains(nb) {
                    if next_set.insert(nb) {
                        next.push(nb.clone());
                    }
                    edges.push((n.clone(), nb.clone()));
                }
            }
        }
        visited.extend(next.iter().cloned());
        frontier = next;
    }
    let edges = complete_induced_edges(adj, &visited, edges);
    (visited, edges)
}

/// Go `dfsTraverse`.
pub(super) fn dfs_traverse(
    g: &Graph,
    adj: &Adj,
    deg: &BTreeMap<String, i64>,
    seeds: &[String],
    depth: i64,
) -> Traversal {
    let hub = hub_threshold(g, deg);
    let seed_set: HashSet<&str> = seeds.iter().map(String::as_str).collect();
    let mut visited = BTreeSet::new();
    let mut edges = Vec::new();
    let mut stack: Vec<(&str, i64)> = seeds.iter().rev().map(|s| (s.as_str(), 0)).collect();
    while let Some((node, d)) = stack.pop() {
        if visited.contains(node) || d > depth {
            continue;
        }
        visited.insert(node.to_string());
        if !seed_set.contains(node) && deg_of(deg, node) >= hub {
            continue;
        }
        for nb in neighbors(adj, node) {
            if !visited.contains(nb) {
                stack.push((nb.as_str(), d + 1));
                edges.push((node.to_string(), nb.clone()));
            }
        }
    }
    let edges = complete_induced_edges(adj, &visited, edges);
    (visited, edges)
}

/// Go `completeInducedEdges`: appends every edge between two visited nodes the traversal did
/// not record (self-loops skipped), scanning the visited nodes in sorted order.
fn complete_induced_edges(
    adj: &Adj,
    visited: &BTreeSet<String>,
    mut edges: Vec<(String, String)>,
) -> Vec<(String, String)> {
    let mut seen: HashSet<(String, String)> = HashSet::with_capacity(edges.len() * 2);
    for (u, v) in &edges {
        seen.insert((u.clone(), v.clone()));
        seen.insert((v.clone(), u.clone()));
    }
    // BTreeSet iterates in sort.Strings order.
    for u in visited {
        for v in neighbors(adj, u) {
            if u == v || !visited.contains(v) || seen.contains(&(u.clone(), v.clone())) {
                continue;
            }
            seen.insert((u.clone(), v.clone()));
            seen.insert((v.clone(), u.clone()));
            edges.push((u.clone(), v.clone()));
        }
    }
    edges
}

/// Go `subgraphToText`: NODE and EDGE lines, seeds first, then the rest by hop distance from the
/// seeds and degree, cut at the token budget (~3 chars per token).
#[allow(clippy::too_many_arguments)]
fn subgraph_to_text(
    g: &Graph,
    adj: &Adj,
    deg: &BTreeMap<String, i64>,
    pairs: &HashMap<(&str, &str), Vec<&Link>>,
    nodes: &BTreeSet<String>,
    edges: &[(String, String)],
    token_budget: i64,
    seeds: &[String],
) -> String {
    let char_budget = token_budget * 3;
    let seed_set: HashSet<&str> = seeds.iter().map(String::as_str).collect();
    let mut ordered: Vec<&str> = seeds
        .iter()
        .filter(|s| nodes.contains(*s))
        .map(String::as_str)
        .collect();
    // keeps the answer above incidental hubs (upstream #BUG2)
    let mut dist: HashMap<&str, i64> = ordered.iter().map(|&s| (s, 0)).collect();
    let mut frontier = ordered.clone();
    let mut hop = 1;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for n in &frontier {
            for nb in adj.get(*n).into_iter().flatten() {
                if nodes.contains(nb) && !dist.contains_key(nb.as_str()) {
                    dist.insert(nb.as_str(), hop);
                    next.push(nb.as_str());
                }
            }
        }
        frontier = next;
        hop += 1;
    }
    let mut rest: Vec<&str> = Vec::new();
    for id in nodes {
        if !seed_set.contains(id.as_str()) {
            rest.push(id.as_str());
            dist.entry(id.as_str()).or_insert(1 << 30);
        }
    }
    gostd::sort::slice(&mut rest, |a, b| {
        if dist[a] != dist[b] {
            return dist[a] < dist[b];
        }
        let (da, db) = (deg_of(deg, a), deg_of(deg, b));
        if da != db {
            return da > db;
        }
        a < b
    });
    ordered.extend(rest);

    let mut lines = Vec::new();
    for id in ordered {
        let (mut label, mut src, mut loc, mut comm) = (id, "", "", String::new());
        if let Some(nd) = g.node(id) {
            label = label_of(nd);
            (src, loc) = (nd.source_file.as_str(), nd.source_location.as_str());
            if let Some(c) = nd.community {
                comm = c.to_string();
            }
        }
        lines.push(format!(
            "NODE {} [src={} loc={} community={}]",
            sanitize_label(label),
            sanitize_label(src),
            sanitize_label(loc),
            sanitize_label(&comm)
        ));
    }
    for (u, v) in edges {
        if !nodes.contains(u) || !nodes.contains(v) {
            continue;
        }
        for l in pairs.get(&pair_key(u, v)).into_iter().flatten() {
            let mut at = String::new();
            if !l.source_location.is_empty() {
                at = format!(
                    " at={}:{}",
                    sanitize_label(&l.source_file),
                    sanitize_label(&l.source_location)
                );
            }
            lines.push(format!(
                "EDGE {} --{} [{}]--> {}{at}",
                sanitize_label(node_label(g, &l.source)),
                sanitize_label(&l.relation),
                sanitize_label(&l.confidence),
                sanitize_label(node_label(g, &l.target))
            ));
        }
    }
    let output = lines.join("\n");
    if char_budget > 0 && output.len() as i64 > char_budget {
        let b = output.as_bytes();
        let budget = char_budget as usize;
        let cut = match b[..budget].iter().rposition(|&c| c == b'\n') {
            Some(i) if i > 0 => i,
            _ => budget,
        };
        let total = lines.iter().filter(|l| l.starts_with("NODE ")).count();
        // Go cuts bytes: with no newline in the budget the cut can split a rune, which a Rust
        // `String` cannot hold, so the partial rune becomes U+FFFD.
        let head = String::from_utf8_lossy(&b[..cut]);
        let mut shown = head.matches("\nNODE ").count();
        if output.starts_with("NODE ") {
            shown += 1;
        }
        return format!(
            "{head}\n... (truncated — {} more nodes cut by ~{token_budget}-token budget. Narrow the question or use `graphify explain` for a specific symbol)",
            total - shown
        );
    }
    output
}

/// Go `attributesText`: a node's attributes as one lowercased "key value ..." string; empty for
/// a node with none.
// D-009: Go joins the pairs in map order (ask.go:574); here they are in sorted key order, which
// changes Go's answer only when a term spans two attributes.
fn attributes_text(n: &Node) -> String {
    if n.attributes.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = n
        .attributes
        .iter()
        .flat_map(|(k, v)| [k.as_str(), v.as_str()])
        .collect();
    to_lower(&parts.join(" "))
}

/// Go `normLabel`: the lowercase norm_label, else the lowercased label.
fn norm_label(n: &Node) -> String {
    if !n.norm_label.is_empty() {
        return to_lower(&n.norm_label);
    }
    to_lower(&n.label)
}

/// Go `labelOf`: the label, else the id.
fn label_of(n: &Node) -> &str {
    if n.label.is_empty() { &n.id } else { &n.label }
}

/// Go `nodeLabel`: the label for a node id, else the id.
fn node_label<'a>(g: &'a Graph, id: &'a str) -> &'a str {
    g.node(id).map_or(id, label_of)
}

/// Go `neighbors`: a node's neighbour ids in sorted order (a `BTreeSet` already is).
fn neighbors<'a>(adj: &'a Adj, id: &str) -> impl Iterator<Item = &'a String> {
    adj.get(id).into_iter().flatten()
}
