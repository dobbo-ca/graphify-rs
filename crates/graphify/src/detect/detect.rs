//! Port of `detect.go`.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard, PoisonError};

use gostd::filepath::{DirEntry, Walk, base_bytes, clean_bytes, eval_symlinks, join_bytes};
use gostd::regexp::Regexp;

use super::gitignore::new_ignorer;
use crate::goerr::path_error;

/// The bytes of a path (Go holds a path as a byte string).
pub(super) fn bytes(p: &Path) -> &[u8] {
    p.as_os_str().as_bytes()
}

/// A path from raw bytes.
pub(super) fn path_of(b: &[u8]) -> PathBuf {
    PathBuf::from(OsStr::from_bytes(b))
}

/// Go `filepath.Join(elems...)` on bytes.
pub(super) fn join(elems: &[&[u8]]) -> PathBuf {
    path_of(&join_bytes(elems))
}

/// Go `filepath.Clean`.
pub(super) fn clean(p: &Path) -> PathBuf {
    path_of(&clean_bytes(bytes(p)))
}

/// Go `filepath.Dir`: everything before the last slash, cleaned.
pub(super) fn dir_of(p: &Path) -> PathBuf {
    let b = bytes(p);
    let end = b.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
    path_of(&clean_bytes(&b[..end]))
}

/// Go `detect.IsSensitive`: whether the slash-relative path `rel` names a file that must not be
/// read (keys, credentials, env files).
pub fn is_sensitive(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    // Parents only: a root file named "credentials" is handled by name patterns.
    if parts[..parts.len() - 1]
        .iter()
        .any(|p| SENSITIVE_DIRS.contains(p))
    {
        return true;
    }
    let name = parts[parts.len() - 1];
    if SENSITIVE_PATTERNS.iter().any(|p| p.match_string(name)) {
        return true;
    }
    // Stage 3: generic secret keywords, only when load-bearing in the name; real source files
    // are exempt, data and document formats are not (detect.go:786-796).
    if generic_keyword_hit(name) {
        let ext = gostd::strings::to_lower(gostd::filepath::ext(name));
        let ext = ext.as_str();
        let is_source_code = SUPPORTED_EXTENSIONS.contains(&ext)
            && !DOCUMENT_EXTS.contains(&ext)
            && !SECRET_PRONE_DATA_EXTS.contains(&ext);
        return !is_source_code;
    }
    false
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
    let interp = shebang_interpreter(src);
    SHEBANG_EXTRACTOR_EXT
        .iter()
        .find(|&&(name, _)| name == interp)
        .map_or("", |&(_, ext)| ext)
}

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

/// Go `skipDirs`.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".hg",
    ".svn",
    "dist",
    "build",
    "target",
    "vendor",
    "__snapshots__",
    ".next",
    ".nuxt",
    ".turbo",
    ".svelte-kit",
    ".cache",
    ".idea",
    ".worktrees",
    "graphify-out",
    ".graphify",
    ".terraform",
    ".nox",
    ".tox",
    "site-packages",
    "lib64",
    ".eggs",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "venv",
    ".venv",
    "__pycache__",
    "lcov-report",
    "visual-tests",
    "visual-test",
    "storybook-static",
    "dist-protected",
    ".angular",
    ".parcel-cache",
    ".serverless",
    ".obsidian",
    ".smart-env",
];

/// Go `skipDirSuffixes`: setuptools metadata dirs and `*_venv` are generated, never source.
const SKIP_DIR_SUFFIXES: &[&str] = &[".egg-info", "_venv"];

/// Go `graphifyOut`: the output dir.
const GRAPHIFY_OUT: &str = "graphify-out";
/// Go `memoryDir`: under `graphify-out`, the Q&A docs `graphify save-result` writes back.
const MEMORY_DIR: &str = "graphify-out/memory";

/// Go `inMemoryDir`: whether `slash_rel` is `graphify-out/memory` or a path below it.
fn in_memory_dir(slash_rel: &str) -> bool {
    slash_rel == MEMORY_DIR
        || slash_rel
            .strip_prefix(MEMORY_DIR)
            .is_some_and(|r| r.starts_with('/'))
}

