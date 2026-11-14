//! graphify-go `internal/extract/scala_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_scala_*.py` files whose assertion holds for graphify-go v0.12.0 (see the `extract-scala`
//! block of `parity/README.md`). The upstream sources are written inline, so no fixture is copied.
//! Upstream names the nodes of a method `.name()`; Go labels them `Owner.name()`. Go edges carry no
//! `context`, so the upstream `context == ...` checks are not ported, except as the negatives that
//! hold because Go has none; those carry the `pins Go v0.12.0` comment.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file_from_bytes, kinds, resolve};
use crate::model::{Confidence, Extraction};
use crate::testutil::testdata;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_extract_scala() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("scalaproj");
    let files = ["util/math.scala", "web/server.scala"];
    let results: Vec<_> = files
        .iter()
        .map(|f| {
            let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("ReadFile({f}): {e}"));
            super::extract_scala(f, &src).expect("parse budget")
        })
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
        "math.scala",
        "server.scala",
        "add()",
        "boot()",
        "MathUtils",
        "MathUtils.double()",
        "Server",
        "Server.start()",
        "scala.collection.mutable.Map", // external import dependency node
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

    // Type method scoped under its type, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start()"
    );
    assert!(
        has("MathUtils", "contains", "MathUtils.double()"),
        "expected MathUtils --contains--> MathUtils.double()"
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
    // Import recorded as an external dependency edge from the file node.
    assert!(
        has("server.scala", "imports", "scala.collection.mutable.Map"),
        "expected server.scala --imports--> scala.collection.mutable.Map"
    );
}

// ---- upstream graphify v0.9.107 `test_scala_*.py` ----

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

fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// Upstream `nid[label]`: the id of the first node with this label.
fn find<'a>(ext: &'a Extraction, label: &str) -> &'a str {
    ext.nodes
        .iter()
        .find(|n| n.label == label)
        .unwrap_or_else(|| panic!("no node labelled {label:?}"))
        .id
        .as_str()
}

/// The labels of the targets of every `relation` edge whose source is the node `src` (a label).
fn targets_from<'a>(ext: &'a Extraction, src: &str, relation: &str) -> Vec<&'a str> {
    let label = id2label(ext);
    let id = find(ext, src);
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation && e.source == id)
        .map(|e| label.get(e.target.as_str()).copied().unwrap_or(""))
        .collect()
}

fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    targets_from(ext, src, relation).contains(&tgt)
}

/// `references` edges from `src` back to `src`.
fn self_refs(ext: &Extraction, src: &str) -> usize {
    let id = find(ext, src);
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == "references" && e.source == id && e.target == id)
        .count()
}

/// The labels of the nodes without a `source_file` (stubs).
fn sourceless_labels(ext: &Extraction) -> BTreeSet<&str> {
    ext.nodes
        .iter()
        .filter(|n| n.source_file.is_empty())
        .map(|n| n.label.as_str())
        .collect()
}

// ---- test_scala_abstract_methods.py ----

// pins Go v0.12.0 (contains); upstream 0.9.107 emits method -> P3 (B28)
#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_deferred_method_without_parens_is_a_node() {
    let ext = run(&[("s.scala", "trait Shape {\n  def area: Double\n}\n")]);
    assert!(has_edge(&ext, "Shape", "contains", "Shape.area()"));
}

// pins Go v0.12.0 (contains); upstream 0.9.107 emits method -> P3 (B28)
#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_deferred_method_with_params_is_a_node() {
    let ext = run(&[(
        "s.scala",
        "trait Shape {\n  def scale(f: Double): Shape\n}\n",
    )]);
    assert!(has_edge(&ext, "Shape", "contains", "Shape.scale()"));
}

