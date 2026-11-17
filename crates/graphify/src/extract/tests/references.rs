//! graphify-go `internal/extract/references_test.go`.

use std::collections::{BTreeSet, HashMap};

use crate::extract::tests::util::resolve_src;
use crate::extract::{Def, Result, TypeRef, resolve};
use crate::idutil::make_id;
use crate::model::{Confidence, Node, Relation};

type Nodes = BTreeSet<[String; 2]>;
type Edges = BTreeSet<[String; 3]>;

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Go `refResult`: a hand-built file; its defs (a label ending in "()" is a function) and the
/// names it references from the function f().
fn ref_result(file: &str, defs: &[&str], refs: &[TypeRef]) -> Result {
    let fid = make_id(&[file]);
    let mut r = Result {
        file: file.to_string(),
        nodes: vec![Node {
            id: fid,
            label: file.to_string(),
            file_type: "code".to_string(),
            source_file: file.to_string(),
            ..Node::default()
        }],
        ..Result::default()
    };
    let labels = std::iter::once("f()").chain(defs.iter().copied());
    for label in labels {
        let name = match label.strip_suffix("()") {
            Some(n) if label.len() > 2 => n,
            _ => label,
        };
        let id = make_id(&[file, name]);
        r.nodes.push(Node {
            id: id.clone(),
            label: label.to_string(),
            file_type: "code".to_string(),
            source_file: file.to_string(),
            ..Node::default()
        });
        r.defs.push(Def {
            id,
            name: name.to_string(),
            file: file.to_string(),
            ..Def::default()
        });
    }
    for t in refs {
        let mut t = t.clone();
        t.from_id = make_id(&[file, "f"]);
        t.file = file.to_string();
        t.stub = true;
        if t.relation.is_empty() {
            t.relation = "references".to_string();
        }
        r.type_refs.push(t);
    }
    r
}

fn tr(name: &str) -> TypeRef {
    TypeRef {
        name: name.to_string(),
        ..TypeRef::default()
    }
}

fn tr_rel(name: &str, relation: &str) -> TypeRef {
    TypeRef {
        relation: relation.to_string(),
        ..tr(name)
    }
}

fn tr_deco(name: &str) -> TypeRef {
    TypeRef {
        deco: true,
        ..tr(name)
    }
}

fn s3(w: &[&str; 3]) -> [String; 3] {
    [w[0].to_string(), w[1].to_string(), w[2].to_string()]
}

/// Go's loops over `want` and `unwanted` edges: every `want` edge is there and no `unwanted` one.
fn check_edges(edges: &Edges, want: &[[&str; 3]], unwanted: &[[&str; 3]]) {
    let missing: Vec<_> = want.iter().filter(|w| !edges.contains(&s3(w))).collect();
    assert!(missing.is_empty(), "missing edges {missing:?}");
    let found: Vec<_> = unwanted.iter().filter(|w| edges.contains(&s3(w))).collect();
    assert!(found.is_empty(), "unexpected edges {found:?}");
}

/// Go's loop over labels that must have no stub (a node of no source file).
fn check_no_stubs(nodes: &Nodes, labels: &[&str]) {
    let found: Vec<_> = labels
        .iter()
        .filter(|l| nodes.contains(&[String::new(), (**l).to_string()]))
        .collect();
    assert!(found.is_empty(), "unexpected stubs {found:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_references_bind_or_stub() {
    let results = vec![
        ref_result(
            "a/x.py",
            &["Local", "Dup", "Early"],
            &[
                tr("Local"),                // same file
                tr("Uniq"),                 // one definition in the corpus
                tr("Dup"),                  // same file wins over b/y.py
                tr("Gone"),                 // no definition: stub
                tr("Gone"),                 // one stub per file and name
                tr("helper"),               // a function is not a type
                tr_deco("helper"),          // but it is a decorator
                tr("earlY"),                // stub id folds onto Early
                tr_deco("f"),               // self reference
                tr_rel("Base", "inherits"), // unresolved base
            ],
        ),
        ref_result(
            "b/y.py",
            &["Uniq", "Dup", "helper()"],
            &[tr("Gone"), tr("Amb")],
        ),
        ref_result("c/z.py", &["Amb"], &[]),
        ref_result("d/z.py", &["Amb"], &[]),
    ];
    let files: Vec<String> = ["a/x.py", "b/y.py", "c/z.py", "d/z.py"]
        .iter()
        .map(|f| f.to_string())
        .collect();
    let ext = resolve(&results, &files, workers());

    let mut label: HashMap<&str, [&str; 2]> = HashMap::new();
    let mut stubs: HashMap<&str, usize> = HashMap::new();
    for n in &ext.nodes {
        label.insert(n.id.as_str(), [n.source_file.as_str(), n.label.as_str()]);
        if n.source_file.is_empty() {
            assert_eq!(
                n.file_type, "code",
                "stub {} has file type {:?}, want code",
                n.id, n.file_type
            );
            *stubs.entry(n.id.as_str()).or_default() += 1;
        }
    }
    let want_stubs = [
        make_id(&["a/x.py", "Gone"]),
        make_id(&["a/x.py", "helper"]),
        make_id(&["a/x.py", "Base"]),
        make_id(&["b/y.py", "Gone"]),
        make_id(&["b/y.py", "Amb"]),
    ];
    for id in &want_stubs {
        let n = stubs.get(id.as_str()).copied().unwrap_or(0);
        assert_eq!(n, 1, "stub {id}: got {n} nodes, want 1");
    }
    assert_eq!(
        stubs.len(),
        want_stubs.len(),
        "got stubs {stubs:?}, want {want_stubs:?}"
    );

    let mut got: BTreeSet<[String; 3]> = BTreeSet::new();
    for e in &ext.edges {
        if e.relation == Relation::CONTAINS {
            continue;
        }
        let none = ["", ""];
        let tgt = label.get(e.target.as_str()).unwrap_or(&none);
        let src = label.get(e.source.as_str()).unwrap_or(&none);
        got.insert([
            src[0].to_string(),
            e.relation.as_str().to_string(),
            format!("{}:{}", tgt[0], tgt[1]),
        ]);
        if e.relation == Relation::REFERENCES {
            assert_eq!(
                e.confidence,
                Confidence::EXTRACTED,
                "references edge to {tgt:?} is not EXTRACTED"
            );
        }
    }
    let want = [
        ["a/x.py", "references", "a/x.py:Local"],
        ["a/x.py", "references", "b/y.py:Uniq"],
        ["a/x.py", "references", "a/x.py:Dup"],
        ["a/x.py", "references", ":Gone"],
        ["a/x.py", "references", ":helper"],
        ["a/x.py", "references", "b/y.py:helper()"],
        ["a/x.py", "inherits", ":Base"],
        ["b/y.py", "references", ":Gone"],
        ["b/y.py", "references", ":Amb"],
    ];
    for w in &want {
        assert!(got.remove(&s3(w)), "missing edge {w:?}");
    }
    assert!(got.is_empty(), "unexpected edges {got:?}");
}

// An inherits TypeRef without Stub drops when its base is outside the corpus.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_unresolved_base_without_stub_drops() {
    let mut r = ref_result("a/x.py", &[], &[tr_rel("Base", "inherits")]);
    r.type_refs[0].stub = false;
    let ext = resolve(&[r], &["a/x.py".to_string()], workers());
    for n in &ext.nodes {
        assert!(!n.source_file.is_empty(), "unexpected stub {n:?}");
    }
}

