//! Go 1.27.4 `path/filepath/symlink.go` and `symlink_unix.go`: `EvalSymlinks`. Call sites:
//! graphify-go's `EvalSymlinks` uses.

use super::{clean_bytes, path_of};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

/// `syscall.ENOTDIR` (the same number on Linux and macOS).
const ENOTDIR: i32 = 20;

/// Go `filepath.EvalSymlinks` (`walkSymlinks`, symlink.go:16). The volume name is always empty on
/// unix, so `volLen` is 1 for an absolute path and 0 otherwise.
pub fn eval_symlinks(p: &Path) -> io::Result<PathBuf> {
    let mut path: Vec<u8> = p.as_os_str().as_bytes().to_vec();
    let mut vol_len = usize::from(path.first() == Some(&b'/'));
    let mut vol: Vec<u8> = path[..vol_len].to_vec();
    let mut dest = vol.clone();
    let mut links_walked = 0;
    let mut start = vol_len;
    // Go's `for ...; start < len(path); start = end`: every `continue` below sets `start = end`.
    while start < path.len() {
        while start < path.len() && path[start] == b'/' {
            start += 1;
        }
        let mut end = start;
        while end < path.len() && path[end] != b'/' {
            end += 1;
        }

        // The next path component is in path[start:end].
        if end == start {
            // No more path components.
            break;
        } else if path[start..end] == *b"." {
            // symlink.go:45 ignore path component ".".
            start = end;
            continue;
        } else if path[start..end] == *b".." {
            // symlink.go:48 back up to previous component if possible. Set r to the index of the
            // last slash in dest, after the volume.
            match last_slash(&dest, vol_len) {
                Some(r) if dest[r + 1..] != *b".." => {
                    // Discard everything since the last slash.
                    dest.truncate(r);
                }
                _ => {
                    // Either path has no slashes (it's empty) or it ends in a ".." we had to
                    // keep. Either way, keep this "..".
                    if dest.len() > vol_len {
                        dest.push(b'/');
                    }
                    dest.extend_from_slice(b"..");
                }
            }
            start = end;
            continue;
        }

        // symlink.go:75 ordinary path component. Add it to result.
        if !dest.is_empty() && dest[dest.len() - 1] != b'/' {
            dest.push(b'/');
        }
        dest.extend_from_slice(&path[start..end]);

        // symlink.go:83 resolve symlink.
        let fi = fs::symlink_metadata(Path::new(OsStr::from_bytes(&dest)))?;
        if !fi.file_type().is_symlink() {
            if !fi.is_dir() && end < path.len() {
                return Err(io::Error::from_raw_os_error(ENOTDIR));
            }
            start = end;
            continue;
        }

        // Found symlink.
        links_walked += 1;
        if links_walked > 255 {
            // symlink.go:100
            return Err(io::Error::other("EvalSymlinks: too many links"));
        }
        let link = fs::read_link(Path::new(OsStr::from_bytes(&dest)))?
            .into_os_string()
            .into_vec();
        let mut next = link.clone();
        next.extend_from_slice(&path[end..]);
        path = next;

        if link.first() == Some(&b'/') {
            // symlink.go:127 symlink to absolute path.
            dest = link[..1].to_vec();
            end = 1;
            vol = link[..1].to_vec();
            vol_len = 1;
        } else {
            // symlink.go:133 symlink to relative path; replace last path component in dest.
            match last_slash(&dest, vol_len) {
                Some(r) => dest.truncate(r),
                None => dest = vol.clone(),
            }
            end = 0;
        }
        start = end;
    }
    Ok(path_of(clean_bytes(&dest)))
}

/// The index of the last `/` in `dest` at or after `vol_len` (Go's
/// `for r = len(dest) - 1; r >= volLen; r--`).
fn last_slash(dest: &[u8], vol_len: usize) -> Option<usize> {
    dest[vol_len..]
        .iter()
        .rposition(|&c| c == b'/')
        .map(|i| i + vol_len)
}
