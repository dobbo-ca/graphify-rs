//! graphify-go `internal/extract/markdown_test.go`, and `TestExtractSkillUsesMarkdown` from
//! `rake_skill_test.go` (moved here from extract-ruby: it tests the `.skill` arm of the
//! `extract.go` dispatch and this extractor). `TestResolveMDTarget` tests `resolveMDTarget`
//! (`resolve.go:1356`), stubbed in `resolve.rs`.

use std::collections::{BTreeMap, BTreeSet};

use super::{extract_markdown, split_frontmatter};
use crate::extract::resolve::{Corpus, resolve_md_target};
use crate::extract::{Def, Result, file, resolve};
use crate::model::{Edge, Extraction, Node, Relation};
use crate::testutil::{TempDir, testdata};

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

fn files(fs: &[&str]) -> Vec<String> {
    fs.iter().map(|f| f.to_string()).collect()
}

/// A code definition as the Go tests write one (`Def{ID, Name, File}`).
fn def(id: &str, name: &str, file: &str) -> Def {
    Def {
        id: id.to_string(),
        name: name.to_string(),
        file: file.to_string(),
        ..Def::default()
    }
}

/// Whether `ext` has a `rel` edge from `src` to `tgt`.
fn has_edge(ext: &Extraction, src: &str, rel: Relation, tgt: &str) -> bool {
    ext.edges
        .iter()
        .any(|e| e.relation == rel && e.source == src && e.target == tgt)
}

/// The number of `references` edges from `src`.
fn refs_from(ext: &Extraction, src: &str) -> usize {
    ext.edges
        .iter()
        .filter(|e| e.relation == Relation::REFERENCES && e.source == src)
        .count()
}

// TestExtractMarkdown resolves a small OKF-style markdown bundle and checks that
// each file becomes one concept node carrying its frontmatter `type`, that
// markdown links become `references` edges (absolute /tables/... and relative),
// that http(s) links are ignored, and that a directory's index.md `contains` its
// siblings.
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_markdown() {
    let root = testdata().join("extract").join("mdproj");
    let fs = files(&[
        "index.md",
        "glossary.md",
        "tables/index.md",
        "tables/orders.md",
    ]);

    let results: Vec<Result> = fs
        .iter()
        .map(|f| file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let ext = resolve(&results, &fs, workers());

    // One concept node per file, with the frontmatter `type` as file_type.
    let want_nodes = [
        ("index", "bundle", "Analytics Knowledge Bundle"),
        ("glossary", "glossary", "Business Glossary"),
        ("tables_index", "index", "Tables"),
        ("tables_orders", "table", "orders"),
    ];
    let by_id: BTreeMap<&str, (&str, &str)> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), (n.file_type.as_str(), n.label.as_str())))
        .collect();
    let concepts = ext
        .nodes
        .iter()
        .filter(|n| n.file_type != "heading")
        .count();
    assert_eq!(
        concepts,
        want_nodes.len(),
        "concept node count = {concepts}, want {} (one concept per file)",
        want_nodes.len()
    );
    for (id, file_type, label) in want_nodes {
        let Some(&(got_type, got_label)) = by_id.get(id) else {
            panic!("missing concept node {id:?}");
        };
        assert_eq!(
            got_type, file_type,
            "node {id:?} file_type = {got_type:?}, want {file_type:?} (frontmatter type)"
        );
        assert_eq!(
            got_label, label,
            "node {id:?} label = {got_label:?}, want {label:?} (frontmatter title)"
        );
    }

    const TESTS: [(&str, &str, &str, &str, bool); 5] = [
        (
            "absolute link becomes references edge",
            "index",
            "references",
            "tables_orders",
            true,
        ),
        (
            "relative link becomes references edge",
            "index",
            "references",
            "glossary",
            true,
        ),
        (
            "cross-dir absolute link resolves",
            "tables_orders",
            "references",
            "glossary",
            true,
        ),
        (
            "index.md contains sibling",
            "tables_index",
            "contains",
            "tables_orders",
            true,
        ),
        (
            "external http link is ignored",
            "index",
            "references",
            "tree_sitter_github_io",
            false,
        ),
    ];
    for (name, src, rel, tgt, want_present) in TESTS {
        let got = ext
            .edges
            .iter()
            .any(|e| e.relation.as_str() == rel && e.source == src && e.target == tgt);
        assert_eq!(
            got, want_present,
            "{name}: edge {src} --{rel}--> {tgt} present={got}, want {want_present}"
        );
    }

    // description and tags ride along on ComputedName as searchable metadata.
    let orders = ext
        .nodes
        .iter()
        .rev()
        .find(|n| n.id == "tables_orders")
        .map_or("", |n| n.computed_name.as_str());
    assert!(
        !orders.is_empty(),
        "tables_orders ComputedName empty; expected description/tags metadata"
    );
}

