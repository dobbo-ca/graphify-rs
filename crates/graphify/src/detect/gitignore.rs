//! Port of `gitignore.go`: the `.gitignore`, `.graphifyignore` and `$GIT_DIR/info/exclude`
//! rules of a tree, and the ignore files above the scan root up to the VCS root.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use gostd::regexp::{Regexp, quote_meta};
use gostd::strings::bytes_trim_space;

use super::detect::{Env, bytes, clean, dir_of, go_lossy, join, path_of, resolve_root};

/// Go `ignorer`: whether a path is excluded by the ignore files of the tree under `root` and of
/// the directories above it up to the VCS root. Each directory's rules are read once.
pub(super) struct Ignorer {
    root: PathBuf,
    /// `env.io(root)`: where the root's files are read.
    io_root: PathBuf,
    env: Env,
    /// Go `cache` (a `sync.Map`): dir (slash-relative to root, `""` = root) to its parsed rules
    /// (`None`: no ignore file).
    cache: Mutex<HashMap<String, Option<Arc<IgnoreFile>>>>,
    /// Go `ancOnce`, `anc` and `ceilingRoot`.
    anc: OnceLock<Ancestors>,
}

/// The ignore files above the root, outermost first, and whether the root is its own VCS
/// ceiling.
struct Ancestors {
    files: Vec<IgnoreFile>,
    ceiling_root: bool,
}

/// Go `newIgnorer`, with the process state (`$HOME`, the cwd) in `env`.
pub(super) fn new_ignorer(root: &Path, env: &Env) -> Ignorer {
    let env = env.physical();
    Ignorer {
        root: root.to_path_buf(),
        io_root: env.io(root),
        env,
        cache: Mutex::new(HashMap::new()),
        anc: OnceLock::new(),
    }
}

/// Go `ignoreFile`: the rules of one directory's ignore files.
pub(super) struct IgnoreFile {
    /// The directory holding the file, slash-relative to the root (`""` for the root); for an
    /// ancestor, the root relative to that directory.
    pub base: String,
    pub rules: Vec<IgnoreRule>,
}

/// Go `ignoreRule`: one pattern line.
pub(super) struct IgnoreRule {
    /// Unanchored: matched against the basename (`None` for an anchored rule).
    pub re: Option<Regexp>,
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
pub(super) struct Seg {
    pub globstar: bool,
    /// Used when `re` is `None`.
    pub lit: String,
    /// `None` for a piece with no wildcard.
    pub re: Option<Regexp>,
}

impl Seg {
    /// Go `seg.match`.
    fn matches(&self, name: &str) -> bool {
        match &self.re {
            None => name == self.lit,
            Some(re) => re.match_string(name),
        }
    }
}

/// Go `readRegular`: the bytes of `path` only when `os.Stat` says it is a regular file, so a FIFO
/// or device named `.gitignore` is never opened.
pub(super) fn read_regular(path: &Path) -> Option<Vec<u8>> {
    if !fs::metadata(path).is_ok_and(|st| st.file_type().is_file()) {
        return None;
    }
    fs::read(path).ok()
}

impl Ignorer {
    /// Go `ignorer.load`: the parsed rules of `dir` (slash-relative to the root), read and cached
    /// on first use. `.graphifyignore` rules come after the `.gitignore` rules (last match wins),
    /// and at the VCS root `$GIT_DIR/info/exclude` comes first (lowest precedence).
    fn load(&self, dir: &str) -> Option<Arc<IgnoreFile>> {
        if let Some(f) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(dir)
        {
            return f.clone();
        }
        let mut f: Option<IgnoreFile> = None;
        if dir.is_empty()
            && self.ceiling_is_root()
            && let Some(excl) = info_exclude_of(&self.io_root)
            && let Some(data) = read_regular(&excl)
        {
            f = Some(parse_ignore("", &go_lossy(&data)));
        }
        for name in [".gitignore", ".graphifyignore"] {
            let Some(data) = read_regular(&join(&[
                bytes(&self.io_root),
                dir.as_bytes(),
                name.as_bytes(),
            ])) else {
                continue;
            };
            let parsed = parse_ignore(dir, &go_lossy(&data));
            if let Some(f) = f.as_mut() {
                f.rules.extend(parsed.rules);
                continue;
            }
            f = Some(parsed);
        }
        let f = f.map(Arc::new);
        // Two walkers may both read it; same result.
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(dir.to_string(), f.clone());
        f
    }

