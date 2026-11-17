//! Port of `cmd/graphify/semantic.go`: the opt-in `--semantic` stage of `build`.

use std::path::Path;

use gostd::filepath::join;

use super::main::out_dir_for;
use super::{Error, Out, msg, path_error};
use crate::detect::go_lossy;
use crate::model::Extraction;
use crate::semantic::{self, Backend, BedrockBackend, Config, Note};

// Go marshals `semantic.Cache` by reflection; the semantic package gives `Entry` no JSON of its
// own, and only this file persists it.
gostd::json_fields!(decode semantic::Entry, "semantic.Entry", {
    hash => "hash",
    nodes => "nodes",
    edges => "edges",
});

/// Go `semanticCacheFile`: the content-hash cache of the semantic stage, written beside
/// graph.json so only changed notes re-pay LLM tokens on a rebuild.
const SEMANTIC_CACHE_FILE: &str = ".graphify_semantic.json";

/// Go `semanticOpts`: the enrichment-stage settings derived from the build flags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SemanticOpts {
    pub enabled: bool,
    pub backend: String,
}

/// Go `proseTypes`: the markdown node file_types whose source documents go to the backend.
const PROSE_TYPES: [&str; 5] = ["document", "note", "learning", "decision", "adr"];

/// Go `enrich`: runs the semantic stage over `ext` and returns the additively enriched
/// extraction, loading and saving the semantic cache beside graph.json.
pub(crate) fn enrich(
    root: &str,
    ext: Extraction,
    opts: &SemanticOpts,
    stdout: Out<'_>,
    stderr: Out<'_>,
) -> Result<Extraction, Error> {
    let backend = new_semantic_backend(&opts.backend)?;
    let notes = collect_notes(root, &ext);
    if notes.is_empty() {
        return Ok(ext);
    }
    let cache_path = join(&[&out_dir_for(root), SEMANTIC_CACHE_FILE]);
    let prev = load_semantic_cache(&cache_path);
    let cfg = Config {
        backend: Some(&*backend),
    };
    let (out, new_cache) = semantic::enrich(&cfg, ext, &notes, prev).map_err(msg)?;
    if let Err(e) = save_semantic_cache(&cache_path, &new_cache) {
        // A cache-write failure is non-fatal: the enrichment is already in the graph.
        w!(stderr, "  semantic: cache not saved ({e})\n")?;
    }
    w!(
        stdout,
        "semantic enrichment ({}): {} notes considered\n",
        backend.name(),
        notes.len()
    )?;
    Ok(out)
}

/// Go `newSemanticBackend`: only `bedrock` is implemented.
fn new_semantic_backend(name: &str) -> Result<Box<dyn Backend>, Error> {
    match name {
        "bedrock" => Ok(Box::new(BedrockBackend::new("us-east-1").map_err(msg)?)),
        _ => Err(Error::Msg(format!(
            "unknown semantic backend {} (want: bedrock)",
            gostd::strconv::quote(name.as_bytes())
        ))),
    }
}

/// Go `collectNotes`: a note per prose node with a readable source file.
fn collect_notes(root: &str, ext: &Extraction) -> Vec<Note> {
    ext.nodes
        .iter()
        .filter(|n| !n.source_file.is_empty() && PROSE_TYPES.contains(&n.file_type.as_str()))
        .filter_map(|n| {
            let body = std::fs::read(join(&[root, &n.source_file])).ok()?;
            Some(Note {
                id: n.id.clone(),
                file: n.source_file.clone(),
                content: go_lossy(&body),
            })
        })
        .collect()
}

/// Go `loadSemanticCache`: a missing or malformed file is an empty cache.
fn load_semantic_cache(path: &str) -> semantic::Cache {
    std::fs::read(path)
        .ok()
        .and_then(|data| gostd::json::unmarshal(&data).ok())
        .unwrap_or_default()
}

/// Go `saveSemanticCache`.
fn save_semantic_cache(path: &str, c: &semantic::Cache) -> Result<(), Error> {
    let data = gostd::json::marshal(c).map_err(msg)?;
    std::fs::write(path, data).map_err(|e| path_error("open", Path::new(path), &e))
}
