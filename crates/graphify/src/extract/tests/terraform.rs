//! graphify-go `internal/extract/terraform_test.go`. The `nulllabel_test.go` tests are in
//! `nulllabel.rs`; the upstream cases are in `upstream_terraform.rs` and
//! `upstream_terraform_modules.rs`. A block's attributes are a `BTreeMap` here where Go has a
//! `map` that is nil when empty, so "no attributes" is an empty map.

use std::collections::{BTreeMap, BTreeSet};

use super::REDACTED_VALUE;
use crate::extract::tests::util::extract_bytes;
use crate::extract::{file, file_from_bytes, kinds, resolve};
use crate::model::{Edge, Node};
use crate::testutil::testdata;

/// Go `Resolve` of `results` with `files` (GOMAXPROCS workers).
fn resolve_files(results: &[crate::extract::Result], files: &[&str]) -> crate::model::Extraction {
    let files: Vec<String> = files.iter().map(|f| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(results, &files, workers)
}

/// The Go tests' `id2label` map.
fn id2label(nodes: &[Node]) -> BTreeMap<&str, &str> {
    nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// The Go tests' `has` closure: an edge of `rel` from the node labelled `src` to the one labelled
/// `tgt` (an id with no node reads as "", as a Go map miss does).
fn has(nodes: &[Node], edges: &[Edge], src: &str, rel: &str, tgt: &str) -> bool {
    let label = id2label(nodes);
    let of = |id: &str| label.get(id).copied().unwrap_or("");
    edges
        .iter()
        .any(|e| e.relation.as_str() == rel && of(&e.source) == src && of(&e.target) == tgt)
}

/// The Go tests' `byLabel`: each node's attributes by its label.
fn by_label(nodes: &[Node]) -> BTreeMap<&str, &BTreeMap<String, String>> {
    nodes
        .iter()
        .map(|n| (n.label.as_str(), &n.attributes))
        .collect()
}

/// Go's `m[k]` on a string map: "" when absent.
fn get<'a>(m: &'a BTreeMap<String, String>, k: &str) -> &'a str {
    m.get(k).map_or("", String::as_str)
}

// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_terraform() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("tf");
    let files = ["main.tf", "outputs.tf"];

    let results: Vec<_> = files
        .iter()
        .map(|f| file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let ext = resolve_files(&results, &files);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in [
        "aws_instance.web",
        "aws_vpc.main",
        "data.aws_ami.ubuntu",
        "var.region",
        "output.instance_id",
    ] {
        assert!(labels.contains(want), "missing node {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| has(&ext.nodes, &ext.edges, src, rel, tgt);
    assert!(
        has("aws_instance.web", "references", "var.region"),
        "expected aws_instance.web --references--> var.region"
    );
    assert!(
        has("aws_instance.web", "depends_on", "aws_vpc.main"),
        "expected aws_instance.web --depends_on--> aws_vpc.main"
    );
    // Cross-file resolution: outputs.tf references a resource defined in main.tf.
    assert!(
        has("output.instance_id", "references", "aws_instance.web"),
        "expected cross-file output.instance_id --references--> aws_instance.web"
    );
}

// Empty block bodies (e.g. `data "aws_region" "current" {}`) are pervasive in
// real Terraform. tree-sitter yields a nil body for them, which previously
// panicked when refsFrom walked it. The block node must still be extracted.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_terraform_empty_body() {
    let src = br#"data "aws_region" "current" {}
data "aws_caller_identity" "current" {}
resource "aws_x" "y" {}
module "m" {}
output "o" {}
"#;
    let res = extract_bytes("empty.tf", src);
    let labels: BTreeSet<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in [
        "data.aws_region.current",
        "data.aws_caller_identity.current",
        "aws_x.y",
        "module.m",
        "output.o",
    ] {
        assert!(
            labels.contains(want),
            "missing node {want:?} from empty-body blocks"
        );
    }
}

// Terraform addresses are directory-scoped. Two directories that merely share a
// base name (e.g. workspaces/scalr-agents and modules/scalr-agents) must NOT
// collapse their addresses into one node — that fabricates cross-directory
// references and drops real nodes. Scope is the full directory path, so the same
// resource address in different directories yields distinct nodes.
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_terraform_scope_by_full_path() {
    kinds::register_kinds();
    let src = br#"resource "aws_x" "r" {}"#;
    let files = ["a/dup/main.tf", "b/dup/main.tf"];
    let results = [
        file_from_bytes(files[0], src),
        file_from_bytes(files[1], src),
    ];
    let ext = resolve_files(&results, &files);

    let ids: Vec<&str> = ext
        .nodes
        .iter()
        .filter(|n| n.label == "aws_x.r")
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "expected 2 distinct aws_x.r nodes (one per directory), got {}: {ids:?}",
        ids.len()
    );
    assert_ne!(
        ids[0], ids[1],
        "same-basename directories collided into one node id {:?}",
        ids[0]
    );
}

