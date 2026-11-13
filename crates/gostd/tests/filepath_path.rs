//! Ported from path/filepath/path_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2009 The Go
//! Authors), unix rows only. Source line numbers are those of go1.27.4.
//! - TestClean (129), TestJoin (485), TestExt (511), TestBase (994), TestDir (1042), TestIsAbs (1101),
//!   TestRel (1537): ported (unix rows; the Windows tables and the AllocsPerRun check, a Go runtime
//!   feature, are left out)
//! - TestAbs (1392): ported, adapted. Go calls chdir; Rust tests share one process and run in parallel
//!   threads, so no test changes the working directory. The relative rows are tried as absolute paths
//!   under `<root>/a` (the directory Go changes into), and again against the real working directory
//!   as `clean(cwd + "/" + p)`
//! - TestWalkDir (626): ported without the PermErr subtest (Go skips it under -short and as root, and
//!   its second chmod hits `entries[3]`, a file, so it raises no error and the test cannot pass as
//!   written). Go's `t.Chdir(t.TempDir())` becomes a tree under a temp dir
//! - TestWalkSkipDirOnFile (722): ported (WalkDir half; Go's own check walks the same root twice)
//! - TestWalkFileError (826): ported, adapted. Go swaps the `os.Lstat` hook, which WalkDir does not
//!   call. The failing entry is a directory with mode 0, so `ReadDir` fails (skipped when it is still
//!   readable, as root)
//! - TestWalkSymlinkRoot (869): ported with WalkDir in place of Walk (same Lstat of the root)
//! - TestEvalSymlinks (1194): ported, adapted: absolute paths only. The variants that chdir (`"."`,
//!   the relative path from `tmpDir`) are left out; the `".."+path` variant is run as
//!   `<tmp>/test/../<path>`
//! - TestEvalSymlinksIsNotExist (1255): ported with absolute paths (Go chdirs)
//! - TestIssue13582 (1276): ported
//! - TestRelativeSymlinkToAbsolute (1336): ported with the link given by absolute path (Go chdirs)
//! - TestEvalSymlinksAboveRoot (1756): ported
//! - TestEvalSymlinksAboveRootChdir (1807): ported, adapted: Go chdirs into `d/e/f` and resolves the
//!   relative `../../../c/file`; here the same walk is spelled as an absolute path
//! - TestEvalSymlinksTooManyLinks (1900): ported
//!
//! Not ported (15 of the 34 funcs):
//! - TestIsLocal (228), TestLocalize (285): IsLocal and Localize are not reached by graphify-go
//! - TestFromAndToSlash (319): ToSlash and FromSlash are the identity on unix (no gostd function)
//! - TestSplitList (365), TestSplit (402): SplitList and Split are not reached by graphify-go
//! - TestWalk (617), TestWalkSymlink (1723), TestBug3486 (1658): `filepath.Walk` is not reached by
//!   graphify-go (TestBug3486 also reads GOROOT)
//! - TestWalkSkipAllOnFile (768): SkipAll is not reached by graphify-go
//! - TestAbsEmptyString (1453): Windows special case (issue 24441), not in the plan's list
//! - TestVolumeName (1624), TestDriveLetterInEvalSymlinks (1635): Windows only
//! - TestIssue29372 (1728): ENOTDIR on a file with trailing slashes, not in the plan's list
//! - TestIssue51617 (1841): chmod-0 directory under WalkDir, not in the plan's list (the chmod idea
//!   is used by test_walk_file_error)
//! - TestEscaping (1876): IsLocal is not reached by graphify-go
//!
//! Added (no Go source): go_not_rust_semantics, walk_dir_non_utf8_name (Review Focus 5).

use gostd::filepath::{
    DirEntry, Walk, abs, base, clean, dir, eval_symlinks, ext, glob, is_abs, join, rel, walk_dir,
};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A temp dir removed when it goes out of scope (also when a test panics).
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "gostd-filepath-path-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn touch(p: &Path) {
    fs::File::create(p).unwrap();
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Go `simpleJoin`: does not use `join`, so `..` is not evaluated.
fn simple_join(dir: &str, path: &str) -> String {
    format!("{dir}/{path}")
}

/// A `Walk::Stop` carrying a copy of `e` (`io::Error` is not `Clone`).
fn stop(e: &io::Error) -> Walk {
    Walk::Stop(io::Error::new(e.kind(), e.to_string()))
}

