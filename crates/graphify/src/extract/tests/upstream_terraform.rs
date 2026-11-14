//! The cases of the upstream graphify v0.9.107 `tests/test_terraform.py` whose assertion holds for
//! graphify-go v0.12.0 (see the `extract-hcl` block of `parity/README.md`). The sources are
//! written inline, and upstream's `_write` + `extract_terraform(path)` is Go's `FileFromBytes` of
//! the same name. Go keeps a block's attributes as a flat string map: every value is its source
//! text (a list of strings is joined with ", "), an object, and a list or tuple of objects, is
//! dropped, and a provider block gets no `references` edge. Upstream's typed values, nested maps
//! and `provider.aws.<alias>` nodes are the P3 items named at each test.

use std::collections::{BTreeMap, BTreeSet};

use super::REDACTED_VALUE;
use crate::extract::Result as Extracted;
use crate::extract::resolve;
use crate::extract::tests::util::extract_bytes;
use crate::model::{Node, Relation};

/// Upstream `_write` + `extract_terraform`: `src` extracted as the file `rel`.
fn extract(rel: &str, src: &str) -> Extracted {
    extract_bytes(rel, src.as_bytes())
}

/// Upstream `_labels`.
fn labels(r: &Extracted) -> BTreeSet<&str> {
    r.nodes.iter().map(|n| n.label.as_str()).collect()
}

/// Upstream `_rel_pairs`: (source label, target label) of every edge of `relation`; an id with no
/// node stays the id (`lab.get(id, id)`).
fn rel_pairs(r: &Extracted, relation: &str) -> BTreeSet<(String, String)> {
    let lab: BTreeMap<&str, &str> = r
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let of = |id: &str| lab.get(id).copied().unwrap_or(id).to_string();
    r.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .map(|e| (of(&e.source), of(&e.target)))
        .collect()
}

fn has_pair(pairs: &BTreeSet<(String, String)>, src: &str, tgt: &str) -> bool {
    pairs.contains(&(src.to_string(), tgt.to_string()))
}

/// Upstream `next(n for n in r["nodes"] if n["label"] == label)`.
fn node<'a>(r: &'a Extracted, label: &str) -> &'a Node {
    r.nodes
        .iter()
        .find(|n| n.label == label)
        .unwrap_or_else(|| panic!("no node {label:?}"))
}

/// Go's `m[k]` on a string map: "" when absent.
fn get<'a>(m: &'a BTreeMap<String, String>, k: &str) -> &'a str {
    m.get(k).map_or("", String::as_str)
}

/// Upstream's `secret not in json.dumps(node)`: no node holds any of `secrets`.
fn assert_no_secret(r: &Extracted, secrets: &[&str]) {
    for n in &r.nodes {
        let dumped = format!("{n:?}");
        for s in secrets {
            assert!(!dumped.contains(s), "secret {s:?} leaked into {dumped}");
        }
    }
}

const SAMPLE: &str = r#"# leading comment so the body is not children[0]
terraform {
  required_providers { azurerm = { source = "hashicorp/azurerm" } }
}

variable "region" { default = "us-east-1" }

provider "aws" { region = var.region }

data "aws_ami" "ubuntu" { most_recent = true }

resource "aws_instance" "web" {
  ami       = data.aws_ami.ubuntu.id
  subnet_id = var.region
  depends_on = [aws_security_group.sg]
}

resource "aws_security_group" "sg" { name = "sg" }

module "vpc" {
  source = "./modules/vpc"
  cidr   = local.cidr
}

locals { cidr = "10.0.0.0/16" }

output "ip" { value = aws_instance.web.private_ip }
"#;

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_no_error_and_all_block_types_become_nodes() {
    let r = extract("main.tf", SAMPLE);
    assert!(!r.over_budget, "parse over budget");
    let labels = labels(&r);
    // one node per block type (the terraform{} settings block is intentionally skipped)
    for expected in [
        "var.region",
        "provider.aws",
        "data.aws_ami.ubuntu",
        "aws_instance.web",
        "aws_security_group.sg",
        "module.vpc",
        "local.cidr",
        "output.ip",
    ] {
        assert!(labels.contains(expected), "missing node {expected:?}");
    }
}

