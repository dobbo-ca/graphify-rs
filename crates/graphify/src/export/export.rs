//! Port of `export.go`: graph.json, the shrink guard and `norm_label`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use gostd::json::{Error as JsonError, RawMessage, Value, marshal_indent, unmarshal};
use gostd::strings::{bytes_trim_space, to_lower};
use gostd::unicode::{self, MN};
use gostd::xtext::norm::nfkd;

use super::oldnodes::{OldNode, old_nodes};
use crate::cluster::node_community;
use crate::fsutil::write_atomic;
use crate::model::{Edge, Graph, Loc, Node, append_json_float, append_json_string};
use crate::security::sanitize_label;

/// Go `ErrGraphShrink`: `to_json` would replace a larger graph.json with a smaller one.
pub const ERR_GRAPH_SHRINK: &str = "refusing to overwrite graph.json with a smaller graph";

/// Go `ErrGraphUnverifiable`: the existing graph.json cannot be parsed to compare sizes.
pub const ERR_GRAPH_UNVERIFIABLE: &str = "refusing to overwrite unparseable graph.json";

/// The errors of this module. `Display` is Go's text byte for byte (plan R6); the two guard
/// errors hold the whole wrapped message, which starts with their sentinel text.
#[derive(Debug)]
pub enum Error {
    /// Go `fmt.Errorf("%w: ...", ErrGraphShrink, ...)`.
    GraphShrink(String),
    /// Go `fmt.Errorf("%w %s to verify ...", ErrGraphUnverifiable, ...)`.
    GraphUnverifiable(String),
    Io(io::Error),
    Json(JsonError),
    /// Any other Go `errors.New` or `fmt.Errorf` text (`export.go:137`, `okf.go:278`).
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::GraphShrink(s) | Error::GraphUnverifiable(s) | Error::Other(s) => f.write_str(s),
            Error::Io(e) => write!(f, "{e}"),
            Error::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

/// Go's `*os.PathError` text for a failed `op` on `path` (plan hazard "Go I/O error text").
pub(super) fn path_err(op: &str, path: &Path, e: &io::Error) -> Error {
    Error::Other(crate::goerr::path_error(op, path, e).to_string())
}

/// Go `os.WriteFile(path, data, 0o644)`.
pub(super) fn write_file(path: &Path, data: &[u8]) -> Result<(), Error> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(path)
        .map_err(|e| path_err("open", path, &e))?;
    // ponytail: Go also returns the close error; std closes on drop and drops it. A checked close
    // needs unsafe (or an fsync per file); close errors on a local file are rare.
    f.write_all(data).map_err(|e| path_err("write", path, &e))
}

/// Go `os.MkdirAll(path, 0o755)` (os/path.go): an existing non-directory is ENOTDIR on `path`;
/// otherwise the parent is made first, so an error names the component that failed, as Go's do.
/// The recursion is one level per path component, not per input byte.
pub(super) fn mkdir_all(path: &Path) -> Result<(), Error> {
    use std::os::unix::ffi::OsStrExt;
    if let Ok(m) = fs::metadata(path) {
        if m.is_dir() {
            return Ok(());
        }
        return Err(path_err(
            "mkdir",
            path,
            &io::Error::from_raw_os_error(20), // ENOTDIR
        ));
    }
    // Slow path: make sure the parent exists, then make `path`.
    let b = path.as_os_str().as_bytes();
    let mut i = b.len();
    while i > 0 && b[i - 1] == b'/' {
        i -= 1; // skip trailing separators
    }
    let mut j = i;
    while j > 0 && b[j - 1] != b'/' {
        j -= 1; // scan back over the last element
    }
    if j > 1 {
        let parent = Path::new(std::ffi::OsStr::from_bytes(&b[..j - 1]));
        mkdir_all(parent)?;
    }
    if let Err(e) = DirBuilder::new().mode(0o755).create(path) {
        // Handle arguments like "foo/." by double-checking that the directory does not exist.
        if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
            return Ok(());
        }
        return Err(path_err("mkdir", path, &e));
    }
    Ok(())
}

/// Go `jsonNode` (`export.go:43`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct JsonNode {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) file_type: String,
    pub(super) source_file: String,
    pub(super) source_location: String,
    pub(super) community: Option<i64>,
    pub(super) norm_label: String,
    pub(super) computed_name: String,
    pub(super) attributes: BTreeMap<String, String>,
}

