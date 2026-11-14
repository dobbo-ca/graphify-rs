//! upstream graphify v0.9.107 `test_csharp_extension_calls.py`, `test_csharp_extension_metadata.py`,
//! `test_csharp_extension_resolution.py` and `test_csharp_interface_dispatch.py`, ported for
//! graphify-go v0.12.0.
//!
//! Go has no extension-method binding: a member call is bound on its receiver's declared type and
//! the type's bases only (`csharp_resolve.go` `bind`/`lookup`), and a call whose receiver has no
//! type is not recorded at all (`csharp.go` `csCalls`), so `x.Ext()` never reaches a `this`
//! parameter method. The resolution fixtures that only forbid an edge therefore hold, and the ones
//! that expect an extension edge do not.

use std::collections::BTreeSet;

use super::csharp_tests::{has_edge, pairs, run};
use crate::model::Extraction;

// ---------------------------------------------------------------------------------------------
// test_csharp_extension_calls.py (13 tests, none ported)
//
// Every test scores the candidate binding strategies S0..S9 of the Python harness
// (`csharp_extension_harness.py` `apply_strategy`, `strategy_bindings`, `STRATEGIES`): python
// internals, there is no Rust counterpart to run. `test_fixture_markers_are_unambiguous` only
// checks the Python fixtures' own markers (python internals as well).
//
// test_csharp_extension_metadata.py (9 tests, none ported)
//
// Every test reads `metadata` of a node (`extension`, `extended_type`, `extended_type_arity`,
// `extended_is_type_param`, `arity`, `min_arity`, `unresolved_calls`) or the extractor's
// `raw_calls`: Go lacks node metadata and raw calls.
//
// test_csharp_extension_resolution.py (16 tests)
//
// Ported: the ten tests that hold without extension binding. Not ported, all "Go lacks it"
// (extension-method binding): `test_production_binds_the_decidable_extension_calls`,
// `test_extension_edge_is_inferred_at_the_call_site`,
// `test_out_of_corpus_receiver_binds_through_the_guarded_rung`,
// `test_out_of_corpus_extension_edge_is_scored_lower`,
// `test_open_generic_extension_applies_only_as_a_wildcard`,
// `test_chained_receiver_binds_when_the_name_is_unique`.
// ---------------------------------------------------------------------------------------------

