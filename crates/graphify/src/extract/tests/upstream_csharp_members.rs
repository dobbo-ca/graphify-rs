//! upstream graphify v0.9.107 `test_csharp_partial_classes.py`, `test_csharp_overloads.py`,
//! `test_csharp_member_nodes.py`, `test_csharp_method_owner_labels.py` and
//! `test_csharp_object_creation.py`: the cases whose assertion holds for graphify-go v0.12.0.
//! Sources are inline. Go labels a method `Type.Method()` (upstream `.Method()`), a type `Name`.
//! Go keeps every file's types apart (a node id carries the file), so `partial` halves never merge.

use std::collections::BTreeSet;

use super::csharp_tests::{has_edge, label_set, pairs, run};
use crate::model::Extraction;

fn has_call(ext: &Extraction, src: &str, tgt: &str) -> bool {
    has_edge(ext, src, "calls", tgt)
}

/// (source id, target id) of every `calls` edge.
fn call_ids(ext: &Extraction) -> BTreeSet<(&str, &str)> {
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == "calls")
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect()
}

/// Ids of every node labelled `label`, stubs included.
fn ids_of<'a>(ext: &'a Extraction, label: &str) -> BTreeSet<&'a str> {
    ext.nodes
        .iter()
        .filter(|n| n.label == label)
        .map(|n| n.id.as_str())
        .collect()
}

/// The id of the node labelled `label` declared in `file`.
fn id_in<'a>(ext: &'a Extraction, label: &str, file: &str) -> &'a str {
    ext.nodes
        .iter()
        .find(|n| n.label == label && n.source_file == file)
        .map(|n| n.id.as_str())
        .unwrap_or_else(|| panic!("no node {label:?} in {file}"))
}

fn count_label(ext: &Extraction, label: &str) -> usize {
    ext.nodes.iter().filter(|n| n.label == label).count()
}

// ---------------------------------------------------------------------------
// upstream `test_csharp_partial_classes.py` (29 tests).
// Left out: every test that needs the halves of a partial class merged onto one node, across
// files, assemblies (.csproj) or path-colliding stems (Go keeps one node per file); every test
// of a nested type (Go has no node for it); the self-call tests (Go drops a call to the caller
// itself); the `_merge_csharp_partial_class_nodes` and schema-cache tests (Python internals); and
// `test_cross_half_unqualified_call_beats_a_same_named_method_elsewhere`, where the bare call is
// ambiguous in Go (two `Alpha`) and binds nothing.
// ---------------------------------------------------------------------------

const FOO_PART_A: &str = r#"namespace App {
    public partial class Foo {
        public void Alpha() {}
    }
}
"#;

