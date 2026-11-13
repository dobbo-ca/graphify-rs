//! Port of `cycles_test.go` (3 tests), then the upstream cases of `test_analyze.py` and
//! `test_undirected_two_file_import_cycle.py` that hold for graphify-go v0.12.0, and the added
//! `deep_import_chain`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;
use std::time::Duration;

use super::{edge, names, node};
use crate::analyze::{Cycle, import_cycles};
use crate::cluster::tests::gorand::GoRand;
use crate::model::{Confidence, Graph, Relation, rel};

/// Go `importCyclesOracle`'s state: the pre-SCC `ImportCycles`, kept as the reference the pruned
/// search must match cycle for cycle.
struct Oracle {
    adj: BTreeMap<String, Vec<String>>,
    seen: BTreeSet<String>,
    cycles: Vec<Cycle>,
    max_len: i64,
    top_n: i64,
}

impl Oracle {
    fn dfs(
        &mut self,
        start: &str,
        cur: &str,
        path: &mut Vec<String>,
        visited: &mut BTreeSet<String>,
    ) {
        if path.len() as i64 > self.max_len || self.cycles.len() as i64 >= self.top_n * 10 {
            return;
        }
        let mut nbrs = self.adj.get(cur).cloned().unwrap_or_default();
        nbrs.sort();
        for nb in nbrs {
            if nb == start && path.len() >= 2 {
                if self.seen.insert(rotate_key(path)) {
                    self.cycles.push(Cycle {
                        files: path.clone(),
                    });
                }
                continue;
            }
            if nb.as_str() > start || visited.contains(&nb) {
                continue;
            }
            visited.insert(nb.clone());
            path.push(nb.clone());
            self.dfs(start, &nb, path, visited);
            path.pop();
            visited.remove(&nb);
        }
    }
}

fn import_cycles_oracle(g: &Graph, max_len: i64, top_n: i64) -> Vec<Cycle> {
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in g.edges() {
        if e.relation != Relation::IMPORTS_FROM || e.type_only() {
            continue;
        }
        let uf = g.node(&e.source).map_or("", |n| n.source_file.as_str());
        let vf = g.node(&e.target).map_or("", |n| n.source_file.as_str());
        if uf.is_empty() || vf.is_empty() || uf == vf {
            continue;
        }
        adj.entry(uf.to_string()).or_default().push(vf.to_string());
    }
    let starts: Vec<String> = adj.keys().cloned().collect();
    let mut o = Oracle {
        adj,
        seen: BTreeSet::new(),
        cycles: Vec::new(),
        max_len,
        top_n,
    };
    for s in &starts {
        o.dfs(s, s, &mut vec![s.clone()], &mut BTreeSet::from([s.clone()]));
    }
    let mut cycles = o.cycles;
    cycles.sort_by_key(|c| c.files.len());
    cycles.truncate(top_n as usize);
    cycles
}

fn rotate_key(path: &[String]) -> String {
    let (mut min, mut idx) = (&path[0], 0);
    for (i, p) in path.iter().enumerate() {
        if p < min {
            min = p;
            idx = i;
        }
    }
    let rot: Vec<&str> = path[idx..]
        .iter()
        .chain(path[..idx].iter())
        .map(String::as_str)
        .collect();
    rot.join("\0")
}

/// Go `importGraph`: one node per file and an imports_from edge per "a>b" pair.
fn import_graph(edges: &[&str]) -> Graph {
    let mut g = Graph::new();
    for e in edges {
        let (a, b) = e.split_once('>').unwrap_or((*e, ""));
        g.add_node(node(a, "", a));
        g.add_node(node(b, "", b));
        g.add_edge(edge(a, b, Relation::IMPORTS_FROM, Confidence::default()));
    }
    g
}

fn cycle_strings(cs: &[Cycle]) -> Vec<String> {
    cs.iter().map(|c| c.files.join(",")).collect()
}

