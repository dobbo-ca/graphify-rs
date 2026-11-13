//! Go source: src/unicode/letter_test.go (Go 1.27.4)
//!
//! - TestIsLetter: ported
//! - TestIsUpper: ported
//! - TestTo: ported
//! - TestToUpperCase: ported
//! - TestToLowerCase: ported
//! - TestToTitleCase: ported
//! - TestIsSpace: ported
//! - TestLetterOptimizations: ported (IsLower/IsTitle checks left out: not in the gostd API)
//! - TestTurkishCase: not ported: SpecialCase is not reached by graphify-go
//! - TestSimpleFold: ported (the SimpleFold(-42) check left out: Rust char cannot be negative)
//! - TestCalibrate: not ported: Go perf calibration
//! - TestLatinOffset: ported
//! - TestSpecialCaseNoMapping: not ported: SpecialCase is not reached by graphify-go
//! - TestNegativeRune: not ported: Rust char cannot be negative
//!
//! Rows left out of tables: 4 (caseTest: `{-1, '\n', 0xFFFD}` invalid case, `{UpperCase, -1, -1}`
//! negative rune, `{UpperCase, 1 << 30, 1 << 30}` rune > 0x10FFFF; simpleFold: SimpleFold(-42)).

use gostd::strings;
use gostd::unicode::*;

const UPPER_TEST: [char; 22] = [
    '\u{41}',
    '\u{c0}',
    '\u{d8}',
    '\u{100}',
    '\u{139}',
    '\u{14a}',
    '\u{178}',
    '\u{181}',
    '\u{376}',
    '\u{3cf}',
    '\u{13bd}',
    '\u{1f2a}',
    '\u{2102}',
    '\u{2c00}',
    '\u{2c10}',
    '\u{2c20}',
    '\u{a650}',
    '\u{a722}',
    '\u{ff3a}',
    '\u{10400}',
    '\u{1d400}',
    '\u{1d7ca}',
];
const NOT_UPPER_TEST: [char; 11] = [
    '\u{40}',
    '\u{5b}',
    '\u{61}',
    '\u{185}',
    '\u{1b0}',
    '\u{377}',
    '\u{387}',
    '\u{2150}',
    '\u{ab7d}',
    '\u{ffff}',
    '\u{10000}',
];
const LETTER_TEST: [char; 32] = [
    '\u{41}',
    '\u{61}',
    '\u{aa}',
    '\u{ba}',
    '\u{c8}',
    '\u{db}',
    '\u{f9}',
    '\u{2ec}',
    '\u{535}',
    '\u{620}',
    '\u{6e6}',
    '\u{93d}',
    '\u{a15}',
    '\u{b99}',
    '\u{dc0}',
    '\u{edd}',
    '\u{1000}',
    '\u{1200}',
    '\u{1312}',
    '\u{1401}',
    '\u{2c00}',
    '\u{a800}',
    '\u{f900}',
    '\u{fa30}',
    '\u{ffda}',
    '\u{ffdc}',
    '\u{10000}',
    '\u{10300}',
    '\u{10400}',
    '\u{20000}',
    '\u{2f800}',
    '\u{2fa1d}',
];
const NOT_LETTER_TEST: [char; 9] = [
    '\u{20}',
    '\u{35}',
    '\u{375}',
    '\u{619}',
    '\u{700}',
    '\u{1885}',
    '\u{fffe}',
    '\u{1ffff}',
    '\u{10ffff}',
];

// Contains all the special cased Latin-1 chars.
const SPACE_TEST: [char; 10] = [
    '\u{9}', '\u{a}', '\u{b}', '\u{c}', '\u{d}', '\u{20}', '\u{85}', '\u{a0}', '\u{2000}',
    '\u{3000}',
];

const UPPER_CASE: usize = 0;
const LOWER_CASE: usize = 1;
const TITLE_CASE: usize = 2;

