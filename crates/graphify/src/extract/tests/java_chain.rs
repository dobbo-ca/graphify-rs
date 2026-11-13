//! graphify-go `internal/extract/java_chain_test.go`.

use std::collections::BTreeMap;

use crate::extract::{file_from_bytes, resolve};
use crate::model::{Confidence, Relation};

type Srcs = BTreeMap<String, String>;
type Edge3 = (&'static str, &'static str, &'static str);

fn srcs_of(pairs: &[(&str, &str)]) -> Srcs {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Go `extractAll` (`inherits_test.go`): `FileFromBytes` on each source, `Resolve`, and the
/// edges as (source label, relation, target label).
fn extract_all(srcs: &Srcs) -> Vec<(String, String, String)> {
    crate::extract::kinds::register_kinds();
    let mut results = Vec::new();
    let mut files = Vec::new();
    for (rel, src) in srcs {
        results.push(file_from_bytes(rel, src.as_bytes()));
        files.push(rel.clone());
    }
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    let label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    ext.edges
        .iter()
        .map(|e| {
            (
                label
                    .get(e.source.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_string(),
                e.relation.as_str().to_string(),
                label
                    .get(e.target.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .collect()
}

/// Go `wantEdges` (`inherits_test.go`).
fn want_edges(srcs: &Srcs, want: &[Edge3], unwanted: &[Edge3]) {
    let got = extract_all(srcs);
    let has = |w: &Edge3| got.iter().any(|e| e.0 == w.0 && e.1 == w.1 && e.2 == w.2);
    for w in want {
        assert!(
            has(w),
            "missing edge {} --{}--> {}; got {:?}",
            w.0,
            w.1,
            w.2,
            got
        );
    }
    for w in unwanted {
        assert!(!has(w), "unexpected edge {} --{}--> {}", w.0, w.1, w.2);
    }
}

// javaBuilder names its steps like methods of an unrelated class, so only
// the declared return types can pick the right ones.
fn java_builder(client: &str) -> Srcs {
    srcs_of(&[
        ("Product.java", "class Product { void ship() {} }\n"),
        (
            "Builder.java",
            "class Builder {\n    Builder size(int n) { return this; }\n    Product build() { return new Product(); }\n}\n",
        ),
        (
            "Factory.java",
            "class Factory { static Builder builder() { return new Builder(); } }\n",
        ),
        (
            "Decoy.java",
            "class Decoy { Decoy size(int n) { return this; } void build() {} void ship() {} }\n",
        ),
        ("Client.java", client),
    ])
}

#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_typed_from_return_types() {
    want_edges(
        &java_builder(
            "class Client { void run() { Factory.builder().size(3).build().ship(); } }\n",
        ),
        &[
            ("Client.run()", "calls", "Builder.size()"),
            ("Client.run()", "calls", "Builder.build()"),
            ("Client.run()", "calls", "Product.ship()"),
        ],
        &[
            ("Client.run()", "calls", "Decoy.size()"),
            ("Client.run()", "calls", "Decoy.build()"),
            ("Client.run()", "calls", "Decoy.ship()"),
        ],
    );
}

#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_new_cast_bare_this_super_receivers() {
    want_edges(
        &java_builder(
            "class Client {\n\
             \x20   Builder mine() { return new Builder(); }\n\
             \x20   void viaNew() { new Builder().size(1); }\n\
             \x20   void viaCast(Object o) { ((Builder) o).build(); }\n\
             \x20   void viaBare() { mine().build(); }\n\
             \x20   void viaField() { this.factory().size(2); }\n\
             \x20   Builder factory() { return Factory.builder(); }\n\
             }\n\
             class Kid extends Builder { void viaSuper() { super.size(1).build(); } }\n",
        ),
        &[
            ("Client.viaNew()", "calls", "Builder.size()"),
            ("Client.viaCast()", "calls", "Builder.build()"),
            ("Client.viaBare()", "calls", "Builder.build()"),
            ("Client.viaField()", "calls", "Builder.size()"),
            ("Kid.viaSuper()", "calls", "Builder.build()"),
        ],
        &[
            ("Client.viaNew()", "calls", "Decoy.size()"),
            ("Client.viaBare()", "calls", "Decoy.build()"),
        ],
    );
}

// Upstream nests each Builder; nested types have no node here.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_return_type_named_in_several_files() {
    crate::extract::kinds::register_kinds();
    let srcs = srcs_of(&[
        (
            "Alpha.java",
            "class Alpha { static Builder builder() { return new Builder(); } }\n\
             class Builder { Alpha get() { return null; } }\n",
        ),
        (
            "Beta.java",
            "class Beta {}\nclass Builder { Beta get() { return null; } }\n",
        ),
        (
            "Client.java",
            "class Client { void run() { Alpha.builder().get(); } }\n",
        ),
    ]);
    let mut results = Vec::new();
    let mut files = Vec::new();
    for (rel, src) in &srcs {
        results.push(file_from_bytes(rel, src.as_bytes()));
        files.push(rel.clone());
    }
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    let file: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.source_file.as_str()))
        .collect();
    let mut got: Vec<&str> = Vec::new();
    for e in &ext.edges {
        if e.relation == Relation::CALLS
            && file.get(e.source.as_str()) == Some(&"Client.java")
            && file.get(e.target.as_str()) != Some(&"Client.java")
        {
            got.push(e.target.as_str());
        }
    }
    let (alpha_get, beta_get) = ("alpha_java_builder_get", "beta_java_builder_get");
    assert!(
        got.contains(&alpha_get) && !got.contains(&beta_get),
        "Client.run calls {got:?}; want {alpha_get} and not {beta_get}"
    );
}

// A type parameter, a library type or an untyped local carries no type.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_untyped_link_stays_unlinked() {
    let mut srcs = java_builder(
        "class Client<T> {\n\
         \x20   T self() { return null; }\n\
         \x20   String text() { return \"\"; }\n\
         \x20   void viaTypeParam() { self().build(); }\n\
         \x20   void viaLibrary() { text().length(); }\n\
         \x20   void viaVar() { var b = Factory.builder(); b.size(1).build(); }\n\
         }\n",
    );
    srcs.insert(
        "String.java".into(),
        "class String { int length() { return 0; } }\n".into(),
    );
    srcs.insert("T.java".into(), "class T { void build() {} }\n".into());
    want_edges(
        &srcs,
        &[],
        &[
            ("Client.viaLibrary()", "calls", "String.length()"),
            ("Client.viaTypeParam()", "calls", "T.build()"),
            ("Client.viaTypeParam()", "calls", "Builder.build()"),
            ("Client.viaTypeParam()", "calls", "Decoy.build()"),
            ("Client.viaVar()", "calls", "Builder.build()"),
            ("Client.viaVar()", "calls", "Decoy.build()"),
        ],
    );
}

// Upstream types this from the field; no local type table here yet.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_lowercase_root_unlinked() {
    want_edges(
        &srcs_of(&[(
            "Services.java",
            "class Gateway { void charge() {} Gateway create() { return this; } }\n\
             class Checkout { Gateway factory; void chained() { factory.create().charge(); } }\n",
        )]),
        &[],
        &[("Checkout.chained()", "calls", "Gateway.charge()")],
    );
}

