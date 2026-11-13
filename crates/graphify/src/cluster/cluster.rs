//! Port of `cluster.go`: Louvain, oversized-community split, and stable community ids.

use std::collections::{BTreeMap, BTreeSet};

use super::louvain::modularize;
use crate::model::Graph;

/// Go `maxCommunityFraction` (cluster.go:14): communities larger than this share of the graph are
/// split.
const MAX_COMMUNITY_FRACTION: f64 = 0.25;
/// Go `minSplitSize` (cluster.go:15): ...but only if they have at least this many nodes.
const MIN_SPLIT_SIZE: i64 = 10;
/// Go `seed` (cluster.go:16).
pub(super) const SEED: u64 = 42;

/// Go `Cluster` (cluster.go:20): partitions `g` into communities, `communityID -> sorted node IDs`.
pub fn cluster(g: &Graph) -> BTreeMap<i64, Vec<String>> {
    if g.num_nodes() == 0 {
        return BTreeMap::new();
    }
    let groups = louvain(g, g.node_ids());

    // Split any community that dominates the graph.
    let mut max_size = MIN_SPLIT_SIZE;
    let frac = (g.num_nodes() as f64 * MAX_COMMUNITY_FRACTION) as i64;
    if frac > max_size {
        max_size = frac;
    }
    let mut last = Vec::new();
    for nodes in groups {
        if nodes.len() as i64 > max_size {
            last.extend(split_oversized(g, nodes));
        } else {
            last.push(nodes);
        }
    }
    reindex_by_size(last)
}

/// Go `louvain` (cluster.go:44): Louvain over the induced subgraph on `node_set`, as string-ID
/// groups.
pub(super) fn louvain(g: &Graph, node_set: &[String]) -> Vec<Vec<String>> {
    // Position in node_set of each graph node, -1 outside it.
    let mut local = vec![-1i64; g.num_nodes()];
    for (i, id) in node_set.iter().enumerate() {
        if let Some(at) = g.index(id) {
            local[at as usize] = i as i64;
        }
    }
    // Neighbor order is free: modularize gives the same result for any order.
    let mut adj: Vec<Vec<i64>> = vec![Vec::new(); node_set.len()];
    for (i, id) in node_set.iter().enumerate() {
        for &nb in g.neighbor_indexes(id) {
            let j = local[nb as usize];
            if j >= 0 && j != i as i64 {
                adj[i].push(j);
            }
        }
    }

    modularize(&adj)
        .into_iter()
        .map(|comm| {
            comm.into_iter()
                .map(|n| node_set[n as usize].clone())
                .collect()
        })
        .collect()
}

/// Go `splitOversized` (cluster.go:79): a second Louvain pass on a community's subgraph. If it
/// doesn't break apart (or has no internal edges), the community is returned unchanged so the
/// caller doesn't lose nodes.
fn split_oversized(g: &Graph, nodes: Vec<String>) -> Vec<Vec<String>> {
    let sub = louvain(g, &nodes);
    if sub.len() <= 1 {
        return vec![nodes];
    }
    sub
}

/// Go `reindexBySize` (cluster.go:89): sorts communities by descending size (lexical tie-break on
/// sorted member IDs) and assigns sequential IDs, making the mapping reproducible.
fn reindex_by_size(mut groups: Vec<Vec<String>>) -> BTreeMap<i64, Vec<String>> {
    for g in &mut groups {
        g.sort();
    }
    // Go `sort.SliceStable` with `len desc, less`. Slice `Ord` is `less` (lexicographic, shorter
    // prefix first), and the lengths are equal whenever it is reached.
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    groups
        .into_iter()
        .enumerate()
        .map(|(i, g)| (i as i64, g))
        .collect()
}

/// Go `less` (cluster.go:106): lexicographic order on string slices.
fn less(a: &[String], b: &[String]) -> bool {
    a < b
}

