//! graphify-go `internal/extract/jsts_parity_test.go`. (`TestJSImportJsSpecifierAndBarrelCycle`
//! tests `resolveModulePath` in `resolve.go` and is ported with the resolve tests.)

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::Result as Res;
use crate::extract::{file_from_bytes, resolve};

type Nodes = BTreeSet<(String, String)>;
type Edges = BTreeSet<(String, String, String)>;

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
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

fn has_node(nodes: &Nodes, file: &str, label: &str) -> bool {
    nodes.contains(&(file.to_string(), label.to_string()))
}

fn has_edge(edges: &Edges, e: &(&str, &str, &str)) -> bool {
    edges.contains(&(e.0.to_string(), e.1.to_string(), e.2.to_string()))
}

fn refs2(v: &[(String, String)]) -> Vec<(&str, &str)> {
    v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
}

fn refs3(v: &[(String, String, String)]) -> Vec<(&str, &str, &str)> {
    v.iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect()
}

/// Go `wantGraph` (`jsts_parity_test.go:8`): every `wn` node and `we` edge is there and no `no`
/// edge is. (Go reports every miss with `t.Errorf`; this fails once with all of them.)
fn want_graph(
    nodes: &Nodes,
    edges: &Edges,
    wn: &[(&str, &str)],
    we: &[(&str, &str, &str)],
    no: &[(&str, &str, &str)],
) {
    let mut errs = Vec::new();
    for n in wn {
        if !has_node(nodes, n.0, n.1) {
            errs.push(format!("missing node {n:?}"));
        }
    }
    for e in we {
        if !has_edge(edges, e) {
            errs.push(format!("missing edge {e:?}"));
        }
    }
    for e in no {
        if has_edge(edges, e) {
            errs.push(format!("unexpected edge {e:?}"));
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

// callTargets lists "file:line" of what the caller label calls.
fn call_targets(srcs: &[(&str, &str)], file: &str, caller: &str) -> BTreeSet<String> {
    crate::extract::kinds::register_kinds();
    let mut results = Vec::new();
    let mut files = Vec::new();
    for s in srcs {
        results.push(file_from_bytes(s.0, s.1.as_bytes()));
        files.push(s.0.to_string());
    }
    let ext = resolve(&results, &files, workers());
    let mut by_id: BTreeMap<&str, String> = BTreeMap::new();
    let mut from: Vec<&str> = Vec::new();
    for n in &ext.nodes {
        by_id.insert(
            n.id.as_str(),
            format!("{}:{}", n.source_file, n.source_location),
        );
        if n.source_file == file && n.label == caller {
            from.push(n.id.as_str());
        }
    }
    let mut out = BTreeSet::new();
    for e in &ext.edges {
        if e.relation.as_str() == "calls" && from.contains(&e.source.as_str()) {
            out.insert(by_id.get(e.target.as_str()).cloned().unwrap_or_default());
        }
    }
    out
}

fn want_calls(srcs: &[(&str, &str)], file: &str, caller: &str, want: &[&str]) {
    let got = call_targets(srcs, file, caller);
    assert!(
        got.len() == want.len() && want.iter().all(|w| got.contains(*w)),
        "{caller} calls {got:?}, want {want:?}"
    );
}

// Nested declarations are nodes under their parent and own their calls.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_nested_functions() {
    let src = r#"function helper() {}
function outer() {
  function inner() { helper(); }
  inner();
  const cb = () => { function deep() { helper(); } deep(); };
  after();
}
function after() {}
const arrow = () => { function inArrow() { helper(); } inArrow(); };
class K { m() { function inMethod() { helper(); } inMethod(); } }
"#;
    let (nodes, edges) = resolve_src(&[("n.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[
            ("n.ts", "inner()"),
            ("n.ts", "deep()"),
            ("n.ts", "inArrow()"),
            ("n.ts", "inMethod()"),
        ],
        &[
            ("n.ts:outer()", "contains", "n.ts:inner()"),
            ("n.ts:outer()", "contains", "n.ts:deep()"),
            ("n.ts:arrow()", "contains", "n.ts:inArrow()"),
            ("n.ts:K.m()", "contains", "n.ts:inMethod()"),
            ("n.ts:inner()", "calls", "n.ts:helper()"),
            ("n.ts:deep()", "calls", "n.ts:helper()"),
            ("n.ts:inArrow()", "calls", "n.ts:helper()"),
            ("n.ts:inMethod()", "calls", "n.ts:helper()"),
            ("n.ts:outer()", "calls", "n.ts:inner()"),
            ("n.ts:outer()", "calls", "n.ts:after()"), // after inner ends
        ],
        &[
            ("n.ts:outer()", "calls", "n.ts:helper()"),
            ("n.ts:arrow()", "calls", "n.ts:helper()"),
            ("n.ts:K.m()", "calls", "n.ts:helper()"),
        ],
    );
}

// Declarations behind a wrapper are file-level; closure calls go to the
// nearest tracked body, or the file for a top-level callback.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_wrapped_declarations() {
    let src = r#"function helper() {}
(function () { function inIife() { helper(); } })();
if (x) { function inIf() {} }
describe("x", () => { helper(); function inCb() { helper(); } class InCls {} });
function f() { [1].map(() => helper()); }
"#;
    let (nodes, edges) = resolve_src(&[("w.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[
            ("w.ts", "inIife()"),
            ("w.ts", "inIf()"),
            ("w.ts", "inCb()"),
            ("w.ts", "InCls"),
        ],
        &[
            ("w.ts:w.ts", "contains", "w.ts:inIife()"),
            ("w.ts:w.ts", "contains", "w.ts:inCb()"),
            ("w.ts:w.ts", "contains", "w.ts:InCls"),
            ("w.ts:inIife()", "calls", "w.ts:helper()"),
            ("w.ts:inCb()", "calls", "w.ts:helper()"),
            ("w.ts:f()", "calls", "w.ts:helper()"),
        ],
        &[("w.ts:w.ts", "calls", "w.ts:helper()")],
    );
}

// A nested function binds only from inside its parent.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_nested_scope() {
    let a = r#"export function top() {
  function only() {}
  function caller() { only(); }
}
function user() { only(); }
"#;
    let b = "function far() { only(); }\n";
    let (_, edges) = resolve_src(&[("a.ts", a), ("b.ts", b)]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[("a.ts:caller()", "calls", "a.ts:only()")],
        &[
            ("b.ts:far()", "calls", "a.ts:only()"),
            ("a.ts:user()", "calls", "a.ts:only()"),
        ],
    );
}

// A declaration below module level makes no node; its calls go to the
// enclosing body, or the file for a top-level callback.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_local_declarations_no_node() {
    let src = r#"function helper() {}
(function () { var inner = function () { helper(); }; })();
if (x) { const inIf = () => { helper(); }; }
describe("x", () => { const lh = () => {}; });
"#;
    let (nodes, edges) = resolve_src(&[("l.ts", src)]);
    for l in ["inner()", "inIf()", "lh()"] {
        assert!(!has_node(&nodes, "l.ts", l), "unexpected node {l}");
    }
    want_graph(
        &nodes,
        &edges,
        &[],
        &[],
        &[("l.ts:l.ts", "calls", "l.ts:helper()")],
    );
}

