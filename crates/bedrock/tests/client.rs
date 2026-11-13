//! Public-API tests of `Client` against the in-process mock server (plan Task 7, bead
//! graphify-rs-u2o.8). Ports are from anthropic-sdk-go v1.51.0.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bedrock::{Client, Config, Env, Error, MAX_NOTE_BYTES};
use common::{MockReply, MockServer, RecordedRequest};

const AKID: &str = "AKIDEXAMPLE";
const SECRET: &str = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
const TOKEN: &str = "test-session-token";
const KEY: &[(&str, &str)] = &[("AWS_BEARER_TOKEN_BEDROCK", "test-bearer-token")];
const SIGV4: &[(&str, &str)] = &[
    ("AWS_ACCESS_KEY_ID", AKID),
    ("AWS_SECRET_ACCESS_KEY", SECRET),
    ("AWS_SESSION_TOKEN", TOKEN),
];
/// The raw `input` text of the tool_use block in `tool_response()`.
const TOOL_INPUT: &str = r#"{"concepts":[{"name":"Flux","relation":"cites","score":0.9}]}"#;
const NOTE_FILE: &str = "docs/a.md";
const NOTE_CONTENT: &str = "hello <world> & \"q\"\n";
const JSON: (&str, &str) = ("content-type", "application/json");
/// Makes retried replies come back at once.
const FAST: (&str, &str) = ("Retry-After-Ms", "1");

fn path_str(p: &Path) -> String {
    p.to_str().expect("utf-8 temp path").to_owned()
}

/// A tool_use response for `record_concepts`.
fn tool_response() -> String {
    format!(
        r#"{{"id":"msg_test","type":"message","role":"assistant","content":[{{"type":"text","text":"hi"}},{{"type":"tool_use","id":"toolu_1","name":"record_concepts","input":{TOOL_INPUT}}}],"stop_reason":"tool_use"}}"#
    )
}

fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> MockReply {
    MockReply {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: body.as_bytes().to_vec(),
        ..MockReply::default()
    }
}

fn ok() -> MockReply {
    MockReply::json(200, &tool_response())
}

fn closed() -> MockReply {
    MockReply {
        close_without_reply: true,
        ..MockReply::default()
    }
}

/// Env with HOME and both AWS files in a fresh temp dir (so the real ~/.aws is never read),
/// the mock base URL when given, then `extra` (which wins).
fn env(base: Option<&str>, extra: &[(&str, &str)]) -> Env {
    let home = common::tmp_dir("home");
    let home_s = path_str(&home);
    let config = path_str(&home.join("config"));
    let credentials = path_str(&home.join("credentials"));
    let mut pairs: Vec<(&str, &str)> = vec![
        ("HOME", home_s.as_str()),
        ("AWS_CONFIG_FILE", config.as_str()),
        ("AWS_SHARED_CREDENTIALS_FILE", credentials.as_str()),
    ];
    if let Some(b) = base {
        pairs.push(("ANTHROPIC_BEDROCK_MANTLE_BASE_URL", b));
    }
    pairs.extend_from_slice(extra);
    Env::from_pairs(pairs)
}

fn cfg(env: Env) -> Config {
    Config {
        region: "us-east-1".into(),
        env,
        max_retries: 2,
        attempt_timeout: Duration::from_secs(600),
        extra_root_pem: None,
    }
}

fn setup_cfg(
    script: Vec<MockReply>,
    extra: &[(&str, &str)],
    max_retries: u32,
    attempt_timeout: Duration,
) -> (MockServer, Client) {
    let server = MockServer::start(false, script);
    let base = format!("{}/anthropic", server.url());
    let mut c = cfg(env(Some(&base), extra));
    c.max_retries = max_retries;
    c.attempt_timeout = attempt_timeout;
    let client = Client::new(c).expect("Client::new");
    (server, client)
}

/// 2 retries, 600 s per attempt: the production settings.
fn setup(script: Vec<MockReply>, extra: &[(&str, &str)]) -> (MockServer, Client) {
    setup_cfg(script, extra, 2, Duration::from_secs(600))
}

fn call(client: &Client) -> Result<Option<String>, Error> {
    client.record_concepts(NOTE_FILE, NOTE_CONTENT)
}

fn new_err(cfg: Config) -> Error {
    match Client::new(cfg) {
        Ok(_) => panic!("expected Client::new to fail"),
        Err(e) => e,
    }
}

fn messages_url(server: &MockServer) -> String {
    format!("{}/anthropic/v1/messages", server.url())
}

