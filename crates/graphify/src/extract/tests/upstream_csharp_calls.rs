//! The cases of the upstream graphify v0.9.107 C# call tests whose assertion holds for graphify-go
//! v0.12.0 (see the `extract-csharp` block of `parity/README.md`): `test_csharp_member_calls.py`,
//! `test_csharp_member_chain_calls.py`, `test_csharp_var_call_return_type.py`,
//! `test_csharp_foreach_loop_variable.py`, `test_csharp_generic_callsites.py` and
//! `test_csharp_call_site_generic_args.py`. Sources are inline. Node ids differ by design, so a call
//! edge is named `Label[file]` (Go labels a method `Type.Method()`, upstream `.Method()`).
//!
//! Not ported:
//! - member_calls: the five preprocessor tests (`test_preprocessor_*`): Go has no directive blanking.
//! - member_chain_calls: the two constructor tests: Go makes no node for a constructor.
//! - var_call_return_type: the four tests that expect `var x = R.M<A>()` typed from M's return type
//!   (Go leaves it untyped, byx.46).
//! - generic_callsites: the two tests that expect a bare `Get<int>()` call edge (Go records only the
//!   type arguments of an unqualified generic call).
//! - call_site_generic_args: the exactly-once count of `IZeta` (Go's resolve keeps both edges and
//!   dedupes later, in the graph build); the rest of that test is ported.

use std::collections::{BTreeMap, BTreeSet};

use super::csharp_tests::{pairs, run};
use crate::model::{Extraction, Node};

/// (source, target) of each `calls` edge, as `Label[file]`, with the confidence of the first one.
type Calls = BTreeMap<(String, String), &'static str>;

fn file_base(f: &str) -> &str {
    f.rsplit('/').next().unwrap_or(f)
}

fn node_name(n: &Node) -> String {
    format!("{}[{}]", n.label, file_base(&n.source_file))
}

fn calls(ext: &Extraction) -> Calls {
    let by_id: BTreeMap<&str, &Node> = ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut out = Calls::new();
    for e in ext.edges.iter().filter(|e| e.relation.as_str() == "calls") {
        let (Some(s), Some(t)) = (by_id.get(e.source.as_str()), by_id.get(e.target.as_str()))
        else {
            continue;
        };
        out.entry((node_name(s), node_name(t)))
            .or_insert(e.confidence.as_str());
    }
    out
}

fn has_call(c: &Calls, s: &str, t: &str) -> bool {
    c.contains_key(&(s.to_string(), t.to_string()))
}

fn assert_call(c: &Calls, s: &str, t: &str) {
    assert!(has_call(c, s, t), "missing call {s} -> {t}; got {c:?}");
}

fn refute_call(c: &Calls, s: &str, t: &str) {
    assert!(!has_call(c, s, t), "unexpected call {s} -> {t}; got {c:?}");
}

/// No call edge from `s` to a node whose name contains `frag`.
fn refute_like(c: &Calls, s: &str, frag: &str) {
    assert!(
        !c.keys().any(|k| k.0 == s && k.1.contains(frag)),
        "unexpected call from {s} to *{frag}*; got {c:?}"
    );
}

/// No call edge at all from `s`.
fn refute_from(c: &Calls, s: &str) {
    assert!(
        !c.keys().any(|k| k.0 == s),
        "unexpected call from {s}; got {c:?}"
    );
}

fn conf(c: &Calls, s: &str, t: &str) -> Option<&'static str> {
    c.get(&(s.to_string(), t.to_string())).copied()
}

fn refs(ext: &Extraction) -> BTreeSet<(String, String)> {
    pairs(ext, "references")
}

// ---- test_csharp_member_calls.py ----

const AMBIG: &str = "public class Server { public bool Save() => true; }\n\
public class Cache  { public bool Save() => false; }\n\
public class Repo {\n\
    private Server _server = new Server();\n\
    public bool Commit() { return _server.Save(); }\n\
}\n";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_field_receiver_resolves_to_declared_type_not_bare_match() {
    let c = calls(&run(&[("S.cs", AMBIG)]));
    assert_call(&c, "Repo.Commit()[S.cs]", "Server.Save()[S.cs]");
    refute_call(&c, "Repo.Commit()[S.cs]", "Cache.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_parameter_receiver_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Svc { public static bool Copy(Server server) { return server.Save(); } }\n",
    )]));
    assert_call(&c, "Svc.Copy()[S.cs]", "Server.Save()[S.cs]");
    refute_call(&c, "Svc.Copy()[S.cs]", "Cache.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_local_var_receiver_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class R {\n\
             public bool A() { Server s = new Server(); return s.Save(); }\n\
             public bool B() { var v = new Server(); return v.Save(); }\n\
         }\n",
    )]));
    assert_call(&c, "R.A()[S.cs]", "Server.Save()[S.cs]"); // explicit-typed local
    assert_call(&c, "R.B()[S.cs]", "Server.Save()[S.cs]"); // var = new T() local
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_cross_file_receiver_resolves() {
    let c = calls(&run(&[
        (
            "Server.cs",
            "public class Server { public bool Save() => true; }\n\
             public class Cache  { public bool Save() => false; }\n",
        ),
        (
            "Repo.cs",
            "public class Repo { private Server _s = new Server(); \
             public bool Commit() { return _s.Save(); } }\n",
        ),
    ]));
    assert_call(&c, "Repo.Commit()[Repo.cs]", "Server.Save()[Server.cs]");
    refute_call(&c, "Repo.Commit()[Repo.cs]", "Cache.Save()[Server.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_this_and_static_receivers() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Util { public static int F() => 1; }\n\
         public class R {\n\
             public bool A() { return this.B(); }\n\
             public bool B() => true;\n\
             public int G() { return Util.F(); }\n\
         }\n",
    )]));
    assert_call(&c, "R.A()[S.cs]", "R.B()[S.cs]"); // this.B() -> R.B
    assert_call(&c, "R.G()[S.cs]", "Util.F()[S.cs]"); // Util.F() -> Util.F
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_untyped_receiver_emits_no_edge() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class R { public bool C(dynamic x) { return x.Save(); } }\n",
    )]));
    refute_like(&c, "R.C()[S.cs]", "Save");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_method_absent_on_type_emits_no_edge() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class R { private Server _s = new Server(); \
         public bool C() { return _s.Missing(); } }\n",
    )]));
    refute_like(&c, "R.C()[S.cs]", "Save");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_unqualified_call_still_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class R { public bool A() { Helper(); return true; } \
         private void Helper() {} }\n",
    )]));
    assert_call(&c, "R.A()[S.cs]", "R.Helper()[S.cs]");
}

