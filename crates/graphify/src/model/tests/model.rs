//! Port of graphify-go `internal/model/model_test.go`. `TestAddEdgesAtMatchesAddEdge` is adapted:
//! Rust's `add_edges_at` takes the edges by value, so Go's checks that the caller's edges are not
//! written and that some edges are kept in place (pointer identity) have no Rust form.

use super::Rng;
use crate::model::{Edge, Graph, NO_NODE, Node, rel};

fn node(id: &str) -> Node {
    Node {
        id: id.into(),
        ..Node::default()
    }
}

fn edge(source: &str, target: &str, relation: &str) -> Edge {
    Edge {
        source: source.into(),
        target: target.into(),
        relation: rel(relation),
        ..Edge::default()
    }
}

/// Edges whose ids hold NUL collided under the joined dedup key of v0.10.1.
#[test]
fn test_add_edge_nul_ids_collide() {
    let mut g = Graph::new();
    for id in ["a", "\x00a", "a\x00"] {
        g.add_node(node(id));
    }
    g.add_edge(edge("a", "\x00a", "r"));
    g.add_edge(edge("a\x00", "a", "r"));
    assert_eq!(g.edges().len(), 1);
}

/// Both directions, a second relation and a self-loop each count once.
#[test]
fn test_neighbors_are_distinct() {
    let mut g = Graph::new();
    for id in ["c", "a", "b"] {
        g.add_node(node(id));
    }
    for e in [
        edge("a", "b", "r"),
        edge("b", "a", "r"),
        edge("a", "b", "s"),
        edge("a", "a", "r"),
        edge("c", "a", "r"),
    ] {
        g.add_edge(e);
    }
    assert_eq!(g.neighbors("a"), ["a", "b", "c"]);
    assert_eq!((g.degree("a"), g.degree("b"), g.degree("x")), (3, 1, 0));
    assert!(
        g.has_edge("b", "a")
            && g.has_edge("a", "a")
            && !g.has_edge("b", "c")
            && !g.has_edge("b", "x")
    );
}

/// `add_edges_at` must leave the graph `add_edge` leaves, edge and neighbour order included,
/// also when ids with NUL make different edges collide.
#[test]
fn test_add_edges_at_matches_add_edge() {
    let mut rng = Rng::new(1);
    for ids in [
        ["a", "b", "c", "d", "e"],
        ["a", "\x00a", "a\x00", "a\x00a", "b"],
    ] {
        for _round in 0..200 {
            let (mut one, mut bulk) = (Graph::new(), Graph::new());
            for id in ids {
                one.add_node(node(id));
                bulk.add_node(node(id));
            }
            let n = rng.intn(40);
            let mut edges = Vec::with_capacity(n);
            let mut order: Vec<i32> = (0..n as i32).collect();
            let (mut src, mut tgt) = (vec![0u32; n], vec![0u32; n]);
            for i in 0..n {
                src[i] = rng.intn(ids.len()) as u32;
                tgt[i] = rng.intn(ids.len()) as u32;
                let mut e = edge("x", "y", ["r", "s", "a\x00r"][rng.intn(3)]);
                e.weight = i as f64;
                if rng.intn(2) == 0 {
                    e.source = ids[src[i] as usize].into();
                }
                if rng.intn(2) == 0 {
                    e.target = ids[tgt[i] as usize].into();
                }
                edges.push(e);
            }
            rng.shuffle(&mut order);
            for &i in &order {
                let i = i as usize;
                if rng.intn(8) == 0 {
                    src[i] = NO_NODE;
                    continue;
                }
                let mut e = edges[i].clone();
                e.source = ids[src[i] as usize].into();
                e.target = ids[tgt[i] as usize].into();
                one.add_edge(e);
            }
            bulk.add_edges_at(edges, &order, &src, &tgt);
            assert_eq!(bulk.edges(), one.edges(), "edges differ");
            assert_eq!(bulk.adj, one.adj, "neighbours differ");
            assert_eq!(bulk.pairs, one.pairs, "pairs differ");
        }
    }
}
