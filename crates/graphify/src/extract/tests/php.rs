//! graphify-go `internal/extract/php_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_extract_php_closures.py`, `test_php_*.py` files whose assertion holds for graphify-go
//! v0.12.0 (see the `extract-php` block of `parity/README.md`). The upstream sources are written
//! inline, so no fixture is copied. Upstream names the nodes of a method `.name()`; Go labels
//! them `Type.name()`, and an upstream `.method` edge is Go's `contains` (B28).

use std::collections::{BTreeMap, BTreeSet};

use super::extract_php;
use crate::extract::{file_from_bytes, kinds, resolve};
use crate::model::{Confidence, Edge, Extraction};
use crate::testutil::testdata;

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_php() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("phpproj");
    let files = ["util/math.php", "web/server.php"];

    // Call extractPHP directly: the central File() dispatch is wired elsewhere.
    let results: Vec<_> = files
        .iter()
        .map(|f| {
            let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("ReadFile({f}): {e}"));
            extract_php(f, &src).unwrap_or_else(|| panic!("extract_php({f}): over budget"))
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
        "math.php",
        "server.php",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
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
    // Method scoped under its type, with a contains edge.
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
    // External import edge for the `use` statement (Psr\Log\LoggerInterface).
    let rels = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        rels != 0,
        "no external import edges (expected Psr\\Log\\LoggerInterface)"
    );
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_language_constructs_not_calls() {
    kinds::register_kinds();
    let src = "<?php\n\
               class A {\n\
               \x20   function isset($k) { return true; }\n\
               \x20   function list() { return []; }\n\
               \x20   function run($x) {\n\
               \x20       if (isset($x)) { return $this->list(); }\n\
               \x20       return empty($x) ? array() : EXIT();\n\
               \x20   }\n\
               }\n";
    let result = extract_php("A.php", src.as_bytes()).expect("extract_php: over budget");
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&[result], &["A.php".to_string()], workers);
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let mut got_list = false;
    for e in &ext.edges {
        if e.relation.as_str() != "calls" {
            continue;
        }
        match id2label.get(e.target.as_str()).copied() {
            Some("A.isset()") => panic!("isset(...) construct recorded as a call"),
            Some("A.list()") => got_list = true,
            _ => {}
        }
    }
    assert!(got_list, "$this->list() call lost");
}

// ---- upstream graphify v0.9.107 `test_*php*.py` ----

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

/// The first edge of `relation` from the node labelled `src` to the one labelled `tgt`.
fn find_edge<'a>(ext: &'a Extraction, src: &str, relation: &str, tgt: &str) -> Option<&'a Edge> {
    let label = id2label(ext);
    ext.edges.iter().find(|e| {
        e.relation.as_str() == relation
            && label.get(e.source.as_str()) == Some(&src)
            && label.get(e.target.as_str()) == Some(&tgt)
    })
}

fn label_set(ext: &Extraction) -> BTreeSet<&str> {
    ext.nodes.iter().map(|n| n.label.as_str()).collect()
}

/// The ids of the nodes labelled `label` that come from `file`.
fn ids_of<'a>(ext: &'a Extraction, file: &str, label: &str) -> BTreeSet<&'a str> {
    ext.nodes
        .iter()
        .filter(|n| n.source_file == file && n.label == label)
        .map(|n| n.id.as_str())
        .collect()
}

/// (source id, target id) of every edge, of any relation, from one of `from` to one of `to`.
fn edges_between<'a>(
    ext: &'a Extraction,
    from: &BTreeSet<&str>,
    to: &BTreeSet<&str>,
) -> Vec<(&'a str, &'a str)> {
    ext.edges
        .iter()
        .filter(|e| from.contains(e.source.as_str()) && to.contains(e.target.as_str()))
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect()
}

/// The labels of the nodes of the members of `class` (`Class.name()`).
fn members(ext: &Extraction, class: &str) -> BTreeSet<String> {
    let prefix = format!("{class}.");
    ext.nodes
        .iter()
        .filter(|n| n.label.starts_with(&prefix))
        .map(|n| n.label.clone())
        .collect()
}

// test_extract_php_closures.py: Go makes no closure or route node (B45), and records no call at
// file scope; its walk of a method body goes into a closure and keeps the method as the caller.

// pins Go v0.12.0 (the call is attributed to the enclosing method `Service.run()`; no closure node); upstream 0.9.107 attributes it to the `{closure#1}()` node -> P3 B45
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_method_scope_closure_call_attributes_to_closure_not_method() {
    let ext = run(&[(
        "svc.php",
        "<?php\n\
         function target($x) { return $x; }\n\
         class Service {\n\
         \x20   public function run() {\n\
         \x20       $cb = function($x) { return target($x); };\n\
         \x20       return $cb;\n\
         \x20   }\n\
         }\n",
    )]);
    let callers: BTreeSet<String> = pairs(&ext, "calls")
        .into_iter()
        .filter(|(_, t)| t == "target()")
        .map(|(s, _)| s)
        .collect();
    assert_eq!(callers, BTreeSet::from(["Service.run()".to_string()]));
    assert!(!label_set(&ext).iter().any(|l| l.starts_with("{closure#")));
}

// test_php_class_const_initializer_refs.py: Go has no `references_constant` relation and reads no
// class-level initializer (B46). A type position in the same class gives it a `references` edge to
// the same pair.

