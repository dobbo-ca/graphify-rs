//! HTTP/1.1 over `std::net::TcpStream`, with TLS from rustls + rustls-graviola + webpki-roots,
//! and HTTP CONNECT proxies. One connection per request.

use std::time::Instant;

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
        todo!("graphify-rs-u2o.7")
    }

    /// `host` or `host:port` when the port is not the scheme's default (the `Host` header).
    pub(crate) fn host_header(&self) -> String {
        todo!("graphify-rs-u2o.7")
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

/// The TLS client configuration, built once.
pub(crate) struct Transport {
    tls: std::sync::Arc<rustls::ClientConfig>,
}

impl Transport {
    /// webpki-roots, plus the certificates in `extra_root_pem` (tests).
    pub(crate) fn new(extra_root_pem: Option<&[u8]>) -> Result<Transport, String> {
        todo!("graphify-rs-u2o.7")
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
        todo!("graphify-rs-u2o.7")
    }
}
