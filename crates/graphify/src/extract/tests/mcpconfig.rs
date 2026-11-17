//! graphify-go `internal/extract/mcpconfig_test.go`, and the plan's added test
//! `mcp_env_sorted_d001` (D-001: Go ranges the `env` map in random order; Rust sorts the keys).
//! `TestIsMCPConfigPath` runs: `is_mcp_config_path` is on main.

use std::collections::BTreeMap;

use super::{detect_package_from_args, is_mcp_config_path, strip_version};
use crate::extract::{Result, file_from_bytes};
use crate::idutil::make_id;

#[test]
fn test_extract_mcp_config() {
    let src = br#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem@1.2.3", "/data"],
      "env": {"FS_TOKEN": "sk-secret-value"}
    },
    "fetch": {
      "command": "uvx",
      "args": ["mcp-server-fetch"]
    }
  }
}"#;
    let res = file_from_bytes(".mcp.json", src);

    let id2label: BTreeMap<&str, &str> = res
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let has_label = |l: &str| res.nodes.iter().any(|n| n.label == l);
    for want in [
        ".mcp.json",
        "filesystem",
        "fetch",
        "npx",
        "uvx",
        "@modelcontextprotocol/server-filesystem", // version stripped
        "mcp-server-fetch",
        "FS_TOKEN",
    ] {
        assert!(has_label(want), "missing node {want:?}");
    }

    // Env VALUE must never appear as a node label.
    assert!(
        !has_label("sk-secret-value"),
        "env value leaked into a node label"
    );

    let has = |src_label: &str, rel: &str, tgt_label: &str| {
        res.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src_label)
                && id2label.get(e.target.as_str()) == Some(&tgt_label)
        })
    };
    assert!(
        has(".mcp.json", "contains", "filesystem"),
        "expected .mcp.json --contains--> filesystem"
    );
    assert!(
        has("filesystem", "references", "npx"),
        "expected filesystem --references--> npx"
    );
    assert!(
        has(
            "filesystem",
            "references",
            "@modelcontextprotocol/server-filesystem"
        ),
        "expected filesystem --references--> package"
    );
    assert!(
        has("filesystem", "requires_env", "FS_TOKEN"),
        "expected filesystem --requires_env--> FS_TOKEN"
    );
    assert!(
        has("fetch", "references", "mcp-server-fetch"),
        "expected fetch --references--> mcp-server-fetch"
    );
}

/// Go `idOf`: the id of the first node labelled `label`, or "".
fn id_of<'a>(res: &'a Result, label: &str) -> &'a str {
    res.nodes
        .iter()
        .find(|n| n.label == label)
        .map_or("", |n| n.id.as_str())
}

// Two configs declaring the same command/package/env var share those global
// nodes (so an agent can ask "which configs use this package?"), while their
// server nodes stay stem-scoped and distinct.
#[test]
fn test_extract_mcp_config_global_nodes_shared() {
    let cfg = br#"{"mcpServers": {"fs": {"command": "npx", "args": ["@scope/pkg"], "env": {"TOK": "x"}}}}"#;
    let a = file_from_bytes("a/.mcp.json", cfg);
    let b = file_from_bytes("b/.mcp.json", cfg);

    assert!(
        id_of(&a, "npx") == id_of(&b, "npx") && !id_of(&a, "npx").is_empty(),
        "mcp_command node should be global (shared across configs)"
    );
    assert_eq!(
        id_of(&a, "@scope/pkg"),
        id_of(&b, "@scope/pkg"),
        "mcp_package node should be global"
    );
    assert_eq!(
        id_of(&a, "TOK"),
        id_of(&b, "TOK"),
        "env_var node should be global"
    );
    assert!(
        id_of(&a, "fs") != id_of(&b, "fs") && !id_of(&a, "fs").is_empty(),
        "mcp_server node should be stem-scoped (distinct per config)"
    );
}

// The nested {"mcp": {"servers": {...}}} shape some tools emit is also accepted.
#[test]
fn test_extract_mcp_config_nested_shape() {
    let src = br#"{"mcp": {"servers": {"time": {"command": "uvx", "args": ["mcp-server-time", "--local-timezone=UTC"]}}}}"#;
    let res = file_from_bytes("mcp.json", src);
    let labels: Vec<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(
        labels.contains(&"time") && labels.contains(&"uvx") && labels.contains(&"mcp-server-time"),
        "nested mcp.servers shape not extracted: {labels:?}"
    );
    // The option arg must not be mistaken for a package.
    assert!(
        !labels.contains(&"--local-timezone=UTC"),
        "option arg leaked as a package node"
    );
}

