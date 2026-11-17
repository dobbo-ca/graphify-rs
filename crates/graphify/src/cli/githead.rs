//! Port of `githead.go`, `githead_unix.go` and `githead_other.go`: the commit SHA of a tree,
//! read from the repo files, with `git rev-parse HEAD` only when unsure. The owner and device
//! checks are `#[cfg(unix)]` (Go `githead_unix.go`); elsewhere `owned_by_me` is false and
//! `same_device` true (Go `githead_other.go`).

use std::fs::{self, Metadata};
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use gostd::filepath::{abs, dir, eval_symlinks, is_abs, join};
use gostd::strings::trim_space;

use crate::detect::go_lossy;

/// Go `gitEnv`: the variables that move git's repo discovery or ref storage.
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

/// A path as the Go string (plan R3).
fn path_str(p: &Path) -> String {
    go_lossy(p.as_os_str().as_encoded_bytes())
}

/// Go `gitHead`: the current commit SHA of the repo at `root`, or `""`.
pub(crate) fn git_head(root: &Path) -> String {
    let (sha, ok) = read_git_head(root);
    if ok {
        return sha;
    }
    match Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
    {
        Ok(out) if out.status.success() => trim_space(&go_lossy(&out.stdout)).to_string(),
        _ => String::new(),
    }
}

/// Go `readGitHead`: `(sha, ok)`; `ok` is false when git must decide. It is [`read_git_head_with`]
/// over the process environment.
pub(crate) fn read_git_head(root: &Path) -> (String, bool) {
    read_git_head_with(root, &|k| std::env::var_os(k).is_some())
}

/// [`read_git_head`] with `env_set(k)` saying whether a [`GIT_ENV`] variable is set, so a test
/// gives it an environment without Go's `t.Setenv` (process-global; `set_var` is `unsafe` in
/// edition 2024).
pub(crate) fn read_git_head_with(root: &Path, env_set: &dyn Fn(&str) -> bool) -> (String, bool) {
    let no = || (String::new(), false);
    if GIT_ENV.iter().any(|&k| env_set(k)) {
        return no();
    }
    // Without git, main stamps no commit.
    if !look_path_git() {
        return (String::new(), true);
    }
    let Ok(mut d) = phys_dir(root) else {
        return no();
    };
    let fi = match fs::metadata(&d) {
        Ok(fi) if fi.is_dir() => fi,
        _ => return no(),
    };
    loop {
        let (gitdir, found, ok) = git_dir_at(&d);
        if !ok {
            return no();
        }
        if found {
            return head_of(&d, &gitdir);
        }
        let parent = dir(&d);
        if parent == d {
            return (String::new(), true);
        }
        let Ok(pfi) = fs::metadata(&parent) else {
            return no();
        };
        if !same_device(&fi, &pfi) {
            // git stops at a mount point and fails.
            return (String::new(), true);
        }
        d = parent;
    }
}

/// Go `exec.LookPath("git")` succeeding: an executable regular file `git` in a `$PATH` entry,
/// found under an absolute directory (Go 1.19+ refuses one found through a relative entry).
fn look_path_git() -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for entry in go_lossy(path.as_encoded_bytes()).split(':') {
        let entry = if entry.is_empty() { "." } else { entry };
        let p = join(&[entry, "git"]);
        if is_executable(&p) {
            return is_abs(&p);
        }
    }
    false
}

/// Go `findExecutable` (`exec/lp_unix.go`): not a directory, some execute bit set.
#[cfg(unix)]
fn is_executable(p: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).is_ok_and(|m| !m.is_dir() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &str) -> bool {
    fs::metadata(p).is_ok_and(|m| !m.is_dir())
}

/// Go `physDir`: symlinks resolved, since git discovers from the physical cwd.
fn phys_dir(root: &Path) -> Result<String, ()> {
    let p = path_str(&eval_symlinks(root).map_err(|_| ())?);
    if is_abs(&p) {
        return Ok(p);
    }
    let wd = abs(Path::new(".")).map_err(|_| ())?;
    let wd = path_str(&eval_symlinks(&wd).map_err(|_| ())?);
    Ok(join(&[&wd, &p]))
}

