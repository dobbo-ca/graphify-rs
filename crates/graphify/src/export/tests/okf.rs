//! Port of `okf_test.go` (5 tests), then the added test `okf_ties_use_go_pdqsort`. No upstream
//! file tests the OKF bundle (graphify v0.9.107 has no OKF export).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gostd::filepath::{Walk, walk_dir};

use crate::export::okf::{concept_dir, safe_join};
use crate::export::okf_from_json;
use crate::testutil::TempDir;

// okfSampleJSON has two nodes in distinct source dirs, one edge between them, and
// a community + label with a YAML-significant character to exercise quoting.
const OKF_SAMPLE_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"src_a_go_add","label":"Add: x","file_type":"code","source_file":"src/a.go","source_location":"L3","community":0,"norm_label":"add: x"},
 {"id":"pkg_b_go_b","label":"B()","file_type":"code","source_file":"pkg/b.go","community":1,"norm_label":"b()"}
],
"links":[
 {"source":"src_a_go_add","target":"pkg_b_go_b","relation":"calls","confidence":"INFERRED","confidence_score":0.5}
],
"hyperedges":[]}"#;

/// Go `writeOKFSample`: the temp dir (kept alive by the caller) and the graph.json in it.
fn write_okf_sample() -> (TempDir, PathBuf) {
    let dir = TempDir::new();
    let json_path = dir.write("graph.json", OKF_SAMPLE_JSON.as_bytes());
    (dir, json_path)
}

/// Go `readBundle`: walks `out` and returns a map of bundle-relative slash paths to file
/// contents, so tests can assert on the whole tree.
fn read_bundle(out: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    walk_dir(out, |p, d, err| {
        if let Some(e) = err {
            return Walk::Stop(io::Error::new(e.kind(), e.to_string()));
        }
        if d.is_none_or(|d| d.file_type.is_dir()) {
            return Walk::Continue;
        }
        let rel = p.strip_prefix(out).expect("under out");
        let data = fs::read(p).unwrap_or_default();
        files.insert(
            rel.to_str().expect("UTF-8 path").to_string(),
            String::from_utf8(data).expect("UTF-8 file"),
        );
        Walk::Continue
    })
    .expect("walk");
    files
}

/// Go `keys`.
fn keys(m: &BTreeMap<String, String>) -> Vec<&String> {
    m.keys().collect()
}

#[test]
fn test_okf_concept_path_and_frontmatter() {
    let (dir, json_path) = write_okf_sample();
    let out = dir.join("okf");
    okf_from_json(&json_path, &out).expect("OKFFromJSON");
    let files = read_bundle(&out);

    // Concept ID = bundle path with .md stripped: <source-dir>/<id>.md. Each path
    // segment is sanitised via idutil for safety, so "a.go" becomes "a_go".
    const CONCEPT: &str = "src/a_go/src_a_go_add.md";
    let Some(doc) = files.get(CONCEPT) else {
        panic!(
            "missing concept document {CONCEPT}; bundle: {:?}",
            keys(&files)
        );
    };

    // Frontmatter is a leading --- block with the REQUIRED `type` field.
    let Some(fm) = doc.strip_prefix("---\n") else {
        panic!("concept does not start with YAML frontmatter:\n{doc}");
    };
    let Some(end) = fm.find("\n---\n") else {
        panic!("frontmatter not terminated:\n{doc}");
    };
    let fm = &fm[..end];
    for want in [
        r#"type: "code""#,
        r#"title: "Add: x""#, // the colon must be quoted, not breaking YAML
        "description:",
        r#"resource: "src/a.go:L3""#,
        "community:0",
    ] {
        assert!(fm.contains(want), "frontmatter missing {want:?}:\n{fm}");
    }
}

