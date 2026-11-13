#![allow(dead_code)]
//! In-process mock HTTP/1.1 server and CONNECT proxy for tests. Std plus rustls only; the
//! parsers here are their own, not crate code. Integration tests cannot use `crate::testutil`,
//! so `tmp_dir` and `write_script` are copied from it.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// `crates/bedrock/testdata/<rel>`.
pub fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(rel)
}

/// The PEM of the CA that signed the mock server's certificate (`Config::extra_root_pem`).
pub fn ca_pem() -> Vec<u8> {
    std::fs::read(testdata("tls/ca.cert")).unwrap()
}

/// A new empty directory under the system temp dir, unique per call and per process.
pub fn tmp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bedrock-{}-{tag}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `body` to `dir/name` as an executable `#!/bin/sh` script and returns its path.
pub fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RecordedRequest {
    /// The first header named `name`, ignoring ASCII case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct MockReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Wait this long after reading the request, before replying.
    pub delay: Duration,
    /// Read the request, then drop the socket without a reply.
    pub close_without_reply: bool,
    /// Send exactly these bytes instead of a built reply (chunked, EOF body, 100-continue).
    pub raw: Option<Vec<u8>>,
}

impl Default for MockReply {
    fn default() -> MockReply {
        MockReply {
            status: 200,
            headers: Vec::new(),
            body: Vec::new(),
            delay: Duration::ZERO,
            close_without_reply: false,
            raw: None,
        }
    }
}

impl MockReply {
    pub fn new(status: u16, body: &str) -> MockReply {
        MockReply {
            status,
            body: body.as_bytes().to_vec(),
            ..MockReply::default()
        }
    }

    pub fn json(status: u16, body: &str) -> MockReply {
        MockReply {
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            ..MockReply::new(status, body)
        }
    }

    pub fn raw(bytes: &[u8]) -> MockReply {
        MockReply {
            raw: Some(bytes.to_vec()),
            ..MockReply::default()
        }
    }
}

type Headers = Vec<(String, String)>;

trait Stream: Read + Write + Send {
    /// Called after the last write: TLS sends close_notify.
    fn finish(&mut self) {}
}

impl Stream for TcpStream {
    /// Send FIN now: dropping the socket does not, when another process holds a copy of it
    /// (a child forked by a concurrent test inherits it until its exec).
    fn finish(&mut self) {
        let _ = self.shutdown(std::net::Shutdown::Write);
    }
}

impl Stream for rustls::StreamOwned<rustls::ServerConnection, TcpStream> {
    fn finish(&mut self) {
        self.conn.send_close_notify();
        let _ = self.flush();
        let _ = self.sock.shutdown(std::net::Shutdown::Write);
    }
}

/// Request line (or status line) and headers, up to the blank line.
fn read_head<R: BufRead>(r: &mut R) -> Option<(String, Vec<(String, String)>)> {
    let mut first = String::new();
    if r.read_line(&mut first).ok()? == 0 {
        return None;
    }
    let first = first.trim_end().to_string();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (k, v) = line.split_once(':')?;
        headers.push((k.trim().to_string(), v.trim().to_string()));
    }
    Some((first, headers))
}

/// The body named by `Content-Length` (none when absent).
fn read_body<R: Read>(r: &mut R, headers: &[(String, String)]) -> Option<Vec<u8>> {
    let n = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, v)| v.parse::<usize>().unwrap_or(0));
    let mut body = vec![0u8; n];
    r.read_exact(&mut body).ok()?;
    Some(body)
}

fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        407 => "Proxy Authentication Required",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Status line, the reply's headers, `Content-Length` unless `Transfer-Encoding` is set,
/// `Connection: close`, then the body.
fn encode(reply: &MockReply) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", reply.status, reason(reply.status));
    for (k, v) in &reply.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    if !reply
        .headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("transfer-encoding"))
    {
        out.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
    }
    out.push_str("Connection: close\r\n\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&reply.body);
    bytes
}

fn server_config() -> Arc<rustls::ServerConfig> {
    let chain: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(testdata("tls/end.fullchain"))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
    let key = PrivateKeyDer::from_pem_file(testdata("tls/end.key")).unwrap();
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(rustls_graviola::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .unwrap(),
    )
}

pub struct MockServer {
    port: u16,
    tls: bool,
    reqs: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl MockServer {
    /// Binds 127.0.0.1:0 and serves one reply per connection, in script order; the last reply
    /// repeats when the script runs out.
    pub fn start(tls: bool, script: Vec<MockReply>) -> MockServer {
        assert!(!script.is_empty(), "MockServer needs at least one reply");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let reqs = Arc::new(Mutex::new(Vec::new()));
        let cfg = tls.then(server_config);
        let script = Arc::new(script);
        let next = Arc::new(AtomicUsize::new(0));
        let reqs2 = Arc::clone(&reqs);
        thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(tcp) = conn else { continue };
                let (cfg, script, next, reqs) = (
                    cfg.clone(),
                    Arc::clone(&script),
                    Arc::clone(&next),
                    Arc::clone(&reqs2),
                );
                thread::spawn(move || serve(tcp, cfg, &script, &next, &reqs));
            }
        });
        MockServer { port, tls, reqs }
    }

    /// `http://127.0.0.1:<port>` or `https://localhost:<port>`.
    pub fn url(&self) -> String {
        if self.tls {
            format!("https://localhost:{}", self.port)
        } else {
            format!("http://127.0.0.1:{}", self.port)
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.reqs.lock().unwrap().clone()
    }
}

