use super::*;
use crate::testutil::{testdata, tmp_dir};
use std::time::{Duration, SystemTime};

/// Go's tests run `cat testdata/<f>` from the package dir; here the path is absolute.
fn cat(fixture: &str) -> String {
    format!(
        "cat {}",
        testdata(&format!("processcreds/{fixture}")).display()
    )
}

fn env() -> Env {
    Env::from_pairs([("PATH", "/usr/bin:/bin")])
}

fn run_default(command: &str) -> Result<Credentials, String> {
    run(command, &env(), PROCESS_TIMEOUT)
}

/// Go ProviderError.Error() prefix (provider.go:34-36).
const PREFIX: &str = "process provider error: ";

fn assert_provider_error(err: &str, want: &str, case: &str) {
    assert!(
        err.starts_with(PREFIX),
        "{case}: expected prefix {PREFIX:?}, got {err}"
    );
    assert!(err.contains(want), "{case}: expected {want}, got {err}");
}

fn rfc3339(t: SystemTime) -> String {
    let u = crate::time::Utc::from_system(t);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        u.year, u.month, u.day, u.hour, u.min, u.sec
    )
}

/// Writes process JSON (Go `credentialTest`) with the given expiry into a fresh dir and returns
/// the `cat` command for it.
fn expiring_command(tag: &str, expiration: SystemTime) -> String {
    let dir = tmp_dir(tag);
    let path = dir.join(format!("tmp_{tag}"));
    let json = format!(
        "{{\"Version\":1,\"AccessKeyId\":\"accesskey\",\"SecretAccessKey\":\"secretkey\",\"Expiration\":\"{}\"}}",
        rfc3339(expiration)
    );
    std::fs::write(&path, json).unwrap();
    format!("cat {}", path.display())
}

fn expired(c: &Credentials) -> bool {
    c.expires.is_some_and(|e| e < SystemTime::now())
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:19 TestProviderBadCommand
#[test]
fn provider_bad_command() {
    let err = run_default("/bad/process").unwrap_err();
    assert_provider_error(&err, "error in credential_process", "bad command");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:31 TestProviderMoreEmptyCommands
#[test]
fn provider_more_empty_commands() {
    let err = run_default("").unwrap_err();
    assert_provider_error(&err, "failed to prepare command", "empty command");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:43 TestProviderExpectErrors
#[test]
fn provider_expect_errors() {
    // (Go step, fixture, expected text)
    const CASES: [(&str, &str, &str); 4] = [
        (
            "malformed",
            "malformed.json",
            "parse failed of process output",
        ),
        (
            "wrongversion",
            "wrongversion.json",
            "wrong version in process output",
        ),
        ("missingkey", "missingkey.json", "missing AccessKeyId"),
        (
            "missingsecret",
            "missingsecret.json",
            "missing SecretAccessKey",
        ),
    ];
    for (case, fixture, want) in CASES {
        let err = run_default(&cat(fixture)).unwrap_err();
        assert_provider_error(&err, want, case);
    }
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:101 TestProviderTimeout
#[test]
fn provider_timeout() {
    let err = run("/bin/sleep 2", &env(), Duration::from_secs(1)).unwrap_err();
    assert_provider_error(&err, "credential process timed out", "timeout");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:121 TestProviderWithLongSessionToken
#[test]
fn provider_with_long_session_token() {
    let v = run_default(&cat("longsessiontoken.json")).unwrap();
    // Text string same length as session token returned by AWS for AssumeRoleWithWebIdentity.
    let want = "X".repeat(560);
    assert_eq!(v.session_token.as_deref(), Some(want.as_str()));
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:148 TestProviderStatic
#[test]
fn provider_static() {
    let v = run_default(&cat("static.json")).unwrap();
    assert_eq!(v.access_key_id, "accesskey");
    assert_eq!(v.secret_access_key, "secretkey");
    assert!(v.expires.is_none(), "static credentials can not expire");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:166 TestProviderNotExpired
#[test]
fn provider_not_expired() {
    let soon = SystemTime::now() + Duration::from_secs(3600);
    let v = run_default(&expiring_command("not_expired", soon)).unwrap();
    assert!(v.expires.is_some(), "expiring credentials carry an expiry");
    assert!(!expired(&v), "expected not expired, got expired");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:204 TestProviderExpired
#[test]
fn provider_expired() {
    let past = SystemTime::now() - Duration::from_secs(3600);
    let v = run_default(&expiring_command("expired", past)).unwrap();
    assert!(expired(&v), "expected expired, got not expired");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:242 TestProviderForceExpire
#[test]
fn provider_force_expire() {
    let soon = SystemTime::now() + Duration::from_secs(3600);
    let command = expiring_command("force_expire", soon);
    let v = run_default(&command).unwrap();
    assert!(!expired(&v), "expected not expired, got expired");
    // Re-retrieve credentials.
    let v = run_default(&command).unwrap();
    assert!(
        !expired(&v),
        "expected not expired on second retrieve, got expired"
    );
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:292 TestProviderAltConstruct
// ADAPT: no NewProviderCommand or command builder here; `run` takes the command string and a 1 s timeout.
#[test]
fn provider_alt_construct() {
    let v = run(
        &cat("static.json"),
        &env(),
        crate::testutil::scaled(Duration::from_secs(1)),
    )
    .unwrap();
    assert!(v.expires.is_none(), "static credentials can not expire");
}

// Port of credentials@v1.17.27/processcreds/provider_test.go:334 TestProviderWithAccountID
// ADAPT: Credentials has no account id field; the fixture's AccountId must be accepted and ignored.
#[test]
fn provider_with_account_id() {
    let v = run_default(&cat("accountid.json")).unwrap();
    assert_eq!(v.access_key_id, "accesskey");
    assert_eq!(v.secret_access_key, "secretkey");
}