/// Pins what the report has always shown on small graphs.
#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_import_cycles_pinned() {
    struct Case {
        name: &'static str,
        edges: &'static [&'static str],
        max_len: i64,
        top_n: i64,
        want: &'static [&'static str],
    }
    const CASES: [Case; 9] = [
        Case {
            name: "self loop is not a cycle",
            edges: &["a>a", "a>b"],
            max_len: 5,
            top_n: 20,
            want: &[],
        },
        Case {
            name: "isolated and acyclic nodes",
            edges: &["a>b", "b>c", "x>y"],
            max_len: 5,
            top_n: 20,
            want: &[],
        },
        // A cycle starts at its largest file and walks the import direction.
        Case {
            name: "triangle",
            edges: &["a>b", "b>c", "c>a"],
            max_len: 5,
            top_n: 20,
            want: &["c,a,b"],
        },
        Case {
            name: "disconnected components in start order",
            edges: &["y>x", "x>y", "b>a", "a>b"],
            max_len: 5,
            top_n: 20,
            want: &["b,a", "y,x"],
        },
        // Equal lengths keep discovery order: start ascending, then neighbour ascending.
        Case {
            name: "ties",
            edges: &["a>b", "b>a", "a>c", "c>a", "b>c", "c>b"],
            max_len: 5,
            top_n: 20,
            want: &["b,a", "c,a", "c,b", "c,a,b", "c,b,a"],
        },
        Case {
            name: "shortest first",
            edges: &["a>b", "b>c", "c>a", "y>z", "z>y"],
            max_len: 5,
            top_n: 20,
            want: &["z,y", "c,a,b"],
        },
        Case {
            name: "length bound",
            edges: &["a>b", "b>c", "c>d", "d>a"],
            max_len: 3,
            top_n: 20,
            want: &[],
        },
        Case {
            name: "length bound inclusive",
            edges: &["a>b", "b>c", "c>d", "d>a"],
            max_len: 4,
            top_n: 20,
            want: &["d,a,b,c"],
        },
        Case {
            name: "topN cap",
            edges: &["a>b", "b>a", "a>c", "c>a", "b>c", "c>b"],
            max_len: 5,
            top_n: 2,
            want: &["b,a", "c,a"],
        },
    ];
    for tc in &CASES {
        let g = import_graph(tc.edges);
        let got = cycle_strings(&import_cycles(&g, tc.max_len, tc.top_n));
        assert_eq!(got, tc.want, "{}: got {got:?}, want {:?}", tc.name, tc.want);
        let old = cycle_strings(&import_cycles_oracle(&g, tc.max_len, tc.top_n));
        assert_eq!(
            old, tc.want,
            "{}: oracle got {old:?}, want {:?}",
            tc.name, tc.want
        );
    }
}

/// Random import graphs, sparse to complete, including several nodes per file, duplicate edges,
/// non-import and type-only edges, and caps small enough to trip the topN*10 early exit.
#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_import_cycles_matches_oracle() {
    let (mut non_empty, mut capped) = (0, 0);
    for trial in 0..4000_i64 {
        let mut r = GoRand::new(trial);
        let n = 2 + r.intn(14);
        let mut g = Graph::new();
        for i in 0..n {
            g.add_node(node(&format!("n{i}"), "", &format!("f{}.go", r.intn(n))));
        }
        // Density cycles from sparse up to several edges per ordered pair.
        for _ in 0..r.intn(n * (1 + (trial % 12) as usize)) {
            let s = format!("n{}", r.intn(n));
            let t = format!("n{}", r.intn(n));
            let relation = rel(["imports_from", "imports_from", "calls"][r.intn(3)]);
            let mut e = edge(&s, &t, relation, Confidence::default());
            e.set_type_only(r.intn(8) == 0);
            g.add_edge(e);
        }
        let max_len = 1 + r.intn(6) as i64;
        let top_n = 1 + r.intn(4) as i64;
        let want = import_cycles_oracle(&g, max_len, top_n);
        let got = import_cycles(&g, max_len, top_n);
        if !want.is_empty() {
            non_empty += 1;
        }
        if want.len() as i64 == top_n {
            capped += 1;
        }
        assert_eq!(
            want, got,
            "trial {trial} maxLen={max_len} topN={top_n}:\nwant {want:?}\ngot  {got:?}"
        );
    }
    assert!(
        non_empty >= 1000 && capped >= 500,
        "weak coverage: nonEmpty={non_empty} capped={capped}"
    );
}

