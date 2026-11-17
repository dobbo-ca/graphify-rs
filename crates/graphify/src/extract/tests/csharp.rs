//! graphify-go `internal/extract/csharp_test.go`, and the helpers the C# upstream test files
//! (`upstream_csharp_*.rs`) share. The upstream cases of graphify v0.9.107 are in those files; the
//! `extract-csharp` block of `parity/README.md` says which hold for graphify-go v0.12.0.

use std::collections::{BTreeMap, BTreeSet};

use super::extract_csharp;
use crate::extract::{file_from_bytes, kinds, resolve};
use crate::model::Extraction;
use crate::testutil::testdata;

/// Go `FileFromBytes` on each `(rel, src)`, then `Resolve` (GOMAXPROCS workers).
pub(super) fn run(srcs: &[(&str, &str)]) -> Extraction {
    kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

pub(super) fn label_set(ext: &Extraction) -> BTreeSet<&str> {
    ext.nodes.iter().map(|n| n.label.as_str()).collect()
}

pub(super) fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// (source label, target label) of every edge of `relation`; a missing node is "".
pub(super) fn pairs(ext: &Extraction, relation: &str) -> BTreeSet<(String, String)> {
    let label = id2label(ext);
    let of = |id: &str| label.get(id).copied().unwrap_or("").to_string();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .map(|e| (of(&e.source), of(&e.target)))
        .collect()
}

pub(super) fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    pairs(ext, relation).contains(&(src.to_string(), tgt.to_string()))
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // csharp test (ihc.28): needs resolve
fn test_extract_c_sharp() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("csharpproj");
    let files = ["util/math.cs", "web/server.cs"];

    // .cs is not yet wired into File(); call the extractor directly per file.
    let mut results = Vec::new();
    for f in files {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("ReadFile({f}): {e}"));
        results.push(extract_csharp(f, &src).expect("parse budget"));
    }
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels = label_set(&ext);
    for want in [
        "math.cs",
        "server.cs",
        "Add()",
        "Server",
        "Server.Start()",
        "Boot()",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    // Method scoped under its type, with a contains edge.
    assert!(
        has_edge(&ext, "Server", "contains", "Server.Start()"),
        "expected Server --contains--> Server.Start"
    );
    // Same-file call: Server.Start -> Boot.
    assert!(
        has_edge(&ext, "Server.Start()", "calls", "Boot()"),
        "expected Server.Start --calls--> Boot"
    );
    // Cross-file call: Boot -> Add (unique global definition).
    assert!(
        has_edge(&ext, "Boot()", "calls", "Add()"),
        "expected cross-file Boot --calls--> Add"
    );

    // External import edge for System.Text.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports > 0,
        "no external import edges (expected System.Text)"
    );
}

