//! Port of `detect.go`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use gostd::filepath::{DirEntry, Walk};

/// Go `detect.IsSensitive`: whether the slash-relative path `rel` names a file that must not be
/// read (keys, credentials, env files).
pub fn is_sensitive(rel: &str) -> bool {
    todo!("graphify-rs-ihc.14")
}

/// A file name as Go holds it, made a Rust string (plan R3, D-008): each byte that Go's
/// `utf8.DecodeRune` reads as `RuneError` of width 1 becomes one U+FFFD, decoded rune by rune
/// with `gostd::utf8::decode_rune`. Not `String::from_utf8_lossy`, which makes one U+FFFD of a
/// maximal invalid subpart (`E2 82 41`: Go gives two U+FFFD, lossy one).
pub fn go_lossy(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let (r, size) = gostd::utf8::decode_rune(&b[i..]);
        out.push(r);
        i += size;
    }
    out
}

/// Go `detect.ShebangExt`: the extension the interpreter on a `#!` first line stands for
/// (`".sh"`, `".py"`, ...), or `""`.
pub fn shebang_ext(src: &[u8]) -> &'static str {
    todo!("graphify-rs-ihc.14")
}

// graphify-rs-ihc.14: stubs until the code PR

/// Go `SupportedExtensions` (a `map[string]bool`): the lowercased file suffixes an extractor
/// handles, in Go's declaration order.
pub static SUPPORTED_EXTENSIONS: &[&str] = &[
    ".go",
    ".js",
    ".jsx",
    ".mjs",
    ".cjs",
    ".ts",
    ".mts",
    ".cts",
    ".tsx",
    ".tf",
    ".tfvars",
    ".json",
    ".hcl",
    ".py",
    ".rs",
    ".c",
    ".h",
    ".cpp",
    ".cc",
    ".cxx",
    ".hpp",
    ".hh",
    ".hxx",
    ".java",
    ".cs",
    ".rb",
    ".rake",
    ".php",
    ".phtml",
    ".sh",
    ".bash",
    ".scala",
    ".sc",
    ".jl",
    ".v",
    ".sv",
    ".svh",
    ".vh",
    ".kt",
    ".kts",
    ".lua",
    ".zig",
    ".md",
    ".mdx",
    ".markdown",
    ".skill",
    ".vue",
    ".svelte",
    ".astro",
];

/// The error of a walk (Display = Go's text, R6). Go's `CollectFilesReport` never fails and
/// `CollectManifests` only passes on `filepath.WalkDir`'s error.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// What Go reads from the process during a walk: `$GRAPHIFY_OUT` (`skipDir`), the working
/// directory (a relative root, `absPhys`) and `$HOME` (`findVCSRoot`). A test builds one instead
/// of Go's `t.Setenv` and `t.Chdir`: both are process-global, and `set_var` is `unsafe` in
/// edition 2024.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default)]
pub(super) struct Env {
    /// `$GRAPHIFY_OUT`; `None` or empty: unset.
    pub graphify_out: Option<OsString>,
    /// The directory a relative root is resolved against, for I/O and for `absPhys`: Go's
    /// `os.Getwd`, which is the logical `$PWD` when that names the cwd (`t.Chdir` sets it), so it
    /// may run through a symlink. `None`: Go's `Getwd` failed.
    pub cwd: Option<PathBuf>,
    /// `$HOME` as set, uncleaned (a trailing slash is kept); `None`: unset.
    pub home: Option<PathBuf>,
}

impl Env {
    /// The process's own values (what the Go functions read).
    #[allow(dead_code, unused_variables)]
    pub(super) fn process() -> Env {
        todo!("graphify-rs-ihc.14")
    }
}

/// Go `WalkReport`: the files `collect_files_report` collected and the diagnostics of the walk.
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WalkReport {
    /// Collected source files, relative to the root (R3), in `filepath.WalkDir` order.
    pub files: Vec<String>,
    /// `"<path>: <error>"` for each entry the walk could not read.
    pub walk_errors: Vec<String>,
    /// Files seen but dropped for an extension no extractor handles.
    pub skipped: i64,
    /// `skipped` by lowercased extension (`"(no extension)"` for none); sums to `skipped`.
    pub skipped_exts: BTreeMap<String, i64>,
}

