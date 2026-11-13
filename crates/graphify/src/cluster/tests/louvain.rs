//! Port of `louvain_test.go` (3 tests) and the added `louvain_shuffle_uses_pcg_seed_42`.

use std::collections::{BTreeMap, BTreeSet};

use super::gorand::GoRand;
use crate::cluster::cluster;
use crate::cluster::cluster::louvain;
use crate::cluster::louvain::{local_move_order, modularize, modularize_with};
use crate::model::{Edge, Graph, Node, Relation};

struct LouvainCase {
    name: String,
    g: Graph,
}

/// Go `build`: a graph of n nodes named by index, with the given index pairs as edges.
fn build(n: usize, pairs: &[[usize; 2]]) -> Graph {
    let mut g = Graph::new();
    for i in 0..n {
        g.add_node(Node {
            id: format!("n{i:03}"),
            ..Node::default()
        });
    }
    let ids = g.node_ids().to_vec();
    for p in pairs {
        g.add_edge(Edge {
            source: ids[p[0]].clone(),
            target: ids[p[1]].clone(),
            relation: Relation::CALLS,
            ..Edge::default()
        });
    }
    g
}

fn clique(from: usize, to: usize) -> Vec<[usize; 2]> {
    let mut out = Vec::new();
    for i in from..to {
        for j in i + 1..to {
            out.push([i, j]);
        }
    }
    out
}

fn louvain_cases() -> Vec<LouvainCase> {
    let case = |name: &str, g: Graph| LouvainCase {
        name: name.to_string(),
        g,
    };
    let mut cases = vec![
        case("single", build(1, &[])),
        case("isolated", build(5, &[])),
        case("pair", build(2, &[[0, 1]])),
        case(
            "two_triangles",
            build(6, &[clique(0, 3), clique(3, 6), vec![[0, 3]]].concat()),
        ),
        // Every move ties: the lowest community index must win.
        case("tie_cycle4", build(4, &[[0, 1], [1, 2], [2, 3], [3, 0]])),
        case(
            "tie_star",
            build(7, &[[0, 1], [0, 2], [0, 3], [0, 4], [0, 5], [0, 6]]),
        ),
        case(
            "tie_path",
            build(
                9,
                &[
                    [0, 1],
                    [1, 2],
                    [2, 3],
                    [3, 4],
                    [4, 5],
                    [5, 6],
                    [6, 7],
                    [7, 8],
                ],
            ),
        ),
        case(
            "self_loops",
            build(5, &[[0, 0], [1, 1], [4, 4], [0, 1], [1, 2], [2, 0]]),
        ),
        case(
            "duplicate_edges",
            build(4, &[[0, 1], [1, 0], [0, 1], [2, 3], [3, 2], [1, 2]]),
        ),
        case(
            "disconnected",
            build(
                14,
                &[clique(0, 4), clique(4, 8), vec![[8, 9], [9, 10], [11, 12]]].concat(),
            ),
        ),
        // A dense clique raises the total weight so the ring of small cliques
        // merges into one oversized community, which splitOversized breaks up.
        case(
            "oversized",
            build(
                60,
                &[
                    clique(0, 40),
                    clique(40, 45),
                    clique(45, 50),
                    clique(50, 55),
                    clique(55, 60),
                    vec![[40, 45], [46, 50], [51, 55], [56, 41]],
                ]
                .concat(),
            ),
        ),
    ];
    for trial in 0..40 {
        let mut r = GoRand::new(trial);
        let n = 2 + r.intn(250);
        let blocks = 1 + r.intn(8);
        let mut pairs: Vec<[usize; 2]> = Vec::new();
        let k = 1 + r.intn(6);
        for _ in 0..r.intn(n * k) {
            let a = r.intn(n);
            let mut b = r.intn(n);
            if r.intn(4) > 0 {
                // planted structure
                b = (a / blocks * blocks + r.intn(blocks)) % n;
            }
            pairs.push([a, b]);
        }
        if trial % 2 == 1 {
            // heavy core, so sparse groups merge and get split again
            pairs.extend(clique(0, n / 5));
        }
        cases.push(case(&format!("random_{trial:02}"), build(n, &pairs)));
    }
    cases
}

/// Go `formatGroups`: renders groups as node indices, keeping group and member order.
fn format_groups(groups: &[Vec<String>]) -> String {
    let parts: Vec<String> = groups
        .iter()
        .map(|group| {
            let idx: Vec<String> = group
                .iter()
                .map(|id| {
                    id[1..]
                        .parse::<i64>()
                        .expect("node id n<digits>")
                        .to_string()
                })
                .collect();
            idx.join(" ")
        })
        .collect();
    parts.join("|")
}

