//! Port of `githead_test.go` (4 tests; unix, for the symlink cases and the owner check). Go
//! clears the `gitEnv` variables with `t.Setenv` + `Unsetenv`; here the reader is
//! [`read_git_head_with`] over an empty environment, and `git_head` (which also runs `git`)
//! needs the variables unset in the test process, which the first assert checks.

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{GIT_ENV, config_needs_git, git_head, read_git_head_with};
use crate::cli::maintenance::maint_util::git_run;
use crate::testutil::TempDir;

const COMMIT: [&str; 10] = [
    "-c",
    "commit.gpgsign=false",
    "-c",
    "user.name=t",
    "-c",
    "user.email=t@t",
    "commit",
    "-q",
    "--allow-empty",
    "-m",
];

fn no_env(_: &str) -> bool {
    false
}

fn commit(dir: &Path, msg: &str) {
    let mut a: Vec<&str> = COMMIT.to_vec();
    a.push(msg);
    git_run(dir, &a);
}

/// `git -C dir rev-parse HEAD`, or `""` when git fails.
fn rev_parse(dir: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("run git");
    if !out.status.success() {
        return String::new();
    }
    gostd::strings::trim_space(std::str::from_utf8(&out.stdout).expect("UTF-8 sha")).to_string()
}

fn assert_git_env_unset() {
    for k in GIT_ENV {
        assert!(
            std::env::var_os(k).is_none(),
            "{k} is set in the test environment; Go clears it with t.Setenv"
        );
    }
}

/// Temp dirs that live as long as the case (Go `t.TempDir()`).
struct Dirs(Vec<TempDir>);

impl Dirs {
    fn temp(&mut self) -> PathBuf {
        let d = TempDir::new();
        let p = std::fs::canonicalize(d.path()).expect("canonical temp dir");
        self.0.push(d);
        p
    }

    fn new_repo(&mut self) -> PathBuf {
        let d = self.temp();
        git_run(&d, &["init", "-q", "-b", "main"]);
        commit(&d, "x");
        d
    }
}

type Setup = fn(&mut Dirs) -> PathBuf;

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_read_git_head_matches_git() {
    assert_git_env_unset();
    let tests: [(&str, Setup); 9] = [
        ("branch", |d| d.new_repo()),
        ("packed refs", |d| {
            let r = d.new_repo();
            git_run(&r, &["pack-refs", "--all"]);
            let _ = std::fs::remove_file(r.join(".git/refs/heads/main"));
            r
        }),
        ("detached", |d| {
            let r = d.new_repo();
            git_run(&r, &["checkout", "-q", "--detach"]);
            r
        }),
        ("worktree branch", |d| {
            let r = d.new_repo();
            let w = d.temp().join("w");
            git_run(
                &r,
                &["worktree", "add", "-q", "-b", "other", w.to_str().unwrap()],
            );
            commit(&w, "x");
            w
        }),
        ("worktree detached", |d| {
            let r = d.new_repo();
            let w = d.temp().join("w");
            git_run(
                &r,
                &["worktree", "add", "-q", "--detach", w.to_str().unwrap()],
            );
            w
        }),
        ("subdirectory", |d| {
            let r = d.new_repo();
            let sub = r.join("a/b");
            std::fs::create_dir_all(&sub).unwrap();
            sub
        }),
        ("unborn", |d| {
            let r = d.temp();
            git_run(&r, &["init", "-q", "-b", "main"]);
            r
        }),
        ("symbolic ref chain", |d| {
            let r = d.new_repo();
            git_run(&r, &["symbolic-ref", "refs/heads/alias", "refs/heads/main"]);
            git_run(&r, &["symbolic-ref", "HEAD", "refs/heads/alias"]);
            r
        }),
        ("no repo", |d| d.temp()),
    ];
    for (name, setup) in tests {
        let mut d = Dirs(Vec::new());
        let dir = setup(&mut d);
        let want = rev_parse(&dir);
        let (got, ok) = read_git_head_with(&dir, &no_env);
        assert!(
            ok,
            "{name}: reader fell back, want in-process answer {want:?}"
        );
        assert_eq!(got, want, "{name}");
    }
}

