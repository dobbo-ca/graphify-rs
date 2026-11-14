//! graphify v0.9.107 `test_case_sensitive_resolution.py` (4 tests): 1 ported, 3 not.
//!
//! Not ported:
//! - `test_python_Path_does_not_resolve_to_shell_PATH`: asserts the shell `PATH` node and that it
//!   keeps at most one edge (upstream's `defines`); what graphify-go's Bash extractor emits for
//!   `export PATH=...` is not traced.
//! - `test_case_sensitive_cross_file_ref_respects_case`: a Rust `struct Wrap(Path)` reference
//!   must not bind to a `PATH` const; the Rust type-reference extractor is not traced.
//! - `test_php_case_insensitive_resolution_preserved`: graphify-go's `resolve.go` has no case
//!   folding for PHP calls (`strings.ToLower` is used only for extensions and qualifiers), so the
//!   assertion holds only if the PHP extractor folds; not traced.

use crate::extract::kinds::register_kinds;
use crate::extract::tests::util::want_edges;

// An exact-case import still resolves (the same shape as `TestImportGuidedNonAliased`).
#[test]
#[ignore = "graphify-rs-ihc.25"] // also needs resolve (ihc.18)
fn test_exact_case_cross_file_still_resolves() {
    register_kinds();
    want_edges(
        &[
            ("h.py", "def helper():\n    return 1\n"),
            (
                "m.py",
                "from h import helper\ndef go():\n    return helper()\n",
            ),
        ],
        &[["go()", "calls", "helper()"]],
        &[],
    );
}
