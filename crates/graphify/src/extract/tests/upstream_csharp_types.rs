//! The cases of the upstream graphify v0.9.107 C# type tests (`test_csharp_type_resolution.py`,
//! `test_csharp_tuple_type_refs.py`, `test_csharp_field_generic_args.py`,
//! `test_csharp_delegate_declarations.py`, `test_csharp_enum_members.py`,
//! `test_csharp_verbatim_interpolation.py`) whose assertion holds for graphify-go v0.12.0 (see the
//! `extract-csharp` block of `parity/README.md`). The upstream sources are written inline.
//!
//! Go binds a base or a type position by name alone (the file's own definition, else the one
//! definition of the corpus, else a stub per file); it has no namespace-aware binding, no node
//! for a nested type, an enum member or a delegate, and node ids carry no namespace.

use std::collections::BTreeMap;

use super::csharp_tests::{has_edge, pairs, run};
use crate::model::{Extraction, Node};

fn by_id(ext: &Extraction) -> BTreeMap<&str, &Node> {
    ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect()
}

/// The node each `relation` edge points at, when its label is `label` (one entry per edge).
fn targets<'a>(ext: &'a Extraction, relation: &str, label: &str) -> Vec<&'a Node> {
    let nodes = by_id(ext);
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .filter_map(|e| nodes.get(e.target.as_str()).copied())
        .filter(|n| n.label == label)
        .collect()
}

/// The node each `relation` edge from a node labelled `src` points at.
fn targets_from<'a>(ext: &'a Extraction, relation: &str, src: &str) -> Vec<&'a Node> {
    let nodes = by_id(ext);
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .filter(|e| nodes.get(e.source.as_str()).is_some_and(|n| n.label == src))
        .filter_map(|e| nodes.get(e.target.as_str()).copied())
        .collect()
}

/// The nodes labelled `label` that have a source file (a real definition, not a stub).
fn defs<'a>(ext: &'a Extraction, label: &str) -> Vec<&'a Node> {
    ext.nodes
        .iter()
        .filter(|n| n.label == label && !n.source_file.is_empty())
        .collect()
}

/// The nodes labelled `label` with no source file (a stub).
fn stubs<'a>(ext: &'a Extraction, label: &str) -> Vec<&'a Node> {
    ext.nodes
        .iter()
        .filter(|n| n.label == label && n.source_file.is_empty())
        .collect()
}

// ---- upstream `test_csharp_tuple_type_refs.py` (2 of 2 ported) ----

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_tuple_element_names_are_not_type_references() {
    let ext = run(&[(
        "Reader.cs",
        concat!(
            "namespace Demo\n{\n    public class Reader\n    {\n",
            "        public static (int mode, string label) Read() => (1, \"a\");\n",
            "    }\n}\n"
        ),
    )]);

    // Element NAMES must not be minted as type nodes.
    for n in &ext.nodes {
        assert!(n.label != "mode", "node for tuple element name mode");
        assert!(n.label != "label", "node for tuple element name label");
    }
    // And no references edge may target a placeholder minted from a name.
    for name in ["mode", "label"] {
        assert!(
            targets(&ext, "references", name).is_empty(),
            "references edge to tuple element name {name}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_user_defined_tuple_element_types_are_still_referenced() {
    let ext = run(&[(
        "Reader.cs",
        concat!(
            "namespace Demo\n{\n",
            "    public class Mode { }\n",
            "    public class Label { }\n",
            "    public class Reader\n    {\n",
            "        public static (Mode mode, Label label) Read() => (null!, null!);\n",
            "    }\n}\n"
        ),
    )]);

    let labels: Vec<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"Mode"));
    assert!(labels.contains(&"Label"));
    assert!(!labels.contains(&"mode"));
    assert!(!labels.contains(&"label"));

    // The declaring member references its tuple element types.
    let refs = pairs(&ext, "references");
    assert!(
        refs.iter().any(|(_, t)| t == "Mode"),
        "no references edge to Mode"
    );
    assert!(
        refs.iter().any(|(_, t)| t == "Label"),
        "no references edge to Label"
    );
}

