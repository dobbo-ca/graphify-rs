//! Port of `maintenance.go`: `hook` and `watch`. Filled by graphify-rs-ihc.21.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Error, Out};

/// Go `hookMarker`: tags hook scripts graphify wrote.
const HOOK_MARKER: &str = "# graphify-managed hook";

/// Go `mergeAttrLine`: wires `graphify-out/graph.json` to the union merge driver.
const MERGE_ATTR_LINE: &str = "graphify-out/graph.json merge=graphify";

/// Go `postCheckoutGuards`.
const POST_CHECKOUT_GUARDS: &str = "[ \"$1\" = \"$2\" ] && exit 0\n[ \"$3\" = \"0\" ] && exit 0\n";

/// Go `managedGitHooks`.
const MANAGED_GIT_HOOKS: [&str; 3] = ["post-commit", "post-merge", "post-checkout"];

/// Go `cmdHook`: `hook install|uninstall|status [path]`.
pub fn cmd_hook(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `cmdWatch`: `watch [path]` (`args[0]`, default `.`).
pub fn cmd_watch(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `resolveHooksDir`: `(dir, effective)`, where hooks are written and git's effective hooks dir.
fn resolve_hooks_dir(root: &Path) -> Result<(PathBuf, PathBuf), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `hookScript`: the exact text `hook_install` writes for hook `h`; `self_` is Go's `self`.
fn hook_script(h: &str, self_: &str) -> String {
    todo!("graphify-rs-ihc.21")
}

/// Go `hookInstall`.
fn hook_install(root: &Path, stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `hookUninstall`.
fn hook_uninstall(root: &Path, stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `hookStatus`.
fn hook_status(root: &Path, stdout: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `mergeDriverStatus`: `registered`, `partial` or `not registered`.
fn merge_driver_status(root: &Path) -> String {
    todo!("graphify-rs-ihc.21")
}

/// Go `watchTick`: whether any collected file differs from the cache on disk.
fn watch_tick(root: &Path) -> Result<bool, Error> {
    todo!("graphify-rs-ihc.21")
}

#[cfg(test)]
#[path = "tests/hook_mergedriver_status.rs"]
mod hook_mergedriver_status_tests;
#[cfg(test)]
#[path = "tests/hook_mergedriver.rs"]
mod hook_mergedriver_tests;
#[cfg(test)]
#[path = "tests/hook_postcheckout.rs"]
mod hook_postcheckout_tests;
#[cfg(test)]
#[path = "tests/maint_util.rs"]
pub(crate) mod maint_util;
#[cfg(test)]
#[path = "tests/maintenance.rs"]
mod maintenance_tests;
