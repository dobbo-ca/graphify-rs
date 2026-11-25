//! Port of `semantic.go`: the opt-in enrichment stage (`Enrich`, `sanitize`, `collect`).

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io::Write;

use crate::cache::hash_bytes;
use crate::detect::is_sensitive;
use crate::model::{Confidence, Edge, Extraction, File, Loc, Node};

/// Go `ConceptType`: the file_type of semantic concept nodes.
pub const CONCEPT_TYPE: &str = "semantic_concept";

/// Go `RationaleType`: the file_type of rationale nodes a backend may emit.
pub const RATIONALE_TYPE: &str = "rationale";

/// Go `MaxNoteBytes`: notes larger than this are skipped before any backend sees them.
pub const MAX_NOTE_BYTES: usize = 200 * 1024;

/// Go `validRelations` (semantic.go:45): the only edge relations the semantic stage emits. Go's
/// map is only looked up, never iterated.
pub(super) const VALID_RELATIONS: [&str; 3] = [
    "cites",
    "conceptually_related_to",
    "semantically_similar_to",
];

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
        match self {
            Error::NoBackend => f.write_str("semantic: no backend configured"),
            Error::Msg(m) => f.write_str(m),
        }
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

/// Go `Enrich` (semantic.go:98): runs the semantic stage over `notes` and merges the result
/// additively into `base`. Returns the enriched extraction and the cache to persist. `prev` is the
/// previous run's cache (empty for a cold run, Go nil).
///
/// Go takes a `context.Context` and passes it to `Backend.Extract` only; nothing in `Enrich`
/// checks it. It is dropped here (no cancellation in this API). Go returns `(base, prev, err)` on
/// the no-backend error; the `Err` carries only the error.
pub fn enrich(
    cfg: &Config,
    base: Extraction,
    notes: &[Note],
    prev: Cache,
) -> Result<(Extraction, Cache), Error> {
    let Some(backend) = cfg.backend else {
        return Err(Error::NoBackend);
    };

    // IDs that already exist in the deterministic core. Deterministic nodes win on collision, and
    // an emitted edge may only target an id that exists in the core or in a concept node emitted
    // this run.
    let known: HashSet<&str> = base.nodes.iter().map(|n| n.id.as_str()).collect();

    let mut new_cache = Cache::new();
    let mut add_nodes: Vec<Node> = Vec::new();
    let mut add_edges: Vec<Edge> = Vec::new();
    // Concept-node IDs emitted this run, so edges to them resolve and a concept is not appended
    // twice.
    let mut emitted: HashSet<String> = HashSet::new();

    for note in notes {
        if is_sensitive(&note.file) {
            continue;
        }
        if note.content.len() > MAX_NOTE_BYTES {
            continue;
        }

        let hash = hash_bytes(note.content.as_bytes());

        if let Some(e) = prev.get(&note.id)
            && e.hash == hash
        {
            new_cache.insert(note.id.clone(), e.clone());
            collect(
                &mut add_nodes,
                &mut add_edges,
                &mut emitted,
                &known,
                &e.nodes,
                &e.edges,
            );
            continue;
        }

        let (ns, es) = match backend.extract(note) {
            Ok(r) => r,
            Err(err) => {
                // Go ignores the write error of Fprintf(os.Stderr, ...).
                let _ = writeln!(
                    std::io::stderr(),
                    "  semantic: skipped {} ({err})",
                    note.file
                );
                continue;
            }
        };
        let (nodes, edges) = sanitize(note, ns, es);
        collect(
            &mut add_nodes,
            &mut add_edges,
            &mut emitted,
            &known,
            &nodes,
            &edges,
        );
        new_cache.insert(note.id.clone(), Entry { hash, nodes, edges });
    }

    // Go copies base into fresh slices; moving it is the same result (base is owned here).
    let mut out_nodes = base.nodes;
    out_nodes.extend(add_nodes);
    let mut out_edges = base.edges;
    out_edges.extend(add_edges);
    Ok((
        Extraction {
            nodes: out_nodes,
            edges: out_edges,
        },
        new_cache,
    ))
}

/// Go `sanitize` (semantic.go:163): coerces a backend's raw output into safe semantic fragments.
/// Every emitted node is a concept/rationale node (file_type forced so it can never masquerade as
/// a code node); every edge is from the note, carries a valid semantic relation and an
/// INFERRED|AMBIGUOUS confidence. Anything else is dropped. Endpoint existence is enforced later
/// in [`collect`].
fn sanitize(note: &Note, nodes: Vec<Node>, edges: Vec<Edge>) -> (Vec<Node>, Vec<Edge>) {
    let mut out_n = Vec::new();
    for mut n in nodes {
        if n.id.is_empty() {
            continue;
        }
        if n.file_type != RATIONALE_TYPE {
            n.file_type = CONCEPT_TYPE.to_string();
        }
        // Concept/rationale nodes are model inferences, not file-located facts: strip any source
        // provenance a backend invented (computed_name, which can carry the model's rationale
        // text, is kept).
        n.source_file.clear();
        n.source_location.clear();
        out_n.push(n);
    }
    let mut out_e = Vec::new();
    for mut e in edges {
        if !VALID_RELATIONS.contains(&e.relation.as_str()) {
            continue;
        }
        if e.source != note.id {
            // edges originate from the note being read
            continue;
        }
        if e.confidence != Confidence::AMBIGUOUS {
            e.confidence = Confidence::INFERRED;
        }
        // Strip any source-file provenance a backend invented; the type-only flag rides the
        // location bits and is kept.
        let type_only = e.type_only();
        e.source_file = File::default();
        e.source_location = Loc::default();
        e.set_type_only(type_only);
        out_e.push(e);
    }
    (out_n, out_e)
}

/// Go `collect` (semantic.go:204): folds one note's sanitized fragments into the run
/// accumulators. Appends each concept node once (never overwriting a deterministic node), then
/// keeps only edges whose target resolves to a known core node or a concept node emitted this run,
/// dropping danglers so the additive layer cannot introduce a phantom endpoint. Go returns the
/// accumulators and takes an unused `note`; here they are `&mut` and `note` is dropped.
fn collect(
    add_nodes: &mut Vec<Node>,
    add_edges: &mut Vec<Edge>,
    emitted: &mut HashSet<String>,
    known: &HashSet<&str>,
    nodes: &[Node],
    edges: &[Edge],
) {
    for n in nodes {
        if known.contains(n.id.as_str()) || emitted.contains(&n.id) {
            continue; // deterministic node wins; concept already added
        }
        emitted.insert(n.id.clone());
        add_nodes.push(n.clone());
    }
    for e in edges {
        if !known.contains(e.target.as_str()) && !emitted.contains(&e.target) {
            continue; // dangling: target is neither core nor an emitted concept
        }
        add_edges.push(e.clone());
    }
}
