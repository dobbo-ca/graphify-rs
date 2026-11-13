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
