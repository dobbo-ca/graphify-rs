//! Upstream graphify v0.9.107 `tests/test_detect.py`: the cases that hold for graphify-go
//! v0.12.0, with Go's values. Upstream's `detect(root)["files"]` is Go's `collect_files` (one
//! list for code and documents), `classify_file` is `SUPPORTED_EXTENSIONS`, `_is_sensitive` is
//! `is_sensitive`, `_shebang_interpreter` is `shebang_interpreter`, `_find_vcs_root` is
//! `find_vcs_root`, and `_is_ignored` is the ignorer's verdict through the pruning walk
//! (`walk_ignored`). A case that holds only because Go lacks the feature carries
//! `// pins Go v0.12.0; upstream 0.9.107 differs -> P3`; a case where upstream asserts another
//! value carries `// pins Go v0.12.0 (<Go value>); upstream 0.9.107 emits <x> -> P3`. Go's
//! `vendor` is a skip dir (upstream's is not), so the cases that put source under `vendor/`
//! pin Go's pruning. The not-ported cases are listed in `parity/README.md` (detect block).

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::symlink;
use std::path::Path;

use super::{
    WORKERS, chmod, collect, collected, collected_env, env_home, mk_git, must_write,
    readable_despite_chmod, resolved, walk_ignored,
};
use crate::detect::detect::{Env, shebang_ext_of_file, shebang_interpreter, skip_dir};
use crate::detect::gitignore::{find_vcs_root, new_ignorer};
use crate::detect::{SUPPORTED_EXTENSIONS, collect_files_report, is_sensitive, shebang_ext};
use crate::testutil::TempDir;

/// Upstream's `any(needle in f for f in files)`.
fn any_has(files: &BTreeSet<String>, needle: &str) -> bool {
    files.iter().any(|f| f.contains(needle))
}

/// `_is_ignored` for each `(rel, is_dir)` through a fresh ignorer of `root`.
fn ignored(root: &Path, rel: &str, is_dir: bool) -> bool {
    walk_ignored(&new_ignorer(root, &Env::process()), rel, is_dir)
}

// --- classify_file: Go's SUPPORTED_EXTENSIONS ---

/// `test_classify_python`, `test_classify_typescript`, `test_classify_markdown`,
/// `test_classify_skill` (#1901), `test_classify_unknown_returns_none`.
#[test]
fn classify_by_extension() {
    for ext in [".py", ".ts", ".md", ".skill"] {
        assert!(SUPPORTED_EXTENSIONS.contains(&ext), "{ext} is supported");
    }
    assert!(!SUPPORTED_EXTENSIONS.contains(&".zip"), ".zip is not");
}

/// `test_detect_verilog_headers` (`.vh` and `.VH`: the extension is lowercased).
#[test]
fn detect_verilog_headers() {
    for suffix in [".vh", ".VH"] {
        let t = TempDir::new();
        let name = format!("defs{suffix}");
        must_write(&t.join(&name), "`define WIDTH 8\n");
        assert!(collected(t.path()).contains(&name), "{name}");
    }
}

// --- noise dirs ---

/// `test_detect_skips_obsidian_vault_metadata_dirs` (#2493): `.obsidian` and `.smart-env` are
/// pruned, `.trash` is not, in walk order.
#[test]
fn detect_skips_obsidian_vault_metadata_dirs() {
    let t = TempDir::new();
    for dir in [".obsidian", ".smart-env", ".trash"] {
        must_write(&t.join(dir).join("state.json"), "{}");
    }
    must_write(&t.join("project.json"), "{}");
    let files = collect_files_report(t.path(), WORKERS)
        .expect("CollectFiles")
        .files;
    assert_eq!(files, [".trash/state.json", "project.json"]);
}

/// `test_detect_skips_coverage_dir` (#870).
#[test]
fn detect_skips_coverage_dir() {
    let t = TempDir::new();
    must_write(
        &t.join("coverage/lcov-report/index.html"),
        "<html>coverage report</html>",
    );
    must_write(
        &t.join("coverage/lcov-report/src.ts.html"),
        "<html>file coverage</html>",
    );
    must_write(&t.join("main.py"), "def hello(): pass");
    let got = collected(t.path());
    assert!(!got.iter().any(|f| f.starts_with("coverage")), "{got:?}");
    assert!(got.contains("main.py"), "{got:?}");
}

/// `test_detect_skips_coverage_dir_by_lcov_info` (#870, #2339).
#[test]
fn detect_skips_coverage_dir_by_lcov_info() {
    let t = TempDir::new();
    must_write(
        &t.join("coverage/lcov.info"),
        "TN:\nSF:src/app.ts\nend_of_record\n",
    );
    must_write(
        &t.join("coverage/prettify.js"),
        "var PR_SHOULD_USE_CONTINUATION=true;",
    );
    must_write(&t.join("main.py"), "def hello(): pass");
    let got = collected(t.path());
    assert!(!got.iter().any(|f| f.starts_with("coverage")), "{got:?}");
    assert!(got.contains("main.py"), "{got:?}");
}

/// `test_detect_keeps_coverage_code_namespace` (#2339).
#[test]
fn detect_keeps_coverage_code_namespace() {
    let t = TempDir::new();
    let pkg = "auditor_toolkit/assurance/coverage";
    must_write(
        &t.join(&format!("{pkg}/__init__.py")),
        "from .impact import Impact\n",
    );
    must_write(
        &t.join(&format!("{pkg}/impact.py")),
        "class Impact:\n    def score(self): return 1\n",
    );
    must_write(
        &t.join(&format!("{pkg}/inventory.py")),
        "def inventory():\n    return []\n",
    );
    must_write(
        &t.join("app.py"),
        "from auditor_toolkit.assurance import coverage\n",
    );
    let got = collected(t.path());
    for f in ["impact.py", "inventory.py", "__init__.py"] {
        assert!(got.contains(&format!("{pkg}/{f}")), "{f}: {got:?}");
    }
}

/// `test_collect_files_keeps_coverage_code_namespace` (#2339): as the walk root and below it; a
/// real Istanbul report dir beside it is still pruned.
#[test]
fn collect_files_keeps_coverage_code_namespace() {
    let t = TempDir::new();
    let pkg = t.join("auditor_toolkit/assurance/coverage");
    for name in ["__init__.py", "impact.py", "mapping.py"] {
        must_write(&pkg.join(name), "def f(): pass\n");
    }
    must_write(
        &t.join("webapp/coverage/index.html"),
        "<html>coverage</html>",
    );
    must_write(&t.join("webapp/coverage/base.css"), "body{}");
    must_write(&t.join("webapp/coverage/prettify.js"), "var PR=1;");

    assert_eq!(collect(&pkg), ["__init__.py", "impact.py", "mapping.py"]);
    let walked = collected(t.path());
    assert!(any_has(&walked, "impact.py"), "{walked:?}");
    assert!(
        !any_has(&walked, "webapp"),
        "a generated Istanbul report dir must still be pruned (#870)"
    );
}

/// `test_is_noise_dir_coverage_is_evidence_gated`, through Go's `skipDir`. The no-parent call
/// (`_is_noise_dir("coverage")`) has no Go form.
#[test]
fn is_noise_dir_coverage_is_evidence_gated() {
    let t = TempDir::new();
    let env = Env::process();
    must_write(&t.join("coverage/__init__.py"), "");
    assert!(!skip_dir(t.path(), OsStr::new("coverage"), &env));
    must_write(&t.join("report/coverage/coverage-final.json"), "{}");
    assert!(skip_dir(&t.join("report"), OsStr::new("coverage"), &env));
    assert!(skip_dir(t.path(), OsStr::new("lcov-report"), &env));
}

/// `test_detect_skips_visual_tests_dir` (#869).
#[test]
fn detect_skips_visual_tests_dir() {
    let t = TempDir::new();
    must_write(
        &t.join("visual-tests/bundle.js"),
        "var u3=function(){};var d2=function(){}",
    );
    must_write(
        &t.join("visual-tests/screens.tsx"),
        "export const Screen = () => <div/>",
    );
    must_write(&t.join("app.py"), "def main(): pass");
    let got = collected(t.path());
    assert!(!any_has(&got, "visual-tests"), "{got:?}");
    assert!(got.contains("app.py"), "{got:?}");
}

/// `test_detect_skips_snapshots_dir`.
#[test]
fn detect_skips_snapshots_dir() {
    let t = TempDir::new();
    must_write(
        &t.join("__snapshots__/app.test.ts.snap"),
        "// Jest Snapshot\nexports[`test 1`] = `<div/>`",
    );
    must_write(
        &t.join("snapshots/component.test.tsx.snap"),
        "exports[`renders`] = `<span/>`",
    );
    must_write(
        &t.join("app.ts"),
        "export function greet() { return 'hi'; }",
    );
    let got = collected(t.path());
    assert!(!any_has(&got, "__snapshots__"), "{got:?}");
    assert!(!got.iter().any(|f| f.starts_with("snapshots/")), "{got:?}");
    assert!(got.contains("app.ts"), "{got:?}");
}

/// `test_detect_keeps_snapshots_code_namespace` (#1666).
#[test]
fn detect_keeps_snapshots_code_namespace() {
    let t = TempDir::new();
    must_write(
        &t.join("app/services/snapshots/round_reader.rb"),
        "class RoundReader\n  def call; end\nend\n",
    );
    must_write(
        &t.join("app/services/snapshots/backfill_marker.rb"),
        "class BackfillMarker\n  def run; end\nend\n",
    );
    must_write(&t.join("app.rb"), "class App; end\n");
    let got = collected(t.path());
    assert!(any_has(&got, "round_reader.rb"), "{got:?}");
    assert!(any_has(&got, "backfill_marker.rb"), "{got:?}");
}

/// `test_detect_skips_storybook_static_dir`.
#[test]
fn detect_skips_storybook_static_dir() {
    let t = TempDir::new();
    must_write(
        &t.join("storybook-static/index.html"),
        "<html>storybook</html>",
    );
    must_write(
        &t.join("storybook-static/main.js"),
        "(function(){var s=1;})()",
    );
    must_write(
        &t.join("Button.tsx"),
        "export const Button = () => <button/>",
    );
    let got = collected(t.path());
    assert!(!any_has(&got, "storybook-static"), "{got:?}");
    assert!(got.contains("Button.tsx"), "{got:?}");
}

/// `test_detect_allows_github_dir` (#873): `.github` is walked, but Go has no YAML extractor.
#[test]
fn detect_allows_github_dir() {
    let t = TempDir::new();
    must_write(
        &t.join(".github/workflows/ci.yml"),
        "name: CI\non: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n",
    );
    must_write(&t.join("main.py"), "def run(): pass");
    let got = collected(t.path());
    // pins Go v0.12.0 (ci.yml not collected: .yml is not a supported extension); upstream 0.9.107 emits .github/workflows/ci.yml -> P3 (B27)
    assert!(!any_has(&got, ".github"), "{got:?}");
}

