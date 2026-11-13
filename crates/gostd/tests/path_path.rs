//! Ported from path/path_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2009 The Go Authors) against
//! `gostd::path`. Source line numbers are those of go1.27.4.
//! - TestClean (68), TestJoin (140), TestExt (160), TestBase (183), TestDir (207), TestIsAbs (231):
//!   ported, all rows
//!
//! Not ported:
//! - TestCleanMallocs (79): `testing.AllocsPerRun` is a Go runtime feature
//! - TestSplit (108): Split is not reached by graphify-go
//!
//! path_test.go has 8 test funcs: 6 ported, 2 not.

use gostd::path::{base, clean, dir, ext, join};

// (path, result)
const CLEAN_TESTS: &[(&str, &str)] = &[
    // Already clean
    ("", "."),
    ("abc", "abc"),
    ("abc/def", "abc/def"),
    ("a/b/c", "a/b/c"),
    (".", "."),
    ("..", ".."),
    ("../..", "../.."),
    ("../../abc", "../../abc"),
    ("/abc", "/abc"),
    ("/", "/"),
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
    ("//abc", "/abc"),
    ("///abc", "/abc"),
    ("//abc//", "/abc"),
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
    // Combinations
    ("abc/./../def", "def"),
    ("abc//./../def", "def"),
    ("abc/../../././../def", "../../def"),
];

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
    (&["a"], "a"),
    // two parameters
    (&["a", "b"], "a/b"),
    (&["a", ""], "a"),
    (&["", "b"], "b"),
    (&["/", "a"], "/a"),
    (&["/", ""], "/"),
    (&["a/", "b"], "a/b"),
    (&["a/", ""], "a"),
    (&["", ""], ""),
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

// (path, result)
const BASE_TESTS: &[(&str, &str)] = &[
    // Already clean
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
    ("////", "/"),
    ("/foo", "/"),
    ("x/", "x"),
    ("abc", "."),
    ("abc/def", "abc"),
    ("abc////def", "abc"),
    ("a/b/.x", "a/b"),
    ("a/b/c.", "a/b"),
    ("a/b/c.x", "a/b"),
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

// Go `path.IsAbs` is `len(path) > 0 && path[0] == '/'`: on unix the same code as filepath's,
// re-exported by gostd::path.
#[test]
#[ignore = "graphify-rs-aa6.15"]
fn test_is_abs() {
    for (path, want) in IS_ABS_TESTS {
        assert_eq!(gostd::path::is_abs(path), *want, "is_abs({path:?})");
    }
}