const ENUM_ROLE: &str = "enum Role {\n    case Viewer;\n    case Admin;\n}\n";

/// The (source label, target label) pairs of the `references` edges of `Perms.php`.
fn refs(body: &str) -> BTreeSet<(String, String)> {
    let src = format!("<?php\nnamespace App;\n{ENUM_ROLE}{body}");
    pairs(&run(&[("Perms.php", src.as_str())]), "references")
}

// pins Go v0.12.0 (relation references, from the property type `Role`); upstream 0.9.107 emits references_constant from the `Role::Viewer` default -> P3 B46
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_property_default_scope_member_is_referenced() {
    let got = refs("class Settings {\n    private Role $fallback = Role::Viewer;\n}\n");
    assert!(
        got.contains(&("Settings".to_string(), "Role".to_string())),
        "{got:?}"
    );
}

// pins Go v0.12.0 (relation references, from the promoted parameter type `Role`); upstream 0.9.107 emits references_constant from the `Role::Viewer` default -> P3 B46
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_promoted_constructor_parameter_default_is_referenced() {
    let got = refs(
        "class Guard {\n    public function __construct(private Role $default = Role::Viewer) {}\n}\n",
    );
    assert!(
        got.contains(&("Guard".to_string(), "Role".to_string())),
        "{got:?}"
    );
}

// pins Go v0.12.0 (relation references, from the return type `Role`); upstream 0.9.107 emits references_constant from the `Role::Viewer` in the body -> P3 B46
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_method_body_reference_still_works() {
    let got = refs("class Check {\n    public function pick(): Role { return Role::Viewer; }\n}\n");
    assert!(
        got.iter()
            .any(|(src, tgt)| src == "Check.pick()" && tgt == "Role"),
        "{got:?}"
    );
}

// pins Go v0.12.0 (holds only because Go has no references_constant relation); upstream 0.9.107 emits none for `self::`, `static::` and `parent::` either -> P3 B46
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_self_static_parent_scope_emits_no_spurious_edge() {
    let src = "<?php\nnamespace App;\nclass Base { const SEED = 1; }\n\
               class C extends Base {\n\
               \x20   const A = 1;\n\
               \x20   const B = self::A;\n\
               \x20   private int $x = static::A;\n\
               \x20   public function __construct(int $n = parent::SEED) {}\n\
               }\n";
    let ext = run(&[("Perms.php", src)]);
    let got = pairs(&ext, "references_constant");
    assert!(got.is_empty(), "{got:?}");
}

// pins Go v0.12.0 (relation references, from the property type `Role`); upstream 0.9.107 emits references_constant from the `\App\Role::Viewer` default -> P3 B46
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_qualified_scope_resolves_to_local_class() {
    let got = refs("class S {\n    private Role $f = \\App\\Role::Viewer;\n}\n");
    assert!(
        got.contains(&("S".to_string(), "Role".to_string())),
        "{got:?}"
    );
    assert!(
        !got.iter()
            .any(|(_, tgt)| tgt.contains("App") || tgt.contains('\\')),
        "{got:?}"
    );
}

// test_php_language_construct_calls.py (#3830). Go skips `array`, `die`, `empty`, `eval`,
// `exit`, `isset`, `list` and `unset` by lower-cased name (`phpConstructs`).

