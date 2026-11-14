//! graphify-go `internal/extract/csharp_calls_test.go`.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file, kinds, resolve};
use crate::testutil::TempDir;

/// Go `csCallEdges`: resolves the C# files and returns each calls edge as
/// "src[file] -> tgt[file] CONF score", the first per pair as the graph keeps.
pub(super) fn cs_call_edges(files: &[(&str, &str)]) -> Vec<String> {
    kinds::register_kinds();
    let root = TempDir::new();
    let mut rels: Vec<&str> = Vec::new();
    for &(rel, src) in files {
        root.write(rel, src.as_bytes());
        rels.push(rel);
    }
    rels.sort();
    let results: Vec<_> = rels
        .iter()
        .map(|rel| file(root.path(), rel).unwrap_or_else(|e| panic!("File({rel}): {e}")))
        .collect();
    let rels: Vec<String> = rels.iter().map(|r| r.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &rels, workers);
    let label: BTreeMap<&str, String> = ext
        .nodes
        .iter()
        .map(|n| {
            let base = n.source_file.rsplit('/').next().unwrap_or("");
            (n.id.as_str(), format!("{}[{}]", n.label, base))
        })
        .collect();
    let name = |id: &str| label.get(id).cloned().unwrap_or_default();
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut out = Vec::new();
    for e in &ext.edges {
        if e.relation.as_str() != "calls" || !seen.insert((e.source.as_str(), e.target.as_str())) {
            continue;
        }
        let mut score = e.confidence_score;
        if score == 0.0 {
            score = match e.confidence.as_str() {
                "EXTRACTED" => 1.0,
                "INFERRED" => 0.5,
                _ => 0.0,
            };
        }
        out.push(format!(
            "{} -> {} {} {}",
            name(&e.source),
            name(&e.target),
            e.confidence.as_str(),
            score
        ));
    }
    out.sort();
    out
}

