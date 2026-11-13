//! Port of `main.go`: the command dispatch and the build, update and query-side commands.
//! Owner: the extractors-and-CLI stream; filled by graphify-rs-ihc.20 (plan Task 20). The
//! `cmd_*` signatures are shared API.

use std::ffi::OsString;

use super::{Error, Out};

/// Go `cmdBuild`: `build [path] [--cargo] [--no-manifests] [--force] [--no-cluster]
/// [--semantic --backend X]`.
pub fn cmd_build(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdUpdate`: `update [path]` with the `build` flags.
pub fn cmd_update(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}