const BAG: &str = "<?php\n\
    namespace App;\n\
    \n\
    class Bag\n\
    {\n\
    \x20   private array $items = [];\n\
    \n\
    \x20   public function empty(): bool\n\
    \x20   {\n\
    \x20       return $this->items === [];\n\
    \x20   }\n\
    \n\
    \x20   public function isset(string $k): bool\n\
    \x20   {\n\
    \x20       return array_key_exists($k, $this->items);\n\
    \x20   }\n\
    }\n";

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_construct_in_another_file_does_not_bind_to_user_method() {
    let ext = run(&[
        ("Bag.php", BAG),
        (
            "Mailer.php",
            "<?php\n\
             namespace App;\n\
             \n\
             class Mailer\n\
             {\n\
             \x20   public function send(array $to): void\n\
             \x20   {\n\
             \x20       if (empty($to) || !isset($to[0])) {\n\
             \x20           return;\n\
             \x20       }\n\
             \x20   }\n\
             }\n",
        ),
    ]);
    let mut method_ids = ids_of(&ext, "Bag.php", "Bag.empty()");
    method_ids.extend(ids_of(&ext, "Bag.php", "Bag.isset()"));
    let send_ids = ids_of(&ext, "Mailer.php", "Mailer.send()");
    assert!(
        !method_ids.is_empty() && !send_ids.is_empty(),
        "the methods and the caller must still be extracted"
    );
    let phantom = edges_between(&ext, &send_ids, &method_ids);
    assert!(
        phantom.is_empty(),
        "a construct in Mailer.php bound to a Bag method: {phantom:?}"
    );
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_user_method_node_survives_the_filter() {
    let ext = run(&[("Bag.php", BAG)]);
    let labels = label_set(&ext);
    assert!(
        labels.contains("Bag.empty()") && labels.contains("Bag.isset()"),
        "a Bag method disappeared; labels were {labels:?}"
    );
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_construct_does_not_bind_in_file() {
    let ext = run(&[(
        "Bag.php",
        "<?php\n\
         namespace App;\n\
         \n\
         class Bag\n\
         {\n\
         \x20   private array $items = [];\n\
         \n\
         \x20   public function empty(): bool\n\
         \x20   {\n\
         \x20       return $this->items === [];\n\
         \x20   }\n\
         \n\
         \x20   public function isset(string $k): bool\n\
         \x20   {\n\
         \x20       return array_key_exists($k, $this->items);\n\
         \x20   }\n\
         \n\
         \x20   public function add(string $k, $v): void\n\
         \x20   {\n\
         \x20       if (empty($k) || isset($this->items[$k])) {\n\
         \x20           return;\n\
         \x20       }\n\
         \x20       $this->items[$k] = $v;\n\
         \x20   }\n\
         }\n",
    )]);
    let mut method_ids = ids_of(&ext, "Bag.php", "Bag.empty()");
    method_ids.extend(ids_of(&ext, "Bag.php", "Bag.isset()"));
    let add_ids = ids_of(&ext, "Bag.php", "Bag.add()");
    assert!(
        !method_ids.is_empty() && !add_ids.is_empty(),
        "both methods and the caller must still be extracted"
    );
    let phantom = edges_between(&ext, &add_ids, &method_ids);
    assert!(
        phantom.is_empty(),
        "a construct in add() bound to a sibling method: {phantom:?}"
    );
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_construct_is_matched_case_insensitively() {
    let ext = run(&[
        ("Bag.php", BAG),
        (
            "helpers.php",
            "<?php\n\
             namespace App;\n\
             \n\
             function blank(array $a): bool\n\
             {\n\
             \x20   return EMPTY($a);\n\
             }\n",
        ),
    ]);
    let method_ids = ids_of(&ext, "Bag.php", "Bag.empty()");
    let blank_ids = ids_of(&ext, "helpers.php", "blank()");
    assert!(
        !method_ids.is_empty() && !blank_ids.is_empty(),
        "the method and the caller must still be extracted"
    );
    let phantom = edges_between(&ext, &blank_ids, &method_ids);
    assert!(
        phantom.is_empty(),
        "EMPTY() bound to Bag::empty(): {phantom:?}"
    );
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_calls_inside_construct_arguments_still_resolve() {
    let ext = run(&[(
        "Repo.php",
        "<?php\n\
         namespace App;\n\
         \n\
         class Repo\n\
         {\n\
         \x20   public function load(): array\n\
         \x20   {\n\
         \x20       return [];\n\
         \x20   }\n\
         \n\
         \x20   public function missing(): bool\n\
         \x20   {\n\
         \x20       return empty($this->load());\n\
         \x20   }\n\
         }\n",
    )]);
    let load_ids = ids_of(&ext, "Repo.php", "Repo.load()");
    let missing_ids = ids_of(&ext, "Repo.php", "Repo.missing()");
    assert!(
        !load_ids.is_empty() && !missing_ids.is_empty(),
        "both methods must still be extracted"
    );
    assert!(
        !edges_between(&ext, &missing_ids, &load_ids).is_empty(),
        "$this->load() inside empty(...) must still resolve"
    );
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_non_construct_function_call_still_resolves() {
    let ext = run(&[
        (
            "helpers.php",
            "<?php\n\
             namespace App;\n\
             \n\
             function format_name(string $n): string\n\
             {\n\
             \x20   return trim($n);\n\
             }\n",
        ),
        (
            "User.php",
            "<?php\n\
             namespace App;\n\
             \n\
             class User\n\
             {\n\
             \x20   public function label(string $n): string\n\
             \x20   {\n\
             \x20       return format_name($n);\n\
             \x20   }\n\
             }\n",
        ),
    ]);
    let target_ids = ids_of(&ext, "helpers.php", "format_name()");
    let caller_ids = ids_of(&ext, "User.php", "User.label()");
    assert!(
        !target_ids.is_empty() && !caller_ids.is_empty(),
        "the function and the caller must still be extracted"
    );
    assert!(
        !edges_between(&ext, &caller_ids, &target_ids).is_empty(),
        "a genuine cross-file function call must still resolve"
    );
}

/// `(name, statement)`: a method named like a construct, the construct used as a keyword in a
/// method and in a function, and the method called through `$this`.
const CONSTRUCT_CASES: [(&str, &str); 6] = [
    ("die", "die($value);"),
    ("eval", "eval($value);"),
    ("array", "$result = array($value);"),
    ("exit", "exit($value);"),
    ("list", "list($value) = $values;"),
    ("unset", "unset($value);"),
];

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_construct_named_methods_keep_only_real_member_calls() {
    for (name, statement) in CONSTRUCT_CASES {
        let bag = format!(
            "<?php\nclass Bag {{\n public function {name}($value) {{ return $value; }}\n \
             public function constructUse($value, $values) {{ {statement} }}\n \
             public function memberUse($value) {{ return $this->{name}($value); }}\n}}\n"
        );
        let outside = format!("<?php\nfunction externalUse($value, $values) {{ {statement} }}\n");
        let ext = run(&[("Bag.php", bag.as_str()), ("outside.php", outside.as_str())]);
        let target = ids_of(&ext, "Bag.php", &format!("Bag.{name}()"));
        let local = ids_of(&ext, "Bag.php", "Bag.constructUse()");
        let external = ids_of(&ext, "outside.php", "externalUse()");
        let member = ids_of(&ext, "Bag.php", "Bag.memberUse()");
        assert!(
            !target.is_empty() && !local.is_empty() && !external.is_empty() && !member.is_empty(),
            "{name}: a node is missing"
        );
        let from: BTreeSet<&str> = local.union(&external).copied().collect();
        assert!(
            edges_between(&ext, &from, &target).is_empty(),
            "{name}: the keyword use bound to the method"
        );
        assert!(
            !edges_between(&ext, &member, &target).is_empty(),
            "{name}: the member call lost its edge"
        );
    }
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_builtin_named_self_call_does_not_bind_to_another_class() {
    let ext = run(&[(
        "Bag.php",
        "<?php\nclass Bag { public function useList() { $this->list(); } }\n\
         class Other { public function list() {} }\n",
    )]);
    let caller = ids_of(&ext, "Bag.php", "Bag.useList()");
    let target = ids_of(&ext, "Bag.php", "Other.list()");
    assert!(!caller.is_empty() && !target.is_empty());
    assert!(edges_between(&ext, &caller, &target).is_empty());
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_builtin_named_self_call_does_not_bind_to_a_free_function() {
    let ext = run(&[(
        "Bag.php",
        "<?php\nfunction open() {}\n\
         class Bag { public function useOpen() { $this->open(); } }\n",
    )]);
    let caller = ids_of(&ext, "Bag.php", "Bag.useOpen()");
    let target = ids_of(&ext, "Bag.php", "open()");
    assert!(!caller.is_empty() && !target.is_empty());
    assert!(edges_between(&ext, &caller, &target).is_empty());
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_builtin_named_this_call_without_a_class_owner_stays_unresolved() {
    let ext = run(&[(
        "Bag.php",
        "<?php\nfunction useList() { $this->list(); }\n\
         function makeListCallable() { return function() { $this->list(); }; }\n\
         class Other { public function list() {} }\n",
    )]);
    let target = ids_of(&ext, "Bag.php", "Other.list()");
    assert!(!target.is_empty() && !ids_of(&ext, "Bag.php", "useList()").is_empty());
    assert!(!ids_of(&ext, "Bag.php", "makeListCallable()").is_empty());
    assert!(
        !ext.edges
            .iter()
            .any(|e| e.relation.as_str() == "calls" && target.contains(e.target.as_str()))
    );
}

// test_php_member_collisions.py (#2810, B01). Go ids ignore `_`, case and a trailing `!`, so
// `_x` and `x` of one type are one node: the first declaration gives the label, a call from the
// merged node to itself is dropped, and a call to either name binds to it.

const CLASS_B: &str = "<?php\n\
    class B {\n\
    \x20   private function _x() { return 1; }\n\
    \x20   public function x() { return $this->_x(); }\n\
    \x20   public function y() { return $this->x(); }\n\
    }\n";

// pins Go v0.12.0 (`_x` and `x` are one node `B._x()`, the first declared); upstream 0.9.107 emits two nodes -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_both_methods_are_nodes_with_distinct_ids() {
    let ext = run(&[("b.php", CLASS_B)]);
    assert_eq!(
        members(&ext, "B"),
        BTreeSet::from(["B._x()".to_string(), "B.y()".to_string()])
    );
}

// pins Go v0.12.0 (`_x` and `x` are one node `B._x()`; its call to itself is dropped); upstream 0.9.107 emits `.x()` -> `._x()` -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_calls_bind_to_the_right_method_and_no_self_loop() {
    let ext = run(&[("b.php", CLASS_B)]);
    let calls = pairs(&ext, "calls");
    assert!(calls.contains(&("B.y()".to_string(), "B._x()".to_string())));
    assert!(!calls.iter().any(|(s, t)| s == t), "{calls:?}");
}

/// Callers of `B.x()` outside the class.
const USE_SRCS: [&str; 2] = [
    "<?php\nfunction outside() { $b = new B(); return $b->x(); }\n",
    "<?php\nfunction outside(B $b) { return $b->x(); }\n",
];

// pins Go v0.12.0 (the call binds to the merged node `B._x()`, by the receiver name `b` matching the file `b.php`); upstream 0.9.107 emits `outside()` -> `.x()` only -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_external_call_binds_to_the_public_method() {
    for use_src in USE_SRCS {
        let ext = run(&[("b.php", CLASS_B), ("use.php", use_src)]);
        let calls = pairs(&ext, "calls");
        assert!(
            calls.contains(&("outside()".to_string(), "B._x()".to_string())),
            "{use_src}: {calls:?}"
        );
    }
}

// pins Go v0.12.0 (the first declaration names the merged node, so the two orders differ); upstream 0.9.107 emits the same two nodes in either order -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_order_does_not_matter() {
    let a = members(
        &run(&[(
            "a.php",
            "<?php\nclass A { private function _x() {} public function x() {} }\n",
        )]),
        "A",
    );
    let b = members(
        &run(&[(
            "a.php",
            "<?php\nclass A { public function x() {} private function _x() {} }\n",
        )]),
        "A",
    );
    assert_eq!(a, BTreeSet::from(["A._x()".to_string()]));
    assert_eq!(b, BTreeSet::from(["A.x()".to_string()]));
}

// pins Go v0.12.0 (`_x` and `x` are one node `T._x()`); upstream 0.9.107 emits two nodes -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_trait_with_the_same_pattern() {
    let ext = run(&[(
        "t.php",
        "<?php\ntrait T {\n\
         \x20   protected function _x() { return 1; }\n\
         \x20   public function x() { return $this->_x(); }\n\
         \x20   public function y() { return $this->x(); }\n\
         }\n",
    )]);
    assert_eq!(
        members(&ext, "T"),
        BTreeSet::from(["T._x()".to_string(), "T.y()".to_string()])
    );
    let calls = pairs(&ext, "calls");
    assert!(calls.contains(&("T.y()".to_string(), "T._x()".to_string())));
}

/// An interface, an enum and an abstract class that declare `_x` before `x`.
const BODY_SRCS: [&str; 3] = [
    "<?php\ninterface I { public function _x(); public function x(); }\n",
    "<?php\nenum I { case A; private function _x() {} public function x() {} }\n",
    "<?php\nabstract class I { abstract protected function _x(); public function x() {} }\n",
];

// pins Go v0.12.0 (`_x` and `x` are one node `I._x()`); upstream 0.9.107 emits two nodes -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_interface_enum_and_abstract_bodies() {
    for src in BODY_SRCS {
        let ext = run(&[("i.php", src)]);
        assert_eq!(
            members(&ext, "I"),
            BTreeSet::from(["I._x()".to_string()]),
            "{src}"
        );
    }
}

// pins Go v0.12.0 (`_run` and `Run` are one node `K._run()`: ids fold case; a call by either name binds to it); upstream 0.9.107 emits two nodes -> P3 B01
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_method_names_are_case_insensitive() {
    let ext = run(&[(
        "k.php",
        "<?php\nclass K {\n\
         \x20   private function _run() { return 1; }\n\
         \x20   public function Run() { return $this->_run(); }\n\
         \x20   public function go() { return $this->Run(); }\n\
         }\n",
    )]);
    assert_eq!(
        members(&ext, "K"),
        BTreeSet::from(["K._run()".to_string(), "K.go()".to_string()])
    );
    let calls = pairs(&ext, "calls");
    assert!(calls.contains(&("K.go()".to_string(), "K._run()".to_string())));
    assert!(!calls.iter().any(|(s, t)| s == t), "{calls:?}");
}

// test_php_object_creation.py (#3115): Go records no `object_creation_expression` (no `new X()`
// edge), and a scoped call binds to the method by its receiver.

// pins Go v0.12.0 (holds only because Go records no `new` expression); upstream 0.9.107 emits a calls edge to the constructed class -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_dynamic_and_self_construction_produce_no_junk() {
    let ext = run(&[(
        "Caller.php",
        "<?php\nnamespace App;\nclass Caller {\n\
         \x20   public function a($cls) { return new $cls(); }\n\
         \x20   public function b() { return new self(); }\n\
         \x20   public function c() { return new static(); }\n}\n",
    )]);
    let calls = pairs(&ext, "calls");
    assert!(
        !calls
            .iter()
            .any(|(_, t)| matches!(t.as_str(), "self" | "static" | "$cls")),
        "{calls:?}"
    );
    let labels = label_set(&ext);
    assert!(!labels.contains("self") && !labels.contains("static"));
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_static_call_resolves_to_the_actual_method_not_the_class() {
    let ext = run(&[
        (
            "Baz.php",
            "<?php\nnamespace App;\nclass Baz {\n\
             \x20   public static function create(): self { return new self(); }\n}\n",
        ),
        (
            "Caller.php",
            "<?php\nnamespace App;\nclass Caller {\n\
             \x20   public function run() { return Baz::create(); }\n}\n",
        ),
    ]);
    assert!(has_edge(&ext, "Caller.run()", "calls", "Baz.create()"));
    assert!(!has_edge(&ext, "Caller.run()", "calls", "Baz"));
}

// test_php_receiver_member_calls.py: Go types no receiver (no property, parameter or `new`
// binding), and a receiver binds only to a method qualified by the receiver's last name
// (`$greeter` -> `greeter`, which is not `Greeter`). The negative cases hold for that reason.

const GREETER: &str = "<?php\nclass Greeter {\n    public function greet(): void {}\n}\n";

/// Whether a `calls` edge from a node labelled `*run*` reaches a `*.greet()` method.
fn greet_edge(ext: &Extraction) -> bool {
    pairs(ext, "calls")
        .iter()
        .any(|(s, t)| s.contains("run") && t.ends_with(".greet()"))
}

// pins Go v0.12.0 (holds because Go types no receiver); upstream 0.9.107 also resolves a typed receiver and refuses the untyped one -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_an_untyped_parameter_resolves_to_nothing() {
    let ext = run(&[
        ("Greeter.php", GREETER),
        (
            "App.php",
            "<?php\nclass App {\n\
             \x20   public function run($greeter): void { $greeter->greet(); }\n}\n",
        ),
    ]);
    assert!(!greet_edge(&ext), "{:?}", pairs(&ext, "calls"));
}

// pins Go v0.12.0 (holds because Go types no receiver); upstream 0.9.107 resolves a single declared type and refuses a union -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_a_union_typed_receiver_resolves_to_nothing() {
    let ext = run(&[
        ("Greeter.php", GREETER),
        (
            "Other.php",
            "<?php\nclass Other {\n    public function greet(): void {}\n}\n",
        ),
        (
            "App.php",
            "<?php\nclass App {\n\
             \x20   public function run(Greeter|Other $greeter): void { $greeter->greet(); }\n}\n",
        ),
    ]);
    assert!(!greet_edge(&ext), "{:?}", pairs(&ext, "calls"));
}

