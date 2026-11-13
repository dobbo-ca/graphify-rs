//! Port of `model.go`: Node, Edge, Extraction and the assembled Graph.

use std::collections::{BTreeMap, HashMap, HashSet};

use gostd::json::{DecodeState, EncodeState, Error as JsonError, Marshal, Unmarshal, write_field};

use super::code::{Confidence, File, LOC_DEFERRED, LOC_FLAGS, LOC_TYPE_ONLY, Loc, Relation};

/// Go `model.Node`. JSON keys: `id`, `label`, `file_type`, `source_file`,
/// `source_location` (omitempty), `computed_name` (omitempty), `attributes` (omitempty).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub file_type: String,
    pub source_file: String,
    pub source_location: String,
    pub computed_name: String,
    /// A block's literal key/value pairs (Terraform only).
    pub attributes: BTreeMap<String, String>,
}

gostd::json_fields!(decode Node, "model.Node", {
    id => "id",
    label => "label",
    file_type => "file_type",
    source_file => "source_file",
    source_location => "source_location" omitempty,
    computed_name => "computed_name" omitempty,
    attributes => "attributes" omitempty,
});

/// Go `model.Edge`. The type-only and deferred flags live in bits 30 and 29 of
/// `source_location`; read them with the methods. JSON keys: `source`, `target`, `relation`,
/// `confidence`, `type_only`, `deferred`, `source_file`, `source_location`, `weight`,
/// `confidence_score` (Go `Edge.MarshalJSON`, `model.go:75`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: Relation,
    pub confidence: Confidence,
    pub source_file: File,
    pub source_location: Loc,
    pub weight: f64,
    pub confidence_score: f64,
}

impl Edge {
    /// Go `Edge.Loc`: the location without the flag bits.
    pub fn loc(&self) -> Loc {
        Loc(self.source_location.0 & !LOC_FLAGS)
    }

    /// Go `Edge.TypeOnly`: an import erased at compile time (TypeScript `import type`).
    pub fn type_only(&self) -> bool {
        self.source_location.0 & LOC_TYPE_ONLY != 0
    }

    /// Go `Edge.SetTypeOnly`.
    pub fn set_type_only(&mut self, on: bool) {
        self.set_flag(LOC_TYPE_ONLY, on);
    }

    /// Go `Edge.Deferred`: an `import()` edge, which cannot form a static import cycle.
    pub fn deferred(&self) -> bool {
        self.source_location.0 & LOC_DEFERRED != 0
    }

    /// Go `Edge.SetDeferred`.
    pub fn set_deferred(&mut self, on: bool) {
        self.set_flag(LOC_DEFERRED, on);
    }

    fn set_flag(&mut self, bit: u32, on: bool) {
        self.source_location.0 &= !bit;
        if on {
            self.source_location.0 |= bit;
        }
    }
}

/// Go `Edge.MarshalJSON`: the flags as `type_only` and `deferred`, the location without them.
impl Marshal for Edge {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), JsonError> {
        let (type_only, deferred, loc) = (self.type_only(), self.deferred(), self.loc());
        let mut first = true;
        write_field(e, &mut first, "source", &self.source, false)?;
        write_field(e, &mut first, "target", &self.target, false)?;
        write_field(e, &mut first, "relation", &self.relation, false)?;
        write_field(e, &mut first, "confidence", &self.confidence, false)?;
        write_field(e, &mut first, "type_only", &type_only, !type_only)?;
        write_field(e, &mut first, "deferred", &deferred, !deferred)?;
        write_field(
            e,
            &mut first,
            "source_file",
            &self.source_file,
            self.source_file.0 == 0,
        )?;
        write_field(e, &mut first, "source_location", &loc, loc.0 == 0)?;
        write_field(e, &mut first, "weight", &self.weight, self.weight == 0.0)?;
        write_field(
            e,
            &mut first,
            "confidence_score",
            &self.confidence_score,
            self.confidence_score == 0.0,
        )?;
        e.write_raw(b"}");
        Ok(())
    }
}

