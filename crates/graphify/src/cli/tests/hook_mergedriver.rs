//! Port of `hook_mergedriver_test.go` (2 tests), then the merge-driver cases of upstream
//! `test_hooks.py` that hold for graphify-go v0.12.0.

use super::MERGE_ATTR_LINE;
use super::maint_util::{git_config_get, git_run, install_ok, uninstall_ok};
use crate::testutil::TempDir;

fn init(repo: &TempDir) {
    git_run(repo.path(), &["init"]);
}

// hook install must register the graph.json union merge driver (config key + .gitattributes
// line), do so idempotently, and hook uninstall must remove both (#1902).
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_hook_registers_merge_driver() {
    let repo = TempDir::new();
    init(&repo);
    let attr_path = repo.join(".gitattributes");

    install_ok(repo.path());

    // merge.graphify.driver config points at graphify's merge-driver command.
    let out = git_config_get(repo.path(), "merge.graphify.driver");
    assert!(
        out.as_deref().is_some_and(|o| o.contains("merge-driver")),
        "merge.graphify.driver not registered: out={out:?}"
    );
    // .gitattributes carries the graph.json line exactly once.
    let data =
        std::fs::read_to_string(&attr_path).unwrap_or_else(|e| panic!("read .gitattributes: {e}"));
    let c = data.matches(MERGE_ATTR_LINE).count();
    assert_eq!(
        c, 1,
        ".gitattributes has {c} copies of the merge line, want 1:\n{data}"
    );

    // Idempotent: reinstall must not duplicate the .gitattributes line.
    install_ok(repo.path());
    let data = std::fs::read_to_string(&attr_path).unwrap_or_default();
    let c = data.matches(MERGE_ATTR_LINE).count();
    assert_eq!(
        c, 1,
        "after reinstall .gitattributes has {c} copies, want 1:\n{data}"
    );

    // Uninstall removes both the config key and the .gitattributes line.
    uninstall_ok(repo.path());
    assert!(
        git_config_get(repo.path(), "merge.graphify.driver").is_none(),
        "merge.graphify.driver still set after uninstall"
    );
    let data = std::fs::read_to_string(&attr_path).unwrap_or_default();
    assert!(
        !data.contains(MERGE_ATTR_LINE),
        ".gitattributes still has the merge line after uninstall:\n{data}"
    );
}

// Registering the merge driver must preserve pre-existing .gitattributes entries.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_merge_driver_preserves_existing_attributes() {
    let repo = TempDir::new();
    init(&repo);
    let attr_path = repo.write(".gitattributes", b"*.md text\n");
    install_ok(repo.path());
    let data = std::fs::read_to_string(&attr_path).unwrap_or_default();
    assert!(
        data.contains("*.md text"),
        "existing .gitattributes entry was lost:\n{data}"
    );
    assert!(
        data.contains(MERGE_ATTR_LINE),
        "merge line not appended:\n{data}"
    );
}

// ---- upstream `test_hooks.py` (#1902) ----

// Go's driver is `"<self>" merge-driver %O %A %B`, with the executable quoted (`%q`).
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_registers_merge_driver() {
    let repo = TempDir::new();
    init(&repo);
    let result = install_ok(repo.path());
    let driver = git_config_get(repo.path(), "merge.graphify.driver").expect("driver key is set");
    assert!(!driver.is_empty());
    assert!(driver.contains("merge-driver %O %A %B"), "{driver}");
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(
        attrs
            .lines()
            .any(|l| l.contains("graph.json") && l.contains("merge=graphify"))
    );
    // Go prints "registered graph.json union merge driver".
    assert!(result.contains("merge driver"), "{result}");
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_merge_driver_idempotent() {
    let repo = TempDir::new();
    init(&repo);
    install_ok(repo.path());
    install_ok(repo.path());
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert_eq!(
        attrs
            .lines()
            .filter(|l| l.contains("merge=graphify"))
            .count(),
        1
    );
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_install_preserves_existing_gitattributes() {
    let repo = TempDir::new();
    init(&repo);
    repo.write(".gitattributes", b"*.png binary\n");
    install_ok(repo.path());
    let content = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(content.contains("*.png binary"));
    assert!(content.contains("merge=graphify"));
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_uninstall_removes_merge_driver_keeps_other_attrs() {
    let repo = TempDir::new();
    init(&repo);
    repo.write(".gitattributes", b"*.png binary\n");
    install_ok(repo.path());
    uninstall_ok(repo.path());
    assert!(git_config_get(repo.path(), "merge.graphify.driver").is_none());
    let content = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(content.contains("*.png binary"));
    assert!(!content.contains("merge=graphify"));
}
