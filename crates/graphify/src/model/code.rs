//! Port of `code.go`: relation, confidence, location and file codes in process-wide tables.

use std::collections::HashMap;
use std::fmt;
use std::sync::{LazyLock, RwLock};

use gostd::json::{DecodeState, EncodeState, Error as JsonError, Marshal, Unmarshal};

use super::jsonenc::append_json_string;

/// Go `codeTable`: each string gets a number; a number gives back its string. Strings are
/// leaked to `&'static str`: Go never frees a code either. No cap: a code wraps past 2^29
/// strings (`Loc`), a table over 10 GB.
struct CodeTable(RwLock<(HashMap<&'static str, u32>, Vec<&'static str>)>);

impl CodeTable {
    fn new(names: &[&'static str]) -> CodeTable {
        let ids = names
            .iter()
            .enumerate()
            .map(|(i, &s)| (s, i as u32))
            .collect();
        CodeTable(RwLock::new((ids, names.to_vec())))
    }

    /// Go `codeTable.code`.
    fn code(&self, s: &str) -> u32 {
        if let Some(&id) = self.0.read().expect("code table").0.get(s) {
            return id;
        }
        let mut t = self.0.write().expect("code table");
        if let Some(&id) = t.0.get(s) {
            return id;
        }
        let id = t.1.len() as u32;
        let s: &'static str = Box::leak(s.to_owned().into_boxed_str());
        t.0.insert(s, id);
        t.1.push(s);
        id
    }

    /// Go `codeTable.name`; panics on a number no string has, as Go's index does.
    fn name(&self, id: u32) -> &'static str {
        self.0.read().expect("code table").1[id as usize]
    }
}

static REL_TABLE: LazyLock<CodeTable> = LazyLock::new(|| {
    CodeTable::new(&[
        "",
        "contains",
        "calls",
        "imports",
        "imports_from",
        "references",
        "inherits",
        "depends_on",
        "crate_depends_on",
        "case_of",
        "dispatches_to",
        "rationale_for",
        "cites",
        "implements",
        "re_exports",
        "dynamic_import",
    ])
});
static CONF_TABLE: LazyLock<CodeTable> =
    LazyLock::new(|| CodeTable::new(&["", "EXTRACTED", "INFERRED", "AMBIGUOUS"]));
static LOC_TABLE: LazyLock<CodeTable> = LazyLock::new(|| CodeTable::new(&[""]));
static FILE_TABLE: LazyLock<CodeTable> = LazyLock::new(|| CodeTable::new(&[""]));

/// Go `model.Relation`: a code for a relation name. Any string is a valid value; [`rel`] gives
/// its code and [`Relation::as_str`] its text. Code 0 is the empty string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Relation(pub u32);

impl Relation {
    pub const CONTAINS: Relation = Relation(1);
    pub const CALLS: Relation = Relation(2);
    pub const IMPORTS: Relation = Relation(3);
    pub const IMPORTS_FROM: Relation = Relation(4);
    pub const REFERENCES: Relation = Relation(5);
    pub const INHERITS: Relation = Relation(6);
    pub const DEPENDS_ON: Relation = Relation(7);
    pub const CRATE_DEPENDS_ON: Relation = Relation(8);
    pub const CASE_OF: Relation = Relation(9);
    pub const DISPATCHES_TO: Relation = Relation(10);
    pub const RATIONALE_FOR: Relation = Relation(11);
    pub const CITES: Relation = Relation(12);
    pub const IMPLEMENTS: Relation = Relation(13);
    pub const RE_EXPORTS: Relation = Relation(14);
    pub const DYNAMIC_IMPORT: Relation = Relation(15);

    /// Go `Relation.String`.
    pub fn as_str(self) -> &'static str {
        REL_TABLE.name(self.0)
    }
}

/// Go `model.Rel`: the code of a relation name.
pub fn rel(s: &str) -> Relation {
    Relation(REL_TABLE.code(s))
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Go `model.Confidence`: a code for a confidence name. Code 0 is the empty string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Confidence(pub u32);

impl Confidence {
    pub const EXTRACTED: Confidence = Confidence(1);
    pub const INFERRED: Confidence = Confidence(2);
    pub const AMBIGUOUS: Confidence = Confidence(3);

    /// Go `Confidence.String`.
    pub fn as_str(self) -> &'static str {
        CONF_TABLE.name(self.0)
    }
}

