//! Port of `analyze.go`: god nodes, surprising cross-file connections, and file-level import
//! cycles.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use gostd::{filepath, strings};

use crate::model::{Graph, Node, Relation};

/// Go `g.Nodes[id]` on a missing id: the zero `Node`.
static ZERO_NODE: Node = Node {
    id: String::new(),
    label: String::new(),
    file_type: String::new(),
    source_file: String::new(),
    source_location: String::new(),
    computed_name: String::new(),
    attributes: BTreeMap::new(),
};

/// Go `g.Nodes[id]`.
fn node_of<'a>(g: &'a Graph, id: &str) -> &'a Node {
    g.node(id).unwrap_or(&ZERO_NODE)
}

/// Go `GodNode` (`analyze.go:18`): a highly-connected core abstraction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GodNode {
    pub id: String,
    pub label: String,
    pub degree: i64,
}

/// Go `GodNodes` (`analyze.go:32`): the `top_n` most-connected real entities (file hubs, concept
/// nodes and generic JSON keys excluded), optionally without nodes above the degree percentile
/// `exclude_hubs_percentile` (1-100; 0 keeps the historical ranking).
pub fn god_nodes(g: &Graph, top_n: i64, exclude_hubs_percentile: i64) -> Vec<GodNode> {
    let ids = g.node_ids();
    let (mut hub_threshold, mut has_threshold) = (0, false);
    if exclude_hubs_percentile > 0 && !ids.is_empty() {
        let mut degrees: Vec<i64> = ids.iter().map(|id| g.degree(id) as i64).collect();
        degrees.sort();
        // Go int arithmetic wraps; an out-of-range index panics in both languages.
        let mut idx = (degrees.len() as i64).wrapping_mul(exclude_hubs_percentile) / 100;
        if idx > 0 {
            idx -= 1;
        }
        if idx >= degrees.len() as i64 {
            idx = degrees.len() as i64 - 1;
        }
        hub_threshold = degrees[idx as usize];
        has_threshold = true;
    }
    // The first topN of a stable sort by degree, without the sort: a node goes in behind every
    // kept node of its degree or more, so ties stay in id order.
    let top_n = top_n.max(1);
    let mut out: Vec<GodNode> = Vec::new();
    for id in ids {
        let d = g.degree(id) as i64;
        if out.len() as i64 >= top_n && out.last().is_some_and(|l| d <= l.degree) {
            continue;
        }
        if has_threshold && d > hub_threshold {
            continue;
        }
        if is_file_node(g, id) || is_concept_node(g, id) || is_json_key_node(g, id) {
            continue;
        }
        let mut at = out.len();
        while at > 0 && out[at - 1].degree < d {
            at -= 1;
        }
        out.insert(
            at,
            GodNode {
                id: id.clone(),
                label: node_of(g, id).label.clone(),
                degree: d,
            },
        );
        out.truncate(top_n as usize);
    }
    out
}

/// Go `jsonNoiseLabels` (`analyze.go:78`): generic JSON keys (package.json, schemas) that
/// accumulate edges mechanically without naming an abstraction.
fn is_json_noise_label(s: &str) -> bool {
    matches!(
        s,
        "start"
            | "end"
            | "name"
            | "id"
            | "type"
            | "properties"
            | "value"
            | "key"
            | "data"
            | "items"
            | "title"
            | "description"
            | "version"
            | "dependencies"
            | "devdependencies"
            | "peerdependencies"
            | "optionaldependencies"
            | "bundleddependencies"
            | "bundledependencies"
    )
}

/// Go `isJSONKeyNode` (`analyze.go:87`): whether a node is a generic key inside a .json file.
pub(super) fn is_json_key_node(g: &Graph, id: &str) -> bool {
    let n = node_of(g, id);
    if !strings::to_lower(&n.source_file).ends_with(".json") {
        return false;
    }
    is_json_noise_label(&strings::to_lower(strings::trim_space(&n.label)))
}

