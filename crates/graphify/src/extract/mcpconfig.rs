//! Port of `mcpconfig.go`: MCP server configs. Owner: the extractors-and-CLI stream; filled by
//! graphify-rs-ihc.17.

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use gostd::json::{self, DecodeState, Error as JsonError, RawMessage, Unmarshal};
use gostd::regexp::Regexp;
use gostd::strings::trim_space;

use super::extract::Result;
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Node, file_of, line, rel as relation_of};
use crate::security::sanitize_label;

/// Go `mcpMaxServers`: a generous cap; flags pathological configs.
const MCP_MAX_SERVERS: usize = 200;

/// Go `extract.IsMCPConfigPath`: `.mcp.json`, `claude_desktop_config.json`, `mcp.json` or
/// `mcp_servers.json`, matched case-sensitively on the basename.
pub fn is_mcp_config_path(name: &str) -> bool {
    matches!(
        gostd::filepath::base(name).as_str(),
        ".mcp.json" | "claude_desktop_config.json" | "mcp.json" | "mcp_servers.json"
    )
}

/// Go `map[string]json.RawMessage`.
type RawMap = BTreeMap<String, RawMessage>;

/// Go `extractMCPConfig`: the file node, one `mcp_server` node per entry of the `mcpServers`
/// map, and global `mcp_command`, `mcp_package` and `env_var` nodes. Env var names only, never
/// their values. A malformed file, or one with no server map, gives an empty result.
pub(crate) fn extract_mcp_config(rel: &str, src: &[u8]) -> Result {
    let Ok(doc) = json::unmarshal::<RawMap>(src) else {
        return Result::default();
    };
    let Some(servers) = mcp_servers_map(&doc) else {
        return Result::default();
    };

    let file_id = make_id(&[rel]);
    let rel_f = file_of(rel);
    let mut res = Result::default();
    res.nodes.push(Node {
        id: file_id.clone(),
        label: gostd::filepath::base(rel),
        file_type: "code".to_string(),
        source_file: rel.to_string(),
        source_location: "L1".to_string(),
        ..Node::default()
    });
    let mut seen_node: HashSet<String> = HashSet::from([file_id.clone()]);
    let mut seen_edge: HashSet<String> = HashSet::new();
    let stem = rel;

    let mut add_node = |res: &mut Result, id: &str, label: &str| {
        if id.is_empty() || !seen_node.insert(id.to_string()) {
            return;
        }
        res.nodes.push(Node {
            id: id.to_string(),
            label: sanitize_label(label),
            file_type: "code".to_string(),
            source_file: rel.to_string(),
            source_location: "L1".to_string(),
            ..Node::default()
        });
    };
    let mut add_edge = |res: &mut Result, src_id: &str, tgt_id: &str, relation: &str| {
        if src_id.is_empty() || tgt_id.is_empty() || src_id == tgt_id {
            return;
        }
        if !seen_edge.insert(format!("{src_id}\x00{tgt_id}\x00{relation}")) {
            return;
        }
        res.edges.push(Edge {
            source: src_id.to_string(),
            target: tgt_id.to_string(),
            relation: relation_of(relation),
            confidence: Confidence::EXTRACTED,
            source_file: rel_f,
            source_location: line(1),
            ..Edge::default()
        });
    };

    // Stable iteration: JSON object order is non-deterministic, so sort names (Go `sortedKeys`:
    // the map's keys without "", sorted; a BTreeMap is).
    let mut server_count = 0;
    for (name, raw) in servers.iter().filter(|(k, _)| !k.is_empty()) {
        if server_count >= MCP_MAX_SERVERS {
            break;
        }
        let Ok(spec) = json::unmarshal::<McpServerSpec>(&raw.0) else {
            continue; // skip non-object / malformed server entries silently
        };
        server_count += 1;
        let server_id = make_id(&[stem, "mcp_server", name]);
        add_node(&mut res, &server_id, name);
        add_edge(&mut res, &file_id, &server_id, "contains");

        let cmd = trim_space(&spec.command);
        if !cmd.is_empty() {
            let cmd_id = make_id(&["mcp_command", cmd]);
            add_node(&mut res, &cmd_id, cmd);
            add_edge(&mut res, &server_id, &cmd_id, "references");
        }
        let pkg = detect_package_from_args(&spec.args);
        if !pkg.is_empty() {
            let pkg_id = make_id(&["mcp_package", &pkg]);
            add_node(&mut res, &pkg_id, &pkg);
            add_edge(&mut res, &server_id, &pkg_id, "references");
        }
        // ONLY env var KEYS: values may hold secrets and are never read. Go ranges the map
        // (`mcpconfig.go:113`, random order); the keys are sorted here (D-001).
        for env_name in spec.env.keys().filter(|k| !k.is_empty()) {
            let env_id = make_id(&["env_var", env_name]);
            add_node(&mut res, &env_id, env_name);
            add_edge(&mut res, &server_id, &env_id, "requires_env");
        }
    }
    res
}

