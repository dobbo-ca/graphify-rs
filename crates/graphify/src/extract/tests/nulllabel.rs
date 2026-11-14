//! graphify-go `internal/extract/nulllabel_test.go`. None of these tests calls a function of
//! `resolve.go` or `nulllabel_resolve.go` itself: the six that complete a partial id from a
//! wrapper chain call `Resolve`, so they stay here (ignored on `graphify-rs-ihc.18`), and nothing
//! moved to the resolve tests PR.

use crate::extract::tests::util::extract_bytes;
use crate::extract::{Result as Extracted, file, file_from_bytes, kinds, resolve};
use crate::idutil::make_id;
use crate::model::{Extraction, Relation};
use crate::testutil::testdata;

/// Go `composeFromHCL`: parses a single module body and returns its reconstructed id.
fn compose_from_hcl(body: &str) -> String {
    let res = extract_bytes(
        "main.tf",
        format!("module \"this\" {{\n  source = \"cloudposse/label/null\"\n{body}\n}}\n")
            .as_bytes(),
    );
    for n in &res.nodes {
        if n.label == "module.this [null-label]" {
            return n.computed_name.clone();
        }
    }
    panic!("no null-label module node found")
}

/// `Resolve` of `results` with `files` (GOMAXPROCS workers).
fn resolve_files(results: &[Extracted], files: &[&str]) -> Extraction {
    let files: Vec<String> = files.iter().map(|f| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(results, &files, workers)
}

/// Go `computedNameOf`: the `ComputedName` of the null-label module node living in `dir`, or ""
/// if not found. The node ID encodes its dir scope, so two null-label modules in different dirs
/// are distinguishable.
fn computed_name_of(ext: &Extraction, dir: &str) -> String {
    let want = make_id(&[dir, "module.this"]);
    for n in &ext.nodes {
        if n.id == want {
            return n.computed_name.clone();
        }
    }
    String::new()
}

/// Go `TestComposeID`'s cases: (name, module body, want).
const COMPOSE_ID_CASES: [(&str, &str, &str); 9] = [
    (
        "defaultOrder",
        concat!(
            r#"name = "asdf""#,
            "\n",
            r#"attributes = ["1","2"]"#,
            "\n",
            r#"delimiter = "!""#
        ),
        "asdf!1!2",
    ),
    (
        "reorder",
        concat!(
            r#"name = "asdf""#,
            "\n",
            r#"attributes = ["1","2"]"#,
            "\n",
            r#"delimiter = "!""#,
            "\n",
            r#"label_order = ["attributes","name"]"#
        ),
        "1!2!asdf",
    ),
    (
        "tenantViaOrder",
        concat!(
            r#"namespace = "eg""#,
            "\n",
            r#"name = "app""#,
            "\n",
            r#"tenant = "acme""#,
            "\n",
            r#"label_order = ["tenant","namespace","name"]"#
        ),
        "acme-eg-app",
    ),
    (
        "defaultDelim",
        concat!(
            r#"namespace = "eg""#,
            "\n",
            r#"stage = "prod""#,
            "\n",
            r#"name = "app""#
        ),
        "eg-prod-app",
    ),
    (
        "emptyDropped",
        concat!(
            r#"namespace = "eg""#,
            "\n",
            r#"stage = """#,
            "\n",
            r#"name = "app""#
        ),
        "eg-app",
    ),
    (
        "lowercased",
        concat!(r#"namespace = "EG""#, "\n", r#"name = "App""#),
        "eg-app",
    ),
    (
        "regexStripped",
        concat!(r#"namespace = "eg_x""#, "\n", r#"name = "a.b""#),
        "egx-ab",
    ),
    (
        "caseNone",
        concat!(
            r#"namespace = "EG""#,
            "\n",
            r#"name = "App""#,
            "\n",
            r#"label_value_case = "none""#
        ),
        "EG-App",
    ),
    (
        "attrsDedup",
        concat!(r#"name = "app""#, "\n", r#"attributes = ["a","a","b"]"#),
        "app-a-b",
    ),
];

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_compose_id() {
    for (name, body, want) in COMPOSE_ID_CASES {
        let got = compose_from_hcl(body);
        assert_eq!(got, want, "composeID({name})");
    }
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_compose_id_partial() {
    let got = compose_from_hcl(concat!(
        "namespace = var.ns",
        "\n",
        r#"name = "app""#,
        "\n",
        "context = var.context"
    ));
    assert_eq!(
        got, "{namespace}-app (partial)",
        "partial id = {got:?}, want {{namespace}}-app (partial)"
    );
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_compose_id_unresolved() {
    let got = compose_from_hcl(concat!(r#"name = "app""#, "\n", "label_order = var.order"));
    assert_eq!(got, "", "unresolved id = {got:?}, want empty");
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_compose_id_no_literals() {
    let got = compose_from_hcl(concat!("namespace = var.ns", "\n", "name = var.name"));
    assert_eq!(got, "", "no-literals id = {got:?}, want empty");
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_null_label_computed_name_on_node() {
    let src = br#"
module "this" {
  source     = "cloudposse/label/null"
  namespace  = "eg"
  stage      = "prod"
  name       = "app"
  attributes = ["public"]
}
resource "aws_s3_bucket" "b" {
  bucket = module.this.id
}
"#;
    let res = extract_bytes("main.tf", src);
    let mut got = "";
    for n in &res.nodes {
        if n.label == "module.this [null-label]" {
            got = n.computed_name.as_str();
        }
    }
    assert_eq!(
        got, "eg-prod-app-public",
        "ComputedName = {got:?}, want eg-prod-app-public"
    );
    let id2label: std::collections::BTreeMap<&str, &str> = res
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let of = |id: &str| id2label.get(id).copied().unwrap_or("");
    let linked = res.edges.iter().any(|e| {
        e.relation == Relation::REFERENCES
            && of(&e.source) == "aws_s3_bucket.b"
            && of(&e.target) == "module.this [null-label]"
    });
    assert!(
        linked,
        "expected aws_s3_bucket.b --references--> module.this"
    );
}

/// The wrapper module of the chain tests: `cloudposse/label/null` fed from variables.
const WRAPPER_VARS: &[u8] = br#"
variable "namespace" {}
variable "stage" {}
variable "name" {}
variable "attributes" { default = [] }

module "this" {
  source     = "cloudposse/label/null"
  namespace  = var.namespace
  stage      = var.stage
  name       = var.name
  attributes = var.attributes
}
"#;

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_null_label_chain_resolved() {
    kinds::register_kinds();
    let root = br#"
module "label" {
  source     = "../modules/label"
  namespace  = "eg"
  stage      = "prod"
  name       = "app"
  attributes = ["1"]
}
"#;
    let r_root = file_from_bytes("root/main.tf", root);
    let r_wrap = file_from_bytes("modules/label/main.tf", WRAPPER_VARS);
    let ext = resolve_files(
        &[r_root, r_wrap],
        &["root/main.tf", "modules/label/main.tf"],
    );
    let got = computed_name_of(&ext, "modules/label");
    assert_eq!(
        got, "eg-prod-app-1",
        "chain ComputedName = {got:?}, want eg-prod-app-1"
    );
}

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_null_label_chain_partial_stop() {
    kinds::register_kinds();
    let root = br#"
module "label" {
  source     = "../modules/label"
  namespace  = var.something
  stage      = "prod"
  name       = "app"
  attributes = ["1"]
}
"#;
    let r_root = file_from_bytes("root/main.tf", root);
    let r_wrap = file_from_bytes("modules/label/main.tf", WRAPPER_VARS);
    let ext = resolve_files(
        &[r_root, r_wrap],
        &["root/main.tf", "modules/label/main.tf"],
    );
    let got = computed_name_of(&ext, "modules/label");
    assert!(
        got.contains("{namespace}"),
        "partial-stop ComputedName = {got:?}, want it to contain {{namespace}}"
    );
    assert!(
        got.ends_with("(partial)"),
        "partial-stop ComputedName = {got:?}, want it to end with (partial)"
    );
}

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_null_label_inherit_partial_parent() {
    kinds::register_kinds();
    let src = br#"
variable "x" {}
module "base" {
  source      = "cloudposse/label/null"
  namespace   = var.x
  environment = "ue1"
}
module "this" {
  source  = "cloudposse/label/null"
  stage   = "prod"
  name    = "app"
  context = module.base.context
}
"#;
    let ext = resolve_files(&[file_from_bytes("main.tf", src)], &["main.tf"]);
    let got = computed_name_of(&ext, "tf");
    assert!(
        got.contains("{namespace}"),
        "inherit-partial-parent ComputedName = {got:?}, want it to contain {{namespace}}"
    );
    assert!(
        got.ends_with("(partial)"),
        "inherit-partial-parent ComputedName = {got:?}, want it to end with (partial)"
    );
}

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_null_label_inherit_exact_parent() {
    kinds::register_kinds();
    let src = br#"
module "base" {
  source      = "cloudposse/label/null"
  namespace   = "eg"
  environment = "ue1"
}
module "this" {
  source  = "cloudposse/label/null"
  stage   = "prod"
  name    = "app"
  context = module.base.context
}
"#;
    let ext = resolve_files(&[file_from_bytes("main.tf", src)], &["main.tf"]);
    let got = computed_name_of(&ext, "tf");
    assert_eq!(
        got, "eg-ue1-prod-app",
        "inherit-exact-parent ComputedName = {got:?}, want eg-ue1-prod-app"
    );
}

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_null_label_wrapper_context_child() {
    kinds::register_kinds();
    let root = br#"
module "label" {
  source      = "../modules/label"
  namespace   = "eg"
  environment = "ue1"
}
"#;
    let wrapper = br#"
variable "namespace" {}
variable "environment" {}

module "base" {
  source      = "cloudposse/label/null"
  namespace   = var.namespace
  environment = var.environment
}
module "this" {
  source  = "cloudposse/label/null"
  stage   = "prod"
  name    = "app"
  context = module.base.context
}
"#;
    let r_root = file_from_bytes("root/main.tf", root);
    let r_wrap = file_from_bytes("modules/label/main.tf", wrapper);
    let ext = resolve_files(
        &[r_root, r_wrap],
        &["root/main.tf", "modules/label/main.tf"],
    );
    let got = computed_name_of(&ext, "modules/label");
    assert_eq!(
        got, "eg-ue1-prod-app",
        "wrapper-context-child ComputedName = {got:?}, want eg-ue1-prod-app"
    );
}

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_null_label_fixture() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("tf").join("label");
    let r = file(&root, "main.tf").unwrap_or_else(|e| panic!("File: {e}"));
    let ext = resolve_files(&[r], &["label/main.tf"]);
    let mut got = "";
    for n in &ext.nodes {
        if n.label == "module.this [null-label]" {
            got = n.computed_name.as_str();
        }
    }
    assert_eq!(
        got, "eg-ue1-prod-app-public",
        "fixture ComputedName = {got:?}, want eg-ue1-prod-app-public"
    );
}