/// Go `gitDirAt`: `(gitdir, found, ok)`, the git dir that `d` is the top of, as git discovery
/// finds it.
fn git_dir_at(d: &str) -> (String, bool, bool) {
    let no = || (String::new(), false, false);
    let dotgit = join(&[d, ".git"]);
    match fs::symlink_metadata(&dotgit) {
        Ok(fi) if fi.is_dir() => {
            if !looks_like_git_dir(&dotgit) {
                return no();
            }
            return (dotgit, true, true);
        }
        Ok(fi) if fi.is_file() => {
            let Ok(b) = fs::read(&dotgit) else {
                return no();
            };
            let s = go_lossy(&b);
            let Some(p) = s.trim_end_matches(['\r', '\n']).strip_prefix("gitdir: ") else {
                return no();
            };
            let p = if is_abs(p) {
                p.to_string()
            } else {
                join(&[d, p])
            };
            return (p, true, true);
        }
        Err(e) if e.kind() != ErrorKind::NotFound => return no(),
        Ok(_) => return no(),
        Err(_) => {}
    }
    // Bare repos and admin dirs: let git apply its config.
    match fs::symlink_metadata(join(&[d, "HEAD"])) {
        Err(e) if e.kind() == ErrorKind::NotFound => (String::new(), false, true),
        _ => no(),
    }
}

/// Go `fileExists`.
fn file_exists(p: &str) -> bool {
    fs::metadata(p).is_ok()
}

/// Go `looksLikeGitDir`.
fn looks_like_git_dir(d: &str) -> bool {
    ["HEAD", "objects", "refs"]
        .iter()
        .all(|n| file_exists(&join(&[d, n])))
}

/// Go `headOf`: HEAD of `gitdir`, which `top` (the work tree or bare dir) holds.
fn head_of(top: &str, gitdir: &str) -> (String, bool) {
    let no = || (String::new(), false);
    if !owned_by_me(top) || !owned_by_me(gitdir) {
        return no();
    }
    let mut common = gitdir.to_string();
    match fs::read(join(&[gitdir, "commondir"])) {
        Ok(b) => {
            common = trim_space(&go_lossy(&b)).to_string();
            if !is_abs(&common) {
                common = join(&[gitdir, &common]);
            }
        }
        Err(e) if e.kind() != ErrorKind::NotFound => return no(),
        Err(_) => {}
    }
    // git rejects a gitdir without these.
    if !looks_like_git_dir(gitdir)
        && !(file_exists(&join(&[gitdir, "HEAD"])) && looks_like_git_dir(&common))
    {
        return no();
    }
    if fs::metadata(join(&[&common, "reftable"])).is_ok() {
        return no();
    }
    let cfg = match fs::read(join(&[&common, "config"])) {
        Ok(b) => b,
        Err(e) if e.kind() != ErrorKind::NotFound => return no(),
        Err(_) => Vec::new(),
    };
    if config_needs_git(&cfg) {
        return no();
    }
    resolve_ref("HEAD", gitdir, &common, 0)
}

/// Go `configNeedsGit`: config keys that change ref or repo layout. Branch names are not
/// matched, so a branch called worktree-x is fine.
pub(crate) fn config_needs_git(cfg: &[u8]) -> bool {
    let cfg = gostd::strings::to_lower(&go_lossy(cfg));
    for ln in cfg.split('\n') {
        let mut ln = trim_space(ln);
        if ln.starts_with('[') {
            let mut name = ln.trim_start_matches(['[', ' ', '\t']);
            if let Some(i) = name.find([' ', '\t', ']', '.', '"']) {
                name = &name[..i];
            }
            if name == "extensions" || name == "include" || name == "includeif" {
                return true;
            }
            // A key may follow the header on the same line.
            ln = trim_space(ln.split_once(']').map_or("", |(_, r)| r));
        }
        let (key, val) = ln.split_once('=').unwrap_or((ln, ""));
        match (trim_space(key), trim_space(val)) {
            ("worktree" | "refstorage", _) => return true,
            ("repositoryformatversion", v) if v != "0" => return true,
            _ => {}
        }
    }
    false
}