// pins Go v0.12.0 (holds because Go types no receiver); upstream 0.9.107 types the first link of a chain and refuses a longer one -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_a_longer_chain_resolves_to_nothing() {
    let ext = run(&[
        ("Greeter.php", GREETER),
        (
            "App.php",
            "<?php\nclass App {\n    private Greeter $greeter;\n\
             \x20   public function run(): void { $this->greeter->inner->greet(); }\n}\n",
        ),
    ]);
    assert!(!greet_edge(&ext), "{:?}", pairs(&ext, "calls"));
}

// pins Go v0.12.0 (holds because Go types no receiver); upstream 0.9.107 refuses to guess between two classes of one name -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_two_classes_of_the_same_name_resolve_to_neither() {
    let ext = run(&[
        ("a/Greeter.php", GREETER),
        ("b/Greeter.php", GREETER),
        (
            "App.php",
            "<?php\nclass App {\n    private Greeter $greeter;\n\
             \x20   public function run(): void { $this->greeter->greet(); }\n}\n",
        ),
    ]);
    assert!(!greet_edge(&ext), "{:?}", pairs(&ext, "calls"));
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_a_static_call_keeps_its_own_path() {
    let ext = run(&[
        (
            "Helper.php",
            "<?php\nclass Helper {\n    public static function format(): void {}\n}\n",
        ),
        (
            "App.php",
            "<?php\nclass App {\n    public function run(): void { Helper::format(); }\n}\n",
        ),
    ]);
    let edge = find_edge(&ext, "App.run()", "calls", "Helper.format()")
        .unwrap_or_else(|| panic!("{:?}", pairs(&ext, "calls")));
    assert_eq!(edge.confidence, Confidence::INFERRED);
}

