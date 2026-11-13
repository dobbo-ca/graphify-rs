use std::path::Path;
use std::time::SystemTime;

use crate::creds::Credentials;
use crate::json::{self, Kind, Value};
use crate::sha::{hex, sha256};
use crate::sigv4::{Options, Request, canonical_request, sign, string_to_sign};
use crate::testutil::testdata;
use crate::time::{Utc, parse_rfc3339};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// The two vectors migrated into the aws-sigv4 suite; they have no `context.json`.
const MIGRATED: [&str; 2] = ["double-encode-path", "double-url-encode"];

/// Hex SHA-256 of the empty string (internal/v4/const.go:6 `EmptyStringSHA256`).
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// URL.Opaque path used by Go's `buildRequest` (v4_test.go:40), as the signer sees it.
const GO_PATH: &str = "/bucket/key-._~,!@#$%^&*()";

fn h(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The file's text with ONE trailing newline removed, if it has one.
fn read_expected(dir: &Path, file: &str) -> String {
    let s = read(&dir.join(file));
    s.strip_suffix('\n').unwrap_or(&s).to_string()
}

/// The value of the `Authorization:` line of a `header-signed-request.txt`.
fn expected_authorization(dir: &Path) -> Option<String> {
    let signed = read(&dir.join("header-signed-request.txt"));
    signed
        .lines()
        .find_map(|l| l.strip_prefix("Authorization:"))
        .map(|v| v.trim_start().to_string())
}

struct Parsed {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: String,
}

/// Parses a suite `request.txt`: request line `METHOD target HTTP/1.1` (the target may hold
/// spaces), `Name:value` header lines, continuation lines starting with whitespace, a blank line,
/// then the body.
fn parse_request(txt: &str) -> Parsed {
    let lines: Vec<&str> = txt.split('\n').collect();
    let (method, rest) = lines[0].split_once(' ').expect("request line has a method");
    let (target, _version) = rest.rsplit_once(' ').expect("request line has a version");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut i = 1;
    while i < lines.len() && !lines[i].is_empty() {
        let line = lines[i];
        if line.starts_with(' ') || line.starts_with('\t') {
            let last = headers.last_mut().expect("continuation follows a header");
            last.1 = format!("{} {}", last.1.trim(), line.trim());
        } else {
            let (name, value) = line.split_once(':').expect("header line has a colon");
            headers.push((name.to_string(), value.to_string()));
        }
        i += 1;
    }
    let body = lines.get(i + 1..).unwrap_or_default().join("\n");
    Parsed {
        method: method.to_string(),
        path: path.to_string(),
        query: query.to_string(),
        headers,
        body,
    }
}

struct Context {
    creds: Credentials,
    region: String,
    service: String,
    t: Utc,
    normalize: bool,
    sign_body: bool,
    omit_session_token: bool,
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("context.json: no string field {key}"))
        .to_string()
}

fn bool_field(v: &Value, key: &str) -> bool {
    matches!(v.get(key).map(|x| &x.kind), Some(Kind::Bool(true)))
}

fn utc(ts: &str) -> Utc {
    Utc::from_system(parse_rfc3339(ts).unwrap_or_else(|| panic!("bad timestamp {ts}")))
}

