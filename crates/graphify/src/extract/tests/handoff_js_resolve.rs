//! Handoff from the runtime stream (branch `handoff-js-resolve-tests-5e21`): the Go tests of the
//! JS/TS files whose subject is `resolve.go`: `modulepath_test.go`, `javascript_alias_test.go`,
//! and from `jsts_imports_test.go` and `jsts_parity_test.go` the ones that test `importTargets`, `jsSelfFallback`, `resolveImportGuided`, the `new X()` binding and the
//! import-edge merge. They parse JS/TS, so they pass with graphify-rs-ihc.26 and resolve
//! (graphify-rs-ihc.18); `test_resolve_module_path` parses nothing.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::resolve::Corpus;

use crate::extract::Result as Res;
use crate::extract::{file_from_bytes, resolve};
use crate::model::Extraction;

type Nodes = BTreeSet<(String, String)>;
type Edges = BTreeSet<(String, String, String)>;

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Go `Resolve` over each `(rel, src)` extracted in order, with GOMAXPROCS workers.
fn resolve_srcs<A: AsRef<str>, B: AsRef<str>>(srcs: &[(A, B)]) -> Extraction {
    let results: Vec<_> = srcs
        .iter()
        .map(|(f, s)| file_from_bytes(f.as_ref(), s.as_ref().as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(f, _)| f.as_ref().to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

/// Go `edgeSet`: names each edge "file:label relation file:label", with " deferred" after a
/// deferred one, for the relations that tests below look at.
fn edge_set<A: AsRef<str>, B: AsRef<str>>(srcs: &[(A, B)], rels: &[&str]) -> BTreeSet<String> {
    let ext = resolve_srcs(srcs);
    let name = |id: &str| {
        ext.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| format!("{}:{}", n.source_file, n.label))
            .unwrap_or_else(|| ":".to_string())
    };
    let mut out = BTreeSet::new();
    for e in &ext.edges {
        if !rels.contains(&e.relation.as_str()) {
            continue;
        }
        let mut k = format!(
            "{} {} {}",
            name(&e.source),
            e.relation.as_str(),
            name(&e.target)
        );
        if e.deferred() {
            k.push_str(" deferred");
        }
        out.insert(k);
    }
    out
}

/// Go `checkEdges`.
fn check_edges(got: &BTreeSet<String>, want: &[&str], not: &[&str]) {
    for w in want {
        assert!(got.contains(*w), "missing edge {w:?}; got {got:?}");
    }
    for n in not {
        assert!(!got.contains(*n), "unexpected edge {n:?}");
    }
}

/// Go `resolveSrc` (`nodeid_test.go:6`): extracts each in-memory file and resolves the set.
fn resolve_src(srcs: &[(&str, &str)]) -> (Nodes, Edges) {
    crate::extract::kinds::register_kinds();
    let mut results = Vec::new();
    let mut files = Vec::new();
    for (rel, src) in srcs {
        // "go.mod" names the module of the corpus root.
        if *rel == "go.mod" {
            results.push(Res {
                go_mods: BTreeMap::from([(".".to_string(), (*src).to_string())]),
                ..Res::default()
            });
            continue;
        }
        results.push(file_from_bytes(rel, src.as_bytes()));
        files.push((*rel).to_string());
    }
    let ext = resolve(&results, &files, workers());
    let mut nodes = Nodes::new(); // source file, label
    let mut at: BTreeMap<&str, (String, String)> = BTreeMap::new();
    for n in &ext.nodes {
        let k = (n.source_file.clone(), n.label.clone());
        nodes.insert(k.clone());
        at.insert(n.id.as_str(), k);
    }
    let mut edges = Edges::new(); // source file/label, relation, target file/label
    for e in &ext.edges {
        let s = at.get(e.source.as_str()).cloned().unwrap_or_default();
        let d = at.get(e.target.as_str()).cloned().unwrap_or_default();
        edges.insert((
            format!("{}:{}", s.0, s.1),
            e.relation.as_str().to_string(),
            format!("{}:{}", d.0, d.1),
        ));
    }
    (nodes, edges)
}

fn has_edge(edges: &Edges, e: &(&str, &str, &str)) -> bool {
    edges.contains(&(e.0.to_string(), e.1.to_string(), e.2.to_string()))
}

// Go `resolveModulePath` (`resolve.go:1877`); it returns "" when nothing matches. No grammar is
// parsed, so resolve alone makes it pass.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_module_path() {
    let corpus: Corpus = [
        "a/exact",
        "a/x.ts",
        "a/x.js",
        "a/y.d.ts",
        "a/dir/index.tsx",
        "a/both.js",
        "a/both/index.ts",
        "a/z.d.ts/index.js",
        "a/last/index.d.ts",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for (base, want) in [
        ("a/exact", "a/exact"),
        ("a/x", "a/x.ts"), // extension order, not corpus order
        ("a/y", "a/y.d.ts"),
        ("a/dir", "a/dir/index.tsx"),
        ("a/both", "a/both.js"), // a file beats a directory index
        ("a/last", "a/last/index.d.ts"),
        ("a/z", ""), // the index probe starts from the bare path again
        ("a/none", ""),
        ("", ""),
    ] {
        let got = crate::extract::resolve::resolve_module_path(base, &corpus);
        assert_eq!(
            got, want,
            "resolveModulePath({base:?}) = {got:?}, want {want:?}"
        );
    }
}

// `@/lib/x` is the project-root alias convention: it must resolve to the corpus
// file lib/x.ts (imports_from), not become an external dependency node.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_resolve_at_slash_alias_import() {
    crate::extract::kinds::register_kinds();
    let ext = resolve_srcs(&[
        (
            "app/page.tsx",
            "import { x } from '@/lib/x';\nimport React from 'react';\n",
        ),
        ("lib/x.ts", "export const x = 1;\n"),
    ]);

    let from_id = crate::idutil::make_id(&["app/page.tsx"]);
    let to_id = crate::idutil::make_id(&["lib/x.ts"]);
    let mut found = false;
    for e in &ext.edges {
        if e.relation.as_str() == "imports_from" && e.source == from_id && e.target == to_id {
            found = true;
        }
        assert!(
            !(e.relation.as_str() == "imports"
                && e.source == from_id
                && e.target == crate::idutil::make_id(&["@/lib/x"])),
            "@/lib/x became an external import edge"
        );
    }
    assert!(found, "expected app/page.tsx --imports_from--> lib/x.ts");

    // A genuinely bare specifier still becomes an external node.
    let bare = ext
        .nodes
        .iter()
        .any(|n| n.label == "react" && n.file_type == "concept");
    assert!(bare, "expected external concept node for react");
}

// A .js specifier names a .ts or .tsx file; a real .js file wins.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_import_js_falls_back_to_tsx() {
    crate::extract::kinds::register_kinds();
    let vue = "<script setup lang=\"ts\">\nimport { Button } from './Button.js'\n</script>\n";
    let (_, edges) = resolve_src(&[
        ("Button.tsx", "export function Button() {}\n"),
        ("Widget.tsx", "export function render() {}\n"),
        ("other.ts", "export function render() {}\n"),
        ("Comp.tsx", "export function Comp() {}\n"),
        ("both.ts", "export function bothTs() {}\n"),
        ("both.tsx", "export function bothTsx() {}\n"),
        ("real.js", "export function realJs() {}\n"),
        ("real.ts", "export function realTs() {}\n"),
        ("jsx.ts", "export function jsxTs() {}\n"),
        ("barrel.ts", "export * from './Comp.js';\n"),
        (
            "main.ts",
            r#"import { Button } from './Button.js';
import { render } from './Widget.js';
import { bothTs } from './both.js';
import { realJs } from './real.js';
import { jsxTs } from './jsx.jsx';
export function main() { Button(); render(); bothTs(); realJs(); jsxTs() }
"#,
        ),
        ("plain.js", "import { Button } from './Button.js';\n"),
        ("App.vue", vue),
    ]);
    for want in [
        ("main.ts:main.ts", "imports_from", "Button.tsx:Button.tsx"),
        ("main.ts:main.ts", "imports_from", "Widget.tsx:Widget.tsx"),
        ("main.ts:main.ts", "imports_from", "both.ts:both.ts"),
        ("main.ts:main.ts", "imports_from", "real.js:real.js"),
        ("plain.js:plain.js", "imports_from", "Button.tsx:Button.tsx"),
        ("App.vue:App.vue", "imports_from", "Button.tsx:Button.tsx"),
        ("barrel.ts:barrel.ts", "re_exports", "Comp.tsx:Comp.tsx"),
        // the alias binds, not a tie-break
        ("main.ts:main()", "calls", "Widget.tsx:render()"),
    ] {
        assert!(has_edge(&edges, &want), "missing edge {want:?}");
    }
    for not in [
        ("main.ts:main.ts", "imports_from", "both.tsx:both.tsx"),
        ("main.ts:main.ts", "imports_from", "real.ts:real.ts"),
        ("main.ts:main.ts", "imports_from", "jsx.ts:jsx.ts"), // .jsx never maps to .ts
    ] {
        assert!(!has_edge(&edges, &not), "unexpected edge {not:?}");
    }
}

