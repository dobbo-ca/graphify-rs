//! graphify-go `internal/extract/jsts_imports_test.go`. Not ported here: tests whose subject is
//! `resolve.go`, ported with the resolve tests (graphify-rs-ihc.18): `TestJSImportJsFallsBackToTsx`,
//! `TestJSImportJsSelfSkip`, `TestJSImportJsSwapFirst`, `TestJSImportJsSelfNotExternal`,
//! `TestJSDynamicImportSelfFallback` (`importTargets` `resolve.go:1686`, `jsSelfFallback`
//! `resolve.go:1766`), `TestSFCCallDoesNotCrossToPython`, `TestSFCImportKeepsPythonCall`
//! (`resolveImportGuided` `resolve.go:1441`), `TestJSNewAmbiguous`,
//! `TestJSNewAmbiguousWithOtherNodes`, `TestJSNewSameLabel` (the `new X()` binding in `Resolve`,
//! `resolve.go:720-866`). Left out: `TestJSDynamicImportLines` needs Go `dynImport`
//! (`extract.go:554`), which `extract.rs` does not have yet.

use std::collections::BTreeSet;

use crate::extract::{file_from_bytes, resolve};
use crate::model::Extraction;

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

// `new X()` binds like a bare call, but across files only with an import.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_new_expression() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "shapes.ts",
                "export class Circle {}\nexport class Item {}\n",
            ),
            ("other.ts", "export class Item {}\nexport class Lonely {}\n"),
            ("orphan.ts", "export class Orphan {}\n"),
            ("side.ts", "export class Side {}\n"),
            (
                "main.ts",
                r#"import { Circle, Item } from './shapes';
import * as ns from './other';
class Local {}
export function mk() { return new Local(); }
export function mkNoParens() { return new Local; }
export const arrow = () => new Local();
export function mkCircle() { return new Circle(); }
export function mkItem() { return new Item(); }
export function mkLonely() { return new Lonely(); }
export function mkNs() { return new ns.Item(); }
export function mkMap() { return new Map(); }
export function mkOrphan() { return new Orphan(); }
export class Holder { make() { return new Circle(); } }
const top = new Circle();
describe('x', () => { new Circle(); new Local(); });
"#,
            ),
            (
                "effect.ts",
                "import './side';\nexport function c() { return new Side(); }\n",
            ),
            (
                "noeffect.ts",
                "export function c() { return new Side(); }\n",
            ),
            (
                "plain.js",
                "class JsThing {}\nexport function mkJs() { return new JsThing(); }\n",
            ),
            (
                "builtin.ts",
                "class Response {}\nexport function r() { return new Response(); }\n",
            ),
        ],
        &["calls"],
    );
    check_edges(
        &got,
        &[
            "main.ts:mk() calls main.ts:Local",
            "main.ts:mkNoParens() calls main.ts:Local",
            "main.ts:arrow() calls main.ts:Local",
            "main.ts:mkCircle() calls shapes.ts:Circle",
            "main.ts:mkItem() calls shapes.ts:Item", // the import picks one of two
            "main.ts:mkLonely() calls other.ts:Lonely",
            "main.ts:Holder.make() calls shapes.ts:Circle",
            "effect.ts:c() calls side.ts:Side",
            "plain.js:mkJs() calls plain.js:JsThing",
        ],
        &[
            "main.ts:mkItem() calls other.ts:Item",
            "main.ts:mkNs() calls other.ts:Item", // a namespace member is no edge
            "main.ts:mkNs() calls shapes.ts:Item",
            "main.ts:mkOrphan() calls orphan.ts:Orphan", // never imported
            "noeffect.ts:c() calls side.ts:Side",
            "builtin.ts:r() calls builtin.ts:Response",
        ],
    );
    // no edge: builtin, file level or closure
    for k in &got {
        assert!(
            !(k.starts_with("main.ts:mkMap") || k.starts_with("main.ts:main.ts calls")),
            "unexpected edge {k:?}"
        );
    }
}

