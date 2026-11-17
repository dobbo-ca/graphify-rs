//! Port of `detect_parity_test.go` (6 tests).

use std::ffi::OsString;
use std::os::unix::fs::symlink;
use std::path::Path;

use super::{collect, collect_env, env_cwd, put, resolved};
use crate::detect::detect::{Env, has_build_output_markers};
use crate::testutil::TempDir;

/// Go `TestSymlinkContainment`: a file symlink is collected only when its target stays under the
/// root and is a regular file; directory symlinks are never followed.
#[test]
fn test_symlink_containment() {
    let base = TempDir::new();
    let root = base.join("root");
    let outside = base.join("outside");
    put(&root, "real.go", "package a\n");
    put(&root, "sub/in.go", "package a\n");
    put(&outside, "out.go", "package a\n");
    put(&outside, "dir/x.go", "package a\n");
    let links = [
        ("inlink.go", root.join("sub/in.go")),
        ("outlink.go", outside.join("out.go")),
        ("broken.go", base.join("missing.go")),
        ("dirlink", root.join("sub")),
        ("outdirlink", outside.join("dir")),
        ("chain1.go", root.join("chain2.go")),
        ("..foo.go", root.join("real.go")),
    ];
    for (name, target) in &links {
        symlink(target, root.join(name)).expect("symlink");
    }
    symlink(outside.join("out.go"), root.join("chain2.go")).expect("symlink");
    let got = collect(&root);
    let want = ["..foo.go", "inlink.go", "real.go", "sub/in.go"];
    assert_eq!(got, want, "got {got:?} want {want:?}");
}

/// Go `TestNoiseDirs`: the name-only and evidence-gated noise dirs.
#[test]
fn test_noise_dirs() {
    // (dir, evidence, pruned): evidence "" means the name alone prunes.
    const CASES: &[(&str, &str, bool)] = &[
        ("venv", "", true),
        (".venv", "", true),
        ("__pycache__", "", true),
        ("lcov-report", "", true),
        ("visual-tests", "", true),
        ("visual-test", "", true),
        ("storybook-static", "", true),
        ("dist-protected", "", true),
        (".angular", "", true),
        (".parcel-cache", "", true),
        (".serverless", "", true),
        (".obsidian", "", true),
        (".smart-env", "", true),
        ("my_venv", "", true),
        ("env", "pyvenv.cfg", true),
        (".env", "bin/activate", true),
        ("x_env", "lib/python3.12/a", true),
        ("env", "conda-meta/a", true),
        ("env", "", false),
        (".env", "", false),
        ("x_env", "", false),
        ("coverage", "lcov.info", true),
        ("coverage", "lcov-report/a", true),
        ("coverage", "", false),
        ("out", "production/a", true),
        ("out", "_next/a", true),
        ("out", "A.CLASS", true),
        ("out", "sub/a.class", true),
        ("out", "sub/a.js.map", true),
        ("out", "", false),
        ("out", "sub/Repo.java", false),
        (".vscode", "", false),
        ("snapshots", "a.snap", true),
        ("snapshots", "", false),
    ];
    for &(dir, evidence, pruned) in CASES {
        let root = TempDir::new();
        put(root.path(), &format!("p/{dir}/keep.ts"), "x\n");
        if !evidence.is_empty() {
            put(root.path(), &format!("p/{dir}/{evidence}"), "x\n");
        }
        let got = collect(root.path()).contains(&format!("p/{dir}/keep.ts"));
        assert_ne!(
            got, pruned,
            "{dir} evidence {evidence:?}: pruned={} want {pruned}",
            !got
        );
    }
    let root = TempDir::new();
    put(root.path(), "__tests__/snapshots/keep.ts", "x\n");
    put(root.path(), "adapter/out/Repo.java", "class A {}\n");
    let got = collect(root.path());
    assert!(
        !got.iter().any(|f| f == "__tests__/snapshots/keep.ts")
            && got.iter().any(|f| f == "adapter/out/Repo.java"),
        "got {got:?}"
    );
}

/// Go `TestRelativeRootSymlinkAndSnapshots`: root `.` (Go `t.Chdir`) keeps in-root file symlinks,
/// and a `snapshots/` under a cwd named `__tests__` is pruned.
#[test]
fn test_relative_root_symlink_and_snapshots() {
    let (_d, root) = resolved();
    put(&root, "b.go", "package a\n");
    put(&root, "sub/d.go", "package a\n");
    symlink("b.go", root.join("a.go")).expect("symlink");
    symlink("../b.go", root.join("sub/c.go")).expect("symlink");
    let got = collect_env(Path::new("."), &env_cwd(root.clone()));
    assert_eq!(
        got,
        ["a.go", "b.go", "sub/c.go", "sub/d.go"],
        "relative root: {got:?}"
    );

    let (_d2, r2) = resolved();
    let tests = r2.join("__tests__");
    put(&tests, "c.go", "package a\n");
    put(&tests, "snapshots/x.go", "package a\n");
    let got = collect_env(Path::new("."), &env_cwd(tests));
    assert_eq!(got, ["c.go"], "snapshots under relative root: {got:?}");
}

/// Go `TestGraphifyOutBeatsNoiseGate`: `GRAPHIFY_OUT=out` prunes `out/` by name before the
/// build-output evidence probe.
#[test]
fn test_graphify_out_beats_noise_gate() {
    let (_d, root) = resolved();
    put(&root, "control.go", "package a\n");
    put(&root, "out/cache/x.go", "package a\n");
    let env = Env {
        graphify_out: Some(OsString::from("out")),
        ..Env::process()
    };
    let got = collect_env(&root, &env);
    let abs = root.join("control.go").to_string_lossy().into_owned();
    assert!(
        got == [abs.as_str()] || got == ["control.go"],
        "GRAPHIFY_OUT=out: {got:?}"
    );
}

/// Go `TestBuildOutputProbeUsesDirOrder`: the probe of the dir itself reads it unsorted and still
/// finds a marker one level down.
#[test]
fn test_build_output_probe_uses_dir_order() {
    let (_d, root) = resolved();
    put(&root, "out/yy/X.class", "");
    put(&root, "out/m.go", "package a\n");
    assert!(
        has_build_output_markers(&root.join("out")),
        "expected build output markers"
    );
}

/// Go `TestSymlinkedRootWithDotDot`: a root of `<symlink>/../proj` resolves `..` after the
/// symlink, as the kernel does.
#[test]
fn test_symlinked_root_with_dot_dot() {
    let (_d, base) = resolved();
    put(&base, "deep/inner/x", "");
    put(&base, "deep/proj/a.go", "package a\n");
    symlink("a.go", base.join("deep/proj/l.go")).expect("symlink");
    symlink(base.join("deep/inner"), base.join("lnk")).expect("symlink");
    let root = format!("{}/../proj", base.join("lnk").display());
    let got = collect(Path::new(&root));
    assert_eq!(got, ["a.go", "l.go"], "got {got:?}");
}
