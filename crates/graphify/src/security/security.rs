//! Port of `security.go`: the guards external input passes through: URL validation (SSRF),
//! graph-file path containment, the graph.json size cap and label sanitisation.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::path::Path;
use std::sync::LazyLock;

use gostd::filepath;
use gostd::regexp::Regexp;
use gostd::strconv::{parse_int, quote};
use gostd::strings::{to_lower, to_upper, to_valid_utf8, trim_space};

/// A `security` error; `Display` is Go's error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Go `MaxGraphFileBytes`: the default graph.json size cap, 512 MiB.
pub const MAX_GRAPH_FILE_BYTES: i64 = 512 * 1024 * 1024;

/// Go `maxLabelLen`.
const MAX_LABEL_LEN: usize = 256;

/// Go `maxGraphFileBytes`: the cap from `GRAPHIFY_MAX_GRAPH_BYTES`, read on every call.
pub(super) fn max_graph_file_bytes() -> i64 {
    match std::env::var("GRAPHIFY_MAX_GRAPH_BYTES") {
        Ok(v) => max_graph_file_bytes_from(Some(&v)),
        Err(std::env::VarError::NotPresent) => max_graph_file_bytes_from(None),
        // Go parses the bytes and fails on them: the default.
        Err(std::env::VarError::NotUnicode(_)) => MAX_GRAPH_FILE_BYTES,
    }
}

/// The parse of Go `maxGraphFileBytes` for the variable's value (`None`: unset): plain bytes or
/// an `MB`/`GB` suffix (any case, binary multipliers); unset, blank, unparseable or not positive
/// gives [`MAX_GRAPH_FILE_BYTES`]. Separate because tests cannot set the variable:
/// `std::env::set_var` is `unsafe` in edition 2024.
pub(super) fn max_graph_file_bytes_from(raw: Option<&str>) -> i64 {
    let raw = trim_space(raw.unwrap_or(""));
    if raw.is_empty() {
        return MAX_GRAPH_FILE_BYTES;
    }
    let upper = to_upper(raw);
    let (text, multiplier) = if let Some(t) = upper.strip_suffix("GB") {
        (trim_space(t), 1024 * 1024 * 1024)
    } else if let Some(t) = upper.strip_suffix("MB") {
        (trim_space(t), 1024 * 1024)
    } else {
        (upper.as_str(), 1)
    };
    match parse_int(text, 10, 64) {
        Ok(v) if v > 0 => v.wrapping_mul(multiplier),
        _ => MAX_GRAPH_FILE_BYTES,
    }
}

/// Go `security.ValidateURL`: rejects a URL that is not http or https, or whose host is a cloud
/// metadata name or resolves to a private, loopback, link-local or unspecified address.
pub fn validate_url(raw: &str) -> Result<(), Error> {
    let u = gostd::url::parse(raw)
        .map_err(|e| Error(format!("invalid URL {}: {e}", quote(raw.as_bytes()))))?;
    let scheme = to_lower(&u.scheme);
    if scheme != "http" && scheme != "https" {
        return Err(Error(format!(
            "blocked URL scheme {} - only http and https are allowed",
            quote(u.scheme.as_bytes())
        )));
    }
    let host = String::from_utf8(u.hostname().to_vec()).unwrap_or_default();
    if host.is_empty() {
        return Err(Error(format!("URL {} has no host", quote(raw.as_bytes()))));
    }
    let lower = to_lower(&host);
    if lower == "metadata.google.internal" || lower == "metadata.google.com" {
        return Err(Error(format!(
            "blocked cloud metadata endpoint {}",
            quote(host.as_bytes())
        )));
    }
    // ponytail: the system resolver (getaddrinfo) stands for Go's pure-Go net.LookupIP; nothing in
    // graphify-go calls ValidateURL outside this package, so its DNS error text never reaches output.
    let addrs = (host.as_str(), 0).to_socket_addrs().map_err(|e| {
        Error(format!(
            "DNS resolution failed for {}: {e}",
            quote(host.as_bytes())
        ))
    })?;
    for a in addrs {
        if is_blocked_ip(a.ip()) {
            return Err(Error(format!(
                "blocked private/internal IP {} (resolved from {})",
                a.ip(),
                quote(host.as_bytes())
            )));
        }
    }
    Ok(())
}

