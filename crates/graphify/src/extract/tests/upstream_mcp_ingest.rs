//! Upstream graphify v0.9.107 `tests/test_mcp_ingest.py`: 26 of 29 tests, asserting
//! graphify-go v0.12.0. The fixture `tests/fixtures/sample.mcp.json` is inlined as `SAMPLE`.
//!
//! Upstream tags each node with `metadata.mcp_kind`; Go's nodes carry no metadata, so a node's
//! kind is read from Go's id scheme (`mcpconfig.go:84-109`): `make_id(rel, "mcp_server", name)`,
//! `make_id("mcp_command", cmd)`, `make_id("mcp_package", pkg)`, `make_id("env_var", name)`.
//! Go returns an empty result with no `error` text for a malformed or serverless file, so those
//! cases assert the empty result only (pins Go v0.12.0; upstream also asserts the error -> P3).
//!
//! Not ported (3): `test_recognised_filenames_set_is_frozen` (Python's public `frozenset`),
//! `test_dispatch_routes_mcp_filename_to_mcp_extractor` and
//! `test_dispatch_does_not_reroute_generic_json` (Python's `_get_extractor`; Go's routing is
//! `file_from_bytes`, pinned by `json.rs` `test_extract_json_mcp_config_still_routed_to_mcp_
//! extractor`). Upstream calls `extract_mcp_config` directly, and so do these.
//! `test_is_mcp_config_path_*` run: `is_mcp_config_path` is on main.

use std::collections::BTreeSet;

use super::{extract_mcp_config, is_mcp_config_path};
use crate::extract::Result;
use crate::idutil::make_id;
use crate::model::Confidence;

/// Upstream `tests/fixtures/sample.mcp.json`.
const SAMPLE: &str = r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp/workspace"],
      "env": {
        "FILESYSTEM_ROOT": "/tmp/workspace"
      }
    },
    "fetch": {
      "command": "uvx",
      "args": ["mcp-server-fetch"]
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github@0.6.2"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_PLACEHOLDER_NOT_A_REAL_TOKEN"
      }
    },
    "time": {
      "command": "uvx",
      "args": ["mcp-server-time", "--local-timezone=UTC"]
    }
  }
}
"#;

/// Upstream `extract_mcp_config(FIXTURES / "sample.mcp.json")`.
fn sample() -> Result {
    extract_mcp_config("sample.mcp.json", SAMPLE.as_bytes())
}

/// Upstream `_label_by_kind`, the kind read from Go's id scheme. `rel` is the config's path.
fn label_by_kind<'a>(r: &'a Result, rel: &str, kind: &str) -> Vec<&'a str> {
    r.nodes
        .iter()
        .filter(|n| {
            let want = if kind == "mcp_server" {
                make_id(&[rel, "mcp_server", &n.label])
            } else {
                make_id(&[kind, &n.label])
            };
            n.id == want
        })
        .map(|n| n.label.as_str())
        .collect()
}

/// The id of the first node of `kind` (upstream `next(n["id"] for n ... if mcp_kind == kind)`).
fn id_by_kind(r: &Result, rel: &str, kind: &str) -> String {
    let label = label_by_kind(r, rel, kind)[0];
    r.nodes
        .iter()
        .find(|n| n.label == label)
        .map(|n| n.id.clone())
        .unwrap_or_default()
}

fn set(v: Vec<&str>) -> BTreeSet<&str> {
    v.into_iter().collect()
}

// ── Detection ────────────────────────────────────────────────────────────────

#[test]
fn test_is_mcp_config_path_recognises_known_filenames() {
    for name in [
        ".mcp.json",
        "claude_desktop_config.json",
        "mcp.json",
        "mcp_servers.json",
    ] {
        assert!(is_mcp_config_path(&format!("/some/dir/{name}")), "{name}");
    }
}

#[test]
fn test_is_mcp_config_path_rejects_generic_json() {
    assert!(!is_mcp_config_path("package.json"));
    assert!(!is_mcp_config_path("config.json"));
    assert!(!is_mcp_config_path("tsconfig.json"));
}

// ── Happy path with the bundled fixture ──────────────────────────────────────

#[test]
fn test_fixture_parses_without_error() {
    let r = sample();
    assert!(!r.nodes.is_empty());
    assert!(!r.edges.is_empty());
}

#[test]
fn test_fixture_emits_every_server() {
    let r = sample();
    let server_labels = set(label_by_kind(&r, "sample.mcp.json", "mcp_server"));
    assert_eq!(
        server_labels,
        BTreeSet::from(["filesystem", "fetch", "github", "time"])
    );
}

#[test]
fn test_fixture_emits_commands_as_global_nodes() {
    let r = sample();
    let commands = set(label_by_kind(&r, "sample.mcp.json", "mcp_command"));
    assert_eq!(commands, BTreeSet::from(["npx", "uvx"]));
}

#[test]
fn test_fixture_emits_npm_packages() {
    let r = sample();
    let packages = set(label_by_kind(&r, "sample.mcp.json", "mcp_package"));
    assert!(packages.contains("@modelcontextprotocol/server-filesystem"));
    assert!(packages.contains("@modelcontextprotocol/server-github"));
}