// A module block's source must link the module call to what it instantiates: a
// local/relative source resolves to the target directory node (and that node
// gains contains edges to the directory's files); a registry/private-registry
// source becomes an external concept node. Without this every module node is an
// island with no edge to its implementation.
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_terraform_module_source() {
    kinds::register_kinds();
    let files = ["workspaces/ws/main.tf", "modules/proscia/main.tf"];
    let results = [
        file_from_bytes(
            files[0],
            br#"
module "p" {
  source = "../../modules/proscia"
}
module "vpc" {
  source  = "cloudposse/vpc/aws"
  version = "2.2.0"
}
"#,
        ),
        file_from_bytes(files[1], br#"resource "aws_x" "y" {}"#),
    ];
    let ext = resolve_files(&results, &files);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let has = |src: &str, rel: &str, tgt: &str| has(&ext.nodes, &ext.edges, src, rel, tgt);

    // Local source resolves to the target directory node.
    assert!(
        labels.contains("modules/proscia"),
        "missing directory node for resolved local module source modules/proscia"
    );
    assert!(
        has("module.p", "references", "modules/proscia"),
        "expected module.p --references--> modules/proscia (dir node)"
    );
    // The dir node is navigable into the module's files.
    assert!(
        has("modules/proscia", "contains", "main.tf"),
        "expected modules/proscia --contains--> main.tf"
    );
    // Registry source becomes an external concept node.
    assert!(
        has("module.vpc", "references", "cloudposse/vpc/aws"),
        "expected module.vpc --references--> cloudposse/vpc/aws (external node)"
    );
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_terraform_inherits_context() {
    let src = br#"
module "this" {
  source = "cloudposse/label/null"
}
module "label" {
  source  = "cloudposse/label/null"
  context = module.this.context
}
"#;
    let res = extract_bytes("main.tf", src);
    assert!(
        has(
            &res.nodes,
            &res.edges,
            "module.label [null-label]",
            "inherits_context",
            "module.this [null-label]"
        ),
        "expected module.label --inherits_context--> module.this"
    );
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_terraform_null_label_marker() {
    let src = br#"
module "this" {
  source  = "cloudposse/label/null"
  version = "0.25.0"
}
module "label" {
  source = "git::https://github.com/cloudposse/terraform-null-label.git?ref=tags/0.25.0"
}
module "plain" {
  source = "../vpc"
}
"#;
    let res = extract_bytes("main.tf", src);
    let labels: BTreeSet<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(
        labels.contains("module.this [null-label]"),
        "expected module.this tagged [null-label] (registry source)"
    );
    assert!(
        labels.contains("module.label [null-label]"),
        "expected module.label tagged [null-label] (git source)"
    );
    assert!(
        labels.contains("module.plain"),
        "expected module.plain to be untagged"
    );
}

// Block attributes are persisted on the node so "which resources run t3.large"
// is a graph query instead of a file read. Sensitive keys keep the key and
// redact the value: graph.json is committed in CI, so a hardcoded secret must
// never reach it.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_terraform_attributes() {
    let src = br#"resource "aws_instance" "web" {
  instance_type = "t3.large"
  count         = 2
  monitoring    = true
  ami           = var.ami_id
  vpc_security_group_ids = ["sg-1", "sg-2"]
  tags = {
    Name = "web"
  }
  db_password = "hunter2"
}

locals {
  x = "y"
}
"#;
    let res = extract_bytes("main.tf", src);
    let mut got: Option<&BTreeMap<String, String>> = None;
    for n in &res.nodes {
        if n.label == "aws_instance.web" {
            got = Some(&n.attributes);
        }
        if n.label == "local.x" && !n.attributes.is_empty() {
            panic!(
                "locals entries must not carry attributes, got {:?}",
                n.attributes
            );
        }
    }
    let got = got.expect("aws_instance.web has no attributes");
    assert!(!got.is_empty(), "aws_instance.web has no attributes");
    let want = [
        ("instance_type", "t3.large"),
        ("count", "2"),
        ("monitoring", "true"),
        ("ami", "var.ami_id"),
        ("vpc_security_group_ids", "sg-1, sg-2"),
        ("db_password", REDACTED_VALUE),
    ];
    for (k, v) in want {
        assert_eq!(get(got, k), v, "attribute {k:?}");
    }
    if let Some(v) = got.get("tags") {
        panic!("nested object attribute should be dropped, got tags={v:?}");
    }
}

// Redaction is keyed on the attribute NAME, so every common secret spelling is
// caught regardless of where it appears.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_terraform_attribute_redaction() {
    let src = br#"resource "x" "y" {
  db_password           = "p"
  aws_secret_access_key = "s"
  client_secret         = "c"
  api_key               = "k"
  connection_string     = "conn"
  ami                   = "ami-123"
}
"#;
    let res = extract_bytes("main.tf", src);
    let mut got = &BTreeMap::new();
    for n in &res.nodes {
        if n.label == "x.y" {
            got = &n.attributes;
        }
    }
    for k in [
        "db_password",
        "aws_secret_access_key",
        "client_secret",
        "api_key",
        "connection_string",
    ] {
        assert_eq!(get(got, k), REDACTED_VALUE, "attribute {k:?}");
    }
    assert_eq!(
        get(got, "ami"),
        "ami-123",
        "non-sensitive attribute was lost"
    );

    // The secret's name can live in the block LABEL, and Terraform's own
    // `sensitive = true` is the strongest author-declared signal; both must
    // redact the value-carrying attribute (`default` / `value`).
    let src = br#"variable "db_password" {
  type      = string
  sensitive = true
  default   = "hunter2"
}