    /// Go `ceilingIsRoot`.
    fn ceiling_is_root(&self) -> bool {
        self.ancestors().ceiling_root
    }

    /// Go `ancOnce.Do(initAncestors)`.
    fn ancestors(&self) -> &Ancestors {
        self.anc
            .get_or_init(|| init_ancestors(&self.root, &self.env))
    }

    /// Go `ignorer.ignored`: whether `rel` (slash-relative to the root) is excluded, given whether
    /// it is a directory. Its ancestors are not tested: the walk prunes them first.
    pub(super) fn ignored(&self, rel: &str, is_dir: bool) -> bool {
        let mut decision = false;
        for f in &self.ancestors().files {
            decision = apply_rules(&f.rules, &format!("{}/{rel}", f.base), is_dir, decision);
        }
        for dir in ancestor_dirs(rel) {
            let Some(f) = self.load(dir) else {
                continue;
            };
            // Path of rel relative to this ignore file's directory.
            let sub = if dir.is_empty() {
                rel
            } else {
                rel.strip_prefix(dir)
                    .and_then(|r| r.strip_prefix('/'))
                    .unwrap_or(rel)
            };
            decision = apply_rules(&f.rules, sub, is_dir, decision);
        }
        decision
    }
}

/// Go `infoExcludeOf`: the repo's `$GIT_DIR/info/exclude` for the repo rooted at `dir`, or
/// `None` when `dir` is not a git repo or has no exclude file. A `.git` file (linked worktree or
/// submodule) is followed through `gitdir:` and then the worktree's `commondir`.
fn info_exclude_of(dir: &Path) -> Option<PathBuf> {
    let git_path = join(&[bytes(dir), b".git"]);
    let info = fs::metadata(&git_path).ok()?;
    let mut git_dir = git_path.clone();
    let is_file = !info.is_dir();
    if is_file {
        let data = read_regular(&git_path)?;
        let line = bytes_trim_space(&data);
        let gd = bytes_trim_space(line.strip_prefix(b"gitdir:")?);
        git_dir = if gd.first() == Some(&b'/') {
            path_of(gd)
        } else {
            join(&[bytes(dir), gd])
        };
    }
    // A linked worktree's gitdir carries a `commondir` pointing at the shared git dir, which is
    // where info/exclude lives. Upstream skips commondir for a .git dir.
    let mut common_dir = git_dir.clone();
    let data = if is_file {
        read_regular(&join(&[bytes(&git_dir), b"commondir"]))
    } else {
        None
    };
    if let Some(data) = data {
        let cd = bytes_trim_space(&data);
        if !cd.is_empty() {
            let cd = if cd.first() == Some(&b'/') {
                path_of(cd)
            } else {
                join(&[bytes(&git_dir), cd])
            };
            common_dir = clean(&cd);
        }
    }
    let excl = join(&[bytes(&common_dir), b"info", b"exclude"]);
    if !fs::metadata(&excl).is_ok_and(|st| st.file_type().is_file()) {
        return None;
    }
    Some(excl)
}

/// Go `isValidGitMarker`: a `.git` dir needs `HEAD`, `objects/` and `refs/`; a `.git` file must
/// start with `gitdir:`.
fn is_valid_git_marker(dot_git: &Path) -> bool {
    let Ok(st) = fs::metadata(dot_git) else {
        return false;
    };
    let d = bytes(dot_git);
    if st.is_dir() {
        if !fs::metadata(join(&[d, b"HEAD"])).is_ok_and(|h| h.file_type().is_file()) {
            return false;
        }
        return ["objects", "refs"]
            .iter()
            .all(|sub| fs::metadata(join(&[d, sub.as_bytes()])).is_ok_and(|s| s.is_dir()));
    }
    if !st.file_type().is_file() {
        return false;
    }
    let Ok(mut f) = fs::File::open(dot_git) else {
        return false;
    };
    let mut buf = [0u8; 7];
    let n = f.read(&mut buf).unwrap_or(0);
    &buf[..n] == b"gitdir:"
}

/// Go `hasVCSMarker`.
fn has_vcs_marker(dir: &Path) -> bool {
    let d = bytes(dir);
    if is_valid_git_marker(&join(&[d, b".git"])) {
        return true;
    }
    [".hg", ".svn", "_darcs", ".fossil"]
        .iter()
        .any(|m| fs::metadata(join(&[d, m.as_bytes()])).is_ok())
}

/// Go `findVCSRoot`: from `start` (already resolved) up to the first directory with a valid VCS
/// marker (a `.git` dir with `HEAD`, `objects/` and `refs/`, a `.git` file starting `gitdir:`,
/// or `.hg`, `.svn`, `_darcs`, `.fossil`); `None` at the filesystem root or at `env.home`
/// (cleaned).
pub(super) fn find_vcs_root(start: &Path, env: &Env) -> Option<PathBuf> {
    // os.UserHomeDir: "" when $HOME is unset, and Clean("") is ".".
    let home = clean(env.home.as_deref().unwrap_or(Path::new("")));
    let mut cur = start.to_path_buf();
    loop {
        if has_vcs_marker(&env.io(&cur)) {
            return Some(cur);
        }
        let parent = dir_of(&cur);
        if bytes(&parent) == bytes(&cur) || bytes(&cur) == bytes(&home) {
            return None;
        }
        cur = parent;
    }
}

/// Go `initAncestors`: the ignore files of every directory from the VCS root down to (not
/// including) the scan root, plus the VCS root's info/exclude.
fn init_ancestors(root: &Path, env: &Env) -> Ancestors {
    let mut anc = Ancestors {
        files: Vec::new(),
        ceiling_root: true,
    };
    let root = resolve_root(root, env);
    let Some(ceiling) = find_vcs_root(&root, env) else {
        return anc;
    };
    if bytes(&ceiling) == bytes(&root) {
        return anc;
    }
    anc.ceiling_root = false;
    let mut dirs = Vec::new(); // innermost first
    let mut d = dir_of(&root);
    loop {
        let parent = dir_of(&d);
        let last = bytes(&d) == bytes(&ceiling) || bytes(&parent) == bytes(&d);
        dirs.push(d);
        if last {
            break;
        }
        d = parent;
    }
    for d in dirs.iter().rev() {
        let Ok(rel) = gostd::filepath::rel(&go_lossy(bytes(d)), &go_lossy(bytes(&root))) else {
            continue;
        };
        let mut f = IgnoreFile {
            base: rel,
            rules: Vec::new(),
        };
        let io = env.io(d);
        if bytes(d) == bytes(&ceiling)
            && let Some(excl) = info_exclude_of(&io)
            && let Some(data) = read_regular(&excl)
        {
            f.rules.extend(parse_ignore("", &go_lossy(&data)).rules);
        }
        for name in [".gitignore", ".graphifyignore"] {
            if let Some(data) = read_regular(&join(&[bytes(&io), name.as_bytes()])) {
                f.rules.extend(parse_ignore("", &go_lossy(&data)).rules);
            }
        }
        if !f.rules.is_empty() {
            anc.files.push(f);
        }
    }
    anc
}

/// Go `applyRules`: runs `rules` in order over `sub` and returns the last match's verdict, or
/// `decision` when none matches.
pub(super) fn apply_rules(rules: &[IgnoreRule], sub: &str, is_dir: bool, decision: bool) -> bool {
    let base = match sub.rfind('/') {
        Some(i) => &sub[i + 1..],
        None => sub,
    };
    let mut decision = decision;
    for r in rules {
        if r.dir_only && !is_dir {
            continue;
        }
        let target = if r.anchored { sub } else { base };
        if target.starts_with(&r.prefix) && target.ends_with(&r.suffix) && r.matches(target) {
            decision = !r.negate;
        }
    }
    decision
}

impl IgnoreRule {
    /// Go `ignoreRule.matches`: tests `target` (the basename, or the sub-path for an anchored
    /// rule) without the literal prefix and suffix check.
    pub(super) fn matches(&self, target: &str) -> bool {
        if !self.anchored {
            return self.re.as_ref().is_some_and(|re| re.match_string(target));
        }
        if !self.globstar {
            let mut rest = target;
            for (i, s) in self.segs.iter().enumerate() {
                let (part, tail, more) = match rest.split_once('/') {
                    Some((part, tail)) => (part, tail, true),
                    None => (rest, "", false),
                };
                if more != (i < self.segs.len() - 1) || !s.matches(part) {
                    return false;
                }
                rest = tail;
            }
            return true;
        }
        let path: Vec<&str> = target.split('/').collect();
        match_globstar(&path, &self.segs, 0, 0, &mut HashMap::new())
    }
}

/// Go `matchGlobstar` (upstream `_match_globstar_parts`): `**` spans zero or more segments, but
/// one or more when it is the last segment. Memoized; the recursion is bounded by the segment
/// counts of the path and the pattern.
fn match_globstar(
    path: &[&str],
    pat: &[Seg],
    pi: usize,
    qi: usize,
    memo: &mut HashMap<(usize, usize), bool>,
) -> bool {
    if let Some(&v) = memo.get(&(pi, qi)) {
        return v;
    }
    let res = if qi == pat.len() {
        pi == path.len()
    } else if pat[qi].globstar {
        if qi == pat.len() - 1 {
            pi < path.len()
        } else {
            match_globstar(path, pat, pi, qi + 1, memo)
                || (pi < path.len() && match_globstar(path, pat, pi + 1, qi, memo))
        }
    } else {
        pi < path.len()
            && pat[qi].matches(path[pi])
            && match_globstar(path, pat, pi + 1, qi + 1, memo)
    };
    memo.insert((pi, qi), res);
    res
}

/// Go `ancestorDirs`: the directories to consult for `rel`, from the root (`""`) down to its
/// parent. For `a/b/c`: `["", "a", "a/b"]`.
fn ancestor_dirs(rel: &str) -> Vec<&str> {
    let mut dirs = vec![""];
    dirs.extend(rel.match_indices('/').map(|(i, _)| &rel[..i]));
    dirs
}

/// Go `parseIgnore`: the rules of one ignore file's text (`data`, the file's bytes through
/// `go_lossy`), for the directory `base`. A pattern whose regexp does not compile is dropped.
pub(super) fn parse_ignore(base: &str, data: &str) -> IgnoreFile {
    let mut f = IgnoreFile {
        base: base.to_string(),
        rules: Vec::new(),
    };
    for raw in data.split('\n') {
        let line = raw.trim_end_matches('\r').trim_end_matches(' ');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut r = IgnoreRule {
            re: None,
            segs: Vec::new(),
            globstar: false,
            negate: false,
            dir_only: false,
            anchored: false,
            prefix: String::new(),
            suffix: String::new(),
        };
        let mut line = line;
        if let Some(l) = line.strip_prefix('!') {
            r.negate = true;
            line = l;
        }
        line = line.strip_prefix('\\').unwrap_or(line); // escaped leading # or !
        if let Some(l) = line.strip_suffix('/') {
            r.dir_only = true;
            line = l;
        }
        if line.is_empty() {
            continue;
        }
        // A pattern is anchored if it has a leading or embedded slash; otherwise it matches the
        // basename at any depth.
        let leading_slash = line.starts_with('/');
        let trimmed = line.strip_prefix('/').unwrap_or(line);
        r.anchored = leading_slash || trimmed.contains('/');
        if r.anchored {
            let mut ok = true;
            for part in trimmed.split('/') {
                let mut s = Seg {
                    globstar: false,
                    lit: part.to_string(),
                    re: None,
                };
                if part == "**" {
                    s.globstar = true;
                    r.globstar = true;
                } else if part.contains(['*', '?', '[']) {
                    match Regexp::compile(&fnmatch_regex(part)) {
                        Ok(re) => s.re = Some(re),
                        Err(_) => ok = false,
                    }
                }
                r.segs.push(s);
            }
            if !ok {
                continue;
            }
        } else {
            let Ok(re) = Regexp::compile(&fnmatch_regex(trimmed)) else {
                continue;
            };
            r.re = Some(re);
        }
        // Only *, ? and [ start a wildcard; a "[" with no closing "]" stays literal, so stopping
        // the literals there is merely conservative.
        r.prefix = trimmed.to_string();
        if let Some(i) = trimmed.find(['*', '?', '[']) {
            r.prefix = trimmed[..i].to_string();
            if !r.anchored && !trimmed.contains('[') {
                let j = trimmed.rfind(['*', '?']).map_or(0, |k| k + 1);
                r.suffix = trimmed[j..].to_string();
            }
        }
        f.rules.push(r);
    }
    f
}

/// Go `fnmatchRegex`: Python `fnmatch.translate` as an anchored RE2 expression.
fn fnmatch_regex(pat: &str) -> String {
    let p: Vec<char> = pat.chars().collect();
    let n = p.len();
    let mut b = String::from("^(?s:");
    let mut i = 0;
    while i < n {
        let c = p[i];
        i += 1;
        match c {
            '*' => {
                b.push_str(".*");
                while i < n && p[i] == '*' {
                    i += 1;
                }
            }
            '?' => b.push('.'),
            '[' => {
                let mut j = i;
                if j < n && p[j] == '!' {
                    j += 1;
                }
                if j < n && p[j] == ']' {
                    j += 1;
                }
                while j < n && p[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    b.push_str(r"\[");
                    continue;
                }
                let mut chunks = class_chunks(&p, i, j);
                i = j + 1;
                let stuff = chunks.join("-");
                if stuff.is_empty() {
                    b.push_str(r"[^\x00-\x{10FFFF}]"); // never matches
                } else if stuff == "!" {
                    b.push('.');
                } else {
                    b.push('[');
                    if stuff.starts_with('!') {
                        b.push('^');
                        chunks[0].remove(0);
                    }
                    for (ci, ch) in chunks.iter().enumerate() {
                        if ci > 0 {
                            b.push('-');
                        }
                        for r in ch.chars() {
                            if r"\]^[-".contains(r) {
                                b.push('\\');
                            }
                            b.push(r);
                        }
                    }
                    b.push(']');
                }
            }
            _ => b.push_str(&quote_meta(c.encode_utf8(&mut [0; 4]))),
        }
    }
    b.push_str(")$");
    b
}

/// Go `classChunks`: splits the class body `p[i..j]` at range hyphens and drops reversed ranges,
/// exactly as `fnmatch.translate` does.
fn class_chunks(p: &[char], i: usize, j: usize) -> Vec<String> {
    let hyphen = |from: usize| (from..j).find(|&k| p[k] == '-');
    if hyphen(i).is_none() {
        return vec![p[i..j].iter().collect()];
    }
    let mut i = i;
    let mut chunks: Vec<Vec<char>> = Vec::new();
    let mut k = if p[i] == '!' { i + 2 } else { i + 1 };
    while let Some(h) = hyphen(k) {
        k = h;
        chunks.push(p[i..k].to_vec());
        i = k + 1;
        k += 3;
    }
    if i < j {
        chunks.push(p[i..j].to_vec());
    } else if let Some(last) = chunks.last_mut() {
        last.push('-');
    }
    for k in (1..chunks.len()).rev() {
        let (a, c) = (&chunks[k - 1], &chunks[k]);
        if let (Some(&al), Some(&c0)) = (a.last(), c.first())
            && al > c0
        {
            let merged: Vec<char> = a[..a.len() - 1].iter().chain(&c[1..]).copied().collect();
            chunks[k - 1] = merged;
            chunks.remove(k);
        }
    }
    chunks
        .into_iter()
        .map(|c| c.into_iter().collect())
        .collect()
}
