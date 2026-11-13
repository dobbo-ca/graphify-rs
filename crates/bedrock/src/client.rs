//! The Messages call: request body, headers, Bearer or SigV4 auth, and the retry loop of
//! anthropic-sdk-go v1.51.0 (internal/requestconfig/requestconfig.go).

use std::time::{Duration, SystemTime};

use crate::http::Response;
use crate::{Config, Error};

#[cfg(test)]
mod tests;

/// graphify-go internal/semantic/bedrock.go:61-64, verbatim.
pub(crate) const SYSTEM_PROMPT: &str = "You extract the key concepts an engineering or design note is about, for a knowledge graph that links related notes.
Given one note, identify the 1-6 most central concepts (systems, tools, services, or topics \u{2014} e.g. \"AppConfig\", \"Flux\", \"forge\", \"build environment\", \"CI dependency management\").
Prefer concept names that other notes about the same system would also use, so notes about the same topic converge on the same concept.
Call the record_concepts tool exactly once. Do not invent concepts the note does not discuss.";

/// A Bedrock mantle Messages client.
pub struct Client {
    cfg: Config,
}

impl Client {
    /// Resolves the base URL and auth. With SigV4, fetches credentials now (awsauth.go:192).
    pub fn new(cfg: Config) -> Result<Client, Error> {
        todo!("graphify-rs-u2o.8")
    }

    /// Asks the model for the note's concepts. Returns the raw JSON text of the
    /// `record_concepts` tool input, or None when the response has no such block.
    pub fn record_concepts(
        &self,
        note_file: &str,
        note_content: &str,
    ) -> Result<Option<String>, Error> {
        todo!("graphify-rs-u2o.8")
    }
}

/// The exact request body graphify-go sends (testdata/request_body.json).
pub(crate) fn request_body(note_file: &str, note_content: &str) -> String {
    todo!("graphify-rs-u2o.4")
}

/// The messages URL for a base URL (option/requestoption.go:148-152, requestconfig.go:391).
pub(crate) fn join_base(base: &str) -> Result<String, String> {
    todo!("graphify-rs-u2o.8")
}

/// requestconfig.go:248-273. `None` is a connection error.
pub(crate) fn should_retry(res: Option<&Response>) -> bool {
    todo!("graphify-rs-u2o.8")
}

/// requestconfig.go:275-380. `jitter(n)` returns a value in `0..n` (nanoseconds).
pub(crate) fn retry_delay(
    res: Option<&Response>,
    retry: u32,
    now: SystemTime,
    jitter: &mut impl FnMut(u64) -> u64,
) -> Duration {
    todo!("graphify-rs-u2o.8")
}