const NS_A: &str = "namespace A { public class Svc { public bool Do() => true; } }\n";
const NS_B: &str = "namespace B { public class Svc { public bool Do() => false; } }\n";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_namespace_using_directive_disambiguates_receiver_type() {
    let c = calls(&run(&[
        ("A.cs", NS_A),
        ("B.cs", NS_B),
        (
            "Caller.cs",
            "using A;\n\
             namespace App {\n\
                 public class Runner { public bool Go(Svc s) { return s.Do(); } }\n\
             }\n",
        ),
    ]));
    assert_call(&c, "Runner.Go()[Caller.cs]", "Svc.Do()[A.cs]");
    refute_call(&c, "Runner.Go()[Caller.cs]", "Svc.Do()[B.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_namespace_using_directive_resolves_to_other_namespace() {
    let c = calls(&run(&[
        ("A.cs", NS_A),
        ("B.cs", NS_B),
        (
            "Caller.cs",
            "using B;\n\
             namespace App {\n\
                 public class Runner { public bool Go(Svc s) { return s.Do(); } }\n\
             }\n",
        ),
    ]));
    assert_call(&c, "Runner.Go()[Caller.cs]", "Svc.Do()[B.cs]");
    refute_call(&c, "Runner.Go()[Caller.cs]", "Svc.Do()[A.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_namespace_ambiguous_without_using_bails() {
    let c = calls(&run(&[
        ("A.cs", NS_A),
        ("B.cs", NS_B),
        (
            "Caller.cs",
            "namespace App {\n\
                 public class Runner { public bool Go(Svc s) { return s.Do(); } }\n\
             }\n",
        ),
    ]));
    refute_like(&c, "Runner.Go()[Caller.cs]", "Svc.Do()");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_same_namespace_receiver_resolves_without_using() {
    let c = calls(&run(&[
        ("A.cs", NS_A),
        ("B.cs", NS_B),
        (
            "A2.cs",
            "namespace A {\n\
                 public class Client { public bool Go(Svc s) { return s.Do(); } }\n\
             }\n",
        ),
    ]));
    assert_call(&c, "Client.Go()[A2.cs]", "Svc.Do()[A.cs]");
    refute_call(&c, "Client.Go()[A2.cs]", "Svc.Do()[B.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_local_shadowing_field_of_different_type_poisons_name() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Run() => true; }\n\
         public class Other  { public bool Run() => false; }\n\
         public class Holder {\n\
             private Server x = new Server();\n\
             public bool A() { Other x = new Other(); return x.Run(); }\n\
         }\n",
    )]));
    refute_call(&c, "Holder.A()[S.cs]", "Server.Run()[S.cs]");
    refute_call(&c, "Holder.A()[S.cs]", "Other.Run()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_untyped_redeclaration_poisons_typed_field() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Run() => true; }\n\
         public class Holder {\n\
             private Server x = new Server();\n\
             public object Compute() => new object();\n\
             public bool A() { var x = Compute(); return x.Run(); }\n\
         }\n",
    )]));
    refute_like(&c, "Holder.A()[S.cs]", "Run()");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_this_field_receiver_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Repo {\n\
             private Server _s = new Server();\n\
             public bool Commit() { return this._s.Save(); }\n\
         }\n",
    )]));
    assert_call(&c, "Repo.Commit()[S.cs]", "Server.Save()[S.cs]");
    refute_call(&c, "Repo.Commit()[S.cs]", "Cache.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_null_conditional_field_receiver_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Repo {\n\
             private Server _s = new Server();\n\
             public bool? Commit() { return _s?.Save(); }\n\
         }\n",
    )]));
    assert_call(&c, "Repo.Commit()[S.cs]", "Server.Save()[S.cs]");
    refute_call(&c, "Repo.Commit()[S.cs]", "Cache.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_null_conditional_this_field_receiver_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Repo {\n\
             private Server _s = new Server();\n\
             public bool? Commit() { return this._s?.Save(); }\n\
         }\n",
    )]));
    assert_call(&c, "Repo.Commit()[S.cs]", "Server.Save()[S.cs]");
    refute_call(&c, "Repo.Commit()[S.cs]", "Cache.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_null_conditional_bare_this_receiver_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Other { public bool Save() => false; }\n\
         public class Repo {\n\
             public bool Save() => true;\n\
             public bool? Commit() { return this?.Save(); }\n\
         }\n",
    )]));
    assert_call(&c, "Repo.Commit()[S.cs]", "Repo.Save()[S.cs]");
    refute_call(&c, "Repo.Commit()[S.cs]", "Other.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_null_conditional_cross_file_receiver_resolves() {
    let c = calls(&run(&[
        (
            "Server.cs",
            "public class Server { public bool Save() => true; }\n\
             public class Cache  { public bool Save() => false; }\n",
        ),
        (
            "Repo.cs",
            "public class Repo { private Server _s = new Server(); \
             public bool? Commit() { return _s?.Save(); } }\n",
        ),
    ]));
    assert_call(&c, "Repo.Commit()[Repo.cs]", "Server.Save()[Server.cs]");
    refute_call(&c, "Repo.Commit()[Repo.cs]", "Cache.Save()[Server.cs]");
}