// Without upstream's `("provider.aws", "var.region")` pair: Go emits no `references` edge from a
// provider block (`terraform.go:152` gives the provider a node and does not walk its body).
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_reference_edges() {
    let r = extract("main.tf", SAMPLE);
    let refs = rel_pairs(&r, "references");
    assert!(has_pair(&refs, "aws_instance.web", "data.aws_ami.ubuntu"));
    assert!(has_pair(&refs, "aws_instance.web", "var.region"));
    assert!(has_pair(&refs, "module.vpc", "local.cidr"));
    assert!(has_pair(&refs, "output.ip", "aws_instance.web"));
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_depends_on_edge() {
    let r = extract("main.tf", SAMPLE);
    assert!(has_pair(
        &rel_pairs(&r, "depends_on"),
        "aws_instance.web",
        "aws_security_group.sg"
    ));
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_file_contains_blocks() {
    let r = extract("main.tf", SAMPLE);
    let contains = rel_pairs(&r, "contains");
    assert!(has_pair(&contains, "main.tf", "aws_instance.web"));
    assert!(has_pair(&contains, "main.tf", "var.region"));
}

// count.index / each.key / self.* / path.module are builtins, not references.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_meta_heads_not_emitted() {
    let body = r#"resource "aws_instance" "web" {
  count = 2
  name  = "web-${count.index}"
  tags  = each.value
  dir   = path.module
}
"#;
    let r = extract("main.tf", body);
    let targets: BTreeSet<String> = rel_pairs(&r, "references")
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert!(
        !targets
            .iter()
            .any(|t| t.starts_with("count") || t.starts_with("each") || t.starts_with("path")),
        "meta head emitted as a reference: {targets:?}"
    );
}

// A resource defined in one file is referenced from another in the same
// directory; directory-scoped IDs must let the edge resolve at build time. Go has no
// `build_from_json` here: the merge is `Resolve`, which joins the per-file nodes and edges.
// hcl test (ihc.31): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_cross_file_references_resolve_after_merge() {
    let defn = "resource \"azurerm_resource_group\" \"main\" { name = \"rg\" }\n";
    let user = r#"resource "azurerm_network_interface" "nic" {
  resource_group_name = azurerm_resource_group.main.name
}
"#;
    let r_defn = extract("main.tf", defn);
    let r_user = extract("nic.tf", user);

    // The cross-file edge target id equals the definition's node id.
    let rg_id = node(&r_defn, "azurerm_resource_group.main").id.clone();
    let nic_ref_targets: BTreeSet<&str> = r_user
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "references")
        .map(|e| e.target.as_str())
        .collect();
    assert!(nic_ref_targets.contains(rg_id.as_str()));

    // And it survives the merge: the edge is present.
    let nic_id = node(&r_user, "azurerm_network_interface.nic").id.clone();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(
        &[r_defn, r_user],
        &["main.tf".to_string(), "nic.tf".to_string()],
        workers,
    );
    assert!(
        ext.edges
            .iter()
            .any(|e| e.relation == Relation::REFERENCES && e.source == nic_id && e.target == rg_id),
        "the cross-file edge was dropped"
    );
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_empty_and_commentonly_files_are_safe() {
    let r = extract("a.tf", "");
    assert!(!r.over_budget);
    let r = extract("b.tf", "# just a comment\n");
    // only the file node, no crash
    assert_eq!(r.nodes.len(), 1);
}

// .tfvars files contain only key=value assignments (no block structure),
// so extractTerraform produces zero block nodes — only the file node.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_tfvars_key_value_is_safe() {
    let r = extract(
        "terraform.tfvars",
        "region = \"us-east-1\"\nenv = \"prod\"\n",
    );
    assert!(!r.over_budget);
    assert_eq!(r.nodes.len(), 1); // only the file node, no variable nodes
}

