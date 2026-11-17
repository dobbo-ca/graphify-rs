//! Port of `main.go`: the command dispatch and the build, update and query-side commands.
//! Owner: the extractors-and-CLI stream; filled by graphify-rs-ihc.20 (plan Task 20). The
//! `cmd_*` signatures are shared API.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::BufRead;
use std::path::Path;

use super::semantic::SemanticOpts;
use super::{Error, Out};
use crate::cache::{Cache, StatIndex};
use crate::detect::WalkReport;
use crate::extract::Result as ExtractResult;
use crate::model::Graph;

/// Go `version`: build metadata, injected via `-ldflags` at release time.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) const VERSION: &str = "dev";

/// Go `rootFileName`: the sidecar in the out directory that records the scanned tree.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) const ROOT_FILE_NAME: &str = ".graphify_root";

/// Go `bigFile`: the size from which a file is parsed ahead of the rest.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) const BIG_FILE: usize = 256 << 10;

/// Go `bigInFlight`: how many bytes of large files parse at once.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) const BIG_IN_FLIGHT: usize = 8 << 20;

/// Go `parseFile`'s type: the per-file parse `assemble` runs (a variable in Go so a test can
/// watch what parses side by side; a parameter of [`assemble_with`] here).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code)]
pub(crate) type ParseFn = dyn Fn(&str, &[u8]) -> ExtractResult + Sync;

/// Go `assembleStats`: how an assemble run used the cache.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AssembleStats {
    pub parsed: i64,
    pub reused: i64,
    pub dropped: i64,
}

/// Go `assemble`: one result per file in `files` order, re-parsing only files whose hash differs
/// from `prev` (`None`: Go's nil cache, a cold build, which parses the large files first).
/// `workers` is Go's `runtime.NumCPU()` (R5).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn assemble(
    root: &Path,
    files: &[String],
    prev: Option<&Cache>,
    prev_stat: &StatIndex,
    workers: usize,
) -> (Vec<ExtractResult>, Cache, StatIndex, AssembleStats) {
    todo!("graphify-rs-ihc.20")
}

/// [`assemble`] with Go's `parseFile` given: `assemble` is this with `extract::file_from_bytes`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn assemble_with(
    root: &Path,
    files: &[String],
    prev: Option<&Cache>,
    prev_stat: &StatIndex,
    workers: usize,
    parse: &ParseFn,
) -> (Vec<ExtractResult>, Cache, StatIndex, AssembleStats) {
    todo!("graphify-rs-ihc.20")
}

/// Go `largeFirst`: the indexes of the files of [`BIG_FILE`] bytes or more, largest first.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn large_first(root: &Path, files: &[String]) -> Vec<usize> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cacheStamp`: `cache::stamp(VERSION)` plus `-b<parse budget>`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn cache_stamp() -> String {
    todo!("graphify-rs-ihc.20")
}

/// Go `writeOutputs`: resolves `results` into a graph and writes graph.json, GRAPH_REPORT.md, the
/// cache and the stat sidecar under the out directory of `root`. A refused shrink is a warning
/// on `stderr` and `Ok`, with nothing written. `workers` is the fan-out of the build (R5).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables, clippy::too_many_arguments)] // Go's parameter list
pub(crate) fn write_outputs(
    root: &str,
    walk: &WalkReport,
    results: Vec<ExtractResult>,
    new_cache: Cache,
    new_stat: StatIndex,
    sem: &SemanticOpts,
    force: bool,
    no_cluster: bool,
    keep_cache: bool,
    flags: &str,
    workers: usize,
    stdout: Out<'_>,
    stderr: Out<'_>,
) -> Result<(Graph, BTreeMap<i64, Vec<String>>), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `buildOpts`: the parsed build/update arguments.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BuildOpts {
    pub root: String,
    /// An explicit path argument was given (vs the `.` default).
    pub root_set: bool,
    pub cargo: bool,
    pub no_manifests: bool,
    pub semantic: bool,
    pub backend: String,
    pub force: bool,
    pub no_cluster: bool,
}

/// Go `parseBuildOpts`: `--semantic` without `--backend` is an error.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn parse_build_opts(args: &[OsString]) -> Result<BuildOpts, Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `parseBuildArgs`: `(root, cargo, no_manifests, force, no_cluster)`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn parse_build_args(args: &[OsString]) -> (String, bool, bool, bool, bool) {
    todo!("graphify-rs-ihc.20")
}

/// Go `readPathList`: the non-blank, space-trimmed lines of `r` (`affected -`).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn read_path_list(r: impl BufRead) -> Vec<String> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdBuild`: `build [path] [--cargo] [--no-manifests] [--force] [--no-cluster]
/// [--semantic --backend X]`.
pub fn cmd_build(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// [`cmd_build`] with the worker count given (`cmd_build` passes `available_parallelism`), so a
/// test can build with 1 and with n workers (R5, Go's GOMAXPROCS tests).
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn cmd_build_with(
    args: &[OsString],
    workers: usize,
    stdout: Out<'_>,
    stderr: Out<'_>,
) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdUpdate`: `update [path]` with the `build` flags.
pub fn cmd_update(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdQuery`: `query <pattern>`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_query(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdAsk`: `ask "<question>" [--dfs] [--budget N] [--context REL] [--graph path]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_ask(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdExplain`: `explain <node> [--graph path]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_explain(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdPath`: `path <from> <to> [--undirected] [--graph path]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_path(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdGodNodes`: `god-nodes [--top N] [--graph path] [--json]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_god_nodes(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdExtract`: `extract <file>`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_extract(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdExport`: `export <graphml|dot|csv|okf> [path]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_export(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdAffected`: `affected [file...|-] [--depth N] [--relation R]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_affected(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdDiff`: `diff <old.json> <new.json> [--json]`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_diff(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdMergeDriver`: `merge-driver <base> <current> <other>`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_merge_driver(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

/// Go `cmdValidate`: `validate`.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn cmd_validate(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.20")
}

#[cfg(test)]
#[path = "tests/affected_stdin.rs"]
mod affected_stdin_tests;
#[cfg(test)]
#[path = "tests/ask.rs"]
mod ask_tests;
#[cfg(test)]
#[path = "tests/budget.rs"]
mod budget_tests;
#[cfg(test)]
#[path = "tests/cargo.rs"]
mod cargo_tests;
#[cfg(test)]
#[path = "tests/core.rs"]
mod core_tests;
#[cfg(test)]
#[path = "tests/core_util.rs"]
pub(crate) mod core_util;
#[cfg(test)]
#[path = "tests/largefirst.rs"]
mod largefirst_tests;
#[cfg(test)]
#[path = "tests/nocluster.rs"]
mod nocluster_tests;
#[cfg(test)]
#[path = "tests/semantic.rs"]
mod semantic_tests;
#[cfg(test)]
#[path = "tests/update.rs"]
mod update_tests;
#[cfg(test)]
#[path = "tests/upstream_out_dir_evidence.rs"]
mod upstream_out_dir_evidence_tests;
#[cfg(test)]
#[path = "tests/upstream_update.rs"]
mod upstream_update_tests;