// import() is a file-level dynamic_import once per target, and a deferred
// imports_from from the function holding it; require() stays imports_from.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import() {
    crate::extract::kinds::register_kinds();
    let dynsrc = r#"const top = import("./a.js");
export function f() { return import("./b"); }
export const g = async () => (await import("./c")).c;
export function t() { return import(`./d`); }
const r = require("./e");
export function lazy() { const m = require("./f"); return m; }
export function ext() { return import("lodash"); }
export function twice() { import("./b"); return import("./b", { with: {} }); }
function inner() { return import("./h"); }
export function sub(n) { return import(`./${n}`); }
"#;
    let mut srcs: Vec<(String, String)> = vec![
        ("dyn.ts".to_string(), dynsrc.to_string()),
        (
            "dyn2.js".to_string(),
            "export function jf() { return import('./a.js'); }\n".to_string(),
        ),
    ];
    for f in ["a", "b", "c", "d", "e", "f", "h"] {
        srcs.push((format!("{f}.ts"), format!("export function {f}() {{}}\n")));
    }
    let got = edge_set(&srcs, &["imports_from", "dynamic_import", "imports"]);
    check_edges(
        &got,
        &[
            "dyn.ts:dyn.ts dynamic_import a.ts:a.ts",
            "dyn.ts:dyn.ts dynamic_import b.ts:b.ts",
            "dyn.ts:dyn.ts dynamic_import c.ts:c.ts",
            "dyn.ts:dyn.ts dynamic_import d.ts:d.ts",
            "dyn.ts:dyn.ts dynamic_import h.ts:h.ts",
            "dyn.ts:dyn.ts dynamic_import :lodash",
            "dyn2.js:dyn2.js dynamic_import a.ts:a.ts",
            "dyn.ts:f() imports_from b.ts:b.ts deferred",
            "dyn.ts:twice() imports_from b.ts:b.ts deferred",
            "dyn.ts:g() imports_from c.ts:c.ts deferred",
            "dyn.ts:t() imports_from d.ts:d.ts deferred",
            "dyn.ts:inner() imports_from h.ts:h.ts deferred",
            "dyn.ts:ext() imports_from :lodash deferred",
            "dyn2.js:jf() imports_from a.ts:a.ts deferred",
            "dyn.ts:dyn.ts imports_from e.ts:e.ts",
            "dyn.ts:dyn.ts imports_from f.ts:f.ts", // require() stays at the file
        ],
        &[
            "dyn.ts:dyn.ts imports_from a.ts:a.ts",
            "dyn.ts:dyn.ts imports_from b.ts:b.ts",
            "dyn.ts:dyn.ts imports_from h.ts:h.ts",
            "dyn.ts:dyn.ts dynamic_import e.ts:e.ts",
            "dyn.ts:dyn.ts imports :lodash",
        ],
    );
    let n = got
        .iter()
        .filter(|k| k.starts_with("dyn.ts:dyn.ts dynamic_import"))
        .count();
    assert_eq!(
        n, 6,
        "dyn.ts has {n} dynamic_import edges, want 6 (a b c d h lodash)"
    );
}

// An import() in a closure of a top-level call is the file's deferred import,
// and replaces its dynamic_import. In a function it is both.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_in_top_level_closure() {
    crate::extract::kinds::register_kinds();
    let a = ("a.ts", "export function a() {}\n");
    let got = edge_set(
        &[a, ("spec.ts", "describe('x', () => { import('./a'); });\n")],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &["spec.ts:spec.ts imports_from a.ts:a.ts deferred"],
        &[
            "spec.ts:spec.ts dynamic_import a.ts:a.ts",
            "spec.ts:spec.ts imports_from a.ts:a.ts",
        ],
    );

    // closure beats function
    let got = edge_set(
        &[
            a,
            (
                "spec.ts",
                "function f() { import('./a'); }\ndescribe('x', () => { import('./a'); });\n",
            ),
        ],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "spec.ts:spec.ts imports_from a.ts:a.ts deferred",
            "spec.ts:f() imports_from a.ts:a.ts deferred",
        ],
        &["spec.ts:spec.ts dynamic_import a.ts:a.ts"],
    );
}

