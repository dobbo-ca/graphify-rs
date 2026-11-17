//! graphify-go `internal/extract/python_nested_test.go`. `resolvePy` is in
//! `python_import_guided_test.go`.

use std::collections::{HashMap, HashSet};

use crate::extract::{file_from_bytes, resolve};
use crate::idutil::make_id;
use crate::model::{Confidence, Extraction, Relation};

/// Go `resolvePy` (`python_import_guided_test.go:10`): extracts each in-memory Python file and
/// stitches them with `resolve`.
fn resolve_py(srcs: &[(&str, &str)]) -> Extraction {
    crate::extract::kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(f, s)| file_from_bytes(f, s.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(f, _)| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

// pyEdges resolves the sources and returns the edges of the given relations
// as sorted "source label -relation-> target label" lines. A file node shows
// as its path.
fn py_edges(srcs: &[(&str, &str)], rels: &[Relation]) -> Vec<String> {
    let ext = resolve_py(srcs);
    let mut label: HashMap<String, String> = HashMap::new();
    for n in &ext.nodes {
        label.insert(n.id.clone(), n.label.clone());
    }
    // A label of two files shows with its file.
    let mut files: HashMap<&str, &str> = HashMap::new();
    let mut dup: HashSet<&str> = HashSet::new();
    for n in &ext.nodes {
        if files
            .get(n.label.as_str())
            .is_some_and(|f| *f != n.source_file)
        {
            dup.insert(n.label.as_str());
        }
        files.insert(n.label.as_str(), n.source_file.as_str());
    }
    let mut in_file: HashMap<(&str, &str), usize> = HashMap::new();
    for n in &ext.nodes {
        *in_file
            .entry((n.source_file.as_str(), n.label.as_str()))
            .or_default() += 1;
    }
    for n in &ext.nodes {
        if dup.contains(n.label.as_str()) {
            label.insert(n.id.clone(), format!("{}:{}", n.source_file, n.label));
        }
        // Two nodes of one label in a file show as their ids.
        if in_file[&(n.source_file.as_str(), n.label.as_str())] > 1 {
            label.insert(n.id.clone(), n.id.clone());
        }
    }
    let file_ids: Vec<(String, &str)> = srcs.iter().map(|(f, _)| (make_id(&[f]), *f)).collect();
    for (id, f) in &file_ids {
        label.insert(id.clone(), (*f).to_string());
    }
    let mut out = Vec::new();
    for e in &ext.edges {
        if rels.contains(&e.relation) {
            let mut tgt = label.get(&e.target).cloned().unwrap_or_default();
            if tgt.is_empty() {
                tgt = e.target.clone();
            }
            let src = label.get(&e.source).cloned().unwrap_or_default();
            out.push(format!("{src} -{}-> {tgt}", e.relation.as_str()));
        }
    }
    out.sort();
    out.dedup();
    out
}

// A def inside a function is a node of its own, held by the function around
// it, and a call is credited to the innermost def that holds it.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_python_nested_functions() {
    let src = concat!(
        "def leaf(): pass\n",
        "def other(): pass\n",
        "def third(): pass\n",
        "\n",
        "def outer():\n",
        "    def inner(a=third()):\n",
        "        def deep():\n",
        "            leaf()\n",
        "        other()\n",
        "        deep()\n",
        "    @deco(third())\n",
        "    def wrapped():\n",
        "        helper()\n",
        "    def helper(): pass\n",
        "    lam = lambda: other()\n",
        "    class Local:\n",
        "        third()\n",
        "        def meth(self):\n",
        "            leaf()\n",
        "    if lam:\n",
        "        def cond(): pass\n",
        "    inner()\n",
        "\n",
        "class K:\n",
        "    def m(self):\n",
        "        def in_m():\n",
        "            leaf()\n",
        "        in_m()\n",
        "\n",
        "def unrelated():\n",
        "    helper()\n",
        "    inner()\n"
    );
    let got = py_edges(&[("a.py", src)], &[Relation::CONTAINS, Relation::CALLS]);
    let want = [
        "K -contains-> K.m()",
        "K.m() -calls-> in_m()",
        "K.m() -contains-> in_m()",
        "a.py -contains-> K",
        "a.py -contains-> leaf()",
        "a.py -contains-> other()",
        "a.py -contains-> outer()",
        "a.py -contains-> third()",
        "a.py -contains-> unrelated()",
        "deep() -calls-> leaf()",
        "in_m() -calls-> leaf()",
        "inner() -calls-> deep()",
        "inner() -calls-> other()",
        "inner() -contains-> deep()",
        "meth() -calls-> leaf()",
        "outer() -calls-> inner()",
        // the lambda, the decorator and the class body run in outer
        "outer() -calls-> other()",
        "outer() -calls-> third()",
        "outer() -contains-> cond()",
        "outer() -contains-> helper()",
        "outer() -contains-> inner()",
        "outer() -contains-> meth()",
        "outer() -contains-> wrapped()",
        "wrapped() -calls-> helper()",
    ];
    assert_eq!(got, want, "edges:\n got {got:?}\nwant {want:?}");
}

// A call binds through the import that names its callee or its receiver: a
// module alias, an import inside the function, and an aliased name. other/mod.py
// makes each name one the name alone cannot bind.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_python_calls_through_imports() {
    let srcs: &[(&str, &str)] = &[
        ("pkg/__init__.py", "from .mod import g as reexp\n"),
        (
            "pkg/mod.py",
            concat!(
                "def f(): pass\n",
                "def g(): pass\n",
                "class Cls:\n",
                "    def f(self): pass\n"
            ),
        ),
        ("other/mod.py", "def f(): pass\ndef g(): pass\n"),
        (
            "main.py",
            concat!(
                "import pkg.mod as m\n",
                "from pkg import mod as mm\n",
                "from pkg import reexp\n",
                "from pkg.mod import g as galias\n",
                "\n",
                "def alias():\n",
                "    m.f()\n",
                "def from_mod():\n",
                "    mm.g()\n",
                "def from_alias():\n",
                "    galias()\n",
                "def via_init():\n",
                "    reexp()\n",
                "def local_from():\n",
                "    from pkg.mod import f\n",
                "    f()\n",
                "def local_alias():\n",
                "    from pkg.mod import f as ff\n",
                "    ff()\n",
                "def local_mod():\n",
                "    from pkg import mod as lm\n",
                "    lm.g()\n",
                "def local_import():\n",
                "    import pkg.mod as pm\n",
                "    def inner():\n",
                "        pm.f()\n",
                "def no_leak():\n",
                "    ff()\n",
                "    lm.g()\n",
                "def shadowed(m, galias):\n",
                "    m.f()\n",
                "    galias()\n",
                "def rebound():\n",
                "    mm = 1\n",
                "    mm.g()\n",
                "class K:\n",
                "    def meth(self):\n",
                "        m.g()\n"
            ),
        ),
    ];
    let got = py_edges(srcs, &[Relation::CALLS]);
    let want = [
        "K.meth() -calls-> pkg/mod.py:g()",
        "alias() -calls-> pkg/mod.py:f()",
        "from_alias() -calls-> pkg/mod.py:g()",
        "from_mod() -calls-> pkg/mod.py:g()",
        "inner() -calls-> pkg/mod.py:f()",
        "local_alias() -calls-> pkg/mod.py:f()",
        "local_from() -calls-> pkg/mod.py:f()",
        "local_mod() -calls-> pkg/mod.py:g()",
        "via_init() -calls-> pkg/mod.py:g()",
    ];
    assert_eq!(got, want, "calls:\n got {got:?}\nwant {want:?}");
    let ext = resolve_py(srcs);
    for e in &ext.edges {
        assert!(
            !(e.relation == Relation::CALLS && e.confidence != Confidence::EXTRACTED),
            "call {} -> {} is {}, want EXTRACTED",
            e.source,
            e.target,
            e.confidence
        );
    }
}

