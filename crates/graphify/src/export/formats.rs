//! Port of `formats.go`: GraphML, DOT and CSV from a graph.json.

use std::fs::{self, File};
use std::path::Path;

use gostd::csv::Writer;
use gostd::json::unmarshal;
use gostd::strconv::format_float;
use gostd::xml::{HEADER, Node, marshal_indent};

use super::export::{Error, JsonGraph, path_err, write_file};

// Additional export formats derived from a built graph.json. They read the canonical artifact
// rather than rebuilding, so `graphify export <format>` reflects exactly what was committed.
// GraphML and CSV use the stdlib encoders for correct escaping; DOT is small enough to format
// directly.

/// Go `readGraphJSON` (`formats.go:18`).
pub(super) fn read_graph_json(path: &Path) -> Result<JsonGraph, Error> {
    // Go's `os.ReadFile` fails on a directory at the read, not the open.
    let data = fs::read(path).map_err(|e| {
        let op = if e.raw_os_error() == Some(21) {
            "read"
        } else {
            "open"
        };
        path_err(op, path, &e)
    })?;
    unmarshal::<JsonGraph>(&data).map_err(Error::Json)
}

// --- GraphML ---

/// An element with attributes and text, as Go's `gmlData`, `gmlKey` and the like marshal.
fn element(name: &str, attrs: &[(&str, &str)], chardata: &str, children: Vec<Node>) -> Node {
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

/// Go `gmlData{Key: key, Val: val}`.
fn gml_data(key: &str, val: &str) -> Node {
    element("data", &[("key", key)], val, Vec::new())
}

/// Go `GraphMLFromJSON` (`formats.go:69`).
pub fn graphml_from_json(json_path: &Path, out_path: &Path) -> Result<(), Error> {
    let g = read_graph_json(json_path)?;
    let mut children: Vec<Node> = [
        ("label", "node", "label", "string"),
        ("file_type", "node", "file_type", "string"),
        ("source_file", "node", "source_file", "string"),
        ("community", "node", "community", "int"),
        ("relation", "edge", "relation", "string"),
        ("confidence", "edge", "confidence", "string"),
    ]
    .iter()
    .map(|&(id, for_, attr_name, attr_type)| {
        let attrs = [
            ("id", id),
            ("for", for_),
            ("attr.name", attr_name),
            ("attr.type", attr_type),
        ];
        element("key", &attrs, "", Vec::new())
    })
    .collect();
    let mut graph = Vec::new();
    for n in g.nodes.as_deref().unwrap_or_default() {
        let mut data = vec![
            gml_data("label", &n.label),
            gml_data("file_type", &n.file_type),
            gml_data("source_file", &n.source_file),
        ];
        if let Some(c) = n.community {
            data.push(gml_data("community", &c.to_string()));
        }
        graph.push(element("node", &[("id", &n.id)], "", data));
    }
    for e in g.links.as_deref().unwrap_or_default() {
        let data = vec![
            gml_data("relation", &e.relation),
            gml_data("confidence", &e.confidence),
        ];
        let attrs = [("source", e.source.as_str()), ("target", e.target.as_str())];
        graph.push(element("edge", &attrs, "", data));
    }
    children.push(element("graph", &[("edgedefault", "directed")], "", graph));
    let doc = element(
        "graphml",
        &[("xmlns", "http://graphml.graphdrawing.org/xmlns")],
        "",
        children,
    );
    let mut out = HEADER.as_bytes().to_vec();
    out.extend(marshal_indent(&doc, "", "  "));
    out.push(b'\n');
    write_file(out_path, &out)
}

// --- DOT (Graphviz) ---

/// Go `DOTFromJSON` (`formats.go:113`).
pub fn dot_from_json(json_path: &Path, out_path: &Path) -> Result<(), Error> {
    let g = read_graph_json(json_path)?;
    let mut b = String::new();
    b.push_str("digraph graphify {\n");
    b.push_str("  node [shape=box];\n");
    for n in g.nodes.as_deref().unwrap_or_default() {
        b.push_str(&format!(
            "  {} [label={}];\n",
            dot_quote(&n.id),
            dot_quote(&n.label)
        ));
    }
    for e in g.links.as_deref().unwrap_or_default() {
        b.push_str(&format!(
            "  {} -> {} [label={}];\n",
            dot_quote(&e.source),
            dot_quote(&e.target),
            dot_quote(&e.relation)
        ));
    }
    b.push_str("}\n");
    write_file(out_path, b.as_bytes())
}

/// Go `dotQuote` (`formats.go:133`): `s` as a DOT double-quoted ID, escaping quotes and
/// backslashes and flattening newlines.
fn dot_quote(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for c in s.chars() {
        match c {
            '\\' => q.push_str("\\\\"),
            '"' => q.push_str("\\\""),
            '\n' => q.push_str("\\n"),
            '\r' => {}
            c => q.push(c),
        }
    }
    q.push('"');
    q
}

// --- CSV ---

/// Go `CSVFromJSON` (`formats.go:141`).
pub fn csv_from_json(json_path: &Path, nodes_path: &Path, edges_path: &Path) -> Result<(), Error> {
    let g = read_graph_json(json_path)?;
    write_csv(
        nodes_path,
        &[
            "id",
            "label",
            "file_type",
            "source_file",
            "source_location",
            "community",
        ],
        |w| {
            for n in g.nodes.as_deref().unwrap_or_default() {
                let comm = n.community.map(|c| c.to_string()).unwrap_or_default();
                w.write(&[
                    &n.id,
                    &n.label,
                    &n.file_type,
                    &n.source_file,
                    &n.source_location,
                    &comm,
                ])?;
            }
            Ok(())
        },
    )?;
    write_csv(
        edges_path,
        &[
            "source",
            "target",
            "relation",
            "confidence",
            "confidence_score",
        ],
        |w| {
            for e in g.links.as_deref().unwrap_or_default() {
                w.write(&[
                    &e.source,
                    &e.target,
                    &e.relation,
                    &e.confidence,
                    &format_float(e.confidence_score, b'g'),
                ])?;
            }
            Ok(())
        },
    )
}

/// Go `writeCSV` (`formats.go:173`).
fn write_csv(
    path: &Path,
    header: &[&str],
    rows: impl FnOnce(&mut Writer<File>) -> std::io::Result<()>,
) -> Result<(), Error> {
    let f = File::create(path).map_err(|e| path_err("open", path, &e))?;
    let mut w = Writer::new(f);
    w.write(header)
        .and_then(|()| rows(&mut w))
        .and_then(|()| w.flush())
        .map_err(|e| path_err("write", path, &e))
}
