//! Port of graphify-go `internal/semantic`. Filled by graphify-rs-ihc.1 (plan Task 19).

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