fn load_context(dir: &Path) -> Context {
    let src = read(&dir.join("context.json"));
    let v = json::parse(&src).unwrap_or_else(|e| panic!("{}: context.json: {e}", dir.display()));
    let c = v.get("credentials").expect("context.json: credentials");
    // The suite spells the session token `token`.
    let token = c
        .get("token")
        .or_else(|| c.get("session_token"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Context {
        creds: Credentials {
            access_key_id: str_field(c, "access_key_id"),
            secret_access_key: str_field(c, "secret_access_key"),
            session_token: token,
            expires: None,
        },
        region: str_field(&v, "region"),
        service: str_field(&v, "service"),
        t: utc(&str_field(&v, "timestamp")),
        normalize: bool_field(&v, "normalize"),
        sign_body: bool_field(&v, "sign_body"),
        omit_session_token: bool_field(&v, "omit_session_token"),
    }
}

fn check(fails: &mut Vec<String>, dir: &Path, what: &str, expected: &str, actual: &str) {
    if expected != actual {
        let name = dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        fails.push(format!(
            "{name}: {what}\n  expected: {expected:?}\n  actual:   {actual:?}"
        ));
    }
}

/// Signs one aws-c-auth vector and returns a message per mismatch.
fn run_vector(dir: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    let ctx = load_context(dir);
    let mut p = parse_request(&read(&dir.join("request.txt")));
    // aws-sigv4 1.6.5 rewrote aws-c-auth's `/example space/` to `%20` and ignores this vector
    // (sign.rs:1156); header-signed-request.txt keeps the original request line.
    if dir.ends_with("get-space-normalized") {
        p.path = p.path.replace("%20", " ");
    }
    let body_hash = hex(&sha256(p.body.as_bytes()));
    let mut headers = p.headers;
    if ctx.sign_body {
        headers.push(h("x-amz-content-sha256", &body_hash));
    }
    let mut req = Request {
        method: &p.method,
        path: &p.path,
        query: &p.query,
        headers,
        body_sha256_hex: body_hash,
    };
    let opts = Options {
        normalize_path: ctx.normalize,
        sign_session_token: !ctx.omit_session_token,
    };
    let out = sign(
        &mut req,
        &ctx.creds,
        &ctx.region,
        &ctx.service,
        &ctx.t,
        opts,
    );

    let tokens: Vec<&str> = req
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("x-amz-security-token"))
        .map(|(_, v)| v.as_str())
        .collect();
    let want_tokens: Vec<&str> = ctx.creds.session_token.as_deref().into_iter().collect();
    check(
        &mut fails,
        dir,
        "X-Amz-Security-Token headers",
        &format!("{want_tokens:?}"),
        &format!("{tokens:?}"),
    );
    let dates: Vec<&str> = req
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("x-amz-date"))
        .map(|(_, v)| v.as_str())
        .collect();
    check(
        &mut fails,
        dir,
        "X-Amz-Date headers",
        &format!("{:?}", [ctx.t.amz_date()]),
        &format!("{dates:?}"),
    );

    let want_creq = read_expected(dir, "header-canonical-request.txt");
    check(
        &mut fails,
        dir,
        "canonical request",
        &want_creq,
        &out.canonical_request,
    );
    let want_sts = read_expected(dir, "header-string-to-sign.txt");
    check(
        &mut fails,
        dir,
        "string to sign",
        &want_sts,
        &out.string_to_sign,
    );
    let want_sig = read_expected(dir, "header-signature.txt");
    check(&mut fails, dir, "signature", &want_sig, &out.signature);
    match expected_authorization(dir) {
        Some(want) => check(&mut fails, dir, "Authorization", &want, &out.authorization),
        None => fails.push(format!(
            "{}: no Authorization line in header-signed-request.txt",
            dir.display()
        )),
    }
    fails
}

fn go_creds() -> Credentials {
    Credentials {
        access_key_id: "AKID".to_string(),
        secret_access_key: "SECRET".to_string(),
        session_token: Some("SESSION".to_string()),
        expires: None,
    }
}

/// The headers Go's `buildRequest` (v4_test.go:40-44) sets, in order.
fn go_build_request_headers() -> Vec<(String, String)> {
    vec![
        h("X-Amz-Target", "prefix.Operation"),
        h("Content-Type", "application/x-amz-json-1.0"),
        h("X-Amz-Meta-Other-Header", "some-value=!@#$%^&* (+)"),
        h(
            "X-Amz-Meta-Other-Header_With_Underscore",
            "some-value=!@#$%^&* (+)",
        ),
        h(
            "X-amz-Meta-Other-Header_With_Underscore",
            "some-value=!@#$%^&* (+)",
        ),
    ]
}

