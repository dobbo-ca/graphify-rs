//! Upstream graphify v0.9.107 `tests/test_markdown_code_spans.py`: 36 of 46 tests, asserting
//! graphify-go v0.12.0 (sources inline, no fixture).
//!
//! Go's model of a code span differs from upstream's in three ways that every case reflects:
//! - The source of a `references` edge is the document's concept node (`make_id` of the path
//!   without `.md`, `markdown.go` MDRef `FromID`); upstream draws it from the heading that holds
//!   the span. A case that asserts a heading's edges asserts the document's.
//! - Go binds a span that is an identifier or a `.`/`::` chain of them
//!   (`resolve.go:1057` `mdCodeSymbol`) to the one code definition of that name whose
//!   qualifiers match, with no further evidence, as EXTRACTED. Upstream asks for evidence for a
//!   bare prose-like name (B08), reads a trailing `()`, `App\Ns\Type` and `path::Sym` spans
//!   (B48), and links source files and vault-wide wikilinks (B47).
//! - A link resolves to any corpus file (`resolve.go:1373`), and its edge targets
//!   `make_id(path without extension)`, which for a source file is not its file node
//!   (`make_id(path)`, B47).
//!
//! Each case that asserts another value carries `pins Go v0.12.0 (...)` with its bead.
//!
//! Not ported (10): `test_code_span_mentions_are_classified` (`_code_span_mention`),
//! `test_cited_file_matching` (`_match_cited_file`),
//! `test_extract_markdown_reports_mentions_as_markdown_raw_calls` (the `raw_calls` channel and
//! its heading owners and path qualifiers), `test_owner_evidence_survives_an_edge_that_already_
//! exists` and `test_target_with_no_source_file_is_safely_skipped`
//! (`resolve_markdown_mentions` on hand-made dicts), `test_cited_file_fallback_respects_explicit_
//! relative_discipline` and `test_escaping_citation_does_not_match_in_corpus_file`
//! (`_match_cited_file_any_form`): Python internals; `test_mentions_survive_a_rebuild_over_an_
//! existing_graph`, `test_file_citation_evidence_survives_a_rebuild` (`watch._rebuild_code`) and
//! `test_mentions_survive_the_extraction_cache` (Python's cache): the Python pipeline.

use std::collections::BTreeSet;

use super::extract_markdown;
use crate::extract::{Result, file_from_bytes, resolve};
use crate::idutil::make_id;
use crate::model::{Confidence, Extraction, Relation};

const WIDGET_PY: &str = "class Widget:
    def render(self):
        return \"w\"


def helper():
    return Widget()
";

const GADGET_PY: &str = "class Gadget:
    def render(self):
        return \"g\"


def helper():
    return Gadget()
";

const GUIDE_MD: &str = "# Guide

Start with `Widget`; `str` and `print()` are built-ins.

## Rendering

`Widget` draws through `Widget.render()`; `helper()` is defined twice.

```python
print(Widget)
```

## Pinned

See `src/widget.py::Widget::render` and `src/gadget.py::helper`.
Also `../src/widget.py::Widget` and the missing `src/nowhere.py::Widget`.
";

/// Upstream `_extract`: every file extracted and resolved together.
struct Run {
    results: Vec<Result>,
    ext: Extraction,
}

fn run(files: &[(&str, &str)]) -> Run {
    let results: Vec<Result> = files
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let names: Vec<String> = files.iter().map(|(rel, _)| rel.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &names, workers);
    Run { results, ext }
}

impl Run {
    /// The id of the definition `name` in `file` (upstream `_node(r, label, file)`).
    fn def(&self, file: &str, name: &str) -> String {
        self.results
            .iter()
            .flat_map(|r| &r.defs)
            .find(|d| d.file == file && d.name == name)
            .map(|d| d.id.clone())
            .unwrap_or_else(|| panic!("no definition {name} in {file}"))
    }