struct Fixture {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    /// Call markers that must not bind to anything.
    forbid: &'static [&'static str],
    /// (call marker, target file, target marker) that must not bind.
    forbid_pairs: &'static [(&'static str, &'static str, &'static str)],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "issue_repro",
        files: &[
            (
                "Ext.cs",
                concat!(
                    "namespace Ext {\n",
                    "  public static class StringExtensions {\n",
                    "    public static string Mask(this string s, char m = '*') { return s; } // D1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            (
                "Svc.cs",
                concat!(
                    "using Ext;\n",
                    "public class Svc {\n",
                    "  public string A(string s) { return s.Mask(); } // E1\n",
                    "  public string B(long v) { return v.ToString().Mask(); } // E2\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "domain_cross_file",
        files: &[
            (
                "Model.cs",
                concat!(
                    "namespace App {\n",
                    "  public class Widget { }\n",
                    "  public static class WidgetExtensions {\n",
                    "    public static void Paint(this Widget w) { } // D1\n",
                    "    public static void Paint(this Widget w, int c) { } // D2\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            (
                "Use.cs",
                concat!(
                    "using App;\n",
                    "namespace App {\n",
                    "  public class Painter {\n",
                    "    public void Run(Widget w) {\n",
                    "      w.Paint(); // E1\n",
                    "      w.Paint(3); // E2\n",
                    "    }\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "same_file",
        files: &[(
            "All.cs",
            concat!(
                "namespace App {\n",
                "  public class Widget { }\n",
                "  public static class WExt {\n",
                "    public static void Paint(this Widget w) { } // D1\n",
                "  }\n",
                "  public class Painter {\n",
                "    public void Run(Widget w) { w.Paint(); } // E1\n",
                "  }\n",
                "}\n",
            ),
        )],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "base_and_interface",
        files: &[
            (
                "Domain.cs",
                concat!(
                    "namespace App {\n",
                    "  public class Animal { }\n",
                    "  public class Dog : Animal { }\n",
                    "  public interface IStore { }\n",
                    "  public class SqlStore : IStore { }\n",
                    "  public static class Exts {\n",
                    "    public static void Speak(this Animal a) { } // D1\n",
                    "    public static void SaveStore(this IStore s) { } // D2\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            (
                "UseApp.cs",
                concat!(
                    "using App;\n",
                    "namespace App {\n",
                    "  public class Svc {\n",
                    "    public void A(Dog d) { d.Speak(); } // E1\n",
                    "    public void B(SqlStore s) { s.SaveStore(); } // E2\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "unresolved_base_poison",
        files: &[
            (
                "W.cs",
                concat!(
                    "namespace App {\n",
                    "  public class W : ThirdParty.Base { }\n",
                    "  public static class WExt {\n",
                    "    public static void Reset(this W w) { } // D1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            (
                "Use.cs",
                concat!(
                    "using App;\n",
                    "namespace App {\n",
                    "  public class U {\n",
                    "    public void A(W w) { w.Reset(); } // E1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &["// E1"],
        forbid_pairs: &[],
    },
    Fixture {
        name: "ambiguous_same_scope",
        files: &[
            ("Common.cs", "public class Thing { }\n"),
            (
                "A.cs",
                concat!(
                    "namespace N1 { public static class X1 {\n",
                    "  public static void Go(this Thing t) { } // D1\n",
                    "} }\n",
                ),
            ),
            (
                "B.cs",
                concat!(
                    "namespace N2 { public static class X2 {\n",
                    "  public static void Go(this Thing t) { } // D2\n",
                    "} }\n",
                ),
            ),
            (
                "Use.cs",
                concat!(
                    "using N1;\n",
                    "using N2;\n",
                    "public class U {\n",
                    "  public void Run(Thing o) { o.Go(); } // E1\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &["// E1"],
        forbid_pairs: &[],
    },
    Fixture {
        name: "inner_scope_wins",
        files: &[
            (
                "Outer.cs",
                concat!(
                    "namespace N {\n",
                    "  public class Thing { }\n",
                    "  public static class OuterExt {\n",
                    "    public static void Go(this Thing t) { } // D1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            (
                "Inner.cs",
                concat!(
                    "namespace N.Inner { public static class InnerExt {\n",
                    "  public static void Go(this Thing t) { } // D2\n",
                    "} }\n",
                ),
            ),
            (
                "Use.cs",
                concat!(
                    "using N;\n",
                    "namespace N.Inner {\n",
                    "  public class U { public void Run(Thing o) { o.Go(); } // E1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[("// E1", "Outer.cs", "// D1")],
    },
    Fixture {
        name: "out_of_scope",
        files: &[
            (
                "Other.cs",
                concat!(
                    "namespace Other {\n",
                    "  public static class OExt {\n",
                    "    public static void Secret(this App.Widget w) { } // D1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            ("Model.cs", "namespace App { public class Widget { } }\n"),
            (
                "Use.cs",
                concat!(
                    "using App;\n",
                    "namespace App {\n",
                    "  public class U { public void Run(Widget w) { w.Secret(); } // E1\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &["// E1"],
        forbid_pairs: &[],
    },
    Fixture {
        name: "instance_wins",
        files: &[(
            "Cache.cs",
            concat!(
                "namespace App {\n",
                "  public class Cache {\n",
                "    public object GetValue() { return null; } // D1\n",
                "  }\n",
                "  public static class CacheExt {\n",
                "    public static object GetValue(this Cache c) { return c; } // D2\n",
                "  }\n",
                "  public class U {\n",
                "    public object Run(Cache c) { return c.GetValue(); } // E1\n",
                "  }\n",
                "}\n",
            ),
        )],
        forbid: &[],
        forbid_pairs: &[("// E1", "Cache.cs", "// D2")],
    },
    Fixture {
        name: "bcl_name_cost",
        files: &[
            (
                "E.cs",
                concat!(
                    "public static class ObjExt {\n",
                    "  public static object GetValue(this object o) { return o; } // D1\n",
                    "  public static object GetValue(this object o, object idx) { return o; } // D2\n",
                    "}\n",
                ),
            ),
            (
                "U.cs",
                concat!(
                    "using System.Reflection;\n",
                    "public class U {\n",
                    "  public object A(PropertyInfo p) { return p.GetValue(null); } // E1\n",
                    "  public object B(object o) { return o.GetValue(); } // E2\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &["// E1"],
        forbid_pairs: &[],
    },
    Fixture {
        name: "arity",
        files: &[(
            "Arity.cs",
            concat!(
                "namespace App {\n",
                "  public class W { }\n",
                "  public static class E {\n",
                "    public static void Log(this W w) { } // D1\n",
                "    public static void Log(this W w, int n) { } // D2\n",
                "    public static void Opt(this W w, int a, int b = 0) { } // D3\n",
                "    public static void Many(this W w, params object[] rest) { } // D4\n",
                "  }\n",
                "  public class U {\n",
                "    public void C1(W w) { w.Log(); } // E1\n",
                "    public void C2(W w) { w.Log(1); } // E2\n",
                "    public void C3(W w) { w.Opt(1); } // E3\n",
                "    public void C4(W w) { w.Opt(1, 2); } // E4\n",
                "    public void C5(W w) { w.Many(); } // E5\n",
                "    public void C6(W w) { w.Many(1, 2, 3); } // E6\n",
                "    public void C7(W w) { w.Opt(); } // F1\n",
                "    public void C8(W w) { w.Log(1, 2); } // F2\n",
                "  }\n",
                "}\n",
            ),
        )],
        forbid: &["// F1", "// F2"],
        forbid_pairs: &[],
    },
    Fixture {
        name: "primitive_and_thirdparty",
        files: &[
            (
                "Ext.cs",
                concat!(
                    "namespace App {\n",
                    "  public static class StringExt {\n",
                    "    public static string Mask(this string s) { return s; } // D1\n",
                    "    public static int Twice(this int i) { return i * 2; } // D2\n",
                    "  }\n",
                    "  public static class HttpExt {\n",
                    "    public static string GetJson(this System.Net.Http.HttpClient c) { return \"\"; } // D3\n",
                    "  }\n",
                    "}\n",
                ),
            ),
            (
                "Use.cs",
                concat!(
                    "using App;\n",
                    "using System.Net.Http;\n",
                    "namespace App {\n",
                    "  public class U {\n",
                    "    public string A(string s) { return s.Mask(); } // E1\n",
                    "    public int B(int i) { return i.Twice(); } // E2\n",
                    "    public string C(HttpClient c) { return c.GetJson(); } // E3\n",
                    "  }\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "type_param_vs_class",
        files: &[
            (
                "T.cs",
                concat!(
                    "public class T { }\n",
                    "public static class TExt {\n",
                    "  public static void Go(this T t) { } // D1\n",
                    "}\n",
                    "public static class GenExt {\n",
                    "  public static void Go<T>(this T t) { } // D2\n",
                    "}\n",
                ),
            ),
            (
                "U.cs",
                concat!(
                    "public class U {\n",
                    "  public void A(T t) { t.Go(); } // E1\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "generic_wildcard",
        files: &[
            (
                "Gen.cs",
                concat!(
                    "public static class GenExt {\n",
                    "  public static void Go<T>(this T t) { } // D1\n",
                    "}\n",
                ),
            ),
            (
                "U.cs",
                concat!(
                    "public class U {\n",
                    "  public void A(int i) { i.Go(); } // E1\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[],
    },
    Fixture {
        name: "static_class_receiver",
        files: &[
            (
                "E.cs",
                concat!(
                    "public static class Conv {\n",
                    "  public static int ToInt32(object o) { return 0; } // D1\n",
                    "}\n",
                    "public static class SExt {\n",
                    "  public static int ToInt32(this string s) { return 0; } // D2\n",
                    "}\n",
                ),
            ),
            (
                "U.cs",
                concat!(
                    "public class U {\n",
                    "  public int A(object o) { return Conv.ToInt32(o); } // B1\n",
                    "  public int B(string s) { return s.ToInt32(); } // E1\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &[],
        forbid_pairs: &[("// B1", "E.cs", "// D2")],
    },
    Fixture {
        name: "untyped_regular_name",
        files: &[
            (
                "Cache.cs",
                "public class Cache {\n  public void Flush() { } // D1\n}\n",
            ),
            (
                "E.cs",
                concat!(
                    "public static class Ext {\n",
                    "  public static void Flush(this string s) { } // D2\n",
                    "}\n",
                ),
            ),
            (
                "U.cs",
                concat!(
                    "public class U {\n",
                    "  public void A(long v) { v.ToString().Flush(); } // F1\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &["// F1"],
        forbid_pairs: &[],
    },
    Fixture {
        name: "god_node_control",
        files: &[
            (
                "Cache.cs",
                "public class Cache {\n  public void Save() { } // D1\n}\n",
            ),
            ("Server.cs", "public class Server { }\n"),
            (
                "Use.cs",
                concat!(
                    "public class U {\n",
                    "  public void Run(Server s) { s.Save(); } // F1\n",
                    "}\n",
                ),
            ),
        ],
        forbid: &["// F1"],
        forbid_pairs: &[],
    },
];

fn fixture(name: &str) -> &'static Fixture {
    FIXTURES
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no fixture {name}"))
}

/// The marker (`// E1`) a line ends with, as the harness's `marker_at_line` finds it.
fn marker_of(line: &str) -> Option<String> {
    let i = line.rfind("// ")?;
    let tag = &line[i + 3..];
    let mut chars = tag.chars();
    let first = chars.next()?;
    if first.is_ascii_uppercase() && tag.len() > 1 && chars.all(|c| c.is_ascii_digit()) {
        Some(format!("// {tag}"))
    } else {
        None
    }
}

/// (call marker, (target file, target marker)).
type Binding = (String, (String, String));

struct Bindings {
    bound: BTreeSet<Binding>,
    /// `calls` edges whose caller line or target declaration line has no marker.
    phantoms: Vec<String>,
}

/// The harness's `graph_bindings`: each `calls` edge as (call marker, target declaration marker).
fn graph_bindings(f: &Fixture) -> Bindings {
    let ext = run(f.files);
    let marker_at = |file: &str, loc: &str| -> Option<String> {
        let text = f.files.iter().find(|(n, _)| *n == file)?.1;
        let n: usize = loc.strip_prefix('L')?.parse().ok()?;
        marker_of(text.lines().nth(n.checked_sub(1)?)?)
    };
    let mut bound = BTreeSet::new();
    let mut phantoms = Vec::new();
    for e in ext.edges.iter().filter(|e| e.relation.as_str() == "calls") {
        let call = marker_at(e.source_file.as_str(), &e.source_location.to_string());
        let target = ext.nodes.iter().find(|n| n.id == e.target);
        let hit = call.zip(target).and_then(|(c, t)| {
            let m = marker_at(&t.source_file, &t.source_location)?;
            Some((c, (t.source_file.clone(), m)))
        });
        if let Some(b) = hit {
            bound.insert(b);
        } else {
            phantoms.push(format!("{} -> {}", e.source, e.target));
        }
    }
    Bindings { bound, phantoms }
}

fn bound(b: &Bindings, call: &str, file: &str, target: &str) -> bool {
    b.bound
        .contains(&(call.to_string(), (file.to_string(), target.to_string())))
}

fn binds_call(b: &Bindings, call: &str) -> bool {
    b.bound.iter().any(|(m, _)| m == call)
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_production_never_emits_a_forbidden_edge() {
    for f in FIXTURES {
        let b = graph_bindings(f);
        let mut violations: Vec<String> = b.phantoms.clone();
        for (call, (file, target)) in &b.bound {
            if f.forbid.contains(&call.as_str()) {
                violations.push(format!("{call} -> {file} {target}"));
            }
            if f.forbid_pairs.iter().any(|(c, tf, tm)| {
                *c == call.as_str() && *tf == file.as_str() && *tm == target.as_str()
            }) {
                violations.push(format!("{call} -> {file} {target}"));
            }
        }
        assert!(violations.is_empty(), "{}: {violations:?}", f.name);
    }
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_production_invents_no_call_site() {
    for f in FIXTURES {
        let b = graph_bindings(f);
        assert!(b.phantoms.is_empty(), "{}: {:?}", f.name, b.phantoms);
    }
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
/// `Cache.GetValue()` is nominal; the extension of the same name must not win.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_instance_member_wins_over_a_same_named_extension() {
    let b = graph_bindings(fixture("instance_wins"));
    assert!(bound(&b, "// E1", "Cache.cs", "// D1"));
    assert!(!bound(&b, "// E1", "Cache.cs", "// D2"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
/// `Conv.ToInt32(x)` names its type in source and is bound by the nominal pass.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_static_form_is_never_rebound_to_the_extension() {
    let b = graph_bindings(fixture("static_class_receiver"));
    assert!(bound(&b, "// B1", "E.cs", "// D1"));
    assert!(!bound(&b, "// B1", "E.cs", "// D2"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_ambiguity_binds_nothing() {
    let b = graph_bindings(fixture("ambiguous_same_scope"));
    assert!(!binds_call(&b, "// E1"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_out_of_scope_extension_binds_nothing() {
    let b = graph_bindings(fixture("out_of_scope"));
    assert!(!binds_call(&b, "// E1"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_unresolved_base_poisons_the_lookup() {
    let b = graph_bindings(fixture("unresolved_base_poison"));
    assert!(!binds_call(&b, "// E1"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
/// `PropertyInfo.GetValue(obj)` is an instance call; the corpus cannot prove otherwise, so the
/// same-named extension is refused.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_bcl_member_name_on_an_undeclared_type_is_not_bound() {
    let b = graph_bindings(fixture("bcl_name_cost"));
    assert!(!binds_call(&b, "// E1"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_untyped_rung_refuses_a_name_a_regular_method_uses() {
    let b = graph_bindings(fixture("untyped_regular_name"));
    assert!(!binds_call(&b, "// F1"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B20
/// `Conv.ToInt32(x)` names a type; the extension must not be considered.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_untyped_rung_refuses_a_capitalized_type_receiver() {
    let b = graph_bindings(fixture("static_class_receiver"));
    assert!(!bound(&b, "// B1", "E.cs", "// D2"));
}

// ---------------------------------------------------------------------------------------------
// test_csharp_interface_dispatch.py (18 tests)
//
// Ported (11): see the tests below. Not ported (7):
// - "Go lacks it" (2): `test_two_implementers_both_get_an_edge_at_lower_confidence` and
//   `test_three_implementers_floor_at_the_weak_inference_tier`: Go emits `dispatches_to` only
//   for an interface with exactly one implementer (`dispatch.go`), and gives the edge no
//   confidence_score (0.85/0.75/0.65 tiers are Python's).
// - "python internals" (5): `test_one_ambiguous_implementer_does_not_suppress_its_siblings`,
//   `test_an_unrelated_edge_between_the_members_does_not_suppress_the_link`,
//   `test_an_existing_dispatch_edge_is_not_duplicated` and
//   `test_a_gate_failing_implementer_does_not_dilute_confidence` call Python's
//   `resolve_csharp_interface_dispatch` on hand-built node/edge dicts;
//   `test_member_name_is_read_up_to_the_first_parenthesis` calls Python's `_method_label`.
// ---------------------------------------------------------------------------------------------

const INJECTED: &[(&str, &str)] = &[
    ("IReport.cs", "public interface IReport { void Build(); }\n"),
    (
        "Report.cs",
        concat!(
            "public class Report : IReport {\n",
            "    public void Build() { Format(); }\n",
            "    public void Format() { }\n",
            "}\n",
        ),
    ),
    (
        "Runner.cs",
        concat!(
            "public class Runner {\n",
            "    private readonly IReport _report;\n",
            "    public Runner(IReport report) { _report = report; }\n",
            "    public void Go() { _report.Build(); }\n",
            "}\n",
        ),
    ),
];

fn dispatch_pairs(ext: &Extraction) -> BTreeSet<(String, String)> {
    pairs(ext, "dispatches_to")
}

fn node_id<'a>(ext: &'a Extraction, label: &str) -> &'a str {
    ext.nodes
        .iter()
        .find(|n| n.label == label)
        .unwrap_or_else(|| panic!("no node {label:?}"))
        .id
        .as_str()
}

/// Upstream asserts confidence 0.85 as well; Go's `dispatches_to` edge carries no score, so only
/// the INFERRED confidence is kept.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_single_implementer_links_the_interface_method() {
    let ext = run(INJECTED);
    assert!(has_edge(
        &ext,
        "IReport.Build()",
        "dispatches_to",
        "Report.Build()"
    ));
    for e in ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "dispatches_to")
    {
        assert_eq!(e.confidence.as_str(), "INFERRED");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_chain_through_an_injected_dependency_becomes_reachable() {
    let ext = run(INJECTED);
    assert!(!dispatch_pairs(&ext).is_empty());
    // Go -> IReport.Build -> Report.Build -> Format
    let start = node_id(&ext, "Runner.Go()");
    let goal = node_id(&ext, "Report.Format()");
    let mut seen: BTreeSet<&str> = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(cur) = stack.pop() {
        for e in ext.edges.iter().filter(|e| e.source == cur) {
            if seen.insert(e.target.as_str()) {
                stack.push(e.target.as_str());
            }
        }
    }
    assert!(
        seen.contains(goal),
        "Report.Format() not reachable from Runner.Go()"
    );
}

/// Additive: the call site really does name the interface.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_the_call_to_the_interface_method_is_kept() {
    let ext = run(INJECTED);
    assert!(has_edge(&ext, "Runner.Go()", "calls", "IReport.Build()"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_no_matching_member_produces_no_edge() {
    let ext = run(&[(
        "S.cs",
        concat!(
            "public interface IR { void Build(); }\n",
            "public class A : IR { public void Other() { } }\n",
        ),
    )]);
    assert!(dispatch_pairs(&ext).is_empty());
}

/// Go keeps one node per method name (overloads share an id), so the interface's `M()` pairs with
/// the one `A.M()`.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_overloaded_implementer_still_links() {
    let ext = run(&[(
        "S.cs",
        concat!(
            "public interface IR { void M(); }\n",
            "public class A : IR { public void M() { } public void M(int x) { } }\n",
        ),
    )]);
    assert!(has_edge(&ext, "IR.M()", "dispatches_to", "A.M()"));
}

/// `class Impl : Base` is an `inherits` edge, and a base method may well be the real target, so
/// walking inheritance is a different bet from dispatch.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_abstract_base_class_is_out_of_scope() {
    let ext = run(&[(
        "S.cs",
        concat!(
            "public abstract class Base { public abstract void M(); }\n",
            "public class Impl : Base { public override void M() { } }\n",
        ),
    )]);
    assert!(dispatch_pairs(&ext).is_empty());
}

/// Known limit: IBase's only implementer is IChild, which declares nothing, so IBase.M never
/// reaches Impl.M.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_interface_hierarchy_is_not_walked_transitively() {
    let ext = run(&[(
        "S.cs",
        concat!(
            "public interface IBase { void M(); }\n",
            "public interface IChild : IBase { }\n",
            "public class Impl : IChild { public void M() { } }\n",
        ),
    )]);
    assert!(dispatch_pairs(&ext).is_empty());
}

// Also parses the Java grammar.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_java_interface_and_implementation_are_untouched() {
    let ext = run(&[
        (
            "IReport.java",
            "public interface IReport { void build(); }\n",
        ),
        (
            "Report.java",
            "public class Report implements IReport { public void build() { } }\n",
        ),
    ]);
    assert!(dispatch_pairs(&ext).is_empty());
}

/// C# is case sensitive: an implementation must spell the member exactly, so `build` does not
/// implement `Build`.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_case_only_member_difference_is_not_a_match() {
    let ext = run(&[(
        "S.cs",
        concat!(
            "public interface IR { void Build(); }\n",
            "public class A : IR { public void build() { } }\n",
        ),
    )]);
    assert!(dispatch_pairs(&ext).is_empty());
}

// Also parses the Java grammar.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_a_non_csharp_implementer_is_not_dispatched_to() {
    let ext = run(&[
        ("IReport.cs", "public interface IReport { void Build(); }\n"),
        (
            "Report.java",
            "public class Report implements IReport { public void Build() { } }\n",
        ),
    ]);
    assert!(dispatch_pairs(&ext).is_empty());
}

// Also parses the Java grammar.
#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_mixed_corpus_links_only_the_csharp_pair() {
    let ext = run(&[
        ("IReport.cs", "public interface IReport { void Build(); }\n"),
        (
            "Report.cs",
            "public class Report : IReport { public void Build() { } }\n",
        ),
        ("IJob.java", "public interface IJob { void run(); }\n"),
        (
            "Job.java",
            "public class Job implements IJob { public void run() { } }\n",
        ),
    ]);
    let file_of = |id: &str| {
        ext.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.source_file.clone())
            .unwrap_or_default()
    };
    let linked: BTreeSet<(String, String)> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "dispatches_to")
        .map(|e| (file_of(&e.source), file_of(&e.target)))
        .collect();
    let want = BTreeSet::from([("IReport.cs".to_string(), "Report.cs".to_string())]);
    assert_eq!(linked, want);
}
