//! HTTP/1.1 over `std::net::TcpStream`, with TLS from rustls + rustls-graviola + webpki-roots,
//! and HTTP CONNECT proxies. One connection per request.

use std::io::{self, BufRead, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Url {
    pub https: bool,
    /// A name, an IPv4 address, or an IPv6 address without brackets.
    pub host: String,
    pub port: u16,
    /// Path and query as written; may be empty.
    pub path: String,
    /// Percent-encoded `user[:password]`, for proxy URLs.
    pub userinfo: Option<String>,
}

impl Url {
    pub(crate) fn parse(s: &str) -> Result<Url, String> {
        let (scheme, rest) = s
            .split_once("://")
            .ok_or_else(|| format!("invalid URL {s:?}"))?;
        let https = match scheme.to_ascii_lowercase().as_str() {
            "http" => false,
            "https" => true,
            other => return Err(format!("unsupported scheme {other:?}")),
        };
        let rest = rest.split('#').next().unwrap_or("");
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        let (authority, path) = rest.split_at(end);
        let (userinfo, hostport) = match authority.rsplit_once('@') {
            Some((u, h)) => (Some(u.to_string()), h),
            None => (None, authority),
        };
        let (host, port) = if let Some(v6) = hostport.strip_prefix('[') {
            let (host, after) = v6
                .split_once(']')
                .ok_or_else(|| format!("invalid URL {s:?}"))?;
            let port = match after.strip_prefix(':') {
                Some(p) => p,
                None if after.is_empty() => "",
                None => return Err(format!("invalid URL {s:?}")),
            };
            (host, port)
        } else {
            hostport.rsplit_once(':').unwrap_or((hostport, ""))
        };
        if host.is_empty() {
            return Err(format!("invalid URL {s:?}: no host"));
        }
        let port = if port.is_empty() {
            if https { 443 } else { 80 }
        } else {
            port.parse::<u16>()
                .map_err(|_| format!("invalid URL {s:?}: bad port"))?
        };
        Ok(Url {
            https,
            host: host.to_string(),
            port,
            path: path.to_string(),
            userinfo,
        })
    }

    /// `host` or `host:port` when the port is not the scheme's default (the `Host` header).
    pub(crate) fn host_header(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == if self.https { 443 } else { 80 } {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// The first header named `name`, ignoring ASCII case.
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SendError {
    Io(String),
    Timeout,
}

impl From<io::Error> for SendError {
    fn from(e: io::Error) -> SendError {
        match e.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => SendError::Timeout,
            _ => SendError::Io(e.to_string()),
        }
    }
}

fn malformed() -> SendError {
    SendError::Io("malformed HTTP response".to_string())
}

/// A socket whose every read and write stops at the deadline.
struct Timed {
    tcp: TcpStream,
    deadline: Instant,
}

impl Timed {
    fn left(&self) -> io::Result<Duration> {
        let d = self.deadline.saturating_duration_since(Instant::now());
        if d.is_zero() {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(d)
        }
    }
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.tcp.set_read_timeout(Some(self.left()?))?;
        self.tcp.read(buf)
    }
}

impl Write for Timed {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.tcp.set_write_timeout(Some(self.left()?))?;
        self.tcp.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp.flush()
    }
}

/// Tries every address of `host` in turn, each for at most the time left.
// ponytail: the DNS lookup itself is not bounded by the deadline (std has no timeout for it).
fn connect(host: &str, port: u16, deadline: Instant) -> Result<TcpStream, SendError> {
    let mut last = None;
    for addr in (host, port).to_socket_addrs()? {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(SendError::Timeout);
        }
        match TcpStream::connect_timeout(&addr, left) {
            Ok(tcp) => return Ok(tcp),
            Err(e) => last = Some(e),
        }
    }
    Err(match last {
        Some(e) => e.into(),
        None => SendError::Io(format!("no addresses for {host}")),
    })
}

