//! Port of `code.go`: relation, confidence, location and file codes in process-wide tables.

use std::fmt;

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
        todo!("graphify-rs-ihc.7")
    }
}

/// Go `model.Rel`: the code of a relation name.
pub fn rel(s: &str) -> Relation {
    todo!("graphify-rs-ihc.7")
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
        todo!("graphify-rs-ihc.7")
    }
}

/// Go `model.Conf`: the code of a confidence name.
pub fn conf(s: &str) -> Confidence {
    todo!("graphify-rs-ihc.7")
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
    todo!("graphify-rs-ihc.7")
}

/// Go `model.LocOf`: the location `s` names.
pub fn loc_of(s: &str) -> Loc {
    todo!("graphify-rs-ihc.7")
}

impl Loc {
    /// Go `Loc.AppendJSON`: appends the location as a JSON string.
    pub fn append_json(self, dst: &mut Vec<u8>) {
        todo!("graphify-rs-ihc.7")
    }
}

/// Go `Loc.String`.
impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("graphify-rs-ihc.7")
    }
}

/// Go `model.File`: a source file path as a code into a process-wide table; 0 is none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct File(pub u32);

impl File {
    /// Go `File.String`.
    pub fn as_str(self) -> &'static str {
        todo!("graphify-rs-ihc.7")
    }
}

/// Go `model.FileOf`: the code of a path.
pub fn file_of(s: &str) -> File {
    todo!("graphify-rs-ihc.7")
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