const FOO_PART_B: &str = r#"namespace App {
    public partial class Foo {
        public void Beta() { Alpha(); }
    }
}
"#;

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_cross_half_unqualified_in_class_call_resolves() {
    // Beta() in half B calls Alpha() declared in half A: the one definition of the name.
    let ext = run(&[("FooPartA.cs", FOO_PART_A), ("FooPartB.cs", FOO_PART_B)]);
    assert!(
        has_call(&ext, "Foo.Beta()", "Foo.Alpha()"),
        "cross-half unqualified in-class call must resolve"
    );
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_same_name_different_namespace_not_merged() {
    let ext = run(&[
        (
            "A.cs",
            "namespace Alpha { public partial class Foo { public void FromA() {} } }\n",
        ),
        (
            "B.cs",
            "namespace Beta { public partial class Foo { public void FromB() {} } }\n",
        ),
    ]);
    assert_eq!(
        count_label(&ext, "Foo"),
        2,
        "same-named partials in DIFFERENT namespaces are distinct types"
    );
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_non_partial_same_name_not_merged() {
    let ext = run(&[
        (
            "A.cs",
            "namespace App { public partial class Foo { public void FromA() {} } }\n",
        ),
        (
            "B.cs",
            "namespace App { public class Foo { public void FromB() {} } }\n",
        ),
    ]);
    assert_eq!(
        count_label(&ext, "Foo"),
        2,
        "a non-partial declaration never merges with a partial half"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_receiver_qualified_calls_are_not_rebound_to_the_own_type() {
    // The own-type binding is for unqualified calls only: `base.M()` and `Type.M()` keep their
    // targets. Upstream also merges the two `Engine` halves; Go keeps two `Engine` nodes, and
    // `Go()` is a method of the second one.
    let ext = run(&[
        (
            "Machine.cs",
            "namespace App { public class Machine { public virtual void Reset() {} } }\n",
        ),
        (
            "E1.cs",
            concat!(
                "namespace App { public partial class Engine : Machine {",
                " public override void Reset() {} public static void Stop() {} } }\n"
            ),
        ),
        (
            "E2.cs",
            concat!(
                "namespace App { public partial class Engine : Machine {",
                " public void Go() { base.Reset(); Valve.Stop(); } } }\n"
            ),
        ),
        (
            "Valve.cs",
            "namespace App { public static class Valve { public static void Stop() {} } }\n",
        ),
    ]);
    assert!(has_call(&ext, "Engine.Go()", "Machine.Reset()"));
    assert!(has_call(&ext, "Engine.Go()", "Valve.Stop()"));
    assert!(!has_call(&ext, "Engine.Go()", "Engine.Reset()"));
    assert!(!has_call(&ext, "Engine.Go()", "Engine.Stop()"));
}

// ---------------------------------------------------------------------------
// upstream `test_csharp_overloads.py` (13 tests): none ported.
// Go gives overloads one node (the id is the type and method name), so every test that needs a
// node per overload, a call bound by argument count, `arity`/`signature` metadata or a salted id
// has nothing to assert on. `test_identical_signature_behind_preprocessor_stays_one_node` is
// left out too: Go walks only the direct members of a type body, so what it makes of a method
// inside `#if` is not traced.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// upstream `test_csharp_member_nodes.py` (18 tests).
// Left out: every test of a property, indexer, operator or accessor node (Go makes no node for
// them, no `defines` edge, and records no call made inside them).
// ---------------------------------------------------------------------------

// pins Go v0.12.0; upstream 0.9.107 differs -> P3
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_a_field_alone_makes_no_member_node_but_keeps_its_type_reference() {
    // Fields are out for now, and the reference to a field's type is untouched.
    let ext = run(&[(
        "S.cs",
        r#"public class Runner {
    private readonly Worker _worker;
}
public class Worker { }
"#,
    )]);
    assert!(pairs(&ext, "defines").is_empty());
    // Upstream also asserts the edge's context is "field"; a Go edge has no context.
    assert!(has_edge(&ext, "Runner", "references", "Worker"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B21
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_property_sharing_a_methods_id_does_not_lend_it_its_calls() {
    // The explicit interface property `IRun.Next` has no node in Go and its call is not
    // recorded, so the method keeps only its own call.
    let ext = run(&[(
        "S.cs",
        r#"public interface IRun { int Next { get; } }
public class Run : IRun {
    int Next() { return Other(); }
    int IRun.Next => Next();
    int Other() { return 0; }
}
"#,
    )]);
    assert!(!has_call(&ext, "Run.Next()", "Run.Next()"));
    assert!(has_call(&ext, "Run.Next()", "Run.Other()"));
}

// ---------------------------------------------------------------------------
// upstream `test_csharp_method_owner_labels.py` (12 tests).
// Left out: everything that goes through Python's watch, build, analyze or cache code, the
// bare-label test of the extraction (ported below asserting Go's `Type.Name()`), the Java and
// Swift label test, and the id/edge comparison test.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_issue_repro_methods_display_their_owning_type() {
    // Upstream runs this twice (`no_cluster` False, True) through `_rebuild_code`, and reads the
    // labels by node id; Go has one path and different ids, so the labels are read as a set.
    let ext = run(&[(
        "Report.cs",
        r#"public interface IReport { string Build(); }
public class PdfReport : IReport { public string Build() { return "pdf"; } }
public class CsvReport : IReport { public string Build() { return "csv"; } }
public class Exporter {
  private readonly IReport _report;
  public Exporter(IReport report) { _report = report; }
  public string Run() { return _report.Build(); }
}
"#,
    )]);
    let labels = label_set(&ext);
    for want in [
        "IReport.Build()",
        "PdfReport.Build()",
        "CsvReport.Build()",
        "Exporter.Run()",
        "IReport",
        "Report.cs",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }
}

// pins Go v0.12.0 (labels `CsvReport.Build()`, `Exporter.Run()`, ..., no constructor node);
// upstream 0.9.107 emits the bare `.Build()`, `.Run()` and `.Exporter()` -> P3
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_extraction_keeps_the_bare_resolver_label() {
    let ext = run(&[(
        "Report.cs",
        r#"public interface IReport { string Build(); }
public class PdfReport : IReport { public string Build() { return "pdf"; } }
public class CsvReport : IReport { public string Build() { return "csv"; } }
public class Exporter {
  private readonly IReport _report;
  public Exporter(IReport report) { _report = report; }
  public string Run() { return _report.Build(); }
}
"#,
    )]);
    let mut methods: Vec<&str> = ext
        .nodes
        .iter()
        .filter(|n| n.label.ends_with("()"))
        .map(|n| n.label.as_str())
        .collect();
    methods.sort();
    assert_eq!(
        methods,
        [
            "CsvReport.Build()",
            "Exporter.Run()",
            "IReport.Build()",
            "PdfReport.Build()"
        ]
    );
}

// ---------------------------------------------------------------------------
// upstream `test_csharp_object_creation.py` (30 tests).
// Left out: the constructor tests (Go has no constructor node: 8 tests), the two tests of a
// type nested in another (Go has no node for it), the `build_from_json` test and the
// incremental-rebuild test (Python internals).
// ---------------------------------------------------------------------------

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_explicitly_declared_local_links_to_constructed_type() {
    let ext = run(&[(
        "S.cs",
        r#"public class Worker { }
public class Caller {
    public void Go() { Worker w = new Worker(); }
}
"#,
    )]);
    assert!(has_call(&ext, "Caller.Go()", "Worker"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_var_local_links_to_constructed_type() {
    let ext = run(&[(
        "S.cs",
        r#"public class Worker { }
public class Caller {
    public void Go() { var w = new Worker(); }
}
"#,
    )]);
    assert!(has_call(&ext, "Caller.Go()", "Worker"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_argument_position_links_to_constructed_type() {
    // The publish shape: the message type never appears in a declared position.
    let ext = run(&[(
        "S.cs",
        r#"public class OrderPlaced { }
public class Publisher {
    public void Publish() { Send(new OrderPlaced()); }
    private void Send(object payload) { }
}
"#,
    )]);
    assert!(has_call(&ext, "Publisher.Publish()", "OrderPlaced"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_object_initializer_without_parens_links() {
    let ext = run(&[(
        "S.cs",
        r#"public class OrderPlaced { public long Id { get; set; } }
public class Publisher {
    public void Publish(long id) { Send(new OrderPlaced { Id = id }); }
    private void Send(object payload) { }
}
"#,
    )]);
    assert!(has_call(&ext, "Publisher.Publish()", "OrderPlaced"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_generic_construction_names_the_outer_type() {
    let ext = run(&[(
        "S.cs",
        r#"public class Cache<T> { }
public class Caller {
    public void Go() { var c = new Cache<string>(); }
}
"#,
    )]);
    assert!(has_call(&ext, "Caller.Go()", "Cache"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_qualified_construction_names_the_last_segment() {
    let ext = run(&[
        (
            "Store.cs",
            "namespace Infra.Data;\npublic class Cache { }\n",
        ),
        (
            "Caller.cs",
            r#"public class Caller {
    public void Go() { var c = new Infra.Data.Cache(); }
}
"#,
        ),
    ]);
    assert!(has_call(&ext, "Caller.Go()", "Cache"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_target_typed_new_produces_no_edge() {
    // `new()` carries no type node; guessing one needs the target's declared type.
    let ext = run(&[(
        "S.cs",
        r#"public class Worker { }
public class Caller {
    public Worker Build() { Worker w = new(); return w; }
}
"#,
    )]);
    let worker = ids_of(&ext, "Worker");
    assert!(!call_ids(&ext).iter().any(|(_, t)| worker.contains(t)));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_cross_file_publisher_reaches_the_message_class() {
    let ext = run(&[
        (
            "Events/OrderPlaced.cs",
            "namespace Demo.Events;\npublic class OrderPlaced { public long Id { get; set; } }\n",
        ),
        (
            "Publisher.cs",
            r#"using Demo.Events;
public class Publisher {
    public void Publish(long id) { Send(new OrderPlaced { Id = id }); }
    private void Send(object payload) { }
}
"#,
        ),
    ]);
    assert!(has_call(&ext, "Publisher.Publish()", "OrderPlaced"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_ambiguous_type_name_produces_no_edge() {
    // Two classes share a name, so the construction site cannot be pinned to one of them.
    let ext = run(&[
        ("Left.cs", "namespace Left;\npublic class Cache { }\n"),
        ("Right.cs", "namespace Right;\npublic class Cache { }\n"),
        (
            "Caller.cs",
            r#"using Right;
public class Caller {
    public void Go() { var c = new Cache(); }
}
"#,
        ),
    ]);
    let caches = ids_of(&ext, "Cache");
    assert_eq!(caches.len(), 2);
    assert!(!call_ids(&ext).iter().any(|(_, t)| caches.contains(t)));
}

const COLLIDING_CACHES: [(&str, &str); 2] = [
    ("Left.cs", "namespace Infra.Data;\npublic class Cache { }\n"),
    ("Right.cs", "namespace Other;\npublic class Cache { }\n"),
];

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_qualified_construction_picks_the_named_namespace() {
    // The bare name is ambiguous, but the source says which Cache it means.
    let caller = r#"public class Caller {
    public void Go() { var c = new Infra.Data.Cache(); }
}
"#;
    let ext = run(&[
        COLLIDING_CACHES[0],
        COLLIDING_CACHES[1],
        ("Caller.cs", caller),
    ]);
    let wanted = id_in(&ext, "Cache", "Left.cs");
    let other = id_in(&ext, "Cache", "Right.cs");
    let go = id_in(&ext, "Caller.Go()", "Caller.cs");
    let calls = call_ids(&ext);
    assert!(calls.contains(&(go, wanted)));
    assert!(!calls.contains(&(go, other)));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_partially_qualified_construction_stays_unresolved() {
    // `new Data.Cache()` under `using Infra;` would need the using directives in scope to become
    // a namespace. Resolving it by suffix would be a guess.
    let caller = r#"using Infra;
public class Caller {
    public void Go() { var c = new Data.Cache(); }
}
"#;
    let ext = run(&[
        COLLIDING_CACHES[0],
        COLLIDING_CACHES[1],
        ("Caller.cs", caller),
    ]);
    let caches = ids_of(&ext, "Cache");
    assert!(!call_ids(&ext).iter().any(|(_, t)| caches.contains(t)));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_qualified_construction_with_two_candidates_in_one_namespace() {
    // Same namespace declared in two files, both holding a Cache: the qualifier cannot separate
    // them either, so the guard still applies.
    let ext = run(&[
        ("One.cs", "namespace Infra.Data;\npublic class Cache { }\n"),
        ("Two.cs", "namespace Infra.Data;\npublic class Cache { }\n"),
        (
            "Caller.cs",
            r#"public class Caller {
    public void Go() { var c = new Infra.Data.Cache(); }
}
"#,
        ),
    ]);
    let caches = ids_of(&ext, "Cache");
    assert_eq!(caches.len(), 2);
    assert!(!call_ids(&ext).iter().any(|(_, t)| caches.contains(t)));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_receiver_typed_member_call_still_resolves() {
    // Guard for #1609: the object-creation path must not disturb the invocation path that binds
    // a call to its receiver's declared type.
    let ext = run(&[(
        "S.cs",
        r#"public class Server { public bool Save() => true; }
public class Cache  { public bool Save() => false; }
public class Repo {
    private Server _server = new Server();
    public bool Commit() { return _server.Save(); }
}
"#,
    )]);
    assert!(has_call(&ext, "Repo.Commit()", "Server.Save()"));
    assert!(!has_call(&ext, "Repo.Commit()", "Cache.Save()"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_qualified_construction_resolves_when_the_method_also_declares_the_type() {
    // The method takes the type as a parameter and constructs it. The declared position mints a
    // bare-name stub in this file; the call must land on the class, not on that stub.
    let caller = r#"public class Caller {
    public void Go(Infra.Data.Cache existing) { var c = new Infra.Data.Cache(); }
}
"#;
    let ext = run(&[
        COLLIDING_CACHES[0],
        COLLIDING_CACHES[1],
        ("Caller.cs", caller),
    ]);
    let go = id_in(&ext, "Caller.Go()", "Caller.cs");
    let left = id_in(&ext, "Cache", "Left.cs");
    let calls = call_ids(&ext);
    assert!(calls.contains(&(go, left)));
    let sourceless: BTreeSet<&str> = ext
        .nodes
        .iter()
        .filter(|n| n.label == "Cache" && n.source_file.is_empty())
        .map(|n| n.id.as_str())
        .collect();
    assert!(!calls.iter().any(|(_, t)| sourceless.contains(t)));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_qualified_construction_of_a_foreign_type_makes_no_stub_edge() {
    // `new System.Text.StringBuilder()` has no declaration in the corpus. No edge beats an edge
    // into a sourceless placeholder that stands for nothing.
    let ext = run(&[(
        "Caller.cs",
        r#"public class Caller {
    public void Go() { var b = new System.Text.StringBuilder(); }
}
"#,
    )]);
    let builders = ids_of(&ext, "StringBuilder");
    assert!(!call_ids(&ext).iter().any(|(_, t)| builders.contains(t)));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_construction_never_binds_to_a_same_named_method_cross_file() {
    let ext = run(&[
        (
            "Market.cs",
            concat!(
                "namespace Demo { public class Market {",
                " public static int List(int a) { return a; } } }\n"
            ),
        ),
        (
            "Use.cs",
            concat!(
                "namespace Demo { public class Use {",
                " public void Run() { var xs = new List<int>(); } } }\n"
            ),
        ),
    ]);
    assert!(!has_call(&ext, "Use.Run()", "Market.List()"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_construction_inside_a_same_named_method_is_no_self_loop() {
    let ext = run(&[(
        "Box.cs",
        concat!(
            "namespace Demo { public class Box {",
            " static Bounds Bounds(int a) { return new Bounds(a); } } }\n"
        ),
    )]);
    assert!(!has_call(&ext, "Box.Bounds()", "Box.Bounds()"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_construction_still_binds_to_the_type_beside_a_same_named_method() {
    let ext = run(&[
        ("Kit.cs", "namespace Demo { public class Piece { } }\n"),
        (
            "Shop.cs",
            concat!(
                "namespace Demo { public class Shop {",
                " public static int Piece() { return 0; } } }\n"
            ),
        ),
        (
            "Setup.cs",
            concat!(
                "namespace Demo { public class Setup {",
                " public void Link() { var p = new Piece(); } } }\n"
            ),
        ),
    ]);
    assert!(has_call(&ext, "Setup.Link()", "Piece"));
    assert!(!has_call(&ext, "Setup.Link()", "Shop.Piece()"));
}