/// Request with an empty body hash, for canonical-request-only checks.
fn creq(path: &str, query: &str, headers: Vec<(String, String)>, hash: &str) -> String {
    let req = Request {
        method: "POST",
        path,
        query,
        headers,
        body_sha256_hex: hash.to_string(),
    };
    canonical_request(
        &req,
        Options {
            normalize_path: false,
            sign_session_token: true,
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Vectors (aws-sigv4 1.6.5 aws-signing-test-suite/v4)
// ---------------------------------------------------------------------------------------------

// New: harness over the 38 aws-c-auth v0.9.0 vectors in testdata/sigv4/v4 (aws-sigv4 1.6.5).
#[test]
fn aws_c_auth_vectors() {
    let root = testdata("sigv4/v4");
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("read {}: {e}", root.display()))
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| !MIGRATED.contains(&n.as_str()))
        .collect();
    names.sort();
    let mut fails: Vec<String> = Vec::new();
    for n in &names {
        fails.extend(run_vector(&root.join(n)));
    }
    if names.len() != 38 {
        fails.push(format!(
            "expected 38 aws-c-auth vector dirs, found {}",
            names.len()
        ));
    }
    assert!(
        fails.is_empty(),
        "{} failure(s):\n{}",
        fails.len(),
        fails.join("\n")
    );
}

// New: the 2 vectors migrated into aws-sigv4 (canonical_request.rs:941 test_double_url_encode_path,
// :955 test_double_url_encode). Their canonical requests carry 20210511T154045Z, so the date header
// of request.txt is replaced; the credentials do not matter for these checks.
#[test]
fn migrated_double_encode() {
    let t = utc("2021-05-11T15:40:45Z");
    for name in MIGRATED {
        let dir = testdata(&format!("sigv4/v4/{name}"));
        let p = parse_request(&read(&dir.join("request.txt")));
        let mut headers: Vec<(String, String)> = p
            .headers
            .into_iter()
            .filter(|(n, _)| !n.eq_ignore_ascii_case("x-amz-date"))
            .collect();
        headers.push(h("X-Amz-Date", &t.amz_date()));
        let req = Request {
            method: &p.method,
            path: &p.path,
            query: &p.query,
            headers,
            body_sha256_hex: hex(&sha256(p.body.as_bytes())),
        };
        let opts = Options {
            normalize_path: true,
            sign_session_token: true,
        };
        let creq = canonical_request(&req, opts);
        let want = read_expected(&dir, "header-canonical-request.txt");
        assert_eq!(creq, want, "{name}: canonical request");

        if dir.join("header-string-to-sign.txt").exists() {
            // Scope from the Credential= of header-signed-request.txt.
            let scope = format!("{}/us-east-2/lambda/aws4_request", t.short_date());
            let want = read_expected(&dir, "header-string-to-sign.txt");
            assert_eq!(
                string_to_sign(&creq, &t, &scope),
                want,
                "{name}: string to sign"
            );
        }
    }
}

// New: aws-sigv4 sign.rs:761 test_sign_url_escape, over the double-encode-path vector:
// credentials ANOTREAL, 2015-08-30T12:36:00Z, us-east-1, service "service".
#[test]
fn migrated_double_encode_path_signature() {
    let dir = testdata("sigv4/v4/double-encode-path");
    let p = parse_request(&read(&dir.join("request.txt")));
    let headers: Vec<(String, String)> = p
        .headers
        .into_iter()
        .filter(|(n, _)| !n.eq_ignore_ascii_case("x-amz-date"))
        .collect();
    let mut req = Request {
        method: &p.method,
        path: &p.path,
        query: &p.query,
        headers,
        body_sha256_hex: hex(&sha256(p.body.as_bytes())),
    };
    let creds = Credentials {
        access_key_id: "ANOTREAL".to_string(),
        secret_access_key: "notrealrnrELgWzOk3IfjzDKtFBhDby".to_string(),
        session_token: None,
        expires: None,
    };
    let t = utc("2015-08-30T12:36:00Z");
    let opts = Options {
        normalize_path: true,
        sign_session_token: true,
    };
    let out = sign(&mut req, &creds, "us-east-1", "service", &t, opts);
    assert_eq!(
        out.signature, "57d157672191bac40bae387e48bbe14b15303c001fdbb01f4abf295dccb09705",
        "double-encode-path: signature"
    );
    let want = expected_authorization(&dir).expect("Authorization line");
    assert_eq!(out.authorization, want, "double-encode-path: Authorization");
}

// ---------------------------------------------------------------------------------------------
// aws-sdk-go-v2 v1.30.3 signer tests
// ---------------------------------------------------------------------------------------------

// Port of aws/signer/v4/v4_test.go:163 TestSignRequest (aws-sdk-go-v2 v1.30.3).
// Go signs Host and Content-Length itself; here the caller list carries them. Go's URL.Opaque
// path is passed as the on-wire path (the signer escapes it once, as Go's EscapePath does).
#[test]
fn sign_request() {
    let body_hash = hex(&sha256(b"{}"));
    let mut headers = vec![
        h("Host", "dynamodb.us-east-1.amazonaws.com"),
        h("Content-Length", "2"),
    ];
    headers.extend(go_build_request_headers());
    let mut req = Request {
        method: "POST",
        path: GO_PATH,
        query: "",
        headers,
        body_sha256_hex: body_hash,
    };
    let t = Utc::from_system(SystemTime::UNIX_EPOCH);
    let opts = Options {
        normalize_path: false,
        sign_session_token: true,
    };
    let out = sign(&mut req, &go_creds(), "us-east-1", "dynamodb", &t, opts);

    let expected_date = "19700101T000000Z";
    let expected_sig = "AWS4-HMAC-SHA256 Credential=AKID/19700101/us-east-1/dynamodb/aws4_request, SignedHeaders=content-length;content-type;host;x-amz-date;x-amz-meta-other-header;x-amz-meta-other-header_with_underscore;x-amz-security-token;x-amz-target, Signature=a518299330494908a70222cec6899f6f32f297f8595f6df1776d998936652ad9";
    assert_eq!(out.authorization, expected_sig, "Authorization");
    let date = req
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("x-amz-date"))
        .map(|(_, v)| v.as_str());
    assert_eq!(date, Some(expected_date), "X-Amz-Date header");
}

