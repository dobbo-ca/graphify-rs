//! graphify-go `internal/extract/preproc_test.go`. Go's `resolveSrc` (`nodeid_test.go:6`) has no
//! shared Rust helper yet, so `resolve_src` below is its port (without the `go.mod` case, which
//! these tests do not use).

use std::collections::BTreeSet;

use crate::extract::{file_from_bytes, resolve};

type Nodes = BTreeSet<(String, String)>;
type Edges = BTreeSet<(String, String, String)>;

/// Go `resolveSrc`: nodes as (source file, label), edges as ("file:label", relation, "file:label").
fn resolve_src(srcs: &[(&str, &str)]) -> (Nodes, Edges) {
    crate::extract::kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    let mut nodes = Nodes::new();
    let mut at = std::collections::BTreeMap::new();
    for n in &ext.nodes {
        nodes.insert((n.source_file.clone(), n.label.clone()));
        at.insert(n.id.clone(), (n.source_file.clone(), n.label.clone()));
    }
    let key = |id: &str| {
        let (f, l) = at.get(id).cloned().unwrap_or_default();
        format!("{f}:{l}")
    };
    let edges = ext
        .edges
        .iter()
        .map(|e| {
            (
                key(&e.source),
                e.relation.as_str().to_string(),
                key(&e.target),
            )
        })
        .collect();
    (nodes, edges)
}

fn has_node(nodes: &Nodes, file: &str, label: &str) -> bool {
    nodes.contains(&(file.to_string(), label.to_string()))
}

fn has_edge(edges: &Edges, e: [&str; 3]) -> bool {
    edges.contains(&(e[0].to_string(), e[1].to_string(), e[2].to_string()))
}

// C items behind a preprocessor conditional or an extern "C" block are extracted.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_c_items_inside_conditionals() {
    let guard = "#ifndef GUARD_H\n#define GUARD_H\n#include <stdio.h>\n#include \"other.h\"\n"
        .to_string()
        + "static inline int inl(int a) { return a; }\n"
        + "#ifdef __cplusplus\nextern \"C\" {\n#endif\nint in_extern(void) { return inl(1); }\n#ifdef __cplusplus\n}\n#endif\n#endif\n";
    let a = "int base(void) { return 0; }\n".to_string()
        + "#ifdef A\nint in_ifdef(void) { return base(); }\n#else\n#include \"guard.h\"\nint in_else(void) { return base(); }\n#endif\n"
        + "#if X\nint in_if(void) { return 1; }\n#elif Y\nint in_elif(void) { return 2; }\n#else\n#if Z\nint nested(void) { return in_if(); }\n#endif\n#endif\n";
    let (nodes, edges) = resolve_src(&[("guard.h", &guard), ("other.h", "int o;\n"), ("a.c", &a)]);
    for want in [
        ("guard.h", "inl()"),
        ("guard.h", "in_extern()"),
        ("a.c", "in_ifdef()"),
        ("a.c", "in_else()"),
        ("a.c", "in_if()"),
        ("a.c", "in_elif()"),
        ("a.c", "nested()"),
    ] {
        assert!(has_node(&nodes, want.0, want.1), "missing node {want:?}");
    }
    for want in [
        ["guard.h:in_extern()", "calls", "guard.h:inl()"],
        ["a.c:in_else()", "calls", "a.c:base()"],
        ["a.c:nested()", "calls", "a.c:in_if()"],
        ["guard.h:guard.h", "imports_from", "other.h:other.h"],
        ["a.c:a.c", "imports_from", "guard.h:guard.h"],
    ] {
        assert!(has_edge(&edges, want), "missing edge {want:?}\n{edges:?}");
    }
}