fn only_request(server: &MockServer) -> RecordedRequest {
    let mut reqs = server.requests();
    assert_eq!(reqs.len(), 1, "expected exactly one request");
    reqs.remove(0)
}

fn retry_counts(server: &MockServer) -> Vec<String> {
    server
        .requests()
        .iter()
        .map(|r| {
            r.header("X-Stainless-Retry-Count")
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

fn signed_headers(auth: &str) -> &str {
    let rest = auth
        .split("SignedHeaders=")
        .nth(1)
        .expect("SignedHeaders= in Authorization");
    rest.split(',').next().expect("SignedHeaders value")
}

fn signature(auth: &str) -> &str {
    auth.split("Signature=")
        .nth(1)
        .expect("Signature= in Authorization")
}

/// Today's UTC date as `YYYYMMDD` (Howard Hinnant's civil_from_days).
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let z = secs.div_euclid(86400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}")
}

// ---- client_test.go ----

// Port of client_test.go:59 TestRetryAfter (anthropic-sdk-go v1.51.0)
#[test]
fn retry_after() {
    let script: Vec<MockReply> = (0..3)
        .map(|_| reply(429, &[("Retry-After", "0.1")], ""))
        .collect();
    let (server, client) = setup(script, KEY);
    assert!(
        call(&client).is_err(),
        "expected an error after the retries"
    );
    assert_eq!(server.requests().len(), 3, "expected 3 attempts");
    assert_eq!(retry_counts(&server), ["0", "1", "2"]);
}

// Port of client_test.go:186 TestRetryAfterMs (anthropic-sdk-go v1.51.0)
#[test]
fn retry_after_ms() {
    let script: Vec<MockReply> = (0..3)
        .map(|_| reply(429, &[("Retry-After-Ms", "100")], ""))
        .collect();
    let (server, client) = setup(script, KEY);
    assert!(
        call(&client).is_err(),
        "expected an error after the retries"
    );
    assert_eq!(server.requests().len(), 3, "expected 3 attempts");
}

// Port of client_test.go:286 TestContextDeadline (anthropic-sdk-go v1.51.0), adapted: the
// deadline is `attempt_timeout` (100 ms), the mock never answers in time, and the error is
// `Error::Timeout` with Go's text. Tolerance is 1.5 s, not 30 ms, for a loaded CI runner.
#[test]
fn context_deadline() {
    let slow = MockReply {
        delay: Duration::from_secs(2),
        ..ok()
    };
    let (server, client) = setup_cfg(vec![slow], KEY, 2, Duration::from_millis(100));
    let start = Instant::now();
    let err = call(&client).expect_err("expected a deadline error");
    let took = start.elapsed();
    let Error::Timeout { url } = &err else {
        panic!("expected Error::Timeout, got {err:?}");
    };
    assert_eq!(url, &messages_url(&server));
    // Go returns the bare ctx.Err() (requestconfig.go:451/:477), with no "Post" prefix.
    assert_eq!(err.to_string(), "context deadline exceeded");
    assert!(
        took >= Duration::from_millis(90) && took < common::scaled(Duration::from_millis(1500)),
        "client returned after {took:?}, expected about 100 ms"
    );
}

// ---- internal/requestconfig/requestconfig_test.go ----
// The Go tests build a RequestConfig by hand; here the error comes out of record_concepts.
// max_retries is 0 so the 500 in the third test is not retried (the Go test uses a bare
// RequestConfig, whose retries default to 0 as well).

// Port of internal/requestconfig/requestconfig_test.go:24 TestErrorWithRequestID
// (anthropic-sdk-go v1.51.0), adapted to bedrock::Error::Api
#[test]
fn error_with_request_id() {
    let body = r#"{"error":{"type":"invalid_request_error","message":"Invalid request"}}"#;
    let (_server, client) = setup_cfg(
        vec![reply(400, &[("request-id", "req_123456789")], body)],
        KEY,
        0,
        Duration::from_secs(600),
    );
    let err = call(&client).expect_err("expected an error");
    let Error::Api(api) = &err else {
        panic!("expected Error::Api, got {err:?}");
    };
    assert_eq!(api.request_id.as_deref(), Some("req_123456789"));
    assert!(
        err.to_string().contains("Request-ID: req_123456789"),
        "error message should contain the request id: {err}"
    );
}

// Port of internal/requestconfig/requestconfig_test.go:69 TestErrorType (anthropic-sdk-go
// v1.51.0), adapted: Rust has no `Type()` accessor, so the error type is checked in the body.
#[test]
fn error_type() {
    let body =
        r#"{"type":"error","error":{"type":"invalid_request_error","message":"Bad request"}}"#;
    let (_server, client) = setup_cfg(
        vec![reply(400, &[("request-id", "req_test123")], body)],
        KEY,
        0,
        Duration::from_secs(600),
    );
    let err = call(&client).expect_err("expected an error");
    let Error::Api(api) = &err else {
        panic!("expected Error::Api, got {err:?}");
    };
    assert!(
        api.body.contains(r#""type":"invalid_request_error""#),
        "body: {}",
        api.body
    );
    assert_eq!(api.status, 400);
    assert_eq!(api.request_id.as_deref(), Some("req_test123"));
}

// Port of internal/requestconfig/requestconfig_test.go:97 TestErrorTypeAbsent
// (anthropic-sdk-go v1.51.0), adapted: no error type in the body, and no request id header.
#[test]
fn error_type_absent() {
    let body = r#"{"message":"Internal error"}"#;
    let (_server, client) = setup_cfg(
        vec![reply(500, &[], body)],
        KEY,
        0,
        Duration::from_secs(600),
    );
    let err = call(&client).expect_err("expected an error");
    let Error::Api(api) = &err else {
        panic!("expected Error::Api, got {err:?}");
    };
    assert!(!api.body.contains(r#""type""#), "body: {}", api.body);
    assert_eq!(api.status, 500);
    assert_eq!(api.request_id, None);
    assert_eq!(api.body, body);
}

// ---- bedrock/bedrockmantle_test.go ----

// Port of bedrock/bedrockmantle_test.go:94 TestMantleRequiresBaseURLOrRegion
// (anthropic-sdk-go v1.51.0)
#[test]
fn mantle_requires_base_url_or_region() {
    let mut c = cfg(env(None, &[("AWS_BEARER_TOKEN_BEDROCK", "my-key")]));
    c.region = String::new();
    let err = new_err(c);
    assert!(matches!(err, Error::Config(_)), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "no base URL found; set BaseURL or AWSRegion in ClientConfig, or set the \
         ANTHROPIC_BEDROCK_MANTLE_BASE_URL or AWS_REGION environment variable"
    );
}

// Port of bedrock/bedrockmantle_test.go:106 TestMantleRegionRequiredForSigV4
// (anthropic-sdk-go v1.51.0). No base URL is set; the region check comes first (awsauth.go:135).
#[test]
fn mantle_region_required_for_sigv4() {
    let mut c = cfg(env(None, SIGV4));
    c.region = String::new();
    let err = new_err(c);
    assert!(matches!(err, Error::Config(_)), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "no AWS region found; set AWSRegion in ClientConfig or set the AWS_REGION environment variable"
    );
}

// Port of bedrock/bedrockmantle_test.go:122 TestMantleAPIKeyModeHeaders (anthropic-sdk-go
// v1.51.0)
#[test]
fn mantle_api_key_mode_headers() {
    let (server, client) = setup(vec![ok()], &[("AWS_BEARER_TOKEN_BEDROCK", "my-api-key")]);
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.header("Authorization"), Some("Bearer my-api-key"));
    assert_eq!(
        req.header("X-Api-Key"),
        None,
        "no X-Api-Key header in bearer auth mode"
    );
}

// Port of bedrock/bedrockmantle_test.go:142 TestMantleSigV4ModeHeaders (anthropic-sdk-go
// v1.51.0), adapted: keys come from the AWS env vars (AKIDEXAMPLE doc keys).
#[test]
fn mantle_sigv4_mode_headers() {
    let (server, client) = setup(vec![ok()], SIGV4);
    call(&client).expect("request failed");
    let req = only_request(&server);
    let auth = req
        .header("Authorization")
        .expect("Authorization header in SigV4 mode");
    assert!(
        auth.starts_with("AWS4-HMAC-SHA256"),
        "expected AWS4 signature, got: {auth}"
    );
    assert!(
        auth.contains("bedrock-mantle"),
        "expected the service name in: {auth}"
    );
    assert!(
        req.header("X-Amz-Date").is_some(),
        "expected X-Amz-Date in SigV4 mode"
    );
}

// Port of bedrock/bedrockmantle_test.go:171 TestMantleSigV4ServiceName (anthropic-sdk-go
// v1.51.0), adapted: Go calls the middleware directly, here the scope is read off the wire.
#[test]
fn mantle_sigv4_service_name() {
    let (server, client) = setup(vec![ok()], SIGV4);
    call(&client).expect("request failed");
    let req = only_request(&server);
    let auth = req.header("Authorization").expect("Authorization header");
    assert!(
        auth.contains("/bedrock-mantle/"),
        "expected Authorization to contain the service, got: {auth}"
    );
}

// Port of bedrock/bedrockmantle_test.go:201 TestMantleAPIKeyFallbackToAWSEnv
// (anthropic-sdk-go v1.51.0), adapted to Env
#[test]
fn mantle_api_key_fallback_to_aws_env() {
    let (server, client) = setup(vec![ok()], &[("ANTHROPIC_AWS_API_KEY", "aws-fallback-key")]);
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.header("Authorization"), Some("Bearer aws-fallback-key"));
}

// Port of bedrock/bedrockmantle_test.go:214 TestMantleAPIKeyMantleEnvOverridesAWSEnv
// (anthropic-sdk-go v1.51.0), adapted to Env
#[test]
fn mantle_api_key_mantle_env_overrides_aws_env() {
    let (server, client) = setup(
        vec![ok()],
        &[
            ("AWS_BEARER_TOKEN_BEDROCK", "mantle-key"),
            ("ANTHROPIC_AWS_API_KEY", "aws-key"),
        ],
    );
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.header("Authorization"), Some("Bearer mantle-key"));
}

// Port of bedrock/bedrockmantle_test.go:229 TestMantleBaseURLDerivedFromRegion
// (anthropic-sdk-go v1.51.0): the region-derived URL has no mock, so it is checked through
// `client::join_base` in src/client/tests.rs (mantle_base_url_derived_from_region).

// Port of bedrock/bedrockmantle_test.go:247 TestMantleBaseURLFromEnv (anthropic-sdk-go
// v1.51.0), adapted: the env base URL is the mock, and the request lands on it. No region is
// set (Go's config is empty too); a bearer key needs none.
#[test]
fn mantle_base_url_from_env() {
    let server = MockServer::start(false, vec![ok()]);
    let base = format!("{}/anthropic", server.url());
    let mut c = cfg(env(Some(&base), KEY));
    c.region = String::new();
    let client = Client::new(c).expect("Client::new");
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/anthropic/v1/messages");
}

// Port of bedrock/bedrockmantle_test.go:261 TestMantleBaseURLExplicitOverridesRegion
// (anthropic-sdk-go v1.51.0): our only explicit base is the env var. A region is set too
// (us-east-1) and the request still goes to the mock, not to bedrock-mantle.us-east-1.api.aws.
#[test]
fn mantle_base_url_explicit_overrides_region() {
    let (server, client) = setup(vec![ok()], KEY);
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.path, "/anthropic/v1/messages");
}

// Port of bedrock/bedrockmantle_test.go:313 TestNewMantleClientMessages (anthropic-sdk-go
// v1.51.0)
#[test]
fn new_mantle_client_messages() {
    let (server, client) = setup(vec![ok()], &[("AWS_BEARER_TOKEN_BEDROCK", "test-key")]);
    let out = call(&client).expect("request failed");
    assert_eq!(out.as_deref(), Some(TOOL_INPUT));
    let req = only_request(&server);
    assert_eq!(req.header("Authorization"), Some("Bearer test-key"));
}

// Port of bedrock/bedrockmantle_test.go:419 TestMantleUserMiddlewareRunsBeforeSigV4Signing
// (anthropic-sdk-go v1.51.0), adapted: there is no user middleware; what is kept is that the
// signature covers the request as sent. `content-length` is signed, equals the length of the
// body on the wire, and the body is the golden request body.
#[test]
fn mantle_user_middleware_runs_before_sigv4_signing() {
    let (server, client) = setup(vec![ok()], SIGV4);
    call(&client).expect("request failed");
    let req = only_request(&server);
    let auth = req.header("Authorization").expect("Authorization header");
    assert!(auth.starts_with("AWS4-HMAC-SHA256"), "got: {auth}");
    assert!(
        signed_headers(auth)
            .split(';')
            .any(|h| h == "content-length"),
        "content-length is not signed: {auth}"
    );
    let len = req.body.len().to_string();
    assert_eq!(req.header("Content-Length"), Some(len.as_str()));
    let golden =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/request_body.json"))
            .expect("read testdata/request_body.json");
    assert_eq!(req.body, golden, "wire body differs from the golden body");
}

// Port of bedrock/bedrockmantle_test.go:514 TestMantleDoesNotLeakAnthropicAPIKey
// (anthropic-sdk-go v1.51.0)
#[test]
fn mantle_does_not_leak_anthropic_api_key() {
    let mut extra = SIGV4.to_vec();
    extra.push(("ANTHROPIC_API_KEY", "should-not-appear"));
    let (server, client) = setup(vec![ok()], &extra);
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(
        req.header("X-Api-Key"),
        None,
        "no X-Api-Key header when using SigV4"
    );
    let auth = req.header("Authorization").unwrap_or("");
    assert!(
        auth.starts_with("AWS4-HMAC-SHA256"),
        "expected SigV4 Authorization, got {auth:?}"
    );
    for (name, value) in &req.headers {
        assert!(
            !value.contains("should-not-appear"),
            "ANTHROPIC_API_KEY leaked in header {name}"
        );
    }
}

// ---- message_test.go ----

// Port of message_test.go:547 TestCalculateNonStreamingTimeout (anthropic-sdk-go v1.51.0),
// adapted: max_tokens is always 1024, so the timeout is the 10 minute default; the
// `Config::new` half is in src/client/tests.rs (calculate_non_streaming_timeout).
#[test]
fn non_streaming_timeout_header() {
    let (server, client) = setup(vec![ok()], KEY);
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.header("X-Stainless-Timeout"), Some("600"));
}