gostd::json_fields!(decode JsonNode, "export.jsonNode", {
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

/// Go `jsonLink` (`export.go:55`). `source_location` is the text of Go's `model.Loc`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct JsonLink {
    pub(super) source: String,
    pub(super) target: String,
    pub(super) relation: String,
    pub(super) confidence: String,
    pub(super) deferred: bool,
    pub(super) source_file: String,
    pub(super) source_location: Loc,
    pub(super) weight: f64,
    pub(super) confidence_score: f64,
}

gostd::json_fields!(decode JsonLink, "export.jsonLink", {
    source => "source",
    target => "target",
    relation => "relation",
    confidence => "confidence",
    deferred => "deferred" omitempty,
    source_file => "source_file" omitempty,
    source_location => "source_location" omitempty,
    weight => "weight" omitempty,
    confidence_score => "confidence_score",
});

/// Go `graphAttrs` (`export.go:70`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct GraphAttrs {
    pub(super) unclassified_files: i64,
    pub(super) unclassified_exts: BTreeMap<String, i64>,
}

gostd::json_fields!(decode GraphAttrs, "export.graphAttrs", {
    unclassified_files => "unclassified_files",
    unclassified_exts => "unclassified_extensions" omitempty,
});

/// Go `jsonGraph` (`export.go:75`). A Go nil slice is `None`, which encodes as `null`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct JsonGraph {
    pub(super) directed: bool,
    pub(super) multigraph: bool,
    pub(super) graph: GraphAttrs,
    pub(super) nodes: Option<Vec<JsonNode>>,
    pub(super) links: Option<Vec<JsonLink>>,
    pub(super) hyperedges: Option<Vec<Value>>,
    pub(super) built_at_commit: String,
}

gostd::json_fields!(decode JsonGraph, "export.jsonGraph", {
    directed => "directed",
    multigraph => "multigraph",
    graph => "graph",
    nodes => "nodes",
    links => "links",
    hyperedges => "hyperedges",
    built_at_commit => "built_at_commit" omitempty,
});

/// Go `confidenceScore` (`export.go:29`): a map, so an unknown confidence is 0.
fn confidence_score(confidence: &str) -> f64 {
    match confidence {
        "EXTRACTED" => 1.0,
        "INFERRED" => 0.5,
        "AMBIGUOUS" => 0.2,
        _ => 0.0,
    }
}

/// Go `jsonChunk` (`export.go:109`): nodes or links one worker encodes at a time.
pub(super) const JSON_CHUNK: usize = 512;

/// Go `envelopeSplit` (`export.go:114`): where the node and link arrays sit in the indented
/// envelope when both are nil.
const ENVELOPE_SPLIT: &[u8] = b"\n  \"nodes\": null,\n  \"links\": null,\n";

/// Go `ToJSON` (`export.go:94`). `workers` is the encoder fan-out (plan R5).
pub fn to_json(
    g: &Graph,
    c: &BTreeMap<i64, Vec<String>>,
    path: &Path,
    built_at_commit: &str,
    force: bool,
    workers: usize,
) -> Result<(), Error> {
    if !force {
        guard_no_shrink(path, g.num_nodes())?;
    }

    // `write_atomic` carries `io::Error`: the export error of `write_json` waits here.
    let mut failed = None;
    write_atomic(path, 0o644, |w| {
        write_json(w, g, c, built_at_commit, workers, JSON_CHUNK).map_err(|e| match e {
            Error::Io(e) => e,
            e => {
                failed = Some(e);
                io::Error::other("write_json")
            }
        })
    })
    // fsutil's errors already carry Go's text for their step (open/write/chmod/rename/createtemp).
    .map_err(|e| failed.take().unwrap_or_else(|| Error::Other(e.to_string())))
}

