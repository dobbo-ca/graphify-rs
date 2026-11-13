//! Port of `bedrock.go`: the Bedrock-backed [`Backend`] and `parseConcepts`.

use super::semantic::{Backend, Error, Note};
use crate::model::{Edge, Node};

/// Go `BedrockBackend`: Anthropic Claude on Amazon Bedrock, over `bedrock::Client::record_concepts`.
pub struct BedrockBackend {
    client: ::bedrock::Client,
}

impl BedrockBackend {
    /// Go `NewBedrockBackend`: `region` defaults to `us-east-1` when empty. Error text
    /// `bedrock: <err>`.
    pub fn new(region: &str) -> Result<BedrockBackend, Error> {
        todo!("graphify-rs-ihc.1")
    }
}

impl Backend for BedrockBackend {
    fn name(&self) -> &str {
        todo!("graphify-rs-ihc.1")
    }

    fn extract(&self, n: &Note) -> Result<(Vec<Node>, Vec<Edge>), Error> {
        todo!("graphify-rs-ihc.1")
    }
}

/// Go `parseConcepts`: turns the record_concepts tool input into concept nodes and the note's
/// edges. Malformed JSON is `bedrock: bad tool output: <err>`.
pub(super) fn parse_concepts(n: &Note, raw: &[u8]) -> Result<(Vec<Node>, Vec<Edge>), Error> {
    todo!("graphify-rs-ihc.1")
}
