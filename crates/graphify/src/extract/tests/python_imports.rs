//! graphify-go `internal/extract/python_imports_test.go`. It tests `resolvePyModule`
//! (`resolve.go:1827`) through `Resolve`, so it is ported with the resolve tests (handoff branch
//! `handoff-py-resolve-tests-9d16`).

use std::collections::BTreeSet;

use crate::extract::python::extract_python;
use crate::extract::resolve;

// pyResolve extracts each Python source and resolves the corpus, returning the
// sorted "src rel tgt[ type]" import edges and the node ids.
fn py_resolve(srcs: &[(&str, &str)]) -> (Vec<String>, BTreeSet<String>) {
    crate::extract::kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(f, s)| extract_python(f, s.as_bytes()).expect("parse budget"))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(f, _)| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    let mut edges = Vec::new();
    for e in &ext.edges {
        if e.relation.as_str() != "imports" && e.relation.as_str() != "imports_from" {
            continue;
        }
        let mut s = format!("{} {} {}", e.source, e.relation.as_str(), e.target);
        if e.type_only() {
            s += " type";
        }
        edges.push(s);
    }
    edges.sort();
    let ids = ext.nodes.iter().map(|n| n.id.clone()).collect();
    (edges, ids)
}

type Case = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
);

const CASES: &[Case] = &[
    (
        "sibling cycle",
        &[("pkg/a.py", "import b\n"), ("pkg/b.py", "import a\n")],
        &[
            "pkg_a_py imports_from pkg_b_py",
            "pkg_b_py imports_from pkg_a_py",
        ],
    ),
    (
        "relative module",
        &[("pkg/a.py", "from .b import helper\n"), ("pkg/b.py", "")],
        &["pkg_a_py imports_from pkg_b_py"],
    ),
    (
        "from dot import module",
        &[("pkg/a.py", "from . import b\n"), ("pkg/b.py", "")],
        &["pkg_a_py imports_from pkg_b_py"],
    ),
    (
        "from package import submodule",
        &[
            ("pkg/a.py", "from .sub import mod\n"),
            ("pkg/sub/__init__.py", ""),
            ("pkg/sub/mod.py", ""),
        ],
        &[
            "pkg_a_py imports_from pkg_sub_init_py",
            "pkg_a_py imports_from pkg_sub_mod_py",
        ],
    ),
    (
        "parent relative",
        &[("pkg/sub/a.py", "from ..b import x\n"), ("pkg/b.py", "")],
        &["pkg_sub_a_py imports_from pkg_b_py"],
    ),
    (
        "absolute dotted",
        &[
            ("main.py", "import pkg.b\nfrom pkg.a import A\n"),
            ("pkg/a.py", ""),
            ("pkg/b.py", ""),
        ],
        &[
            "main_py imports_from pkg_a_py",
            "main_py imports_from pkg_b_py",
        ],
    ),
    (
        "ambiguous absolute drops",
        &[("src/a.py", "import b\n"), ("src/b.py", ""), ("b.py", "")],
        &["src_a_py imports b"],
    ),
    (
        "sibling of a package stays external",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/a.py", "import json\n"),
            ("pkg/json.py", ""),
        ],
        &["pkg_a_py imports json"],
    ),
    (
        "nested blocks",
        &[
            (
                "a.py",
                "try:\n    import b\nexcept ImportError:\n    import c\nwith x:\n    import d\n",
            ),
            ("b.py", ""),
            ("c.py", ""),
            ("d.py", ""),
        ],
        &[
            "a_py imports_from b_py",
            "a_py imports_from c_py",
            "a_py imports_from d_py",
        ],
    ),
    (
        "type checking",
        &[
            (
                "a.py",
                "import typing\nif typing.TYPE_CHECKING:\n    import b\nelse:\n    import c\nif TYPE_CHECKING:\n    import d\n",
            ),
            ("b.py", ""),
            ("c.py", ""),
            ("d.py", ""),
        ],
        &[
            "a_py imports typing",
            "a_py imports_from b_py type",
            "a_py imports_from c_py",
            "a_py imports_from d_py type",
        ],
    ),
    (
        "negated or compound type checking runs",
        &[
            (
                "a.py",
                "import typing\nif not typing.TYPE_CHECKING:\n    import b\nif x or typing.TYPE_CHECKING:\n    import c\n",
            ),
            ("b.py", ""),
            ("c.py", ""),
        ],
        &[
            "a_py imports typing",
            "a_py imports_from b_py",
            "a_py imports_from c_py",
        ],
    ),
    (
        "future and unresolved dot",
        &[(
            "pkg/a.py",
            "from __future__ import annotations\nfrom . import missing\n",
        )],
        &[],
    ),
];

#[test]
#[ignore = "graphify-rs-ihc.25"] // also needs resolve (ihc.18)
fn test_python_imports_resolve_to_files() {
    for (name, srcs, want) in CASES {
        let (got, ids) = py_resolve(srcs);
        assert_eq!(&got[..], *want, "{name}: edges = {got:?}, want {want:?}");
        assert!(
            !ids.contains("") && !ids.contains("annotations") && !ids.contains("future"),
            "{name}: phantom node in {ids:?}"
        );
    }
}