// A file imports each top-level definition a from-import names, at any depth
// and through a package __init__.py; a submodule, a constant, a method and a
// nested def get no such edge.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_python_imported_symbol_edges() {
    let srcs: &[(&str, &str)] = &[
        (
            "pkg/__init__.py",
            "from .core import reexp, renamed_src as renamed\n",
        ),
        (
            "pkg/core.py",
            concat!(
                "CONST = 1\n",
                "def func_a(): pass\n",
                "def func_b(): pass\n",
                "def reexp(): pass\n",
                "def renamed_src(): pass\n",
                "class Klass:\n",
                "    def meth_only(self): pass\n",
                "def outer():\n",
                "    def nested_fn(): pass\n"
            ),
        ),
        ("pkg/mod.py", "def in_mod(): pass\ndef starred(): pass\n"),
        ("pkg/plain.py", "from .core import func_b\n"),
        (
            "main.py",
            concat!(
                "from typing import TYPE_CHECKING\n",
                "from pkg.core import func_a, func_b as fb\n",
                "from pkg import mod\n",
                "from pkg import reexp, renamed\n",
                "from pkg.core import CONST, missing, meth_only, nested_fn\n",
                "from pkg.plain import func_b as via_plain\n",
                "from pkg.mod import *\n",
                "from main import lazy\n",
                "if TYPE_CHECKING:\n",
                "    from pkg.core import Klass\n",
                "def lazy():\n",
                "    from pkg.mod import in_mod\n",
                "    from pkg.core import func_a\n"
            ),
        ),
    ];
    let got = py_edges(srcs, &[Relation::IMPORTS]);
    let want = [
        "main.py -imports-> Klass",
        "main.py -imports-> func_a()",
        "main.py -imports-> func_b()",
        "main.py -imports-> in_mod()",
        // an import of the file itself stays the external it was
        "main.py -imports-> main",
        "main.py -imports-> reexp()",
        "main.py -imports-> renamed_src()",
        "main.py -imports-> typing",
        "pkg/__init__.py -imports-> reexp()",
        "pkg/__init__.py -imports-> renamed_src()",
        "pkg/plain.py -imports-> func_b()",
    ];
    assert_eq!(got, want, "imports:\n got {got:?}\nwant {want:?}");
    // The module edges stay as they are: none for the import inside lazy.
    let got = py_edges(srcs, &[Relation::IMPORTS_FROM]);
    let want = [
        "main.py -imports_from-> pkg/__init__.py",
        "main.py -imports_from-> pkg/core.py",
        "main.py -imports_from-> pkg/mod.py",
        "main.py -imports_from-> pkg/plain.py",
        "pkg/__init__.py -imports_from-> pkg/core.py",
        "pkg/plain.py -imports_from-> pkg/core.py",
    ];
    assert_eq!(got, want, "imports_from:\n got {got:?}\nwant {want:?}");
}