// Odd layouts may fall back to git, but must never disagree with it.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_read_git_head_never_disagrees() {
    assert_git_env_unset();
    let tests: [(&str, Setup); 15] = [
        ("symlink into other repo", |d| {
            let (a, b) = (d.new_repo(), d.new_repo());
            std::fs::create_dir(b.join("sub")).unwrap();
            symlink(b.join("sub"), a.join("link")).unwrap();
            a.join("link")
        }),
        ("symlink out of repo", |d| {
            let a = d.new_repo();
            std::fs::create_dir(a.join("sub")).unwrap();
            let l = d.temp().join("link");
            symlink(a.join("sub"), &l).unwrap();
            l
        }),
        ("nested dotgit without objects", |d| {
            let r = d.new_repo();
            std::fs::create_dir_all(r.join("sub/.git/refs")).unwrap();
            std::fs::write(r.join("sub/.git/HEAD"), "ref: refs/heads/main\n").unwrap();
            r.join("sub")
        }),
        ("format version 2", |d| {
            let r = d.new_repo();
            git_run(&r, &["config", "core.repositoryformatversion", "2"]);
            r
        }),
        ("unknown extension", |d| {
            let r = d.new_repo();
            git_run(&r, &["config", "core.repositoryformatversion", "1"]);
            git_run(&r, &["config", "extensions.bogusext", "true"]);
            r
        }),
        ("ref chain too deep", |d| {
            let r = d.new_repo();
            git_run(&r, &["update-ref", "refs/heads/e", "HEAD"]);
            let mut prev = "e";
            for n in ["d", "c", "b", "a"] {
                std::fs::write(
                    r.join(".git/refs/heads").join(n),
                    format!("ref: refs/heads/{prev}\n"),
                )
                .unwrap();
                prev = n;
            }
            std::fs::write(r.join(".git/HEAD"), "ref: refs/heads/a\n").unwrap();
            r
        }),
        ("worktree admin dir", |d| {
            let r = d.new_repo();
            let w = d.temp().join("w");
            git_run(
                &r,
                &["worktree", "add", "-q", "-b", "other", w.to_str().unwrap()],
            );
            r.join(".git/worktrees/w")
        }),
        ("64 hex head", |d| {
            let r = d.new_repo();
            let h = std::fs::read_to_string(r.join(".git/refs/heads/main")).unwrap();
            let head = format!("{}{}\n", gostd::strings::trim_space(&h), "0".repeat(24));
            std::fs::write(r.join(".git/HEAD"), head).unwrap();
            r
        }),
        ("leading space head in nested repo", |d| {
            let r = d.new_repo();
            let inner = r.join("in");
            std::fs::create_dir(&inner).unwrap();
            git_run(&inner, &["init", "-q", "-b", "main"]);
            commit(&inner, "y");
            std::fs::write(inner.join(".git/HEAD"), " ref: refs/heads/main\n").unwrap();
            inner
        }),
        ("gitfile without space", |d| {
            let a = d.new_repo();
            let f = d.temp();
            let gitdir = format!("gitdir:{}\n", a.join(".git").display());
            std::fs::write(f.join(".git"), gitdir).unwrap();
            f
        }),
        ("format version 09", |d| {
            let r = d.new_repo();
            git_run(&r, &["config", "core.repositoryformatversion", "09"]);
            r
        }),
        ("bad refname", |d| {
            let r = d.new_repo();
            std::fs::write(r.join(".git/HEAD"), "ref: refs/heads//main\n").unwrap();
            r
        }),
        ("symlink ref", |d| {
            let r = d.new_repo();
            git_run(&r, &["branch", "b"]);
            git_run(&r, &["pack-refs", "--all"]);
            symlink("refs/heads/b", r.join(".git/refs/heads/a")).unwrap();
            std::fs::write(r.join(".git/HEAD"), "ref: refs/heads/a\n").unwrap();
            r
        }),
        ("gitfile to non-repo", |d| {
            let t = d.temp();
            let fake = t.join("fake");
            std::fs::create_dir(&fake).unwrap();
            std::fs::write(
                fake.join("HEAD"),
                "0123456789abcdef0123456789abcdef01234567\n",
            )
            .unwrap();
            let w = t.join("w");
            std::fs::create_dir(&w).unwrap();
            std::fs::write(w.join(".git"), format!("gitdir: {}\n", fake.display())).unwrap();
            w
        }),
        ("bare repo", |d| {
            let r = d.new_repo();
            let b = d.temp().join("b.git");
            git_run(
                &r,
                &[
                    "clone",
                    "-q",
                    "--bare",
                    r.to_str().unwrap(),
                    b.to_str().unwrap(),
                ],
            );
            b
        }),
    ];
    for (name, setup) in tests {
        let mut d = Dirs(Vec::new());
        let dir = setup(&mut d);
        let want = rev_parse(&dir);
        assert_eq!(git_head(&dir), want, "{name}: git_head");
        let (got, ok) = read_git_head_with(&dir, &no_env);
        assert!(!ok || got == want, "{name}: reader {got:?}, want {want:?}");
    }
}

// Go sets GIT_REFERENCE_BACKEND=reftable; the reader is given that environment.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_read_git_head_falls_back_on_ref_backend_env() {
    let dir = TempDir::new();
    let (_, ok) = read_git_head_with(dir.path(), &|k| k == "GIT_REFERENCE_BACKEND");
    assert!(!ok, "reader ignored GIT_REFERENCE_BACKEND");
}

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_config_needs_git() {
    const CASES: [(&str, bool); 10] = [
        (
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
            false,
        ),
        (
            "[branch \"worktree-x\"]\n\tmerge = refs/heads/worktree-x\n",
            false,
        ),
        ("[remote \"vscode-extensions\"]\n\turl = x/include\n", false),
        ("[core]\n\tworktree = ../w\n", true),
        ("[core] worktree = ../w\n", true),
        ("[extensions]\n\tworktreeConfig = true\n", true),
        ("[includeIf \"gitdir:x\"]\n\tpath = y\n", true),
        ("[include]\n\tpath = y\n", true),
        ("[core]\n\trepositoryformatversion = 1\n", true),
        ("[core]\n\trepositoryFormatVersion=0\n", false),
    ];
    for (cfg, want) in CASES {
        assert_eq!(
            config_needs_git(cfg.as_bytes()),
            want,
            "config_needs_git({cfg:?})"
        );
    }
}