// test_php_static_call_resolution.py (#3872): a scoped call binds to the method named by the
// scope text, or by `self`, `static` and `parent`; Go marks each such edge INFERRED.

const HELPER: &str = "<?php\nnamespace App;\nclass Helper {\n\
    \x20   public static function sanitize($x) { return $x; }\n}\n";

// pins Go v0.12.0 (INFERRED, confidence_score 0.0); upstream 0.9.107 emits EXTRACTED 1.0 -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_same_namespace_static_call_is_extracted_not_inferred() {
    let ext = run(&[
        ("Helper.php", HELPER),
        (
            "Caller.php",
            "<?php\nnamespace App;\nclass Caller {\n\
             \x20   public function run($x) { return Helper::sanitize($x); }\n}\n",
        ),
    ]);
    let edge = find_edge(&ext, "Caller.run()", "calls", "Helper.sanitize()")
        .unwrap_or_else(|| panic!("{:?}", pairs(&ext, "calls")));
    assert_eq!(edge.confidence, Confidence::INFERRED);
    assert_eq!(edge.confidence_score, 0.0);
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_self_and_static_scope_resolve_to_enclosing_class() {
    let ext = run(&[
        (
            "Widget.php",
            "<?php\nnamespace App;\nclass Widget {\n\
             \x20   public static function build() { return self::helper(); }\n\
             \x20   public static function render() { return static::helper(); }\n\
             \x20   public static function helper() { return 1; }\n}\n",
        ),
        (
            "Decoy.php",
            "<?php\nnamespace App;\nclass Decoy {\n\
             \x20   public static function helper() { return 2; }\n}\n",
        ),
    ]);
    assert!(has_edge(&ext, "Widget.build()", "calls", "Widget.helper()"));
    assert!(has_edge(
        &ext,
        "Widget.render()",
        "calls",
        "Widget.helper()"
    ));
    assert!(!has_edge(&ext, "Widget.build()", "calls", "Decoy.helper()"));
    let edge = find_edge(&ext, "Widget.build()", "calls", "Widget.helper()").unwrap();
    assert_eq!(edge.confidence, Confidence::INFERRED);
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_self_scope_does_not_resolve_to_other_class() {
    let ext = run(&[
        (
            "A.php",
            "<?php\nnamespace App;\nclass A {\n\
             \x20   public function build() { return self::helper(); }\n}\n",
        ),
        (
            "B.php",
            "<?php\nnamespace App;\nclass B {\n\
             \x20   public function helper() { return 1; }\n}\n",
        ),
    ]);
    let calls = pairs(&ext, "calls");
    assert!(!calls.iter().any(|(s, _)| s == "A.build()"), "{calls:?}");
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_parent_scope_resolves_through_inheritance() {
    let ext = run(&[
        (
            "Base.php",
            "<?php\nnamespace App;\nclass Base {\n\
             \x20   public static function helper() { return 1; }\n}\n",
        ),
        (
            "Child.php",
            "<?php\nnamespace App;\nclass Child extends Base {\n\
             \x20   public static function run() { return parent::helper(); }\n}\n",
        ),
    ]);
    assert!(has_edge(&ext, "Child.run()", "calls", "Base.helper()"));
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_dynamic_scope_produces_no_phantom_edge() {
    let ext = run(&[(
        "Caller.php",
        "<?php\nnamespace App;\nclass Caller {\n\
         \x20   public function run($cls) { return $cls::helper(); }\n\
         \x20   public function helper() { return 1; }\n}\n",
    )]);
    let calls = pairs(&ext, "calls");
    assert!(!calls.iter().any(|(s, _)| s == "Caller.run()"), "{calls:?}");
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lowercase_class_name_still_resolves() {
    let ext = run(&[
        (
            "helper.php",
            "<?php\nnamespace App;\nclass helper {\n\
             \x20   public static function sanitize($x) { return $x; }\n}\n",
        ),
        (
            "Caller.php",
            "<?php\nnamespace App;\nclass Caller {\n\
             \x20   public function run($x) { return helper::sanitize($x); }\n}\n",
        ),
    ]);
    let edge = find_edge(&ext, "Caller.run()", "calls", "helper.sanitize()")
        .unwrap_or_else(|| panic!("{:?}", pairs(&ext, "calls")));
    assert_eq!(edge.confidence, Confidence::INFERRED);
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_fully_qualified_scope_resolves_by_last_segment() {
    let ext = run(&[
        ("Helper.php", HELPER),
        (
            "Caller.php",
            "<?php\nnamespace Other;\nclass Caller {\n\
             \x20   public function run($x) { return \\App\\Helper::sanitize($x); }\n}\n",
        ),
    ]);
    let edge = find_edge(&ext, "Caller.run()", "calls", "Helper.sanitize()")
        .unwrap_or_else(|| panic!("{:?}", pairs(&ext, "calls")));
    assert_eq!(edge.confidence, Confidence::INFERRED);
}

// test_php_type_resolution.py. Go binds a base class by its name alone: the one corpus
// definition, else the one in the importing file's directory or named by the qualifier's last
// segment. It reads no `use` clause for a base (the `use` is an import to an external node
// labelled with the full name, never to a corpus class).

/// The nodes labelled `label` that have a source file: the corpus definitions, not stubs.
fn class_defs<'a>(ext: &'a Extraction, label: &str) -> Vec<&'a crate::model::Node> {
    ext.nodes
        .iter()
        .filter(|n| n.label == label && !n.source_file.is_empty())
        .collect()
}

fn node_by_id<'a>(ext: &'a Extraction, id: &str) -> Option<&'a crate::model::Node> {
    ext.nodes.iter().find(|n| n.id == id)
}

/// The edges of `relation` whose source node is labelled `src`.
fn edges_from<'a>(ext: &'a Extraction, src: &str, relation: &str) -> Vec<&'a Edge> {
    let label = id2label(ext);
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation && label.get(e.source.as_str()) == Some(&src))
        .collect()
}

