//! The generated UCD 17.0.0 tables are present and well formed. Their content is proved by
//! the unicode tests (Task 3) and the oracle sweep against Go.

use gostd::tables::RangeTable;
use gostd::tables::unicode17 as t;

fn contains(tab: &RangeTable, r: u32) -> bool {
    tab.r16.iter().any(|g| {
        (g.lo as u32..=g.hi as u32).contains(&r) && (r - g.lo as u32) % g.stride as u32 == 0
    }) || tab
        .r32
        .iter()
        .any(|g| (g.lo..=g.hi).contains(&r) && (r - g.lo) % g.stride == 0)
}

fn lookup(map: &[(&str, &'static RangeTable)], name: &str) -> &'static RangeTable {
    map.iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("no table {name}"))
}

#[test]
fn version() {
    assert_eq!(t::VERSION, "17.0.0");
}

#[test]
fn maps_are_sorted_and_unique() {
    for map in [
        t::CATEGORIES,
        t::SCRIPTS,
        t::PROPERTIES,
        t::FOLD_CATEGORY,
        t::FOLD_SCRIPT,
    ] {
        assert!(map.windows(2).all(|w| w[0].0 < w[1].0));
    }
    assert!(t::CATEGORY_ALIASES.windows(2).all(|w| w[0].0 < w[1].0));
}

#[test]
fn spot_checks() {
    assert!(contains(lookup(t::CATEGORIES, "Lu"), 'A' as u32));
    assert!(!contains(lookup(t::CATEGORIES, "Lu"), 'a' as u32));
    assert!(contains(lookup(t::CATEGORIES, "Mn"), 0x0301));
    assert!(contains(lookup(t::CATEGORIES, "Nl"), 0x2160));
    assert!(!contains(lookup(t::CATEGORIES, "L"), 0x2160));
    assert!(contains(lookup(t::SCRIPTS, "Greek"), 0x03A3));
    assert!(contains(lookup(t::PROPERTIES, "White_Space"), 0x3000));
    assert!(contains(lookup(t::FOLD_CATEGORY, "Ll"), 0x212A));
    assert!(t::CATEGORY_ALIASES.contains(&("Letter", "L")));
    // A..Z: lower delta 32 (Go CaseRanges[0] is {0x41, 0x5A, d{0, 32, 0}}).
    assert_eq!(t::CASE_RANGES[0].lo, 0x41);
    assert_eq!(t::CASE_RANGES[0].delta, [0, 32, 0]);
    assert!(t::CASE_ORBIT.contains(&(0x4B, 0x6B)));
}

#[test]
fn ranges_are_sorted_and_latin_offset_holds() {
    for (_, tab) in t::CATEGORIES.iter().chain(t::SCRIPTS).chain(t::PROPERTIES) {
        assert!(tab.r16.windows(2).all(|w| w[0].hi < w[1].lo));
        assert!(tab.r32.windows(2).all(|w| w[0].hi < w[1].lo));
        assert_eq!(
            tab.latin_offset,
            tab.r16.iter().take_while(|g| g.hi <= 0xFF).count()
        );
    }
}
