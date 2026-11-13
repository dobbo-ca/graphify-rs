//! Port of graphify-go `internal/semantic`. Filled by graphify-rs-ihc.1 (plan Task 19).
#![allow(dead_code, unused_variables)]

mod bedrock;
#[allow(clippy::module_inception)]
mod semantic;

pub use bedrock::BedrockBackend;
pub use semantic::{
    Backend, CONCEPT_TYPE, Cache, Config, Entry, Error, MAX_NOTE_BYTES, Note, RATIONALE_TYPE,
    enrich,
};

#[cfg(test)]
mod tests;