    /// The targets of the `references` edges from `src`.
    fn refs(&self, src: &str) -> BTreeSet<String> {
        self.ext
            .edges
            .iter()
            .filter(|e| e.relation == Relation::REFERENCES && e.source == src)
            .map(|e| e.target.clone())
            .collect()
    }
}

/// The concept node id of the markdown file `rel`.
fn doc(rel: &str) -> String {
    make_id(&[rel.strip_suffix(".md").unwrap_or(rel)])
}

fn set<S: AsRef<str>>(ids: &[S]) -> BTreeSet<String> {
    ids.iter().map(|s| s.as_ref().to_string()).collect()
}

// pins Go v0.12.0 (the span is an MDRef of the document's concept node); upstream 0.9.107 owns
// it by the heading -> P3
#[test]
fn test_extract_markdown_heading_code_span_belongs_to_that_heading() {
    let r = extract_markdown("api.md", b"# API\n\n## `Widget`\n\nText.\n");
    assert!(
        r.nodes.iter().any(|n| n.label == "`Widget`"),
        "{:?}",
        r.nodes
    );
    let refs: Vec<(&str, &str)> = r
        .md_refs
        .iter()
        .map(|m| (m.from_id.as_str(), m.target.as_str()))
        .collect();
    assert_eq!(refs, [("api", "Widget")]);
}

// pins Go v0.12.0 (only the bare `Widget` binds, from the document, EXTRACTED: `Widget.render()`
// and `helper()` end in `()`, and `path::Sym` spans are no identifier chain); upstream 0.9.107
// binds them per heading, INFERRED for bare names -> P3 (B48)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_mentions_resolve_to_references_edges_end_to_end() {
    let r = run(&[
        ("src/widget.py", WIDGET_PY),
        ("src/gadget.py", GADGET_PY),
        ("docs/guide.md", GUIDE_MD),
    ]);
    let guide = doc("docs/guide.md");
    let widget = r.def("src/widget.py", "Widget");
    assert_eq!(r.refs(&guide), set(&[&widget]));
    let e = r
        .ext
        .edges
        .iter()
        .find(|e| e.relation == Relation::REFERENCES && e.source == guide)
        .unwrap();
    assert_eq!(e.confidence, Confidence::EXTRACTED);
    // No mention becomes a call.
    assert!(
        !r.ext
            .edges
            .iter()
            .any(|e| e.relation == Relation::CALLS && e.source == guide)
    );
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_ambiguous_bare_name_yields_no_edge() {
    let r = run(&[
        ("a/thing.py", "class Thing:\n    pass\n"),
        ("b/thing.py", "class Thing:\n    pass\n"),
        ("notes.md", "# Notes\n\nUse `Thing`.\n"),
    ]);
    assert!(r.refs(&doc("notes.md")).is_empty());
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_dotted_mentions_need_qualifier_evidence() {
    let widget_py = format!("{WIDGET_PY}\n\ndef sleep():\n    pass\n");
    let gadget_py = format!("{GADGET_PY}\n\ndef toml():\n    pass\n");
    let r = run(&[
        ("src/widget.py", widget_py.as_str()),
        ("src/gadget.py", gadget_py.as_str()),
        (
            "docs/notes.md",
            concat!(
                "# Notes\n\n",
                "`time.sleep` and `pyproject.toml` are not this repo's sleep or toml.\n",
                "`widget.Widget`, `src.gadget.Gadget` and `Widget.render` are.\n",
                "`Gadget.helper` is not: `helper` is not owned by `Gadget`.\n"
            ),
        ),
    ]);
    let widget = r.def("src/widget.py", "Widget");
    let gadget = r.def("src/gadget.py", "Gadget");
    let widget_render = r.def("src/widget.py", "render");
    assert_eq!(
        r.refs(&doc("docs/notes.md")),
        set(&[&widget, &gadget, &widget_render])
    );
}

// pins Go v0.12.0 (no edge: `App\Services\Mailer` and `Mailer::send()` are no identifier chain);
// upstream 0.9.107 binds both -> P3 (B48)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_namespace_and_member_chains_resolve() {
    let r = run(&[
        (
            "mailer.py",
            "class Mailer:\n    def send(self):\n        return 1\n",
        ),
        (
            "gadget.py",
            "class Gadget:\n    def send(self):\n        return 2\n",
        ),
        (
            "README.md",
            concat!(
                "# Mail\n\n",
                "See `App\\Services\\Mailer` and `Mailer::send()`.\n\n",
                "## Constants\n\n",
                "`Foo::CONST` and `Foo::class` land on nothing.\n"
            ),
        ),
    ]);
    assert!(r.refs(&doc("README.md")).is_empty());
}

