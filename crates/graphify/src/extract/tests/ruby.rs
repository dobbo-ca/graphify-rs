//! graphify-go `internal/extract/ruby_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_ruby_bare_self_send_calls.py` and `test_ruby_resolution.py` whose assertion holds for
//! graphify-go v0.12.0 (see the `extract-ruby` block of `parity/README.md`). The upstream sources
//! are written inline, so no fixture is copied. Upstream names the nodes of a method `.name()`;
//! Go labels them `Type.name()`, and an upstream `method` edge is Go's `contains` (B28). Go reads
//! only top-level `def`, `class` and `module` (not `def self.x`, `class << self`, a class nested
//! in a class or module, or `X = Struct.new`), types no receiver, and emits no `mixes_in` edge.

use std::collections::{BTreeMap, BTreeSet};

use super::extract_ruby;
use crate::extract::{Result as Extracted, file, file_from_bytes, kinds, resolve};
use crate::model::{Confidence, Edge, Extraction};
use crate::testutil::testdata;

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_ruby() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("rubyproj");
    let files = ["util/math.rb", "web/server.rb"];

    let results: Vec<_> = files
        .iter()
        .map(|f| file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.rb",
        "server.rb",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
        "json",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };
    // Method scoped under its class, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "boot()"),
        "expected Server.start --calls--> boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );
    // require "json" becomes an external import edge from the file node.
    assert!(
        has("server.rb", "imports", "json"),
        "expected server.rb --imports--> json"
    );
}

// ---- upstream graphify v0.9.107 `test_ruby_*.py` ----

/// Go `FileFromBytes` on each `(rel, src)`, then `Resolve` (GOMAXPROCS workers).
fn run(srcs: &[(&str, &str)]) -> Extraction {
    kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

/// Go `extractRuby` on one file, without `Resolve`.
fn extract_one(rel: &str, src: &str) -> Extracted {
    kinds::register_kinds();
    extract_ruby(rel, src.as_bytes()).expect("extract_ruby: over budget")
}

fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// (source label, target label) of every edge of `relation`; a missing node is "".
fn pairs(ext: &Extraction, relation: &str) -> BTreeSet<(String, String)> {
    let label = id2label(ext);
    let of = |id: &str| label.get(id).copied().unwrap_or("").to_string();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .map(|e| (of(&e.source), of(&e.target)))
        .collect()
}

fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    pairs(ext, relation).contains(&(src.to_string(), tgt.to_string()))
}

/// The edges of `relation` from the node labelled `src` to the one labelled `tgt`.
fn edges_of<'a>(ext: &'a Extraction, src: &str, relation: &str, tgt: &str) -> Vec<&'a Edge> {
    let label = id2label(ext);
    ext.edges
        .iter()
        .filter(|e| {
            e.relation.as_str() == relation
                && label.get(e.source.as_str()) == Some(&src)
                && label.get(e.target.as_str()) == Some(&tgt)
        })
        .collect()
}

/// Whether any `calls` edge starts at the node labelled `src`.
fn calls_from(ext: &Extraction, src: &str) -> bool {
    pairs(ext, "calls").iter().any(|(s, _)| s == src)
}

fn label_set(ext: &Extraction) -> BTreeSet<&str> {
    ext.nodes.iter().map(|n| n.label.as_str()).collect()
}

/// The labels of the nodes of the members of `class` (`Class.name()`).
fn members(labels: &BTreeSet<&str>, class: &str) -> BTreeSet<String> {
    let prefix = format!("{class}.");
    labels
        .iter()
        .filter(|l| l.starts_with(&prefix))
        .map(|l| l.to_string())
        .collect()
}

/// The one `calls` edge from `Child.call()` to `<owner>.helper()`, INFERRED (Go never marks a
/// name-bound call EXTRACTED).
fn assert_one_inferred_call(ext: &Extraction, owner: &str) {
    let target = format!("{owner}.helper()");
    let edges = edges_of(ext, "Child.call()", "calls", &target);
    assert_eq!(edges.len(), 1, "{:?}", pairs(ext, "calls"));
    assert_eq!(edges[0].confidence, Confidence::INFERRED);
}