/// Go `walkTree`'s callback: records into the report it is handed (one per directory) and returns
/// `Walk::Continue`, or `Walk::SkipDir` to prune a directory. Called from several threads.
#[allow(dead_code, unused_variables)]
pub(super) type WalkFn<'a> =
    dyn Fn(&mut WalkReport, &Path, Option<&DirEntry>, Option<&io::Error>) -> Walk + Sync + 'a;

/// Go `CollectFilesReport`: walks `root` like `collect_files` and also records directory-read
/// errors and the files skipped for an unsupported extension. `workers` is Go's GOMAXPROCS (R5);
/// the walk caps it at 4 (`walkers`, detect.go).
#[allow(dead_code, unused_variables)]
pub fn collect_files_report(root: &Path, workers: usize) -> Result<WalkReport, Error> {
    todo!("graphify-rs-ihc.14")
}

/// `collect_files_report` with the process state in `env` (`collect_files_report` is this with
/// `Env::process()`).
#[allow(dead_code, unused_variables)]
pub(super) fn collect_files_report_in(
    root: &Path,
    workers: usize,
    env: &Env,
) -> Result<WalkReport, Error> {
    todo!("graphify-rs-ihc.14")
}

/// Go `CollectFiles`: the supported source files under `root`, relative to it, in walk order.
#[allow(dead_code, unused_variables)]
pub fn collect_files(root: &Path) -> Result<Vec<String>, Error> {
    todo!("graphify-rs-ihc.14")
}

/// Go `CollectManifests`: the package manifests (`pyproject.toml`, `Cargo.toml`, `go.mod`,
/// `pom.xml`) under `root`, with the skip and ignore rules of `collect_files`.
#[allow(dead_code, unused_variables)]
pub fn collect_manifests(root: &Path) -> Result<Vec<String>, Error> {
    todo!("graphify-rs-ihc.14")
}

/// Go `walkTree`: calls `f` for every entry under `root` as `filepath.WalkDir` would, with
/// directories read on up to `min(workers, 4)` threads, and joins the per-directory reports in
/// `WalkDir` order (detect.go:642-679).
#[allow(dead_code, unused_variables)]
pub(super) fn walk_tree(root: &Path, workers: usize, f: &WalkFn<'_>) -> WalkReport {
    todo!("graphify-rs-ihc.14")
}

/// Go `skipDir`: whether directory `n` under `parent` is pruned (a skip name, `$GRAPHIFY_OUT`'s
/// base, a skip suffix, or an evidence-gated name: `env`, `coverage`, `out`, `snapshots`).
#[allow(dead_code, unused_variables)]
pub(super) fn skip_dir(parent: &Path, n: &std::ffi::OsStr, env: &Env) -> bool {
    todo!("graphify-rs-ihc.14")
}

/// Go `statIsFile`: `os.Stat` (follows symlinks) names a regular file.
#[allow(dead_code, unused_variables)]
pub(super) fn stat_is_file(p: &Path) -> bool {
    todo!("graphify-rs-ihc.14")
}

/// Go `hasBuildOutputMarkers`: whether `dir` looks compiler or bundler generated. The entries of
/// `dir` are probed in directory order (Go `File.ReadDir(-1)`, not sorted), and an entry whose
/// stat fails with anything but ENOENT aborts the probe.
#[allow(dead_code, unused_variables)]
pub(super) fn has_build_output_markers(dir: &Path) -> bool {
    todo!("graphify-rs-ihc.14")
}

/// Go `shebangInterpreter`: the interpreter basename on a leading `#!` line (through
/// `/usr/bin/env`, skipping its `-flags` and `NAME=value` words), or `""`.
#[allow(dead_code, unused_variables)]
pub(super) fn shebang_interpreter(src: &[u8]) -> String {
    todo!("graphify-rs-ihc.14")
}

/// Go `shebangExtOfFile`: `shebang_ext` of the first 256 bytes of `path`, `""` if it cannot be
/// read.
#[allow(dead_code, unused_variables)]
pub(super) fn shebang_ext_of_file(path: &Path) -> &'static str {
    todo!("graphify-rs-ihc.14")
}