// pins Go v0.12.0 (inherits binds the one internal `Page` by name; the `use Filament\Pages\Page` is not read); upstream 0.9.107 emits an external stub `Filament\Pages\Page` -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_external_namespaced_base_does_not_collapse_onto_internal_class() {
    let ext = run(&[
        (
            "app/Models/Page.php",
            "<?php\nnamespace App\\Models;\nclass Page extends Model {}\n",
        ),
        (
            "app/Filament/Pages/ManageSiteSettings.php",
            "<?php\nnamespace App\\Filament\\Pages;\n\
             use Filament\\Pages\\Page;\n\
             class ManageSiteSettings extends Page {}\n",
        ),
    ]);
    // Exactly one internal `Page` definition, and it is App\Models\Page.
    let page_defs = class_defs(&ext, "Page");
    assert_eq!(page_defs.len(), 1);
    assert!(page_defs[0].source_file.contains("Models"));
    let internal_page_id = page_defs[0].id.clone();

    let inherits = edges_from(&ext, "ManageSiteSettings", "inherits");
    assert!(
        !inherits.is_empty(),
        "expected an inherits edge from ManageSiteSettings"
    );
    for e in &inherits {
        assert_eq!(e.target, internal_page_id);
    }

    // The file-level import edge does not target the internal Page.
    let imports = edges_from(&ext, "ManageSiteSettings.php", "imports");
    for e in &imports {
        assert_ne!(e.target, internal_page_id);
        let tgt = node_by_id(&ext, &e.target).expect("import target node");
        assert_eq!(tgt.label, "Filament\\Pages\\Page");
    }
}