/// (case, in, out)
const CASE_TEST: [(usize, char, char); 74] = [
    // not ported: {-1, '\n', 0xFFFD} (invalid case)
    // not ported: {UpperCase, -1, -1} (negative rune)
    // not ported: {UpperCase, 1 << 30, 1 << 30} (rune > 0x10FFFF)
    (0, '\u{a}', '\u{a}'),
    (0, '\u{61}', '\u{41}'),
    (0, '\u{41}', '\u{41}'),
    (0, '\u{37}', '\u{37}'),
    (1, '\u{a}', '\u{a}'),
    (1, '\u{61}', '\u{61}'),
    (1, '\u{41}', '\u{61}'),
    (1, '\u{37}', '\u{37}'),
    (2, '\u{a}', '\u{a}'),
    (2, '\u{61}', '\u{41}'),
    (2, '\u{41}', '\u{41}'),
    (2, '\u{37}', '\u{37}'),
    (0, '\u{80}', '\u{80}'),
    (0, '\u{c5}', '\u{c5}'),
    (0, '\u{e5}', '\u{c5}'),
    (1, '\u{80}', '\u{80}'),
    (1, '\u{c5}', '\u{e5}'),
    (1, '\u{e5}', '\u{e5}'),
    (2, '\u{80}', '\u{80}'),
    (2, '\u{c5}', '\u{c5}'),
    (2, '\u{e5}', '\u{c5}'),
    (0, '\u{131}', '\u{49}'),
    (1, '\u{131}', '\u{131}'),
    (2, '\u{131}', '\u{49}'),
    (0, '\u{133}', '\u{132}'),
    (1, '\u{133}', '\u{133}'),
    (2, '\u{133}', '\u{132}'),
    (0, '\u{212a}', '\u{212a}'),
    (1, '\u{212a}', '\u{6b}'),
    (2, '\u{212a}', '\u{212a}'),
    (0, '\u{a640}', '\u{a640}'),
    (1, '\u{a640}', '\u{a641}'),
    (2, '\u{a640}', '\u{a640}'),
    (0, '\u{a641}', '\u{a640}'),
    (1, '\u{a641}', '\u{a641}'),
    (2, '\u{a641}', '\u{a640}'),
    (0, '\u{a64e}', '\u{a64e}'),
    (1, '\u{a64e}', '\u{a64f}'),
    (2, '\u{a64e}', '\u{a64e}'),
    (0, '\u{a65f}', '\u{a65e}'),
    (1, '\u{a65f}', '\u{a65f}'),
    (2, '\u{a65f}', '\u{a65e}'),
    (0, '\u{139}', '\u{139}'),
    (1, '\u{139}', '\u{13a}'),
    (2, '\u{139}', '\u{139}'),
    (0, '\u{13f}', '\u{13f}'),
    (1, '\u{13f}', '\u{140}'),
    (2, '\u{13f}', '\u{13f}'),
    (0, '\u{148}', '\u{147}'),
    (1, '\u{148}', '\u{148}'),
    (2, '\u{148}', '\u{147}'),
    (0, '\u{ab78}', '\u{13a8}'),
    (1, '\u{ab78}', '\u{ab78}'),
    (2, '\u{ab78}', '\u{13a8}'),
    (0, '\u{13a8}', '\u{13a8}'),
    (1, '\u{13a8}', '\u{ab78}'),
    (2, '\u{13a8}', '\u{13a8}'),
    (0, '\u{10400}', '\u{10400}'),
    (1, '\u{10400}', '\u{10428}'),
    (2, '\u{10400}', '\u{10400}'),
    (0, '\u{10427}', '\u{10427}'),
    (1, '\u{10427}', '\u{1044f}'),
    (2, '\u{10427}', '\u{10427}'),
    (0, '\u{10428}', '\u{10400}'),
    (1, '\u{10428}', '\u{10428}'),
    (2, '\u{10428}', '\u{10400}'),
    (0, '\u{1044f}', '\u{10427}'),
    (1, '\u{1044f}', '\u{1044f}'),
    (2, '\u{1044f}', '\u{10427}'),
    (0, '\u{10450}', '\u{10450}'),
    (1, '\u{10450}', '\u{10450}'),
    (2, '\u{10450}', '\u{10450}'),
    (1, '\u{2161}', '\u{2171}'),
    (0, '\u{345}', '\u{399}'),
];