// Upstream 0.9.88 on the same two files (fixtures pyx, py1): parameter and
// return annotations and decorators give references; a plain unresolved base
// and an unresolved name give a sourceless stub.
#[test]
#[ignore = "graphify-rs-ihc.25"] // also needs resolve (ihc.18)
fn test_python_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "pkg/m.py",
            "class Real:\n    pass\n\n\ndef traced(f):\n    return f\n",
        ),
        (
            "pkg/a.py",
            r#"from typing import Optional, TypeVar
from .m import Real, traced
from .m import Real as R

T = TypeVar("T")


class K(Exception, Generic[T], em.Base):
    @staticmethod
    @traced
    def run(self, x: T, y: "Real", *a: Va, k: Optional[Real] = None) -> Later:
        return None

    @property
    def p(self) -> int:
        return 1

    @p.setter
    def p(self, v: em.Thing):
        pass


class Later:
    pass


@other.deco(1)
@functools.wraps(K)
async def f(a: list[Real], b: traced) -> dict[str, Gone]:
    return {}


class S(R):
    def m(self, x: Annotated[R, Field(gt=0)], y: _):
        pass
"#,
        ),
        (
            "pkg/b.py",
            r#"from .m import Real as Base
from .m import traced as Late
from .a import Later as Late


class Real(Base):
    def clone(self, other: Base, l: Late) -> Base:
        return other
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["pkg/a.py:K", "inherits", ":Exception"],
            ["pkg/a.py:K.run()", "references", "pkg/m.py:traced()"],
            ["pkg/a.py:K.run()", "references", ":Va"],
            ["pkg/a.py:K.run()", "references", "pkg/m.py:Real"],
            ["pkg/a.py:K.run()", "references", "pkg/a.py:Later"],
            ["pkg/a.py:K.p()", "references", ":Thing"],
            ["pkg/a.py:f()", "references", ":deco"],
            ["pkg/a.py:f()", "references", "pkg/m.py:Real"],
            ["pkg/a.py:f()", "references", ":Gone"],
            ["pkg/a.py:f()", "references", ":traced"], // a function is not a type
            ["pkg/a.py:S", "inherits", "pkg/m.py:Real"], // through the import alias
            ["pkg/a.py:S.m()", "references", "pkg/m.py:Real"],
            ["pkg/a.py:S.m()", "references", ":Field"],
            ["pkg/b.py:Real", "inherits", "pkg/m.py:Real"], // not the class of the same name here
            ["pkg/b.py:Real.clone()", "references", "pkg/m.py:Real"],
            ["pkg/b.py:Real.clone()", "references", "pkg/a.py:Later"], // the last import of Late
        ],
        // the name _ folds onto the file id and must give no edge
        &[["pkg/a.py:S.m()", "references", "pkg/a.py:a.py"]],
    );
    check_no_stubs(
        &nodes,
        &[
            "T",
            "Generic",
            "Base",
            "Optional",
            "int",
            "str",
            "dict",
            "list",
            "staticmethod",
            "property",
            "setter",
            "wraps",
            "p",
            "R",
            "gt",
            "_",
        ],
    );
}