// './self.js' in self.ts never falls through to the self.tsx sibling.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_import_js_self_skip() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "self.ts",
                "import { X } from './self.js';\nexport class X {}\nexport function s() { return new X(); }\n",
            ),
            ("self.tsx", "export class Z {}\n"),
        ],
        &["imports_from", "imports", "calls"],
    );
    check_edges(
        &got,
        &["self.ts:s() calls self.ts:X"],
        &[
            "self.ts:self.ts imports_from self.tsx:self.tsx",
            "self.ts:s() calls self.tsx:X",
        ],
    );
}

// './x.js' is x.ts before x.js.ts or x.js/index.ts, as upstream tries it.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_import_js_swap_first() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            ("lib/x.ts", "export class A {}\n"),
            ("lib/x.js.ts", "export class A {}\n"),
            ("lib2/y.ts", "export class B {}\n"),
            ("lib2/y.js/index.ts", "export class B {}\n"),
            ("m.ts", "import './lib/x.js';\nimport './lib2/y.js';\n"),
        ],
        &["imports_from"],
    );
    check_edges(
        &got,
        &[
            "m.ts:m.ts imports_from lib/x.ts:x.ts",
            "m.ts:m.ts imports_from lib2/y.ts:y.ts",
        ],
        &[
            "m.ts:m.ts imports_from lib/x.js.ts:x.js.ts",
            "m.ts:m.ts imports_from lib2/y.js/index.ts:index.ts",
        ],
    );
}