// Components get a dynamic_import for the file and a deferred edge for the function.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_sfc_dynamic_import() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            ("a.ts", "export function a() {}\n"),
            ("b.ts", "export function b() {}\n"),
            (
                "Comp.vue",
                "<template><div/></template>\n<script setup lang=\"ts\">\nconst A = () => import('./a.js');\n</script>\n",
            ),
            (
                "Page.svelte",
                "<script lang=\"ts\">\n  const B = import('./b');\n</script>\n<div/>\n",
            ),
            (
                "Lay.astro",
                "---\nconst m = await import('./a');\n---\n<div/>\n",
            ),
        ],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "Comp.vue:Comp.vue dynamic_import a.ts:a.ts",
            "Comp.vue:A() imports_from a.ts:a.ts deferred",
            "Page.svelte:Page.svelte dynamic_import b.ts:b.ts",
            "Lay.astro:Lay.astro dynamic_import a.ts:a.ts",
        ],
        &[
            "Comp.vue:Comp.vue imports_from a.ts:a.ts",
            "Page.svelte:Page.svelte imports_from b.ts:b.ts",
            "Lay.astro:Lay.astro imports_from a.ts:a.ts",
        ],
    );
}

// Only a lone literal in `import(` makes a file dynamic_import; a recovered
// call, a comment argument or a `$` template still defers.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_shapes() {
    crate::extract::kinds::register_kinds();
    let mut srcs: Vec<(String, String)> = vec![
        (
            "main.js".to_string(),
            "import a = require(\"./a\");\nfunction f() { import c = require(\"./c\") }\nimport.defer(\"./e\");\n".to_string(),
        ),
        (
            "m.ts".to_string(),
            r#"export function x() { return import("./a", { with: { type: "json" } }); }
export const top = import("./b", { with: {} });
export function w() { return import(/* chunk */ './c'); }
export function d() { return import(`./e$f`); }
"#
            .to_string(),
        ),
    ];
    for f in ["a", "b", "c", "e"] {
        srcs.push((format!("{f}.ts"), format!("export const {f} = 1;\n")));
    }
    let got = edge_set(&srcs, &["imports_from", "dynamic_import"]);
    check_edges(
        &got,
        &[
            "main.js:f() imports_from c.ts:c.ts deferred",
            "m.ts:x() imports_from a.ts:a.ts deferred",
            "m.ts:w() imports_from c.ts:c.ts deferred",
            "m.ts:d() imports_from :./e$f deferred",
            "main.js:main.js imports_from a.ts:a.ts", // as on main
        ],
        &[
            "main.js:main.js dynamic_import a.ts:a.ts",
            "main.js:main.js dynamic_import c.ts:c.ts",
            "main.js:main.js dynamic_import e.ts:e.ts",
            "m.ts:m.ts dynamic_import a.ts:a.ts",
            "m.ts:m.ts dynamic_import b.ts:b.ts",
            "m.ts:m.ts dynamic_import c.ts:c.ts",
        ],
    );
}

// Upstream finds dynamic_import by regex: comments and strings count, a
// line comment or a longer name does not, and a static import hides it.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_regex() {
    crate::extract::kinds::register_kinds();
    let mut srcs: Vec<(String, String)> = vec![(
        "r.ts".to_string(),
        r#"import { s } from './s';
import type { ty } from './ty';
export * from './re';
/** @type {import("./a").A} */
const str = "import('./b')";
/* import('./c') */
// import('./d')
const x = reimport('./e'), y = import('./s'), z = import('./ty'), w = import('./re');
"#
        .to_string(),
    )];
    for f in ["a", "b", "c", "d", "e", "s", "ty", "re"] {
        srcs.push((format!("{f}.ts"), format!("export const {f} = 1;\n")));
    }
    let got = edge_set(&srcs, &["imports_from", "dynamic_import", "re_exports"]);
    check_edges(
        &got,
        &[
            "r.ts:r.ts dynamic_import a.ts:a.ts",
            "r.ts:r.ts dynamic_import b.ts:b.ts",
            "r.ts:r.ts dynamic_import c.ts:c.ts",
            "r.ts:r.ts imports_from ty.ts:ty.ts",
        ],
        &[
            "r.ts:r.ts dynamic_import d.ts:d.ts",
            "r.ts:r.ts dynamic_import e.ts:e.ts",
            "r.ts:r.ts dynamic_import s.ts:s.ts",
            "r.ts:r.ts dynamic_import ty.ts:ty.ts",
            "r.ts:r.ts dynamic_import re.ts:re.ts",
        ],
    );
}