/// Go `writeJSON` (`export.go:119`): streams the document `marshal_indent(JsonGraph, "", "  ")`
/// would produce, with chunks of `chunk` records on `workers` threads.
pub(super) fn write_json(
    w: &mut dyn Write,
    g: &Graph,
    communities: &BTreeMap<i64, Vec<String>>,
    built_at_commit: &str,
    workers: usize,
    chunk: usize,
) -> Result<(), Error> {
    let nc = node_community(communities);
    let mut out = JsonGraph {
        hyperedges: Some(Vec::new()),
        built_at_commit: built_at_commit.to_string(),
        ..JsonGraph::default()
    };
    for (ext, n) in &g.unclassified {
        out.graph.unclassified_exts.insert(ext.clone(), *n);
        out.graph.unclassified_files += *n;
    }
    let env = marshal_indent(&out, "", "  ").map_err(Error::Json)?;
    let Some(at) = env
        .windows(ENVELOPE_SPLIT.len())
        .position(|win| win == ENVELOPE_SPLIT)
    else {
        return Err(Error::Other(
            "export: graph.json envelope has an unexpected shape".to_string(),
        ));
    };

    w.write_all(&env[..at])?;
    w.write_all(b"\n  \"nodes\": ")?;
    let ids = g.node_ids();
    write_chunks(w, ids.len(), workers, chunk, &|b, i| {
        let n = g.node(&ids[i]).expect("node of node_ids");
        append_json_node(b, &to_json_node(n, &nc))
    })?;
    w.write_all(b",\n  \"links\": ")?;
    let edges = g.edges();
    write_chunks(w, edges.len(), workers, chunk, &|b, i| {
        append_json_link(b, &to_json_link(&edges[i]))
    })?;
    w.write_all(b",\n")?;
    w.write_all(&env[at + ENVELOPE_SPLIT.len()..])?;
    Ok(())
}

