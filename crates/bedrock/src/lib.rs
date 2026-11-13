//! Bedrock (mantle) Anthropic Messages client: HTTP/1.1, SigV4, credential chain.
//!
//! Makes graphify-go's one Bedrock call (internal/semantic/bedrock.go:103) with the same request
//! body, auth, credential chain, retries and error text. Plan:
//! docs/superpowers/plans/2026-11-13-bedrock.md.

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
    /// A credential refresh failed during a call (awsauth.go:226-229); retried like a
    /// connection error.
    RetrieveCredentials(String),
    Transport {
        url: String,
        msg: String,
    },
    Timeout {
        url: String,
    },
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
        match self {
            Error::Config(m) | Error::Decode(m) => f.write_str(m),
            Error::Credentials(m) => write!(f, "failed to resolve AWS credentials: {m}"),
            Error::RetrieveCredentials(m) => write!(f, "failed to retrieve AWS credentials: {m}"),
            Error::Transport { url, msg } => write!(f, "Post \"{url}\": {msg}"),
            Error::Timeout { .. } => f.write_str("context deadline exceeded"),
            Error::Api(e) => {
                write!(
                    f,
                    "POST \"{}\": {} {}",
                    e.url,
                    e.status,
                    status_text(e.status)
                )?;
                if let Some(id) = &e.request_id {
                    write!(f, " (Request-ID: {id})")?;
                }
                write!(f, " {}", e.body)
            }
        }
    }
}

/// Go `http.StatusText` (net/http/status.go); empty for an unknown code such as 529.
fn status_text(code: u16) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Request Entity Too Large",
        414 => "Request URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _ => "",
    }
}

impl std::error::Error for Error {}