// Port of aws/signer/v4/v4_test.go:224 TestRequestHost (aws-sdk-go-v2 v1.30.3).
// Go: req.Host overrides URL.Host; here the caller puts that value in the Host header.
#[test]
fn request_host() {
    let mut headers = vec![h("Host", "myhost")];
    headers.extend(go_build_request_headers());
    headers.push(h("X-Amz-Date", "20240101T000000Z"));
    let canonical = creq(
        GO_PATH,
        "Foo=z&Foo=o&Foo=m&Foo=a&X-Amz-Expires=5",
        headers,
        EMPTY_SHA256,
    );
    assert!(
        canonical.contains("host:myhost"),
        "canonical host header invalid:\n{canonical}"
    );
}

// Port of aws/signer/v4/v4_test.go:251 TestSign_buildCanonicalHeadersContentLengthPresent
// (aws-sdk-go-v2 v1.30.3). Go dedups its own Content-Length against a caller-set one; here the
// caller supplies it once.
#[test]
fn sign_build_canonical_headers_content_length_present() {
    let body = r#"{"description": "this is a test"}"#;
    let content_length = body.len().to_string();
    let mut headers = vec![h("Host", "myhost"), h("Content-Length", &content_length)];
    headers.extend(go_build_request_headers());
    headers.push(h("X-Amz-Date", "20240101T000000Z"));
    let canonical = creq(
        GO_PATH,
        "Foo=z&Foo=o&Foo=m&Foo=a&X-Amz-Expires=5",
        headers,
        EMPTY_SHA256,
    );
    let want = format!("content-length:{content_length}\n");
    assert!(
        canonical.contains(&want),
        "canonical header content-length invalid:\n{canonical}"
    );
    assert_eq!(
        canonical.matches("content-length:").count(),
        1,
        "content-length signed once:\n{canonical}"
    );
}