// A .js specifier that falls back to its own file is resolved, not external.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_import_js_self_not_external() {
    crate::extract::kinds::register_kinds();
    let srcs = [
        (
            "self.ts",
            "import './self.js';\nexport async function s() { return import('./self.js'); }\n",
        ),
        ("self.tsx", "export class Z {}\n"),
        ("me.ts", "export class M {}\n"),
        ("me.tsx", "import './me.jsx';\n"),
    ];
    for n in &resolve_srcs(&srcs).nodes {
        assert!(!n.label.contains(".js"), "external stub node {:?}", n.label);
    }
    let got = edge_set(&srcs, &["imports", "imports_from", "dynamic_import"]);
    check_edges(
        &got,
        &[],
        &[
            "self.ts:self.ts imports :./self.js",
            "me.tsx:me.tsx imports :./me.jsx",
            "self.ts:self.ts imports_from self.tsx:self.tsx",
            "self.ts:self.ts imports_from self.ts:self.ts",
            "me.tsx:me.tsx imports_from me.tsx:me.tsx",
        ],
    );
}

// Only the static './x.js' self import is dropped: import() of the file
// itself still gives the file's dynamic_import and the function's deferred edge.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_dynamic_import_self_fallback() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "self.ts",
                "export function s1() { return import('./self.js'); }\n",
            ),
            (
                "tx.tsx",
                "export function s2() { return import('./tx.jsx'); }\n",
            ),
            (
                "st.ts",
                "import './st.js';\nexport function s3() { return import('./st.js'); }\n",
            ),
        ],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "self.ts:self.ts dynamic_import self.ts:self.ts",
            "self.ts:s1() imports_from self.ts:self.ts deferred",
            "tx.tsx:tx.tsx dynamic_import tx.tsx:tx.tsx",
            "tx.tsx:s2() imports_from tx.tsx:tx.tsx deferred",
            "st.ts:st.ts dynamic_import st.ts:st.ts",
        ],
        &["st.ts:st.ts imports_from st.ts:st.ts"],
    );
}