#[test]
fn test_okf_cross_link_form() {
    let (dir, json_path) = write_okf_sample();
    let out = dir.join("okf");
    okf_from_json(&json_path, &out).expect("OKFFromJSON");
    let files = read_bundle(&out);

    let doc = &files["src/a_go/src_a_go_add.md"];
    assert!(
        doc.contains("# Relations"),
        "concept missing # Relations section:\n{doc}"
    );
    // Outgoing `calls` edge -> absolute, bundle-relative link (leading /, .md).
    const LINK: &str = "(/pkg/b_go/pkg_b_go_b.md)";
    assert!(
        doc.contains(LINK),
        "relation link {LINK:?} not found:\n{doc}"
    );
    assert!(
        doc.contains("## calls"),
        "relations not grouped by relation name:\n{doc}"
    );
    // The reverse direction is recorded on the callee.
    let callee = &files["pkg/b_go/pkg_b_go_b.md"];
    assert!(
        callee.contains("(/src/a_go/src_a_go_add.md)"),
        "callee missing incoming relation link:\n{callee}"
    );
}

#[test]
fn test_okf_index_generation() {
    let (dir, json_path) = write_okf_sample();
    let out = dir.join("okf");
    okf_from_json(&json_path, &out).expect("OKFFromJSON");
    let files = read_bundle(&out);

    for idx in [
        "index.md",
        "src/index.md",
        "src/a_go/index.md",
        "pkg/index.md",
        "pkg/b_go/index.md",
    ] {
        assert!(
            files.contains_key(idx),
            "missing {idx}; bundle: {:?}",
            keys(&files)
        );
    }
    // The root index lists its subdirectories.
    let root = &files["index.md"];
    assert!(
        root.contains("(/src/index.md)") && root.contains("(/pkg/index.md)"),
        "root index missing subdirectory links:\n{root}"
    );
    // A leaf index lists its concept with a description.
    let leaf = &files["src/a_go/index.md"];
    assert!(
        leaf.contains("(/src/a_go/src_a_go_add.md)"),
        "leaf index missing concept link:\n{leaf}"
    );
}

#[test]
fn test_okf_deterministic() {
    let (dir, json_path) = write_okf_sample();
    let out1 = dir.join("okf1");
    let out2 = dir.join("okf2");
    okf_from_json(&json_path, &out1).expect("OKFFromJSON");
    okf_from_json(&json_path, &out2).expect("OKFFromJSON");
    let (a, b) = (read_bundle(&out1), read_bundle(&out2));
    assert_eq!(
        a.len(),
        b.len(),
        "bundle file counts differ: {} vs {}",
        a.len(),
        b.len()
    );
    for (k, va) in &a {
        assert!(
            b.get(k).is_some_and(|vb| va == vb),
            "byte-stable mismatch for {k}"
        );
    }
}

#[test]
fn test_okf_containment() {
    // A source path with .. components must not escape the bundle root; conceptDir
    // sanitises each segment to a safe idutil token, so traversal is impossible.
    let d = concept_dir("../../etc/passwd");
    assert!(
        !d.contains(".."),
        "conceptDir leaked traversal segments: {d:?}"
    );
    assert!(
        safe_join(Path::new("/base"), "../escape", "x.md").is_err(),
        "safeJoin accepted an escaping path, want error"
    );
}

// ---- Added test ----