// Port of aws/signer/v4/v4_test.go:282 TestSign_buildCanonicalHeaders (aws-sdk-go-v2 v1.30.3);
// golden at :315-331. The payload hash is empty in Go's test, so the string ends after the
// signed-headers line and its newline. X-Amz-Date is added here (Go adds it in Build).
#[test]
fn sign_build_canonical_headers() {
    let headers = vec![
        h("Host", "mockAPI.mock-region.amazonaws.com"),
        h("FooInnerSpace", "   inner      space    "),
        h("FooLeadingSpace", "    leading-space"),
        h("FooMultipleSpace", "no-space"),
        h("FooMultipleSpace", "\ttab-space"),
        h("FooMultipleSpace", "trailing-space    "),
        h("FooNoSpace", "no-space"),
        h("FooTabSpace", "\ttab-space\t"),
        h("FooTrailingSpace", "trailing-space    "),
        h("FooWrappedSpace", "   wrapped-space    "),
        h("X-Amz-Date", "20211020T124200Z"),
    ];
    let expect = [
        "POST",
        "/",
        "",
        "fooinnerspace:inner space",
        "fooleadingspace:leading-space",
        "foomultiplespace:no-space,tab-space,trailing-space",
        "foonospace:no-space",
        "footabspace:tab-space",
        "footrailingspace:trailing-space",
        "foowrappedspace:wrapped-space",
        "host:mockAPI.mock-region.amazonaws.com",
        "x-amz-date:20211020T124200Z",
        "",
        "fooinnerspace;fooleadingspace;foomultiplespace;foonospace;footabspace;footrailingspace;foowrappedspace;host;x-amz-date",
        "",
    ]
    .join("\n");
    assert_eq!(creq("/", "", headers, ""), expect);
}

const STRIP_VALS: [&str; 14] = [
    "",
    "123",
    "1 2 3",
    "1 2 3 ",
    "  1 2 3",
    "1  2 3",
    "1  23",
    "1  2  3",
    "1  2  ",
    " 1  2  ",
    "12   3",
    "12   3   1",
    "12           3     1",
    "12     3       1abc123",
];

const STRIP_EXPECTED: [&str; 14] = [
    "",
    "123",
    "1 2 3",
    "1 2 3",
    "1 2 3",
    "1 2 3",
    "1 23",
    "1 2 3",
    "1 2",
    "1 2",
    "12 3",
    "12 3 1",
    "12 3 1",
    "12 3 1abc123",
];

// Port of aws/signer/internal/v4/util_test.go:90 TestStripExcessHeaders (aws-sdk-go-v2 v1.30.3).
// StripExcessSpaces is Go-internal; it is tested through the value of a signed header in
// canonical_request.
#[test]
fn strip_excess_headers() {
    for (i, (val, want)) in STRIP_VALS.iter().zip(STRIP_EXPECTED.iter()).enumerate() {
        let canonical = creq("/", "", vec![h("X-Test", val)], "H");
        let expect = format!("POST\n/\n\nx-test:{want}\n\nx-test\nH");
        assert_eq!(canonical, expect, "case {i} value {val:?}");
    }
}

// Port of aws/signer/internal/v4/util_test.go:15 TestGetURIPath (aws-sdk-go-v2 v1.30.3),
// non-Opaque cases only. GetURIPath is Go-internal and works on a parsed URL; the caller here
// passes the path it sends; Go's EscapedPath() is "" for the four no-path cases, so those pass ""
// and the canonical path line must be "/" (the empty-path rule of GetURIPath). The four Opaque cases ("without scheme", "without scheme,
// with path", "opaque with query string", "failing test") are skipped.
#[test]
fn get_uri_path() {
    // (Go case name, URL, on-wire path, expected canonical path)
    let cases = [
        ("with scheme", "https://localhost:9000", "", "/"),
        ("no port, with scheme", "https://localhost", "", "/"),
        (
            "without scheme, with separator",
            "//localhost:9000",
            "",
            "/",
        ),
        (
            "no port, without scheme, with separator",
            "//localhost",
            "",
            "/",
        ),
        (
            "without scheme, with separator, with path",
            "//localhost:9000/abc123",
            "/abc123",
            "/abc123",
        ),
        (
            "no port, without scheme, with separator, with path",
            "//localhost/abc123",
            "/abc123",
            "/abc123",
        ),
    ];
    for (name, url, path, expect) in cases {
        let canonical = creq(path, "", vec![h("Host", "localhost")], EMPTY_SHA256);
        let got = canonical.lines().nth(1).unwrap_or_default();
        assert_eq!(got, expect, "{name} ({url})");
    }
}