// pins Go v0.12.0 (README --references--> Cache.redis: a unique name binds with no evidence);
// upstream 0.9.107 emits no edge -> P3 (B08)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_lowercase_word_does_not_link_to_an_unrelated_method() {
    let r = run(&[
        (
            "cache.py",
            "class Cache:\n    def redis(self):\n        return 'connection'\n",
        ),
        (
            "README.md",
            "# Queue\n\nJobs run on `redis` (the server, not our code).\n",
        ),
    ]);
    assert_eq!(
        r.refs(&doc("README.md")),
        set(&[&r.def("cache.py", "redis")])
    );
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_method_mention_survives_when_the_document_cites_its_class() {
    let r = run(&[
        (
            "handler.py",
            "class HandlerResult:\n    def successWithWarnings(self):\n        return 1\n",
        ),
        (
            "notes.md",
            "# Notes\n\n`HandlerResult` returns `successWithWarnings`.\n",
        ),
    ]);
    let method = r.def("handler.py", "successWithWarnings");
    assert!(r.refs(&doc("notes.md")).contains(&method));
}

// pins Go v0.12.0 (no edge: a span ending in `()` is no identifier); upstream 0.9.107 binds it
// -> P3 (B48)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_method_with_parentheses_still_resolves() {
    let r = run(&[
        (
            "cache.py",
            "class Cache:\n    def redis(self):\n        return 'connection'\n",
        ),
        ("notes.md", "# Notes\n\nCall `redis()` on the cache.\n"),
    ]);
    assert!(r.refs(&doc("notes.md")).is_empty());
}

// pins Go v0.12.0 (docs/feed --references--> token(): a unique name binds with no evidence);
// upstream 0.9.107 emits no edge -> P3 (B08)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_bare_lowercase_word_does_not_link_to_an_unrelated_function() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) {\n  return name;\n}\n",
        ),
        (
            "docs/feed.md",
            "# Quote feed\n\nThe client sends the API key as the `token` query parameter.\n",
        ),
    ]);
    assert_eq!(
        r.refs(&doc("docs/feed.md")),
        set(&[&r.def("web/charts.js", "token")])
    );
}

// pins Go v0.12.0 (only token: `palette()` ends in `()`); upstream 0.9.107 binds both -> P3 (B48)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_bare_function_mention_survives_when_the_document_cites_another_symbol_from_its_file() {
    let r = run(&[
        (
            "web/charts.js",
            concat!(
                "export function palette() { return 'dark'; }\n",
                "export function token(name) { return name; }\n"
            ),
        ),
        (
            "docs/feed.md",
            "# Quote feed\n\nWalks through charts, mentions `palette()` and then `token`.\n",
        ),
    ]);
    assert_eq!(
        r.refs(&doc("docs/feed.md")),
        set(&[&r.def("web/charts.js", "token")])
    );
}

// pins Go v0.12.0 (no edge: `deploy()` ends in `()`); upstream 0.9.107 binds it -> P3 (B48)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_function_with_parentheses_still_resolves() {
    let r = run(&[
        ("deploy.py", "def deploy():\n    pass\n"),
        ("notes.md", "# Notes\n\nRun `deploy()` after the tests.\n"),
    ]);
    assert!(r.refs(&doc("notes.md")).is_empty());
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_function_survives_when_a_class_in_its_file_is_cited() {
    let r = run(&[
        (
            "deploy.py",
            "class Deployer:\n    pass\n\ndef deploy():\n    pass\n",
        ),
        ("notes.md", "# Notes\n\nUse `Deployer` then run `deploy`.\n"),
    ]);
    assert!(
        r.refs(&doc("notes.md"))
            .contains(&r.def("deploy.py", "deploy"))
    );
}

