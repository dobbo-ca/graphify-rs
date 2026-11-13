//! Port of `semantic.go`: the opt-in enrichment stage (`Enrich`, `sanitize`, `collect`).

use std::collections::BTreeMap;
use std::fmt;

use crate::model::{Edge, Extraction, Node};

/// Go `ConceptType`: the file_type of semantic concept nodes.
pub const CONCEPT_TYPE: &str = "semantic_concept";

/// Go `RationaleType`: the file_type of rationale nodes a backend may emit.
pub const RATIONALE_TYPE: &str = "rationale";

/// Go `MaxNoteBytes`: notes larger than this are skipped before any backend sees them.
pub const MAX_NOTE_BYTES: usize = 200 * 1024;

/// Go `Note`: one prose document handed to the backend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    pub id: String,
    pub file: String,
    pub content: String,
}

/// Go `error` values of this package. `Display` is Go's text byte for byte (plan R6).
#[derive(Debug, PartialEq)]
pub enum Error {
    /// `semantic: no backend configured`.
    NoBackend,
    /// Any other Go error text, verbatim (`bedrock: ...`, `bedrock invoke: ...`).
    Msg(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-ihc.1")
    }
}

impl std::error::Error for Error {}

/// Go `Backend` interface: an LLM concept-extraction provider.
pub trait Backend {
    /// Go `Name`: identifies the backend (e.g. "bedrock"), for diagnostics.
    fn name(&self) -> &str;
    /// Go `Extract`: nodes and edges inferred from one note. An `Err` is a per-note failure:
    /// that note contributes nothing, but the run continues.
    fn extract(&self, n: &Note) -> Result<(Vec<Node>, Vec<Edge>), Error>;
}

/// Go `Config`. `backend: None` is Go's nil interface.
#[derive(Clone, Copy, Default)]
pub struct Config<'a> {
    pub backend: Option<&'a dyn Backend>,
}

/// Go `Cache`: note id to its content hash and last extracted fragments.
pub type Cache = BTreeMap<String, Entry>;

/// Go `Entry` (JSON keys `hash`, `nodes`, `edges`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub hash: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Go `Enrich`: runs the semantic stage over `notes` and merges the result additively into
/// `base`. Returns the enriched extraction and the cache to persist. `prev` is the previous
/// run's cache (empty for a cold run, Go nil).
pub fn enrich(
    cfg: &Config,
    base: Extraction,
    notes: &[Note],
    prev: Cache,
) -> Result<(Extraction, Cache), Error> {
    todo!("graphify-rs-ihc.1")
}
