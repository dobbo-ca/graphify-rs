//! Port of `detect_test.go` (15 tests).

use std::collections::BTreeMap;
use std::ffi::OsString;

use super::{
    WORKERS, chmod, collect_env, collected, collected_env, env_cwd, must_write,
    readable_despite_chmod,
};
use crate::detect::detect::Env;
use crate::detect::{collect_files, collect_files_report, is_sensitive};
use crate::testutil::TempDir;

/// Go `TestCollectFilesSkipsTerraform`: `.terraform` (the `terraform init` cache) is pruned with
/// no `.gitignore` covering it.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_skips_terraform() {
    let root = TempDir::new();
    must_write(&root.join("main.tf"), r#"resource "aws_x" "y" {}"#);
    must_write(
        &root.join(".terraform/modules/vendored.tf"),
        r#"resource "aws_z" "w" {}"#,
    );

    let files = collect_files(root.path()).expect("CollectFiles");
    for f in &files {
        assert_eq!(f, "main.tf", "expected only main.tf, got {f:?}");
    }
    assert_eq!(files.len(), 1, "expected 1 file, got {files:?}");
}

/// Go `TestCollectFilesIncludesMCPConfigs`: MCP config files are matched by basename, and a
/// plain `.json` is collected too.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_includes_mcp_configs() {
    let root = TempDir::new();
    must_write(&root.join(".mcp.json"), r#"{"mcpServers": {}}"#);
    must_write(&root.join("mcp_servers.json"), r#"{"mcpServers": {}}"#);
    must_write(
        &root.join("sub/claude_desktop_config.json"),
        r#"{"mcpServers": {}}"#,
    );
    must_write(&root.join("package.json"), "{}");

    let got = collected(root.path());
    for want in [
        ".mcp.json",
        "mcp_servers.json",
        "sub/claude_desktop_config.json",
        "package.json",
    ] {
        assert!(
            got.contains(want),
            "expected {want:?} to be collected, got {got:?}"
        );
    }
}

/// Go `TestIsSensitiveSourceExemption`: a keyword in a source file's name does not drop it; the
/// credential stores and secret-keyword documents still drop.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_is_sensitive_source_exemption() {
    let kept = [
        "password_reset.go",
        "passwords_controller.rb",
        "credentials_controller.rb",
        "secret_store.py",
    ];
    for name in kept {
        assert!(
            !is_sensitive(name),
            "isSensitive({name:?}) = true, want false (real source, not a secret store)"
        );
    }
    let dropped = [
        "credentials.json",
        "secrets.yaml",
        ".env",
        "id_rsa",
        "server.pem",
        // Document formats: upstream FileType.DOCUMENT, not exempt.
        "secret_notes.md",
        "credentials.md",
        "password.mdx",
        "secrets.markdown",
    ];
    for name in dropped {
        assert!(
            is_sensitive(name),
            "isSensitive({name:?}) = false, want true (secret store)"
        );
    }
}

/// Go `TestIsSensitiveAdjacentKeywords`: two keywords with only a separator between them are
/// both seen, so the trailing one names the store.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_is_sensitive_adjacent_keywords() {
    let dropped = ["aws_secret_credentials.json", "foo_secret_password.txt"];
    for name in dropped {
        assert!(
            is_sensitive(name),
            "isSensitive({name:?}) = false, want true (trailing keyword names a secret store)"
        );
    }
    let kept = ["aws_secret_credentials.go"];
    for name in kept {
        assert!(
            !is_sensitive(name),
            "isSensitive({name:?}) = true, want false (real source, not a secret store)"
        );
    }
}

/// Go `TestCollectFilesKeepsKeywordNamedSource`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_keeps_keyword_named_source() {
    let root = TempDir::new();
    must_write(&root.join("password_reset.go"), "package p");
    must_write(
        &root.join("passwords_controller.rb"),
        "class PasswordsController; end",
    );
    must_write(
        &root.join("credentials_controller.rb"),
        "class CredentialsController; end",
    );
    must_write(&root.join("secret_store.py"), "x = 1");
    must_write(&root.join("credentials.json"), r#"{"password": "hunter2"}"#);

    let got = collected(root.path());
    for want in [
        "password_reset.go",
        "passwords_controller.rb",
        "credentials_controller.rb",
        "secret_store.py",
    ] {
        assert!(
            got.contains(want),
            "expected {want:?} to be collected, got {got:?}"
        );
    }
    assert!(
        !got.contains("credentials.json"),
        "expected credentials.json to be dropped as a secret store, got {got:?}"
    );
}