// Upstream fixture r3o: the import statement of an alias names its module.
#[test]
#[ignore = "graphify-rs-ihc.25"] // also needs resolve (ihc.18)
fn test_python_alias_binds_its_own_module() {
    let (_, edges) = resolve_src(&[
        ("a/models.py", "class User:\n    pass\n"),
        ("b/models.py", "class User:\n    pass\n"),
        (
            "a/views.py",
            "from a.models import User as AUser\nfrom b.models import User as BUser\n\n\nclass Remote(BUser):\n    pass\n\n\ndef handle(u: BUser, v: AUser):\n    pass\n",
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a/views.py:Remote", "inherits", "b/models.py:User"],
            ["a/views.py:handle()", "references", "b/models.py:User"],
            ["a/views.py:handle()", "references", "a/models.py:User"],
        ],
        &[],
    );
}

// Upstream fixtures r4p, r4o, r4q: a fallback alias from outside the corpus
// leaves the plain import in force; two imports on one line keep their own
// module; `from . import Name as N` is an alias too.
#[test]
#[ignore = "graphify-rs-ihc.25"] // also needs resolve (ihc.18)
fn test_python_alias_import_order() {
    let (_, edges) = resolve_src(&[
        ("pkg/__init__.py", "class Name:\n    pass\n"),
        (
            "pkg/fast.py",
            "class Foo:\n    pass\n\n\nclass X:\n    pass\n",
        ),
        ("pkg/c.py", "class X:\n    pass\n"),
        (
            "pkg/app.py",
            r#"try:
    from .fast import Foo
except ImportError:
    from slowext import SlowFoo as Foo
from .fast import X as P; from .c import X as R
from . import Name as N


class K(Foo):
    def m(self, p: P, r: R, n: N):
        pass
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["pkg/app.py:K", "inherits", "pkg/fast.py:Foo"],
            ["pkg/app.py:K.m()", "references", "pkg/fast.py:X"],
            ["pkg/app.py:K.m()", "references", "pkg/c.py:X"],
            ["pkg/app.py:K.m()", "references", "pkg/__init__.py:Name"],
        ],
        &[],
    );
}

// Upstream fixtures r6g, r6e: a plain import binds in its own module although
// another module has the name; a qualified name is not the file's alias.
#[test]
#[ignore = "graphify-rs-ihc.25"] // also needs resolve (ihc.18)
fn test_python_plain_import_and_qualified_name() {
    let (_, edges) = resolve_src(&[
        ("p1.py", "class Config:\n    pass\n"),
        (
            "p2.py",
            "class Config:\n    pass\n\n\nclass R:\n    pass\n\n\ndef load():\n    pass\n",
        ),
        ("lib.py", "class Real:\n    pass\n"),
        (
            "user.py",
            "import p2\nfrom p1 import Config\nfrom p2 import load\nfrom lib import Real as R\n\n\nclass Mine(Config):\n    pass\n\n\nclass KQ(p2.R):\n    pass\n\n\nclass KS(p2.R[int]):\n    pass\n",
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["user.py:Mine", "inherits", "p1.py:Config"],
            ["user.py:KQ", "inherits", "p2.py:R"],
            ["user.py:KS", "inherits", "p2.py:R"],
        ],
        &[],
    );
}

// Upstream 0.9.88 on fixture gox: signature and struct field types give
// references; pkg.T binds through the import path or gets a stub labelled
// with it. Not copied: the same-package Dup left on a stub, no edge for a
// variadic parameter.
#[test]
#[ignore = "graphify-rs-ihc.24"] // also needs resolve (ihc.18)
fn test_go_references() {
    let (nodes, edges) = resolve_src(&[
        ("go.mod", "example.com/m"),
        (
            "lib/t.go",
            "package lib\n\ntype Thing struct{}\n\ntype Dup struct{}\n",
        ),
        (
            "app/b.go",
            "package app\n\nfunc G(l Local, d Dup) Inner { return 0 }\n",
        ),
        (
            "app/a.go",
            "package app

import (
	\"context\"
	\"io\"

	lb \"example.com/m/lib\"
)

type Dup struct{}

type Inner int

type Gen[T any] struct{ V T }

type Num interface{ ~int | Inner }

type Tilde interface{ ~[]Dup }

type Emb struct {
	Gen[lb.Thing]
}

type Local struct {
	A lb.Thing
	B Gen[io.Reader]
	C struct{ In Inner }
	D func(ctx context.Context) (*Local, error)
	E []*lb.Dup
	F Dup
	G unknownpkg.T
	Inner
}

func (g *Gen[T]) Get(d Dup, x T) lb.Dup { return lb.Dup{} }

func F[U any](l *Local, u U, vs ...Inner) (n int, err error) { return }
",
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["app/a.go:Local", "references", "lib/t.go:Thing"],
            ["app/a.go:Local", "references", "app/a.go:Gen"],
            ["app/a.go:Local", "references", ":io.Reader"],
            ["app/a.go:Local", "references", "app/a.go:Inner"], // field C; the embedded Inner is no reference
            ["app/a.go:Local", "references", ":context.Context"],
            ["app/a.go:Local", "references", "lib/t.go:Dup"],
            ["app/a.go:Local", "references", "app/a.go:Dup"],
            ["app/a.go:Local", "references", ":unknownpkg.T"],
            ["app/a.go:Num", "references", "app/a.go:Inner"],
            ["app/a.go:Tilde", "references", "app/a.go:Dup"],
            ["app/a.go:Emb", "references", "lib/t.go:Thing"],
            ["app/a.go:Gen.Get()", "references", "app/a.go:Dup"],
            ["app/a.go:Gen.Get()", "references", "lib/t.go:Dup"],
            ["app/a.go:F()", "references", "app/a.go:Local"],
            ["app/a.go:F()", "references", "app/a.go:Inner"],
            ["app/b.go:G()", "references", "app/a.go:Dup"],
            ["app/b.go:G()", "references", "app/a.go:Local"],
        ],
        &[],
    );
    check_no_stubs(
        &nodes,
        &["T", "U", "int", "error", "Dup", "Reader", "Context"],
    );
}

// Upstream 0.9.88 on fixtures c1, c3, c6: a typedef name in a return or
// parameter type gives references; `struct S` and primitive types give
// nothing. Not copied: upstream has no struct node, so its target is a stub.
#[test]
#[ignore = "graphify-rs-ihc.18"] // also needs resolve (ihc.18)
fn test_c_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "types.h",
            "typedef struct Point { int x; } Point;\nstruct Box { Point origin; };\n",
        ),
        ("kv.h", "typedef struct _kv kv;\n"),
        (
            "kv.c",
            "#include \"kv.h\"\nstruct _kv { int n; };\nint kvlen(kv *k) { return 0; }\n",
        ),
        ("bench.c", "typedef struct kv { int other; } kv;\n"),
        (
            "main.c",
            r#"#include "types.h"
Point (paren)(Missing m) { Point p; return p; }
Point PREFIX(name3)(FILE *f) { Point p; return p; }
Point make_point(int x, struct Box *b) { Point p; return p; }
int area(const Point *p, Missing m, FILE **f, uint32_t n, size_t k, void (*cb)(Other)) { return 0; }
Pt (*getfp(Arg a))(Inner) { return 0; }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["main.c:make_point()", "references", "types.h:Point"],
            ["main.c:area()", "references", "types.h:Point"],
            ["main.c:area()", "references", ":Missing"],
            ["main.c:area()", "references", ":FILE"],
            ["main.c:getfp()", "references", ":Pt"],
            ["main.c:getfp()", "references", ":Arg"],
            ["main.c:paren()", "references", ":Missing"],
            ["main.c:PREFIX()", "references", ":FILE"],
            ["kv.c:kvlen()", "references", "kv.c:_kv"], // not the unrelated kv of bench.c
        ],
        // struct Box is a tag, not a typedef name
        &[["main.c:make_point()", "references", "types.h:Box"]],
    );
    check_no_stubs(
        &nodes,
        &[
            "uint32_t", "size_t", "Other", "Inner", "Box", "int", "name3", "kv",
        ],
    );
}

// Upstream 0.9.88 on fixtures cpp1, cpp2, cpp4, cpp7: return, parameter and
// field types and the template arguments of a base give references; an
// unresolved base gets a stub. Not copied: a stub for a template parameter.
#[test]
#[ignore = "graphify-rs-ihc.30"] // also needs resolve (ihc.18)
fn test_cpp_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "types.hpp",
            "namespace geo {\nclass Point {};\nclass Dep {};\ntemplate <class X> class Base {};\n}\n",
        ),
        ("sample/other.cpp", "class Point {};\nclass Base {};\n"),
        (
            "shape.cpp",
            r#"#include "types.hpp"
#include <vector>
class Shape : public geo::Base<geo::Dep>, public Missing {
  std::vector<geo::Point> pts;
  geo::Point *origin;
  Unknown (*cb)(int);
  geo::Dep decl(Hidden h);
  geo::Point center(const geo::Point &p, std::string s) { return p; }
};
template <typename T, class U = geo::Dep>
T ident(T v, geo::Point q, U u) { return v; }
geo::Point Shape::far(Other o) { return geo::Point(); }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["shape.cpp:Shape", "inherits", "types.hpp:Base"],
            ["shape.cpp:Shape", "inherits", ":Missing"],
            ["shape.cpp:Shape", "references", "types.hpp:Dep"],
            ["shape.cpp:Shape", "references", ":vector"],
            ["shape.cpp:Shape", "references", "types.hpp:Point"],
            ["shape.cpp:Shape", "references", ":Unknown"],
            ["shape.cpp:Shape.center()", "references", "types.hpp:Point"],
            ["shape.cpp:Shape.center()", "references", ":string"],
            ["shape.cpp:ident()", "references", "types.hpp:Point"],
            ["shape.cpp:Shape.far()", "references", ":Other"],
            ["shape.cpp:Shape.far()", "references", "types.hpp:Point"],
        ],
        &[],
    );
    check_no_stubs(&nodes, &["T", "U", "Hidden", "int"]);
}

