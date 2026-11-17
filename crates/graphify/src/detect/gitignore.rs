//! Port of `gitignore.go`: the `.gitignore`, `.graphifyignore` and `$GIT_DIR/info/exclude`
//! rules of a tree, and the ignore files above the scan root up to the VCS root.

use std::path::{Path, PathBuf};

use super::detect::Env;

// graphify-rs-ihc.14: stubs until the code PR

/// Go `ignorer`: whether a path is excluded by the ignore files of the tree under `root` and of
/// the directories above it up to the VCS root. Each directory's rules are read once.
#[allow(dead_code, unused_variables)]
pub(super) struct Ignorer {
    root: PathBuf,
}

/// Go `newIgnorer`, with the process state (`$HOME`, the cwd) in `env`.
#[allow(dead_code, unused_variables)]
pub(super) fn new_ignorer(root: &Path, env: &Env) -> Ignorer {
    todo!("graphify-rs-ihc.14")
}

impl Ignorer {
    /// Go `ignorer.ignored`: whether `rel` (slash-relative to the root) is excluded, given whether
    /// it is a directory. Its ancestors are not tested: the walk prunes them first.
    #[allow(dead_code, unused_variables)]
    pub(super) fn ignored(&self, rel: &str, is_dir: bool) -> bool {
        todo!("graphify-rs-ihc.14")
    }
}

/// Go `ignoreFile`: the rules of one directory's ignore files.
#[allow(dead_code, unused_variables)]
pub(super) struct IgnoreFile {
    /// The directory holding the file, slash-relative to the root (`""` for the root).
    pub base: String,
    pub rules: Vec<IgnoreRule>,
}

/// Go `ignoreRule`: one pattern line.
#[allow(dead_code, unused_variables)]
pub(super) struct IgnoreRule {
    /// Unanchored: matched against the basename (`None` for an anchored rule).
    pub re: Option<gostd::regexp::Regexp>,
    /// Anchored: matched segment-wise against the sub-path.
    pub segs: Vec<Seg>,
    /// `segs` contains `**`.
    pub globstar: bool,
    pub negate: bool,
    pub dir_only: bool,
    /// Matches the whole sub-path (the pattern has a slash), not just the basename.
    pub anchored: bool,
    /// Literal bytes every match starts with and, for a basename pattern, ends with.
    pub prefix: String,
    pub suffix: String,
}

/// Go `seg`: one `/`-separated piece of an anchored pattern.
#[allow(dead_code, unused_variables)]
pub(super) struct Seg {
    pub globstar: bool,
    /// Used when `re` is `None`.
    pub lit: String,
    /// `None` for a piece with no wildcard.
    pub re: Option<gostd::regexp::Regexp>,
}

impl IgnoreRule {
    /// Go `ignoreRule.matches`: tests `target` (the basename, or the sub-path for an anchored
    /// rule) without the literal prefix and suffix check.
    #[allow(dead_code, unused_variables)]
    pub(super) fn matches(&self, target: &str) -> bool {
        todo!("graphify-rs-ihc.14")
    }
}

/// Go `parseIgnore`: the rules of one ignore file's text (`data`, the file's bytes through
/// `go_lossy`), for the directory `base`. A pattern whose regexp does not compile is dropped.
#[allow(dead_code, unused_variables)]
pub(super) fn parse_ignore(base: &str, data: &str) -> IgnoreFile {
    todo!("graphify-rs-ihc.14")
}

/// Go `applyRules`: runs `rules` in order over `sub` and returns the last match's verdict, or
/// `decision` when none matches.
#[allow(dead_code, unused_variables)]
pub(super) fn apply_rules(rules: &[IgnoreRule], sub: &str, is_dir: bool, decision: bool) -> bool {
    todo!("graphify-rs-ihc.14")
}

/// Go `readRegular`: the bytes of `path` only when `os.Stat` says it is a regular file, so a FIFO
/// or device named `.gitignore` is never opened.
#[allow(dead_code, unused_variables)]
pub(super) fn read_regular(path: &Path) -> Option<Vec<u8>> {
    todo!("graphify-rs-ihc.14")
}

/// Go `findVCSRoot`: from `start` (already resolved) up to the first directory with a valid VCS
/// marker (a `.git` dir with `HEAD`, `objects/` and `refs/`, a `.git` file starting `gitdir:`,
/// or `.hg`, `.svn`, `_darcs`, `.fossil`); `None` at the filesystem root or at `env.home`
/// (cleaned).
#[allow(dead_code, unused_variables)]
pub(super) fn find_vcs_root(start: &Path, env: &Env) -> Option<PathBuf> {
    todo!("graphify-rs-ihc.14")
}
