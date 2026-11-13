//! Go std `unicode` (Go 1.27.4, UCD 17.0.0): the tables of `tables.go` (generated, see
//! `crate::tables`) and the predicates and mappings graphify-go calls. These differ from Rust's
//! `char` methods: `is_letter` is category L (not Alphabetic), `is_upper` is Lu (not Uppercase),
//! and the case mappings are Go's simple per-rune mappings.

pub use crate::tables::unicode17::*;
pub use crate::tables::{CaseRange, Range16, Range32, RangeTable, UPPER_LOWER};

const MAX_LATIN1: u32 = 0xFF;

// Go's case indexes into CaseRange::delta.
const UPPER_CASE: usize = 0;
const LOWER_CASE: usize = 1;
const TITLE_CASE: usize = 2;

/// Go `unicode.Is`: whether `r` is in `t`.
pub fn is(t: &RangeTable, r: char) -> bool {
    let r = r as u32;
    if let Ok(r16) = u16::try_from(r) {
        let i = t.r16.partition_point(|g| g.hi < r16);
        return t
            .r16
            .get(i)
            .is_some_and(|g| g.lo <= r16 && (r16 - g.lo).is_multiple_of(g.stride));
    }
    let i = t.r32.partition_point(|g| g.hi < r);
    t.r32
        .get(i)
        .is_some_and(|g| g.lo <= r && (r - g.lo).is_multiple_of(g.stride))
}

fn lookup<T: Copy>(map: &[(&str, T)], name: &str) -> Option<T> {
    map.binary_search_by(|(k, _)| (*k).cmp(name))
        .ok()
        .map(|i| map[i].1)
}

/// Go `unicode.Categories[name]`.
pub fn category(name: &str) -> Option<&'static RangeTable> {
    lookup(CATEGORIES, name)
}

/// Go `unicode.Scripts[name]`.
pub fn script(name: &str) -> Option<&'static RangeTable> {
    lookup(SCRIPTS, name)
}

/// Go `unicode.Properties[name]`.
pub fn property(name: &str) -> Option<&'static RangeTable> {
    lookup(PROPERTIES, name)
}

/// Go `unicode.FoldCategory[name]`.
pub fn fold_category(name: &str) -> Option<&'static RangeTable> {
    lookup(FOLD_CATEGORY, name)
}

/// Go `unicode.FoldScript[name]`.
pub fn fold_script(name: &str) -> Option<&'static RangeTable> {
    lookup(FOLD_SCRIPT, name)
}

/// Go `unicode.CategoryAliases[name]`.
pub fn category_alias(name: &str) -> Option<&'static str> {
    lookup(CATEGORY_ALIASES, name)
}

/// Go `unicode.IsLetter`: category L.
pub fn is_letter(r: char) -> bool {
    is(&L, r)
}

/// Go `unicode.IsNumber`: category N.
pub fn is_number(r: char) -> bool {
    is(&N, r)
}

/// Go `unicode.IsUpper`: category Lu.
pub fn is_upper(r: char) -> bool {
    is(&LU, r)
}

/// Go `unicode.IsSpace`: `\t \n \v \f \r ' ' U+0085 U+00A0` in Latin-1, else White_Space.
pub fn is_space(r: char) -> bool {
    if r as u32 <= MAX_LATIN1 {
        return matches!(
            r,
            '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{85}' | '\u{A0}'
        );
    }
    is(&WHITE_SPACE, r)
}

/// Go `unicode.IsPrint`: categories L, M, N, P, S and U+0020 (Go's Latin-1 `properties` table
/// gives the same set below U+0100).
pub fn is_print(r: char) -> bool {
    r == ' ' || [&L, &M, &N, &P, &S].into_iter().any(|t| is(t, r))
}

/// Go `lookupCaseRange` and `convertCase`.
fn to(case: usize, r: char) -> Option<char> {
    let c = r as u32;
    let i = CASE_RANGES.partition_point(|cr| cr.hi < c);
    let cr = CASE_RANGES.get(i).filter(|cr| cr.lo <= c)?;
    let delta = cr.delta[case];
    let mapped = if delta > 0x10FFFF {
        // An Upper-Lower run: even offsets are upper case, odd offsets lower case.
        cr.lo + (((c - cr.lo) & !1) | (case as u32 & 1))
    } else {
        c.wrapping_add_signed(delta)
    };
    Some(char::from_u32(mapped).expect("case mapping is a valid rune"))
}

/// Go `unicode.ToLower`: simple mapping through `CASE_RANGES`.
pub fn to_lower(r: char) -> char {
    if r.is_ascii() {
        return r.to_ascii_lowercase();
    }
    to(LOWER_CASE, r).unwrap_or(r)
}

/// Go `unicode.ToUpper`.
pub fn to_upper(r: char) -> char {
    if r.is_ascii() {
        return r.to_ascii_uppercase();
    }
    to(UPPER_CASE, r).unwrap_or(r)
}

/// Go `unicode.ToTitle`.
pub fn to_title(r: char) -> char {
    if r.is_ascii() {
        return r.to_ascii_uppercase();
    }
    to(TITLE_CASE, r).unwrap_or(r)
}

/// Go `unicode.SimpleFold`: the next rune in the simple case folding orbit of `r`.
pub fn simple_fold(r: char) -> char {
    let c = r as u32;
    // Go reads asciiFold for ASCII; the generator builds it from the same orbit and case data, so
    // the general path below gives the same answer.
    let i = CASE_ORBIT.partition_point(|&(from, _)| from < c);
    if let Some(&(from, to_r)) = CASE_ORBIT.get(i)
        && from == c
    {
        return char::from_u32(to_r).expect("orbit rune");
    }
    if let Some(l) = to(LOWER_CASE, r)
        && l != r
    {
        return l;
    }
    to(UPPER_CASE, r).unwrap_or(r)
}
