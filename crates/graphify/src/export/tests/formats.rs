//! Port of `formats_test.go` (3 tests), then the upstream cases of `test_export.py` and
//! `test_export_control_characters.py` that hold for graphify-go v0.12.0, then the added test.

use std::fs;
use std::path::PathBuf;

use gostd::xml::{Token, tokens};

use crate::export::{csv_from_json, dot_from_json, graphml_from_json};
use crate::testutil::TempDir;

// sampleGraphJSON has labels with XML/DOT/CSV metacharacters to exercise escaping.
const SAMPLE_GRAPH_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"a","label":"A()","file_type":"code","source_file":"a.go","source_location":"L1","community":0,"norm_label":"a()"},
 {"id":"b","label":"B<x> \"q\"","file_type":"code","source_file":"b.go","community":1,"norm_label":"b"}
],
"links":[
 {"source":"a","target":"b","relation":"calls","confidence":"INFERRED","confidence_score":0.5}
],
"hyperedges":[]}"#;

/// Go `writeSample`: the temp dir (kept alive by the caller) and the graph.json in it.
fn write_sample() -> (TempDir, PathBuf) {
    let dir = TempDir::new();
    let json_path = dir.write("graph.json", SAMPLE_GRAPH_JSON.as_bytes());
    (dir, json_path)
}

/// What Go's `xml.Unmarshal` into `graphml` gives the tests: the number of `node` and `edge`
/// elements, and `node_data(id, key)`, the text of the `data` element of node `id` with that key.
struct GraphMl {
    nodes: usize,
    edges: usize,
    node_data: Vec<(String, String, String)>,
}

impl GraphMl {
    fn data(&self, id: &str, key: &str) -> Option<&str> {
        self.node_data
            .iter()
            .find(|(i, k, _)| i == id && k == key)
            .map(|(_, _, v)| v.as_str())
    }
}

/// Parses GraphML text with the XML tokenizer; panics on a syntax error.
fn parse_graphml(data: &[u8]) -> GraphMl {
    let toks = tokens(data).unwrap_or_else(|e| panic!("output is not valid XML: {e}"));
    let mut doc = GraphMl {
        nodes: 0,
        edges: 0,
        node_data: Vec::new(),
    };
    let mut node_id: Option<String> = None;
    let mut key: Option<String> = None;
    for t in toks {
        match t {
            Token::StartElement { name, attr } => {
                let attr_of = |n: &str| {
                    attr.iter()
                        .find(|a| a.name.local == n)
                        .map(|a| a.value.clone())
                };
                match name.local.as_str() {
                    "node" => {
                        doc.nodes += 1;
                        node_id = attr_of("id");
                    }
                    "edge" => doc.edges += 1,
                    "data" => {
                        if let (Some(id), Some(k)) = (&node_id, attr_of("key")) {
                            doc.node_data.push((id.clone(), k.clone(), String::new()));
                            key = Some(k);
                        }
                    }
                    _ => {}
                }
            }
            Token::CharData(text) => {
                if key.is_some() {
                    let last = doc.node_data.last_mut().expect("data element is open");
                    last.2
                        .push_str(std::str::from_utf8(&text).expect("chardata is UTF-8"));
                }
            }
            Token::EndElement { name } => match name.local.as_str() {
                "data" => key = None,
                "node" => node_id = None,
                _ => {}
            },
            _ => {}
        }
    }
    doc
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_graph_ml_from_json() {
    let (dir, json_path) = write_sample();
    let out = dir.join("graph.graphml");
    graphml_from_json(&json_path, &out).expect("GraphMLFromJSON");
    let data = fs::read(&out).expect("read");

    let doc = parse_graphml(&data);
    assert!(
        doc.nodes == 2 && doc.edges == 1,
        "got {} nodes / {} edges, want 2 / 1",
        doc.nodes,
        doc.edges
    );
    // The metacharacter label must round-trip through XML escaping.
    let b_label = doc.data("b", "label").unwrap_or_default();
    assert_eq!(b_label, r#"B<x> "q""#, "label round-trip");
}

#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_dot_from_json() {
    let (dir, json_path) = write_sample();
    let out = dir.join("graph.dot");
    dot_from_json(&json_path, &out).expect("DOTFromJSON");
    let s = String::from_utf8(fs::read(&out).expect("read")).expect("DOT is UTF-8");
    for want in [
        "digraph graphify {",
        r#""a" [label="A()"];"#,
        r#""a" -> "b" [label="calls"];"#,
        r#"\"q\""#, // the quote in b's label is escaped
    ] {
        assert!(s.contains(want), "DOT output missing {want:?}\n{s}");
    }
}

// Go reads both files back with csv.NewReader(...).ReadAll(); gostd has no csv reader, so the
// files are compared as text. Go's writer quotes exactly the fields with a comma, a quote or a
// line break: b's label `B<x> "q"` is quoted with the quote doubled, and an empty source_location
// is an empty field. The Go assertions follow from this text: 3 node rows (header + 2), header
// `id` .. `community`, row 2 label `B<x> "q"`, 2 edge rows, `erows[1][2]` = `calls`.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_csv_from_json() {
    let (dir, json_path) = write_sample();
    let nodes = dir.join("graph.nodes.csv");
    let edges = dir.join("graph.edges.csv");
    csv_from_json(&json_path, &nodes, &edges).expect("CSVFromJSON");

    let got = fs::read_to_string(&nodes).expect("read nodes.csv");
    assert_eq!(
        got,
        "id,label,file_type,source_file,source_location,community\n\
         a,A(),code,a.go,L1,0\n\
         b,\"B<x> \"\"q\"\"\",code,b.go,,1\n",
        "nodes.csv"
    );
    let got = fs::read_to_string(&edges).expect("read edges.csv");
    assert_eq!(
        got,
        "source,target,relation,confidence,confidence_score\n\
         a,b,calls,INFERRED,0.5\n",
        "edges.csv"
    );
}

