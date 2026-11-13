//! The AWS credential chain: env, then a static or `credential_process` profile, then
//! `aws configure export-credentials` (D-006 in parity/DIVERGENCES.md).

pub(crate) mod ini;
pub(crate) mod process;
pub(crate) mod profile;

use std::time::SystemTime;

use crate::env::Env;
use process::PROCESS_TIMEOUT;
use profile::Profile;

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
    #[cfg_attr(not(test), allow(dead_code))]
    Static {
        profile: String,
    },
    Process {
        command: String,
    },
    Cli {
        profile: Option<String>,
    },
}

/// env_config.go:24-30, 88-96, 319: the long names win; both keys are needed.
fn env_credentials(env: &Env) -> Option<Credentials> {
    let id = env
        .get("AWS_ACCESS_KEY_ID")
        .or_else(|| env.get("AWS_ACCESS_KEY"))?;
    let secret = env
        .get("AWS_SECRET_ACCESS_KEY")
        .or_else(|| env.get("AWS_SECRET_KEY"))?;
    Some(Credentials {
        access_key_id: id.to_string(),
        secret_access_key: secret.to_string(),
        session_token: env.get("AWS_SESSION_TOKEN").map(str::to_string),
        expires: None,
    })
}

fn key<'a>(p: &'a Profile, name: &str) -> Option<&'a str> {
    p.keys
        .get(name)
        .map(String::as_str)
        .filter(|v| !v.is_empty())
}

/// D-006: what sends this profile (or the environment) to the AWS CLI, if anything.
fn cli_reason(profile: Option<&Profile>, env: &Env) -> Option<String> {
    if let Some(p) = profile {
        let first = [
            "role_arn",
            "source_profile",
            "credential_source",
            "web_identity_token_file",
            "sso_session",
        ]
        .into_iter()
        .find(|k| key(p, k).is_some())
        .or_else(|| {
            p.keys
                .keys()
                .map(String::as_str)
                .find(|k| k.starts_with("sso_") && key(p, k).is_some())
        });
        if let Some(k) = first {
            return Some(format!("profile \"{}\" uses {k}", p.name));
        }
    }
    env.get("AWS_WEB_IDENTITY_TOKEN_FILE")
        .map(|_| "the environment sets AWS_WEB_IDENTITY_TOKEN_FILE".to_string())
}

/// Runs the chain once (Go fetches eagerly at client construction, awsauth.go:192).
pub(crate) fn resolve(env: &Env) -> Result<(Source, Credentials), String> {
    if let Some(c) = env_credentials(env) {
        return Ok((Source::Env, c));
    }
    let sel = profile::selection(env);
    let profile = profile::load(env)?;
    let needs_cli = cli_reason(profile.as_ref(), env);

    // Go never uses a profile's static keys or process when it also has role or SSO keys
    // (resolve_credentials.go:138-180); those go to the CLI.
    let usable = if needs_cli.is_none() {
        profile.as_ref()
    } else {
        None
    };
    if let Some(p) = usable {
        if let (Some(id), Some(secret)) =
            (key(p, "aws_access_key_id"), key(p, "aws_secret_access_key"))
        {
            let creds = Credentials {
                access_key_id: id.to_string(),
                secret_access_key: secret.to_string(),
                session_token: key(p, "aws_session_token").map(str::to_string),
                expires: None,
            };
            return Ok((
                Source::Static {
                    profile: p.name.clone(),
                },
                creds,
            ));
        }
        if let Some(command) = key(p, "credential_process") {
            let creds = process::run(command, env, PROCESS_TIMEOUT)?;
            let command = command.to_string();
            return Ok((Source::Process { command }, creds));
        }
    }

    if process::find_aws(env).is_none() {
        let middle = match needs_cli {
            Some(why) => format!("{why}, which needs the AWS CLI"),
            None => format!(
                "no static keys or credential_process in profile \"{}\" ({}, {})",
                sel.name, sel.config_path, sel.credentials_path
            ),
        };
        return Err(format!(
            "no AWS credentials found: no AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY in the environment, {middle}, and the AWS CLI (aws) is not on PATH; it is needed for SSO, role and instance credentials. Set AWS_BEARER_TOKEN_BEDROCK, set AWS keys, or install the AWS CLI and run aws sso login"
        ));
    }
    let cli_profile = sel.named.then_some(sel.name);
    let creds = process::cli(cli_profile.as_deref(), env)?;
    Ok((
        Source::Cli {
            profile: cli_profile,
        },
        creds,
    ))
}

/// Fetches credentials again from a source `resolve` chose.
pub(crate) fn fetch(src: &Source, env: &Env) -> Result<Credentials, String> {
    match src {
        // These never expire; asking the chain again gives the same answer.
        Source::Env | Source::Static { .. } => resolve(env).map(|(_, creds)| creds),
        Source::Process { command } => process::run(command, env, PROCESS_TIMEOUT),
        Source::Cli { profile } => process::cli(profile.as_deref(), env),
    }
}