// C++ items behind a conditional, in an extern "C" block, and methods behind
// a conditional in a class body are extracted.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_cpp_items_inside_conditionals() {
    let hpp = "#ifndef W_HPP\n#define W_HPP\n#include \"dep.hpp\"\nnamespace ns {\n#ifdef FEATURE\n"
        .to_string()
        + "class Widget : public Base {\npublic:\n  void draw() {}\n#if EXTRA\n  void extra() { draw(); }\n#else\n  void plain() {}\n#endif\n};\n"
        + "template <typename T> T tmpl(T v) { return v; }\n#endif\n}\n"
        + "#ifdef __cplusplus\nextern \"C\" {\n#endif\nint c_api(void) { return 0; }\n#ifdef __cplusplus\n}\n#endif\n#endif\n";
    let e = "extern \"C\" {\nint plain_extern(void) { return 0; }\n}\nextern \"C\" int single_extern(void) { return plain_extern(); }\n";
    let (nodes, edges) = resolve_src(&[
        ("w.hpp", &hpp),
        ("dep.hpp", "class Base {};\n"),
        ("e.cpp", e),
    ]);
    for want in [
        ("w.hpp", "Widget"),
        ("w.hpp", "Widget.draw()"),
        ("w.hpp", "Widget.extra()"),
        ("w.hpp", "Widget.plain()"),
        ("w.hpp", "tmpl()"),
        ("w.hpp", "c_api()"),
        ("e.cpp", "plain_extern()"),
        ("e.cpp", "single_extern()"),
    ] {
        assert!(has_node(&nodes, want.0, want.1), "missing node {want:?}");
    }
    for want in [
        ["w.hpp:Widget", "contains", "w.hpp:Widget.extra()"],
        ["w.hpp:Widget.extra()", "calls", "w.hpp:Widget.draw()"],
        ["w.hpp:Widget", "inherits", "dep.hpp:Base"],
        ["w.hpp:w.hpp", "imports_from", "dep.hpp:dep.hpp"],
        ["e.cpp:single_extern()", "calls", "e.cpp:plain_extern()"],
    ] {
        assert!(has_edge(&edges, want), "missing edge {want:?}\n{edges:?}");
    }
}

// A signature split by a conditional is a parse error; the definitions under
// the ERROR node are still extracted.
#[test]
#[ignore = "graphify-rs-ihc.29"]
fn test_c_items_under_parse_error() {
    let src = "#ifdef A\nint split(int a) {\n#else\nint split(long a) {\n#endif\n  return after();\n}\nint after(void) { return 0; }\n";
    let (nodes, _) = resolve_src(&[("e.c", src)]);
    assert!(
        has_node(&nodes, "e.c", "after()"),
        "missing after(): {nodes:?}"
    );
}

// A function defined in both branches of a conditional is one call target.
#[test]
#[ignore = "graphify-rs-ihc.29"]
fn test_call_to_function_defined_in_both_branches() {
    let a = "int x1(void) { return 0; }\n#ifdef A\nint dup(void) { return x1(); }\n#else\nint dup(void) { return x1(); }\n#endif\n";
    let (_, edges) = resolve_src(&[("a.c", a), ("b.c", "int user(void) { return dup(); }\n")]);
    let want = ["b.c:user()", "calls", "a.c:dup()"];
    assert!(has_edge(&edges, want), "missing edge {want:?}\n{edges:?}");
}

// A struct without a body is a reference, and a typedef struct in a C++
// header is a definition.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_struct_forward_decl_and_typedef() {
    let (nodes, _) = resolve_src(&[
        (
            "a.c",
            "struct fwd;\nstruct var_t v;\nstruct real { int a; };\n",
        ),
        ("n.hpp", "typedef struct Named { int a; } NamedT;\n"),
    ]);
    assert!(
        !has_node(&nodes, "a.c", "fwd") && !has_node(&nodes, "a.c", "var_t"),
        "node for a struct without a body: {nodes:?}"
    );
    assert!(
        has_node(&nodes, "a.c", "real") && has_node(&nodes, "n.hpp", "Named"),
        "missing struct node: {nodes:?}"
    );
}

// An overload declared abstract and then defined keeps its concrete target.
#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_abstract_then_concrete_overload_still_binds() {
    let (_, edges) = resolve_src(&[
        (
            "a.scala",
            "trait T {\n  def run(x: Int): Int\n  def run(x: String): Int = 0\n}\n",
        ),
        ("b.scala", "trait U {\n  def run(x: Int): Int\n}\n"),
        ("c.scala", "object C {\n  def go(): Int = run(\"a\")\n}\n"),
    ]);
    let want = ["c.scala:C.go()", "calls", "a.scala:T.run()"];
    assert!(has_edge(&edges, want), "missing edge {want:?}\n{edges:?}");
}
