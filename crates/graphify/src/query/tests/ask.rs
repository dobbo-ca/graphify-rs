//! Port of `ask_test.go` (22 tests). `ask` ranks with `gostd::math::log` (plan R10,
//! graphify-rs-aa6.21, which lands before the code PR), so every test that runs `ask` waits for
//! graphify-rs-ihc.13 like the rest.

use std::collections::BTreeSet;

use super::{graph_json, link_json, load_json, load_str, node_json};
use crate::query::ask::{ScoredNode, pick_seeds, query_terms};
use crate::query::{Graph, Node, ask};

/// Go `askGraph`: a rare identifier (authValidate) one hop from a common helper, plus an
/// unrelated noise node, so TF-IDF scoring and seed selection are exercised.
const ASK_GRAPH: &str = r#"{
  "directed": true, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"auth_validate","label":"authValidate()","file_type":"code","source_file":"auth.go","source_location":"L10","community":0,"norm_label":"authvalidate()"},
    {"id":"auth_check","label":"checkToken()","file_type":"code","source_file":"auth.go","source_location":"L20","community":0,"norm_label":"checktoken()"},
    {"id":"util_log","label":"log()","file_type":"code","source_file":"util.go","source_location":"L1","community":1,"norm_label":"log()"},
    {"id":"noise","label":"unrelated()","file_type":"code","source_file":"other.go","source_location":"L1","community":2,"norm_label":"unrelated()"}
  ],
  "links": [
    {"source":"auth_validate","target":"auth_check","relation":"calls","confidence":"INFERRED"},
    {"source":"auth_check","target":"util_log","relation":"calls","confidence":"INFERRED"}
  ]
}"#;

/// Go `loadAsk`.
fn load_ask() -> Graph {
    load_str(ASK_GRAPH)
}

/// Go's `[]string{...}` of relations; `&[]` is Go `nil`.
fn rels(r: &[&str]) -> Vec<String> {
    r.iter().map(|s| s.to_string()).collect()
}

/// Go `scoredNode{score, id, labelLen}`.
fn sn(score: f64, id: &str, label_len: i64) -> ScoredNode {
    ScoredNode {
        score,
        id: id.to_string(),
        label_len,
    }
}

/// A graph whose `by_id` holds `nodes` (Go `&Graph{byID: map[string]*Node{...}}`); the nodes
/// are not in `g.nodes`' links, as in Go.
fn graph_by_id(nodes: &[(&str, &str)]) -> Graph {
    let mut g = Graph::default();
    for (i, &(id, norm_label)) in nodes.iter().enumerate() {
        g.nodes.push(Node {
            norm_label: norm_label.to_string(),
            ..Node::default()
        });
        g.by_id.insert(id.to_string(), i);
    }
    g
}

