//! The generated UCD 15.0.0 x/text tables (crates/gostd-gen src/xtext15.rs). Expected rows are
//! derived by hand from x/text v0.27.0 maketables.go (Properties of the NFKC trie, the
//! recomposition map) and cases/gen.go. Their full content is proved by the xtext oracle sweep.

use gostd::tables::NormProps;
use gostd::tables::{cases15, norm15};

fn props(r: u32) -> Option<NormProps> {
    norm15::NFKC_PROPS
        .binary_search_by_key(&r, |&(k, _)| k)
        .ok()
        .map(|i| norm15::NFKC_PROPS[i].1)
}

fn p(ccc: u8, n_lead: u8, flags: u8, decomp: &'static str) -> Option<NormProps> {
    Some(NormProps {
        ccc,
        n_lead,
        flags,
        decomp,
    })
}

#[test]
fn versions() {
    assert_eq!(norm15::VERSION, "15.0.0");
    assert_eq!(cases15::VERSION, "15.0.0");
}

#[test]
fn tables_sorted() {
    assert!(norm15::NFKC_PROPS.windows(2).all(|w| w[0].0 < w[1].0));
    assert!(norm15::RECOMP.windows(2).all(|w| w[0].0 < w[1].0));
    assert!(cases15::FOLD.windows(2).all(|w| w[0].0 < w[1].0));
    // Hangul syllables are computed, not stored.
    assert!(
        norm15::NFKC_PROPS
            .iter()
            .all(|&(r, _)| !(0xAC00..=0xD7A3).contains(&r))
    );
}

#[test]
fn nfkc_props_rows() {
    assert_eq!(props(0x00A0), p(0, 0, 0x14, "\u{20}"));
    assert_eq!(props(0x00C0), p(0, 0, 0x05, "\u{41}\u{300}"));
    assert_eq!(props(0x0301), p(230, 1, 0x19, ""));
    assert_eq!(props(0x0316), p(220, 1, 0x01, ""));
    assert_eq!(props(0x0344), p(230, 2, 0x16, "\u{308}\u{301}"));
    assert_eq!(props(0x0F73), p(129, 2, 0x16, "\u{F71}\u{F72}"));
    assert_eq!(props(0x1161), p(0, 1, 0x19, ""));
    assert_eq!(props(0x1E9B), p(0, 0, 0x15, "\u{73}\u{307}"));
    assert_eq!(props(0xFF9E), p(8, 1, 0x15, "\u{3099}"));
    assert_eq!(props(u32::from('a')), None);
}

#[test]
fn recomp_rows() {
    let get = |k: u32| {
        norm15::RECOMP
            .binary_search_by_key(&k, |&(key, _)| key)
            .ok()
            .map(|i| norm15::RECOMP[i].1)
    };
    assert_eq!(get(0x0041_0300), Some(0xC0));
    assert_eq!(get(0x017F_0307), Some(0x1E9B));
    assert_eq!(get(0x1099_10BA), Some(0x1109A));
    // U+0F73 decomposes canonically to F71 F72 but is excluded from composition.
    assert_eq!(get(0x0F71_0F72), None);
}

#[test]
fn fold_rows() {
    let get = |r: u32| {
        cases15::FOLD
            .binary_search_by_key(&r, |&(k, _)| k)
            .ok()
            .map(|i| cases15::FOLD[i].1)
    };
    assert_eq!(get(0x41), Some("a"));
    assert_eq!(get(0xDF), Some("ss"));
    assert_eq!(get(0x130), Some("i\u{307}"));
    assert_eq!(get(0x212A), Some("k"));
    assert_eq!(get(0x61), None);
}
