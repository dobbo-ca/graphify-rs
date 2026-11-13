//! Port of `maintenance.go`: `hook` and `watch`. Filled by graphify-rs-ihc.21.

use std::ffi::OsString;

use super::{Error, Out};

/// Go `cmdHook`: `hook install|uninstall|status [path]`.
pub fn cmd_hook(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `cmdWatch`: `watch [path]` (`args[0]`, default `.`).
pub fn cmd_watch(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}