/// `Editor.Run()` with `body`, next to `IStore`/`Store` in another file.
fn store_call(body: &str) -> Calls {
    calls(&run(&[
        (
            "Store.cs",
            "public interface IStore { void Save(); }\n\
             public class Store : IStore { public void Save() { } }\n",
        ),
        (
            "Editor.cs",
            &format!(
                "public class Editor {{\n    object _obj;\n    IStore Store => (IStore)_obj;\n    public void Run() {{ {body} }}\n}}\n"
            ),
        ),
    ]))
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_cast_receiver_resolves_to_cast_type() {
    let c = store_call("((IStore)_obj).Save();");
    assert_call(&c, "Editor.Run()[Editor.cs]", "IStore.Save()[Store.cs]");
    refute_call(&c, "Editor.Run()[Editor.cs]", "Store.Save()[Store.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_as_cast_receiver_resolves_to_cast_type() {
    let c = store_call("(_obj as IStore).Save();");
    assert_call(&c, "Editor.Run()[Editor.cs]", "IStore.Save()[Store.cs]");
    refute_call(&c, "Editor.Run()[Editor.cs]", "Store.Save()[Store.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_property_receiver_shadows_same_named_type() {
    let c = store_call("Store.Save();");
    assert_call(&c, "Editor.Run()[Editor.cs]", "IStore.Save()[Store.cs]");
    refute_call(&c, "Editor.Run()[Editor.cs]", "Store.Save()[Store.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_base_receiver_resolves_to_base_class_method() {
    let c = calls(&run(&[
        (
            "Base.cs",
            "public class BaseSvc { public bool Ping() => true; }\n",
        ),
        (
            "Sub.cs",
            "public class Sub : BaseSvc {\n\
                 public bool Go() { return base.Ping(); }\n\
             }\n",
        ),
    ]));
    assert_call(&c, "Sub.Go()[Sub.cs]", "BaseSvc.Ping()[Base.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_inherited_method_resolves_through_base_chain() {
    let c = calls(&run(&[(
        "S.cs",
        "public class BaseSvc { public bool Ping() => true; }\n\
         public class Derived : BaseSvc { }\n\
         public class User {\n\
             public bool Use(Derived d) { return d.Ping(); }\n\
         }\n",
    )]));
    assert_call(&c, "User.Use()[S.cs]", "BaseSvc.Ping()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_unresolved_base_poisons_inherited_member_lookup() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Ext : NotInCorpus { }\n\
         public class User {\n\
             public bool U(Ext e) { return e.Save(); }\n\
         }\n",
    )]));
    refute_like(&c, "User.U()[S.cs]", "Save");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_cross_method_name_reuse_does_not_poison() {
    let c = calls(&run(&[
        (
            "Item.cs",
            "namespace Demo {\n    public class Item { public void Handle() {} }\n}\n",
        ),
        (
            "Runner.cs",
            "using System.Collections.Generic;\n\
             namespace Demo {\n\
                 public class Runner {\n\
                     public void RunOne(Item item) { item.Handle(); }\n\
                     public void RunIndexed(List<Item> items, int i) {\n\
                         var item = items[i];\n\
                         item.Handle();\n\
                     }\n\
                 }\n\
             }\n",
        ),
    ]));
    let handle = "Item.Handle()[Item.cs]";
    assert_call(&c, "Runner.RunOne()[Runner.cs]", handle);
    assert_eq!(
        conf(&c, "Runner.RunOne()[Runner.cs]", handle),
        Some("INFERRED")
    );
    refute_call(&c, "Runner.RunIndexed()[Runner.cs]", handle);
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_per_method_locals_resolve_independently() {
    let c = calls(&run(&[(
        "S.cs",
        "public class HtmlWriter { public void Render() {} }\n\
         public class TextWriter { public void Render() {} }\n\
         public class Doc {\n\
             public void AsHtml() { var w = new HtmlWriter(); w.Render(); }\n\
             public void AsText() { var w = new TextWriter(); w.Render(); }\n\
         }\n",
    )]));
    assert_call(&c, "Doc.AsHtml()[S.cs]", "HtmlWriter.Render()[S.cs]");
    assert_call(&c, "Doc.AsText()[S.cs]", "TextWriter.Render()[S.cs]");
    refute_call(&c, "Doc.AsHtml()[S.cs]", "TextWriter.Render()[S.cs]");
    refute_call(&c, "Doc.AsText()[S.cs]", "HtmlWriter.Render()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_same_method_shadow_still_poisons() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Run() => true; }\n\
         public class Other  { public bool Run() => false; }\n\
         public class Holder {\n\
             public bool A(Server x) { Other x = new Other(); return x.Run(); }\n\
         }\n",
    )]));
    refute_call(&c, "Holder.A()[S.cs]", "Server.Run()[S.cs]");
    refute_call(&c, "Holder.A()[S.cs]", "Other.Run()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_file_scoped_namespace_receiver_resolves() {
    let c = calls(&run(&[
        (
            "Item.cs",
            "namespace Demo;\npublic class Item { public void Handle() {} }\n",
        ),
        (
            "Runner.cs",
            "namespace Demo;\n\
             public class Runner {\n\
                 public void RunOne(Item item) { item.Handle(); }\n\
             }\n",
        ),
    ]));
    assert_call(&c, "Runner.RunOne()[Runner.cs]", "Item.Handle()[Item.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_method_chained_off_new_expression_resolves() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Merger {\n\
             public Merger(int x) {}\n\
             public int Combine(int a, bool b) { return a; }\n\
         }\n\
         public class Svc {\n\
             public int Run(int ctx) {\n\
                 return new Merger(ctx).Combine(ctx, true);\n\
             }\n\
         }\n",
    )]));
    assert_call(&c, "Svc.Run()[S.cs]", "Merger.Combine()[S.cs]");
}

