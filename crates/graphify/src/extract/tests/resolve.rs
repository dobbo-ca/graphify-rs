//! graphify-go `internal/extract/resolve_test.go`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc;
use std::time::Duration;

use crate::extract::resolve::{IMP_PART, self_target};
use crate::extract::tests::util::want_edges;
use crate::extract::{Call, Def, Imp, MdRef, ModRef, Result, resolve, resolve_parts};
use crate::idutil::make_id;
use crate::model::line;

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

fn def(id: &str, name: &str, file: &str) -> Def {
    Def {
        id: id.to_string(),
        name: name.to_string(),
        file: file.to_string(),
        ..Def::default()
    }
}

fn call(caller_id: u32, callee: u32, loc: i64) -> Call {
    Call {
        caller_id,
        callee,
        loc: line(loc),
        ..Call::default()
    }
}

/// Go `hasCall`: scans resolved edges for a src->tgt calls edge.
fn has_call(edges: &[(String, String, String)], src: &str, tgt: &str) -> bool {
    edges
        .iter()
        .any(|(s, r, t)| r == "calls" && s == src && t == tgt)
}

/// The edges of `resolve` as (source, relation, target) ids (Go's `edge` list).
fn resolved(results: &[Result], files: &[&str]) -> Vec<(String, String, String)> {
    let files = strs(files);
    resolve(results, &files, workers())
        .edges
        .iter()
        .map(|e| {
            (
                e.source.clone(),
                e.relation.as_str().to_string(),
                e.target.clone(),
            )
        })
        .collect()
}

// TestResolveAmbiguousByImport checks that a call to a name defined in two files
// resolves to the one the caller actually imports, instead of being skipped.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_ambiguous_by_import() {
    let results = vec![
        Result {
            defs: vec![def("A", "helper", "a/util.js")],
            ..Result::default()
        },
        Result {
            defs: vec![def("B", "helper", "b/util.js")],
            ..Result::default()
        },
        Result {
            defs: vec![def("MAIN", "run", "main.js")],
            calls: vec![call(1, 2, 2)],
            call_text: strs(&["", "MAIN", "helper"]),
            file: "main.js".to_string(),
            imps: vec![Imp {
                file_id: "mainfile".to_string(),
                file: "main.js".to_string(),
                spec: "./a/util".to_string(),
                loc: line(1),
                ..Imp::default()
            }],
            ..Result::default()
        },
    ];
    let es = resolved(&results, &["a/util.js", "b/util.js", "main.js"]);
    assert!(
        has_call(&es, "MAIN", "A"),
        "expected MAIN --calls--> A (the imported helper)"
    );
    assert!(
        !has_call(&es, "MAIN", "B"),
        "did not expect MAIN --calls--> B (not imported)"
    );
}

// TestResolveNoCrossFamilyCall checks that a call whose name is uniquely defined
// only in a different language family never binds: a Python call to `process`
// must not resolve to a Java `process` method (a phantom name collision), while
// a call resolvable within its own family still binds.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_no_cross_family_call() {
    let results = vec![
        Result {
            defs: vec![def("JAVA", "process", "Svc.java")],
            ..Result::default()
        },
        Result {
            defs: vec![def("PYHELPER", "helper", "util.py")],
            ..Result::default()
        },
        Result {
            defs: vec![def("PYMAIN", "run", "main.py")],
            file: "main.py".to_string(),
            calls: vec![
                call(1, 2, 2), // cross-family, must not bind
                call(1, 3, 3), // same-family, must bind
            ],
            call_text: strs(&["", "PYMAIN", "process", "helper"]),
            ..Result::default()
        },
    ];
    let es = resolved(&results, &["Svc.java", "util.py", "main.py"]);
    assert!(
        !has_call(&es, "PYMAIN", "JAVA"),
        "did not expect PYMAIN --calls--> JAVA (cross language family)"
    );
    assert!(
        has_call(&es, "PYMAIN", "PYHELPER"),
        "expected PYMAIN --calls--> PYHELPER (same language family)"
    );
}

