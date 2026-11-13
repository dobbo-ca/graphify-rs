//! `xml-enc`: gostd::xml::marshal_indent of graphify's GraphML tree against Go `xml.MarshalIndent`
//! (see parity/go/oracle/xml.go and graphify-go internal/export/formats.go:60-110).

use super::Op;
use gostd::xml::{HEADER, Node, marshal_indent};

/// Decodes `x<hex>` (the input encoding of xml.go) to a string.
fn unhex(s: &str) -> String {
    let nib = |c: u8| char::from(c).to_digit(16).unwrap() as u8;
    let (pairs, _) = s.as_bytes()[1..].as_chunks::<2>();
    let bytes: Vec<u8> = pairs.iter().map(|p| (nib(p[0]) << 4) | nib(p[1])).collect();
    String::from_utf8(bytes).unwrap()
}

fn node(name: &str, attrs: &[(&str, &str)], chardata: &str, children: Vec<Node>) -> Node {
    Node {
        name: name.to_string(),
        attrs: attrs
            .iter()
            .map(|&(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        chardata: chardata.to_string(),
        children,
    }
}

fn data(key: &str, val: &str) -> Node {
    node("data", &[("key", key)], val, Vec::new())
}

/// The six `gmlKey` rows of formats.go:77-82: id, for, attr.name, attr.type.
const KEYS: [[&str; 4]; 6] = [
    ["label", "node", "label", "string"],
    ["file_type", "node", "file_type", "string"],
    ["source_file", "node", "source_file", "string"],
    ["community", "node", "community", "int"],
    ["relation", "edge", "relation", "string"],
    ["confidence", "edge", "confidence", "string"],
];

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let text = std::str::from_utf8(input).unwrap();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for line in text.split('\n').filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split(' ').collect();
        match f[0] {
            "N" => {
                let mut d = vec![
                    data("label", &unhex(f[2])),
                    data("file_type", &unhex(f[3])),
                    data("source_file", &unhex(f[4])),
                ];
                if f[5] != "-" {
                    d.push(data("community", f[5]));
                }
                let id = unhex(f[1]);
                nodes.push(node("node", &[("id", id.as_str())], "", d));
            }
            "E" => {
                let (source, target) = (unhex(f[1]), unhex(f[2]));
                let d = vec![
                    data("relation", &unhex(f[3])),
                    data("confidence", &unhex(f[4])),
                ];
                let attrs = [("source", source.as_str()), ("target", target.as_str())];
                edges.push(node("edge", &attrs, "", d));
            }
            other => panic!("bad record {other}"),
        }
    }
    let mut children: Vec<Node> = KEYS
        .iter()
        .map(|[id, f, n, t]| {
            let attrs = [
                ("id", *id),
                ("for", *f),
                ("attr.name", *n),
                ("attr.type", *t),
            ];
            node("key", &attrs, "", Vec::new())
        })
        .collect();
    nodes.extend(edges);
    children.push(node("graph", &[("edgedefault", "directed")], "", nodes));
    let doc = node(
        "graphml",
        &[("xmlns", "http://graphml.graphdrawing.org/xmlns")],
        "",
        children,
    );
    let mut out = HEADER.as_bytes().to_vec();
    out.extend(marshal_indent(&doc, "", "  "));
    out.push(b'\n');
    vec![("graphml", Ok(out))]
}
