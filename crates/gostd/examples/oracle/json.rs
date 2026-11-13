//! `json`: gostd::json against Go encoding/json (see parity/go/oracle/json.go).

use std::collections::BTreeMap;

use super::Op;
use gostd::json::{
    self, DecodeState, EncodeState, Encoder, Error, Marshal, RawMessage, Unmarshal, Value,
};

// The typed shapes, mirroring the graphify-go types in parity/go/oracle/json.go. A Go nil-able
// slice or map is an `Option`, `any` is `Value`.

/// graphify-go internal/query/query.go:22-33 (`Graph`; the unexported fields are left out).
#[derive(Default)]
struct Graph {
    nodes: Option<Vec<Node>>,
    links: Option<Vec<Link>>,
    attrs: GraphAttrs,
    built_at_commit: String,
    path: String,
}
gostd::json_fields!(decode Graph, "main.Graph", {
    nodes => "nodes",
    links => "links",
    attrs => "graph",
    built_at_commit => "built_at_commit" omitempty,
    path => "-",
});

/// query.go:38-41 (`GraphAttrs`).
#[derive(Default)]
struct GraphAttrs {
    unclassified_files: i64,
    unclassified_exts: Option<BTreeMap<String, i64>>,
}
gostd::json_fields!(decode GraphAttrs, "main.GraphAttrs", {
    unclassified_files => "unclassified_files",
    unclassified_exts => "unclassified_extensions",
});

/// query.go:77-87 (`Node`).
#[derive(Default)]
struct Node {
    id: String,
    label: String,
    file_type: String,
    source_file: String,
    source_location: String,
    community: Option<i64>,
    norm_label: String,
    computed_name: String,
    attributes: Option<BTreeMap<String, String>>,
}
gostd::json_fields!(decode Node, "main.Node", {
    id => "id",
    label => "label",
    file_type => "file_type",
    source_file => "source_file",
    source_location => "source_location",
    community => "community",
    norm_label => "norm_label",
    computed_name => "computed_name",
    attributes => "attributes",
});

/// query.go:90-97 (`Link`).
#[derive(Default)]
struct Link {
    source: String,
    target: String,
    relation: String,
    confidence: String,
    source_file: String,
    source_location: String,
}
gostd::json_fields!(decode Link, "main.Link", {
    source => "source",
    target => "target",
    relation => "relation",
    confidence => "confidence",
    source_file => "source_file",
    source_location => "source_location",
});

/// graphify-go internal/export/export.go:39-49 (`jsonNode`).
#[derive(Default)]
struct JsonNode {
    id: String,
    label: String,
    file_type: String,
    source_file: String,
    source_location: String,
    community: Option<i64>,
    norm_label: String,
    computed_name: String,
    attributes: Option<BTreeMap<String, String>>,
}
gostd::json_fields!(decode JsonNode, "main.jsonNode", {
    id => "id",
    label => "label",
    file_type => "file_type",
    source_file => "source_file",
    source_location => "source_location" omitempty,
    community => "community",
    norm_label => "norm_label",
    computed_name => "computed_name" omitempty,
    attributes => "attributes" omitempty,
});

/// export.go:51-60 (`jsonLink`).
#[derive(Default)]
struct JsonLink {
    source: String,
    target: String,
    relation: String,
    confidence: String,
    source_file: String,
    source_location: String,
    weight: f64,
    confidence_score: f64,
}
gostd::json_fields!(decode JsonLink, "main.jsonLink", {
    source => "source",
    target => "target",
    relation => "relation",
    confidence => "confidence",
    source_file => "source_file" omitempty,
    source_location => "source_location" omitempty,
    weight => "weight" omitempty,
    confidence_score => "confidence_score",
});

/// export.go:65-68 (`graphAttrs`).
#[derive(Default)]
struct ExportAttrs {
    unclassified_files: i64,
    unclassified_exts: Option<BTreeMap<String, i64>>,
}
gostd::json_fields!(decode ExportAttrs, "main.graphAttrs", {
    unclassified_files => "unclassified_files",
    unclassified_exts => "unclassified_extensions" omitempty,
});

/// export.go:70-78 (`jsonGraph`).
#[derive(Default)]
struct JsonGraph {
    directed: bool,
    multigraph: bool,
    graph: ExportAttrs,
    nodes: Option<Vec<JsonNode>>,
    links: Option<Vec<JsonLink>>,
    hyperedges: Option<Vec<Value>>,
    built_at_commit: String,
}
gostd::json_fields!(decode JsonGraph, "main.jsonGraph", {
    directed => "directed",
    multigraph => "multigraph",
    graph => "graph",
    nodes => "nodes",
    links => "links",
    hyperedges => "hyperedges",
    built_at_commit => "built_at_commit" omitempty,
});