// TestResolveAmbiguousBySameDir checks the same-package (same directory)
// tiebreaker: an ambiguous name with one definition in the caller's directory
// resolves there, mirroring same-package calls in Go.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_ambiguous_by_same_dir() {
    let results = vec![
        Result {
            defs: vec![def("LOCAL", "doIt", "pkg/a.go")],
            ..Result::default()
        },
        Result {
            defs: vec![def("FAR", "doIt", "other/b.go")],
            ..Result::default()
        },
        Result {
            defs: vec![def("CALLER", "run", "pkg/c.go")],
            calls: vec![call(1, 2, 3)],
            call_text: strs(&["", "CALLER", "doIt"]),
            file: "pkg/c.go".to_string(),
            ..Result::default()
        },
    ];
    let es = resolved(&results, &["pkg/a.go", "other/b.go", "pkg/c.go"]);
    assert!(
        has_call(&es, "CALLER", "LOCAL"),
        "expected CALLER --calls--> LOCAL (same directory)"
    );
    assert!(
        !has_call(&es, "CALLER", "FAR"),
        "did not expect CALLER --calls--> FAR (different directory)"
    );
}

// TestResolveSameNameMethodsInOneFile checks that when one file declares two
// types each owning a method of the same name, an unqualified call to that name
// binds to neither (drop-on-ambiguity) instead of the last one parsed, while an
// unambiguous same-file name still resolves.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_same_name_methods_in_one_file() {
    let results = vec![Result {
        defs: vec![
            def("x_a_build", "Build", "x.cs"),
            def("x_a_helper", "Helper", "x.cs"),
            def("x_b_helper", "Helper", "x.cs"),
            def("x_b_only", "Only", "x.cs"),
        ],
        file: "x.cs".to_string(),
        calls: vec![call(1, 2, 3), call(1, 3, 4)],
        call_text: strs(&["", "x_a_build", "Helper", "Only"]),
        ..Result::default()
    }];
    let es = resolved(&results, &["x.cs"]);
    assert!(
        !has_call(&es, "x_a_build", "x_b_helper"),
        "did not expect x_a_build --calls--> x_b_helper (ambiguous same-file name)"
    );
    assert!(
        !has_call(&es, "x_a_build", "x_a_helper"),
        "did not expect a guessed edge to x_a_helper either; the name is ambiguous"
    );
    assert!(
        has_call(&es, "x_a_build", "x_b_only"),
        "expected x_a_build --calls--> x_b_only (unambiguous same-file name)"
    );
}

/// Go's `c` in `TestResolveReceiverCalls`: a `calls` edge between two labels.
fn c<'a>(src: &'a str, tgt: &'a str) -> [&'a str; 3] {
    [src, "calls", tgt]
}

/// One case of `TestResolveReceiverCalls` (Go `t.Run`; the name is printed for a failure).
fn receiver_case(name: &str, srcs: &[(&str, &str)], want: &[[&str; 3]], unwanted: &[[&str; 3]]) {
    println!("case: {name}");
    want_edges(srcs, want, unwanted);
}

