//! Upstream graphify v0.9.107 `tests/test_ignore_file_encoding.py`: the cases that hold for
//! graphify-go v0.12.0. Go reads an ignore file's bytes as they are (no BOM strip, no UTF-16 or
//! cp1252 decode, no Unicode normalization), so a rule in another encoding matches nothing.
//! Not ported: the two stderr-warning tests and the five direct `_read_ignore_text` tests (no
//! decode step in Go).

use std::collections::BTreeSet;

use super::{collected, must_write};
use crate::testutil::TempDir;

/// `"Orçamento"` in NFC.
const NAME: &str = "Or\u{e7}amento";

/// Upstream `_corpus`: `ignore_bytes` as `.graphifyignore`, `<dirname>/contrato.py` and
/// `main.py`; returns the collected file names (upstream `_scanned`).
fn scanned(ignore_bytes: &[u8], dirname: &str) -> BTreeSet<String> {
    let t = TempDir::new();
    std::fs::write(t.join(".graphifyignore"), ignore_bytes).expect("write");
    must_write(&t.join(dirname).join("contrato.py"), "x = 1");
    must_write(&t.join("main.py"), "print('hi')");
    collected(t.path())
        .into_iter()
        .map(|f| f.rsplit('/').next().unwrap_or_default().to_string())
        .collect()
}

/// `test_ansi_encoded_rule_still_excludes`: a cp1252 rule (`ç` is the byte E7).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn ansi_encoded_rule_still_excludes() {
    let got = scanned(b"Or\xe7amento/\n", NAME);
    // pins Go v0.12.0 (contrato.py collected: the E7 byte matches nothing); upstream 0.9.107 excludes it -> P3
    assert!(got.contains("contrato.py"), "{got:?}");
    assert!(got.contains("main.py"), "{got:?}");
}

/// `test_utf8_rule_is_unaffected`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn utf8_rule_is_unaffected() {
    let got = scanned(format!("{NAME}/\n").as_bytes(), NAME);
    assert!(!got.contains("contrato.py"), "{got:?}");
}

/// `test_ascii_rules_are_untouched`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn ascii_rules_are_untouched() {
    let t = TempDir::new();
    std::fs::write(t.join(".graphifyignore"), b"vendor/\n").expect("write");
    must_write(&t.join("vendor/lib.py"), "x = 1");
    must_write(&t.join("main.py"), "x = 1");
    assert_eq!(collected(t.path()), BTreeSet::from(["main.py".to_string()]));
}

/// `test_utf8_rules_still_match_across_normalisation_forms` (NFC pattern on an NFD dir, NFD
/// pattern on an NFC dir).
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn utf8_rules_still_match_across_normalisation_forms() {
    const NFD: &str = "Orc\u{327}amento";
    for (form, pattern, other) in [("NFC", NAME, NFD), ("NFD", NFD, NAME)] {
        let got = scanned(format!("{pattern}/\n").as_bytes(), other);
        // pins Go v0.12.0 (contrato.py collected: no normalization); upstream 0.9.107 excludes it -> P3
        assert!(got.contains("contrato.py"), "{form}: {got:?}");
    }
}

/// `test_utf16_bom_encoded_rule_still_excludes`: Python's `"utf-16"` codec writes a
/// little-endian BOM.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn utf16_bom_encoded_rule_still_excludes() {
    let mut bytes = vec![0xff, 0xfe];
    for u in format!("{NAME}/\n").encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let got = scanned(&bytes, NAME);
    // pins Go v0.12.0 (contrato.py collected: the UTF-16 rule matches nothing); upstream 0.9.107 excludes it -> P3
    assert!(got.contains("contrato.py"), "{got:?}");
    assert!(got.contains("main.py"), "{got:?}");
}
