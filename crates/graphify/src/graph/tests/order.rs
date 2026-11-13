//! Port of graphify-go `internal/graph/order_test.go`. `TestBuildPartsMatchesBuild` drops Go's
//! "BuildParts wrote to its parts" check: Rust's `build_parts` takes the parts by value.

use crate::graph::build::edge_order;
use crate::graph::{build, build_parts};
use crate::model::{Edge, Extraction, Node, conf, loc_of, rel};
use crate::testutil::Rng;

/// `edge_order` must reproduce a stable sort on (source, target, relation), including the
/// relative order of equal keys.
#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_edge_order_matches_stable_sort() {
    let mut rng = Rng::new(1);
    for n in [0usize, 1, 2, 3, 50, 1000, 200_000] {
        // Few distinct keys, so most edges tie on one, two or all three fields.
        let k = 1 + n / 200;
        let edges: Vec<Edge> = (0..n)
            .map(|i| Edge {
                source: format!("s{}", rng.intn(k)),
                target: format!("t{}", rng.intn(3)),
                relation: rel(["calls", "imports", "contains"][rng.intn(3)]),
                source_location: loc_of(&i.to_string()),
                ..Edge::default()
            })
            .collect();
        let mut want = edges.clone();
        want.sort_by(|a, b| {
            (a.source.as_str(), a.target.as_str(), a.relation.as_str()).cmp(&(
                b.source.as_str(),
                b.target.as_str(),
                b.relation.as_str(),
            ))
        });
        for workers in [1, 4] {
            let order = edge_order(&edges, workers);
            assert_eq!(order.len(), n, "n={n}");
            for (pos, &i) in order.iter().enumerate() {
                assert_eq!(
                    edges[i as usize], want[pos],
                    "n={n} workers={workers}: position {pos}"
                );
            }
        }
    }
}

/// `build_parts` must leave the graph `build` leaves for the joined extraction, wherever the
/// parts are cut.
#[test]
#[ignore = "graphify-rs-ihc.8"]
fn test_build_parts_matches_build() {
    let mut rng = Rng::new(2);
    for round in 0..100 {
        let mut ext = Extraction::default();
        for i in 0..1 + rng.intn(30) {
            // Few ids, some spelled two ways: repeated nodes, edges that tie on all three keys,
            // endpoints found by their normalized form.
            let id = format!("{}{}", ["n", "N.", "m"][rng.intn(3)], rng.intn(4));
            ext.nodes.push(Node {
                id,
                label: i.to_string(),
                source_file: ["", "a.go", "b.py"][rng.intn(3)].into(),
                ..Node::default()
            });
        }
        for i in 0..rng.intn(80) {
            ext.edges.push(Edge {
                source: format!("{}{}", ["n", "N.", "m", "x"][rng.intn(4)], rng.intn(4)),
                target: format!("{}{}", ["n", "n.", "m"][rng.intn(3)], rng.intn(4)),
                relation: rel(["calls", "imports", "contains"][rng.intn(3)]),
                confidence: conf(["INFERRED", "EXTRACTED"][rng.intn(2)]),
                weight: i as f64,
                ..Edge::default()
            });
        }
        let cut = |rng: &mut Rng, len: usize| {
            let mut bounds = Vec::new();
            let mut lo = 0;
            while lo < len {
                let hi = lo + rng.intn(len - lo + 1);
                bounds.push((lo, hi));
                lo = hi;
            }
            bounds
        };
        let nodes: Vec<Vec<Node>> = cut(&mut rng, ext.nodes.len())
            .into_iter()
            .map(|(lo, hi)| ext.nodes[lo..hi].to_vec())
            .collect();
        let edges: Vec<Vec<Edge>> = cut(&mut rng, ext.edges.len())
            .into_iter()
            .map(|(lo, hi)| ext.edges[lo..hi].to_vec())
            .collect();
        let want = build(ext);
        let got = build_parts(nodes, edges, 1 + round % 4);
        assert_eq!(got.node_ids(), want.node_ids(), "round {round}: nodes");
        for id in want.node_ids() {
            assert_eq!(got.node(id), want.node(id), "round {round}: node {id}");
            assert_eq!(
                got.neighbor_indexes(id),
                want.neighbor_indexes(id),
                "round {round}: neighbours of {id}"
            );
        }
        assert_eq!(got.edges(), want.edges(), "round {round}: edges");
    }
}
