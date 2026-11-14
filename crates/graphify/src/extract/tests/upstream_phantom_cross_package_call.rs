//! graphify v0.9.107 `tests/test_phantom_cross_package_call.py` (5 tests): 2 ported, 3 not.
//!
//! Not ported:
//! - `test_unimported_cross_package_call_emits_no_edge` and
//!   `test_many_files_do_not_collapse_onto_one_export`: upstream gates JS/TS cross-file calls on
//!   import evidence. graphify-go's `disambiguate` (`resolve.go:1585`) lets one candidate win
//!   outright and gates a JS/TS bare call only on `owner`/`export` (`resolve.go:782`); whether
//!   the unimported `export function validate` is a candidate is not traced, so the assertion
//!   is not known to hold.
//! - `test_imported_cross_file_call_still_resolves`: also asserts the edge is EXTRACTED; Go's
//!   JS/TS import-bound call confidence is not traced.

use crate::extract::kinds::register_kinds;
use crate::extract::tests::util::want_edges;

#[test]
#[ignore = "graphify-rs-ihc.26"] // also needs resolve (ihc.18)
fn test_same_file_call_unaffected() {
    register_kinds();
    want_edges(
        &[(
            "s.ts",
            "function helper() { return 1; }\nexport function main() { return helper(); }\n",
        )],
        &[["main()", "calls", "helper()"]],
        &[],
    );
}

// The JS/TS gate is not applied to other languages: Ruby calls a lone same-named function across
// files without an import.
#[test]
#[ignore = "graphify-rs-ihc.33"] // also needs resolve (ihc.18)
fn test_non_js_single_candidate_cross_file_still_resolves() {
    register_kinds();
    want_edges(
        &[
            ("helper.rb", "def transform(data)\n  data.upcase\nend\n"),
            ("main.rb", "def handle(v)\n  transform(v)\nend\n"),
        ],
        &[["handle()", "calls", "transform()"]],
        &[],
    );
}