// Upstream 0.9.88 on fixtures ts1, tsdeco, tsdeco2, tsshape: a class or
// interface member references the types the file imports by name, never a
// stub; a decorator binds or gets a stub. Not copied: a type position bound
// to a function, an aliased decorator left on a stub.
#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_ts_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "models.ts",
            "export interface Point {}\nexport class Box {}\nexport function traced(t: any) {}\nexport function helper() {}\n",
        ),
        (
            "svc.ts",
            r#"import { Point, Box, traced as tr, helper } from './models';
import { Remote } from 'pkg';
class Local {}
interface Shape { origin: Point; area(p: Box): Local; }
@tr
@Injectable()
export class Svc<T extends Point> {
  private box: Box;
  @Input() tags: Array<Point | Local>;
  bad: helper;
  constructor(private r: Remote, @Inject('x') p: Map<string, Box>) {}
  @ng.HostListener('x')
  run(b: Box, u: Unknown): Promise<Point> { return null as any; }
}
function top(a: Point): Box { return null as any; }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["svc.ts:Shape", "references", "models.ts:Point"],
            ["svc.ts:Svc", "references", "models.ts:Box"],
            ["svc.ts:Svc", "references", "models.ts:Point"],
            ["svc.ts:Svc", "references", "models.ts:traced()"],
            ["svc.ts:Svc", "references", ":Injectable"],
            ["svc.ts:Svc", "references", ":Input"],
            ["svc.ts:Svc.constructor()", "references", "models.ts:Box"],
            ["svc.ts:Svc.constructor()", "references", ":Inject"],
            ["svc.ts:Svc.run()", "references", "models.ts:Box"],
            ["svc.ts:Svc.run()", "references", "models.ts:Point"],
            ["svc.ts:Svc.run()", "references", ":HostListener"],
        ],
        &[
            ["svc.ts:Svc", "references", "svc.ts:Local"], // a same-file type gives nothing upstream
            ["svc.ts:Svc", "references", "models.ts:helper()"], // a function is no type
            ["svc.ts:top()", "references", "models.ts:Point"], // a top-level function gives nothing upstream
        ],
    );
    check_no_stubs(
        &nodes,
        &[
            "Unknown", "Remote", "Map", "Promise", "Array", "T", "helper",
        ],
    );
}