/// Go `isBlockedIP`: `IsPrivate`, `IsLoopback`, `IsLinkLocalUnicast`, `IsLinkLocalMulticast`,
/// `IsUnspecified`, or in 100.64.0.0/10. An IPv4-mapped IPv6 address is judged as IPv4, as Go's
/// `To4` does.
pub(super) fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => blocked_v4(v4),
        IpAddr::V6(v6) => blocked_v6(v6),
    }
}

fn blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    a == 10                                       // private
        || (a == 172 && b & 0xf0 == 16)           // private
        || (a == 192 && b == 168)                 // private
        || a == 127                               // loopback
        || (a == 169 && b == 254)                 // link-local unicast
        || (a == 224 && b == 0 && c == 0)         // link-local multicast
        || ip == Ipv4Addr::UNSPECIFIED
        || (a == 100 && b & 0xc0 == 64) // 100.64.0.0/10, carrier-grade NAT
}

fn blocked_v6(ip: Ipv6Addr) -> bool {
    let o = ip.octets();
    o[0] & 0xfe == 0xfc                        // private, fc00::/7
        || ip == Ipv6Addr::LOCALHOST
        || (o[0] == 0xfe && o[1] & 0xc0 == 0x80) // link-local unicast, fe80::/10
        || (o[0] == 0xff && o[1] & 0x0f == 0x02) // link-local multicast
        || ip == Ipv6Addr::UNSPECIFIED
}

/// Go `security.ValidateGraphPath`: resolves `path` and checks it stays inside `base` (which
/// must exist) and exists; returns the absolute path.
pub fn validate_graph_path(path: &str, base: &str) -> Result<String, Error> {
    let io = |e: std::io::Error| Error(e.to_string());
    let abs_base = filepath::abs(Path::new(base)).map_err(io)?;
    let abs_base_s = abs_base.to_string_lossy().into_owned();
    if !std::fs::metadata(&abs_base).is_ok_and(|m| m.is_dir()) {
        return Err(Error(format!(
            "graph base directory does not exist: {abs_base_s}"
        )));
    }
    let abs = filepath::abs(Path::new(path)).map_err(io)?;
    let abs_s = abs.to_string_lossy().into_owned();
    match filepath::rel(&abs_base_s, &abs_s) {
        Ok(rel) if rel != ".." && !rel.starts_with("../") => {}
        _ => {
            return Err(Error(format!(
                "path {} escapes the allowed directory {abs_base_s}",
                quote(path.as_bytes())
            )));
        }
    }
    if std::fs::metadata(&abs).is_err() {
        return Err(Error(format!("graph file not found: {abs_s}")));
    }
    Ok(abs_s)
}

/// Go `security.CheckGraphFileSize`: an error if `path` is larger than the cap
/// (`GRAPHIFY_MAX_GRAPH_BYTES`).
pub fn check_graph_file_size(path: &Path) -> Result<(), Error> {
    check_size(path, max_graph_file_bytes())
}

/// Go `CheckGraphFileSize` with the cap given; `Ok` when `path` cannot be stat'ed (its
/// existence is the caller's concern).
pub(super) fn check_size(path: &Path, limit: i64) -> Result<(), Error> {
    let Ok(m) = std::fs::metadata(path) else {
        return Ok(());
    };
    let size = i64::try_from(m.len()).unwrap_or(i64::MAX);
    if size > limit {
        return Err(Error(format!(
            "graph file {} is {size} bytes, exceeds {limit}-byte cap",
            path.display()
        )));
    }
    Ok(())
}

/// Go `controlChars`.
static CONTROL_CHARS: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"[\x00-\x1f\x7f]"));

/// Go `security.SanitizeLabel`: control characters removed, then cut to 256 bytes without
/// splitting a rune.
pub fn sanitize_label(s: &str) -> String {
    let s = CONTROL_CHARS.replace_all_string(s, "");
    if s.len() > MAX_LABEL_LEN {
        return to_valid_utf8(&s.as_bytes()[..MAX_LABEL_LEN], "");
    }
    s
}
