//! Port of `build.go`: deterministic edge order, id-normalisation of edge endpoints, dangling
//! and cross-language phantom edges dropped, provenance kept on an id collision.

use std::collections::HashMap;

use crate::idutil::normalize_id;
use crate::langfamily;
use crate::model::{Confidence, Edge, Extraction, Graph, NO_NODE, Node, Relation};

/// Go `graph.Build`.
pub fn build(ext: Extraction) -> Graph {
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    build_parts(vec![ext.nodes], vec![ext.edges], workers)
}

/// Go `graph.BuildParts`: [`build`] for an extraction left in parts (`extract::resolve_parts`);
/// `workers` is Go's GOMAXPROCS (plan R5).
pub fn build_parts(nodes: Vec<Vec<Node>>, parts: Vec<Vec<Edge>>, workers: usize) -> Graph {
    let n_nodes = nodes.iter().map(Vec::len).sum();
    let edges: Vec<Edge> = parts.into_iter().flatten().collect();
    let order = edge_order(&edges, workers);

    let mut g = Graph::new();
    g.reserve(n_nodes);
    for n in nodes.into_iter().flatten() {
        add_node_keeping_provenance(&mut g, n);
    }

    // A normalised form of every node id maps back to the real id, so an edge endpoint that
    // differs only in punctuation or case still connects. A later id wins, as Go's map does.
    let norm_to_id: HashMap<String, String> = g
        .node_ids()
        .iter()
        .map(|id| (normalize_id(id), id.clone()))
        .collect();
    let node = |id: &str| -> Option<&Node> {
        g.node(id).or_else(|| {
            norm_to_id
                .get(&normalize_id(id))
                .and_then(|real| g.node(real))
        })
    };

    let (mut src, mut tgt) = (vec![NO_NODE; edges.len()], vec![0u32; edges.len()]);
    for (i, e) in edges.iter().enumerate() {
        let (Some(s), Some(t)) = (node(&e.source), node(&e.target)) else {
            continue; // a dangling edge to an external or stdlib node: expected
        };
        let cross = || {
            langfamily::cross(&s.source_file, &t.source_file)
                && !langfamily::component(&s.source_file)
                && !langfamily::component(&t.source_file)
        };
        let phantom = match e.relation {
            Relation::CALLS => e.confidence == Confidence::INFERRED && cross(),
            // The same guard for EXTRACTED imports and references: an unresolved `import time`
            // must not bind by bare stem onto a `time.ts` across a language boundary (#1749).
            // A non-code endpoint has no family, so config/manifest/md -> code edges stay.
            Relation::IMPORTS | Relation::IMPORTS_FROM | Relation::REFERENCES => cross(),
            _ => false,
        };
        if phantom {
            continue;
        }
        src[i] = g.index(&s.id).expect("a found node has an index");
        tgt[i] = g.index(&t.id).expect("a found node has an index");
    }
    g.add_edges_at(edges, &order, &src, &tgt);
    g
}

/// Go `edgeOrder`: the indexes of `edges` in the order a stable sort by (source, target,
/// relation text) leaves them. The index is the last key, so the order is total and one sort
/// gives what Go's per-CPU sorts and pairwise merges give.
// ponytail: one thread; Go sorts runs per CPU and merges them (P4, if profiling asks for it).
pub(super) fn edge_order(edges: &[Edge], workers: usize) -> Vec<i32> {
    let _ = workers;
    let mut order: Vec<i32> = (0..edges.len() as i32).collect();
    order.sort_by(|&i, &j| {
        let (a, b) = (&edges[i as usize], &edges[j as usize]);
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.relation.as_str().cmp(b.relation.as_str()))
            .then(i.cmp(&j))
    });
    order
}

/// Go `addNodeKeepingProvenance`: on an id collision keep the record that has provenance (an
/// external import node with no source file must not erase the real file:line node); when both
/// name the same source file, the incoming one takes any attribute it left empty.
fn add_node_keeping_provenance(g: &mut Graph, mut n: Node) {
    let Some(prev) = g.node(&n.id) else {
        g.add_node(n);
        return;
    };
    if n.source_file.is_empty() && !prev.source_file.is_empty() {
        return;
    }
    if n.source_file == prev.source_file {
        for (field, old) in [
            (&mut n.label, &prev.label),
            (&mut n.file_type, &prev.file_type),
            (&mut n.source_location, &prev.source_location),
            (&mut n.computed_name, &prev.computed_name),
        ] {
            if field.is_empty() {
                field.clone_from(old);
            }
        }
    }
    g.add_node(n);
}
