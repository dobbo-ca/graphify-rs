//! graphify-go `internal/extract/rake_skill_test.go`: `TestExtractRakeUsesRubyExtractor`.
//! `TestExtractSkillUsesMarkdown` tests the `.skill` arm of the `extract.go` dispatch and the
//! markdown extractor, not Ruby: moved to extract-nogrammar (`graphify-rs-ihc.17`).

use std::collections::BTreeSet;

use crate::extract::{Result, file, kinds};
use crate::testutil::TempDir;

/// Go `labelSet`.
fn label_set(r: &Result) -> BTreeSet<&str> {
    r.nodes.iter().map(|n| n.label.as_str()).collect()
}

// A .rake file is plain Ruby and must route through the Ruby extractor, emitting
// the same symbol nodes as the identical content in a .rb file (#1784).
#[test]
#[ignore = "graphify-rs-ihc.33"]
fn test_extract_rake_uses_ruby_extractor() {
    kinds::register_kinds();
    let src = "def run\n  greet\nend\n\ndef greet\nend\n";
    let root = TempDir::new();
    root.write("build.rake", src.as_bytes());
    root.write("build.rb", src.as_bytes());

    let rake = file(root.path(), "build.rake").unwrap_or_else(|e| panic!("File(build.rake): {e}"));
    let rb = file(root.path(), "build.rb").unwrap_or_else(|e| panic!("File(build.rb): {e}"));

    let rake_labels = label_set(&rake);
    assert!(
        rake_labels.contains("run()") && rake_labels.contains("greet()"),
        ".rake did not extract ruby defs, got labels {rake_labels:?}"
    );
    // Every symbol the .rb equivalent produces (bar the differently-named file
    // node) must also come out of the .rake — proving identical routing.
    for l in label_set(&rb) {
        if l == "build.rb" {
            continue;
        }
        assert!(
            rake_labels.contains(l),
            ".rake missing symbol {l:?} that the .rb equivalent produced"
        );
    }
}