/// Go `firstNodeLine`: the first NODE line of rendered output.
fn first_node_line(out: &str) -> &str {
    out.split('\n')
        .find(|l| l.starts_with("NODE "))
        .unwrap_or("")
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_seeds_rare_identifier() {
    let g = load_ask();
    let out = ask(&g, "where is authValidate", false, 2, 2000, &[]);
    assert!(
        out.contains("Traversal: BFS depth=2"),
        "missing BFS header: {out:?}"
    );
    // authValidate is the dominant exact-ish match and must lead the output.
    assert!(
        first_node_line(&out).starts_with("NODE authValidate()"),
        "seed not rendered first, got:\n{out}"
    );
    // The unrelated noise node shares no query token and must not be pulled in.
    assert!(
        !out.contains("unrelated()"),
        "noise node leaked into result:\n{out}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_traverses_to_neighbors() {
    let g = load_ask();
    let out = ask(&g, "authValidate", false, 2, 2000, &[]);
    // depth 2 from authValidate reaches checkToken (1 hop) and log (2 hops).
    for want in ["checkToken()", "log()"] {
        assert!(
            out.contains(want),
            "expected {want:?} in 2-hop traversal:\n{out}"
        );
    }
    assert!(
        out.contains("EDGE authValidate() --calls [INFERRED]--> checkToken()"),
        "expected rendered edge, got:\n{out}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_no_match() {
    let g = load_ask();
    let out = ask(&g, "nonexistentxyz", false, 2, 2000, &[]);
    assert_eq!(out, "No matching nodes found.", "want no-match message");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_dfs_mode() {
    let g = load_ask();
    let out = ask(&g, "authValidate", true, 2, 2000, &[]);
    assert!(
        out.contains("Traversal: DFS depth=2"),
        "missing DFS header: {out:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_budget_truncates() {
    let g = load_ask();
    // A tiny budget forces the truncation branch.
    let out = ask(&g, "authValidate", false, 2, 5, &[]);
    assert!(
        out.contains("truncated"),
        "expected truncation marker with tiny budget:\n{out}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_query_terms_drops_short_and_punctuation() {
    let got = query_terms("Is an auth-token ok?");
    // "is", "an", "ok" are <=2 chars and dropped; "auth"/"token" survive, the hyphen splits the
    // compound, and the "?" is stripped.
    let want = ["auth", "token"];
    assert_eq!(got.len(), want.len(), "got {got:?}, want keys of {want:?}");
    for t in &got {
        assert!(
            want.contains(&t.as_str()),
            "unexpected term {t:?} in {got:?}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_pick_seeds_gap_cutoff() {
    let scored = [sn(1000.0, "a", 1), sn(1.0, "b", 1), sn(0.9, "c", 1)];
    let seeds = pick_seeds(&Graph::default(), &scored, 3, 0.2);
    assert_eq!(seeds, ["a"], "gap cutoff failed");
}

/// Go `TestPickSeedsDedupsByLabel`: a swarm of homonymous nodes (three `GET` handlers) must not
/// fill every seed slot: dedup by normalized label seeds one representative and leaves room for
/// the distinct relevant node (#1766).
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_pick_seeds_dedups_by_label() {
    let g = graph_by_id(&[
        ("get1", "get()"),
        ("get2", "get()"),
        ("get3", "get()"),
        ("users", "usersmodel()"),
    ]);
    let scored = [
        sn(1000.0, "get1", 5),
        sn(1000.0, "get2", 5),
        sn(1000.0, "get3", 5),
        sn(900.0, "users", 12),
    ];
    let seeds = pick_seeds(&g, &scored, 3, 0.2);
    assert_eq!(
        seeds.len(),
        2,
        "expected 2 seeds (one GET + users), got {seeds:?}"
    );
    let set: BTreeSet<&str> = seeds.iter().map(String::as_str).collect();
    assert!(
        set.contains("users"),
        "distinct users node must be seeded, got {seeds:?}"
    );
    let gets = ["get1", "get2", "get3"]
        .iter()
        .filter(|id| set.contains(*id))
        .count();
    assert_eq!(
        gets, 1,
        "exactly one GET homonym should be seeded, got {gets} in {seeds:?}"
    );
}

/// Go `TestPickSeedsDedupNormalizesCase`: case-variant homonyms (GET/Get/get) must collapse to
/// one seed: exercises normLabel's case-folding, not just exact-string equality.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_pick_seeds_dedup_normalizes_case() {
    let g = graph_by_id(&[
        ("h1", "GET()"),
        ("h2", "Get()"),
        ("h3", "get()"),
        ("u", "usersmodel()"),
    ]);
    let scored = [
        sn(1000.0, "h1", 5),
        sn(1000.0, "h2", 5),
        sn(1000.0, "h3", 5),
        sn(900.0, "u", 12),
    ];
    let seeds = pick_seeds(&g, &scored, 3, 0.2);
    assert_eq!(
        seeds.len(),
        2,
        "case-variant homonyms must collapse to one seed, got {seeds:?}"
    );
}

/// Go `TestPickSeedsKeepsLabellessDistinct`: label-less nodes must NOT collapse into a single
/// seed: they stay distinct via the id fallback (mirrors upstream `... or nid`).
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_pick_seeds_keeps_labelless_distinct() {
    let g = graph_by_id(&[("n1", ""), ("n2", ""), ("n3", "")]);
    let scored = [
        sn(1000.0, "n1", 0),
        sn(1000.0, "n2", 0),
        sn(1000.0, "n3", 0),
    ];
    let seeds = pick_seeds(&g, &scored, 3, 0.2);
    assert_eq!(
        seeds.len(),
        3,
        "distinct label-less nodes must not be deduped to one; got {seeds:?}"
    );
}

/// Go `TestQueryTermsDropsStopwords`: question/filler stopwords (English and German/Romance) are
/// dropped so the content noun drives seeding (#1900).
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_query_terms_drops_stopwords() {
    let got = query_terms("Wie funktioniert die Authentifizierung?");
    assert_eq!(got, ["authentifizierung"], "German stopwords not dropped");
    let got = query_terms("how does the cache work");
    assert_eq!(got, ["cache"], "English stopwords not dropped");
}

/// Go `TestQueryTermsAllStopwordsFallback`: an all-stopword query falls back to the unfiltered
/// searchable tokens so it still seeds on something.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_query_terms_all_stopwords_fallback() {
    let got = query_terms("wie funktioniert das");
    assert_eq!(
        got.len(),
        3,
        "all-stopword query should fall back to unfiltered terms, got {got:?}"
    );
}

/// Go `TestQueryTermsHandlesAccentedTokens`: accented tokens must be kept whole (Unicode-aware
/// tokenizer): an accented content noun survives while accented/ASCII stopwords around it drop.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_query_terms_handles_accented_tokens() {
    let got = query_terms("warum funktioniert die Größe nicht");
    assert_eq!(got, ["größe"], "accented content noun should survive whole");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_emits_edge_between_seeds() {
    let g = load_ask();
    // Both endpoints are seeds, so the traversal itself records no edge between them; the
    // induced-edge pass must still emit it, exactly once.
    for dfs in [false, true] {
        let out = ask(&g, "authValidate checkToken", dfs, 1, 2000, &[]);
        let n = out
            .split('\n')
            .filter(|l| {
                l.starts_with("EDGE ") && l.contains("authValidate()") && l.contains("checkToken()")
            })
            .count();
        assert_eq!(
            n, 1,
            "dfs={dfs}: want 1 authValidate<->checkToken edge, got {n}:\n{out}"
        );
    }
}

/// Go `contextGraph`: authValidate has one `calls` neighbour and one `imports` neighbour so
/// relation filtering can be observed.
const CONTEXT_GRAPH: &str = r#"{
  "directed": true, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"auth_validate","label":"authValidate()","file_type":"code","source_file":"auth.go","source_location":"L10","community":0,"norm_label":"authvalidate()"},
    {"id":"auth_check","label":"checkToken()","file_type":"code","source_file":"auth.go","source_location":"L20","community":0,"norm_label":"checktoken()"},
    {"id":"crypto_mod","label":"crypto","file_type":"code","source_file":"crypto.go","source_location":"L1","community":1,"norm_label":"crypto"}
  ],
  "links": [
    {"source":"auth_validate","target":"auth_check","relation":"calls","confidence":"INFERRED"},
    {"source":"auth_validate","target":"crypto_mod","relation":"imports","confidence":"EXPLICIT"}
  ]
}"#;

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_context_filter() {
    let g = load_str(CONTEXT_GRAPH);
    let unfiltered = ask(&g, "authValidate", false, 2, 2000, &[]);
    assert!(
        unfiltered.contains("checkToken()") && unfiltered.contains("crypto"),
        "expected both neighbours without a filter:\n{unfiltered}"
    );
    let out = ask(&g, "authValidate", false, 2, 2000, &rels(&["calls"]));
    assert!(
        out.contains("checkToken()"),
        "calls neighbour missing under --context calls:\n{out}"
    );
    assert!(
        !out.contains("crypto"),
        "imports neighbour leaked under --context calls:\n{out}"
    );
    let dfs = ask(&g, "authValidate", true, 2, 2000, &rels(&["calls"]));
    assert!(
        !dfs.contains("crypto"),
        "imports neighbour leaked under DFS --context calls:\n{dfs}"
    );
}

/// Go `TestAskMatchesAttributes`: a node's Terraform attributes feed the search text, so an
/// agent can ask "which resource runs t3.large" and land on the resource that sets it.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_matches_attributes() {
    let src = r#"{
  "directed": true, "multigraph": false, "graph": {},
  "nodes": [
    {"id":"web","label":"aws_instance.web","file_type":"code","source_file":"main.tf","source_location":"L1","community":0,"norm_label":"aws_instance.web","attributes":{"instance_type":"t3.large"}},
    {"id":"db","label":"aws_instance.db","file_type":"code","source_file":"main.tf","source_location":"L9","community":0,"norm_label":"aws_instance.db","attributes":{"instance_type":"t3.micro"}}
  ],
  "links": []
}"#;
    let g = load_str(src);
    let out = ask(&g, "t3.large", false, 1, 2000, &[]);
    assert!(
        first_node_line(&out).starts_with("NODE aws_instance.web"),
        "attribute match did not seed aws_instance.web, got:\n{out}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_stored_direction_and_site() {
    let g = load_json(&graph_json(
        &[
            node_json("m_py", "m.py", "m.py", "L1"),
            node_json("m_py_alpha", "alpha()", "m.py", "L1"),
            node_json("m_py_beta", "beta()", "m.py", "L5"),
        ],
        &[
            link_json(
                "m_py",
                "m_py_alpha",
                "contains",
                "EXTRACTED",
                "m.py",
                "L1",
                "",
            ),
            link_json(
                "m_py",
                "m_py_beta",
                "contains",
                "EXTRACTED",
                "m.py",
                "L5",
                "",
            ),
            link_json(
                "m_py_beta",
                "m_py_alpha",
                "calls",
                "INFERRED",
                "m.py",
                "L6",
                "",
            ),
        ],
    ));
    let want = "Traversal: BFS depth=2 | Start: [beta()] | 3 nodes found\n\nNODE beta() [src=m.py loc=L5 community=0]\nNODE m.py [src=m.py loc=L1 community=0]\nNODE alpha() [src=m.py loc=L1 community=0]\nEDGE m.py --contains [EXTRACTED]--> beta() at=m.py:L5\nEDGE beta() --calls [INFERRED]--> alpha() at=m.py:L6\nEDGE m.py --contains [EXTRACTED]--> alpha() at=m.py:L1";
    let got = ask(&g, "beta", false, 2, 2000, &[]);
    assert_eq!(got, want, "got:\n{got}\nwant:\n{want}");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_parallel_relations() {
    let g = load_json(&graph_json(
        &[
            node_json("a_py", "a.py", "a.py", "L1"),
            node_json("a_py_alpha", "Alpha", "a.py", "L1"),
            node_json("a_py_make_alpha", "make_alpha()", "a.py", "L4"),
        ],
        &[
            link_json(
                "a_py",
                "a_py_alpha",
                "contains",
                "EXTRACTED",
                "a.py",
                "L1",
                "",
            ),
            link_json(
                "a_py",
                "a_py_make_alpha",
                "contains",
                "EXTRACTED",
                "a.py",
                "L4",
                "",
            ),
            link_json(
                "a_py_make_alpha",
                "a_py_alpha",
                "calls",
                "INFERRED",
                "a.py",
                "L5",
                "",
            ),
            link_json(
                "a_py_make_alpha",
                "a_py_alpha",
                "references",
                "EXTRACTED",
                "a.py",
                "L4",
                "",
            ),
        ],
    ));
    let want = "Traversal: BFS depth=2 | Start: [make_alpha()] | 3 nodes found\n\nNODE make_alpha() [src=a.py loc=L4 community=0]\nNODE Alpha [src=a.py loc=L1 community=0]\nNODE a.py [src=a.py loc=L1 community=0]\nEDGE a.py --contains [EXTRACTED]--> make_alpha() at=a.py:L4\nEDGE make_alpha() --calls [INFERRED]--> Alpha at=a.py:L5\nEDGE make_alpha() --references [EXTRACTED]--> Alpha at=a.py:L4\nEDGE a.py --contains [EXTRACTED]--> Alpha at=a.py:L1";
    let got = ask(&g, "make_alpha", false, 2, 2000, &[]);
    assert_eq!(got, want, "got:\n{got}\nwant:\n{want}");
    let got = ask(&g, "make_alpha", false, 2, 2000, &rels(&["calls"]));
    assert!(
        got.contains("2 nodes found")
            && got.ends_with("EDGE make_alpha() --calls [INFERRED]--> Alpha at=a.py:L5")
            && !got.contains("references"),
        "relation filter: {got}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_mutual_context_self_loop() {
    let g = load_json(&graph_json(
        &[
            node_json("a", "alpha()", "m.py", "L1"),
            node_json("b", "bravo()", "m.py", "L1"),
        ],
        &[
            link_json(
                "a",
                "b",
                "calls",
                "EXTRACTED",
                "m.py",
                "L5",
                r#","context":"call""#,
            ),
            link_json("b", "a", "calls", "EXTRACTED", "m.py", "L9", ""),
            link_json("a", "b", "references", "EXTRACTED", "m.py", "L4", ""),
            link_json("a", "a", "calls", "EXTRACTED", "m.py", "L2", ""),
        ],
    ));
    let want = [
        "EDGE alpha() --calls [EXTRACTED]--> bravo() at=m.py:L5",
        "EDGE bravo() --calls [EXTRACTED]--> alpha() at=m.py:L9",
        "EDGE alpha() --references [EXTRACTED]--> bravo() at=m.py:L4",
    ];
    let out = ask(&g, "alpha", false, 2, 2000, &[]);
    let got: Vec<&str> = out.split('\n').filter(|l| l.starts_with("EDGE ")).collect();
    assert_eq!(got, want, "got {got:?} want {want:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_ask_hop_order() {
    let g = load_json(&graph_json(
        &[
            node_json("s", "zeta()", "m.py", "L1"),
            node_json("x", "xray()", "m.py", "L2"),
            node_json("h", "hub()", "m.py", "L3"),
            node_json("l1", "leaf1()", "m.py", "L4"),
            node_json("l2", "leaf2()", "m.py", "L5"),
            node_json("l3", "leaf3()", "m.py", "L6"),
        ],
        &[
            link_json("s", "x", "calls", "EXTRACTED", "m.py", "L1", ""),
            link_json("x", "h", "calls", "EXTRACTED", "m.py", "L2", ""),
            link_json("l1", "h", "calls", "EXTRACTED", "m.py", "L4", ""),
            link_json("l2", "h", "calls", "EXTRACTED", "m.py", "L5", ""),
            link_json("l3", "h", "calls", "EXTRACTED", "m.py", "L6", ""),
        ],
    ));
    let want = "Traversal: BFS depth=2 | Start: [zeta()] | 3 nodes found\n\nNODE zeta() [src=m.py loc=L1 community=0]\nNODE xray() [src=m.py loc=L2 community=0]\nNODE hub() [src=m.py loc=L3 community=0]\nEDGE zeta() --calls [EXTRACTED]--> xray() at=m.py:L1\nEDGE xray() --calls [EXTRACTED]--> hub() at=m.py:L2";
    let got = ask(&g, "zeta", false, 2, 2000, &[]);
    assert_eq!(got, want, "got:\n{got}\nwant:\n{want}");
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_hub_threshold_counts_isolated_nodes() {
    let mut nodes = vec![
        node_json("seedling", "seedling", "m.py", "L1"),
        node_json("hubone", "hubone", "m.py", "L1"),
        node_json("hubtwo", "hubtwo", "m.py", "L1"),
        node_json("x1", "x1", "m.py", "L1"),
        node_json("x2", "x2", "m.py", "L1"),
    ];
    let mut links = vec![
        link_json("seedling", "hubtwo", "calls", "EXTRACTED", "", "", ""),
        link_json("hubone", "x1", "calls", "EXTRACTED", "", "", ""),
        link_json("hubone", "x2", "calls", "EXTRACTED", "", "", ""),
    ];
    for k in 0..52 {
        let id = format!("leaf{k}");
        nodes.push(node_json(&id, &id, "m.py", "L1"));
        links.push(link_json("hubone", &id, "calls", "EXTRACTED", "", "", ""));
        links.push(link_json("hubtwo", &id, "calls", "EXTRACTED", "", "", ""));
    }
    for k in 0..50 {
        let id = format!("iso{k}");
        nodes.push(node_json(&id, &id, "m.py", "L1"));
    }
    let g = load_json(&graph_json(&nodes, &links));
    let got = ask(&g, "seedling", false, 2, 2000, &[]);
    assert!(
        got.contains("| 2 nodes found"),
        "got header: {}",
        got.chars().take(120).collect::<String>()
    );
}

#[test]
#[ignore = "graphify-rs-ihc.13"]
fn test_hub_threshold_context_keeps_dangling_nodes() {
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    let mut add = |id: &str| nodes.push(node_json(id, id, "m.py", "L1"));
    add("seedling");
    add("x");
    add("y");
    for k in 0..29 {
        add(&format!("lx{k}"));
    }
    for k in 0..40 {
        add(&format!("ly{k}"));
    }
    for k in 0..28 {
        add(&format!("pad{k}"));
    }
    let mut rel = |a: &str, b: &str, r: &str| {
        links.push(link_json(a, b, r, "EXTRACTED", "", "", ""));
    };
    rel("seedling", "x", "rela");
    rel("x", "seedling", "rela");
    for k in 0..29 {
        let id = format!("lx{k}");
        rel("x", &id, "rela");
        rel("x", &id, "rela");
    }
    for k in 0..40 {
        let id = format!("ly{k}");
        rel("y", &id, "rela");
        rel("y", &id, "rela");
    }
    rel("pad0", "ghost", "relb");
    let g = load_json(&graph_json(&nodes, &links));
    let got = ask(&g, "seedling", false, 2, 2000, &rels(&["rela"]));
    assert!(
        got.contains("| 2 nodes found"),
        "got header: {}",
        got.chars().take(120).collect::<String>()
    );
}