// pins Go v0.12.0 (each value is its source text: "true", "false", "42", "3.14", "null");
// upstream 0.9.107 emits True, False, 42, 3.14 and None -> P3
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_attributes_primitive_literals() {
    let body = r#"resource "aws_s3_bucket" "example" {
  bucket        = "my-test-bucket"
  force_destroy = true
  versioning    = false
  max_size      = 42
  ratio         = 3.14
  logging       = null
}
"#;
    let r = extract("main.tf", body);
    let attrs = &node(&r, "aws_s3_bucket.example").attributes;
    assert!(!attrs.is_empty());
    for (k, v) in [
        ("bucket", "my-test-bucket"),
        ("force_destroy", "true"),
        ("versioning", "false"),
        ("max_size", "42"),
        ("ratio", "3.14"),
        ("logging", "null"),
    ] {
        assert_eq!(get(attrs, k), v, "attribute {k:?}");
    }
}

// pins Go v0.12.0 ("10.0.0.0/16, 192.168.1.0/24", and `tags` is dropped: Go keeps no object
// attribute); upstream 0.9.107 emits a list and the map -> P3
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_attributes_collections() {
    let body = r#"resource "aws_security_group" "sg" {
  name          = "sg"
  tags          = { Environment = "production", ManagedBy = "terraform" }
  ingress_cidrs = ["10.0.0.0/16", "192.168.1.0/24"]
}
"#;
    let r = extract("main.tf", body);
    let attrs = &node(&r, "aws_security_group.sg").attributes;
    assert!(!attrs.is_empty());
    assert_eq!(get(attrs, "name"), "sg");
    assert!(
        !attrs.contains_key("tags"),
        "object attribute should be dropped, got {:?}",
        attrs.get("tags")
    );
    assert_eq!(get(attrs, "ingress_cidrs"), "10.0.0.0/16, 192.168.1.0/24");
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_attributes_expressions_and_references() {
    let body = r#"variable "bucket_name" { default = "my-bucket" }
data "aws_ami" "ubuntu" { most_recent = true }

resource "aws_s3_bucket" "example" {
  bucket = var.bucket_name
  ami    = data.aws_ami.ubuntu.id
}
"#;
    let r = extract("main.tf", body);
    let attrs = &node(&r, "aws_s3_bucket.example").attributes;
    assert!(!attrs.is_empty());
    assert_eq!(get(attrs, "bucket"), "var.bucket_name");
    assert_eq!(get(attrs, "ami"), "data.aws_ami.ubuntu.id");

    // Verify existing reference edges remain intact
    let refs = rel_pairs(&r, "references");
    assert!(has_pair(&refs, "aws_s3_bucket.example", "var.bucket_name"));
    assert!(has_pair(
        &refs,
        "aws_s3_bucket.example",
        "data.aws_ami.ubuntu"
    ));
}

// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_multiple_attributes_and_nested_block_isolation() {
    let body = r#"resource "aws_instance" "web" {
  ami           = "ami-12345"
  instance_type = "t3.micro"
  lifecycle {
    prevent_destroy = true
  }
}
"#;
    let r = extract("main.tf", body);
    let attrs = &node(&r, "aws_instance.web").attributes;
    assert!(!attrs.is_empty());
    assert_eq!(get(attrs, "ami"), "ami-12345");
    assert_eq!(get(attrs, "instance_type"), "t3.micro");
    // Ownership rule: only direct attributes of the block are captured.
    // Nested blocks (e.g. lifecycle) must NOT be flattened into parent attributes.
    assert!(!attrs.contains_key("prevent_destroy"));
    assert!(!attrs.contains_key("lifecycle"));
}