/// `test_detect_skips_next_cache` (#873).
#[test]
fn detect_skips_next_cache() {
    let t = TempDir::new();
    must_write(&t.join(".next/cache/build.js"), "(function(){var s=1;})()");
    must_write(
        &t.join("pages/index.tsx"),
        "export default function Home() { return <div/> }",
    );
    let got = collected(t.path());
    assert!(!any_has(&got, ".next"), "{got:?}");
    assert!(got.contains("pages/index.tsx"), "{got:?}");
}

/// `test_detect_skips_nox_virtualenv` (#1804).
#[test]
fn detect_skips_nox_virtualenv() {
    let t = TempDir::new();
    must_write(
        &t.join(".nox/tests/lib/site-packages/pydeck/widget.py"),
        "class Deck: pass",
    );
    must_write(&t.join("app.py"), "def go(): pass");
    let got = collected(t.path());
    assert!(!any_has(&got, ".nox"), "{got:?}");
    assert!(got.contains("app.py"), "{got:?}");
}

/// `test_detect_skips_graphify_own_cache` (#873).
#[test]
fn detect_skips_graphify_own_cache() {
    let t = TempDir::new();
    must_write(
        &t.join(".graphify/cache/abc123.json"),
        r#"{"nodes": [], "edges": []}"#,
    );
    must_write(&t.join("app.py"), "def go(): pass");
    let got = collected(t.path());
    assert!(!any_has(&got, ".graphify"), "{got:?}");
    assert!(got.contains("app.py"), "{got:?}");
}

/// `test_detect_skips_worktrees_dir` (#947).
#[test]
fn detect_skips_worktrees_dir() {
    let t = TempDir::new();
    must_write(&t.join(".worktrees/feature-branch/main.py"), "x = 1");
    must_write(&t.join("app.py"), "y = 2");
    let got = collected(t.path());
    assert!(got.contains("app.py"), "{got:?}");
    assert!(!any_has(&got, ".worktrees"), "{got:?}");
}

/// `test_detect_skips_nested_worktrees_dir` (#1023).
#[test]
fn detect_skips_nested_worktrees_dir() {
    let t = TempDir::new();
    must_write(&t.join(".claude/worktrees/feature-branch/main.py"), "x = 1");
    must_write(&t.join("app.py"), "y = 2");
    let got = collected(t.path());
    assert!(got.contains("app.py"), "{got:?}");
    assert!(!any_has(&got, "worktrees"), "{got:?}");
}

/// `test_detect_keeps_skills_graphify_source_layouts` (#2479).
#[test]
fn detect_keeps_skills_graphify_source_layouts() {
    let keep = [
        "graphify/skill.md",
        "graphify/skills/claude/references/query.md",
        "graphify/skills/codex/references/update.md",
        "graphify/detect.py",
        "skills/graphify/SKILL.md",
        "src/skills/graphify/loader.py",
        "docs/agent/skills/graphify/notes.md",
        ".claude/skills/graphify-extras/SKILL.md",
        ".claude/graphify/notes.md",
    ];
    let t = TempDir::new();
    for rel in keep {
        let body = if rel.ends_with(".py") {
            "def f():\n    return 1\n"
        } else {
            "# Title\n\nText.\n"
        };
        must_write(&t.join(rel), body);
    }
    let got = collected(t.path());
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go never prunes an installed skill)
    for rel in keep {
        assert!(got.contains(rel), "{rel}: {got:?}");
    }
}

/// `test_detect_keeps_files_named_like_graphify_installs`.
#[test]
fn detect_keeps_files_named_like_graphify_installs() {
    let keep = [
        "docs/graphify.md",
        "rules/graphify.md",
        "steering/graphify.md",
        "src/plugins/graphify.js",
        "plugins/graphify.js",
        ".github/rules/graphify.md",
        ".opencode/plugins/team.js",
        ".kiro/steering/product.md",
        ".agents/rules/style.md",
        ".windsurf/rules/graphify-notes.md",
    ];
    let t = TempDir::new();
    for rel in keep {
        let body = if rel.ends_with(".js") {
            "export function f() { return 1 }\n"
        } else {
            "# Title\n\nText.\n"
        };
        must_write(&t.join(rel), body);
    }
    let got = collected(t.path());
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go never prunes an installed file)
    for rel in keep {
        assert!(got.contains(rel), "{rel}: {got:?}");
        assert!(!ignored(t.path(), rel, false), "{rel}");
    }
}

/// `test_detect_keeps_env_source_dirs` (#2058): `env/` and `*_env/` without venv markers stay.
#[test]
fn detect_keeps_env_source_dirs() {
    let t = TempDir::new();
    must_write(
        &t.join("src_env/env/ctrl_mem_env.py"),
        "def build_env():\n    return 1\n",
    );
    must_write(
        &t.join("src_env/other_dir/also_real.py"),
        "def x():\n    return 2\n",
    );
    let got = collected(t.path());
    assert!(
        any_has(&got, "ctrl_mem_env.py"),
        "env/ source dir wrongly pruned (#2058)"
    );
    assert!(
        any_has(&got, "also_real.py"),
        "*_env/ subtree wrongly pruned (#2058)"
    );
    let nested = collected(&t.join("src_env"));
    assert!(
        any_has(&nested, "ctrl_mem_env.py"),
        "nested env/ pruned when scanned directly (#2058)"
    );
}

/// `test_detect_still_prunes_real_env_venv` (#2058), without `pruned_noise_dirs` (no such
/// report in Go).
#[test]
fn detect_still_prunes_real_env_venv() {
    let t = TempDir::new();
    must_write(&t.join("env/pyvenv.cfg"), "home = /usr/bin\n");
    must_write(&t.join("env/lib/sixish.py"), "x = 1\n");
    must_write(&t.join("main.py"), "def main():\n    return 1\n");
    let got = collected(t.path());
    assert!(
        !any_has(&got, "sixish.py"),
        "real venv env/ must still be pruned"
    );
    assert!(got.contains("main.py"), "{got:?}");
}

/// `test_detect_prunes_venv_names_without_markers` (#2058).
#[test]
fn detect_prunes_venv_names_without_markers() {
    let t = TempDir::new();
    for name in ["venv", ".venv", "my_venv"] {
        must_write(&t.join(name).join("mod.py"), "y = 1\n");
    }
    must_write(&t.join("app.py"), "def a():\n    return 1\n");
    let got = collected(t.path());
    assert!(got.contains("app.py"), "{got:?}");
    for name in ["venv", ".venv", "my_venv"] {
        assert!(
            !got.iter().any(|f| f.starts_with(&format!("{name}/"))),
            "{name} must stay pruned"
        );
    }
}

/// `test_nested_graphify_out_prunes_only_configured_path` (#2273, 4 params). Go's `skipDir`
/// prunes every dir named like `$GRAPHIFY_OUT`'s base, and does not resolve a symlinked out dir.
#[test]
fn nested_graphify_out_prunes_only_configured_path() {
    // (id, configured_out, absolute, symlink_target, Go: source kept, Go: generated kept)
    type OutCase = (
        &'static str,
        &'static str,
        bool,
        Option<&'static str>,
        bool,
        bool,
    );
    const CASES: &[OutCase] = &[
        // pins Go v0.12.0 (source pruned: src/.../nlp shares the base name); upstream 0.9.107 emits the source -> P3
        (
            "default-parent",
            "graphify-out/nlp",
            false,
            None,
            false,
            false,
        ),
        // pins Go v0.12.0 (source pruned); upstream 0.9.107 emits the source -> P3
        ("custom-parent", "artifacts/nlp", false, None, false, false),
        // pins Go v0.12.0 (source pruned); upstream 0.9.107 emits the source -> P3
        ("absolute", "artifacts/nlp", true, None, false, false),
        // pins Go v0.12.0 (generated.py collected: the real out dir is not named output-link); upstream 0.9.107 prunes it -> P3
        (
            "in-root-symlink",
            "aliases/output-link",
            false,
            Some("artifacts/nlp"),
            true,
            true,
        ),
    ];
    for &(id, configured, absolute, link_target, source_kept, generated_kept) in CASES {
        let t = TempDir::new();
        let configured_out = if absolute {
            t.join(configured).into_os_string()
        } else {
            OsString::from(configured)
        };
        let source = "src/revil/nexus/nlp/core.py";
        must_write(
            &t.join(source),
            "def tokenize(text):\n    return text.split()\n",
        );
        let out_rel = link_target.unwrap_or(configured);
        if let Some(target) = link_target {
            std::fs::create_dir_all(t.join(target)).expect("mkdir");
            std::fs::create_dir_all(t.join(configured).parent().expect("parent")).expect("mkdir");
            symlink(t.join(target), t.join(configured)).expect("symlink");
        }
        let generated = format!("{out_rel}/generated.py");
        must_write(&t.join(&generated), "SHOULD_NOT_BE_INDEXED = True\n");
        let env = Env {
            graphify_out: Some(configured_out),
            ..Env::process()
        };
        let got = collected_env(t.path(), &env);
        assert_eq!(got.contains(source), source_kept, "{id}: source {got:?}");
        assert_eq!(
            got.contains(&generated),
            generated_kept,
            "{id}: generated {got:?}"
        );
    }
}

// --- symlinks: Go follows an in-root file link and never a directory link ---

/// `test_detect_follows_symlinked_directory`, its `follow_symlinks=False` half (Go has no
/// `follow_symlinks`).
#[test]
fn detect_follows_symlinked_directory() {
    let t = TempDir::new();
    must_write(&t.join("real_lib/util.py"), "x = 1");
    symlink(t.join("real_lib"), t.join("linked_lib")).expect("symlink");
    let got = collected(t.path());
    assert!(any_has(&got, "real_lib"), "{got:?}");
    assert!(!any_has(&got, "linked_lib"), "{got:?}");
}

/// `test_detect_follows_symlinked_file`.
#[test]
fn detect_follows_symlinked_file() {
    let t = TempDir::new();
    must_write(&t.join("real.py"), "x = 1");
    symlink(t.join("real.py"), t.join("link.py")).expect("symlink");
    let got = collected(t.path());
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (no follow_symlinks: an in-root file link is always collected)
    assert!(
        got.contains("real.py") && got.contains("link.py"),
        "{got:?}"
    );
}

/// `test_detect_handles_circular_symlinks`.
#[test]
fn detect_handles_circular_symlinks() {
    let t = TempDir::new();
    must_write(&t.join("a/main.py"), "x = 1");
    symlink(t.path(), t.join("a/loop")).expect("symlink");
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (a directory link is never followed)
    assert!(collected(t.path()).contains("a/main.py"));
}

/// `test_detect_default_does_not_auto_follow_direct_symlink_child`.
#[test]
fn detect_default_does_not_auto_follow_direct_symlink_child() {
    let t = TempDir::new();
    must_write(&t.join("real_lib/util.py"), "x = 1");
    symlink(t.join("real_lib"), t.join("linked_lib")).expect("symlink");
    let got = collected(t.path());
    assert!(any_has(&got, "real_lib"), "{got:?}");
    assert!(!any_has(&got, "linked_lib"), "{got:?}");
}