// pins Go v0.12.0 (b --references--> token(): a unique name binds with no evidence); upstream
// 0.9.107 emits no edge -> P3 (B08)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_function_evidence_cited_in_another_document_does_not_count() {
    let r = run(&[
        (
            "web/charts.js",
            concat!(
                "export function palette() { return 'dark'; }\n",
                "export function token(name) { return name; }\n"
            ),
        ),
        ("a.md", "# A\n\nUses `palette()`.\n"),
        ("b.md", "# B\n\nSends `token`.\n"),
    ]);
    assert_eq!(
        r.refs(&doc("b.md")),
        set(&[&r.def("web/charts.js", "token")])
    );
}

// pins Go v0.12.0 (b --references--> successWithWarnings: a unique name binds with no evidence);
// upstream 0.9.107 emits no edge -> P3 (B08)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_owner_cited_in_another_document_does_not_count() {
    let r = run(&[
        (
            "handler.py",
            "class HandlerResult:\n    def successWithWarnings(self):\n        return 1\n",
        ),
        ("a.md", "# A\n\n`HandlerResult` is the result type.\n"),
        ("b.md", "# B\n\nReads `successWithWarnings`.\n"),
    ]);
    assert_eq!(
        r.refs(&doc("b.md")),
        set(&[&r.def("handler.py", "successWithWarnings")])
    );
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_dotted_mention_resolves_when_method_name_is_a_builtin() {
    let r = run(&[
        (
            "src/repo.py",
            concat!(
                "class Repository:\n",
                "    def open(self):\n",
                "        return 1\n",
                "    def connect(self):\n",
                "        return 2\n"
            ),
        ),
        (
            "docs/notes.md",
            "# Notes\n\nSee `Repository.open` and `Repository.connect`.\n",
        ),
    ]);
    let cited = r.refs(&doc("docs/notes.md"));
    assert!(cited.contains(&r.def("src/repo.py", "open")));
    assert!(cited.contains(&r.def("src/repo.py", "connect")));
}

// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_bare_function_survives_when_file_is_cited() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/span.md",
            "# Span\n\n- `web/charts.js` \u{2014} uses `token` to read styles.\n",
        ),
        (
            "docs/link.md",
            "# Link\n\nSee [charts](../web/charts.js). Uses `token`.\n",
        ),
    ]);
    let token = r.def("web/charts.js", "token");
    assert!(r.refs(&doc("docs/span.md")).contains(&token));
    assert!(r.refs(&doc("docs/link.md")).contains(&token));
}