// (path, result)
const CLEAN_TESTS: &[(&str, &str)] = &[
    // Already clean
    ("abc", "abc"),
    ("abc/def", "abc/def"),
    ("a/b/c", "a/b/c"),
    (".", "."),
    ("..", ".."),
    ("../..", "../.."),
    ("../../abc", "../../abc"),
    ("/abc", "/abc"),
    ("/", "/"),
    // Empty is current dir
    ("", "."),
    // Remove trailing slash
    ("abc/", "abc"),
    ("abc/def/", "abc/def"),
    ("a/b/c/", "a/b/c"),
    ("./", "."),
    ("../", ".."),
    ("../../", "../.."),
    ("/abc/", "/abc"),
    // Remove doubled slash
    ("abc//def//ghi", "abc/def/ghi"),
    ("abc//", "abc"),
    // Remove . elements
    ("abc/./def", "abc/def"),
    ("/./abc/def", "/abc/def"),
    ("abc/.", "abc"),
    // Remove .. elements
    ("abc/def/ghi/../jkl", "abc/def/jkl"),
    ("abc/def/../ghi/../jkl", "abc/jkl"),
    ("abc/def/..", "abc"),
    ("abc/def/../..", "."),
    ("/abc/def/../..", "/"),
    ("abc/def/../../..", ".."),
    ("/abc/def/../../..", "/"),
    ("abc/def/../../../ghi/jkl/../../../mno", "../../mno"),
    ("/../abc", "/abc"),
    ("a/../b:/../../c", "../c"),
    // Combinations
    ("abc/./../def", "def"),
    ("abc//./../def", "def"),
    ("abc/../../././../def", "../../def"),
    // nonwincleantests: remove leading doubled slash
    ("//abc", "/abc"),
    ("///abc", "/abc"),
    ("//abc//", "/abc"),
];

// Go t.Errorf continues after a failure; assert_eq! stops at the first.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_clean() {
    for (path, result) in CLEAN_TESTS {
        assert_eq!(clean(path), *result, "clean({path:?})");
        assert_eq!(clean(result), *result, "clean({result:?})");
    }
}

