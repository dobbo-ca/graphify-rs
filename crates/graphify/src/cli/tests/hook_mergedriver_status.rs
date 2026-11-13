//! Port of `hook_mergedriver_status_test.go` (6 tests; its `gitRun` and `hookInstalled` helpers
//! live in `maint_util.rs`), then the `core.hooksPath` and worktree cases of upstream
//! `test_hooks.py` that hold for graphify-go v0.12.0.

use std::path::Path;

use super::maint_util::{
    git_run, hook_installed, install_ok, is_exec, status_ok, uninstall_ok, write_exec,
};
use super::{HOOK_MARKER, MERGE_ATTR_LINE, merge_driver_status, resolve_hooks_dir};
use crate::testutil::TempDir;

fn init(repo: &TempDir) {
    git_run(repo.path(), &["init"]);
}

fn utf8(p: &Path) -> &str {
    p.to_str().expect("UTF-8 path")
}

// mergeDriverStatus must report "not registered" before install and "registered" after a full
// hookInstall wires both the config key and the .gitattributes line (#1902).
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_merge_driver_status() {
    let repo = TempDir::new();
    init(&repo);

    let got = merge_driver_status(repo.path());
    assert_eq!(
        got, "not registered",
        "before install: merge_driver_status = {got:?}"
    );
    install_ok(repo.path());
    let got = merge_driver_status(repo.path());
    assert_eq!(
        got, "registered",
        "after install: merge_driver_status = {got:?}"
    );
}

// Uninstalling the merge driver must strip only the graphify line and preserve other
// pre-existing .gitattributes entries (#1902).
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_preserves_other_attributes() {
    let repo = TempDir::new();
    init(&repo);
    let attr_path = repo.write(".gitattributes", b"*.md text\n");

    install_ok(repo.path());
    uninstall_ok(repo.path());

    let data = std::fs::read_to_string(&attr_path)
        .unwrap_or_else(|e| panic!(".gitattributes should still exist after uninstall: {e}"));
    assert!(
        data.contains("*.md text"),
        "pre-existing entry lost after uninstall:\n{data}"
    );
    assert!(
        !data.contains(MERGE_ATTR_LINE),
        "merge line still present after uninstall:\n{data}"
    );
}

// When the graphify line is the only entry, uninstall must delete the .gitattributes file rather
// than leave an empty one behind (#1902).
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_removes_empty_gitattributes() {
    let repo = TempDir::new();
    init(&repo);
    let attr_path = repo.join(".gitattributes");

    install_ok(repo.path());
    uninstall_ok(repo.path());
    assert!(
        !attr_path.exists(),
        ".gitattributes should be deleted when the merge line was its only entry"
    );
}

// In-repo core.hooksPath is honoured; out-of-repo is refused and flagged inactive.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_install_honours_hooks_path() {
    let repo = TempDir::new();
    init(&repo);
    git_run(repo.path(), &["config", "core.hooksPath", ".husky"]);
    install_ok(repo.path());
    assert!(
        hook_installed(&repo.join(".husky")) && !hook_installed(&repo.join(".git/hooks")),
        "in-repo hooksPath not honoured"
    );

    let out = TempDir::new();
    let repo2 = TempDir::new();
    init(&repo2);
    git_run(
        repo2.path(),
        &["config", "core.hooksPath", utf8(out.path())],
    );
    install_ok(repo2.path());
    assert!(
        !hook_installed(out.path()) && hook_installed(&repo2.join(".git/hooks")),
        "out-of-repo hooksPath not refused"
    );
    let (_, eff) = resolve_hooks_dir(repo2.path()).unwrap_or_default();
    let want = gostd::filepath::clean(utf8(out.path()));
    assert_eq!(utf8(&eff), want, "effective = {eff:?}, want {want:?}");
}

// A linked worktree has a .git file; install must use the common-dir hooks.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_install_linked_worktree() {
    let repo = TempDir::new();
    init(&repo);
    git_run(
        repo.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--allow-empty",
            "-m",
            "x",
        ],
    );
    let parent = TempDir::new();
    let wt = parent.join("wt");
    git_run(repo.path(), &["worktree", "add", utf8(&wt)]);
    install_ok(repo.path());
    install_ok(&wt);
    let hooks = repo.join(".git/hooks");
    assert!(
        hook_installed(&hooks),
        "worktree install did not write common-dir hooks"
    );
    let b = std::fs::read_to_string(hooks.join("post-commit")).unwrap_or_default();
    assert!(
        !b.contains(utf8(&wt)) && !b.contains(utf8(repo.path())),
        "hook bakes in an install path: {b}"
    );
}

