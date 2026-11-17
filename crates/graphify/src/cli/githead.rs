//! Port of `githead.go`, `githead_unix.go` and `githead_other.go`: the commit SHA of a tree,
//! read from the repo files, with `git rev-parse HEAD` only when unsure. The owner and device
//! checks are `#[cfg(unix)]` (Go `githead_unix.go`); elsewhere `owned_by_me` is false and
//! `same_device` true (Go `githead_other.go`). Filled by graphify-rs-ihc.20.

use std::path::Path;

/// Go `gitEnv`: the variables that move git's repo discovery or ref storage.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) const GIT_ENV: [&str; 12] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_REFERENCE_BACKEND",
    "GIT_TEST_ASSUME_DIFFERENT_OWNER",
];

/// Go `gitHead`: the current commit SHA of the repo at `root`, or `""`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn git_head(root: &Path) -> String {
    todo!("graphify-rs-ihc.20")
}

/// Go `readGitHead`: `(sha, ok)`; `ok` is false when git must decide. It is [`read_git_head_with`]
/// over the process environment.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn read_git_head(root: &Path) -> (String, bool) {
    todo!("graphify-rs-ihc.20")
}

/// [`read_git_head`] with `env_set(k)` saying whether a [`GIT_ENV`] variable is set, so a test
/// gives it an environment without Go's `t.Setenv` (process-global; `set_var` is `unsafe` in
/// edition 2024).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn read_git_head_with(root: &Path, env_set: &dyn Fn(&str) -> bool) -> (String, bool) {
    todo!("graphify-rs-ihc.20")
}

/// Go `configNeedsGit`: config keys that change ref or repo layout.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn config_needs_git(cfg: &[u8]) -> bool {
    todo!("graphify-rs-ihc.20")
}

#[cfg(all(test, unix))]
#[path = "tests/githead.rs"]
mod githead_tests;