// (elem, path)
#[allow(clippy::type_complexity)]
const JOIN_TESTS: &[(&[&str], &str)] = &[
    // zero parameters
    (&[], ""),
    // one parameter
    (&[""], ""),
    (&["/"], "/"),
    (&["a"], "a"),
    // two parameters
    (&["a", "b"], "a/b"),
    (&["a", ""], "a"),
    (&["", "b"], "b"),
    (&["/", "a"], "/a"),
    (&["/", "a/b"], "/a/b"),
    (&["/", ""], "/"),
    (&["/a", "b"], "/a/b"),
    (&["a", "/b"], "a/b"),
    (&["/a", "/b"], "/a/b"),
    (&["a/", "b"], "a/b"),
    (&["a/", ""], "a"),
    (&["", ""], ""),
    // three parameters
    (&["/", "a", "b"], "/a/b"),
    // nonwinjointests
    (&["//", "a"], "/a"),
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_join() {
    for (elem, path) in JOIN_TESTS {
        assert_eq!(join(elem), *path, "join({elem:?})");
    }
}

// (path, ext)
const EXT_TESTS: &[(&str, &str)] = &[
    ("path.go", ".go"),
    ("path.pb.go", ".go"),
    ("a.dir/b", ""),
    ("a.dir/b.go", ".go"),
    ("a.dir/", ""),
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_ext() {
    for (path, want) in EXT_TESTS {
        assert_eq!(ext(path), *want, "ext({path:?})");
    }
}

// Node is Go's tree node; entries is None for a file.
struct Node {
    name: &'static str,
    entries: Option<Vec<Node>>,
    mark: i32,
}

fn node(name: &'static str, entries: Option<Vec<Node>>) -> Node {
    Node {
        name,
        entries,
        mark: 0,
    }
}

// Go's `tree`, including the repeated "c" entry.
fn tree() -> Node {
    node(
        "testdata",
        Some(vec![
            node("a", None),
            node("b", Some(vec![])),
            node("c", None),
            node("c", None),
            node(
                "d",
                Some(vec![
                    node("x", None),
                    node("y", Some(vec![])),
                    node("z", Some(vec![node("u", None), node("v", None)])),
                ]),
            ),
        ]),
    )
}

fn walk_tree(n: &mut Node, path: &Path, f: &mut dyn FnMut(&Path, &mut Node)) {
    f(path, n);
    if let Some(entries) = n.entries.as_mut() {
        for e in entries {
            let p = path.join(e.name);
            walk_tree(e, &p, f);
        }
    }
}

fn make_tree(tree: &mut Node, root: &Path) {
    let top = root.join(tree.name);
    walk_tree(tree, &top, &mut |path, n| {
        if n.entries.is_none() {
            touch(path);
        } else {
            let _ = fs::create_dir(path);
        }
    });
}

fn check_marks(tree: &mut Node, report: bool) {
    walk_tree(tree, Path::new(""), &mut |_, n| {
        if report {
            assert_eq!(n.mark, 1, "node {} mark", n.name);
        }
        n.mark = 0;
    });
}

// Go `mark` with clear = true: every error is recorded and cleared.
// Assumes that each node name is unique. Good enough for a test.
fn mark(
    tree: &mut Node,
    d: Option<&DirEntry>,
    err: Option<&io::Error>,
    errors: &mut Vec<String>,
) -> Walk {
    let name = &d.unwrap().name;
    walk_tree(tree, Path::new(""), &mut |_, n| {
        if *name == n.name {
            n.mark += 1;
        }
    });
    if let Some(e) = err {
        errors.push(e.to_string());
    }
    Walk::Continue
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_walk_dir() {
    let td = TempDir::new();
    let mut tr = tree();
    make_tree(&mut tr, &td.0);
    let mut errors = Vec::new();
    // Expect no errors.
    let res = walk_dir(&td.0.join(tr.name), |_, d, err| {
        mark(&mut tr, d, err, &mut errors)
    });
    res.expect("no error expected");
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    check_marks(&mut tr, true);
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_walk_skip_dir_on_file() {
    let td = TempDir::new();
    fs::create_dir_all(td.0.join("dir")).unwrap();
    touch(&td.0.join("dir/foo1"));
    touch(&td.0.join("dir/foo2"));

    // Go's WalkDir closure ignores its root argument and always walks td.
    let check = |_root: &Path| {
        let mut saw_foo2 = false;
        walk_dir(&td.0, |path, _, _| {
            if path.as_os_str().as_bytes().ends_with(b"foo2") {
                saw_foo2 = true;
            }
            if path.as_os_str().as_bytes().ends_with(b"foo1") {
                return Walk::SkipDir;
            }
            Walk::Continue
        })
        .unwrap();
        assert!(
            !saw_foo2,
            "SkipDir on file foo1 did not block processing of foo2"
        );
    };
    check(&td.0);
    check(&td.0.join("dir"));
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_walk_file_error() {
    let td = TempDir::new();
    touch(&td.0.join("foo"));
    touch(&td.0.join("bar"));
    let d = td.0.join("dir");
    fs::create_dir_all(&d).unwrap();
    touch(&d.join("baz"));
    let noaccess = d.join("noaccess");
    fs::create_dir(&noaccess).unwrap();
    fs::set_permissions(&noaccess, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&noaccess).is_ok() {
        // root can read a mode 0 directory
        fs::set_permissions(&noaccess, fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipping: mode 0 directory is readable (running as root)");
        return;
    }
    // value: whether the last callback for the path carried an error
    let mut got: BTreeMap<String, bool> = BTreeMap::new();
    let res = walk_dir(&td.0, |path, _, err| {
        let r = rel(s(&td.0), s(path)).unwrap();
        got.insert(r, err.is_some());
        Walk::Continue
    });
    fs::set_permissions(&noaccess, fs::Permissions::from_mode(0o755)).unwrap();
    res.unwrap();
    let want: BTreeMap<String, bool> = [
        (".", false),
        ("foo", false),
        ("bar", false),
        ("dir", false),
        ("dir/baz", false),
        ("dir/noaccess", true),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(got, want);
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_walk_symlink_root() {
    let td = TempDir::new();
    let d = td.0.join("dir");
    fs::create_dir_all(&d).unwrap();
    touch(&d.join("foo"));

    let link = td.0.join("link");
    symlink("dir", &link).unwrap();
    let abslink = td.0.join("abslink");
    symlink(&d, &abslink).unwrap();
    let linklink = td.0.join("linklink");
    symlink("link", &linklink).unwrap();

    // Go: a root without a trailing slash is reported as the symlink itself; with a slash the
    // symlink is resolved and its directory walked.
    // (desc, root, want, known bug on darwin, https://go.dev/issue/59586)
    let (link, abslink, linklink) = (s(&link), s(&abslink), s(&linklink));
    let cases = vec![
        ("no slash", link.to_string(), vec![link.to_string()], false),
        (
            "slash",
            format!("{link}/"),
            vec![link.to_string(), format!("{link}/foo")],
            false,
        ),
        (
            "abs no slash",
            abslink.to_string(),
            vec![abslink.to_string()],
            false,
        ),
        (
            "abs with slash",
            format!("{abslink}/"),
            vec![abslink.to_string(), format!("{abslink}/foo")],
            false,
        ),
        (
            "double link no slash",
            linklink.to_string(),
            vec![linklink.to_string()],
            false,
        ),
        (
            "double link with slash",
            format!("{linklink}/"),
            vec![linklink.to_string(), format!("{linklink}/foo")],
            true,
        ),
    ];
    for (desc, root, want, buggy_darwin) in cases {
        let mut walked = Vec::new();
        walk_dir(Path::new(&root), |path, _, err| {
            if let Some(e) = err {
                return stop(e);
            }
            walked.push(clean(s(path)));
            Walk::Continue
        })
        .unwrap();
        if walked != want {
            if buggy_darwin && cfg!(target_os = "macos") {
                eprintln!("{desc}: ignoring known bug on macos");
            } else {
                panic!("{desc}: walk({root:?}) visited {walked:?}; want {want:?}");
            }
        }
    }
}

// (path, result)
const BASE_TESTS: &[(&str, &str)] = &[
    ("", "."),
    (".", "."),
    ("/.", "."),
    ("/", "/"),
    ("////", "/"),
    ("x/", "x"),
    ("abc", "abc"),
    ("abc/def", "def"),
    ("a/b/.x", ".x"),
    ("a/b/c.", "c."),
    ("a/b/c.x", "c.x"),
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_base() {
    for (path, want) in BASE_TESTS {
        assert_eq!(base(path), *want, "base({path:?})");
    }
}

// (path, result)
const DIR_TESTS: &[(&str, &str)] = &[
    ("", "."),
    (".", "."),
    ("/.", "/"),
    ("/", "/"),
    ("/foo", "/"),
    ("x/", "x"),
    ("abc", "."),
    ("abc/def", "abc"),
    ("a/b/.x", "a/b"),
    ("a/b/c.", "a/b"),
    ("a/b/c.x", "a/b"),
    // nonwindirtests
    ("////", "/"),
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_dir() {
    for (path, want) in DIR_TESTS {
        assert_eq!(dir(path), *want, "dir({path:?})");
    }
}

// (path, isAbs)
const IS_ABS_TESTS: &[(&str, bool)] = &[
    ("", false),
    ("/", true),
    ("/usr/bin/gcc", true),
    ("..", false),
    ("/a/../bb", true),
    (".", false),
    ("./", false),
    ("lala", false),
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_is_abs() {
    for (path, want) in IS_ABS_TESTS {
        assert_eq!(is_abs(path), *want, "is_abs({path:?})");
    }
}

// (path, dest): if dest is empty, the path is created; otherwise dest is symlinked to the path.
const EVAL_SYMLINKS_TEST_DIRS: &[(&str, &str)] = &[
    ("test", ""),
    ("test/dir", ""),
    ("test/dir/link3", "../../"),
    ("test/link1", "../test"),
    ("test/link2", "dir"),
    ("test/linkabs", "/"),
    ("test/link4", "../test2"),
    ("test2", "test/dir"),
    // Issue 23444.
    ("src", ""),
    ("src/pool", ""),
    ("src/pool/test", ""),
    ("src/versions", ""),
    ("src/versions/current", "../../version"),
    ("src/versions/v1", ""),
    ("src/versions/v1/modules", ""),
    ("src/versions/v1/modules/test", "../../../pool/test"),
    ("version", "src/versions/v1"),
];

const EVAL_SYMLINKS_TESTS: &[(&str, &str)] = &[
    ("test", "test"),
    ("test/dir", "test/dir"),
    ("test/dir/../..", "."),
    ("test/link1", "test"),
    ("test/link2", "test/dir"),
    ("test/link1/dir", "test/dir"),
    ("test/link2/..", "test"),
    ("test/dir/link3", "."),
    ("test/link2/link3/test", "test"),
    ("test/linkabs", "/"),
    ("test/link4/..", "test"),
    ("src/versions/current/modules/test", "src/pool/test"),
];

fn check_eval_symlinks(path: &str, want: &str) {
    let have = eval_symlinks(Path::new(path))
        .unwrap_or_else(|e| panic!("eval_symlinks({path:?}) error: {e}"));
    assert_eq!(
        clean(s(&have)),
        clean(want),
        "eval_symlinks({path:?}) returns {have:?}, want {want:?}"
    );
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_eval_symlinks() {
    let td = TempDir::new();
    // /tmp may itself be a symlink! Avoid the confusion, although it means trusting the thing we're testing.
    let tmp_dir = eval_symlinks(&td.0).unwrap();
    let tmp_dir = s(&tmp_dir);

    // Create the symlink farm using relative paths.
    for (path, dest) in EVAL_SYMLINKS_TEST_DIRS {
        let p = simple_join(tmp_dir, path);
        if dest.is_empty() {
            fs::create_dir(&p).unwrap();
        } else {
            symlink(dest, &p).unwrap();
        }
    }

    // Evaluate the symlink farm.
    let test_dir = simple_join(tmp_dir, "test");
    for (path, dest) in EVAL_SYMLINKS_TESTS {
        let p = simple_join(tmp_dir, path);
        let abs_dest = dest.starts_with('/');

        let want = if abs_dest {
            dest.to_string()
        } else {
            simple_join(tmp_dir, dest)
        };
        check_eval_symlinks(&p, &want);

        // Go: chdir(<tmp>/test), then EvalSymlinks(".." + path), want ".." + dest
        let dotdot_p = simple_join(&test_dir, &simple_join("..", path));
        let dotdot_want = if abs_dest {
            dest.to_string()
        } else {
            simple_join(&test_dir, &simple_join("..", dest))
        };
        check_eval_symlinks(&dotdot_p, &dotdot_want);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_eval_symlinks_is_not_exist() {
    let td = TempDir::new();

    let err = eval_symlinks(&td.0.join("notexist")).unwrap_err();
    assert_eq!(
        err.kind(),
        io::ErrorKind::NotFound,
        "expected the file is not found, got {err}"
    );

    symlink("notexist", td.0.join("link")).unwrap();
    let err = eval_symlinks(&td.0.join("link")).unwrap_err();
    assert_eq!(
        err.kind(),
        io::ErrorKind::NotFound,
        "expected the file is not found, got {err}"
    );
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_issue13582() {
    let td = TempDir::new();
    let tmp_dir = &td.0;

    let d = tmp_dir.join("dir");
    fs::create_dir(&d).unwrap();
    let link_to_dir = tmp_dir.join("link_to_dir");
    symlink(&d, &link_to_dir).unwrap();
    let file = link_to_dir.join("file");
    fs::write(&file, b"").unwrap();
    let link1 = link_to_dir.join("link1");
    symlink(&file, &link1).unwrap();
    let link2 = link_to_dir.join("link2");
    symlink(&link1, &link2).unwrap();

    // /tmp may itself be a symlink!
    let real_tmp_dir = eval_symlinks(tmp_dir).unwrap();
    let real_dir = real_tmp_dir.join("dir");
    let real_file = real_dir.join("file");

    let tests = [
        (&d, &real_dir),
        (&link_to_dir, &real_dir),
        (&file, &real_file),
        (&link1, &real_file),
        (&link2, &real_file),
    ];
    for (i, (path, want)) in tests.into_iter().enumerate() {
        let have = eval_symlinks(path).unwrap();
        assert_eq!(
            &have, want,
            "test#{i}: eval_symlinks({path:?}) returns {have:?}, want {want:?}"
        );
    }
}

// Issue 57905.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_relative_symlink_to_absolute() {
    let td = TempDir::new();
    let tmp_dir = &td.0;

    // Create "link" as a symlink to an arbitrary absolute path. On macOS this path is likely to
    // begin with a symlink itself (/var or /tmp).
    let link = tmp_dir.join("link");
    symlink(tmp_dir, &link).unwrap();

    let p = eval_symlinks(&link).unwrap();
    let want = eval_symlinks(tmp_dir).unwrap();
    assert_eq!(p, want, "eval_symlinks({link:?}) = {p:?}; want {want:?}");
}

// Test directories relative to temporary directory. The tests are run in absTestDirs[0].
const ABS_TEST_DIRS: &[&str] = &["a", "a/b", "a/b/c"];

// Test paths relative to temporary directory. $ expands to the directory.
// The tests are run in absTestDirs[0]. We create absTestDirs first.
const ABS_TESTS: &[&str] = &[
    ".",
    "b",
    "b/",
    "../a",
    "../a/b",
    "../a/b/./c/../../.././a",
    "../a/b/./c/../../.././a/",
    "$",
    "$/.",
    "$/a/../a/b",
    "$/a/b/c/../../.././a",
    "$/a/b/c/../../.././a/",
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_abs() {
    let td = TempDir::new();
    let root = s(&td.0);
    for d in ABS_TEST_DIRS {
        fs::create_dir(td.0.join(d)).unwrap();
    }

    let cwd = std::env::current_dir().unwrap();
    for p in ABS_TESTS {
        // Go runs in <root>/a: a relative row is spelled under it
        let path = if p.contains('$') {
            p.replace('$', root)
        } else {
            format!("{root}/a/{p}")
        };
        let info = fs::metadata(&path).unwrap_or_else(|e| panic!("{path}: {e}"));

        let abspath = abs(Path::new(&path)).unwrap_or_else(|e| panic!("abs({path:?}) error: {e}"));
        let absinfo = fs::metadata(&abspath);
        assert!(
            absinfo.is_ok_and(|a| a.dev() == info.dev() && a.ino() == info.ino()),
            "abs({path:?})={abspath:?}, not the same file"
        );
        assert!(
            is_abs(s(&abspath)),
            "abs({path:?})={abspath:?}, not an absolute path"
        );
        assert_eq!(
            s(&abspath),
            clean(s(&abspath)),
            "abs({path:?})={abspath:?}, isn't clean"
        );

        // The real working directory takes the place of Go's chdir.
        if !p.contains('$') {
            let want = clean(&format!("{}/{p}", s(&cwd)));
            assert_eq!(
                abs(Path::new(p)).unwrap(),
                PathBuf::from(want),
                "abs({p:?})"
            );
        }
    }
}

// (root, path, want); want "err" means Rel must fail
const REL_TESTS: &[(&str, &str, &str)] = &[
    ("a/b", "a/b", "."),
    ("a/b/.", "a/b", "."),
    ("a/b", "a/b/.", "."),
    ("./a/b", "a/b", "."),
    ("a/b", "./a/b", "."),
    ("ab/cd", "ab/cde", "../cde"),
    ("ab/cd", "ab/c", "../c"),
    ("a/b", "a/b/c/d", "c/d"),
    ("a/b", "a/b/../c", "../c"),
    ("a/b/../c", "a/b", "../b"),
    ("a/b/c", "a/c/d", "../../c/d"),
    ("a/b", "c/d", "../../c/d"),
    ("a/b/c/d", "a/b", "../.."),
    ("a/b/c/d", "a/b/", "../.."),
    ("a/b/c/d/", "a/b", "../.."),
    ("a/b/c/d/", "a/b/", "../.."),
    ("../../a/b", "../../a/b/c/d", "c/d"),
    ("/a/b", "/a/b", "."),
    ("/a/b/.", "/a/b", "."),
    ("/a/b", "/a/b/.", "."),
    ("/ab/cd", "/ab/cde", "../cde"),
    ("/ab/cd", "/ab/c", "../c"),
    ("/a/b", "/a/b/c/d", "c/d"),
    ("/a/b", "/a/b/../c", "../c"),
    ("/a/b/../c", "/a/b", "../b"),
    ("/a/b/c", "/a/c/d", "../../c/d"),
    ("/a/b", "/c/d", "../../c/d"),
    ("/a/b/c/d", "/a/b", "../.."),
    ("/a/b/c/d", "/a/b/", "../.."),
    ("/a/b/c/d/", "/a/b", "../.."),
    ("/a/b/c/d/", "/a/b/", "../.."),
    ("/../../a/b", "/../../a/b/c/d", "c/d"),
    (".", "a/b", "a/b"),
    (".", "..", ".."),
    ("", "../../.", "../.."),
    // can't do purely lexically
    ("..", ".", "err"),
    ("..", "a", "err"),
    ("../..", "..", "err"),
    ("a", "/a", "err"),
    ("/a", "a", "err"),
];

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_rel() {
    for (root, path, want) in REL_TESTS {
        let got = rel(root, path);
        if *want == "err" {
            assert!(got.is_err(), "rel({root:?}, {path:?})={got:?}, want error");
            continue;
        }
        assert_eq!(got.as_deref(), Ok(*want), "rel({root:?}, {path:?})");
    }
    // Added: the error text (Go: "Rel: can't make " + targPath + " relative to " + basePath).
    assert_eq!(
        rel("a", "/a").unwrap_err().to_string(),
        "Rel: can't make /a relative to a"
    );
}

// Issue 30520 part 1.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_eval_symlinks_above_root() {
    let td = TempDir::new();
    let eval_tmp_dir = eval_symlinks(&td.0).unwrap();

    fs::create_dir(eval_tmp_dir.join("a")).unwrap();
    symlink(eval_tmp_dir.join("a"), eval_tmp_dir.join("b")).unwrap();
    fs::write(eval_tmp_dir.join("a").join("file"), b"").unwrap();

    // Count the number of ".." elements to get to the root directory.
    let eval = s(&eval_tmp_dir);
    let c = eval.matches('/').count();
    let dd = vec![".."; c + 2];

    let want_suffix = "a/file";

    // Try different numbers of "..".
    for i in [c, c + 1, c + 2] {
        let dots = dd[..i].join("/");
        let check = [eval, dots.as_str(), &eval[1..], "b", "file"].join("/");
        let resolved = eval_symlinks(Path::new(&check));
        match resolved {
            // On darwin, the temp dir is sometimes cleaned up mid-test (issue 37910).
            Err(e) if cfg!(target_os = "macos") && e.kind() == io::ErrorKind::NotFound => {
                eprintln!("skipping flaky darwin case (go.dev/issue/37910)");
                return;
            }
            Err(e) => panic!("eval_symlinks({check:?}) failed: {e}"),
            Ok(r) => assert!(
                s(&r).ends_with(want_suffix),
                "eval_symlinks({check:?}) = {r:?} does not end with {want_suffix:?}"
            ),
        }
    }
}

// Issue 30520 part 2.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_eval_symlinks_above_root_chdir() {
    let td = TempDir::new();
    let root = s(&td.0);

    fs::create_dir_all(td.0.join("a/b")).unwrap();
    symlink("a/b", td.0.join("c")).unwrap();
    fs::write(td.0.join("a/b/file"), b"").unwrap();

    fs::create_dir_all(td.0.join("d/e/f")).unwrap();

    // Go chdirs into d/e/f and resolves the relative ../../../c/file.
    let check = format!("{root}/d/e/f/../../../c/file");
    let want_suffix = "a/b/file";
    let resolved = eval_symlinks(Path::new(&check))
        .unwrap_or_else(|e| panic!("eval_symlinks({check:?}) failed: {e}"));
    assert!(
        s(&resolved).ends_with(want_suffix),
        "eval_symlinks({check:?}) = {resolved:?} does not end with {want_suffix:?}"
    );
}

#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_eval_symlinks_too_many_links() {
    let td = TempDir::new();
    let d = td.0.join("dir");
    symlink(&d, &d).unwrap();
    assert!(eval_symlinks(&d).is_err(), "expected error, got nil");
}

// Added (no Go source): behaviours that differ from what a Rust reader expects.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn go_not_rust_semantics() {
    assert_eq!(ext(".bashrc"), ".bashrc");
    assert_eq!(ext("a."), ".");
    assert_eq!(join(&["a", "/b"]), "a/b");
    assert_eq!(join(&["", ""]), "");
    assert_eq!(dir("a"), ".");
    assert_eq!(dir("/"), "/");
    assert_eq!(base(""), ".");
    assert_eq!(base("///"), "/");
}

// Added (Review Focus 5): a non-UTF-8 name is visited, sorted by bytes, and does not panic.
// Linux only in practice: macOS APFS rejects such names.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn walk_dir_non_utf8_name() {
    let td = TempDir::new();
    let bad = td.0.join(OsStr::from_bytes(b"\xff.txt"));
    touch(&td.0.join("z.txt"));
    touch(&bad);

    let mut seen: Vec<PathBuf> = Vec::new();
    walk_dir(&td.0, |path, _, err| {
        assert!(err.is_none());
        seen.push(path.to_path_buf());
        Walk::Continue
    })
    .unwrap();
    assert_eq!(seen, [td.0.clone(), td.0.join("z.txt"), bad.clone()]);

    let got = glob(&td.0.join("*.txt")).unwrap();
    assert_eq!(got, [td.0.join("z.txt"), bad]);
}
