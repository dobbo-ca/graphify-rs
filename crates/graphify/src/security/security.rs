//! Port of `security.go`: URL, graph path and label checks.

use std::fmt;
use std::path::Path;

/// A `security` error; `Display` is Go's error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Go `security.ValidateURL`: http or https, and no loopback, private or link-local host.
pub fn validate_url(raw: &str) -> Result<(), Error> {
    todo!("graphify-rs-ihc.6")
}

/// Go `security.ValidateGraphPath`: `path` resolved and kept inside `base`; returns the
/// resolved path.
pub fn validate_graph_path(path: &str, base: &str) -> Result<String, Error> {
    todo!("graphify-rs-ihc.6")
}

/// Go `security.CheckGraphFileSize`: the file at `path` is no larger than the limit
/// (`GRAPHIFY_MAX_GRAPH_BYTES`).
pub fn check_graph_file_size(path: &Path) -> Result<(), Error> {
    todo!("graphify-rs-ihc.6")
}

/// Go `security.SanitizeLabel`: control bytes removed, cut to 256 bytes, invalid UTF-8 dropped.
pub fn sanitize_label(s: &str) -> String {
    todo!("graphify-rs-ihc.6")
}

/// Go `MaxGraphFileBytes`: the default graph.json size cap, 512 MiB.
pub const MAX_GRAPH_FILE_BYTES: i64 = 512 * 1024 * 1024;

/// Go `maxGraphFileBytes`: the cap from `GRAPHIFY_MAX_GRAPH_BYTES`, read on every call.
pub(super) fn max_graph_file_bytes() -> i64 {
    todo!("graphify-rs-ihc.6")
}

/// The parse of Go `maxGraphFileBytes` for the variable's value (`None`: unset). Separate
/// because tests cannot set the variable: `std::env::set_var` is `unsafe` in edition 2024.
pub(super) fn max_graph_file_bytes_from(raw: Option<&str>) -> i64 {
    todo!("graphify-rs-ihc.6")
}

/// Go `CheckGraphFileSize` with the cap given: an error if `path` is larger than `limit`;
/// `Ok` when `path` cannot be stat'ed.
pub(super) fn check_size(path: &Path, limit: i64) -> Result<(), Error> {
    todo!("graphify-rs-ihc.6")
}

/// Go `isBlockedIP`: private, loopback, link-local unicast or multicast, unspecified, or in
/// 100.64.0.0/10.
pub(super) fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    todo!("graphify-rs-ihc.6")
}