// TestResolveReceiverCalls checks that a member or qualified call binds on its
// receiver, not on the bare method name.
#[test]
#[ignore = "graphify-rs-ihc.39"] // also needs resolve (ihc.18)
fn test_resolve_receiver_calls() {
    crate::extract::kinds::register_kinds();
    receiver_case(
        "go stdlib package",
        &[(
            "a.go",
            "package a\nimport \"errors\"\nfunc New() int { return 1 }\nfunc f() error { return errors.New(\"x\") }\n",
        )],
        &[],
        &[c("f()", "New()")],
    );
    receiver_case(
        "rust foreign type",
        &[(
            "a.rs",
            "struct Config;\nimpl Config { fn new() -> Config { Config } }\nfn a() { let _m = HashMap::new(); }\nfn b() { let _c = Config::new(); }\n",
        )],
        &[c("b()", "Config.new()")],
        &[c("a()", "Config.new()")],
    );
    receiver_case(
        "ts local variable",
        &[(
            "a.ts",
            "function push(x: number) {}\nfunction f(arr: number[]) { arr.push(1) }\n",
        )],
        &[],
        &[c("f()", "push()")],
    );
    receiver_case(
        "js other file",
        &[
            (
                "lib.js",
                "export function format() {}\nexport function get() {}\n",
            ),
            (
                "app.js",
                "function h(res) { res.format({}); axios.get() }\n",
            ),
        ],
        &[],
        &[c("h()", "format()"), c("h()", "get()")],
    );
    receiver_case(
        "python attribute chain",
        &[
            ("pkg/b.py", "def get():\n    pass\n"),
            (
                "a.py",
                "class C:\n    def run(self):\n        self.session.get()\n",
            ),
        ],
        &[],
        &[c("C.run()", "get()")],
    );
    receiver_case(
        "js this in unrelated class",
        &[(
            "a.js",
            "class A { run() { this.m() } }\nclass B { m() {} }\n",
        )],
        &[],
        &[c("A.run()", "B.m()")],
    );
    receiver_case(
        "python self in unrelated class",
        &[(
            "a.py",
            "class A:\n    def run(self):\n        self.save()\nclass B:\n    def save(self):\n        pass\n",
        )],
        &[],
        &[c("A.run()", "B.save()")],
    );
    receiver_case(
        "go corpus package",
        &[
            ("util/join.go", "package util\nfunc Join() {}\n"),
            ("main.go", "package main\nfunc main() { util.Join() }\n"),
        ],
        &[c("main()", "Join()")],
        &[],
    );
    receiver_case(
        "go receiver",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) a() { s.b() }\nfunc (s *S) b() {}\nfunc (t *T) b() {}\n",
        )],
        &[c("S.a()", "S.b()")],
        &[c("S.a()", "T.b()")],
    );
    receiver_case(
        "java super",
        &[(
            "A.java",
            "class Base { void log() {} }\nclass A extends Base { void go() { super.log(); } }\n",
        )],
        &[c("A.go()", "Base.log()")],
        &[],
    );
    receiver_case(
        "python inherited self",
        &[(
            "a.py",
            "class B:\n    def save(self):\n        pass\nclass A(B):\n    def run(self):\n        self.save()\n",
        )],
        &[c("A.run()", "B.save()")],
        &[],
    );
    receiver_case(
        "python unknown ancestor",
        &[(
            "a.py",
            "class B:\n    def save(self):\n        pass\nclass A(Ext, B):\n    def run(self):\n        self.save()\n",
        )],
        &[],
        &[c("A.run()", "B.save()")],
    );
    receiver_case(
        "lua self",
        &[(
            "a.lua",
            "local A = {}\nlocal B = {}\nfunction A:step() end\nfunction B:step() end\nfunction A:run() self:step() end\n",
        )],
        &[c("A.run()", "A.step()")],
        &[c("A.run()", "B.step()")],
    );
    receiver_case(
        "python typed parameter",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\nclass Other:\n    def fetch(self):\n        pass\ndef use(c: Client):\n    return c.fetch()\n",
        )],
        &[c("use()", "Client.fetch()")],
        &[c("use()", "Other.fetch()")],
    );
    receiver_case(
        "python constructor binding",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\nclass Other:\n    def fetch(self):\n        pass\ndef use():\n    c = Client()\n    c.fetch()\n",
        )],
        &[c("use()", "Client.fetch()")],
        &[c("use()", "Other.fetch()")],
    );
    receiver_case(
        "python rebound local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\nclass Other:\n    def fetch(self):\n        pass\ndef use(x):\n    c = Client()\n    c = x\n    c.fetch()\n",
        )],
        &[],
        &[c("use()", "Client.fetch()"), c("use()", "Other.fetch()")],
    );
    receiver_case(
        "go typed parameter",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t *T) b() {}\nfunc use(x *S) { x.b() }\n",
        )],
        &[c("use()", "S.b()")],
        &[c("use()", "T.b()")],
    );
    receiver_case(
        "go package-qualified parameter",
        &[
            (
                "m/m.go",
                "package m\ntype G struct{}\ntype H struct{}\nfunc (g *G) Deg() {}\nfunc (h *H) Deg() {}\n",
            ),
            ("a/a.go", "package a\nfunc use(g *m.G) { g.Deg() }\n"),
        ],
        &[c("use()", "G.Deg()")],
        &[c("use()", "H.Deg()")],
    );
    receiver_case(
        "go var declaration",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t *T) b() {}\nfunc use() { var x S; x.b() }\n",
        )],
        &[c("use()", "S.b()")],
        &[c("use()", "T.b()")],
    );
    receiver_case(
        "go same-file constructor",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t *T) b() {}\nfunc newS() *S { return &S{} }\nfunc use() { x := newS(); x.b() }\n",
        )],
        &[c("use()", "S.b()")],
        &[c("use()", "T.b()")],
    );
    receiver_case(
        "go composite literal",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t T) b() {}\nfunc use() { x := &S{}; y := T{}; x.b(); y.b() }\n",
        )],
        &[c("use()", "S.b()"), c("use()", "T.b()")],
        &[],
    );
    receiver_case(
        "python for target shadows typed local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\ndef looped(c: Client, items):\n    for c in items:\n        c.fetch()\n",
        )],
        &[],
        &[c("looped()", "Client.fetch()")],
    );
    receiver_case(
        "python comprehension target shadows typed local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\ndef comp(c: Client, items):\n    return [c.fetch() for c in items]\n",
        )],
        &[],
        &[c("comp()", "Client.fetch()")],
    );
    receiver_case(
        "python with target shadows typed local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\ndef withed(c: Client):\n    with opener() as c:\n        c.fetch()\n",
        )],
        &[],
        &[c("withed()", "Client.fetch()")],
    );
    receiver_case(
        "python tuple target shadows typed local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\ndef tupled(c: Client):\n    c, d = pair()\n    c.fetch()\n",
        )],
        &[],
        &[c("tupled()", "Client.fetch()")],
    );
    receiver_case(
        "python walrus shadows typed local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\ndef walrus(c: Client):\n    if (c := pick()):\n        c.fetch()\n",
        )],
        &[],
        &[c("walrus()", "Client.fetch()")],
    );
    receiver_case(
        "python attribute assignment keeps typed local",
        &[(
            "a.py",
            "class Client:\n    def fetch(self):\n        pass\nclass Other:\n    def fetch(self):\n        pass\ndef use(c: Client):\n    c.x, c.y = 1, 2\n    c.fetch()\n",
        )],
        &[c("use()", "Client.fetch()")],
        &[],
    );
    receiver_case(
        "go range shadows typed local",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t *T) b() {}\nfunc ranged(s S, ts []T) { for _, s := range ts { s.b() } }\n",
        )],
        &[],
        &[c("ranged()", "S.b()")],
    );
    receiver_case(
        "go var shadows typed local",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t *T) b() {}\nfunc shadowVar(s S) { { var s T; s.b() } }\n",
        )],
        &[],
        &[c("shadowVar()", "S.b()")],
    );
    receiver_case(
        "go type switch shadows typed local",
        &[(
            "a.go",
            "package a\ntype S struct{}\ntype T struct{}\nfunc (s *S) b() {}\nfunc (t *T) b() {}\nfunc tswitch(s *S, v interface{}) { switch s := v.(type) { case T: s.b() } }\n",
        )],
        &[],
        &[c("tswitch()", "S.b()")],
    );
    receiver_case(
        "ruby bare self-send stays in its class",
        &[(
            "a.rb",
            "class A\n  def helper\n  end\nend\nclass B\n  def run\n    helper\n  end\nend\n",
        )],
        &[],
        &[c("B.run()", "A.helper()")],
    );
    receiver_case(
        "ruby bare self-send picks its own class",
        &[(
            "a.rb",
            "class A\n  def dup_name\n  end\nend\nclass B\n  def dup_name\n  end\n  def run\n    dup_name\n  end\nend\n",
        )],
        &[c("B.run()", "B.dup_name()")],
        &[c("B.run()", "A.dup_name()")],
    );
    receiver_case(
        "ruby bare self-send inherited",
        &[(
            "a.rb",
            "class A\n  def helper\n  end\nend\nclass B < A\n  def run\n    helper\n  end\nend\n",
        )],
        &[c("B.run()", "A.helper()")],
        &[],
    );
    receiver_case(
        "ruby bare call from a top-level method",
        &[("a.rb", "def helper\nend\ndef run\n  helper\nend\n")],
        &[c("run()", "helper()")],
        &[],
    );
}