/// A dense acyclic import graph has width^maxLen paths and no cycle; the search must not walk
/// them.
#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_import_cycles_dense_acyclic_is_fast() {
    const LAYERS: usize = 7;
    const WIDTH: usize = 40;
    let id = |l: usize, i: usize| format!("l{l}/f{i:02}.go");
    let mut g = Graph::new();
    for l in 0..LAYERS {
        for i in 0..WIDTH {
            g.add_node(node(&id(l, i), "", &id(l, i)));
        }
    }
    for l in 1..LAYERS {
        for i in 0..WIDTH {
            for j in 0..WIDTH {
                g.add_edge(edge(
                    &id(l, i),
                    &id(l - 1, j),
                    Relation::IMPORTS_FROM,
                    Confidence::default(),
                ));
            }
        }
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(import_cycles(&g, 5, 20));
    });
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(cycles) => assert!(cycles.is_empty(), "expected no cycles, got {cycles:?}"),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("import_cycles still running after 10s on a dense acyclic graph")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("import_cycles panicked on a dense acyclic graph")
        }
    }
}

// ---- Upstream: test_analyze.py (find_import_cycles) ----

/// `_make_cycle_graph_directed`: a <-> b, b -> c -> d -> b, a self-loop on c, mixed relations,
/// and an edge to a node without a source file.
fn make_cycle_graph_directed() -> Graph {
    let mut g = Graph::new();
    for (id, file) in [
        ("src_a_ts", "src/a.ts"),
        ("src_b_ts", "src/b.ts"),
        ("src_c_ts", "src/c.ts"),
        ("src_d_ts", "src/d.ts"),
    ] {
        g.add_node(node(id, file.rsplit('/').next().unwrap_or(file), file));
    }
    // External-like node (no source_file): must be skipped safely.
    g.add_node(node("react", "react", ""));
    let imports = |g: &mut Graph, s: &str, t: &str| {
        g.add_edge(edge(s, t, Relation::IMPORTS_FROM, Confidence::EXTRACTED));
    };
    // 2-cycle: a <-> b
    imports(&mut g, "src_a_ts", "src_b_ts");
    imports(&mut g, "src_b_ts", "src_a_ts");
    // 3-cycle: b -> c -> d -> b
    imports(&mut g, "src_b_ts", "src_c_ts");
    imports(&mut g, "src_c_ts", "src_d_ts");
    imports(&mut g, "src_d_ts", "src_b_ts");
    // Self-loop: c imports itself
    imports(&mut g, "src_c_ts", "src_c_ts");
    // Mixed edge types: must not bleed into the cycle graph
    g.add_edge(edge(
        "src_a_ts",
        "react",
        Relation::CALLS,
        Confidence::INFERRED,
    ));
    g.add_edge(edge(
        "src_a_ts",
        "react",
        Relation::CONTAINS,
        Confidence::EXTRACTED,
    ));
    // Edge whose target has no source_file: must be skipped
    imports(&mut g, "src_a_ts", "react");
    g
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_find_import_cycles_detects_2_and_3_cycles() {
    let cycles = import_cycles(&make_cycle_graph_directed(), 5, 20);
    let sets: Vec<BTreeSet<&str>> = cycles
        .iter()
        .map(|c| c.files.iter().map(String::as_str).collect())
        .collect();
    let has = |want: &[&str]| sets.iter().any(|s| want.iter().all(|f| s.contains(f)));
    assert!(has(&["src/a.ts", "src/b.ts"]), "cycles = {cycles:?}");
    assert!(
        has(&["src/b.ts", "src/c.ts", "src/d.ts"]),
        "cycles = {cycles:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_find_import_cycles_respects_max_cycle_length() {
    let cycles = import_cycles(&make_cycle_graph_directed(), 2, 20);
    assert!(
        cycles.iter().all(|c| c.files.len() <= 2),
        "cycles = {cycles:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_find_import_cycles_skips_nodes_without_source_file() {
    let cycles = import_cycles(&make_cycle_graph_directed(), 5, 20);
    let flat = cycles
        .iter()
        .map(|c| c.files.join(" "))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!flat.contains("react"), "flat = {flat:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_find_import_cycles_ignores_non_import_relations() {
    let mut g = Graph::new();
    g.add_node(node("a", "a.ts", "src/a.ts"));
    g.add_node(node("b", "b.ts", "src/b.ts"));
    // Bidirectional non-import edges should not be considered a dependency cycle.
    g.add_edge(edge("a", "b", Relation::CALLS, Confidence::INFERRED));
    g.add_edge(edge("b", "a", Relation::CONTAINS, Confidence::EXTRACTED));
    let cycles = import_cycles(&g, 5, 20);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_find_import_cycles_empty_graph() {
    let cycles = import_cycles(&Graph::new(), 5, 20);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_find_import_cycles_no_cycles() {
    let mut g = Graph::new();
    g.add_node(node("x", "x.ts", "x.ts"));
    g.add_node(node("y", "y.ts", "y.ts"));
    g.add_edge(edge(
        "x",
        "y",
        Relation::IMPORTS_FROM,
        Confidence::EXTRACTED,
    ));
    let cycles = import_cycles(&g, 5, 20);
    assert!(cycles.is_empty(), "cycles = {cycles:?}");
}

// ---- Upstream: test_undirected_two_file_import_cycle.py ----
// graphify-go keeps both directed edges of a file pair, so the `reverse_import` stamp and the
// one-stored-edge-per-pair assertions have no Go counterpart; the cycle assertions are ported.

/// `_graph`: files a.py and b.py with the given edges between them.
fn two_file_graph(edges: Vec<crate::model::Edge>) -> Graph {
    let mut g = Graph::new();
    g.add_node(node("a", "a.py", "a.py"));
    g.add_node(node("b", "b.py", "b.py"));
    for e in edges {
        g.add_edge(e);
    }
    g
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_only_a_runtime_reverse_import_closes_a_cycle() {
    struct Case {
        name: &'static str,
        relation: &'static str,
        deferred: bool,
        type_only: bool,
    }
    const CASES: [Case; 4] = [
        Case {
            name: "other-relation",
            relation: "calls",
            deferred: false,
            type_only: false,
        },
        Case {
            name: "symbol-imports",
            relation: "imports",
            deferred: false,
            type_only: false,
        },
        Case {
            name: "deferred",
            relation: "imports_from",
            deferred: true,
            type_only: false,
        },
        Case {
            name: "type-only",
            relation: "imports_from",
            deferred: false,
            type_only: true,
        },
    ];
    for tc in &CASES {
        let mut reverse = edge("b", "a", rel(tc.relation), Confidence::EXTRACTED);
        reverse.set_deferred(tc.deferred);
        reverse.set_type_only(tc.type_only);
        let g = two_file_graph(vec![
            edge("a", "b", Relation::IMPORTS_FROM, Confidence::EXTRACTED),
            reverse,
        ]);
        let cycles = import_cycles(&g, 5, 20);
        assert!(cycles.is_empty(), "{}: cycles = {cycles:?}", tc.name);
    }
}

#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_the_directed_build_is_unchanged() {
    let g = two_file_graph(vec![
        edge("a", "b", Relation::IMPORTS_FROM, Confidence::EXTRACTED),
        edge("b", "a", Relation::IMPORTS_FROM, Confidence::EXTRACTED),
    ]);
    assert_eq!(g.num_edges(), 2);
    let cycles = import_cycles(&g, 5, 20);
    assert_eq!(
        names(&cycles),
        vec![vec!["a.py".to_string(), "b.py".to_string()]],
        "cycles = {cycles:?}"
    );
}

// ---- Added: deep_import_chain (plan hazard "Stack depth") ----

const CHAIN: usize = 100_000;

/// File `i` of a chain. Zero-padded so name order is numeric order, which is the order Go's
/// `ImportCycles` gives files ints in (`analyze.go:197-198`).
fn chain_file(i: usize) -> String {
    format!("f{i:05}")
}

/// A graph of `CHAIN` files, f00000 -> f00001 -> ... -> f99999, plus `extra` imports_from edges
/// given as file indexes.
fn chain_graph(extra: &[(usize, usize)]) -> Graph {
    let mut g = Graph::new();
    for i in 0..CHAIN {
        let f = chain_file(i);
        g.add_node(node(&f, "", &f));
    }
    let imports = |g: &mut Graph, a: usize, b: usize| {
        g.add_edge(edge(
            &chain_file(a),
            &chain_file(b),
            Relation::IMPORTS_FROM,
            Confidence::default(),
        ));
    };
    for i in 0..CHAIN - 1 {
        imports(&mut g, i, i + 1);
    }
    for &(a, b) in extra {
        imports(&mut g, a, b);
    }
    g
}

/// `import_cycles` on 100,000-file chains, on the production stack. The expected values follow
/// Go's algorithm (`analyze.go`: `ImportCycles` at :176, `scc` at :286); files are ints in name
/// order (:197-198) and the start of a cycle is its largest file:
///
/// - Plain chain: `scc` (Tarjan, iterative) gives every file its own component (:216), so the
///   reverse BFS of each start (:254-268) only takes `u` with `comp[u] == comp[s]` (:263) and
///   never grows past the start; `len(queue) == 1` skips the start (:269). No cycle.
/// - The chain closed by f99999 -> f00000: one component and one cycle of 100,000 files. The DFS
///   stops at `len(path) > max_len` (:226) and takes only `nb` stamped by the reverse BFS, which
///   reaches `max_len` files back (:259), with `nb <= start` (:242). Each start's only out-edge
///   goes to a larger file, except f99999 -> f00000, and f00000 is never stamped. No cycle.
/// - That ring plus f99999 -> f99998: start f99999 has f99998 stamped at distance 1, walks
///   f99999 -> f99998 -> f99999 and records `[f99999, f99998]` (:230-236); f99999 -> f00000 is
///   skipped (:242). Every other start's out-edge goes to a larger file. Exactly that cycle.
/// - The plain chain with the same f99999 -> f99998 edge: the only component of two files is
///   f99998, f99999; the same single cycle.
#[test]
#[ignore = "graphify-rs-ihc.10"]
fn test_deep_import_chain() {
    struct Case {
        name: &'static str,
        extra: &'static [(usize, usize)],
        want: &'static [&'static [usize]],
    }
    const CASES: [Case; 4] = [
        Case {
            name: "plain chain",
            extra: &[],
            want: &[],
        },
        Case {
            name: "chain closed into one cycle longer than max_len",
            extra: &[(CHAIN - 1, 0)],
            want: &[],
        },
        Case {
            name: "closed chain with a short cycle at the far end",
            extra: &[(CHAIN - 1, 0), (CHAIN - 1, CHAIN - 2)],
            want: &[&[CHAIN - 1, CHAIN - 2]],
        },
        Case {
            name: "plain chain with a short cycle at the far end",
            extra: &[(CHAIN - 1, CHAIN - 2)],
            want: &[&[CHAIN - 1, CHAIN - 2]],
        },
    ];
    for tc in &CASES {
        let g = chain_graph(tc.extra);
        let got = crate::testutil::on_worker_stack(|| import_cycles(&g, 5, 20));
        let want: Vec<Cycle> = tc
            .want
            .iter()
            .map(|c| Cycle {
                files: c.iter().map(|&i| chain_file(i)).collect(),
            })
            .collect();
        assert_eq!(got, want, "{}", tc.name);
    }
}