// TestSplitFrontmatter covers frontmatter parsing edge cases: no frontmatter, a
// well-formed block, quoted scalars, and an unterminated block (whole source is
// body, no keys).
#[test]
fn test_split_frontmatter() {
    const TESTS: [(&str, &str, &str, &str); 4] = [
        ("no frontmatter", "# Title\n\nbody", "", "# Title\n\nbody"),
        (
            "typed block",
            "---\ntype: table\n---\n# T\nbody",
            "table",
            "# T\nbody",
        ),
        (
            "quoted scalar",
            "---\ntype: \"table\"\n---\nbody",
            "table",
            "body",
        ),
        (
            "unterminated",
            "---\ntype: table\nbody without close",
            "",
            "---\ntype: table\nbody without close",
        ),
    ];
    for (name, src, want_type, want_body) in TESTS {
        let (fm, body) = split_frontmatter(src);
        let got_type = fm.get("type").map_or("", String::as_str);
        assert_eq!(
            got_type, want_type,
            "{name}: type = {got_type:?}, want {want_type:?}"
        );
        assert_eq!(
            body, want_body,
            "{name}: body = {body:?}, want {want_body:?}"
        );
    }
}

// TestResolveMDTarget covers link-target resolution: absolute vs relative,
// anchors stripped, external schemes ignored, extensionless targets, and
// off-corpus targets.
// nogrammar test (ihc.17): needs resolve (resolveMDTarget is resolve.go:1356)
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_md_target() {
    let corpus: Corpus = ["index.md", "tables/orders.md", "a%20b.md", "c d.md"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    const TESTS: [(&str, &str, &str, &str); 10] = [
        (
            "absolute with ext",
            "index.md",
            "/tables/orders.md",
            "tables/orders.md",
        ),
        (
            "relative from subdir",
            "tables/orders.md",
            "../index.md",
            "index.md",
        ),
        (
            "anchor stripped",
            "index.md",
            "/tables/orders.md#cols",
            "tables/orders.md",
        ),
        (
            "extensionless absolute",
            "index.md",
            "/tables/orders",
            "tables/orders.md",
        ),
        ("http ignored", "index.md", "https://example.com", ""),
        ("mailto ignored", "index.md", "mailto:a@b.com", ""),
        ("in-page anchor only", "index.md", "#section", ""),
        ("off corpus", "index.md", "/missing.md", ""),
        ("literal percent name", "index.md", "a%20b", "a%20b.md"),
        ("escaped fallback", "index.md", "c%20d.md", "c d.md"),
    ];
    for (name, from, target, want) in TESTS {
        let got = resolve_md_target(from, target, &corpus);
        assert_eq!(
            got, want,
            "{name}: resolveMDTarget({from:?}, {target:?}) = {got:?}, want {want:?}"
        );
    }
}

// TestExtractMarkdownHeadingsRefs covers the additive depth: ATX headings become
// nodes with `contains` edges (nested by level), and `[[wikilinks]]`,
// reference-style `[a]: url` definitions and inline `code` spans become MDRefs
// alongside inline links. It then runs Resolve to confirm an in-corpus wikilink
// stitches into a `references` edge while an external reference definition and an
// unresolved backtick symbol do not.
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_markdown_headings_refs() {
    let doc = concat!(
        "# Top\n\n",
        "Intro links to [[other]] and mentions `Symbol` inline.\n\n",
        "[a]: http://x\n\n",
        "## Nested\n\n",
        "More detail about `Symbol`.\n",
    );

    let r = extract_markdown("notes.md", doc.as_bytes());

    // Heading nodes, typed "heading", one per ATX heading.
    let nodes: BTreeMap<&str, &Node> = r.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    for (id, label) in [("notes_top", "Top"), ("notes_nested", "Nested")] {
        let Some(n) = nodes.get(id) else {
            panic!("missing heading node {id:?}");
        };
        assert_eq!(
            n.file_type, "heading",
            "heading {id:?} file_type = {:?}, want heading",
            n.file_type
        );
        assert_eq!(
            n.label, label,
            "heading {id:?} label = {:?}, want {label:?}",
            n.label
        );
    }

    let has_contain = |src: &str, tgt: &str| {
        r.edges
            .iter()
            .any(|e| e.relation == Relation::CONTAINS && e.source == src && e.target == tgt)
    };
    assert!(
        has_contain("notes", "notes_top"),
        "missing contains edge notes --contains--> notes_top (file -> heading)"
    );
    assert!(
        has_contain("notes_top", "notes_nested"),
        "missing contains edge notes_top --contains--> notes_nested (heading nesting)"
    );

    // Wikilink, reference definition and backtick each captured as an MDRef.
    let ref_targets: BTreeSet<&str> = r.md_refs.iter().map(|m| m.target.as_str()).collect();
    for tgt in ["other", "http://x", "Symbol"] {
        assert!(ref_targets.contains(tgt), "missing MDRef target {tgt:?}");
    }

    // Resolve: the wikilink resolves to the sibling doc; the external ref def and
    // the unresolved backtick symbol produce no `references` edge.
    let other = extract_markdown("other.md", b"# Other\n");
    let ext = resolve(&[r, other], &files(&["notes.md", "other.md"]), workers());
    assert!(
        has_edge(&ext, "notes", Relation::REFERENCES, "other"),
        "wikilink [[other]] did not resolve to references edge notes --references--> other"
    );
    let refs = refs_from(&ext, "notes");
    assert_eq!(
        refs, 1,
        "references edges from notes = {refs}, want 1 (external ref def and backtick symbol must not resolve)"
    );
}

// TestResolveBacktickCodeSymbol covers backtick code-span resolution: a unique
// code definition named by an inline `code` span becomes a `references` edge from
// the doc concept to that code node; an ambiguous name (defined twice) and a
// noise token with no matching definition both drop.
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_backtick_code_symbol() {
    let doc = "# Doc\n\nUses `UniqueSymbolXyz`, `Dupe` and `true` inline.\n";
    let r = extract_markdown("notes.md", doc.as_bytes());

    let code = Result {
        defs: vec![
            def("UNIQUE", "UniqueSymbolXyz", "a.go"),
            def("DUPE1", "Dupe", "a.go"),
            def("DUPE2", "Dupe", "b.go"),
        ],
        ..Result::default()
    };

    let ext = resolve(&[r, code], &files(&["notes.md", "a.go", "b.go"]), workers());
    let has_ref = |src: &str, tgt: &str| has_edge(&ext, src, Relation::REFERENCES, tgt);

    // (a) unique backtick symbol -> references edge to the code def.
    assert!(
        has_ref("notes", "UNIQUE"),
        "expected notes --references--> UNIQUE (unique backtick code symbol)"
    );
    // (b) ambiguous symbol (defined twice) -> no edge.
    assert!(
        !has_ref("notes", "DUPE1") && !has_ref("notes", "DUPE2"),
        "did not expect a references edge for ambiguous backtick symbol `Dupe`"
    );
    // (c) exactly one references edge: `true` (noise, no def) also drops.
    let refs = refs_from(&ext, "notes");
    assert_eq!(
        refs, 1,
        "references edges from notes = {refs}, want 1 (only the unique code symbol resolves)"
    );
}

// TestResolveQualifiedCodeSymbol covers qualified backtick spans: a dotted or
// ::-separated mention resolves when every qualifier is a path segment of the
// definition's file or a label on its containment chain, and drops otherwise
// (so `time.sleep` never binds to a repo's own `sleep`).
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_qualified_code_symbol() {
    let doc = "# Doc\n\nSee `pkg.Widget`, `Widget::render`, `time.sleep` and `other.Widget`.\n";
    let r = extract_markdown("notes.md", doc.as_bytes());

    let code = Result {
        nodes: vec![Node {
            id: "WIDGET".to_string(),
            label: "Widget".to_string(),
            ..Node::default()
        }],
        edges: vec![Edge {
            source: "WIDGET".to_string(),
            target: "RENDER".to_string(),
            relation: Relation::CONTAINS,
            ..Edge::default()
        }],
        defs: vec![
            def("WIDGET", "Widget", "pkg/widget.go"),
            def("RENDER", "render", "pkg/widget.go"),
            def("SLEEP", "sleep", "util/helpers.go"),
        ],
        ..Result::default()
    };

    let ext = resolve(
        &[r, code],
        &files(&["notes.md", "pkg/widget.go", "util/helpers.go"]),
        workers(),
    );
    let has_ref = |tgt: &str| has_edge(&ext, "notes", Relation::REFERENCES, tgt);

    // pkg.Widget: "pkg" is a path segment of pkg/widget.go
    assert!(
        has_ref("WIDGET"),
        "expected notes --references--> WIDGET for `pkg.Widget`"
    );
    // Widget::render: "Widget" owns render
    assert!(
        has_ref("RENDER"),
        "expected notes --references--> RENDER for `Widget::render`"
    );
    // time.sleep: no "time" qualifier anywhere
    assert!(
        !has_ref("SLEEP"),
        "did not expect notes --references--> SLEEP for `time.sleep`"
    );
    // `other.Widget` must not bind either: the only references edges are the two above.
    let refs = refs_from(&ext, "notes");
    assert_eq!(refs, 2, "references edges from notes = {refs}, want 2");
}

// TestMarkdownSpacedAndEscapedLinks covers angle-bracket and percent-encoded
// link targets plus the escaped-pipe wikilink alias used inside tables.
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_markdown_spaced_and_escaped_links() {
    for doc in [
        "[a](<My Note.md>)\n",
        "[b](My%20Note.md)\n",
        "| [[My Note\\|alias]] |\n",
    ] {
        let r = extract_markdown("notes.md", doc.as_bytes());
        let md_refs = r.md_refs.clone();
        let note = extract_markdown("My Note.md", b"# N\n");
        let note_id = note.nodes[0].id.clone();
        let ext = resolve(&[r, note], &files(&["notes.md", "My Note.md"]), workers());
        assert!(
            has_edge(&ext, "notes", Relation::REFERENCES, &note_id),
            "{doc:?}: no references edge to My Note.md; refs={md_refs:?}"
        );
    }
}

// From rake_skill_test.go. A .skill file is Markdown (+YAML frontmatter) and must route through
// the markdown extractor, emitting heading nodes (#1901).
#[test]
fn test_extract_skill_uses_markdown() {
    let src = "---\nname: orch\n---\n# Orchestrator\n\n## Step One\n";
    let root = TempDir::new();
    root.write("orch.skill", src.as_bytes());
    let r = file(root.path(), "orch.skill").unwrap_or_else(|e| panic!("File(orch.skill): {e}"));
    let labels: BTreeSet<&str> = r.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in ["Orchestrator", "Step One"] {
        assert!(
            labels.contains(want),
            ".skill did not extract markdown heading {want:?}, got {labels:?}"
        );
    }
}