// Upstream 0.9.88 on fixtures java1 to java5: field, parameter, return,
// record component and annotation-element types, generic arguments and
// annotations give references; an unresolved base gets a stub. Not copied:
// a stub for a type parameter, a type position bound to an enum constant.
#[test]
#[ignore = "graphify-rs-ihc.27"] // also needs resolve (ihc.18)
fn test_java_references() {
    let (nodes, edges) = resolve_src(&[
        ("a/Base.java", "package a;\npublic class Base {}\n"),
        ("a/Dep.java", "package a;\npublic class Dep {}\n"),
        ("a/Anno.java", "package a;\npublic @interface Anno {}\n"),
        ("a/St.java", "package a;\npublic enum St { OK, NO }\n"),
        (
            "a/Svc.java",
            r#"package a;
import java.util.List;
@Anno @Deprecated @org.x.Foo(Cls.class)
public class Svc<T extends Num> extends Base implements Gone, Iface<Dep> {
  Dep dep;
  List<Later[]> arr;
  java.util.Map<String, Dep> mp;
  OK ok;
  T tt;
  int prim;
  Svc(Hidden h, Ghost9... va) {}
  @Override Base get(Ghost g, T t, List<Dep> l) { return null; }
  <U> Svc<Dep> gen(U u, Svc s) { return null; }
}
class Later {}
record Rec(Base a, Ghost3 g) {}
enum En { A { Ghost15 f; }, B; Dep f; }
@interface Anno2 { Dep q(); Class<?> v(); }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a/Svc.java:Svc", "inherits", "a/Base.java:Base"],
            ["a/Svc.java:Svc", "implements", ":Gone"],
            ["a/Svc.java:Svc", "implements", ":Iface"],
            ["a/Svc.java:Svc", "references", "a/Dep.java:Dep"],
            ["a/Svc.java:Svc", "references", "a/Anno.java:Anno"],
            ["a/Svc.java:Svc", "references", ":Deprecated"],
            ["a/Svc.java:Svc", "references", ":org.x.Foo"],
            ["a/Svc.java:Svc", "references", ":Cls"],
            ["a/Svc.java:Svc", "references", "a/Svc.java:Later"],
            ["a/Svc.java:Svc", "references", ":OK"],
            ["a/Svc.java:Svc.Svc()", "references", ":Hidden"],
            ["a/Svc.java:Svc.get()", "references", "a/Base.java:Base"],
            ["a/Svc.java:Svc.get()", "references", ":Override"],
            ["a/Svc.java:Svc.get()", "references", ":Ghost"],
            ["a/Svc.java:Svc.gen()", "references", "a/Svc.java:Svc"],
            ["a/Svc.java:Rec", "references", ":Ghost3"],
            ["a/Svc.java:En", "references", "a/Dep.java:Dep"],
            ["a/Svc.java:A", "references", ":Ghost15"],
            ["a/Svc.java:Anno2", "references", "a/Dep.java:Dep"],
        ],
        // type bound to an enum constant
        &[["a/Svc.java:Svc", "references", "a/St.java:OK"]],
    );
    // a varargs parameter gave a stub
    check_no_stubs(
        &nodes,
        &[
            "T", "U", "Num", "String", "List", "Map", "Class", "int", "Ghost9",
        ],
    );
}

// A type nested in the file must not bind to a top-level namesake elsewhere.
#[test]
#[ignore = "graphify-rs-ihc.27"] // also needs resolve (ihc.18)
fn test_java_nested_type_no_bind() {
    let (_, edges) = resolve_src(&[
        ("b/Config.java", "package b;\npublic class Config {}\n"),
        (
            "a/Svc.java",
            "package a;\npublic class Svc {\n  static class Config {}\n  Config c;\n  Config get() { return null; }\n}\n",
        ),
    ]);
    for e in &edges {
        assert!(
            !(e[1] == "references" && e[2] == "b/Config.java:Config"),
            "bound to unrelated class: {e:?}"
        );
    }
}

// Upstream 0.9.88 on fixtures rsn1, rsn2: signatures, fields and variant
// payloads reference their types, a supertrait or trait impl gets a stub.
// Not copied: a stub for a type parameter, an associated type or its name,
// a type position bound to a function, a trait's arguments as its base.
#[test]
#[ignore = "graphify-rs-ihc.32"] // also needs resolve (ihc.18)
fn test_rust_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "b.rs",
            "pub struct Uniq;\npub struct Dup;\npub fn helper() {}\n",
        ),
        (
            "a.rs",
            r#"use serde::Ext;
pub struct S<T> { a: Early, b: Vec<Box<Uniq>>, c: Option<Ext>, d: T, e: crate::b::Dup, f: helper, g: i32, h: String }
pub struct Tup(pub Early, *const Raw);
pub enum En { V(Late), W { x: Uniq } }
pub trait Tr: Sup + Unknown + Debug + Into<Late> { fn m(&self, x: Early) -> Late; }
pub trait Sup {}
pub struct Early;
pub struct Late;
pub struct Path;
impl Tr for S<u8> {}
impl Ext2 for Early {}
impl From<Uniq> for Late {}
impl Tr for Foreign {}
pub fn f<U: Bd>(a: Early, b: &mut Late, c: impl Imp, u: U, p: Path, s: Self::Item) -> Result<Uniq, Err1> {}
impl<T> S<T> { fn g(&self, t: T, x: Self::Out) -> S<T> {} }
pub fn d(f: &dyn Fn(Early) -> Late, g: Box<dyn Tr<Item = Late>>, c: fn(M) -> N) {}
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a.rs:S", "references", "a.rs:Early"],
            ["a.rs:S", "references", "b.rs:Uniq"],
            ["a.rs:S", "references", ":Ext"],
            ["a.rs:S", "references", "b.rs:Dup"],
            ["a.rs:Tup", "references", "a.rs:Early"],
            ["a.rs:En", "references", "a.rs:Late"],
            ["a.rs:En", "references", "b.rs:Uniq"],
            ["a.rs:Tr", "inherits", "a.rs:Sup"],
            ["a.rs:Tr", "inherits", ":Unknown"],
            ["a.rs:Tr", "references", "a.rs:Late"],
            ["a.rs:S", "implements", "a.rs:Tr"],
            ["a.rs:Early", "implements", ":Ext2"],
            ["a.rs:Late", "references", "b.rs:Uniq"],
            ["a.rs:f()", "references", "a.rs:Early"],
            ["a.rs:f()", "references", "a.rs:Late"],
            ["a.rs:f()", "references", ":Imp"],
            ["a.rs:f()", "references", "a.rs:Path"],
            ["a.rs:f()", "references", "b.rs:Uniq"],
            ["a.rs:f()", "references", ":Err1"],
            ["a.rs:S.g()", "references", "a.rs:S"],
            ["a.rs:d()", "references", "a.rs:Early"],
            ["a.rs:d()", "references", "a.rs:Tr"],
            ["a.rs:d()", "references", ":M"],
            ["a.rs:d()", "references", ":N"],
        ],
        &[
            ["a.rs:S", "references", "b.rs:helper()"],
            ["a.rs:Tup", "references", ":Raw"],
            ["a.rs:Late", "implements", "b.rs:Uniq"],
            ["a.rs:Tr", "inherits", "a.rs:Late"],
        ],
    );
    check_no_stubs(
        &nodes,
        &[
            "T", "U", "Item", "Out", "Debug", "Result", "Fn", "Box", "String", "Raw", "Foreign",
        ],
    );
}