// pins Go v0.12.0 (contains); upstream 0.9.107 emits method -> P3 (B28)
#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_pure_interface_trait_keeps_every_contract_method() {
    let ext = run(&[(
        "s.scala",
        "trait Shape {\n  def area: Double\n  def name(): String\n  def describe: String = name()\n}\n",
    )]);
    for m in ["Shape.area()", "Shape.name()", "Shape.describe()"] {
        assert!(has_edge(&ext, "Shape", "contains", m), "{m}");
    }
    // The concrete method's call has a target to resolve to.
    assert!(has_edge(&ext, "Shape.describe()", "calls", "Shape.name()"));
}

// pins Go v0.12.0 (contains); upstream 0.9.107 emits method -> P3 (B28)
#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_abstract_class_deferred_method_is_a_node() {
    let ext = run(&[(
        "s.scala",
        "abstract class Base {\n  def compute(x: Int): Int\n  def run(): Int = compute(1)\n}\n",
    )]);
    assert!(has_edge(&ext, "Base", "contains", "Base.compute()"));
    assert!(has_edge(&ext, "Base.run()", "calls", "Base.compute()"));
}

// ---- test_scala_context_bounds.py ----

const BOUNDS_SRC: &str = r#"trait Ordering
trait Async
trait Temporal
trait Comparable

def sort[A: Ordering](xs: List[A]): List[A] = xs

def program[F[_]: Async: Temporal](x: F[Int]): F[Int] = x

def plain[T](t: T): T = t

def bounded[T <: Comparable[T]](t: T): T = t

class Container {
  def member[K: Ordering](k: K): K = k
}
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_unbounded_type_parameter_emits_no_bound_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go edges have no `context`)
    // Upstream: no `type_bound` reference from `plain()`. Go: no reference at all, as `T` is its
    // own type parameter.
    let ext = run(&[("ContextBounds.scala", BOUNDS_SRC)]);
    assert!(targets_from(&ext, "plain()", "references").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_upper_bound_emits_no_bound_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go edges have no `context`)
    // `[T <: Comparable[T]]` is an `upper_bound`: a subtype constraint, not a dependency.
    let ext = run(&[("ContextBounds.scala", BOUNDS_SRC)]);
    assert!(targets_from(&ext, "bounded()", "references").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_bound_never_points_at_the_function_itself() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go edges have no `context`)
    let ext = run(&[("ContextBounds.scala", BOUNDS_SRC)]);
    for label in ["sort()", "program()", "Container.member()"] {
        assert_eq!(self_refs(&ext, label), 0, "{label} references itself");
    }
}

const CLASS_BOUNDS_SRC: &str = r#"trait Ordering
trait Show
trait Eq
trait Async
trait Comparable

class Sorted[A: Ordering](xs: List[A])

case class Pretty[T: Show: Eq](t: T)

trait Effectful[F[_]: Async]

enum Ranked[A: Ordering] { case Low }

class Plain[T](t: T)

class Upper[T <: Comparable[T]](t: T)

class Holder {
  def member[K: Ordering](k: K): K = k
}
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_class_without_context_bound_emits_no_bound_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go edges have no `context`)
    let ext = run(&[("ClassContextBounds.scala", CLASS_BOUNDS_SRC)]);
    assert!(targets_from(&ext, "Plain", "references").is_empty());
    assert!(targets_from(&ext, "Upper", "references").is_empty());
}

const DERIVES_SRC: &str = r#"trait Show
trait Eq

case class Point(x: Int) derives Show, Eq

enum Color derives Eq { case Red }

case class Wrapped(x: Int) derives cats.Show

class Bare(x: Int)
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_class_without_derives_emits_no_derives_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (Go edges have no `context`)
    // Go gives `Bare` its `x: Int` reference, but none to a typeclass.
    let ext = run(&[("Derives.scala", DERIVES_SRC)]);
    let targets = targets_from(&ext, "Bare", "references");
    assert!(
        !targets.contains(&"Show") && !targets.contains(&"Eq"),
        "{targets:?}"
    );
}

// ---- test_scala_extension_methods.py ----

const EXTENSION_SRC: &str = r#"class Point(val x: Int):
  def norm: Int = x

extension (p: Point)
  def flip(): Point = p
  def twice: Int = flip().norm * 2

extension (x: Int)
  def double: Int = x * 2