// A call from a component never binds to a Python definition.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_sfc_call_does_not_cross_to_python() {
    crate::extract::kinds::register_kinds();
    let (_, edges) = resolve_src(&[
        (
            "App.vue",
            "<script setup lang=\"ts\">\nfunction run() { processData() }\n</script>\n",
        ),
        ("util.py", "def processData():\n    pass\n"),
    ]);
    for e in &edges {
        assert!(
            !(e.1 == "calls" && e.0.starts_with("App")),
            "unexpected edge {e:?}"
        );
    }
}

// A component keeps an import and a call bound through it, whatever the language.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_sfc_import_keeps_python_call() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            ("util.py", "def helper(): pass\n"),
            (
                "App.vue",
                "<script setup lang=\"ts\">\nimport { helper } from './util.py';\nfunction run() { helper(); other(); }\n</script>\n",
            ),
            ("o.py", "def other(): pass\n"),
        ],
        &["imports_from", "calls"],
    );
    check_edges(
        &got,
        &[
            "App.vue:App.vue imports_from util.py:util.py",
            "App.vue:run() calls util.py:helper()",
        ],
        &["App.vue:run() calls o.py:other()"],
    );
}

// An ambiguous new X() binds only by a unique symbol import, then by a unique
// imported file; never by directory, and never past a second symbol import.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_new_ambiguous() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            ("a.ts", "export class X {}\n"),
            ("sub/b.ts", "export class X {}\n"),
            (
                "c.ts",
                "import './a';\nimport './sub/b';\nexport function f() { return new X(); }\n",
            ),
            (
                "d.ts",
                "import * as A from './a';\nimport * as B from './sub/b';\nexport function g() { return new X(); }\n",
            ),
            (
                "sub/s.ts",
                "import '../a';\nimport './b';\nexport function k() { return new X(); }\n",
            ),
            (
                "e.ts",
                "import './sub/b';\nexport function h() { return new X(); }\n",
            ),
            ("st/a.ts", "export class Store {}\n"),
            ("st/b.ts", "export class Store {}\n"),
            (
                "u1.ts",
                "import { Store } from './st/a';\nimport { Store as S2 } from './st/b';\nexport function f1() { return new Store(); }\n",
            ),
            (
                "u2.ts",
                "import { Store } from './st/a';\nimport type { Store as T2 } from './st/b';\nexport function g2() { return new Store(); }\n",
            ),
            (
                "u3.ts",
                "import { Store } from './st/a';\nexport function h3() { return new Store(); }\n",
            ),
        ],
        &["calls"],
    );
    check_edges(
        &got,
        &[
            "e.ts:h() calls sub/b.ts:X",
            "u3.ts:h3() calls st/a.ts:Store",
        ],
        &[
            "c.ts:f() calls a.ts:X",
            "c.ts:f() calls sub/b.ts:X",
            "d.ts:g() calls a.ts:X",
            "d.ts:g() calls sub/b.ts:X",
            "sub/s.ts:k() calls a.ts:X",
            "sub/s.ts:k() calls sub/b.ts:X",
            "u1.ts:f1() calls st/a.ts:Store",
            "u1.ts:f1() calls st/b.ts:Store",
            "u2.ts:g2() calls st/a.ts:Store",
            "u2.ts:g2() calls st/b.ts:Store",
        ],
    );
}

// Namespaces, values and class members of the same name make a new X() ambiguous.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_new_ambiguous_with_other_nodes() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "s1.ts",
                "export class A {}\nexport class D {}\nexport class E {}\nexport class Order {}\nexport class User {}\nexport class Solo {}\nexport namespace M { export const z = 1; }\n",
            ),
            (
                "t.ts",
                "export namespace A { export const x = 1; }\nexport const D = 1;\nexport class K { E() {} }\n",
            ),
            ("api.ts", "export class Client { User() { return 1; } }\n"),
            ("m.ts", "export class M {}\n"),
            (
                "u.ts",
                "import * as S from './s1';\nimport * as T from './t';\nimport * as Q from './m';\nexport function f() { new A(); new D(); new E(); new M(); }\n",
            ),
            (
                "w.ts",
                "import { A, D } from './s1';\nimport './t';\nexport function h() { new A(); new D(); new E(); }\n",
            ),
            (
                "svc.ts",
                "import { Order as O } from './s1';\nimport { Client } from './api';\nexport function make() { new User(); new Order(); return new Client(); }\n",
            ),
            (
                "solo.ts",
                "import { Solo } from './s1';\nexport function k() { return new Solo(); }\n",
            ),
        ],
        &["calls"],
    );
    check_edges(
        &got,
        &[
            "svc.ts:make() calls api.ts:Client",
            "solo.ts:k() calls s1.ts:Solo",
            "w.ts:h() calls s1.ts:A",
            "w.ts:h() calls s1.ts:D",
        ],
        &[
            "u.ts:f() calls s1.ts:A",
            "u.ts:f() calls s1.ts:D",
            "u.ts:f() calls s1.ts:E",
            "u.ts:f() calls m.ts:M",
            "u.ts:f() calls s1.ts:M",
            "w.ts:h() calls s1.ts:E",
            "svc.ts:make() calls s1.ts:User",
        ],
    );
}