#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_overloads_must_agree() {
    want_edges(
        &java_builder(
            "class Client {\n\
             \x20   Builder same() { return null; }\n\
             \x20   Builder same(int n) { return null; }\n\
             \x20   Product mixed(int n) { return null; }\n\
             \x20   Builder mixed() { return null; }\n\
             \x20   void agree() { same().build(); }\n\
             \x20   void disagree() { mixed().build(); }\n\
             }\n",
        ),
        &[("Client.agree()", "calls", "Builder.build()")],
        &[("Client.disagree()", "calls", "Builder.build()")],
    );
}

// One base of another language stops the bases walk.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_non_jvm_base_binds_nothing() {
    want_edges(
        &srcs_of(&[
            ("base.py", "class Base:\n    def run(self):\n        pass\n"),
            (
                "Sub.java",
                "interface JRun { void run(); }\n\
                 interface Sub extends Base, JRun {}\n\
                 interface JSub extends JRun {}\n",
            ),
            (
                "Client.java",
                "class Client { void viaPy(Object o) { ((Sub) o).run(); } void viaJava(Object o) { ((JSub) o).run(); } }\n",
            ),
        ]),
        &[("Client.viaJava()", "calls", "JRun.run()")],
        &[
            ("Client.viaPy()", "calls", "JRun.run()"),
            ("Client.viaPy()", "calls", "Base.run()"),
        ],
    );
}

