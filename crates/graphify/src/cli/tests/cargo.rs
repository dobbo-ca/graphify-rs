//! Port of `cargo_test.go` (2 tests).

use super::core_util::{read_graph, write_tree};
use super::{cmd_build, parse_build_args};
use crate::cli::maintenance::maint_util::args;
use crate::testutil::TempDir;

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_parse_build_args() {
    // (args, root, cargo, no_manifests, force, no_cluster)
    type Case = (
        &'static [&'static str],
        &'static str,
        bool,
        bool,
        bool,
        bool,
    );
    const CASES: [Case; 10] = [
        (&[], ".", false, false, false, false),
        (&["--cargo"], ".", true, false, false, false),
        (&["some/path"], "some/path", false, false, false, false),
        (
            &["some/path", "--cargo"],
            "some/path",
            true,
            false,
            false,
            false,
        ),
        (
            &["--cargo", "some/path"],
            "some/path",
            true,
            false,
            false,
            false,
        ),
        (&["--force"], ".", false, false, true, false),
        (
            &["some/path", "--cargo", "--force"],
            "some/path",
            true,
            false,
            true,
            false,
        ),
        (&["--no-manifests"], ".", false, true, false, false),
        (&["--no-cluster"], ".", false, false, false, true),
        (
            &["some/path", "--cargo", "--no-cluster"],
            "some/path",
            true,
            false,
            false,
            true,
        ),
    ];
    for (a, root, cargo, no_manifests, force, no_cluster) in CASES {
        let got = parse_build_args(&args(a));
        let want = (root.to_string(), cargo, no_manifests, force, no_cluster);
        assert_eq!(got, want, "parse_build_args({a:?})");
    }
}

// --cargo adds crate nodes/edges to graph.json and a plain build over the same corpus does not,
// so the pass stays opt-in.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_build_cargo_flag() {
    let root = TempDir::new();
    // A real Rust source file so CollectFiles finds a corpus; the crate edges come from the
    // manifests, independent of the source extractor.
    write_tree(
        root.path(),
        &[
            ("app/src/lib.rs", "pub fn run() {}\n"),
            ("core/src/lib.rs", "pub fn helper() {}\n"),
            ("Cargo.toml", "[workspace]\nmembers = [\"app\", \"core\"]\n"),
            (
                "app/Cargo.toml",
                "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\ncore = { path = \"../core\" }\nserde = \"1\"\n",
            ),
            (
                "core/Cargo.toml",
                "[package]\nname = \"core\"\nversion = \"0.1.0\"\n",
            ),
        ],
    );
    let graph_path = root.join("graphify-out/graph.json");
    let r = root.path().to_str().expect("UTF-8 temp path");
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());

    // Plain build: no crate nodes.
    if let Err(e) = cmd_build(&args(&[r]), &mut out, &mut err) {
        panic!("build: {e}");
    }
    assert!(
        !read_graph(&graph_path).contains("crate:app"),
        "plain build must not emit crate nodes"
    );

    // --cargo build: crate nodes and the internal edge appear.
    if let Err(e) = cmd_build(&args(&[r, "--cargo"]), &mut out, &mut err) {
        panic!("build --cargo: {e}");
    }
    let data = std::fs::read(&graph_path).expect("read graph");
    let g: gostd::json::Value = gostd::json::unmarshal(&data).expect("unmarshal graph");
    let gostd::json::Value::Object(g) = &g else {
        panic!("graph.json is not an object");
    };
    let list = |key: &str| -> Vec<std::collections::BTreeMap<String, String>> {
        let Some(gostd::json::Value::Array(a)) = g.get(key) else {
            return Vec::new();
        };
        a.iter()
            .filter_map(|v| match v {
                gostd::json::Value::Object(o) => Some(
                    o.iter()
                        .filter_map(|(k, v)| match v {
                            gostd::json::Value::String(s) => Some((k.clone(), s.clone())),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .collect()
    };
    let ids: Vec<String> = list("nodes")
        .into_iter()
        .filter_map(|mut n| n.remove("id"))
        .collect();
    let has = |id: &str| ids.iter().any(|i| i == id);
    assert!(
        has("crate:app") && has("crate:core"),
        "--cargo build missing crate nodes; have {ids:?}"
    );
    assert!(
        !has("crate:serde"),
        "registry dep serde leaked into the graph"
    );
    let found = list("links").iter().any(|l| {
        l.get("source").map(String::as_str) == Some("crate:app")
            && l.get("target").map(String::as_str) == Some("crate:core")
            && l.get("relation").map(String::as_str) == Some("crate_depends_on")
    });
    assert!(
        found,
        "--cargo build missing crate:app -crate_depends_on-> crate:core edge"
    );
}
