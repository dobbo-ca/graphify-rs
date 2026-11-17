//! D-005 (plan R9): `GFY_CPUPROFILE` and `GFY_MEMPROFILE` are ignored. graphify-go
//! `cmd/graphify/prof_test.go` (3 tests) asserts the pprof files `startProf` writes; ported as
//! D-005 checks: a `build` with the variables set exits 0 and creates nothing (no file, no
//! directory, no warning). Plus the Task 20 added test `prof_ignored_d005`.

mod common;

use common::{TempDir, ok_in};

/// The names in `dir`, sorted.
#[allow(clippy::disallowed_methods)] // a test listing, sorted below
fn names(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read dir")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .into_string()
                .expect("UTF-8 name")
        })
        .collect();
    v.sort();
    v
}

/// A tree with one Go file, built with `env`; returns the build's stderr.
fn build_with(env: &[(&str, &str)]) -> String {
    let src = TempDir::new();
    src.write("a.go", "package p\n\nfunc A() {}\n");
    ok_in(src.path(), &["build", "."], env).stderr
}

// Go: both profiles are gzip pprof files. D-005: neither file exists.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_prof_writes_both_profiles() {
    let dir = TempDir::new();
    let (cpu, mem) = (dir.join("cpu.pprof"), dir.join("mem.pprof"));
    build_with(&[
        ("GFY_CPUPROFILE", cpu.to_str().unwrap()),
        ("GFY_MEMPROFILE", mem.to_str().unwrap()),
    ]);
    assert!(names(dir.path()).is_empty(), "D-005: profile files written");
}

// Both unset (empty) is a no-op in Go too: nothing is created in the working directory beyond
// the build's own graphify-out.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_prof_unset_is_no_op() {
    let src = TempDir::new();
    src.write("a.go", "package p\n\nfunc A() {}\n");
    ok_in(
        src.path(),
        &["build", "."],
        &[("GFY_CPUPROFILE", ""), ("GFY_MEMPROFILE", "")],
    );
    assert_eq!(names(src.path()), ["a.go", "graphify-out"]);
}

// Go warns `[graphify] warning: no profile: ...` and goes on. D-005: no warning, no entry.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_prof_bad_path_does_not_panic() {
    let dir = TempDir::new();
    let stderr = build_with(&[
        (
            "GFY_CPUPROFILE",
            dir.join("missing/cpu.pprof").to_str().unwrap(),
        ),
        (
            "GFY_MEMPROFILE",
            dir.join("missing/mem.pprof").to_str().unwrap(),
        ),
    ]);
    assert!(!stderr.contains("profile"), "stderr = {stderr:?}");
    assert!(names(dir.path()).is_empty(), "bad path created entries");
}

// Task 20 added test: with both variables naming a path, `build` and `update` exit 0 and no
// profile file appears (R9).
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn prof_ignored_d005() {
    let dir = TempDir::new();
    let src = TempDir::new();
    src.write("a.go", "package p\n\nfunc A() {}\n");
    let (cpu, mem) = (dir.join("cpu.pprof"), dir.join("mem.pprof"));
    let env = [
        ("GFY_CPUPROFILE", cpu.to_str().unwrap()),
        ("GFY_MEMPROFILE", mem.to_str().unwrap()),
    ];
    ok_in(src.path(), &["build", "."], &env);
    ok_in(src.path(), &["update", "."], &env);
    assert!(
        !cpu.exists() && !mem.exists(),
        "D-005: a profile was written"
    );
}