/// Go `isNestedWorktree`: whether `dir` is a dot-dir's `worktrees/` folder (`.claude/worktrees`)
/// or holds a `.git` file (linked worktree or submodule).
fn is_nested_worktree(dir: &Path, slash_rel: &str, env: &Env) -> bool {
    let parent = gostd::path::base(&gostd::path::dir(slash_rel));
    if base_bytes(bytes(dir)) == b"worktrees"
        && parent != "."
        && parent != ".."
        && parent.starts_with('.')
    {
        return true;
    }
    fs::symlink_metadata(join(&[bytes(&env.io(dir)), b".git"])).is_ok_and(|fi| !fi.is_dir())
}

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
    pub(super) fn process() -> Env {
        Env {
            graphify_out: std::env::var_os("GRAPHIFY_OUT"),
            // os.Getwd, cleaned (filepath.Abs(".")); absPhys resolves it before use.
            cwd: gostd::filepath::abs(Path::new(".")).ok(),
            home: std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(PathBuf::from),
        }
    }

    /// `self` with `cwd` symlink-free (`EvalSymlinks`; kept when that fails). The kernel resolves
    /// a relative path from the physical cwd, so a path made by `io` stays right under the lexical
    /// `Join` and `Clean` that follow (`lnk/..` is the parent of `lnk`'s target). `absPhys`
    /// resolves the cwd the same way, so it sees no difference.
    pub(super) fn physical(&self) -> Env {
        let mut env = self.clone();
        if let Some(wd) = &env.cwd
            && let Ok(r) = eval_symlinks(wd)
        {
            env.cwd = Some(r);
        }
        env
    }

    /// Where the I/O for the Go path `p` happens: a relative `p` under `cwd` (Go runs with the
    /// process cwd there; a test's `cwd` stands in for `t.Chdir`).
    pub(super) fn io(&self, p: &Path) -> PathBuf {
        match &self.cwd {
            Some(wd) if bytes(p).first().is_some_and(|&c| c != b'/') => wd.join(p),
            _ => p.to_path_buf(),
        }
    }
}

/// Go `skipDir`: whether directory `n` under `parent` is pruned (a skip name, `$GRAPHIFY_OUT`'s
/// base, a skip suffix, or an evidence-gated name: `env`, `coverage`, `out`, `snapshots`).
pub(super) fn skip_dir(parent: &Path, n: &OsStr, env: &Env) -> bool {
    let nb = n.as_bytes();
    if SKIP_DIRS.iter().any(|s| s.as_bytes() == nb) {
        return true;
    }
    // Mirrors upstream's GRAPHIFY_OUT_NAME: a renamed out directory inside the scanned tree is
    // not ingested as source.
    if let Some(v) = &env.graphify_out
        && !v.is_empty()
        && nb == base_bytes(v.as_bytes())
    {
        return true;
    }
    let full = || env.io(&join(&[bytes(parent), nb]));
    if nb == b"env" || nb == b".env" || nb.ends_with(b"_env") {
        return has_venv_markers(&full());
    }
    if nb == b"coverage" {
        return has_coverage_artifacts(&full());
    }
    if nb == b"out" {
        return has_build_output_markers(&full());
    }
    if nb == b"snapshots" {
        return has_snapshot_evidence(parent, nb, env);
    }
    SKIP_DIR_SUFFIXES.iter().any(|s| nb.ends_with(s.as_bytes()))
}

/// Go `statIsFile`: `os.Stat` (follows symlinks) names a regular file.
pub(super) fn stat_is_file(p: &Path) -> bool {
    fs::metadata(p).is_ok_and(|fi| fi.file_type().is_file())
}

/// Go `statIsDir`.
fn stat_is_dir(p: &Path) -> bool {
    fs::metadata(p).is_ok_and(|fi| fi.is_dir())
}

/// Go `entryKind`: `(is_file, is_dir, ok)` of `e` in `dir`, statting a symlink; `ok` is false on
/// any stat error but ENOENT, where upstream's `is_file` raises and aborts the probe.
fn entry_kind(dir: &Path, e: &DirEntry) -> (bool, bool, bool) {
    if !e.file_type.is_symlink() {
        return (e.file_type.is_file(), e.file_type.is_dir(), true);
    }
    match fs::metadata(join(&[bytes(dir), e.name.as_bytes()])) {
        Err(err) => (false, false, err.kind() == io::ErrorKind::NotFound),
        Ok(fi) => (fi.file_type().is_file(), fi.is_dir(), true),
    }
}