// ---- upstream `test_csharp_field_generic_args.py` (8 of 8 ported) ----

const FIELD_TYPES: &str = concat!(
    "public interface IThing { }\n",
    "public class Box<T> { }\n",
    "public class Outer { public class Inner { } }\n"
);

/// {(source label, target label)} of the `references` edges of `T.cs` + `P.cs`.
fn field_refs(probe: &str) -> std::collections::BTreeSet<(String, String)> {
    pairs(
        &run(&[("T.cs", FIELD_TYPES), ("P.cs", probe)]),
        "references",
    )
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_field_generic_argument_produces_edge() {
    let refs = field_refs("public class Probe { private Box<IThing> _f = null!; }\n");
    assert!(
        refs.contains(&("Probe".to_string(), "Box".to_string())),
        "outer field type must still be linked"
    );
    assert!(
        refs.contains(&("Probe".to_string(), "IThing".to_string())),
        "generic argument in field position must be linked"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_field_matches_property_behaviour() {
    // A field and a property of the same type must produce the same references.
    let refs = field_refs(concat!(
        "public class WithField { private Box<IThing> _f = null!; }\n",
        "public class WithProp  { public Box<IThing> P { get; set; } = null!; }\n"
    ));
    let of = |owner: &str| -> Vec<&String> {
        refs.iter()
            .filter(|(s, _)| s == owner)
            .map(|(_, t)| t)
            .collect()
    };
    assert_eq!(
        of("WithField"),
        of("WithProp"),
        "field and property disagree"
    );
    assert!(!of("WithField").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_nested_generic_arguments() {
    let refs = field_refs("public class Probe { private Box<Box<IThing>> _f = null!; }\n");
    assert!(
        refs.contains(&("Probe".to_string(), "IThing".to_string())),
        "innermost generic argument must be linked"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_multiple_declarators_share_the_type() {
    let refs = field_refs("public class Probe { private Box<IThing> _a = null!, _b = null!; }\n");
    assert!(refs.contains(&("Probe".to_string(), "IThing".to_string())));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_bare_type_parameter_is_not_fabricated() {
    let ext = run(&[(
        "P.cs",
        "public class Probe<T> { private T _item = default!; }\n",
    )]);
    assert!(
        !pairs(&ext, "references").iter().any(|(_, t)| t == "T"),
        "type parameter must not become a node"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_type_parameter_as_generic_argument_is_not_fabricated() {
    let refs = field_refs("public class Probe<T> { private Box<T> _f = null!; }\n");
    assert!(
        refs.contains(&("Probe".to_string(), "Box".to_string())),
        "outer type is still a real reference"
    );
    assert!(
        !refs.iter().any(|(_, t)| t == "T"),
        "type parameter must not become a node"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_predefined_types_are_not_fabricated() {
    let refs = field_refs(
        "public class Probe { private Box<int> _f = null!; private string _s = \"\"; }\n",
    );
    assert!(
        !refs.iter().any(|(_, t)| t == "int" || t == "string"),
        "builtin types must not become nodes"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_plain_field_still_links() {
    let refs = field_refs("public class Probe { private IThing _f = null!; }\n");
    assert!(refs.contains(&("Probe".to_string(), "IThing".to_string())));
}

// ---- upstream `test_csharp_delegate_declarations.py` (0 of 2 ported) ----
// Both tests need a node for a `delegate` declaration. Go lacks it: `csItems` handles class,
// struct, interface, enum and record declarations only, so `BinOp` is never a sourced node and
// the reference to it ends on a sourceless stub.

// ---- upstream `test_csharp_enum_members.py` (3 of 12 ported) ----
// Not ported: the ten tests that read a node per enum member or a `case_of` edge (Go lacks
// enum member nodes and the `case_of` relation: its type body walk only reads methods, fields
// and properties). Of the delegate-field test, which also needs a member node, nothing is left.

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_the_enum_type_node_is_kept() {
    // The enum type stays a node and stays contained by its file.
    let ext = run(&[("S.cs", "public enum ProcessingType {\n    Process\n}\n")]);
    let enum_nodes = defs(&ext, "ProcessingType");
    assert_eq!(enum_nodes.len(), 1);
    let contained = ext
        .edges
        .iter()
        .any(|e| e.relation.as_str() == "contains" && e.target == enum_nodes[0].id);
    assert!(contained, "enum type is not contained by its file");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_an_empty_enum_emits_no_members() {
    let ext = run(&[("S.cs", "public enum Empty {\n}\n")]);
    assert!(!defs(&ext, "Empty").is_empty());
    assert!(pairs(&ext, "case_of").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_element_access_member_call_still_binds_to_the_real_method_not_an_enum_case() {
    // Upstream also looks up the enum member `Pause` through `case_of` and asserts the call
    // does not land on it; Go has no enum member nodes, so only the call to the real method
    // is asserted.
    let ext = run(&[(
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
    )]);
    assert!(
        has_edge(&ext, "Injector.StopAll()", "calls", "Handle.Pause()"),
        "StopAll must call Handle.Pause"
    );
}

// ---- upstream `test_csharp_verbatim_interpolation.py` (0 of 8 ported) ----
// Six tests call Python's `_normalize_csharp_verbatim_interpolation` or
// `_csharp_broken_escape_offsets` (python internals). The other two extract a file with the
// `$@"a""{e}"""` literal and need the members after it; Go lacks the byte-length-preserving mask
// that makes the grammar accept the literal, so they are not ported.

// ---- upstream `test_csharp_type_resolution.py` (24 of 60 ported) ----
// Not ported: node metadata (namespace, scope_chain, using_kind, ref_token, qualified,
// is_nested_type) and namespace nodes (Go lacks them); node ids that carry the namespace or the
// arity (Go ids are file + name, so `A.T` and `B.T` or `Effect` and `Effect<T>` are one node);
// namespace-aware binding of a type ref (Go binds by name: the file's own definition, else the
// unique one of the corpus, else a stub per file); `using` alias and `using static` binding of
// type refs; nested type nodes; one shared external stub; `CsharpNameResolver`.

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_cross_file_inherits_resolves_to_real_def() {
    let ext = run(&[
        (
            "core.cs",
            "namespace Game.Core { public class Damage { public int Calc() { return 1; } } }\n",
        ),
        (
            "combat.cs",
            "using Game.Core;\nnamespace Game.Combat { public class Weapon : Damage {} }\n",
        ),
    ]);
    let damage = targets(&ext, "inherits", "Damage");
    assert!(!damage.is_empty(), "expected an inherits edge to Damage");
    assert!(
        damage.iter().all(|d| !d.source_file.is_empty()),
        "Weapon : Damage must resolve to the real Damage def, not a shadow stub"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_global_using_and_global_namespace() {
    let ext = run(&[
        ("gadget.cs", "public class Gadget {}\n"),
        (
            "user.cs",
            "global using System;\npublic class Widget : Gadget {}\n",
        ),
    ]);
    let g = targets(&ext, "inherits", "Gadget");
    assert!(!g.is_empty(), "expected an inherits edge to Gadget");
    assert!(
        g.iter().all(|x| !x.source_file.is_empty()),
        "Widget : Gadget must resolve; `global using` must not break parsing"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_cross_namespace_enum_reference_resolves_to_real_def() {
    let ext = run(&[
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
    ]);
    let element_defs = defs(&ext, "Element");
    assert!(
        !element_defs.is_empty(),
        "enum Element should be emitted as a real type definition node"
    );
    assert!(
        element_defs
            .iter()
            .all(|n| n.source_file.contains("core.cs"))
    );

    let element_refs: Vec<&Node> = targets(&ext, "references", "Element")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(
        !element_refs.is_empty(),
        "Element field reference should resolve to the enum definition"
    );
    assert!(
        element_refs
            .iter()
            .all(|n| n.source_file.contains("core.cs"))
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_cross_namespace_struct_and_record_references_resolve() {
    let ext = run(&[
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
    ]);
    for label in ["Coord", "Player"] {
        assert!(
            !defs(&ext, label).is_empty(),
            "{label} should be emitted as a real type definition node"
        );
        let resolved: Vec<&Node> = targets(&ext, "references", label)
            .into_iter()
            .filter(|n| !n.source_file.is_empty())
            .collect();
        assert!(
            !resolved.is_empty(),
            "{label} field reference should resolve to the real definition"
        );
        assert!(resolved.iter().all(|n| n.source_file.contains("core.cs")));
    }
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_ambiguous_using_does_not_resolve() {
    // WeaponData is defined in two files and the referrer opens both namespaces: the reference
    // must stay on a stub rather than bind an arbitrary definition.
    let ext = run(&[
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
    ]);
    let wd_refs = targets(&ext, "references", "WeaponData");
    assert!(
        !wd_refs.is_empty(),
        "expected a WeaponData reference edge (otherwise the test is vacuous)"
    );
    assert!(
        wd_refs.iter().all(|n| n.source_file.is_empty()),
        "ambiguous WeaponData must NOT resolve to either def"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_type_parameter_emits_no_reference() {
    let ext = run(&[(
        "a.cs",
        "namespace N { class T {} class Box<T> { T value; } }\n",
    )]);
    assert!(
        !defs(&ext, "T").is_empty(),
        "the real N.T must exist for the test to mean anything"
    );
    for relation in ["references", "inherits", "implements"] {
        let to_t = targets_from(&ext, relation, "Box")
            .into_iter()
            .filter(|n| n.label == "T")
            .count();
        assert_eq!(
            to_t, 0,
            "type parameter T must not produce a {relation} to the real N.T"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_qualified_ref_known_namespace_resolves() {
    let ext = run(&[
        ("n.cs", "namespace N { class T {} }\n"),
        ("m.cs", "namespace M { class Use : N.T {} }\n"),
    ]);
    let t = defs(&ext, "T");
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].source_file, "n.cs");
    assert!(
        has_edge(&ext, "Use", "inherits", "T"),
        "M.Use : N.T must bind N.T"
    );
    assert!(
        targets(&ext, "inherits", "T")
            .iter()
            .all(|n| n.source_file == "n.cs")
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_qualified_generic_resolves_to_real_def() {
    let ext = run(&[(
        "g.cs",
        "namespace N { class Box<TI> {} class Use { N.Box<int> b; } }\n",
    )]);
    let box_defs = defs(&ext, "Box");
    assert_eq!(box_defs.len(), 1);
    let refs = targets_from(&ext, "references", "Use");
    assert!(
        refs.iter().any(|n| n.id == box_defs[0].id),
        "N.Box<int> field must resolve to the real N.Box def"
    );
    assert!(
        !ext.nodes.iter().any(|n| n.label.contains('<')),
        "no node should carry a junk generic label"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_qualified_alias_namespace_resolves() {
    // using B = X.Y (namespace alias) then B.T -> the type T in namespace X.Y.
    let ext = run(&[
        ("n.cs", "namespace X.Y { class T {} }\n"),
        (
            "m.cs",
            "using B = X.Y;\nnamespace M { class Use : B.T {} }\n",
        ),
    ]);
    assert_eq!(defs(&ext, "T").len(), 1);
    assert!(
        targets(&ext, "inherits", "T")
            .iter()
            .any(|n| n.source_file == "n.cs"),
        "B.T with `using B = X.Y;` must resolve to X.Y.T"
    );
    assert!(has_edge(&ext, "Use", "inherits", "T"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_qualified_out_of_scope_alias_falls_through_to_namespace() {
    let ext = run(&[
        ("b.cs", "namespace B { class T {} }\n"),
        (
            "m.cs",
            "namespace A { using B = X.Y; }\nnamespace M { class Use : B.T {} }\n",
        ),
    ]);
    assert_eq!(defs(&ext, "T").len(), 1);
    assert!(
        targets(&ext, "inherits", "T")
            .iter()
            .any(|n| n.source_file == "b.cs"),
        "out-of-scope alias B must fall through to namespace B"
    );
    assert!(has_edge(&ext, "Use", "inherits", "T"));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_nested_type_not_importable_via_using() {
    // Go has no node for a nested type, so the reference stays on a stub.
    let ext = run(&[
        ("a.cs", "namespace N { class Outer { class Inner {} } }\n"),
        ("b.cs", "using N;\nnamespace M { class Use { Inner x; } }\n"),
    ]);
    let resolved: Vec<&Node> = targets(&ext, "references", "Inner")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(
        resolved.is_empty(),
        "nested Inner must not resolve via `using N;`"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_type_ref_never_targets_a_file_label() {
    let ext = run(&[
        ("core.cs", "namespace N { class Box {} }\n"),
        ("b.cs", "using B = N.Box;\nclass Use : B {}\n"),
    ]);
    for relation in ["inherits", "implements", "references"] {
        for (_, target) in pairs(&ext, relation) {
            assert!(
                !target.ends_with(".cs"),
                "a C# type ref must not target a .cs file-labeled node: {target}"
            );
        }
    }
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_file_level_using_applies_across_blocks() {
    let ext = run(&[
        ("n.cs", "namespace N { class T {} }\n"),
        (
            "u.cs",
            "using N;\nnamespace A { class X : T {} } namespace B { class Y : T {} }\n",
        ),
    ]);
    let resolved = targets(&ext, "inherits", "T")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .count();
    assert!(
        resolved >= 2,
        "file-level using N must reach both A.X and B.Y: {resolved}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_using_flows_into_nested_block() {
    let ext = run(&[
        ("n.cs", "namespace N { class T {} }\n"),
        (
            "u.cs",
            "namespace A { using N; namespace B { class Inner : T {} } }\n",
        ),
    ]);
    let resolved = targets(&ext, "inherits", "T")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .count();
    assert!(
        resolved > 0,
        "using N in outer block A must flow into nested block B"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_same_file_enum_member_does_not_preempt_class_inheritance() {
    // Upstream also finds the enum member node `EffectKind.Follow` and asserts FollowFloat does
    // not inherit it; Go has no enum member nodes, so only the edge to the class is asserted.
    let ext = run(&[(
        "effects.cs",
        concat!(
            "namespace Demo {\n",
            "    public enum EffectKind { Tween, Follow }\n",
            "    public abstract class Follow { }\n",
            "    public sealed class FollowFloat : Follow { }\n",
            "}\n"
        ),
    )]);
    assert_eq!(defs(&ext, "Follow").len(), 1);
    assert!(
        has_edge(&ext, "FollowFloat", "inherits", "Follow"),
        "FollowFloat must inherit from class Follow"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_enum_member_cannot_steal_external_parameter_type() {
    // Upstream also finds the member node `Channel.Vector2` and asserts the reference does not
    // end on it; Go has no enum member nodes, so only the stub is asserted.
    let ext = run(&[
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
    ]);
    let move_refs = targets_from(&ext, "references", "Mover.Move()");
    assert!(
        move_refs
            .iter()
            .any(|n| n.label == "Vector2" && n.source_file.is_empty()),
        "Vector2 parameter must remain on an external stub"
    );
    assert!(defs(&ext, "Vector2").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_property_cannot_steal_external_parameter_type() {
    // Upstream also finds the property node `Node.Type` and asserts the reference does not end
    // on it; Go has no property nodes, so only the stub is asserted.
    let ext = run(&[
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
    ]);
    let describe_refs = targets_from(&ext, "references", "Mover.Describe()");
    assert!(
        describe_refs
            .iter()
            .any(|n| n.label == "Type" && n.source_file.is_empty()),
        "Type parameter must remain on an external stub"
    );
    assert!(defs(&ext, "Type").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_cross_file_collision_resolved_by_kind_not_filename_order() {
    // Upstream also finds the enum member node `Follow` in A_Enum.cs and asserts FollowFloat
    // does not inherit it; Go has no enum member nodes, so only the edge to the class is asserted.
    let ext = run(&[
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
    ]);
    let follow = defs(&ext, "Follow");
    assert_eq!(follow.len(), 1);
    assert_eq!(follow[0].source_file, "Follow.cs");
    assert!(
        targets_from(&ext, "inherits", "FollowFloat")
            .iter()
            .any(|n| n.id == follow[0].id),
        "FollowFloat must inherit from class Follow despite A_Enum.cs sorting first"
    );
}

/// The checks the three `test_csharp_enclosing_namespace_*` cases that share a shape make:
/// `Registry.Item` references `Widget`, `Factory : Base`, `Factory.Make()` calls `new Widget()`,
/// and no `Widget` or `Base` is left as a sourceless stub. `types_file` defines both types.
fn check_widget_base(ext: &Extraction, types_file: &str) {
    let widget = defs(ext, "Widget");
    let base = defs(ext, "Base");
    assert_eq!(widget.len(), 1);
    assert_eq!(widget[0].source_file, types_file);
    assert_eq!(base.len(), 1);
    assert_eq!(base[0].source_file, types_file);

    // Inherits: Factory : Base
    let inherits: Vec<&Node> = targets(ext, "inherits", "Base")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(!inherits.is_empty() && inherits[0].id == base[0].id);

    // References: Registry.Item -> Widget
    let refs: Vec<&Node> = targets(ext, "references", "Widget")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(!refs.is_empty() && refs[0].id == widget[0].id);

    // Calls: Factory.Make() -> new Widget()
    assert!(
        targets_from(ext, "calls", "Factory.Make()")
            .iter()
            .any(|n| n.id == widget[0].id),
        "Factory.Make must call Widget"
    );

    // No orphan shadow stubs.
    assert!(stubs(ext, "Widget").is_empty(), "shadow stub for Widget");
    assert!(stubs(ext, "Base").is_empty(), "shadow stub for Base");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_enclosing_namespace_dotted() {
    let ext = run(&[
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
    ]);
    check_widget_base(&ext, "Demo.cs");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_enclosing_namespace_nested_blocks() {
    let ext = run(&[(
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
    )]);
    check_widget_base(&ext, "nested.cs");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_enclosing_namespace_file_scoped() {
    let ext = run(&[
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
    ]);
    check_widget_base(&ext, "Demo.cs");
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_enclosing_namespace_regression_controls() {
    // Upstream picks `CollidingType` of `Demo.Inner` by its namespace metadata and asserts the
    // reference binds it rather than `Demo.CollidingType`. Go has no metadata and gives both
    // declarations one node (one id per file and name), so only the binding to that one
    // definition is asserted.
    let ext = run(&[(
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
    )]);
    let colliding = defs(&ext, "CollidingType");
    let imported_type = defs(&ext, "ImportedType");
    assert_eq!(imported_type.len(), 1);

    let ref_colliding: Vec<&Node> = targets(&ext, "references", "CollidingType")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(!ref_colliding.is_empty() && ref_colliding.iter().all(|r| r.id == colliding[0].id));

    // ImportedType reference must bind to Other.ImportedType.
    let ref_imported: Vec<&Node> = targets(&ext, "references", "ImportedType")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(!ref_imported.is_empty() && ref_imported.iter().all(|r| r.id == imported_type[0].id));

    // UnknownType must remain a sourceless stub.
    let ref_missing = targets(&ext, "references", "UnknownType");
    assert!(!ref_missing.is_empty() && ref_missing.iter().all(|r| r.source_file.is_empty()));
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_unimported_nested_type_dangles() {
    // Go has no node for the nested Record, so the reference stays on a stub.
    let ext = run(&[
        (
            "layer.cs",
            "namespace Demo\n{\n    public class Layer\n    {\n        public class Record {}\n    }\n}\n",
        ),
        (
            "consumer.cs",
            "namespace Consumer\n{\n    public class Service\n    {\n        public Record value;\n    }\n}\n",
        ),
    ]);
    let resolved: Vec<&Node> = targets(&ext, "references", "Record")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(
        resolved.is_empty(),
        "unimported nested Record must not resolve"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.28"]
fn test_csharp_ambiguous_static_imports_dangle() {
    // Go has no node for either nested Record, so the reference stays on a stub.
    let ext = run(&[
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
    ]);
    let resolved: Vec<&Node> = targets(&ext, "references", "Record")
        .into_iter()
        .filter(|n| !n.source_file.is_empty())
        .collect();
    assert!(
        resolved.is_empty(),
        "ambiguous static imports must fail closed and remain unresolved"
    );
}
