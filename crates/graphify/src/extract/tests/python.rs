//! graphify-go `internal/extract/python_test.go`.

use crate::extract::tests::util::want_edges;
use crate::extract::{file, file_from_bytes, resolve};
use crate::idutil::make_id;
use crate::testutil::testdata;

#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_python() {
    crate::extract::kinds::register_kinds();
    let root = testdata().join("extract").join("pyproj");
    let files = ["util/math.py", "web/server.py"];

    let mut results = Vec::new();
    for f in files {
        results.push(file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")));
    }
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels: std::collections::HashSet<&str> =
        ext.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in [
        "math.py",
        "server.py",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }
    let id2label: std::collections::HashMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();

    let has = |src_label: &str, rel: &str, tgt_label: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src_label)
                && id2label.get(e.target.as_str()) == Some(&tgt_label)
        })
    };
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "boot()"),
        "expected Server.start --calls--> boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );
    // External import edge for os.
    assert!(
        ext.edges.iter().any(|e| e.relation.as_str() == "imports"),
        "no external import edges (expected os)"
    );
}

// A `# NOTE:` comment becomes a rationale node with an edge to the file, and a
// function docstring becomes a rationale node with an edge to that function.
#[test]
fn test_extract_python_rationale() {
    crate::extract::kinds::register_kinds();
    let src = concat!(
        "# NOTE: keep boot fast\n",
        "def boot():\n",
        "    \"\"\"Boot the service and return its status code for callers.\"\"\"\n",
        "    return 1\n"
    );
    let res = file_from_bytes("svc/app.py", src.as_bytes());

    let file_id = make_id(&["svc/app.py"]);
    let func_id = make_id(&["svc/app.py", "boot"]);

    let mut label = std::collections::HashMap::new();
    let mut ftype = std::collections::HashMap::new();
    for n in &res.nodes {
        label.insert(n.id.as_str(), n.label.as_str());
        ftype.insert(n.id.as_str(), n.file_type.as_str());
    }
    let has_rationale_for = |label_prefix: &str, target: &str| {
        res.edges.iter().any(|e| {
            e.relation.as_str() == "rationale_for"
                && e.target == target
                && ftype.get(e.source.as_str()) == Some(&"rationale")
                && label
                    .get(e.source.as_str())
                    .is_some_and(|l| l.starts_with(label_prefix))
        })
    };
    // NOTE comment -> rationale node, edge to the file.
    assert!(
        has_rationale_for("# NOTE: keep boot fast", &file_id),
        "expected rationale_for edge from NOTE comment to file {file_id}"
    );
    // Function docstring -> rationale node, edge to the function.
    assert!(
        has_rationale_for("Boot the service", &func_id),
        "expected rationale_for edge from docstring to function {func_id}"
    );
}

// A leading comment as a body child (which tree-sitter includes but upstream's
// AST omits) must not hide the docstring that follows it. A module-level comment
// is a direct sibling of the docstring statement, so module.Child(0) is the
// comment and module.Child(1) is the docstring: this exercises the comment-skip
// in pyDocstring, unlike a function body where the comment attaches to the
// function_definition node rather than the block.
#[test]
fn test_extract_python_docstring_after_comment() {
    crate::extract::kinds::register_kinds();
    let src = concat!(
        "# a leading comment\n",
        "\"\"\"Module docstring longer than twenty characters explaining architecture.\"\"\"\n"
    );
    let res = file_from_bytes("svc/app.py", src.as_bytes());

    let file_id = make_id(&["svc/app.py"]);

    let mut label = std::collections::HashMap::new();
    let mut ftype = std::collections::HashMap::new();
    for n in &res.nodes {
        label.insert(n.id.as_str(), n.label.as_str());
        ftype.insert(n.id.as_str(), n.file_type.as_str());
    }
    let found = res.edges.iter().any(|e| {
        e.relation.as_str() == "rationale_for"
            && e.target == file_id
            && ftype.get(e.source.as_str()) == Some(&"rationale")
            && label
                .get(e.source.as_str())
                .is_some_and(|l| l.starts_with("Module docstring"))
    });
    assert!(
        found,
        "expected docstring after leading comment to yield rationale_for edge to {file_id}"
    );
}

const GATED: &[(&str, &str)] = &[
    (
        "method",
        "class A:\n    def f(self):\n        os.getcwd()\n",
    ),
    (
        "setattr",
        "def f(monkeypatch):\n    monkeypatch.setattr(x, 'a', 1)\n    os.getcwd()\n",
    ),
    (
        "delattr",
        "def g(monkeypatch):\n    monkeypatch.delattr(x, 'a')\ndef f():\n    os.getcwd()\n",
    ),
    ("exec", "def f():\n    exec('1', {})\n    os.getcwd()\n"),
    ("eval", "X = eval('1')\ndef f():\n    os.getcwd()\n"),
    (
        "globals",
        "def f():\n    os.getcwd()\n    return globals()\n",
    ),
    ("locals", "def f():\n    os.getcwd()\n    return locals()\n"),
    ("vars", "def f(vars):\n    os.getcwd()\n"),
    (
        "global x",
        "def g():\n    global n\n    n = 1\ndef f():\n    os.getcwd()\n",
    ),
    (
        "nonlocal x",
        "def g():\n    n = 0\n    def h():\n        nonlocal n\n        n = 1\ndef f():\n    os.getcwd()\n",
    ),
    (
        "def of name",
        "def eval(x): pass\ndef f():\n    os.getcwd()\n",
    ),
];