// Only a closure in a call statement makes the file its caller.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_file_caller_shapes() {
    let src = r#"function a() {} function b() {} function c() {} function d() {} function e() {}
late = () => { a(); };
export default () => { b(); };
!function () { c(); }();
x.y = { z: () => d() };
ok && foo(() => e());
"#;
    let (_, edges) = resolve_src(&[("c.ts", src)]);
    let no: Vec<(String, String, String)> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|f| {
            (
                "c.ts:c.ts".to_string(),
                "calls".to_string(),
                format!("c.ts:{f}()"),
            )
        })
        .collect();
    want_graph(&Nodes::new(), &edges, &[], &[], &refs3(&no));
}

// A const initialised by a call, array or object makes no declaration
// nodes; a ternary or var initializer still does.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_const_initializer_no_recursion() {
    let src = r#"const w1 = wrap(() => { function g1() {} });
const w3 = [function () { function g3() {} }];
export const w8 = wrap(() => { function g10() {} });
const w2 = cond ? function () { function g2() {} } : null;
var w6 = wrap(function () { function g7() {} });
"#;
    let (nodes, _) = resolve_src(&[("k.ts", src)]);
    for l in ["g1()", "g3()", "g10()"] {
        assert!(!has_node(&nodes, "k.ts", l), "unexpected node {l}");
    }
    want_graph(
        &nodes,
        &Edges::new(),
        &[("k.ts", "g2()"), ("k.ts", "g7()")],
        &[],
        &[],
    );
}

// A module-level const or let that is no function: a node for an object,
// array, cast, call or new value, or for any exported plain name.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_top_level_const_nodes() {
    let src = r#"export const LIMIT = 3;
const plain = 5;
const obj = { a: 1 };
const arr = [1];
const made = make();
const inst = new Map();
let counter = 0;
let lobj = {};
var legacy = {};
export let x1 = "s", x2 = 2;
export const { p, q: r } = make();
const cast = {} as Foo;
const sat = {} satisfies Foo;
const noinit: number;
export const { d1 } = other;
function f() { const local = {}; }
"#;
    let (nodes, edges) = resolve_src(&[("v.ts", src)]);
    let mut wn = Vec::new();
    let mut we = Vec::new();
    for l in [
        "LIMIT", "obj", "arr", "made", "inst", "lobj", "x1", "x2", "p", "q", "cast", "sat",
    ] {
        wn.push(("v.ts".to_string(), l.to_string()));
        we.push((
            "v.ts:v.ts".to_string(),
            "contains".to_string(),
            format!("v.ts:{l}"),
        ));
    }
    for l in ["plain", "counter", "legacy", "noinit", "r", "d1", "local"] {
        assert!(!has_node(&nodes, "v.ts", l), "unexpected node {l}");
    }
    want_graph(&nodes, &edges, &refs2(&wn), &refs3(&we), &[]);
}

// A closure in a call initializer is called by the const; an object
// literal's methods are not tracked.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_const_closure_calls() {
    let src = r#"function h1() {} function h2() {} function h3() {} function h4() {}
const ex = wrap(() => { h1(); function g() { h2(); } });
const sat = wrap(() => h3()) satisfies X;
const lit = { a: () => h4() };
"#;
    let (_, edges) = resolve_src(&[("w.ts", src)]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[
            ("w.ts:ex", "calls", "w.ts:h1()"),
            ("w.ts:sat", "calls", "w.ts:h3()"),
        ],
        &[
            ("w.ts:lit", "calls", "w.ts:h4()"),
            ("w.ts:w.ts", "calls", "w.ts:h1()"),
        ],
    );
}

// A const sharing a callee's name must not hide the call.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_const_keeps_call_binding() {
    let (_, edges) = resolve_src(&[
        ("a.ts", "export function trace() {}"),
        ("ns/ts.ts", "export * from \"../a\";"),
        (
            "b.ts",
            "import { trace } from \"./ns/ts\";\nexport function run() { trace(); }",
        ),
        ("c.ts", "let trace: any[] = [];"),
    ]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[("b.ts:run()", "calls", "a.ts:trace()")],
        &[],
    );
}

