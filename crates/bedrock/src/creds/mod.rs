//! The AWS credential chain: env, then a static or `credential_process` profile, then
//! `aws configure export-credentials` (D-006 in parity/DIVERGENCES.md).

pub(crate) mod ini;
pub(crate) mod process;
pub(crate) mod profile;

use std::time::SystemTime;

use crate::env::Env;

#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    /// None: never expires.
    pub expires: Option<SystemTime>,
}

// Never print secrets.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key_id", &self.access_key_id)
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

/// Where credentials came from; `fetch` asks the same source again after expiry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Env,
    Static { profile: String },
    Process { command: String },
    Cli { profile: Option<String> },
}

/// Runs the chain once (Go fetches eagerly at client construction, awsauth.go:192).
pub(crate) fn resolve(env: &Env) -> Result<(Source, Credentials), String> {
    todo!("graphify-rs-u2o.6")
}

/// Fetches credentials again from a source `resolve` chose.
pub(crate) fn fetch(src: &Source, env: &Env) -> Result<Credentials, String> {
    todo!("graphify-rs-u2o.6")
}