/// Go `coverageArtifactFiles`.
const COVERAGE_ARTIFACT_FILES: &[&str] = &[
    "lcov.info",
    "coverage-final.json",
    "coverage-summary.json",
    "clover.xml",
    "coverage.xml",
    "cobertura-coverage.xml",
    "jacoco.xml",
    ".coverage",
    "index.html",
];

/// Go `hasCoverageArtifacts`: whether `dir` holds coverage tool output.
fn has_coverage_artifacts(dir: &Path) -> bool {
    let d = bytes(dir);
    COVERAGE_ARTIFACT_FILES
        .iter()
        .any(|n| stat_is_file(&join(&[d, n.as_bytes()])))
        || stat_is_dir(&join(&[d, b"lcov-report"]))
        || stat_is_dir(&join(&[d, b"html-report"]))
}

/// Go `buildOutputSuffixes`.
const BUILD_OUTPUT_SUFFIXES: &[&str] = &[
    ".class", ".jar", ".o", ".obj", ".exe", ".dll", ".pyc", ".js.map", ".css.map", ".d.ts",
];

/// Go `hasBuildOutputSuffix` (`strings.ToLower` of the name, R3).
fn has_build_output_suffix(name: &[u8]) -> bool {
    let name = gostd::strings::to_lower(&go_lossy(name));
    BUILD_OUTPUT_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// Go `hasBuildOutputMarkers`: whether `dir` looks compiler or bundler generated. The entries of
/// `dir` are probed in directory order (Go `File.ReadDir(-1)`, not sorted), and an entry whose
/// stat fails with anything but ENOENT aborts the probe.
pub(super) fn has_build_output_markers(dir: &Path) -> bool {
    let d = bytes(dir);
    if stat_is_dir(&join(&[d, b"production"])) || stat_is_dir(&join(&[d, b"_next"])) {
        return true;
    }
    // Directory order, like scandir.
    let Ok((entries, None)) = read_dir_unsorted(dir, usize::MAX) else {
        return false;
    };
    let mut subs = Vec::new();
    for e in &entries {
        let (is_file, is_dir, ok) = entry_kind(dir, e);
        if !ok {
            return false;
        }
        if is_file && has_build_output_suffix(e.name.as_bytes()) {
            return true;
        }
        if is_dir {
            subs.push(join(&[d, e.name.as_bytes()]));
        }
    }
    let mut budget: usize = 4000; // matches upstream scan bound
    for sub in &subs {
        if budget == 0 {
            break;
        }
        let Ok((es, _)) = read_dir_unsorted(sub, budget) else {
            return false;
        };
        for e in &es {
            let (is_file, _, ok) = entry_kind(sub, e);
            if !ok {
                return false;
            }
            if is_file && has_build_output_suffix(e.name.as_bytes()) {
                return true;
            }
        }
        budget -= es.len();
    }
    false
}

/// Go `hasVenvMarkers`: whether `dir` has virtualenv or conda structure.
fn has_venv_markers(dir: &Path) -> bool {
    let d = bytes(dir);
    if stat_is_file(&join(&[d, b"pyvenv.cfg"]))
        || stat_is_file(&join(&[d, b"bin", b"activate"]))
        || stat_is_file(&join(&[d, b"Scripts", b"activate"]))
        || stat_is_dir(&join(&[d, b"conda-meta"]))
    {
        return true;
    }
    let (es, _) = read_dir_sorted(&join(&[d, b"lib"]));
    es.iter().any(|e| e.name.as_bytes().starts_with(b"python"))
}

/// Go `hasSnapshotEvidence`: whether `parent/n` is a Jest or Vitest snapshot dir.
fn has_snapshot_evidence(parent: &Path, n: &[u8], env: &Env) -> bool {
    let parent = resolve_root(parent, env); // "." has no useful base name
    let pb = base_bytes(bytes(&parent));
    if pb == b"__tests__" || pb == b"__test__" {
        return true;
    }
    let (es, _) = read_dir_sorted(&join(&[bytes(&parent), n]));
    es.iter().any(|e| e.name.as_bytes().ends_with(b".snap"))
}

/// Go `resolveRoot`: `root` as an absolute, symlink-free path.
pub(super) fn resolve_root(root: &Path, env: &Env) -> PathBuf {
    let abs = abs_phys(root, env);
    match eval_symlinks(&abs) {
        Ok(r) => r,
        Err(_) => clean(&abs),
    }
}

/// Go `absPhys`: a relative path anchored at the physical cwd, uncleaned (Getwd returns the
/// logical `$PWD` and Clean would fold `..` lexically).
fn abs_phys(p: &Path, env: &Env) -> PathBuf {
    if bytes(p).first() == Some(&b'/') {
        return p.to_path_buf();
    }
    let Some(wd) = &env.cwd else {
        return p.to_path_buf();
    };
    let wd = eval_symlinks(wd).unwrap_or_else(|_| wd.clone());
    let mut b = bytes(&wd).to_vec();
    b.push(b'/');
    b.extend_from_slice(bytes(p));
    path_of(&b)
}

/// Go `admitFile`: whether a non-dir entry is a regular file whose symlink target, if any, stays
/// under `real_root`. It opens nothing.
fn admit_file(rel: &[u8], d: &DirEntry, real_root: &Path) -> bool {
    if d.file_type.is_file() {
        return true;
    }
    if !d.file_type.is_symlink() {
        return false;
    }
    let Ok(target) = eval_symlinks(&join(&[bytes(real_root), rel])) else {
        return false;
    };
    match gostd::filepath::rel(&go_lossy(bytes(real_root)), &go_lossy(bytes(&target))) {
        Ok(r) if r != ".." && !r.starts_with("../") => stat_is_file(&target),
        _ => false,
    }
}

/// Go `skipFiles`: generated lock files.
const SKIP_FILES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "go.sum",
    "go.work.sum",
];

