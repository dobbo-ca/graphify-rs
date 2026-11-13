//! Port of graphify-go `internal/detect`: the corpus walk, ignore files and sensitive files.
//!
//! Shared API (plan Task 1): the signatures are fixed; the bodies come with graphify-rs-ihc.14
//! (plan Task 13).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod detect;

pub use detect::{go_lossy, is_sensitive};
