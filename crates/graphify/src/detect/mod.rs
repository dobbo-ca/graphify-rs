//! Port of graphify-go `internal/detect`: the corpus walk, ignore files and sensitive files.
//!
//! Shared API (plan Task 1): the signatures are fixed; the bodies come with graphify-rs-ihc.14
//! (plan Task 13).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod detect;
mod gitignore;

pub use detect::{
    Error, SUPPORTED_EXTENSIONS, WalkReport, collect_files, collect_files_report,
    collect_manifests, go_lossy, is_sensitive, shebang_ext,
};

#[cfg(test)]
mod tests;
