//! Port of `maintenance_test.go` (6 tests), then the upstream cases of `test_hooks.py` that hold
//! for graphify-go v0.12.0 (the basic install, uninstall and status behaviour of the three hooks).

use std::path::Path;

use super::maint_util::{
    git_run, hook_args, install_ok, is_exec, must, status_ok, text, uninstall_ok, write_exec,
};
use super::{HOOK_MARKER, cmd_hook, hook_install, hook_script, watch_tick};
use crate::cli::main::{cmd_build, cmd_update};
use crate::testutil::TempDir;

const FOREIGN: &[u8] = b"#!/bin/sh\necho mine\n";

fn hook_file(root: &TempDir, h: &str) -> std::path::PathBuf {
    root.join(&format!(".git/hooks/{h}"))
}

fn init(root: &TempDir) {
    git_run(root.path(), &["init"]);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_install() {
    let root = TempDir::new();
    init(&root);
    // A pre-existing, non-graphify hook must not be clobbered.
    let foreign = hook_file(&root, "post-merge");
    write_exec(&foreign, FOREIGN);

    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    must(
        "hook install",
        cmd_hook(&hook_args("install", root.path()), &mut out, &mut err),
    );

    let pc = hook_file(&root, "post-commit");
    let data =
        std::fs::read_to_string(&pc).unwrap_or_else(|e| panic!("post-commit not written: {e}"));
    assert!(
        data.contains(HOOK_MARKER) && data.contains("update"),
        "post-commit missing marker or update call:\n{data}"
    );
    assert!(is_exec(&pc), "post-commit is not executable");
    assert_eq!(
        std::fs::read(&foreign).unwrap(),
        FOREIGN,
        "foreign post-merge hook was overwritten"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_install_rejects_non_repo() {
    let root = TempDir::new();
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    assert!(
        cmd_hook(&hook_args("install", root.path()), &mut out, &mut err).is_err(),
        "expected error installing hooks outside a git repo"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_uninstall() {
    let root = TempDir::new();
    init(&root);
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    must(
        "hook install",
        cmd_hook(&hook_args("install", root.path()), &mut out, &mut err),
    );
    // A foreign hook (no marker) must survive uninstall.
    let foreign = hook_file(&root, "post-merge");
    write_exec(&foreign, FOREIGN);

    must(
        "hook uninstall",
        cmd_hook(&hook_args("uninstall", root.path()), &mut out, &mut err),
    );
    assert!(
        !hook_file(&root, "post-commit").exists(),
        "post-commit hook was not removed"
    );
    assert_eq!(
        std::fs::read(&foreign).unwrap(),
        FOREIGN,
        "foreign post-merge hook was removed"
    );

    // Uninstall is idempotent: a second run on an already-clean repo succeeds.
    must(
        "second uninstall",
        cmd_hook(&hook_args("uninstall", root.path()), &mut out, &mut err),
    );
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_status() {
    let root = TempDir::new();
    init(&root);

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    must(
        "hook status",
        cmd_hook(&hook_args("status", root.path()), &mut out, &mut err),
    );
    let got = text(out);
    assert!(
        got.contains("post-commit: not installed"),
        "before install, want post-commit not installed:\n{got}"
    );

    must(
        "hook install",
        cmd_hook(
            &hook_args("install", root.path()),
            &mut Vec::<u8>::new(),
            &mut err,
        ),
    );
    let mut out = Vec::<u8>::new();
    must(
        "hook status",
        cmd_hook(&hook_args("status", root.path()), &mut out, &mut err),
    );
    let got = text(out);
    assert!(
        got.contains("post-commit: installed"),
        "after install, want post-commit installed:\n{got}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_unknown_subcommand() {
    let root = TempDir::new();
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    let bogus = hook_args("bogus", root.path());
    assert!(
        cmd_hook(&bogus, &mut out, &mut err).is_err(),
        "expected error for unknown hook subcommand"
    );
    assert!(
        cmd_hook(&[], &mut out, &mut err).is_err(),
        "expected error for missing hook subcommand"
    );
}

// Go calls `cmdBuild([]string{root})` and `cmdUpdate([]string{root})` (maintenance_test.go:140,
// :153). It needs `.go` extraction: the golang grammar (graphify-rs-ihc.24).
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_watch_tick() {
    let root = TempDir::new();
    let write = |rel: &str, content: &str| {
        root.write(rel, content.as_bytes());
    };
    let root_arg = [root.path().as_os_str().to_owned()];
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    write("a.go", "package p\n\nfunc A() {}\n");
    must("build", cmd_build(&root_arg, &mut out, &mut err));

    let changed = must("watch_tick", watch_tick(root.path()));
    assert!(!changed, "no edit: changed=true, want false");

    write("a.go", "package p\n\nfunc A() {}\n\nfunc B() {}\n");
    assert!(
        watch_tick(root.path()).unwrap_or(false),
        "edited file: want changed=true"
    );

    must("update", cmd_update(&root_arg, &mut out, &mut err));
    write("c.go", "package p\n\nfunc C() {}\n");
    assert!(
        watch_tick(root.path()).unwrap_or(false),
        "added file: want changed=true"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn hook_script_quotes_exe_path() {
    // Review Focus 5: the hook text is Go `strconv.Quote` (`%q`) of `os.Executable()`.
    // Quote rules for `/tmp/a b"/é<0x01>/graphify`: the space and `/` are printable ASCII and stay;
    // `"` becomes `\"`; U+00E9 is printable and stays as UTF-8; U+0001 becomes the four characters
    // `\x01`.
    let exe = "/tmp/a b\"/é\x01/graphify";
    assert!(exe.contains('\u{1}'), "the input holds a real 0x01 byte");
    let want = r#"exec "/tmp/a b\"/é\x01/graphify" update"#;
    assert!(
        !want.contains('\u{1}'),
        "the expected text holds the four characters \\x01, not a 0x01 byte"
    );
    for h in ["post-commit", "post-merge", "post-checkout"] {
        let got = hook_script(h, exe);
        assert!(
            got.contains(want),
            "hook_script({h:?}) = {got:?}, want it to contain {want:?}"
        );
    }
}

// ---- upstream `test_hooks.py` ----

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_creates_hook() {
    let root = TempDir::new();
    init(&root);
    let result = install_ok(root.path());
    let hook = hook_file(&root, "post-commit");
    assert!(hook.exists());
    assert!(read(&hook).contains(HOOK_MARKER));
    assert!(result.contains("installed"), "{result}");
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_is_executable() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    assert!(is_exec(&hook_file(&root, "post-commit")));
}

// Go rewrites its own whole-file hook on reinstall (it prints "installed git hooks" again, not
// "already installed"); the marker still appears once.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_idempotent() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    install_ok(root.path());
    let hook = hook_file(&root, "post-commit");
    assert_eq!(read(&hook).matches(HOOK_MARKER).count(), 1);
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_removes_hook() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    let result = uninstall_ok(root.path());
    assert!(!hook_file(&root, "post-commit").exists());
    assert!(result.contains("removed post-commit"), "{result}");
}

// Go prints nothing for a hook that is absent.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_no_hook() {
    let root = TempDir::new();
    init(&root);
    let result = uninstall_ok(root.path());
    assert!(!result.contains("removed"), "{result}");
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_status_installed() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    assert!(status_ok(root.path()).contains("installed"));
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_status_not_installed() {
    let root = TempDir::new();
    init(&root);
    assert!(status_ok(root.path()).contains("not installed"));
}

// Go's error text is `<root> is not a git repository` (upstream: RuntimeError "No git repository").
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_no_git_repo_raises() {
    let root = TempDir::new();
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    let e = hook_install(&root.join("not_a_repo"), &mut out, &mut err).unwrap_err();
    assert!(e.to_string().contains("is not a git repository"), "{e}");
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_creates_post_checkout_hook() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    let hook = hook_file(&root, "post-checkout");
    assert!(hook.exists());
    assert!(read(&hook).contains(HOOK_MARKER));
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_post_checkout_is_executable() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    assert!(is_exec(&hook_file(&root, "post-checkout")));
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_removes_post_checkout_hook() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    uninstall_ok(root.path());
    assert!(!hook_file(&root, "post-checkout").exists());
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_creates_post_merge_hook() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    let hook = hook_file(&root, "post-merge");
    assert!(hook.exists());
    assert!(read(&hook).contains(HOOK_MARKER));
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_post_merge_is_executable() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    assert!(is_exec(&hook_file(&root, "post-merge")));
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_removes_post_merge_hook() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    uninstall_ok(root.path());
    assert!(!hook_file(&root, "post-merge").exists());
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_status_shows_both_hooks() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    let result = status_ok(root.path());
    assert!(result.contains("post-commit"));
    assert!(result.contains("post-checkout"));
    assert!(result.matches("installed").count() >= 2, "{result}");
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_status_shows_post_merge_hook() {
    let root = TempDir::new();
    init(&root);
    install_ok(root.path());
    assert!(status_ok(root.path()).contains("post-merge: installed"));

    uninstall_ok(root.path());
    assert!(status_ok(root.path()).contains("post-merge: not installed"));
}