// ---- new tests ----

#[test]
fn request_headers_shape() {
    let (server, client) = setup(vec![ok()], KEY);
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/anthropic/v1/messages");
    assert_eq!(req.header("Content-Type"), Some("application/json"));
    assert_eq!(req.header("Accept"), Some("application/json"));
    assert_eq!(req.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(req.header("X-Stainless-Retry-Count"), Some("0"));
    assert_eq!(req.header("X-Stainless-Timeout"), Some("600"));
    assert_eq!(
        req.header("User-Agent"),
        Some(concat!("graphify-rs-bedrock/", env!("CARGO_PKG_VERSION")))
    );
    let host = server.url().trim_start_matches("http://").to_string();
    assert_eq!(req.header("Host"), Some(host.as_str()));
    for (name, _) in &req.headers {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("x-stainless-") {
            assert!(
                lower == "x-stainless-retry-count" || lower == "x-stainless-timeout",
                "unexpected header {name}"
            );
        }
    }
}

#[test]
fn wire_body_matches_golden() {
    let (server, client) = setup(vec![ok()], KEY);
    call(&client).expect("request failed");
    let req = only_request(&server);
    let golden =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/request_body.json"))
            .expect("read testdata/request_body.json");
    assert_eq!(req.body, golden);
}

#[test]
fn bearer_header_and_no_aws_files() {
    let home = common::tmp_dir("brokenhome");
    std::fs::create_dir_all(home.join(".aws")).unwrap();
    let broken = "[profile\nnot = = valid\n\0[[";
    let files = [
        home.join(".aws/config"),
        home.join(".aws/credentials"),
        home.join("config"),
        home.join("credentials"),
    ];
    for f in &files {
        std::fs::write(f, broken).unwrap();
    }
    let home_s = path_str(&home);
    let config = path_str(&home.join("config"));
    let credentials = path_str(&home.join("credentials"));
    let (server, client) = setup(
        vec![ok()],
        &[
            ("HOME", home_s.as_str()),
            ("AWS_CONFIG_FILE", config.as_str()),
            ("AWS_SHARED_CREDENTIALS_FILE", credentials.as_str()),
            ("AWS_PROFILE", "no-such-profile"),
            ("AWS_BEARER_TOKEN_BEDROCK", "test-bearer-token"),
        ],
    );
    call(&client).expect("request failed");
    let req = only_request(&server);
    assert_eq!(
        req.header("Authorization"),
        Some("Bearer test-bearer-token")
    );
    assert_eq!(req.header("X-Amz-Date"), None);
    assert_eq!(req.header("X-Amz-Security-Token"), None);
}

#[test]
fn sigv4_headers_shape() {
    let before = today();
    let (server, client) = setup(vec![ok()], SIGV4);
    call(&client).expect("request failed");
    let after = today();
    let req = only_request(&server);
    let auth = req.header("Authorization").expect("Authorization header");
    let amz_date = req.header("X-Amz-Date").expect("X-Amz-Date header");
    let day = &amz_date[..8];
    assert!(
        day == before || day == after,
        "X-Amz-Date {amz_date} is not today ({before})"
    );
    let prefix = format!(
        "AWS4-HMAC-SHA256 Credential={AKID}/{day}/us-east-1/bedrock-mantle/aws4_request, SignedHeaders="
    );
    assert!(auth.starts_with(&prefix), "got: {auth}");
    let signed = signed_headers(auth);
    assert!(
        signed.contains("content-length;content-type;host;x-amz-date;x-amz-security-token"),
        "signed headers: {signed}"
    );
    assert!(
        !signed.split(';').any(|h| h == "user-agent"),
        "signed headers: {signed}"
    );
    assert_eq!(req.header("X-Amz-Security-Token"), Some(TOKEN));
}

#[test]
fn each_attempt_signed_again() {
    let script = vec![reply(503, &[FAST], ""), reply(503, &[FAST], ""), ok()];
    let (server, client) = setup(script, SIGV4);
    assert_eq!(
        call(&client).expect("request failed").as_deref(),
        Some(TOOL_INPUT)
    );
    assert_eq!(retry_counts(&server), ["0", "1", "2"]);
    let mut sigs = BTreeSet::new();
    for req in server.requests() {
        assert!(
            req.header("X-Amz-Date").is_some(),
            "attempt without X-Amz-Date"
        );
        let auth = req.header("Authorization").expect("Authorization header");
        assert!(
            signed_headers(auth)
                .split(';')
                .any(|h| h == "x-stainless-retry-count"),
            "retry count is not signed: {auth}"
        );
        sigs.insert(signature(auth).to_string());
    }
    assert_eq!(
        sigs.len(),
        3,
        "the three signatures should differ: {sigs:?}"
    );
}

#[test]
fn retry_rules() {
    struct Case {
        name: &'static str,
        status: u16,
        headers: &'static [(&'static str, &'static str)],
        attempts: usize,
    }
    let cases = [
        Case {
            name: "400 is final",
            status: 400,
            headers: &[],
            attempts: 1,
        },
        Case {
            name: "408 retried",
            status: 408,
            headers: &[FAST],
            attempts: 3,
        },
        Case {
            name: "409 retried",
            status: 409,
            headers: &[FAST],
            attempts: 3,
        },
        Case {
            name: "429 retried",
            status: 429,
            headers: &[FAST],
            attempts: 3,
        },
        Case {
            name: "500 retried",
            status: 500,
            headers: &[FAST],
            attempts: 3,
        },
        Case {
            name: "529 retried",
            status: 529,
            headers: &[FAST],
            attempts: 3,
        },
        Case {
            name: "x-should-retry false on 503",
            status: 503,
            headers: &[("x-should-retry", "false")],
            attempts: 1,
        },
        Case {
            name: "x-should-retry true on 400",
            status: 400,
            headers: &[("x-should-retry", "true"), FAST],
            attempts: 3,
        },
    ];
    for case in &cases {
        let script: Vec<MockReply> = (0..case.attempts)
            .map(|_| reply(case.status, case.headers, ""))
            .collect();
        let (server, client) = setup(script, KEY);
        let err = call(&client).expect_err(case.name);
        let Error::Api(api) = &err else {
            panic!("{}: expected Error::Api, got {err:?}", case.name);
        };
        assert_eq!(api.status, case.status, "{}", case.name);
        assert_eq!(
            server.requests().len(),
            case.attempts,
            "{}: attempts",
            case.name
        );
    }

    // closed connection: retried, then Error::Transport
    let (server, client) = setup(vec![closed(), closed(), closed()], KEY);
    let err = call(&client).expect_err("closed connection");
    assert!(
        matches!(err, Error::Transport { .. }),
        "closed connection: got {err:?}"
    );
    assert_eq!(server.requests().len(), 3, "closed connection: attempts");
}

#[test]
fn retry_after_past_deadline_is_timeout() {
    let (server, client) = setup_cfg(
        vec![reply(429, &[("Retry-After", "5")], ""), ok()],
        KEY,
        2,
        Duration::from_millis(300),
    );
    let start = Instant::now();
    let err = call(&client).expect_err("expected a timeout");
    assert!(matches!(err, Error::Timeout { .. }), "got {err:?}");
    // Go returns the bare ctx.Err() (requestconfig.go:451/:477), with no "Post" prefix.
    assert_eq!(err.to_string(), "context deadline exceeded");
    assert_eq!(server.requests().len(), 1, "no second attempt");
    assert!(
        start.elapsed() < common::scaled(Duration::from_secs(3)),
        "waited for the Retry-After"
    );
}

#[test]
fn api_error_display() {
    let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad"}}"#;
    struct Case {
        name: &'static str,
        status: u16,
        headers: &'static [(&'static str, &'static str)],
        want: &'static str,
    }
    let cases = [
        Case {
            name: "400 with request id",
            status: 400,
            headers: &[JSON, ("request-id", "req_123")],
            want: "400 Bad Request (Request-ID: req_123)",
        },
        Case {
            name: "529 has no status text",
            status: 529,
            headers: &[JSON],
            want: "529 ",
        },
        Case {
            name: "500 without request id",
            status: 500,
            headers: &[JSON],
            want: "500 Internal Server Error",
        },
    ];
    for case in &cases {
        let (server, client) = setup_cfg(
            vec![reply(case.status, case.headers, body)],
            KEY,
            0,
            Duration::from_secs(600),
        );
        let err = call(&client).expect_err(case.name);
        assert!(matches!(err, Error::Api(_)), "{}: got {err:?}", case.name);
        assert_eq!(
            err.to_string(),
            format!("POST \"{}\": {} {body}", messages_url(&server), case.want),
            "{}",
            case.name
        );
    }
}

#[test]
fn tool_input_raw() {
    // The first record_concepts block wins; its input text is kept byte for byte.
    let body = r#"{"id":"m","content":[{"type":"text","text":"hi"},{"type":"tool_use","id":"t0","name":"other","input":{"x":1}},{"type":"tool_use","id":"t1","name":"record_concepts","input": {"concepts" : [ ]} },{"type":"tool_use","id":"t2","name":"record_concepts","input":{"second":true}}],"stop_reason":"tool_use"}"#;
    let (_server, client) = setup(vec![MockReply::json(200, body)], KEY);
    let out = call(&client).expect("request failed");
    assert_eq!(out.as_deref(), Some(r#"{"concepts" : [ ]}"#));
}

#[test]
fn tool_use_without_input_is_empty() {
    let body = r#"{"content":[{"type":"tool_use","id":"t1","name":"record_concepts"}]}"#;
    let (_server, client) = setup(vec![MockReply::json(200, body)], KEY);
    let out = call(&client).expect("request failed");
    assert_eq!(out.as_deref(), Some(""));
}

#[test]
fn no_tool_use_is_none() {
    let cases = [
        (
            "text block only",
            r#"{"content":[{"type":"text","text":"hi"}]}"#,
        ),
        ("empty content", r#"{"content":[]}"#),
        (
            "other tool name",
            r#"{"content":[{"type":"tool_use","id":"t","name":"other","input":{}}]}"#,
        ),
        ("no content key", r#"{"id":"m"}"#),
    ];
    for (name, body) in cases {
        let (_server, client) = setup(vec![MockReply::json(200, body)], KEY);
        let out = call(&client).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(out, None, "{name}");
    }
}

#[test]
fn json_media_types_accepted() {
    for ct in [
        "application/json",
        "application/json; charset=utf-8",
        "application/vnd.api+json",
    ] {
        let (_server, client) = setup(
            vec![reply(200, &[("content-type", ct)], &tool_response())],
            KEY,
        );
        let out = call(&client).unwrap_or_else(|e| panic!("{ct}: {e}"));
        assert_eq!(out.as_deref(), Some(TOOL_INPUT), "{ct}");
    }
}

#[test]
fn non_json_content_type_error() {
    let (_server, client) = setup(
        vec![reply(200, &[("content-type", "text/plain")], "hi")],
        KEY,
    );
    let err = call(&client).expect_err("expected a decode error");
    assert!(matches!(err, Error::Decode(_)), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "expected destination type of 'string' or '[]byte' for responses with content-type \
         'text/plain' that is not 'application/json'"
    );
}

#[test]
fn bad_json_is_decode_error() {
    let (_server, client) = setup(vec![MockReply::json(200, "not json")], KEY);
    let err = call(&client).expect_err("expected a decode error");
    assert!(matches!(err, Error::Decode(_)), "got {err:?}");
    assert!(
        err.to_string().starts_with("error parsing response json: "),
        "got: {err}"
    );
}

#[test]
fn note_cut_mid_rune() {
    // MAX_NOTE_BYTES - 1 ASCII bytes, then a 2-byte rune: the byte cut leaves its lead byte
    // alone, which Go (and we) write as one U+FFFD.
    let content = format!("{}é", "a".repeat(MAX_NOTE_BYTES - 1));
    let (server, client) = setup(vec![ok()], KEY);
    client
        .record_concepts(NOTE_FILE, &content)
        .expect("request failed");
    let req = only_request(&server);
    let body = String::from_utf8(req.body).expect("request body is valid UTF-8");
    let want = format!(
        "{}\\ufffd\",\"type\":\"text\"}}",
        "a".repeat(MAX_NOTE_BYTES - 1)
    );
    assert!(
        body.contains(&want),
        "the note text should end with a single U+FFFD"
    );
    assert!(!body.contains('é'), "the rune should be cut");
}

#[test]
fn expired_creds_are_fetched_again() {
    let dir = common::tmp_dir("expired");
    let count = dir.join("count");
    let script = common::write_script(
        &dir,
        "creds.sh",
        &format!(
            "echo run >> {}\n\
             echo '{{\"Version\":1,\"AccessKeyId\":\"{AKID}\",\"SecretAccessKey\":\"{SECRET}\",\"SessionToken\":\"{TOKEN}\",\"Expiration\":\"2020-01-01T00:00:00Z\"}}'",
            path_str(&count)
        ),
    );
    let config = dir.join("config");
    std::fs::write(
        &config,
        format!("[default]\ncredential_process = {}\n", path_str(&script)),
    )
    .unwrap();
    let credentials = dir.join("credentials");
    let config_s = path_str(&config);
    let credentials_s = path_str(&credentials);
    let dir_s = path_str(&dir);
    let (server, client) = setup(
        vec![ok(), ok()],
        &[
            ("HOME", dir_s.as_str()),
            ("AWS_CONFIG_FILE", config_s.as_str()),
            ("AWS_SHARED_CREDENTIALS_FILE", credentials_s.as_str()),
            ("PATH", "/usr/bin:/bin"),
        ],
    );
    let runs = |count: &Path| {
        std::fs::read_to_string(count)
            .unwrap_or_default()
            .lines()
            .count()
    };
    assert_eq!(runs(&count), 1, "Client::new fetches once");
    call(&client).expect("first call");
    assert_eq!(runs(&count), 2, "the first call fetches again (expired)");
    call(&client).expect("second call");
    assert_eq!(runs(&count), 3, "the second call fetches again (expired)");
    for req in server.requests() {
        let auth = req.header("Authorization").expect("Authorization header");
        assert!(auth.contains(&format!("Credential={AKID}/")), "got: {auth}");
    }
}

/// A `credential_process` script: run 1 (in `Client::new`) prints expired credentials; later
/// runs fail when their number is in `fail_runs`, else print expired credentials again.
fn flaky_creds(tag: &str, fail_runs: &str) -> (std::path::PathBuf, Vec<(String, String)>) {
    let dir = common::tmp_dir(tag);
    let count = dir.join("count");
    let script = common::write_script(
        &dir,
        "creds.sh",
        &format!(
            "echo run >> {count}\n\
             n=$(wc -l < {count} | tr -d ' ')\n\
             case \" {fail_runs} \" in *\" $n \"*) echo boom >&2; exit 1;; esac\n\
             echo '{{\"Version\":1,\"AccessKeyId\":\"{AKID}\",\"SecretAccessKey\":\"{SECRET}\",\"SessionToken\":\"{TOKEN}\",\"Expiration\":\"2020-01-01T00:00:00Z\"}}'",
            count = path_str(&count)
        ),
    );
    let config = dir.join("config");
    std::fs::write(
        &config,
        format!("[default]\ncredential_process = {}\n", path_str(&script)),
    )
    .unwrap();
    let env = vec![
        ("HOME".to_string(), path_str(&dir)),
        ("AWS_CONFIG_FILE".to_string(), path_str(&config)),
        (
            "AWS_SHARED_CREDENTIALS_FILE".to_string(),
            path_str(&dir.join("credentials")),
        ),
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
    ];
    (count, env)
}

fn lines_in(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .count()
}

#[test]
fn credential_refresh_failure_is_retried() {
    // awsauth.go:226-229 + requestconfig.go:256: a refresh error has no response, so it is
    // retried like a connection error. Run 2 (the first attempt's refresh) fails.
    let (count, env) = flaky_creds("refresh-once", "2");
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (server, client) = setup(vec![ok()], &env);
    call(&client).expect("the retry succeeds");
    assert_eq!(lines_in(&count), 3, "new, failed refresh, refresh on retry");
    let reqs = server.requests();
    assert_eq!(reqs.len(), 1, "only the retry reaches the server");
    assert_eq!(reqs[0].header("X-Stainless-Retry-Count"), Some("1"));
}

#[test]
fn credential_refresh_always_failing_reports_go_text() {
    let (count, env) = flaky_creds("refresh-never", "2 3 4");
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (server, client) = setup(vec![ok()], &env);
    let err = call(&client).expect_err("every refresh fails");
    assert!(matches!(err, Error::RetrieveCredentials(_)), "got {err:?}");
    assert!(
        err.to_string()
            .starts_with("failed to retrieve AWS credentials: "),
        "got: {err}"
    );
    assert_eq!(lines_in(&count), 4, "new + 3 attempts (2 retries)");
    assert!(server.requests().is_empty(), "no request is sent");
}
