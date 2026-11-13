//! Port of `typeonly_cycle_test.go` (3 tests), then the upstream cases of
//! `test_type_only_import_cycles.py` and `test_undirected_two_file_import_cycle.py` that run
//! through extract, resolve and `import_cycles`, and hold for graphify-go v0.12.0.
//!
//! The TypeScript tests need the js extractor (graphify-rs-ihc.26), the Python ones the python
//! extractor (graphify-rs-ihc.25); both need the resolver (graphify-rs-ihc.18).

use super::names;
use crate::analyze::{Cycle, import_cycles};
use crate::extract::{self, Result as ExtractResult};
use crate::model::Graph;

/// Go `tsCycleGraph` generalised to any corpus: extracts and resolves `files` (path, source),
/// then builds the graph the report runs cycle detection over.
fn cycle_graph(files: &[(&str, &str)]) -> Graph {
    let results: Vec<ExtractResult> = files
        .iter()
        .map(|(f, src)| extract::file_from_bytes(f, src.as_bytes()))
        .collect();
    let paths: Vec<String> = files.iter().map(|(f, _)| f.to_string()).collect();
    // Go `extract.Resolve(results, files)`; the output does not depend on the worker count.
    let ext = extract::resolve(&results, &paths, 1);

    let mut g = Graph::new();
    for n in ext.nodes {
        g.add_node(n);
    }
    for e in ext.edges {
        g.add_edge(e);
    }
    g
}

/// Go `tsCycleGraph`: a two-file TS corpus where a.ts imports from b.ts and b.ts imports back
/// from a.ts. `a_import` is the import line used in a.ts.
fn ts_cycle_graph(a_import: &str) -> Graph {
    let a = format!("{a_import}\nexport function mkA(): number {{ return 1; }}\n");
    cycle_graph(&[
        ("a.ts", &a),
        (
            "b.ts",
            "import { mkA } from './a';\nexport class B { v = mkA(); }\n",
        ),
    ])
}

fn cycles_of(files: &[(&str, &str)]) -> Vec<Cycle> {
    import_cycles(&cycle_graph(files), 5, 20)
}

/// A type-only import is erased at compile time, so the round trip it closes is not a runtime
/// cycle; a value import in the same position still is.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_import_cycles_skips_type_only_imports() {
    let type_only = ts_cycle_graph("import type { B } from './b';");
    let cycles = import_cycles(&type_only, 5, 20);
    assert!(
        cycles.is_empty(),
        "expected no cycles for a type-only import, got {cycles:?}"
    );

    for imp in [
        "import { B } from './b';",
        "import { type B, other } from './b';", // mixed: still a value import
    ] {
        let cycles = import_cycles(&ts_cycle_graph(imp), 5, 20);
        assert_eq!(
            cycles.len(),
            1,
            "expected 1 cycle for {imp:?}, got {}: {cycles:?}",
            cycles.len()
        );
    }
}

/// Two imports from the same module (one type-only, one value) collapse to a single
/// imports_from edge, so the value import must win regardless of which line comes first.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_import_cycles_type_and_value_import_same_module() {
    for imp in [
        "import type { B } from './b';\nimport { mkB } from './b';",
        "import { mkB } from './b';\nimport type { B } from './b';",
    ] {
        let cycles = import_cycles(&ts_cycle_graph(imp), 5, 20);
        assert_eq!(
            cycles.len(),
            1,
            "expected 1 cycle for {imp:?}, got {}: {cycles:?}",
            cycles.len()
        );
    }
}

/// A deferred import() runs late, so it closes no static cycle.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_import_cycles_skips_deferred_imports() {
    for imp in [
        "export async function lazy() { return import('./b'); }", // from a function
        "describe('x', () => { import('./b'); });",               // from the file
    ] {
        let cycles = import_cycles(&ts_cycle_graph(imp), 5, 20);
        assert!(
            cycles.is_empty(),
            "expected no cycles for {imp:?}, got {cycles:?}"
        );
    }
    let cycles = import_cycles(&ts_cycle_graph("import { B } from './b';"), 5, 20);
    assert_eq!(
        cycles.len(),
        1,
        "a static import still closes the cycle, got {cycles:?}"
    );
}

// ---- Upstream: test_type_only_import_cycles.py (TypeScript) ----
// The stamp tests of that file (`test_import_type_statement_is_stamped` and the other edge-flag
// checks) assert extractor output, not cycles: they belong to the extract tests.