// A generator function expression is a function, not a const.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_exported_generator_const() {
    let src = "function p1() {}\nexport const E = function* () { p1(); };\nexport const A = async function* () { p1(); };\n";
    let (nodes, edges) = resolve_src(&[("g.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[("g.ts", "E()"), ("g.ts", "A()")],
        &[
            ("g.ts:E()", "calls", "g.ts:p1()"),
            ("g.ts:A()", "calls", "g.ts:p1()"),
        ],
        &[],
    );
    assert!(
        !has_node(&nodes, "g.ts", "E") && !has_node(&nodes, "g.ts", "A"),
        "generator const made a bare node"
    );
}

// Prose naming a const does not link to it.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_const_no_markdown_reference() {
    let (_, edges) = resolve_src(&[
        (
            "a.ts",
            "export const SETTINGS = { a: 1 };\nexport function realFn() {}",
        ),
        ("README.md", "# Doc\n\nUse `SETTINGS` with `realFn`.\n"),
    ]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[("README.md:README.md", "references", "a.ts:realFn()")],
        &[("README.md:README.md", "references", "a.ts:SETTINGS")],
    );
}

// Namespaces and modules are file-contained nodes; members carry the
// namespace in their id, so equal names in two namespaces both survive.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_namespaces() {
    let src = r#"namespace NS {
  export class Inner { im() {} }
  export function nf() {}
  namespace Deep { export function df() { nf(); } }
}
namespace Other { export function nf() {} }
module Old { export enum E { A } }
declare module "pkg" { export function pf(): void; }
declare class Amb { am(): void; }
declare function afn(): void;
declare const ac: number;
declare namespace DN { function dnf(): void; }
declare global { interface Window { w(): void } }
export declare function edf(): void;
namespace A.B { export function ab() {} }
"#;
    let (nodes, edges) = resolve_src(&[("ns.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[
            ("ns.ts", "NS"),
            ("ns.ts", "Deep"),
            ("ns.ts", "Other"),
            ("ns.ts", "Old"),
            ("ns.ts", "pkg"),
            ("ns.ts", "DN"),
            ("ns.ts", "A.B"),
            ("ns.ts", "Inner"),
            ("ns.ts", "E"),
            ("ns.ts", "Amb"),
            ("ns.ts", "Window"),
            ("ns.ts", "ab()"),
            ("ns.ts", "df()"),
        ],
        &[
            ("ns.ts:ns.ts", "contains", "ns.ts:NS"),
            ("ns.ts:ns.ts", "contains", "ns.ts:Deep"),
            ("ns.ts:ns.ts", "contains", "ns.ts:A.B"),
            ("ns.ts:ns.ts", "contains", "ns.ts:Inner"),
            ("ns.ts:ns.ts", "contains", "ns.ts:Amb"),
        ],
        &[],
    );
    let n = file_from_bytes("ns.ts", src.as_bytes())
        .nodes
        .iter()
        .filter(|nd| nd.label == "nf()")
        .count();
    assert_eq!(n, 2, "nf() nodes = {n}, want 2");
    for bad in ["afn()", "pf()", "dnf()", "edf()", "ac", "global"] {
        assert!(!has_node(&nodes, "ns.ts", bad), "unexpected node {bad}");
    }
}

// A call binds to the innermost namespace that has the name.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_namespace_lexical_calls() {
    let a = "export function mk() {}\nnamespace P { export function mk() {} export function other() { mk(); } }\nexport function local() { mk(); }\n";
    let b = "import { mk } from \"./a\";\nexport function u() { mk(); }\n";
    let srcs = [("a.ts", a), ("b.ts", b)];
    want_calls(&srcs, "a.ts", "local()", &["a.ts:L1"]);
    want_calls(&srcs, "a.ts", "other()", &["a.ts:L2"]);
    want_calls(&srcs, "b.ts", "u()", &["a.ts:L1"]);

    let c = r#"namespace N {
  export function a() { b(); }
  export function b() {}
  export namespace In { export function c() { b(); d(); } function d() {} }
}
namespace O {
  export function b() {}
  export function z() { b(); }
}
function b() {}
function topcall() { b(); }
"#;
    let srcs = [("c.ts", c)];
    want_calls(&srcs, "c.ts", "a()", &["c.ts:L3"]);
    want_calls(&srcs, "c.ts", "c()", &["c.ts:L3", "c.ts:L4"]);
    want_calls(&srcs, "c.ts", "z()", &["c.ts:L7"]);
    want_calls(&srcs, "c.ts", "topcall()", &["c.ts:L10"]);

    let d = "namespace NS {\n  export function nf() {}\n  namespace Deep { export function df() { nf(); } }\n}\nnamespace Other { export function nf() {} }\n";
    want_calls(&[("d.ts", d)], "d.ts", "df()", &["d.ts:L2"]);
}

// A namespace named `_` or `$` still has its members.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_degenerate_namespace_members() {
    let src = "declare namespace _ { interface Coll {} class Kept {} }\nnamespace $ { export function d() {} }\nnamespace Outer { namespace _ { export class Hidden {} } }\n";
    let (nodes, _) = resolve_src(&[("o.ts", src)]);
    for l in ["Coll", "Kept", "d()", "Hidden"] {
        assert!(has_node(&nodes, "o.ts", l), "missing node {l}");
    }
}

// A namespace member never takes the id of a class member or top-level name.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_namespace_member_ids() {
    let src = r#"class Foo { create() {} }
namespace Foo { export function create() {} }
function Bar() { function helper() {} }
namespace Bar { export function helper() {} }
namespace A_B { export function u() {} }
function A_B_u() {}
"#;
    crate::extract::kinds::register_kinds();
    let mut got: BTreeMap<String, usize> = BTreeMap::new();
    for n in &file_from_bytes("m.ts", src.as_bytes()).nodes {
        *got.entry(n.label.clone()).or_insert(0) += 1;
    }
    for (l, c) in [
        ("create()", 1),
        ("Foo.create()", 1),
        ("helper()", 2),
        ("u()", 1),
        ("A_B_u()", 1),
    ] {
        let g = got.get(l).copied().unwrap_or(0);
        assert_eq!(g, c, "nodes {l:?} = {g}, want {c}");
    }
    let (_, edges) = resolve_src(&[("m.ts", src)]);
    assert!(
        !has_edge(&edges, &("m.ts:m.ts", "contains", "m.ts:Foo.create()")),
        "file contains the method Foo.create()"
    );
}

