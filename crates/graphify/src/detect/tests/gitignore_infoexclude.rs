//! Port of `gitignore_infoexclude_test.go` (3 tests).

use super::{collected, must_write};
use crate::testutil::TempDir;

/// Go `TestInfoExcludeHonored`: a dir excluded only by `.git/info/exclude` is pruned (#1810).
#[test]
fn test_info_exclude_honored() {
    let root = TempDir::new();
    must_write(&root.join(".git/info/exclude"), "buildcache/\n");
    must_write(&root.join("buildcache/x.py"), "x = 1");
    must_write(&root.join("app.py"), "def go(): pass");

    let got = collected(root.path());
    assert!(
        got.contains("app.py"),
        "app.py should be collected, got {got:?}"
    );
    assert!(
        !got.contains("buildcache/x.py"),
        "buildcache/x.py is excluded by .git/info/exclude and must be pruned, got {got:?}"
    );
}

/// Go `TestInfoExcludeWorktreeGitFile`: with a `.git` FILE (a linked worktree), `info/exclude`
/// is found through `gitdir:` and `commondir` (#1809).
#[test]
fn test_info_exclude_worktree_git_file() {
    let tmp = TempDir::new();
    let main = tmp.join("main");
    let wt = tmp.join("wt");

    must_write(&main.join(".git/info/exclude"), "excluded/\n");
    must_write(&main.join(".git/worktrees/foo/commondir"), "../..\n");

    let gitdir = main.join(".git/worktrees/foo");
    must_write(&wt.join(".git"), &format!("gitdir: {}\n", gitdir.display()));
    must_write(&wt.join("excluded/y.py"), "y = 1");
    must_write(&wt.join("keep.py"), "def go(): pass");

    let got = collected(&wt);
    assert!(
        got.contains("keep.py"),
        "keep.py should be collected, got {got:?}"
    );
    assert!(
        !got.contains("excluded/y.py"),
        "excluded/y.py (via shared .git/info/exclude) must be pruned, got {got:?}"
    );
}

/// Go `TestInfoExcludePrecedence`: `info/exclude` is the lowest precedence; a `.gitignore`
/// negation re-includes.
#[test]
fn test_info_exclude_precedence() {
    let root = TempDir::new();
    must_write(
        &root.join(".git/info/exclude"),
        "buildcache/\nsecretcache/\n",
    );
    must_write(&root.join(".gitignore"), "!buildcache/\n");
    must_write(&root.join("buildcache/x.py"), "x = 1");
    must_write(&root.join("secretcache/y.py"), "y = 1");

    let got = collected(root.path());
    assert!(
        got.contains("buildcache/x.py"),
        "a .gitignore negation must override .git/info/exclude, got {got:?}"
    );
    assert!(
        !got.contains("secretcache/y.py"),
        "secretcache is excluded by info/exclude (not negated) and must be pruned, got {got:?}"
    );
}