// pins Go v0.12.0 (`tags`, an object, is dropped and so is nothing redacted inside it);
// upstream 0.9.107 emits the map with `client_secret` redacted -> P3
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_sensitive_attribute_values_are_redacted() {
    let body = r#"resource "aws_db_instance" "main" {
  identifier    = "prod-db"
  instance_class = "db.t3.medium"
  password      = "hunter2-super-secret"
  db_password   = "another-secret"
  aws_secret_access_key = "AKIAWHATEVER"
  tags = {
    Name         = "prod"
    client_secret = "leaky"
  }
}
"#;
    let r = extract("db.tf", body);
    let attrs = &node(&r, "aws_db_instance.main").attributes;
    // non-secret keys pass through unchanged
    assert_eq!(get(attrs, "identifier"), "prod-db");
    assert_eq!(get(attrs, "instance_class"), "db.t3.medium");
    // secret-named keys keep their key but redact the value
    assert_eq!(get(attrs, "password"), REDACTED_VALUE);
    assert_eq!(get(attrs, "db_password"), REDACTED_VALUE);
    assert_eq!(get(attrs, "aws_secret_access_key"), REDACTED_VALUE);
    assert!(!attrs.contains_key("tags"));
    // the literal secret must appear nowhere in the serialized node
    assert_no_secret(&r, &["hunter2-super-secret", "AKIAWHATEVER", "leaky"]);
}

// pins Go v0.12.0 (`connections`, a list of objects, is dropped); upstream 0.9.107 emits the list
// with each secret redacted -> P3
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_secret_in_list_of_objects_is_redacted() {
    let body = r#"resource "aws_x" "y" {
  name = "app"
  connections = [
    { host = "db1", password = "leaky-in-list" },
    { host = "db2", token = "tok-in-list" },
  ]
}
"#;
    let r = extract("conns.tf", body);
    let attrs = &node(&r, "aws_x.y").attributes;
    assert_eq!(get(attrs, "name"), "app");
    assert!(!attrs.contains_key("connections"));
    assert_no_secret(&r, &["leaky-in-list", "tok-in-list"]);
}

// pins Go v0.12.0 ("[redacted]" for var.db_password's `type`: a block whose label reads as a
// secret redacts every value, not just the obvious one); upstream 0.9.107 emits "string" -> P3
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_secret_named_variable_default_and_output_value_are_redacted() {
    let body = r#"variable "db_password" {
  type    = string
  default = "hunter2-default"
}

variable "region" {
  type    = string
  default = "us-east-1"
}

variable "signing_material" {
  type      = string
  sensitive = true
  default   = "marked-sensitive"
}

output "admin_token" {
  value = "tok-in-output"
}

output "bucket_name" {
  value = "my-bucket"
}
"#;
    let r = extract("vars.tf", body);
    let by_label: BTreeMap<&str, &BTreeMap<String, String>> = r
        .nodes
        .iter()
        .map(|n| (n.label.as_str(), &n.attributes))
        .collect();
    // secret-named blocks: the literal is redacted
    assert_eq!(get(by_label["var.db_password"], "default"), REDACTED_VALUE);
    assert_eq!(get(by_label["var.db_password"], "type"), REDACTED_VALUE);
    assert_eq!(get(by_label["output.admin_token"], "value"), REDACTED_VALUE);
    // Terraform's explicit `sensitive = true` marker is honoured
    assert_eq!(
        get(by_label["var.signing_material"], "default"),
        REDACTED_VALUE
    );
    // ordinary variables/outputs are untouched
    assert_eq!(get(by_label["var.region"], "default"), "us-east-1");
    assert_eq!(get(by_label["output.bucket_name"], "value"), "my-bucket");
    assert_no_secret(
        &r,
        &["hunter2-default", "tok-in-output", "marked-sensitive"],
    );
}

// pins Go v0.12.0 (`environment`, `env_refs` and `single`, lists of objects and an object, are
// dropped); upstream 0.9.107 emits them with the secret values redacted -> P3
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_terraform_name_value_pair_secret_is_redacted() {
    let body = r#"resource "aws_ecs_task_definition" "app" {
  family = "app"
  environment = [
    { name = "DB_PASSWORD", value = "hunter2" },
    { name = "LOG_LEVEL", value = "debug" },
  ]
  env_refs = [{ name = "API_KEY", valueFrom = "arn:aws:ssm:us-east-1:1:parameter/api" }]
  single = { name = "client_secret", value = "in-a-map" }
}
"#;
    let r = extract("ecs.tf", body);
    let attrs = &node(&r, "aws_ecs_task_definition.app").attributes;
    assert_eq!(get(attrs, "family"), "app");
    for k in ["environment", "env_refs", "single"] {
        assert!(!attrs.contains_key(k), "{k:?} should be dropped");
    }
    assert_no_secret(&r, &["hunter2", "in-a-map", "parameter/api"]);
}