/// `test_detect_default_does_not_follow_when_no_symlinks`.
#[test]
fn detect_default_does_not_follow_when_no_symlinks() {
    let t = TempDir::new();
    must_write(&t.join("main.py"), "x = 1");
    must_write(&t.join("sub/other.py"), "y = 2");
    let got = collected(t.path());
    assert!(
        got.contains("main.py") && got.contains("sub/other.py"),
        "{got:?}"
    );
}

/// `test_detect_explicit_false_overrides_auto_detect`.
#[test]
fn detect_explicit_false_overrides_auto_detect() {
    let t = TempDir::new();
    must_write(&t.join("real_lib/util.py"), "x = 1");
    symlink(t.join("real_lib"), t.join("linked_lib")).expect("symlink");
    assert!(!any_has(&collected(t.path()), "linked_lib"));
}

/// `test_detect_skips_out_of_root_symlinked_directory_even_when_following`, without the
/// `skipped_sensitive` message (no such report in Go).
#[test]
fn detect_skips_out_of_root_symlinked_directory_even_when_following() {
    let t = TempDir::new();
    must_write(&t.join("outside/secret.py"), "token = 'outside'");
    std::fs::create_dir_all(t.join("root")).expect("mkdir");
    symlink(t.join("outside"), t.join("root/linked_secret")).expect("symlink");
    assert!(!any_has(&collected(&t.join("root")), "linked_secret"));
}

/// `test_detect_skips_out_of_root_symlinked_file_by_default`, without the `skipped_sensitive`
/// message.
#[test]
fn detect_skips_out_of_root_symlinked_file_by_default() {
    let t = TempDir::new();
    must_write(&t.join("outside/secret.py"), "token = 'outside'");
    std::fs::create_dir_all(t.join("root")).expect("mkdir");
    symlink(t.join("outside/secret.py"), t.join("root/secret_link.py")).expect("symlink");
    assert!(!any_has(&collected(&t.join("root")), "secret_link.py"));
}

// --- ignore files through collect_files ---

/// `test_graphifyignore_excludes_file`, without `graphifyignore_patterns` (no count in Go).
#[test]
fn graphifyignore_excludes_file() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "vendor/\n*.generated.py\n");
    must_write(&t.join("vendor/lib.py"), "x = 1");
    must_write(&t.join("main.py"), "print('hi')");
    must_write(&t.join("schema.generated.py"), "x = 1");
    let got = collected(t.path());
    assert!(got.contains("main.py"), "{got:?}");
    assert!(!any_has(&got, "vendor"), "{got:?}");
    assert!(!any_has(&got, "generated"), "{got:?}");
}

/// `test_graphifyignore_matches_nfd_path_with_nfc_pattern`: Go compares bytes, with no Unicode
/// normalization.
#[test]
fn graphifyignore_matches_nfd_path_with_nfc_pattern() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "Or\u{e7}amento/\n");
    must_write(&t.join("Orc\u{327}amento/contrato.py"), "x = 1");
    must_write(&t.join("main.py"), "print('hi')");
    let got = collected(t.path());
    assert!(got.contains("main.py"), "{got:?}");
    // pins Go v0.12.0 (contrato.py collected); upstream 0.9.107 excludes it -> P3
    assert!(any_has(&got, "contrato.py"), "{got:?}");
}

/// `test_graphifyignore_matches_nfc_path_with_nfd_pattern`.
#[test]
fn graphifyignore_matches_nfc_path_with_nfd_pattern() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "Orc\u{327}amento/\n");
    must_write(&t.join("Or\u{e7}amento/contrato.py"), "x = 1");
    must_write(&t.join("main.py"), "print('hi')");
    let got = collected(t.path());
    assert!(got.contains("main.py"), "{got:?}");
    // pins Go v0.12.0 (contrato.py collected); upstream 0.9.107 excludes it -> P3
    assert!(any_has(&got, "contrato.py"), "{got:?}");
}

/// `test_graphifyignore_ascii_patterns_unaffected`.
#[test]
fn graphifyignore_ascii_patterns_unaffected() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "vendor/\n");
    must_write(&t.join("vendor/lib.py"), "x = 1");
    must_write(&t.join("main.py"), "x = 1");
    let got = collected(t.path());
    assert!(got.contains("main.py"), "{got:?}");
    assert!(!any_has(&got, "vendor"), "{got:?}");
}

/// `test_graphifyignore_comments_ignored`.
#[test]
fn graphifyignore_comments_ignored() {
    let t = TempDir::new();
    must_write(
        &t.join(".graphifyignore"),
        "# this is a comment\n\nmain.py\n",
    );
    must_write(&t.join("main.py"), "x = 1");
    must_write(&t.join("other.py"), "x = 2");
    let got = collected(t.path());
    assert!(
        !got.contains("main.py") && got.contains("other.py"),
        "{got:?}"
    );
}

/// The UTF-8 BOM cases (#2163). Go keeps the BOM in the first line, so a BOM'd first pattern
/// never matches; each case still holds because the file it names has no supported extension
/// (`.log`), is under a skip or sensitive dir (`build/`, `secrets/`), or the next line does the
/// work. `graphifyignore_patterns` has no Go counterpart.
/// `test_graphifyignore_utf8_bom_first_pattern_honored`, `test_gitignore_utf8_bom_matches_git`,
/// `test_graphifyignore_bom_only_file`, `test_graphifyignore_bom_then_comment`,
/// `test_nested_gitignore_utf8_bom`, `test_git_info_exclude_utf8_bom`.
#[test]
fn utf8_bom_ignore_files() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go does not strip a leading BOM)
    let t = TempDir::new();
    std::fs::write(t.join(".graphifyignore"), b"\xef\xbb\xbf*.log\nbuild/\n").expect("write");
    must_write(&t.join("build/lib.py"), "x = 1");
    must_write(&t.join("app.log"), "log line");
    must_write(&t.join("main.py"), "print('hi')");
    let got = collected(t.path());
    assert!(
        !any_has(&got, "app.log") && !any_has(&got, "build"),
        "first_pattern: {got:?}"
    );
    assert!(got.contains("main.py"), "first_pattern: {got:?}");

    let t = TempDir::new();
    std::fs::write(t.join(".gitignore"), b"\xef\xbb\xbf*.log\n").expect("write");
    must_write(&t.join("app.log"), "log line");
    must_write(&t.join("main.py"), "print('hi')");
    let got = collected(t.path());
    assert!(
        !any_has(&got, "app.log") && got.contains("main.py"),
        "gitignore: {got:?}"
    );

    let t = TempDir::new();
    std::fs::write(t.join(".graphifyignore"), b"\xef\xbb\xbf").expect("write");
    must_write(&t.join("main.py"), "x = 1");
    assert!(collected(t.path()).contains("main.py"), "bom_only");

    let t = TempDir::new();
    std::fs::write(
        t.join(".graphifyignore"),
        b"\xef\xbb\xbf# comment\nmain.py\n",
    )
    .expect("write");
    must_write(&t.join("main.py"), "x = 1");
    must_write(&t.join("other.py"), "x = 2");
    let got = collected(t.path());
    assert!(
        !got.contains("main.py") && got.contains("other.py"),
        "bom_then_comment: {got:?}"
    );

    let t = TempDir::new();
    std::fs::create_dir_all(t.join("sub")).expect("mkdir");
    std::fs::write(t.join("sub/.gitignore"), b"\xef\xbb\xbf*.log\n").expect("write");
    must_write(&t.join("sub/app.log"), "log line");
    must_write(&t.join("sub/keep.py"), "x = 1");
    let got = collected(t.path());
    assert!(
        !any_has(&got, "app.log") && got.contains("sub/keep.py"),
        "nested: {got:?}"
    );

    let t = TempDir::new();
    std::fs::create_dir_all(t.join(".git/info")).expect("mkdir");
    std::fs::write(t.join(".git/info/exclude"), b"\xef\xbb\xbfsecrets/\n").expect("write");
    must_write(&t.join("secrets/x.py"), "token = 'x'");
    must_write(&t.join("real.py"), "def real(): pass");
    let got = collected(t.path());
    assert!(
        !any_has(&got, "secrets") && got.contains("real.py"),
        "info_exclude: {got:?}"
    );
}

/// `test_graphifyignore_hermetic_without_vcs`: with no VCS root, a parent `.graphifyignore`
/// does not apply.
#[test]
fn graphifyignore_hermetic_without_vcs() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "vendor/\n");
    let sub = t.join("packages/mylib");
    must_write(&sub.join("main.py"), "x = 1");
    must_write(&sub.join("vendor/dep.py"), "y = 2");
    let got = collected(&sub);
    assert!(got.contains("main.py"), "{got:?}");
    // pins Go v0.12.0 (vendor/dep.py not collected: vendor is a skip dir); upstream 0.9.107 emits it -> P3
    assert!(!any_has(&got, "vendor"), "{got:?}");
}

/// `test_graphifyignore_discovered_from_parent_in_vcs`.
#[test]
fn graphifyignore_discovered_from_parent_in_vcs() {
    let (_d, t) = resolved();
    mk_git(&t);
    must_write(&t.join(".graphifyignore"), "vendor/\n");
    let sub = t.join("packages/mylib");
    must_write(&sub.join("main.py"), "x = 1");
    must_write(&sub.join("vendor/dep.py"), "y = 2");
    let got = collected_env(&sub, &env_home(t.join("nohome")));
    assert!(
        got.contains("main.py") && !any_has(&got, "vendor"),
        "{got:?}"
    );
}

/// `test_graphifyignore_stops_at_git_boundary`.
#[test]
fn graphifyignore_stops_at_git_boundary() {
    let (_d, t) = resolved();
    must_write(&t.join(".graphifyignore"), "main.py\n");
    mk_git(&t.join("repo"));
    must_write(&t.join("repo/sub/main.py"), "x = 1");
    let got = collected_env(&t.join("repo/sub"), &env_home(t.join("nohome")));
    assert!(got.contains("main.py"), "{got:?}");
}

/// `test_graphifyignore_at_git_root_is_included`.
#[test]
fn graphifyignore_at_git_root_is_included() {
    let (_d, t) = resolved();
    mk_git(&t.join("repo"));
    must_write(&t.join("repo/.graphifyignore"), "vendor/\n");
    let sub = t.join("repo/packages/mylib");
    must_write(&sub.join("main.py"), "x = 1");
    must_write(&sub.join("vendor/dep.py"), "y = 2");
    let got = collected_env(&sub, &env_home(t.join("nohome")));
    assert!(
        got.contains("main.py") && !any_has(&got, "vendor"),
        "{got:?}"
    );
}

