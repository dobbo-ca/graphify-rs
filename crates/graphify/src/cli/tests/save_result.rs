//! Port of `save_result_test.go` (3 tests). `maintenance_test.go` and `save_result_test.go` name
//! "golang" in the test map only because of `.go` file names and `query.Load` in strings: neither
//! parses Go source, so these need no grammar. `t.Chdir` becomes `cmd_save_result_in(cwd, ..)`.

use super::{MEMORY_DIR_PATH, cmd_save_result_in, save_result};
use crate::cli::maintenance::maint_util::{args, must, names};
use crate::testutil::TempDir;

// Covers the frontmatter, body sections and the unique filename of a saved Q&A result.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_save_result_writes_memory_doc() {
    let base = TempDir::new();
    let dir = base.join("memory");
    let q = r#"Where is "Load" defined?"#;
    let path = must(
        "save_result",
        save_result(
            &dir,
            q,
            "In internal/query.",
            "query",
            "useful",
            "it was query.Load",
            &["Load()", "query.go"],
        ),
    );
    let got = std::fs::read_to_string(&path).unwrap();
    for want in [
        r#"question: "Where is \"Load\" defined?""#,
        r#"type: "query""#,
        r#"contributor: "graphify""#,
        r#"outcome: "useful""#,
        r#"correction: "it was query.Load""#,
        r#"source_nodes: ["Load()", "query.go"]"#,
        "## Answer\n\nIn internal/query.",
        "- Signal: useful",
        "- Correction: it was query.Load",
        "## Source Nodes\n\n- Load()\n- query.go",
    ] {
        assert!(
            got.contains(want),
            "saved doc missing {want:?}; got:\n{got}"
        );
    }
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("query_") && name.ends_with("_where_is_load_defined.md"),
        "filename = {name:?}, want query_<ts>_<hex>_<slug>.md"
    );

    // A second save of the same question in the same second is its own file.
    let other = must(
        "save_result",
        save_result(&dir, q, "again", "query", "", "", &[]),
    );
    assert_ne!(
        other,
        path,
        "concurrent saves collided on {}",
        path.display()
    );
}

// Keeps a minimal save minimal.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_save_result_omits_optional_sections() {
    let dir = TempDir::new();
    let path = must(
        "save_result",
        save_result(dir.path(), "q?", "a", "query", "", "", &[]),
    );
    let body = std::fs::read_to_string(path).unwrap_or_default();
    for unwanted in [
        "outcome:",
        "correction:",
        "source_nodes:",
        "## Outcome",
        "## Source Nodes",
    ] {
        assert!(
            !body.contains(unwanted),
            "minimal save contains {unwanted:?}:\n{body}"
        );
    }
}

// Covers argument parsing, including the variadic --nodes list and the required/validated flags.
#[test]
#[ignore = "graphify-rs-ihc.21"]
fn test_cmd_save_result_flags() {
    let cwd = TempDir::new();
    let mut out = Vec::<u8>::new();

    must(
        "cmd_save_result",
        cmd_save_result_in(
            cwd.path(),
            &args(&[
                "--question",
                "How?",
                "--answer=Like this",
                "--nodes",
                "a()",
                "b()",
                "--outcome",
                "dead_end",
            ]),
            &mut out,
        ),
    );
    let mem = cwd.join(MEMORY_DIR_PATH);
    let entries = names(&mem).unwrap_or_default();
    assert_eq!(
        entries.len(),
        1,
        "ReadDir({MEMORY_DIR_PATH}) = {entries:?}; want 1 file"
    );
    let body = std::fs::read_to_string(mem.join(&entries[0])).unwrap_or_default();
    assert!(
        body.contains(r#"source_nodes: ["a()", "b()"]"#) && body.contains("Like this"),
        "unexpected doc:\n{body}"
    );

    for a in [
        &["--answer", "a"][..],
        &["--question", "q"],
        &["--question", "q", "--answer", "a", "--outcome", "maybe"],
        &["--question", "q", "--answer", "a", "--bogus"],
        &["--question"],
    ] {
        let r = cmd_save_result_in(cwd.path(), &args(a), &mut out);
        assert!(r.is_err(), "cmd_save_result({a:?}) = Ok, want error");
    }
}