/// Go `mcpServerSpec`: the part of an `mcpServers` entry the extractor reads.
#[derive(Default)]
struct McpServerSpec {
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl Unmarshal for McpServerSpec {
    const GO_TYPE: &'static str = "extract.mcpServerSpec";

    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> std::result::Result<(), JsonError> {
        d.read_struct(&["command", "args", "env"], |d, key| match key {
            "command" => d.value(&mut self.command),
            "args" => d.value(&mut self.args),
            _ => d.value(&mut self.env),
        })
    }
}

/// Go's anonymous `struct { Servers map[string]json.RawMessage `json:"servers"` }`.
#[derive(Default)]
struct NestedServers {
    servers: Option<RawMap>,
}

impl Unmarshal for NestedServers {
    const GO_TYPE: &'static str =
        "struct { Servers map[string]json.RawMessage \"json:\\\"servers\\\"\" }";

    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> std::result::Result<(), JsonError> {
        d.read_struct(&["servers"], |d, _| d.value(&mut self.servers))
    }
}

/// Go `mcpServersMap`: the top-level `mcpServers` map, else the nested
/// `{"mcp": {"servers": {}}}` one; `None` when neither is there (a Go nil map).
fn mcp_servers_map(doc: &RawMap) -> Option<RawMap> {
    if let Some(raw) = doc.get("mcpServers")
        && let Ok(Some(m)) = json::unmarshal::<Option<RawMap>>(&raw.0)
    {
        return Some(m);
    }
    if let Some(raw) = doc.get("mcp")
        && let Ok(nested) = json::unmarshal::<NestedServers>(&raw.0)
        && nested.servers.is_some()
    {
        return nested.servers;
    }
    None
}

/// Go `npmPkgRe`: an npm scoped name, optionally `@version`.
static NPM_PKG_RE: LazyLock<Regexp> = LazyLock::new(|| {
    Regexp::must_compile(r"^@[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*(?:@[\w.\-+]+)?$")
});

/// Go `pyMCPPkgRe`: a Python `mcp-` / `-mcp` package id.
static PY_MCP_PKG_RE: LazyLock<Regexp> = LazyLock::new(|| {
    Regexp::must_compile(r"^[a-z0-9][a-z0-9._-]*-mcp(?:-[a-z0-9._-]+)?$|^mcp-[a-z0-9][a-z0-9._-]*$")
});

/// Go `argFlagRe`.
static ARG_FLAG_RE: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"^-{1,2}\w"));

/// Go `detectPackageFromArgs`: the first arg that looks like an npm or PyPI package id (an npm
/// `@version` dropped), else "". Flags (`-y`, `--tz=UTC`) are skipped.
pub(crate) fn detect_package_from_args(args: &[String]) -> String {
    for raw in args {
        let arg = trim_space(raw);
        if arg.is_empty() || ARG_FLAG_RE.match_string(arg) {
            continue;
        }
        if NPM_PKG_RE.match_string(arg) {
            return strip_version(arg).to_string();
        }
        if PY_MCP_PKG_RE.match_string(arg) {
            return arg.to_string();
        }
    }
    String::new()
}

/// Go `stripVersion`: drops the `@version` suffix of an npm package id, keeping a leading
/// `@scope`.
pub(crate) fn strip_version(pkg: &str) -> &str {
    if let Some(rest) = pkg.strip_prefix('@') {
        return match rest.find('@') {
            Some(i) => &pkg[..i + 1],
            None => pkg,
        };
    }
    match pkg.find('@') {
        Some(i) => &pkg[..i],
        None => pkg,
    }
}

#[cfg(test)]
#[path = "tests/mcpconfig.rs"]
mod mcpconfig_tests;

#[cfg(test)]
#[path = "tests/upstream_mcp_ingest.rs"]
mod upstream_mcp_ingest_tests;
