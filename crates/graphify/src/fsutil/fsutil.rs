//! Port of `fsutil.go`: atomic file writes.

use std::fs::{self, File, OpenOptions, Permissions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, BufWriter, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Go `fsutil.WriteFileAtomic`: writes `data` to a `.gfy-*` temp file in the same directory,
/// sets mode `perm`, and renames it over `path`, so a concurrent reader sees either the old file
/// or the complete new one.
pub fn write_file_atomic(path: &Path, data: &[u8], perm: u32) -> io::Result<()> {
    write_temp(path, perm, |f| f.write_all(data))
}

/// Go `fsutil.WriteAtomic`: [`write_file_atomic`] for content made piece by piece; `write`
/// gets a 1 MiB buffered writer over the temp file.
pub fn write_atomic(
    path: &Path,
    perm: u32,
    write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    write_temp(path, perm, |f| {
        let mut bw = BufWriter::with_capacity(1 << 20, f);
        write(&mut bw)?;
        bw.flush()
    })
}

/// Go `writeAtomic`. The temp file is removed on every path but a successful rename (Go's
/// deferred `os.Remove`).
fn write_temp(
    path: &Path,
    perm: u32,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let (f, tmp) = create_temp(dir_of(path))?;
    let result = fill_and_rename(f, &tmp, path, perm, write);
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// The body of Go `writeAtomic` after `CreateTemp`: write, `Chmod(perm)`, `Close`, `Rename`.
fn fill_and_rename(
    mut f: File,
    tmp: &Path,
    path: &Path,
    perm: u32,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    write(&mut f)?;
    f.set_permissions(Permissions::from_mode(perm & 0o7777))?;
    drop(f);
    fs::rename(tmp, path)
}

/// Go `filepath.Dir(path)` for the temp file: the parent, `.` when there is none.
fn dir_of(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Go `os.CreateTemp(dir, ".gfy-*")`: `.gfy-` and a random decimal number, created with
/// `O_EXCL` and mode 0600; a name that exists is tried again, at most 10,000 times.
fn create_temp(dir: &Path) -> io::Result<(File, PathBuf)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let seed = RandomState::new();
    for _ in 0..10_000 {
        let n = seed.hash_one(NEXT.fetch_add(1, Ordering::Relaxed)) as u32;
        let tmp = dir.join(format!(".gfy-{n}"));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
        {
            Ok(f) => return Ok((f, tmp)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("open {}: file exists", dir.join(".gfy-*").display()),
    ))
}
