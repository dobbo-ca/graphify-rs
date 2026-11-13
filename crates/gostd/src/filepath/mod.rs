//! Go 1.27.4 `path/filepath` (unix only): the lexical functions of `path/filepath/path.go` and
//! `path_unix.go`. Call sites: graphify-go's ~220 `path/filepath` uses (`Join`, `Dir`, `Base`,
//! `Ext`, `Abs`, `Rel`, `Clean`, `IsAbs`). `ToSlash`, `FromSlash` and `Separator` are the identity
//! and `'/'` on unix: no gostd function. Lexical rules work on `as_os_str().as_bytes()`.

mod match_;
mod symlink;
mod walk;

pub use match_::{BadPattern, glob, match_pattern};
pub use symlink::eval_symlinks;
pub use walk::{DirEntry, Walk, walk_dir};

use std::ffi::OsString;
use std::fmt;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

/// Go `Rel`'s error: `Rel: can't make <targ> relative to <base>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelError {
    pub base: String,
    pub targ: String,
}

impl fmt::Display for RelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rel: can't make {} relative to {}", self.targ, self.base)
    }
}

impl std::error::Error for RelError {}

/// A `PathBuf` from raw bytes (file names need not be UTF-8).
pub(crate) fn path_of(b: Vec<u8>) -> PathBuf {
    PathBuf::from(OsString::from_vec(b))
}

/// A `String` from bytes that are valid UTF-8 by construction (whole pieces of a `&str` and ASCII
/// separators); never lossy in practice.
fn string_of(b: Vec<u8>) -> String {
    String::from_utf8_lossy(&b).into_owned()
}

/// internal/filepathlite/path.go:65 `Clean` (volume length is always 0 on unix). `out.w` of Go's
/// `lazybuf` is `out.len()`: the lazy copy only avoids an allocation and is not observable.
pub(crate) fn clean_bytes(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        // `originalPath + "."` for an empty path.
        return b".".to_vec();
    }
    let rooted = path[0] == b'/';
    let n = path.len();
    let mut out: Vec<u8> = Vec::with_capacity(n);
    // dotdot is the index in out where .. must stop, either because it is the leading slash or
    // because it is a leading ../../.. prefix.
    let (mut r, mut dotdot) = (0, 0);
    if rooted {
        out.push(b'/');
        r = 1;
        dotdot = 1;
    }
    while r < n {
        if path[r] == b'/' {
            // path.go:93 empty path element
            r += 1;
        } else if path[r] == b'.' && (r + 1 == n || path[r + 1] == b'/') {
            // path.go:96 . element
            r += 1;
        } else if path[r] == b'.' && path[r + 1] == b'.' && (r + 2 == n || path[r + 2] == b'/') {
            // path.go:99 .. element: remove to last separator
            r += 2;
            if out.len() > dotdot {
                // path.go:103 can backtrack: `out.w--` then
                // `for out.w > dotdot && out.index(out.w) != '/'`, where index(w) is the byte
                // just dropped
                let mut last = out.pop();
                while out.len() > dotdot && last != Some(b'/') {
                    last = out.pop();
                }
            } else if !rooted {
                // path.go:109 cannot backtrack, but not rooted, so append .. element
                if !out.is_empty() {
                    out.push(b'/');
                }
                out.extend_from_slice(b"..");
                dotdot = out.len();
            }
        } else {
            // path.go:121 real path element: add slash if needed
            if (rooted && out.len() != 1) || (!rooted && !out.is_empty()) {
                out.push(b'/');
            }
            // copy element
            while r < n && path[r] != b'/' {
                out.push(path[r]);
                r += 1;
            }
        }
    }
    // path.go:132 turn empty string into "."
    if out.is_empty() {
        out.push(b'.');
    }
    out
}

/// path_unix.go:32 `join`: the first non-empty element starts the join; the result is cleaned.
pub(crate) fn join_bytes(elems: &[&[u8]]) -> Vec<u8> {
    for (i, e) in elems.iter().enumerate() {
        if !e.is_empty() {
            return clean_bytes(&elems[i..].join(&b'/'));
        }
    }
    Vec::new()
}

/// internal/filepathlite/path.go:225 `Base`.
pub(crate) fn base_bytes(path: &[u8]) -> &[u8] {
    if path.is_empty() {
        return b".";
    }
    // Strip trailing slashes.
    let mut path = path;
    while let [rest @ .., b'/'] = path {
        path = rest;
    }
    // Throw away the last element's leading directory.
    if let Some(i) = path.iter().rposition(|&c| c == b'/') {
        path = &path[i + 1..];
    }
    // If empty now, it had only slashes.
    if path.is_empty() {
        return b"/";
    }
    path
}

