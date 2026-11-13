//! Port of `install.go`: `install [--uninstall]`. Filled by graphify-rs-ihc.21.

use std::ffi::OsString;
use std::path::Path;

use super::{Error, Out};

/// Go `cmdInstall`.
pub fn cmd_install(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `cmdInstall` with `os.UserHomeDir()` replaced by `home`, so a test can give it a TempDir
/// (Go's `t.Setenv("HOME", ...)`; `std::env::set_var` is unsafe in edition 2024).
/// `cmd_install` is this with `$HOME`; the usage check comes before any write.
fn install_in(home: &Path, args: &[OsString], stdout: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

#[cfg(test)]
#[path = "tests/install.rs"]
mod install_tests;