const TWO_GO: &str = "public class Sect { public bool Go() => true; }\n\
public class Twig { public bool Go() => false; }\n";

/// `TWO_GO`, then `rest`, in one file `S.cs`.
fn two_go(rest: &str) -> Calls {
    calls(&run(&[("S.cs", &format!("{TWO_GO}{rest}"))]))
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_out_declared_receiver_resolves() {
    let c = two_go(
        "public class Box { public bool TryGet(out Sect s) { s = new Sect(); return true; } }\n\
         public class R {\n\
             public bool A(Box b) { if (b.TryGet(out Sect s)) { return s.Go(); } return false; }\n\
         }\n",
    );
    assert_call(&c, "R.A()[S.cs]", "Sect.Go()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Twig.Go()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_out_var_receiver_stays_unbound() {
    let c = two_go(
        "public class Box { public bool TryGet(out Sect s) { s = new Sect(); return true; } }\n\
         public class R {\n\
             public bool B(Box b) { b.TryGet(out var v); return v.Go(); }\n\
         }\n",
    );
    refute_like(&c, "R.B()[S.cs]", "Go()");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_is_pattern_receiver_resolves() {
    let c = two_go(
        "public class R {\n\
             public bool A(object o) { if (o is Sect s) { return s.Go(); } return false; }\n\
         }\n",
    );
    assert_call(&c, "R.A()[S.cs]", "Sect.Go()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Twig.Go()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_is_not_pattern_receiver_resolves() {
    let c = two_go(
        "public class R {\n\
             public bool A(object o) { if (o is not Sect s) { return false; } return s.Go(); }\n\
         }\n",
    );
    assert_call(&c, "R.A()[S.cs]", "Sect.Go()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Twig.Go()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_case_pattern_receiver_resolves() {
    let c = two_go(
        "public class R {\n\
             public bool A(object o) {\n\
                 switch (o) { case Sect s: return s.Go(); }\n\
                 return false;\n\
             }\n\
         }\n",
    );
    assert_call(&c, "R.A()[S.cs]", "Sect.Go()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Twig.Go()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_switch_arm_pattern_receiver_resolves() {
    let c = two_go(
        "public class R {\n\
             public bool A(object o) {\n\
                 return o switch { Sect s => s.Go(), _ => false };\n\
             }\n\
         }\n",
    );
    assert_call(&c, "R.A()[S.cs]", "Sect.Go()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Twig.Go()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_static_local_function_param_survives_out_var_reuse() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Target2 {\n\
             public bool Alpha() => true;\n\
             public bool Beta() => true;\n\
             public bool Gamma() => true;\n\
         }\n\
         public class Maker { public bool Make(out int v) { v = 1; return true; } }\n\
         public class R {\n\
             public bool Outer(Maker m) {\n\
                 m.Make(out var shared);\n\
                 shared.Gamma();\n\
                 return Inner(new Target2());\n\
                 static bool Inner(Target2 shared) { return shared.Alpha() && shared.Beta(); }\n\
             }\n\
         }\n",
    )]));
    let outer = "R.Outer()[S.cs]";
    for t in ["Target2.Alpha()[S.cs]", "Target2.Beta()[S.cs]"] {
        assert_call(&c, outer, t);
        assert_eq!(conf(&c, outer, t), Some("INFERRED"), "{t}");
    }
    refute_call(&c, outer, "Target2.Gamma()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_untypeable_out_var_in_sibling_block_does_not_poison() {
    let c = calls(&run(&[(
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Maker { public bool Make(out int v) { v = 1; return true; } }\n\
         public class R {\n\
             public bool A(Maker m, bool flag) {\n\
                 if (flag) {\n\
                     Server s = new Server();\n\
                     return s.Save();\n\
                 } else {\n\
                     m.Make(out var s);\n\
                     return true;\n\
                 }\n\
             }\n\
         }\n",
    )]));
    assert_call(&c, "R.A()[S.cs]", "Server.Save()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Cache.Save()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_sibling_pattern_rebind_conflict_poisons() {
    let c = two_go(
        "public class R {\n\
             public bool A(object o) {\n\
                 if (o is Sect x) { return x.Go(); }\n\
                 if (o is Twig x) { return x.Go(); }\n\
                 return false;\n\
             }\n\
         }\n",
    );
    refute_call(&c, "R.A()[S.cs]", "Sect.Go()[S.cs]");
    refute_call(&c, "R.A()[S.cs]", "Twig.Go()[S.cs]");
}

const PRIMARY_CTOR: &str = "public interface IDep { bool Plain(); }\n\
public class Holder(IDep dep) {\n\
    public bool Run() { return dep.Plain(); }\n\
}\n";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_primary_constructor_parameter_emits_references_edge() {
    let r = refs(&run(&[("S.cs", PRIMARY_CTOR)]));
    assert!(
        r.contains(&("Holder".to_string(), "IDep".to_string())),
        "{r:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_primary_constructor_parameter_resolves_member_calls() {
    let c = calls(&run(&[("S.cs", PRIMARY_CTOR)]));
    assert_call(&c, "Holder.Run()[S.cs]", "IDep.Plain()[S.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_record_positional_parameter_emits_references_edge() {
    let r = refs(&run(&[(
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public record Holder(IDep Dep) {\n\
             public bool Run() { return Dep.Plain(); }\n\
         }\n",
    )]));
    assert!(
        r.contains(&("Holder".to_string(), "IDep".to_string())),
        "{r:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_primary_constructor_type_parameter_is_not_referenced() {
    let r = refs(&run(&[(
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public class Holder<T>(IDep dep, T item) {\n\
             public bool Run() { return dep.Plain(); }\n\
         }\n",
    )]));
    assert!(
        r.contains(&("Holder".to_string(), "IDep".to_string())),
        "{r:?}"
    );
    assert!(!r.iter().any(|(s, t)| s == "Holder" && t == "T"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_primary_constructor_builtin_param_is_not_fabricated() {
    let r = refs(&run(&[(
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public class Holder(IDep dep, int count) {\n\
             public bool Run() { return dep.Plain(); }\n\
         }\n",
    )]));
    assert!(
        r.contains(&("Holder".to_string(), "IDep".to_string())),
        "{r:?}"
    );
    assert!(
        !r.iter()
            .any(|(s, t)| s == "Holder" && (t == "int" || t == "Int32")),
        "{r:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_struct_primary_constructor_parameter_emits_references_edge() {
    let r = refs(&run(&[(
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public struct Holder(IDep dep) {\n\
             public bool Run() { return dep.Plain(); }\n\
         }\n",
    )]));
    assert!(
        r.contains(&("Holder".to_string(), "IDep".to_string())),
        "{r:?}"
    );
}

// ---- test_csharp_member_chain_calls.py ----

const CHAIN_MODEL: &str = "namespace Demo\n{\n\
    public class Sequence { public void Apply() { } }\n\
    public class Decoy { public void Apply() { } }\n\
    public class Run { public Sequence Sequence; public Decoy[] Decoys; }\n\
}\n";

fn controller(body: &str, extra: &str) -> String {
    format!(
        "namespace Demo\n{{\n    public class Controller {{ {extra} public void Tick(Run run) {{ {body} }} }}\n}}\n"
    )
}

fn chain(body: &str, extra: &str) -> Calls {
    calls(&run(&[
        ("Model.cs", CHAIN_MODEL),
        ("Controller.cs", &controller(body, extra)),
    ]))
}

const TICK: &str = "Controller.Tick()[Controller.cs]";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_chain_on_parameter_field_resolves() {
    let c = chain("run.Sequence.Apply();", "");
    assert_call(&c, TICK, "Sequence.Apply()[Model.cs]");
    refute_call(&c, TICK, "Decoy.Apply()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_chain_on_field_receiver_resolves() {
    let c = chain("_run.Sequence.Apply();", "private Run _run;");
    assert_call(&c, TICK, "Sequence.Apply()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_chain_with_unknown_field_gives_no_edge() {
    let c = chain("run.Missing.Apply(); run.Decoys.Apply();", "");
    refute_from(&c, TICK);
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_chain_with_untyped_root_gives_no_edge() {
    let c = chain("var x = Make(); x.Sequence.Apply();", "");
    refute_from(&c, TICK);
}

const CONFIG: &str = "namespace Demo\n{\n\
    public class Config { public void WriteIds() { } }\n\
    public class Other { public void WriteIds() { } }\n\
}\n";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_element_access_on_array_field_resolves_to_element_type() {
    let c = calls(&run(&[
        ("Config.cs", CONFIG),
        (
            "Anim.cs",
            "namespace Demo\n{\n\
                 public class Anim\n    {\n\
                     private Config[] _configs;\n\
                     public void Save() { for (int i = 0; i < _configs.Length; i++) \
             if (_configs[i] != null) _configs[i].WriteIds(); }\n\
                 }\n}\n",
        ),
    ]));
    assert_call(&c, "Anim.Save()[Anim.cs]", "Config.WriteIds()[Config.cs]");
    refute_call(&c, "Anim.Save()[Anim.cs]", "Other.WriteIds()[Config.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_element_access_on_list_local_resolves() {
    let c = calls(&run(&[
        ("Config.cs", CONFIG),
        (
            "Anim.cs",
            "using System.Collections.Generic;\n\
             namespace Demo\n{\n\
                 public class Anim\n    {\n\
                     public void Save() { List<Config> items = Load(); items[0].WriteIds(); }\n\
                 }\n}\n",
        ),
    ]));
    assert_call(&c, "Anim.Save()[Anim.cs]", "Config.WriteIds()[Config.cs]");
    refute_call(&c, "Anim.Save()[Anim.cs]", "Other.WriteIds()[Config.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_element_access_without_element_type_gives_no_edge() {
    // a non-collection local shadows the array field; object[] has no class
    let c = calls(&run(&[
        ("Config.cs", CONFIG),
        (
            "Anim.cs",
            "namespace Demo\n{\n\
                 public class Anim\n    {\n\
                     private Config[] _configs;\n\
                     private object[] _objs;\n\
                     public void Save() { var _configs = Load(); _configs[0].WriteIds(); \
             _objs[0].WriteIds(); }\n\
                 }\n}\n",
        ),
    ]));
    refute_from(&c, "Anim.Save()[Anim.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_issue_3797_shapes_still_resolve() {
    let c = calls(&run(&[
        (
            "Store.cs",
            "public interface IStore { void Save(); }\n\
             public class Store : IStore { public void Save() { } }\n",
        ),
        (
            "Editor.cs",
            "public class Editor {\n\
                 private object _obj; private IStore _s;\n\
                 public void A() { ((IStore)_obj).Save(); }\n\
                 public void B() { _s?.Save(); }\n\
             }\n",
        ),
    ]));
    for name in ["Editor.A()[Editor.cs]", "Editor.B()[Editor.cs]"] {
        assert_call(&c, name, "IStore.Save()[Store.cs]");
        refute_call(&c, name, "Store.Save()[Store.cs]");
    }
}

const CONFIGS: &str = "namespace Demo\n{\n\
    public class Config { public void WriteIds() { } }\n\
    public class List { public void WriteIds() { } }\n\
}\n";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_element_of_type_parameter_list_stays_unresolved() {
    // `Config` here is the class's type parameter, not Demo.Config.
    let c = calls(&run(&[
        ("Config.cs", CONFIGS),
        (
            "Holder.cs",
            "using System.Collections.Generic;\nnamespace Demo\n{\n\
                 public class Holder<Config>\n    {\n\
                     public List<Config> Items { get; set; }\n\
                     public void Run() { Items[0].WriteIds(); }\n\
                     public void Local(List<Config> xs) { xs[0].WriteIds(); }\n\
                 }\n}\n",
        ),
    ]));
    assert!(
        !c.keys().any(|k| k.0.ends_with("[Holder.cs]")),
        "unexpected call from Holder: {c:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_element_of_nested_list_stays_unresolved() {
    let c = calls(&run(&[
        ("Config.cs", CONFIGS),
        (
            "Grid.cs",
            "using System.Collections.Generic;\nnamespace Demo\n{\n\
                 public class Grid\n    {\n\
                     private List<List<Config>> _grid;\n\
                     public void Run() { _grid[0].WriteIds(); }\n\
                 }\n}\n",
        ),
    ]));
    assert!(
        !c.keys().any(|k| k.0.ends_with("[Grid.cs]")),
        "unexpected call from Grid: {c:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_chain_never_binds_to_the_callers_same_named_method() {
    let c = chain("run.Sequence.Apply();", "public void Apply() { }");
    refute_call(&c, TICK, "Controller.Apply()[Controller.cs]");
    assert_call(&c, TICK, "Sequence.Apply()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_chained_receiver_in_one_file_gives_no_self_loop() {
    let c = calls(&run(&[(
        "Crowd.cs",
        "namespace Demo\n{\n\
             public class Crowd\n    {\n\
                 public void LateUpdate() { Pick().look.LateUpdate(); }\n\
                 object Pick() { return null; }\n\
             }\n}\n",
    )]));
    refute_call(
        &c,
        "Crowd.LateUpdate()[Crowd.cs]",
        "Crowd.LateUpdate()[Crowd.cs]",
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_untyped_chain_never_binds_to_a_same_file_method() {
    let c = calls(&run(&[(
        "Inventory.cs",
        "using System.Collections.Generic;\nnamespace Demo\n{\n\
             public class State { public List<int> Items; }\n\
             public class Inventory\n    {\n\
                 public int Count() { return 0; }\n\
                 public int Owned(State s) { return s.Items.Count(); }\n\
             }\n}\n",
    )]));
    refute_call(
        &c,
        "Inventory.Owned()[Inventory.cs]",
        "Inventory.Count()[Inventory.cs]",
    );
}

// ---- test_csharp_var_call_return_type.py ----
// The four tests that expect `var x = ServiceFactory.M<A>()` to type x from M's return type
// (generic_factory_same_file, generic_factory_cross_file, non_generic_return_type,
// generic_list_return_type) are not ported: Go leaves such a `var` untyped. The negative ones
// below hold in Go because the `var` is never typed.

const LIB: &str = "namespace Demo\n{\n\
    public interface IReport { string Build(string name); }\n\
    public class Report : IReport { public string Build(string name) => name; }\n\
    public class Widget { public void Spin() { } }\n\
    public class Item { }\n\
    public class List<T> { public void Add(T item) { } }\n\
    public class Box<T> { public T Value() => default; }\n\
    public static class ServiceFactory\n    {\n\
        public static T Get<T>(object scope) where T : new() => new T();\n\
        public static Widget Make() => new Widget();\n\
        public static List<T> CreateList<T>() => new List<T>();\n\
        public static Widget Pick(int a) => null;\n\
        public static IReport Pick(string b) => null;\n\
        public static T Infer<T>(T value) => value;\n\
    }\n}\n";

fn consumer(body: &str) -> Calls {
    calls(&run(&[
        ("Lib.cs", LIB),
        (
            "Consumer.cs",
            &format!(
                "namespace Demo\n{{\n    public class Consumer {{ public void Run(object scope) {{ {body} }} }}\n}}\n"
            ),
        ),
    ]))
}

const RUN: &str = "Consumer.Run()[Consumer.cs]";
const SPIN: &str = "Widget.Spin()[Lib.cs]";

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_ambiguous_overload_no_edge() {
    let c = consumer("var w = ServiceFactory.Pick(1); w.Spin();");
    refute_call(&c, RUN, SPIN);
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_missing_declaration_no_edge() {
    let c = consumer(
        "var w = ServiceFactory.Unknown<Widget>(); w.Spin(); var v = Elsewhere.Get<Widget>(scope); v.Spin();",
    );
    refute_call(&c, RUN, SPIN);
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_inferred_generic_no_edge() {
    let c = consumer("var w = ServiceFactory.Infer(new Widget()); w.Spin();");
    refute_call(&c, RUN, SPIN);
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_class_type_parameter_return_no_edge() {
    let c = consumer("var box = new Box<Widget>(); var w = box.Value(); w.Spin();");
    assert_call(&c, RUN, "Box.Value()[Lib.cs]");
    refute_call(&c, RUN, SPIN);
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_explicit_type_and_new_still_resolve() {
    let c = consumer(
        "IReport a = ServiceFactory.Get<IReport>(scope); a.Build(\"x\"); var w = new Widget(); w.Spin();",
    );
    assert_call(&c, RUN, "IReport.Build()[Lib.cs]");
    assert_call(&c, RUN, SPIN);
}

// ---- test_csharp_foreach_loop_variable.py ----

const FOREACH_MODEL: &str = "namespace Demo\n{\n\
    public class Look { public void Animate() { } }\n\
    public class Agent { public Look look; public void Step() { } }\n\
    public class Decoy { public void Step() { } public void Animate() { } }\n\
}\n";

fn crowd(methods: &str) -> Calls {
    calls(&run(&[
        ("Model.cs", FOREACH_MODEL),
        (
            "Crowd.cs",
            &format!(
                "using System.Collections.Generic;\nnamespace Demo\n{{\n    public class Crowd\n    {{\n\
                 \x20       readonly Dictionary<string, Agent> agents = new Dictionary<string, Agent>();\n\
                 \x20       readonly Dictionary<Agent, int> scores = new Dictionary<Agent, int>();\n\
                 \x20       readonly List<Agent> list = new List<Agent>();\n\
                 \x20       Agent[] array;\n\
                 \x20       public IEnumerable<Agent> All {{ get; set; }}\n\
                 \x20       {methods}\n    }}\n}}\n"
            ),
        ),
    ]))
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_var_over_dictionary_values_types_the_chain() {
    let c = crowd("public void Tick() { foreach (var a in agents.Values) a.look.Animate(); }");
    assert_call(&c, "Crowd.Tick()[Crowd.cs]", "Look.Animate()[Model.cs]");
    refute_call(&c, "Crowd.Tick()[Crowd.cs]", "Decoy.Animate()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_var_over_dictionary_keys_takes_the_key_type() {
    let c = crowd("public void Tick() { foreach (var a in scores.Keys) a.Step(); }");
    assert_call(&c, "Crowd.Tick()[Crowd.cs]", "Agent.Step()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_var_over_sequences_takes_the_element_type() {
    let c = crowd(
        "public void Listed() { foreach (var a in list) a.Step(); }\n\
         public void Arrayed() { foreach (var a in array) a.Step(); }\n\
         public void Property() { foreach (var a in All) a.Step(); }\n\
         public void ThisField() { foreach (var a in this.list) a.Step(); }",
    );
    for caller in ["Listed", "Arrayed", "Property", "ThisField"] {
        let src = format!("Crowd.{caller}()[Crowd.cs]");
        assert_call(&c, &src, "Agent.Step()[Model.cs]");
        refute_call(&c, &src, "Decoy.Step()[Model.cs]");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_explicitly_typed_loop_variable_binds_its_type() {
    let c = crowd("public void Tick(IEnumerable<object> xs) { foreach (Agent a in xs) a.Step(); }");
    assert_call(&c, "Crowd.Tick()[Crowd.cs]", "Agent.Step()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_iterating_a_dictionary_itself_stays_untyped() {
    let c = crowd("public void Tick() { foreach (var kv in agents) kv.Step(); }");
    refute_from(&c, "Crowd.Tick()[Crowd.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_a_local_shadowing_the_field_leaves_the_loop_untyped() {
    let c = crowd(
        "public void Tick() { var list = new List<Decoy>(); foreach (var a in list) a.Step(); }",
    );
    refute_call(&c, "Crowd.Tick()[Crowd.cs]", "Agent.Step()[Model.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_the_loop_variable_does_not_leak_past_its_loop() {
    let c = crowd("public void Tick(Decoy a) { foreach (var b in list) { } a.Step(); }");
    assert_call(&c, "Crowd.Tick()[Crowd.cs]", "Decoy.Step()[Model.cs]");
    refute_call(&c, "Crowd.Tick()[Crowd.cs]", "Agent.Step()[Model.cs]");
}

// ---- test_csharp_generic_callsites.py ----
// Not ported: test_unqualified_generic_call_resolves_to_base_member and
// test_same_class_generic_call_resolves (Go records no call edge for a bare `Get<int>()`).

fn generic_callsites() -> Calls {
    calls(&run(&[
        (
            "Settings.cs",
            "namespace Demo\n{\n    public class Settings\n    {\n\
                 \x20       public T Get<T>(string key) { return default(T); }\n\
                 \x20       public string GetRaw(string key) { return key; }\n    }\n}\n",
        ),
        (
            "Reader.cs",
            "namespace Demo\n{\n    public class Reader : Settings\n    {\n\
                 \x20       public int A() { return Get<int>(\"port\"); }\n\
                 \x20       public int B() { return this.Get<int>(\"port\"); }\n\
                 \x20       public string C() { return GetRaw(\"host\"); }\n\
                 \x20       public string D() { return this.GetRaw(\"host\"); }\n    }\n}\n",
        ),
        (
            "Local.cs",
            "namespace Demo\n{\n    public class Local\n    {\n\
                 \x20       public T Make<T>() { return default(T); }\n\
                 \x20       public int E() { return Make<int>(); }\n    }\n}\n",
        ),
    ]))
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_this_qualified_generic_call_resolves() {
    let c = generic_callsites();
    assert_call(&c, "Reader.B()[Reader.cs]", "Settings.Get()[Settings.cs]");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_generic_call_never_targets_the_bracketed_text() {
    let c = generic_callsites();
    assert!(!c.keys().any(|k| k.1.contains('<')), "{c:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_non_generic_controls_unchanged() {
    let c = generic_callsites();
    assert_call(
        &c,
        "Reader.C()[Reader.cs]",
        "Settings.GetRaw()[Settings.cs]",
    );
    assert_call(
        &c,
        "Reader.D()[Reader.cs]",
        "Settings.GetRaw()[Settings.cs]",
    );
}

// ---- test_csharp_call_site_generic_args.py ----
// Upstream writes some of these probes with an expression body (`=> r.Do<IThing>();`); they are
// block bodies here so the test does not depend on the grammar's `body` field for `=>`.

const GA_TYPES: &str = "public interface IThing { }\n\
public interface IService { }\n\
public interface IImpl { }\n\
public class Box<T> { }\n\
public static class StaticHolder\n\
{\n\
    public static void Invoke<T>() { }\n\
    public static void Register<T, U>() { }\n\
}\n\
public class Registry { public void Do<T>() { } }\n";

fn ga_refs(probe: &str) -> BTreeSet<(String, String)> {
    refs(&run(&[("T.cs", GA_TYPES), ("P.cs", probe)]))
}

fn has_ref(r: &BTreeSet<(String, String)>, s: &str, t: &str) -> bool {
    r.contains(&(s.to_string(), t.to_string()))
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_call_with_one_type_argument() {
    let r = ga_refs("public class Probe { public void A(Registry r) { r.Do<IThing>(); } }\n");
    assert!(has_ref(&r, "Probe.A()", "IThing"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_member_call_with_multiple_type_arguments() {
    let r = ga_refs(
        "public interface IServiceCollection { }\n\
         public static class Ext\n\
         {\n\
             public static void AddScoped<TService, TImpl>(this IServiceCollection s) { }\n\
         }\n\
         public class Probe\n\
         {\n\
             public void A(IServiceCollection s) { s.AddScoped<IService, IImpl>(); }\n\
         }\n",
    );
    assert!(has_ref(&r, "Probe.A()", "IService"), "{r:?}");
    assert!(has_ref(&r, "Probe.A()", "IImpl"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_null_conditional_member_call_with_type_argument() {
    let r = ga_refs("public class Probe { public void A(Registry r) { r?.Do<IThing>(); } }\n");
    assert!(has_ref(&r, "Probe.A()", "IThing"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_nested_type_argument_in_call_site() {
    let r = ga_refs("public class Probe { public void A(Registry r) { r.Do<Box<IThing>>(); } }\n");
    assert!(has_ref(&r, "Probe.A()", "IThing"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_call_without_type_argument_is_unchanged() {
    let r = ga_refs("public class Probe { public void A(Registry r) { r.Do(); } }\n");
    assert!(!has_ref(&r, "Probe.A()", "IThing"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_type_parameter_in_call_site_arg_is_not_fabricated() {
    let r = ga_refs(
        "public class Holder<T>\n\
         {\n\
             public void Use(Registry r) { r.Do<T>(); }\n\
         }\n",
    );
    assert!(!r.iter().any(|(_, t)| t == "T"), "{r:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_call_site_generic_args_appear_in_issue_repro() {
    let types = "public interface IAlpha { }\n\
        public interface IBeta { }\n\
        public interface IGamma { }\n\
        public interface IDelta { }\n\
        public interface IEpsilon { }\n\
        public interface IZeta { }\n\
        public class Box<T> { }\n\
        public class Registry { public void Do<T>() { } }\n\
        public interface IServiceCollection { }\n\
        public static class Ext\n\
        {\n\
            public static void AddScoped<TService, TImpl>(this IServiceCollection s) { }\n\
        }\n";
    let r = refs(&run(&[
        ("Types.cs", types),
        (
            "Probe.cs",
            "public class Probe\n\
             {\n\
                 private Box<IAlpha> _field = null!;\n\
                 public Box<IBeta> Prop { get; set; } = null!;\n\
                 public Box<IGamma> Ret() => null!;\n\
                 public void Param(Box<IDelta> p) { }\n\
                 public void Call(Registry r) { r.Do<IEpsilon>(); }\n\
                 public void Di(IServiceCollection s) { s.AddScoped<IZeta, Box<IZeta>>(); }\n\
             }\n",
        ),
    ]));
    assert!(has_ref(&r, "Probe.Call()", "IEpsilon"), "{r:?}");
    // Upstream also asserts IZeta is linked exactly once; graphify-go's resolve keeps both edges
    // (the graph build dedupes), so only existence is asserted here.
    assert!(has_ref(&r, "Probe.Di()", "IZeta"), "{r:?}");
    assert!(has_ref(&r, "Probe.Di()", "Box"), "{r:?}");
}
