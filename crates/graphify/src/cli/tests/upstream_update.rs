//! Upstream graphify v0.9.107 cases of `build`/`update` that hold for graphify-go v0.12.0, run
//! in-process on an absolute root: `test_update_incremental.py` (8 of 17) and
//! `test_update_missing_graph.py` (1, both parameters). Upstream drives its Python
//! `_rebuild_code(root, incremental=True)` and spies on `extract()`; Go's `update` is incremental
//! and prints `(<n> reparsed, <n> reused, <n> removed)`, so each case asserts Go's line.
//! The cases that start with `update .` on a fresh tree run the binary (`cli_update.rs`).

use std::collections::BTreeSet;
use std::path::Path;

use super::core_util::{copy_tree, graph_nodes, str_field, write_tree};
use super::{cmd_build, cmd_update};
use crate::cli::maintenance::maint_util::args;
use crate::testutil::TempDir;

/// Upstream `_corpus`: four Python files, one of which imports across files.
fn corpus(root: &Path) {
    write_tree(
        root,
        &[
            ("src/a.py", "def fa():\n    return 1\n"),
            ("src/b.py", "def fb():\n    return 1\n"),
            ("src/c.py", "def fc():\n    return 1\n"),
            (
                "src/uses.py",
                "from src.a import fa\n\ndef use():\n    return fa()\n",
            ),
        ],
    );
}

fn run(f: super::core_util::Cmd, root: &Path, extra: &[&str]) -> String {
    let mut a = vec![root.to_str().expect("UTF-8 temp path")];
    a.extend_from_slice(extra);
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    if let Err(e) = f(&args(&a), &mut out, &mut err) {
        panic!("{a:?}: {e}");
    }
    String::from_utf8(out).expect("stdout is UTF-8")
}

/// Upstream fixture `built`: the corpus under `<tmp>/corpus`, built.
fn built(tmp: &TempDir) -> std::path::PathBuf {
    let root = tmp.join("corpus");
    corpus(&root);
    run(cmd_build, &root, &[]);
    root
}

/// Upstream `_sources`: the base names of the nodes' source files.
fn sources(root: &Path) -> BTreeSet<String> {
    graph_nodes(&root.join("graphify-out/graph.json"))
        .iter()
        .map(|n| str_field(n, "source_file"))
        .filter(|s| !s.is_empty())
        .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
        .collect()
}

/// Upstream `_graph_shape`: the sorted (id, label, source_file) nodes and (source, target,
/// relation) links.
fn graph_shape(root: &Path) -> (Vec<[String; 3]>, Vec<[String; 3]>) {
    let data = std::fs::read(root.join("graphify-out/graph.json")).expect("read graph.json");
    let v: gostd::json::Value = gostd::json::unmarshal(&data).expect("unmarshal graph.json");
    let gostd::json::Value::Object(g) = &v else {
        panic!("graph.json is not an object");
    };
    let rows = |key: &str, fields: [&str; 3]| -> Vec<[String; 3]> {
        let Some(gostd::json::Value::Array(a)) = g.get(key) else {
            return Vec::new();
        };
        let mut out: Vec<[String; 3]> = a
            .iter()
            .filter_map(|x| match x {
                gostd::json::Value::Object(o) => Some(fields.map(|f| str_field(o, f).to_string())),
                _ => None,
            })
            .collect();
        out.sort();
        out
    };
    (
        rows("nodes", ["id", "label", "source_file"]),
        rows("links", ["source", "target", "relation"]),
    )
}