/// `test_gitignore_nested_below_root_excludes_file` (#1206), without the pattern count.
#[test]
fn gitignore_nested_below_root_excludes_file() {
    let t = TempDir::new();
    must_write(&t.join(".gitignore"), "*.log\n");
    must_write(&t.join("vendor/sub/.gitignore"), "secret.txt\n");
    must_write(&t.join("root.py"), "x = 1");
    must_write(&t.join("root.log"), "noise");
    must_write(&t.join("vendor/sub/keep.py"), "y = 2");
    must_write(&t.join("vendor/sub/secret.txt"), "shh");
    let got = collected(t.path());
    assert!(got.contains("root.py"), "{got:?}");
    // pins Go v0.12.0 (vendor/sub/keep.py not collected: vendor is a skip dir); upstream 0.9.107 emits it -> P3
    assert!(!any_has(&got, "keep.py"), "{got:?}");
    assert!(
        !any_has(&got, "root.log") && !any_has(&got, "secret.txt"),
        "{got:?}"
    );
}

/// `test_gitignore_nested_below_root_prunes_whole_directory`.
#[test]
fn gitignore_nested_below_root_prunes_whole_directory() {
    let t = TempDir::new();
    must_write(&t.join("vendor/sub/.gitignore"), "build/\n");
    must_write(&t.join("vendor/sub/build/generated.py"), "x = 1");
    must_write(&t.join("vendor/sub/keep.py"), "y = 2");
    let got = collected(t.path());
    // pins Go v0.12.0 (keep.py not collected: vendor is a skip dir); upstream 0.9.107 emits it -> P3
    assert!(!any_has(&got, "keep.py"), "{got:?}");
    assert!(!any_has(&got, "generated.py"), "{got:?}");
}

/// `test_gitignore_nested_negation_overrides_broader_root_rule`.
#[test]
fn gitignore_nested_negation_overrides_broader_root_rule() {
    let t = TempDir::new();
    must_write(&t.join(".gitignore"), "*.py\n");
    must_write(&t.join("vendor/sub/.gitignore"), "!important.py\n");
    must_write(&t.join("root.py"), "a = 1");
    must_write(&t.join("vendor/sub/important.py"), "b = 1");
    must_write(&t.join("vendor/sub/other.py"), "c = 1");
    let got = collected(t.path());
    // pins Go v0.12.0 (vendor/sub/important.py not collected: vendor is a skip dir); upstream 0.9.107 emits it -> P3
    assert!(!any_has(&got, "important.py"), "{got:?}");
    assert!(
        !any_has(&got, "root.py") && !any_has(&got, "other.py"),
        "{got:?}"
    );
}

/// `test_nested_ignore_overrides_git_info_exclude_and_root`.
#[test]
fn nested_ignore_overrides_git_info_exclude_and_root() {
    let t = TempDir::new();
    must_write(&t.join(".git/info/exclude"), "*.py\n");
    must_write(&t.join(".gitignore"), "keep.py\n");
    must_write(&t.join("a/b/.gitignore"), "!keep.py\n");
    must_write(&t.join("a/b/keep.py"), "x = 1");
    must_write(&t.join("drop.py"), "y = 1");
    let got = collected(t.path());
    assert!(
        got.contains("a/b/keep.py"),
        "nested ! must beat root + info/exclude"
    );
    assert!(!any_has(&got, "drop.py"), "{got:?}");
}

/// `test_global_git_ignore_needs_a_git_repo`: Go reads no global excludes file at all.
#[test]
fn global_git_ignore_needs_a_git_repo() {
    let t = TempDir::new();
    must_write(&t.join("home/.config/git/ignore"), "secret.py\n");
    must_write(&t.join("proj/secret.py"), "SECRET = 'abc'");
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (no core.excludesFile or ~/.config/git/ignore)
    let got = collected_env(&t.join("proj"), &env_home(t.join("home")));
    assert!(got.contains("secret.py"), "{got:?}");
}

/// `test_detect_honors_git_info_exclude` (#1810).
#[test]
fn detect_honors_git_info_exclude() {
    let t = TempDir::new();
    must_write(&t.join(".git/info/exclude"), "worktrees/\n");
    must_write(&t.join("worktrees/foo/dupe.py"), "def dupe(): pass");
    must_write(&t.join("real.py"), "def real(): pass");
    let got = collected(t.path());
    assert!(!any_has(&got, "dupe.py"), "worktree dir was not excluded");
    assert!(got.contains("real.py"), "real source was dropped");
}

/// `test_git_info_exclude_ranks_below_gitignore_negation` (#1810).
#[test]
fn git_info_exclude_ranks_below_gitignore_negation() {
    let t = TempDir::new();
    must_write(&t.join(".git/info/exclude"), "secret*.txt\n");
    must_write(&t.join(".gitignore"), "!secret-ok.txt\n");
    assert!(ignored(t.path(), "secret-bad.txt", false));
    assert!(!ignored(t.path(), "secret-ok.txt", false));
}

/// `test_gitignore_fallback_when_no_graphifyignore` (#945).
#[test]
fn gitignore_fallback_when_no_graphifyignore() {
    let t = TempDir::new();
    std::fs::create_dir_all(t.join(".git")).expect("mkdir");
    must_write(&t.join(".gitignore"), "vendor/\n*.generated.py\n");
    must_write(&t.join("vendor/lib.py"), "x = 1");
    must_write(&t.join("main.py"), "print('hi')");
    must_write(&t.join("schema.generated.py"), "x = 1");
    let got = collected(t.path());
    assert!(got.contains("main.py"), "{got:?}");
    assert!(
        !any_has(&got, "vendor") && !any_has(&got, "generated"),
        "{got:?}"
    );
}

/// `test_graphifyignore_and_gitignore_are_merged` (#1363).
#[test]
fn graphifyignore_and_gitignore_are_merged() {
    let t = TempDir::new();
    std::fs::create_dir_all(t.join(".git")).expect("mkdir");
    must_write(&t.join(".gitignore"), "main.py\n");
    must_write(&t.join(".graphifyignore"), "other.py\n");
    for (f, body) in [
        ("main.py", "x = 1"),
        ("other.py", "x = 2"),
        ("keep.py", "x = 3"),
    ] {
        must_write(&t.join(f), body);
    }
    let got = collected(t.path());
    assert!(
        !got.contains("main.py") && !got.contains("other.py"),
        "{got:?}"
    );
    assert!(got.contains("keep.py"), "{got:?}");
}

/// `test_graphifyignore_negation_overrides_gitignore` (#1363).
#[test]
fn graphifyignore_negation_overrides_gitignore() {
    let t = TempDir::new();
    std::fs::create_dir_all(t.join(".git")).expect("mkdir");
    must_write(&t.join(".gitignore"), "*.py\n");
    must_write(&t.join(".graphifyignore"), "!keep.py\n");
    must_write(&t.join("main.py"), "x = 1");
    must_write(&t.join("keep.py"), "x = 2");
    let got = collected(t.path());
    assert!(
        got.contains("keep.py") && !got.contains("main.py"),
        "{got:?}"
    );
}

/// `test_anchored_root_wildcard_negation_reincludes_subtree` (#1975).
#[test]
fn anchored_root_wildcard_negation_reincludes_subtree() {
    let t = TempDir::new();
    for rel in [
        "src/app/main.py",
        "src/lib/util.py",
        "docs/guide.md",
        "README.md",
    ] {
        must_write(&t.join(rel), "x\n");
    }
    must_write(&t.join(".graphifyignore"), "/*\n!/src/\n");
    assert_eq!(collect(t.path()), ["src/app/main.py", "src/lib/util.py"]);
}

/// `test_anchored_negation_cannot_skip_excluded_parent`.
#[test]
fn anchored_negation_cannot_skip_excluded_parent() {
    let t = TempDir::new();
    must_write(&t.join("src/app/main.py"), "x\n");
    must_write(&t.join(".graphifyignore"), "/*\n!/src/app/\n");
    assert!(collect(t.path()).is_empty());
}

/// `test_path_pattern_single_star_does_not_cross_segment`.
#[test]
fn path_pattern_single_star_does_not_cross_segment() {
    let t = TempDir::new();
    must_write(&t.join("src/main.py"), "x\n");
    must_write(&t.join("src/app/main.py"), "x\n");
    for pattern in ["/src/*.py", "src/*.py"] {
        must_write(&t.join(".graphifyignore"), &format!("{pattern}\n"));
        let got = collected(t.path());
        assert!(
            !got.contains("src/main.py"),
            "`{pattern}` failed to exclude the direct child: {got:?}"
        );
        assert!(
            got.contains("src/app/main.py"),
            "`{pattern}` crossed a path segment and excluded the nested file: {got:?}"
        );
    }
}

/// `test_directory_only_negation_does_not_reinclude_file`.
#[test]
fn directory_only_negation_does_not_reinclude_file() {
    let t = TempDir::new();
    must_write(&t.join("README.md"), "# docs\n");
    must_write(&t.join(".graphifyignore"), "/*\n!/README.md/\n");
    assert!(collect(t.path()).is_empty());
}

/// `test_anchored_double_star_crosses_path_segments`.
#[test]
fn anchored_double_star_crosses_path_segments() {
    let t = TempDir::new();
    must_write(&t.join("src/generated.py"), "x\n");
    must_write(&t.join("src/app/deep/generated.py"), "x\n");
    must_write(&t.join(".graphifyignore"), "/src/**/generated.py\n");
    assert!(collect(t.path()).is_empty());
}

/// `test_unanchored_pattern_in_nested_gitignore_matches_a_name`.
#[test]
fn unanchored_pattern_in_nested_gitignore_matches_a_name() {
    let t = TempDir::new();
    for rel in [
        "app.py",
        "sub/test_one.py",
        "sub/deeper/test_two.py",
        "sub/test_helpers/util.py",
    ] {
        must_write(&t.join(rel), "x = 1\n");
    }
    must_write(&t.join("sub/.gitignore"), "test_*.py\n");
    assert_eq!(collect(t.path()), ["app.py", "sub/test_helpers/util.py"]);
}

/// `test_unanchored_pattern_reaches_every_consumer_of_the_engine`: Go's one consumer is the walk
/// (`detect` and `collect_files` are the same list); the ignorer gives the same verdicts.
#[test]
fn unanchored_pattern_reaches_every_consumer_of_the_engine() {
    let t = TempDir::new();
    for rel in [
        "app.py",
        ".github/scripts/release.py",
        ".cache/junk.py",
        "test_helpers/util.py",
        "test_one.py",
    ] {
        must_write(&t.join(rel), "x = 1\n");
    }
    must_write(&t.join(".graphifyignore"), ".*\n!.github/\ntest_*.py\n");
    let expected = [
        ".github/scripts/release.py",
        "app.py",
        "test_helpers/util.py",
    ];
    assert_eq!(collect(t.path()), expected);
    let ign = new_ignorer(t.path(), &Env::process());
    let present = [
        ".cache/junk.py",
        "test_one.py",
        ".github/scripts/release.py",
        "app.py",
        "test_helpers/util.py",
    ];
    let kept: BTreeSet<&str> = present
        .into_iter()
        .filter(|rel| !walk_ignored(&ign, rel, false))
        .collect();
    assert_eq!(kept, BTreeSet::from(expected));
}

