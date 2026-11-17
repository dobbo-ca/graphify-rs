//! Port of `gitignore_test.go` (3 tests).

use super::{collected, put};
use crate::detect::gitignore::parse_ignore;
use crate::testutil::TempDir;

/// Go `TestCollectFilesGitignore`: `.gitignore` honoured as git does (the expected sets were
/// checked against `git check-ignore`).
#[test]
fn test_collect_files_gitignore() {
    let root = TempDir::new();
    put(
        root.path(),
        ".gitignore",
        "build/\n/rootonly.go\ngen_*.go\nvendored/**\n!vendored/keep.go\n",
    );
    put(root.path(), "sub/.gitignore", "*.py\n");
    for f in [
        "keep.go",
        "rootonly.go",
        "gen_a.go",
        "build/b.go",
        "sub/rootonly.go",
        "sub/c.go",
        "sub/d.py",
        "vendored/v.go",
        "vendored/keep.go",
    ] {
        put(root.path(), f, "package x\n");
    }

    let got = collected(root.path());
    let want = ["keep.go", "sub/rootonly.go", "sub/c.go", "vendored/keep.go"];
    let ignored = [
        "rootonly.go",
        "gen_a.go",
        "build/b.go",
        "sub/d.py",
        "vendored/v.go",
    ];
    for w in want {
        assert!(
            got.contains(w),
            "expected {w:?} to be collected, but it was excluded"
        );
    }
    for ig in ignored {
        assert!(
            !got.contains(ig),
            "expected {ig:?} to be gitignored, but it was collected"
        );
    }
    assert_eq!(
        got.len(),
        want.len(),
        "collected {} files, want {}: {got:?}",
        got.len(),
        want.len()
    );
}

/// Go `TestCollectFilesGraphifyignore`: `.graphifyignore` layers on `.gitignore` (its rules come
/// after, so its `!` wins), and applies in a directory with no `.gitignore`.
#[test]
fn test_collect_files_graphifyignore() {
    let root = TempDir::new();
    put(root.path(), ".gitignore", "build/\n");
    put(
        root.path(),
        ".graphifyignore",
        "prod_dump.go\ndata/*.go\n!data/keep.go\n",
    );
    put(root.path(), "sub/.graphifyignore", "*.py\n");
    for f in [
        "keep.go",
        "prod_dump.go",
        "build/b.go",
        "data/drop.go",
        "data/keep.go",
        "sub/c.go",
        "sub/d.py",
    ] {
        put(root.path(), f, "package x\n");
    }

    let got = collected(root.path());
    let want = ["keep.go", "data/keep.go", "sub/c.go"];
    let excluded = ["prod_dump.go", "build/b.go", "data/drop.go", "sub/d.py"];
    for w in want {
        assert!(
            got.contains(w),
            "expected {w:?} to be collected, but it was excluded"
        );
    }
    for ex in excluded {
        assert!(
            !got.contains(ex),
            "expected {ex:?} to be excluded, but it was collected"
        );
    }
    assert_eq!(
        got.len(),
        want.len(),
        "collected {} files, want {}: {got:?}",
        got.len(),
        want.len()
    );
}

/// Go `TestIgnoreRuleLiteralsNeverRejectAMatch`: the literal prefix and suffix checked before a
/// rule's regexp hold for every path the regexp matches.
#[test]
fn test_ignore_rule_literals_never_reject_a_match() {
    let globs = [
        "logs",
        "*.log",
        "npm-debug.log*",
        "report.[0-9]*.[0-9]*.json",
        "a?c",
        "?x",
        "x?",
        "/build",
        "/out/*.js",
        "docs/**/gen",
        "**/tmp",
        "src/**",
        "a/*/c",
        "*",
        "**",
        "a*b*c",
        "node_modules/",
        "!keep.log",
        r"\#lit",
        "a.b+c",
        "é*ü",
        "*.pid.lock",
        ".pnp.*",
    ];
    let parts = [
        "",
        "a",
        "b",
        "c",
        "x",
        "logs",
        ".log",
        "npm-debug.log",
        "report.[0-9]",
        "1",
        ".json",
        "build",
        "out",
        "/",
        ".js",
        "docs",
        "gen",
        "tmp",
        "src",
        "keep",
        "#lit",
        "a.b+c",
        "é",
        "ü",
        ".pid.lock",
        ".pnp.",
        "node_modules",
    ];
    let mut targets = Vec::new();
    for a in parts {
        for b in parts {
            for c in parts {
                targets.push(format!("{a}{b}{c}"));
            }
        }
    }
    let mut matched = 0;
    for (i, r) in parse_ignore("", &globs.join("\n")).rules.iter().enumerate() {
        for tg in &targets {
            if !r.matches(tg) {
                continue;
            }
            matched += 1;
            assert!(
                tg.starts_with(&r.prefix) && tg.ends_with(&r.suffix),
                "rule {i} (prefix {:?}, suffix {:?}) matches {tg:?} but the literal check rejects it",
                r.prefix,
                r.suffix
            );
        }
    }
    assert!(
        matched >= 100,
        "only {matched} matches: the test no longer exercises the check"
    );
}