/// Go `filepath.Clean`.
pub fn clean(p: &str) -> String {
    string_of(clean_bytes(p.as_bytes()))
}

/// Go `filepath.Join`.
pub fn join(elems: &[&str]) -> String {
    let b: Vec<&[u8]> = elems.iter().map(|e| e.as_bytes()).collect();
    string_of(join_bytes(&b))
}

/// Go `filepath.Dir` (internal/filepathlite/path.go:251): drop the last element, then `Clean`.
pub fn dir(p: &str) -> String {
    let b = p.as_bytes();
    let end = b.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
    string_of(clean_bytes(&b[..end]))
}

/// Go `filepath.Base`.
pub fn base(p: &str) -> String {
    string_of(base_bytes(p.as_bytes()).to_vec())
}

/// Go `filepath.Ext` (internal/filepathlite/path.go:215): the suffix from the final dot of the
/// final element.
pub fn ext(p: &str) -> &str {
    let b = p.as_bytes();
    match b.iter().rposition(|&c| c == b'.' || c == b'/') {
        Some(i) if b[i] == b'.' => &p[i..],
        _ => "",
    }
}

/// Go `filepath.IsAbs`.
pub fn is_abs(p: &str) -> bool {
    p.starts_with('/')
}

/// Go `filepath.Rel` (path.go:185); the volume name is always empty on unix.
pub fn rel(base: &str, targ: &str) -> Result<String, RelError> {
    let err = || RelError {
        base: base.to_string(),
        targ: targ.to_string(),
    };
    let mut basep = clean_bytes(base.as_bytes());
    let targp = clean_bytes(targ.as_bytes());
    if targp == basep {
        return Ok(".".to_string());
    }
    if basep == b"." {
        basep.clear();
    }
    // path.go:203 (unix: a plain leading slash).
    let base_slashed = basep.first() == Some(&b'/');
    let targ_slashed = targp.first() == Some(&b'/');
    if base_slashed != targ_slashed {
        return Err(err());
    }
    // path.go:219 position base[b0:bi] and targ[t0:ti] at the first differing elements.
    let (bl, tl) = (basep.len(), targp.len());
    let (mut b0, mut bi, mut t0, mut ti) = (0, 0, 0, 0);
    loop {
        while bi < bl && basep[bi] != b'/' {
            bi += 1;
        }
        while ti < tl && targp[ti] != b'/' {
            ti += 1;
        }
        if targp[t0..ti] != basep[b0..bi] {
            break;
        }
        if bi < bl {
            bi += 1;
        }
        if ti < tl {
            ti += 1;
        }
        b0 = bi;
        t0 = ti;
    }
    if basep[b0..bi] == *b".." {
        // path.go:231
        return Err(err());
    }
    if b0 != bl {
        // path.go:234 base elements left. Must go up before going down.
        let seps = basep[b0..bl].iter().filter(|&&c| c == b'/').count();
        let mut buf = b"..".to_vec();
        for _ in 0..seps {
            buf.extend_from_slice(b"/..");
        }
        if t0 != tl {
            buf.push(b'/');
            buf.extend_from_slice(&targp[t0..]);
        }
        return Ok(string_of(clean_bytes(&buf)));
    }
    Ok(string_of(targp[t0..].to_vec()))
}

/// Go `filepath.Abs` (`unixAbs`, path.go:165): the working directory joined with `p`, cleaned.
pub fn abs(p: &Path) -> std::io::Result<PathBuf> {
    let b = p.as_os_str().as_bytes();
    if b.first() == Some(&b'/') {
        return Ok(path_of(clean_bytes(b)));
    }
    let wd = getwd()?;
    Ok(path_of(join_bytes(&[wd.as_os_str().as_bytes(), b])))
}

/// Go `os.Getwd` (os/getwd.go): `$PWD` when it is absolute and names the same directory as `.`
/// (same device and inode), else the kernel's answer. Rust's `current_dir` alone resolves
/// symlinks, so a symlinked working directory would give a different `abs`.
fn getwd() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let dot = std::fs::metadata(".")?;
    if let Some(pwd) = std::env::var_os("PWD") {
        let pwd = PathBuf::from(pwd);
        if pwd.is_absolute()
            && let Ok(m) = std::fs::metadata(&pwd)
            && m.dev() == dot.dev()
            && m.ino() == dot.ino()
        {
            return Ok(pwd);
        }
    }
    std::env::current_dir()
}
