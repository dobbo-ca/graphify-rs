//! `credential_process` (aws-sdk-go-v2 credentials v1.17.27 processcreds) and the
//! `aws configure export-credentials` fallback.

use std::time::Duration;

use super::Credentials;
use crate::env::Env;

#[cfg(test)]
mod tests;

/// Go processcreds default timeout (provider.go:24).
pub(crate) const PROCESS_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs `sh -c <command>` with `env` as the child environment.
pub(crate) fn run(command: &str, env: &Env, timeout: Duration) -> Result<Credentials, String> {
    todo!("graphify-rs-u2o.6")
}

/// Runs `aws configure export-credentials --format process [--profile <p>]`, with `aws` found
/// through `env`'s `PATH`.
pub(crate) fn cli(profile: Option<&str>, env: &Env) -> Result<Credentials, String> {
    todo!("graphify-rs-u2o.6")
}

/// Parses process output JSON (provider.go:154-221).
pub(crate) fn parse_output(stdout: &str) -> Result<Credentials, String> {
    todo!("graphify-rs-u2o.6")
}