/// One `TestCSharpTypedCalls` case: its files and the calls edges it wants.
struct Case {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    want: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        name: "builtins",
        files: &[
            (
                "Model.cs",
                r#"namespace App { public class Color { public void Paint(){} } public class Request { } public class Widget { } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace App { public class Use { void Make(){ var c = new Color(); c.Paint(); new Request(); new Widget(); } } }
"#,
            ),
        ],
        want: &[
            "Use.Make()[Use.cs] -> Color.Paint()[Model.cs] INFERRED 0.85",
            "Use.Make()[Use.cs] -> Widget[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "chains",
        files: &[
            (
                "Model.cs",
                r#"namespace App.Core {
  public class Look { public void Animate(){} public Look Deep; }
  public class Agent { public Look look; public Look Look2 { get; set; } public void Step(){} public Agent Next(){ return null; } }
  public class Holder { public static Agent Shared; }
  public class Util { public static void Help(){} }
}
"#,
            ),
            (
                "Use.cs",
                r#"namespace App.Core {
  public class Use {
    Agent a;
    void One(Agent p){ p.look.Animate(); p.Look2.Animate(); }
    void Deep(Agent p){ p.look.Deep.Animate(); }
    void ThisDeep(){ this.a.look.Animate(); }
    void Static(){ Holder.Shared.Step(); }
    void Qualified(){ App.Core.Util.Help(); }
    void CallResult(Agent p){ p.Next().Step(); }
    void FieldChain(){ a.look.Animate(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Use.CallResult()[Use.cs] -> Agent.Next()[Model.cs] INFERRED 0.85",
            "Use.FieldChain()[Use.cs] -> Look.Animate()[Model.cs] INFERRED 0.85",
            "Use.One()[Use.cs] -> Look.Animate()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "condaccess-new",
        files: &[
            (
                "Model.cs",
                r#"namespace App { public class Agent { public void Step(){} public Agent Next(){ return null; } } public class Decoy { public void Step(){} } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace App {
  public class Use {
    Agent field;
    void Cond(Agent c){ c?.Step(); }
    void CondField(){ field?.Step(); }
    void NewRecv(){ new Agent().Step(); }
    void NewChain(){ new Agent().Next().Step(); }
    void CondUntyped(object o){ o?.ToString(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Use.Cond()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.CondField()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.NewChain()[Use.cs] -> Agent.Next()[Model.cs] INFERRED 0.85",
            "Use.NewChain()[Use.cs] -> Agent[Model.cs] INFERRED 0.85",
            "Use.NewRecv()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.NewRecv()[Use.cs] -> Agent[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "dedup",
        files: &[
            (
                "K.cs",
                r#"namespace N { public class Piece { public void Use(){} } }
"#,
            ),
            (
                "M.cs",
                r#"namespace N {
  public class Local { public static Local Create(){ return new Local(); } }
  public class M {
    void WithParam(Piece p){ new Piece(); }
    Piece WithReturn(){ return new Piece(); }
    void NoParam(){ new Piece(); }
    void Same(Local l){ new Local(); }
    void Twice(Piece p){ p.Use(); p.Use(); }
    public M Clone(){ return new M(); }
    void Order(){ Helper(); this.Helper(); }
    void Helper(){}
  }
}
"#,
            ),
        ],
        want: &[
            "M.NoParam()[M.cs] -> Piece[K.cs] INFERRED 0.85",
            // In-file bare calls stay INFERRED in Go.
            "M.Order()[M.cs] -> M.Helper()[M.cs] INFERRED 0.85",
            "M.Same()[M.cs] -> Local[M.cs] EXTRACTED 1",
            "M.Twice()[M.cs] -> Piece.Use()[K.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "foreach",
        files: &[
            (
                "Crowd.cs",
                r#"using System.Collections.Generic;
namespace App {
  public class Crowd {
    List<Agent> list;
    Agent[] arr;
    IEnumerable<Agent> seq;
    Dictionary<Agent, Decoy> dict;
    HashSet<Agent> set;
    List<List<Agent>> nested;
    void FieldList(){ foreach (var a in list) { a.Step(); } }
    void FieldArr(){ foreach (var a in arr) { a.Step(); } }
    void FieldSeq(){ foreach (var a in seq) { a.Step(); } }
    void DictKeys(){ foreach (var k in dict.Keys) { k.Step(); } }
    void DictValues(){ foreach (var v in this.dict.Values) { v.Step(); } }
    void ThisList(){ foreach (var a in this.list) { a.Step(); } }
    void SetLoop(){ foreach (var a in set) { a.Step(); } }
    void Nested(){ foreach (var a in nested) { a.Step(); } }
    void ParamList(List<Agent> xs){ foreach (var a in xs) { a.Step(); } }
    void LocalList(){ var ys = new List<Agent>(); foreach (var a in ys) { a.Step(); } }
    void Explicit(object[] objs){ foreach (Decoy d in objs) { d.Step(); } }
    void ShadowedField(List<Decoy> list){ foreach (var a in list) { a.Step(); } }
  }
}
"#,
            ),
            (
                "Model.cs",
                r#"namespace App { public class Agent { public void Step(){} } public class Decoy { public void Step(){} } }
"#,
            ),
        ],
        want: &[
            "Crowd.DictKeys()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.DictValues()[Crowd.cs] -> Decoy.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Explicit()[Crowd.cs] -> Decoy.Step()[Model.cs] INFERRED 0.85",
            "Crowd.FieldArr()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.FieldList()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.FieldSeq()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.SetLoop()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.ThisList()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "generics",
        files: &[
            (
                "Model.cs",
                r#"namespace App { public class Agent { public void Step(){} } public class Box<T> { public void Open(){} } public class Gen<T> { public void GM(){} } public class Gen { public void GM0(){} } }
"#,
            ),
            (
                "Use.cs",
                r#"using System.Collections.Generic;
namespace App {
  public class Use<T> {
    List<Agent> list;
    Agent[] arr;
    IReadOnlyList<Agent> ro;
    List<T> tl;
    void Index(List<Agent> lp, Agent[] ap, IList<Agent> il){ lp[0].Step(); ap[1].Step(); il[2].Step(); list[0].Step(); arr[1].Step(); ro[0].Step(); }
    void WholeList(List<Agent> lp){ lp.Step(); }
    void ArrWhole(Agent[] ap){ ap.Step(); }
    void BoxRecv(Box<Agent> b){ b.Open(); }
    void GenRecv(Gen<int> g){ g.GM(); g.GM0(); }
    void TypeParam(){ tl[0].Step(); }
    void Nullable(Agent? n){ n.Step(); }
    void GenericCall(Agent a){ a.Step<int>(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Use.ArrWhole()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.BoxRecv()[Use.cs] -> Box.Open()[Model.cs] INFERRED 0.85",
            // Same line: upstream keeps Gen, id gen before gen_1.
            "Use.GenRecv()[Use.cs] -> Gen.GM0()[Model.cs] INFERRED 0.85",
            "Use.GenericCall()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.Index()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.Nullable()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "iface",
        files: &[
            (
                "Model.cs",
                r#"namespace App {
  public interface IStore { void Save(); }
  public class Store : IStore { public void Save(){} }
  public interface IRoot { void RootM(); }
  public interface IChild : IRoot { void ChildM(); }
  public interface IDef { void Dm() {} }
  public class DefImpl : IDef { }
}
"#,
            ),
            (
                "Use.cs",
                r#"namespace App {
  public class Use {
    IStore store;
    void Field(){ store.Save(); }
    void Param(IStore s){ s.Save(); }
    void Chain(IChild c){ c.RootM(); c.ChildM(); }
    void Default(DefImpl d){ d.Dm(); }
    void Cast(object o){ ((IStore)o).Save(); (o as Store).Save(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Use.Cast()[Use.cs] -> IStore.Save()[Model.cs] EXTRACTED 1",
            "Use.Cast()[Use.cs] -> Store.Save()[Model.cs] EXTRACTED 1",
            "Use.Chain()[Use.cs] -> IChild.ChildM()[Model.cs] INFERRED 0.85",
            "Use.Chain()[Use.cs] -> IRoot.RootM()[Model.cs] INFERRED 0.85",
            "Use.Field()[Use.cs] -> IStore.Save()[Model.cs] INFERRED 0.85",
            "Use.Param()[Use.cs] -> IStore.Save()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "inherit",
        files: &[
            (
                "Model.cs",
                r#"namespace App {
  public class BaseStore { public int Get(){ return 0; } public virtual void Ov(){} public Agent baseAgent; }
  public class Mid : BaseStore { public override void Ov(){} }
  public class Widget : Mid { public void Probe(){ baseAgent.Step(); } }
  public class Ext : System.Collections.ObjectModel.Collection<int> { public void Own(){} }
  public class ExtChild : Ext { }
  public class OutBase : ExternalThing { public void Mine(){} }
  public class Agent { public void Step(){} }
}
"#,
            ),
            (
                "Use.cs",
                r#"namespace App {
  public class Use : Widget {
    void Inh(Widget w){ w.Get(); w.Ov(); }
    void ExtCall(ExtChild e){ e.Own(); e.Add(1); }
    void OutCall(OutBase ob){ ob.Mine(); ob.Theirs(); }
    void Inherited(){ baseAgent.Step(); base.Get(); this.Ov(); }
  }
  public class Leaf : OutBase { void Up(){ base.Mine(); this.Mine(); } }
}
"#,
            ),
        ],
        want: &[
            "Leaf.Up()[Use.cs] -> OutBase.Mine()[Model.cs] EXTRACTED 1",
            "Use.ExtCall()[Use.cs] -> Ext.Own()[Model.cs] INFERRED 0.85",
            "Use.Inh()[Use.cs] -> BaseStore.Get()[Model.cs] INFERRED 0.85",
            "Use.Inh()[Use.cs] -> Mid.Ov()[Model.cs] INFERRED 0.85",
            "Use.Inherited()[Use.cs] -> BaseStore.Get()[Model.cs] EXTRACTED 1",
            "Use.Inherited()[Use.cs] -> Mid.Ov()[Model.cs] EXTRACTED 1",
            "Use.OutCall()[Use.cs] -> OutBase.Mine()[Model.cs] INFERRED 0.85",
            "Widget.Probe()[Model.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "locals",
        files: &[
            (
                "Model.cs",
                r#"namespace App { public class Agent { public void Step(){} } public class Decoy { public void Step(){} } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace App {
  public class Use {
    void Explicit(){ Agent q = Make(); q.Step(); }
    void OutTyped(){ if (Try(out Agent a)) { a.Step(); } }
    void OutVar(){ if (Try(out var a)) { a.Step(); } }
    void IsPat(object o){ if (o is Agent a) { a.Step(); } }
    void CasePat(object o){ switch (o) { case Decoy d: d.Step(); break; } }
    void UsingVar(){ using var u = new Agent(); u.Step(); }
    void UsingStmt(){ using (var u = new Agent()) { u.Step(); } }
    void LambdaTyped(){ System.Action<Agent> f = (Agent la) => la.Step(); }
    void LocalFn(){ void Inner(Decoy x){ x.Step(); } }
    void CastLocal(object o){ var q = (Agent)o; q.Step(); }
    void AsLocal(object o){ var q = o as Agent; q.Step(); }
    void Catch(){ try {} catch (AgentEx ce) { ce.Step(); } }
    void ForVar(){ for (Agent i = null; i != null; ) { i.Step(); } }
    Agent Make(){ return null; }
    bool Try(out Agent a){ a = null; return true; }
  }
  public class AgentEx : System.Exception { public void Step(){} }
}
"#,
            ),
        ],
        want: &[
            "Use.CasePat()[Use.cs] -> Decoy.Step()[Model.cs] INFERRED 0.85",
            "Use.Explicit()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            // In-file bare calls stay INFERRED in Go.
            "Use.Explicit()[Use.cs] -> Use.Make()[Use.cs] INFERRED 0.85",
            "Use.IsPat()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.LambdaTyped()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.LocalFn()[Use.cs] -> Decoy.Step()[Model.cs] INFERRED 0.85",
            "Use.OutTyped()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.OutTyped()[Use.cs] -> Use.Try()[Use.cs] INFERRED 0.85",
            "Use.OutVar()[Use.cs] -> Use.Try()[Use.cs] INFERRED 0.85",
            "Use.UsingStmt()[Use.cs] -> Agent[Model.cs] INFERRED 0.85",
            "Use.UsingVar()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Use.UsingVar()[Use.cs] -> Agent[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "nested-new",
        files: &[
            (
                "Outer.cs",
                r#"namespace N {
  public class Outer {
    public class Inner { public void InM(){} }
    void Build(){ var i = new Inner(); i.InM(); }
  }
  public class Setup { void Link(){ var q = new Outer.Inner(); q.InM(); } void Bare(){ new Slot(); } }
  public class Holder { public class Slot {} }
}
"#,
            ),
            (
                "other/Inner.cs",
                r#"namespace M { public class Inner { public void InM(){} } public class Slot {} }
"#,
            ),
        ],
        want: &[
            // Every target is a nested type, which has no Go node.
        ],
    },
    Case {
        name: "ns-using",
        files: &[
            (
                "Deep.cs",
                r#"namespace Two.Deep { public class Inner2 { void M(Svc s){ s.Run(); } void N(){ new Svc(); } } }
"#,
            ),
            (
                "One.cs",
                r#"namespace One { public class Svc { public void Run(){} } public class OnlyOne { public void Go(){} } }
"#,
            ),
            (
                "Two.cs",
                r#"namespace Two { public class Svc { public void Run(){} } }
"#,
            ),
            (
                "far/Both.cs",
                r#"using One; using Two;
namespace Five { public class W { void A(Svc s){ s.Run(); } void N(){ new Svc(); } } }
"#,
            ),
            (
                "far/NoRef.cs",
                r#"namespace Six { public class Z { void N(){ new Svc(); } } }
"#,
            ),
            (
                "far/NoUse.cs",
                r#"namespace Four { public class V { void A(Svc s){ s.Run(); } void N(){ new Svc(); } } }
"#,
            ),
            (
                "far/Use.cs",
                r#"using One;
namespace Three {
  public class U {
    void A(Svc s){ s.Run(); }
    void D(OnlyOne o){ o.Go(); }
    void N(){ var x = new Svc(); var y = new Two.Svc(); }
    void S(){ Svc.Run(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Inner2.M()[Deep.cs] -> Svc.Run()[Two.cs] INFERRED 0.85",
            "Inner2.N()[Deep.cs] -> Svc[Two.cs] INFERRED 0.85",
            "U.A()[Use.cs] -> Svc.Run()[One.cs] INFERRED 0.85",
            "U.D()[Use.cs] -> OnlyOne.Go()[One.cs] INFERRED 0.85",
            "U.N()[Use.cs] -> Svc[One.cs] INFERRED 0.85",
            "U.N()[Use.cs] -> Svc[Two.cs] EXTRACTED 1",
            "U.S()[Use.cs] -> Svc.Run()[One.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "recv-kinds",
        files: &[
            (
                "Lib.cs",
                r#"namespace App {
  public class Util { public static void Help(){} public void Inst(){} }
  public interface IStore { void Save(); }
  public class Store { public void Save(){} public void Load(){} }
  public class Engine { public void Run(){} public void Stop(){} }
  public class Base { public virtual void Hook(){} public void Shared(){} }
}
"#,
            ),
            (
                "Use.cs",
                r#"namespace App {
  public class Car : Base {
    Engine Engine;
    IStore Store => null;
    Engine motor;
    public void Self(){ this.Own(); Own(); }
    public void Own(){}
    public override void Hook(){ base.Hook(); base.Shared(); }
    public void Static(){ Util.Help(); Util.Inst(); }
    public void CapField(){ Engine.Run(); }
    public void CapProp(){ Store.Save(); Store.Load(); }
    public void LowField(){ motor.Stop(); this.motor.Run(); }
    public void ThisCap(){ this.Engine.Stop(); }
    public void Missing(){ Nowhere.Go(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Car.CapField()[Use.cs] -> Engine.Run()[Lib.cs] EXTRACTED 1",
            "Car.CapProp()[Use.cs] -> IStore.Save()[Lib.cs] EXTRACTED 1",
            "Car.Hook()[Use.cs] -> Base.Hook()[Lib.cs] EXTRACTED 1",
            "Car.Hook()[Use.cs] -> Base.Shared()[Lib.cs] EXTRACTED 1",
            "Car.LowField()[Use.cs] -> Engine.Run()[Lib.cs] INFERRED 0.85",
            "Car.LowField()[Use.cs] -> Engine.Stop()[Lib.cs] INFERRED 0.85",
            "Car.Self()[Use.cs] -> Car.Own()[Use.cs] EXTRACTED 1",
            "Car.Static()[Use.cs] -> Util.Help()[Lib.cs] EXTRACTED 1",
            "Car.Static()[Use.cs] -> Util.Inst()[Lib.cs] EXTRACTED 1",
            "Car.ThisCap()[Use.cs] -> Engine.Stop()[Lib.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "shadow",
        files: &[
            (
                "Crowd.cs",
                r#"namespace App {
  public class Crowd {
    Agent field;
    Agent other;
    void Shadow(){ field.Step(); var field = new Decoy(); }
    void SameType(){ other.Step(); var other = new Agent(); }
    void LambdaUntyped(){ field.Step(); System.Action<Agent> f = field => field.Step(); }
    void Sibling(bool b){ if (b) { var s = new Agent(); s.Step(); } else { var s = new Decoy(); s.Step(); } }
    void Inner(Agent p){ { Decoy p2 = null; p2.Step(); } p.Step(); }
    void Untyped(){ var u = Make(); u.Step(); }
    Agent Make(){ return null; }
  }
}
"#,
            ),
            (
                "Model.cs",
                r#"namespace App { public class Agent { public void Step(){} } public class Decoy { public void Step(){} } }
"#,
            ),
        ],
        want: &[
            "Crowd.Inner()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Inner()[Crowd.cs] -> Decoy.Step()[Model.cs] INFERRED 0.85",
            "Crowd.SameType()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.SameType()[Crowd.cs] -> Agent[Model.cs] INFERRED 0.85",
            "Crowd.Shadow()[Crowd.cs] -> Decoy[Model.cs] INFERRED 0.85",
            "Crowd.Sibling()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Sibling()[Crowd.cs] -> Agent[Model.cs] INFERRED 0.85",
            "Crowd.Sibling()[Crowd.cs] -> Decoy.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Sibling()[Crowd.cs] -> Decoy[Model.cs] INFERRED 0.85",
            // No type for var u = Make() (byx.46).
            // In-file bare calls stay INFERRED in Go.
            "Crowd.Untyped()[Crowd.cs] -> Crowd.Make()[Crowd.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "type-method-name",
        files: &[
            (
                "Kit.cs",
                r#"namespace N { public class Piece { public void Use(){} } }
"#,
            ),
            (
                "Same.cs",
                r#"namespace N {
  public class Gadget { }
  public class Maker {
    static Gadget Gadget(){ return null; }
    void Make(){ new Gadget(); }
  }
}
"#,
            ),
            (
                "Shop.cs",
                r#"namespace N {
  public class Shop {
    static int Piece(){ return 1; }
    void Buy(){ var p = new Piece(); Piece(); }
  }
}
"#,
            ),
        ],
        want: &[
            "Maker.Make()[Same.cs] -> Gadget[Same.cs] INFERRED 0.85",
            "Shop.Buy()[Shop.cs] -> Piece[Kit.cs] INFERRED 0.85",
            // In-file bare calls stay INFERRED in Go.
            "Shop.Buy()[Shop.cs] -> Shop.Piece()[Shop.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "v57-bead",
        files: &[
            (
                "Crowd.cs",
                r#"using System.Collections.Generic;
namespace App {
  class Crowd {
    Agent field;
    Dictionary<string, Agent> agents;
    void Param(Agent p){ p.Step(); }
    void Local(){ var q = new Agent(); q.Step(); }
    void Field(){ field.Step(); }
    void Loop(List<Agent> xs){ foreach (var a in xs) { a.Step(); } }
    void Tick(){ foreach (var a in agents.Values) { a.look.Animate(); } }
    void Inh(Widget other){ other.Get(); }
  }
}
"#,
            ),
            (
                "Model.cs",
                r#"namespace App {
  class Look { public void Animate(){} }
  class Agent { public Look look; public void Step(){} }
  class Decoy { public void Step(){} public void Animate(){} }
  class BaseStore { public int Get(){ return 0; } }
  class Mid : BaseStore {}
  class Widget : Mid {}
}
"#,
            ),
        ],
        want: &[
            "Crowd.Field()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Inh()[Crowd.cs] -> BaseStore.Get()[Model.cs] INFERRED 0.85",
            "Crowd.Local()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Local()[Crowd.cs] -> Agent[Model.cs] INFERRED 0.85",
            "Crowd.Param()[Crowd.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Crowd.Tick()[Crowd.cs] -> Look.Animate()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "v58-bead",
        files: &[
            (
                "Controller.cs",
                r#"namespace App { class Controller { void Tick(Run run){ run.Sequence.Apply(); } void Tock(Run run){ run.Seq.Apply(); } } }
"#,
            ),
            (
                "Model.cs",
                r#"namespace App { class Sequence { public void Apply(){} } class Decoy { public void Apply(){} } class Run { public Decoy Sequence; public Sequence Seq; } }
"#,
            ),
        ],
        want: &[
            "Controller.Tick()[Controller.cs] -> Decoy.Apply()[Model.cs] INFERRED 0.85",
            "Controller.Tock()[Controller.cs] -> Sequence.Apply()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "v59-bead",
        files: &[
            (
                "Kit.cs",
                r#"namespace N { class Piece {} }
"#,
            ),
            (
                "Setup.cs",
                r#"namespace N { class Outer { public class Inner {} } class Setup { void Link() { var p = new Piece(); var q = new Outer.Inner(); var c = new Cache<string>(); } } class Cache<T> {} }
"#,
            ),
            (
                "Shop.cs",
                r#"namespace N { class Shop { static int Piece() { return 1; } } }
"#,
            ),
        ],
        want: &[
            "Setup.Link()[Setup.cs] -> Cache[Setup.cs] EXTRACTED 1",
            // Inner is nested: no Go node.
            "Setup.Link()[Setup.cs] -> Piece[Kit.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "primary-ctor",
        files: &[
            (
                "Model.cs",
                r#"namespace App { public class Agent { public void Step(){} } public interface IGlob { void Find(); } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace App {
  public class Validator(IGlob config, int n) {
    void Run(){ config.Find(); }
    void Local(){ void Inner(){ config.Find(); } }
  }
  public record Rec(Agent a) { void Go(){ a.Step(); } }
  public class Shadowed(Agent a) { Decoy a2; void Go(){ a.Step(); } }
  public class Decoy { }
}
"#,
            ),
        ],
        want: &[
            "Rec.Go()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Shadowed.Go()[Use.cs] -> Agent.Step()[Model.cs] INFERRED 0.85",
            "Validator.Local()[Use.cs] -> IGlob.Find()[Model.cs] INFERRED 0.85",
            "Validator.Run()[Use.cs] -> IGlob.Find()[Model.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "nested-and-top-level",
        files: &[(
            "Use.cs",
            r#"namespace App;
class Destination { public void Fill(){} }
public class Outer
{
    struct Destination { }
    void Make(){ var d = new Destination(); d.Fill(); }
}
public class Other
{
    void Make(){ var d = new Destination(); d.Fill(); }
}
"#,
        )],
        want: &[
            // Outer.Make() names the nested struct; upstream gives it the
            // top-level type's node.
            "Other.Make()[Use.cs] -> Destination.Fill()[Use.cs] INFERRED 0.85",
            "Other.Make()[Use.cs] -> Destination[Use.cs] EXTRACTED 1",
            "Outer.Make()[Use.cs] -> Destination.Fill()[Use.cs] INFERRED 0.85",
            "Outer.Make()[Use.cs] -> Destination[Use.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "file-scoped-namespace",
        files: &[
            (
                "Lib.cs",
                r#"namespace App.Internal;
public class Req { public void Go(){} }
"#,
            ),
            (
                "Use.cs",
                r#"namespace App;
public class Use { Req r; void A(){ new Req(); } void B(){ r.Go(); } }
"#,
            ),
        ],
        want: &["Use.B()[Use.cs] -> Req.Go()[Lib.cs] INFERRED 0.85"],
    },
    Case {
        name: "call-type-args",
        files: &[
            (
                "Lib.cs",
                r#"public class Src{} public class Dst{}
public interface IM{ T Map<S,T>(S s); T Map<T>(object s); }
"#,
            ),
            (
                "Use.cs",
                r#"public class U {
  void A(IM m){ m.Map<Src, Dst>(new Src()); }
  void B(IM m){ m.Map<Dst>(new Src()); }
}
"#,
            ),
        ],
        want: &[
            // Map<Src, Dst> references Src.
            "U.A()[Use.cs] -> IM.Map()[Lib.cs] INFERRED 0.85",
            "U.B()[Use.cs] -> IM.Map()[Lib.cs] INFERRED 0.85",
            "U.B()[Use.cs] -> Src[Lib.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "arity-twins",
        files: &[
            (
                "TMC.cs",
                r#"namespace A {
public class TypeMapConfiguration { public void Configure(int x){} }
public abstract class MappingExpressionBase<TS,TD> : TypeMapConfiguration { }
}
"#,
            ),
            (
                "ME.cs",
                r#"namespace A {
public sealed class MappingExpression : MappingExpressionBase<object,object> { }
public class MappingExpression<TS,TD> : MappingExpressionBase<TS,TD> { }
}
"#,
            ),
            (
                "Use.cs",
                r#"namespace Other;
public class P { public void Run(){ MappingExpression expression = new(1); expression.Configure(2); } }
public class Q { public void Make(){ new MappingExpression(1); } }
"#,
            ),
        ],
        want: &[
            // Two upstream nodes: a tie.
        ],
    },
    Case {
        name: "generic-stub",
        files: &[
            (
                "P.cs",
                r#"public interface ITC<A,B>{ B Convert(A s); }
public class Desc<T>{ public int V; }
public class Conv<T> : ITC<T, Desc<T>> { public Desc<T> Convert(T s){ return new Desc<T>{V=1}; } }
public class Conv2 { public void M(){ new Desc<int>(); } }
"#,
            ),
            (
                "Q.cs",
                r#"public class Box<T>{ }
public class Sub : Box<int> { }
public class U { public void M(){ new Box<int>(); } }
"#,
            ),
        ],
        want: &[
            // Later Desc<T> refs hide Desc.
            "Conv2.M()[P.cs] -> Desc[P.cs] INFERRED 0.85",
            "U.M()[Q.cs] -> Box[Q.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "alias",
        files: &[
            (
                "One.cs",
                r#"namespace One { public class Svc { public void Run() { } } }
"#,
            ),
            (
                "Two.cs",
                r#"namespace Two { public class Svc { public void Run() { } } }
"#,
            ),
            (
                "Use.cs",
                r#"using One;
using Svc = Two.Svc;
using Foo = Two.Svc;
namespace App {
    public class User {
        public void A(Svc s) { s.Run(); }
        public void B() { Svc.Run(); }
        public void C(Foo f) { f.Run(); }
        public void D() { new Svc(); }
    }
}
"#,
            ),
        ],
        want: &[
            "User.A()[Use.cs] -> Svc.Run()[Two.cs] INFERRED 0.85",
            "User.B()[Use.cs] -> Svc.Run()[Two.cs] EXTRACTED 1",
            "User.C()[Use.cs] -> Svc.Run()[Two.cs] INFERRED 0.85",
            "User.D()[Use.cs] -> Svc[Two.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "scoped-usings",
        files: &[
            (
                "One.cs",
                r#"namespace One { public class Svc { public void Run() { } } }
"#,
            ),
            (
                "Two.cs",
                r#"namespace Two { public class Svc { public void Run() { } } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace A { using One; public class UA { public void M(Svc s) { s.Run(); } } }
namespace B { public class UB { public void M(Svc s) { s.Run(); } } }
namespace Two.Deep { namespace Er2 { public class UC { public void M(Svc s) { s.Run(); } } } }
"#,
            ),
        ],
        want: &[
            "UA.M()[Use.cs] -> Svc.Run()[One.cs] INFERRED 0.85",
            "UC.M()[Use.cs] -> Svc.Run()[Two.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "stub-ref-new",
        files: &[
            (
                "P1.cs",
                r#"namespace P1 { public class Piece { } }
"#,
            ),
            (
                "P2.cs",
                r#"namespace P2 { public class Piece { } }
"#,
            ),
            (
                "Use.cs",
                r#"using P1;
namespace App { public class Shop { public void A() { new Piece(); } public void B(Piece p) { new Piece(); } } }
"#,
            ),
        ],
        want: &[
            // B's references edge holds the pair.
            "Shop.A()[Use.cs] -> Piece[P1.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "nested-member-type",
        files: &[(
            "A.cs",
            r#"namespace N {
    public class Store { public void Save() { } }
    public class Host {
        public class Inner { public void Save() { } }
        public Inner Store { get; set; }
        public void M() { Store.Save(); }
        public void P(Inner Cache) { Cache.Save(); }
    }
    public class Cache { public void Save() { } }
}
"#,
        )],
        want: &[
            // Upstream binds nested Inner.Save.
        ],
    },
    Case {
        name: "nested-scope",
        files: &[
            (
                "A.cs",
                r#"namespace N {
    public class Outer { class Cfg { } }
    public class Other { public void M2(Cfg c) { c.Go(); } }
}
"#,
            ),
            (
                "Helper.cs",
                r#"namespace N { public class Cfg { public void Go() { } } }
"#,
            ),
            (
                "X.cs",
                r#"namespace X { public class Outer2 { public class Inner { public void InM(){} } } }
"#,
            ),
            (
                "Z.cs",
                r#"namespace Z { public class Inner { public void InM(){} } }
namespace Y { public class U { void M(Inner i){ i.InM(); } } }
"#,
            ),
        ],
        want: &[
            // Nested Inner ties the fallback.
            "Other.M2()[A.cs] -> Cfg.Go()[Helper.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "qualified-new-namespace",
        files: &[
            (
                "Models.cs",
                r#"namespace App.Models { public class Order { } public class Item { } }
"#,
            ),
            (
                "Other.cs",
                r#"namespace App { public class Models { } }
namespace Lib { public class Item { } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace Client { public class User {
    public void A() { new App.Models.Order(); }
    public void B() { new App.Models.Item(); }
} }
"#,
            ),
        ],
        want: &[
            "User.A()[Use.cs] -> Order[Models.cs] INFERRED 0.85",
            "User.B()[Use.cs] -> Item[Models.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "own-method-new",
        files: &[(
            "A.cs",
            r#"namespace N {
    public class Shop { static int Piece() { return 0; } public void M() { new Piece(); } }
    public class Other { public void O() { new Piece(); } }
    public class Piece { }
}
"#,
        )],
        want: &[
            "Other.O()[A.cs] -> Piece[A.cs] EXTRACTED 1",
            "Shop.M()[A.cs] -> Piece[A.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "spaced-names",
        files: &[(
            "A.cs",
            "using Microsoft.Extensions\n    .DependencyInjection;\nusing System.Text /* a  b */;\nnamespace A.\n    B { public class Svc { public void Run(){} } public class U { void M(Svc s){ s.Run(); } } }\n",
        )],
        want: &["U.M()[A.cs] -> Svc.Run()[A.cs] INFERRED 0.85"],
    },
    Case {
        name: "call-type-arg-no-stub",
        files: &[
            (
                "Lib.cs",
                r#"namespace X { public class Src { public void Go(){} } public interface IM { T Map<T>(object s); } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace Y { public class User { public void A(X.IM m){ m.Map<Src>(1); } public void B(){ new Src(); } } }
"#,
            ),
            (
                "P.cs",
                r#"namespace Z { public interface IN { T Map<T>(object s); } public class Desc<T> { }
  public class Conv { public void Use(IN m){ m.Map<Desc<int>>(1); } public object Make(){ return new Desc<int>(); } } }
"#,
            ),
        ],
        want: &[
            "Conv.Make()[P.cs] -> Desc[P.cs] EXTRACTED 1",
            "Conv.Use()[P.cs] -> IN.Map()[P.cs] INFERRED 0.85",
            "User.A()[Use.cs] -> IM.Map()[Lib.cs] INFERRED 0.85",
            "User.B()[Use.cs] -> Src[Lib.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "upper-ext-with-lower",
        files: &[
            (
                "A.CS",
                r#"namespace App { public class Svc { public void Run(){} public void Me(){ this.Run(); } } }
"#,
            ),
            (
                "B.CS",
                r#"namespace App { public class Use { void Go(Svc s){ s.Run(); } } }
"#,
            ),
            (
                "C.cs",
                r#"namespace App { public class Lower { void L(){ new Svc(); } } }
"#,
            ),
        ],
        want: &[
            "Lower.L()[C.cs] -> Svc[A.CS] INFERRED 0.85",
            "Svc.Me()[A.CS] -> Svc.Run()[A.CS] EXTRACTED 1",
            "Use.Go()[B.CS] -> Svc.Run()[A.CS] INFERRED 0.85",
        ],
    },
    Case {
        name: "upper-ext-only",
        files: &[
            (
                "A.CS",
                r#"namespace App { public class Svc { public void Run(){} } }
"#,
            ),
            (
                "B.CS",
                r#"namespace App { public class Use { void Go(Svc s){ s.Run(); new Svc(); } void K(){ new T(); new Svc(); } } public class T {} }
"#,
            ),
        ],
        // Member calls need a .cs file.
        want: &["Use.K()[B.CS] -> T[B.CS] EXTRACTED 1"],
    },
    Case {
        name: "qualified-new-namespace-past-nested",
        files: &[
            (
                "M.cs",
                r#"namespace Models { public class Inner { public void Go(){} } }
"#,
            ),
            (
                "O.cs",
                r#"namespace Other { public class Models { public class Inner { } } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace App { public class User { public void B(){ new Models.Inner(); } } }
"#,
            ),
        ],
        // Upstream also binds O.cs's nested Inner, which has no node.
        want: &["User.B()[Use.cs] -> Inner[M.cs] EXTRACTED 1"],
    },
    Case {
        name: "new-test-filter",
        files: &[
            (
                "A/UseTests.cs",
                r#"namespace P { class Use { void Run(){ new Piece(); } } }
"#,
            ),
            (
                "A/K.cs",
                r#"namespace P { class Piece { } }
"#,
            ),
            (
                "B/KTests.cs",
                r#"namespace R { class Piece { } }
"#,
            ),
        ],
        want: &["Use.Run()[UseTests.cs] -> Piece[KTests.cs] INFERRED 0.85"],
    },
    Case {
        name: "new-common-dir",
        files: &[
            (
                "A/x/Use.cs",
                r#"namespace P { class Use { void Run(){ new Piece(); } } }
"#,
            ),
            (
                "A/y/K.cs",
                r#"namespace Q { class Piece { } }
"#,
            ),
            (
                "B/K.cs",
                r#"namespace R { class Piece { } }
"#,
            ),
        ],
        want: &["Use.Run()[Use.cs] -> Piece[K.cs] INFERRED 0.85"],
    },
    Case {
        name: "new-nested-candidate",
        files: &[
            (
                "A/UseTests.cs",
                r#"namespace P { class Use { void Run(){ new Piece(); } } }
"#,
            ),
            (
                "A/K.cs",
                r#"namespace P { class Piece { } }
"#,
            ),
            (
                "B/KTests.cs",
                r#"namespace R { class Holder { class Piece { } } }
"#,
            ),
        ],
        // Upstream binds the nested Piece, which has no node.
        want: &[],
    },
    Case {
        name: "own-nested-top-level-twin",
        files: &[
            (
                "Lib.cs",
                r#"namespace N { public class Inner { public void Go(){} } }
"#,
            ),
            (
                "Use.cs",
                r#"namespace N { public class Host { class Inner { public void Go(){} } Inner f; Inner Prop { get; set; }
  void A(){ f.Go(); } void B(Inner p){ p.Go(); } void C(){ Prop.Go(); } void D(){ var x = new Inner(); x.Go(); } } }
"#,
            ),
        ],
        want: &[
            "Host.A()[Use.cs] -> Inner.Go()[Lib.cs] INFERRED 0.85",
            "Host.B()[Use.cs] -> Inner.Go()[Lib.cs] INFERRED 0.85",
            "Host.C()[Use.cs] -> Inner.Go()[Lib.cs] EXTRACTED 1",
            "Host.D()[Use.cs] -> Inner.Go()[Lib.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "alias-other-block",
        files: &[
            (
                "One.cs",
                r#"namespace One { public class Svc { public void Run(){} } }
"#,
            ),
            (
                "Two.cs",
                r#"namespace Two { public class Svc { public void Run(){} } }
"#,
            ),
            (
                "Use.cs",
                r#"using One;
namespace A { using Svc = Two.Svc; public class UA { public void M(Svc s){ s.Run(); } } }
namespace B { public class UB { public void M(Svc s){ s.Run(); } public void N(){ new Svc(); } } }
"#,
            ),
        ],
        want: &[
            "UA.M()[Use.cs] -> Svc.Run()[Two.cs] INFERRED 0.85",
            "UB.N()[Use.cs] -> Svc[Two.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "base-walk-all-branches",
        files: &[(
            "M.cs",
            r#"namespace D {
public interface IRoot { void Conv(); }
public interface IBase : IRoot { void Conv(); void Only(); }
public interface ISide : IRoot { }
public interface IMap : IBase, ISide { }
public class U { void Run(IMap m){ m.Conv(); m.Only(); } }
}
"#,
        )],
        want: &["U.Run()[M.cs] -> IBase.Only()[M.cs] INFERRED 0.85"],
    },
    Case {
        name: "twin-this",
        files: &[(
            "A.cs",
            r#"namespace N {
public class K1 { public void K1m() {} }
public class Foo<T> { public void G() { this.F(); this.H(); } public void H() {} }
public class Foo : K1 { public void F() { this.G(); this.K1m(); this.F2(); } public void F2() {} }
public class Sub : Foo { public void S() { base.F(); base.G(); this.F2(); } }
}
"#,
        )],
        // Each twin's this sees its own methods and bases.
        want: &[
            "Foo.F()[A.cs] -> Foo.F2()[A.cs] EXTRACTED 1",
            "Foo.F()[A.cs] -> K1.K1m()[A.cs] EXTRACTED 1",
            "Foo.G()[A.cs] -> Foo.H()[A.cs] EXTRACTED 1",
            "Sub.S()[A.cs] -> Foo.G()[A.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "twin-this-kept",
        files: &[(
            "A.cs",
            r#"namespace N {
public class Foo<T> : Foo { public void G() { this.F(); } }
public class Foo { public void F() {} }
public class U { void Run(Foo f, Foo<int> g) { f.F(); g.F(); } }
}
"#,
        )],
        want: &[],
    },
    Case {
        name: "twin-kept-bases",
        files: &[(
            "A.cs",
            r#"namespace N {
public class K1 { public void K1m() {} }
public class BaseG { public void Bm() {} }
public class Foo<T> : BaseG
{
    public void G() {}
}
public class Foo : K1
{
    public void F() {}
}
public class U { void Run(Foo f) { f.F(); f.G(); f.Bm(); f.K1m(); } }
}
"#,
        )],
        // Only the kept twin's bases join the walk.
        want: &[
            "U.Run()[A.cs] -> BaseG.Bm()[A.cs] INFERRED 0.85",
            "U.Run()[A.cs] -> Foo.G()[A.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "twin-no-fields",
        files: &[(
            "A.cs",
            r#"namespace N {
public class A1 { public void Step() {} }
public class A2 { public void Step() {} }
public class A3 { public void Step() {} }
public class Foo<T> { public A1 Ag; }
public class Foo { public A2 Bg; }
public class Box { public A3 Cg; }
public class U { void Run(Foo f, Box b) { f.Ag.Step(); f.Bg.Step(); b.Cg.Step(); } }
}
"#,
        )],
        // No field table for a label twice in a file.
        want: &["U.Run()[A.cs] -> A3.Step()[A.cs] INFERRED 0.85"],
    },
    Case {
        name: "twin-base-keyword",
        files: &[(
            "A.cs",
            r#"namespace N {
public class B1 { public void M() {} }
public class B2 { public void M() {} public void Q() {} }
public class Foo<T> : B1 { public void G() { base.M(); } }
public class Foo : B2 { public void F() { base.M(); this.Q(); } }
}
"#,
        )],
        want: &[
            "Foo.F()[A.cs] -> B2.M()[A.cs] EXTRACTED 1",
            "Foo.F()[A.cs] -> B2.Q()[A.cs] EXTRACTED 1",
            "Foo.G()[A.cs] -> B1.M()[A.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "twin-same-line",
        files: &[(
            "A.cs",
            r#"namespace N {
public class Foo<T> { public void G() {} } public class Foo { public void F() {} }
public class U { void Run(Foo f) { f.F(); f.G(); } }
}
"#,
        )],
        want: &["U.Run()[A.cs] -> Foo.F()[A.cs] INFERRED 0.85"],
    },
    Case {
        name: "self-named-open-base",
        files: &[(
            "A.cs",
            r#"namespace N {
public interface IHas { void Z(); }
public interface IFoo : Ext.IFoo, IHas { void G(); }
public class U { void Run(IFoo f) { f.Z(); f.G(); } }
}
"#,
        )],
        // Ext.IFoo is outside the corpus: it spoils the walk.
        want: &["U.Run()[A.cs] -> IFoo.G()[A.cs] INFERRED 0.85"],
    },
    Case {
        name: "twin-open-base",
        files: &[(
            "A.cs",
            r#"namespace N {
public interface IHas { void Z(); }
public interface IFoo<T> : IExt, IHas { void G(); }
public interface IFoo : IFoo<int> { void F(); }
public class U { void Run(IFoo f) { f.Z(); f.G(); } }
}
"#,
        )],
        want: &["U.Run()[A.cs] -> IFoo.G()[A.cs] INFERRED 0.85"],
    },
    Case {
        name: "upper-ext-scoping",
        files: &[
            (
                "A.CS",
                r#"namespace One { public class Svc { public void Run() {} } public class Base { public void BRun() {} } public class Kid : Base { } }
"#,
            ),
            (
                "B.cs",
                r#"namespace Two { public class Svc { public void Run() {} } }
"#,
            ),
            (
                "U.cs",
                r#"using One;
namespace U { public class UA { void M(Svc s, Kid k) { s.Run(); k.BRun(); new Svc(); } } }
"#,
            ),
        ],
        // Upstream's index, usings and bases skip .CS files.
        want: &[],
    },
    Case {
        name: "upper-ext-no-bases",
        files: &[
            (
                "A.CS",
                r#"namespace One { public class Svc { public void Run() {} } public class Base { public void BRun() {} } public class Kid : Base { } }
"#,
            ),
            (
                "B.cs",
                r#"namespace Two { public class Other { } }
"#,
            ),
            (
                "U.cs",
                r#"using One;
namespace U { public class UA { void M(Svc s, Kid k) { s.Run(); k.BRun(); new Svc(); } } }
"#,
            ),
        ],
        want: &["UA.M()[U.cs] -> Svc.Run()[A.CS] INFERRED 0.85"],
    },
    Case {
        name: "new-type-param-namesake",
        files: &[(
            "A.cs",
            r#"namespace N {
public class TItem { public void Run() {} public void Go() {} }
public class G<TItem> { TItem t; TItem P { get; } void M() { t.Run(); P.Go(); new TItem(); } }
public class H { void M<TItem>(TItem x) { x.Run(); TItem.Go(); new TItem(); } }
public class K<TItem> where TItem : new() { void M() { var y = new TItem(); y.Run(); } }
}
"#,
        )],
        // Upstream binds new T() to a namesake class.
        want: &[
            "G.M()[A.cs] -> TItem.Go()[A.cs] EXTRACTED 1",
            "G.M()[A.cs] -> TItem[A.cs] EXTRACTED 1",
            "H.M()[A.cs] -> TItem.Go()[A.cs] EXTRACTED 1",
            "H.M()[A.cs] -> TItem.Run()[A.cs] INFERRED 0.85",
            "H.M()[A.cs] -> TItem[A.cs] EXTRACTED 1",
            "K.M()[A.cs] -> TItem.Run()[A.cs] INFERRED 0.85",
            "K.M()[A.cs] -> TItem[A.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "new-type-param-other-file",
        files: &[
            (
                "A.cs",
                r#"namespace N {
public class G<TItem> where TItem : new() { void M() { new TItem(); } }
public class H { void M<TItem>() where TItem : new() { new TItem(); } }
}
"#,
            ),
            (
                "L.cs",
                r#"namespace N { public class TItem { public void Run() {} } }
"#,
            ),
        ],
        want: &[
            "G.M()[A.cs] -> TItem[L.cs] INFERRED 0.85",
            "H.M()[A.cs] -> TItem[L.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "spaced-type-names",
        files: &[
            (
                "Host.cs",
                r#"namespace App {
  public class Host {
    public One.Gen <int> F;
    public One.Gen <One.Svc> F2;
    private global::System.Collections.Generic.Dictionary<global::One.Dv,
        string> map;
    public One.Arr [] arr;
    public One.Pr <int> Prop { get; }
    public System.Collections.Generic.List<One.Lst > lst;
    public One.Ch <int> Chf;
    public void M(){ F.Run(); }
    public void M2(){ F2.Run(); }
    public void M3(){ arr[0].Run(); }
    public void M4(){ Prop.Run(); }
    public void M5(){ foreach (var v in lst) v.Run(); }
    public void M6(One . Svc p){ p.Run(); }
    public void M7(){ One.Cmt/*c d*/<int> v = null; v.Run(); }
    public void M8(){ One.
        Nl x = null; x.Run(); }
    public void M9(object o){ ((One.Cst <int>)o).Run(); }
    public void M10(){ new One.Nw <int>(); new One . Nw2(); new global::One.Nw3/*c*/(); }
    public void M11(){ map.Run(); }
  }
  public class Pcl(One.Pc /*x y*/ g) { void M(){ g.Run(); } }
  public class K : One.Bs <int> { void M(){ base.BRun(); this.BRun(); } }
}
"#,
            ),
            (
                "Lib.cs",
                r#"namespace One {
  public class Gen<T> { public void Run(){} }
  public class Svc { public void Run(){} }
  public class Arr { public void Run(){} }
  public class Dv { public void Run(){} }
  public class Cmt<T> { public void Run(){} }
  public class Nl { public void Run(){} }
  public class Pc { public void Run(){} }
  public class Pr<T> { public void Run(){} }
  public class Cst<T> { public void Run(){} }
  public class Nw<T> { public void Run(){} }
  public class Nw2 { public void Run(){} }
  public class Nw3 { public void Run(){} }
  public class Lst { public void Run(){} }
  public class Ch<T> { public void Run(){} }
  public class Bs<T> { public void BRun(){} }
}
"#,
            ),
            (
                "Use.cs",
                r#"using G2 = One.Gen <int>;
using One /* q r */ ;
namespace A /*x y*/ . B {
  public class U { void M(App.Host h, G2 g){ h.Chf.Run(); g.Run(); } }
}
"#,
            ),
        ],
        // Spaced or commented names bind as upstream reads them.
        want: &[
            "Host.M()[Host.cs] -> Gen.Run()[Lib.cs] EXTRACTED 1",
            "Host.M10()[Host.cs] -> Nw3[Lib.cs] INFERRED 0.85",
            "Host.M2()[Host.cs] -> Svc.Run()[Lib.cs] EXTRACTED 1",
            "Host.M3()[Host.cs] -> Arr.Run()[Lib.cs] INFERRED 0.85",
            "Host.M4()[Host.cs] -> Pr.Run()[Lib.cs] EXTRACTED 1",
            "Host.M9()[Host.cs] -> Cst.Run()[Lib.cs] EXTRACTED 1",
            "Pcl.M()[Host.cs] -> Pc.Run()[Lib.cs] INFERRED 0.85",
            "U.M()[Use.cs] -> Ch.Run()[Lib.cs] INFERRED 0.85",
            "U.M()[Use.cs] -> Gen.Run()[Lib.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "spaced-base-names",
        files: &[
            (
                "Host.cs",
                r#"using One;
namespace App {
  public class K : One.Bs <int> { void M(){ base.BRun(); } }
  public class K2 : One . Bs2 { void M(){ base.BRun(); } }
  public class K3 : One.Bs3 /*c d*/ { void M(){ base.BRun(); } }
  public class K4 : Bs4 <int> { void M(){ base.BRun(); } }
  public class K5 : One.Bs5<
     int> { void M(){ base.BRun(); } }
  public class U { void R(K k, K2 k2, K4 k4){ k.BRun(); k2.BRun(); k4.BRun(); } }
}
"#,
            ),
            (
                "Lib.cs",
                r#"namespace One {
  public class Bs<T> { public void BRun(){} }
  public class Bs2 { public void BRun(){} }
  public class Bs3 { public void BRun(){} }
  public class Bs4<T> { public void BRun(){} }
  public class Bs5<T> { public void BRun(){} }
}
"#,
            ),
        ],
        want: &[
            "K3.M()[Host.cs] -> Bs3.BRun()[Lib.cs] EXTRACTED 1",
            "K4.M()[Host.cs] -> Bs4.BRun()[Lib.cs] EXTRACTED 1",
            "K5.M()[Host.cs] -> Bs5.BRun()[Lib.cs] EXTRACTED 1",
            "U.R()[Host.cs] -> Bs4.BRun()[Lib.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "generic-twin-kept",
        files: &[
            (
                "Lib/IM.cs",
                "namespace N;\n\n\n\n\n\n\n\npublic interface IM<A, B, C> { void Deep(); }\npublic interface IM : IM<object, object, object> { void Top(); }\n",
            ),
            (
                "Use/U.cs",
                r#"namespace N.Use; public class U { public void Run(IM m){ m.Deep(); m.Top(); } }
"#,
            ),
        ],
        // "L10" sorts before "L9": the non-generic IM is kept.
        want: &["U.Run()[U.cs] -> IM.Top()[IM.cs] INFERRED 0.85"],
    },
    Case {
        name: "primary-ctor-base-args",
        files: &[(
            "A.cs",
            r#"namespace N {
public class Logger { public void Log() {} public void Run() {} }
public class Base { public Base(Logger l) {} public void Run() {} }
public class Svc(Logger Logger) : Base(Logger) {
  public void A() { this.Run(); }
  public void B() { this.Log(); }
}
public class P2 { public P2(int x) {} public void Move() {} }
public class C2(int y) : P2(y) { public void A() { this.Move(); } }
public record R1 { public void Move() {} }
public record R3(int Z) : R1 { public void A() { this.Move(); } }
public record R4(int Z) : R1() { public void A() { this.Move(); } }
public record Child(int X, int Y) : R3(X) { public void B() { base.Move(); } }
public class Use { Svc s; C2 c; R4 r; Child h; public void M() { s.Run(); s.Log(); c.Move(); r.Move(); h.Move(); } }
}
"#,
        )],
        // Upstream reads no base from `B(x)` records.
        want: &[
            "C2.A()[A.cs] -> P2.Move()[A.cs] EXTRACTED 1",
            "R3.A()[A.cs] -> R1.Move()[A.cs] EXTRACTED 1",
            "Svc.A()[A.cs] -> Base.Run()[A.cs] EXTRACTED 1",
            "Use.M()[A.cs] -> Base.Run()[A.cs] INFERRED 0.85",
            "Use.M()[A.cs] -> P2.Move()[A.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "verbatim-keys",
        files: &[(
            "A.cs",
            r#"namespace N {
public class @Foo { public void Go() {} public static void S() {} }
public class Bar { public void @Go() {} public void Run() {} public static void @S() {} }
public class Use {
  Foo f; Bar b;
  public void A() { f.Go(); }
  public void E() { Foo.S(); }
  public void G() { b.Go(); }
  public void H() { b.@Run(); }
  public void I() { Bar.S(); }
}
}
"#,
        )],
        want: &[
            "Use.A()[A.cs] -> @Foo.Go()[A.cs] INFERRED 0.85",
            "Use.E()[A.cs] -> @Foo.S()[A.cs] EXTRACTED 1",
            "Use.G()[A.cs] -> Bar.@Go()[A.cs] INFERRED 0.85",
            "Use.H()[A.cs] -> Bar.Run()[A.cs] INFERRED 0.85",
            "Use.I()[A.cs] -> Bar.@S()[A.cs] EXTRACTED 1",
        ],
    },
    Case {
        name: "twin-fields",
        files: &[(
            "A.cs",
            r#"class Svc { public void Run(){} public void S(){} }
class Other { public void Run(){} public void O(){} }
class Box { Svc f; Svc g; void M(){ f.Run(); g.S(); h.O(); } }
class Box<T> { Other f; Other h; void N(){ f.Run(); h.O(); g.S(); } }
class D : Box<int> { void Q(){ f.Run(); g.S(); h.O(); } }
class E : Box { void R(){ f.Run(); g.S(); h.O(); } }
"#,
        )],
        want: &[
            "Box.M()[A.cs] -> Svc.Run()[A.cs] INFERRED 0.85",
            "Box.M()[A.cs] -> Svc.S()[A.cs] INFERRED 0.85",
            "Box.N()[A.cs] -> Other.O()[A.cs] INFERRED 0.85",
            "Box.N()[A.cs] -> Other.Run()[A.cs] INFERRED 0.85",
            "D.Q()[A.cs] -> Other.O()[A.cs] INFERRED 0.85",
            "D.Q()[A.cs] -> Other.Run()[A.cs] INFERRED 0.85",
            "E.R()[A.cs] -> Svc.Run()[A.cs] INFERRED 0.85",
            "E.R()[A.cs] -> Svc.S()[A.cs] INFERRED 0.85",
        ],
    },
    Case {
        name: "alias-nested-and-dotless",
        files: &[
            (
                "A.cs",
                r#"public class Svc { public void Run() {} }
namespace N {
public class Outer<T> { public void Run() {} public class Inner { public void Run() {} } }
public class Box<T> { public void Go() {} }
}
"#,
            ),
            (
                "B.cs",
                r#"using X = N.Outer<int>.Inner;
using Y = Svc;
using Z = N.Box<N.Outer<int>>;
namespace U {
public class Use {
    X x; Y y; Z z;
    public void M() { x.Run(); y.Run(); Y.Run(); z.Go(); }
}
}
"#,
            ),
        ],
        want: &["Use.M()[B.cs] -> Box.Go()[A.cs] INFERRED 0.85"],
    },
    Case {
        name: "fallback-counts-other-languages",
        files: &[
            (
                "A.cs",
                r#"namespace N { public class Svc { public void Run() {} } }"#,
            ),
            (
                "B.cs",
                r#"namespace U { public class Use { public void M(Svc s) { s.Run(); } public void K() { new Svc(); } } }"#,
            ),
            ("svc.py", "class Svc:\n    def run(self):\n        pass\n"),
        ],
        want: &[],
    },
    Case {
        name: "new-lowercase-receiver",
        files: &[(
            "A.cs",
            r#"public class svc { public void Run() {} }
public class _Thing { public void Go() {} }
public class Use {
    public void M() { new svc().Run(); }
    public void N() { new _Thing().Go(); }
    public void J() { ((svc)null).Run(); (null as _Thing).Go(); }
}
"#,
        )],
        want: &[
            "Use.M()[A.cs] -> svc.Run()[A.cs] INFERRED 0.85",
            "Use.M()[A.cs] -> svc[A.cs] EXTRACTED 1",
            "Use.N()[A.cs] -> _Thing.Go()[A.cs] INFERRED 0.85",
            "Use.N()[A.cs] -> _Thing[A.cs] EXTRACTED 1",
        ],
    },
];

/// `TestCSharpTypedCalls` holds upstream 0.9.92's calls edges for C# member calls and `new X()`.
/// Where Go differs on purpose the case says why. The case "fallback-counts-other-languages"
/// also parses a Python file (`svc.py`, graphify-rs-ihc.25).
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_c_sharp_typed_calls() {
    for tc in CASES {
        let got = cs_call_edges(tc.files);
        let mut want: Vec<String> = tc.want.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(got, want, "{}", tc.name);
    }
}

/// Go `TestCSharpOddTypeSpellings` feeds spaces, newlines and comments into every place a C#
/// type name is read: none may panic.
const ODD_TYPES: [&str; 15] = [
    "One.Gen <int>",
    "One.Gen <One.Svc>",
    "One . Svc",
    "One.\n  Svc",
    "One.Gen/*a b*/<int>",
    "global::System.Collections.Generic.Dictionary<global::One.Svc,\n   string>",
    "One.Gen<One.Gen <int> >",
    "One.Svc /*x y*/",
    "One.Gen <\tint\t>",
    "global :: One.Svc",
    "One.Gen <int> []",
    "One.Gen <int>?",
    "(One.Svc a, One.Gen <int> b)",
    "One.Gen</*c*/ One.Svc /*d*/>",
    "One // c\n .Svc",
];

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_c_sharp_odd_type_spellings() {
    for (i, typ) in ODD_TYPES.iter().enumerate() {
        let src = format!(
            "using A1 = {typ};\nusing One /* p q */ ;\nnamespace N{i} /* r s */ . In {{\n\
             public class K : {typ} {{ }}\n\
             public class P({typ} g) {{ void M(){{ g.Run(); }} }}\n\
             public class H {{\n  public {typ} F;\n  public {typ} Prop {{ get; }}\n  public System.Collections.Generic.List<{typ}> L;\n\
             \x20 void M({typ} p, object o){{ {typ} v = default; F.Run(); Prop.Run(); p.Run(); v.Run(); L[0].Run(); this.F.Run();\n\
             \x20   foreach (var e in L) e.Run(); (({typ})o).Run(); (o as {typ}).Run(); new {typ}().Run(); A1.Run(); }}\n}}\n}}\n"
        );
        // Go ignores the edges: the run is the check.
        cs_call_edges(&[
            (
                "Lib.cs",
                "namespace One { public class Gen<T> { public void Run(){} } public class Svc { public void Run(){} } }\n",
            ),
            ("Odd.cs", src.as_str()),
        ]);
    }
}
