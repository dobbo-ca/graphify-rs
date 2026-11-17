//! Port of graphify-go `cmd/graphify/outdir_test.go` (9 tests). Each changes the working
//! directory or sets `GRAPHIFY_OUT`, so each runs the binary with that directory and
//! environment. The asserts on private functions are made through the commands: `outDir()`
//! (`TestOutDirHonoursEnv`) by which graph `explain` loads, `scanRoot()`
//! (`TestScanRootIgnoresForeignMarker`) by where `update` writes, and `updateRoot` plus
//! `watchTick` (`TestWatchTickIdleAfterRedirect`) by `update` from the subdirectory followed by an
//! idle `update` of the same root (`watchTick` is cli-maint's, graphify-rs-ihc.21).

mod common;

use std::path::Path;

use common::{TempDir, ok_in, read, run_in, s, write};

const ALPHA: &str = r#"{"nodes":[{"id":"alpha","label":"alpha"}],"links":[]}"#;

fn exists(p: &Path) -> bool {
    std::fs::metadata(p).is_ok()
}

// Read commands find the repo's graph from a nested subdirectory.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_out_dir_walks_up_from_subdir() {
    let dir = TempDir::new();
    write(&dir.join("graphify-out/graph.json"), ALPHA);
    let sub = dir.join("internal/deep");
    std::fs::create_dir_all(&sub).expect("mkdir");
    ok_in(&sub, &["explain", "alpha"], &[]);
}

// GRAPHIFY_OUT overrides both the upward walk and the cwd-local default.
#[test]
// cli test (ihc.20): needs query
#[ignore = "graphify-rs-ihc.13"]
fn test_out_dir_honours_env() {
    let dir = TempDir::new();
    write(&dir.join("graphify-out/graph.json"), ALPHA);
    let alt = dir.join("elsewhere");
    write(
        &alt.join("graph.json"),
        r#"{"nodes":[{"id":"beta","label":"beta"}],"links":[]}"#,
    );
    let env = [("GRAPHIFY_OUT", s(&alt))];
    ok_in(dir.path(), &["explain", "beta"], &env);
    let r = run_in(dir.path(), &["explain", "alpha"], &env);
    assert_ne!(
        r.code,
        Some(0),
        "explain alpha = ok, want miss (env graph has no alpha)"
    );
}

// `update` with no path argument refreshes the graph the read commands load rather than building
// a stray second one rooted at the cwd.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_update_from_subdir_updates_repo_graph() {
    let dir = TempDir::new();
    dir.write("a.go", "package a\n\nfunc Alpha() {}\n");
    ok_in(dir.path(), &["build"], &[]);
    let root = read(&dir.join("graphify-out/.graphify_root"));
    assert!(!root.is_empty(), ".graphify_root is empty");

    let sub = dir.join("nested");
    std::fs::create_dir_all(&sub).expect("mkdir");
    ok_in(&sub, &["update"], &[]);
    assert!(
        !exists(&sub.join("graphify-out")),
        "update from a subdirectory built a stray graphify-out under the cwd"
    );
}

// A .graphify_root recording a tree that does not contain the cwd is ignored rather than
// redirecting the scan (and clobbering) that unrelated tree.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_scan_root_ignores_foreign_marker() {
    let dir = TempDir::new();
    let victim = TempDir::new();
    dir.write("a.go", "package a\n\nfunc Alpha() {}\n");
    ok_in(dir.path(), &["build"], &[]);
    write(
        &dir.join("graphify-out/.graphify_root"),
        &format!("{}\n", s(victim.path())),
    );
    ok_in(dir.path(), &["update"], &[]);
    assert!(
        !exists(&victim.join("graphify-out")),
        "update wrote a graphify-out into the tree named by the foreign marker"
    );
    // Go asserts scanRoot() == dir: the update scanned dir, so it recorded dir.
    assert_eq!(
        read(&dir.join("graphify-out/.graphify_root")),
        format!("{}\n", s(dir.path())),
        "scan root"
    );
}

// `build` writes under the tree it scanned instead of walking up and overwriting the repo's
// graph with a subtree-only one.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_build_from_subdir_keeps_subdir_out() {
    let dir = TempDir::new();
    dir.write("a.go", "package a\n\nfunc Alpha() {}\n");
    dir.write("nested/b.go", "package b\n\nfunc Beta() {}\n");
    let sub = dir.join("nested");
    ok_in(dir.path(), &["build"], &[]);
    let before = read(&dir.join("graphify-out/graph.json"));
    ok_in(&sub, &["build"], &[]);
    let after = read(&dir.join("graphify-out/graph.json"));
    assert!(
        before == after,
        "build from a subdirectory overwrote the parent tree's graph"
    );
    assert!(
        exists(&sub.join("graphify-out/graph.json")),
        "build from a subdirectory did not write its own graphify-out"
    );
}