extension [A](xs: List[A])
  def second: A = xs(1)

extension (using ctx: Ctx)(s: String)
  def shout: String = s

extension (t: pkg.Thing) {
  def first: Int = 1
  def last: Int = 2
}

extension [T](t: T)
  def self2: T = t
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_extension_is_deterministic() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 B58
    // Go reads no `extension_definition`, so the extension methods are no nodes.
    type Key = (Vec<String>, Vec<(String, String, String)>);
    let key = |ext: &Extraction| -> Key {
        let mut nodes: Vec<String> = ext.nodes.iter().map(|n| n.id.clone()).collect();
        nodes.sort();
        let mut edges: Vec<(String, String, String)> = ext
            .edges
            .iter()
            .map(|e| {
                (
                    e.source.clone(),
                    e.relation.as_str().to_string(),
                    e.target.clone(),
                )
            })
            .collect();
        edges.sort();
        (nodes, edges)
    };
    let a = run(&[("ext.scala", EXTENSION_SRC)]);
    let b = run(&[("ext.scala", EXTENSION_SRC)]);
    assert_eq!(key(&a), key(&b));
}

// ---- test_scala_generic_type_params.py ----

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_generic_class_and_method_type_params_mint_no_stub() {
    let ext = run(&[(
        "S.scala",
        "class Box[T](v: T) {\n  def get(): T = v\n  def map[R](f: T => R): R = ???\n}\n",
    )]);
    assert!(!ext.edges.iter().any(|e| e.target == "t" || e.target == "r"));
    let stubs = sourceless_labels(&ext);
    assert!(!stubs.contains("T"), "sourceless T: {stubs:?}");
    assert!(!stubs.contains("R"), "sourceless R: {stubs:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_bounds_param_dropped_but_context_bound_and_real_types_kept() {
    // `A` is a parameter (dropped); the `Ordering` context bound is a real dependency (kept,
    // #2046), and a genuine user type in the signature still references.
    let ext = run(&[(
        "S.scala",
        "class User(name: String)\nclass Processor[A: Ordering] {\n  def run(u: User): User = u\n}\n",
    )]);
    let label = id2label(&ext);
    let refs: BTreeSet<&str> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "references")
        .filter_map(|e| label.get(e.target.as_str()).copied())
        .collect();
    assert!(refs.contains("Ordering"), "{refs:?}");
    assert!(refs.contains("User"), "{refs:?}");
    assert!(!refs.contains("A"), "{refs:?}");
}

// ---- test_scala_given_instances.py ----

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_given_members_do_not_leak_to_file_scope() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3
    // Go reads no `given_definition`: the members are no nodes, scoped or not.
    let ext = run(&[(
        "given.scala",
        "trait Show[A]:\n  def show(a: A): String\n\ngiven intShow: Show[Int] with\n  def show(a: Int): String = helper(a)\n  def helper(a: Int): String = a.toString\n\ngiven Show[String] with\n  def show(a: String): String = a\n",
    )]);
    // a leaked member would be a bare, file-scoped `helper()`
    assert!(
        !ext.nodes.iter().any(|n| n.label == "helper()"),
        "given member leaked to file scope"
    );
}

// pins Go v0.12.0 (contains); upstream 0.9.107 emits method -> P3 (B28)
#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_trait_method_is_untouched_by_the_given() {
    // Go reads no `given_definition`, so the given steals nothing; the trait keeps its method.
    let ext = run(&[(
        "given.scala",
        "trait Show[A]:\n  def show(a: A): String\n\ngiven intShow: Show[Int] with\n  def show(a: Int): String = helper(a)\n  def helper(a: Int): String = a.toString\n\ngiven Show[String] with\n  def show(a: String): String = a\n",
    )]);
    assert!(
        has_edge(&ext, "Show", "contains", "Show.show()"),
        "trait lost its own method"
    );
    assert!(targets_from(&ext, "Show", "calls").is_empty());
}

// ---- test_scala_self_type.py ----