/// `test_unanchored_pattern_verdicts_do_not_depend_on_cache_order`: one ignorer (Go caches the
/// parsed files per dir) gives the same verdicts in either order.
#[test]
fn unanchored_pattern_verdicts_do_not_depend_on_cache_order() {
    // (rel, is_dir, ignored)
    const EXPECTED: &[(&str, bool, bool)] = &[
        (".github", true, false),
        (".github/scripts", true, false),
        (".github/scripts/release.py", false, false),
        (".cache/junk.py", false, true),
        ("test_helpers", true, false),
        ("test_helpers/util.py", false, false),
        ("test_one.py", false, true),
    ];
    let t = TempDir::new();
    for rel in [
        ".github/scripts/release.py",
        ".cache/junk.py",
        "test_helpers/util.py",
        "test_one.py",
    ] {
        must_write(&t.join(rel), "x = 1\n");
    }
    must_write(&t.join(".graphifyignore"), ".*\n!.github/\ntest_*.py\n");
    for &(rel, is_dir, want) in EXPECTED {
        assert_eq!(ignored(t.path(), rel, is_dir), want, "uncached {rel}");
    }
    for reverse in [false, true] {
        let ign = new_ignorer(t.path(), &Env::process());
        let mut order: Vec<_> = EXPECTED.to_vec();
        if reverse {
            order.reverse();
        }
        for (rel, is_dir, want) in order {
            assert_eq!(
                walk_ignored(&ign, rel, is_dir),
                want,
                "cached {rel} (reverse={reverse})"
            );
        }
    }
}

/// `test_negation_does_not_disable_directory_pruning`, its detection half (the walk-visit count
/// patches `os.walk`).
#[test]
fn negation_does_not_disable_directory_pruning() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "myignored/\n*.md\n!docs/**\n");
    must_write(&t.join("myignored/deep/deeper/junk.py"), "x = 1");
    must_write(&t.join("docs/guide.md"), "# guide");
    must_write(&t.join("src/app.py"), "y = 2");
    let got = collected(t.path());
    assert!(
        got.contains("src/app.py") && got.contains("docs/guide.md"),
        "{got:?}"
    );
    assert!(!any_has(&got, "junk.py"), "{got:?}");
}

/// `test_detect_does_not_ignore_scan_root_itself_via_parent_gitignore` (#2468).
#[test]
fn detect_does_not_ignore_scan_root_itself_via_parent_gitignore() {
    let t = TempDir::new();
    std::fs::create_dir_all(t.join(".git")).expect("mkdir");
    must_write(&t.join("graphify-corpus/keep.py"), "x = 1\n");
    must_write(&t.join("graphify-corpus/docs/intro.md"), "# Introduction\n");
    must_write(&t.join(".gitignore"), "graphify-corpus/\n");
    assert_eq!(
        collect(&t.join("graphify-corpus")),
        ["docs/intro.md", "keep.py"]
    );
}

/// `test_detect_preserves_unrelated_parent_ignores_inside_scan_root` (#2468).
#[test]
fn detect_preserves_unrelated_parent_ignores_inside_scan_root() {
    let t = TempDir::new();
    std::fs::create_dir_all(t.join(".git")).expect("mkdir");
    must_write(&t.join("graphify-corpus/keep.py"), "x = 1\n");
    must_write(&t.join("graphify-corpus/docs/intro.md"), "# Introduction\n");
    must_write(
        &t.join("graphify-corpus/node_modules/lib.js"),
        "console.log(1);\n",
    );
    must_write(&t.join(".gitignore"), "graphify-corpus/\nnode_modules/\n");
    assert_eq!(
        collect(&t.join("graphify-corpus")),
        ["docs/intro.md", "keep.py"]
    );
}

/// `test_ignored_dir_pruned_walk_never_lists_contents`, its detection half (no `ignored` list or
/// walk-visit count in Go).
#[test]
fn ignored_dir_pruned_walk_never_lists_contents() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "graphify-notes/\n");
    for d in 0..5 {
        for f in 0..4 {
            must_write(
                &t.join(&format!("graphify-notes/note-dir-{d}/note-{f}.md")),
                "# note",
            );
        }
    }
    must_write(&t.join("src/app.py"), "y = 2");
    must_write(&t.join("notes.md"), "# kept");
    let got = collected(t.path());
    assert!(!any_has(&got, "graphify-notes"), "{got:?}");
    assert!(
        got.contains("src/app.py") && got.contains("notes.md"),
        "{got:?}"
    );
}

/// `test_nested_gitignore_star_does_not_ignore_outside_its_dir` (#1873).
#[test]
fn nested_gitignore_star_does_not_ignore_outside_its_dir() {
    let t = TempDir::new();
    must_write(&t.join("README.md"), "# hello");
    must_write(&t.join("main.py"), "x = 1");
    must_write(&t.join(".hypothesis/.gitignore"), "*\n");
    must_write(&t.join(".hypothesis/cached.py"), "y = 2");
    assert_eq!(collect(t.path()), ["README.md", "main.py"]);
}

/// `test_nested_gitignore_patterns_still_apply_inside_their_dir`.
#[test]
fn nested_gitignore_patterns_still_apply_inside_their_dir() {
    let t = TempDir::new();
    must_write(&t.join("main.py"), "x = 1");
    must_write(&t.join("sub/.gitignore"), "*.log\n");
    must_write(&t.join("sub/keep.py"), "y = 2");
    must_write(&t.join("sub/noise.log"), "z");
    assert_eq!(collect(t.path()), ["main.py", "sub/keep.py"]);
}

/// `test_nested_gitignore_does_not_govern_sibling_project` (#1922), without the `ignored` list.
#[test]
fn nested_gitignore_does_not_govern_sibling_project() {
    let t = TempDir::new();
    must_write(&t.join("run.py"), "x = 1");
    must_write(&t.join("project_a/data/loader.py"), "def load(): pass");
    must_write(&t.join("project_b/.gitignore"), "data/\n");
    must_write(&t.join("project_b/data/dump.csv"), "a,b\n1,2\n");
    let got = collected(t.path());
    assert!(got.contains("project_a/data/loader.py"), "{got:?}");
    assert!(!any_has(&got, "dump.csv"), "{got:?}");
}

// --- _is_ignored: the ignorer through the pruning walk ---

/// `test_negation_cannot_rescue_file_under_excluded_dir` (#882).
#[test]
fn negation_cannot_rescue_file_under_excluded_dir() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "android/\n!src/\n");
    assert!(ignored(t.path(), "android/app/src/Main.kt", false));
}

/// `test_negation_works_when_no_ancestor_excluded` (#882).
#[test]
fn negation_works_when_no_ancestor_excluded() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "*.py\n!src/keep.py\n");
    assert!(!ignored(t.path(), "src/keep.py", false));
}

/// `test_negation_ancestor_itself_reincluded` (#882).
#[test]
fn negation_ancestor_itself_reincluded() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "vendor/\n!vendor/\n");
    assert!(!ignored(t.path(), "vendor/lib/utils.py", false));
}

/// `test_reincluded_directory_brings_its_files_back` (3 params).
#[test]
fn reincluded_directory_brings_its_files_back() {
    // (id, ignore text, ignored, kept)
    const CASES: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "dot-directory",
            ".*\n!.github/\n",
            &[".cache/junk.py", ".hidden.py", ".github/.local.py"],
            &[".github/scripts/release.py", ".github/CODEOWNERS", "app.py"],
        ),
        (
            "tags-template",
            "tags\n!tags/\n",
            &["tags"],
            &["src/tags/handler.py", "src/tags/sub/deep.py"],
        ),
        (
            "wildcard-directory",
            "cache*\n!cache_keep/\n",
            &["cache.db", "cache_old/a.txt"],
            &["cache_keep/a.txt"],
        ),
    ];
    for &(id, text, ign, kept) in CASES {
        let t = TempDir::new();
        must_write(&t.join(".graphifyignore"), text);
        for rel in ign {
            assert!(ignored(t.path(), rel, false), "{id}: {rel} ignored");
        }
        for rel in kept {
            assert!(!ignored(t.path(), rel, false), "{id}: {rel} kept");
        }
    }
}

/// `test_unanchored_pattern_matches_a_name_not_a_path` (8 params).
#[test]
fn unanchored_pattern_matches_a_name_not_a_path() {
    // (id, pattern, ignored, kept)
    const CASES: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "directory-sharing-a-prefix",
            "test_*.py",
            &["test_one.py", "pkg/test_two.py"],
            &["test_helpers/util.py", "test_helpers/sub/deep.py"],
        ),
        (
            "wildcard-before-suffix",
            "doc*.md",
            &["doc.md", "pkg/docs.md"],
            &["docs/api.md", "docs/guide/readme.md"],
        ),
        (
            "star-does-not-cross-slash",
            "s*c.py",
            &["sc.py", "pkg/spec.py"],
            &["src/c.py", "src/a/c.py"],
        ),
        (
            "negation-names-the-entry-not-its-files",
            "*.py\n!vendor",
            &["c.py", "vendor/a.py", "vendor/sub/b.py"],
            &["vendor/notes.txt"],
        ),
        (
            "negation-does-not-cross-slash",
            "*.py\n!test_*.py",
            &["util.py", "test_helpers/util.py"],
            &["test_one.py", "pkg/test_two.py"],
        ),
        (
            "negated-ancestor-name-re-includes-nothing-below",
            "build\n!vendor",
            &["build/out.py", "vendor/build/out.py"],
            &["vendor/ok.py", "pkg/ok.py"],
        ),
        (
            "plain-name-still-excludes-its-directory",
            "build",
            &["build/out.py", "pkg/build/deep/out.py"],
            &["build.py", "builder/ok.py", "notbuild/ok.py"],
        ),
        (
            "directory-only-still-excludes-its-directory",
            "build/",
            &["build/out.py", "pkg/build/deep/out.py"],
            &["build.py", "builder/ok.py", "notbuild/ok.py"],
        ),
    ];
    for &(id, pattern, ign, kept) in CASES {
        let t = TempDir::new();
        must_write(&t.join(".graphifyignore"), &format!("{pattern}\n"));
        for rel in ign {
            assert!(ignored(t.path(), rel, false), "{id}: {rel} ignored");
        }
        for rel in kept {
            assert!(!ignored(t.path(), rel, false), "{id}: {rel} kept");
        }
    }
}

/// `test_anchored_dir_not_matched_at_depth` (#1087).
#[test]
fn anchored_dir_not_matched_at_depth() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "/inbox/\n");
    assert!(!ignored(t.path(), "src/inbox/main.rs", false));
    assert!(!ignored(t.path(), "src/inbox", true));
}

/// `test_anchored_dir_matches_at_root` (#1087).
#[test]
fn anchored_dir_matches_at_root() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "/inbox/\n");
    assert!(ignored(t.path(), "inbox/data.json", false));
    assert!(ignored(t.path(), "inbox", true));
}

/// `test_anchored_file_not_matched_at_depth` (#1087).
#[test]
fn anchored_file_not_matched_at_depth() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "/build\n");
    assert!(!ignored(t.path(), "src/build", true));
}

/// `test_unanchored_dir_still_matches_at_depth` (#1087).
#[test]
fn unanchored_dir_still_matches_at_depth() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "inbox/\n");
    assert!(ignored(t.path(), "src/inbox/main.rs", false));
}