// ---- Upstream (graphify v0.9.107) cases that hold for graphify-go v0.12.0 ----

// test_export.py::test_to_graphml_creates_file
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_graphml_creates_file() {
    let (dir, json_path) = write_sample();
    let out = dir.join("graph.graphml");
    graphml_from_json(&json_path, &out).expect("GraphMLFromJSON");
    assert!(out.exists(), "graph.graphml was not written");
}

// test_export.py::test_to_graphml_valid_xml
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_graphml_valid_xml() {
    let (dir, json_path) = write_sample();
    let out = dir.join("graph.graphml");
    graphml_from_json(&json_path, &out).expect("GraphMLFromJSON");
    let content = fs::read_to_string(&out).expect("read");
    assert!(content.contains("<graphml"), "no <graphml in {content}");
    assert!(content.contains("<node"), "no <node in {content}");
}

// test_export.py::test_to_graphml_has_community_attribute
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_to_graphml_has_community_attribute() {
    let (dir, json_path) = write_sample();
    let out = dir.join("graph.graphml");
    graphml_from_json(&json_path, &out).expect("GraphMLFromJSON");
    let content = fs::read_to_string(&out).expect("read");
    assert!(content.contains("community"), "no community in {content}");
}

// test_export_control_characters.py. A label or id with a C0 control character must not abort
// the export. Go's xml encoder writes each character XML 1.0 forbids as U+FFFD (xml.go
// `escapeText`), so the output is well-formed and carries the rest of the label. Tab, LF and CR
// are legal; Go writes them as the character references `&#x9;`, `&#xA;`, `&#xD;`, which decode
// back to the same character (so, unlike Python, CR round-trips too). The Python assertion that
// `"a\tb"` appears verbatim in the text does not hold for Go; the decoded label is compared
// instead. The label reaches graph.json as a JSON escape, so the sample is written with one.

/// A two-node graph.json whose node `a` has the label `LABEL`, given as the inside of a JSON
/// string, and the edge of the Python `_graph` helper.
fn ctl_graph_json(id: &str, label_json: &str) -> String {
    r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"ID","label":"LABEL","file_type":"document","source_file":"d.md","community":0,"norm_label":"x"},
 {"id":"b","label":"plain","file_type":"code","source_file":"b.py","community":0,"norm_label":"plain"}
],
"links":[
 {"source":"ID","target":"b","relation":"references","confidence":"INFERRED","confidence_score":0.85,"source_file":"d.md"}
],
"hyperedges":[]}"#
        .replace("ID", id)
        .replace("LABEL", label_json)
}

/// Writes `json` as graph.json, exports it as GraphML and returns the GraphML bytes.
fn graphml_of(json: &str) -> Vec<u8> {
    let dir = TempDir::new();
    let json_path = dir.write("graph.json", json.as_bytes());
    let out = dir.join("g.graphml");
    graphml_from_json(&json_path, &out).expect("GraphMLFromJSON");
    fs::read(&out).expect("read")
}

// Rejected by XML 1.0: JSON escape, and the character.
const BREAK: [(&str, char); 7] = [
    (r"\u0000", '\u{0}'),
    (r"\u0007", '\u{7}'),
    (r"\u0008", '\u{8}'),
    (r"\u000b", '\u{b}'),
    (r"\u000c", '\u{c}'),
    (r"\u001b", '\u{1b}'),
    (r"\u001f", '\u{1f}'),
];

// Legal in XML: JSON escape, and the character.
const KEEP: [(&str, char); 3] = [(r"\t", '\t'), (r"\n", '\n'), (r"\r", '\r')];

