//! Port of `walk_test.go` (1 test).

use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use gostd::filepath::{DirEntry, Walk, walk_dir};

use super::{chmod, readable_despite_chmod};
use crate::cluster::tests::gorand::GoRand;
use crate::detect::detect::{WalkFn, walk_tree};
use crate::detect::{WalkReport, go_lossy};
use crate::testutil::TempDir;

/// The Go test's `fn`: records walk errors and files by path, prunes directories whose path ends
/// in `7`, and counts files starting with `z` as skipped, keyed by their first two bytes.
fn record(
    rep: &mut WalkReport,
    path: &Path,
    d: Option<&DirEntry>,
    err: Option<&io::Error>,
) -> Walk {
    let p = go_lossy(path.as_os_str().as_bytes());
    if err.is_some() {
        rep.walk_errors.push(p);
        return Walk::Continue;
    }
    let d = d.expect("entry without an error");
    if d.file_type.is_dir() {
        if p.ends_with('7') {
            return Walk::SkipDir;
        }
        return Walk::Continue;
    }
    let name = d.name.as_bytes();
    if name.starts_with(b"z") {
        rep.skipped += 1;
        *rep.skipped_exts.entry(go_lossy(&name[..2])).or_insert(0) += 1;
        return Walk::Continue;
    }
    rep.files.push(p);
    Walk::Continue
}

/// `filepath.WalkDir` with `record`.
fn walk_dir_report(root: &Path) -> WalkReport {
    let mut rep = WalkReport::default();
    let _ = walk_dir(root, |p, d, e| record(&mut rep, p, d, e));
    rep
}

/// Go `TestWalkTreeMatchesWalkDir`: `walk_tree` reports what `filepath.WalkDir` reports, in the
/// same order, on a random tree with an unreadable dir. Go runs as a non-root user, where the
/// `locked` dir is the one walk error; as root (the CI containers) it reads fine and there is
/// none. The worker count alternates between 1 and 4 (Go: GOMAXPROCS).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_walk_tree_matches_walk_dir() {
    let root = TempDir::new();
    let mut rng = GoRand::new(1);
    let mut dirs = vec![root.path().to_path_buf()];
    for _ in 0..400 {
        let parent = dirs[rng.intn(dirs.len())].clone();
        let d = parent.join(format!("d{}", rng.intn(50)));
        std::fs::create_dir_all(&d).expect("mkdir");
        dirs.push(d);
    }
    for i in 0..3000 {
        // Names sort before, between and after the dN directories.
        let c = char::from(b"Cdz"[rng.intn(3)]);
        let name = format!("{c}{i}.x");
        std::fs::write(dirs[rng.intn(dirs.len())].join(name), b"").expect("write");
    }
    let locked = root.join("locked");
    std::fs::create_dir(&locked).expect("mkdir");
    chmod(&locked, 0);
    let want_errors = usize::from(!readable_despite_chmod(&locked));

    let f: &WalkFn<'_> = &record;
    let want = walk_dir_report(root.path());
    let mut runs = Vec::new();
    for i in 0..20 {
        let workers = if i % 2 == 0 { 1 } else { 4 };
        runs.push(walk_tree(root.path(), workers, f));
    }
    let missing = root.join("missing");
    let want_missing = walk_dir_report(&missing);
    let got_missing = walk_tree(&missing, 4, f);
    chmod(&locked, 0o755);

    assert!(
        want.files.len() >= 500 && want.skipped != 0 && want.walk_errors.len() == want_errors,
        "weak tree: {} files, {} skipped, errors {:?}",
        want.files.len(),
        want.skipped,
        want.walk_errors
    );
    for (i, got) in runs.iter().enumerate() {
        assert!(
            *got == want,
            "run {i}: walkTree differs from WalkDir ({} files against {}, errors {:?} against {:?})",
            got.files.len(),
            want.files.len(),
            got.walk_errors,
            want.walk_errors
        );
    }
    assert_eq!(got_missing, want_missing, "missing root");
}