// Exported variables of a namespace or ambient module make an orphan node.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_namespace_variable_nodes() {
    let src = r#"namespace M {
  export const arrow = () => {};
  export const obj = { a: 1 };
  export let arr = [1];
  const priv = { b: 2 };
}
declare module "m1" { export const k1: number; export let k2: string; }
declare namespace G { const k3: number; export const k4: { a: 1 }; }
"#;
    let (nodes, edges) = resolve_src(&[("v.ts", src)]);
    for l in ["arrow", "obj", "arr", "k1", "k2", "k4"] {
        assert!(has_node(&nodes, "v.ts", l), "missing node {l}");
    }
    for l in ["priv", "k3", "arrow()"] {
        assert!(!has_node(&nodes, "v.ts", l), "unexpected node {l}");
    }
    assert!(
        !has_edge(&edges, &("v.ts:v.ts", "contains", "v.ts:k1")),
        "k1 is an orphan node upstream"
    );
}

// A namespace const is no call target; functions and classes are.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_namespace_call_targets() {
    let src = r#"namespace T {
  export const cfn = () => {};
  export function useC() { cfn(); }
  export class Cls {}
  export function useK() { Cls(); }
  export function fn() {}
  export function useF() { fn(); }
}
"#;
    let (_, edges) = resolve_src(&[("t.ts", src)]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[
            ("t.ts:useK()", "calls", "t.ts:Cls"),
            ("t.ts:useF()", "calls", "t.ts:fn()"),
        ],
        &[
            ("t.ts:useC()", "calls", "t.ts:cfn"),
            ("t.ts:t.ts", "contains", "t.ts:cfn"),
        ],
    );
}

// A method body is a boundary: its calls go nowhere, also in a closure
// the file or a const owns.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_method_in_closure_drops_calls() {
    let src = r#"function h01() {} function h08() {} function h1() {} function h2() {}
foo(() => { return { m01() { h01(); } }; });
foo(() => { const K = class { m08() { h08(); } }; });
const c1 = foo(() => { return { m() { h1(); } }; });
const c2 = foo(class { m() { h2(); } });
"#;
    let (_, edges) = resolve_src(&[("m.ts", src)]);
    let mut no: Vec<(String, String, String)> = Vec::new();
    for h in ["h01", "h08", "h1", "h2"] {
        for from in ["m.ts:m.ts", "m.ts:c1", "m.ts:c2"] {
            no.push((from.to_string(), "calls".to_string(), format!("m.ts:{h}()")));
        }
    }
    want_graph(&Nodes::new(), &edges, &[], &[], &refs3(&no));
}

// A file-level callback or IIFE calling a function of the file adds no
// edge: upstream's contains edge hides it.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_file_closure_calls_hidden() {
    let src = r#"function helper() {}
describe("x", () => { helper(); });
[1].forEach(() => helper());
(function () { helper(); })();
p.then(() => helper());
"#;
    let (_, edges) = resolve_src(&[("f.ts", src)]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[],
        &[("f.ts:f.ts", "calls", "f.ts:helper()")],
    );
}

// A member-assigned function has no nested nodes; calls in its inner
// declarations are its own.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_member_assign_no_nested() {
    let src = r#"function helper() {}
exports.run = function () { function inRun() { helper(); } inRun(); };
module.exports.go = () => { function inGo() { helper(); } };
Ctor.prototype.m = function () { function inM() { helper(); } };
exports.gen = async function* () { helper(); };
"#;
    let (nodes, edges) = resolve_src(&[("e.ts", src)]);
    for l in ["inRun()", "inGo()", "inM()"] {
        assert!(!has_node(&nodes, "e.ts", l), "unexpected node {l}");
    }
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("e.ts:run()", "calls", "e.ts:helper()"),
            ("e.ts:go()", "calls", "e.ts:helper()"),
            ("e.ts:Ctor.m()", "calls", "e.ts:helper()"),
            ("e.ts:gen()", "calls", "e.ts:helper()"),
        ],
        &[],
    );
}

// A module-level const is a call target, in its file and through an import.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_const_call_targets() {
    let a = "export const eObj = {};\nexport const eCall = make();\nconst loc = new Map();\nfunction f() { loc(); eObj(); }\n";
    let b = "import { eCall } from \"./a\";\nfunction g() { eCall(); }\n";
    let (_, edges) = resolve_src(&[("a.ts", a), ("b.ts", b)]);
    want_graph(
        &Nodes::new(),
        &edges,
        &[],
        &[
            ("a.ts:f()", "calls", "a.ts:loc"),
            ("a.ts:f()", "calls", "a.ts:eObj"),
            ("b.ts:g()", "calls", "a.ts:eCall"),
        ],
        &[],
    );
}

// A static block holds file-level declarations; an exported destructure
// makes a node per key.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_static_block_and_pattern_keys() {
    let src = r#"function helper() {}
class S { static { function inStatic() { helper(); } inStatic(); } }
export const { a, b: c, d: { e }, f = 1, g: h = 2, ...rest } = x();
"#;
    let (nodes, edges) = resolve_src(&[("s.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[
            ("s.ts", "inStatic()"),
            ("s.ts", "a"),
            ("s.ts", "b"),
            ("s.ts", "d"),
            ("s.ts", "g"),
        ],
        &[("s.ts:inStatic()", "calls", "s.ts:helper()")],
        &[],
    );
    for l in ["c", "e", "f", "h", "rest"] {
        assert!(!has_node(&nodes, "s.ts", l), "unexpected node {l}");
    }
}

