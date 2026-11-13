use super::*;
use std::time::Duration;

#[path = "../../tests/common/mod.rs"]
mod common;
use common::{MockProxy, MockReply, MockServer};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

fn headers() -> Vec<(String, String)> {
    vec![("Content-Type".to_string(), "application/json".to_string())]
}

fn transport() -> Transport {
    Transport::new(Some(&common::ca_pem())).unwrap()
}

fn url_for(srv: &MockServer, path: &str) -> Url {
    Url::parse(&format!("{}{path}", srv.url())).unwrap()
}

/// Sends `raw` as the whole reply from a plain mock and returns the result.
fn send_raw(raw: &[u8]) -> Result<Response, SendError> {
    let srv = MockServer::start(false, vec![MockReply::raw(raw)]);
    transport().send(&url_for(&srv, "/x"), None, &headers(), b"{}", deadline())
}

// (input, https, host, port, path)
const URL_CASES: &[(&str, bool, &str, u16, &str)] = &[
    ("http://example.com", false, "example.com", 80, ""),
    ("https://example.com", true, "example.com", 443, ""),
    (
        "http://example.com:8080/v1/messages?x=1",
        false,
        "example.com",
        8080,
        "/v1/messages?x=1",
    ),
    ("https://[::1]:8443", true, "::1", 8443, ""),
    ("http://127.0.0.1:9/", false, "127.0.0.1", 9, "/"),
];

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn url_parse_cases() {
    for (input, https, host, port, path) in URL_CASES {
        let u = Url::parse(input).unwrap_or_else(|e| panic!("{input}: {e}"));
        assert_eq!(
            (
                u.https,
                u.host.as_str(),
                u.port,
                u.path.as_str(),
                u.userinfo.as_deref()
            ),
            (*https, *host, *port, *path, None),
            "{input}"
        );
    }
    let u = Url::parse("http://u%40x:p%3Ass@127.0.0.1:5").unwrap();
    assert_eq!(u.userinfo.as_deref(), Some("u%40x:p%3Ass"));
    assert_eq!(u.host, "127.0.0.1");
    assert_eq!(
        Url::parse("http://example.com").unwrap().host_header(),
        "example.com"
    );
    assert_eq!(
        Url::parse("https://example.com:443").unwrap().host_header(),
        "example.com"
    );
    assert_eq!(
        Url::parse("http://example.com:8080").unwrap().host_header(),
        "example.com:8080"
    );
    assert!(Url::parse("ftp://x").is_err(), "ftp must be rejected");
}

fn check_round_trip(srv: &MockServer, t: &Transport) {
    let url = url_for(srv, "/v1/messages");
    let res = t
        .send(&url, None, &headers(), b"{\"a\":1}", deadline())
        .unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.header("x-test"), Some("yes"));
    assert_eq!(res.body, b"hello");
    let reqs = srv.requests();
    assert_eq!(reqs.len(), 1);
    let r = &reqs[0];
    assert_eq!(r.method, "POST");
    assert_eq!(r.path, "/v1/messages");
    assert_eq!(
        r.header("Host"),
        Some(format!("{}:{}", url.host, url.port).as_str())
    );
    assert_eq!(r.header("Content-Type"), Some("application/json"));
    assert_eq!(r.header("Content-Length"), Some("7"));
    assert_eq!(r.body, b"{\"a\":1}");
}

