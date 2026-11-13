//! Go 1.27.4 `path/filepath` `WalkDir` (`path.go`) with `io/fs` `ReadDir` semantics: lexical
//! order, `SkipDir` on a file skips the rest of its directory. Call site: graphify-go
//! `detect.go:734` (`WalkDir`, `SkipDir`).

use super::{base_bytes, join_bytes, path_of};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

/// The `fs.DirEntry` parts a callback needs.
pub struct DirEntry {
    pub name: OsString,
    pub file_type: std::fs::FileType,
}

/// A callback's answer: Go `nil`, `fs.SkipDir`, or any other error (stops the walk).
pub enum Walk {
    Continue,
    SkipDir,
    Stop(std::io::Error),
}

/// What a step hands back to its caller: Go's `SkipDir`, or another error.
enum Ctl {
    Skip,
    Err(io::Error),
}

fn ctl(w: Walk) -> Result<(), Ctl> {
    match w {
        Walk::Continue => Ok(()),
        Walk::SkipDir => Err(Ctl::Skip),
        Walk::Stop(e) => Err(Ctl::Err(e)),
    }
}

/// Go `filepath.WalkDir` (path.go:396). The callback gets the path, the entry (`None` only when
/// the root cannot be stat'ed) and the error, if any. For the root, `name` is the last element of
/// `root`.
pub fn walk_dir(
    root: &Path,
    mut f: impl FnMut(&Path, Option<&DirEntry>, Option<&std::io::Error>) -> Walk,
) -> std::io::Result<()> {
    let res = match fs::symlink_metadata(root) {
        Err(e) => ctl(f(root, None, Some(&e))),
        Ok(info) => {
            // fs.FileInfoToDirEntry(info): the name is the base of the path as given.
            let d = DirEntry {
                name: OsString::from_vec(base_bytes(root.as_os_str().as_bytes()).to_vec()),
                file_type: info.file_type(),
            };
            walk_dir_rec(root, &d, &mut f)
        }
    };
    // path.go:403 SkipDir (and SkipAll) end the walk without an error.
    match res {
        Ok(()) | Err(Ctl::Skip) => Ok(()),
        Err(Ctl::Err(e)) => Err(e),
    }
}

/// path.go:310 `walkDir`: recursively descends path, calling the callback.
fn walk_dir_rec<F>(path: &Path, d: &DirEntry, f: &mut F) -> Result<(), Ctl>
where
    F: FnMut(&Path, Option<&DirEntry>, Option<&io::Error>) -> Walk,
{
    let is_dir = d.file_type.is_dir();
    let res = ctl(f(path, Some(d), None));
    if res.is_err() || !is_dir {
        return match res {
            // Successfully skipped directory.
            Err(Ctl::Skip) if is_dir => Ok(()),
            r => r,
        };
    }

    let (dirs, err) = read_dir(path);
    if let Some(e) = err {
        // path.go:321 second call, to report ReadDir error.
        match ctl(f(path, Some(d), Some(&e))) {
            Ok(()) => {}
            // `d` is a directory here, so SkipDir skips it.
            Err(Ctl::Skip) => return Ok(()),
            Err(c) => return Err(c),
        }
    }

    for d1 in &dirs {
        let path1 = path_of(join_bytes(&[
            path.as_os_str().as_bytes(),
            d1.name.as_bytes(),
        ]));
        match walk_dir_rec(&path1, d1, f) {
            // path.go:334 SkipDir from a file skips the rest of this directory (from a directory
            // it was already turned into success).
            Err(Ctl::Skip) => break,
            Err(c) => return Err(c),
            Ok(()) => {}
        }
    }
    Ok(())
}

/// `os.ReadDir` (os/dir.go:114): the entries sorted by name bytes, and the error, if any. On an
/// error the entries read before it are returned.
fn read_dir(path: &Path) -> (Vec<DirEntry>, Option<io::Error>) {
    let rd = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => return (Vec::new(), Some(e)),
    };
    let mut dirs = Vec::new();
    let mut err = None;
    for ent in rd {
        match ent.and_then(|e| {
            e.file_type().map(|file_type| DirEntry {
                name: e.file_name(),
                file_type,
            })
        }) {
            Ok(d) => dirs.push(d),
            // Go's ReadDir skips an entry that vanished during the read (os/dir_unix.go).
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    dirs.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    (dirs, err)
}