output "conn" {
  sensitive = true
  value     = "postgres://admin:hunter2@db"
}

output "api_token" {
  value = "tok-abc"
}

resource "aws_instance" "web" {
  instance_type = "t3.large"
  user_data     = <<-EOT
    #!/bin/bash
    export AWS_SECRET=AKIAhunter2SECRET
  EOT
}
"#;
    let res = extract_bytes("main.tf", src);
    let by_label = by_label(&res.nodes);
    let empty = BTreeMap::new();
    let attrs = |label: &str| *by_label.get(label).unwrap_or(&&empty);
    for (label, key) in [
        ("var.db_password", "default"),
        ("output.conn", "value"),
        ("output.api_token", "value"),
    ] {
        assert_eq!(get(attrs(label), key), REDACTED_VALUE, "{label}.{key}");
    }
    // A sensitive block redacts every value, not just the obvious one.
    assert_eq!(
        get(attrs("var.db_password"), "type"),
        REDACTED_VALUE,
        "var.db_password type"
    );
    if let Some(v) = attrs("aws_instance.web").get("user_data") {
        panic!("bootstrap script must be dropped, got user_data={v:?}");
    }
    assert_eq!(
        get(attrs("aws_instance.web"), "instance_type"),
        "t3.large",
        "instance_type"
    );
}

// A comment between `=` and the value is a named child of the attribute, so the
// value must be found by skipping comments — otherwise the comment is stored as
// the value and `sensitive = true` stops redacting.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_extract_terraform_attribute_comment_before_value() {
    let src = br#"variable "thing" {
  sensitive = // yes
    true
  default = "hunter2"
}

resource "aws_instance" "web" {
  instance_type = /* c */ "t3.large"
}
"#;
    let res = extract_bytes("main.tf", src);
    let by_label = by_label(&res.nodes);
    let empty = BTreeMap::new();
    let attrs = |label: &str| *by_label.get(label).unwrap_or(&&empty);
    assert_eq!(
        get(attrs("var.thing"), "default"),
        REDACTED_VALUE,
        "var.thing default"
    );
    assert_eq!(
        get(attrs("aws_instance.web"), "instance_type"),
        "t3.large",
        "instance_type"
    );
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_fallback_values_redacted() {
    let src = br#"resource "aws_ecs_task_definition" "t" {
  container_definitions = jsonencode([{ name = "x", environment = [{ name = "API_TOKEN", value = "hunter2json" }] }])
  env                   = ["DB_PASSWORD=hunter2inline", "plain"]
  instance_type         = "t3.large"
  ami                   = data.aws_ami.x.id
}
output "conn" {
  value = "postgres://u:hunter2out@h/db"
}
output "redis" {
  value = "redis://:hunter2nouser@h:6379"
}
output "slash" {
  value = "postgres://u:hunter2/slash@h/db"
}
resource "x_thing" "s" {
  env_single = "DB_PASSWORD=hunter2single"
  conn       = "Server=db;User=sa;Password=hunter2semi"
  hdr        = "Authorization: Bearer hunter2hdr"
  bare       = "Bearer hunter2bare"
  plain      = "db-secret-name"
}
"#;
    let res = extract_bytes("main.tf", src);
    for n in &res.nodes {
        for (k, v) in &n.attributes {
            assert!(
                !v.contains("hunter2"),
                "{}.{k} leaks secret: {v:?}",
                n.label
            );
        }
        if n.label == "x_thing.s" {
            assert_eq!(
                get(&n.attributes, "plain"),
                "db-secret-name",
                "plain: want it kept"
            );
        }
        if n.label == "aws_ecs_task_definition.t" {
            assert!(
                get(&n.attributes, "instance_type") == "t3.large"
                    && get(&n.attributes, "ami") == "data.aws_ami.x.id",
                "benign attrs changed: {:?}",
                n.attributes
            );
            assert_eq!(
                get(&n.attributes, "container_definitions"),
                REDACTED_VALUE,
                "container_definitions"
            );
        }
    }
}