// A builtin base gets no upstream edge, so the walk goes on.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_builtin_base_is_transparent() {
    want_edges(
        &srcs_of(&[
            ("J.java", "interface J { void jm(); }\n"),
            ("I.java", "interface I extends J, Comparable<I> {}\n"),
            (
                "Use.java",
                "class Use { static I i() { return null; } void u() { i().jm(); } }\n",
            ),
        ]),
        &[("Use.u()", "calls", "J.jm()")],
        &[],
    );
}

// Upstream's caller there is the inner method.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_in_anonymous_class_unlinked() {
    want_edges(
        &srcs_of(&[
            (
                "Types.java",
                "class Tool { void go() {} }\nclass Item { void stop() {} }\nclass Other { void stop() {} }\n",
            ),
            (
                "Outer.java",
                "class Outer {\n\
                 \x20   Tool helper() { return null; }\n\
                 \x20   Item item() { return null; }\n\
                 \x20   void start() { Runnable r = new Runnable() {\n\
                 \x20       Other item() { return null; }\n\
                 \x20       public void run() { helper().go(); this.item().stop(); }\n\
                 \x20   }; }\n\
                 }\n",
            ),
        ]),
        &[],
        &[
            ("Outer.start()", "calls", "Tool.go()"),
            ("Outer.start()", "calls", "Item.stop()"),
            ("Outer.start()", "calls", "Other.stop()"),
        ],
    );
}

// An unchanged file comes back from the cache as JSON.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_through_cached_result() {
    crate::extract::kinds::register_kinds();
    let srcs = java_builder("class Client { void run() { Factory.builder().size(3).build(); } }\n");
    let mut results = Vec::new();
    let mut files = Vec::new();
    for (rel, src) in &srcs {
        let mut r = file_from_bytes(rel, src.as_bytes());
        if rel != "Client.java" {
            let mut b = Vec::new();
            r.append_json(&mut b).unwrap();
            r = match gostd::json::unmarshal::<crate::extract::Result>(&b) {
                Ok(r) if r.calls_ok() => r,
                Ok(_) => panic!("decode {rel}: calls not ok"),
                Err(e) => panic!("decode {rel}: {e}"),
            };
        }
        results.push(r);
        files.push(rel.clone());
    }
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);
    for e in &ext.edges {
        if e.source == "client_java_client_run" && e.target == "builder_java_builder_build" {
            assert!(
                e.confidence == Confidence::INFERRED && e.confidence_score == 0.85,
                "edge {e:?}; want INFERRED 0.85"
            );
            return;
        }
    }
    panic!("missing Client.run --calls--> Builder.build after a cache round trip");
}

// Upstream's nested and anonymous Items make the name ambiguous.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_nested_namesake_unlinked() {
    for shadow in [
        "class Outer { static class Item { void run() {} } }\n",
        "class Anon { void a() { new Item() { void run() {} }; } }\n",
    ] {
        want_edges(
            &srcs_of(&[
                ("Item.java", "class Item { void run() {} }\n"),
                ("Shadow.java", shadow),
                ("Use.java", "class Use { void u() { new Item().run(); } }\n"),
                (
                    "Own.java",
                    "class Own { static class Item { void run() {} } void u() { new Item().run(); } }\n",
                ),
            ]),
            &[],
            &[
                ("Use.u()", "calls", "Item.run()"),
                ("Own.u()", "calls", "Item.run()"),
            ],
        );
    }
}

// Upstream binds the nested Builder, which has no node here.
#[test]
#[ignore = "graphify-rs-ihc.27"]
fn test_java_chain_nested_return_type_unlinked() {
    want_edges(
        &srcs_of(&[
            (
                "Builder.java",
                "class Builder { Builder size(int n) { return this; } void build() {} }\n",
            ),
            (
                "Foo.java",
                "class Foo {\n\
                 \x20   static Builder builder() { return null; }\n\
                 \x20   static class Builder { Builder size(int n) { return this; } Foo build() { return null; } }\n\
                 \x20   void make() { builder().size(3).build(); }\n\
                 }\n",
            ),
            (
                "Use.java",
                "class Use { void u() { Foo.builder().size(3).build(); } }\n",
            ),
        ]),
        &[],
        &[
            ("Foo.make()", "calls", "Builder.size()"),
            ("Foo.make()", "calls", "Builder.build()"),
            ("Use.u()", "calls", "Builder.size()"),
            ("Use.u()", "calls", "Builder.build()"),
        ],
    );
}