/// Go `mcpConfigFiles`: indexed by basename although `.json` alone is not enough.
const MCP_CONFIG_FILES: &[&str] = &[
    ".mcp.json",
    "claude_desktop_config.json",
    "mcp.json",
    "mcp_servers.json",
];

/// Go `shebangExtractorExt`: a shebang interpreter's basename to the extension whose extractor
/// handles an extensionless script (only interpreters with an extractor).
const SHEBANG_EXTRACTOR_EXT: &[(&str, &str)] = &[
    ("python", ".py"),
    ("python3", ".py"),
    ("python2", ".py"),
    ("node", ".js"),
    ("nodejs", ".js"),
    ("ruby", ".rb"),
    ("bash", ".sh"),
    ("sh", ".sh"),
    ("dash", ".sh"),
    ("zsh", ".sh"),
    ("ksh", ".sh"),
    ("lua", ".lua"),
    ("php", ".php"),
    ("julia", ".jl"),
];

/// Go `shebangInterpreter`: the interpreter basename on a leading `#!` line (through
/// `/usr/bin/env`, skipping its `-flags` and `NAME=value` words), or `""`.
pub(super) fn shebang_interpreter(src: &[u8]) -> String {
    let Some(line) = src.strip_prefix(b"#!") else {
        return String::new();
    };
    let line = match line.iter().position(|&c| c == b'\n') {
        Some(i) => &line[..i],
        None => line,
    };
    let line = go_lossy(line);
    let fields = gostd::strings::fields(&line);
    let Some(first) = fields.first() else {
        return String::new();
    };
    let mut interp = gostd::filepath::base(first);
    if interp == "env" {
        // Skip env(1) flags (-S, -i, ...) and inline NAME=value assignments to reach the
        // interpreter token.
        interp = String::new();
        for f in &fields[1..] {
            if f.starts_with('-') || f.contains('=') {
                continue;
            }
            interp = gostd::filepath::base(f);
            break;
        }
    }
    interp
}

/// Go `shebangExtOfFile`: `shebang_ext` of the first 256 bytes of `path`, `""` if it cannot be
/// read.
pub(super) fn shebang_ext_of_file(path: &Path) -> &'static str {
    let Ok(mut f) = fs::File::open(path) else {
        return "";
    };
    let mut buf = [0u8; 256];
    let n = f.read(&mut buf).unwrap_or(0);
    shebang_ext(&buf[..n])
}

/// Go `packageManifestFiles`.
const PACKAGE_MANIFEST_FILES: &[&str] = &["pyproject.toml", "Cargo.toml", "go.mod", "pom.xml"];

/// Go `sensitiveDirs`: any file under one is skipped.
const SENSITIVE_DIRS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".gcloud",
    "secrets",
    ".secrets",
    "credentials",
];