// A component's dynamic_import is its regex alone: no template literal, no
// extra argument. The script still defers.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_sfc_dynamic_import_shapes() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            ("a.ts", "export const a = 1;\n"),
            ("b.ts", "export const b = 1;\n"),
            ("c.ts", "export const c = 1;\n"),
            (
                "R.vue",
                "<script setup lang=\"ts\">\nasync function load() { await import(`./a`); await import('./b', { with: {} }); }\nfunction f() { return import(/* c */ './c') }\n</script>\n",
            ),
        ],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "R.vue:load() imports_from a.ts:a.ts deferred",
            "R.vue:load() imports_from b.ts:b.ts deferred",
            "R.vue:f() imports_from c.ts:c.ts deferred",
        ],
        &[
            "R.vue:R.vue dynamic_import a.ts:a.ts",
            "R.vue:R.vue dynamic_import b.ts:b.ts",
            "R.vue:R.vue dynamic_import c.ts:c.ts",
        ],
    );
}

// `new X()` binds an imported class by its own name, once, outside its own
// members; a local class does not hide it, as upstream has no node for one.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_new_binding() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "x.ts",
                "export class X {}\nexport class Y {}\nexport class Widget {}\nexport default class Thing {}\n",
            ),
            ("item.ts", "export class Other {}\n"),
            ("barrel.ts", "export { Other as Renamed } from './item';\n"),
            (
                "b.ts",
                r#"import { X, Y, Widget as W } from './x';
import Def from './x';
import { Renamed } from './barrel';
export class B {
  retM(): X { return new X(1); }
  noRetM() { return new Y(2); }
  paramM(x: X) { return new X(3); }
  static make() { return new B(); }
}
export function alias() { return new W(); }
export function dflt() { return new Def(); }
export function barrel() { return new Renamed(); }
export function shadowCls() { class Y {} return new Y(); }
"#,
            ),
            (
                "s.ts",
                "import { Y } from './x';\nexport function shadowFn() { function Y() {} return new Y(); }\n",
            ),
            (
                "c5a.ts",
                "const Thing1 = mk();\nfunction f1() { return new Thing1(); }\nexport namespace Evt { export const v = 1; }\nfunction bar() { return new Evt(); }\n",
            ),
            ("c5b.ts", "export class Thing1 {}\nexport class Evt {}\n"),
            (
                "e1.ts",
                "export namespace Solo1 { export const v = 1; }\nfunction baz() { return new Solo1(); }\n",
            ),
            (
                "nest.ts",
                "function o() { function Q() {} function d() { return new Q(); } function self() { return new Q(); } }\nfunction p() { function R() { return new R(); } }\n",
            ),
        ],
        &["calls"],
    );
    check_edges(
        &got,
        &[
            "b.ts:B.noRetM() calls x.ts:Y",
            "b.ts:shadowCls() calls x.ts:Y",
            "c5a.ts:f1() calls c5a.ts:Thing1",
            "c5a.ts:bar() calls c5a.ts:Evt",
            "e1.ts:baz() calls e1.ts:Solo1",
            "nest.ts:d() calls nest.ts:Q()",
            "nest.ts:self() calls nest.ts:Q()",
        ],
        &[
            "nest.ts:o() calls nest.ts:Q()",
            "nest.ts:R() calls nest.ts:R()",
            "c5a.ts:f1() calls c5b.ts:Thing1",
            "c5a.ts:bar() calls c5b.ts:Evt",
            "b.ts:B.retM() calls x.ts:X", // a references edge is there already
            "b.ts:B.paramM() calls x.ts:X",
            "b.ts:B.make() calls b.ts:B", // upstream drops the owner pair
            "b.ts:alias() calls x.ts:Widget",
            "b.ts:dflt() calls x.ts:Thing",
            "b.ts:barrel() calls item.ts:Other",
            "s.ts:shadowFn() calls x.ts:Y",
            "s.ts:shadowFn() calls s.ts:Y()",
        ],
    );
}