// Go binds the `Page` of the importing file's directory, not the one the `use` names.
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_ambiguous_base_disambiguated_by_use() {
    let ext = run(&[
        (
            "app/Models/Page.php",
            "<?php\nnamespace App\\Models;\nclass Page {}\n",
        ),
        (
            "app/Cms/Page.php",
            "<?php\nnamespace App\\Cms;\nclass Page {}\n",
        ),
        (
            "app/Cms/Editor.php",
            "<?php\nnamespace App\\Cms;\n\
             use App\\Cms\\Page;\n\
             class Editor extends Page {}\n",
        ),
    ]);
    let inherits = edges_from(&ext, "Editor", "inherits");
    assert_eq!(inherits.len(), 1);
    let tgt = node_by_id(&ext, &inherits[0].target).expect("inherits target node");
    assert!(!tgt.source_file.is_empty());
    assert!(tgt.source_file.contains("Cms") && !tgt.source_file.contains("Models"));
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_aliased_import_edge_shares_target_with_plain_import() {
    let ext = run(&[
        (
            "src/A.php",
            "<?php\nnamespace App;\n\
             use GuzzleHttp\\Client;\n\
             class A { public function __construct(private Client $client) {} }\n",
        ),
        (
            "src/B.php",
            "<?php\nnamespace App;\n\
             use GuzzleHttp\\Client as HttpClient;\n\
             class B { public function __construct(private HttpClient $client) {} }\n",
        ),
    ]);
    let a = edges_from(&ext, "A.php", "imports");
    let b = edges_from(&ext, "B.php", "imports");
    assert!(!a.is_empty() && !b.is_empty());
    assert_eq!(
        a[0].target, b[0].target,
        "plain and aliased imports of the same class split into different targets"
    );
    let tgt = node_by_id(&ext, &a[0].target).expect("import target node");
    assert_eq!(tgt.label, "GuzzleHttp\\Client");
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_distinct_aliases_for_same_named_classes_do_not_collapse() {
    let ext = run(&[(
        "src/A.php",
        "<?php\nnamespace App;\n\
         use App\\Models\\Session as ModelSession;\n\
         use Shopify\\Auth\\Session as ShopifySession;\n\
         class A {\n\
         \x20   public function __construct(private ModelSession $m, private ShopifySession $s) {}\n\
         }\n",
    )]);
    let imports: BTreeSet<&str> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .map(|e| e.target.as_str())
        .collect();
    let labels: BTreeSet<&str> = imports
        .iter()
        .map(|t| {
            node_by_id(&ext, t)
                .expect("import target node")
                .label
                .as_str()
        })
        .collect();
    assert!(labels.contains("App\\Models\\Session"), "{labels:?}");
    assert!(labels.contains("Shopify\\Auth\\Session"), "{labels:?}");
    assert_eq!(
        imports.len(),
        2,
        "distinct aliased classes collapsed onto one target: {imports:?}"
    );
}

// pins Go v0.12.0 (the base `Baz` is a stub node with no source file: `use ... as` is not read); upstream 0.9.107 binds `Baz` to `Foo\Bar` -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_use_alias_resolves() {
    let ext = run(&[
        ("src/Foo/Bar.php", "<?php\nnamespace Foo;\nclass Bar {}\n"),
        (
            "src/App/X.php",
            "<?php\nnamespace App;\n\
             use Foo\\Bar as Baz;\n\
             class X extends Baz {}\n",
        ),
    ]);
    let inherits = edges_from(&ext, "X", "inherits");
    assert!(!inherits.is_empty());
    let tgt = node_by_id(&ext, &inherits[0].target).expect("inherits target node");
    assert_eq!(tgt.label, "Baz");
    assert!(tgt.source_file.is_empty());
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_fully_qualified_base_resolves() {
    let ext = run(&[
        (
            "app/Models/Page.php",
            "<?php\nnamespace App\\Models;\nclass Page {}\n",
        ),
        (
            "app/Http/Y.php",
            "<?php\nnamespace App\\Http;\n\
             class Y extends \\App\\Models\\Page {}\n",
        ),
    ]);
    let inherits = edges_from(&ext, "Y", "inherits");
    assert!(!inherits.is_empty());
    let tgt = node_by_id(&ext, &inherits[0].target).expect("inherits target node");
    assert!(!tgt.source_file.is_empty());
    assert!(tgt.source_file.contains("Models"));
}

// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_plain_no_namespace_inheritance_preserved() {
    let ext = run(&[
        ("src/Base.php", "<?php\nclass Base {}\n"),
        ("src/Child.php", "<?php\nclass Child extends Base {}\n"),
    ]);
    let inherits: Vec<&Edge> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "inherits")
        .collect();
    assert!(!inherits.is_empty());
    let tgt = node_by_id(&ext, &inherits[0].target).expect("inherits target node");
    assert!(
        !tgt.source_file.is_empty(),
        "no-namespace inheritance must still resolve to the real Base def"
    );
    assert_eq!(tgt.label, "Base");
}