fn hello_reply() -> MockReply {
    MockReply {
        headers: vec![("X-Test".to_string(), "yes".to_string())],
        body: b"hello".to_vec(),
        ..MockReply::default()
    }
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn plain_round_trip() {
    let srv = MockServer::start(false, vec![hello_reply()]);
    check_round_trip(&srv, &Transport::new(None).unwrap());
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn tls_round_trip() {
    let srv = MockServer::start(true, vec![hello_reply()]);
    check_round_trip(&srv, &transport());
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn tls_rejects_unknown_ca() {
    let srv = MockServer::start(true, vec![hello_reply()]);
    let t = Transport::new(None).unwrap();
    match t.send(&url_for(&srv, "/x"), None, &headers(), b"{}", deadline()) {
        Err(SendError::Io(m)) => assert!(m.contains("UnknownIssuer"), "message: {m}"),
        other => panic!("want Io(UnknownIssuer), got {other:?}"),
    }
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn chunked_body() {
    let res = send_raw(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n\
          5\r\nhello\r\n6\r\n world\r\n0\r\nX-Trailer: 1\r\n\r\n",
    )
    .unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"hello world");
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn content_length_body() {
    let res = send_raw(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello-extra").unwrap();
    assert_eq!(res.body, b"hello");
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn eof_body() {
    let res = send_raw(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nuntil eof").unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"until eof");
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn skips_100_continue() {
    let res = send_raw(
        b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok",
    )
    .unwrap();
    assert_eq!(res.status, 201);
    assert_eq!(res.body, b"ok");
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn deadline_while_reading() {
    let slow = MockReply {
        delay: Duration::from_secs(2),
        ..hello_reply()
    };
    let srv = MockServer::start(false, vec![slow]);
    let start = Instant::now();
    let res = transport().send(
        &url_for(&srv, "/x"),
        None,
        &headers(),
        b"{}",
        Instant::now() + Duration::from_millis(200),
    );
    assert_eq!(res, Err(SendError::Timeout));
    assert!(
        start.elapsed() < Duration::from_millis(1500),
        "returned after {:?}",
        start.elapsed()
    );
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn connection_refused_is_io() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = Url::parse(&format!("http://127.0.0.1:{port}/x")).unwrap();
    let res = transport().send(&url, None, &headers(), b"{}", deadline());
    assert!(matches!(res, Err(SendError::Io(_))), "got {res:?}");
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn tls_through_connect_proxy() {
    let srv = MockServer::start(true, vec![hello_reply()]);
    let proxy = MockProxy::start();
    let url = url_for(&srv, "/v1/messages");
    let purl = Url::parse(&proxy.url()).unwrap();
    let res = transport()
        .send(&url, Some(&purl), &headers(), b"{}", deadline())
        .unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"hello");
    assert_eq!(
        proxy.requests(),
        vec![format!("CONNECT localhost:{} HTTP/1.1", srv.port())]
    );
    let reqs = srv.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].path, "/v1/messages");
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn proxy_basic_auth() {
    let srv = MockServer::start(true, vec![hello_reply()]);
    let proxy = MockProxy::start();
    let purl = Url::parse(&proxy.url().replace("http://", "http://u%40x:p%3Ass@")).unwrap();
    let res = transport()
        .send(
            &url_for(&srv, "/x"),
            Some(&purl),
            &headers(),
            b"{}",
            deadline(),
        )
        .unwrap();
    assert_eq!(res.status, 200);
    let heads = proxy.headers();
    assert_eq!(heads.len(), 1);
    let auth = heads[0]
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("proxy-authorization"))
        .map(|(_, v)| v.as_str());
    assert_eq!(auth, Some("Basic dUB4OnA6c3M="));
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn proxy_connect_refused_status() {
    // A plain mock stands in for a proxy that refuses the tunnel.
    let proxy = MockServer::start(false, vec![MockReply::new(407, "")]);
    let purl = Url::parse(&proxy.url()).unwrap();
    let url = Url::parse("https://localhost:9/x").unwrap();
    match transport().send(&url, Some(&purl), &headers(), b"{}", deadline()) {
        Err(SendError::Io(m)) => assert!(m.contains("407"), "message: {m}"),
        other => panic!("want Io(407), got {other:?}"),
    }
}

#[test]
#[ignore = "graphify-rs-u2o.7"]
fn http_through_proxy_absolute_form() {
    let srv = MockServer::start(false, vec![hello_reply()]);
    let proxy = MockProxy::start();
    let purl = Url::parse(&proxy.url()).unwrap();
    let res = transport()
        .send(
            &url_for(&srv, "/v1/messages"),
            Some(&purl),
            &headers(),
            b"{}",
            deadline(),
        )
        .unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(
        proxy.requests(),
        vec![format!(
            "POST http://127.0.0.1:{}/v1/messages HTTP/1.1",
            srv.port()
        )]
    );
    assert_eq!(srv.requests()[0].path, "/v1/messages");
}