// A same-file new X() binds the last node labelled X, as upstream's label
// map does; a namespace twin of a plain class is no candidate.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_new_same_label() {
    crate::extract::kinds::register_kinds();
    // (name, file, src, from, to)
    for (name, file, src, from, to) in [
        (
            "ns twin",
            "m1.ts",
            "class Repo {}\nnamespace inner { export class Repo {} }\nfunction use() { return new Repo(); }\n",
            "m1_ts_use",
            "m1_ts_repo",
        ),
        (
            "nested after top",
            "a15.ts",
            "function T15() {}\nexport function o15() { function T15() {} return new T15(); }\nexport function p15() { return new T15(); }\n",
            "a15_ts_p15",
            "a15_ts_o15_t15",
        ),
        (
            "nested after top js",
            "j3.js",
            "function T3() {}\nfunction o3() { function T3() {} return new T3(); }\nfunction p3() { return new T3(); }\n",
            "j3_js_p3",
            "j3_js_o3_t3",
        ),
        (
            "method after class",
            "a1.ts",
            "class Foo1 {}\nclass Other1 { Foo1() { return 1; } }\nexport function h1() { return new Foo1(); }\n",
            "a1_ts_h1",
            "a1_ts_other1_foo1",
        ),
        (
            "nested fn after class",
            "a2.ts",
            "class Foo2 {}\nfunction g2() { function Foo2() {} return Foo2; }\nexport function h2() { return new Foo2(); }\n",
            "a2_ts_h2",
            "a2_ts_g2_foo2",
        ),
        (
            "caller nested, top last",
            "n3.ts",
            "export function o3() { function Q3() {} return new Q3(); }\nexport function Q3() {}\n",
            "n3_ts_o3",
            "n3_ts_q3",
        ),
        (
            "nested outer, top last",
            "n2.ts",
            "export function o2() { function Q2() {} function d2() { return new Q2(); } }\nexport function Q2() {}\n",
            "n2_ts_o2_d2",
            "n2_ts_q2",
        ),
    ] {
        let ext = resolve_srcs(&[(file, src)]);
        let mut got = "";
        for e in &ext.edges {
            if e.relation.as_str() == "calls" && e.source == from {
                got = e.target.as_str();
            }
        }
        assert_eq!(got, to, "{name}: {from} calls {got:?}, want {to:?}");
    }
    // the nested def is last, and contains links it
    let ext = resolve_srcs(&[(
        "q.ts",
        "export function Q2() {}\nexport function o2() { function Q2() {} return new Q2(); }\n",
    )]);
    for e in &ext.edges {
        assert!(
            !(e.relation.as_str() == "calls" && e.source == "q_ts_o2"),
            "o2 calls {:?}, want no edge",
            e.target
        );
    }
}