// test_ruby_bare_self_send_calls.py: a bare identifier statement that names a method of the file
// and no local is a send on `self` (a class-method send binds to its own type).

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_self_send_in_a_class_is_a_call() {
    let ext = run(&[(
        "w.rb",
        "class Widget\n  def run\n    build\n  end\n  def build\n    1\n  end\nend\n",
    )]);
    assert!(
        has_edge(&ext, "Widget.run()", "calls", "Widget.build()"),
        "{:?}",
        pairs(&ext, "calls")
    );
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_bare_call_between_module_methods_is_a_call() {
    // The whole module layer used to be a set of leaves with no internal edges.
    let ext = run(&[(
        "g.rb",
        "module Greetable\n  def greet\n    build_message\n  end\n  def build_message\n    \"hi\"\n  end\nend\n",
    )]);
    assert!(
        has_edge(
            &ext,
            "Greetable.greet()",
            "calls",
            "Greetable.build_message()"
        ),
        "{:?}",
        pairs(&ext, "calls")
    );
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_a_local_variable_read_is_not_a_call() {
    // `build` is assigned first, so the later bare `build` is a variable read,
    // not a send — even though a method named `build` exists. No edge.
    let ext = run(&[(
        "v.rb",
        "class W\n  def run\n    build = 1\n    build\n  end\n  def build\n    2\n  end\nend\n",
    )]);
    assert!(!has_edge(&ext, "W.run()", "calls", "W.build()"));
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_a_parameter_read_is_not_a_call() {
    // A parameter named like a method shadows it; the bare read is the parameter.
    let ext = run(&[(
        "p.rb",
        "class W\n  def run(build)\n    build\n  end\n  def build\n    1\n  end\nend\n",
    )]);
    assert!(!has_edge(&ext, "W.run()", "calls", "W.build()"));
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_a_block_parameter_read_is_not_a_call() {
    // `item` is a block parameter inside the method, not a send.
    let ext = run(&[(
        "b.rb",
        "class W\n  def run\n    [1].each do |item|\n      item\n    end\n  end\n  def item\n  end\nend\n",
    )]);
    assert!(!has_edge(&ext, "W.run()", "calls", "W.item()"));
}

// pins Go v0.12.0 (Resolve keeps one calls edge per call site: two); upstream 0.9.107 emits one -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_self_send_and_bare_send_do_not_duplicate_an_edge() {
    let ext = run(&[(
        "s.rb",
        "class W\n  def run\n    build\n    self.build\n  end\n  def build\n  end\nend\n",
    )]);
    assert_eq!(edges_of(&ext, "W.run()", "calls", "W.build()").len(), 2);
}

// test_ruby_resolution.py. Go has no receiver typing (`p = Processor.new; p.run`): a member call
// binds only to a method qualified by the receiver's last name, so the "stays unresolved" tests
// hold for that reason.

const HELPER_RB: &str = "def transform(data)\n  data.upcase\nend\n\n\
    class Processor\n  def run(items)\n    items.map { |i| transform(i) }\n  end\nend\n";

const PARENT_RB: &str = "class Parent\n  def run; :parent; end\nend\n";
const CHILD_RB: &str = "class Child < Parent\nend\n";
const MAIN_TYPED_RB: &str = "def process_all\n  worker = Child.new\n  worker.run\nend\n";

// pins Go v0.12.0 (holds because Go types no receiver: `worker.run` binds to nothing; the method is `Parent.run()`, a contains edge); upstream 0.9.107 emits `method` and withholds the edge only for an unsafe file -> P3 B28
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_typed_member_call_stays_unresolved_when_any_ruby_file_is_unsafe() {
    let ext = run(&[
        ("child.rb", CHILD_RB),
        ("main.rb", MAIN_TYPED_RB),
        ("parent.rb", PARENT_RB),
        (
            "refinement.rb",
            "module R\n  refine Parent do\n    def other; :refined; end\n  end\nend\n",
        ),
    ]);
    assert!(has_edge(&ext, "Parent", "contains", "Parent.run()"));
    assert!(!has_edge(&ext, "process_all()", "calls", "Parent.run()"));
}

// pins Go v0.12.0 (holds because Go types no receiver: `worker.run` binds to nothing; the method is `Parent.run()`, a contains edge); upstream 0.9.107 emits `method` and withholds the edge only for an external owner alias -> P3 B28
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_typed_member_call_stays_unresolved_with_external_owner_alias() {
    let ext = run(&[
        ("child.rb", CHILD_RB),
        ("main.rb", MAIN_TYPED_RB),
        ("parent.rb", PARENT_RB),
        (
            "patch.rb",
            "AliasChild ||= Child\nAliasChild.class_eval { attr_reader :run }\n",
        ),
    ]);
    assert!(has_edge(&ext, "Parent", "contains", "Parent.run()"));
    assert!(
        !pairs(&ext, "calls")
            .iter()
            .any(|(_, t)| t == "Parent.run()")
    );
}

// pins Go v0.12.0 (holds because Go types no receiver); upstream 0.9.107 refuses a receiver of unknown type and resolves a known one -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_no_false_positive_when_type_unknown() {
    // A member call on a receiver with no known type must NOT be resolved.
    let ext = run(&[
        (
            "main.rb",
            "require_relative \"helper\"\n\ndef process_all(thing)\n  thing.run(1)\nend\n",
        ),
        ("helper.rb", HELPER_RB),
    ]);
    // `thing` is a parameter of unknown type => no precise target => no edge.
    assert!(
        !pairs(&ext, "calls")
            .iter()
            .any(|(s, t)| s.contains("process_all") && t.contains("run")),
        "{:?}",
        pairs(&ext, "calls")
    );
}

#[test]
#[ignore = "graphify-rs-ihc.33"]
fn test_plain_module_gets_a_node_with_methods() {
    // #1640 shape 1: `module Foo` must get a node and own its methods.
    let r = extract_one(
        "tax.rb",
        "module TaxCalculator\n  module_function\n  def rate_for(order)\n    0.2\n  end\nend\n",
    );
    let labels: BTreeSet<&str> = r.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains("TaxCalculator"));
    // The method attaches to the module (a contains edge), not the file.
    let id2label: BTreeMap<&str, &str> = r
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    assert!(r.edges.iter().any(|e| {
        e.relation.as_str() == "contains"
            && id2label.get(e.source.as_str()) == Some(&"TaxCalculator")
            && id2label.get(e.target.as_str()) == Some(&"TaxCalculator.rate_for()")
    }));
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_constant_receiver_module_function_call_resolves() {
    // #1634 + #1640: `TaxCalculator.rate_for` resolves across files to a module_function.
    let ext = run(&[
        (
            "pp.rb",
            "class PaymentProcessor\n  def process(order)\n    TaxCalculator.rate_for(order)\n  end\nend\n",
        ),
        (
            "tax.rb",
            "module TaxCalculator\n  module_function\n  def rate_for(o)\n    0.2\n  end\nend\n",
        ),
    ]);
    assert!(
        has_edge(
            &ext,
            "PaymentProcessor.process()",
            "calls",
            "TaxCalculator.rate_for()"
        ),
        "{:?}",
        pairs(&ext, "calls")
    );
}

// pins Go v0.12.0 (holds because Go extracts no `def self.call` and no class nested in a module); upstream 0.9.107 refuses an ambiguous receiver -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_ambiguous_constant_receiver_emits_no_edge() {
    let ext = run(&[
        (
            "c.rb",
            "class Runner\n  def run\n    Processor.call\n  end\nend\n",
        ),
        (
            "a.rb",
            "module A\n  class Processor\n    def self.call; end\n  end\nend\n",
        ),
        (
            "b.rb",
            "module B\n  class Processor\n    def self.call; end\n  end\nend\n",
        ),
    ]);
    assert!(
        !calls_from(&ext, "Runner.run()"),
        "{:?}",
        pairs(&ext, "calls")
    );
}

// pins Go v0.12.0 (holds because Go emits no mixes_in edge); upstream 0.9.107 emits none for `extend self` either -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extend_self_and_nonconstant_args_emit_no_mixin() {
    // `extend self` and `include some_var` are not constant module references.
    let ext = run(&[("m.rb", "module M\n  extend self\n  def go; end\nend\n")]);
    let mix = pairs(&ext, "mixes_in");
    assert!(!mix.iter().any(|(_, t)| t == "self"));
    assert!(mix.is_empty(), "{mix:?}");
}