/// Go `semanticRelations` (`analyze.go:97`): the LLM-inferred edge relations the enrichment stage
/// emits; `surprising` treats them as insights in their own right.
fn is_semantic_relation(s: &str) -> bool {
    matches!(
        s,
        "cites" | "conceptually_related_to" | "semantically_similar_to"
    )
}

/// Go `Surprise` (`analyze.go:104`): a non-obvious cross-file connection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surprise {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    pub source_files: [String; 2],
    pub note: String,
}

/// Go `Surprising` (`analyze.go:115`): up to `top_n` cross-file edges between real entities,
/// ranked by how non-obvious they are. A node with no community is community 0
/// (`analyze.go:147`).
pub fn surprising(g: &Graph, c: &BTreeMap<i64, Vec<String>>, top_n: i64) -> Vec<Surprise> {
    // Go `cluster.NodeCommunity`. Go ranges a map, so a node listed in two communities gets a
    // random one; here the largest community id wins (ascending order, last write).
    let mut nc: BTreeMap<&str, i64> = BTreeMap::new();
    for (cid, nodes) in c {
        for n in nodes {
            nc.insert(n.as_str(), *cid);
        }
    }

    struct Scored {
        s: Surprise,
        score: i64,
    }
    let mut cands: Vec<Scored> = Vec::new();
    for e in g.edges() {
        if e.relation == Relation::IMPORTS
            || e.relation == Relation::IMPORTS_FROM
            || e.relation == Relation::CONTAINS
        {
            continue;
        }
        let (u, v) = (e.source.as_str(), e.target.as_str());
        // A semantic edge (LLM-inferred relation into a concept) is itself the insight: a prose
        // note linking to a concept is meaningful even when the note looks like a file hub (its
        // label is the file basename). So the file-hub exclusion, which suppresses mechanical
        // AST edges, is skipped for semantic edges; the extract-concept exclusion still applies.
        let sem = is_semantic_relation(e.relation.as_str());
        if is_concept_node(g, u) || is_concept_node(g, v) {
            continue;
        }
        if !sem && (is_file_node(g, u) || is_file_node(g, v)) {
            continue;
        }
        let (uf, vf) = (entity_loc(g, u), entity_loc(g, v));
        if uf.is_empty() || vf.is_empty() || uf == vf {
            continue;
        }
        let mut score = match e.confidence.as_str() {
            "AMBIGUOUS" => 3,
            "INFERRED" => 2,
            "EXTRACTED" => 1,
            _ => 0,
        };
        let mut note = "";
        let (cu, cv) = (
            nc.get(u).copied().unwrap_or(0),
            nc.get(v).copied().unwrap_or(0),
        );
        if cu != cv {
            score += 1;
            note = "bridges separate communities";
        }
        cands.push(Scored {
            s: Surprise {
                source: node_of(g, u).label.clone(),
                target: node_of(g, v).label.clone(),
                relation: e.relation.as_str().to_string(),
                confidence: e.confidence.as_str().to_string(),
                source_files: [uf, vf],
                note: note.to_string(),
            },
            score,
        });
    }
    // Go `sort.SliceStable` by score descending (`analyze.go:157`).
    cands.sort_by_key(|c| Reverse(c.score));
    cands
        .into_iter()
        .take(top_n.max(0) as usize)
        .map(|c| c.s)
        .collect()
}

/// Go `Cycle` (`analyze.go:169`): a circular file-level import dependency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cycle {
    pub files: Vec<String>,
}