// pins Go v0.12.0 (the `use` is an import of an external node `App\Entities\Pivot`, never of the corpus class `Pivot`); upstream 0.9.107 targets the `Pivot` class node -> P3
// php test (ihc.34): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_php_import_resolves_when_target_name_prefixes_sibling_classes() {
    let mut srcs: Vec<(String, String)> = vec![
        (
            "src/Pivot.php".to_string(),
            "<?php\nnamespace App\\Entities;\nclass Pivot {}\n".to_string(),
        ),
        (
            "src/ModelWithRelation.php".to_string(),
            "<?php\nnamespace App\\Models;\n\
             use App\\Entities\\Pivot;\n\
             class ModelWithRelation {}\n"
                .to_string(),
        ),
    ];
    for name in [
        "PivotRepository",
        "PivotRepositoryEloquent",
        "PivotValidator",
    ] {
        srcs.push((
            format!("src/{name}.php"),
            format!("<?php\nnamespace App\\Repositories;\nclass {name} {{}}\n"),
        ));
    }
    let srcs: Vec<(&str, &str)> = srcs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let ext = run(&srcs);
    let pivot_id = class_defs(&ext, "Pivot")[0].id.clone();
    let imports = edges_from(&ext, "ModelWithRelation.php", "imports");
    assert_eq!(imports.len(), 1);
    assert_ne!(imports[0].target, pivot_id);
    let tgt = node_by_id(&ext, &imports[0].target).expect("import target node");
    assert_eq!(tgt.label, "App\\Entities\\Pivot");
    assert!(tgt.source_file.is_empty());
}
