//! graphify-go `cmd/graphify/update_test.go`: `TestReportTitleMatchesBetweenBuildAndUpdate` (8
//! cases x 2 `GRAPHIFY_OUT` settings) and the `GRAPHIFY_FORCE=1` step of
//! `TestForceBypassesCache`, which change the working directory or the environment, so they run
//! the binary. The other update tests are unit tests (`src/cli/tests/update.rs`).

mod common;

use std::path::Path;

use common::{TempDir, ok_in, read, s};

// `update` titles the report like `build` does for the same root argument.
#[cfg(unix)]
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_report_title_matches_between_build_and_update() {
    let holder = TempDir::new();
    let dir = holder.join("d");
    std::fs::create_dir_all(dir.join("sub")).expect("mkdir");
    common::write(&dir.join("a.go"), "package p\n\nfunc A() {}\n");
    let link = holder.join("link");
    std::os::unix::fs::symlink(&dir, &link).expect("symlink");
    let sub = dir.join("sub");
    let cases: [(&str, &Path, &[&str]); 8] = [
        ("bare", &dir, &[]),
        ("dot", &dir, &["."]),
        ("dot slash", &dir, &["./"]),
        ("unclean", &dir, &["sub/.."]),
        ("abs", &dir, &[s(&dir)]),
        ("parent", &sub, &[".."]),
        ("bare in symlinked cwd", &link, &[]),
        ("dot in symlinked cwd", &link, &["."]),
    ];
    for (name, cwd, args) in cases {
        for env_on in [false, true] {
            let out_tmp = TempDir::new();
            let mut out = dir.join("graphify-out");
            let _ = std::fs::remove_dir_all(&out);
            let env_val = if env_on {
                out = out_tmp.join("out");
                s(&out).to_string()
            } else {
                String::new()
            };
            // Go's t.Chdir sets PWD, which os.Getwd returns when it names the cwd: the
            // symlinked spelling survives.
            let env = [("GRAPHIFY_OUT", env_val.as_str()), ("PWD", s(cwd))];
            let title = || {
                let b = read(&out.join("GRAPH_REPORT.md"));
                b.split('\n').next().unwrap_or("").to_string()
            };
            let mut a = vec!["build"];
            a.extend_from_slice(args);
            ok_in(cwd, &a, &env);
            let built = title();
            a[0] = "update";
            ok_in(cwd, &a, &env);
            assert_eq!(
                title(),
                built,
                "{name}/GRAPHIFY_OUT={env_on}: update title vs build title"
            );
        }
    }
}

// The GRAPHIFY_FORCE=1 step of TestForceBypassesCache: with the cache poisoned (a.go's entry
// holds the result of a version without C), the env var makes update re-parse and restore C.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_force_bypasses_cache_env() {
    use graphify::cache;
    let root = TempDir::new();
    let r = root.path();
    root.write("a.go", "package p\n\nfunc A() {}\n\nfunc C() {}\n");
    ok_in(r, &["build", s(r)], &[]);
    let graph_path = root.join("graphify-out/graph.json");
    let full = read(&graph_path);
    assert!(full.contains("\"C()\""), "build lacks C()");

    // Go `cacheStamp()`: the stamp of version "dev" and the default parse budget.
    let stamp = format!("{}-b{}", cache::stamp("dev"), graphify::ts::budget());
    let cache_path = root.join(&format!("graphify-out/{}", cache::FILE_NAME));
    let mut c = cache::load(&cache_path, &stamp, 1);
    let e = c.get_mut("a.go").expect("a.go cached");
    e.result = graphify::extract::file_from_bytes("a.go", b"package p\n\nfunc A() {}\n");
    if let Err(e) = cache::save(&cache_path, &stamp, &c) {
        panic!("save cache: {e}");
    }

    ok_in(r, &["update", s(r)], &[("GRAPHIFY_FORCE", "1")]);
    assert!(
        read(&graph_path).contains("\"C()\""),
        "GRAPHIFY_FORCE=1 update kept the poisoned entry"
    );
}
