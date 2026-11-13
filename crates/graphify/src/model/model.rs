//! Port of `model.go`: Node, Edge, Extraction and the assembled Graph.

use std::collections::{BTreeMap, HashMap, HashSet};

use gostd::json::{DecodeState, EncodeState, Error as JsonError, Marshal, Unmarshal};

use super::code::{Confidence, File, Loc, Relation};

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
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Edge.TypeOnly`.
    pub fn type_only(&self) -> bool {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Edge.SetTypeOnly`.
    pub fn set_type_only(&mut self, on: bool) {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Edge.Deferred`.
    pub fn deferred(&self) -> bool {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Edge.SetDeferred`.
    pub fn set_deferred(&mut self, on: bool) {
        todo!("graphify-rs-ihc.7")
    }
}

/// Go `model.Extraction`: one extractor's output for one file (`nodes`, `edges`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extraction {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Go `model.NoNode`: in `src`, tells [`Graph::add_edges_at`] to skip an edge.
pub const NO_NODE: u32 = u32::MAX;

/// Go `model.Graph`: the full directed edge list plus an undirected neighbour set. Nodes are
/// looked up by id with [`Graph::node`] and iterated in insertion order with
/// [`Graph::node_ids`].
#[derive(Debug, Default)]
pub struct Graph {
    pub(super) nodes: HashMap<String, Node>,
    pub(super) edges: Vec<Edge>,
    pub(super) adj: Vec<Vec<u32>>,
    pub(super) pairs: HashSet<u64>,
    pub(super) seen: HashSet<(u32, u32, u32)>,
    pub(super) seen_s: HashSet<String>,
    pub(super) index: HashMap<String, u32>,
    pub(super) order: Vec<String>,
    /// Go `Graph.Unclassified`: per lowercased extension, files the walk saw but no extractor handles.
    pub unclassified: BTreeMap<String, i64>,
    /// Go `Graph.OverBudget`: files whose parse the budget stopped.
    pub over_budget: i64,
}

impl Graph {
    /// Go `model.New`.
    pub fn new() -> Graph {
        Graph::default()
    }

    /// Go `Graph.Reserve`: room for `n` nodes.
    pub fn reserve(&mut self, n: usize) {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.AddNode`: insert or overwrite (last write wins).
    pub fn add_node(&mut self, n: Node) {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.AddEdge`: a directed edge between existing nodes; exact (source, target,
    /// relation) duplicates are ignored.
    pub fn add_edge(&mut self, e: Edge) {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.AddEdgesAt`: adds `edges[i]` for each `i` of `order`, between the nodes at
    /// positions `src[i]` and `tgt[i]` (`src[i] == NO_NODE` skips it).
    pub fn add_edges_at(&mut self, edges: Vec<Edge>, order: &[i32], src: &[u32], tgt: &[u32]) {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.HasEdge`: whether an undirected edge connects `a` and `b`.
    pub fn has_edge(&self, a: &str, b: &str) -> bool {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.Degree`: the number of distinct neighbours.
    pub fn degree(&self, id: &str) -> usize {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.Neighbors`: neighbour ids, sorted.
    pub fn neighbors(&self, id: &str) -> Vec<String> {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.Index`: a node's position in [`Graph::node_ids`].
    pub fn index(&self, id: &str) -> Option<u32> {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.NeighborIndexes`: neighbour positions, in no particular order.
    pub fn neighbor_indexes(&self, id: &str) -> &[u32] {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.NodeIDs`: all node ids in insertion order.
    pub fn node_ids(&self) -> &[String] {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `g.Nodes[id]`.
    pub fn node(&self, id: &str) -> Option<&Node> {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `g.Nodes[id]`, for writing.
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.NumNodes`.
    pub fn num_nodes(&self) -> usize {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.NumEdges`.
    pub fn num_edges(&self) -> usize {
        todo!("graphify-rs-ihc.7")
    }

    /// Go `Graph.Edges`: every stored edge in insertion order.
    pub fn edges(&self) -> &[Edge] {
        todo!("graphify-rs-ihc.7")
    }
}

impl Marshal for Node {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), JsonError> {
        todo!("graphify-rs-ihc.7")
    }
}

impl Unmarshal for Node {
    const GO_TYPE: &'static str = "model.Node";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), JsonError> {
        todo!("graphify-rs-ihc.7")
    }
}

impl Marshal for Edge {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), JsonError> {
        todo!("graphify-rs-ihc.7")
    }
}

impl Unmarshal for Edge {
    const GO_TYPE: &'static str = "model.Edge";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), JsonError> {
        todo!("graphify-rs-ihc.7")
    }
}

impl Marshal for Extraction {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), JsonError> {
        todo!("graphify-rs-ihc.7")
    }
}

impl Unmarshal for Extraction {
    const GO_TYPE: &'static str = "model.Extraction";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), JsonError> {
        todo!("graphify-rs-ihc.7")
    }
}