// Not copied from upstream: a stub for a type parameter, a type position
// bound to a function, an enum constant or a nested type, the head of a
// qualified Kotlin type.
#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_c_sharp_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "b.cs",
            "public class Uniq {}\npublic class MyAttr {}\npublic class Late {}\npublic static class Util { public static void Helper() {} }\npublic enum Color { Red }\npublic class Inner {}\n",
        ),
        (
            "a.cs",
            r#"public interface IFoo { Early Get(Late l); }
public class Early {}
public class Base<T> {}
public class Foo<T> : Base<T1>, IFoo, IMiss, MissingBase {
    public Early f1;
    public List<Uniq> f2;
    public int f3;
    public T f5;
    public Ext.Lib.Qual f6;
    public Nullable<Dup> f8;
    public (Early a, Pe b) f10;
    public Helper f11;
    public Color f12;
    public Prop1 P { get; set; }
    public Inner nn;
    [MyAttr] public Mk1 Make<U>(Mk2 a, U u, int i, ref Mk3 r, Mk4[] arr, List<T> lt) { return null; }
    [Obsolete] [Ext.Attr] public void Run(Func<Mk5, Mk6> fn) {}
    public Foo(Ctor1 c) {}
    public event Evt1 E1;
    public void Loc() { Local1 x = null; }
    public class Inner { public Uniq iu; }
}
public record Rec(Rp1 a, List<Rp2> b);
public class Dc(Pc1 a) : Base<Pc2> { }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a.cs:Foo", "inherits", ":MissingBase"],
            ["a.cs:Foo", "implements", ":IMiss"],
            ["a.cs:Foo", "implements", "a.cs:IFoo"],
            ["a.cs:Foo", "references", ":T1"],
            ["a.cs:Foo", "references", "a.cs:Early"],
            ["a.cs:Foo", "references", "b.cs:Uniq"],
            ["a.cs:Foo", "references", ":List"],
            ["a.cs:Foo", "references", ":Qual"],
            ["a.cs:Foo", "references", ":Nullable"],
            ["a.cs:Foo", "references", ":Dup"],
            ["a.cs:Foo", "references", ":Pe"],
            ["a.cs:Foo", "references", ":Helper"],
            ["a.cs:Foo", "references", "b.cs:Color"],
            ["a.cs:Foo", "references", ":Prop1"],
            ["a.cs:Foo.Make()", "references", ":Mk1"],
            ["a.cs:Foo.Make()", "references", ":Mk2"],
            ["a.cs:Foo.Make()", "references", ":Mk3"],
            ["a.cs:Foo.Make()", "references", ":Mk4"],
            ["a.cs:Foo.Make()", "references", ":List"],
            ["a.cs:Foo.Make()", "references", "b.cs:MyAttr"],
            ["a.cs:Foo.Run()", "references", ":Obsolete"],
            ["a.cs:Foo.Run()", "references", ":Attr"],
            ["a.cs:Foo.Run()", "references", ":Func"],
            ["a.cs:Foo.Run()", "references", ":Mk5"],
            ["a.cs:Foo.Run()", "references", ":Mk6"],
            ["a.cs:IFoo.Get()", "references", "a.cs:Early"],
            ["a.cs:IFoo.Get()", "references", "b.cs:Late"],
            ["a.cs:Rec", "references", ":Rp1"],
            ["a.cs:Rec", "references", ":Rp2"],
            ["a.cs:Dc", "references", ":Pc1"],
            ["a.cs:Dc", "references", ":Pc2"],
        ],
        &[
            ["a.cs:Foo", "references", "b.cs:Inner"],
            ["a.cs:Foo", "references", ":Ctor1"],
            ["a.cs:Foo", "references", ":Evt1"],
            ["a.cs:Foo", "references", ":Local1"],
            ["a.cs:Foo.Make()", "references", ":U"],
            ["a.cs:Foo.Make()", "references", ":T"],
            ["a.cs:Foo", "references", "b.cs:Util.Helper()"],
        ],
    );
    check_no_stubs(&nodes, &["T", "U", "int", "Ctor1", "Evt1", "Local1"]);
}