// Two `provider "aws" {...}` blocks distinguished only by `alias` collapse onto one node, and the
// second block's attributes replace the first's.
// pins Go v0.12.0 (one node `provider.aws`, holding the last block's attributes); upstream
// 0.9.107 emits `provider.aws.west` and `provider.aws.east`, each with its own region -> P3 B07
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_aliased_providers_of_the_same_type_get_distinct_nodes() {
    let body = r#"provider "aws" {
  alias  = "west"
  region = "us-west-1"
}

provider "aws" {
  alias  = "east"
  region = "us-east-1"
}
"#;
    let r = extract("main.tf", body);
    let labels = labels(&r);
    assert!(labels.contains("provider.aws"));
    assert!(!labels.contains("provider.aws.west"));
    assert!(!labels.contains("provider.aws.east"));
    let providers: Vec<&Node> = r
        .nodes
        .iter()
        .filter(|n| n.label == "provider.aws")
        .collect();
    assert_eq!(providers.len(), 1);
    assert_eq!(get(&providers[0].attributes, "region"), "us-east-1");
}

// The common case, a single provider block for a type with no `alias`.
// Without upstream's `("provider.aws", "var.region")` pair: Go emits no `references` edge from a
// provider block.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_unaliased_provider_keeps_its_plain_id() {
    let body = "variable \"region\" { default = \"us-east-1\" }\nprovider \"aws\" { region = var.region }\n";
    let r = extract("main.tf", body);
    let labels = labels(&r);
    assert!(labels.contains("provider.aws"));
    assert!(!labels.contains("provider.aws.None"));
}

// `provider = aws.west` in a resource reads as a resource reference `aws.west`.
// pins Go v0.12.0 (a `references` edge from aws_instance.web to the id of `aws.west`, a node that
// is never minted, and none to `provider.aws`); upstream 0.9.107 emits an edge to
// `provider.aws.west` and no dangling target -> P3 B07
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_provider_reference_resolves_to_its_aliased_block() {
    let body = r#"provider "aws" {
  alias  = "west"
  region = "us-west-1"
}

resource "aws_instance" "web" {
  provider = aws.west
}
"#;
    let r = extract("main.tf", body);
    let refs = rel_pairs(&r, "references");
    assert!(!has_pair(&refs, "aws_instance.web", "provider.aws"));
    assert!(!has_pair(&refs, "aws_instance.web", "provider.aws.west"));
    let ids: BTreeSet<&str> = r.nodes.iter().map(|n| n.id.as_str()).collect();
    let targets: BTreeSet<&str> = r
        .edges
        .iter()
        .filter(|e| e.relation == Relation::REFERENCES)
        .map(|e| e.target.as_str())
        .collect();
    assert_eq!(targets.len(), 1, "targets: {targets:?}");
    assert!(
        !targets.is_subset(&ids),
        "the target of `provider = aws.west` should be a dangling id"
    );
}

// A local variable that happens to be NAMED `provider` is a local definition, not a provider
// assignment; its own value resolves through the normal reference rules.
// hcl test (ihc.31): file runs the MCP-config check
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_a_local_named_provider_is_not_misread_as_a_provider_reference() {
    let body = r#"locals {
  provider = data.aws_caller_identity.current.account_id
}

data "aws_caller_identity" "current" {}
"#;
    let r = extract("main.tf", body);
    let refs = rel_pairs(&r, "references");
    assert!(has_pair(
        &refs,
        "local.provider",
        "data.aws_caller_identity.current"
    ));
}