#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_the_reporters_two_file_cycle_is_gone() {
    let cycles = cycles_of(&[
        (
            "types.ts",
            "export type Variables = { requestId: string };\n\
             export type { RouterType } from \"./v1.js\";\n",
        ),
        (
            "v1.ts",
            "import type { Variables } from \"./types.js\";\n\
             export type RouterType = { v: Variables };\n\
             export function makeRouter() { return {} as RouterType; }\n",
        ),
    ]);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_a_real_runtime_cycle_is_still_reported() {
    let cycles = cycles_of(&[
        (
            "a.ts",
            "import { b } from \"./b.js\";\nexport const a = 1;\n",
        ),
        (
            "b.ts",
            "import { a } from \"./a.js\";\nexport const b = 2;\n",
        ),
    ]);
    assert_eq!(cycles.len(), 1, "cycles = {cycles:?}");
    assert_eq!(names(&cycles), [["a.ts", "b.ts"]], "cycles = {cycles:?}");
}

/// One runtime leg and one type-only leg: no runtime cycle exists.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_a_cycle_with_one_type_only_leg_is_not_a_cycle() {
    let cycles = cycles_of(&[
        (
            "a.ts",
            "import { b } from \"./b.js\";\nexport type AT = number;\nexport const a = 1;\n",
        ),
        (
            "b.ts",
            "import type { AT } from \"./a.js\";\nexport const b = 2;\n",
        ),
    ]);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