// Port of aws/signer/internal/v4/headers_test.go:37 TestIgnoredHeaders (aws-sdk-go-v2 v1.30.3).
// IgnoredHeaders is Go-internal; checked through the signed-headers line of canonical_request.
#[test]
fn ignored_headers() {
    // (Go case name, header, expect ignored)
    let cases = [
        ("expect", "Expect", true),
        ("authorization", "Authorization", true),
        ("X-AMZ header", "X-Amz-Content-Sha256", false),
    ];
    for (name, header, expect_ignored) in cases {
        let canonical = creq(
            "/",
            "",
            vec![h("Host", "example.com"), h(header, "v")],
            EMPTY_SHA256,
        );
        let signed = canonical.lines().nth_back(1).expect("signed headers line");
        let lower = header.to_ascii_lowercase();
        let ignored = !signed.split(';').any(|s| s == lower);
        assert_eq!(ignored, expect_ignored, "{name}: signed headers {signed:?}");
    }
}

// New: the other two headers of internal/v4/headers.go:4-13 IgnoredHeaders, which the Go test
// does not cover (plan Task 4 lists them).
#[test]
fn ignored_headers_user_agent_and_trace_id() {
    let headers = vec![
        h("Host", "example.com"),
        h("User-Agent", "ua"),
        h("X-Amzn-Trace-Id", "Root=1-abc"),
    ];
    let canonical = creq("/", "", headers, EMPTY_SHA256);
    let signed = canonical.lines().nth_back(1).expect("signed headers line");
    assert_eq!(
        signed, "host",
        "user-agent and x-amzn-trace-id are not signed"
    );
}

struct StandaloneCase {
    orig_uri: &'static str,
    orig_query: &'static str,
    region: &'static str,
    service: &'static str,
    sub_domain: &'static str,
    exp_sig: &'static str,
    escaped_uri: &'static str,
}

// functional_test.go:15 standaloneSignCases.
const STANDALONE_CASES: [StandaloneCase; 1] = [StandaloneCase {
    orig_uri: "/logs-*/_search",
    orig_query: "pretty=true",
    region: "us-west-2",
    service: "es",
    sub_domain: "hostname-clusterkey",
    escaped_uri: "/logs-%2A/_search",
    exp_sig: "AWS4-HMAC-SHA256 Credential=AKID/19700101/us-west-2/es/aws4_request, SignedHeaders=host;x-amz-date;x-amz-security-token, Signature=79d0760751907af16f64a537c1242416dacf51204a7dd5284492d15577973b91",
}];

/// Signs a GET of `path` for each standalone case; Go's signer takes the escaped path.
fn run_standalone(test: &str, check_canonical_path: bool) {
    let t = Utc::from_system(SystemTime::UNIX_EPOCH);
    let opts = Options {
        normalize_path: false,
        sign_session_token: true,
    };
    for c in &STANDALONE_CASES {
        let host = format!("{}.{}.{}.amazonaws.com", c.sub_domain, c.region, c.service);
        let mut req = Request {
            method: "GET",
            path: c.escaped_uri,
            query: c.orig_query,
            headers: vec![h("Host", &host)],
            body_sha256_hex: EMPTY_SHA256.to_string(),
        };
        let out = sign(&mut req, &go_creds(), c.region, c.service, &t, opts);
        assert_eq!(
            out.authorization, c.exp_sig,
            "{test}: Authorization for {}",
            c.orig_uri
        );
        if check_canonical_path {
            // Go signs EscapedPath() escaped again: `%2A` -> `%252A`.
            let path = out.canonical_request.lines().nth(1).unwrap_or_default();
            assert_eq!(path, "/logs-%252A/_search", "{test}: canonical path");
        }
    }
}

// Port of aws/signer/v4/functional_test.go:62 TestStandaloneSign (aws-sdk-go-v2 v1.30.3).
// Go sets URL.Path to OrigURI and the signer reads EscapedPath(); here the path passed is that
// escaped form (EscapedURI). The checks on req.URL.Path / EscapedPath() are Go-internal, skipped.
#[test]
fn standalone_sign() {
    run_standalone("standalone_sign", false);
}

// Port of aws/signer/v4/functional_test.go:101 TestStandaloneSign_RawPath (aws-sdk-go-v2
// v1.30.3). Go sets RawPath = EscapedURI; the on-wire path is the same as in `standalone_sign`,
// so this also checks the double-escaped canonical path.
#[test]
fn standalone_sign_raw_path() {
    run_standalone("standalone_sign_raw_path", true);
}
