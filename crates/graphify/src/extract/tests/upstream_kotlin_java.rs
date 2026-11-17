//! upstream graphify v0.9.107 `test_kotlin_java_call_edges_4935.py` (17 tests), ported for
//! graphify-go v0.12.0: 11 ported, 6 not.
//!
//! Upstream names the nodes of a method `.name()`; Go labels them `Owner.name()`. The pairs are
//! `"<source file stem>:<label>"` as upstream's `_calls` builds them.
//!
//! Go binds a call by name (`resolve.go:774-853`): Java and Kotlin overloads share one node id,
//! and the name index holds an id once, so an overloaded member binds like any other. Go has no
//! guard for a class with an out-of-corpus base, so a bare call there binds to the only
//! definition of the name in the corpus.
//!
//! Not ported, all "Go lacks it" (B43: Go reads no property initializer and no `init` block, so no
//! call of either has an edge): `test_a_property_initializer_call_still_resolves`,
//! `test_an_init_block_call_attaches_to_its_class`,
//! `test_an_init_block_call_to_an_overloaded_method_resolves`,
//! `test_a_class_with_a_property_and_an_init_block_gets_both_edges`,
//! `test_an_init_block_is_attributed_like_a_property_initializer_in_a_nested_class`,
//! `test_object_and_companion_init_blocks_produce_an_edge`.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file_from_bytes, kinds, resolve};

const LOGGER_JAVA: &str = "package util;
public class Logger {
    public static int single(String t) { return 0; }
    public static int overloaded(String t) { return 0; }
    public static int overloaded(String t, String m) { return 0; }
}
";
const PREFS_JAVA: &str = "package util;
public class Prefs { public String getString(String k) { return k; } }
";
const SETTINGS_KT: &str = "package util
object Settings {
    fun get(k: String): String = k
    fun get(k: String, d: String): String = d
    fun fetchValue(k: String): String = k
}
";

/// Upstream `_calls`: the `calls` edges as `("Stem:label", "Stem:label")` pairs.
fn calls(files: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    kinds::register_kinds();
    let results: Vec<_> = files
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let rels: Vec<String> = files.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &rels, workers);
    let at: BTreeMap<&str, String> = ext
        .nodes
        .iter()
        .map(|n| {
            let base = gostd::filepath::base(&n.source_file);
            let stem = base
                .strip_suffix(gostd::filepath::ext(&base))
                .unwrap_or(&base)
                .to_string();
            (n.id.as_str(), format!("{stem}:{}", n.label))
        })
        .collect();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == "calls")
        .filter_map(|e| {
            Some((
                at.get(e.source.as_str())?.clone(),
                at.get(e.target.as_str())?.clone(),
            ))
        })
        .collect()
}

fn pair(s: &str, t: &str) -> (String, String) {
    (s.to_string(), t.to_string())
}

// ---- controls ----