// okf.go sorts with `sort.Slice` (Go pdqsort, unstable) at three sites. A tie is a pair of
// elements the comparator calls equal; the order pdqsort leaves them in is not the stable
// order, so each site is pinned by an input with 40+ elements and 13+ equal keys (pdqsort
// insertion-sorts up to 12 elements, which is stable). The expected order is built here with
// `gostd::sort::slice` and the comparator copied from okf.go (gostd's sort is checked against
// Go's pdqsort), and each expectation is also required to differ from a stable `sort_by`, so a
// stable sort in the port fails the test.
//
// Setup: 42 or 45 nodes `g<k>-..-z` in `src/a.go`, k in 0..m. idutil.MakeID maps every dash run
// to one `_`, so all nodes with the same k share the concept file stem `g<k>_z` (the ties). They
// are labelled `L<j>` by position j. One node `hub` in `src/h.go` has an edge to every `L<j>`,
// so its relations tie by (relation, direction, link) in groups too.
//
//  - okf.go:64, `sort.Slice(concepts, file)`: the concepts are written in the sorted order, and
//    nodes with one file stem write one file, so the last of the group in sorted order wins:
//    its label is the `title:` of `src/a_go/g<k>_z.md`.
//  - okf.go:219, `sort.Slice(cs, file)` of one directory's concepts (taken from the :64 order):
//    the order of the `- [label](link) - code` lines of `src/a_go/index.md`. The input is
//    already sorted by file, and pdqsort leaves a sorted input alone (its pivot probe sees no
//    swap, `partialInsertionSort` finds no inversion), so the result is the :64 order; the
//    expectation still applies both sorts, as Go does.
//  - okf.go:118, `sort.Slice(r, rel, dir, link)` of the hub's relations: the order of the
//    `- -> [L<j>](link)` lines of `src/h_go/hub.md`.
//
// The key pattern (j * a) % m is searched over a few (n, m, a) until the three orders all
// differ from the stable ones: the one that is used is not computed by hand, the assertions are.
#[test]
fn okf_ties_use_go_pdqsort() {
    /// A concept as okf.go:64 sorts it.
    #[derive(Clone)]
    struct C {
        dir: &'static str,
        file: String,
        label: String,
    }
    /// A relation as okf.go:118 sorts it.
    #[derive(Clone)]
    struct R {
        rel: &'static str,
        dir: &'static str,
        label: String,
        link: String,
    }
    const A_DIR: &str = "src/a_go";
    const H_DIR: &str = "src/h_go";

    // okf.go:64 and :219.
    let less_file = |a: &C, b: &C| a.file < b.file;
    // okf.go:118-127.
    let less_rel = |a: &R, b: &R| {
        if a.rel != b.rel {
            return a.rel < b.rel;
        }
        if a.dir != b.dir {
            return a.dir < b.dir;
        }
        a.link < b.link
    };
    let go_sort = |v: &[C]| {
        let mut o = v.to_vec();
        gostd::sort::slice(&mut o, less_file);
        o
    };

    // What the three sites make of one key pattern, or None when a stable sort would match.
    struct Expect {
        keys: Vec<usize>,
        winners: BTreeMap<usize, String>,
        index_labels: Vec<String>,
        hub_labels: Vec<String>,
    }
    let expect = |keys: Vec<usize>| -> Option<Expect> {
        let mut concepts: Vec<C> = keys
            .iter()
            .enumerate()
            .map(|(j, k)| C {
                dir: A_DIR,
                file: format!("g{k}_z"),
                label: format!("L{j}"),
            })
            .collect();
        concepts.push(C {
            dir: H_DIR,
            file: "hub".to_string(),
            label: "Hub".to_string(),
        });

        // :64, then the winner per file stem: the last in sorted order.
        let after64 = go_sort(&concepts);
        let mut winners = BTreeMap::new();
        let mut stable_winners = BTreeMap::new();
        for (j, k) in keys.iter().enumerate() {
            stable_winners.insert(*k, format!("L{j}")); // stable: the last in input order
        }
        for c in after64.iter().filter(|c| c.dir == A_DIR) {
            let k: usize = c.file[1..c.file.len() - 2].parse().expect("g<k>_z");
            winners.insert(k, c.label.clone());
        }
        // :219 on the concepts of the directory, in the :64 order.
        let in_dir: Vec<C> = after64.iter().filter(|c| c.dir == A_DIR).cloned().collect();
        let index_labels: Vec<String> = go_sort(&in_dir).into_iter().map(|c| c.label).collect();
        let stable_index: Vec<String> = {
            let mut v: Vec<C> = concepts
                .iter()
                .filter(|c| c.dir == A_DIR)
                .cloned()
                .collect();
            v.sort_by(|a, b| a.file.cmp(&b.file));
            v.into_iter().map(|c| c.label).collect()
        };
        // :118 on the hub's relations, in edge order.
        let rels: Vec<R> = keys
            .iter()
            .enumerate()
            .map(|(j, k)| R {
                rel: "calls",
                dir: "->",
                label: format!("L{j}"),
                link: format!("/{A_DIR}/g{k}_z.md"),
            })
            .collect();
        let mut sorted = rels.clone();
        gostd::sort::slice(&mut sorted, less_rel);
        let hub_labels: Vec<String> = sorted.into_iter().map(|r| r.label).collect();
        let stable_hub: Vec<String> = {
            let mut v = rels.clone();
            v.sort_by(|a, b| a.link.cmp(&b.link));
            v.into_iter().map(|r| r.label).collect()
        };

        (winners != stable_winners && index_labels != stable_index && hub_labels != stable_hub)
            .then_some(Expect {
                keys,
                winners,
                index_labels,
                hub_labels,
            })
    };
    let mut found = None;
    'search: for n in [42, 45] {
        for m in [2, 3] {
            for a in [1, 2, 5, 7] {
                let keys: Vec<usize> = (0..n).map(|j| (j * a) % m).collect();
                if let Some(e) = expect(keys) {
                    found = Some(e);
                    break 'search;
                }
            }
        }
    }
    let e = found.expect("no key pattern separates Go's pdqsort from a stable sort");

    // The graph.json: the group nodes in position order, then the hub.
    let mut nodes = Vec::new();
    for (j, k) in e.keys.iter().enumerate() {
        let id = format!("g{k}{}z", "-".repeat(j + 1));
        nodes.push(format!(
            r#"{{"id":"{id}","label":"L{j}","file_type":"code","source_file":"src/a.go","community":null,"norm_label":"l{j}"}}"#
        ));
    }
    nodes.push(
        r#"{"id":"hub","label":"Hub","file_type":"code","source_file":"src/h.go","community":null,"norm_label":"hub"}"#
            .to_string(),
    );
    let links: Vec<String> = e
        .keys
        .iter()
        .enumerate()
        .map(|(j, k)| {
            let id = format!("g{k}{}z", "-".repeat(j + 1));
            format!(
                r#"{{"source":"hub","target":"{id}","relation":"calls","confidence":"EXTRACTED","confidence_score":1}}"#
            )
        })
        .collect();
    let json = format!(
        r#"{{"directed":true,"multigraph":false,"graph":{{}},"nodes":[{}],"links":[{}],"hyperedges":[]}}"#,
        nodes.join(",\n"),
        links.join(",\n")
    );
    let dir = TempDir::new();
    let json_path = dir.write("graph.json", json.as_bytes());
    let out = dir.join("okf");
    okf_from_json(&json_path, &out).expect("OKFFromJSON");
    let files = read_bundle(&out);

    // :64: the title of each shared concept file is the last label in sorted order.
    for (k, label) in &e.winners {
        let doc = &files[&format!("{A_DIR}/g{k}_z.md")];
        assert!(
            doc.contains(&format!("title: \"{label}\"\n")),
            "g{k}_z.md should be written last by {label}:\n{doc}"
        );
    }
    // :219: the index lists the concepts in the sorted order.
    let index: Vec<&str> = files[&format!("{A_DIR}/index.md")]
        .lines()
        .filter(|l| l.starts_with("- ["))
        .collect();
    let want_index: Vec<String> = e
        .index_labels
        .iter()
        .map(|label| {
            let j: usize = label[1..].parse().expect("L<j>");
            format!("- [{label}](/{A_DIR}/g{}_z.md) - code", e.keys[j])
        })
        .collect();
    assert_eq!(index, want_index, "{A_DIR}/index.md order");
    // :118: the hub's relations are listed in the sorted order.
    let hub: Vec<&str> = files[&format!("{H_DIR}/hub.md")]
        .lines()
        .filter(|l| l.starts_with("- -> ["))
        .collect();
    let want_hub: Vec<String> = e
        .hub_labels
        .iter()
        .map(|label| {
            let j: usize = label[1..].parse().expect("L<j>");
            format!("- -> [{label}](/{A_DIR}/g{}_z.md)", e.keys[j])
        })
        .collect();
    assert_eq!(hub, want_hub, "{H_DIR}/hub.md relation order");
}