/// graphify-go internal/query/merge.go:21-29 (`rawGraph`).
#[derive(Default)]
struct RawGraph {
    directed: bool,
    multigraph: bool,
    graph: RawMessage,
    nodes: Option<Vec<RawMessage>>,
    links: Option<Vec<RawMessage>>,
    hyperedges: RawMessage,
    built_at_commit: RawMessage,
}
gostd::json_fields!(decode RawGraph, "main.rawGraph", {
    directed => "directed",
    multigraph => "multigraph",
    graph => "graph",
    nodes => "nodes",
    links => "links",
    hyperedges => "hyperedges" omitempty,
    built_at_commit => "built_at_commit" omitempty,
});

/// graphify-go cmd/graphify/serve.go:131-136 (`rpcRequest`).
#[derive(Default)]
struct RpcRequest {
    jsonrpc: String,
    id: RawMessage,
    method: String,
    params: RawMessage,
}
gostd::json_fields!(decode RpcRequest, "main.rpcRequest", {
    jsonrpc => "jsonrpc",
    id => "id" omitempty,
    method => "method",
    params => "params" omitempty,
});

/// serve.go:138-143 (`rpcResponse`).
struct RpcResponse {
    jsonrpc: String,
    id: RawMessage,
    result: Value,
    error: Option<RpcError>,
}
gostd::json_fields!(RpcResponse, "main.rpcResponse", {
    jsonrpc => "jsonrpc",
    id => "id" omitempty,
    result => "result" omitempty,
    error => "error" omitempty,
});

/// serve.go:145-148 (`rpcError`).
struct RpcError {
    code: i64,
    message: String,
}
gostd::json_fields!(RpcError, "main.rpcError", { code => "code", message => "message" });

/// Go `int64`: gostd's `i64` is Go `int` (`GO_TYPE` `"int"`), which error text would name.
#[derive(Default)]
struct I64(i64);

impl Unmarshal for I64 {
    const GO_TYPE: &'static str = "int64";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if let Some(n) = d.read_i64(Self::GO_TYPE, 64)? {
            self.0 = n;
        }
        Ok(())
    }
}

impl Marshal for I64 {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.0.marshal_json(e)
    }
}

/// graphify-go internal/cache/cache.go:43-48 (`StatEntry`).
#[derive(Default)]
struct StatEntry {
    size: I64,
    mtime_ns: I64,
    hash: String,
    indexed_at_ns: I64,
}
gostd::json_fields!(decode StatEntry, "main.StatEntry", {
    size => "size",
    mtime_ns => "mtime_ns",
    hash => "hash",
    indexed_at_ns => "indexed_at_ns",
});

/// cache.go:93 (`StatIndex`, a named `map[string]StatEntry`, which error text names).
#[derive(Default)]
struct StatIndex(Option<BTreeMap<String, StatEntry>>);

impl Unmarshal for StatIndex {
    const GO_TYPE: &'static str = "main.StatIndex";
    /// As `Option<BTreeMap>` (a nil-able map), but the map is decoded with this type current.
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), Error> {
        if d.peek_null() {
            d.skip_value()?;
            self.0 = None;
            return Ok(());
        }
        self.0.get_or_insert_default().unmarshal_json(d)
    }
}

impl Marshal for StatIndex {
    fn marshal_json(&self, e: &mut EncodeState) -> Result<(), Error> {
        self.0.marshal_json(e)
    }
}

/// cache.go:113-116 (`statFile`).
#[derive(Default)]
struct StatFile {
    v: String,
    files: StatIndex,
}
gostd::json_fields!(decode StatFile, "main.statFile", { v => "v", files => "files" });

fn records(mut input: &[u8]) -> Vec<&[u8]> {
    let mut v = Vec::new();
    while let Some((len, rest)) = input.split_first_chunk::<4>() {
        let (s, rest) = rest.split_at(u32::from_be_bytes(*len) as usize);
        v.push(s);
        input = rest;
    }
    v
}

fn line(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(b);
    out.push(b'\n');
}

fn strings(input: &[u8]) -> Vec<Op> {
    let (mut html, mut nohtml) = (Vec::new(), Vec::new());
    for s in records(input) {
        json::append_string(&mut html, s, true);
        html.push(b'\n');
        json::append_string(&mut nohtml, s, false);
        nohtml.push(b'\n');
    }
    vec![("html", Ok(html)), ("nohtml", Ok(nohtml))]
}

fn floats(input: &[u8]) -> Vec<Op> {
    let text = std::str::from_utf8(input).expect("json floats are ASCII");
    let mut out = Vec::new();
    for l in text.strip_suffix('\n').unwrap_or(text).split('\n') {
        let f = f64::from_bits(u64::from_str_radix(l, 16).unwrap());
        if let Err(e) = json::append_float(&mut out, f) {
            out.extend_from_slice(e.to_string().as_bytes());
        }
        out.push(b'\n');
    }
    vec![("json", Ok(out))]
}