// A namespace member without `export` is no call target outside it.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_ts_namespace_private_not_global() {
    let p = "namespace P {\n  function hidden() {}\n  export function shown() {}\n  export function parse() { hidden(); }\n}\n";
    let u = "export function user() { hidden(); }\n";
    let v = "export function viewer() { shown(); }\n";
    let srcs = [("p.ts", p), ("u.ts", u), ("v.ts", v)];
    want_calls(&srcs, "u.ts", "user()", &[]);
    want_calls(&srcs, "v.ts", "viewer()", &["p.ts:L3"]);
    want_calls(&srcs, "p.ts", "parse()", &["p.ts:L2"]);
}

// A bare call in JS/TS never names a class method.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_bare_call_skips_methods() {
    let srcs = [
        ("k.ts", "export class K { some() {} only() {} }\n"),
        ("f.ts", "export function some() {}\n"),
        ("c.ts", "export function u() { some(); only(); }\n"),
        (
            "d.ts",
            "export class D { run() { this.only(); } only() {} }\n",
        ),
    ];
    want_calls(&srcs, "c.ts", "u()", &["f.ts:L1"]);
    want_calls(&srcs, "d.ts", "D.run()", &["d.ts:L1"]);
}

// An exported variable without a node of its own still gets one, edgeless.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_exported_variable_orphans() {
    let src = r#"export const c1: number;
export let l1: string;
export var v1 = 1;
export var v2;
export const { d1, d2: d3 } = obj;
export const [e1, e2] = arr;
export let { f1 } = obj;
export var { g1 } = obj;
export const { m1 } = a.b;
export const h1 = 5;
"#;
    let (nodes, edges) = resolve_src(&[("a.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[
            ("a.ts", "c1"),
            ("a.ts", "l1"),
            ("a.ts", "v1"),
            ("a.ts", "v2"),
            ("a.ts", "{ d1, d2: d3 }"),
            ("a.ts", "[e1, e2]"),
            ("a.ts", "{ f1 }"),
            ("a.ts", "h1"),
        ],
        &[("a.ts:a.ts", "contains", "a.ts:h1")],
        &[
            ("a.ts:a.ts", "contains", "a.ts:c1"),
            ("a.ts:a.ts", "contains", "a.ts:v1"),
        ],
    );
    for bad in ["{ g1 }", "g1", "{ m1 }", "m1"] {
        assert!(!has_node(&nodes, "a.ts", bad), "unexpected node {bad}");
    }
    // under an ERROR node
    let (nodes, _) = resolve_src(&[("b.js", "function wrap() {\n    export const bad = 1;\n}\n")]);
    assert!(
        has_node(&nodes, "b.js", "bad"),
        "missing node bad under error"
    );
}

// Member assignments below module level and `var v = function*` make no node.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_member_assign_module_level_only() {
    let src = r#"function t() {}
o.top = function () {};
if (c) { o.nested = function () {}; }
(function () { o.iife = function () {}; })();
class K { static { o.stat = function () {}; } }
var v = function* () { t(); };
var w = function () {};
"#;
    let (nodes, _) = resolve_src(&[("a.js", src)]);
    // An unexported receiver proves no owner: no node, as upstream.
    assert!(has_node(&nodes, "a.js", "w()"), "missing node w()");
    for l in ["top()", "nested()", "iife()", "stat()", "v()"] {
        assert!(!has_node(&nodes, "a.js", l), "unexpected node {l}");
    }
}

// Heritage binds a same-file or named-import base, never a guess.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_heritage() {
    let m = r#"export class Base {}
export class Solo {}
export class Nm {}
export function Fn() {}
export default class Dm {}
"#;
    let use_ = r#"import { Base as B2, Fn, Nm } from "./m";
import D from "./m";
import * as ns from "./m";
class NotImported extends Solo {}
class Aliased extends B2 {}
class Amb extends Base {}
class OfFn extends Fn {}
class ViaDef extends D {}
class Q extends ns.Nm {}
class G extends Nm<string> {}
class Call extends mix(B2) {}
class Own extends Later implements Iface, ns.X {}
class Later {}
interface Iface extends Later {}
interface J extends B2, Unknown {}
"#;
    let other = "export class Base {}\n";
    let (nodes, edges) = resolve_src(&[("m.ts", m), ("use.ts", use_), ("other.ts", other)]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("use.ts:Aliased", "inherits", "m.ts:Base"),
            ("use.ts:ViaDef", "inherits", "m.ts:Dm"),
            ("use.ts:G", "inherits", "m.ts:Nm"),
            ("use.ts:Own", "inherits", "use.ts:Later"),
            ("use.ts:Own", "implements", "use.ts:Iface"),
            ("use.ts:Iface", "inherits", "use.ts:Later"),
            ("use.ts:J", "inherits", "m.ts:Base"),
        ],
        &[
            ("use.ts:NotImported", "inherits", "m.ts:Solo"),
            ("use.ts:Amb", "inherits", "m.ts:Base"),
            ("use.ts:Amb", "inherits", "other.ts:Base"),
            ("use.ts:OfFn", "inherits", "m.ts:Fn()"),
            ("use.ts:Q", "inherits", "m.ts:Nm"),
            ("use.ts:Call", "inherits", "m.ts:Base"),
        ],
    );
    let (nodes, edges) = resolve_src(&[(
        "a.js",
        "class B {}\nclass C extends B {}\nclass D extends x.B {}\nconst lib = {};\nclass F extends lib.Old {}\nclass Q extends lib(B) {}\n",
    )]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[("a.js:C", "inherits", "a.js:B")],
        &[
            ("a.js:D", "inherits", "a.js:B"),
            ("a.js:F", "inherits", "a.js:lib"),
            ("a.js:Q", "inherits", "a.js:lib"),
            ("a.js:Q", "inherits", "a.js:B"),
        ],
    );
}

