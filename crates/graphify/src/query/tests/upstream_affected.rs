//! Upstream graphify v0.9.107 tests of `affected` and its seed resolution, as graphify-go
//! v0.12.0 answers them: `test_affected_defines.py` (10 of 10), `test_affected_member_seed.py`
//! (5 of 5), `test_affected_overrides.py` (8 of 11), `test_affected_path_file_node.py` (4 of 6),
//! `test_affected_seed_reference_forms.py` (5 of 8).
//!
//! Upstream's `affected_nodes(G, seed, depth, relations)` walks back from one node, the seed a
//! node id; Go's `affected` seeds with every node whose `source_file` is one of the changed
//! files, and has no symbol seed. So for a node-id seed Go finds nothing, whatever the graph:
//! the rows below pin that. Upstream's `resolve_seed` (an id, `path::Symbol`, a dotted name, a
//! file path) is closest to Go's `Graph::resolve`, the lookup of `explain` and `path`; the
//! resolve rows run it.

use super::load_str;
use crate::query::affected::DEP_RELATIONS;
use crate::query::{AffectedOptions, Error, Graph, affected};

/// A networkx `node_link_data` graph.json of `(id, label, source_file, source_location)` nodes
/// (empty fields left out) and `(source, target, relation)` links.
fn nx_graph(
    directed: bool,
    nodes: &[(&str, &str, &str, &str)],
    links: &[(&str, &str, &str)],
) -> Graph {
    let nodes: Vec<String> = nodes
        .iter()
        .map(|&(id, label, file, loc)| {
            let mut s = format!(r#"{{"id": "{id}", "label": "{label}""#);
            if !file.is_empty() {
                s += &format!(r#", "source_file": "{file}""#);
            }
            if !loc.is_empty() {
                s += &format!(r#", "source_location": "{loc}""#);
            }
            s + "}"
        })
        .collect();
    let links: Vec<String> = links
        .iter()
        .map(|(s, t, r)| format!(r#"{{"source": "{s}", "target": "{t}", "relation": "{r}"}}"#))
        .collect();
    load_str(&format!(
        r#"{{"directed": {directed}, "multigraph": false, "graph": {{}}, "nodes": [{}], "links": [{}]}}"#,
        nodes.join(", "),
        links.join(", ")
    ))
}

/// One upstream `affected_nodes` call: the test, the graph, the seed, the depth, the one
/// relation of `relations=` (`""`: none given).
type Row = (&'static str, Graph, &'static str, i64, &'static str);

/// Runs each row through Go's `affected` and checks Go's answer: nothing changed, nothing
/// impacted (a node id is no file).
fn assert_no_hits(rows: Vec<Row>) {
    for (name, g, seed, depth, relation) in rows {
        let opts = AffectedOptions {
            depth,
            relations: [relation]
                .iter()
                .filter(|r| !r.is_empty())
                .map(|r| r.to_string())
                .collect(),
        };
        let res = affected(&g, &[seed.to_string()], &opts);
        assert!(
            res.changed.is_empty() && res.impacted.is_empty(),
            "{name}: affected({seed:?}) = {res:?}, want no hits"
        );
    }
}

// ---- test_affected_defines.py ----

/// Upstream `_dart_style_graph` (`with_import`: the consumer's import edge).
fn dart_graph(with_import: bool) -> Graph {
    let mut links = vec![("db_file", "db_class", "defines")];
    if with_import {
        links.push(("consumer_file", "db_file", "imports"));
    }
    nx_graph(
        true,
        &[
            (
                "db_file",
                "database_wrapper.dart",
                "lib/database/database_wrapper.dart",
                "L1",
            ),
            (
                "db_class",
                "DatabaseWrapper",
                "lib/database/database_wrapper.dart",
                "",
            ),
            ("consumer_file", "app.dart", "lib/app.dart", ""),
        ],
        &links,
    )
}

/// Upstream `_python_style_graph`.
fn python_graph() -> Graph {
    nx_graph(
        true,
        &[
            ("mod_file", "module.py", "pkg/module.py", "L1"),
            ("mod_class", "MyClass", "pkg/module.py", ""),
            ("consumer_file", "consumer.py", "pkg/consumer.py", ""),
        ],
        &[
            ("mod_file", "mod_class", "defines"),
            ("consumer_file", "mod_file", "imports"),
        ],
    )
}

// pins Go v0.12.0 (no hits: a node id is not a file seed); upstream 0.9.107 walks back from the
// symbol through `defines` to its file and the file's importers -> P3 (B52)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_affected_defines() {
    assert_no_hits(vec![
        (
            "test_affected_reaches_defining_file_via_defines",
            dart_graph(true),
            "db_class",
            2,
            "",
        ),
        (
            "test_affected_reaches_importers_through_defines",
            dart_graph(true),
            "db_class",
            2,
            "",
        ),
        (
            "test_defines_relation_reported_in_hit",
            dart_graph(true),
            "db_class",
            2,
            "",
        ),
        (
            "test_defines_only_one_hop",
            dart_graph(false),
            "db_class",
            2,
            "",
        ),
        (
            "test_defines_respects_depth_limit",
            dart_graph(true),
            "db_class",
            1,
            "",
        ),
        (
            "test_defines_excluded_by_relation_filter",
            dart_graph(true),
            "db_class",
            2,
            "calls",
        ),
        (
            "test_file_seed_unaffected_by_defines_change",
            dart_graph(true),
            "db_file",
            2,
            "",
        ),
        (
            "test_python_module_works_too",
            python_graph(),
            "mod_class",
            2,
            "",
        ),
        (
            "test_non_defines_relations_still_work",
            nx_graph(
                true,
                &[
                    ("caller", "use()", "caller.py", ""),
                    ("target", "Foo", "target.py", ""),
                ],
                &[("caller", "target", "references")],
            ),
            "target",
            1,
            "",
        ),
    ]);
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_defines_is_in_default_affected_relations() {
    assert!(
        DEP_RELATIONS.contains(&"defines"),
        "defines must be in the default relations"
    );
}

// ---- test_affected_member_seed.py ----

/// Upstream `_g`: Runner.run calls Processor.call.
fn member_graph() -> Graph {
    nx_graph(
        true,
        &[
            ("proc", "Processor", "", ""),
            ("proc_call", ".call()", "", ""),
            ("runner", "Runner", "", ""),
            ("runner_run", ".run()", "", ""),
        ],
        &[
            ("proc", "proc_call", "method"),
            ("runner", "runner_run", "method"),
            ("runner_run", "proc_call", "calls"),
        ],
    )
}

// pins Go v0.12.0 (no hits: a node id is not a file seed, and these nodes have no file);
// upstream 0.9.107 seeds the class's members and reaches the method-bound callers -> P3 (B52)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_affected_member_seed() {
    assert_no_hits(vec![
        (
            "test_class_affected_reaches_method_bound_caller",
            member_graph(),
            "proc",
            2,
            "",
        ),
        (
            "test_member_method_node_not_reported_as_hit",
            member_graph(),
            "proc",
            2,
            "",
        ),
        (
            "test_method_contains_still_excluded_from_general_walk",
            nx_graph(
                true,
                &[
                    ("a", "A", "", ""),
                    ("a_m", ".m()", "", ""),
                    ("b", "B", "", ""),
                    ("b_m", ".n()", "", ""),
                ],
                &[
                    ("a", "a_m", "method"),
                    ("a_m", "b", "calls"),
                    ("b", "b_m", "method"),
                ],
            ),
            "a",
            3,
            "",
        ),
        (
            "test_class_level_caller_still_works",
            nx_graph(
                true,
                &[("svc", "Svc", "", ""), ("caller", ".use()", "", "")],
                &[("caller", "svc", "references")],
            ),
            "svc",
            2,
            "",
        ),
        (
            "test_file_affected_reaches_nested_method_callers",
            nx_graph(
                true,
                &[
                    ("file_p", "processor.py", "", ""),
                    ("proc", "Processor", "", ""),
                    ("proc_call", ".call()", "", ""),
                    ("runner", "Runner", "", ""),
                    ("runner_run", ".run()", "", ""),
                ],
                &[
                    ("file_p", "proc", "contains"),
                    ("proc", "proc_call", "method"),
                    ("runner", "runner_run", "method"),
                    ("runner_run", "proc_call", "calls"),
                ],
            ),
            "file_p",
            2,
            "",
        ),
    ]);
}

// ---- test_affected_overrides.py ----

/// Upstream `_graph(*edges)`: a node per endpoint, labelled `.<last part>()` when the id has a
/// dot, else the id.
fn overrides_graph(directed: bool, edges: &[(&str, &str, &str)]) -> Graph {
    let mut ids: Vec<&str> = Vec::new();
    for &(s, t, _) in edges {
        for id in [s, t] {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let labels: Vec<String> = ids
        .iter()
        .map(|id| match id.rsplit_once('.') {
            Some((_, last)) => format!(".{last}()"),
            None => id.to_string(),
        })
        .collect();
    let nodes: Vec<(&str, &str, &str, &str)> = ids
        .iter()
        .zip(&labels)
        .map(|(id, label)| (*id, label.as_str(), "", ""))
        .collect();
    nx_graph(directed, &nodes, edges)
}

/// Upstream's `Visitor`/`Impl` graph of the first and last rows.
const VISITOR: &[(&str, &str, &str)] = &[
    ("Visitor", "Visitor.head", "method"),
    ("Impl", "Impl.head", "method"),
    ("Impl", "Visitor", "implements"),
    ("walk", "Visitor.head", "calls"),
];

// pins Go v0.12.0 (no hits: a node id is not a file seed, and these nodes have no file);
// upstream 0.9.107 walks from an override to the callers of the member it overrides -> P3 (B52)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_affected_overrides() {
    assert_no_hits(vec![
        (
            "test_caller_of_the_overridden_interface_method_is_affected",
            overrides_graph(true, VISITOR),
            "Impl.head",
            2,
            "",
        ),
        (
            "test_override_through_a_superclass_chain",
            overrides_graph(
                true,
                &[
                    ("Root", "Root.close", "method"),
                    ("Mid", "Root", "inherits"),
                    ("Leaf", "Mid", "inherits"),
                    ("Leaf", "Leaf.close", "method"),
                    ("client", "Root.close", "calls"),
                    ("outer", "client", "calls"),
                ],
            ),
            "Leaf.close",
            2,
            "",
        ),
        (
            "test_overridden_member_reached_by_a_real_edge_is_reported",
            overrides_graph(
                true,
                &[
                    ("Base", "Base.read", "method"),
                    ("Base.read", "Base.read_all", "calls"),
                    ("Base", "Base.read_all", "method"),
                    ("Impl", "Base", "inherits"),
                    ("Impl", "Impl.read", "method"),
                    ("Base.read_all", "Base.read", "calls"),
                ],
            ),
            "Impl.read",
            2,
            "",
        ),
        (
            "test_recursive_overridden_member_is_a_caller_of_the_override",
            overrides_graph(
                true,
                &[
                    ("Base", "Base.m", "method"),
                    ("Base.m", "Base.m", "calls"),
                    ("Impl", "Base", "inherits"),
                    ("Impl", "Impl.m", "method"),
                ],
            ),
            "Impl.m",
            2,
            "",
        ),
        (
            "test_same_depth_keeps_the_depth_limit_exact",
            overrides_graph(
                true,
                &[
                    ("Iface", "Iface.run", "method"),
                    ("Impl", "Iface", "implements"),
                    ("Impl", "Impl.run", "method"),
                    ("a", "Iface.run", "calls"),
                    ("b", "a", "calls"),
                ],
            ),
            "Impl.run",
            1,
            "",
        ),
        (
            "test_unrelated_same_named_method_is_not_walked",
            overrides_graph(
                true,
                &[
                    ("Other", "Other.head", "method"),
                    ("Impl", "Impl.head", "method"),
                    ("walk", "Other.head", "calls"),
                ],
            ),
            "Impl.head",
            2,
            "",
        ),
        (
            "test_inheritance_cycle_terminates",
            overrides_graph(
                true,
                &[
                    ("A", "B", "inherits"),
                    ("B", "A", "inherits"),
                    ("A", "A.m", "method"),
                    ("B", "B.m", "method"),
                    ("x", "B.m", "calls"),
                ],
            ),
            "A.m",
            2,
            "",
        ),
        (
            "test_undirected_graph_is_unchanged",
            overrides_graph(false, VISITOR),
            "Impl.head",
            2,
            "",
        ),
    ]);
}

// ---- resolve: test_affected_path_file_node.py, test_affected_seed_reference_forms.py ----

/// What Go's `Graph::resolve` gives a row.
enum Want {
    NoMatch,
    /// [`Error::Ambiguous`] with these candidates.
    Ambiguous(&'static [&'static str]),
}

/// Runs `g.resolve(q)` and checks it against `want`.
fn assert_resolves(name: &str, g: &Graph, q: &str, want: &Want) {
    let got = g.resolve(q);
    match (&got, want) {
        (Ok(None), Want::NoMatch) => {}
        (Err(Error::Ambiguous(e)), Want::Ambiguous(c)) if e.candidates == *c && e.more == 0 => {}
        _ => panic!("{name}: resolve({q:?}) = {got:?}"),
    }
}

/// Upstream `_graph_with_shared_basename(extra_l1=...)`: the file node (label `alpha/hooks.py`,
/// #2032's relabel) and `check()`, plus a second node on line 1. networkx updates a node added
/// twice in place, so the one-line-definition variant has two nodes.
fn shared_basename(extra_l1: (&str, &str, &str)) -> Graph {
    let file_node = r#"{"id": "app_alpha_hooks", "label": "alpha/hooks.py", "source_file": "app/alpha/hooks.py", "source_location": "L1", "file_type": "code"}"#;
    let node = |id: &str, label: &str, loc: &str, ft: &str| {
        format!(
            r#"{{"id": "{id}", "label": "{label}", "source_file": "app/alpha/hooks.py", "source_location": "{loc}", "file_type": "{ft}"}}"#
        )
    };
    let (id, label, ft) = extra_l1;
    let nodes = if id == "app_alpha_hooks_check" {
        vec![file_node.to_string(), node(id, label, "L1", ft)]
    } else {
        vec![
            file_node.to_string(),
            node("app_alpha_hooks_check", "check()", "L4", "code"),
            node(id, label, "L1", ft),
        ]
    };
    load_str(&format!(
        r#"{{"directed": true, "multigraph": false, "graph": {{}}, "nodes": [{}], "links": []}}"#,
        nodes.join(", ")
    ))
}

/// The docstring variant: a `rationale` node on L1.
const DOCSTRING: (&str, &str, &str) = (
    "app_alpha_hooks_rationale_1",
    "Checks a request before it runs.",
    "rationale",
);

/// `test_affected_path_file_node.py`, the four graph-level tests (Go's `fileNodes` needs the
/// file node to be the lone non-heading node on L1 when its label is not the bare basename).
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_affected_path_file_node() {
    // pins Go v0.12.0 (no match: two non-heading nodes on L1 and no label "hooks.py");
    // upstream 0.9.107 resolves the file node app_alpha_hooks -> P3 (B52)
    assert_resolves(
        "test_shared_basename_with_module_docstring_on_l1_resolves_file_node",
        &shared_basename(DOCSTRING),
        "app/alpha/hooks.py",
        &Want::NoMatch,
    );
    assert_resolves(
        "test_shared_basename_with_one_line_definition_on_l1_resolves_file_node",
        &shared_basename(("app_alpha_hooks_check", "check()", "code")),
        "app/alpha/hooks.py",
        &Want::NoMatch,
    );
    // Upstream passes `root=tmp_path`; Go makes an absolute path relative to the working
    // directory, so the absolute path here is under the test process's cwd.
    let cwd = std::env::current_dir().expect("getwd");
    let abs = cwd.join("app/alpha/hooks.py");
    assert_resolves(
        "test_shared_basename_resolves_from_absolute_path",
        &shared_basename(DOCSTRING),
        abs.to_str().expect("utf-8 cwd"),
        &Want::NoMatch,
    );
    // Holds for Go: two file-shaped labels are not guessed between.
    let two = nx_graph(
        true,
        &[
            ("one", "alpha/hooks.py", "app/alpha/hooks.py", "L1"),
            ("two", "alpha/hooks.py", "app/alpha/hooks.py", "L1"),
        ],
        &[],
    );
    assert_resolves(
        "test_ambiguous_file_shaped_labels_still_refuse_to_guess",
        &two,
        "app/alpha/hooks.py",
        &Want::NoMatch,
    );
}

/// Upstream `_two_same_named_functions`.
fn two_states() -> Graph {
    load_str(
        r#"{"directed": true, "multigraph": false, "graph": {}, "nodes": [
 {"id": "pkg", "label": "pkg", "source_file": "pkg/__init__.py", "source_location": "L1"},
 {"id": "pkg_auth_mfa_state", "label": "state()", "norm_label": "state()", "source_file": "pkg/auth/mfa.py", "source_location": "L1"},
 {"id": "pkg_other_flow_state", "label": "state()", "norm_label": "state()", "source_file": "pkg/other/flow.py", "source_location": "L1"},
 {"id": "pkg_user_use", "label": "use()", "norm_label": "use()", "source_file": "pkg/user.py", "source_location": "L3"}],
"links": [{"source": "pkg_user_use", "target": "pkg_auth_mfa_state", "relation": "calls", "source_file": "pkg/user.py", "source_location": "L4"}]}"#,
    )
}

/// `test_affected_seed_reference_forms.py`, the five `resolve_seed` tests.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_affected_seed_reference_forms() {
    let g = two_states();
    // pins Go v0.12.0 (no match: `::state` must equal the label `state()`, and Go has no dotted
    // form); upstream 0.9.107 resolves pkg_auth_mfa_state -> P3 (B52)
    assert_resolves(
        "test_resolve_seed_accepts_path_scoped_symbol",
        &g,
        "pkg/auth/mfa.py::state",
        &Want::NoMatch,
    );
    assert_resolves(
        "test_resolve_seed_accepts_dotted_python_name",
        &g,
        "pkg.auth.mfa.state",
        &Want::NoMatch,
    );
    // Holds for Go: a bare name shared by two modules is not guessed; the error names both.
    const RIVALS: &[&str] = &[
        "state() (pkg/auth/mfa.py:1)",
        "state() (pkg/other/flow.py:1)",
    ];
    assert_resolves(
        "test_resolve_seed_bare_ambiguous_name_still_returns_none",
        &g,
        "state",
        &Want::Ambiguous(RIVALS),
    );
    assert_resolves(
        "test_resolve_seed_outcome_reports_rivals_for_ambiguous_name",
        &g,
        "state",
        &Want::Ambiguous(RIVALS),
    );
    assert_resolves(
        "test_resolve_seed_outcome_reports_no_rivals_for_a_miss",
        &g,
        "nothing_like_this",
        &Want::NoMatch,
    );
}
