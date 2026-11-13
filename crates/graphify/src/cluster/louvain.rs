//! Port of `louvain.go`: gonum `community.Modularize` on an unweighted graph, seed 42.

/// Go `modularize`: Louvain over a deduplicated, self-loop-free undirected adjacency on nodes
/// `0..n`, as node-index groups.
pub(super) fn modularize(adj: &[Vec<i64>]) -> Vec<Vec<i64>> {
    todo!("graphify-rs-ihc.9")
}

/// Go `modularize` with the package variable `lookAtAll` as an argument (a `static` would race
/// between parallel tests): `true` turns the skip in `localMove` off.
pub(super) fn modularize_with(adj: &[Vec<i64>], look_at_all: bool) -> Vec<Vec<i64>> {
    todo!("graphify-rs-ihc.9")
}

/// The visit order of the first sweep of `localMove` (louvain.go:130-133) on `n` nodes: `order`
/// after the Fisher-Yates driven by a fresh seed-42 PCG (`rand.New(rand.NewSource(seed)).Intn`).
pub(super) fn local_move_order(n: usize) -> Vec<usize> {
    todo!("graphify-rs-ihc.9")
}
