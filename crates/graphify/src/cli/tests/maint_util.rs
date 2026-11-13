//! Shared helpers of the cli-maint tests (`maintenance.rs`, `install.rs`, `save_result.rs`).

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use crate::cli::Error;

/// The `OsString` argument list of a command.
pub(crate) fn args(a: &[&str]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

/// A `hook <sub> <root>` argument list.
pub(crate) fn hook_args(sub: &str, root: &Path) -> Vec<OsString> {
    vec![OsString::from(sub), root.as_os_str().to_owned()]
}

/// Go `t.Fatalf("<what>: %v", err)`.
pub(crate) fn must<T>(what: &str, r: Result<T, Error>) -> T {
    r.unwrap_or_else(|e| panic!("{what}: {e}"))
}

/// A buffer's text.
pub(crate) fn text(buf: Vec<u8>) -> String {
    String::from_utf8(buf).expect("output is UTF-8")
}

/// Go `gitRun` (`hook_mergedriver_status_test.go`): `git -C dir <args>`, fatal on failure.
pub(crate) fn git_run(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}: {}",
        out.status,
        text(out.stderr)
    );
}

/// `git -C dir config --get key`: the value, or `None` when the key is unset.
pub(crate) fn git_config_get(dir: &Path, key: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "--get", key])
        .output()
        .expect("run git");
    out.status.success().then(|| text(out.stdout))
}

/// Go `hookInstalled`: whether `dir/post-commit` carries graphify's marker.
pub(crate) fn hook_installed(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("post-commit")).is_ok_and(|s| s.contains(super::HOOK_MARKER))
}

/// Go `os.WriteFile(path, data, 0o755)`.
pub(crate) fn write_exec(path: &Path, data: &[u8]) {
    std::fs::write(path, data).expect("write file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

/// The directory's entry names, sorted (Go `os.ReadDir`).
#[allow(clippy::disallowed_methods)] // a test listing, sorted below
pub(crate) fn names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir)? {
        v.push(e?.file_name().into_string().expect("UTF-8 name"));
    }
    v.sort();
    Ok(v)
}

/// Whether any execute bit of `path` is set (Go `fi.Mode()&0o111 != 0`).
pub(crate) fn is_exec(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).expect("stat").permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// `hook_install(root)`, fatal on error; what it printed to stdout.
pub(crate) fn install_ok(root: &Path) -> String {
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    must("hookInstall", super::hook_install(root, &mut out, &mut err));
    text(out)
}

/// `hook_uninstall(root)`, fatal on error; what it printed to stdout.
pub(crate) fn uninstall_ok(root: &Path) -> String {
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    must(
        "hookUninstall",
        super::hook_uninstall(root, &mut out, &mut err),
    );
    text(out)
}

/// `hook_status(root)`, fatal on error; what it printed (Go's `captureStdout`).
pub(crate) fn status_ok(root: &Path) -> String {
    let mut out = Vec::<u8>::new();
    must("hookStatus", super::hook_status(root, &mut out));
    text(out)
}