/// Go `TestCollectFilesReportsWalkError`: an unreadable directory is a walk error, and the walk
/// goes on to its siblings. Go skips as root (the permission bits do not bind).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_reports_walk_error() {
    let root = TempDir::new();
    must_write(&root.join("main.go"), "package p");
    let locked = root.join("locked");
    must_write(&locked.join("hidden.go"), "package p");
    chmod(&locked, 0o000);
    if readable_despite_chmod(&locked) {
        chmod(&locked, 0o755);
        return; // running as root bypasses directory permission bits
    }

    let rep = collect_files_report(root.path(), WORKERS);
    chmod(&locked, 0o755); // restore so TempDir cleanup can remove it
    let rep = rep.expect("CollectFilesReport");
    assert!(
        !rep.walk_errors.is_empty(),
        "expected a walk error for the unreadable dir, got none"
    );
    assert!(
        rep.walk_errors.iter().any(|we| we.contains("locked")),
        "expected a walk error mentioning {:?}, got {:?}",
        "locked",
        rep.walk_errors
    );
    assert!(
        rep.files.iter().any(|f| f == "main.go"),
        "expected main.go collected despite the unreadable sibling dir, got {:?}",
        rep.files
    );
}

/// Go `TestCollectFilesReportCountsSkipped`: files with no extractor are counted by extension;
/// lock files are not.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_report_counts_skipped() {
    let root = TempDir::new();
    must_write(&root.join("main.go"), "package p"); // collected
    must_write(&root.join("notes.unknownext"), "x"); // no extractor -> counted
    must_write(&root.join("image.png"), "x"); // no extractor -> counted
    must_write(&root.join("Makefile"), "all:"); // extensionless -> counted
    must_write(&root.join("go.sum"), "x"); // lock file -> NOT counted

    let rep = collect_files_report(root.path(), WORKERS).expect("CollectFilesReport");
    assert_eq!(
        rep.skipped, 3,
        "expected 3 skipped (no-extractor) files, got {}",
        rep.skipped
    );
    assert_eq!(
        rep.files,
        ["main.go"],
        "expected only main.go collected, got {:?}",
        rep.files
    );
    let want = BTreeMap::from([
        (".unknownext".to_string(), 1),
        (".png".to_string(), 1),
        ("(no extension)".to_string(), 1),
    ]);
    assert_eq!(rep.skipped_exts, want, "SkippedExts");
}

/// Go `TestCollectFilesShebangAndMts`: `.mts`/`.cts` are collected, and an extensionless file
/// only with a shebang naming an interpreter that has an extractor.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_shebang_and_mts() {
    let root = TempDir::new();
    must_write(&root.join("mod.mts"), "export const x = 1;");
    must_write(&root.join("cmod.cts"), "export const y = 2;");
    must_write(&root.join("deploy"), "#!/usr/bin/env bash\necho hi\n");
    must_write(&root.join("run.py.helper"), ""); // has an (unsupported) ext, not extensionless
    must_write(&root.join("notes"), "just some text, no shebang\n");
    must_write(&root.join("oldscript"), "#!/usr/bin/perl\nprint 1;\n"); // perl: no Go extractor

    let got = collected(root.path());
    for want in ["mod.mts", "cmod.cts", "deploy"] {
        assert!(
            got.contains(want),
            "expected {want:?} to be collected, got {got:?}"
        );
    }
    for not_want in ["run.py.helper", "notes", "oldscript"] {
        assert!(
            !got.contains(not_want),
            "did not expect {not_want:?} to be collected, got {got:?}"
        );
    }
}

/// Go `TestCollectFilesSkipsNoxVirtualenv` (#1804).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_skips_nox_virtualenv() {
    let root = TempDir::new();
    must_write(&root.join(".nox/noxenv/pkg/widget.py"), "class Deck: pass");
    must_write(&root.join("app.py"), "def go(): pass");

    let got = collected(root.path());
    assert!(
        got.contains("app.py"),
        "expected app.py collected, got {got:?}"
    );
    for f in &got {
        assert!(
            !f.contains(".nox"),
            "did not expect .nox file collected: {f:?}"
        );
    }
}