#[test]
#[ignore = "graphify-rs-ihc.37"] // also needs resolve (ihc.18)
fn test_kotlin_references() {
    let (nodes, edges) = resolve_src(&[
        (
            "b.kt",
            "class Uniq\nannotation class Anno\nfun helper() {}\nenum class Color { RED }\nclass Nest1\n",
        ),
        (
            "a.kt",
            r#"class Early
class Foo<T>(val pa: Early, var pb: Pb1?, pc: Pc1, @Anno val pd: List<Pd1>) : MissingBase(), IMiss, Early, Other<Gx> by Dl {
    val f1: Early = Early()
    val f2: Uniq? = null
    val f3: Map<String, Dup>
    val f5: T
    val f6: com.other.Qual
    val f7: (Fn1) -> Fn2
    val f8: helper
    val f9: Color
    val f11: Array<Arr1>
    val f12 = Inf1()
    @Anno val f13: Ann1 = x
    val f20: Map.Entry<K1x, V1>
    val nn: Nest1
    fun <U> make(a: Mk1, b: U, c: Int, d: List<Mk2>, e: Mk3?, f: (Mk4) -> Mk5, g: T): Mk6 { return x }
    @Anno @Ext.Q fun run(): Unit {}
    @Target(Cls1::class) fun lit() {}
    fun Ex1.recv(vararg v: Va1) {}
    class Nest1
    inner class Inn {
        val ii: Late1
    }
    companion object {
        val cc: Cmp1
    }
}
interface Iface : Early, Other2 {
    val p: Ip1
    fun m(x: Ip2): Ip3
}
object O : Early() {
    val o: Ob1 = x
}
class Ex : Exception()
enum class E1(val v: En1) : Early { A(x) { fun a(q: En2) {} }, B }
fun top(a: Tp1, b: Early): Tp2 {}
fun <V> gen(v: V): V = v
val topProp: Tp3 = x
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a.kt:Foo", "inherits", ":MissingBase"],
            ["a.kt:Foo", "implements", ":IMiss"],
            ["a.kt:Foo", "implements", "a.kt:Early"],
            ["a.kt:Foo", "implements", ":Other"],
            ["a.kt:Foo", "references", ":Gx"],
            ["a.kt:Foo", "references", "a.kt:Early"],
            ["a.kt:Foo", "references", ":Pb1"],
            ["a.kt:Foo", "references", ":Pd1"],
            ["a.kt:Foo", "references", "b.kt:Anno"],
            ["a.kt:Foo", "references", "b.kt:Uniq"],
            ["a.kt:Foo", "references", ":Dup"],
            ["a.kt:Foo", "references", ":Qual"],
            ["a.kt:Foo", "references", ":Entry"],
            ["a.kt:Foo", "references", ":K1x"],
            ["a.kt:Foo", "references", ":Arr1"],
            ["a.kt:Foo", "references", "b.kt:Color"],
            ["a.kt:Foo", "references", ":Ann1"],
            ["a.kt:Foo", "references", ":Cmp1"],
            ["a.kt:Foo.make()", "references", ":Mk1"],
            ["a.kt:Foo.make()", "references", ":Mk2"],
            ["a.kt:Foo.make()", "references", ":Mk3"],
            ["a.kt:Foo.make()", "references", ":Mk6"],
            ["a.kt:Foo.run()", "references", "b.kt:Anno"],
            ["a.kt:Foo.run()", "references", ":Q"],
            ["a.kt:Foo.lit()", "references", ":Target"],
            ["a.kt:Foo.recv()", "references", ":Va1"],
            ["a.kt:Iface", "implements", "a.kt:Early"],
            ["a.kt:Iface", "implements", ":Other2"],
            ["a.kt:Iface", "references", ":Ip1"],
            ["a.kt:Iface.m()", "references", ":Ip2"],
            ["a.kt:Iface.m()", "references", ":Ip3"],
            ["a.kt:O", "inherits", "a.kt:Early"],
            ["a.kt:O", "references", ":Ob1"],
            ["a.kt:Ex", "inherits", ":Exception"],
            ["a.kt:E1", "references", ":En1"],
            ["a.kt:E1", "implements", "a.kt:Early"],
            ["a.kt:A.a()", "references", ":En2"],
            ["a.kt:top()", "references", ":Tp1"],
            ["a.kt:top()", "references", "a.kt:Early"],
            ["a.kt:top()", "references", ":Tp2"],
        ],
        &[
            ["a.kt:Foo", "references", "b.kt:helper()"],
            ["a.kt:Foo", "references", "b.kt:Nest1"],
            ["a.kt:Foo", "references", ":Pc1"],
            ["a.kt:Foo", "references", ":Fn1"],
            ["a.kt:Foo", "references", ":Inf1"],
            ["a.kt:Foo", "references", ":Late1"],
            ["a.kt:Foo.make()", "references", ":Mk4"],
            ["a.kt:Foo.recv()", "references", ":Ex1"],
            ["a.kt:top()", "references", ":Tp3"],
        ],
    );
    check_no_stubs(
        &nodes,
        &[
            "T", "U", "V", "com", "Map", "List", "Int", "Unit", "Tp3", "Inf1",
        ],
    );
}

// Not copied from upstream: stubs for type parameters, nested types
// binding to a namesake elsewhere.
#[test]
#[ignore = "graphify-rs-ihc.36"] // also needs resolve (ihc.18)
fn test_scala_references() {
    let (nodes, edges) = resolve_src(&[
        ("b.scala", "class Late\nclass Uniq\nclass Nest1\n"),
        (
            "a.scala",
            r#"class Early
class Foo[T, U: Ordering](val a: Early, b: Pb1, c: Map[String, Ext.Q], d: Option[T]) extends MissingBase[G1] with Early with IMiss {
  class Nest1
  val v: Late = null
  var w: Lst[Early] = null
  val tup: (Tup1, Tup2) = null
  val fn: Fn1 => Fn2 = null
  type Al = Alias1
  val h: T = null
  val q: a.b.Qual = null
  val n: Nest1 = null
  def m[V](x: Late, y: V, z: => ByName, w: Seq[Seq[Deep]])(implicit ev: Impl): Ret1 = null
  def n2[A: Ctx](x: A): A = x
  def p(u: Late with Other): Uniq = null
  @Anno def q2(): Unit = ()
  def r(): Unit = { val local: Loc1 = null }
}
trait Tr extends Early with Late { def g(x: Early): Rt1 }
class Der[A] derives Show
case class CC(x: Early, y: Mis3)
enum En { case A(x: EnumP) }
class Dd extends p.Early
def top(x: Early): Tp2 = null
sealed trait Opt[+A] { def get: A }
class Fn[-In, +Out, P] { def run(i: In, p: P): Out }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a.scala:Foo", "inherits", ":MissingBase"],
            ["a.scala:Foo", "mixes_in", "a.scala:Early"],
            ["a.scala:Foo", "mixes_in", ":IMiss"],
            ["a.scala:Foo", "references", ":Ordering"],
            ["a.scala:Foo", "references", "a.scala:Early"],
            ["a.scala:Foo", "references", ":Pb1"],
            ["a.scala:Foo", "references", ":Map"],
            ["a.scala:Foo", "references", ":String"],
            ["a.scala:Foo", "references", ":Option"],
            ["a.scala:Foo", "references", "b.scala:Late"],
            ["a.scala:Foo", "references", ":Lst"],
            ["a.scala:Foo", "references", ":Tup1"],
            ["a.scala:Foo", "references", ":Fn2"],
            ["a.scala:Foo", "references", ":Alias1"],
            ["a.scala:Foo.m()", "references", "b.scala:Late"],
            ["a.scala:Foo.m()", "references", ":Seq"],
            ["a.scala:Foo.m()", "references", ":Deep"],
            ["a.scala:Foo.m()", "references", ":Ret1"],
            ["a.scala:Foo.n2()", "references", ":Ctx"],
            ["a.scala:Foo.p()", "references", ":Other"],
            ["a.scala:Foo.p()", "references", "b.scala:Uniq"],
            ["a.scala:Tr", "inherits", "a.scala:Early"],
            ["a.scala:Tr", "mixes_in", "b.scala:Late"],
            ["a.scala:Tr.g()", "references", ":Rt1"],
            ["a.scala:Der", "references", ":Show"],
            ["a.scala:CC", "references", ":Mis3"],
            ["a.scala:Dd", "inherits", "a.scala:Early"],
            ["a.scala:top()", "references", ":Tp2"],
        ],
        &[
            ["a.scala:Foo", "references", "b.scala:Nest1"],
            ["a.scala:Foo", "references", ":G1"],
            ["a.scala:Foo", "references", ":Q"],
            ["a.scala:Foo", "references", ":Qual"],
            ["a.scala:Foo", "references", ":Fn1"],
            ["a.scala:Foo.m()", "references", ":ByName"],
            ["a.scala:Foo.m()", "references", ":Impl"],
            ["a.scala:Foo.q2()", "references", ":Anno"],
            ["a.scala:Foo.r()", "references", ":Loc1"],
        ],
    );
    check_no_stubs(
        &nodes,
        &["T", "U", "V", "A", "EnumP", "Nest1", "In", "Out", "P"],
    );
}

// Not copied from upstream: stubs for self, static and parent.
#[test]
#[ignore = "graphify-rs-ihc.34"] // also needs resolve (ihc.18)
fn test_php_references() {
    let (nodes, edges) = resolve_src(&[
        ("b.php", "<?php\nclass Late {}\nclass Uniq {}\n"),
        (
            "a.php",
            r#"<?php
namespace App;
use Ext\Lib\Ext;
interface IEarly {}
class Early {}
#[Attr1]
class User extends MissingBase implements IMiss, IEarly {
    use TraitMiss, Early;
    public Early $e;
    public ?Late $l;
    public int|Uniq $u;
    public \Other\Qual $q;
    public Ext $x;
    public self $s;
    public array $arr;
    public function __construct(private Ctor2 $p, int $n, ?Param3 $z = null) {}
    #[Attr2]
    public function meth(Param1 $a, int $b, Early&Inter $c): ?Ret3 { return null; }
    public function m3(): static { return $this; }
    public function m4(parent $p): void {}
    public function m6(): Ext\Deep {}
}
interface I2 extends IMiss1, IEarly { public function im(Late $x): Early; }
function top(Late $a, Top1 $b): TopRet {}
trait TrA { public Late $tl; }
enum En: string implements IMiss2 { case A = 'a'; }
"#,
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a.php:User", "inherits", ":MissingBase"],
            ["a.php:User", "implements", ":IMiss"],
            ["a.php:User", "implements", "a.php:IEarly"],
            ["a.php:User", "mixes_in", ":TraitMiss"],
            ["a.php:User", "mixes_in", "a.php:Early"],
            ["a.php:User", "references", "a.php:Early"],
            ["a.php:User", "references", "b.php:Late"],
            ["a.php:User", "references", "b.php:Uniq"],
            ["a.php:User", "references", ":Qual"],
            ["a.php:User", "references", ":Ext"],
            ["a.php:User", "references", ":Ctor2"],
            ["a.php:User.__construct()", "references", ":Ctor2"],
            ["a.php:User.__construct()", "references", ":Param3"],
            ["a.php:User.meth()", "references", ":Param1"],
            ["a.php:User.meth()", "references", ":Inter"],
            ["a.php:User.meth()", "references", ":Ret3"],
            ["a.php:User.m6()", "references", ":Deep"],
            ["a.php:I2", "inherits", ":IMiss1"],
            ["a.php:I2.im()", "references", "b.php:Late"],
            ["a.php:I2.im()", "references", "a.php:Early"],
            ["a.php:top()", "references", ":Top1"],
            ["a.php:top()", "references", ":TopRet"],
            ["a.php:TrA", "references", "b.php:Late"],
            ["a.php:En", "implements", ":IMiss2"],
        ],
        &[
            ["a.php:User", "references", ":Attr1"],
            ["a.php:User.meth()", "references", ":Attr2"],
        ],
    );
    check_no_stubs(
        &nodes,
        &[
            "self", "static", "parent", "array", "int", "void", "Attr1", "Attr2",
        ],
    );
}

// Upstream emits no type references for Ruby, only the superclass stub.
#[test]
#[ignore = "graphify-rs-ihc.33"] // also needs resolve (ihc.18)
fn test_ruby_references() {
    let (nodes, edges) = resolve_src(&[
        ("b.rb", "class Late; end\n"),
        (
            "a.rb",
            "class Early; end\nclass Same < Early; end\nclass Miss < Missing::Deep; end\nclass Far < Late; end\nclass Inc < Early\n  include IMiss\n  def m(a = Pm1.new); end\nend\n",
        ),
    ]);
    check_edges(
        &edges,
        &[
            ["a.rb:Same", "inherits", "a.rb:Early"],
            ["a.rb:Miss", "inherits", ":Deep"],
            ["a.rb:Far", "inherits", "b.rb:Late"],
        ],
        &[],
    );
    check_no_stubs(&nodes, &["IMiss", "Pm1", "Missing"]);
}

// Upstream fixtures java5, scphv2-qual: a qualified name binds only a type
// whose file or directory has the name of its last qualifier. Not copied:
// upstream drops the qualifier and binds any corpus class of that name.
#[test]
#[ignore = "graphify-rs-ihc.34"] // also needs resolve (ihc.18)
fn test_qualified_name_needs_its_qualifier() {
    let (_, edges) = resolve_src(&[
        ("p2/Entry.java", "package p2;\npublic class Entry {}\n"),
        ("p2/Bar.java", "package p2;\npublic class Bar {}\n"),
        (
            "p1/A.java",
            "package p1;\nclass A {\n  Map.Entry e;\n  org.x.Bar b;\n  p2.Bar ok;\n}\n",
        ),
        (
            "lib/Base.php",
            "<?php\nnamespace App\\Lib;\nclass Base {}\n",
        ),
        (
            "app/K.php",
            "<?php\nclass K extends \\Ext\\Other\\Base {}\nclass L extends \\App\\Lib\\Base {}\n",
        ),
    ]);
    for (w, want) in [
        (["p1/A.java:A", "references", "p2/Entry.java:Entry"], false),
        (["p1/A.java:A", "references", ":Entry"], true),
        (["p1/A.java:A", "references", "p2/Bar.java:Bar"], true), // p2.Bar
        (["p1/A.java:A", "references", ":Bar"], true),            // org.x.Bar
        (["app/K.php:K", "inherits", "lib/Base.php:Base"], false),
        (["app/K.php:L", "inherits", "lib/Base.php:Base"], true),
    ] {
        let got = edges.contains(&s3(&w));
        assert_eq!(got, want, "edge {w:?}: got {got}, want {want}");
    }
}
