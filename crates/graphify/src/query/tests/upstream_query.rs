//! Upstream graphify v0.9.107 tests of `query` (Go: `ask`) and `explain`, as graphify-go
//! v0.12.0 answers them: `tests/test_query_deterministic_order.py` (2 of 2),
//! `tests/test_query_seed_order.py` (1 of 5), `tests/test_query_induced_edges.py` (11 of 11),
//! `tests/test_query_source_first.py` (3 of 10), `test_truncation_full_answer.py` (1 of 4).
//!
//! Upstream's `graphify query "<question>"` is Go's `ask` (Go's `query` is the regexp search),
//! and its `_bfs`/`_dfs(G, seeds, depth)` are Go's `bfs_traverse`/`dfs_traverse` over
//! `relation_adj`. An upstream `context_filter` (`call`, `import`) is Go's relation list
//! (`calls`, `imports`). Rows marked "pins Go v0.12.0" assert Go's value where upstream asserts
//! another.

use std::collections::BTreeSet;

use super::{link_json, load_str, node_json};
use crate::cli::main::explain_lines;
use crate::query::ask::{
    Traversal, bfs_traverse, dfs_traverse, query_terms, relation_adj, score_nodes,
};
use crate::query::{Graph, ask, explain};

/// A networkx `node_link_data` graph.json of an undirected simple graph.
fn nx_graph(nodes: &[String], links: &[String]) -> String {
    format!(
        r#"{{"directed": false, "multigraph": false, "graph": {{}}, "nodes": [{}], "links": [{}]}}"#,
        nodes.join(", "),
        links.join(", ")
    )
}