// An explicit path inside the recorded scan root updates the repo graph instead of forking a
// subdir one.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_update_explicit_subdir_redirects_to_root() {
    for env_on in [false, true] {
        let dir = TempDir::new();
        let out_tmp = TempDir::new();
        dir.write("sub/a.go", "package a\n\nfunc Alpha() {}\n");
        dir.write("other/b.go", "package b\n\nfunc Beta() {}\n");
        let sub = dir.join("sub");
        let out = if env_on {
            out_tmp.join("out")
        } else {
            dir.join("graphify-out")
        };
        let env_val = if env_on {
            s(&out).to_string()
        } else {
            String::new()
        };
        let env = [("GRAPHIFY_OUT", env_val.as_str())];
        ok_in(dir.path(), &["build", "."], &env);
        if env_on {
            ok_in(dir.path(), &["update", "sub"], &env);
        } else {
            ok_in(&sub, &["update", "."], &env);
        }
        assert!(
            !exists(&sub.join("graphify-out")),
            "env={env_on}: stray sub/graphify-out created"
        );
        let g = std::fs::read_to_string(out.join("graph.json")).unwrap_or_default();
        assert!(
            g.contains("Beta"),
            "env={env_on}: sibling symbol lost from graph"
        );
    }
}

// `update .` never widens to the parent of GRAPHIFY_OUT when no trusted marker exists.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_update_dot_with_sibling_out_keeps_root() {
    for marker in ["", "."] {
        let base = TempDir::new();
        let repo = base.join("repo");
        base.write("repo/a.go", "package a\n\nfunc Alpha() {}\n");
        base.write("unrelated/u.go", "package u\n\nfunc Foreign() {}\n");
        let out = base.join("out");
        let env = [("GRAPHIFY_OUT", s(&out))];
        ok_in(&repo, &["build", "."], &env);
        if marker.is_empty() {
            let _ = std::fs::remove_file(out.join(".graphify_root"));
        } else {
            write(&out.join(".graphify_root"), marker);
        }
        ok_in(&repo, &["update", "."], &env);
        let g = std::fs::read_to_string(out.join("graph.json")).unwrap_or_default();
        assert!(
            !g.contains("Foreign") && g.contains("Alpha"),
            "marker={marker:?}: update . scanned outside the repo"
        );
    }
}

// A committed .graphify_root naming an ancestor of the repository cannot widen an update above
// the work tree.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_scan_root_stays_inside_git_work_tree() {
    for explicit in [false, true] {
        let base = TempDir::new();
        let repo = base.join("repo");
        let sub = repo.join("sub");
        base.write("repo/a.go", "package a\n\nfunc Alpha() {}\n");
        base.write("repo/sub/b.go", "package b\n\nfunc Beta() {}\n");
        base.write("private/u.go", "package u\n\nfunc Foreign() {}\n");
        common::git(&repo, &["init"]);
        let env = [("GRAPHIFY_OUT", "")];
        ok_in(&repo, &["build", "."], &env);
        let out = repo.join("graphify-out");
        write(&out.join(".graphify_root"), "..");
        if explicit {
            ok_in(&sub, &["update", "."], &env);
        } else {
            ok_in(&repo, &["update"], &env);
        }
        assert!(
            !exists(&base.join("graphify-out")),
            "explicit={explicit}: update wrote a graph above the work tree"
        );
        let g = std::fs::read_to_string(out.join("graph.json")).unwrap_or_default();
        assert!(
            !g.contains("Foreign") && g.contains("Beta"),
            "explicit={explicit}: update scanned above the work tree"
        );
    }
}

// watch polls the redirected root, so an unchanged tree reads as idle. Go: `updateRoot(".",
// true)` from sub gives dir, `cmdUpdate([dir])`, then `watchTick(dir)` is false. Here: `update .`
// from sub (the redirect), then a second `update` of dir reparses and removes nothing and writes
// no stray sub/graphify-out.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_watch_tick_idle_after_redirect() {
    let dir = TempDir::new();
    dir.write("sub/a.go", "package a\n\nfunc Alpha() {}\n");
    dir.write("b.go", "package b\n\nfunc Beta() {}\n");
    let sub = dir.join("sub");
    let env = [("GRAPHIFY_OUT", "")];
    ok_in(dir.path(), &["build", "."], &env);
    ok_in(&sub, &["update", "."], &env);
    assert!(
        !exists(&sub.join("graphify-out")),
        "stray sub/graphify-out created"
    );
    let r = ok_in(dir.path(), &["update", s(dir.path())], &env);
    assert!(
        r.stdout.contains("(0 reparsed, 2 reused, 0 removed)"),
        "idle tree: {:?}",
        r.stdout
    );
}
