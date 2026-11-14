//! graphify-go `internal/extract/gaps_test.go`.

use std::collections::{HashMap, HashSet};

use crate::extract::{Result, file, resolve};
use crate::testutil::TempDir;

fn resolve_all(results: &[Result], files: &[&str]) -> crate::model::Extraction {
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(results, &files, workers)
}

/// Go `gapEdges`: extracts one file and returns "src rel tgt" labels of the edges plus the node
/// labels.
fn gap_edges(rel: &str, code: &str) -> (HashSet<String>, HashSet<String>) {
    crate::extract::kinds::register_kinds();
    let root = TempDir::new();
    root.write(rel, code.as_bytes());
    let r = file(root.path(), rel).expect("extract");
    let ext = resolve_all(&[r], &[rel]);
    let mut id2label: HashMap<&str, &str> = HashMap::new();
    let mut nodes = HashSet::new();
    for n in &ext.nodes {
        id2label.insert(n.id.as_str(), n.label.as_str());
        nodes.insert(n.label.clone());
    }
    let label = |id: &str| id2label.get(id).copied().unwrap_or("");
    let mut edges = HashSet::new();
    for e in &ext.edges {
        edges.insert(format!(
            "{} {} {}",
            label(&e.source),
            e.relation.as_str(),
            label(&e.target)
        ));
    }
    (edges, nodes)
}

#[test]
#[ignore = "graphify-rs-ihc.28"] // also needs resolve (ihc.18)
fn test_gap_c_sharp_null_conditional_call() {
    let (edges, _) = gap_edges(
        "a.cs",
        "class S { public void Save() {} }\nclass C { void Run(S st) { st?.Save(); } }\n",
    );
    assert!(
        edges.contains("C.Run() calls S.Save()"),
        "missing calls edge: {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.33"] // also needs resolve (ihc.18)
fn test_gap_ruby_bare_self_send() {
    let (edges, _) = gap_edges(
        "a.rb",
        "class W\n  def run(helper)\n    helper\n    local = 1\n    local\n    do_thing\n  end\n\n  def do_thing\n  end\n\n  def helper\n  end\nend\n",
    );
    assert!(
        edges.contains("W.run() calls W.do_thing()"),
        "missing bare self-send edge: {edges:?}"
    );
    assert!(
        !edges.contains("W.run() calls W.helper()"),
        "parameter treated as call: {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_gap_js_member_assigned_function() {
    let (edges, nodes) = gap_edges(
        "a.js",
        "function helper2() {}\nexports.format = function () { helper2() }\n",
    );
    assert!(
        nodes.contains("format()") && edges.contains("format() calls helper2()"),
        "nodes={nodes:?} edges={edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_gap_ts_abstract_method() {
    let (edges, nodes) = gap_edges(
        "a.ts",
        "abstract class Shape {\n  abstract area(): number;\n  describe() { return this.area(); }\n}\n",
    );
    assert!(
        nodes.contains("Shape.area()") && edges.contains("Shape.describe() calls Shape.area()"),
        "nodes={nodes:?} edges={edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.36"] // also needs resolve (ihc.18)
fn test_gap_scala_deferred_def() {
    let (edges, nodes) = gap_edges(
        "a.scala",
        "trait Shape {\n  def area: Double\n  def describe(): String = this.area().toString\n}\n",
    );
    assert!(
        nodes.contains("Shape.area()") && edges.contains("Shape.describe() calls Shape.area()"),
        "nodes={nodes:?} edges={edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.36"] // also needs resolve (ihc.18)
fn test_gap_abstract_with_same_file_override() {
    let (edges, _) = gap_edges(
        "a.ts",
        "abstract class Shape {\n  abstract area(): number;\n  describe() { return this.area(); }\n}\nclass Circle extends Shape {\n  area() { return 1; }\n}\n",
    );
    assert!(
        edges.contains("Shape.describe() calls Shape.area()"),
        "ts: {edges:?}"
    );
    let (edges, _) = gap_edges(
        "a.scala",
        "trait Shape {\n  def area: Double\n  def describe(): String = this.area().toString\n}\nclass Circle extends Shape {\n  def area: Double = 1.0\n}\n",
    );
    assert!(
        edges.contains("Shape.describe() calls Shape.area()"),
        "scala: {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.33"] // also needs resolve (ihc.18)
fn test_gap_ruby_locals_not_self_sends() {
    for (name, body) in [
        ("opassign", "value ||= 3\n    value"),
        ("multi", "helper, value = 1, 2\n    value"),
        ("block", "[1].each do |value|\n      value\n    end"),
        ("lambda", "f = ->(value) { value }\n    f"),
        (
            "rescue",
            "begin\n      1\n    rescue => value\n      value\n    end",
        ),
        ("for", "for value in [1]\n      value\n    end"),
    ] {
        let (edges, _) = gap_edges(
            "a.rb",
            &format!("class C\n  def value\n    1\n  end\n  def run\n    {body}\n  end\nend\n"),
        );
        assert!(
            !edges.contains("C.run() calls C.value()") && !edges.contains(".run() calls .value()"),
            "{name}: fabricated edge: {edges:?}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.36"] // also needs resolve (ihc.18)
fn test_gap_abstract_does_not_block_single_impl() {
    // An untyped receiver binds nothing, so the calls go through a
    // qualifier (the file stem) and a bare name.
    for (name, file, src, want) in [
        (
            "ts",
            "a.ts",
            "abstract class Sh { abstract size(): number }\nclass Ci extends Sh { size() { return 1 } }\nfunction useIt() { return a.size() }\n",
            "useIt() calls Ci.size()",
        ),
        (
            "scala",
            "a.scala",
            "trait Sh { def size: Int }\nclass Ci extends Sh { def size: Int = 1 }\nobject O { def useIt() = size() }\n",
            "O.useIt() calls Ci.size()",
        ),
    ] {
        let (edges, _) = gap_edges(file, src);
        assert!(edges.contains(want), "{name} same-file: {edges:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_gap_abstract_cross_file_single_impl() {
    crate::extract::kinds::register_kinds();
    let root = TempDir::new();
    let srcs = [
        (
            "shape.ts",
            "export abstract class Sh { abstract size(): number }\nexport class Ci extends Sh { size() { return 1 } }\n",
        ),
        // The namespace receiver qualifies both defs; an untyped one binds nothing.
        (
            "use.ts",
            "import * as shape from './shape'\nexport function useIt() { return shape.size() }\n",
        ),
    ];
    let mut rs = Vec::new();
    let mut files = Vec::new();
    for (f, s) in srcs {
        root.write(f, s.as_bytes());
        rs.push(file(root.path(), f).expect("extract"));
        files.push(f);
    }
    let ext = resolve_all(&rs, &files);
    let lbl: HashMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let of = |id: &str| lbl.get(id).copied().unwrap_or("");
    assert!(
        ext.edges.iter().any(|e| e.relation.as_str() == "calls"
            && of(&e.source) == "useIt()"
            && of(&e.target) == "Ci.size()"),
        "missing useIt() calls Ci.size(): {:?}",
        ext.edges
    );
}
