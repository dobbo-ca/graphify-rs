//! Port of graphify-go `internal/fsutil`.
//!
//! Shared API (plan Task 1): the signatures are fixed; the bodies come with graphify-rs-ihc.3 (plan Task 2).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod fsutil;

pub use fsutil::*;