#[test]
fn test_fixture_emits_python_packages() {
    let r = sample();
    let packages = set(label_by_kind(&r, "sample.mcp.json", "mcp_package"));
    assert!(packages.contains("mcp-server-fetch"));
    assert!(packages.contains("mcp-server-time"));
}

#[test]
fn test_fixture_strips_version_from_npm_package() {
    let r = sample();
    let packages = set(label_by_kind(&r, "sample.mcp.json", "mcp_package"));
    // Source has "@modelcontextprotocol/server-github@0.6.2"
    assert!(packages.contains("@modelcontextprotocol/server-github"));
    assert!(!packages.contains("@modelcontextprotocol/server-github@0.6.2"));
}

#[test]
fn test_fixture_emits_env_var_names() {
    let r = sample();
    let env_vars = set(label_by_kind(&r, "sample.mcp.json", "env_var"));
    assert!(env_vars.contains("FILESYSTEM_ROOT"));
    assert!(env_vars.contains("GITHUB_PERSONAL_ACCESS_TOKEN"));
}

#[test]
fn test_env_var_values_never_appear_anywhere() {
    // The fixture has GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_PLACEHOLDER_NOT_A_REAL_TOKEN".
    // That string must not appear in any node field or edge field (Go has no metadata).
    let secret = "ghp_PLACEHOLDER_NOT_A_REAL_TOKEN";
    let r = sample();
    for n in &r.nodes {
        for v in [&n.id, &n.label, &n.source_file, &n.computed_name] {
            assert!(!v.contains(secret), "{n:?}");
        }
    }
    for e in &r.edges {
        assert!(
            !e.source.contains(secret)
                && !e.target.contains(secret)
                && !e.source_file.as_str().contains(secret),
            "{e:?}"
        );
    }
}

#[test]
fn test_filesystem_path_not_persisted_as_node() {
    // `args` contains "/tmp/workspace" — args are intentionally NOT persisted
    // as nodes/edges to avoid leaking local filesystem paths.
    let r = sample();
    for n in &r.nodes {
        assert!(!n.label.contains("/tmp/workspace"), "{n:?}");
    }
}

#[test]
fn test_fixture_relations_include_contains_references_requires_env() {
    let r = sample();
    let rels: BTreeSet<&str> = r.edges.iter().map(|e| e.relation.as_str()).collect();
    assert!(rels.contains("contains"));
    assert!(rels.contains("references"));
    assert!(rels.contains("requires_env"));
}

#[test]
fn test_no_dangling_edges() {
    let r = sample();
    let node_ids: BTreeSet<&str> = r.nodes.iter().map(|n| n.id.as_str()).collect();
    for e in &r.edges {
        assert!(node_ids.contains(e.source.as_str()), "{e:?}");
        assert!(node_ids.contains(e.target.as_str()), "{e:?}");
    }
}

// pins Go v0.12.0 (weight and confidence_score unset, 0, in the extract result:
// `mcpconfig.go:79`); upstream 0.9.107 writes 1.0 for both -> P3
#[test]
fn test_every_edge_has_confidence_score() {
    let r = sample();
    for e in &r.edges {
        assert_eq!(e.confidence, Confidence::EXTRACTED, "{e:?}");
        assert_eq!(e.confidence_score, 0.0, "{e:?}");
        assert_eq!(e.weight, 0.0, "{e:?}");
    }
}

// ── Cross-config emergent edges (global node IDs) ────────────────────────────

#[test]
fn test_same_command_collapses_to_one_node_across_configs() {
    // Two configs both use "npx". The mcp_command node should be shared.
    let a = ".mcp.json";
    let b = "subdir/claude_desktop_config.json";
    let r_a = extract_mcp_config(
        a,
        br#"{"mcpServers": {"a": {"command": "npx", "args": ["@scope/server-a"]}}}"#,
    );
    let r_b = extract_mcp_config(
        b,
        br#"{"mcpServers": {"b": {"command": "npx", "args": ["@scope/server-b"]}}}"#,
    );
    assert_eq!(
        id_by_kind(&r_a, a, "mcp_command"),
        id_by_kind(&r_b, b, "mcp_command")
    );
}

#[test]
fn test_same_env_var_collapses_to_one_node_across_configs() {
    // Two configs both require OPENAI_API_KEY. The env_var node ID must be identical.
    let a = ".mcp.json";
    let b = "sub/claude_desktop_config.json";
    let r_a = extract_mcp_config(
        a,
        br#"{"mcpServers": {"x": {"command": "npx", "args": ["@scope/x"], "env": {"OPENAI_API_KEY": "v1"}}}}"#,
    );
    let r_b = extract_mcp_config(
        b,
        br#"{"mcpServers": {"y": {"command": "uvx", "args": ["mcp-server-y"], "env": {"OPENAI_API_KEY": "v2"}}}}"#,
    );
    assert_eq!(
        id_by_kind(&r_a, a, "env_var"),
        id_by_kind(&r_b, b, "env_var")
    );
}