#[test]
fn test_extract_mcp_config_empty_results() {
    // Go ranges a map of these cases; the order does not matter.
    const CASES: [(&str, &str); 4] = [
        ("not json", "{not valid json"),
        ("no mcpServers", r#"{"other": {}}"#),
        ("mcpServers not map", r#"{"mcpServers": []}"#),
        ("root not object", "[1, 2, 3]"),
    ];
    for (name, body) in CASES {
        let res = file_from_bytes("mcp.json", body.as_bytes());
        assert!(
            res.nodes.is_empty() && res.edges.is_empty(),
            "{name}: expected empty result, got {} nodes / {} edges",
            res.nodes.len(),
            res.edges.len()
        );
    }
}

#[test]
fn test_detect_package_from_args() {
    const CASES: [(&[&str], &str); 7] = [
        (&["-y", "@org/pkg@1.2.3"], "@org/pkg"), // scoped npm, version stripped
        (&["some-mcp@2.0.0"], ""),               // py id pattern has no @version branch: no match
        (&["server-mcp"], "server-mcp"),         // py "-mcp" suffix
        (&["mcp-server-git"], "mcp-server-git"), // py "mcp-" prefix
        (&["-y", "--yes", "/abs/path"], ""),     // flags + bare path: no package
        (&["@scope/name"], "@scope/name"),       // scoped, no version
        (&[], ""),                               // no args
    ];
    for (args, want) in CASES {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let got = detect_package_from_args(&args);
        assert_eq!(
            got, want,
            "detectPackageFromArgs({args:?}) = {got:?}, want {want:?}"
        );
    }
}

#[test]
fn test_strip_version() {
    // Go ranges a map of these cases; the order does not matter.
    const CASES: [(&str, &str); 4] = [
        ("@scope/name", "@scope/name"),
        ("@scope/name@1.2.3", "@scope/name"),
        ("name", "name"),
        ("name@1.2.3", "name"),
    ];
    for (input, want) in CASES {
        let got = strip_version(input);
        assert_eq!(
            got, want,
            "stripVersion({input:?}) = {got:?}, want {want:?}"
        );
    }
}

#[test]
fn test_is_mcp_config_path() {
    let yes = [
        ".mcp.json",
        "claude_desktop_config.json",
        "mcp.json",
        "mcp_servers.json",
        "sub/dir/.mcp.json",
        "/abs/path/mcp_servers.json",
    ];
    for p in yes {
        assert!(
            is_mcp_config_path(p),
            "IsMCPConfigPath({p:?}) = false, want true"
        );
    }
    let no = [
        "config.json",
        "package.json",
        "mcp.yaml",
        "mymcp.json",
        "main.go",
    ];
    for p in no {
        assert!(
            !is_mcp_config_path(p),
            "IsMCPConfigPath({p:?}) = true, want false"
        );
    }
}

// Added (plan Task 16, D-001): Go ranges `spec.Env` (`mcpconfig.go:113`), so the order of the
// env_var nodes and requires_env edges is random in Go; Rust takes the keys sorted.
#[test]
fn mcp_env_sorted_d001() {
    let src = br#"{"mcpServers": {"s": {"command": "npx", "env": {"ZED": "1", "ALPHA": "2", "MID": "3"}}}}"#;
    let res = file_from_bytes(".mcp.json", src);
    let is_env = |id: &str, label: &str| id == make_id(&["env_var", label]);
    let env_nodes: Vec<&str> = res
        .nodes
        .iter()
        .filter(|n| is_env(&n.id, &n.label))
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(env_nodes, ["ALPHA", "MID", "ZED"], "env_var node order");
    let env_edges: Vec<&str> = res
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "requires_env")
        .map(|e| e.target.as_str())
        .collect();
    let want: Vec<String> = ["ALPHA", "MID", "ZED"]
        .iter()
        .map(|k| make_id(&["env_var", k]))
        .collect();
    assert_eq!(env_edges, want, "requires_env edge order");
}
