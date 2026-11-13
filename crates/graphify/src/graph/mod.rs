//! Port of graphify-go `internal/graph`: assembles extraction output into the graph.

mod build;

pub use build::{build, build_parts};

#[cfg(test)]
mod tests;