/// Upstream `_cross_file_calls`: `<file>:<label> -> <file>:<label>` for every calls edge that
/// spans two files.
fn cross_file_calls(root: &Path) -> BTreeSet<String> {
    let data = std::fs::read(root.join("graphify-out/graph.json")).expect("read graph.json");
    let v: gostd::json::Value = gostd::json::unmarshal(&data).expect("unmarshal graph.json");
    let gostd::json::Value::Object(g) = &v else {
        panic!("graph.json is not an object");
    };
    let objects = |key: &str| -> Vec<&std::collections::BTreeMap<String, gostd::json::Value>> {
        match g.get(key) {
            Some(gostd::json::Value::Array(a)) => a
                .iter()
                .filter_map(|x| match x {
                    gostd::json::Value::Object(o) => Some(o),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    let labels: std::collections::BTreeMap<&str, (String, &str)> = objects("nodes")
        .into_iter()
        .map(|n| {
            let sf = str_field(n, "source_file");
            let base = if sf.is_empty() {
                "?"
            } else {
                sf.rsplit('/').next().unwrap_or(sf)
            };
            (
                str_field(n, "id"),
                (base.to_string(), str_field(n, "label")),
            )
        })
        .collect();
    let mut edges = BTreeSet::new();
    for e in objects("links") {
        if str_field(e, "relation") != "calls" {
            continue;
        }
        if let (Some(s), Some(d)) = (
            labels.get(str_field(e, "source")),
            labels.get(str_field(e, "target")),
        ) && s.0 != d.0
        {
            edges.insert(format!("{}:{} -> {}:{}", s.0, s.1, d.0, d.1));
        }
    }
    edges
}

/// Upstream `_add_caller_of_fa`.
fn add_caller_of_fa(root: &Path) {
    write_tree(
        root,
        &[(
            "src/uses.py",
            "from src.a import fa\n\ndef use():\n    return fa()\n\ndef added():\n    return fa()\n",
        )],
    );
}

// pins Go v0.12.0 ("(0 reparsed, 4 reused, 0 removed)"); upstream 0.9.107 asserts its extract()
// spy saw no batch -> P3
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_unchanged_copied_checkout_re_extracts_nothing() {
    let tmp = TempDir::new();
    let root = built(&tmp);
    let copy = tmp.join("copy");
    copy_tree(&root, &copy);
    let out = run(cmd_update, &copy, &[]);
    assert!(out.contains("(0 reparsed, 4 reused, 0 removed)"), "{out:?}");
}

// pins Go v0.12.0 ("(2 reparsed, 2 reused, 0 removed)"); upstream 0.9.107 asserts one extract()
// batch of a.py and uses.py -> P3
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_only_changed_files_are_re_extracted() {
    let tmp = TempDir::new();
    let root = built(&tmp);
    write_tree(
        &root,
        &[
            ("src/a.py", "def fa():\n    return 11\n"),
            (
                "src/uses.py",
                "from src.a import fa\n\ndef use():\n    return fa() + 1\n",
            ),
        ],
    );
    let out = run(cmd_update, &root, &[]);
    assert!(out.contains("(2 reparsed, 2 reused, 0 removed)"), "{out:?}");
}

// pins Go v0.12.0 ("(0 reparsed, 4 reused, 0 removed)"); upstream 0.9.107 asserts its extract()
// spy saw no batch -> P3
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_mtime_bump_with_identical_content_is_not_re_extracted() {
    let tmp = TempDir::new();
    let root = built(&tmp);
    // Upstream touches the file; rewriting the same bytes moves the mtime the same way.
    let b = root.join("src/b.py");
    let data = std::fs::read(&b).expect("read b.py");
    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(&b, data).expect("rewrite b.py");
    let out = run(cmd_update, &root, &[]);
    assert!(out.contains("(0 reparsed, 4 reused, 0 removed)"), "{out:?}");
}

// pins Go v0.12.0 ("(1 reparsed, 3 reused, 1 removed)"); upstream 0.9.107 asserts one extract()
// batch of new.py -> P3
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_added_file_is_re_extracted_and_deleted_file_drops_its_nodes() {
    let tmp = TempDir::new();
    let root = built(&tmp);
    assert!(
        sources(&root).contains("c.py"),
        "c.py not in the first build"
    );
    std::fs::remove_file(root.join("src/c.py")).expect("remove c.py");
    write_tree(&root, &[("src/new.py", "def brand_new():\n    return 5\n")]);
    let out = run(cmd_update, &root, &[]);
    assert!(out.contains("(1 reparsed, 3 reused, 1 removed)"), "{out:?}");
    let s = sources(&root);
    assert!(!s.contains("c.py"), "c.py still a source: {s:?}");
    assert!(s.contains("new.py"), "new.py not a source: {s:?}");
}

// pins Go v0.12.0 ("(4 reparsed, 0 reused, 0 removed)"); upstream 0.9.107 asserts its
// `incremental=True, force=True` run takes the full path -> P3
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_force_keeps_the_full_path() {
    let tmp = TempDir::new();
    let root = built(&tmp);
    let out = run(cmd_update, &root, &["--force"]);
    assert!(out.contains("(4 reparsed, 0 reused, 0 removed)"), "{out:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_incremental_second_run_matches_a_full_rebuild() {
    let edit = |root: &Path| {
        write_tree(
            root,
            &[
                ("src/a.py", "def renamed_fa():\n    return 11\n"),
                (
                    "src/uses.py",
                    "from src.a import renamed_fa\n\ndef use():\n    return renamed_fa()\n",
                ),
                ("src/new.py", "def brand_new():\n    return 5\n"),
            ],
        );
        std::fs::remove_file(root.join("src/c.py")).expect("remove c.py");
    };
    let tmp = TempDir::new();
    let root = built(&tmp);
    let other = tmp.join("other");
    corpus(&other);
    run(cmd_build, &other, &[]);

    edit(&root);
    edit(&other);
    run(cmd_update, &root, &[]);
    run(cmd_build, &other, &[]);
    assert_eq!(graph_shape(&root), graph_shape(&other));
}

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_cross_file_call_edge_survives_an_incremental_update() {
    let tmp = TempDir::new();
    let root = built(&tmp);
    assert!(
        cross_file_calls(&root).contains("uses.py:use() -> a.py:fa()"),
        "{:?}",
        cross_file_calls(&root)
    );
    add_caller_of_fa(&root);
    run(cmd_update, &root, &[]);
    let edges = cross_file_calls(&root);
    assert!(edges.contains("uses.py:added() -> a.py:fa()"), "{edges:?}");
    assert!(edges.contains("uses.py:use() -> a.py:fa()"), "{edges:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_cross_file_call_edge_survives_a_copied_checkout() {
    // graphify-out/.graphify_root of the copy still names the original tree.
    let tmp = TempDir::new();
    let root = built(&tmp);
    let copy = tmp.join("copy");
    copy_tree(&root, &copy);
    assert!(
        cross_file_calls(&copy).contains("uses.py:use() -> a.py:fa()"),
        "{:?}",
        cross_file_calls(&copy)
    );
    add_caller_of_fa(&copy);
    run(cmd_update, &copy, &[]);
    let edges = cross_file_calls(&copy);
    assert!(edges.contains("uses.py:added() -> a.py:fa()"), "{edges:?}");
}

// Upstream `test_update_missing_graph.py`: a missing graph.json is rebuilt with every source,
// with and without a changed source (its two parameters).
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_update_recreates_missing_graph_with_all_sources() {
    for change_one_source in [false, true] {
        let tmp = TempDir::new();
        let r = tmp.path();
        write_tree(
            r,
            &[
                ("first.py", "def first(): return 1\n"),
                ("second.py", "def second(): return 2\n"),
            ],
        );
        run(cmd_build, r, &["--no-cluster"]);
        std::fs::remove_file(tmp.join("graphify-out/graph.json")).expect("remove graph.json");
        if change_one_source {
            write_tree(r, &[("first.py", "def renamed(): return 123\n")]);
        }
        run(cmd_update, r, &["--no-cluster"]);
        let labels: BTreeSet<String> = graph_nodes(&tmp.join("graphify-out/graph.json"))
            .iter()
            .map(|n| str_field(n, "label").to_string())
            .collect();
        let want = if change_one_source {
            "renamed()"
        } else {
            "first()"
        };
        assert!(
            labels.contains("second()") && labels.contains(want),
            "change_one_source={change_one_source}: labels {labels:?}"
        );
    }
}