// A namespace class binds like a bare call, but not from its own member; a
// local class shadows nothing, and an aliased import falls back to the class
// of that name.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_new_scopes() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "ns.ts",
                r#"namespace Foo.Bar { class Greeter {} function foo() { return new Greeter(); } }
module m1 { class C2 {} export function f11() { return new C2(); } }
namespace Solo { export class Box {} export function mk() { return new Box(); } }
namespace N { export class A { m() { const f = () => new A(); } static s() { return new A(); } } export class C { n() { return new C(); } } export class D { g() { return new A(); } } }
module P.Q { export class E { static mk() { return new E(); } } }
"#,
            ),
            ("m.ts", "export class Widget {}\n"),
            (
                "u.ts",
                r#"import { Widget } from './m';
export function sh3() { if (true) { class Widget {} return new Widget(); } }
export function sh5() { { class Widget {} } return new Widget(); }
export function sh6() { const g = () => { class Widget {} return 1; }; return new Widget(); }
export function own() { class Widget {} return new Widget(); }
"#,
            ),
            (
                "lib.ts",
                "export class ChannelClient {}\nexport class IPCClient {}\n",
            ),
            (
                "use.ts",
                "import { ChannelClient as IPCClient } from './lib';\nexport function mk() { return new IPCClient(); }\n",
            ),
            ("st/a.ts", "export class Store {}\n"),
            ("st/b.ts", "export class Store {}\n"),
            (
                "two.ts",
                "import { Store as S } from './st/a';\nimport './st/b';\nexport function b2() { return new Store(); }\n",
            ),
        ],
        &["calls"],
    );
    check_edges(
        &got,
        &[
            "ns.ts:foo() calls ns.ts:Greeter",
            "ns.ts:f11() calls ns.ts:C2",
            "ns.ts:mk() calls ns.ts:Box",
            "u.ts:sh3() calls m.ts:Widget",
            "u.ts:sh5() calls m.ts:Widget",
            "u.ts:sh6() calls m.ts:Widget",
            "u.ts:own() calls m.ts:Widget",
            "ns.ts:D.g() calls ns.ts:A",
            "use.ts:mk() calls lib.ts:IPCClient",
            "two.ts:b2() calls st/a.ts:Store",
        ],
        &[
            "use.ts:mk() calls lib.ts:ChannelClient",
            "ns.ts:A.m() calls ns.ts:A",
            "ns.ts:A.s() calls ns.ts:A",
            "ns.ts:C.n() calls ns.ts:C",
            "ns.ts:E.mk() calls ns.ts:E",
        ],
    );
}

// A new X() in a closure passed to a top-level call is the file's, unless an
// imports or contains edge already links the pair; module scope makes none.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_new_file_closure() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "x/a.ts",
                "export class Foo {}\nexport class Bar {}\nexport function helper() {}\n",
            ),
            (
                "t.ts",
                "import './x/a';\nsuite('s', () => { new Foo(); });\n",
            ),
            (
                "u.ts",
                "import * as ns from './x/a';\nsuite('s', async () => { new Bar(); });\nnew Foo();\n",
            ),
            (
                "v.ts",
                "import { helper } from './x/a';\nsuite(() => { new Foo(); new Bar(); });\n",
            ),
            (
                "n.ts",
                "import { Foo } from './x/a';\nsuite('s', () => { new Foo(); });\n",
            ),
            (
                "w.ts",
                "export class Loc {}\nsuite('s', () => { new Loc(); });\n",
            ),
        ],
        &["calls"],
    );
    check_edges(
        &got,
        &[
            "t.ts:t.ts calls x/a.ts:Foo",
            "u.ts:u.ts calls x/a.ts:Bar",
            "v.ts:v.ts calls x/a.ts:Foo",
            "v.ts:v.ts calls x/a.ts:Bar",
        ],
        &[
            "u.ts:u.ts calls x/a.ts:Foo",
            "n.ts:n.ts calls x/a.ts:Foo",
            "w.ts:w.ts calls w.ts:Loc",
        ],
    );
}

