//! Port of `stream_test.go` (1 test). No upstream file tests the streamed writer.

use std::collections::BTreeMap;

use crate::cluster::node_community;
use crate::export::export::{JsonGraph, to_json_link, to_json_node, write_json};
use crate::model::{Edge, Graph, Node, conf, file_of, line, rel};

/// Go `wholeJSON`: the pre-streaming encoder, one MarshalIndent of the document.
fn whole_json(g: &Graph, communities: &BTreeMap<i64, Vec<String>>, commit: &str) -> Vec<u8> {
    let nc = node_community(communities);
    let mut out = JsonGraph {
        hyperedges: Some(Vec::new()),
        built_at_commit: commit.to_string(),
        ..JsonGraph::default()
    };
    for (ext, n) in &g.unclassified {
        out.graph.unclassified_exts.insert(ext.clone(), *n);
        out.graph.unclassified_files += *n;
    }
    let nodes: Vec<_> = g
        .node_ids()
        .iter()
        .map(|id| to_json_node(g.node(id).expect("node of node_ids"), &nc))
        .collect();
    out.nodes = (!nodes.is_empty()).then_some(nodes);
    let links: Vec<_> = g.edges().iter().map(to_json_link).collect();
    out.links = (!links.is_empty()).then_some(links);
    gostd::json::marshal_indent(&out, "", "  ").expect("MarshalIndent")
}

/// Go `streamGraph`: n nodes and m edges whose fields cover what the encoder treats specially:
/// HTML and control characters, non-ASCII, invalid UTF-8, omitempty fields, attribute maps (nil,
/// empty, multi-key) and weights. A Go string with an invalid UTF-8 byte (`\xff`, `\x80`) is a
/// `String` with U+FFFD (plan R3); both encoders write it as `EF BF BD`.
fn stream_graph(n: usize, m: usize) -> (Graph, BTreeMap<i64, Vec<String>>) {
    let labels = [
        "plain",
        "<b>&amp;</b>",
        "naïve café",
        "日本語   sep",
        "tab\there\nnl",
        "quote\"back\\slash",
        "bad\u{FFFD}utf8",
        "\"nodes\": null,",
        "del\x7f",
        "sep\u{2028}\u{2029}",
        "a&b",
        "a<b",
        "a>b",
        "a\\b",
        "a\"b",
        "a\x1fb",
        "a\u{FFFD}b",
    ];
    let mut g = Graph::new();
    let mut communities: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for i in 0..n {
        let mut node = Node {
            id: format!("n{i}"),
            label: labels[i % labels.len()].to_string(),
            file_type: "code".to_string(),
            source_file: "a/ü.go".to_string(),
            ..Node::default()
        };
        match i % 4 {
            1 => {
                node.source_location = format!("L{i}");
                node.attributes = BTreeMap::new();
            }
            2 => {
                node.computed_name = format!("computed {}", labels[(i + 1) % labels.len()]);
                node.attributes = BTreeMap::from([
                    ("z".to_string(), "<1>".to_string()),
                    ("a".to_string(), "é".to_string()),
                    ("\"nodes\": null,".to_string(), "x".to_string()),
                ]);
            }
            _ => {}
        }
        if !i.is_multiple_of(3) {
            communities
                .entry((i % 5) as i64)
                .or_default()
                .push(node.id.clone());
        }
        g.add_node(node);
    }
    let rels = ["calls", "imports", "contains", "rel<é>"];
    let confs = ["EXTRACTED", "INFERRED", "AMBIGUOUS", "OTHER"];
    for i in 0..m {
        let mut e = Edge {
            source: format!("n{}", i % n),
            target: format!("n{}", (i * 7 + 1) % n),
            relation: rel(&format!("{}{}", rels[i % rels.len()], i / n)),
            confidence: conf(confs[i % 4]),
            ..Edge::default()
        };
        match i % 3 {
            0 => e.set_type_only(i.is_multiple_of(2)), // no location
            1 => {
                e.source_file = file_of("b/日.py");
                e.source_location = line(9);
                e.weight = 0.1 + (i as f64) / 3.0;
                e.set_type_only(i.is_multiple_of(2));
                e.set_deferred(i % 2 == 1);
            }
            _ => {
                e.confidence_score = 1e-7 * (i as f64);
                if i.is_multiple_of(5) {
                    e.confidence_score = 1e21 * ((i + 1) as f64);
                    e.weight = -1e-7;
                }
            }
        }
        g.add_edge(e);
    }
    (g, communities)
}

#[test]
fn test_write_json_matches_marshal_indent() {
    // Go forces chunk boundaries with `jsonChunk = 8` and runs on GOMAXPROCS workers; here both
    // are arguments, and the worker counts below cover one worker and a batch of several.
    const CHUNK: usize = 8;
    let mut cases = vec![(0, 0), (1, 0), (1, 1), (2, 1)];
    // 300 crosses a batch of chunks on up to 37 CPUs.
    for n in [7, 8, 9, 15, 16, 17, 100, 300] {
        for m in [0, 1, 7, 8, 9, 16, 17, 100, 300] {
            cases.push((n, m));
        }
    }
    // Go's nil map and empty map both leave the key out; a Rust map cannot be nil.
    let unclassified = [
        BTreeMap::new(),
        BTreeMap::from([(".x\"y".to_string(), 2), (".<a>".to_string(), 1)]),
    ];
    for (nodes, edges) in cases {
        let (mut g, communities) = if nodes == 0 {
            (Graph::new(), BTreeMap::new())
        } else {
            stream_graph(nodes, edges)
        };
        for unc in &unclassified {
            for commit in ["", "abc123"] {
                g.unclassified = unc.clone();
                let want = whole_json(&g, &communities, commit);
                for workers in [1, 4] {
                    let mut got = Vec::new();
                    write_json(&mut got, &g, &communities, commit, workers, CHUNK)
                        .expect("writeJSON");
                    let text = std::str::from_utf8(&got).expect("graph.json is UTF-8");
                    assert!(
                        !text.contains(r#""source_location": """#)
                            && text
                                .matches("\"source_location\": \"L9\",\n      \"weight\"")
                                .count()
                                == (edges + 1) / 3,
                        "nodes={nodes} edges={edges}: a type-only edge changed a location\n got: {text}"
                    );
                    assert!(
                        got == want,
                        "nodes={nodes} edges={edges} unclassified={unc:?} commit={commit:?} workers={workers}: streamed output differs\n got: {text}\nwant: {}",
                        std::str::from_utf8(&want).expect("graph.json is UTF-8")
                    );
                }
            }
        }
    }
}
