//! The plan's added tests for Task 12: `deep_path_chain` (hazard "Stack depth"),
//! `validate_bom_error_text` (Review Focus 3), `load_error_texts`, and the three
//! `*_ties_use_go_pdqsort` tests (hazard "sort.Slice with ties": `query.go:159`, `:215`,
//! `affected.go:140`).
//!
//! The JSON error texts are Go 1.27's (jsonv2-backed `encoding/json`) as gostd reproduces them:
//! the BOM row of `crates/gostd/tests/json_scanner.rs` (`invalid character '﻿' looking for
//! beginning of value`), "unexpected end of JSON input" (`json_decode.rs`), and the
//! `UnmarshalTypeError` text with the root type's name and the field path
//! (`json_decode.rs` `v1_compat_rules`: "Go struct field .nodes.1.id" for an unnamed root, so
//! `Graph.nodes.1.id` for `query.Graph`).

use std::fmt::Write as _;

use super::{AFFECTED_JSON, ids, load_str, tie_keys, write_graph_json};
use crate::query::{
    AffectedOptions, Match, Neighbor, Node, affected, explain, load, path_edges, query, validate,
};
use crate::testutil::{TempDir, on_worker_stack};

/// Nodes in a chain for `deep_path_chain`.
const CHAIN: usize = 100_000;

/// `path` and `affected` over a 100,000-node chain `n0 -> n1 -> ... -> n99999` (each node in its
/// own file), on the production stack: Go grows a goroutine stack to 1 GB, a Rust thread's is
/// fixed.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn deep_path_chain() {
    let mut body = String::from(r#"{"directed":true,"multigraph":false,"graph":{},"nodes":["#);
    for i in 0..CHAIN {
        if i > 0 {
            body.push(',');
        }
        write!(
            body,
            r#"{{"id":"n{i}","label":"n{i}","file_type":"code","source_file":"f{i}.go","source_location":"L1"}}"#
        )
        .expect("write to String");
    }
    body.push_str(r#"],"links":["#);
    for i in 0..CHAIN - 1 {
        if i > 0 {
            body.push(',');
        }
        write!(
            body,
            r#"{{"source":"n{i}","target":"n{}","relation":"calls","confidence":"EXTRACTED"}}"#,
            i + 1
        )
        .expect("write to String");
    }
    body.push_str("]}");

    on_worker_stack(move || {
        let g = load_str(&body);
        let last = format!("n{}", CHAIN - 1);

        let res = path_edges(&g, "n0", &last, 0, false).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(res.nodes.len(), CHAIN, "path nodes");
        assert_eq!(res.edges.len(), CHAIN - 1, "path edges");
        assert!(
            res.edges.iter().all(|e| e.forward && e.relation == "calls"),
            "every hop is a forward calls edge"
        );
        let res = path_edges(&g, &last, "n0", 0, true).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(res.nodes.len(), CHAIN, "undirected path nodes");

        let seed = format!("f{}.go", CHAIN - 1);
        let res = affected(&g, &[seed], &AffectedOptions::default());
        assert_eq!(ids(&res.changed), [last.as_str()], "changed");
        assert_eq!(res.impacted.len(), CHAIN - 1, "impacted: every other node");
    });
}

/// Review Focus 3: `load` strips a UTF-8 BOM (`query.go:111`), `validate` does not
/// (`validate.go:27`), so `validate` reports Go's decode error for it.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn validate_bom_error_text() {
    let mut data = b"\xef\xbb\xbf".to_vec();
    data.extend_from_slice(AFFECTED_JSON.as_bytes());
    let (_d, p) = write_graph_json(&data);

    match validate(&p) {
        Err(e) => assert_eq!(
            e.to_string(),
            format!(
                "parsing {}: invalid character '\\ufeff' looking for beginning of value",
                p.display()
            )
        ),
        Ok(r) => panic!("validate of a BOM file = {r:?}, want the decode error"),
    }
    let g = load(&p).unwrap_or_else(|e| panic!("load strips the BOM: {e}"));
    assert_eq!(g.nodes.len(), 4, "nodes after the BOM");
}

/// Go's texts for the three ways `load` fails: `os.ReadFile`'s `*PathError` (the size check
/// passes a file it cannot stat), a truncated file and a value of the wrong type.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn load_error_texts() {
    let dir = TempDir::new();
    let missing = dir.join("graphify-out/graph.json");
    match load(&missing) {
        Err(e) => assert_eq!(
            e.to_string(),
            format!("open {}: no such file or directory", missing.display())
        ),
        Ok(g) => panic!("load of a missing file = {g:?}"),
    }

    const CASES: &[(&str, &str, &str)] = &[
        ("truncated", r#"{"nodes":["#, "unexpected end of JSON input"),
        (
            "wrong type",
            r#"{"nodes":[{"id":"a"},{"id":1}]}"#,
            "json: cannot unmarshal number into Go struct field Graph.nodes.1.id of type string",
        ),
    ];
    for &(name, body, want) in CASES {
        let (_d, p) = write_graph_json(body.as_bytes());
        match load(&p) {
            Err(e) => assert_eq!(
                e.to_string(),
                format!("parsing {}: {want}", p.display()),
                "{name}"
            ),
            Ok(g) => panic!("{name}: load = {g:?}, want an error"),
        }
    }
}

