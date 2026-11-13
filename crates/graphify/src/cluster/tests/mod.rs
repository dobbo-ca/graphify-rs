//! Shared helpers (Go: the test files of package cluster share one namespace).

use std::collections::BTreeMap;

use crate::model::{Edge, Graph, Node, Relation};

mod cluster;
mod gorand;
mod louvain;

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// Go `model.Edge{Source: s, Target: t, Relation: model.RelCalls}`.
fn calls(s: &str, t: &str) -> Edge {
    Edge {
        source: s.to_string(),
        target: t.to_string(),
        relation: Relation::CALLS,
        ..Edge::default()
    }
}

/// `communityID -> members` from literals.
fn communities(entries: &[(i64, &[&str])]) -> BTreeMap<i64, Vec<String>> {
    entries
        .iter()
        .map(|(cid, nodes)| (*cid, strs(nodes)))
        .collect()
}

/// The members of community `cid`, empty when absent (Go: a nil slice).
fn members(m: &BTreeMap<i64, Vec<String>>, cid: i64) -> &[String] {
    m.get(&cid).map(Vec::as_slice).unwrap_or_default()
}

/// Go `twoTriangles`: two triangles (a1-a2-a3, b1-b2-b3) joined by a single bridge edge a1-b1,
/// a graph with clear community structure.
fn two_triangles() -> Graph {
    let mut g = Graph::new();
    for id in ["a1", "a2", "a3", "b1", "b2", "b3"] {
        g.add_node(Node {
            id: id.to_string(),
            label: id.to_string(),
            source_file: format!("pkg/{id}.go"),
            ..Node::default()
        });
    }
    g.add_edge(calls("a1", "a2"));
    g.add_edge(calls("a2", "a3"));
    g.add_edge(calls("a3", "a1"));
    g.add_edge(calls("b1", "b2"));
    g.add_edge(calls("b2", "b3"));
    g.add_edge(calls("b3", "b1"));
    g.add_edge(calls("a1", "b1")); // bridge
    g
}

/// Go `nodeCommunityOf`: inverts a communities map into node -> community ID.
fn node_community_of(communities: &BTreeMap<i64, Vec<String>>) -> BTreeMap<String, i64> {
    let mut m = BTreeMap::new();
    for (cid, nodes) in communities {
        for n in nodes {
            m.insert(n.clone(), *cid);
        }
    }
    m
}

/// Go `equalMembers`: whether two node slices contain the same members, ignoring order.
fn equal_members(a: &[String], b: &[&str]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut seen: BTreeMap<&str, i64> = BTreeMap::new();
    for x in a {
        *seen.entry(x.as_str()).or_default() += 1;
    }
    for x in b {
        *seen.entry(x).or_default() -= 1;
    }
    seen.values().all(|c| *c == 0)
}
