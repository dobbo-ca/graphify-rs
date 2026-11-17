//! Port of `cargo.go`: the Cargo workspace pass (`--cargo`). Owner: the extractors-and-CLI
//! stream; filled by graphify-rs-ihc.17.

use std::fmt;
use std::path::Path;

use super::extract::Result;

/// An error of [`introspect_cargo`]; `Display` is Go's error text (a TOML `ParseError`, a
/// `filepath.Abs` or `Rel` error, or the `open` error of a missing root manifest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Go `extract.IntrospectCargo`: a `crate:<name>` node per workspace member with a
/// `[package].name`, and `crate_depends_on` edges for workspace-internal dependencies only. With
/// no `root/Cargo.toml` it takes the shallowest `Cargo.toml` that `detect::collect_manifests`
/// keeps. A TOML syntax error in any manifest is an error.
// graphify-rs-ihc.17: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn introspect_cargo(root: &Path) -> std::result::Result<Result, Error> {
    todo!("graphify-rs-ihc.17")
}

#[cfg(test)]
#[path = "tests/cargo.rs"]
mod cargo_tests;

#[cfg(test)]
#[path = "tests/cargo_rename.rs"]
mod cargo_rename_tests;

#[cfg(test)]
#[path = "tests/upstream_cargo_introspect.rs"]
mod upstream_cargo_introspect_tests;
