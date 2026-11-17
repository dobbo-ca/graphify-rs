//! Port of `mcpconfig.go`: MCP server configs. Owner: the extractors-and-CLI stream; filled by
//! graphify-rs-ihc.17.

use super::extract::Result;

/// Go `extract.IsMCPConfigPath`: `.mcp.json`, `claude_desktop_config.json`, `mcp.json` or
/// `mcp_servers.json`, matched case-sensitively on the basename.
pub fn is_mcp_config_path(name: &str) -> bool {
    matches!(
        gostd::filepath::base(name).as_str(),
        ".mcp.json" | "claude_desktop_config.json" | "mcp.json" | "mcp_servers.json"
    )
}

/// Go `extractMCPConfig`.
pub(crate) fn extract_mcp_config(rel: &str, src: &[u8]) -> Result {
    todo!("graphify-rs-ihc.17")
}

/// Go `detectPackageFromArgs`: the first arg that looks like an npm or PyPI package id (an npm
/// `@version` dropped), else "". Flags (`-y`, `--tz=UTC`) are skipped.
// graphify-rs-ihc.17: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn detect_package_from_args(args: &[String]) -> String {
    todo!("graphify-rs-ihc.17")
}

/// Go `stripVersion`: drops the `@version` suffix of an npm package id, keeping a leading
/// `@scope`.
// graphify-rs-ihc.17: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn strip_version(pkg: &str) -> &str {
    todo!("graphify-rs-ihc.17")
}

#[cfg(test)]
#[path = "tests/mcpconfig.rs"]
mod mcpconfig_tests;

#[cfg(test)]
#[path = "tests/upstream_mcp_ingest.rs"]
mod upstream_mcp_ingest_tests;