fn pct_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = if b[i] == b'%' {
            b.get(i + 1..i + 3)
                .filter(|h| h.iter().all(u8::is_ascii_hexdigit)) // from_str_radix accepts '+'
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        } else {
            None
        };
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    out
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4u32 {
            if (i as usize) <= c.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `Proxy-Authorization: Basic ...\r\n` from the proxy URL's userinfo (percent-decoded).
fn proxy_auth(proxy: &Url) -> String {
    let Some(ui) = proxy.userinfo.as_deref() else {
        return String::new();
    };
    let (user, pass) = ui.split_once(':').unwrap_or((ui, ""));
    let mut cred = pct_decode(user);
    cred.push(b':');
    cred.extend(pct_decode(pass));
    format!("Proxy-Authorization: Basic {}\r\n", base64(&cred))
}

/// One line without its CRLF; EOF is malformed.
fn read_line<R: BufRead>(r: &mut R) -> Result<String, SendError> {
    let mut buf = Vec::new();
    if r.read_until(b'\n', &mut buf)? == 0 {
        return Err(malformed());
    }
    let line = String::from_utf8_lossy(&buf);
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

/// Appends exactly `n` bytes from `r` to `out`.
fn read_n<R: Read>(r: &mut R, n: u64, out: &mut Vec<u8>) -> Result<(), SendError> {
    let got = r.by_ref().take(n).read_to_end(out)?;
    if got as u64 != n {
        return Err(SendError::Io("unexpected EOF in response body".to_string()));
    }
    Ok(())
}

fn read_chunked<R: BufRead>(r: &mut R) -> Result<Vec<u8>, SendError> {
    let mut body = Vec::new();
    loop {
        let line = read_line(r)?;
        let size = line.split(';').next().unwrap_or("").trim();
        let size = u64::from_str_radix(size, 16).map_err(|_| malformed())?;
        if size == 0 {
            // Trailers are read and dropped.
            while !read_line(r)?.is_empty() {}
            return Ok(body);
        }
        read_n(r, size, &mut body)?;
        if !read_line(r)?.is_empty() {
            return Err(malformed());
        }
    }
}

/// Status line, headers (obs-fold joined with one space) and body. 1xx replies are skipped.
fn read_response<R: BufRead>(r: &mut R) -> Result<Response, SendError> {
    loop {
        let line = read_line(r)?;
        let mut parts = line.splitn(3, ' ');
        let version = parts.next().unwrap_or("");
        let code = parts.next().unwrap_or("");
        if !version.starts_with("HTTP/") || code.len() != 3 {
            return Err(malformed());
        }
        let status: u16 = code.parse().map_err(|_| malformed())?;
        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let line = read_line(r)?;
            if line.is_empty() {
                break;
            }
            if line.starts_with([' ', '\t']) {
                let last = headers.last_mut().ok_or_else(malformed)?;
                last.1.push(' ');
                last.1.push_str(line.trim());
                continue;
            }
            let (k, v) = line.split_once(':').ok_or_else(malformed)?;
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
        if (100..200).contains(&status) {
            continue;
        }
        let mut res = Response {
            status,
            headers,
            body: Vec::new(),
        };
        if status == 204 || status == 304 {
            return Ok(res);
        }
        let chunked = res
            .header("Transfer-Encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
        if chunked {
            res.body = read_chunked(r)?;
        } else if let Some(n) = res.header("Content-Length") {
            let n: u64 = n.parse().map_err(|_| malformed())?;
            let mut body = Vec::new();
            read_n(r, n, &mut body)?;
            res.body = body;
        } else {
            r.read_to_end(&mut res.body)?;
        }
        return Ok(res);
    }
}

/// Writes the whole request, then reads the whole response.
fn exchange<S: Read + Write>(mut s: S, req: &[u8]) -> Result<Response, SendError> {
    s.write_all(req)?;
    s.flush()?;
    read_response(&mut io::BufReader::new(s))
}

/// `CONNECT host:port`, then require a 200 status line (read byte by byte: nothing past the
/// blank line may be consumed, the TLS handshake follows on the same socket).
fn tunnel(s: &mut Timed, url: &Url, auth: &str) -> Result<(), SendError> {
    let host = if url.host.contains(':') {
        format!("[{}]", url.host)
    } else {
        url.host.clone()
    };
    let hp = format!("{host}:{}", url.port);
    s.write_all(format!("CONNECT {hp} HTTP/1.1\r\nHost: {hp}\r\n{auth}\r\n").as_bytes())?;
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if s.read(&mut b)? == 0 || head.len() > 16 * 1024 {
            return Err(malformed());
        }
        head.push(b[0]);
    }
    let text = String::from_utf8_lossy(&head);
    let status_line = text.lines().next().unwrap_or("").trim();
    if status_line.split(' ').nth(1) != Some("200") {
        return Err(SendError::Io(format!("proxy CONNECT: {status_line}")));
    }
    Ok(())
}

/// The TLS client configuration, built once.
pub(crate) struct Transport {
    tls: std::sync::Arc<rustls::ClientConfig>,
}

impl Transport {
    /// webpki-roots, plus the certificates in `extra_root_pem` (tests).
    pub(crate) fn new(extra_root_pem: Option<&[u8]>) -> Result<Transport, String> {
        let mut roots = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        if let Some(pem) = extra_root_pem {
            for cert in CertificateDer::pem_slice_iter(pem) {
                let cert = cert.map_err(|e| format!("extra root PEM: {e}"))?;
                roots
                    .add(cert)
                    .map_err(|e| format!("extra root PEM: {e}"))?;
            }
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls_graviola::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Transport { tls: Arc::new(tls) })
    }

    /// Sends `POST url` with `headers` (the caller's; `Host` and `Content-Length` are added
    /// here) and `body`, through `proxy` when given. Every read and write stops at `deadline`.
    pub(crate) fn send(
        &self,
        url: &Url,
        proxy: Option<&Url>,
        headers: &[(String, String)],
        body: &[u8],
        deadline: Instant,
    ) -> Result<Response, SendError> {
        let (dial_host, dial_port) = match proxy {
            Some(p) => (p.host.as_str(), p.port),
            None => (url.host.as_str(), url.port),
        };
        let mut sock = Timed {
            tcp: connect(dial_host, dial_port, deadline)?,
            deadline,
        };
        let path = if url.path.is_empty() {
            "/".to_string()
        } else if url.path.starts_with('?') {
            format!("/{}", url.path)
        } else {
            url.path.clone()
        };
        // An http target through a proxy uses the absolute form and carries the credentials
        // itself; an https target authenticates on the CONNECT request.
        let mut target = path;
        let mut auth = String::new();
        if let Some(p) = proxy {
            if url.https {
                tunnel(&mut sock, url, &proxy_auth(p))?;
            } else {
                target = format!("http://{}{target}", url.host_header());
                auth = proxy_auth(p);
            }
        }
        let mut req = format!("POST {target} HTTP/1.1\r\nHost: {}\r\n", url.host_header());
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str(&auth);
        req.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ));
        let mut bytes = req.into_bytes();
        bytes.extend_from_slice(body);
        if url.https {
            let name = ServerName::try_from(url.host.clone())
                .map_err(|e| SendError::Io(format!("invalid server name {:?}: {e}", url.host)))?;
            let conn = rustls::ClientConnection::new(Arc::clone(&self.tls), name)
                .map_err(|e| SendError::Io(e.to_string()))?;
            exchange(rustls::StreamOwned::new(conn, sock), &bytes)
        } else {
            exchange(sock, &bytes)
        }
    }
}