/// A node with upstream's `label`, `source_file` and `source_location` (none when empty).
fn nx_node(id: &str, label: &str, file: &str, loc: &str) -> String {
    let mut s = format!(r#"{{"id": "{id}", "label": "{label}""#);
    if !file.is_empty() {
        s += &format!(r#", "source_file": "{file}""#);
    }
    if !loc.is_empty() {
        s += &format!(r#", "source_location": "{loc}""#);
    }
    s + "}"
}

/// A link with a relation and confidence.
fn nx_link(src: &str, tgt: &str, rel: &str, conf: &str) -> String {
    format!(
        r#"{{"source": "{src}", "target": "{tgt}", "relation": "{rel}", "confidence": "{conf}"}}"#
    )
}

// ---- tests/test_query_deterministic_order.py ----

/// Upstream `_write_graph`: one seed with 60 neighbours, each with three leaves.
fn widget_graph() -> Graph {
    let mut nodes = vec![nx_node("seed", "widget", "widget.py", "L1")];
    let mut links = Vec::new();
    for i in 0..60 {
        let mid = format!("mid_{i}");
        nodes.push(nx_node(
            &mid,
            &format!("helper_{i}"),
            &format!("h{i}.py"),
            "L1",
        ));
        links.push(nx_link("seed", &mid, "calls", "EXTRACTED"));
        for j in 0..3 {
            let leaf = format!("leaf_{i}_{j}");
            nodes.push(nx_node(
                &leaf,
                &leaf,
                &format!("h{i}.py"),
                &format!("L{}", j + 2),
            ));
            links.push(nx_link(&mid, &leaf, "contains", "EXTRACTED"));
        }
    }
    load_str(&nx_graph(&nodes, &links))
}

/// Upstream runs `query widget --budget 20000` under three hash seeds; Go has no hash seed, so
/// the same answer three times in one process is the check.
fn same_answer_three_times(dfs: bool) {
    let g = widget_graph();
    let outs: BTreeSet<String> = (0..3)
        .map(|_| ask(&g, "widget", dfs, 2, 20000, &[]))
        .collect();
    assert_eq!(outs.len(), 1, "dfs={dfs}: answers differ");
    let out = outs.first().expect("one answer");
    assert!(out.contains("EDGE"), "dfs={dfs}: no EDGE in {out}");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_bfs_query_output_does_not_depend_on_hash_seed() {
    same_answer_three_times(false);
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_dfs_query_output_does_not_depend_on_hash_seed() {
    same_answer_three_times(true);
}

// ---- tests/test_query_seed_order.py ----

/// `test_query_prints_the_stronger_guaranteed_seed_first_and_is_deterministic`. Go has no
/// per-term seed guarantee: the four exact matches tie on score, the gap window keeps three, and
/// the shorter labels (then ids) win.
// pins Go v0.12.0 (Start: [alpha, bravo, zebra]); upstream 0.9.107 also seeds charlie and apple
// through the per-term guarantee, charlie first -> P3 (B65)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_query_prints_the_stronger_guaranteed_seed_first_and_is_deterministic() {
    let mut nodes = Vec::new();
    for name in ["alpha", "bravo", "charlie", "zebra", "apple"] {
        nodes.push(nx_node(name, name, &format!("{name}.py"), "L1"));
    }
    for i in 0..8 {
        nodes.push(nx_node(
            &format!("apple{i}"),
            &format!("apple_util_{i}"),
            &format!("apples{i}.py"),
            "L1",
        ));
    }
    let g = load_str(&nx_graph(&nodes, &[]));
    let q = "alpha bravo charlie apple zebra";
    let text = ask(&g, q, false, 1, 2000, &[]);
    assert_eq!(text, ask(&g, q, false, 1, 2000, &[]), "deterministic");
    assert_eq!(
        text.split('\n').next(),
        Some("Traversal: BFS depth=1 | Start: [alpha, bravo, zebra] | 3 nodes found"),
        "{text}"
    );
}

// ---- tests/test_query_induced_edges.py ----

/// Upstream `_add`: label = id, `<id>.py`, L1, community 0.
fn add(nodes: &mut Vec<String>, names: &[&str]) {
    for n in names {
        nodes.push(node_json(n, n, &format!("{n}.py"), "L1"));
    }
}

/// Upstream `_link`: EXTRACTED, with a `context` Go ignores.
fn link(links: &mut Vec<String>, a: &str, b: &str, relation: &str, context: &str) {
    let extra = format!(r#","context":"{context}""#);
    links.push(link_json(a, b, relation, "EXTRACTED", "", "", &extra));
}

/// `_bfs` / `_dfs` over the whole graph (`want` = `None`) or the links of the relations in it.
fn traverse(
    g: &Graph,
    seeds: &[&str],
    depth: i64,
    dfs: bool,
    want: Option<&BTreeSet<String>>,
) -> Traversal {
    let (adj, deg) = relation_adj(g, want);
    let seeds: Vec<String> = seeds.iter().map(|s| s.to_string()).collect();
    if dfs {
        dfs_traverse(g, &adj, &deg, &seeds, depth)
    } else {
        bfs_traverse(g, &adj, &deg, &seeds, depth)
    }
}

/// Upstream `_pairs`: the edges as unordered pairs.
fn pairs(edges: &[(String, String)]) -> BTreeSet<(String, String)> {
    edges
        .iter()
        .map(|(u, v)| {
            if u <= v {
                (u.clone(), v.clone())
            } else {
                (v.clone(), u.clone())
            }
        })
        .collect()
}

/// An unordered pair.
fn pair(a: &str, b: &str) -> (String, String) {
    pairs(&[(a.to_string(), b.to_string())])
        .pop_first()
        .expect("one pair")
}

/// A set of ids.
fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

/// The seed-hub-hub graph of the two hub tests.
fn hubs_graph() -> Graph {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["seed", "hub_a", "hub_b"]);
    link(&mut links, "seed", "hub_a", "calls", "call");
    link(&mut links, "seed", "hub_b", "calls", "call");
    link(&mut links, "hub_a", "hub_b", "calls", "call");
    for i in 0..60 {
        let (a, b) = (format!("a{i}"), format!("b{i}"));
        add(&mut nodes, &[a.as_str(), b.as_str()]);
        link(&mut links, "hub_a", &a, "calls", "call");
        link(&mut links, "hub_b", &b, "calls", "call");
    }
    load_str(&nx_graph(&nodes, &links))
}

/// The n1-n2-n3 triangle.
fn triangle() -> Graph {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["n1", "n2", "n3"]);
    link(&mut links, "n1", "n2", "calls", "call");
    link(&mut links, "n1", "n3", "calls", "call");
    link(&mut links, "n2", "n3", "calls", "call");
    load_str(&nx_graph(&nodes, &links))
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_bfs_records_edge_between_two_seeds() {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["checkout", "discounted_total"]);
    link(&mut links, "checkout", "discounted_total", "calls", "call");
    let g = load_str(&nx_graph(&nodes, &links));
    let (visited, edges) = traverse(&g, &["checkout", "discounted_total"], 1, false, None);
    assert_eq!(visited, set(&["checkout", "discounted_total"]), "visited");
    assert_eq!(
        pairs(&edges),
        BTreeSet::from([pair("checkout", "discounted_total")]),
        "edges {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_bfs_records_cross_edge_in_a_triangle() {
    let g = triangle();
    let (visited, edges) = traverse(&g, &["n1"], 2, false, None);
    assert_eq!(visited, set(&["n1", "n2", "n3"]), "visited");
    let induced: BTreeSet<(String, String)> = g
        .links
        .iter()
        .filter(|l| visited.contains(&l.source) && visited.contains(&l.target))
        .map(|l| pair(&l.source, &l.target))
        .collect();
    assert_eq!(pairs(&edges), induced, "edges {edges:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_bfs_records_edge_between_two_visited_hubs() {
    let g = hubs_graph();
    let (visited, edges) = traverse(&g, &["seed"], 1, false, None);
    assert_eq!(visited, set(&["seed", "hub_a", "hub_b"]), "visited");
    assert!(
        pairs(&edges).contains(&pair("hub_a", "hub_b")),
        "edges {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_dfs_records_edge_between_two_visited_hubs() {
    let g = hubs_graph();
    let (visited, edges) = traverse(&g, &["seed"], 1, true, None);
    assert_eq!(visited, set(&["seed", "hub_a", "hub_b"]), "visited");
    assert!(
        pairs(&edges).contains(&pair("hub_a", "hub_b")),
        "edges {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_traversal_edges_keep_discovery_order_and_come_first() {
    let g = triangle();
    let (_, edges) = traverse(&g, &["n1"], 2, false, None);
    let first: Vec<(&str, &str)> = edges
        .iter()
        .take(2)
        .map(|(u, v)| (u.as_str(), v.as_str()))
        .collect();
    assert_eq!(first, [("n1", "n2"), ("n1", "n3")], "edges {edges:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_no_duplicate_edges_are_returned() {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["n1", "n2", "n3", "n4"]);
    for (a, b) in [
        ("n1", "n2"),
        ("n1", "n3"),
        ("n2", "n3"),
        ("n2", "n4"),
        ("n3", "n4"),
    ] {
        link(&mut links, a, b, "calls", "call");
    }
    let g = load_str(&nx_graph(&nodes, &links));
    for dfs in [false, true] {
        let (_, edges) = traverse(&g, &["n1"], 3, dfs, None);
        assert_eq!(edges.len(), pairs(&edges).len(), "dfs={dfs}: {edges:?}");
    }
}

/// The completion pass scans the filtered adjacency: upstream's `context` `call` is Go's
/// relation `calls`.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_completion_respects_the_context_filter() {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["n1", "n2", "n3"]);
    link(&mut links, "n1", "n2", "calls", "call");
    link(&mut links, "n1", "n3", "imports", "import");
    link(&mut links, "n2", "n3", "imports", "import");
    let g = load_str(&nx_graph(&nodes, &links));
    let want = set(&["calls"]);
    let (_, edges) = traverse(&g, &["n1", "n2", "n3"], 1, false, Some(&want));
    assert_eq!(
        pairs(&edges),
        BTreeSet::from([pair("n1", "n2")]),
        "an import edge came back: {edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_self_loops_are_not_introduced() {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["recurse", "caller"]);
    link(&mut links, "recurse", "recurse", "calls", "call");
    link(&mut links, "caller", "recurse", "calls", "call");
    let g = load_str(&nx_graph(&nodes, &links));
    let (_, edges) = traverse(&g, &["caller", "recurse"], 1, false, None);
    assert_eq!(
        pairs(&edges),
        BTreeSet::from([pair("caller", "recurse")]),
        "edges {edges:?}"
    );
}

/// Go's adjacency is undirected, so the traversal records the ping-pong pair once; the renderer
/// then prints every link of the pair (`test_ask_mutual_context_self_loop`).
// pins Go v0.12.0 ([(ping, pong)]); upstream 0.9.107 records both (ping, pong) and (pong, ping)
// on a DiGraph -> P3
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_directed_graph_keeps_both_directions_of_a_mutual_edge() {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["ping", "pong"]);
    link(&mut links, "ping", "pong", "calls", "call");
    link(&mut links, "pong", "ping", "calls", "call");
    let g = load_str(&nx_graph(&nodes, &links));
    let (_, edges) = traverse(&g, &["ping", "pong"], 1, false, None);
    assert_eq!(edges, [("ping".to_string(), "pong".to_string())], "edges");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_directed_graph_renders_the_seed_to_seed_edge() {
    let (mut nodes, mut links) = (Vec::new(), Vec::new());
    add(&mut nodes, &["checkout", "discounted_total"]);
    link(&mut links, "checkout", "discounted_total", "calls", "call");
    let g = load_str(&nx_graph(&nodes, &links));
    let text = ask(&g, "checkout discounted_total", false, 1, 2000, &[]);
    assert!(text.contains("EDGE checkout --calls"), "{text}");
    assert!(text.contains("discounted_total"), "{text}");
}

/// `test_query_cli_renders_the_edge_between_two_seeds`: upstream's CLI `query` is Go's `ask`
/// (depth 2, budget 2000, as `cmd_ask` passes them).
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_query_cli_renders_the_edge_between_two_seeds() {
    let nodes = [
        r#"{"id": "app.py::checkout", "label": "checkout", "source_file": "app.py", "source_location": "L4", "community": 0}"#.to_string(),
        r#"{"id": "pricing.py::discounted_total", "label": "discounted_total", "source_file": "pricing.py", "source_location": "L1", "community": 0}"#.to_string(),
    ];
    let links = [r#"{"relation": "calls", "confidence": "EXTRACTED", "context": "call", "source_file": "app.py", "source_location": "L5", "source": "app.py::checkout", "target": "pricing.py::discounted_total"}"#.to_string()];
    let g = load_str(&nx_graph(&nodes, &links));
    let out = ask(&g, "checkout discounted_total", false, 2, 2000, &[]);
    assert!(out.contains("NODE checkout"), "{out}");
    assert!(out.contains("NODE discounted_total"), "{out}");
    let (_, after) = out
        .split_once("EDGE checkout --calls")
        .unwrap_or_else(|| panic!("no checkout edge: {out}"));
    assert!(after.contains("discounted_total"), "{out}");
}

// ---- tests/test_query_source_first.py ----

/// Upstream `_graph`: one name in source, test, doc, config and an external symbol.
fn source_first_graph(extra: &[String]) -> Graph {
    let mut nodes = vec![
        nx_node(
            "src",
            "validate_username()",
            "pkg/auth/validators.py",
            "L10",
        ),
        nx_node(
            "tst",
            "validate_username()",
            "tests/auth/test_validators.py",
            "L5",
        ),
        nx_node("doc", "validate_username", "docs/auth.md", "L1"),
        nx_node("cfg", "username", "pkg/settings.yml", "L1"),
        nx_node("ext", "username", "", ""),
    ];
    nodes.extend_from_slice(extra);
    let links = [nx_link("tst", "src", "calls", "EXTRACTED")];
    load_str(&nx_graph(&nodes, &links))
}

/// Upstream `_nodes_in_order`: the `src=` of each NODE line.
fn nodes_in_order(text: &str) -> Vec<&str> {
    text.split('\n')
        .filter(|l| l.starts_with("NODE "))
        .filter_map(|l| l.split_once(" [src=").map(|(_, r)| r))
        .map(|r| r.split(' ').next().unwrap_or(""))
        .collect()
}

/// `test_query_answer_leads_with_source_not_test`. Go weights no node down: the three exact
/// matches tie, the shorter label (the doc's) leads, and the test's homonym is deduped.
// pins Go v0.12.0 (docs/auth.md first); upstream 0.9.107 leads with pkg/auth/validators.py,
// weighting tests and docs down -> P3 (B51)
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_query_answer_leads_with_source_not_test() {
    let g = source_first_graph(&[]);
    let text = ask(
        &g,
        "validate_username allows a trailing newline",
        false,
        1,
        2000,
        &[],
    );
    assert_eq!(
        nodes_in_order(&text).first(),
        Some(&"docs/auth.md"),
        "{text}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_per_term_seed_guarantee_still_seeds_a_term_only_tests_match() {
    let g = source_first_graph(&[nx_node("fx", "fixture_loader()", "tests/helpers.py", "L3")]);
    let text = ask(&g, "validate_username fixture_loader", false, 1, 2000, &[]);
    let start = text
        .split_once("Start: ")
        .and_then(|(_, r)| r.split('|').next())
        .unwrap_or_else(|| panic!("no Start: {text}"));
    assert!(
        start.contains("fixture_loader()") && start.contains("validate_username()"),
        "{start}"
    );
}

/// `test_path_and_explain_scoring_is_unchanged`, the `_score_nodes` half (Go has no
/// `_score_query`): names alone cannot tell the source from its test.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_path_and_explain_scoring_is_unchanged() {
    let g = source_first_graph(&[]);
    let scored = score_nodes(&g, &query_terms("validate_username"));
    let score = |id: &str| {
        scored
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.score)
            .unwrap_or_else(|| panic!("{id} not scored: {scored:?}"))
    };
    assert_eq!(score("tst"), score("src"), "{scored:?}");
}

// ---- test_truncation_full_answer.py ----

/// `test_explain_cut_names_the_total_and_the_option`: 25 callers of `hub`, as Go's `explain`
/// renders them (`query::explain`, then `explainLines`): 20 in full (label order, all degree
/// 1), the other 5 grouped by file.
// pins Go v0.12.0 ("  ... 5 more connections. Grouped by file:" and the per-file counts);
// upstream 0.9.107 prints "Connections (25):" and "... and 5 more (--all lists all 25)" -> P3
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn upstream_test_explain_cut_names_the_total_and_the_option() {
    let mut nodes =
        vec![r#"{"id": "hub", "label": "hub()", "source_file": "lib/hub.py"}"#.to_string()];
    let mut links = Vec::new();
    for i in 0..25 {
        let fpath = format!("app/mod{}.py", i % 3);
        nodes.push(format!(
            r#"{{"id": "caller_{i}", "label": "caller_{i}()", "source_file": "{fpath}"}}"#
        ));
        links.push(format!(
            r#"{{"source": "caller_{i}", "target": "hub", "relation": "calls", "confidence": "EXTRACTED", "source_file": "{fpath}", "source_location": "L{}"}}"#,
            10 + i
        ));
    }
    let g = load_str(&nx_graph(&nodes, &links));
    let ex = explain(&g, "hub").unwrap_or_else(|e| panic!("{e}"));
    let lines = explain_lines(&ex.neighbors);
    let full = |label: &str, loc: &str| format!("  <- {:<12} {:<32} {}", "calls", label, loc);
    let group = |file: &str, n: i64| format!("    <- {file:<48} {n}");
    assert_eq!(lines.len(), 24, "{lines:#?}");
    assert_eq!(lines[0], full("caller_0()", "app/mod0.py:10"), "{lines:#?}");
    assert_eq!(
        lines[19],
        full("caller_4()", "app/mod1.py:14"),
        "{lines:#?}"
    );
    assert_eq!(
        lines[20..],
        [
            "  ... 5 more connections. Grouped by file:".to_string(),
            group("app/mod0.py", 2),
            group("app/mod2.py", 2),
            group("app/mod1.py", 1),
        ],
        "{lines:#?}"
    );
}
