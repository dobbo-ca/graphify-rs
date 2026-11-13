//! Shared helpers (Go: the test files of package semantic share one namespace).

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::model::{Confidence, Edge, Extraction, Node, Relation};
use crate::semantic::{Backend, Error, Note};

mod bedrock;
mod semantic;

type ResultFn = Box<dyn Fn(&Note) -> (Vec<Node>, Vec<Edge>)>;

/// Go `fakeBackend`: records the notes it is asked to extract from and returns a canned set of
/// nodes/edges per note, so the enrichment plumbing can be tested without a network call.
#[derive(Default)]
struct FakeBackend {
    /// Note IDs passed to `extract`, in order (Go `calls`; Go's `callCnt` is its length).
    calls: RefCell<Vec<String>>,
    result: Option<ResultFn>,
    fail_on: BTreeSet<String>,
}

impl FakeBackend {
    fn with(result: impl Fn(&Note) -> (Vec<Node>, Vec<Edge>) + 'static) -> FakeBackend {
        FakeBackend {
            result: Some(Box::new(result)),
            ..FakeBackend::default()
        }
    }

    fn call_cnt(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl Backend for FakeBackend {
    fn name(&self) -> &str {
        "fake"
    }

    fn extract(&self, n: &Note) -> Result<(Vec<Node>, Vec<Edge>), Error> {
        self.calls.borrow_mut().push(n.id.clone());
        if self.fail_on.contains(&n.id) {
            return Err(Error::Msg("boom".to_string()));
        }
        if let Some(f) = &self.result {
            return Ok(f(n));
        }
        Ok((Vec::new(), Vec::new()))
    }
}

/// Go `model.Node{ID: id, Label: label, FileType: file_type}`.
fn node(id: &str, label: &str, file_type: &str) -> Node {
    Node {
        id: id.to_string(),
        label: label.to_string(),
        file_type: file_type.to_string(),
        ..Node::default()
    }
}

/// Go `model.Edge{Source: source, Target: target, Relation: rel, Confidence: conf}`.
fn edge(source: &str, target: &str, relation: Relation, confidence: Confidence) -> Edge {
    Edge {
        source: source.to_string(),
        target: target.to_string(),
        relation,
        confidence,
        ..Edge::default()
    }
}

/// Go `baseExtraction`: a tiny deterministic core: one note file and one concept it links to,
/// plus an isolated note with no edges.
fn base_extraction() -> Extraction {
    let mut bake = node("notes_appconfig_bake", "AppConfig bake", "document");
    bake.source_file = "learnings/appconfig-bake.md".to_string();
    bake.source_location = "L1".to_string();
    let mut other = node("notes_other", "Other note", "document");
    other.source_file = "learnings/other.md".to_string();
    other.source_location = "L1".to_string();
    Extraction {
        nodes: vec![bake, node("appconfig", "AppConfig", "concept"), other],
        edges: vec![edge(
            "notes_other",
            "appconfig",
            Relation::REFERENCES,
            Confidence::EXTRACTED,
        )],
    }
}

/// Go `notes`.
fn notes() -> Vec<Note> {
    vec![Note {
        id: "notes_appconfig_bake".to_string(),
        file: "learnings/appconfig-bake.md".to_string(),
        content: "AppConfig managed strategies bake for 10 minutes.".to_string(),
    }]
}