/// The C# `(rel, src)` pairs the ignored tests write inline, verbatim (sources a helper builds
/// with `format!` are not here). The tests' named consts are read in place by
/// [`smoke_extract_csharp_sources`].
const INLINE_SOURCES: &[(&str, &str)] = &[
    // upstream_csharp_calls.rs
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Svc { public static bool Copy(Server server) { return server.Save(); } }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class R {\n\
             public bool A() { Server s = new Server(); return s.Save(); }\n\
             public bool B() { var v = new Server(); return v.Save(); }\n\
         }\n",
    ),
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
    (
        "S.cs",
        "public class Util { public static int F() => 1; }\n\
         public class R {\n\
             public bool A() { return this.B(); }\n\
             public bool B() => true;\n\
             public int G() { return Util.F(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class R { public bool C(dynamic x) { return x.Save(); } }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class R { private Server _s = new Server(); \
         public bool C() { return _s.Missing(); } }\n",
    ),
    (
        "S.cs",
        "public class R { public bool A() { Helper(); return true; } \
         private void Helper() {} }\n",
    ),
    (
        "Caller.cs",
        "using A;\n\
         namespace App {\n\
             public class Runner { public bool Go(Svc s) { return s.Do(); } }\n\
         }\n",
    ),
    (
        "Caller.cs",
        "using B;\n\
         namespace App {\n\
             public class Runner { public bool Go(Svc s) { return s.Do(); } }\n\
         }\n",
    ),
    (
        "Caller.cs",
        "namespace App {\n\
             public class Runner { public bool Go(Svc s) { return s.Do(); } }\n\
         }\n",
    ),
    (
        "A2.cs",
        "namespace A {\n\
             public class Client { public bool Go(Svc s) { return s.Do(); } }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Run() => true; }\n\
         public class Other  { public bool Run() => false; }\n\
         public class Holder {\n\
             private Server x = new Server();\n\
             public bool A() { Other x = new Other(); return x.Run(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Run() => true; }\n\
         public class Holder {\n\
             private Server x = new Server();\n\
             public object Compute() => new object();\n\
             public bool A() { var x = Compute(); return x.Run(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Repo {\n\
             private Server _s = new Server();\n\
             public bool Commit() { return this._s.Save(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Repo {\n\
             private Server _s = new Server();\n\
             public bool? Commit() { return _s?.Save(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Cache  { public bool Save() => false; }\n\
         public class Repo {\n\
             private Server _s = new Server();\n\
             public bool? Commit() { return this._s?.Save(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Other { public bool Save() => false; }\n\
         public class Repo {\n\
             public bool Save() => true;\n\
             public bool? Commit() { return this?.Save(); }\n\
         }\n",
    ),
    (
        "Repo.cs",
        "public class Repo { private Server _s = new Server(); \
         public bool? Commit() { return _s?.Save(); } }\n",
    ),
    (
        "Store.cs",
        "public interface IStore { void Save(); }\n\
         public class Store : IStore { public void Save() { } }\n",
    ),
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
    (
        "S.cs",
        "public class BaseSvc { public bool Ping() => true; }\n\
         public class Derived : BaseSvc { }\n\
         public class User {\n\
             public bool Use(Derived d) { return d.Ping(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Save() => true; }\n\
         public class Ext : NotInCorpus { }\n\
         public class User {\n\
             public bool U(Ext e) { return e.Save(); }\n\
         }\n",
    ),
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
    (
        "S.cs",
        "public class HtmlWriter { public void Render() {} }\n\
         public class TextWriter { public void Render() {} }\n\
         public class Doc {\n\
             public void AsHtml() { var w = new HtmlWriter(); w.Render(); }\n\
             public void AsText() { var w = new TextWriter(); w.Render(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public class Server { public bool Run() => true; }\n\
         public class Other  { public bool Run() => false; }\n\
         public class Holder {\n\
             public bool A(Server x) { Other x = new Other(); return x.Run(); }\n\
         }\n",
    ),
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
    (
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
    ),
    (
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
    ),
    (
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
    ),
    (
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public record Holder(IDep Dep) {\n\
             public bool Run() { return Dep.Plain(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public class Holder<T>(IDep dep, T item) {\n\
             public bool Run() { return dep.Plain(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public class Holder(IDep dep, int count) {\n\
             public bool Run() { return dep.Plain(); }\n\
         }\n",
    ),
    (
        "S.cs",
        "public interface IDep { bool Plain(); }\n\
         public struct Holder(IDep dep) {\n\
             public bool Run() { return dep.Plain(); }\n\
         }\n",
    ),
    (
        "Anim.cs",
        "namespace Demo\n{\n\
             public class Anim\n    {\n\
                 private Config[] _configs;\n\
                 public void Save() { for (int i = 0; i < _configs.Length; i++) \
         if (_configs[i] != null) _configs[i].WriteIds(); }\n\
             }\n}\n",
    ),
    (
        "Anim.cs",
        "using System.Collections.Generic;\n\
         namespace Demo\n{\n\
             public class Anim\n    {\n\
                 public void Save() { List<Config> items = Load(); items[0].WriteIds(); }\n\
             }\n}\n",
    ),
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
    (
        "Editor.cs",
        "public class Editor {\n\
             private object _obj; private IStore _s;\n\
             public void A() { ((IStore)_obj).Save(); }\n\
             public void B() { _s?.Save(); }\n\
         }\n",
    ),
    (
        "Holder.cs",
        "using System.Collections.Generic;\nnamespace Demo\n{\n\
             public class Holder<Config>\n    {\n\
                 public List<Config> Items { get; set; }\n\
                 public void Run() { Items[0].WriteIds(); }\n\
                 public void Local(List<Config> xs) { xs[0].WriteIds(); }\n\
             }\n}\n",
    ),
    (
        "Grid.cs",
        "using System.Collections.Generic;\nnamespace Demo\n{\n\
             public class Grid\n    {\n\
                 private List<List<Config>> _grid;\n\
                 public void Run() { _grid[0].WriteIds(); }\n\
             }\n}\n",
    ),
    (
        "Crowd.cs",
        "namespace Demo\n{\n\
             public class Crowd\n    {\n\
                 public void LateUpdate() { Pick().look.LateUpdate(); }\n\
                 object Pick() { return null; }\n\
             }\n}\n",
    ),
    (
        "Inventory.cs",
        "using System.Collections.Generic;\nnamespace Demo\n{\n\
             public class State { public List<int> Items; }\n\
             public class Inventory\n    {\n\
                 public int Count() { return 0; }\n\
                 public int Owned(State s) { return s.Items.Count(); }\n\
             }\n}\n",
    ),
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
    (
        "P.cs",
        "public class Probe { public void A(Registry r) { r.Do<IThing>(); } }\n",
    ),
    (
        "P.cs",
        "public interface IServiceCollection { }\n\
         public static class Ext\n\
         {\n\
             public static void AddScoped<TService, TImpl>(this IServiceCollection s) { }\n\
         }\n\
         public class Probe\n\
         {\n\
             public void A(IServiceCollection s) { s.AddScoped<IService, IImpl>(); }\n\
         }\n",
    ),
    (
        "P.cs",
        "public class Probe { public void A(Registry r) { r?.Do<IThing>(); } }\n",
    ),
    (
        "P.cs",
        "public class Probe { public void A(Registry r) { r.Do<Box<IThing>>(); } }\n",
    ),
    (
        "P.cs",
        "public class Probe { public void A(Registry r) { r.Do(); } }\n",
    ),
    (
        "P.cs",
        "public class Holder<T>\n\
         {\n\
             public void Use(Registry r) { r.Do<T>(); }\n\
         }\n",
    ),
    (
        "Types.cs",
        "public interface IAlpha { }\n\
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
        }\n",
    ),
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
    // upstream_csharp_types.rs
    (
        "Reader.cs",
        concat!(
            "namespace Demo\n{\n    public class Reader\n    {\n",
            "        public static (int mode, string label) Read() => (1, \"a\");\n",
            "    }\n}\n"
        ),
    ),
    (
        "Reader.cs",
        concat!(
            "namespace Demo\n{\n",
            "    public class Mode { }\n",
            "    public class Label { }\n",
            "    public class Reader\n    {\n",
            "        public static (Mode mode, Label label) Read() => (null!, null!);\n",
            "    }\n}\n"
        ),
    ),
    (
        "P.cs",
        "public class Probe { private Box<IThing> _f = null!; }\n",
    ),
    (
        "P.cs",
        concat!(
            "public class WithField { private Box<IThing> _f = null!; }\n",
            "public class WithProp  { public Box<IThing> P { get; set; } = null!; }\n"
        ),
    ),
    (
        "P.cs",
        "public class Probe { private Box<Box<IThing>> _f = null!; }\n",
    ),
    (
        "P.cs",
        "public class Probe { private Box<IThing> _a = null!, _b = null!; }\n",
    ),
    (
        "P.cs",
        "public class Probe<T> { private T _item = default!; }\n",
    ),
    (
        "P.cs",
        "public class Probe<T> { private Box<T> _f = null!; }\n",
    ),
    (
        "P.cs",
        "public class Probe { private Box<int> _f = null!; private string _s = \"\"; }\n",
    ),
    (
        "P.cs",
        "public class Probe { private IThing _f = null!; }\n",
    ),
    ("S.cs", "public enum ProcessingType {\n    Process\n}\n"),
    ("S.cs", "public enum Empty {\n}\n"),
    (
        "Handles.cs",
        concat!(
            "namespace Demo {\n",
            "    public enum State { Pause, Play }\n",
            "    public class Handle { public void Pause() { } }\n",
            "    public class Injector {\n",
            "        public Handle[] Handles;\n",
            "        public void StopAll() {\n",
            "            for (int i = 0; i < Handles.Length; i++) { Handles[i].Pause(); }\n",
            "        }\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "core.cs",
        "namespace Game.Core { public class Damage { public int Calc() { return 1; } } }\n",
    ),
    (
        "combat.cs",
        "using Game.Core;\nnamespace Game.Combat { public class Weapon : Damage {} }\n",
    ),
    ("gadget.cs", "public class Gadget {}\n"),
    (
        "user.cs",
        "global using System;\npublic class Widget : Gadget {}\n",
    ),
    (
        "core.cs",
        "namespace Game.Core { public enum Element { Fire, Ice } public class Damage {} }\n",
    ),
    (
        "combat.cs",
        concat!(
            "using Game.Core;\n",
            "namespace Game.Combat { public class Spell { Element element; Damage dmg; } }\n"
        ),
    ),
    (
        "core.cs",
        concat!(
            "namespace Game.Core { ",
            "public struct Coord { public int X; } ",
            "public record Player(string Name); ",
            "}\n"
        ),
    ),
    (
        "combat.cs",
        concat!(
            "using Game.Core;\n",
            "namespace Game.Combat { public class Spell { Coord coord; Player player; } }\n"
        ),
    ),
    (
        "core.cs",
        "namespace Game.Core { public class WeaponData { public int Number; } }\n",
    ),
    (
        "ui.cs",
        "namespace Game.UI { public class WeaponData { public int Width; } }\n",
    ),
    (
        "holder.cs",
        concat!(
            "using Game.Core;\n",
            "using Game.UI;\n",
            "namespace Game.Combat { public class Holder { public WeaponData data; } }\n"
        ),
    ),
    (
        "a.cs",
        "namespace N { class T {} class Box<T> { T value; } }\n",
    ),
    ("n.cs", "namespace N { class T {} }\n"),
    ("m.cs", "namespace M { class Use : N.T {} }\n"),
    (
        "g.cs",
        "namespace N { class Box<TI> {} class Use { N.Box<int> b; } }\n",
    ),
    ("n.cs", "namespace X.Y { class T {} }\n"),
    (
        "m.cs",
        "using B = X.Y;\nnamespace M { class Use : B.T {} }\n",
    ),
    ("b.cs", "namespace B { class T {} }\n"),
    (
        "m.cs",
        "namespace A { using B = X.Y; }\nnamespace M { class Use : B.T {} }\n",
    ),
    ("a.cs", "namespace N { class Outer { class Inner {} } }\n"),
    ("b.cs", "using N;\nnamespace M { class Use { Inner x; } }\n"),
    ("core.cs", "namespace N { class Box {} }\n"),
    ("b.cs", "using B = N.Box;\nclass Use : B {}\n"),
    (
        "u.cs",
        "using N;\nnamespace A { class X : T {} } namespace B { class Y : T {} }\n",
    ),
    (
        "u.cs",
        "namespace A { using N; namespace B { class Inner : T {} } }\n",
    ),
    (
        "effects.cs",
        concat!(
            "namespace Demo {\n",
            "    public enum EffectKind { Tween, Follow }\n",
            "    public abstract class Follow { }\n",
            "    public sealed class FollowFloat : Follow { }\n",
            "}\n"
        ),
    ),
    (
        "Kinds.cs",
        "namespace Demo {\n    public enum Channel { Vector2 }\n}\n",
    ),
    (
        "Mover.cs",
        concat!(
            "using UnityEngine;\n",
            "namespace Demo {\n",
            "    public class Mover {\n",
            "        public void Move(Vector2 delta) { }\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "Node.cs",
        concat!(
            "namespace Demo {\n",
            "    public class Node {\n",
            "        public System.Type Type { get; set; }\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "Mover.cs",
        concat!(
            "using System;\n",
            "namespace Demo {\n",
            "    public class Mover {\n",
            "        public string Describe(Type t) => t.Name;\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "A_Enum.cs",
        "namespace Demo {\n    public enum EffectKind { Tween, Follow }\n}\n",
    ),
    (
        "Follow.cs",
        concat!(
            "namespace Demo {\n",
            "    public abstract class Follow { }\n",
            "    public sealed class FollowFloat : Follow { }\n",
            "}\n"
        ),
    ),
    (
        "Demo.cs",
        concat!(
            "namespace Demo {\n",
            "    public class Base {}\n",
            "    public class Widget {}\n",
            "}\n"
        ),
    ),
    (
        "Inner.cs",
        concat!(
            "namespace Demo.Inner {\n",
            "    public class Registry {\n",
            "        public Widget Item;\n",
            "    }\n",
            "    public class Factory : Base {\n",
            "        public object Make() {\n",
            "            return new Widget();\n",
            "        }\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "nested.cs",
        concat!(
            "namespace Demo {\n",
            "    public class Base {}\n",
            "    public class Widget {}\n",
            "    namespace Inner {\n",
            "        public class Registry {\n",
            "            public Widget Item;\n",
            "        }\n",
            "        public class Factory : Base {\n",
            "            public object Make() {\n",
            "                return new Widget();\n",
            "            }\n",
            "        }\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "Demo.cs",
        "namespace Demo;\npublic class Base {}\npublic class Widget {}\n",
    ),
    (
        "Scoped.cs",
        concat!(
            "namespace Demo.Scoped;\n",
            "public class Registry {\n",
            "    public Widget Item;\n",
            "}\n",
            "public class Factory : Base {\n",
            "    public object Make() {\n",
            "        return new Widget();\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "types.cs",
        concat!(
            "namespace Demo {\n",
            "    public class OuterType {}\n",
            "    public class CollidingType {}\n",
            "}\n",
            "namespace Other {\n",
            "    public class ImportedType {}\n",
            "}\n",
            "namespace Demo.Inner {\n",
            "    using Other;\n",
            "    public class CollidingType {}\n",
            "    public class Tester {\n",
            "        public CollidingType Local;\n",
            "        public ImportedType Imported;\n",
            "        public UnknownType Missing;\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "layer.cs",
        "namespace Demo\n{\n    public class Layer\n    {\n        public class Record {}\n    }\n}\n",
    ),
    (
        "consumer.cs",
        "namespace Consumer\n{\n    public class Service\n    {\n        public Record value;\n    }\n}\n",
    ),
    (
        "layer_a.cs",
        "namespace DemoA\n{\n    public class LayerA\n    {\n        public class Record {}\n    }\n}\n",
    ),
    (
        "layer_b.cs",
        "namespace DemoB\n{\n    public class LayerB\n    {\n        public class Record {}\n    }\n}\n",
    ),
    (
        "consumer.cs",
        concat!(
            "using static DemoA.LayerA;\n",
            "using static DemoB.LayerB;\n",
            "namespace Consumer\n",
            "{\n",
            "    public class Service\n",
            "    {\n",
            "        public Record value;\n",
            "    }\n",
            "}\n"
        ),
    ),
    (
        "combat.cs",
        "using Game.Core;\nnamespace Game.Combat { public class Holder { public WeaponData data; } }\n",
    ),
    (
        "x.cs",
        "namespace B { class Use : T {} } namespace C { class T {} }\n",
    ),
    (
        "x.cs",
        "namespace B { class T {} } namespace C { class Use : T {} }\n",
    ),
    (
        "y.cs",
        "namespace Game { class Damage {} class Use : Game {} }\n",
    ),
    ("a.cs", "namespace A { class T {} class Use : B.T {} }\n"),
    ("xy.cs", "namespace X.Y { class T {} }\n"),
    (
        "use.cs",
        "namespace A { using B = X.Y; class Good : B.T {} }\nnamespace C { using B = Z.Q; }\n",
    ),
    (
        "u.cs",
        "namespace A { using N; class Good : T {} }\nnamespace A { class Bad : T {} }\n",
    ),
    (
        "outer.cs",
        "namespace Demo {\n    public class Widget { public int Level = 1; }\n}\n",
    ),
    (
        "inner.cs",
        "namespace Demo.Inner {\n    public class Widget { public int Level = 2; }\n}\n",
    ),
    (
        "deep.cs",
        "namespace Demo.Inner.Deep {\n    public class Consumer {\n        public Widget Item;\n    }\n}\n",
    ),
    (
        "other.cs",
        "namespace Other\n{\n    public class Record {}\n}\n",
    ),
    (
        "consumer.cs",
        "using Other;\nusing static Demo.Layer;\nnamespace Consumer\n{\n    public class Service\n    {\n        public Record value;\n    }\n}\n",
    ),
    (
        "A.cs",
        "using System.Collections.Generic;\nnamespace Demo { public class A { public List<int> Items; } }\n",
    ),
    (
        "B.cs",
        "using System.Collections.Generic;\nnamespace Demo { public class B { public List<string> Names; } }\n",
    ),
    (
        "core.cs",
        "namespace Game.Core { public class Damage {} }\n",
    ),
    (
        "combat.cs",
        "using Dmg = Game.Core.Damage;\nnamespace Game.Combat { public class Weapon : Dmg {} }\n",
    ),
    ("use.cs", "using Bx = N.Box<int>;\nclass Use : Bx {}\n"),
    (
        "u.cs",
        "namespace A { using AliasT = N.T; class Good : AliasT {} }\nnamespace A { class Bad : AliasT {} }\n",
    ),
    // upstream_csharp_members.rs
    (
        "A.cs",
        "namespace Alpha { public partial class Foo { public void FromA() {} } }\n",
    ),
    (
        "B.cs",
        "namespace Beta { public partial class Foo { public void FromB() {} } }\n",
    ),
    (
        "A.cs",
        "namespace App { public partial class Foo { public void FromA() {} } }\n",
    ),
    (
        "B.cs",
        "namespace App { public class Foo { public void FromB() {} } }\n",
    ),
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
    (
        "S.cs",
        r#"public class Runner {
    private readonly Worker _worker;
}
public class Worker { }
"#,
    ),
    (
        "S.cs",
        r#"public interface IRun { int Next { get; } }
public class Run : IRun {
    int Next() { return Other(); }
    int IRun.Next => Next();
    int Other() { return 0; }
}
"#,
    ),
    (
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
    ),
    (
        "S.cs",
        r#"public class Worker { }
public class Caller {
    public void Go() { Worker w = new Worker(); }
}
"#,
    ),
    (
        "S.cs",
        r#"public class Worker { }
public class Caller {
    public void Go() { var w = new Worker(); }
}
"#,
    ),
    (
        "S.cs",
        r#"public class OrderPlaced { }
public class Publisher {
    public void Publish() { Send(new OrderPlaced()); }
    private void Send(object payload) { }
}
"#,
    ),
    (
        "S.cs",
        r#"public class OrderPlaced { public long Id { get; set; } }
public class Publisher {
    public void Publish(long id) { Send(new OrderPlaced { Id = id }); }
    private void Send(object payload) { }
}
"#,
    ),
    (
        "S.cs",
        r#"public class Cache<T> { }
public class Caller {
    public void Go() { var c = new Cache<string>(); }
}
"#,
    ),
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
    (
        "S.cs",
        r#"public class Worker { }
public class Caller {
    public Worker Build() { Worker w = new(); return w; }
}
"#,
    ),
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
    (
        "Caller.cs",
        r#"using Infra;
public class Caller {
    public void Go() { var c = new Data.Cache(); }
}
"#,
    ),
    ("One.cs", "namespace Infra.Data;\npublic class Cache { }\n"),
    ("Two.cs", "namespace Infra.Data;\npublic class Cache { }\n"),
    (
        "S.cs",
        r#"public class Server { public bool Save() => true; }
public class Cache  { public bool Save() => false; }
public class Repo {
    private Server _server = new Server();
    public bool Commit() { return _server.Save(); }
}
"#,
    ),
    (
        "Caller.cs",
        r#"public class Caller {
    public void Go(Infra.Data.Cache existing) { var c = new Infra.Data.Cache(); }
}
"#,
    ),
    (
        "Caller.cs",
        r#"public class Caller {
    public void Go() { var b = new System.Text.StringBuilder(); }
}
"#,
    ),
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
    (
        "Box.cs",
        concat!(
            "namespace Demo { public class Box {",
            " static Bounds Bounds(int a) { return new Bounds(a); } } }\n"
        ),
    ),
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
    // upstream_csharp_extension.rs
    (
        "S.cs",
        concat!(
            "public interface IR { void Build(); }\n",
            "public class A : IR { public void Other() { } }\n",
        ),
    ),
    (
        "S.cs",
        concat!(
            "public interface IR { void M(); }\n",
            "public class A : IR { public void M() { } public void M(int x) { } }\n",
        ),
    ),
    (
        "S.cs",
        concat!(
            "public abstract class Base { public abstract void M(); }\n",
            "public class Impl : Base { public override void M() { } }\n",
        ),
    ),
    (
        "S.cs",
        concat!(
            "public interface IBase { void M(); }\n",
            "public interface IChild : IBase { }\n",
            "public class Impl : IChild { public void M() { } }\n",
        ),
    ),
    (
        "S.cs",
        concat!(
            "public interface IR { void Build(); }\n",
            "public class A : IR { public void build() { } }\n",
        ),
    ),
    ("IReport.cs", "public interface IReport { void Build(); }\n"),
    (
        "Report.cs",
        "public class Report : IReport { public void Build() { } }\n",
    ),
    // csharp_calls.rs
    (
        "Lib.cs",
        "namespace One { public class Gen<T> { public void Run(){} } public class Svc { public void Run(){} } }\n",
    ),
    // csharp_dispatch.rs
    ("IReport.cs", "interface IReport {\n  string Build();\n}\n"),
    (
        "Report.cs",
        "class Report {\n  public string Build() { return \"x\"; }\n}\n",
    ),
    (
        "Audit.cs",
        "class Audit {\n  public string Build() { return \"y\"; }\n}\n",
    ),
    // references.rs
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
    // gaps.rs
    (
        "a.cs",
        "class S { public void Save() {} }\nclass C { void Run(S st) { st?.Save(); } }\n",
    ),
    // inherits.rs
    (
        "lib.cs",
        "interface IThing { }\ninterface IOther { }\nclass Base { }\n",
    ),
    (
        "impl.cs",
        "class Impl : Base, IThing, IOther { }\ninterface ISub : IThing { }\n",
    ),
];

/// smoke; the real checks are the ihc.18-ignored tests.
///
/// `extract_csharp` on every C# source the C# tests use: the testdata files, the tests' named
/// sources and [`INLINE_SOURCES`], deduped. Each must parse within the budget.
#[test]
fn smoke_extract_csharp_sources() {
    use super::csharp_calls_tests::CASES;
    use super::upstream_csharp_calls_tests as calls;
    use super::upstream_csharp_extension_tests::{FIXTURES, INJECTED};
    use super::upstream_csharp_members_tests as members;
    use super::upstream_csharp_types_tests::FIELD_TYPES;

    kinds::register_kinds();
    let root = testdata().join("extract").join("csharpproj");
    let mut on_disk = Vec::new();
    for f in ["util/math.cs", "web/server.cs"] {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("ReadFile({f}): {e}"));
        on_disk.push((f, String::from_utf8(src).expect("testdata is UTF-8")));
    }
    let mut all: Vec<(&str, &str)> = on_disk.iter().map(|(f, s)| (*f, s.as_str())).collect();
    all.extend(CASES.iter().flat_map(|c| c.files.iter().copied()));
    all.extend(FIXTURES.iter().flat_map(|f| f.files.iter().copied()));
    all.extend(INJECTED.iter().copied());
    all.extend([
        ("S.cs", calls::AMBIG),
        ("A.cs", calls::NS_A),
        ("B.cs", calls::NS_B),
        ("S.cs", calls::PRIMARY_CTOR),
        ("Model.cs", calls::CHAIN_MODEL),
        ("Config.cs", calls::CONFIG),
        ("Config.cs", calls::CONFIGS),
        ("Lib.cs", calls::LIB),
        ("Model.cs", calls::FOREACH_MODEL),
        ("T.cs", calls::GA_TYPES),
        ("T.cs", FIELD_TYPES),
        ("FooPartA.cs", members::FOO_PART_A),
        ("FooPartB.cs", members::FOO_PART_B),
    ]);
    all.extend(members::COLLIDING_CACHES);
    all.extend(INLINE_SOURCES.iter().copied());

    let mut seen = BTreeSet::new();
    for (rel, src) in all {
        // Go's dispatch: the lowercased extension.
        if !rel.to_ascii_lowercase().ends_with(".cs") || !seen.insert((rel, src)) {
            continue;
        }
        assert!(
            extract_csharp(rel, src.as_bytes()).is_some(),
            "extract_csharp({rel:?}) hit the parse budget; source:\n{src}"
        );
    }
}