/// Go `ImportCycles` (`analyze.go:176`): circular dependencies in the file-level import graph
/// built from imports_from edges, at most `max_len` files long, deduplicated by rotation,
/// shortest first, at most `top_n`.
///
/// Go ranges the map `raw` twice (to collect names, then to fill `adj`/`radj`); `raw` is a
/// `BTreeMap` here. The names are sorted and deduplicated and each `adj` row is sorted, so only
/// the order of a `radj` row could differ, and it only feeds the reverse BFS, whose distances
/// do not depend on queue order within a level.
pub fn import_cycles(g: &Graph, max_len: i64, top_n: i64) -> Vec<Cycle> {
    let mut raw: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in g.edges() {
        // Type-only imports are erased at compile time, so they cannot form a runtime cycle
        // (upstream find_import_cycles skips them too). Deferred imports run late.
        if e.relation != Relation::IMPORTS_FROM || e.type_only() || e.deferred() {
            continue;
        }
        let (uf, vf) = (
            node_of(g, &e.source).source_file.as_str(),
            node_of(g, &e.target).source_file.as_str(),
        );
        if uf.is_empty() || vf.is_empty() || uf == vf {
            continue;
        }
        raw.entry(uf).or_default().push(vf);
    }
    // Files become ints in sorted order, so int order is name order.
    let mut names: Vec<&str> = Vec::with_capacity(raw.len());
    for (s, vs) in &raw {
        names.push(*s);
        names.extend(vs);
    }
    names.sort();
    names.dedup();
    let idx: BTreeMap<&str, usize> = names.iter().enumerate().map(|(i, s)| (*s, i)).collect();
    let n = names.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut radj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (s, vs) in &raw {
        let u = idx[s];
        for v in vs {
            adj[u].push(idx[v]);
        }
        adj[u].sort();
        adj[u].dedup();
        for &v in &adj[u] {
            radj[v].push(u);
        }
    }
    let comp = scc(&adj);

    let cap = top_n.wrapping_mul(10);
    // Go `dfs` returns at once when this holds on entry; it is not re-checked inside the loop.
    let stop = |path_len: usize, cycles: usize| path_len as i64 > max_len || cycles as i64 >= cap;
    let mut cycles: Vec<Cycle> = Vec::new();
    let mut visited = vec![false; n];
    let mut dist = vec![0usize; n]; // edges back to start, valid when stamp matches
    let mut stamp = vec![0usize; n];
    let mut queue: Vec<usize> = Vec::new();
    let mut path: Vec<usize> = Vec::new();
    let mut s = 0;
    while s < n && (cycles.len() as i64) < cap {
        let start = s;
        s += 1;
        // Reverse BFS inside the start's component: a cycle never leaves it.
        queue.clear();
        queue.push(start);
        stamp[start] = start + 1;
        dist[start] = 0;
        let mut h = 0;
        while h < queue.len() {
            let v = queue[h];
            h += 1;
            if dist[v] as i64 >= max_len {
                continue;
            }
            for &u in &radj[v] {
                if u < start && comp[u] == comp[start] && stamp[u] != start + 1 {
                    stamp[u] = start + 1;
                    dist[u] = dist[v] + 1;
                    queue.push(u);
                }
            }
        }
        if queue.len() == 1 {
            continue;
        }
        visited[start] = true;
        path.clear();
        path.push(start);
        // Go's recursive `dfs(start)` run on an explicit stack of (node, next neighbour index)
        // frames. A frame is pushed exactly where Go calls `dfs(nb)` and the entry check passes;
        // when a frame is exhausted the parent un-marks it (`path.pop`, `visited = false`) just
        // as Go does after the call returns. Neighbours are scanned in `adj` order, so cycles
        // are found in Go's order.
        let mut frames: Vec<(usize, usize)> = Vec::new();
        if !stop(path.len(), cycles.len()) {
            frames.push((start, 0));
        }
        while let Some(&(cur, i)) = frames.last() {
            if i == adj[cur].len() {
                frames.pop();
                if !frames.is_empty() {
                    path.pop();
                    visited[cur] = false;
                }
                continue;
            }
            if let Some(f) = frames.last_mut() {
                f.1 += 1;
            }
            let nb = adj[cur][i];
            if nb == start {
                if path.len() >= 2 {
                    cycles.push(Cycle {
                        files: path.iter().map(|&p| names[p].to_string()).collect(),
                    });
                }
                continue;
            }
            // The start is the largest file of its cycle, so each rotation is found once. Skip
            // nodes that cannot close the cycle within maxLen.
            if nb > start
                || visited[nb]
                || stamp[nb] != start + 1
                || (path.len() + dist[nb]) as i64 > max_len
            {
                continue;
            }
            visited[nb] = true;
            path.push(nb);
            if stop(path.len(), cycles.len()) {
                path.pop();
                visited[nb] = false;
            } else {
                frames.push((nb, 0));
            }
        }
        visited[start] = false;
    }
    // Go `sort.SliceStable` by length ascending (`analyze.go:277`).
    cycles.sort_by_key(|c| c.files.len());
    if cycles.len() as i64 > top_n {
        // Go `cycles[:topN]` panics for a negative topN, which also leaves `cycles` empty.
        cycles.truncate(top_n.max(0) as usize);
    }
    cycles
}

