//! Port of `hook_postcheckout_test.go` (1 test), then the upstream case of
//! `test_hook_chain_survives_skip.py` that holds for graphify-go v0.12.0.

use std::process::Command;

use super::POST_CHECKOUT_GUARDS;
use super::maint_util::{git_run, install_ok, text};
use crate::testutil::TempDir;

fn read_hook(repo: &TempDir, h: &str) -> String {
    let p = repo.join(&format!(".git/hooks/{h}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {h}: {e}"))
}

// The post-checkout hook must skip the rebuild when HEAD did not move or when only files were
// checked out; post-commit/post-merge stay unconditional.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_post_checkout_hook_guards() {
    let repo = TempDir::new();
    git_run(repo.path(), &["init"]);
    install_ok(repo.path());

    let got = read_hook(&repo, "post-checkout");
    assert!(
        got.contains(POST_CHECKOUT_GUARDS),
        "post-checkout is missing the guards:\n{got}"
    );
    for h in ["post-commit", "post-merge"] {
        let got = read_hook(&repo, h);
        assert!(!got.contains("exit 0"), "{h} should not be guarded:\n{got}");
    }

    // The generated script must at least be valid shell.
    let hook = repo.join(".git/hooks/post-checkout");
    let out = Command::new("/bin/sh")
        .arg("-n")
        .arg(&hook)
        .output()
        .expect("run /bin/sh");
    assert!(
        out.status.success(),
        "post-checkout is not valid shell: {}: {}",
        out.status,
        text(out.stderr)
    );
}

// ---- upstream `test_hook_chain_survives_skip.py` ----

// Go's hooks are whole files (no chaining), so only the last test holds: reinstall recognises the
// block and rewrites it in place, leaving the same file.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_the_block_is_still_recognised_and_updated_in_place() {
    let repo = TempDir::new();
    git_run(repo.path(), &["init"]);
    install_ok(repo.path());
    let first = read_hook(&repo, "post-commit");
    install_ok(repo.path());
    assert_eq!(read_hook(&repo, "post-commit"), first);
}