// `./a.js` names a.ts, and a cycle of `export *` barrels ends.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_import_js_specifier_and_barrel_cycle() {
    crate::extract::kinds::register_kinds();
    let mut srcs: Vec<(String, String)> = vec![
        ("a.ts".to_string(), "export class A {}\n".to_string()),
        (
            "b.ts".to_string(),
            "import { A } from './a.js';\nexport class B extends A {}\n".to_string(),
        ),
    ];
    // 40 barrels that all re-export each other, and an importer of unknown names.
    let mut names = String::new();
    for i in 0..40 {
        let mut body = String::new();
        for j in 0..40 {
            if j != i {
                body.push_str(&format!("export * from './m{j}';\n"));
            }
        }
        srcs.push((format!("m{i}.ts"), body));
        names.push_str(&format!("x{i}, "));
    }
    srcs.push((
        "use.ts".to_string(),
        format!("import {{ {names}}} from './m0';\n"),
    ));
    srcs.push((
        "self.ts".to_string(),
        "import { S } from './self.js';\nexport class S {}\n".to_string(),
    ));
    srcs.push(("c.mts".to_string(), "export class C {}\n".to_string()));
    srcs.push((
        "d.ts".to_string(),
        "import { C } from './c.mjs';\n".to_string(),
    ));
    let refs: Vec<(&str, &str)> = srcs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let (_, edges) = resolve_src(&refs);
    for want in [
        ("b.ts:b.ts", "imports", "a.ts:A"),
        ("b.ts:B", "inherits", "a.ts:A"),
    ] {
        assert!(has_edge(&edges, &want), "missing edge {want:?}");
    }
    // upstream pairs only .js/.ts and .jsx/.tsx, and a file does not import itself
    for not in [
        ("self.ts:self.ts", "imports_from", "self.ts:self.ts"),
        ("d.ts:d.ts", "imports_from", "c.mts:c.mts"),
    ] {
        assert!(!has_edge(&edges, &not), "unexpected edge {not:?}");
    }
}

// A file-level import() replaces the static edge's line, either order.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_deferred_merge_takes_import_site() {
    crate::extract::kinds::register_kinds();
    let mut srcs: Vec<(String, String)> = vec![
        (
            "s1.ts".to_string(),
            "import {y1} from './y1';\nsetup(() => import('./y1'));\n".to_string(),
        ),
        (
            "s2.ts".to_string(),
            "setup(() => import('./y2'));\nimport {y2} from './y2';\n".to_string(),
        ),
        (
            "s3.ts".to_string(),
            "import type {y3} from './y3';\nsetup(() => import('./y3'));\n".to_string(),
        ),
        (
            "s4.ts".to_string(),
            "setup(() => import('./y4'));\nimport type {y4} from './y4';\n".to_string(),
        ),
    ];
    for y in ["y1", "y2", "y3", "y4"] {
        srcs.push((format!("{y}.ts"), "export const y = 1;\n".to_string()));
    }
    let line = [
        ("s1.ts", "L2"),
        ("s2.ts", "L1"),
        ("s3.ts", "L2"),
        ("s4.ts", "L1"),
    ];
    let mut n = 0;
    for e in &resolve_srcs(&srcs).edges {
        if e.relation.as_str() == "imports_from" && e.deferred() {
            let f = e.source_file.as_str();
            let want = line.iter().find(|(k, _)| *k == f).map_or("", |(_, v)| *v);
            assert_eq!(
                e.source_location.to_string(),
                want,
                "{f}: location, want {want}"
            );
            n += 1;
        }
    }
    assert_eq!(n, 4, "got {n} deferred edges, want 4");
}

// A file-level deferred import marks the static one of the pair, whatever the order.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_deferred_beats_static_either_order() {
    crate::extract::kinds::register_kinds();
    let b = ("b.js", "const a = require('./a');\n");
    for a in [
        "describe('x', () => { import('./b'); });\nconst b = require('./b');\n",
        "const b = require('./b');\ndescribe('x', () => { import('./b'); });\n",
        "import './b';\ndescribe('x', () => { import('./b'); });\n",
    ] {
        let got = edge_set(&[("a.js", a), b], &["imports_from"]);
        check_edges(
            &got,
            &["a.js:a.js imports_from b.js:b.js deferred"],
            &["a.js:a.js imports_from b.js:b.js"],
        );
    }
}

// A static import of the target hides the file's dynamic_import.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_js_dynamic_import_behind_static_import() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            ("a.ts", "export const a = 1;\n"),
            (
                "s.ts",
                "import './a';\nexport function g() { return import('./a'); }\n",
            ),
            (
                "late.ts",
                "export function g() { return import('./a'); }\nimport './a';\n",
            ),
        ],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "s.ts:g() imports_from a.ts:a.ts deferred",
            "late.ts:late.ts imports_from a.ts:a.ts",
        ],
        &[
            "s.ts:s.ts dynamic_import a.ts:a.ts",
            "late.ts:late.ts dynamic_import a.ts:a.ts",
        ],
    );
}