// An export alias names the binding, `default` and a re-exported import
// included.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_aliased_exports() {
    let a = "export default class Dflt {}\nexport class A {}\nexport function f() {}\n";
    let idx = "export { f as g, A as AA, default as Named } from \"./a\";\n";
    let idd = "export { default } from \"./a\";\n";
    let loc = "import { A as Inner } from \"./a\";\nexport { Inner as Outer };\n";
    let use_ = r#"import { g, AA, Named } from "./idx";
import Dd from "./idd";
import { Outer } from "./loc";
import { default as Def } from "./a";
class E1 extends AA {}
class E2 extends Named {}
class E3 extends Dd {}
class E4 extends Outer {}
class E5 extends Def {}
function h() { g(); }
"#;
    let use2 = "import { f, A } from \"./idx\";\nclass E6 extends A {}\n";
    let (nodes, edges) = resolve_src(&[
        ("a.ts", a),
        ("idx.ts", idx),
        ("idd.ts", idd),
        ("loc.ts", loc),
        ("use.ts", use_),
        ("use2.ts", use2),
    ]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("use.ts:use.ts", "imports", "a.ts:f()"),
            ("use.ts:use.ts", "imports", "a.ts:A"),
            ("use.ts:use.ts", "imports", "a.ts:Dflt"),
            ("use.ts:E1", "inherits", "a.ts:A"),
            ("use.ts:E2", "inherits", "a.ts:Dflt"),
            ("use.ts:E3", "inherits", "a.ts:Dflt"),
            ("use.ts:E4", "inherits", "a.ts:A"),
            ("use.ts:E5", "inherits", "a.ts:Dflt"),
            ("loc.ts:loc.ts", "re_exports", "a.ts:a.ts"),
            ("use.ts:h()", "calls", "a.ts:f()"),
        ],
        &[
            ("use2.ts:use2.ts", "imports", "a.ts:f()"),
            ("use2.ts:use2.ts", "imports", "a.ts:A"),
            ("use2.ts:E6", "inherits", "a.ts:A"),
            ("loc.ts:loc.ts", "re_exports", "a.ts:A"),
        ],
    );
}

// A named import targets the definition, through barrels; a namespace import
// targets nothing; a re-export gives re_exports.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_import_symbols() {
    let m = "export function x() {}\nexport class K {}\nexport const C1 = { a: 1 };\nexport default class Dm {}\n";
    let m2 = "export function y() {}\nexport interface Ifc {}\n";
    let m3 = "function dd() {}\nexport default dd;\n";
    let m4 = "export default function () {}\nexport const z = () => {};\n";
    let m5 = "class Foo {}\nexport { Foo as default };\n";
    let idx = "export { x, K } from \"./m\";\nexport * from \"./m2\";\nexport * as nsx from \"./m4\";\nexport { default as Dm2 } from \"./m\";\n";
    let use_ = r#"import { x, y, K as KK, Dm2, gone } from "./index";
import d3 from "./m3";
import d4, { z } from "./m4";
import d5 from "./m5";
import type { Ifc } from "./m2";
import { C1, default as dflt } from "./m";
import * as ns from "./m";
"#;
    let (nodes, edges) = resolve_src(&[
        ("m.ts", m),
        ("m2.ts", m2),
        ("m3.ts", m3),
        ("m4.ts", m4),
        ("m5.ts", m5),
        ("index.ts", idx),
        ("use.ts", use_),
    ]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("use.ts:use.ts", "imports", "m.ts:x()"),
            ("use.ts:use.ts", "imports", "m2.ts:y()"),
            ("use.ts:use.ts", "imports", "m.ts:K"),
            ("use.ts:use.ts", "imports", "m3.ts:dd()"),
            ("use.ts:use.ts", "imports", "m4.ts:z()"),
            ("use.ts:use.ts", "imports", "m5.ts:Foo"),
            ("use.ts:use.ts", "imports", "m2.ts:Ifc"),
            ("use.ts:use.ts", "imports", "m.ts:C1"),
            ("use.ts:use.ts", "imports", "m.ts:Dm"),
            ("index.ts:index.ts", "re_exports", "m.ts:x()"),
            ("index.ts:index.ts", "re_exports", "m.ts:K"),
            ("index.ts:index.ts", "re_exports", "m.ts:m.ts"),
            ("index.ts:index.ts", "re_exports", "m2.ts:m2.ts"),
            ("index.ts:index.ts", "re_exports", "m4.ts:m4.ts"),
        ],
        &[
            ("use.ts:use.ts", "imports", "m4.ts:m4.ts"),
            ("index.ts:index.ts", "re_exports", "m.ts:Dm"),
            ("index.ts:index.ts", "re_exports", "m4.ts:z()"),
        ],
    );
}

// A function-valued field and a body-less signature are methods of the class.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_class_member_forms() {
    let src = r#"function helper() {}