#[test]
fn test_same_server_name_in_different_dirs_does_not_collide() {
    // Two .mcp.json files in different dirs both declare a "filesystem" server.
    // The server nodes should NOT collide (stem-scoped via the path).
    let a = "proj_a/.mcp.json";
    let b = "proj_b/.mcp.json";
    let r_a = extract_mcp_config(
        a,
        br#"{"mcpServers": {"filesystem": {"command": "npx", "args": ["@scope/a"]}}}"#,
    );
    let r_b = extract_mcp_config(
        b,
        br#"{"mcpServers": {"filesystem": {"command": "npx", "args": ["@scope/b"]}}}"#,
    );
    assert_ne!(
        id_by_kind(&r_a, a, "mcp_server"),
        id_by_kind(&r_b, b, "mcp_server")
    );
}

// ── Error handling ───────────────────────────────────────────────────────────

// pins Go v0.12.0 (empty result, no error text); upstream 0.9.107 also reports
// "no mcpServers map" -> P3
#[test]
fn test_missing_mcp_servers_key() {
    let r = extract_mcp_config(".mcp.json", br#"{"unrelated": "shape"}"#);
    assert!(r.nodes.is_empty());
    assert!(r.edges.is_empty());
}

#[test]
fn test_nested_mcp_servers_shape() {
    // Some tools wrap the map: {"mcp": {"servers": {...}}}
    let r = extract_mcp_config(
        ".mcp.json",
        br#"{"mcp": {"servers": {"x": {"command": "node", "args": ["dist/index.js"]}}}}"#,
    );
    assert!(label_by_kind(&r, ".mcp.json", "mcp_server").contains(&"x"));
    assert!(label_by_kind(&r, ".mcp.json", "mcp_command").contains(&"node"));
}

// pins Go v0.12.0 (empty result, no error text); upstream 0.9.107 also reports "json error"
// -> P3
#[test]
fn test_malformed_json_returns_error() {
    let r = extract_mcp_config(".mcp.json", b"{not valid json");
    assert!(r.nodes.is_empty());
    assert!(r.edges.is_empty());
}

// pins Go v0.12.0 (extractMCPConfig has no size cap: the 2 MB arg is no package, and the
// server and its command are extracted); upstream 0.9.107 skips the file as "too large" -> P3
#[test]
fn test_oversize_file_skipped() {
    let payload = format!(
        r#"{{"mcpServers":{{"x":{{"command":"npx","args":["{}"]}}}}}}"#,
        "a".repeat(2_000_000)
    );
    let r = extract_mcp_config(".mcp.json", payload.as_bytes());
    assert_eq!(label_by_kind(&r, ".mcp.json", "mcp_server"), ["x"]);
    assert_eq!(label_by_kind(&r, ".mcp.json", "mcp_command"), ["npx"]);
    assert!(label_by_kind(&r, ".mcp.json", "mcp_package").is_empty());
}

// pins Go v0.12.0 (empty result, no error text); upstream 0.9.107 also reports "root is not an
// object" -> P3
#[test]
fn test_root_not_an_object() {
    let r = extract_mcp_config(".mcp.json", b"[1, 2, 3]");
    assert!(r.nodes.is_empty() && r.edges.is_empty());
}

#[test]
fn test_non_dict_server_entry_skipped() {
    let r = extract_mcp_config(
        ".mcp.json",
        br#"{"mcpServers": {"valid": {"command": "npx", "args": ["@scope/pkg"]}, "broken": ["this", "is", "not", "an", "object"]}}"#,
    );
    let server_labels = label_by_kind(&r, ".mcp.json", "mcp_server");
    assert!(server_labels.contains(&"valid"));
    assert!(!server_labels.contains(&"broken"));
}

// ── Edge case: package detection ─────────────────────────────────────────────

#[test]
fn test_package_detection_skips_flags() {
    // First arg is -y (flag); second is the package. Detection should skip the flag.
    let r = extract_mcp_config(
        ".mcp.json",
        br#"{"mcpServers": {"x": {"command": "npx", "args": ["-y", "@scope/server-x"]}}}"#,
    );
    assert!(label_by_kind(&r, ".mcp.json", "mcp_package").contains(&"@scope/server-x"));
}

#[test]
fn test_no_package_detected_for_unknown_arg_shape() {
    // Args don't look like any known package pattern => no package node.
    let r = extract_mcp_config(
        ".mcp.json",
        br#"{"mcpServers": {"x": {"command": "node", "args": ["./local-script.js", "--verbose"]}}}"#,
    );
    assert!(label_by_kind(&r, ".mcp.json", "mcp_package").is_empty());
}

#[test]
fn test_server_without_command_still_emits_server_node() {
    let r = extract_mcp_config(
        ".mcp.json",
        br#"{"mcpServers": {"x": {"args": ["@scope/server-x"]}}}"#,
    );
    assert!(label_by_kind(&r, ".mcp.json", "mcp_server").contains(&"x"));
    assert!(label_by_kind(&r, ".mcp.json", "mcp_command").is_empty());
}