/// Go's struct decoding of `Edge`: any key case, the last duplicate wins, a `null` or an absent
/// key leaves the value. `type_only` and `deferred` set the flag bits (Go `typeOnlyJSON`,
/// `deferredJSON`), and `source_location` keeps them (Go `Loc.UnmarshalText`).
impl Unmarshal for Edge {
    const GO_TYPE: &'static str = "model.Edge";

    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), JsonError> {
        const KEYS: [&str; 10] = [
            "source",
            "target",
            "relation",
            "confidence",
            "type_only",
            "deferred",
            "source_file",
            "source_location",
            "weight",
            "confidence_score",
        ];
        d.read_struct(&KEYS, |d, key| match key {
            "source" => d.value(&mut self.source),
            "target" => d.value(&mut self.target),
            "relation" => d.value(&mut self.relation),
            "confidence" => d.value(&mut self.confidence),
            "type_only" => {
                let mut on = self.type_only();
                d.value(&mut on)?;
                self.set_type_only(on);
                Ok(())
            }
            "deferred" => {
                let mut on = self.deferred();
                d.value(&mut on)?;
                self.set_deferred(on);
                Ok(())
            }
            "source_file" => d.value(&mut self.source_file),
            "source_location" => d.value(&mut self.source_location),
            "weight" => d.value(&mut self.weight),
            _ => d.value(&mut self.confidence_score),
        })
    }
}

/// Go `model.Extraction`: one extractor's output for one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extraction {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

gostd::json_fields!(decode Extraction, "model.Extraction", {
    nodes => "nodes",
    edges => "edges",
});

/// Go `model.NoNode`: in `src`, tells [`Graph::add_edges_at`] to skip an edge.
pub const NO_NODE: u32 = u32::MAX;

/// Go `model.Graph`: the full directed edge list plus an undirected neighbour set. Nodes are
/// looked up by id with [`Graph::node`] and iterated in insertion order with
/// [`Graph::node_ids`].
#[derive(Debug, Default)]
pub struct Graph {
    pub(super) nodes: HashMap<String, Node>,
    pub(super) edges: Vec<Edge>,
    /// Undirected neighbours by node position, no repeats.
    pub(super) adj: Vec<Vec<u32>>,
    /// Node pairs already in `adj`.
    pub(super) pairs: HashSet<u64>,
    /// Edges already stored, by (source position, target position, relation code).
    pub(super) seen: HashSet<(u32, u32, u32)>,
    /// The same for ids that contain NUL, by the joined key of v0.10.1.
    pub(super) seen_s: HashSet<String>,
    /// Node id -> position in `order`.
    pub(super) index: HashMap<String, u32>,
    /// Node insertion order.
    pub(super) order: Vec<String>,
    /// Go `Graph.Unclassified`: per lowercased extension, files the walk saw but no extractor handles.
    pub unclassified: BTreeMap<String, i64>,
    /// Go `Graph.OverBudget`: files whose parse the budget stopped.
    pub over_budget: i64,
}

/// Go `pairKey`.
fn pair_key(a: u32, b: u32) -> u64 {
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    (u64::from(a) << 32) | u64::from(b)
}

impl Graph {
    /// Go `model.New`.
    pub fn new() -> Graph {
        Graph::default()
    }

    /// Go `Graph.Reserve`: room for `n` nodes (fresh node tables, as Go's `make`).
    pub fn reserve(&mut self, n: usize) {
        self.nodes = HashMap::with_capacity(n);
        self.index = HashMap::with_capacity(n);
        self.order = Vec::with_capacity(n);
        self.adj = Vec::with_capacity(n);
    }

    /// Go `Graph.AddNode`: insert or overwrite (last write wins, as `nx.add_node`).
    pub fn add_node(&mut self, n: Node) {
        if !self.nodes.contains_key(&n.id) {
            self.index.insert(n.id.clone(), self.order.len() as u32);
            self.order.push(n.id.clone());
            self.adj.push(Vec::new());
        }
        self.nodes.insert(n.id.clone(), n);
    }

    /// Go `Graph.AddEdge`: a directed edge between existing nodes; exact (source, target,
    /// relation) duplicates are ignored, both directions and other relations kept.
    pub fn add_edge(&mut self, e: Edge) {
        let Some(&src) = self.index.get(&e.source) else {
            return;
        };
        let Some(&tgt) = self.index.get(&e.target) else {
            return;
        };
        if self.store(e, src, tgt).0 {
            self.link(src, tgt);
        }
    }