// test_graphml_survives_a_control_character
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_graphml_survives_a_control_character() {
    for (esc, ch) in BREAK {
        let data = graphml_of(&ctl_graph_json("a", &format!("Build {esc}log capture")));
        let doc = parse_graphml(&data); // must be well-formed XML
        assert_eq!(
            doc.data("a", "label"),
            Some("Build \u{FFFD}log capture"),
            "U+{:04X}",
            ch as u32
        );
        assert!(
            !data.contains(&(ch as u8)),
            "U+{:04X} reached the output",
            ch as u32
        );
    }
}

// test_graphml_keeps_the_whitespace_xml_allows
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_graphml_keeps_the_whitespace_xml_allows() {
    for (esc, ch) in KEEP {
        let data = graphml_of(&ctl_graph_json("a", &format!("a{esc}b")));
        let doc = parse_graphml(&data);
        assert_eq!(
            doc.data("a", "label"),
            Some(format!("a{ch}b").as_str()),
            "U+{:04X}",
            ch as u32
        );
    }
}

// test_graphml_survives_a_control_character_in_a_node_id: ids become XML attributes too.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_graphml_survives_a_control_character_in_a_node_id() {
    let data = graphml_of(&ctl_graph_json(r"we\u000bird", "x"));
    let doc = parse_graphml(&data);
    assert_eq!(doc.data("we\u{FFFD}ird", "label"), Some("x"));
}

// test_graphml_still_carries_the_readable_part_of_the_label
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_graphml_still_carries_the_readable_part_of_the_label() {
    let data = graphml_of(&ctl_graph_json(
        "a",
        r"Build \u001b[31mlog\u001b[0m capture",
    ));
    let text = String::from_utf8(data).expect("GraphML is UTF-8");
    assert!(text.contains("log") && text.contains("capture"), "{text}");
    assert!(!text.contains('\u{1b}'), "ESC reached the output: {text}");
}

// test_a_clean_label_round_trips_unchanged (the GraphML half)
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_a_clean_label_round_trips_unchanged() {
    let data = graphml_of(&ctl_graph_json("a", "Perfectly Ordinary Heading"));
    let text = String::from_utf8(data).expect("GraphML is UTF-8");
    assert!(text.contains("Perfectly Ordinary Heading"), "{text}");
}

// ---- Added test ----

// formats.go:165 writes confidence_score with strconv.FormatFloat(f, 'g', -1, 64): the shortest
// digits that round-trip, in %e form when exp < -4 || exp >= eprec, else %f, where exp = dp - 1
// and shortest sets eprec = 6 (ftoa.go `%g`). The %e exponent has at least two digits
// (`e-07`, `e+06`). Expected strings are written out by that rule:
//   0.85 -> digits 85, dp 0, exp -1            -> "0.85"
//   1e-07 -> digits 1, dp -6, exp -7 < -4      -> "1e-07"
//   1 -> digits 1, dp 1, exp 0                 -> "1"
//   0.5 -> digits 5, dp 0, exp -1              -> "0.5"
//   0.0001 -> exp -4, not < -4                 -> "0.0001"
//   0.00001 -> exp -5 < -4                     -> "1e-05"
//   100000 -> digits 1, dp 6, exp 5 < 6        -> "100000"
//   1000000 -> dp 7, exp 6 >= 6                -> "1e+06"
//   123456789 -> digits 123456789, exp 8 >= 6  -> "1.23456789e+08"
//   1e21 -> exp 21 >= 6                        -> "1e+21"
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn csv_confidence_score_g_format() {
    const CASES: [(&str, &str); 10] = [
        ("0.85", "0.85"),
        ("1e-07", "1e-07"),
        ("1", "1"),
        ("0.5", "0.5"),
        ("0.0001", "0.0001"),
        ("0.00001", "1e-05"),
        ("100000", "100000"),
        ("1000000", "1e+06"),
        ("123456789", "1.23456789e+08"),
        ("1e21", "1e+21"),
    ];
    for (json_num, want) in CASES {
        let dir = TempDir::new();
        let json = format!(
            r#"{{"directed":true,"multigraph":false,"graph":{{}},"nodes":[],"links":[{{"source":"a","target":"b","relation":"calls","confidence":"INFERRED","confidence_score":{json_num}}}],"hyperedges":[]}}"#
        );
        let json_path = dir.write("graph.json", json.as_bytes());
        let nodes = dir.join("n.csv");
        let edges = dir.join("e.csv");
        csv_from_json(&json_path, &nodes, &edges).expect("CSVFromJSON");
        assert_eq!(
            fs::read_to_string(&edges).expect("read"),
            format!(
                "source,target,relation,confidence,confidence_score\na,b,calls,INFERRED,{want}\n"
            ),
            "confidence_score {json_num}"
        );
    }
}