// TestSelfTargetCyclicInherits checks that a cyclic inherits graph is walked
// once per class instead of fanning out at every level.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_self_target_cyclic_inherits() {
    let ids = ["a", "b", "c", "d", "e", "f"];
    let mut supers: HashMap<String, Vec<String>> = HashMap::new();
    for c in ids {
        for o in ids {
            if o != c {
                supers.entry(c.to_string()).or_default().push(o.to_string());
            }
        }
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let got = self_target(
            "a",
            "missing",
            false,
            &HashMap::new(),
            &supers,
            &HashSet::new(),
        );
        let _ = tx.send(got);
    });
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(got) => assert_eq!(got, "", "self_target = {got:?}, want no target"),
        Err(e) => panic!("self_target did not finish on a 6-class inherits cycle: {e}"),
    }
}

// TestResolveImportEdgesAcrossBatches checks the import edges of a corpus that
// spans several target batches and several edge parts: order, and a TypeOnly
// that a later value import of the same target clears in an earlier part.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_import_edges_across_batches() {
    let n = 3 * IMP_PART / 2 + 7;
    let mut files = vec!["t.ts".to_string()];
    let mut results = vec![Result::default()];
    // (source, target, relation, type only)
    let mut want: Vec<(String, String, String, bool)> = Vec::new();
    for i in 0..n {
        let f = format!("f{i}.ts");
        let id = format!("f{i}");
        files.push(f.clone());
        results.push(Result {
            imps: vec![
                Imp {
                    file_id: id.clone(),
                    file: f.clone(),
                    spec: "./t".to_string(),
                    type_only: true,
                    ..Imp::default()
                },
                Imp {
                    file_id: id.clone(),
                    file: f.clone(),
                    spec: "lib".to_string(),
                    ..Imp::default()
                },
            ],
            ..Result::default()
        });
        // Every third file imports the target again, as a value.
        let again = i % 3 == 0;
        want.push((id.clone(), "t_ts".into(), "imports_from".into(), !again));
        want.push((id.clone(), "lib".into(), "imports".into(), false));
        if again {
            results.push(Result {
                imps: vec![Imp {
                    file_id: id,
                    file: f,
                    spec: "./t".to_string(),
                    ..Imp::default()
                }],
                ..Result::default()
            });
            files.push(format!("g{i}.ts"));
        }
    }
    let got: Vec<_> = resolve(&results, &files, workers())
        .edges
        .iter()
        .map(|e| {
            (
                e.source.clone(),
                e.target.clone(),
                e.relation.as_str().to_string(),
                e.type_only(),
            )
        })
        .collect();
    assert!(
        got == want,
        "import edges differ: got {}, want {}",
        got.len(),
        want.len()
    );
}