/// Go `scc` (`analyze.go:286`): Tarjan's algorithm with an explicit stack; it returns a component
/// id per node. Go's version is already iterative, so this is a direct port.
fn scc(adj: &[Vec<usize>]) -> Vec<i64> {
    let n = adj.len();
    let mut comp = vec![-1i64; n];
    let mut low = vec![0usize; n];
    let mut num = vec![0usize; n];
    let mut st: Vec<usize> = Vec::new();
    let (mut cnt, mut nc) = (0usize, 0i64);
    for r in 0..n {
        if num[r] != 0 {
            continue;
        }
        cnt += 1;
        num[r] = cnt;
        low[r] = cnt;
        st.push(r);
        let mut call: Vec<(usize, usize)> = vec![(r, 0)];
        while let Some(&(v, i)) = call.last() {
            if i < adj[v].len() {
                let w = adj[v][i];
                if let Some(f) = call.last_mut() {
                    f.1 += 1;
                }
                if num[w] == 0 {
                    cnt += 1;
                    num[w] = cnt;
                    low[w] = cnt;
                    st.push(w);
                    call.push((w, 0));
                } else if comp[w] == -1 && num[w] < low[v] {
                    low[v] = num[w];
                }
                continue;
            }
            call.pop();
            match call.last() {
                Some(&(p, _)) if low[v] < low[p] => low[p] = low[v],
                _ => {}
            }
            if low[v] == num[v] {
                while let Some(w) = st.pop() {
                    comp[w] = nc;
                    if w == v {
                        break;
                    }
                }
                nc += 1;
            }
        }
    }
    comp
}

/// Go `isFileNode` (`analyze.go:341`).
pub(super) fn is_file_node(g: &Graph, id: &str) -> bool {
    let n = node_of(g, id);
    if n.label.is_empty() {
        return false;
    }
    if !n.source_file.is_empty() && n.label == filepath::base(&n.source_file) {
        return true;
    }
    if n.label.starts_with('.') && n.label.ends_with("()") {
        return true;
    }
    if n.label.ends_with("()") && g.degree(id) <= 1 {
        return true;
    }
    false
}

/// Go `semanticEntityTypes` (`analyze.go:364`): LLM-derived node file_types that, despite having
/// no source file, are meaningful abstractions.
fn is_semantic_entity_type(s: &str) -> bool {
    matches!(s, "semantic_concept" | "rationale")
}

/// Go `entityLoc` (`analyze.go:373`): a "where" key for the surprising-connection cross-file
/// check. For a normal node that is its source file; for a semantic entity (which has none) it is
/// the node id, so a note-to-concept edge is treated as cross-location.
fn entity_loc(g: &Graph, id: &str) -> String {
    let n = node_of(g, id);
    if !n.source_file.is_empty() {
        return n.source_file.clone();
    }
    if is_semantic_entity_type(&n.file_type) {
        return format!("concept:{id}");
    }
    String::new()
}

/// Go `isConceptNode` (`analyze.go:384`).
pub(super) fn is_concept_node(g: &Graph, id: &str) -> bool {
    let n = node_of(g, id);
    if is_semantic_entity_type(&n.file_type) {
        return false;
    }
    let src = n.source_file.as_str();
    if src.is_empty() {
        return true;
    }
    let base = src.rfind('/').map_or(src, |i| &src[i + 1..]);
    !base.contains('.')
}
