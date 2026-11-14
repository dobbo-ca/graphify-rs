//! graphify v0.9.107 `test_cross_language_call_resolution.py` (4 tests): 3 ported, 1 not.
//!
//! Not ported:
//! - `test_same_language_callback_still_resolves`: asserts an INFERRED `indirect_call` edge for
//!   a TS callback passed by name; graphify-go has no `indirect_call` relation.
//!
//! The call edges are asserted by label, as `langfamily.Cross` (`resolve.go:860`) decides them:
//! a call never binds to a definition of another family, and Kotlin and Java share one.

use crate::extract::kinds::register_kinds;
use crate::extract::tests::util::extract_all;

const KOTLIN_BRIDGE: &str = "class HeadingSensorBridge {\n    fun refreshHeading() {\n        println(\"native sensor\")\n    }\n}\n";

/// Go has no `indirect_call`, so a callback by name gives no edge at all; upstream's assertion is
/// the same: no `calls` edge lands on a label with `needle`.
fn no_call_to(srcs: &[(&str, &str)], needle: &str) {
    register_kinds();
    let edges = extract_all(srcs);
    assert!(
        !edges
            .iter()
            .any(|(_, rel, tgt)| rel == "calls" && tgt.contains(needle)),
        "{edges:?}"
    );
}

// A TSX callback passed by name never binds to the lone same-named Kotlin method.
#[test]
#[ignore = "graphify-rs-ihc.37"] // also needs resolve (ihc.18)
fn test_tsx_callback_does_not_bind_to_kotlin_method() {
    no_call_to(
        &[
            (
                "web/Upcoming.tsx",
                "declare function register(cb: () => void): void;\nexport function UpcomingPanel() {\n  register(refreshHeading);\n  return null;\n}\n",
            ),
            ("android/HeadingSensorBridge.kt", KOTLIN_BRIDGE),
        ],
        "refreshHeading",
    );
}

// A bare Python call never binds to the lone same-named Kotlin function.
#[test]
#[ignore = "graphify-rs-ihc.37"] // also needs resolve (ihc.18)
fn test_python_call_does_not_bind_to_kotlin_function() {
    no_call_to(
        &[
            (
                "py/worker.py",
                "def process():\n    return refreshHeading()\n",
            ),
            ("android/HeadingSensorBridge.kt", KOTLIN_BRIDGE),
        ],
        "refreshHeading",
    );
}

// Kotlin and Java share the JVM: a Kotlin call to a Java method still resolves.
#[test]
#[ignore = "graphify-rs-ihc.37"] // also needs resolve (ihc.18)
fn test_jvm_interop_kotlin_call_to_java_still_resolves() {
    register_kinds();
    let edges = extract_all(&[
        (
            "Alarm.java",
            "public class Alarm {\n    public static void ring() { System.out.println(\"ring\"); }\n}\n",
        ),
        ("Scheduler.kt", "fun schedule() {\n    ring()\n}\n"),
    ]);
    assert!(
        edges
            .iter()
            .any(|(_, rel, tgt)| rel == "calls" && tgt.contains("ring")),
        "{edges:?}"
    );
}
