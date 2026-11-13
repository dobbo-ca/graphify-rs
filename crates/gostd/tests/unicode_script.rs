//! Go source: src/unicode/script_test.go (Go 1.27.4)
//! TestCategories: ported
//! TestProperties: ported
//! Extra (not in Go): lookups_match_maps
//! Rows not ported: 1 (0xdb80 "Cs" is a surrogate; Cs is marked tested via a lookup check).

use gostd::unicode::{
    CATEGORIES, FOLD_CATEGORY, FOLD_SCRIPT, PROPERTIES, SCRIPTS, category, category_alias,
    fold_category, fold_script, is, property, script,
};
use std::collections::HashSet;

const IN_CATEGORY_TEST: &[(char, &str)] = &[
    ('\u{0081}', "Cc"),
    ('\u{200b}', "Cf"),
    ('\u{f0000}', "Co"),
    // not ported: 0xdb80 "Cs" is a surrogate, not a Rust char
    ('\u{0236}', "Ll"),
    ('\u{1d9d}', "Lm"),
    ('\u{07cf}', "Lo"),
    ('\u{1f8a}', "Lt"),
    ('\u{03ff}', "Lu"),
    ('\u{0bc1}', "Mc"),
    ('\u{20df}', "Me"),
    ('\u{07f0}', "Mn"),
    ('\u{1bb2}', "Nd"),
    ('\u{10147}', "Nl"),
    ('\u{2478}', "No"),
    ('\u{fe33}', "Pc"),
    ('\u{2011}', "Pd"),
    ('\u{301e}', "Pe"),
    ('\u{2e03}', "Pf"),
    ('\u{2e02}', "Pi"),
    ('\u{0022}', "Po"),
    ('\u{2770}', "Ps"),
    ('\u{00a4}', "Sc"),
    ('\u{a711}', "Sk"),
    ('\u{25f9}', "Sm"),
    ('\u{2108}', "So"),
    ('\u{2028}', "Zl"),
    ('\u{2029}', "Zp"),
    ('\u{202f}', "Zs"),
    // Unifieds.
    ('\u{04aa}', "L"),
    ('\u{0009}', "C"),
    ('\u{1712}', "M"),
    ('\u{0031}', "N"),
    ('\u{00bb}', "P"),
    ('\u{00a2}', "S"),
    ('\u{00a0}', "Z"),
    ('\u{0065}', "LC"),
    // Unassigned
    ('\u{0378}', "Cn"),
    ('\u{0378}', "C"),
];

const IN_PROP_TEST: &[(char, &str)] = &[
    ('\u{0046}', "ASCII_Hex_Digit"),
    ('\u{200f}', "Bidi_Control"),
    ('\u{2212}', "Dash"),
    ('\u{e0001}', "Deprecated"),
    ('\u{00b7}', "Diacritic"),
    ('\u{30fe}', "Extender"),
    ('\u{ff46}', "Hex_Digit"),
    ('\u{2e17}', "Hyphen"),
    ('\u{2ffb}', "IDS_Binary_Operator"),
    ('\u{2ff3}', "IDS_Trinary_Operator"),
    ('\u{fa6a}', "Ideographic"),
    ('\u{200d}', "Join_Control"),
    ('\u{0ec4}', "Logical_Order_Exception"),
    ('\u{2ffff}', "Noncharacter_Code_Point"),
    ('\u{065e}', "Other_Alphabetic"),
    ('\u{2065}', "Other_Default_Ignorable_Code_Point"),
    ('\u{0bd7}', "Other_Grapheme_Extend"),
    ('\u{0387}', "Other_ID_Continue"),
    ('\u{212e}', "Other_ID_Start"),
    ('\u{2094}', "Other_Lowercase"),
    ('\u{2040}', "Other_Math"),
    ('\u{216f}', "Other_Uppercase"),
    ('\u{0027}', "Pattern_Syntax"),
    ('\u{0020}', "Pattern_White_Space"),
    ('\u{06dd}', "Prepended_Concatenation_Mark"),
    ('\u{300d}', "Quotation_Mark"),
    ('\u{2ef3}', "Radical"),
    ('\u{1f1ff}', "Regional_Indicator"),
    ('\u{061f}', "STerm"), // Deprecated alias of Sentence_Terminal
    ('\u{061f}', "Sentence_Terminal"),
    ('\u{2071}', "Soft_Dotted"),
    ('\u{003a}', "Terminal_Punctuation"),
    ('\u{9fc3}', "Unified_Ideograph"),
    ('\u{fe0f}', "Variation_Selector"),
    ('\u{0020}', "White_Space"),
    ('\u{221e}', "ID_Compat_Math_Start"),
    ('\u{06e3}', "Modifier_Combining_Mark"),
    ('\u{2080}', "ID_Compat_Math_Continue"),
    ('\u{2ffe}', "IDS_Unary_Operator"),
];

#[test]
#[ignore = "graphify-rs-aa6.3"]
fn test_categories() {
    let mut not_tested: HashSet<&str> = CATEGORIES.iter().map(|(k, _)| *k).collect();
    for &(r, name) in IN_CATEGORY_TEST {
        let t = category(name).unwrap_or_else(|| panic!("{name} not a known category"));
        assert!(is(t, r), "IsCategory({r:?}, {name}) = false, want true");
        not_tested.remove(name);
    }
    // not ported row: 0xdb80 "Cs" (surrogate); the category must still exist.
    assert!(category("Cs").is_some(), "Cs not a known category");
    not_tested.remove("Cs");
    assert!(
        not_tested.is_empty(),
        "categories not tested: {not_tested:?}"
    );
}

#[test]
#[ignore = "graphify-rs-aa6.3"]
fn test_properties() {
    let mut not_tested: HashSet<&str> = PROPERTIES.iter().map(|(k, _)| *k).collect();
    for &(r, name) in IN_PROP_TEST {
        let t = property(name).unwrap_or_else(|| panic!("{name} not a known prop"));
        assert!(is(t, r), "IsCategory({r:?}, {name}) = false, want true");
        not_tested.remove(name);
    }
    assert!(
        not_tested.is_empty(),
        "properties not tested: {not_tested:?}"
    );
}

#[test]
#[ignore = "graphify-rs-aa6.3"]
fn lookups_match_maps() {
    type Lookup = fn(&str) -> Option<&'static gostd::unicode::RangeTable>;
    let maps: [(_, Lookup); 5] = [
        (CATEGORIES, category),
        (SCRIPTS, script),
        (PROPERTIES, property),
        (FOLD_CATEGORY, fold_category),
        (FOLD_SCRIPT, fold_script),
    ];
    for (map, lookup) in maps {
        for &(name, table) in map {
            let got = lookup(name).unwrap_or_else(|| panic!("lookup({name}) = None"));
            assert!(
                std::ptr::eq(got, table),
                "lookup({name}) returned a different table"
            );
        }
    }
    assert!(category("Nope").is_none());
    assert_eq!(category_alias("Letter"), Some("L"));
}