#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_kotlin_call_to_a_non_overloaded_java_method_resolves() {
    let edges = calls(&[
        ("util/Logger.java", LOGGER_JAVA),
        (
            "app/Caller.kt",
            "package app\nimport util.Logger\nclass Caller { fun go() { Logger.single(\"a\") } }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Caller:Caller.go()", "Logger:Logger.single()")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_two_classes_with_the_same_name_stay_ambiguous() {
    let edges = calls(&[
        (
            "a/Dup.java",
            "package a;\npublic class Dup { public static void ping() {} }\n",
        ),
        (
            "b/Dup.java",
            "package b;\npublic class Dup { public static void ping() {} }\n",
        ),
        (
            "app/Caller.kt",
            "package app\nimport a.Dup\nclass Caller { fun go() { Dup.ping() } }\n",
        ),
    ]);
    assert!(
        !edges.iter().any(|(s, _)| s.starts_with("Caller")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_inherited_in_repo_members_still_resolve() {
    let edges = calls(&[
        (
            "util/RepoBase.kt",
            "package util\nopen class RepoBase { fun baseHelper() {} }\n",
        ),
        (
            "util/JavaBase.java",
            "package util;\npublic class JavaBase { public void baseJava() {} }\n",
        ),
        (
            "app/KChild.kt",
            "package app\nimport util.RepoBase\nclass KChild : RepoBase() { fun t() = baseHelper() }\n",
        ),
        (
            "app/JChild.java",
            "package app;\nimport util.JavaBase;\npublic class JChild extends JavaBase { public void t() { baseJava(); } }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("KChild:KChild.t()", "RepoBase:RepoBase.baseHelper()")),
        "{edges:?}"
    );
    assert!(
        edges.contains(&pair("JChild:JChild.t()", "JavaBase:JavaBase.baseJava()")),
        "{edges:?}"
    );
}

// ---- overloads ----

#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_kotlin_call_to_an_overloaded_java_method_resolves() {
    let edges = calls(&[
        ("util/Logger.java", LOGGER_JAVA),
        (
            "app/Caller.kt",
            "package app\nimport util.Logger\nclass Caller { fun go() { Logger.overloaded(\"a\") } }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Caller:Caller.go()", "Logger:Logger.overloaded()")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // kotlin test (ihc.37): needs resolve
fn test_kotlin_call_to_an_overloaded_kotlin_object_method_resolves() {
    let edges = calls(&[
        ("util/Settings.kt", SETTINGS_KT),
        (
            "app/Caller.kt",
            "package app\nimport util.Settings\nclass Caller { fun go() { Settings.get(\"a\") } }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Caller:Caller.go()", "Settings:Settings.get()")),
        "{edges:?}"
    );
}

// ---- inherited external methods ----

// pins Go v0.12.0 (the bare call binds to the only `getString`); upstream 0.9.107 emits no edge -> P3 (B25)
#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_an_external_base_does_not_bind_a_bare_call_to_an_unrelated_class() {
    let edges = calls(&[
        ("util/Prefs.java", PREFS_JAVA),
        (
            "app/Screen.kt",
            "package app\nimport android.app.Activity\nclass Screen : Activity() { fun title(): String = getString(1) }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Screen:Screen.title()", "Prefs:Prefs.getString()")),
        "{edges:?}"
    );
}

// pins Go v0.12.0 (the bare call binds to the only `getString`); upstream 0.9.107 emits no edge -> P3 (B25)
#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_a_java_class_with_an_external_base_gets_the_same_guard() {
    let edges = calls(&[
        ("util/Prefs.java", PREFS_JAVA),
        (
            "app/Screen.java",
            "package app;\nimport android.app.Activity;\npublic class Screen extends Activity { public void t() { getString(1); } }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Screen:Screen.t()", "Prefs:Prefs.getString()")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_an_in_repo_base_that_extends_an_external_class_still_taints_its_children() {
    let edges = calls(&[
        ("util/Prefs.java", PREFS_JAVA),
        (
            "util/BaseScreen.kt",
            "package util\nimport android.app.Activity\nopen class BaseScreen : Activity() { fun baseHelper() {} }\n",
        ),
        (
            "app/Screen.kt",
            "package app\nimport util.BaseScreen\nclass Screen : BaseScreen() {\n    fun a() { baseHelper() }\n    fun b(): String = getString(1)\n}\n",
        ),
    ]);
    assert!(
        edges.contains(&pair(
            "Screen:Screen.a()",
            "BaseScreen:BaseScreen.baseHelper()"
        )),
        "in-repo member kept: {edges:?}"
    );
    // pins Go v0.12.0 (the bare call binds to the only `getString`); upstream 0.9.107 emits no edge -> P3 (B25)
    assert!(
        edges.contains(&pair("Screen:Screen.b()", "Prefs:Prefs.getString()")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // kotlin test (ihc.37): needs resolve
fn test_a_top_level_function_is_still_called_from_a_class_with_an_external_base() {
    let edges = calls(&[
        ("util/Top.kt", "package util\nfun topHelper() {}\n"),
        (
            "app/Screen.kt",
            "package app\nimport android.app.Activity\nimport util.topHelper\nclass Screen : Activity() { fun t() = topHelper() }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Screen:Screen.t()", "Top:topHelper()")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // kotlin test (ihc.37): needs resolve
fn test_a_member_imported_by_name_survives_an_external_base_in_kotlin() {
    let edges = calls(&[
        ("util/Settings.kt", SETTINGS_KT),
        (
            "app/Screen.kt",
            "package app\nimport android.app.Activity\nimport util.Settings.fetchValue\nclass Screen : Activity() { fun t() = fetchValue(\"k\") }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Screen:Screen.t()", "Settings:Settings.fetchValue()")),
        "{edges:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.27"] // kotlin-java test (ihc.37): also needs resolve (ihc.18)
fn test_a_member_imported_by_name_survives_an_external_base_in_java() {
    let edges = calls(&[
        (
            "util/JSettings.java",
            "package util;\npublic class JSettings { public static void helperJ() {} }\n",
        ),
        (
            "app/Screen.java",
            "package app;\nimport android.app.Activity;\nimport static util.JSettings.helperJ;\npublic class Screen extends Activity { public void t() { helperJ(); } }\n",
        ),
    ]);
    assert!(
        edges.contains(&pair("Screen:Screen.t()", "JSettings:JSettings.helperJ()")),
        "{edges:?}"
    );
}
