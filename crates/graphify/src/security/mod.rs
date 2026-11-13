//! Port of graphify-go `internal/security`.
//!
//! Shared API (plan Task 1): the signatures are fixed; the bodies come with graphify-rs-ihc.6 (plan Task 5).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod security;

pub use security::*;

#[cfg(test)]
mod tests;
