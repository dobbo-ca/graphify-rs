//! Port of `affected_test.go` (8 tests).

use std::collections::BTreeSet;

use super::{AFFECTED_JSON, graph_json, ids, link_json, load_json, node_json};
use crate::query::{AffectedOptions, AffectedResult, affected};

/// Go `inheritsJSON`: module.this lives in child.tf; module.label (in parent.tf) inherits its
/// cloudposse context from module.this. Changing child.tf must flag module.label, whose computed
/// name depends on the context parent.
const INHERITS_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"module_this","label":"module.this [null-label]","file_type":"code","source_file":"child.tf","source_location":"L1"},
 {"id":"module_label","label":"module.label [null-label]","file_type":"code","source_file":"parent.tf","source_location":"L1"}
],
"links":[
 {"source":"module_label","target":"module_this","relation":"inherits_context","confidence":"EXTRACTED"}
],
"hyperedges":[]}"#;

/// `[s]` as Go's `[]string{s}`.
fn files(s: &[&str]) -> Vec<String> {
    s.iter().map(|f| f.to_string()).collect()
}

/// Go `impacted`: the impacted node ids as a set for terse assertions.
fn impacted(res: &AffectedResult) -> BTreeSet<String> {
    res.impacted.iter().map(|n| n.id.clone()).collect()
}

#[test]
fn test_affected_transitive() {
    let g = load_json(AFFECTED_JSON);
    let res = affected(&g, &files(&["util.go"]), &AffectedOptions::default());
    assert_eq!(ids(&res.changed), ["add"], "changed: got {:?}", res.changed);
    let got = impacted(&res);
    assert!(
        got.contains("compute") && got.contains("run"),
        "impacted should include compute and run (transitive callers), got {got:?}"
    );
    assert!(
        !got.contains("lonely"),
        "lonely() is unrelated and must not be impacted"
    );
    assert!(
        !got.contains("add"),
        "changed nodes must not also appear in impacted"
    );
}

#[test]
fn test_affected_inherits_context() {
    let g = load_json(INHERITS_JSON);
    let res = affected(&g, &files(&["child.tf"]), &AffectedOptions::default());
    assert_eq!(
        ids(&res.changed),
        ["module_this"],
        "changed: got {:?}",
        res.changed
    );
    let got = impacted(&res);
    assert!(
        got.contains("module_label"),
        "impacted should include module.label (inherits context from changed module.this), got {got:?}"
    );
}

#[test]
fn test_affected_no_match() {
    let g = load_json(AFFECTED_JSON);
    let res = affected(&g, &files(&["nonexistent.go"]), &AffectedOptions::default());
    assert!(
        res.changed.is_empty() && res.impacted.is_empty(),
        "expected empty result for unknown file, got {res:?}"
    );
}

/// Go `TestAffectedDepth`: changing util.go (add) reaches compute at depth 1 and run at depth 2.
/// Depth 1 keeps only the direct dependent; depth 2 (and unbounded) reach the transitive caller.
#[test]
fn test_affected_depth() {
    let g = load_json(AFFECTED_JSON);
    let depth = |depth| AffectedOptions {
        depth,
        ..AffectedOptions::default()
    };

    let d1 = impacted(&affected(&g, &files(&["util.go"]), &depth(1)));
    assert!(
        d1.contains("compute"),
        "depth 1 should include compute (direct dependent), got {d1:?}"
    );
    assert!(
        !d1.contains("run"),
        "depth 1 must exclude run (2 hops away), got {d1:?}"
    );

    let d2 = impacted(&affected(&g, &files(&["util.go"]), &depth(2)));
    assert!(
        d2.contains("compute") && d2.contains("run"),
        "depth 2 should include compute and run, got {d2:?}"
    );
}

/// Go `TestAffectedRelation`: with a --relation filter, only edges of the named kind are
/// followed. Filtering to "imports" (an unused relation here) yields no impact even though
/// "calls" edges exist.
#[test]
fn test_affected_relation() {
    let g = load_json(AFFECTED_JSON);
    let rel = |r: &str| AffectedOptions {
        relations: files(&[r]),
        ..AffectedOptions::default()
    };

    let calls = impacted(&affected(&g, &files(&["util.go"]), &rel("calls")));
    assert!(
        calls.contains("compute") && calls.contains("run"),
        "relation=calls should include compute and run, got {calls:?}"
    );

    let imports = impacted(&affected(&g, &files(&["util.go"]), &rel("imports")));
    assert!(
        imports.is_empty(),
        "relation=imports should yield no impact (no imports edges), got {imports:?}"
    );
}

