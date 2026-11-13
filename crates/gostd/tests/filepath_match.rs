//! Ported from path/filepath/match_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2009 The Go
//! Authors), unix rows only. Source line numbers are those of go1.27.4.
//! - TestMatch (90): ported, all 56 matchTests rows (the Windows branch is left out)
//! - TestGlob (135): ported, adapted. Go globs relative to its working directory (the package
//!   source dir, holding match.go); Rust tests do not chdir, so the patterns are absolute under a
//!   temp dir laid out as `<tmp>/filepath/match.go`. `Glob` cleans what it joins, so the wanted
//!   path for `../*/match.go` is `<tmp>/filepath/match.go` (Go's relative result is already clean)
//! - TestGlobError (174), TestCVE202230632 (164), TestGlobSymlink (197), TestNonWindowsGlobEscape
//!   (363): ported (TestNonWindowsGlobEscape with the pattern under a temp dir, no chdir)
//!
//! Not ported:
//! - TestGlobUNC (183): Windows UNC paths
//! - TestWindowsGlob (276): Windows only (skipped by Go on unix)
//!
//! match_test.go has 8 test funcs: 6 ported, 2 not.

use gostd::filepath::{BadPattern, glob, match_pattern};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A temp dir removed when it goes out of scope (also when a test panics).
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "gostd-filepath-match-{}-{}",
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

// (pattern, s, match, err)
const MATCH_TESTS: &[(&str, &str, bool, Option<BadPattern>)] = &[
    ("abc", "abc", true, None),
    ("*", "abc", true, None),
    ("*c", "abc", true, None),
    ("a*", "a", true, None),
    ("a*", "abc", true, None),
    ("a*", "ab/c", false, None),
    ("a*/b", "abc/b", true, None),
    ("a*/b", "a/c/b", false, None),
    ("a*b*c*d*e*/f", "axbxcxdxe/f", true, None),
    ("a*b*c*d*e*/f", "axbxcxdxexxx/f", true, None),
    ("a*b*c*d*e*/f", "axbxcxdxe/xxx/f", false, None),
    ("a*b*c*d*e*/f", "axbxcxdxexxx/fff", false, None),
    ("a*b?c*x", "abxbbxdbxebxczzx", true, None),
    ("a*b?c*x", "abxbbxdbxebxczzy", false, None),
    ("ab[c]", "abc", true, None),
    ("ab[b-d]", "abc", true, None),
    ("ab[e-g]", "abc", false, None),
    ("ab[^c]", "abc", false, None),
    ("ab[^b-d]", "abc", false, None),
    ("ab[^e-g]", "abc", true, None),
    ("a\\*b", "a*b", true, None),
    ("a\\*b", "ab", false, None),
    ("a?b", "a☺b", true, None),
    ("a[^a]b", "a☺b", true, None),
    ("a???b", "a☺b", false, None),
    ("a[^a][^a][^a]b", "a☺b", false, None),
    ("[a-ζ]*", "α", true, None),
    ("*[a-ζ]", "A", false, None),
    ("a?b", "a/b", false, None),
    ("a*b", "a/b", false, None),
    ("[\\]a]", "]", true, None),
    ("[\\-]", "-", true, None),
    ("[x\\-]", "x", true, None),
    ("[x\\-]", "-", true, None),
    ("[x\\-]", "z", false, None),
    ("[\\-x]", "x", true, None),
    ("[\\-x]", "-", true, None),
    ("[\\-x]", "a", false, None),
    ("[]a]", "]", false, Some(BadPattern)),
    ("[-]", "-", false, Some(BadPattern)),
    ("[x-]", "x", false, Some(BadPattern)),
    ("[x-]", "-", false, Some(BadPattern)),
    ("[x-]", "z", false, Some(BadPattern)),
    ("[-x]", "x", false, Some(BadPattern)),
    ("[-x]", "-", false, Some(BadPattern)),
    ("[-x]", "a", false, Some(BadPattern)),
    ("\\", "a", false, Some(BadPattern)),
    ("[a-b-c]", "a", false, Some(BadPattern)),
    ("[", "a", false, Some(BadPattern)),
    ("[^", "a", false, Some(BadPattern)),
    ("[^bc", "a", false, Some(BadPattern)),
    ("a[", "a", false, Some(BadPattern)),
    ("a[", "ab", false, Some(BadPattern)),
    ("a[", "x", false, Some(BadPattern)),
    ("a/b[", "x", false, Some(BadPattern)),
    ("*x", "xxx", true, None),
];