const REBOUND: &[(&str, &str)] = &[
    ("local", "def f():\n    os = object()\n    os.getcwd()\n"),
    ("parameter", "def f(os):\n    os.getcwd()\n"),
    ("default", "def f(*, os=None):\n    os.getcwd()\n"),
    ("splat", "def f(**os):\n    os.getcwd()\n"),
    ("unpack", "def f():\n    os, x = pair\n    os.getcwd()\n"),
    ("augmented", "def f():\n    os += 1\n    os.getcwd()\n"),
    (
        "walrus",
        "def f():\n    (os := object())\n    os.getcwd()\n",
    ),
    ("for", "def f():\n    for os in xs: pass\n    os.getcwd()\n"),
    (
        "comprehension",
        "def f():\n    return [os.getcwd() for os in xs]\n",
    ),
    (
        "with",
        "def f():\n    with open('x') as os: pass\n    os.getcwd()\n",
    ),
    (
        "except",
        "def f():\n    try: pass\n    except Exception as os: pass\n    os.getcwd()\n",
    ),
    (
        "nested def",
        "def f():\n    def os(): pass\n    os.getcwd()\n",
    ),
    ("del", "def f():\n    del os\n    os.getcwd()\n"),
    ("attribute", "def f():\n    os.value = 1\n    os.getcwd()\n"),
    ("local import", "def f():\n    import os\n    os.getcwd()\n"),
    ("global", "def f():\n    global os\n    os.getcwd()\n"),
    (
        "match",
        "def f():\n    match x:\n        case os: pass\n    os.getcwd()\n",
    ),
    (
        "lambda",
        "def f():\n    return (lambda os: os.getcwd())(object())\n",
    ),
    ("module", "os = object()\ndef f():\n    os.getcwd()\n"),
    (
        "guarded",
        "if flag:\n    import os\ndef f():\n    os.getcwd()\n",
    ),
    (
        "wildcard",
        "from elsewhere import *\ndef f():\n    os.getcwd()\n",
    ),
    (
        "other global",
        "def mutate():\n    global os\n    os = other\ndef f():\n    os.getcwd()\n",
    ),
    ("reimport", "import os\ndef f():\n    os.getcwd()\n"),
    (
        "alias clash",
        "import other as os\ndef f():\n    os.getcwd()\n",
    ),
    (
        "from import",
        "from other import os\ndef f():\n    os.getcwd()\n",
    ),
];

const NO_GATE: &[(&str, &str)] = &[
    (
        "string",
        "def f():\n    os.getcwd()\n    return 'setattr eval global x'\n",
    ),
    (
        "comment",
        "def f():\n    # global setattr\n    os.getcwd()\n",
    ),
    (
        "longer names",
        "def f(my_eval, setattrs, globals_, nonlocal_x):\n    os.getcwd()\n",
    ),
];

// A member call on a plainly imported external module is a call to the module
// node, unless the receiver could name anything else.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_python_external_module_calls() {
    crate::extract::kinds::register_kinds();
    let c = |src: &'static str, tgt: &'static str| [src, "calls", tgt];
    want_edges(
        &[(
            "a.py",
            "import os\nimport json\nimport numpy as np\nclass A:\n    json: dict\n    def go(self, x: np.ndarray):\n        os.getcwd()\n        return json.dumps({})\ndef f():\n    return np.array([])\n",
        )],
        &[c("f()", "numpy")],
        &[c("A.go()", "os"), c("A.go()", "json")],
    );

    // An import that resolves to a corpus file never falls back to the module.
    want_edges(
        &[
            ("helper.py", "def other():\n    pass\n"),
            ("a.py", "import helper\ndef f():\n    helper.get()\n"),
        ],
        &[],
        &[c("f()", "helper.py"), c("f()", "helper")],
    );

    for (name, body) in REBOUND {
        let src = format!("import os\n{body}");
        eprintln!("case {name}");
        want_edges(
            &[("a.py", src.as_str())],
            &[],
            &[c("f()", "os"), c("f()", "other")],
        );
    }

    // Upstream gives no such edge in a file that can rebind a module by name:
    // one with a global or nonlocal statement, or with one of these identifiers.
    for (name, body) in GATED {
        let src = format!("import os\n{body}");
        eprintln!("case gate {name}");
        want_edges(
            &[("a.py", src.as_str())],
            &[],
            &[c("f()", "os"), c("A.f()", "os")],
        );
    }
    for (name, body) in NO_GATE {
        let src = format!("import os\n{body}");
        eprintln!("case no gate {name}");
        want_edges(&[("a.py", src.as_str())], &[c("f()", "os")], &[]);
    }
}

/// smoke; the real checks are the ihc.18-ignored tests
#[test]
fn test_extract_python_smoke() {
    crate::extract::kinds::register_kinds();
    let mut srcs: Vec<(String, Vec<u8>)> = Vec::new();
    let root = testdata().join("extract").join("pyproj");
    for f in ["util/math.py", "web/server.py"] {
        let bytes = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
        srcs.push((f.to_string(), bytes));
    }
    for (name, body) in GATED.iter().chain(REBOUND).chain(NO_GATE) {
        srcs.push((
            format!("{name}.py"),
            format!("import os\n{body}").into_bytes(),
        ));
    }
    for (rel, src) in [
        (
            "svc/app.py",
            "# NOTE: keep boot fast\ndef boot():\n    return 1\n",
        ),
        (
            "service.py",
            "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    from models import Item\nelse:\n    from fallback import Item\n",
        ),
        (
            "a.py",
            "import os\nclass A:\n    def go(self, x: os.X):\n        os.getcwd()\n",
        ),
    ] {
        srcs.push((rel.to_string(), src.as_bytes().to_vec()));
    }
    for (rel, src) in &srcs {
        assert!(super::extract_python(rel, src).is_some(), "{rel}");
        let res = crate::extract::file_from_bytes(rel, src);
        assert!(!res.nodes.is_empty(), "{rel}: no file node");
    }
}