export class C {
  plain = 3;
  declare d: number;
  arrow = () => { helper(); };
  static sarrow = () => { helper(); };
  field = function () { helper(); function inField() {} };
  typed: () => void = function () { helper(); };
  over(a: string): void;
  over(a: any) { helper(); }
}
export abstract class A {
  abstract f(): void;
  override abstract h(): void;
}
declare class D { constructor(); dsig(): void; }
class S { only(); }
"#;
    let (nodes, edges) = resolve_src(&[("m.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[
            ("m.ts", "C.arrow()"),
            ("m.ts", "C.sarrow()"),
            ("m.ts", "C.field()"),
            ("m.ts", "C.typed()"),
            ("m.ts", "A.f()"),
            ("m.ts", "A.h()"),
            ("m.ts", "D.constructor()"),
            ("m.ts", "D.dsig()"),
            ("m.ts", "S.only()"),
        ],
        &[
            ("m.ts:C", "contains", "m.ts:C.arrow()"),
            ("m.ts:C", "contains", "m.ts:C.sarrow()"),
            ("m.ts:C", "contains", "m.ts:C.field()"),
            ("m.ts:D", "contains", "m.ts:D.dsig()"),
            ("m.ts:S", "contains", "m.ts:S.only()"),
            ("m.ts:C.arrow()", "calls", "m.ts:helper()"),
            ("m.ts:C.sarrow()", "calls", "m.ts:helper()"),
            ("m.ts:C.field()", "calls", "m.ts:helper()"),
            ("m.ts:C.typed()", "calls", "m.ts:helper()"),
        ],
        &[],
    );
    for n in ["C.plain()", "C.d()", "A.abstract()", "inField()"] {
        assert!(!has_node(&nodes, "m.ts", n), "unexpected node {n}");
    }
    // An overload's node sits at its first signature, as upstream.
    let ext = resolve(
        &[file_from_bytes("m.ts", src.as_bytes())],
        &["m.ts".to_string()],
        workers(),
    );
    for n in &ext.nodes {
        assert!(
            !(n.label == "C.over()" && n.source_location != "L9"),
            "C.over() at {}, want L9",
            n.source_location
        );
    }
}

// An overloaded method with a body is no abstract method: an abstract
// one loses a call to an equal-named method elsewhere.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_overload_not_abstract() {
    crate::extract::kinds::register_kinds();
    let r = file_from_bytes(
        "a.ts",
        b"export class API {\n  run(x: string): void;\n  run(x: any) {}\n  sig(): void;\n}\n",
    );
    let mut got: BTreeMap<&str, bool> = BTreeMap::new();
    for d in &r.defs {
        got.insert(d.name.as_str(), d.is_abstract);
    }
    let run = got.get("run");
    assert!(
        run == Some(&false),
        "run: present {} abstract {:?}, want a concrete def",
        run.is_some(),
        run
    );
    assert!(
        got.get("sig").copied().unwrap_or(false),
        "sig: want an abstract def"
    );
}

// A class owns its member even when another symbol took the id first.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_class_member_collisions() {
    let (nodes, edges) =
        resolve_src(&[("c.ts", "function foo_bar() {}\nclass Foo { bar() {} }\n")]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[("c.ts:Foo", "contains", "c.ts:foo_bar()")],
        &[],
    );
}

// A member assignment hangs on the owner its receiver proves; an unproven
// receiver makes nothing.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_member_assign_owners() {
    let src = r#"function helper() {}
Foo.prototype.early = function () { helper(); };
function Foo() {}
Foo.prototype.bar = function () { helper(); };
Undef.prototype.pm = function () { helper(); };
export const xapi = { run() {} };
xapi.later = function () { helper(); };
other.z = function () { helper(); };
function factory() {
  const o = {};
  o.x = function () { helper(); };
  this.y = function () { helper(); };
  if (c) { this.no = function () {}; }
}
class K { init() { const p = {}; p.px = () => {}; this.cb = function () { helper(); }; } }
const arrow = () => { this.ax = function () {}; };
reg(function () { this.cx = function () { helper(); }; });
(function () { Foo.prototype.inIife = function () {}; })();
"#;
    let (nodes, edges) = resolve_src(&[("a.js", src)]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("a.js:Foo()", "contains", "a.js:Foo.early()"),
            ("a.js:Foo()", "contains", "a.js:Foo.bar()"),
            ("a.js:Foo()", "contains", "a.js:Foo.inIife()"),
            ("a.js:a.js", "contains", "a.js:Undef.pm()"),
            ("a.js:xapi", "contains", "a.js:xapi.later()"),
            ("a.js:factory()", "contains", "a.js:o"),
            ("a.js:o", "contains", "a.js:o.x()"),
            ("a.js:factory()", "contains", "a.js:factory.y()"),
            ("a.js:K", "contains", "a.js:K.cb()"),
            ("a.js:K.init()", "contains", "a.js:p"),
            ("a.js:p", "contains", "a.js:p.px()"),
            ("a.js:arrow()", "contains", "a.js:arrow.ax()"),
            ("a.js:a.js", "contains", "a.js:cx()"),
            ("a.js:Foo.bar()", "calls", "a.js:helper()"),
            ("a.js:o.x()", "calls", "a.js:helper()"),
            ("a.js:factory.y()", "calls", "a.js:helper()"),
            ("a.js:K.cb()", "calls", "a.js:helper()"),
            ("a.js:cx()", "calls", "a.js:helper()"),
        ],
        &[
            ("a.js:factory()", "calls", "a.js:helper()"),
            ("a.js:K.init()", "calls", "a.js:helper()"),
        ],
    );
    for l in ["z()", "other.z()", "no()", "factory.no()"] {
        assert!(!has_node(&nodes, "a.js", l), "unexpected node {l}");
    }
}

// A method of an object literal or a class expression hangs on the
// module-level name it is bound to, else on the class expression's name,
// else on the file. A const or let object keeps no methods, as upstream.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_literal_method_owners() {
    let src = r#"function helper() {}