/// Go `badRef`: names git's refname rules may reject.
fn bad_ref(name: &str) -> bool {
    if name.ends_with('/') || name.ends_with('.') || name.contains("@{") {
        return true;
    }
    if name
        .chars()
        .any(|c| c < ' ' || c == '\x7f' || " ~^:?*[\\".contains(c))
    {
        return true;
    }
    name.split('/')
        .any(|p| p.is_empty() || p.starts_with('.') || p.ends_with(".lock"))
}

/// Go `isSha`.
fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// Go `resolveRef`: follows `name` to a SHA. An unborn ref gives `""` and true.
fn resolve_ref(name: &str, gitdir: &str, common: &str, depth: u32) -> (String, bool) {
    let no = || (String::new(), false);
    if depth >= 5
        || (name != "HEAD" && (!name.starts_with("refs/") || name.contains("..") || bad_ref(name)))
    {
        return no();
    }
    let base = if name == "HEAD"
        || name.starts_with("refs/worktree/")
        || name.starts_with("refs/bisect/")
        || name.starts_with("refs/rewritten/")
    {
        gitdir
    } else {
        common
    };
    let path = join(&[base, name]);
    if fs::symlink_metadata(&path).is_ok_and(|fi| !fi.is_file()) {
        // Legacy symlink refs: let git follow them.
        return no();
    }
    match fs::read(&path) {
        Ok(b) => {
            let s = go_lossy(&b);
            let s = s.strip_suffix('\n').unwrap_or(&s);
            if is_sha(s) {
                return (s.to_string(), true);
            }
            if let Some(target) = s.strip_prefix("ref: ") {
                return resolve_ref(target, gitdir, common, depth + 1);
            }
            return no();
        }
        Err(e) if e.kind() != ErrorKind::NotFound || name == "HEAD" => return no(),
        Err(_) => {}
    }
    let packed = match fs::read(join(&[common, "packed-refs"])) {
        Ok(b) => go_lossy(&b),
        Err(e) => return (String::new(), e.kind() == ErrorKind::NotFound),
    };
    for line in packed.split('\n') {
        if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let (sha, r) = line.split_once(' ').unwrap_or((line, ""));
        if trim_space(r) == name {
            return (sha.to_string(), is_sha(sha));
        }
    }
    (String::new(), true)
}

/// Go `ownedByMe` (`githead_unix.go`): git's safe.directory owner check, the owner of `path`
/// against `os.Getuid()`.
#[cfg(unix)]
fn owned_by_me(path: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(path), getuid()) {
        (Ok(fi), Some(uid)) => fi.uid() == uid,
        _ => false,
    }
}

/// Go `os.Getuid()`. std has no `getuid` and graphify makes no FFI call, so it is read from
/// `/proc/self/status` (the real uid, the first of the `Uid:` line). Where that is missing
/// (macOS) it is `None` and every repo asks git, as Go's `githead_other.go` does: the same
/// answer, one `git` run slower.
#[cfg(unix)]
fn getuid() -> Option<u32> {
    static UID: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *UID.get_or_init(|| {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        let line = status.split('\n').find(|l| l.starts_with("Uid:"))?;
        gostd::strings::fields(line).get(1)?.parse().ok()
    })
}

/// Go `githead_other.go`: no owner check here, always ask git.
#[cfg(not(unix))]
fn owned_by_me(_: &str) -> bool {
    false
}

/// Go `sameDevice` (`githead_unix.go`).
#[cfg(unix)]
fn same_device(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev()
}

/// Go `githead_other.go`: devices unknown, walk on; any repo found falls back.
#[cfg(not(unix))]
fn same_device(_: &Metadata, _: &Metadata) -> bool {
    true
}

#[cfg(all(test, unix))]
#[path = "tests/githead.rs"]
mod githead_tests;