#[test]
fn test_match() {
    for (pattern, s, matched, err) in MATCH_TESTS {
        let want = err.map_or(Ok(*matched), Err);
        assert_eq!(
            match_pattern(pattern, s),
            want,
            "match_pattern({pattern:?}, {s:?})"
        );
    }
}

// (pattern, result)
const GLOB_TESTS: &[(&str, &str)] = &[
    ("match.go", "match.go"),
    ("mat?h.go", "match.go"),
    ("*", "match.go"),
    ("../*/match.go", "../filepath/match.go"),
];

#[test]
fn test_glob() {
    let td = TempDir::new();
    // Go runs in the filepath package dir, whose parent holds it as "filepath".
    let cwd = td.0.join("filepath");
    fs::create_dir(&cwd).unwrap();
    fs::write(cwd.join("match.go"), b"").unwrap();

    for (pattern, result) in GLOB_TESTS {
        let pattern = cwd.join(pattern);
        // Glob joins with Join, which cleans: "../filepath/match.go" below cwd is cwd/match.go.
        let want = if result.starts_with("../") {
            cwd.join("match.go")
        } else {
            cwd.join(result)
        };
        let matches = glob(&pattern).unwrap_or_else(|e| panic!("glob error for {pattern:?}: {e}"));
        assert!(
            matches.contains(&want),
            "glob({pattern:?}) = {matches:?} want {want:?}"
        );
    }
    for pattern in ["no_match", "../*/no_match"] {
        let pattern = cwd.join(pattern);
        let matches = glob(&pattern).unwrap_or_else(|e| panic!("glob error for {pattern:?}: {e}"));
        assert!(
            matches.is_empty(),
            "glob({pattern:?}) = {matches:?} want []"
        );
    }
}

#[test]
fn test_cve202230632() {
    // Prior to CVE-2022-30632, this would cause a stack exhaustion given a
    // large number of separators (more than 4,000,000). There is now a limit
    // of 10,000.
    let pattern = format!("/*{}", "/".repeat(10001));
    assert_eq!(glob(Path::new(&pattern)), Err(BadPattern));
}

#[test]
fn test_glob_error() {
    for pattern in ["[]", "nonexist/[]"] {
        assert_eq!(
            glob(Path::new(pattern)),
            Err(BadPattern),
            "glob({pattern:?}) want BadPattern"
        );
    }
}

// (path, dest, brokenLink)
const GLOB_SYMLINK_TESTS: &[(&str, &str, bool)] =
    &[("test1", "link1", false), ("test2", "link2", true)];

#[test]
fn test_glob_symlink() {
    let td = TempDir::new();
    for (path, dest, broken_link) in GLOB_SYMLINK_TESTS {
        let path = td.0.join(path);
        let dest = td.0.join(dest);
        fs::File::create(&path).unwrap();
        symlink(&path, &dest).unwrap();
        if *broken_link {
            // Break the symlink.
            let _ = fs::remove_file(&path);
        }
        let matches =
            glob(&dest).unwrap_or_else(|e| panic!("glob symlink error for {dest:?}: {e}"));
        assert!(
            matches.contains(&dest),
            "glob({dest:?}) = {matches:?} want {dest:?}"
        );
    }
}

#[test]
fn test_non_windows_glob_escape() {
    let td = TempDir::new();
    fs::write(td.0.join("match.go"), b"").unwrap();
    // Go: pattern `\match.go` in the working directory, want "match.go".
    let mut pattern = td.0.clone().into_os_string();
    pattern.push("/\\match.go");
    let matches = glob(Path::new(&pattern)).unwrap();
    assert_eq!(matches, [td.0.join("match.go")]);
}