/// Go `sensitivePatterns`.
static SENSITIVE_PATTERNS: LazyLock<[Regexp; 4]> = LazyLock::new(|| {
    [
        Regexp::must_compile(r"(?i)(^|[\\/])\.(env|envrc)(\.|$)"),
        Regexp::must_compile(r"(?i)\.(pem|key|p12|pfx|cert|crt|der|p8)$"),
        Regexp::must_compile(r"(id_rsa|id_dsa|id_ecdsa|id_ed25519)(\.pub)?$"),
        Regexp::must_compile(r"(?i)(\.netrc|\.pgpass|\.htpasswd)$"),
    ]
});

/// Go `secretProneDataExts`: data formats that stay subject to the generic-keyword drop.
const SECRET_PRONE_DATA_EXTS: &[&str] = &[
    ".json",
    ".yaml",
    ".yml",
    ".toml",
    ".ini",
    ".cfg",
    ".conf",
    ".config",
    ".xml",
    ".properties",
    ".env",
    ".txt",
];

/// Go `documentExts`: supported extensions upstream classifies as documents, not code.
const DOCUMENT_EXTS: &[&str] = &[".md", ".mdx", ".markdown", ".skill"];

/// Go `genericKeywordPattern`.
static GENERIC_KEYWORD_PATTERN: LazyLock<Regexp> = LazyLock::new(|| {
    Regexp::must_compile(r"(?i)(?:credential|secret|passwd|password|private_key)s?")
});

/// Go `wordSplit`.
static WORD_SPLIT: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"[-_\s]+"));

/// Go `genericKeywordHit`: whether a generic secret keyword is load-bearing in the file name (it
/// ends the stem, or the stem has at most two words). The lookarounds of upstream are checked by
/// hand on ASCII bytes.
fn generic_keyword_hit(name: &str) -> bool {
    // Stem = name up to the first dot, ignoring leading dots.
    let stem = name
        .trim_start_matches('.')
        .split('.')
        .next()
        .unwrap_or_default();
    let sb = stem.as_bytes();
    let mut hit = false;
    for (start, end) in GENERIC_KEYWORD_PATTERN.find_all_string_index(stem, -1) {
        if start > 0 && sb[start - 1].is_ascii_alphanumeric() {
            continue;
        }
        if end < sb.len() && sb[end].is_ascii_alphabetic() {
            continue;
        }
        hit = true;
        if end == sb.len() {
            return true; // keyword+s ends the stem -> names the contents
        }
    }
    if !hit {
        return false;
    }
    let words = WORD_SPLIT
        .split(stem, -1)
        .iter()
        .filter(|w| !w.is_empty())
        .count();
    words <= 2
}

/// Go `WalkReport`: the files `collect_files_report` collected and the diagnostics of the walk.
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
pub(super) type WalkFn<'a> =
    dyn Fn(&mut WalkReport, &Path, Option<&DirEntry>, Option<&io::Error>) -> Walk + Sync + 'a;

/// Go `filepath.Rel(root, path)` for a `path` the walk made: it is `filepath.Join(root, ...)`, so
/// Rel strips `Clean(root)` (`croot`) and the slash after it.
fn rel_bytes<'a>(root: &Path, croot: &[u8], path: &'a Path) -> &'a [u8] {
    let p = bytes(path);
    if p == bytes(root) {
        return b".";
    }
    if croot == b"." {
        return p;
    }
    let rest = p.strip_prefix(croot).unwrap_or(p);
    rest.strip_prefix(b"/").unwrap_or(rest)
}

/// Go `CollectFilesReport`: walks `root` like `collect_files` and also records directory-read
/// errors and the files skipped for an unsupported extension. `workers` is Go's GOMAXPROCS (R5);
/// the walk caps it at 4 (`walkers`, detect.go).
pub fn collect_files_report(root: &Path, workers: usize) -> Result<WalkReport, Error> {
    collect_files_report_in(root, workers, &Env::process())
}