// A file-level import() replaces the static edge's line, either order.
#[test]
#[ignore = "graphify-rs-ihc.26"]
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

// import(' ./d ') trims spaces inside the quotes.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_spaces() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "a.ts",
                "export function f2() { return import(' ./d2 '); }\n",
            ),
            ("d2.ts", "export const d = 1;\n"),
        ],
        &["imports_from"],
    );
    check_edges(&got, &["a.ts:f2() imports_from d2.ts:d2.ts deferred"], &[]);
}

// An external module imported statically still gets the file's dynamic_import.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_external_behind_static() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[(
            "a.ts",
            "import React from 'react';\nexport function f() { return import('react'); }\n",
        )],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "a.ts:a.ts dynamic_import :react",
            "a.ts:f() imports_from :react deferred",
        ],
        &[],
    );
}

// A file-level deferred import marks the static one of the pair, whatever the order.
#[test]
#[ignore = "graphify-rs-ihc.26"]
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
#[ignore = "graphify-rs-ihc.26"]
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

// import("/x") names no package: only the function defers it.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_absolute() {
    crate::extract::kinds::register_kinds();
    let got = edge_set(
        &[
            (
                "a.ts",
                "/** @type {typeof import(\"/some-mod\")} */\nconst top = import(\"/abs/top\");\nexport function f() { return import(\"/abs/inner\"); }\n",
            ),
            (
                "R.vue",
                "<script setup lang=\"ts\">\nfunction g() { return import('/abs/vue'); }\n</script>\n",
            ),
        ],
        &["imports_from", "dynamic_import"],
    );
    check_edges(
        &got,
        &[
            "a.ts:f() imports_from :/abs/inner deferred",
            "R.vue:g() imports_from :/abs/vue deferred",
        ],
        &[
            "a.ts:a.ts dynamic_import :/some-mod",
            "a.ts:a.ts dynamic_import :/abs/top",
            "a.ts:a.ts dynamic_import :/abs/inner",
            "R.vue:R.vue dynamic_import :/abs/vue",
        ],
    );
}

// Upstream's closure walk skips the params and type of a nested callable, so
// an import() there gives the file its dynamic_import and no deferred edge.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_nested_signature() {
    crate::extract::kinds::register_kinds();
    let main = r#"declare function setup(f: () => void): void;
