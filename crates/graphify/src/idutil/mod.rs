//! Port of graphify-go `internal/idutil`.
//!
//! Shared API (plan Task 1): the signatures are fixed; the bodies come with graphify-rs-ihc.4 (plan Task 3).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod idutil;

pub use idutil::*;