/// Go `TestAffectedDynamicImport`: a dynamic import() is a dependency: changing the target
/// reaches the importer.
#[test]
fn test_affected_dynamic_import() {
    let g = load_json(
        r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"lazy","label":"lazy.ts","file_type":"code","source_file":"lazy.ts","source_location":"L1"},
 {"id":"app","label":"app.ts","file_type":"code","source_file":"app.ts","source_location":"L1"}
],
"links":[{"source":"app","target":"lazy","relation":"dynamic_import","confidence":"EXTRACTED"}],
"hyperedges":[]}"#,
    );
    let got = impacted(&affected(
        &g,
        &files(&["lazy.ts"]),
        &AffectedOptions::default(),
    ));
    assert!(
        got.contains("app"),
        "app.ts should be impacted through dynamic_import, got {got:?}"
    );
}

/// Go `TestAffectedSeedPathForms`: a seed naming the same file in bare, "./"-prefixed and
/// absolute form must return the identical result. Go `t.Chdir`s to a temp dir only to know the
/// working directory; the cwd is process-global in Rust (other tests run in parallel), so this
/// reads the test process's own cwd instead, which `affected` sees the same way.
#[test]
fn test_affected_seed_path_forms() {
    let g = load_json(AFFECTED_JSON);
    let cwd = std::env::current_dir().expect("getwd");
    let abs = cwd.join("util.go");
    let abs = abs.to_str().expect("utf-8 cwd");

    for seed in ["util.go", "./util.go", abs] {
        let res = affected(&g, &files(&[seed]), &AffectedOptions::default());
        assert_eq!(
            ids(&res.changed),
            ["add"],
            "seed {seed:?}: changed got {:?}",
            res.changed
        );
        let got = impacted(&res);
        assert!(
            got.contains("compute") && got.contains("run"),
            "seed {seed:?}: impacted got {got:?}, want compute+run"
        );
    }

    // An absolute path outside the working directory is left alone, not turned into a "../.."
    // seed that could collide with a node.
    let res = affected(
        &g,
        &files(&["/elsewhere/util.go"]),
        &AffectedOptions::default(),
    );
    assert!(
        res.changed.is_empty(),
        "outside-cwd seed matched: {:?}",
        res.changed
    );
}

#[test]
fn test_affected_default_follows_inherits() {
    let g = load_json(&graph_json(
        &[
            node_json("base_py", "base.py", "base.py", "L1"),
            node_json("base_py_base", "Base", "base.py", "L1"),
            node_json("base_py_base_run", "Base.run()", "base.py", "L2"),
            node_json("child_py", "child.py", "child.py", "L1"),
            node_json("child_py_child", "Child", "child.py", "L2"),
        ],
        &[
            link_json(
                "base_py",
                "base_py_base",
                "contains",
                "EXTRACTED",
                "base.py",
                "L1",
                "",
            ),
            link_json(
                "base_py_base",
                "base_py_base_run",
                "contains",
                "EXTRACTED",
                "base.py",
                "L2",
                "",
            ),
            link_json(
                "child_py",
                "base_py",
                "imports_from",
                "EXTRACTED",
                "child.py",
                "L1",
                "",
            ),
            link_json(
                "child_py",
                "base_py_base",
                "imports",
                "EXTRACTED",
                "child.py",
                "L1",
                "",
            ),
            link_json(
                "child_py",
                "child_py_child",
                "contains",
                "EXTRACTED",
                "child.py",
                "L2",
                "",
            ),
            link_json(
                "child_py_child",
                "base_py_base",
                "inherits",
                "INFERRED",
                "child.py",
                "L2",
                "",
            ),
        ],
    ));
    let res = affected(&g, &files(&["base.py"]), &AffectedOptions::default());
    let want = "child_py_child,child_py";
    assert_eq!(
        ids(&res.impacted).join(","),
        want,
        "impacted {:?}",
        ids(&res.impacted)
    );
}