/// `collect_files_report` with the process state in `env` (`collect_files_report` is this with
/// `Env::process()`).
pub(super) fn collect_files_report_in(
    root: &Path,
    workers: usize,
    env: &Env,
) -> Result<WalkReport, Error> {
    let env = &env.physical();
    let ign = new_ignorer(root, env);
    let real_root = resolve_root(root, env);
    let croot = clean_bytes(bytes(root));
    let f = |rep: &mut WalkReport,
             path: &Path,
             d: Option<&DirEntry>,
             err: Option<&io::Error>|
     -> Walk {
        if let Some(err) = err {
            // Record the failing entry instead of swallowing it, and keep walking siblings.
            rep.walk_errors
                .push(format!("{}: {err}", go_lossy(bytes(path))));
            return Walk::Continue;
        }
        let Some(d) = d else {
            return Walk::Continue; // Go: d is non-nil when err is nil
        };
        let rel_b = rel_bytes(root, &croot, path);
        let rel = go_lossy(rel_b);
        let slash_rel = rel.as_str();
        if d.file_type.is_dir() {
            if bytes(path) == bytes(root) {
                return Walk::Continue;
            }
            // graphify-out is a skipDir, but its memory/ subtree is scanned anyway.
            if slash_rel == GRAPHIFY_OUT || in_memory_dir(slash_rel) {
                return Walk::Continue;
            }
            if skip_dir(&dir_of(path), &d.name, env)
                || is_nested_worktree(path, slash_rel, env)
                || ign.ignored(slash_rel, true)
            {
                return Walk::SkipDir;
            }
            return Walk::Continue;
        }
        if !admit_file(rel_b, d, &real_root) {
            return Walk::Continue;
        }
        let name = d.name.as_bytes();
        if SKIP_FILES.iter().any(|s| s.as_bytes() == name) {
            return Walk::Continue;
        }
        if slash_rel.starts_with("graphify-out/") && !in_memory_dir(slash_rel) {
            return Walk::Continue; // graph.json and friends are output, not source
        }
        let ext = gostd::strings::to_lower(gostd::filepath::ext(&go_lossy(name)));
        if !SUPPORTED_EXTENSIONS.contains(&ext.as_str())
            && !MCP_CONFIG_FILES.iter().any(|s| s.as_bytes() == name)
        {
            // An extensionless file may still be a script: sniff its first line for a shebang
            // naming an interpreter that has an extractor.
            if !ext.is_empty() || shebang_ext_of_file(&env.io(path)).is_empty() {
                rep.skipped += 1;
                let key = if ext.is_empty() {
                    "(no extension)".to_string()
                } else {
                    ext
                };
                *rep.skipped_exts.entry(key).or_insert(0) += 1;
                return Walk::Continue;
            }
        }
        if is_sensitive(&rel) || (!in_memory_dir(slash_rel) && ign.ignored(slash_rel, false)) {
            return Walk::Continue;
        }
        rep.files.push(rel);
        Walk::Continue
    };
    Ok(walk_tree_in(root, workers, env, &f))
}

/// Go `walkNode`: what one directory adds to the walk: its own files and diagnostics, and its
/// subdirectories in the order they were met, each with `len(rep.files)` at that point (Go's
/// `subs` and `at`). A sub is an index into the walk's node list.
#[derive(Default)]
struct WalkNode {
    rep: WalkReport,
    subs: Vec<(usize, usize)>,
}

/// Go `walkNode.join`: appends node 0 and everything under it to a report in `filepath.WalkDir`
/// order. Go recurses on directory depth; this keeps an explicit stack of the open nodes.
fn join_nodes(mut nodes: Vec<WalkNode>) -> WalkReport {
    /// An open node: its files not yet appended (the first `at` of them were already) and its
    /// subs not yet visited.
    struct Frame {
        files: std::vec::IntoIter<String>,
        at: usize,
        subs: std::vec::IntoIter<(usize, usize)>,
    }
    let mut out = WalkReport::default();
    let mut open = |out: &mut WalkReport, id: usize| -> Frame {
        let n = std::mem::take(&mut nodes[id]);
        out.walk_errors.extend(n.rep.walk_errors);
        out.skipped += n.rep.skipped;
        for (k, v) in n.rep.skipped_exts {
            *out.skipped_exts.entry(k).or_insert(0) += v;
        }
        Frame {
            files: n.rep.files.into_iter(),
            at: 0,
            subs: n.subs.into_iter(),
        }
    };
    let mut stack = vec![open(&mut out, 0)];
    while let Some(top) = stack.last_mut() {
        match top.subs.next() {
            Some((sub, at)) => {
                out.files.extend(top.files.by_ref().take(at - top.at));
                top.at = at;
                let frame = open(&mut out, sub);
                stack.push(frame);
            }
            None => {
                out.files.extend(top.files.by_ref());
                stack.pop();
            }
        }
    }
    out
}