// TestResolveCallEdgesAcrossParts checks the call edges of a run that fills
// more than one part: no part above impPart, and the order of the calls.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_call_edges_across_parts() {
    let n = 3 * IMP_PART / 2 + 7;
    let mut r = Result {
        defs: vec![def("helper", "helper", "a.go")],
        file: "a.go".to_string(),
        call_text: strs(&["", "helper"]),
        ..Result::default()
    };
    let mut want: Vec<String> = Vec::new();
    for i in 0..n {
        let id = format!("c{i}");
        r.call_text.push(id.clone());
        r.calls.push(call(i as u32 + 2, 1, 1));
        want.push(id);
    }
    let (_, parts) = resolve_parts(&[r], &strs(&["a.go"]), workers());
    let mut got: Vec<String> = Vec::new();
    for p in &parts {
        for e in p {
            if e.relation.as_str() != "calls" {
                continue;
            }
            assert!(
                p.len() <= IMP_PART,
                "part of {} call edges, want at most {IMP_PART}",
                p.len()
            );
            got.push(e.source.clone());
        }
    }
    assert!(
        got == want,
        "call edges differ: got {}, want {}",
        got.len(),
        want.len()
    );
}

// TestResolveImportedSetPerFile: the set of imported files is made per caller
// file inside one run, so a file that imports nothing must not bind an
// ambiguous name through what the file before it imports.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_imported_set_per_file() {
    // Go sets GOMAXPROCS to 1: one run for all files.
    let files = strs(&["p/x.ts", "q/y.ts", "r/a.ts", "s/b.ts"]);
    let results = vec![
        Result {
            defs: vec![def("x_f", "f", "p/x.ts")],
            ..Result::default()
        },
        Result {
            defs: vec![def("y_f", "f", "q/y.ts")],
            ..Result::default()
        },
        Result {
            imps: vec![Imp {
                file_id: "a".to_string(),
                file: "r/a.ts".to_string(),
                spec: "../p/x".to_string(),
                ..Imp::default()
            }],
            calls: vec![call(1, 2, 2)],
            call_text: strs(&["", "a_main", "f"]),
            file: "r/a.ts".to_string(),
            ..Result::default()
        },
        Result {
            calls: vec![call(1, 2, 2)],
            call_text: strs(&["", "b_main", "f"]),
            file: "s/b.ts".to_string(),
            ..Result::default()
        },
    ];
    let got: Vec<String> = resolve(&results, &files, 1)
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "calls")
        .map(|e| format!("{}>{}", e.source, e.target))
        .collect();
    assert_eq!(got, vec!["a_main>x_f"], "call edges = {got:?}");
}