/// Go `RemapToPrevious` (cluster.go:121; dead code in Go, ported for its tests): relabels
/// `communities` so their ids stay stable against `prev_node_community`. Greedily matches each new
/// community to the previous one that overlaps it most (one-to-one), then gives fresh ids to the
/// unmatched ones in size-desc, lexical order.
pub fn remap_to_previous(
    communities: &BTreeMap<i64, Vec<String>>,
    prev_node_community: &BTreeMap<String, i64>,
) -> BTreeMap<i64, Vec<String>> {
    if communities.is_empty() {
        return BTreeMap::new();
    }

    let new_sets: BTreeMap<i64, BTreeSet<&str>> = communities
        .iter()
        .map(|(cid, nodes)| (*cid, nodes.iter().map(String::as_str).collect()))
        .collect();
    let mut old_sets: BTreeMap<i64, BTreeSet<&str>> = BTreeMap::new();
    for (node, old_cid) in prev_node_community {
        old_sets.entry(*old_cid).or_default().insert(node);
    }

    struct Overlap {
        count: i64,
        old_cid: i64,
        new_cid: i64,
    }
    let mut overlaps: Vec<Overlap> = Vec::new();
    for (&old_cid, old_nodes) in &old_sets {
        for (&new_cid, new_nodes) in &new_sets {
            let c = new_nodes.iter().filter(|n| old_nodes.contains(*n)).count() as i64;
            if c > 0 {
                overlaps.push(Overlap {
                    count: c,
                    old_cid,
                    new_cid,
                });
            }
        }
    }
    gostd::sort::slice(&mut overlaps, |a, b| {
        if a.count != b.count {
            return a.count > b.count;
        }
        if a.old_cid != b.old_cid {
            return a.old_cid < b.old_cid;
        }
        a.new_cid < b.new_cid
    });

    let mut new_to_final: BTreeMap<i64, i64> = BTreeMap::new();
    let mut used_old_ids: BTreeSet<i64> = BTreeSet::new();
    let mut matched_new_ids: BTreeSet<i64> = BTreeSet::new();
    for o in &overlaps {
        if used_old_ids.contains(&o.old_cid) || matched_new_ids.contains(&o.new_cid) {
            continue;
        }
        new_to_final.insert(o.new_cid, o.old_cid);
        used_old_ids.insert(o.old_cid);
        matched_new_ids.insert(o.new_cid);
    }

    // Go ranges the map `communities` here (random order); ascending id is the deterministic
    // choice. It only matters for ties, i.e. two communities with identical sorted members.
    let mut unmatched: Vec<i64> = communities
        .keys()
        .copied()
        .filter(|cid| !matched_new_ids.contains(cid))
        .collect();
    gostd::sort::slice(&mut unmatched, |&i, &j| {
        let (a, b) = (&communities[&i], &communities[&j]);
        if a.len() != b.len() {
            return a.len() > b.len();
        }
        less(&sorted_copy(a), &sorted_copy(b))
    });
    let mut next_id = 0i64;
    for new_cid in unmatched {
        while used_old_ids.contains(&next_id) {
            next_id += 1;
        }
        new_to_final.insert(new_cid, next_id);
        used_old_ids.insert(next_id);
        next_id += 1;
    }

    let mut remapped = BTreeMap::new();
    for (new_cid, nodes) in communities {
        remapped.insert(new_to_final[new_cid], sorted_copy(nodes));
    }
    remapped
}

/// Go `sortedCopy` (cluster.go:211): a lexically sorted copy of `nodes`, leaving the input intact.
fn sorted_copy(nodes: &[String]) -> Vec<String> {
    let mut out = nodes.to_vec();
    out.sort();
    out
}

/// Go `Cohesion` (cluster.go:219): the ratio of actual intra-community edges to the maximum
/// possible.
pub fn cohesion(g: &Graph, nodes: &[String]) -> f64 {
    let n = nodes.len();
    if n <= 1 {
        return 1.0;
    }
    let in_set: BTreeSet<u32> = nodes.iter().filter_map(|id| g.index(id)).collect();
    let ids = g.node_ids();
    let mut actual = 0i64;
    for id in nodes {
        for &nb in g.neighbor_indexes(id) {
            if in_set.contains(&nb) && *id < ids[nb as usize] {
                actual += 1;
            }
        }
    }
    actual as f64 / (n as f64 * (n - 1) as f64 / 2.0)
}

/// Go `NodeCommunity` (cluster.go:243): inverts communities into node ID -> community ID. Go
/// ranges the map in random order, so a node listed in two communities gets either id; here the
/// highest community id wins (BTreeMap order). `cluster` never produces such a node.
pub fn node_community(c: &BTreeMap<i64, Vec<String>>) -> BTreeMap<String, i64> {
    let mut m = BTreeMap::new();
    for (cid, nodes) in c {
        for n in nodes {
            m.insert(n.clone(), *cid);
        }
    }
    m
}

/// Go `Scores` (cluster.go:254): each community's cohesion.
pub fn scores(g: &Graph, c: &BTreeMap<i64, Vec<String>>) -> BTreeMap<i64, f64> {
    c.iter()
        .map(|(cid, nodes)| (*cid, cohesion(g, nodes)))
        .collect()
}
