//! Go std `unicode` (Go 1.27.4, UCD 17.0.0): the tables of `tables.go` (generated, see
//! `crate::tables`) and the predicates and mappings graphify-go calls. These differ from Rust's
//! `char` methods: `is_letter` is category L (not Alphabetic), `is_upper` is Lu (not Uppercase),
//! and the case mappings are Go's simple per-rune mappings.
#![allow(unused_variables)] // graphify-rs-aa6.3: stubs until the code PR

pub use crate::tables::unicode17::*;
pub use crate::tables::{CaseRange, Range16, Range32, RangeTable, UPPER_LOWER};

/// Go `unicode.Is`: whether `r` is in `t`.
pub fn is(t: &RangeTable, r: char) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.Categories[name]`.
pub fn category(name: &str) -> Option<&'static RangeTable> {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.Scripts[name]`.
pub fn script(name: &str) -> Option<&'static RangeTable> {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.Properties[name]`.
pub fn property(name: &str) -> Option<&'static RangeTable> {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.FoldCategory[name]`.
pub fn fold_category(name: &str) -> Option<&'static RangeTable> {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.FoldScript[name]`.
pub fn fold_script(name: &str) -> Option<&'static RangeTable> {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.CategoryAliases[name]`.
pub fn category_alias(name: &str) -> Option<&'static str> {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.IsLetter`: category L.
pub fn is_letter(r: char) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.IsNumber`: category N.
pub fn is_number(r: char) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.IsUpper`: category Lu.
pub fn is_upper(r: char) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.IsSpace`: `\t \n \v \f \r ' ' U+0085 U+00A0` in Latin-1, else White_Space.
pub fn is_space(r: char) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.IsPrint`: categories L, M, N, P, S and U+0020.
pub fn is_print(r: char) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.ToLower`: simple mapping through `CASE_RANGES`.
pub fn to_lower(r: char) -> char {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.ToUpper`.
pub fn to_upper(r: char) -> char {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.ToTitle`.
pub fn to_title(r: char) -> char {
    todo!("graphify-rs-aa6.3")
}

/// Go `unicode.SimpleFold`: the next rune in the simple case folding orbit of `r`.
pub fn simple_fold(r: char) -> char {
    todo!("graphify-rs-aa6.3")
}
