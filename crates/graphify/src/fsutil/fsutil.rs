//! Port of `fsutil.go`: atomic file writes. Every error's `Display` is Go's text for its step
//! (`goerr`): `open <tmp>: ...` (CreateTemp), `write <tmp>: ...`, `chmod <tmp>: ...`,
//! `rename <tmp> <path>: ...`; an error from the caller's writer passes through unchanged.

use std::fs::{self, File, OpenOptions, Permissions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, BufWriter, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::goerr::{link_error, path_error};

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

/// The temp file, whose write errors read as Go's `*os.File.Write` errors.
struct TempFile {
    f: File,
    name: PathBuf,
}

impl Write for TempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.f
            .write(buf)
            .map_err(|e| path_error("write", &self.name, &e))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Go `writeAtomic`. The temp file is removed on every path but a successful rename (Go's
/// deferred `os.Remove`).
fn write_temp(
    path: &Path,
    perm: u32,
    write: impl FnOnce(&mut TempFile) -> io::Result<()>,
) -> io::Result<()> {
    let mut f = create_temp(dir_of(path))?;
    let tmp = f.name.clone();
    let result = fill_and_rename(&mut f, path, perm, write);
    drop(f);
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// The body of Go `writeAtomic` after `CreateTemp`: write, `Chmod(perm)`, `Rename`.
fn fill_and_rename(
    f: &mut TempFile,
    path: &Path,
    perm: u32,
    write: impl FnOnce(&mut TempFile) -> io::Result<()>,
) -> io::Result<()> {
    write(f)?;
    f.f.set_permissions(Permissions::from_mode(perm & 0o7777))
        .map_err(|e| path_error("chmod", &f.name, &e))?;
    fs::rename(&f.name, path).map_err(|e| link_error("rename", &f.name, path, &e))
}

/// Go `filepath.Dir(path)` for the temp file: the parent, `.` when there is none.
fn dir_of(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Go `os.CreateTemp(dir, ".gfy-*")`: `.gfy-` and a random decimal number, created with
/// `O_EXCL` and mode 0600; a name that exists is tried again, at most 10,000 times. An open
/// error is Go's `open <name>: ...`; running out of names is `createtemp <dir>/.gfy-*: file
/// already exists`.
fn create_temp(dir: &Path) -> io::Result<TempFile> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let seed = RandomState::new();
    for _ in 0..10_000 {
        let n = seed.hash_one(NEXT.fetch_add(1, Ordering::Relaxed)) as u32;
        let name = dir.join(format!(".gfy-{n}"));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&name)
        {
            Ok(f) => return Ok(TempFile { f, name }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(path_error("open", &name, &e)),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "createtemp {}: file already exists",
            dir.join(".gfy-*").display()
        ),
    ))
}
