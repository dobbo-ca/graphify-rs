//! Port of `sigpipe_unix.go` and `sigpipe_other.go`: a downstream reader that closes the pipe
//! early is not a failure (exit 0). Rust binaries start with SIGPIPE ignored, so a write to a
//! closed pipe returns EPIPE; `run` maps it to exit 0 (plan hazard "EPIPE -> exit 0"). Filled by
//! graphify-rs-ihc.20.

use std::io;

/// Go `isBrokenPipe`: whether `e` is EPIPE, raw or with a path (`*os.PathError`). Always false
/// off unix (Go `sigpipe_other.go`).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn is_broken_pipe(e: &io::Error) -> bool {
    todo!("graphify-rs-ihc.20")
}

#[cfg(all(test, unix))]
#[path = "tests/brokenpipe.rs"]
mod brokenpipe_tests;