fn clustered(g: &Graph) -> Vec<Vec<String>> {
    // BTreeMap iterates in ascending community id, as Go sorts cids.
    cluster(g).into_values().collect()
}

// The golden was recorded from gonum's community.Modularize before it was
// replaced. The raw order matters too: splitOversized feeds it back in.
#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_louvain_matches_gonum_golden() {
    let path = crate::testutil::testdata().join("cluster/louvain_gonum.golden");
    let data = std::fs::read_to_string(path).expect("read golden");
    let mut want: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for line in gostd::strings::trim_space(&data).split('\n') {
        let f: Vec<&str> = line.split('\t').collect();
        want.insert(f[0], f[1..].to_vec());
    }
    let cases = louvain_cases();
    assert_eq!(
        want.len(),
        cases.len(),
        "golden has {} cases, want {}",
        want.len(),
        cases.len()
    );
    for c in &cases {
        let w = &want[c.name.as_str()];
        for _run in 0..2 {
            // same answer every run
            let got = format_groups(&louvain(&c.g, c.g.node_ids()));
            assert_eq!(got, w[0], "{}: raw groups", c.name);
            let got = format_groups(&clustered(&c.g));
            assert_eq!(got, w[1], "{}: Cluster", c.name);
        }
    }
}

/// Go `set := map[[2]int]bool{}` then `adj[p[0]] = append(adj[p[0]], p[1])` and the mirror.
fn adjacency(n: usize, set: &BTreeSet<[usize; 2]>) -> Vec<Vec<i64>> {
    let mut adj = vec![Vec::new(); n];
    for p in set {
        adj[p[0]].push(p[1] as i64);
        adj[p[1]].push(p[0] as i64);
    }
    adj
}

// louvain feeds modularize each node's neighbours in map order, so the result
// must not depend on that order, group and member order included.
#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_modularize_ignores_neighbour_order() {
    let mut rng = GoRand::new(7);
    for n in [2, 9, 60, 400, 3000] {
        // Sparse random edges plus a few dense blocks, so ties and real moves both occur.
        let mut set: BTreeSet<[usize; 2]> = BTreeSet::new();
        for _ in 0..3 * n {
            let a = rng.intn(n);
            let b = rng.intn(n);
            if a != b {
                set.insert([a.min(b), a.max(b)]);
            }
        }
        let mut b = 0;
        while b + 6 <= n && b < 60 {
            set.extend(clique(b, b + 6));
            b += 12;
        }
        let mut adj = adjacency(n, &set);
        for row in &mut adj {
            row.sort();
        }
        let want = modularize(&adj);
        for _run in 0..5 {
            let mut shuffled = adj.clone();
            for row in &mut shuffled {
                rng.shuffle(row);
            }
            assert!(
                modularize(&shuffled) == want,
                "n={n}: result changed with neighbour order"
            );
        }
    }
}

// The skip in localMove must give what looking at every node every sweep gives.
#[test]
#[ignore = "graphify-rs-ihc.9"]
fn test_skip_matches_looking_at_all() {
    let mut rng = GoRand::new(11);
    for round in 0..400 {
        // Blocks of uneven size with noise between them: near ties and long
        // tails of sweeps with a few moves each.
        let n = 2 + rng.intn(1500);
        let block = 2 + rng.intn(40);
        let mut set: BTreeSet<[usize; 2]> = BTreeSet::new();
        let mut i = 0;
        // Go re-evaluates the bound, and so draws from rng, on every iteration.
        while i < n * (1 + rng.intn(6)) {
            let a = rng.intn(n);
            let mut b = rng.intn(n);
            if rng.intn(4) > 0 {
                b = a / block * block + rng.intn(block);
            }
            if a != b && b < n {
                set.insert([a.min(b), a.max(b)]);
            }
            i += 1;
        }
        let adj = adjacency(n, &set);
        let want = modularize_with(&adj, true);
        assert!(
            modularize(&adj) == want,
            "round {round}, n={n}: the skip changed the result"
        );
    }
}

/// Added test. The visit order Louvain uses for n = 13 is the Fisher-Yates of louvain.go:130-133
/// over a fresh seed-42 PCG. The reference below is the loop of the gostd oracle op `louvain`
/// (crates/gostd/examples/oracle/pcg.rs); gostd parity checks that op against Go for n = 2..=1000,
/// so the reference is Go's order.
#[test]
#[ignore = "graphify-rs-ihc.9"]
fn louvain_shuffle_uses_pcg_seed_42() {
    let n = 13usize;
    let mut r = gostd::pcg::Rand::new(gostd::pcg::PcgSource::new(42));
    let mut want: Vec<usize> = (0..n).collect();
    for i in 0..n - 1 {
        let j = i + r.intn((n - i) as i64) as usize;
        want.swap(i, j);
    }
    assert_eq!(local_move_order(n), want);
}