/// Go `walkers`: the threads of one walk (on darwin 14 are no faster than 4).
const WALKERS: usize = 4;

/// A directory waiting for a walker: its node, its Go path and its entry.
struct Pending {
    node: usize,
    dir: PathBuf,
    d: DirEntry,
}

/// The walk's shared state. Go's goroutines (`spawn`, one per directory, or inline when no slot
/// is free) and `sync.WaitGroup` become a stack of pending directories that the walkers drain:
/// no thread recurses, so directory depth costs no stack.
struct Queue {
    pending: Vec<Pending>,
    /// Walkers reading a directory.
    busy: usize,
    /// A walker panicked: the others stop instead of waiting for it.
    aborted: bool,
    nodes: Vec<WalkNode>,
}

fn lock(q: &Mutex<Queue>) -> MutexGuard<'_, Queue> {
    q.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Marks the walk aborted when a walker unwinds, so `std::thread::scope` can join the others and
/// pass the panic on.
struct AbortOnPanic<'a>(&'a Mutex<Queue>, &'a Condvar);

impl Drop for AbortOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            lock(self.0).aborted = true;
            self.1.notify_all();
        }
    }
}

/// One walker: takes pending directories until none is left and no walker is busy.
fn walker(q: &Mutex<Queue>, cv: &Condvar, env: &Env, f: &WalkFn<'_>) {
    let _abort = AbortOnPanic(q, cv);
    let mut g = lock(q);
    loop {
        if g.aborted {
            return;
        }
        let Some(t) = g.pending.pop() else {
            if g.busy == 0 {
                return;
            }
            g = cv.wait(g).unwrap_or_else(PoisonError::into_inner);
            continue;
        };
        g.busy += 1;
        let mut node = std::mem::take(&mut g.nodes[t.node]);
        drop(g);
        let subs = walk_one(&mut node, &t.dir, &t.d, env, f);
        g = lock(q);
        g.busy -= 1;
        for (at, dir, d) in subs {
            let id = g.nodes.len();
            g.nodes.push(WalkNode::default());
            node.subs.push((id, at));
            g.pending.push(Pending { node: id, dir, d });
        }
        g.nodes[t.node] = node;
        cv.notify_all();
    }
}

/// Go `walkTree`'s `walk` for one directory: reads `dir` (sorted), calls `f` for each entry and
/// returns the subdirectories to walk, each with `len(n.rep.files)` when it was met.
fn walk_one(
    n: &mut WalkNode,
    dir: &Path,
    d: &DirEntry,
    env: &Env,
    f: &WalkFn<'_>,
) -> Vec<(usize, PathBuf, DirEntry)> {
    // On an error ReadDir still returns the entries it could read.
    let (entries, err) = read_dir_sorted(&env.io(dir));
    if let Some((op, e)) = err
        && !matches!(
            f(&mut n.rep, dir, Some(d), Some(&path_error(op, dir, &e))),
            Walk::Continue
        )
    {
        return Vec::new();
    }
    let mut subs = Vec::new();
    for e in entries {
        let p = join(&[bytes(dir), e.name.as_bytes()]);
        if !matches!(f(&mut n.rep, &p, Some(&e), None), Walk::Continue) || !e.file_type.is_dir() {
            continue;
        }
        subs.push((n.rep.files.len(), p, e));
    }
    subs
}

/// Go `walkTree`: calls `f` for every entry under `root` as `filepath.WalkDir` would, with
/// directories read on up to `min(workers, 4)` threads, and joins the per-directory reports in
/// `WalkDir` order (detect.go:642-679).
#[cfg(test)]
pub(super) fn walk_tree(root: &Path, workers: usize, f: &WalkFn<'_>) -> WalkReport {
    walk_tree_in(root, workers, &Env::default(), f)
}