/// `test_anchored_multi_segment_pattern` (#1087).
#[test]
fn anchored_multi_segment_pattern() {
    let t = TempDir::new();
    must_write(&t.join(".graphifyignore"), "/src/inbox/\n");
    assert!(ignored(t.path(), "src/inbox/a.py", false));
    assert!(!ignored(t.path(), "x/src/inbox/b.py", false));
}

/// `test_is_ignored_cache_matches_uncached_results` (#1235): one ignorer reused over the paths
/// (Go caches each dir's rules) agrees with a fresh one per path, negation included.
#[test]
fn is_ignored_cache_matches_uncached_results() {
    // (rel, is_dir)
    const PATHS: &[(&str, bool)] = &[
        ("build", true),
        ("build/out.o", false),
        ("build/sub", true),
        ("build/sub/deep.o", false),
        ("logs", true),
        ("logs/drop.log", false),
        ("logs/keep.log", false),
        ("src/main.py", false),
    ];
    let t = TempDir::new();
    must_write(
        &t.join(".graphifyignore"),
        "build/\n*.log\n!logs/keep.log\n",
    );
    let shared = new_ignorer(t.path(), &Env::process());
    for &(rel, is_dir) in PATHS {
        assert_eq!(
            walk_ignored(&shared, rel, is_dir),
            ignored(t.path(), rel, is_dir),
            "cached result for {rel} differs from uncached"
        );
    }
    assert!(!ignored(t.path(), "logs/keep.log", false));
    assert!(ignored(t.path(), "logs/drop.log", false));
}

/// `test_string_matcher_preserves_pattern_forms`.
#[test]
fn string_matcher_preserves_pattern_forms() {
    // (rel, is_dir, ignored)
    const CASES: &[(&str, bool, bool)] = &[
        // dir-only pattern: matches the dir and everything under it...
        ("build", true, true),
        ("build/x.py", false, true),
        // ...but never a plain FILE of the same name
        ("notbuild/build", false, false),
        // glob + basename negation, last match wins
        ("logs/drop.log", false, true),
        ("logs/keep.log", false, false),
        // anchored file pattern: root level only
        ("anchored.md", false, true),
        ("sub/anchored.md", false, false),
        // ** crosses path segments under its anchor
        ("docs/deep/guide.md", false, true),
        // bare segment pattern matches an intermediate directory anywhere
        ("x/cache", true, true),
        ("x/cache/y/f.py", false, true),
        // unrelated paths untouched
        ("notbuild", true, false),
        ("sub", true, false),
    ];
    let t = TempDir::new();
    must_write(
        &t.join(".graphifyignore"),
        "build/\n*.log\n!keep.log\n/anchored.md\ndocs/**\ncache\n",
    );
    for &(rel, is_dir, want) in CASES {
        assert_eq!(ignored(t.path(), rel, is_dir), want, "{rel}");
    }
}

// --- _is_sensitive ---

/// The `_is_sensitive` cases Go agrees with, as `(upstream test, path, sensitive)`.
const SENSITIVE_HOLDS: &[(&str, &str, bool)] = &[
    (
        "test_sensitive_flags_underscore_secret",
        "app_secret.yaml",
        true,
    ),
    (
        "test_sensitive_does_not_flag_tokenizer_py",
        "tokenizer.py",
        false,
    ),
    (
        "test_sensitive_does_not_flag_tokenize_py",
        "tokenize.py",
        false,
    ),
    (
        "test_sensitive_does_not_flag_passwords_py",
        "passwords.py",
        false,
    ),
    (
        "test_sensitive_does_not_flag_ruby_code_modules",
        "app/models/device_token.rb",
        false,
    ),
    (
        "test_sensitive_does_not_flag_ruby_code_modules",
        "app/controllers/api/v1/passwords_controller.rb",
        false,
    ),
    (
        "test_sensitive_still_flags_data_secret_stores",
        "credentials.json",
        true,
    ),
    (
        "test_sensitive_still_flags_data_secret_stores",
        "app_secret.yaml",
        true,
    ),
    (
        "test_sensitive_flags_ssh_dir",
        "/home/user/.ssh/id_rsa",
        true,
    ),
    (
        "test_sensitive_flags_secrets_dir",
        "config/secrets/db.json",
        true,
    ),
    (
        "test_sensitive_flags_credentials_json",
        "credentials.json",
        true,
    ),
    (
        "test_sensitive_does_not_flag_root_file_named_credentials",
        "credentials",
        true,
    ),
    (
        "test_sensitive_secret_handler_txt",
        "secret_handler.txt",
        true,
    ),
    (
        "test_sensitive_still_flags_data_under_secrets_dir",
        "secrets/db.json",
        true,
    ),
    (
        "test_sensitive_still_flags_data_under_secrets_dir",
        ".secrets/token.yaml",
        true,
    ),
    (
        "test_sensitive_still_flags_data_under_secrets_dir",
        "deploy/credentials/prod.env",
        true,
    ),
    (
        "test_sensitive_still_flags_data_under_secrets_dir",
        "internal/secrets/README.md",
        true,
    ),
    (
        "test_sensitive_flags_everything_under_credential_store_dirs",
        "/home/user/.ssh/config",
        true,
    ),
    (
        "test_sensitive_flags_everything_under_credential_store_dirs",
        ".aws/credentials",
        true,
    ),
    (
        "test_sensitive_flags_everything_under_credential_store_dirs",
        ".gnupg/helper.py",
        true,
    ),
    (
        "test_sensitive_flags_everything_under_credential_store_dirs",
        "backup/.gcloud/sync.sh",
        true,
    ),
    (
        "test_sensitive_dir_carveout_does_not_bypass_name_screens",
        "credentials/id_rsa",
        true,
    ),
    (
        "test_sensitive_dir_carveout_does_not_bypass_name_screens",
        "secrets/deploy.pem",
        true,
    ),
    (
        "test_sensitive_dir_carveout_still_drops_tfvars_values_store",
        "secrets/prod.tfvars",
        true,
    ),
    (
        "test_sensitive_does_not_flag_token_economics_note",
        "token-economics-of-recall.md",
        false,
    ),
    (
        "test_sensitive_does_not_flag_password_policy_discussion",
        "password-policy-discussion.md",
        false,
    ),
    (
        "test_sensitive_flags_my_private_key_txt",
        "my_private_key.txt",
        true,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "wiki/privacy-tokens.md",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "wiki/ai-token-economics.md",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "wiki/chain-of-hope-tokenomics.md",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "tokenizer.py",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "secretary.py",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "google/oauth2/service_account.py",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "docs/service-account-setup.md",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "wiki/aws_credentials_rotation_guide.md",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "token.economics.notes.md",
        false,
    ),
    (
        "test_sensitive_filter_indexes_topic_prose_and_source",
        "password-reset/design.md",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        ".env",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "id_rsa",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "credentials.json",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "server.pem",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "certs/server.key",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "secrets.md",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "passwords.md",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        ".git-credentials",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "secrets/prod.tfvars",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "credentials/id_rsa",
        true,
    ),
    (
        "test_sensitive_bare_keyword_prose_still_dropped",
        "secrets.md",
        true,
    ),
    (
        "test_sensitive_bare_keyword_prose_still_dropped",
        "token-lifecycle.md",
        false,
    ),
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (these hold only because Go has no `token` keyword)
    (
        "test_sensitive_bare_plural_tokens_prose_indexed",
        "TOKENS.md",
        false,
    ),
    (
        "test_sensitive_bare_plural_tokens_prose_indexed",
        "tokens.md",
        false,
    ),
    (
        "test_sensitive_bare_plural_tokens_prose_indexed",
        "tokens.rst",
        false,
    ),
    (
        "test_sensitive_design_token_source_files_indexed",
        "app/lib/theme/shell_tokens.dart",
        false,
    ),
    (
        "test_sensitive_design_token_source_files_indexed",
        "src/design/tokens.ts",
        false,
    ),
    (
        "test_sensitive_design_token_source_files_indexed",
        "src/hard-tokens.ts",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_env_files",
        ".env",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_env_files",
        ".env.local",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_env_files",
        ".env.production",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_env_files",
        ".envrc",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_env_files",
        ".env.example.local",
        true,
    ),
    (
        "test_sensitive_filter_still_excludes_real_env_files",
        ".env.example.bak",
        true,
    ),
    (
        "test_sensitive_env_template_inside_secrets_dir_still_dropped",
        "secrets/.env.example",
        true,
    ),
    (
        "test_sensitive_env_template_inside_secrets_dir_still_dropped",
        "deploy/credentials/.env.example",
        true,
    ),
];

/// The `_is_sensitive` cases where upstream asserts the opposite, with Go's value.
const SENSITIVE_PINNED: &[(&str, &str, bool)] = &[
    // pins Go v0.12.0 (false: no `token` keyword); upstream 0.9.107 emits true -> P3
    ("test_sensitive_flags_api_token_txt", "api_token.txt", false),
    (
        "test_sensitive_flags_oauth_token_json",
        "oauth_token.json",
        false,
    ),
    (
        "test_sensitive_still_flags_data_secret_stores",
        "oauth_token.json",
        false,
    ),
    ("test_sensitive_flags_token_txt", "token.txt", false),
    (
        "test_sensitive_token_config_yaml",
        "token_config.yaml",
        false,
    ),
    (
        "test_sensitive_flags_keyword_at_end_of_long_name",
        "github-personal-access-token.txt",
        false,
    ),
    ("test_sensitive_flags_dotfile_token", ".token", false),
    (
        "test_sensitive_flags_plural_tokens_txt",
        "tokens.txt",
        false,
    ),
    (
        "test_sensitive_bare_plural_tokens_still_flagged_outside_prose",
        "tokens.txt",
        false,
    ),
    (
        "test_sensitive_bare_plural_tokens_still_flagged_outside_prose",
        "tokens.json",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "token.md",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "token.txt",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "api_token.json",
        false,
    ),
    (
        "test_sensitive_bare_keyword_prose_still_dropped",
        "token.rst",
        false,
    ),
    // pins Go v0.12.0 (false: no service-account, .npmrc, .pypirc or secring rule); upstream 0.9.107 emits true -> P3
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "service-account.json",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        ".npmrc",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        ".pypirc",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "secring.gpg",
        false,
    ),
    // pins Go v0.12.0 (false: dir names and id_rsa match case-sensitively); upstream 0.9.107 emits true -> P3
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "Secrets/creds.json",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "SECRETS/db.json",
        false,
    ),
    (
        "test_sensitive_filter_still_excludes_real_secrets",
        "ID_RSA",
        false,
    ),
    // pins Go v0.12.0 (true: no source carve-out under secrets/ or credentials/); upstream 0.9.107 emits false -> P3
    (
        "test_sensitive_does_not_flag_source_under_secrets_dir",
        "internal/secrets/vault.go",
        true,
    ),
    (
        "test_sensitive_does_not_flag_source_under_secrets_dir",
        "app/services/credentials/manager.py",
        true,
    ),
    (
        "test_sensitive_dir_carveout_does_not_bypass_name_screens",
        "secrets/service_account.py",
        true,
    ),
    (
        "test_sensitive_dir_carveout_still_drops_tfvars_values_store",
        "secrets/loader.py",
        true,
    ),
    (
        "test_sensitive_dir_carveout_still_drops_tfvars_values_store",
        "secrets/main.tf",
        true,
    ),
    // pins Go v0.12.0 (true: no dotenv-template carve-out); upstream 0.9.107 emits false -> P3
    (
        "test_sensitive_filter_indexes_env_templates",
        ".env.example",
        true,
    ),
    (
        "test_sensitive_filter_indexes_env_templates",
        ".env.sample",
        true,
    ),
    (
        "test_sensitive_filter_indexes_env_templates",
        ".env.template",
        true,
    ),
    (
        "test_sensitive_filter_indexes_env_templates",
        ".env.dist",
        true,
    ),
    (
        "test_sensitive_filter_indexes_env_templates",
        ".ENV.EXAMPLE",
        true,
    ),
    (
        "test_sensitive_filter_indexes_env_templates",
        ".envrc.sample",
        true,
    ),
    (
        "test_sensitive_filter_indexes_env_templates",
        ".env.production.example",
        true,
    ),
];