const CE = class { cm() { helper(); } };
const N1 = class Named { nm() {} };
reg(class Reg { rm() {} });
var vo = { vm() { helper(); }, inner: { deep() {} }, arr: [{ am() {} }] };
late = { lm() {} };
const q = cond ? { qm() {} } : null;
exports.K = class { kv() {} };
module.exports = { mm() { helper(); } };
export default { dm() {} };
call({ cm2() { helper(); } });
const api = { run() {} };
let lo = { lom() {} };
function f() { var local = { fm() {} }; return { rm2() {} }; }
reg(() => { var inCb = { cbm() {} }; });
var tv: { vsig(): void } = null as any;
type TL = { tm(): void };
"#;
    let (nodes, edges) = resolve_src(&[("a.ts", src)]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("a.ts:a.ts", "contains", "a.ts:CE"),
            ("a.ts:CE", "contains", "a.ts:CE.cm()"),
            ("a.ts:N1", "contains", "a.ts:N1.nm()"),
            ("a.ts:Reg", "contains", "a.ts:Reg.rm()"),
            ("a.ts:vo", "contains", "a.ts:vo.vm()"),
            ("a.ts:vo", "contains", "a.ts:vo.deep()"),
            ("a.ts:vo", "contains", "a.ts:vo.am()"),
            ("a.ts:late", "contains", "a.ts:late.lm()"),
            ("a.ts:q", "contains", "a.ts:q.qm()"),
            ("a.ts:K", "contains", "a.ts:K.kv()"),
            ("a.ts:a.ts", "contains", "a.ts:mm()"),
            ("a.ts:a.ts", "contains", "a.ts:dm()"),
            ("a.ts:a.ts", "contains", "a.ts:cm2()"),
            ("a.ts:a.ts", "contains", "a.ts:cbm()"),
            ("a.ts:tv", "contains", "a.ts:tv.vsig()"),
            ("a.ts:CE.cm()", "calls", "a.ts:helper()"),
            ("a.ts:vo.vm()", "calls", "a.ts:helper()"),
            ("a.ts:mm()", "calls", "a.ts:helper()"),
            ("a.ts:cm2()", "calls", "a.ts:helper()"),
        ],
        &[],
    );
    for l in [
        "api.run()",
        "run()",
        "lo.lom()",
        "lom()",
        "fm()",
        "rm2()",
        "tm()",
        "TL.tm()",
    ] {
        assert!(!has_node(&nodes, "a.ts", l), "unexpected node {l}");
    }
}

// An owner node never takes the id of a later declaration, and a literal's
// method is no call target of another file.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_literal_owner_no_steal() {
    let (nodes, edges) = resolve_src(&[
        (
            "a.ts",
            "var r1 = { dispose() {} };\nexport type R1 = typeof r1;\ndeclare var fs: { readFileSync(f: string): string };\n",
        ),
        (
            "b.ts",
            "function go() { fs.readFileSync('x'); readFileSync('y'); dispose(); }\n",
        ),
    ]);
    assert!(has_node(&nodes, "a.ts", "R1"), "missing node R1");
    want_graph(
        &nodes,
        &edges,
        &[],
        &[],
        &[
            ("b.ts:go()", "calls", "a.ts:fs.readFileSync()"),
            ("b.ts:go()", "calls", "a.ts:R1.dispose()"),
            ("b.ts:go()", "calls", "a.ts:r1.dispose()"),
        ],
    );
}

// A `this.x = fn` in the constructor does not hide the method x from super.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_this_member_keeps_method() {
    let src = r#"class C {
  constructor() { this.foo = () => {}; }
  foo() {}
}
class D extends C {
  foo() { super.foo(); }
}
"#;
    let (nodes, edges) = resolve_src(&[("i.js", src)]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[("i.js:D.foo()", "calls", "i.js:C.foo()")],
        &[],
    );
}

// A method on an exported object is a call target of other files, and a
// prototype member finds an owner declared in a namespace.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_exported_object_method_target() {
    let (nodes, edges) = resolve_src(&[
        (
            "lib.js",
            "var api = {};\nmodule.exports = api;\napi.pubFn = function () {};\n",
        ),
        (
            "use.js",
            "const { pubFn } = require('./lib');\nfunction main() { pubFn(); }\n",
        ),
        (
            "ns.ts",
            "namespace N { function Foo() {} Foo.prototype.x = function () {}; }\n",
        ),
    ]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[
            ("use.js:main()", "calls", "lib.js:api.pubFn()"),
            ("ns.ts:Foo()", "contains", "ns.ts:Foo.x()"),
        ],
        &[],
    );
}

// Only a member of an exported object binds a bare call: a prototype
// member never does, whatever its owner, and a class method never does.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_bare_call_frees_only_export_members() {
    let (nodes, edges) = resolve_src(&[
        (
            "patch.js",
            "const Emitter = require('events');\nEmitter.prototype.emitAsync = function () {};\nconst Lit = {a: 1};\nLit.prototype.litProto = function () {};\n",
        ),
        (
            "r.js",
            "module.exports = Router;\nRouter.create = function () {};\nfunction Router() {}\nRouter.prototype.handleReq = function () {};\n",
        ),
        (
            "ns.ts",
            "function Foo() {}\nnamespace M { Foo.prototype.outer = function () {}; }\n",
        ),
        (
            "d.js",
            "var M1 = Base.extend({});\nM1.prototype.p1 = function () {};\nUndeclared.prototype.p4 = function () {};\n",
        ),
        (
            "use.js",
            "function u() { emitAsync(); litProto(); handleReq(); p1(); p4(); }\n",
        ),
    ]);
    want_graph(
        &nodes,
        &edges,
        &[],
        &[("ns.ts:Foo()", "contains", "ns.ts:Foo.outer()")],
        &[
            ("use.js:u()", "calls", "patch.js:Emitter.emitAsync()"),
            ("use.js:u()", "calls", "patch.js:Lit.litProto()"),
            ("use.js:u()", "calls", "r.js:Router.handleReq()"),
            ("use.js:u()", "calls", "d.js:M1.p1()"),
            ("use.js:u()", "calls", "d.js:Undeclared.p4()"),
        ],
    );
}