// pins Go v0.12.0 (holds because Go emits no mixes_in edge); upstream 0.9.107 emits none for an undefined module either -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_include_of_undefined_or_ambiguous_module_emits_no_edge() {
    // Undefined module (no node) -> no edge.
    let ext = run(&[("x.rb", "class X\n  include NotDefinedAnywhere\nend\n")]);
    assert!(
        !pairs(&ext, "mixes_in")
            .iter()
            .any(|(_, t)| t == "NotDefinedAnywhere")
    );
}

// pins Go v0.12.0 (holds because Go emits no mixes_in edge); upstream 0.9.107 refuses `extend ActiveSupport::Concern` for a local `Concern` -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_qualified_external_mixin_does_not_bind_to_local() {
    let ext = run(&[
        (
            "concern.rb",
            "module Concern\n  def local_thing; end\nend\n",
        ),
        (
            "post.rb",
            "class Post\n  extend ActiveSupport::Concern\nend\n",
        ),
    ]);
    let mix = pairs(&ext, "mixes_in");
    assert!(!mix.contains(&("Post".to_string(), "Concern".to_string())));
    assert!(mix.is_empty(), "{mix:?}");
}

// pins Go v0.12.0 (one node `Thing.foo()`: `foo`, `foo!`, `foo?` and `foo=` make one id, the first declared names it); upstream 0.9.107 emits four nodes -> P3 B01
#[test]
#[ignore = "graphify-rs-ihc.33"]
fn test_ruby_suffixed_methods_extraction_and_labels() {
    let r = extract_one(
        "thing.rb",
        "class Thing\n  def foo; end\n  def foo!; end\n  def foo?; end\n  def foo=(val); end\nend\n",
    );
    let labels: BTreeSet<&str> = r.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(
        members(&labels, "Thing"),
        BTreeSet::from(["Thing.foo()".to_string()])
    );
}

