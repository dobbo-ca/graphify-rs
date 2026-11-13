//! Port of graphify-go `internal/langfamily`.
//!
//! Shared API (plan Task 1): the signatures are fixed; the bodies come with graphify-rs-ihc.5 (plan Task 4).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod langfamily;

pub use langfamily::*;