/// Go `TestCollectFilesIncludesRakeAndSkill` (#1784, #1901).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_includes_rake_and_skill() {
    let root = TempDir::new();
    must_write(&root.join("tasks/build.rake"), "task :run do\nend\n");
    must_write(
        &root.join("10_Orchestrator.skill"),
        "---\nname: x\n---\n# Title\n",
    );

    let got = collected(root.path());
    for want in ["tasks/build.rake", "10_Orchestrator.skill"] {
        assert!(
            got.contains(want),
            "expected {want:?} collected, got {got:?}"
        );
    }
}

/// Go `TestIsSensitiveSkillIsDocument` (#1901).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_is_sensitive_skill_is_document() {
    assert!(
        is_sensitive("credentials.skill"),
        "credentials.skill should be treated as a sensitive document, not exempt source"
    );
    assert!(
        !is_sensitive("build.rake"),
        "build.rake is source code and should not be sensitive"
    );
}

/// Go `TestCollectFilesIncludesMemoryDir`: `graphify-out` is output, except its `memory/`
/// subtree, which is collected even under a gitignore rule for the whole dir.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_includes_memory_dir() {
    let root = TempDir::new();
    must_write(&root.join(".gitignore"), "graphify-out/\n");
    must_write(&root.join("main.go"), "package main");
    must_write(&root.join("graphify-out/graph.json"), r#"{"nodes":[]}"#);
    must_write(&root.join("graphify-out/raw/notes.md"), "# raw");
    must_write(
        &root.join("graphify-out/memory/query_1_ab_q.md"),
        "---\ntype: \"query\"\n---\n\n# Q: q",
    );

    let got = collected(root.path());
    let want = ["graphify-out/memory/query_1_ab_q.md", "main.go"];
    for f in &got {
        assert!(
            want.contains(&f.as_str()),
            "unexpected file {f:?} in {got:?}"
        );
    }
    for f in want {
        assert!(got.contains(f), "missing file {f:?} in {got:?}");
    }
}

/// Go `TestCollectFilesSkipsRenamedGraphifyOut`: a renamed out dir (`GRAPHIFY_OUT`) inside the
/// tree is pruned like `graphify-out`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_skips_renamed_graphify_out() {
    let root = TempDir::new();
    must_write(&root.join("main.go"), "package main\n");
    must_write(&root.join("gfyout/graph.json"), r#"{"nodes":[]}"#);
    must_write(&root.join("gfyout/GRAPH_REPORT.md"), "# report\n");
    let env = Env {
        graphify_out: Some(OsString::from(root.join("gfyout"))),
        ..Env::process()
    };

    let files = collect_env(root.path(), &env);
    assert_eq!(files, ["main.go"], "expected only main.go, got {files:?}");
}

/// Go `TestCollectFilesSkipsNestedWorktrees`: a dot-dir's `worktrees/` and a dir holding a `.git`
/// file are pruned; other dot-dirs are not.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_skips_nested_worktrees() {
    let root = TempDir::new();
    must_write(&root.join("main.go"), "package main\n");
    must_write(&root.join(".claude/worktrees/x/a.go"), "package a\n");
    must_write(&root.join("linked/.git"), "gitdir: /elsewhere\n");
    must_write(&root.join("linked/b.go"), "package b\n");
    must_write(&root.join(".claude/workflows/w.md"), "# w\n");
    must_write(&root.join(".github/g.md"), "# g\n");

    let got = collected(root.path());
    for bad in [".claude/worktrees/x/a.go", "linked/b.go"] {
        assert!(!got.contains(bad), "{bad} should be skipped");
    }
    for want in ["main.go", ".claude/workflows/w.md", ".github/g.md"] {
        assert!(got.contains(want), "{want} should be indexed: {got:?}");
    }
}

/// Go `TestCollectFilesKeepsTopLevelWorktreesRelativeRoot`: with root `.` (Go `t.Chdir(root)`),
/// a top-level `worktrees/` is kept and `.claude/worktrees` is still pruned.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_collect_files_keeps_top_level_worktrees_relative_root() {
    let root = TempDir::new();
    must_write(&root.join("m.go"), "package m\n");
    must_write(&root.join("worktrees/t.go"), "package t\n");
    must_write(&root.join(".claude/worktrees/x/a.go"), "package a\n");

    let got = collected_env(
        std::path::Path::new("."),
        &env_cwd(root.path().to_path_buf()),
    );
    assert!(
        got.contains("worktrees/t.go") && !got.contains(".claude/worktrees/x/a.go"),
        "unexpected files: {got:?}"
    );
}
