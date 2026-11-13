//! Port of `export.go`: graph.json, the shrink guard and `norm_label`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use gostd::json::{Error as JsonError, Value};

use super::oldnodes::OldNode;
use crate::model::{Edge, Graph, Node};

/// Go `ErrGraphShrink`: `to_json` would replace a larger graph.json with a smaller one.
pub const ERR_GRAPH_SHRINK: &str = "refusing to overwrite graph.json with a smaller graph";

/// Go `ErrGraphUnverifiable`: the existing graph.json cannot be parsed to compare sizes.
pub const ERR_GRAPH_UNVERIFIABLE: &str = "refusing to overwrite unparseable graph.json";

/// The errors of this module. `Display` is Go's text byte for byte (plan R6); the two guard
/// errors hold the whole wrapped message, which starts with their sentinel text.
#[derive(Debug)]
pub enum Error {
    /// Go `fmt.Errorf("%w: ...", ErrGraphShrink, ...)`.
    GraphShrink(String),
    /// Go `fmt.Errorf("%w %s to verify ...", ErrGraphUnverifiable, ...)`.
    GraphUnverifiable(String),
    Io(io::Error),
    Json(JsonError),
    /// Any other Go `errors.New` or `fmt.Errorf` text (`export.go:137`, `okf.go:278`).
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::GraphShrink(s) | Error::GraphUnverifiable(s) | Error::Other(s) => f.write_str(s),
            Error::Io(e) => write!(f, "{e}"),
            Error::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Go `jsonNode` (`export.go:43`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct JsonNode {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) file_type: String,
    pub(super) source_file: String,
    pub(super) source_location: String,
    pub(super) community: Option<i64>,
    pub(super) norm_label: String,
    pub(super) computed_name: String,
    pub(super) attributes: BTreeMap<String, String>,
}

gostd::json_fields!(decode JsonNode, "export.jsonNode", {
    id => "id",
    label => "label",
    file_type => "file_type",
    source_file => "source_file",
    source_location => "source_location" omitempty,
    community => "community",
    norm_label => "norm_label",
    computed_name => "computed_name" omitempty,
    attributes => "attributes" omitempty,
});

/// Go `jsonLink` (`export.go:55`). `source_location` is the text of Go's `model.Loc`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct JsonLink {
    pub(super) source: String,
    pub(super) target: String,
    pub(super) relation: String,
    pub(super) confidence: String,
    pub(super) deferred: bool,
    pub(super) source_file: String,
    pub(super) source_location: String,
    pub(super) weight: f64,
    pub(super) confidence_score: f64,
}

gostd::json_fields!(decode JsonLink, "export.jsonLink", {
    source => "source",
    target => "target",
    relation => "relation",
    confidence => "confidence",
    deferred => "deferred" omitempty,
    source_file => "source_file" omitempty,
    source_location => "source_location" omitempty,
    weight => "weight" omitempty,
    confidence_score => "confidence_score",
});

/// Go `graphAttrs` (`export.go:70`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct GraphAttrs {
    pub(super) unclassified_files: i64,
    pub(super) unclassified_exts: BTreeMap<String, i64>,
}

gostd::json_fields!(decode GraphAttrs, "export.graphAttrs", {
    unclassified_files => "unclassified_files",
    unclassified_exts => "unclassified_extensions" omitempty,
});

/// Go `jsonGraph` (`export.go:75`). A Go nil slice is `None`, which encodes as `null`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct JsonGraph {
    pub(super) directed: bool,
    pub(super) multigraph: bool,
    pub(super) graph: GraphAttrs,
    pub(super) nodes: Option<Vec<JsonNode>>,
    pub(super) links: Option<Vec<JsonLink>>,
    pub(super) hyperedges: Option<Vec<Value>>,
    pub(super) built_at_commit: String,
}

gostd::json_fields!(decode JsonGraph, "export.jsonGraph", {
    directed => "directed",
    multigraph => "multigraph",
    graph => "graph",
    nodes => "nodes",
    links => "links",
    hyperedges => "hyperedges",
    built_at_commit => "built_at_commit" omitempty,
});

/// Go `jsonChunk` (`export.go:109`): nodes or links one worker encodes at a time.
pub(super) const JSON_CHUNK: usize = 512;

/// Go `ToJSON` (`export.go:94`). `workers` is the encoder fan-out (plan R5).
pub fn to_json(
    g: &Graph,
    c: &BTreeMap<i64, Vec<String>>,
    path: &Path,
    built_at_commit: &str,
    force: bool,
    workers: usize,
) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}

/// Go `writeJSON` (`export.go:119`): streams the document `marshal_indent(JsonGraph, "", "  ")`
/// would produce, with chunks of `chunk` records on `workers` threads.
pub(super) fn write_json(
    w: &mut dyn Write,
    g: &Graph,
    communities: &BTreeMap<i64, Vec<String>>,
    built_at_commit: &str,
    workers: usize,
    chunk: usize,
) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}

/// Go `toJSONNode` (`export.go:286`).
pub(super) fn to_json_node(n: &Node, nc: &BTreeMap<String, i64>) -> JsonNode {
    todo!("graphify-rs-ihc.12")
}

/// Go `toJSONLink` (`export.go:307`).
pub(super) fn to_json_link(e: &Edge) -> JsonLink {
    todo!("graphify-rs-ihc.12")
}

/// Go `CheckShrink` (`export.go:338`).
pub fn check_shrink(
    path: &Path,
    g: &Graph,
    skipped: &BTreeSet<String>,
    root: &str,
    force: bool,
) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}

/// Go `ShrinkGuard` (`export.go:347`): the old graph.json read while the new graph is built.
pub struct ShrinkGuard {
    path: PathBuf,
    nodes: Vec<OldNode>,
    err: Option<JsonError>,
}

impl ShrinkGuard {
    /// Go `ShrinkGuard.Check` (`export.go:365`).
    pub fn check(&self, g: &Graph, skipped: &BTreeSet<String>, root: &str) -> Result<(), Error> {
        todo!("graphify-rs-ihc.12")
    }
}

/// Go `ReadShrinkGuard` (`export.go:355`).
pub fn read_shrink_guard(path: &Path) -> Option<ShrinkGuard> {
    todo!("graphify-rs-ihc.12")
}

/// Go `normLabel` (`export.go:441`): NFKD (x/text, Unicode 15), drop `unicode.Mn` (Unicode 17),
/// `strings.ToLower` (Unicode 17).
pub(super) fn norm_label(label: &str) -> String {
    todo!("graphify-rs-ihc.12")
}