fn serve(
    tcp: TcpStream,
    tls: Option<Arc<rustls::ServerConfig>>,
    script: &[MockReply],
    next: &AtomicUsize,
    reqs: &Mutex<Vec<RecordedRequest>>,
) {
    let _ = tcp.set_read_timeout(Some(scaled(Duration::from_secs(10))));
    let stream: Box<dyn Stream> = match tls {
        Some(cfg) => {
            let Ok(conn) = rustls::ServerConnection::new(cfg) else {
                return;
            };
            Box::new(rustls::StreamOwned::new(conn, tcp))
        }
        None => Box::new(tcp),
    };
    let mut r = BufReader::new(stream);
    let Some((line, headers)) = read_head(&mut r) else {
        return;
    };
    let Some(body) = read_body(&mut r, &headers) else {
        return;
    };
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    reqs.lock().unwrap().push(RecordedRequest {
        method,
        path,
        headers,
        body,
    });
    let reply = &script[next.fetch_add(1, Ordering::SeqCst).min(script.len() - 1)];
    if reply.close_without_reply {
        return;
    }
    thread::sleep(reply.delay);
    let bytes = reply.raw.clone().unwrap_or_else(|| encode(reply));
    let out = r.get_mut();
    let _ = out.write_all(&bytes);
    let _ = out.flush();
    out.finish();
}

/// An HTTP proxy for tests. `CONNECT h:p` is answered `200 Connection established`, then bytes
/// are copied both ways to `127.0.0.1:p`. An absolute-form request is forwarded to `127.0.0.1:p`.
pub struct MockProxy {
    port: u16,
    lines: Arc<Mutex<Vec<String>>>,
    heads: Arc<Mutex<Vec<Headers>>>,
}

impl MockProxy {
    pub fn start() -> MockProxy {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let heads = Arc::new(Mutex::new(Vec::new()));
        let (l2, h2) = (Arc::clone(&lines), Arc::clone(&heads));
        thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(tcp) = conn else { continue };
                let (l, h) = (Arc::clone(&l2), Arc::clone(&h2));
                thread::spawn(move || {
                    let _ = proxy_conn(tcp, &l, &h);
                });
            }
        });
        MockProxy { port, lines, heads }
    }

    /// `http://127.0.0.1:<port>`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The request lines seen, e.g. `CONNECT localhost:4443 HTTP/1.1`.
    pub fn requests(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    /// The headers of each request seen, in the same order as `requests()`.
    pub fn headers(&self) -> Vec<Headers> {
        self.heads.lock().unwrap().clone()
    }
}

fn port_of(authority: &str, default: u16) -> u16 {
    authority
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(default)
}

fn proxy_conn(
    client: TcpStream,
    lines: &Mutex<Vec<String>>,
    heads: &Mutex<Vec<Headers>>,
) -> Option<()> {
    let _ = client.set_read_timeout(Some(scaled(Duration::from_secs(10))));
    let mut client_w = client.try_clone().ok()?;
    let mut r = BufReader::new(client);
    let (line, headers) = read_head(&mut r)?;
    lines.lock().unwrap().push(line.clone());
    heads.lock().unwrap().push(headers.clone());
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    if method == "CONNECT" {
        let Ok(up) = TcpStream::connect(("127.0.0.1", port_of(&target, 443))) else {
            let _ = client_w.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
            return None;
        };
        client_w
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .ok()?;
        let mut up_w = up.try_clone().ok()?;
        let mut c_read = r.into_inner();
        thread::spawn(move || {
            let _ = io::copy(&mut c_read, &mut up_w);
            let _ = up_w.shutdown(Shutdown::Write);
        });
        let mut up_r = up;
        let _ = io::copy(&mut up_r, &mut client_w);
        let _ = client_w.shutdown(Shutdown::Write);
    } else {
        let rest = target.strip_prefix("http://").unwrap_or(&target);
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let body = read_body(&mut r, &headers)?;
        let mut up = TcpStream::connect(("127.0.0.1", port_of(authority, 80))).ok()?;
        let mut head = format!("{method} {path} HTTP/1.1\r\n");
        for (k, v) in headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("proxy-authorization"))
        {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        up.write_all(head.as_bytes()).ok()?;
        up.write_all(&body).ok()?;
        let _ = io::copy(&mut up, &mut client_w);
    }
    Some(())
}

/// `d` times `BEDROCK_TEST_TIME_SCALE` (default 1). The `arm64` stage sets 10: under qemu-user every
/// safety deadline needs a wide margin. Only deadlines a test must never hit are scaled.
pub fn scaled(d: std::time::Duration) -> std::time::Duration {
    let k = std::env::var("BEDROCK_TEST_TIME_SCALE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    d * k
}