type Case = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static [Relation],
    &'static [&'static str],
);

const EDGE_CASES: &[Case] = &[
    (
        "self in a def nested in a method is the self of the method",
        &[(
            "a.py",
            concat!(
                "class K:\n",
                "    def save(self): pass\n",
                "    def m(self):\n",
                "        def in_m():\n",
                "            self.save()\n",
                "            def deeper():\n",
                "                self.save()\n"
            ),
        )],
        &[Relation::CALLS],
        &["deeper() -calls-> K.save()", "in_m() -calls-> K.save()"],
    ),
    (
        "self in a method of a local class is not",
        &[(
            "a.py",
            concat!(
                "class K:\n",
                "    def save(self): pass\n",
                "    def m(self):\n",
                "        class Local:\n",
                "            def go(self):\n",
                "                self.save()\n",
                "                def inner():\n",
                "                    self.save()\n"
            ),
        )],
        &[Relation::CALLS],
        &[],
    ),
    (
        "a nested def keeps off the id of a module-level def",
        &[(
            "a.py",
            concat!(
                "def alpha(): pass\n",
                "def beta(): pass\n",
                "def load():\n",
                "    def config():\n",
                "        beta()\n",
                "    return config()\n",
                "def load_config():\n",
                "    alpha()\n"
            ),
        )],
        &[Relation::CALLS, Relation::CONTAINS],
        &[
            "a.py -contains-> alpha()",
            "a.py -contains-> beta()",
            "a.py -contains-> load()",
            "a.py -contains-> load_config()",
            "config() -calls-> beta()",
            "load() -calls-> config()",
            "load() -contains-> config()",
            "load_config() -calls-> alpha()",
        ],
    ),
    (
        "and off the id of a method",
        &[(
            "a.py",
            concat!(
                "def other(): pass\n",
                "class K:\n",
                "    def m_helper(self): pass\n",
                "    def m(self):\n",
                "        def helper():\n",
                "            other()\n"
            ),
        )],
        &[Relation::CALLS, Relation::CONTAINS],
        &[
            "K -contains-> K.m()",
            "K -contains-> K.m_helper()",
            "K.m() -contains-> helper()",
            "a.py -contains-> K",
            "a.py -contains-> other()",
            "helper() -calls-> other()",
        ],
    ),
    (
        "a default and an annotation of a nested def run in the function around it",
        &[(
            "a.py",
            concat!(
                "def make(): pass\n",
                "def ann(): pass\n",
                "def outer(x=make()):\n",
                "    def inner(x=make()) -> ann():\n",
                "        pass\n"
            ),
        )],
        &[Relation::CALLS],
        &["outer() -calls-> ann()", "outer() -calls-> make()"],
    ),
    (
        "an import in the function hides the import of the module",
        &[
            ("utils.py", "def run(): pass\n"),
            ("shim.py", "def run(): pass\n"),
            (
                "app.py",
                concat!(
                    "from utils import run\n",
                    "def build():\n",
                    "    run()\n",
                    "def deploy():\n",
                    "    from shim import run\n",
                    "    run()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &[
            "build() -calls-> utils.py:run()",
            "deploy() -calls-> shim.py:run()",
        ],
    ),
    (
        "an import in a nested def does not reach the function around it",
        &[
            ("m.py", "def run(): pass\n"),
            ("n.py", "def run(): pass\n"),
            (
                "nest.py",
                concat!(
                    "import m as x\n",
                    "def outer():\n",
                    "    def inner():\n",
                    "        import n as x\n",
                    "        x.run()\n",
                    "    def sibling():\n",
                    "        x.run()\n",
                    "    x.run()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &[
            "inner() -calls-> n.py:run()",
            "outer() -calls-> m.py:run()",
            "sibling() -calls-> m.py:run()",
        ],
    ),
    (
        "a from-imported name is no external module",
        &[(
            "pkg/a.py",
            concat!(
                "from sqlalchemy import orm\n",
                "from sqlalchemy.orm import Session\n",
                "from . import x\n",
                "from .x import y\n",
                "def f():\n",
                "    orm.sessionmaker()\n",
                "    x.go()\n"
            ),
        )],
        &[Relation::CALLS],
        &[],
    ),
    (
        "a from-import in a class body",
        &[
            ("mod.py", "def top(): pass\n"),
            ("cb.py", "class Cls:\n    from mod import top\n"),
        ],
        &[Relation::IMPORTS],
        &["cb.py -imports-> top()"],
    ),
    // Round 2.
    (
        "two nested defs of one id are one node that both names reach",
        &[
            ("other.py", "def helper(): pass\n"),
            (
                "m.py",
                concat!(
                    "def under():\n",
                    "    def _helper(): pass\n",
                    "    def helper():\n",
                    "        return _helper()\n",
                    "    return helper()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &["under() -calls-> _helper()"],
    ),
    (
        "an import in the body of a local class is no name of its methods",
        &[
            ("a.py", "def dumps(): pass\n"),
            ("b.py", "def dumps(): pass\n"),
            (
                "m.py",
                concat!(
                    "from a import dumps\n",
                    "def local_cls():\n",
                    "    class C:\n",
                    "        from b import dumps\n",
                    "        def m(self):\n",
                    "            return dumps()\n",
                    "    dumps()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &[
            "local_cls() -calls-> a.py:dumps()",
            "m() -calls-> a.py:dumps()",
        ],
    ),
    (
        "a name the function imports twice or as a module binds nothing",
        &[
            ("a.py", "def f(): pass\n"),
            ("b.py", "def f(): pass\n"),
            ("c.py", "def f(): pass\n"),
            (
                "app.py",
                concat!(
                    "from a import f\n",
                    "def cond(x):\n",
                    "    if x:\n",
                    "        from b import f\n",
                    "    else:\n",
                    "        from c import f\n",
                    "    f()\n",
                    "def plain():\n",
                    "    import b as f\n",
                    "    f()\n",
                    "def ext():\n",
                    "    from extlib import f\n",
                    "    f()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &[],
    ),
    (
        "the path of the import binds before the stem of its module",
        &[
            ("a/util.py", "def fmt(): pass\n"),
            ("b/core.py", "def fmt(): pass\n"),
            ("b/util.py", "from .core import fmt\n"),
            (
                "main.py",
                "from b.util import fmt\ndef main():\n    fmt()\n",
            ),
        ],
        &[Relation::CALLS],
        &["main() -calls-> b/core.py:fmt()"],
    ),
    (
        "a name the package binds is not its submodule",
        &[
            ("pkg/__init__.py", "from .config import config\n"),
            (
                "pkg/config.py",
                "class Config:\n    def load(self): pass\ndef load(): pass\ndef config(): pass\n",
            ),
            (
                "main.py",
                "from pkg import config\ndef main():\n    config.load()\n",
            ),
        ],
        &[Relation::CALLS],
        &[],
    ),
    (
        "a from-import of a bare relative module",
        &[
            ("pkg/__init__.py", "def top(): pass\n"),
            ("pkg/a.py", "from . import top\n"),
            ("pkg/sub/__init__.py", "from .. import top\n"),
            ("far.py", "from pkg.sub import top\n"),
        ],
        &[Relation::IMPORTS],
        &[
            "far.py -imports-> top()",
            "pkg/a.py -imports-> top()",
            "pkg/sub/__init__.py -imports-> top()",
        ],
    ),
    // Round 3.
    (
        "a package that imports its own submodule still gives it",
        &[
            ("p/__init__.py", "from . import core\n"),
            ("p/core.py", "def run(): pass\n"),
            ("x/core.py", "def run(): pass\n"),
            (
                "main.py",
                concat!(
                    "from p import core as cc\n",
                    "def m3():\n",
                    "    from p import core\n",
                    "    core.run()\n",
                    "def m4():\n",
                    "    cc.run()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &[
            "m3() -calls-> p/core.py:run()",
            "m4() -calls-> p/core.py:run()",
        ],
    ),
    (
        "an import in the function binds by the stem of a module off the path",
        &[
            ("src/pkg/m.py", "def gsrc(): pass\n"),
            (
                "tests/u.py",
                concat!(
                    "def gsrc(): pass\n",
                    "def test_fn():\n",
                    "    from pkg.m import gsrc as g\n",
                    "    g()\n"
                ),
            ),
        ],
        &[Relation::CALLS],
        &["test_fn() -calls-> src/pkg/m.py:gsrc()"],
    ),
    (
        "a def nested in one function is no name of a later function of its id",
        &[(
            "m.py",
            concat!(
                "def dup():\n",
                "    def helper(): pass\n",
                "    return helper()\n",
                "def dup():\n",
                "    return helper()\n",
                "def helper(): pass\n",
                "class P:\n",
                "    @property\n",
                "    def val(self):\n",
                "        def calc(): pass\n",
                "        return calc()\n",
                "    @val.setter\n",
                "    def val(self, v):\n",
                "        calc()\n",
                "def calc(): pass\n"
            ),
        )],
        &[Relation::CALLS],
        &[
            "P.val() -calls-> m_py_calc",
            "P.val() -calls-> m_py_p_val_calc",
            "dup() -calls-> m_py_dup_helper",
            "dup() -calls-> m_py_helper",
        ],
    ),
];

// Round 1 of the review of the nested defs and the import-bound calls.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_python_nested_and_import_edge_cases() {
    for (name, srcs, rels, want) in EDGE_CASES {
        let got = py_edges(srcs, rels);
        assert_eq!(got, *want, "{name}: edges:\n got {got:?}\nwant {want:?}");
    }
}

type BuiltinCase = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
);

const BUILTIN_CASES: &[BuiltinCase] = &[
    (
        "a method of another file",
        &[
            (
                "models.py",
                "class Cookies:\n    def set(self, name): pass\n",
            ),
            ("a.py", "def f():\n    return set()\n"),
        ],
        &[],
    ),
    (
        "a function of another file",
        &[
            ("b.py", "def len(x): pass\n"),
            ("a.py", "def f():\n    return len([])\n"),
        ],
        &[],
    ),
    (
        "a function of a file the caller imports",
        &[
            ("b.py", "def open(p): pass\n"),
            ("a.py", "import b\ndef f():\n    return open('x')\n"),
        ],
        &[],
    ),
    (
        "a method of the same file",
        &[(
            "a.py",
            "class Cookies:\n    def set(self, name): pass\ndef f():\n    return set()\n",
        )],
        &[],
    ),
    (
        "a function of the same file",
        &[("a.py", "def len(x): pass\ndef f():\n    return len([])\n")],
        &["f() -calls-> len()"],
    ),
    (
        "an import of the name",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "from b import open\ndef f():\n    return open('x')\n",
            ),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "an import of the name under an alias",
        &[
            ("b.py", "def unpack(p): pass\n"),
            (
                "a.py",
                "from b import unpack as list\ndef f():\n    return list('x')\n",
            ),
        ],
        &["f() -calls-> unpack()"],
    ),
    (
        "an import of the name inside the function",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "def f():\n    from b import open\n    return open('x')\n",
            ),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "an import of the name that a star import gives the module",
        &[
            ("c.py", "def open(p): pass\n"),
            ("b.py", "from c import *\n"),
            (
                "a.py",
                "from b import open\ndef f():\n    return open('x')\n",
            ),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "an import of the name under try",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "try:\n    from b import open\nexcept ImportError:\n    pass\ndef f():\n    return open('x')\n",
            ),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "a star import can bind the name",
        &[
            ("b.py", "def open(p): pass\n"),
            ("a.py", "from b import *\ndef f():\n    return open('x')\n"),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "a class of the same file",
        &[("a.py", "class set: pass\ndef f():\n    return set()\n")],
        &["f() -calls-> set"],
    ),
    (
        "a def whose id is that of the name",
        &[
            ("b.py", "class Console:\n    def print(self, x): pass\n"),
            (
                "a.py",
                "def _print(m): pass\nclass Input: pass\ndef f():\n    print('x')\n    input()\n",
            ),
            ("c.py", "def input(p): pass\n"),
        ],
        &[],
    ),
    (
        "an import under a module-level for",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "for _ in (1,):\n    from b import open\ndef f():\n    return open('x')\n",
            ),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "an import inside another function or a class body",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "def g():\n    from b import open\nclass K:\n    from b import open\n    def m(self):\n        return open('x')\ndef f():\n    return open('x')\n",
            ),
        ],
        &[],
    ),
    (
        "an import inside the function beside another binding",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "def f():\n    try:\n        from b import open\n    except ImportError:\n        open = None\n    return open('x')\n",
            ),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "an import of the function around beside a parameter of a def",
        &[
            ("b.py", "def open(p): pass\n"),
            (
                "a.py",
                "def f():\n    from b import open\n    def cb(open): pass\n    def g():\n        return open('x')\n",
            ),
        ],
        &["g() -calls-> open()"],
    ),
    (
        "exit of the site module",
        &[
            ("b.py", "class App:\n    def exit(self): pass\n"),
            ("a.py", "def f():\n    exit(1)\n"),
        ],
        &[],
    ),
    (
        "a member call keeps its receiver rule",
        &[
            ("b.py", "def open(p): pass\n"),
            ("a.py", "import b\ndef f():\n    return b.open('x')\n"),
        ],
        &["f() -calls-> open()"],
    ),
    (
        "another name still binds by the unique-name rule",
        &[
            ("b.py", "def helper(p): pass\n"),
            ("a.py", "def f():\n    return helper('x')\n"),
        ],
        &["f() -calls-> helper()"],
    ),
];

// A bare call of a Python builtin names the builtin unless the file defines
// or imports the name.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_python_builtin_calls() {
    for (name, srcs, want) in BUILTIN_CASES {
        let got = py_edges(srcs, &[Relation::CALLS]);
        assert_eq!(got, *want, "{name}: edges:\n got {got:?}\nwant {want:?}");
    }
}