declare function stub(o: any): void;
setup(() => {
  stub({ get(m: import('./a.js').Ta) { return 1; } });
  stub({ get(): import('./c.js').Tc | undefined { return undefined; } });
  class K { m(): import('./d.js').Td | undefined { return undefined; } }
  class L { constructor(p: import('./e.js').Te) {} }
  function inner(x: import('./f.js').Tf) { return import('./g.js'); }
  const g = (t: import('./b.js').Tb[]) => t;
});
export function f() {
  function nested(x = import('./h.js')) { return x; }
}
export let sys = (() => { function nf() { return import('./i1.js'); } const o = { om() { return import('./i2.js'); } }; class K { cm() { return import('./i3.js'); } } function* g() { yield import('./i4.js'); } })();
"#;
    let mut srcs: Vec<(String, String)> = vec![("main.ts".to_string(), main.to_string())];
    for f in [
        "a", "b", "c", "d", "e", "f", "g", "h", "i1", "i2", "i3", "i4",
    ] {
        srcs.push((format!("{f}.ts"), format!("export const {f} = 1;\n")));
    }
    let got = edge_set(&srcs, &["imports_from", "dynamic_import"]);
    check_edges(
        &got,
        &[
            "main.ts:main.ts imports_from b.ts:b.ts deferred", // arrow params stay
            "main.ts:inner() imports_from g.ts:g.ts deferred", // bodies stay
            "main.ts:main.ts dynamic_import a.ts:a.ts",
            "main.ts:main.ts dynamic_import c.ts:c.ts",
            "main.ts:main.ts dynamic_import d.ts:d.ts",
            "main.ts:main.ts dynamic_import e.ts:e.ts",
            "main.ts:main.ts dynamic_import f.ts:f.ts",
            "main.ts:main.ts dynamic_import h.ts:h.ts",
            "main.ts:sys imports_from i1.ts:i1.ts deferred", // var IIFE body stays
            "main.ts:sys imports_from i4.ts:i4.ts deferred",
        ],
        &[
            "main.ts:sys imports_from i2.ts:i2.ts deferred",
            "main.ts:sys imports_from i3.ts:i3.ts deferred",
            "main.ts:main.ts imports_from a.ts:a.ts deferred",
            "main.ts:main.ts imports_from c.ts:c.ts deferred",
            "main.ts:main.ts imports_from d.ts:d.ts deferred",
            "main.ts:main.ts imports_from e.ts:e.ts deferred",
            "main.ts:main.ts imports_from f.ts:f.ts deferred",
            "main.ts:f() imports_from h.ts:h.ts deferred",
        ],
    );
}

// A generic call that tree-sitter misreads as a comparison has its import()
// type masked upstream, so only the regex pass sees it.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_masked_type_args() {
    crate::extract::kinds::register_kinds();
    let s = r#"declare function foo<T>(...a: any[]): Promise<T>;
declare function suite(n: string, f: () => void): void;
export function a1() { return foo<typeof import("./t1")>(); }
export async function a2() { return await foo<typeof import('./t2')>(1, ''); }
export function a3() { return foo<import("./t3").X>(); }
export function a4() { return new Map<string, typeof import("./t4")>(); }
suite("x", () => { foo<typeof import("./u")>(); });
export function n1() { return foo<(typeof import('./a'))>(); }
export function n3() { return foo<[typeof import('./c')]>(); }
export function m12() { return foo<(typeof import('./l'))['vl']>(); }
"#;
    let mut srcs: Vec<(String, String)> = vec![("s.ts".to_string(), s.to_string())];
    for f in ["t1", "t2", "t3", "t4", "u", "a", "c", "l"] {
        srcs.push((format!("{f}.ts"), format!("export const {f} = 1;\n")));
    }
    let got = edge_set(&srcs, &["imports_from", "dynamic_import"]);
    check_edges(
        &got,
        &[
            "s.ts:s.ts dynamic_import t1.ts:t1.ts",
            "s.ts:s.ts dynamic_import t3.ts:t3.ts",
            "s.ts:s.ts dynamic_import u.ts:u.ts",
            "s.ts:a2() imports_from t2.ts:t2.ts deferred",
            "s.ts:a4() imports_from t4.ts:t4.ts deferred",
        ],
        &[
            "s.ts:a1() imports_from t1.ts:t1.ts deferred",
            "s.ts:a3() imports_from t3.ts:t3.ts deferred",
            "s.ts:s.ts imports_from u.ts:u.ts deferred",
            "s.ts:n1() imports_from a.ts:a.ts deferred",
            "s.ts:n3() imports_from c.ts:c.ts deferred",
            "s.ts:m12() imports_from l.ts:l.ts deferred",
        ],
    );
}

// Upstream's dynamic_import carries no location.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_no_location() {
    crate::extract::kinds::register_kinds();
    let ext = resolve_srcs(&[
        ("a.ts", "\nexport const x = import('./b');\n"),
        ("b.ts", "export const b = 1;\n"),
    ]);
    let mut n = 0;
    for e in &ext.edges {
        if e.relation.as_str() == "dynamic_import" {
            n += 1;
            assert_eq!(
                e.source_location.0, 0,
                "dynamic_import has location {}",
                e.source_location
            );
        }
    }
    assert_eq!(n, 1, "got {n} dynamic_import edges, want 1");
}