// The ids (`_parse_bang`, `_valid_pred`) of the upstream test are node-ID differences, by
// design: only the labels are asserted.
#[test]
#[ignore = "graphify-rs-ihc.33"]
fn test_ruby_suffixed_toplevel_functions_extraction() {
    let r = extract_one("utils.rb", "def parse!; end\ndef valid?; end\n");
    let labels: BTreeSet<&str> = r.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains("parse!()"), "{labels:?}");
    assert!(labels.contains("valid?()"), "{labels:?}");
}

// pins Go v0.12.0 (holds because Go has no nested class and no `def self.call`: no `Thing::Base` node); upstream 0.9.107 refuses a framework-qualified receiver for a local `Base` -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_framework_qualified_receiver_does_not_bind_same_named_local_class() {
    let ext = run(&[
        (
            "other.rb",
            "class Other\n  def framework_ar\n    ActiveRecord::Base.transaction { save! }\n  end\n\n  def framework_aj\n    ActiveJob::Base.default_queue_name\n  end\nend\n",
        ),
        (
            "thing.rb",
            "class Thing\n  class Base\n    def self.call(x) = x\n  end\nend\n",
        ),
    ]);
    assert!(!label_set(&ext).contains("Thing::Base"));
    assert!(!calls_from(&ext, "Other.framework_ar()"));
    assert!(!calls_from(&ext, "Other.framework_aj()"));
}

// A bare `helper()` binds by name: the one definition of the corpus, INFERRED (upstream walks
// the inheritance chain and emits EXTRACTED). The tests that assert the call has no edge in
// Go are the ones where the caller or the owner is a `def self.x`, which Go does not extract.