fn to(case: usize, r: char) -> char {
    match case {
        UPPER_CASE => to_upper(r),
        LOWER_CASE => to_lower(r),
        _ => to_title(r),
    }
}

/// Go `unicode.To` over `CASE_RANGES`, independent of `to_upper` and friends.
fn to_by_table(case: usize, r: char) -> char {
    let c = r as u32;
    for cr in CASE_RANGES {
        if (cr.lo..=cr.hi).contains(&c) {
            let delta = cr.delta[case];
            if delta > 0x10FFFF {
                return char::from_u32(cr.lo + (((c - cr.lo) & !1) | (case as u32 & 1))).unwrap();
            }
            return char::from_u32((c as i32 + delta) as u32).unwrap();
        }
    }
    r
}

fn case_string(c: usize) -> &'static str {
    match c {
        UPPER_CASE => "UpperCase",
        LOWER_CASE => "LowerCase",
        TITLE_CASE => "TitleCase",
        _ => "ErrorCase",
    }
}

#[test]

fn test_is_letter() {
    for r in UPPER_TEST {
        assert!(
            is_letter(r),
            "IsLetter(U+{:04X}) = false, want true",
            r as u32
        );
    }
    for r in LETTER_TEST {
        assert!(
            is_letter(r),
            "IsLetter(U+{:04X}) = false, want true",
            r as u32
        );
    }
    for r in NOT_LETTER_TEST {
        assert!(
            !is_letter(r),
            "IsLetter(U+{:04X}) = true, want false",
            r as u32
        );
    }
}

#[test]

fn test_is_upper() {
    for r in UPPER_TEST {
        assert!(
            is_upper(r),
            "IsUpper(U+{:04X}) = false, want true",
            r as u32
        );
    }
    for r in NOT_UPPER_TEST {
        assert!(
            !is_upper(r),
            "IsUpper(U+{:04X}) = true, want false",
            r as u32
        );
    }
    for r in NOT_LETTER_TEST {
        assert!(
            !is_upper(r),
            "IsUpper(U+{:04X}) = true, want false",
            r as u32
        );
    }
}

#[test]

fn test_to() {
    for (cas, inp, out) in CASE_TEST {
        let r = to(cas, inp);
        assert_eq!(
            out,
            r,
            "To(U+{:04X}, {}) = U+{:04X} want U+{:04X}",
            inp as u32,
            case_string(cas),
            r as u32,
            out as u32
        );
    }
}

#[test]

fn test_to_upper_case() {
    for (cas, inp, out) in CASE_TEST {
        if cas != UPPER_CASE {
            continue;
        }
        let r = to_upper(inp);
        assert_eq!(
            out, r,
            "ToUpper(U+{:04X}) = U+{:04X} want U+{:04X}",
            inp as u32, r as u32, out as u32
        );
    }
}

#[test]

fn test_to_lower_case() {
    for (cas, inp, out) in CASE_TEST {
        if cas != LOWER_CASE {
            continue;
        }
        let r = to_lower(inp);
        assert_eq!(
            out, r,
            "ToLower(U+{:04X}) = U+{:04X} want U+{:04X}",
            inp as u32, r as u32, out as u32
        );
    }
}

#[test]

fn test_to_title_case() {
    for (cas, inp, out) in CASE_TEST {
        if cas != TITLE_CASE {
            continue;
        }
        let r = to_title(inp);
        assert_eq!(
            out, r,
            "ToTitle(U+{:04X}) = U+{:04X} want U+{:04X}",
            inp as u32, r as u32, out as u32
        );
    }
}

#[test]

