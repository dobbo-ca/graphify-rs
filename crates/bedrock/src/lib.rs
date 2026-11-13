//! Bedrock (mantle) Anthropic Messages client: HTTP/1.1, SigV4, credential chain.
//!
//! Makes graphify-go's one Bedrock call (internal/semantic/bedrock.go:103) with the same request
//! body, auth, credential chain, retries and error text. Plan:
//! docs/superpowers/plans/2026-11-13-bedrock.md.

// Task 1 lands signatures with todo!() bodies and ignored tests; graphify-rs-u2o.8 removes this.
#![allow(dead_code, unused_variables)]

mod client;
mod creds;
mod env;
mod http;
mod json;
mod proxy;
mod sha;
mod sigv4;
mod time;

#[cfg(test)]
mod testutil;

use std::fmt;
use std::time::Duration;

pub use client::Client;
pub use env::Env;

/// Claude Opus 4.8 on Bedrock mantle (graphify-go bedrock.go:17).
pub const MODEL: &str = "anthropic.claude-opus-4-8";
/// graphify-go bedrock.go:106.
pub const MAX_TOKENS: u32 = 1024;
/// graphify-go semantic.go:44 (`MaxNoteBytes`).
pub const MAX_NOTE_BYTES: usize = 200 * 1024;
/// The one tool the model must call (graphify-go bedrock.go:28).
pub const TOOL_NAME: &str = "record_concepts";

/// Settings for one [`Client`].
#[derive(Clone, Debug)]
pub struct Config {
    /// graphify passes "us-east-1" (graphify-go cmd/graphify/semantic.go:71).
    pub region: String,
    /// Environment the client reads (auth, base URL, credentials, proxy).
    pub env: Env,
    /// Retries after the first attempt (anthropic-sdk-go requestconfig.go:173).
    pub max_retries: u32,
    /// Deadline for one attempt, including the wait before the next one (client.go:308).
    pub attempt_timeout: Duration,
    /// Tests only: an extra PEM root to trust (the mock server's CA).
    #[doc(hidden)]
    pub extra_root_pem: Option<Vec<u8>>,
}

impl Config {
    /// The process environment, 2 retries, 600 s per attempt.
    pub fn new(region: &str) -> Config {
        Config {
            region: region.to_string(),
            env: Env::process(),
            max_retries: 2,
            attempt_timeout: Duration::from_secs(600),
            extra_root_pem: None,
        }
    }
}

/// An error from [`Client::new`] or [`Client::record_concepts`]. `Display` copies Go's text.
#[derive(Debug)]
pub enum Error {
    Config(String),
    Credentials(String),
    Transport { url: String, msg: String },
    Timeout { url: String },
    Api(ApiError),
    Decode(String),
}

/// A response with status >= 400 (anthropic-sdk-go internal/apierror).
#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub url: String,
    pub request_id: Option<String>,
    pub body: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-u2o.8")
    }
}

impl std::error::Error for Error {}
