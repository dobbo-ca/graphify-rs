//! Proxy selection from the environment: a port of golang.org/x/net v0.55.0
//! http/httpproxy/proxy.go (the copy vendored in Go 1.27 std).

use crate::env::Env;
use crate::http::Url;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Config {
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String,
    /// `REQUEST_METHOD` is set (CGI): `HTTP_PROXY` is refused.
    pub cgi: bool,
}

impl Config {
    /// Go `FromEnvironment`: uppercase name first, then lowercase.
    pub(crate) fn from_env(env: &Env) -> Config {
        todo!("graphify-rs-u2o.7")
    }

    /// Go `ProxyFunc()(url)`: the proxy for `url`, or None.
    pub(crate) fn proxy_for(&self, url: &Url) -> Result<Option<Url>, String> {
        todo!("graphify-rs-u2o.7")
    }

    /// Go `useProxy(canonicalAddr(url))`, for the ported `TestUseProxy` table.
    pub(crate) fn use_proxy(&self, addr: &str) -> bool {
        todo!("graphify-rs-u2o.7")
    }
}