// A hand-edited or relocated hook must show as out of date in status.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_status_out_of_date() {
    let repo = TempDir::new();
    init(&repo);
    install_ok(repo.path());
    let got = status_ok(repo.path());
    assert!(
        got.contains("post-commit: installed") && !got.contains("out of date"),
        "fresh install: {got:?}"
    );
    let p = repo.join(".git/hooks/post-commit");
    let stale =
        format!("#!/bin/sh\n{HOOK_MARKER}\nexec \"/nonexistent/graphify\" update . || true\n");
    write_exec(&p, stale.as_bytes());
    let got = status_ok(repo.path());
    assert!(
        got.contains("post-commit: installed (out of date: run graphify hook install)"),
        "stale hook: {got:?}"
    );
}

// ---- upstream `test_hooks.py` ----

// A legitimate POSIX core.hooksPath (Husky-style) must still install.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_posix_custom_hookspath_still_works() {
    let repo = TempDir::new();
    init(&repo);
    git_run(repo.path(), &["config", "core.hooksPath", ".husky"]);
    let msg = install_ok(repo.path());
    assert!(msg.contains("post-commit"), "{msg}");
    assert!(repo.join(".husky/post-commit").exists());
}

// #3869: an absolute core.hooksPath outside the repo must not be created. Go prints no
// "refusing hooks path" warning; the default hooks dir is used and named in the message.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hookspath_outside_repo_falls_back_to_default() {
    let base = TempDir::new();
    git_run(base.path(), &["init", "repo"]);
    let repo = base.join("repo");
    let outside = base.join("outside");
    git_run(&repo, &["config", "core.hooksPath", utf8(&outside)]);
    let msg = install_ok(&repo);
    assert!(!outside.exists());
    assert!(repo.join(".git/hooks/post-commit").exists());
    assert!(repo.join(".git/hooks/post-checkout").exists());
    assert!(msg.contains(".git") && msg.contains("hooks"), "{msg}");
}

// #3869: a relative core.hooksPath that resolves outside the repo is refused.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hookspath_relative_escape_falls_back_to_default() {
    let base = TempDir::new();
    git_run(base.path(), &["init", "repo"]);
    let repo = base.join("repo");
    git_run(&repo, &["config", "core.hooksPath", "../escaped"]);
    let (dir, _) = resolve_hooks_dir(&repo).unwrap_or_default();
    assert!(!base.join("escaped").exists());
    assert_eq!(dir, repo.join(".git").join("hooks"));
}

// A linked worktree's hooks live under the common git dir, outside the worktree root. That default
// must still be used when core.hooksPath is unset.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_linked_worktree_keeps_its_default_hooks_dir() {
    let base = TempDir::new();
    let main = base.join("main");
    git_run(base.path(), &["init", "main"]);
    git_run(&main, &["config", "user.email", "t@example.com"]);
    git_run(&main, &["config", "user.name", "t"]);
    base.write("main/f", b"x\n");
    git_run(&main, &["add", "f"]);
    git_run(&main, &["commit", "-m", "x"]);
    let wt = base.join("wt");
    git_run(&main, &["worktree", "add", "-q", utf8(&wt), "-b", "wt"]);
    let (got, _) = resolve_hooks_dir(&wt).unwrap_or_default();
    let got = std::fs::canonicalize(&got).unwrap_or(got);
    let common = std::fs::canonicalize(main.join(".git")).unwrap();
    assert!(got.starts_with(&common), "{got:?} is not within {common:?}");
    assert!(!wt.join(".git/hooks").exists());
}

// No core.hooksPath -> normal .git/hooks install, no rejection.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_default_hooks_dir_unaffected() {
    let repo = TempDir::new();
    init(&repo);
    install_ok(repo.path());
    assert!(repo.join(".git/hooks/post-commit").exists());
    assert!(is_exec(&repo.join(".git/hooks/post-commit")));
}