// pins Go v0.12.0 (INFERRED, bound by the unique name `helper`); upstream 0.9.107 emits EXTRACTED 1.0 through the ancestry -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_instance_call_walks_unique_ancestry() {
    let ext = run(&[
        ("base.rb", "class Base < Grand\nend\n"),
        (
            "child.rb",
            "class Child < Base\n  def call\n    helper()\n  end\nend\n",
        ),
        ("grand.rb", "class Grand\n  def helper; :ok; end\nend\n"),
    ]);
    assert_one_inferred_call(&ext, "Grand");
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_stays_inferred_for_dynamic_superclass_constant_write() {
    let ext = run(&[
        ("base.rb", "class Base\n  def helper; :stale; end\nend\n"),
        (
            "child.rb",
            "Base = resolve_base()\nclass Child < Base\n  def call\n    helper()\n  end\nend\n",
        ),
    ]);
    assert_one_inferred_call(&ext, "Base");
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_stays_inferred_for_multiple_constant_write() {
    let ext = run(&[
        ("base.rb", "class Base\n  def helper; :stale; end\nend\n"),
        (
            "child.rb",
            "Base, Spare = ExternalBase, Object\nclass Child < Base\n  def call\n    helper()\n  end\nend\n",
        ),
    ]);
    assert_one_inferred_call(&ext, "Base");
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_stays_inferred_for_external_owner_eval() {
    let ext = run(&[
        ("base.rb", "class Base\n  def helper; :base; end\nend\n"),
        (
            "child.rb",
            "class Child < Base\n  def call\n    helper()\n  end\nend\n",
        ),
        ("patch.rb", "Base.class_eval { prepend ExternalMethods }\n"),
    ]);
    assert_one_inferred_call(&ext, "Base");
}

// pins Go v0.12.0 (two `helper` definitions in one directory bind to nothing); upstream 0.9.107 stops at the nearer ambiguous method and emits no EXTRACTED edge -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_stops_at_ambiguous_nearer_method() {
    let ext = run(&[
        (
            "base.rb",
            "class Base < Grand\n  def helper; :instance; end\n  def self.helper; :singleton; end\nend\n",
        ),
        (
            "child.rb",
            "class Child < Base\n  def call\n    helper()\n  end\nend\n",
        ),
        ("grand.rb", "class Grand\n  def helper; :grand; end\nend\n"),
    ]);
    assert!(
        !calls_from(&ext, "Child.call()"),
        "{:?}",
        pairs(&ext, "calls")
    );
}

// pins Go v0.12.0 (holds because Go extracts no `def self.call`: there is no `Child.call()` node); upstream 0.9.107 does not cross the method kind -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_does_not_cross_method_kind() {
    let ext = run(&[
        ("base.rb", "class Base\n  def helper; :instance; end\nend\n"),
        (
            "child.rb",
            "class Child < Base\n  def self.call\n    helper()\n  end\nend\n",
        ),
    ]);
    assert!(!label_set(&ext).contains("Child.call()"));
    assert!(!calls_from(&ext, "Child.call()"));
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_stays_inferred_across_lookup_barrier() {
    let ext = run(&[
        ("base.rb", "class Base\n  def helper; :base; end\nend\n"),
        (
            "child.rb",
            "class Child < Base\n  include ExternalMethods\n  def call\n    helper()\n  end\nend\n",
        ),
    ]);
    assert_one_inferred_call(&ext, "Base");
}

// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_call_stays_inferred_for_reopened_class() {
    let ext = run(&[
        ("base.rb", "class Base\n  def helper; :base; end\nend\n"),
        (
            "child.rb",
            "class Child < Base\n  def call\n    helper()\n  end\nend\n",
        ),
        ("reopen.rb", "class Child\n  def other; :other; end\nend\n"),
    ]);
    assert_one_inferred_call(&ext, "Base");
}

// pins Go v0.12.0 (holds because Go extracts no `def self.call` and no `def Child.helper`: there is no `Child.call()` node); upstream 0.9.107 emits no EXTRACTED edge for an external owner -> P3
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_inherited_singleton_call_stays_inferred_for_external_owner() {
    let ext = run(&[
        (
            "base.rb",
            "class Base\n  def self.helper; :base; end\nend\n",
        ),
        (
            "child.rb",
            "class Child < Base\n  def self.call\n    helper()\n  end\nend\n",
        ),
        ("reopen.rb", "def Child.helper; :child; end\n"),
    ]);
    assert!(!label_set(&ext).contains("Child.call()"));
    assert!(!calls_from(&ext, "Child.call()"));
}