fn test_is_space() {
    for c in SPACE_TEST {
        assert!(
            is_space(c),
            "IsSpace(U+{:04X}) = false; want true",
            c as u32
        );
    }
    for c in LETTER_TEST {
        assert!(
            !is_space(c),
            "IsSpace(U+{:04X}) = true; want false",
            c as u32
        );
    }
}

// Check that the optimizations for IsLetter etc. agree with the tables.
// Only the Latin-1 range (plus U+0100) is checked.
#[test]

fn test_letter_optimizations() {
    for i in 0..=0x100u32 {
        let r = char::from_u32(i).unwrap();
        assert_eq!(
            is(&L, r),
            is_letter(r),
            "IsLetter(U+{i:04X}) disagrees with Is(Letter)"
        );
        assert_eq!(
            is(&LU, r),
            is_upper(r),
            "IsUpper(U+{i:04X}) disagrees with Is(Upper)"
        );
        assert_eq!(
            is(&WHITE_SPACE, r),
            is_space(r),
            "IsSpace(U+{i:04X}) disagrees with Is(White_Space)"
        );
        assert_eq!(
            to_by_table(UPPER_CASE, r),
            to_upper(r),
            "ToUpper(U+{i:04X}) disagrees with To(Upper)"
        );
        assert_eq!(
            to_by_table(LOWER_CASE, r),
            to_lower(r),
            "ToLower(U+{i:04X}) disagrees with To(Lower)"
        );
        assert_eq!(
            to_by_table(TITLE_CASE, r),
            to_title(r),
            "ToTitle(U+{i:04X}) disagrees with To(Title)"
        );
    }
}

// SimpleFold(x) returns the next equivalent rune > x or wraps around to smaller values.
const SIMPLE_FOLD_TESTS: [&[char]; 9] = [
    &['\u{41}', '\u{61}'],
    &['\u{3b4}', '\u{394}'],
    &['\u{4b}', '\u{6b}', '\u{212a}'],
    &['\u{53}', '\u{73}', '\u{17f}'],
    &['\u{3c1}', '\u{3f1}', '\u{3a1}'],
    &['\u{345}', '\u{399}', '\u{3b9}', '\u{1fbe}'],
    &['\u{130}'],
    &['\u{131}'],
    &['\u{13b0}', '\u{ab80}'],
];

#[test]

fn test_simple_fold() {
    for cycle in SIMPLE_FOLD_TESTS {
        let mut r = cycle[cycle.len() - 1];
        for &out in cycle {
            let got = simple_fold(r);
            assert_eq!(
                got, out,
                "SimpleFold({r:?} U+{:04X}) = U+{:04X}, want U+{:04X}",
                r as u32, got as u32, out as u32
            );
            r = out;
        }
    }
    // not ported: SimpleFold(-42) (negative rune)
}

#[test]

fn test_latin_offset() {
    let maps: [&[(&str, &RangeTable)]; 5] =
        [CATEGORIES, FOLD_CATEGORY, FOLD_SCRIPT, PROPERTIES, SCRIPTS];
    for m in maps {
        for (name, tab) in m {
            let mut i = 0;
            while i < tab.r16.len() && tab.r16[i].hi <= 0xFF {
                i += 1;
            }
            assert_eq!(
                tab.latin_offset, i,
                "{name}: LatinOffset={}, want {i}",
                tab.latin_offset
            );
        }
    }
}

/// Not from Go: places where Go's unicode differs from Rust's `char` and `str` methods.
#[test]

fn go_not_rust_semantics() {
    assert!(!is_letter('\u{2160}')); // Nl; Rust is_alphabetic is true
    assert!(!is_upper('\u{24B6}')); // Other_Uppercase
    assert_eq!(strings::to_lower("İ"), "i");
    assert_eq!(strings::to_upper("ß"), "ß");
    assert_eq!(strings::to_lower("ΑΣ"), "ασ");
    assert!(strings::equal_fold("\u{212A}", "k"));
    assert!(!strings::equal_fold("ß", "SS"));
}