// TestResolveImportEdgesDedupByTargetID: two files whose paths make the same
// id are one import target for the dedupe.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_import_edges_dedup_by_target_id() {
    let files = strs(&["a-b.ts", "a_b.ts", "m.ts"]);
    assert_eq!(
        make_id(&[files[0].as_str()]),
        make_id(&[files[1].as_str()]),
        "the two paths no longer make one id"
    );
    let results = vec![
        Result::default(),
        Result::default(),
        Result {
            imps: vec![
                Imp {
                    file_id: "m".to_string(),
                    file: "m.ts".to_string(),
                    spec: "./a-b".to_string(),
                    type_only: true,
                    ..Imp::default()
                },
                Imp {
                    file_id: "m".to_string(),
                    file: "m.ts".to_string(),
                    spec: "./a_b".to_string(),
                    ..Imp::default()
                },
            ],
            ..Result::default()
        },
    ];
    let got: Vec<String> = resolve(&results, &files, workers())
        .edges
        .iter()
        .map(|e| {
            format!(
                "{}>{} {} {}",
                e.source,
                e.target,
                e.relation.as_str(),
                e.type_only()
            )
        })
        .collect();
    assert_eq!(
        got,
        vec!["m>a_b_ts imports_from false"],
        "import edges = {got:?}"
    );
}

// TestResolveMarkdownEdgesAfterModuleRefs pins the order of the tail edges:
// the markdown pass runs beside the calls, and its edges still come last.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_markdown_edges_after_module_refs() {
    let results = vec![
        Result {
            mod_refs: vec![ModRef {
                from_id: "M".to_string(),
                file: "main.tf".to_string(),
                source: "acme/vpc/aws".to_string(),
                ..ModRef::default()
            }],
            ..Result::default()
        },
        Result {
            md_refs: vec![MdRef {
                from_id: "D".to_string(),
                file: "a.md".to_string(),
                target: "b.md".to_string(),
                ..MdRef::default()
            }],
            ..Result::default()
        },
    ];
    let got: Vec<String> = resolve(&results, &strs(&["main.tf", "a.md", "b.md"]), workers())
        .edges
        .iter()
        .map(|e| e.source.clone())
        .collect();
    assert_eq!(got, vec!["M", "D"], "edge sources {got:?}, want [M D]");
}

// TestResolveEdgeFiles checks that an edge Resolve makes names the file and
// line of the record it came from, across files.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_resolve_edge_files() {
    let imp = |file_id: &str, file: &str, l: i64| Imp {
        file_id: file_id.to_string(),
        file: file.to_string(),
        spec: "./util".to_string(),
        loc: line(l),
        ..Imp::default()
    };
    let results = vec![
        Result {
            defs: vec![def("U", "helper", "util.js")],
            ..Result::default()
        },
        Result {
            defs: vec![def("A", "a", "a.js")],
            calls: vec![call(1, 2, 5)],
            call_text: strs(&["", "A", "helper"]),
            file: "a.js".to_string(),
            imps: vec![imp("afile", "a.js", 1)],
            ..Result::default()
        },
        Result {
            defs: vec![def("B", "b", "b.js")],
            calls: vec![call(1, 2, 7)],
            call_text: strs(&["", "B", "helper"]),
            file: "b.js".to_string(),
            imps: vec![imp("bfile", "b.js", 2)],
            ..Result::default()
        },
    ];
    let mut got: BTreeMap<String, String> = BTreeMap::new();
    for e in &resolve(&results, &strs(&["util.js", "a.js", "b.js"]), workers()).edges {
        got.insert(
            format!("{} {}", e.source, e.relation.as_str()),
            format!("{}:{}", e.source_file, e.source_location),
        );
    }
    let want = [
        ("A calls", "a.js:L5"),
        ("B calls", "b.js:L7"),
        ("afile imports_from", "a.js:L1"),
        ("bfile imports_from", "b.js:L2"),
    ];
    for (k, v) in want {
        assert_eq!(
            got.get(k).map(String::as_str),
            Some(v),
            "{k}: want {v:?} (all: {got:?})"
        );
    }
}