/// b imports a at runtime, a only `import type`s b: a's type-only import must not become a
/// runtime leg closing a -> b -> c -> a. (Upstream's undirected build merges a file pair into
/// one edge; Go keeps both directed edges, the assertion is the same.)
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_undirected_graph_keeps_the_marker_when_the_reverse_import_is_dropped() {
    let cycles = cycles_of(&[
        (
            "a.ts",
            "import type { B } from './b';\nexport class A { b?: B; }\n",
        ),
        (
            "b.ts",
            "import { A } from './a';\nimport { C } from './c';\n\
             export class B { a = new A(); c = new C(); }\n",
        ),
        (
            "c.ts",
            "import { A } from './a';\nexport class C { a = new A(); }\n",
        ),
    ]);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

/// The same merge hid a real cycle for TypeScript (#3123).
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_a_type_import_beside_a_value_import_of_the_same_file_keeps_the_cycle() {
    let cycles = cycles_of(&[
        (
            "a.ts",
            "import type { B } from \"./b.js\";\nimport { helper } from \"./b.js\";\n\
             export const a = (x: B) => helper(x);\n",
        ),
        (
            "b.ts",
            "import { a } from \"./a.js\";\nexport type B = number;\n\
             export function helper(x: number) { return a; }\n",
        ),
    ]);
    assert_eq!(names(&cycles), [["a.ts", "b.ts"]], "cycles = {cycles:?}");
}

// ---- Upstream: test_undirected_two_file_import_cycle.py ----

const TS_CYCLE: [(&str, &str); 2] = [
    (
        "src/a.ts",
        "import { B } from './b';\nexport class A { b?: B; }\n",
    ),
    (
        "src/b.ts",
        "import { A } from './a';\nexport class B { a?: A; }\n",
    ),
];
const PY_CYCLE: [(&str, &str); 2] = [
    ("a.py", "from b import B\n\nclass A:\n    b: B = None\n"),
    ("b.py", "from a import A\n\nclass B:\n    a: A = None\n"),
];

/// Two files importing each other are an import cycle (TS and Python). Without upstream's
/// `reverse_import` stamp and one-edge-per-pair assertions, which Go has no counterpart for.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_two_file_cycle_is_reported() {
    let ts = cycles_of(&TS_CYCLE);
    assert_eq!(names(&ts), [["a.ts", "b.ts"]], "ts: cycles = {ts:?}");
    let py = cycles_of(&PY_CYCLE);
    assert_eq!(names(&py), [["a.py", "b.py"]], "python: cycles = {py:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_a_one_way_import_is_not_a_cycle() {
    let cycles = cycles_of(&[
        ("a.py", "from b import B\n\nclass A:\n    b: B = None\n"),
        ("b.py", "class B:\n    pass\n"),
    ]);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_longer_cycles_are_still_reported_beside_a_two_file_one() {
    let cycles = cycles_of(&[
        ("a.py", "from b import B\nclass A: pass\n"),
        ("b.py", "from a import A\nfrom c import C\nclass B: pass\n"),
        ("c.py", "from a import A\nclass C: pass\n"),
    ]);
    assert_eq!(
        names(&cycles),
        [vec!["a.py", "b.py"], vec!["a.py", "b.py", "c.py"]],
        "cycles = {cycles:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_a_type_only_reverse_import_closes_no_cycle() {
    const CASES: [(&str, [(&str, &str); 2]); 2] = [
        (
            "ts-import-type",
            [
                (
                    "a.ts",
                    "import { B } from './b';\nexport type AT = number;\nexport const a = 1;\n",
                ),
                (
                    "b.ts",
                    "import type { AT } from './a';\nexport class B {}\n",
                ),
            ],
        ),
        (
            "python-type-checking",
            [
                ("a.py", "from b import B\n\nclass A:\n    b: B = None\n"),
                (
                    "b.py",
                    "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    from a import A\n\n\
                     class B:\n    a: 'A' = None\n",
                ),
            ],
        ),
    ];
    for (name, files) in &CASES {
        let cycles = cycles_of(files);
        assert!(cycles.is_empty(), "{name}: cycles = {cycles:?}");
    }
}

// ---- Upstream: test_type_only_import_cycles.py (Python, `if TYPE_CHECKING:`) ----

/// The shape in the issue: a two-file cycle that closes only through a runtime import.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_python_two_file_cycle_closes_only_through_a_runtime_import() {
    const CASES: [(&str, &str, &[[&str; 2]]); 2] = [
        (
            "guarded",
            "if TYPE_CHECKING:\n    from app import AppState\n",
            &[],
        ),
        (
            "runtime",
            "from app import AppState\n",
            &[["app.py", "service.py"]],
        ),
    ];
    for (name, service_import, expected) in CASES {
        let service = format!(
            "from __future__ import annotations\nfrom typing import TYPE_CHECKING\n\n{service_import}\
             \nclass Service:\n    def run(self, state: AppState):\n        return state\n"
        );
        let cycles = cycles_of(&[
            (
                "app.py",
                "from service import Service\n\nclass AppState:\n    service: Service\n",
            ),
            ("service.py", &service),
        ]);
        assert_eq!(names(&cycles), expected, "{name}: cycles = {cycles:?}");
    }
}

/// The reporter's shape: a three-file ring that closes only through a runtime import.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_python_three_file_ring_closes_only_through_a_runtime_import() {
    const CASES: [(&str, &str, &[[&str; 3]]); 2] = [
        (
            "guarded",
            "if TYPE_CHECKING:\n    from pkg.main import AppState\n",
            &[],
        ),
        (
            "runtime",
            "from pkg.main import AppState\n",
            &[["adapter.py", "main.py", "service.py"]],
        ),
    ];
    for (name, adapter_import, expected) in CASES {
        let adapter = format!(
            "from typing import TYPE_CHECKING\n\n{adapter_import}\
             \nclass Adapter:\n    def bind(self, state: 'AppState'): ...\n"
        );
        let cycles = cycles_of(&[
            ("pkg/__init__.py", ""),
            (
                "pkg/main.py",
                "from pkg.service import Service\n\nclass AppState:\n    service: Service\n",
            ),
            (
                "pkg/service.py",
                "from pkg.adapter import Adapter\n\nclass Service:\n    adapter: Adapter\n",
            ),
            ("pkg/adapter.py", &adapter),
        ]);
        assert_eq!(names(&cycles), expected, "{name}: cycles = {cycles:?}");
    }
}

/// A runtime import and a guarded one for the same pair: the cycle stays.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_python_runtime_and_guarded_import_of_one_module_keep_the_cycle() {
    const CASES: [(&str, &str); 2] = [
        (
            "runtime-first",
            "from typing import TYPE_CHECKING\nfrom b import helper\n\nif TYPE_CHECKING:\n    from b import B\n",
        ),
        (
            "guard-first",
            "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    from b import B\n\nfrom b import helper\n",
        ),
    ];
    for (name, a_body) in CASES {
        let a = format!("{a_body}\ndef f(x: 'B'):\n    return helper(x)\n");
        let cycles = cycles_of(&[
            ("a.py", &a),
            (
                "b.py",
                "from a import f\n\nclass B: ...\n\ndef helper(x):\n    return f\n",
            ),
        ]);
        assert_eq!(
            names(&cycles),
            [["a.py", "b.py"]],
            "{name}: cycles = {cycles:?}"
        );
    }
}

/// A guard in one direction keeps the runtime import in the other: a.py's guarded `import
/// pkg.b` does not hide the ring a -> c -> b -> a. (Upstream's undirected build folds the pair
/// into one edge; Go keeps both directed edges, the assertion is the same.)
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_python_guard_in_one_direction_keeps_the_runtime_import_in_the_other() {
    let cycles = cycles_of(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/a.py",
            "from typing import TYPE_CHECKING\n\nfrom pkg.c import C\n\n\
             if TYPE_CHECKING:\n    import pkg.b\n\nclass A:\n    c: C\n",
        ),
        ("pkg/b.py", "from pkg.a import A\n\nclass B:\n    a: A\n"),
        ("pkg/c.py", "from pkg.b import B\n\nclass C:\n    b: B\n"),
    ]);
    assert_eq!(
        names(&cycles),
        [["a.py", "b.py", "c.py"]],
        "cycles = {cycles:?}"
    );
}
