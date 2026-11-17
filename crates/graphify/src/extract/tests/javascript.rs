//! graphify-go `internal/extract/javascript_test.go`. (`javascript_alias_test.go` tests the `@/`
//! alias of `resolveModulePath`, `resolve.go:1877`, and is ported with the resolve tests,
//! graphify-rs-ihc.18.)

use std::collections::BTreeMap;

use crate::extract::{Result, file_from_bytes, resolve};
use crate::idutil::make_id;

// A `// NOTE:` comment becomes a rationale node with an edge to the file, and an
// `ADR-0001` reference in a comment becomes a normalized doc_ref node cited by
// the file.
#[test]
fn test_extract_js_rationale() {
    crate::extract::kinds::register_kinds();
    let src = concat!(
        "// NOTE: debounce to avoid thrash\n",
        "// See ADR-0001 for the retry policy.\n",
        "export const boot = () => {\n",
        "  return 1;\n",
        "};\n"
    );
    let res = file_from_bytes("web/app.js", src.as_bytes());

    let file_id = make_id(&["web/app.js"]);

    let mut label: BTreeMap<&str, &str> = BTreeMap::new();
    let mut ftype: BTreeMap<&str, &str> = BTreeMap::new();
    for n in &res.nodes {
        label.insert(&n.id, &n.label);
        ftype.insert(&n.id, &n.file_type);
    }
    let label_of = |id: &str| label.get(id).copied().unwrap_or("");
    let ftype_of = |id: &str| ftype.get(id).copied().unwrap_or("");

    // NOTE comment -> rationale node with a rationale_for edge to the file.
    let rationale_ok = res.edges.iter().any(|e| {
        e.relation.as_str() == "rationale_for"
            && e.target == file_id
            && ftype_of(&e.source) == "rationale"
            && label_of(&e.source).starts_with("NOTE: debounce")
    });
    assert!(
        rationale_ok,
        "expected rationale_for edge from NOTE comment to file {file_id}"
    );

    // ADR-0001 comment -> normalized doc_ref node + cites edge from the file.
    let doc_ref_id = make_id(&["docref", "ADR-0001"]);
    assert!(
        ftype_of(&doc_ref_id) == "doc_ref" && label_of(&doc_ref_id) == "ADR-0001",
        "expected doc_ref node {doc_ref_id} labeled ADR-0001, got type={:?} label={:?}",
        ftype_of(&doc_ref_id),
        label_of(&doc_ref_id)
    );
    let cites_ok = res
        .edges
        .iter()
        .any(|e| e.relation.as_str() == "cites" && e.source == file_id && e.target == doc_ref_id);
    assert!(cites_ok, "expected cites edge {file_id} -> {doc_ref_id}");
}

/// Go `importEdges` (`local_imports_test.go:11`) with no go.mod table: extracts each source,
/// resolves the corpus, and returns the sorted import edges.
fn import_edges(srcs: &[(&str, &str)]) -> Vec<String> {
    let mut results = vec![Result::default()];
    let mut files: Vec<String> = Vec::new();
    for (f, s) in srcs {
        results.push(file_from_bytes(f, s.as_bytes()));
        files.push((*f).to_string());
    }
    files.sort();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut edges: Vec<String> = resolve(&results, &files, workers)
        .edges
        .iter()
        .filter(|e| matches!(e.relation.as_str(), "imports" | "imports_from"))
        .map(|e| format!("{} {} {}", e.source, e.relation.as_str(), e.target))
        .collect();
    edges.sort();
    edges
}

// CommonJS require and re-exports each link to the target file.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_js_import_forms_resolve_to_files() {
    crate::extract::kinds::register_kinds();
    const CASES: [(&str, &str); 4] = [
        ("require", "const lib = require('./lib');\n"),
        (
            "nested require",
            "function f() {\n  return require(\"./lib\").x;\n}\n",
        ),
        ("export star", "export * from './lib';\n"),
        ("export named", "export { thing } from './lib';\n"),
    ];
    for (name, src) in CASES {
        let got = import_edges(&[
            ("src/a.ts", src),
            ("src/lib.ts", "export const thing = 1;\n"),
        ]);
        let want = vec!["src_a_ts imports_from src_lib_ts".to_string()];
        assert_eq!(got, want, "{name}");
    }
    // Not imports: a computed spec, a method named require, a plain export.
    let src = "const a = require(name);\nconst b = obj.require('./lib');\nexport { b };\n";
    let got = import_edges(&[("src/a.js", src), ("src/lib.js", "")]);
    assert!(got.is_empty(), "non-imports: got {got:?}, want none");
}