/// Go `writeChunks` (`export.go:172`): writes the indented top-level array of `n` records. `enc`
/// appends record `i`, led by a comma, and runs on several threads. One chunk of `chunk` records
/// per worker is encoded at a time, each into a buffer its worker keeps, and the batch is written
/// in order.
fn write_chunks(
    w: &mut dyn Write,
    n: usize,
    workers: usize,
    chunk: usize,
    enc: &(dyn Fn(&mut Vec<u8>, usize) -> Result<(), JsonError> + Sync),
) -> Result<(), Error> {
    if n == 0 {
        // The whole-document encoder saw a nil slice here.
        w.write_all(b"null")?;
        return Ok(());
    }
    let workers = workers.max(1);
    let mut datas: Vec<Vec<u8>> = vec![Vec::new(); workers];
    for i in (0..n).step_by(workers * chunk) {
        let results: Vec<Result<(), JsonError>> = std::thread::scope(|s| {
            let handles: Vec<_> = datas
                .iter_mut()
                .zip((i..n).step_by(chunk))
                .map(|(b, lo)| {
                    s.spawn(move || {
                        b.clear();
                        (lo..n.min(lo + chunk)).try_for_each(|j| enc(b, j))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
                .collect()
        });
        for (c, r) in results.into_iter().enumerate() {
            r.map_err(Error::Json)?;
            if i == 0 && c == 0 {
                datas[c][0] = b'[';
            }
            w.write_all(&datas[c])?;
        }
    }
    w.write_all(b"\n  ]")?;
    Ok(())
}

// The append functions below write what `marshal_indent` writes for one record of the nodes or
// links array; anything past the plain cases goes through gostd's json itself.
// `test_write_json_matches_marshal_indent` holds them to the whole-document encoder.

/// Go `appendJSONNode` (`export.go:217`).
fn append_json_node(b: &mut Vec<u8>, n: &JsonNode) -> Result<(), JsonError> {
    b.extend_from_slice(b",\n    {\n      \"id\": ");
    append_json_string(b, &n.id);
    b.extend_from_slice(b",\n      \"label\": ");
    append_json_string(b, &n.label);
    b.extend_from_slice(b",\n      \"file_type\": ");
    append_json_string(b, &n.file_type);
    b.extend_from_slice(b",\n      \"source_file\": ");
    append_json_string(b, &n.source_file);
    if !n.source_location.is_empty() {
        b.extend_from_slice(b",\n      \"source_location\": ");
        append_json_string(b, &n.source_location);
    }
    b.extend_from_slice(b",\n      \"community\": ");
    match n.community {
        None => b.extend_from_slice(b"null"),
        Some(c) => b.extend_from_slice(c.to_string().as_bytes()),
    }
    b.extend_from_slice(b",\n      \"norm_label\": ");
    append_json_string(b, &n.norm_label);
    if !n.computed_name.is_empty() {
        b.extend_from_slice(b",\n      \"computed_name\": ");
        append_json_string(b, &n.computed_name);
    }
    if !n.attributes.is_empty() {
        let q = marshal_indent(&n.attributes, "      ", "  ")?;
        b.extend_from_slice(b",\n      \"attributes\": ");
        b.extend_from_slice(&q);
    }
    b.extend_from_slice(b"\n    }");
    Ok(())
}

/// Go `appendJSONLink` (`export.go:253`).
fn append_json_link(b: &mut Vec<u8>, l: &JsonLink) -> Result<(), JsonError> {
    b.extend_from_slice(b",\n    {\n      \"source\": ");
    append_json_string(b, &l.source);
    b.extend_from_slice(b",\n      \"target\": ");
    append_json_string(b, &l.target);
    b.extend_from_slice(b",\n      \"relation\": ");
    append_json_string(b, &l.relation);
    b.extend_from_slice(b",\n      \"confidence\": ");
    append_json_string(b, &l.confidence);
    if l.deferred {
        b.extend_from_slice(b",\n      \"deferred\": true");
    }
    if !l.source_file.is_empty() {
        b.extend_from_slice(b",\n      \"source_file\": ");
        append_json_string(b, &l.source_file);
    }
    if l.source_location.0 != 0 {
        b.extend_from_slice(b",\n      \"source_location\": ");
        l.source_location.append_json(b);
    }
    if l.weight != 0.0 {
        b.extend_from_slice(b",\n      \"weight\": ");
        append_json_float(b, l.weight)?;
    }
    b.extend_from_slice(b",\n      \"confidence_score\": ");
    append_json_float(b, l.confidence_score)?;
    b.extend_from_slice(b"\n    }");
    Ok(())
}

/// Go `toJSONNode` (`export.go:286`).
pub(super) fn to_json_node(n: &Node, nc: &BTreeMap<String, i64>) -> JsonNode {
    let comm = nc.get(&n.id).copied();
    let mut nl = norm_label(&n.label);
    // Computed names are built from repo content (null-label inputs, frontmatter); cap and strip
    // them once here, where every producer converges on serialization.
    let cn = sanitize_label(&n.computed_name);
    if !cn.is_empty() {
        nl = format!("{nl} {}", norm_label(&cn));
    }
    JsonNode {
        id: n.id.clone(),
        label: n.label.clone(),
        file_type: n.file_type.clone(),
        source_file: n.source_file.clone(),
        source_location: n.source_location.clone(),
        community: comm,
        norm_label: nl,
        computed_name: cn,
        attributes: n.attributes.clone(),
    }
}

/// Go `toJSONLink` (`export.go:307`).
pub(super) fn to_json_link(e: &Edge) -> JsonLink {
    let mut score = e.confidence_score;
    if score == 0.0 {
        score = confidence_score(e.confidence.as_str());
    }
    JsonLink {
        source: e.source.clone(),
        target: e.target.clone(),
        relation: e.relation.as_str().to_string(),
        confidence: e.confidence.as_str().to_string(),
        deferred: e.deferred(),
        source_file: e.source_file.as_str().to_string(),
        source_location: e.loc(),
        weight: e.weight,
        confidence_score: score,
    }
}

/// Go `CheckShrink` (`export.go:338`).
pub fn check_shrink(
    path: &Path,
    g: &Graph,
    skipped: &BTreeSet<String>,
    root: &str,
    force: bool,
) -> Result<(), Error> {
    if force {
        return Ok(());
    }
    read_guard(path).check(g, skipped, root)
}

/// Go `ShrinkGuard` (`export.go:347`): the old graph.json read while the new graph is built.
pub struct ShrinkGuard {
    path: PathBuf,
    nodes: Vec<OldNode>,
    err: Option<JsonError>,
}

impl ShrinkGuard {
    /// Go `ShrinkGuard.Check` (`export.go:365`).
    pub fn check(&self, g: &Graph, skipped: &BTreeSet<String>, root: &str) -> Result<(), Error> {
        // An absent, unreadable or empty file has no nodes to lose.
        let (path, existing) = (&self.path, &self.nodes);
        if let Some(err) = &self.err {
            return Err(Error::GraphUnverifiable(format!(
                "{ERR_GRAPH_UNVERIFIABLE} {} to verify the new graph is not smaller ({err}); pass --force to override",
                path.display()
            )));
        }
        let new_n = g.num_nodes();
        if new_n >= existing.len() {
            // Growth (or same size): never a shrink.
            return Ok(());
        }
        // Sources present in the freshly built graph: a lost node from one of these is a normal
        // in-file refactor (the file was rebuilt, its nodes changed), not an eviction, so it
        // never blocks the write.
        let current_sources: BTreeSet<&str> = g
            .node_ids()
            .iter()
            .filter_map(|id| g.node(id))
            .map(|nd| nd.source_file.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        // Net shrink. A lost node is legitimate only when its source was rebuilt in place or
        // truly deleted from disk. Refuse a lost node whose source failed to process (skipped)
        // or left the corpus while still on disk (excluded).
        for n in existing {
            if g.node(&n.id).is_some() {
                continue; // node survived
            }
            if n.source_file.is_empty() || current_sources.contains(n.source_file.as_str()) {
                continue; // derived node, or source rebuilt in place: not an eviction
            }
            if skipped.contains(&n.source_file) {
                return Err(Error::GraphShrink(format!(
                    "{ERR_GRAPH_SHRINK}: existing has {} nodes, new has {new_n} (net -{}); a lost node belongs to {}, which failed to process this run; pass --force to override",
                    existing.len(),
                    existing.len() - new_n,
                    n.source_file
                )));
            }
            // Only a real file on disk is eviction evidence. A synthetic node whose SourceFile
            // names a directory (e.g. a Terraform local-module reference) is not an extractable
            // source, so a still-present directory must not block the shrink: mirrors upstream
            // skipping nodes with no file extractor.
            let on_disk = gostd::filepath::join(&[root, n.source_file.as_str()]);
            if fs::metadata(on_disk).is_ok_and(|m| m.is_file()) {
                // Source still on disk but absent from the corpus: excluded by a new ignore
                // rule, not deleted. Fail closed so its nodes are not evicted.
                return Err(Error::GraphShrink(format!(
                    "{ERR_GRAPH_SHRINK}: existing has {} nodes, new has {new_n} (net -{}); a lost node belongs to {}, which still exists on disk but left the corpus (excluded, not deleted); pass --force to override",
                    existing.len(),
                    existing.len() - new_n,
                    n.source_file
                )));
            }
        }
        Ok(())
    }
}

/// Go `ReadShrinkGuard` (`export.go:355`). Go reads on a goroutine while the caller builds the
/// graph; this reads before it returns (a caller that wants the overlap runs it on a thread).
/// Go's pointer is never nil here, so this is always `Some`.
pub fn read_shrink_guard(path: &Path) -> Option<ShrinkGuard> {
    Some(read_guard(path))
}

/// The body of Go `ReadShrinkGuard`.
fn read_guard(path: &Path) -> ShrinkGuard {
    let (nodes, err) = match old_nodes(path) {
        Ok(nodes) => (nodes, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    ShrinkGuard {
        path: path.to_path_buf(),
        nodes,
        err,
    }
}

/// Go `struct { Nodes []json.RawMessage }` of `guardNoShrink`.
#[derive(Default)]
struct ExistingRaw {
    nodes: Vec<RawMessage>,
}

gostd::json_fields!(decode ExistingRaw, "struct { Nodes []jsontext.Value \"json:\\\"nodes\\\"\" }", {
    nodes => "nodes",
});

/// Go `guardNoShrink` (`export.go:417`): refuses to overwrite an existing graph.json at `path`
/// when the new graph (`new_n` nodes) would drop nodes relative to what is on disk. An absent,
/// unreadable, or empty file is treated as nothing-to-lose.
fn guard_no_shrink(path: &Path, new_n: usize) -> Result<(), Error> {
    let Ok(raw) = fs::read(path) else {
        // Absent (or unreadable) target: no nodes to lose, write normally.
        return Ok(());
    };
    if bytes_trim_space(&raw).is_empty() {
        // Empty/whitespace file (e.g. a freshly touched path): proceed.
        return Ok(());
    }
    let existing = unmarshal::<ExistingRaw>(&raw).map_err(|err| {
        Error::GraphUnverifiable(format!(
            "{ERR_GRAPH_UNVERIFIABLE} {} to verify the new graph is not smaller ({err}); pass force to override",
            path.display()
        ))
    })?;
    let existing_n = existing.nodes.len();
    if new_n < existing_n {
        return Err(Error::GraphShrink(format!(
            "{ERR_GRAPH_SHRINK}: existing has {existing_n} nodes, new has {new_n} (net -{}); pass force to override",
            existing_n - new_n
        )));
    }
    Ok(())
}

/// Go `normLabel` (`export.go:441`): NFKD (x/text, Unicode 15), drop `unicode.Mn` (Unicode 17),
/// `strings.ToLower` (Unicode 17).
pub(super) fn norm_label(label: &str) -> String {
    let b: String = nfkd(label)
        .chars()
        .filter(|&r| !unicode::is(&MN, r))
        .collect();
    to_lower(&b)
}
