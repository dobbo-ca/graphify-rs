//! Port of graphify-go `skills`: the agent-host skill files `graphify install` writes.

/// The Claude Code skill that teaches an agent to query the graph instead of grepping
/// (graphify-go `skills/graphify/SKILL.md`, a Go `//go:embed`). Installed by `graphify install`.
pub const GRAPHIFY: &str = include_str!("graphify/SKILL.md");

#[cfg(test)]
mod tests;