/// The 35 `_is_sensitive` tests (#920, #1666, #1943, #2106, #2184, #2232, #3527), one row per
/// path.
#[test]
fn is_sensitive_cases() {
    for &(name, path, want) in SENSITIVE_HOLDS.iter().chain(SENSITIVE_PINNED) {
        assert_eq!(is_sensitive(path), want, "{name}: {path}");
    }
}

// --- _shebang_interpreter ---

/// `(upstream test, first line(s), Go interpreter, Go ShebangExt)`; `None` where upstream does
/// not assert it.
type ShebangCase = (
    &'static str,
    &'static [u8],
    Option<&'static str>,
    Option<&'static str>,
);

const SHEBANG_HOLDS: &[ShebangCase] = &[
    ("test_shebang_interpreter_plain", b"#!/usr/bin/python3\nprint('x')\n", Some("python3"), None),
    ("test_shebang_interpreter_env_single_arg", b"#!/usr/bin/env python3\nprint('x')\n", Some("python3"), None),
    ("test_shebang_interpreter_env_dash_s", b"#!/usr/bin/env -S python3 -u\nprint('x')\n", Some("python3"), None),
    ("test_shebang_interpreter_env_with_flags", b"#!/usr/bin/env -i bash\necho hi\n", Some("bash"), None),
    ("test_shebang_interpreter_env_with_assignment", b"#!/usr/bin/env DEBUG=1 python3\nprint('x')\n", Some("python3"), None),
    ("test_shebang_interpreter_keeps_unknown_versioned_name", b"#!/usr/bin/env mytool2.1\nrun\n", Some("mytool2.1"), Some("")),
    ("test_shebang_interpreter_no_shebang", b"print('x')\n", Some(""), None),
    ("test_shebang_file_type_classifies_via_interpreter", b"#!/usr/bin/env -S python3 -u\nprint('x')\n", None, Some(".py")),
    ("test_shebang_interpreter_env_clumped_u_operand", b"#!/usr/bin/env -uPYTHONPATH python3\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_missing_operand_returns_none", b"#!/usr/bin/env -u\n", Some(""), None),
    ("test_shebang_interpreter_env_long_unset_equals", b"#!/usr/bin/env --unset=PYTHONPATH python3\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_long_chdir_equals", b"#!/usr/bin/env --chdir=/tmp python3\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_signal_flags", b"#!/usr/bin/env --default-signal=TERM --ignore-signal=PIPE python3\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_dash_s_assignment_before_interpreter", b"#!/usr/bin/env -S PYTHONPATH=/opt/custom:${PYTHONPATH} python3\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_dash_s_flag_before_interpreter", b"#!/usr/bin/env -S -i OLDUSER=${USER} python3\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_long_split_assignment_before_interpreter", b"#!/usr/bin/env --split-string='PYTHONPATH=/opt/custom:${PYTHONPATH} python3 -u'\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_long_split_flag_before_interpreter", b"#!/usr/bin/env --split-string='-i python3 -u'\nprint('x')\n", Some("python3"), Some(".py")),
    ("test_shebang_interpreter_env_vs_assignment_before_interpreter", b"#!/usr/bin/env -vS DEBUG=1 python3 -u\nprint('x')\n", Some("python3"), Some(".py")),
];

