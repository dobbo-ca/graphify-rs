//! graphify-go `internal/extract/inherits_test.go`. Its helpers `extractAll` and `wantEdges` are
//! in `tests/util.rs`.

use crate::extract::kinds::register_kinds;
use crate::extract::tests::util::{extract_all, want_edges};

// Acceptance: `class B extends A implements I` yields inherits and implements
// edges that resolve across files.
#[test]
#[ignore = "graphify-rs-ihc.27"] // also needs resolve (ihc.18)
fn test_java_inherits_implements() {
    register_kinds();
    want_edges(
        &[
            (
                "pkg/A.java",
                "class A { }\ninterface I { }\ninterface J extends I { }\n",
            ),
            (
                "pkg/B.java",
                "import pkg.A;\nclass B extends A implements I, J { }\n",
            ),
        ],
        &[
            ["B", "inherits", "A"],
            ["B", "implements", "I"],
            ["B", "implements", "J"],
            ["J", "inherits", "I"],
        ],
        &[],
    );
}

// A generic, qualified supertype still binds to the bare type name.
#[test]
#[ignore = "graphify-rs-ihc.27"] // also needs resolve (ihc.18)
fn test_java_generic_qualified_base() {
    register_kinds();
    want_edges(
        &[
            ("Base.java", "class Base { }\n"),
            ("Sub.java", "class Sub extends foo.Base<String> { }\n"),
        ],
        &[["Sub", "inherits", "Base"]],
        &[],
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_c_sharp_bases() {
    register_kinds();
    want_edges(
        &[
            (
                "lib.cs",
                "interface IThing { }\ninterface IOther { }\nclass Base { }\n",
            ),
            (
                "impl.cs",
                "class Impl : Base, IThing, IOther { }\ninterface ISub : IThing { }\n",
            ),
        ],
        &[
            ["Impl", "inherits", "Base"],
            ["Impl", "implements", "IThing"],
            ["Impl", "implements", "IOther"],
            ["ISub", "inherits", "IThing"],
        ],
        &[["Impl", "inherits", "IThing"]],
    );
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // also needs resolve (ihc.18)
fn test_python_inherits() {
    register_kinds();
    want_edges(
        &[
            (
                "base.py",
                "class Base:\n    pass\n\nclass Meta(type):\n    pass\n",
            ),
            (
                "sub.py",
                "from base import Base, Meta\n\nclass Sub(Base, metaclass=Meta):\n    pass\n",
            ),
        ],
        &[["Sub", "inherits", "Base"]],
        &[["Sub", "inherits", "Meta"]],
    );
}

// A qualified Python base outside the corpus drops; a plain one gets a stub
// (TestPythonReferences).
#[test]
#[ignore = "graphify-rs-ihc.18"] // also needs resolve (ihc.18)
fn test_unresolved_base_drops() {
    register_kinds();
    let got = extract_all(&[("sub.py", "class Sub(lib.SomeLibBase):\n    pass\n")]);
    for e in &got {
        assert!(
            e.1 != "inherits",
            "unexpected inherits edge for out-of-corpus base: {e:?}"
        );
    }
}

// A base class with an explicit constructor registers a member named after the
// class, which must not make `extends A` ambiguous — same file, same package,
// and cross-file.
#[test]
#[ignore = "graphify-rs-ihc.27"] // also needs resolve (ihc.18)
fn test_java_base_with_constructor() {
    register_kinds();
    want_edges(
        &[
            ("pkg/A.java", "class A { A() {} }\n"),
            ("pkg/B.java", "class B extends A { B() {} }\n"),
            ("other/C.java", "import pkg.A;\nclass C extends A { }\n"),
            ("Same.java", "class P { P() {} }\nclass Q extends P { }\n"),
        ],
        &[
            ["B", "inherits", "A"],
            ["C", "inherits", "A"],
            ["Q", "inherits", "P"],
        ],
        &[],
    );
}
