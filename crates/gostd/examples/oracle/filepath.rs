//! `filepath`: gostd::filepath against Go path/filepath (see parity/go/oracle/filepath.go, which
//! documents the input and output formats). `lex-*` cases run the lexical ops; `tree-*` cases
//! build the described tree under /tmp/oracle-tree/<case>, run walk, glob and eval_symlinks, and
//! remove it.

use super::Op;
use gostd::filepath::{
    Walk, base, clean, dir, eval_symlinks, ext, glob, is_abs, join, match_pattern, rel, walk_dir,
};
use std::fmt::Write;
use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;

const TREE_DIR: &str = "/tmp/oracle-tree";

/// The ten glob patterns of the Go oracle (`fpGlobs`).
const GLOBS: &[&str] = &[
    "*", ".*", "*/*", "*/*/*", "[A-Z]*", "s*/*", "?/*", "*/[a-c]*", "*.[gt]*", "[",
];

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    let text = std::str::from_utf8(input).expect("generated input is valid UTF-8");
    if name.starts_with("tree-") {
        run_tree(name, text)
    } else {
        run_lex(text)
    }
}

fn run_lex(text: &str) -> Vec<Op> {
    let f: Vec<&str> = text.split('\x1f').collect();
    let [a, b, pat, name] = f.as_slice() else {
        panic!("filepath: bad lex input");
    };
    let text_or_err = |r: Result<String, String>| r.map(String::into_bytes);
    vec![
        ("clean", Ok(clean(a).into_bytes())),
        ("dir", Ok(dir(a).into_bytes())),
        ("base", Ok(base(a).into_bytes())),
        ("ext", Ok(ext(a).as_bytes().to_vec())),
        ("is_abs", Ok(is_abs(a).to_string().into_bytes())),
        ("join", Ok(join(&[*a, *b]).into_bytes())),
        ("rel", text_or_err(rel(a, b).map_err(|e| e.to_string()))),
        (
            "match",
            text_or_err(
                match_pattern(pat, name)
                    .map(|m| m.to_string())
                    .map_err(|e| e.to_string()),
            ),
        ),
    ]
}

/// A tree description: the walk root suffix and the entries `(kind, path, target)`.
fn parse_tree(text: &str) -> (&str, Vec<(&str, &str, &str)>) {
    let mut lines = text.strip_suffix('\n').unwrap_or(text).split('\n');
    let root = lines
        .next()
        .and_then(|l| l.strip_prefix("root\t"))
        .expect("tree input starts with a root line");
    let entries = lines
        .map(|l| {
            let mut f = l.split('\t');
            let kind = f.next().unwrap_or("");
            let path = f.next().unwrap_or("");
            (kind, path, f.next().unwrap_or(""))
        })
        .collect();
    (root, entries)
}

/// `fpRel`: p relative to the tree dir.
fn rel_to<'a>(tree: &str, p: &'a str) -> &'a str {
    if p == tree {
        return ".";
    }
    p.strip_prefix(format!("{tree}/").as_str()).unwrap_or(p)
}

/// `fpClass`: the error class (the OS error texts differ between Go and Rust).
fn class(e: &io::Error) -> &'static str {
    if e.kind() == io::ErrorKind::NotFound {
        "notexist"
    } else if e.raw_os_error() == Some(20) {
        "notdir"
    } else if e.to_string() == "EvalSymlinks: too many links" {
        "toomany"
    } else {
        "other"
    }
}

fn run_tree(name: &str, text: &str) -> Vec<Op> {
    let tree = format!("{TREE_DIR}/{name}");
    let (root, entries) = parse_tree(text);
    let _ = fs::remove_dir_all(&tree);
    fs::create_dir_all(&tree).expect("create tree dir");
    for &(kind, path, target) in &entries {
        let p = format!("{tree}/{path}");
        match kind {
            "d" => fs::create_dir(&p).expect("create dir"),
            "f" => drop(fs::File::create(&p).expect("create file")),
            "l" => match target.strip_prefix('@') {
                Some(t) => symlink(format!("{tree}{t}"), &p).expect("create symlink"),
                None => symlink(target, &p).expect("create symlink"),
            },
            _ => panic!("filepath: bad tree entry kind {kind:?}"),
        }
    }
    let start = if root.is_empty() {
        tree.clone()
    } else {
        format!("{tree}/{root}")
    };

    let mut walk = String::new();
    walk_dir(Path::new(&start), |p, d, err| {
        let rp = p.to_string_lossy();
        let rp = rel_to(&tree, &rp);
        let Some(d) = d.filter(|_| err.is_none()) else {
            let _ = writeln!(walk, "{rp}\terr");
            return Walk::Continue;
        };
        let t = if d.file_type.is_dir() {
            "d"
        } else if d.file_type.is_symlink() {
            "l"
        } else {
            "f"
        };
        let _ = writeln!(walk, "{rp}\t{t}");
        if d.name.to_string_lossy().starts_with('s') {
            Walk::SkipDir
        } else {
            Walk::Continue
        }
    })
    .expect("walk callback never stops the walk");

    let mut globs = String::new();
    for pat in GLOBS {
        let _ = writeln!(globs, "# {pat}");
        match glob(Path::new(&format!("{tree}/{pat}"))) {
            Err(e) => {
                let _ = writeln!(globs, "error: {e}");
            }
            Ok(m) => {
                for p in m {
                    let p = p.to_string_lossy();
                    let _ = writeln!(globs, "{}", rel_to(&tree, &p));
                }
            }
        }
    }

    let mut eval = String::new();
    for &(_, path, _) in &entries {
        for suffix in ["", "/..", "/nope"] {
            let p = format!("{path}{suffix}");
            match eval_symlinks(Path::new(&format!("{tree}/{p}"))) {
                Err(e) => {
                    let _ = writeln!(eval, "{p}\t!{}", class(&e));
                }
                Ok(res) => {
                    let res = res.to_string_lossy();
                    let _ = writeln!(eval, "{p}\t{}", rel_to(&tree, &res));
                }
            }
        }
    }

    fs::remove_dir_all(&tree).expect("remove tree dir");
    vec![
        ("walk", Ok(walk.into_bytes())),
        ("glob", Ok(globs.into_bytes())),
        ("eval_symlinks", Ok(eval.into_bytes())),
    ]
}
