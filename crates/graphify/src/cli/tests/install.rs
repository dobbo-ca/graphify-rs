//! Port of `install_test.go` (2 tests), then the upstream case of `test_install.py` that holds for
//! graphify-go v0.12.0. Go's `t.Setenv("HOME", home)` is `install_in(home, ..)`: no test touches
//! the real `$HOME`.

use super::install_in;
use crate::cli::maintenance::maint_util::{args, must};
use crate::testutil::TempDir;

fn dest(home: &TempDir) -> std::path::PathBuf {
    home.join(".claude/skills/graphify/SKILL.md")
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_skill_idempotent_and_uninstall() {
    let home = TempDir::new();
    let dest = dest(&home);
    let mut out = Vec::<u8>::new();

    for _ in 0..2 {
        // re-running is a no-op
        must("install", install_in(home.path(), &[], &mut out));
        let got = std::fs::read_to_string(&dest)
            .unwrap_or_else(|e| panic!("read {}: {e}", dest.display()));
        assert!(
            got.contains("name: graphify"),
            "installed skill missing frontmatter: {:?}",
            got.chars().take(80).collect::<String>()
        );
    }

    must(
        "uninstall",
        install_in(home.path(), &args(&["--uninstall"]), &mut out),
    );
    let dir = dest.parent().unwrap();
    assert!(!dir.exists(), "skill dir still present after uninstall");
    // uninstall removes only the graphify skill, leaving ~/.claude/skills intact
    assert!(home.join(".claude/skills").exists(), "skills dir removed");
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_rejects_unknown_args() {
    let home = TempDir::new();
    let mut out = Vec::<u8>::new();
    assert!(
        install_in(home.path(), &args(&["--nope"]), &mut out).is_err(),
        "expected error for unknown arg"
    );
}

// ---- upstream `test_install.py` ----

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_default_claude() {
    let home = TempDir::new();
    must(
        "install",
        install_in(home.path(), &[], &mut Vec::<u8>::new()),
    );
    assert!(dest(&home).exists());
}
