//! graphify-go `internal/extract` `TestJSDynamicImportLines` (a jsts test file): its subject is
//! `dynImport` (`extract.go:554`), so it is an extract-core test. Ported by the runtime stream
//! (handoff branch handoff-js-resolve-tests-5e21); it parses JS, so it waits for ihc.26.

use crate::extract::file_from_bytes;

// Assumed item, added by the extractors-and-CLI stream (Go `dynImport`, `extract.go:554`, the
// `Imp.Dyn` value of a file's dynamic_import; `dynDeferred` is 2). `Imp.dynamic` is already `u8`:
//   pub(crate) const DYN_IMPORT: u8 = 1; // in crate::extract::extract
// The regex pass tracks lines and `//` per line while it scans once.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_dynamic_import_lines() {
    crate::extract::kinds::register_kinds();
    let src = "import('./a');\n\nconst x = 1; import('./b'); // import('./c')\n// import('./d')\nf(import('./e'), import('./f'));\n";
    let mut got: Vec<String> = Vec::new();
    for im in &file_from_bytes("m.ts", src.as_bytes()).imps {
        if im.dynamic == crate::extract::extract::DYN_IMPORT {
            got.push(format!("{}@{}", im.spec, im.loc));
        }
    }
    let want = "./a@L1 ./b@L3 ./e@L5 ./f@L5";
    assert_eq!(got.join(" "), want, "got {got:?}, want {want}");
}
