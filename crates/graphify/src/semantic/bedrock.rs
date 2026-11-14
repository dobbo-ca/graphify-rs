//! Port of `bedrock.go`: the Bedrock-backed [`Backend`] and `parseConcepts`.
//!
//! Go's `bedrockModel`, `conceptTool`, `systemPrompt` and `notePrompt` (with its `MaxNoteBytes`
//! cut) build the request; they live in the `bedrock` crate (`MODEL`, `request_body`), which
//! `record_concepts` calls with the note's path and content.

use std::collections::HashSet;

use super::semantic::{Backend, CONCEPT_TYPE, Error, Note, VALID_RELATIONS};
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Node, rel};

/// Go `ambiguousScore` (bedrock.go:22): the score floor below which a semantic edge is recorded
/// AMBIGUOUS rather than INFERRED.
const AMBIGUOUS_SCORE: f64 = 0.5;

/// Go `concept` (bedrock.go:67): one item the model returns through the record_concepts tool.
#[derive(Default)]
struct Concept {
    name: String,
    relation: String,
    score: f64,
}

gostd::json_fields!(decode Concept, "semantic.concept", {
    name => "name",
    relation => "relation",
    score => "score",
});

/// Go `conceptsPayload` (bedrock.go:73).
#[derive(Default)]
struct ConceptsPayload {
    concepts: Vec<Concept>,
}

gostd::json_fields!(decode ConceptsPayload, "semantic.conceptsPayload", {
    concepts => "concepts",
});

/// Go `BedrockBackend` (bedrock.go:78): Anthropic Claude on Amazon Bedrock, over
/// `bedrock::Client::record_concepts`.
pub struct BedrockBackend {
    client: ::bedrock::Client,
}

impl BedrockBackend {
    /// Go `NewBedrockBackend` (bedrock.go:86): `region` defaults to `us-east-1` when empty. Error
    /// text `bedrock: <err>`. Go takes a `context.Context` for credential resolution; it is
    /// dropped (no cancellation in this API).
    pub fn new(region: &str) -> Result<BedrockBackend, Error> {
        let region = if region.is_empty() {
            "us-east-1"
        } else {
            region
        };
        let client = ::bedrock::Client::new(::bedrock::Config::new(region))
            .map_err(|e| Error::Msg(format!("bedrock: {e}")))?;
        Ok(BedrockBackend { client })
    }
}

impl Backend for BedrockBackend {
    /// Go `Name` (bedrock.go:97).
    fn name(&self) -> &str {
        "bedrock"
    }

    /// Go `Extract` (bedrock.go:102): asks Claude to record the concepts the note is about and
    /// turns the tool call into concept nodes and edges. A note with no usable tool call yields
    /// nothing rather than an error. Go checks `ctx` only inside the SDK call; it is dropped.
    fn extract(&self, n: &Note) -> Result<(Vec<Node>, Vec<Edge>), Error> {
        match self.client.record_concepts(&n.file, &n.content) {
            Err(e) => Err(Error::Msg(format!("bedrock invoke: {e}"))),
            Ok(Some(raw)) => parse_concepts(n, raw.as_bytes()),
            Ok(None) => Ok((Vec::new(), Vec::new())),
        }
    }
}

/// Go `parseConcepts` (bedrock.go:142): converts the record_concepts tool input into concept
/// nodes and the note's edges. Each concept becomes a stable concept node (id via `make_id`, so
/// notes naming the same concept converge on one node) and an edge from the note carrying the
/// relation and score. Unknown relations are dropped; low-score edges are AMBIGUOUS. Malformed
/// JSON is `bedrock: bad tool output: <err>` so the caller skips the note.
pub(super) fn parse_concepts(n: &Note, raw: &[u8]) -> Result<(Vec<Node>, Vec<Edge>), Error> {
    let p: ConceptsPayload = gostd::json::unmarshal(raw)
        .map_err(|e| Error::Msg(format!("bedrock: bad tool output: {e}")))?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for c in p.concepts {
        if c.name.is_empty() || !VALID_RELATIONS.contains(&c.relation.as_str()) {
            continue;
        }
        let id = make_id(&["concept", &c.name]);
        if id.is_empty() {
            continue;
        }
        if seen.insert(id.clone()) {
            nodes.push(Node {
                id: id.clone(),
                label: c.name.clone(),
                file_type: CONCEPT_TYPE.to_string(),
                ..Node::default()
            });
        }
        let confidence = if c.score < AMBIGUOUS_SCORE {
            Confidence::AMBIGUOUS
        } else {
            Confidence::INFERRED
        };
        edges.push(Edge {
            source: n.id.clone(),
            target: id,
            relation: rel(&c.relation),
            confidence,
            confidence_score: c.score,
            ..Edge::default()
        });
    }
    Ok((nodes, edges))
}
