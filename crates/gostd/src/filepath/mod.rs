//! Go 1.27.4 `path/filepath` (unix only): the lexical functions of `path/filepath/path.go` and
//! `path_unix.go`. Call sites: graphify-go's ~220 `path/filepath` uses (`Join`, `Dir`, `Base`,
//! `Ext`, `Abs`, `Rel`, `Clean`, `IsAbs`). `ToSlash`, `FromSlash` and `Separator` are the identity
//! and `'/'` on unix: no gostd function. Lexical rules work on `as_os_str().as_bytes()`.
#![allow(unused_variables)]

mod match_;
mod symlink;
mod walk;

pub use match_::{BadPattern, glob, match_pattern};
pub use symlink::eval_symlinks;
pub use walk::{DirEntry, Walk, walk_dir};

use std::fmt;
use std::path::{Path, PathBuf};

/// Go `Rel`'s error: `Rel: can't make <targ> relative to <base>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelError {
    pub base: String,
    pub targ: String,
}

impl fmt::Display for RelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rel: can't make {} relative to {}", self.targ, self.base)
    }
}

impl std::error::Error for RelError {}

/// Go `filepath.Clean`.
pub fn clean(p: &str) -> String {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Join`.
pub fn join(elems: &[&str]) -> String {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Dir`.
pub fn dir(p: &str) -> String {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Base`.
pub fn base(p: &str) -> String {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Ext`.
pub fn ext(p: &str) -> &str {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.IsAbs`.
pub fn is_abs(p: &str) -> bool {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Rel`.
pub fn rel(base: &str, targ: &str) -> Result<String, RelError> {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Abs` (`unixAbs`): the working directory joined with `p`, cleaned.
pub fn abs(p: &Path) -> std::io::Result<PathBuf> {
    todo!("graphify-rs-aa6.15")
}