// pins Go v0.12.0 (both bind: a unique name binds with no evidence); upstream 0.9.107 emits no
// edge -> P3 (B08)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_two_bare_functions_from_one_file_with_no_other_citation_yield_no_edge() {
    let r = run(&[
        (
            "web/charts.js",
            concat!(
                "export function palette() { return 'dark'; }\n",
                "export function token(name) { return name; }\n"
            ),
        ),
        ("notes.md", "# Notes\n\nCalls `palette` then `token`.\n"),
    ]);
    assert_eq!(
        r.refs(&doc("notes.md")),
        set(&[
            &r.def("web/charts.js", "palette"),
            &r.def("web/charts.js", "token")
        ])
    );
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_function_resolves_when_class_is_cited_after_it_in_document() {
    let r = run(&[
        (
            "deploy.py",
            "class Deployer:\n    pass\n\ndef deploy():\n    pass\n",
        ),
        (
            "notes.md",
            "# Notes\n\nRun `deploy` first. Then use `Deployer`.\n",
        ),
    ]);
    let refs = r.refs(&doc("notes.md"));
    assert!(refs.contains(&r.def("deploy.py", "deploy")));
    assert!(refs.contains(&r.def("deploy.py", "Deployer")));
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_function_survives_with_path_qualified_evidence() {
    let r = run(&[
        (
            "deploy.py",
            "class Deployer:\n    pass\n\ndef deploy():\n    pass\n",
        ),
        (
            "notes.md",
            "# Notes\n\nSee `deploy.py::Deployer`. Then run `deploy`.\n",
        ),
    ]);
    assert!(
        r.refs(&doc("notes.md"))
            .contains(&r.def("deploy.py", "deploy"))
    );
}

// pins Go v0.12.0 (no edge: Go's Python extractor reads no class nested in a class, so `Inner`
// is no definition); upstream 0.9.107 binds it -> P3
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_nested_class_resolves_standalone() {
    let r = run(&[
        ("tree.py", "class Outer:\n    class Inner:\n        pass\n"),
        ("notes.md", "# Notes\n\nUse `Inner`.\n"),
    ]);
    assert!(r.refs(&doc("notes.md")).is_empty());
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_function_with_snake_case_or_camel_case_resolves_without_file_citation() {
    let r = run(&[
        (
            "build.py",
            concat!(
                "def deduplicate_by_label():\n    pass\n\n",
                "def buildFromJson():\n    pass\n"
            ),
        ),
        (
            "notes.md",
            "# Notes\n\nCalls `deduplicate_by_label` and `buildFromJson`.\n",
        ),
    ]);
    let refs = r.refs(&doc("notes.md"));
    assert!(refs.contains(&r.def("build.py", "deduplicate_by_label")));
    assert!(refs.contains(&r.def("build.py", "buildFromJson")));
}

// The edge is the document's (Go has one source per document for both headings).
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_bare_function_mention_survives_when_symbol_itself_cited_with_parentheses() {
    let r = run(&[
        (
            "web/charts.js",
            "export function palette() { return 'dark'; }\n",
        ),
        (
            "docs/guide.md",
            concat!(
                "# Overview\n\n",
                "Call `palette()` to configure theme.\n\n",
                "## Details\n\n",
                "Reads `palette` for current colors.\n"
            ),
        ),
    ]);
    assert!(
        r.refs(&doc("docs/guide.md"))
            .contains(&r.def("web/charts.js", "palette"))
    );
}

// pins Go v0.12.0 (the link edge targets make_id("web/charts"), not the file node
// make_id("web/charts.js")); upstream 0.9.107 targets the file node -> P3 (B47)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_link_to_scanned_source_file_is_a_reference_and_licenses_bare_mentions() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/guide.md",
            "# Guide\n\nSee [charts](../web/charts.js). Sends `token`.\n",
        ),
    ]);
    let guide = doc("docs/guide.md");
    let refs = r.refs(&guide);
    assert!(refs.contains(&r.def("web/charts.js", "token")));
    let link = make_id(&["web/charts"]);
    let e = r
        .ext
        .edges
        .iter()
        .find(|e| e.relation == Relation::REFERENCES && e.source == guide && e.target == link)
        .unwrap_or_else(|| panic!("no link edge; refs {refs:?}"));
    assert_eq!(e.confidence, Confidence::EXTRACTED);
    assert!(!refs.contains(&make_id(&["web/charts.js"])));
}

