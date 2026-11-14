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
