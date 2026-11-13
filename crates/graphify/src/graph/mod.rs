//! Port of graphify-go `internal/graph`: assembles extraction output into the graph.
#![allow(dead_code, unused_variables)]

mod build;

pub use build::{build, build_parts};

#[cfg(test)]
mod tests;
