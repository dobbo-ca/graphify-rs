//! Port of `cluster_test.go` (10 tests), then the upstream cases of `test_cluster.py` and
//! `test_cluster_ambiguous_scale.py` that hold for graphify-go v0.12.0.

use std::collections::{BTreeMap, BTreeSet};

use super::{calls, communities, equal_members, members, node_community_of, strs, two_triangles};
use crate::cluster::{cluster, cohesion, node_community, remap_to_previous, scores};
use crate::model::{Confidence, Edge, Graph, Node};

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cluster_empty() {
    let got = cluster(&Graph::new());
    assert!(got.is_empty(), "Cluster(empty) = {got:?}, want empty");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cluster_covers_every_node_once() {
    let g = two_triangles();
    let communities = cluster(&g);
    assert!(
        !communities.is_empty(),
        "Cluster returned no communities for a non-empty graph"
    );
    let mut seen: BTreeMap<&str, i64> = BTreeMap::new();
    for nodes in communities.values() {
        for n in nodes {
            *seen.entry(n.as_str()).or_default() += 1;
        }
    }
    assert_eq!(
        seen.len(),
        g.num_nodes(),
        "communities cover {} nodes, want {}",
        seen.len(),
        g.num_nodes()
    );
    for (id, count) in &seen {
        assert_eq!(
            *count, 1,
            "node {id:?} assigned to {count} communities, want 1"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cohesion() {
    let g = two_triangles();
    // A full triangle has all 3 possible internal edges.
    let got = cohesion(&g, &strs(&["a1", "a2", "a3"]));
    assert!(got == 1.0, "Cohesion(triangle) = {got}, want 1.0");
    // A single node is trivially cohesive.
    let got = cohesion(&g, &strs(&["a1"]));
    assert!(got == 1.0, "Cohesion(single) = {got}, want 1.0");
    // Two unconnected nodes (a2 and b2 share no edge) have no internal edges.
    let got = cohesion(&g, &strs(&["a2", "b2"]));
    assert!(got == 0.0, "Cohesion(unconnected pair) = {got}, want 0.0");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_node_community_and_scores() {
    let g = two_triangles();
    let communities = cluster(&g);

    let nc = node_community(&communities);
    for (cid, nodes) in &communities {
        for n in nodes {
            assert_eq!(
                nc.get(n),
                Some(cid),
                "NodeCommunity[{n:?}] = {:?}, want {cid}",
                nc.get(n)
            );
        }
    }

    let scores = scores(&g, &communities);
    assert_eq!(
        scores.len(),
        communities.len(),
        "Scores has {} entries, want {}",
        scores.len(),
        communities.len()
    );
    for cid in communities.keys() {
        assert!(scores.contains_key(cid), "Scores missing community {cid}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_to_previous_empty() {
    let prev = BTreeMap::from([("a".to_string(), 0)]);
    let got = remap_to_previous(&BTreeMap::new(), &prev);
    assert!(
        got.is_empty(),
        "RemapToPrevious(empty) = {got:?}, want empty"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_to_previous_stable_when_unchanged() {
    // Same grouping, but the new run permuted the integer IDs.
    let prev = communities(&[(0, &["a1", "a2", "a3"]), (1, &["b1", "b2"])]);
    let current = communities(&[(0, &["b1", "b2"]), (1, &["a1", "a2", "a3"])]);

    let got = remap_to_previous(&current, &node_community_of(&prev));

    // After remap, IDs should line up with prev exactly.
    assert!(
        equal_members(members(&got, 0), &["a1", "a2", "a3"]),
        "community 0 = {:?}, want a1,a2,a3",
        members(&got, 0)
    );
    assert!(
        equal_members(members(&got, 1), &["b1", "b2"]),
        "community 1 = {:?}, want b1,b2",
        members(&got, 1)
    );
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_to_previous_greedy_by_overlap() {
    // Old community 0 = {a1,a2,a3}, 1 = {b1,b2}.
    let prev = communities(&[(0, &["a1", "a2", "a3"]), (1, &["b1", "b2"])]);
    // New community 5 overlaps old-0 most (a1,a2); new 9 overlaps old-1 most (b1,b2).
    let current = communities(&[(5, &["a1", "a2"]), (9, &["b1", "b2", "a3"])]);

    let got = remap_to_previous(&current, &node_community_of(&prev));

    assert!(
        equal_members(members(&got, 0), &["a1", "a2"]),
        "expected new {{a1,a2}} to inherit old ID 0, got[0]={:?}",
        members(&got, 0)
    );
    assert!(
        equal_members(members(&got, 1), &["a3", "b1", "b2"]),
        "expected new {{b1,b2,a3}} to inherit old ID 1, got[1]={:?}",
        members(&got, 1)
    );
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_to_previous_fresh_ids_avoid_collisions() {
    // One new community matches old ID 0; an unmatched one must NOT reuse 0.
    let prev = communities(&[(0, &["a1", "a2"])]);
    let current = communities(&[(7, &["a1", "a2"]), (8, &["c1", "c2", "c3"])]);

    let got = remap_to_previous(&current, &node_community_of(&prev));

    assert!(
        equal_members(members(&got, 0), &["a1", "a2"]),
        "matched community should keep old ID 0, got[0]={:?}",
        members(&got, 0)
    );
    // The unmatched community gets the next free ID (1), not 0.
    assert!(
        equal_members(members(&got, 1), &["c1", "c2", "c3"]),
        "unmatched community should get fresh ID 1, got[1]={:?}",
        members(&got, 1)
    );
    assert_eq!(
        got.len(),
        2,
        "got {} communities, want 2 (no collision/overwrite)",
        got.len()
    );
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_to_previous_no_previous_is_deterministic() {
    // With no previous assignment, all IDs are fresh, ordered size desc then
    // lexically, matching reindexBySize's contract.
    let current = communities(&[(3, &["b"]), (7, &["a"]), (1, &["x", "y", "z"])]);

    let got = remap_to_previous(&current, &BTreeMap::new());

    assert!(
        equal_members(members(&got, 0), &["x", "y", "z"]),
        "largest community should be ID 0, got[0]={:?}",
        members(&got, 0)
    );
    // Single-node {a} and {b} tie on size; lexical tie-break puts {a} first.
    assert!(
        equal_members(members(&got, 1), &["a"]),
        "expected {{a}} at ID 1, got[1]={:?}",
        members(&got, 1)
    );
    assert!(
        equal_members(members(&got, 2), &["b"]),
        "expected {{b}} at ID 2, got[2]={:?}",
        members(&got, 2)
    );
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cluster_deterministic() {
    let a = cluster(&two_triangles());
    let b = cluster(&two_triangles());
    assert_eq!(
        a.len(),
        b.len(),
        "community count not deterministic: {} vs {}",
        a.len(),
        b.len()
    );
    for (cid, nodes) in &a {
        let other = members(&b, *cid);
        assert_eq!(
            nodes.len(),
            other.len(),
            "community {cid} differs across runs"
        );
        for (i, n) in nodes.iter().enumerate() {
            assert_eq!(
                n, &other[i],
                "community {cid} member {i} differs: {n:?} vs {:?}",
                other[i]
            );
        }
    }
}

// Upstream graphify v0.9.107 `tests/test_cluster.py`. Cases whose assertion needs no Leiden,
// no fixture and no Python-only behaviour; the rest are under "Not ported" in parity/README.md.

/// Python `nx.Graph` with the given nodes and edges (relation calls, no weights).
fn graph(ids: &[&str], edges: &[(&str, &str)]) -> Graph {
    let mut g = Graph::new();
    for id in ids {
        g.add_node(Node {
            id: (*id).to_string(),
            ..Node::default()
        });
    }
    for (s, t) in edges {
        g.add_edge(calls(s, t));
    }
    g
}

/// `nx.complete_graph(4)` relabelled to "0".."3".
fn complete4() -> Graph {
    let mut edges = Vec::new();
    for i in 0..4 {
        for j in i + 1..4 {
            edges.push((i.to_string(), j.to_string()));
        }
    }
    let edges: Vec<(&str, &str)> = edges
        .iter()
        .map(|(s, t)| (s.as_str(), t.as_str()))
        .collect();
    graph(&["0", "1", "2", "3"], &edges)
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cohesion_score_complete_graph() {
    let g = complete4();
    let score = cohesion(&g, g.node_ids());
    assert!(score == 1.0, "cohesion(K4) = {score}, want 1.0");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cohesion_score_disconnected() {
    let g = graph(&["a", "b", "c"], &[]);
    let score = cohesion(&g, &strs(&["a", "b", "c"]));
    assert!(score == 0.0, "cohesion(3 isolated) = {score}, want 0.0");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cohesion_score_ignores_self_loops() {
    // #3558: a recursive calls self-loop must not inflate the ratio. Two nodes
    // with one real edge plus a self-loop is still fully cohesive (1.0), not 2.0.
    let g = graph(&["a", "b"], &[("a", "b"), ("a", "a")]);
    let score = cohesion(&g, &strs(&["a", "b"]));
    assert!(score == 1.0, "cohesion = {score}, want 1.0");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cohesion_score_self_loops_only_is_zero() {
    // Self-loops alone are not inter-node connectivity.
    let g = graph(&["a", "b", "c"], &[("a", "a"), ("b", "b"), ("c", "c")]);
    let score = cohesion(&g, &strs(&["a", "b", "c"]));
    assert!(score == 0.0, "cohesion = {score}, want 0.0");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_cohesion_score_range_with_self_loops() {
    // The 0..1 bound holds even when every node carries a self-loop.
    let mut g = complete4();
    for n in ["0", "1", "2", "3"] {
        g.add_edge(calls(n, n));
    }
    let score = cohesion(&g, g.node_ids());
    assert!(
        (0.0..=1.0).contains(&score),
        "cohesion = {score}, want within 0..=1"
    );
    assert!(score == 1.0, "cohesion = {score}, want 1.0");
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_communities_to_previous_reuses_old_ids() {
    let current = communities(&[(10, &["a", "b", "c"]), (11, &["d", "e"])]);
    let previous: BTreeMap<String, i64> = [("a", 5), ("b", 5), ("c", 5), ("d", 1), ("e", 1)]
        .into_iter()
        .map(|(n, cid)| (n.to_string(), cid))
        .collect();
    let remapped = remap_to_previous(&current, &previous);
    assert_eq!(remapped.keys().copied().collect::<Vec<_>>(), [1, 5]);
    assert_eq!(remapped[&5], ["a", "b", "c"]);
    assert_eq!(remapped[&1], ["d", "e"]);
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_remap_communities_to_previous_assigns_deterministic_new_ids() {
    let current = communities(&[(7, &["x", "y", "z"]), (8, &["m"])]);
    let previous = BTreeMap::from([("a".to_string(), 3)]);
    let remapped = remap_to_previous(&current, &previous);
    assert_eq!(remapped.keys().copied().collect::<Vec<_>>(), [0, 1]);
    assert_eq!(remapped[&0], ["x", "y", "z"]);
    assert_eq!(remapped[&1], ["m"]);
}

/// Only the edge-orientation half of the upstream test: Go's community ids and the Louvain visit
/// order depend on node insertion order (node index = position), so the upstream shuffled-node
/// build is not an invariant of graphify-go v0.12.0.
#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_partition_is_invariant_to_edge_endpoint_orientation() {
    // #3146
    let edges = [
        ("a1", "a2"),
        ("a1", "a3"),
        ("a2", "a3"),
        ("a3", "a4"),
        ("b1", "b2"),
        ("b1", "b3"),
        ("b2", "b3"),
        ("b3", "b4"),
        ("a1", "b1"),
    ];
    let nodes: BTreeSet<&str> = edges.iter().flat_map(|(s, t)| [*s, *t]).collect();
    let nodes: Vec<&str> = nodes.into_iter().collect();
    let flipped: Vec<(&str, &str)> = edges.iter().map(|(s, t)| (*t, *s)).collect();

    let forward = cluster(&graph(&nodes, &edges));
    let backward = cluster(&graph(&nodes, &flipped));
    assert_eq!(
        forward, backward,
        "partition drifted with edge-endpoint orientation"
    );
}

// Upstream `tests/test_cluster_ambiguous_scale.py`. graphify-go has no ambiguous scale (no
// `GRAPHIFY_AMBIGUOUS_SCALE`, no edge weights in Louvain): only the two cases that assert
// coverage of every node hold, without the scale argument.

fn weighted_edge(s: &str, t: &str, confidence: Confidence) -> Edge {
    Edge {
        weight: 1.0,
        confidence,
        ..calls(s, t)
    }
}

fn cover_all(g: &Graph) {
    let communities = cluster(g);
    let all_nodes: BTreeSet<&str> = communities.values().flatten().map(String::as_str).collect();
    let want: BTreeSet<&str> = g.node_ids().iter().map(String::as_str).collect();
    assert_eq!(all_nodes, want);
    assert_eq!(
        communities.values().map(Vec::len).sum::<usize>(),
        g.num_nodes()
    );
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_scale_zero_still_partitions() {
    // Two 4-node cliques linked by five AMBIGUOUS cross-edges (#4199).
    let mut g = Graph::new();
    let left: Vec<String> = (0..4).map(|i| format!("L{i}")).collect();
    let right: Vec<String> = (0..4).map(|i| format!("R{i}")).collect();
    for id in left.iter().chain(&right) {
        g.add_node(Node {
            id: id.clone(),
            ..Node::default()
        });
    }
    for group in [&left, &right] {
        for (i, a) in group.iter().enumerate() {
            for b in &group[i + 1..] {
                g.add_edge(weighted_edge(a, b, Confidence::EXTRACTED));
            }
        }
    }
    for i in 0..4 {
        g.add_edge(weighted_edge(
            &format!("L{i}"),
            &format!("R{i}"),
            Confidence::AMBIGUOUS,
        ));
    }
    g.add_edge(weighted_edge("L0", "R1", Confidence::AMBIGUOUS));
    cover_all(&g);
}

#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_no_copy_when_no_ambiguous_edges() {
    let mut g = Graph::new();
    for i in 0..4 {
        g.add_node(Node {
            id: format!("n{i}"),
            ..Node::default()
        });
    }
    for i in 0..4 {
        for j in i + 1..4 {
            g.add_edge(weighted_edge(
                &format!("n{i}"),
                &format!("n{j}"),
                Confidence::EXTRACTED,
            ));
        }
    }
    cover_all(&g);
}
