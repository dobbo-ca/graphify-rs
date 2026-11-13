//! Go 1.27.4 `path/filepath/match.go` (unix): `Match` and `Glob`. Call site: graphify-go
//! `cargo.go:223` (`Glob`).
#![allow(unused_variables)]

use std::fmt;
use std::path::{Path, PathBuf};

/// Go `filepath.ErrBadPattern`: `syntax error in pattern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadPattern;

impl fmt::Display for BadPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("syntax error in pattern")
    }
}

impl std::error::Error for BadPattern {}

/// Go `filepath.Match`.
pub fn match_pattern(pattern: &str, name: &str) -> Result<bool, BadPattern> {
    todo!("graphify-rs-aa6.15")
}

/// Go `filepath.Glob`: names are matched as bytes, results are sorted by bytes.
pub fn glob(pattern: &Path) -> Result<Vec<PathBuf>, BadPattern> {
    todo!("graphify-rs-aa6.15")
}
