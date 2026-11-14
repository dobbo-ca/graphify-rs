//! The cases of the upstream graphify v0.9.107 `tests/test_terraform_modules.py` whose assertion
//! holds for graphify-go v0.12.0 (see the `extract-hcl` block of `parity/README.md`). The sources
//! are written inline, and upstream's `_extract(root)` is Go's `FileFromBytes` of each file then
//! `Resolve` over those files. Go gives a module call no `module_source` edge and no
//! `Terraform module: <dir>` node. `Resolve` writes a `references` edge from the module block to
//! the directory node of a local source that holds a corpus file (label: the cleaned directory
//! path, with a `contains` edge to each of its files), and to a concept node (label: the source,
//! or the cleaned path of a local one the corpus lacks) for any other source. The edge's location
//! is the module block's line, not its `source` attribute's.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::tests::util::extract_bytes;
use crate::extract::{Result as Extracted, resolve};
use crate::model::{Confidence, Extraction, Relation};

/// Upstream `_extract`: `FileFromBytes` of each `(rel, src)`, then `Resolve` over those files
/// (GOMAXPROCS workers).
fn run(srcs: &[(&str, &str)]) -> Extraction {
    let results: Vec<Extracted> = srcs
        .iter()
        .map(|(rel, src)| extract_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// Whether an edge of `relation` runs from a node labelled `src` to one labelled `tgt`.
fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    let label = id2label(ext);
    ext.edges.iter().any(|e| {
        e.relation.as_str() == relation
            && label.get(e.source.as_str()) == Some(&src)
            && label.get(e.target.as_str()) == Some(&tgt)
    })
}

/// The labels of the nodes the `references` edges of the node labelled `src` reach (an edge to an
/// id with no node, such as `var.name`, is left out).
fn ref_targets(ext: &Extraction, src: &str) -> BTreeSet<String> {
    let label = id2label(ext);
    let from: BTreeSet<&str> = ext
        .nodes
        .iter()
        .filter(|n| n.label == src)
        .map(|n| n.id.as_str())
        .collect();
    ext.edges
        .iter()
        .filter(|e| e.relation == Relation::REFERENCES && from.contains(e.source.as_str()))
        .filter_map(|e| label.get(e.target.as_str()).map(|l| (*l).to_string()))
        .collect()
}

/// How many edges of `relation` leave a node labelled `src`.
fn out_edges(ext: &Extraction, src: &str, relation: Relation) -> usize {
    let label = id2label(ext);
    ext.edges
        .iter()
        .filter(|e| e.relation == relation && label.get(e.source.as_str()) == Some(&src))
        .count()
}

// pins Go v0.12.0 (references edges to the directory nodes `applications/app` and `base`, from the
// module blocks, with the block's line); upstream 0.9.107 emits `module_source` edges to
// `Terraform module: <dir>` nodes with the `source` attribute's line (`L2`) -> P3
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_environment_application_base_topology_and_source_provenance() {
    let ext = run(&[
        (
            "envs/dev/main.tf",
            "module \"app\" {\n  source = \"../../applications/app\"\n}\n",
        ),
        (
            "applications/app/main.tf",
            "module \"base\" { source = \"../../base\" }\n",
        ),
        (
            "applications/app/outputs.tf",
            "output \"id\" { value = \"app\" }\n",
        ),
        (
            "base/storage.tf",
            "resource \"aws_s3_bucket\" \"data\" {}\n",
        ),
    ]);
    // env -> application -> base -> the bucket
    assert!(has_edge(
        &ext,
        "module.app",
        "references",
        "applications/app"
    ));
    assert!(has_edge(&ext, "applications/app", "contains", "main.tf"));
    assert!(has_edge(&ext, "module.base", "references", "base"));
    assert!(has_edge(&ext, "base", "contains", "storage.tf"));
    assert!(has_edge(
        &ext,
        "storage.tf",
        "contains",
        "aws_s3_bucket.data"
    ));
    assert!(has_edge(&ext, "applications/app", "contains", "outputs.tf"));

    let label = id2label(&ext);
    let edge = ext
        .edges
        .iter()
        .find(|e| {
            e.relation == Relation::REFERENCES
                && label.get(e.source.as_str()) == Some(&"module.app")
                && label.get(e.target.as_str()) == Some(&"applications/app")
        })
        .expect("module.app --references--> applications/app");
    assert_eq!(edge.source_file.as_str(), "envs/dev/main.tf");
    assert_eq!(edge.source_location.to_string(), "L1");
    assert_eq!(edge.confidence, Confidence::EXTRACTED);
    let module_edges = ext
        .edges
        .iter()
        .filter(|e| {
            e.relation == Relation::REFERENCES
                && label
                    .get(e.source.as_str())
                    .is_some_and(|l| l.starts_with("module."))
        })
        .count();
    assert_eq!(module_edges, 2);
}

// Without upstream's `a-b/app` and `a_b/app`: the id of `var.name` is built by `MakeID`, which
// turns `-` and `_` alike into `_`, so those two directories share one id (node id, by design).
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_same_named_directories_and_cross_file_references_stay_separate() {
    let mut srcs = Vec::new();
    for dir in ["dev/app", "prod/app"] {
        srcs.push((format!("{dir}/main.tf"), "variable \"name\" {}\n"));
        srcs.push((
            format!("{dir}/use.tf"),
            "output \"name\" { value = var.name }\n",
        ));
    }
    let srcs: Vec<(&str, &str)> = srcs.iter().map(|(rel, src)| (rel.as_str(), *src)).collect();
    let ext = run(&srcs);

    let variables: Vec<_> = ext.nodes.iter().filter(|n| n.label == "var.name").collect();
    let ids: BTreeSet<&str> = variables.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    for variable in variables {
        let (dir, _) = variable
            .source_file
            .rsplit_once('/')
            .expect("a variable's source file has a directory");
        let source = format!("{dir}/use.tf");
        let output = ext
            .nodes
            .iter()
            .find(|n| n.label == "output.name" && n.source_file == source)
            .unwrap_or_else(|| panic!("no output.name in {source}"));
        assert!(
            ext.edges
                .iter()
                .any(|e| e.source == output.id && e.target == variable.id),
            "{source}: output.name does not reference its own var.name"
        );
    }
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_raw_extractor_scopes_ids_by_full_directory() {
    let ids: Vec<String> = ["dev/app/main.tf", "prod/app/main.tf"]
        .iter()
        .map(|rel| {
            let r = extract_bytes(rel, b"variable \"x\" {}");
            r.nodes
                .iter()
                .find(|n| n.label == "var.x")
                .unwrap_or_else(|| panic!("no var.x in {rel}"))
                .id
                .clone()
        })
        .collect();
    assert_ne!(ids[0], ids[1]);
}

// pins Go v0.12.0 (the directory nodes are labelled `child` and `.`, and `module.parent` reaches
// `.` as the source `../` of `child/main.tf` cleans to the corpus root); upstream 0.9.107 labels
// them `Terraform module: child` and `Terraform module: .`, counts them as 2 `module` nodes and
// the `module_source` edges as 3 -> P3
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_multiple_calls_share_one_module_and_cycles_are_finite() {
    let ext = run(&[
        (
            "main.tf",
            "module \"a\" { source = \"./child\" }\nmodule \"b\" { source = \"./child\" }\n",
        ),
        ("child/main.tf", "module \"parent\" { source = \"../\" }\n"),
    ]);
    assert_eq!(ext.nodes.iter().filter(|n| n.label == "child").count(), 1);
    assert_eq!(ext.nodes.iter().filter(|n| n.label == ".").count(), 1);
    assert!(has_edge(&ext, "module.a", "references", "child"));
    assert!(has_edge(&ext, "module.b", "references", "child"));
    assert!(has_edge(&ext, "module.parent", "references", "."));
    let module_edges: usize = ["module.a", "module.b", "module.parent"]
        .iter()
        .map(|m| out_edges(&ext, m, Relation::REFERENCES))
        .sum();
    assert_eq!(module_edges, 3);
}

/// Upstream's `test_unresolved_sources_do_not_create_topology` cases: the `source` expression of
/// a module in `env/main.tf` (`app/main.tf` is in the corpus), and the labels Go's `references`
/// edges of `module.app` reach.
const UNRESOLVED_SOURCES: [(&str, &[&str]); 7] = [
    ("\"hashicorp/consul/aws\"", &["hashicorp/consul/aws"]),
    (
        "\"git::https://example.com/module.git//sub?ref=v1\"",
        &["git::https://example.com/module.git//sub?ref=v1"],
    ),
    (
        "\"https://example.com/module.zip\"",
        &["https://example.com/module.zip"],
    ),
    ("\"../missing\"", &["missing"]),
    ("\"../${var.name}\"", &["${var.name}"]),
    ("var.source", &[]),
    ("\"../app\" + \"suffix\"", &["app"]),
];

// pins Go v0.12.0 (a registry, git or URL source, and a local one the corpus lacks, give a
// `references` edge to a concept node labelled with the source or the cleaned path; the first
// string literal is the source, so `"../app" + "suffix"` is `../app` and reaches the directory
// node `app`, and an interpolated source is a path as written; `var.source` has no literal and
// gives none); upstream 0.9.107 emits no `module_source` edge for any of them -> P3
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_unresolved_sources_do_not_create_topology() {
    for (source, want) in UNRESOLVED_SOURCES {
        let caller = format!("module \"app\" {{ source = {source} }}\n");
        let ext = run(&[
            ("env/main.tf", caller.as_str()),
            ("app/main.tf", "resource \"aws_s3_bucket\" \"data\" {}\n"),
        ]);
        let want: BTreeSet<String> = want.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(ref_targets(&ext, "module.app"), want, "source {source}");
    }
}

// pins Go v0.12.0 (a local source is followed into any directory that holds a corpus file, so
// `hcl` and `vars` are directory nodes with a `contains` edge to `main.hcl` and `main.tfvars`; a
// source outside the corpus, an excluded directory and one that only has a subdirectory of the
// corpus are concept nodes of the cleaned path); upstream 0.9.107 follows none of the five
// -> P3
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_ignored_outside_and_non_tf_targets_are_not_followed() {
    let ext = run(&[
        (
            "main.tf",
            r#"
module "outside" { source = "../outside" }
module "excluded" { source = "./excluded" }
module "hcl" { source = "./hcl" }
module "vars" { source = "./vars" }
module "nested" { source = "./nested" }
"#,
        ),
        ("hcl/main.hcl", "variable \"x\" {}"),
        ("vars/main.tfvars", "x = 1"),
        ("nested/sub/main.tf", "variable \"x\" {}"),
    ]);
    for (module, target) in [
        ("module.outside", "../outside"),
        ("module.excluded", "excluded"),
        ("module.hcl", "hcl"),
        ("module.vars", "vars"),
        ("module.nested", "nested"),
    ] {
        assert_eq!(
            ref_targets(&ext, module),
            BTreeSet::from([target.to_string()]),
            "{module}"
        );
    }
    assert!(has_edge(&ext, "hcl", "contains", "main.hcl"));
    assert!(has_edge(&ext, "vars", "contains", "main.tfvars"));
    for concept in ["../outside", "excluded", "nested"] {
        assert_eq!(
            out_edges(&ext, concept, Relation::CONTAINS),
            0,
            "{concept} is a concept node and contains nothing"
        );
    }
}

// pins Go v0.12.0 (the source text as written: `app` and `$${literal}`, never decoded, so
// both are concept nodes of the cleaned path, and a `source` inside the `settings` object is not
// read); upstream 0.9.107 decodes them to `./app` and `./${literal}`, resolves both to their
// directories, and records `module_source` on the node -> P3
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_source_literals_escapes_and_nested_source_attributes() {
    let ext = run(&[
        (
            "main.tf",
            r#"module "app" {
  source = "./app"
  settings = { source = "./wrong" }
}
module "escaped" { source = "./$${literal}" }
"#,
        ),
        ("app/main.tf", "variable \"x\" {}"),
        ("${literal}/main.tf", "variable \"x\" {}"),
    ]);
    assert_eq!(
        ref_targets(&ext, "module.app"),
        BTreeSet::from([r"app".to_string()])
    );
    assert_eq!(
        ref_targets(&ext, "module.escaped"),
        BTreeSet::from(["$${literal}".to_string()])
    );
}
