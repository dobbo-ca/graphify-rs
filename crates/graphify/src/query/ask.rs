//! Port of `ask.go`: answer a question by TF-IDF seeding and a bounded BFS or DFS, rendered as a
//! token-budgeted text block. The IDF uses `gostd::math::log` (plan R10, Go's amd64 `archLog`).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use super::query::Graph;

// graphify-rs-ihc.13: stubs until the code PR

/// Go `map[string]map[string]bool` adjacency: node id to its neighbour ids.
#[allow(dead_code, unused_variables)]
pub(super) type Adj = BTreeMap<String, BTreeSet<String>>;

/// What `bfs_traverse` and `dfs_traverse` return: the visited node set and the traversed
/// (then induced) edges, in Go's order.
#[allow(dead_code, unused_variables)]
pub(super) type Traversal = (BTreeSet<String>, Vec<(String, String)>);

/// Go `Ask`: the answer to `question` (`depth` hops, `token_budget` tokens, edges limited to
/// `relations` when it is not empty).
#[allow(dead_code, unused_variables)]
pub fn ask(
    g: &Graph,
    question: &str,
    dfs: bool,
    depth: i64,
    token_budget: i64,
    relations: &[String],
) -> String {
    todo!("graphify-rs-ihc.13")
}

/// Go `queryTerms`: the searchable lowercase tokens of a question without stopwords (all of them
/// when every token is a stopword).
#[allow(dead_code, unused_variables)]
pub(super) fn query_terms(question: &str) -> Vec<String> {
    todo!("graphify-rs-ihc.13")
}

/// Go `scoredNode`.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScoredNode {
    pub score: f64,
    pub id: String,
    pub label_len: i64,
}

/// Go `scoreNodes`: the nodes with a positive score for `terms`, best first (ties: shorter
/// label, then id).
#[allow(dead_code, unused_variables)]
pub(super) fn score_nodes(g: &Graph, terms: &[String]) -> Vec<ScoredNode> {
    todo!("graphify-rs-ihc.13")
}

/// Go `pickSeeds`: up to `max_k` seeds, stopping below `gap_ratio` of the top score, one per
/// normalised label.
#[allow(dead_code, unused_variables)]
pub(super) fn pick_seeds(
    g: &Graph,
    scored: &[ScoredNode],
    max_k: usize,
    gap_ratio: f64,
) -> Vec<String> {
    todo!("graphify-rs-ihc.13")
}

/// Go `relationAdj`: the adjacency and degree to traverse, the whole graph's when `want` is
/// `None` (Go `nil`), otherwise only the links whose relation is in `want`.
#[allow(dead_code, unused_variables)]
pub(super) fn relation_adj<'g>(
    g: &'g Graph,
    want: Option<&BTreeSet<String>>,
) -> (Cow<'g, Adj>, BTreeMap<String, i64>) {
    todo!("graphify-rs-ihc.13")
}

/// Go `bfsTraverse`.
#[allow(dead_code, unused_variables)]
pub(super) fn bfs_traverse(
    g: &Graph,
    adj: &Adj,
    deg: &BTreeMap<String, i64>,
    seeds: &[String],
    depth: i64,
) -> Traversal {
    todo!("graphify-rs-ihc.13")
}

/// Go `dfsTraverse`.
#[allow(dead_code, unused_variables)]
pub(super) fn dfs_traverse(
    g: &Graph,
    adj: &Adj,
    deg: &BTreeMap<String, i64>,
    seeds: &[String],
    depth: i64,
) -> Traversal {
    todo!("graphify-rs-ihc.13")
}