/// `walk_tree` with the I/O of a relative root under `env.cwd` (`Env::io`).
fn walk_tree_in(root: &Path, workers: usize, env: &Env, f: &WalkFn<'_>) -> WalkReport {
    let mut top = WalkNode::default();
    let info = match fs::symlink_metadata(env.io(root)) {
        Ok(info) => info,
        Err(e) => {
            let _ = f(
                &mut top.rep,
                root,
                None,
                Some(&path_error("lstat", root, &e)),
            );
            return top.rep;
        }
    };
    // fs.FileInfoToDirEntry(info): the name is the base of the root as given.
    let d = DirEntry {
        name: OsStr::from_bytes(base_bytes(bytes(root))).to_os_string(),
        file_type: info.file_type(),
    };
    if !matches!(f(&mut top.rep, root, Some(&d), None), Walk::Continue) || !d.file_type.is_dir() {
        return join_nodes(vec![top]);
    }
    let q = Mutex::new(Queue {
        pending: vec![Pending {
            node: 0,
            dir: root.to_path_buf(),
            d,
        }],
        busy: 0,
        aborted: false,
        nodes: vec![top],
    });
    let cv = Condvar::new();
    let n = workers.clamp(1, WALKERS);
    std::thread::scope(|s| {
        for _ in 1..n {
            s.spawn(|| walker(&q, &cv, env, f));
        }
        walker(&q, &cv, env, f);
    });
    let nodes = q.into_inner().unwrap_or_else(PoisonError::into_inner).nodes;
    join_nodes(nodes)
}

/// Go `os.Open(dir)` and `File.ReadDir(limit)`: up to `limit` entries of `dir` in directory
/// order (`Err`: the open failed) and the error that ended the read, if any. An entry that
/// vanished during the read is skipped, as Go's ReadDir does. The one `std::fs::read_dir` of the
/// crate (R7): Go's sorted `os.ReadDir` is `read_dir_sorted`.
#[allow(clippy::disallowed_methods)]
fn read_dir_unsorted(dir: &Path, limit: usize) -> io::Result<(Vec<DirEntry>, Option<io::Error>)> {
    let mut es = Vec::new();
    for ent in fs::read_dir(dir)? {
        if es.len() == limit {
            break;
        }
        match ent.and_then(|e| {
            e.file_type().map(|file_type| DirEntry {
                name: e.file_name(),
                file_type,
            })
        }) {
            Ok(d) => es.push(d),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Ok((es, Some(e))),
        }
    }
    Ok((es, None))
}

/// Go `os.ReadDir`: the entries of `dir` sorted by name bytes and the error with Go's op
/// (`open`, or `readdirent` mid-read); on an error, the entries read before it.
fn read_dir_sorted(dir: &Path) -> (Vec<DirEntry>, Option<(&'static str, io::Error)>) {
    let (mut es, err) = match read_dir_unsorted(dir, usize::MAX) {
        Ok((es, err)) => (es, err.map(|e| ("readdirent", e))),
        Err(e) => return (Vec::new(), Some(("open", e))),
    };
    es.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    (es, err)
}

/// The worker count of a walk (Go: GOMAXPROCS = NumCPU, R5).
fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Go `CollectFiles`: the supported source files under `root`, relative to it, in walk order.
pub fn collect_files(root: &Path) -> Result<Vec<String>, Error> {
    Ok(collect_files_report(root, workers())?.files)
}

/// Go `CollectManifests`: the package manifests (`pyproject.toml`, `Cargo.toml`, `go.mod`,
/// `pom.xml`) under `root`, with the skip and ignore rules of `collect_files`.
pub fn collect_manifests(root: &Path) -> Result<Vec<String>, Error> {
    let env = &Env::process().physical();
    let mut files = Vec::new();
    let ign = new_ignorer(root, env);
    let real_root = resolve_root(root, env);
    let croot = clean_bytes(bytes(root));
    let res = gostd::filepath::walk_dir(root, |path, d, err| {
        let (None, Some(d)) = (err, d) else {
            return Walk::Continue; // unreadable entry: skip, don't abort the whole walk
        };
        let rel_b = rel_bytes(root, &croot, path);
        let rel = go_lossy(rel_b);
        if d.file_type.is_dir() {
            if bytes(path) == bytes(root) {
                return Walk::Continue;
            }
            if skip_dir(&dir_of(path), &d.name, env)
                || is_nested_worktree(path, &rel, env)
                || ign.ignored(&rel, true)
            {
                return Walk::SkipDir;
            }
            return Walk::Continue;
        }
        let name = d.name.as_bytes();
        if !PACKAGE_MANIFEST_FILES.iter().any(|s| s.as_bytes() == name)
            || !admit_file(rel_b, d, &real_root)
        {
            return Walk::Continue;
        }
        if ign.ignored(&rel, false) {
            return Walk::Continue;
        }
        files.push(rel);
        Walk::Continue
    });
    res.map(|()| files).map_err(|e| Error(e.to_string()))
}