/// json.go `jsRawField`.
struct RawField {
    m: RawMessage,
}
gostd::json_fields!(RawField, "main.jsRawField", { m => "M" });

/// The `raw` op: `marshal` of a `RawField` per record, or the error text.
fn raw(input: &[u8]) -> Vec<Op> {
    let mut out = Vec::new();
    for d in records(input) {
        match json::marshal(&RawField {
            m: RawMessage(d.to_vec()),
        }) {
            Ok(b) => line(&mut out, &b),
            Err(e) => line(&mut out, e.to_string().as_bytes()),
        }
    }
    vec![("marshal", Ok(out))]
}

fn docs(input: &[u8]) -> Vec<Op> {
    let (mut valid, mut compact, mut compact_html, mut indent) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for d in records(input) {
        line(&mut valid, if json::valid(d) { b"1" } else { b"0" });
        let mut c = Vec::new();
        match json::compact(&mut c, d, false) {
            Ok(()) => {
                line(&mut compact, &c);
                let mut h = Vec::new();
                json::html_escape(&mut h, &c);
                line(&mut compact_html, &h);
            }
            Err(e) => {
                line(&mut compact, e.to_string().as_bytes());
                line(&mut compact_html, e.to_string().as_bytes());
            }
        }
        let mut i = Vec::new();
        let rec = match json::indent(&mut i, d, ">", "\t") {
            Ok(()) => i,
            Err(e) => [&[0][..], e.to_string().as_bytes()].concat(),
        };
        indent.extend_from_slice(&(rec.len() as u32).to_be_bytes());
        indent.extend_from_slice(&rec);
    }
    vec![
        ("valid", Ok(valid)),
        ("compact", Ok(compact)),
        ("compact_html", Ok(compact_html)),
        ("indent", Ok(indent)),
    ]
}

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    if name == "strings" {
        strings(input)
    } else if name == "floats" {
        floats(input)
    } else if name == "raw" {
        raw(input)
    } else if name.starts_with("docs-") {
        docs(input)
    } else if let Some(rest) = name.strip_prefix("typed-") {
        typed_run(rest, input)
    } else {
        Vec::new()
    }
}

/// The typed ops (see json.go): unmarshal (the error text, or empty), value (`marshal` of the
/// value after `unmarshal_into`, error or not), indent (`marshal_indent` of it, "" and two spaces).
fn typed<T: Unmarshal + Marshal + Default + Send>(input: &[u8]) -> (T, Vec<Op>) {
    let mut v = T::default();
    let u = json::unmarshal_into(input, &mut v);
    let ops = vec![
        (
            "unmarshal",
            u.map(|()| Vec::new()).map_err(|e| e.to_string()),
        ),
        ("value", json::marshal(&v).map_err(|e| e.to_string())),
        (
            "indent",
            json::marshal_indent(&v, "", "  ").map_err(|e| e.to_string()),
        ),
    ];
    (v, ops)
}

/// `name` is the case name after "typed-": the shape, "-" and a number.
fn typed_run(name: &str, input: &[u8]) -> Vec<Op> {
    match name.rsplit_once('-').map_or(name, |(shape, _)| shape) {
        "export" => typed::<JsonGraph>(input).1,
        "graph" => typed::<Graph>(input).1,
        "mcp" => typed::<Option<BTreeMap<String, RawMessage>>>(input).1,
        "rawgraph" => typed::<RawGraph>(input).1,
        "rpc" => {
            let (req, mut ops) = typed::<RpcRequest>(input);
            let ok = ops[0].1.is_ok();
            ops.push(("encode", rpc_reply(&req, ok)));
            ops
        }
        "stat" => typed::<StatFile>(input).1,
        _ => Vec::new(),
    }
}

/// serve.go's handle (:176-201) for a request that names no known method, through an `Encoder`
/// (:155-160): the parse-error response if `unmarshal` failed, else "method not found" with the
/// request's id.
fn rpc_reply(req: &RpcRequest, ok: bool) -> Result<Vec<u8>, String> {
    let mut resp = RpcResponse {
        jsonrpc: "2.0".to_string(),
        id: RawMessage::default(),
        result: Value::Null,
        error: Some(RpcError {
            code: -32700,
            message: "parse error".to_string(),
        }),
    };
    if ok {
        resp.id = req.id.clone();
        resp.error = Some(RpcError {
            code: -32601,
            message: format!("method not found: {}", req.method),
        });
    }
    let mut buf = Vec::new();
    Encoder::new(&mut buf)
        .encode(&resp)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}