/// A graph.json of `nodes` (already JSON) and `links`.
fn graph_of(nodes: &[String], links: &[String]) -> String {
    format!(
        r#"{{"directed":true,"multigraph":false,"graph":{{}},"nodes":[{}],"links":[{}]}}"#,
        nodes.join(","),
        links.join(",")
    )
}

/// `query.go:159`: `sort.Slice(out, label)` of the matches, many of them with one label.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn query_ties_use_go_pdqsort() {
    let keys = tie_keys();
    let nodes: Vec<String> = keys
        .iter()
        .enumerate()
        .map(|(j, k)| {
            format!(
                r#"{{"id":"n{j:02}","label":"L{k}","file_type":"code","source_file":"x.go","source_location":"L1"}}"#
            )
        })
        .collect();
    let g = load_str(&graph_of(&nodes, &[]));

    // Go's matches before the sort: every node, in node order.
    let unsorted: Vec<Match> = keys
        .iter()
        .enumerate()
        .map(|(j, k)| Match {
            id: format!("n{j:02}"),
            label: format!("L{k}"),
            location: "x.go:1".to_string(),
        })
        .collect();
    let mut want = unsorted.clone();
    gostd::sort::slice(&mut want, |a, b| a.label < b.label);
    let mut stable = unsorted;
    stable.sort_by(|a, b| a.label.cmp(&b.label));
    assert_ne!(
        want, stable,
        "the input must tell pdqsort from a stable sort"
    );

    let got = query(&g, "L").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(got, want, "matches in Go's pdqsort order");
}

/// `query.go:215`: `sort.Slice(nbrs, degree desc, relation, label, direction)` of a hub's
/// neighbours, many of them equal on all four.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn explain_ties_use_go_pdqsort() {
    let keys = tie_keys();
    let mut nodes = vec![
        r#"{"id":"h","label":"hub","file_type":"code","source_file":"h.go","source_location":"L1"}"#
            .to_string(),
    ];
    let mut links = Vec::new();
    for (j, k) in keys.iter().enumerate() {
        nodes.push(format!(
            r#"{{"id":"n{j:02}","label":"L{k}","file_type":"code","source_file":"x.go","source_location":"L1"}}"#
        ));
        links.push(format!(
            r#"{{"source":"h","target":"n{j:02}","relation":"calls","confidence":"EXTRACTED"}}"#
        ));
    }
    let g = load_str(&graph_of(&nodes, &links));

    // Go's neighbours before the sort: one per link, in link order.
    let unsorted: Vec<Neighbor> = keys
        .iter()
        .enumerate()
        .map(|(j, k)| Neighbor {
            id: format!("n{j:02}"),
            label: format!("L{k}"),
            relation: "calls".to_string(),
            direction: "->".to_string(),
            location: "x.go:1".to_string(),
            file: "x.go".to_string(),
            degree: 1,
        })
        .collect();
    let less = |a: &Neighbor, b: &Neighbor| {
        if a.degree != b.degree {
            return a.degree > b.degree;
        }
        if a.relation != b.relation {
            return a.relation < b.relation;
        }
        if a.label != b.label {
            return a.label < b.label;
        }
        a.direction < b.direction
    };
    let mut want = unsorted.clone();
    gostd::sort::slice(&mut want, less);
    let mut stable = unsorted;
    stable.sort_by(|a, b| a.label.cmp(&b.label));
    assert_ne!(
        want, stable,
        "the input must tell pdqsort from a stable sort"
    );

    let ex = explain(&g, "h").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ex.neighbors, want, "neighbours in Go's pdqsort order");
}

/// `affected.go:140`: `collect`'s `sort.Slice(out, source file, label)`, many nodes with one
/// file and label.
#[test]
#[ignore = "graphify-rs-ihc.13"]
fn affected_ties_use_go_pdqsort() {
    let keys = tie_keys();
    let nodes: Vec<String> = keys
        .iter()
        .enumerate()
        .map(|(j, k)| {
            format!(
                r#"{{"id":"n{j:02}","label":"x","file_type":"code","source_file":"f{k}.go","source_location":"L1"}}"#
            )
        })
        .collect();
    let g = load_str(&graph_of(&nodes, &[]));

    let unsorted: Vec<Node> = g.nodes.clone();
    let mut want = unsorted.clone();
    gostd::sort::slice(&mut want, |a, b| {
        if a.source_file != b.source_file {
            return a.source_file < b.source_file;
        }
        a.label < b.label
    });
    let mut stable = unsorted;
    stable.sort_by(|a, b| a.source_file.cmp(&b.source_file));
    assert_ne!(
        ids(&want),
        ids(&stable),
        "the input must tell pdqsort from a stable sort"
    );

    let files: Vec<String> = keys.iter().map(|k| format!("f{k}.go")).collect();
    let res = affected(&g, &files, &AffectedOptions::default());
    assert_eq!(
        ids(&res.changed),
        ids(&want),
        "changed in Go's pdqsort order"
    );
}