const SELF_TYPE_SRC: &str = r#"trait Loggable
trait Database
trait HasConfig
trait BaseTrait

class SimpleSelfType {
  self: HasConfig =>

  def configure(): Unit = ()
}

class CompoundSelfType {
  self: Loggable with Database =>

  def run(): Unit = ()
}

class RefinedSelfType {
  self: Loggable { def extra: Int } =>

  def refined(): Unit = ()
}

class BinderOnlySelfType {
  self =>

  def noop(): Unit = ()
}

class NoSelfType {
  def plain(): Unit = ()
}

class SelfTypeWithExtends extends BaseTrait {
  self: Loggable =>

  def combined(): Unit = ()
}
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_self_type_binder_only_emits_no_requires_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3
    // Go reads no `self_type`: no `requires` edge anywhere.
    let ext = run(&[("SelfTypes.scala", SELF_TYPE_SRC)]);
    assert!(targets_from(&ext, "BinderOnlySelfType", "requires").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_class_without_self_type_emits_no_requires_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3
    let ext = run(&[("SelfTypes.scala", SELF_TYPE_SRC)]);
    assert!(targets_from(&ext, "NoSelfType", "requires").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_requires_edges_carry_no_context() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3
    // Upstream: every `requires` edge has no `context`. Go has neither `requires` edges nor
    // a `context`.
    let ext = run(&[("SelfTypes.scala", SELF_TYPE_SRC)]);
    assert!(
        !ext.edges.iter().any(|e| e.relation.as_str() == "requires"),
        "a `requires` edge"
    );
}

// ---- test_scala_top_level_binding.py ----

const TOP_LEVEL_SRC: &str = r#"class Inner {
  val nested: String = "x"
  var nestedCounter: Int = 0
}

val greeting: String = "World"
var counter: Int = 0

trait Config
val config: Config = null

val names: List[String] = Nil
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_class_nested_binding_keeps_its_type_reference() {
    // Control: the in-class behaviour. The edge is owned by the class, not the file.
    let ext = run(&[("TopLevel.scala", TOP_LEVEL_SRC)]);
    assert!(has_edge(&ext, "Inner", "references", "String"));
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_top_level_binding_emits_no_self_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3
    // Go reads no file-scope `val`/`var`: the file node owns no reference.
    let ext = run(&[("TopLevel.scala", TOP_LEVEL_SRC)]);
    assert_eq!(self_refs(&ext, "TopLevel.scala"), 0);
}

// ---- test_scala_type_definition.py ----

const ALIAS_SRC: &str = r#"class Marker
class Payload

trait Repo:
  type Alias = List[Payload]
  opaque type Id = Long
  type Elem[X] = X match
    case String => Char

type TopLevel = Map[String, Int]
opaque type OpaqueTop = Marker

class Holder {
  val plain: Payload = null
}
"#;

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_match_type_emits_reference_to_matched_type() {
    // `case String =>` names a real type the alias discriminates on.
    let ext = run(&[("Aliases.scala", ALIAS_SRC)]);
    assert!(has_edge(&ext, "Repo", "references", "String"));
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_type_definition_edges_are_tagged_extracted() {
    let ext = run(&[("Aliases.scala", ALIAS_SRC)]);
    let owners = [find(&ext, "Repo"), find(&ext, "Aliases.scala")];
    let edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "references" && owners.contains(&e.source.as_str()))
        .collect();
    assert!(!edges.is_empty());
    for e in edges {
        assert_eq!(e.confidence, Confidence::EXTRACTED, "{e:?}");
        let loc = e.source_location.to_string();
        assert!(loc.starts_with('L'), "{loc:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.36"]
fn test_alias_reference_never_points_at_the_owning_container() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3
    // The `Repo` half holds in Go; Go reads no file-scope `type`, so the file node has no
    // reference at all.
    let ext = run(&[("Aliases.scala", ALIAS_SRC)]);
    for owner in ["Repo", "Aliases.scala"] {
        assert_eq!(self_refs(&ext, owner), 0, "{owner} references itself");
    }
}