// pins Go v0.12.0 (the link and the span are MDRefs of the document, targets as written);
// upstream 0.9.107 records the link as a `file_link` citation raw call -> P3 (B47)
#[test]
fn test_extract_markdown_records_a_source_file_link_as_a_citation() {
    let r = extract_markdown(
        "docs/guide.md",
        b"# Guide\n\nSee [charts](../web/charts.js) and `token`.\n",
    );
    assert!(r.edges.iter().all(|e| e.relation == Relation::CONTAINS));
    let refs: Vec<(&str, &str)> = r
        .md_refs
        .iter()
        .map(|m| (m.from_id.as_str(), m.target.as_str()))
        .collect();
    assert_eq!(
        refs,
        [("docs_guide", "../web/charts.js"), ("docs_guide", "token")]
    );
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_link_to_unscanned_source_file_yields_no_edge_or_phantom() {
    // ../outside/script.py exists on disk upstream but is not in the corpus.
    let r = run(&[(
        "docs/guide.md",
        "# Guide\n\nSee [script](../outside/script.py).\n",
    )]);
    assert!(
        r.ext.edges.iter().all(|e| e.relation == Relation::CONTAINS),
        "{:?}",
        r.ext.edges
    );
}

// pins Go v0.12.0 (no edge: `[[setup.py]]` resolves against docs/, and `setup.py` names no
// definition `py`); upstream 0.9.107 links the page to the file node -> P3 (B47)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_wikilink_to_scanned_source_file_yields_only_a_file_edge() {
    let r = run(&[
        ("setup.py", "def setup():\n    pass\n"),
        ("docs/guide.md", "# Guide\n\nSee [[setup.py]].\n"),
    ]);
    assert!(r.refs(&doc("docs/guide.md")).is_empty());
}

// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_single_word_pascal_case_function_resolves_standalone() {
    let r = run(&[
        ("worker.go", "package worker\n\nfunc Serve() {}\n"),
        ("docs/guide.md", "# Guide\n\n`Serve` starts the server.\n"),
    ]);
    assert!(
        r.refs(&doc("docs/guide.md"))
            .contains(&r.def("worker.go", "Serve"))
    );
}

// pins Go v0.12.0 (guide --references--> serve(): a unique name binds with no evidence);
// upstream 0.9.107 emits no edge -> P3 (B08)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_single_word_lowercase_function_still_needs_evidence() {
    let r = run(&[
        ("worker.go", "package worker\n\nfunc serve() {}\n"),
        ("docs/guide.md", "# Guide\n\n`serve` starts the server.\n"),
    ]);
    assert_eq!(
        r.refs(&doc("docs/guide.md")),
        set(&[&r.def("worker.go", "serve")])
    );
}

// pins Go v0.12.0 (the document binds token with no evidence and not `palette()`, which ends in
// `()`); upstream 0.9.107 binds palette() from the first section only -> P3 (B08, B48)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_bare_function_mention_in_a_sibling_section_needs_its_own_evidence() {
    let charts_js = concat!(
        "export function palette() { return 'dark'; }\n",
        "export function token(name) { return name; }\n"
    );
    let r = run(&[
        ("web/charts.js", charts_js),
        (
            "docs/guide.md",
            concat!(
                "# Charts\n\n",
                "`palette()` configures the theme.\n\n",
                "# Token notes\n\n",
                "The API key travels as the `token` query parameter.\n"
            ),
        ),
    ]);
    assert_eq!(
        r.refs(&doc("docs/guide.md")),
        set(&[&r.def("web/charts.js", "token")])
    );

    // ...and a bare `palette` binds wherever it is.
    let r = run(&[
        ("web/charts.js", charts_js),
        (
            "docs/guide.md",
            concat!(
                "# Charts\n\n",
                "`palette()` configures the theme.\n\n",
                "## Colors\n\n",
                "Reads `palette` for the current scheme.\n"
            ),
        ),
    ]);
    assert!(
        r.refs(&doc("docs/guide.md"))
            .contains(&r.def("web/charts.js", "palette"))
    );
}

// pins Go v0.12.0 for the first corpus (notes --references--> Token(): a unique name binds with
// no evidence); upstream 0.9.107 emits no edge -> P3 (B08)
// nogrammar test (ihc.17): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_pascal_case_in_a_lowercase_convention_language_needs_evidence() {
    let r = run(&[
        ("build.py", "def Token():\n    pass\n"),
        ("notes.md", "# Notes\n\n`Token` builds the response.\n"),
    ]);
    assert_eq!(
        r.refs(&doc("notes.md")),
        set(&[&r.def("build.py", "Token")])
    );

    // With the defining file cited, the same mention resolves.
    let r = run(&[
        ("build.py", "def Token():\n    pass\n"),
        (
            "notes.md",
            "# Notes\n\nSee `build.py`. `Token` builds the response.\n",
        ),
    ]);
    assert!(
        r.refs(&doc("notes.md"))
            .contains(&r.def("build.py", "Token"))
    );
}

// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_file_span_is_evidence_but_not_a_doc_file_edge() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/span.md",
            "# Span\n\n- `web/charts.js` \u{2014} uses `token` to read styles.\n",
        ),
    ]);
    assert!(
        r.refs(&doc("docs/span.md"))
            .contains(&r.def("web/charts.js", "token"))
    );
    let refs = r
        .ext
        .edges
        .iter()
        .filter(|e| e.relation == Relation::REFERENCES)
        .count();
    assert_eq!(refs, 1, "a span is evidence, not a link: no doc->file edge");
}

// pins Go v0.12.0 (the document binds token with no evidence); upstream 0.9.107 emits no edge
// from the Authentication section -> P3 (B08)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_source_file_link_in_heading_does_not_license_sibling_heading() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/guide.md",
            concat!(
                "# Charts\n\n",
                "See [charts](../web/charts.js).\n\n",
                "# Authentication\n\n",
                "Sends `token` for auth.\n"
            ),
        ),
    ]);
    assert!(
        r.refs(&doc("docs/guide.md"))
            .contains(&r.def("web/charts.js", "token"))
    );
}

// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_missing_source_link_does_not_reference_other_files() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        ("docs/guide.md", "# Guide\n\nSee [charts](web/charts.js).\n"),
    ]);
    let refs = r.refs(&doc("docs/guide.md"));
    assert!(!refs.contains(&make_id(&["web/charts.js"])), "{refs:?}");
    assert!(!refs.contains(&make_id(&["web/charts"])), "{refs:?}");
}

// pins Go v0.12.0 (the definition's link edge targets make_id("web/charts"), not the file node
// make_id("web/charts.js")); upstream 0.9.107 targets the file node -> P3 (B47)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_reference_style_use_licenses_its_own_section() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/guide.md",
            concat!(
                "# Charts\n\n",
                "See [charts][source]. Uses `token`.\n\n",
                "# Authentication\n\n",
                "[source]: ../web/charts.js\n"
            ),
        ),
    ]);
    let refs = r.refs(&doc("docs/guide.md"));
    assert!(refs.contains(&r.def("web/charts.js", "token")));
    assert!(refs.contains(&make_id(&["web/charts"])), "{refs:?}");
}

// pins Go v0.12.0 (the document binds token with no evidence); upstream 0.9.107 emits no edge
// from the Authentication section -> P3 (B08)
// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_reference_definition_section_does_not_license_bare_mention() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/guide.md",
            concat!(
                "# Charts\n\n",
                "Uses `token`.\n\n",
                "# Authentication\n\n",
                "Sends `token` for auth.\n\n",
                "[source]: ../web/charts.js\n"
            ),
        ),
    ]);
    assert!(
        r.refs(&doc("docs/guide.md"))
            .contains(&r.def("web/charts.js", "token"))
    );
}

// nogrammar test (ihc.17): parses javascript (ihc.26); also needs resolve (ihc.18)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_reference_style_use_shared_by_multiple_sections() {
    let r = run(&[
        (
            "web/charts.js",
            "export function token(name) { return name; }\n",
        ),
        (
            "docs/guide.md",
            concat!(
                "# Charts\n\n",
                "See [charts][source]. Uses `token`.\n\n",
                "# Token notes\n\n",
                "Also [charts][source]. Sends `token`.\n\n",
                "[source]: ../web/charts.js\n"
            ),
        ),
    ]);
    assert!(
        r.refs(&doc("docs/guide.md"))
            .contains(&r.def("web/charts.js", "token"))
    );
}
