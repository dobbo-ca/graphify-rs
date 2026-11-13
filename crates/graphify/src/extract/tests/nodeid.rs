//! graphify-go `internal/extract/nodeid_test.go`. Go's `resolveSrc` is `util::resolve_src`
//! (it is shared with the references, preproc and jsts tests).

use std::collections::BTreeSet;

use crate::extract::tests::util::resolve_src;

/// Go indexes the maps with `[N]string` keys; `want` is one.
fn has<const N: usize>(set: &BTreeSet<[String; N]>, want: [&str; N]) -> bool {
    set.contains(&want.map(String::from))
}

/// Files that share a directory name and a stem keep their own symbols.
#[test]
#[ignore = "graphify-rs-ihc.33"]
fn test_node_ids_do_not_collide_across_files() {
    let py = "class ApiClient:\n    def get(self):\n        pass\n\ndef helper():\n    pass\n\ndef only_py():\n    helper()\n";
    let rb = "class ApiClient\n  def get\n  end\nend\n\ndef helper\nend\n";
    let (nodes, edges) = resolve_src(&[
        ("pkg/sample.py", py),
        ("pkg/sample.rb", rb),
        (
            "a/x/foo.go",
            "package x\n\nfunc Helper() {}\n\nfunc OnlyA() { Helper() }\n",
        ),
        (
            "b/x/foo.go",
            "package x\n\nfunc Helper() {}\n\nfunc OnlyB() { Helper() }\n",
        ),
    ]);
    for want in [
        ["pkg/sample.py", "ApiClient"],
        ["pkg/sample.py", "ApiClient.get()"],
        ["pkg/sample.py", "helper()"],
        ["pkg/sample.rb", "ApiClient"],
        ["pkg/sample.rb", "ApiClient.get()"],
        ["pkg/sample.rb", "helper()"],
        ["a/x/foo.go", "Helper()"],
        ["b/x/foo.go", "Helper()"],
    ] {
        assert!(has(&nodes, want), "missing node {want:?}");
    }
    for want in [
        ["pkg/sample.py:only_py()", "calls", "pkg/sample.py:helper()"],
        ["a/x/foo.go:OnlyA()", "calls", "a/x/foo.go:Helper()"],
        ["b/x/foo.go:OnlyB()", "calls", "b/x/foo.go:Helper()"],
        [
            "pkg/sample.py:sample.py",
            "contains",
            "pkg/sample.py:helper()",
        ],
    ] {
        assert!(has(&edges, want), "missing edge {want:?}");
    }
    for e in &edges {
        assert!(
            !(e[1] == "contains"
                && e[0].starts_with("pkg/sample.py")
                && e[2].starts_with("pkg/sample.rb")),
            "contains edge crosses files: {e:?}"
        );
    }
}

/// A name that is all punctuation must not take the id of its file.
#[test]
#[ignore = "graphify-rs-ihc.34"]
fn test_go_blank_func_keeps_its_node() {
    let (nodes, edges) = resolve_src(&[
        (
            "pkg/a.go",
            "package pkg\n\nfunc _() { Helper() }\n\nfunc Helper() {}\n",
        ),
        ("py/i18n.py", "def _(s):\n    return s\n"),
        ("c/gt.c", "static int _(int s) { return s; }\n"),
        ("rb/x.rb", "def _(s)\nend\n"),
        ("php/t.php", "<?php\nfunction __($s) { return $s; }\n"),
    ]);
    for want in [
        ["pkg/a.go", "_()"],
        ["py/i18n.py", "_()"],
        ["c/gt.c", "_()"],
        ["rb/x.rb", "_()"],
        ["php/t.php", "__()"],
    ] {
        assert!(has(&nodes, want), "missing {want:?}: {nodes:?}");
    }
    assert!(
        has(&edges, ["pkg/a.go:_()", "calls", "pkg/a.go:Helper()"])
            && !has(&edges, ["pkg/a.go:a.go", "contains", "pkg/a.go:a.go"]),
        "edges: {edges:?}"
    );
}

/// A call on an external module keeps its module edge when a definition of another language has
/// the same qualified name.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_external_module_call_survives_cross_language_name() {
    let (_, edges) = resolve_src(&[
        (
            "py/app.py",
            "import cv2 as cv\n\ndef f(a):\n    return cv.fitEllipse(a)\n",
        ),
        ("src/shape.cpp", "void cv::fitEllipse(int a) {}\n"),
    ]);
    let found = edges
        .iter()
        .any(|e| e[0] == "py/app.py:f()" && e[1] == "calls");
    assert!(found, "f() lost its call to cv2: {edges:?}");
}

/// The owner of a method is a type of the same language, its own file first.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_sibling_owner_rules() {
    let (_, edges) = resolve_src(&[
        ("p.h", "class Msg;\nclass Msg { public: void run(); };\n"),
        ("p.cc", "void Msg::run() {}\n"),
        (
            "a/widget.go",
            "package a\n\ntype Widget struct{}\n\nfunc (w Widget) Run() {}\n",
        ),
        ("a/widget.py", "class Widget:\n    pass\n"),
    ]);
    let want = ["p.h:Msg", "contains", "p.cc:Msg.run()"];
    assert!(has(&edges, want), "missing edge {want:?}\n{edges:?}");
    assert!(
        !has(
            &edges,
            ["a/widget.go:Widget", "contains", "a/widget.go:Widget.Run()"]
        ),
        "Go method got a type contains edge"
    );
    assert!(
        !has(
            &edges,
            ["a/widget.py:Widget", "contains", "a/widget.go:Widget.Run()"]
        ),
        "Go method owned by a Python class"
    );
}

/// An out-of-line C++ method stays under its class in the sibling header.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_cpp_out_of_line_method_owner_in_header() {
    let (nodes, edges) = resolve_src(&[
        (
            "lib/foo.h",
            "class Foo {\npublic:\n  void bar();\n  void baz() {}\n};\n",
        ),
        (
            "lib/foo.cpp",
            "#include \"foo.h\"\nvoid Foo::bar() { baz(); }\n",
        ),
    ]);
    assert!(
        has(&nodes, ["lib/foo.cpp", "Foo.bar()"]),
        "missing Foo.bar() in foo.cpp: {nodes:?}"
    );
    for want in [
        ["lib/foo.h:Foo", "contains", "lib/foo.cpp:Foo.bar()"],
        ["lib/foo.cpp:foo.cpp", "contains", "lib/foo.cpp:Foo.bar()"],
        ["lib/foo.cpp:Foo.bar()", "calls", "lib/foo.h:Foo.baz()"],
    ] {
        assert!(has(&edges, want), "missing edge {want:?}\n{edges:?}");
    }
}