    /// Go `Graph.AddEdgesAt`: adds `edges[i]` for each `i` of `order`, as `add_edge` would after
    /// setting its endpoints to the ids of the nodes at `src[i]` and `tgt[i]`; `src[i] ==
    /// NO_NODE` skips it. The neighbour lists are linked in `order` first, as Go's goroutine
    /// does; a NUL-key collision relinks them from the kept edges.
    pub fn add_edges_at(&mut self, edges: Vec<Edge>, order: &[i32], src: &[u32], tgt: &[u32]) {
        self.edges.reserve(order.len());
        for &i in order {
            let i = i as usize;
            if src[i] != NO_NODE {
                self.link(src[i], tgt[i]);
            }
        }
        let mut edges: Vec<Option<Edge>> = edges.into_iter().map(Some).collect();
        let mut collided = false;
        for &i in order {
            let i = i as usize;
            if src[i] == NO_NODE {
                continue;
            }
            let Some(mut e) = edges[i].take() else {
                continue;
            };
            let (s, t) = (src[i], tgt[i]);
            if e.source != self.order[s as usize] || e.target != self.order[t as usize] {
                e.source.clone_from(&self.order[s as usize]);
                e.target.clone_from(&self.order[t as usize]);
                let (keep, by_string) = self.store(e, s, t);
                collided |= !keep && by_string;
                continue;
            }
            let (dup, by_string) = self.dup(&e, s, t);
            if !dup {
                self.edges.push(e);
            } else if by_string {
                collided = true;
            }
        }
        if collided {
            self.pairs.clear();
            for a in &mut self.adj {
                a.clear();
            }
            for k in 0..self.edges.len() {
                let (s, t) = (
                    self.index[&self.edges[k].source],
                    self.index[&self.edges[k].target],
                );
                self.link(s, t);
            }
        }
    }

    /// Go `store`: appends `e` unless an edge with its source, target and relation is there;
    /// `.1` reports that the verdict came from the joined key.
    fn store(&mut self, e: Edge, src: u32, tgt: u32) -> (bool, bool) {
        let (dup, by_string) = self.dup(&e, src, tgt);
        if dup {
            return (false, by_string);
        }
        self.edges.push(e);
        (true, false)
    }

    /// Go `dup`: whether an edge with `e`'s source, target and relation is there, recording
    /// `e`'s if not. Ids or a relation that hold NUL use the old joined key, which let such
    /// edges collide.
    fn dup(&mut self, e: &Edge, src: u32, tgt: u32) -> (bool, bool) {
        let rel = e.relation.as_str();
        if e.source.contains('\0') || e.target.contains('\0') || rel.contains('\0') {
            let k = format!("{}\0{}\0{rel}", e.source, e.target);
            return (!self.seen_s.insert(k), true);
        }
        (!self.seen.insert((src, tgt, e.relation.0)), false)
    }

    /// Go `link`: makes `src` and `tgt` neighbours.
    fn link(&mut self, src: u32, tgt: u32) {
        if self.pairs.insert(pair_key(src, tgt)) {
            self.adj[src as usize].push(tgt);
            if src != tgt {
                self.adj[tgt as usize].push(src);
            }
        }
    }

    /// Go `Graph.HasEdge`: whether an undirected edge connects `a` and `b`.
    pub fn has_edge(&self, a: &str, b: &str) -> bool {
        match (self.index.get(a), self.index.get(b)) {
            (Some(&i), Some(&j)) => self.pairs.contains(&pair_key(i, j)),
            _ => false,
        }
    }

    /// Go `Graph.Degree`: the number of distinct neighbours.
    pub fn degree(&self, id: &str) -> usize {
        self.neighbor_indexes(id).len()
    }

    /// Go `Graph.Neighbors`: neighbour ids, sorted.
    pub fn neighbors(&self, id: &str) -> Vec<String> {
        let mut ns: Vec<String> = self
            .neighbor_indexes(id)
            .iter()
            .map(|&n| self.order[n as usize].clone())
            .collect();
        ns.sort();
        ns
    }

    /// Go `Graph.Index`: a node's position in [`Graph::node_ids`].
    pub fn index(&self, id: &str) -> Option<u32> {
        self.index.get(id).copied()
    }

    /// Go `Graph.NeighborIndexes`: neighbour positions, in no particular order.
    pub fn neighbor_indexes(&self, id: &str) -> &[u32] {
        match self.index.get(id) {
            Some(&i) => &self.adj[i as usize],
            None => &[],
        }
    }

    /// Go `Graph.NodeIDs`: all node ids in insertion order.
    pub fn node_ids(&self) -> &[String] {
        &self.order
    }

    /// Go `g.Nodes[id]`.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Go `g.Nodes[id]`, for writing.
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Go `Graph.NumNodes`.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Go `Graph.NumEdges`.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Go `Graph.Edges`: every stored edge in insertion order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
}
