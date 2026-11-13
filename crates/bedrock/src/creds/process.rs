//! `credential_process` (aws-sdk-go-v2 credentials v1.17.27 processcreds) and the
//! `aws configure export-credentials` fallback.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::Credentials;
use crate::env::Env;
use crate::json::{self, Kind, Value};
use crate::time;

#[cfg(test)]
mod tests;

/// Go processcreds default timeout (provider.go:24).
pub(crate) const PROCESS_TIMEOUT: Duration = Duration::from_secs(60);

/// Go `ProviderError.Error()` (provider.go:34-36).
fn provider_error(msg: &str) -> String {
    format!("process provider error: {msg}")
}

/// Why a child did not give output.
enum Fail {
    Start(String),
    /// Exit status text and captured stderr.
    Exit(String, String),
    /// Which wait ran out: the child's exit, or the end of its output.
    Timeout(&'static str),
}

/// Go `ExitError.Error()` text.
fn status_text(s: ExitStatus) -> String {
    match s.code() {
        Some(c) => format!("exit status {c}"),
        None => format!("signal: {}", s.signal().unwrap_or(0)),
    }
}

/// Reads a pipe to the end on its own thread, so a large output cannot fill the pipe and stall
/// the child.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = pipe {
            let _ = r.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// Runs `cmd` with exactly `env` as its environment and returns its stdout. The child is killed
/// at `timeout` (provider.go:208-247). With `capture_stderr` stderr is collected and stdin is
/// closed; otherwise both are inherited (provider.go:87-88, for MFA prompts).
fn exec(
    mut cmd: Command,
    env: &Env,
    timeout: Duration,
    capture_stderr: bool,
) -> Result<Vec<u8>, Fail> {
    cmd.env_clear().envs(env.iter()).stdout(Stdio::piped());
    if capture_stderr {
        cmd.stderr(Stdio::piped()).stdin(Stdio::null());
    } else {
        cmd.stderr(Stdio::inherit()).stdin(Stdio::inherit());
    }
    let mut child = cmd.spawn().map_err(|e| Fail::Start(e.to_string()))?;
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Fail::Start(e.to_string()));
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Fail::Timeout("the process did not exit"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    // A grandchild can keep the pipe open after the child exits; stop waiting at the deadline
    // (plus a second, so output of a child that exits right at the deadline still arrives).
    let end = deadline + Duration::from_secs(1);
    let stdout = out
        .recv_timeout(end.saturating_duration_since(Instant::now()))
        .map_err(|_| Fail::Timeout("its output stayed open after it exited"))?;
    let stderr = err
        .recv_timeout(end.saturating_duration_since(Instant::now()))
        .map_err(|_| Fail::Timeout("its output stayed open after it exited"))?;
    if status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        Err(Fail::Exit(status_text(status), stderr))
    }
}

/// Runs `sh -c <command>` with `env` as the child environment.
pub(crate) fn run(command: &str, env: &Env, timeout: Duration) -> Result<Credentials, String> {
    if command.is_empty() {
        // provider.go:62-66
        return Err(provider_error(
            "failed to prepare command: command must not be empty",
        ));
    }
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    match exec(cmd, env, timeout, false) {
        Ok(out) => parse_output(&String::from_utf8_lossy(&out)),
        // provider.go:230-240 (the killed child's wait error is "signal: killed")
        Err(Fail::Timeout(_)) => Err(provider_error(
            "credential process timed out: signal: killed",
        )),
        Err(Fail::Start(e) | Fail::Exit(e, _)) => {
            Err(provider_error(&format!("error in credential_process: {e}")))
        }
    }
}

/// The first executable file named `aws` in `env`'s `PATH`.
pub(crate) fn find_aws(env: &Env) -> Option<PathBuf> {
    env.get("PATH")?
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| PathBuf::from(dir).join("aws"))
        .find(|p| {
            std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// Runs `aws configure export-credentials --format process [--profile <p>]`, with `aws` found
/// through `env`'s `PATH`.
pub(crate) fn cli(profile: Option<&str>, env: &Env) -> Result<Credentials, String> {
    const FAILED: &str = "no AWS credentials found: aws configure export-credentials failed: ";
    let Some(aws) = find_aws(env) else {
        return Err("no AWS credentials found: the AWS CLI (aws) is not on PATH".to_string());
    };
    let mut cmd = Command::new(aws);
    cmd.args(["configure", "export-credentials", "--format", "process"]);
    if let Some(p) = profile {
        cmd.args(["--profile", p]);
    }
    match exec(cmd, env, PROCESS_TIMEOUT, true) {
        Ok(out) => parse_output(&String::from_utf8_lossy(&out)),
        Err(Fail::Exit(status, stderr)) => Err(format!("{FAILED}{status}: {stderr}")),
        Err(Fail::Start(e)) => Err(format!("{FAILED}{e}")),
        Err(Fail::Timeout(why)) => Err(format!("{FAILED}timed out: {why}")),
    }
}

/// A string field as Go's `encoding/json` fills it: absent or null is "".
fn string_field(root: &Value, key: &str) -> Result<String, String> {
    match root.get_go(key).map(|v| &v.kind) {
        None | Some(Kind::Null) => Ok(String::new()),
        Some(Kind::String(s)) => Ok(s.clone()),
        Some(_) => Err(format!("cannot unmarshal {key} into a string")),
    }
}

/// Parses process output JSON (provider.go:154-221).
pub(crate) fn parse_output(stdout: &str) -> Result<Credentials, String> {
    // Go prints the raw output here; it can hold secrets, so it is left out.
    let bad = |e: String| provider_error(&format!("parse failed of process output: {e}"));
    let root = json::parse(stdout).map_err(bad)?;
    if !matches!(root.kind, Kind::Object(_) | Kind::Null) {
        return Err(bad("cannot unmarshal into a struct".to_string()));
    }
    let version = match root.get_go("Version").map(|v| &v.kind) {
        None | Some(Kind::Null) => 0,
        Some(Kind::Number(n)) => n
            .parse::<i64>()
            .map_err(|_| bad("cannot unmarshal Version into an int".to_string()))?,
        Some(_) => return Err(bad("cannot unmarshal Version into an int".to_string())),
    };
    let access_key_id = string_field(&root, "AccessKeyId").map_err(bad)?;
    let secret_access_key = string_field(&root, "SecretAccessKey").map_err(bad)?;
    let session_token = string_field(&root, "SessionToken").map_err(bad)?;
    // `*time.Time`: null is nil, a string must be RFC 3339.
    let expires = match root.get_go("Expiration").map(|v| &v.kind) {
        None | Some(Kind::Null) => None,
        Some(Kind::String(s)) => {
            Some(time::parse_rfc3339(s).ok_or_else(|| bad("cannot parse Expiration".to_string()))?)
        }
        Some(_) => return Err(bad("cannot unmarshal Expiration into a time".to_string())),
    };

    if version != 1 {
        return Err(provider_error("wrong version in process output (not 1)"));
    }
    if access_key_id.is_empty() {
        return Err(provider_error("missing AccessKeyId in process output"));
    }
    if secret_access_key.is_empty() {
        return Err(provider_error("missing SecretAccessKey in process output"));
    }
    Ok(Credentials {
        access_key_id,
        secret_access_key,
        session_token: (!session_token.is_empty()).then_some(session_token),
        expires,
    })
}