/// Go's `env` parsing skips every `-word` and `NAME=value` word and takes the next one as the
/// interpreter, with no operands, quoting or `-S` splitting.
const SHEBANG_PINNED: &[ShebangCase] = &[
    // pins Go v0.12.0 (the versioned name, no extractor); upstream 0.9.107 emits python3/python2/lua/php and CODE -> P3 (B54)
    (
        "test_shebang_interpreter_drops_version_suffix",
        b"#!/usr/bin/env python3.12\nx = 1\n",
        Some("python3.12"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_drops_version_suffix",
        b"#!/usr/local/bin/python3.11\nx = 1\n",
        Some("python3.11"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_drops_version_suffix",
        b"#!/usr/bin/python2.7\nx = 1\n",
        Some("python2.7"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_drops_version_suffix",
        b"#!/usr/bin/lua5.4\nx = 1\n",
        Some("lua5.4"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_drops_version_suffix",
        b"#!/usr/bin/env php8.2\nx = 1\n",
        Some("php8.2"),
        Some(""),
    ),
    // pins Go v0.12.0 (`python3"`: no shlex quoting); upstream 0.9.107 emits python3 -> P3
    (
        "test_shebang_interpreter_quoted_path",
        b"#!\"/usr/local/bin/python3\"\nprint(\"x\")\n",
        Some("python3\""),
        None,
    ),
    // pins Go v0.12.0 (the option's operand, and no extractor); upstream 0.9.107 emits python3 and CODE -> P3
    (
        "test_shebang_interpreter_env_unset_with_operand",
        b"#!/usr/bin/env -u PYTHONPATH python3\nprint('x')\n",
        Some("PYTHONPATH"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_chdir_with_operand",
        b"#!/usr/bin/env -C /tmp python3\nprint('x')\n",
        Some("tmp"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_path_with_operand",
        b"#!/usr/bin/env -P /bin python3\nprint('x')\n",
        Some("bin"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_gnu_argv0_operand",
        b"#!/usr/bin/env -a alias python3\nprint('x')\n",
        Some("alias"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_long_unset_separate_operand",
        b"#!/usr/bin/env --unset PYTHONPATH python3\nprint('x')\n",
        Some("PYTHONPATH"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_long_chdir_separate_operand",
        b"#!/usr/bin/env --chdir /tmp python3\nprint('x')\n",
        Some("tmp"),
        Some(""),
    ),
    // pins Go v0.12.0 (`"python3`: the quote stays on the word); upstream 0.9.107 emits python3 and CODE -> P3
    (
        "test_shebang_interpreter_env_dash_s_after_flag",
        b"#!/usr/bin/env -i -S \"python3 -u\"\nprint(\"x\")\n",
        Some("\"python3"),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_gnu_split_string_separate",
        b"#!/usr/bin/env --split-string \"python3 -u\"\nprint(\"x\")\n",
        Some("\"python3"),
        Some(""),
    ),
    // pins Go v0.12.0 (""; every word starts with `-`); upstream 0.9.107 emits python3 and CODE -> P3
    (
        "test_shebang_interpreter_env_gnu_split_string_equals",
        b"#!/usr/bin/env --split-string='python3 -u'\nprint('x')\n",
        Some(""),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_compact_dash_s",
        b"#!/usr/bin/env -Spython3 -u\nprint('x')\n",
        Some(""),
        Some(""),
    ),
    (
        "test_shebang_interpreter_env_compact_v_then_s",
        b"#!/usr/bin/env -vSpython3 -u\nprint('x')\n",
        Some(""),
        Some(""),
    ),
    // pins Go v0.12.0 (python3); upstream 0.9.107 emits None -> P3
    (
        "test_shebang_interpreter_env_unknown_option_returns_none",
        b"#!/usr/bin/env --no-such-flag python3\n",
        Some("python3"),
        None,
    ),
    (
        "test_shebang_interpreter_env_nested_split_string_rejected",
        b"#!/usr/bin/env -S -S python3 -u\nprint('x')\n",
        Some("python3"),
        None,
    ),
];

/// The 33 `_shebang_interpreter` tests that read a first line, one row per case (the five
/// versions of `drops_version_suffix` are five rows).
#[test]
fn shebang_interpreter_cases() {
    for &(name, src, interp, ext) in SHEBANG_HOLDS.iter().chain(SHEBANG_PINNED) {
        if let Some(want) = interp {
            assert_eq!(shebang_interpreter(src), want, "{name}: interpreter");
        }
        if let Some(want) = ext {
            assert_eq!(shebang_ext(src), want, "{name}: ShebangExt");
        }
    }
}

/// `test_shebang_interpreter_unreadable_returns_none`.
#[test]
fn shebang_interpreter_unreadable_returns_none() {
    let t = TempDir::new();
    assert_eq!(shebang_ext_of_file(&t.join("does_not_exist")), "");
}

// --- unclassified files and walk errors: Go's WalkReport ---

/// `test_detect_records_unclassified_extensionless_files` (#1692): Go counts them by extension
/// instead of listing them.
#[test]
fn detect_records_unclassified_extensionless_files() {
    let t = TempDir::new();
    must_write(&t.join("app.py"), "def f():\n    return 1\n");
    must_write(
        &t.join("Dockerfile"),
        "FROM python:3.12\nRUN pip install x\n",
    );
    must_write(&t.join("Makefile"), "build:\n\techo hi\n");
    must_write(&t.join("LICENSE"), "MIT License\n");
    let rep = collect_files_report(t.path(), WORKERS).expect("CollectFilesReport");
    assert_eq!(rep.skipped, 3);
    assert_eq!(
        rep.skipped_exts,
        BTreeMap::from([("(no extension)".to_string(), 3)])
    );
    assert!(rep.files.iter().any(|f| f == "app.py"), "{:?}", rep.files);
}

/// `test_detect_unclassified_empty_when_all_supported`.
#[test]
fn detect_unclassified_empty_when_all_supported() {
    let t = TempDir::new();
    must_write(&t.join("a.py"), "x = 1\n");
    must_write(&t.join("README.md"), "# hi\n");
    let rep = collect_files_report(t.path(), WORKERS).expect("CollectFilesReport");
    assert_eq!(rep.skipped, 0);
    assert!(rep.skipped_exts.is_empty(), "{:?}", rep.skipped_exts);
}

/// `test_unclassified_warning_fires_when_most_of_the_corpus_has_no_extractor` (#2942),
/// `test_unclassified_warning_silent_under_the_absolute_floor`,
/// `test_unclassified_warning_silent_under_the_fraction_threshold`: the count; Go has no
/// `unclassified_warning` (B67).
#[test]
fn unclassified_warning_counts() {
    // (upstream test, .xml files, .py files)
    const CASES: &[(&str, usize, usize)] = &[
        ("fires_when_most_of_the_corpus_has_no_extractor", 30, 10),
        ("silent_under_the_absolute_floor", 2, 20),
        ("silent_under_the_fraction_threshold", 25, 400),
    ];
    for &(name, xml, py) in CASES {
        let t = TempDir::new();
        for i in 0..xml {
            must_write(&t.join(&format!("doc{i}.xml")), "<root/>\n");
        }
        for i in 0..py {
            must_write(&t.join(&format!("code{i}.py")), "x = 1\n");
        }
        let rep = collect_files_report(t.path(), WORKERS).expect("CollectFilesReport");
        assert_eq!(rep.skipped, xml as i64, "{name}");
        assert_eq!(rep.files.len(), py, "{name}");
    }
}

/// `test_graphifyinclude_is_inert_and_not_unclassified` (#2112): a `.graphifyinclude` changes
/// no collected file.
#[test]
fn graphifyinclude_is_inert_and_not_unclassified() {
    let t = TempDir::new();
    must_write(&t.join("main.py"), "x = 1\n");
    let baseline = collect_files_report(t.path(), WORKERS).expect("CollectFilesReport");
    must_write(&t.join(".graphifyinclude"), ".github/\ndocs/**\n");
    let rep = collect_files_report(t.path(), WORKERS).expect("CollectFilesReport");
    assert_eq!(rep.files, baseline.files);
    assert_eq!(rep.files, ["main.py"]);
    // pins Go v0.12.0 (counted under ".graphifyinclude"); upstream 0.9.107 leaves it out of unclassified and warns once -> P3 (B67)
    assert_eq!(
        rep.skipped_exts,
        BTreeMap::from([(".graphifyinclude".to_string(), 1)])
    );
}

/// `test_detect_reports_walk_errors_key`.
#[test]
fn detect_reports_walk_errors_key() {
    let t = TempDir::new();
    must_write(&t.join("a.py"), "def f(): pass\n");
    let rep = collect_files_report(t.path(), WORKERS).expect("CollectFilesReport");
    assert!(rep.walk_errors.is_empty(), "{:?}", rep.walk_errors);
}

/// `test_detect_surfaces_unreadable_dir_instead_of_silent_skip`, without the stderr line (the
/// CLI prints it). Skipped as root, as upstream does.
#[test]
fn detect_surfaces_unreadable_dir_instead_of_silent_skip() {
    let t = TempDir::new();
    must_write(&t.join("a.py"), "def f(): pass\n");
    must_write(&t.join("locked/b.py"), "def g(): pass\n");
    chmod(&t.join("locked"), 0o000);
    if readable_despite_chmod(&t.join("locked")) {
        chmod(&t.join("locked"), 0o755);
        return; // running as root: chmod 000 does not block the read
    }
    let rep = collect_files_report(t.path(), WORKERS);
    chmod(&t.join("locked"), 0o755);
    let rep = rep.expect("CollectFilesReport");
    assert!(rep.files.iter().any(|f| f == "a.py"), "{:?}", rep.files);
    assert!(!rep.walk_errors.is_empty());
}

// --- VCS root (#4444, #4683): Go's findVCSRoot and nested-worktree pruning ---

/// Upstream `_home_with_invalid_git`: `$HOME` with an empty `.git` and a `.gitignore`, above a
/// project. Returns `(guard, home, proj)`.
fn home_with_invalid_git() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let (d, t) = resolved();
    let home = t.join("home");
    std::fs::create_dir_all(home.join(".git")).expect("mkdir");
    must_write(&home.join(".gitignore"), "node_modules\n");
    let proj = home.join("work/proj");
    must_write(&proj.join(".graphifyignore"), "vendor/\n*.generated.py\n");
    must_write(&proj.join("main.py"), "x = 1");
    (d, home, proj)
}

/// `test_invalid_git_dir_is_not_a_vcs_root` (4 params), the `_find_vcs_root` half.
#[test]
fn invalid_git_dir_is_not_a_vcs_root() {
    let contents: [&[&str]; 4] = [&[], &["config"], &["HEAD"], &["HEAD", "objects"]];
    for c in contents {
        let (_d, home, proj) = home_with_invalid_git();
        for name in c {
            match *name {
                "HEAD" => must_write(&home.join(".git/HEAD"), "ref: refs/heads/main\n"),
                "config" => must_write(&home.join(".git/config"), "[core]\n"),
                dir => std::fs::create_dir_all(home.join(".git").join(dir)).expect("mkdir"),
            }
        }
        assert_eq!(find_vcs_root(&proj, &env_home(home.clone())), None, "{c:?}");
    }
}

/// `test_invalid_git_dir_walk_continues_to_valid_parent`.
#[test]
fn invalid_git_dir_walk_continues_to_valid_parent() {
    let (_d, t) = resolved();
    mk_git(&t);
    must_write(&t.join(".graphifyignore"), "vendor/\n");
    std::fs::create_dir_all(t.join("pkg/.git")).expect("mkdir");
    assert_eq!(
        find_vcs_root(&t.join("pkg"), &env_home(t.join("nohome"))),
        Some(t.clone())
    );
}

/// `test_valid_git_dir_is_a_vcs_root`: and its `.gitignore` reaches the project.
#[test]
fn valid_git_dir_is_a_vcs_root() {
    let (_d, home, proj) = home_with_invalid_git();
    std::fs::remove_dir_all(home.join(".git")).expect("rm");
    mk_git(&home);
    let env = env_home(home.clone());
    assert_eq!(find_vcs_root(&proj, &env), Some(home.clone()));
    assert!(new_ignorer(&proj, &env).ignored("node_modules", true));
}

/// `test_git_file_with_gitdir_is_a_vcs_root`.
#[test]
fn git_file_with_gitdir_is_a_vcs_root() {
    let (_d, t) = resolved();
    let wt = t.join("wt");
    std::fs::create_dir_all(wt.join("sub")).expect("mkdir");
    must_write(&wt.join(".git"), "gitdir: /somewhere/.git/worktrees/wt\n");
    assert_eq!(
        find_vcs_root(&wt.join("sub"), &env_home(t.join("nohome"))),
        Some(wt)
    );
}

/// `test_git_file_without_gitdir_is_skipped`, the `_find_vcs_root` half.
#[test]
fn git_file_without_gitdir_is_skipped() {
    let (_d, home, proj) = home_with_invalid_git();
    std::fs::remove_dir_all(home.join(".git")).expect("rm");
    must_write(&home.join(".git"), "not a git file\n");
    assert_eq!(find_vcs_root(&proj, &env_home(home)), None);
}

/// `test_real_git_repo_and_worktree_are_vcs_roots`, with the layout `git init` and
/// `git worktree add` leave written by hand (Go reads only the `.git` dir or file).
#[test]
fn real_git_repo_and_worktree_are_vcs_roots() {
    let (_d, t) = resolved();
    let repo = t.join("repo");
    mk_git(&repo);
    std::fs::create_dir_all(repo.join("sub")).expect("mkdir");
    let wt = t.join("wt");
    must_write(
        &wt.join(".git"),
        &format!("gitdir: {}\n", repo.join(".git/worktrees/wt").display()),
    );
    let env = env_home(t.join("nohome"));
    assert_eq!(find_vcs_root(&repo.join("sub"), &env), Some(repo));
    assert_eq!(find_vcs_root(&wt, &env), Some(wt.clone()));
}

/// Upstream `_git_workspace_with_worktree` by hand: `ws/app` (a repo) and `ws/app-feature` (its
/// linked worktree, a `.git` file).
fn workspace_with_worktree(t: &Path) -> std::path::PathBuf {
    let ws = t.join("ws");
    mk_git(&ws.join("app"));
    must_write(&ws.join("app/src/a.py"), "x = 1\n");
    must_write(
        &ws.join("app-feature/.git"),
        &format!(
            "gitdir: {}\n",
            ws.join("app/.git/worktrees/app-feature").display()
        ),
    );
    must_write(&ws.join("app-feature/src/a.py"), "x = 1\n");
    ws
}

/// `test_detect_prunes_sibling_linked_worktree` (#4683), without `pruned_noise_dirs`.
#[test]
fn detect_prunes_sibling_linked_worktree() {
    let t = TempDir::new();
    let ws = workspace_with_worktree(t.path());
    let got = collected(&ws);
    assert!(got.contains("app/src/a.py"), "main checkout was dropped");
    assert!(
        !got.iter().any(|f| f.starts_with("app-feature/")),
        "linked worktree indexed as a second copy"
    );
}

/// `test_detect_indexes_worktree_as_scan_root` (#4683).
#[test]
fn detect_indexes_worktree_as_scan_root() {
    let t = TempDir::new();
    let ws = workspace_with_worktree(t.path());
    assert!(collected(&ws.join("app-feature")).contains("src/a.py"));
}

/// `test_detect_keeps_worktree_whose_main_is_outside_root` (#4683): Go prunes every dir that
/// holds a `.git` file.
#[test]
fn detect_keeps_worktree_whose_main_is_outside_root() {
    let t = TempDir::new();
    mk_git(&t.join("main"));
    must_write(&t.join("main/src/a.py"), "x = 1\n");
    must_write(
        &t.join("only/wt/.git"),
        &format!("gitdir: {}\n", t.join("main/.git/worktrees/wt").display()),
    );
    must_write(&t.join("only/wt/src/a.py"), "x = 1\n");
    // pins Go v0.12.0 (wt/src/a.py not collected); upstream 0.9.107 emits it -> P3
    assert!(!any_has(&collected(&t.join("only")), "src/a.py"));
}

/// `test_detect_keeps_submodule_gitdir_without_commondir` (#4683).
#[test]
fn detect_keeps_submodule_gitdir_without_commondir() {
    let t = TempDir::new();
    let ws = t.join("ws");
    let gitdir = ws.join("super/.git/modules/sub");
    std::fs::create_dir_all(&gitdir).expect("mkdir");
    must_write(
        &ws.join("vendor/sub/.git"),
        &format!("gitdir: {}\n", gitdir.display()),
    );
    must_write(&ws.join("vendor/sub/mod.py"), "x = 1\n");
    // pins Go v0.12.0 (sub/mod.py not collected: vendor is a skip dir and sub holds a .git file); upstream 0.9.107 emits it -> P3
    assert!(!any_has(&collected(&ws), "sub/mod.py"));
}

/// `test_git_dir_symlink_is_followed`.
#[test]
fn git_dir_symlink_is_followed() {
    let (_d, t) = resolved();
    mk_git(&t.join("x"));
    std::fs::rename(t.join("x/.git"), t.join("real-git")).expect("rename");
    std::fs::create_dir_all(t.join("repo")).expect("mkdir");
    symlink(t.join("real-git"), t.join("repo/.git")).expect("symlink");
    assert_eq!(
        find_vcs_root(&t.join("repo"), &env_home(t.join("nohome"))),
        Some(t.join("repo"))
    );
}