// `export type { T } from` is type-only, like `import type`.
#[test]
fn test_js_re_export_type_only() {
    crate::extract::kinds::register_kinds();
    const CASES: [(&str, bool); 2] = [
        ("export type { T } from './lib';\n", true),
        ("export { T } from './lib';\n", false),
    ];
    for (src, want) in CASES {
        let imps = file_from_bytes("a.ts", src.as_bytes()).imps;
        assert!(
            imps.len() == 1 && imps[0].type_only == want,
            "{src:?}: got {imps:?}, want one import with TypeOnly={want}"
        );
    }
}

/// smoke; the real checks are the ihc.18-ignored tests
#[test]
fn test_extract_js_smoke() {
    crate::extract::kinds::register_kinds();
    const SRCS: &[(&str, &str)] = &[
        (
            "src/f.ts",
            "import x from \"./x\";\ntop();\nfunction a() { one(); x.two(() => three()); const m = require(\"m\"); }\nconst b = () => four(), c = function () { five.six(); };\nexport class K {\n  m() { this.seven(); }\n  n() {}\n  o() { eight(); }\n}\nbetween();\nexports.p = () => { nine(); };\nexport default function d() { ten(); }\nafter.all();\n",
        ),
        (
            "n.ts",
            "function helper() {}\nfunction outer() {\n  function inner() { helper(); }\n  inner();\n  const cb = () => { function deep() { helper(); } deep(); };\n  after();\n}\nfunction after() {}\nconst arrow = () => { function inArrow() { helper(); } inArrow(); };\nclass K { m() { function inMethod() { helper(); } inMethod(); } }\n",
        ),
        (
            "ns.ts",
            "namespace NS {\n  export class Inner { im() {} }\n  export function nf() {}\n  namespace Deep { export function df() { nf(); } }\n}\nnamespace Other { export function nf() {} }\nmodule Old { export enum E { A } }\ndeclare module \"pkg\" { export function pf(): void; }\ndeclare class Amb { am(): void; }\ndeclare function afn(): void;\ndeclare const ac: number;\ndeclare namespace DN { function dnf(): void; }\ndeclare global { interface Window { w(): void } }\nexport declare function edf(): void;\nnamespace A.B { export function ab() {} }\n",
        ),
        (
            "c.ts",
            "function helper() {}\nexport class C {\n  plain = 3;\n  declare d: number;\n  arrow = () => { helper(); };\n  static sarrow = () => { helper(); };\n  field = function () { helper(); function inField() {} };\n  typed: () => void = function () { helper(); };\n  over(a: string): void;\n  over(a: any) { helper(); }\n}\nexport abstract class A {\n  abstract f(): void;\n  override abstract h(): void;\n}\ndeclare class D { constructor(); dsig(): void; }\nclass S { only(); }\n",
        ),
        (
            "a.js",
            "function helper() {}\nFoo.prototype.early = function () { helper(); };\nfunction Foo() {}\nFoo.prototype.bar = function () { helper(); };\nUndef.prototype.pm = function () { helper(); };\nexport const xapi = { run() {} };\nxapi.later = function () { helper(); };\nother.z = function () { helper(); };\nfunction factory() {\n  const o = {};\n  o.x = function () { helper(); };\n  this.y = function () { helper(); };\n  if (c) { this.no = function () {}; }\n}\nclass K { init() { const p = {}; p.px = () => {}; this.cb = function () { helper(); }; } }\nconst arrow = () => { this.ax = function () {}; };\nreg(function () { this.cx = function () { helper(); }; });\n(function () { Foo.prototype.inIife = function () {}; })();\n",
        ),
        (
            "lit.ts",
            "function helper() {}\nconst CE = class { cm() { helper(); } };\nconst N1 = class Named { nm() {} };\nreg(class Reg { rm() {} });\nvar vo = { vm() { helper(); }, inner: { deep() {} }, arr: [{ am() {} }] };\nlate = { lm() {} };\nconst q = cond ? { qm() {} } : null;\nexports.K = class { kv() {} };\nmodule.exports = { mm() { helper(); } };\nexport default { dm() {} };\ncall({ cm2() { helper(); } });\nconst api = { run() {} };\nlet lo = { lom() {} };\nfunction f() { var local = { fm() {} }; return { rm2() {} }; }\nreg(() => { var inCb = { cbm() {} }; });\nvar tv: { vsig(): void } = null as any;\ntype TL = { tm(): void };\n",
        ),
        (
            "main.ts",
            "import { Circle, Item } from './shapes';\nimport * as ns from './other';\nimport type { T } from './t';\nexport { Item as Alias } from './other';\nexport * from './lib';\nclass Local {}\nexport function mk() { return new Local(); }\nexport function mkNoParens() { return new Local; }\nexport const arrow = () => new Local();\nexport function mkNs() { return new ns.Item(); }\nexport function mkMap() { return new Map(); }\nexport class Holder { make() { return new Circle(); } }\nconst top = new Circle();\ndescribe('x', () => { new Circle(); new Local(); });\n",
        ),
        (
            "dyn.ts",
            "const top = import(\"./a.js\");\nexport function f() { return import(\"./b\"); }\nexport const g = async () => (await import(\"./c\")).c;\nexport function t() { return import(`./d`); }\nconst r = require(\"./e\");\nexport function lazy() { const m = require(\"./f\"); return m; }\nexport function ext() { return import(\"lodash\"); }\nexport function twice() { import(\"./b\"); return import(\"./b\", { with: {} }); }\nfunction inner() { return import(\"./h\"); }\nexport function sub(n) { return import(`./${n}`); }\n",
        ),
        (
            "m.ts",
            "import('./a');\n\nconst x = 1; import('./b'); // import('./c')\n// import('./d')\nf(import('./e'), import('./f'));\n",
        ),
        (
            "t.ts",
            "export const LIMIT = 3;\nexport const { p1, p2: p3 } = obj;\nexport const E = function* () { p1(); };\nexport const c1: number;\nconst w1 = wrap(() => { function g1() {} });\n",
        ),
        (
            "d.ts",
            "@Dec export class X extends Base implements I { @Input() a: string; @Out() m(@Arg() p: Foo.Bar): Baz<Qux> {} }\nexport interface I extends J { p: Widget; }\nexport type U = A | B;\nexport enum En { A, B }\n",
        ),
        (
            "r.js",
            "// NOTE: debounce to avoid thrash\n// See ADR-0001 and RFC 793 for the retry policy.\n/**\n * WHY: capped\n */\nexport const boot = () => { return 1; };\n",
        ),
        (
            "v.tsx",
            "export function App(): JSX.Element { return <div>{foo()}</div>; }\nexport const B = () => <A x={1} />;\n",
        ),
    ];
    for &(rel, src) in SRCS {
        let lang = if rel.ends_with(".js") {
            gositter_grammars::javascript::language()
        } else if rel.ends_with(".tsx") {
            gositter_grammars::tsx::language()
        } else {
            gositter_grammars::typescript::language()
        };
        let res = crate::extract::javascript::extract_js_file(rel, src.as_bytes(), lang);
        assert!(res.is_some(), "{rel}: extract_js_file is None");
        let res = file_from_bytes(rel, src.as_bytes());
        assert!(!res.nodes.is_empty(), "{rel}: no file node");
    }

    const SFCS: [(&str, &str, bool); 3] = [
        (
            "components/Greeter.vue",
            "<template>\n  <component :is=\"() => import('./LazyWidget.vue')\" />\n  <div>{{ greet() }}</div>\n</template>\n<script setup lang=\"ts\">\nimport { helper } from './helper'\nimport { ref } from 'vue'\n\nfunction greet(): string {\n  return helper('hi')\n}\n</script>\n<style scoped>\n.foo { color: red; }\n</style>\n",
            false,
        ),
        (
            "a/B.svelte",
            "<script>\n  import C from './C.svelte';\n  const load = () => import('./D.svelte');\n  function go() { load(); }\n</script>\n<C />\n",
            false,
        ),
        (
            "pages/p.astro",
            "---\nimport L from '../L.astro';\nconst t: string = 'x';\n---\n<L title={t} />\n<script>import('./client');</script>\n",
            true,
        ),
    ];
    for (rel, src, astro) in SFCS {
        let res = crate::extract::component::extract_component(rel, src.as_bytes(), astro);
        assert!(res.is_some(), "{rel}: extract_component is None");
        let res = file_from_bytes(rel, src.as_bytes());
        assert!(!res.nodes.is_empty(), "{rel}: no file node");
    }
}
