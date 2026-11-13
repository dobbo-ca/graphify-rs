//! Proxy selection from the environment: a port of golang.org/x/net v0.55.0
//! http/httpproxy/proxy.go (the copy vendored in Go 1.27 std).

use std::net::IpAddr;

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
        // proxy.go:84 getEnvAny; `Env::get` already treats an empty value as unset.
        let any = |upper: &str, lower: &str| {
            env.get(upper)
                .or_else(|| env.get(lower))
                .unwrap_or("")
                .to_string()
        };
        Config {
            http_proxy: any("HTTP_PROXY", "http_proxy"),
            https_proxy: any("HTTPS_PROXY", "https_proxy"),
            no_proxy: any("NO_PROXY", "no_proxy"),
            cgi: env.get("REQUEST_METHOD").is_some(),
        }
    }

    /// Go `ProxyFunc()(url)`: the proxy for `url`, or None.
    pub(crate) fn proxy_for(&self, url: &Url) -> Result<Option<Url>, String> {
        // proxy.go:114 proxyForURL
        let raw = if url.https {
            &self.https_proxy
        } else {
            &self.http_proxy
        };
        let proxy = parse_proxy(raw);
        if !url.https && self.cgi && !matches!(proxy, Ok(None)) {
            return Err(
                "refusing to use HTTP_PROXY value in CGI environment; see golang.org/s/cgihttpproxy"
                    .to_string(),
            );
        }
        // proxy.go:134: NO_PROXY/loopback win over a bad proxy scheme (Go connects directly).
        match proxy {
            Ok(None) => Ok(None),
            _ if !self.use_proxy(&canonical_addr(url)) => Ok(None),
            p => p,
        }
    }

    /// Go `useProxy(canonicalAddr(url))`, for the ported `TestUseProxy` table.
    pub(crate) fn use_proxy(&self, addr: &str) -> bool {
        // proxy.go:153 useProxy, with `init`'s NO_PROXY parsing (proxy.go:186) done per call:
        // Go builds the matchers once, then returns false when any of them matches.
        if addr.is_empty() {
            return true;
        }
        let Some((host, port)) = split_host_port(addr) else {
            return false;
        };
        if host == "localhost" {
            return false;
        }
        let ip = host.parse::<IpAddr>().ok().map(|ip| ip.to_canonical());
        if ip.is_some_and(|ip| ip.is_loopback()) {
            return false;
        }
        let host = host.trim().to_lowercase();
        for entry in self.no_proxy.split(',') {
            let entry = entry.trim().to_lowercase();
            if entry.is_empty() {
                continue;
            }
            if entry == "*" {
                return false;
            }
            // IPv4/CIDR, IPv6/CIDR
            if let Some((net, bits)) = parse_cidr(&entry) {
                if ip.is_some_and(|ip| cidr_contains(net, bits, ip)) {
                    return false;
                }
                continue;
            }
            // IPv4:port, [IPv6]:port
            let (phost, pport) = match split_host_port(&entry) {
                // No host part: malformed, ignored.
                Some(("", _)) => continue,
                Some(hp) => hp,
                None => (entry.as_str(), ""),
            };
            // IPv4, IPv6
            if let Ok(pip) = phost.parse::<IpAddr>() {
                if ip == Some(pip.to_canonical()) && (pport.is_empty() || pport == port) {
                    return false;
                }
                continue;
            }
            // domain.com, .domain.com, *.domain.com, each with an optional :port.
            // foo.com matches bar.foo.com; .foo.com and *.foo.com match subdomains only.
            // ponytail: proxy.go runs the entry and the host through idnaASCII (punycode);
            // ASCII passes through there unchanged and no endpoint here is non-ASCII.
            let phost = match phost.strip_prefix('*') {
                Some(rest) if rest.starts_with('.') => rest,
                _ => phost,
            };
            let match_host = !phost.starts_with('.');
            let dom = if match_host {
                format!(".{phost}")
            } else {
                phost.to_string()
            };
            // domainMatch.match: never for an IP host.
            if ip.is_none()
                && (host.ends_with(&dom) || (match_host && host == phost))
                && (pport.is_empty() || pport == port)
            {
                return false;
            }
        }
        true
    }
}

/// proxy.go:101 parseProxy. A value without scheme or host is retried as `http://<value>`.
/// D-007: a scheme other than `http` is an error (Go supports https and socks5).
fn parse_proxy(s: &str) -> Result<Option<Url>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    let full = match s.split_once("://") {
        Some((scheme, rest))
            if !scheme.is_empty() && !rest.is_empty() && !rest.starts_with('/') =>
        {
            if !scheme.eq_ignore_ascii_case("http") {
                return Err(format!(
                    "unsupported proxy scheme {}",
                    scheme.to_ascii_lowercase()
                ));
            }
            s.to_string()
        }
        _ => format!("http://{s}"),
    };
    // An unparsable proxy is ignored, as Go ignores a parseProxy error in init.
    Ok(Url::parse(&full).ok())
}

/// proxy.go:223 canonicalAddr: `host:port`, IPv6 hosts in brackets.
fn canonical_addr(url: &Url) -> String {
    if url.host.contains(':') {
        format!("[{}]:{}", url.host, url.port)
    } else {
        format!("{}:{}", url.host, url.port)
    }
}

/// Go `net.SplitHostPort`, enough for NO_PROXY entries and `canonical_addr`.
fn split_host_port(s: &str) -> Option<(&str, &str)> {
    if let Some(rest) = s.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = after.strip_prefix(':')?;
        return (!port.contains(':')).then_some((host, port));
    }
    let (host, port) = s.rsplit_once(':')?;
    (!host.contains(':')).then_some((host, port))
}

/// Go `net.ParseCIDR`: the address and the prefix length.
fn parse_cidr(s: &str) -> Option<(IpAddr, u32)> {
    let (addr, bits) = s.split_once('/')?;
    let addr = addr.parse::<IpAddr>().ok()?;
    let bits = bits.parse::<u32>().ok()?;
    let max = if addr.is_ipv4() { 32 } else { 128 };
    (bits <= max).then_some((addr, bits))
}

fn cidr_contains(net: IpAddr, bits: u32, ip: IpAddr) -> bool {
    fn num(ip: IpAddr) -> (u128, u32) {
        match ip {
            IpAddr::V4(a) => (u128::from(u32::from(a)), 32),
            IpAddr::V6(a) => (u128::from(a), 128),
        }
    }
    let (n, nlen) = num(net);
    let (i, ilen) = num(ip);
    nlen == ilen && (n ^ i).checked_shr(nlen - bits).unwrap_or(0) == 0
}