/// Go `model.Conf`: the code of a confidence name.
pub fn conf(s: &str) -> Confidence {
    Confidence(CONF_TABLE.code(s))
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Go `model.Loc`: a source location. `"L<n>"` is held as n+1, other text as bit 31 plus a code
/// into a process-wide table; 0 is none. Bits 30 and 29 are an Edge's type-only and deferred
/// flags, which the methods ignore.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Loc(pub u32);

/// Go `locText`, `locTypeOnly`, `locDeferred`, `locFlags` (`code.go:118-123`).
pub(super) const LOC_TEXT: u32 = 1 << 31;
pub(super) const LOC_TYPE_ONLY: u32 = 1 << 30;
pub(super) const LOC_DEFERRED: u32 = 1 << 29;
pub(super) const LOC_FLAGS: u32 = LOC_TYPE_ONLY | LOC_DEFERRED;

/// Go `model.Line`: the location of line `n` (Go `int`).
pub fn line(n: i64) -> Loc {
    if n < 0 || n > i64::from(LOC_DEFERRED - 2) {
        return loc_of(&format!("L{n}"));
    }
    Loc(n as u32 + 1)
}

/// Go `model.LocOf`: the location `s` names. Only the spelling `Display` gives back (no sign, no
/// leading zero) is held as a line number.
pub fn loc_of(s: &str) -> Loc {
    if s.is_empty() {
        return Loc(0);
    }
    let b = s.as_bytes();
    if (2..=11).contains(&b.len()) && b[0] == b'L' && (b[1] != b'0' || b.len() == 2) {
        let mut n: i64 = 0;
        for &c in &b[1..] {
            if !c.is_ascii_digit() {
                n = -1;
                break;
            }
            n = n * 10 + i64::from(c - b'0');
        }
        if n >= 0 && n <= i64::from(LOC_DEFERRED - 2) {
            return Loc(n as u32 + 1);
        }
    }
    Loc(LOC_TEXT | (LOC_TABLE.code(s) & (LOC_DEFERRED - 1)))
}

impl Loc {
    /// The location without the flag bits: 0, a line (n+1) or `LOC_TEXT` plus a code.
    fn bare(self) -> u32 {
        self.0 & !LOC_FLAGS
    }

    /// Go `Loc.AppendJSON`: appends the location as a JSON string.
    pub fn append_json(self, dst: &mut Vec<u8>) {
        let l = self.bare();
        if l == 0 || l >= LOC_TEXT {
            append_json_string(dst, LOC_TABLE.name(l & !LOC_TEXT));
            return;
        }
        dst.extend_from_slice(b"\"L");
        dst.extend_from_slice((l - 1).to_string().as_bytes());
        dst.push(b'"');
    }
}

/// Go `Loc.String`.
impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let l = self.bare();
        if l == 0 || l >= LOC_TEXT {
            return f.write_str(LOC_TABLE.name(l & !LOC_TEXT));
        }
        write!(f, "L{}", l - 1)
    }
}

/// Go `model.File`: a source file path as a code into a process-wide table; 0 is none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct File(pub u32);

impl File {
    /// Go `File.String`.
    pub fn as_str(self) -> &'static str {
        FILE_TABLE.name(self.0)
    }
}

/// Go `model.FileOf`: the code of a path.
pub fn file_of(s: &str) -> File {
    File(FILE_TABLE.code(s))
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Go `MarshalText`/`UnmarshalText` of the four code types: in JSON each is the string of its
/// text, and `omitempty` drops the value 0, as Go's `isEmptyValue` does for a `uint32` kind. A
/// JSON `null` leaves the value; another kind is Go's type error.
macro_rules! text_json {
    ($($t:ident => $go:literal, |$v:ident, $s:ident| $set:expr;)*) => {$(
        impl Marshal for $t {
            fn marshal_json(&self, e: &mut EncodeState) -> Result<(), JsonError> {
                todo!("graphify-rs-ihc.7")
            }

            fn is_empty_value(&self) -> bool {
                self.0 == 0
            }
        }

        impl Unmarshal for $t {
            const GO_TYPE: &'static str = $go;

            fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> Result<(), JsonError> {
                // The JSON bodies land after graphify-rs-aa6.10 closes (PR #89).
                let _ = (d, |$v: &mut Self, $s: String| -> Self { $set });
                todo!("graphify-rs-ihc.7")
            }
        }
    )*};
}

text_json! {
    Relation => "model.Relation", |_v, s| rel(&s);
    Confidence => "model.Confidence", |_v, s| conf(&s);
    // Go `Loc.UnmarshalText` keeps the flag bits: "type_only" may come first (`code.go:179`).
    Loc => "model.Loc", |v, s| Loc((v.0 & LOC_FLAGS) | loc_of(&s).0);
    File => "model.File", |_v, s| file_of(&s);
}
