//! Go 1.27.4 regexp/syntax: parser, simplifier and compiler for RE2 syntax.

use std::fmt;
use std::ops::{BitAnd, BitOr, Not};

mod compile;
mod parse;
mod perl_groups;
mod prog;
mod regexp;
mod simplify;

/// Go `Flags` (parse.go:56). Parser behaviour flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(pub u16);

impl Flags {
    /// Go `FoldCase` (parse.go:59): case-insensitive match.
    pub const FOLD_CASE: Flags = Flags(1 << 0);
    /// Go `Literal` (parse.go:60): treat pattern as literal string.
    pub const LITERAL: Flags = Flags(1 << 1);
    /// Go `ClassNL` (parse.go:61): classes like `[^a-z]` may match newline.
    pub const CLASS_NL: Flags = Flags(1 << 2);
    /// Go `DotNL` (parse.go:62): `.` may match newline.
    pub const DOT_NL: Flags = Flags(1 << 3);
    /// Go `OneLine` (parse.go:63): `^` and `$` match only at text edges.
    pub const ONE_LINE: Flags = Flags(1 << 4);
    /// Go `NonGreedy` (parse.go:64): repetition is non-greedy by default.
    pub const NON_GREEDY: Flags = Flags(1 << 5);
    /// Go `PerlX` (parse.go:65): allow Perl extensions.
    pub const PERL_X: Flags = Flags(1 << 6);
    /// Go `UnicodeGroups` (parse.go:66): allow `\p{Han}` and `\P{Han}`.
    pub const UNICODE_GROUPS: Flags = Flags(1 << 7);
    /// Go `WasDollar` (parse.go:67): OpEndText was `$`, not `\z`.
    pub const WAS_DOLLAR: Flags = Flags(1 << 8);
    /// Go `Simple` (parse.go:68): no counted repetition.
    pub const SIMPLE: Flags = Flags(1 << 9);
    /// Go `MatchNL` (parse.go:70): `ClassNL | DotNL`.
    pub const MATCH_NL: Flags = Flags(Self::CLASS_NL.0 | Self::DOT_NL.0);
    /// Go `Perl` (parse.go:72): `ClassNL | OneLine | PerlX | UnicodeGroups`.
    pub const PERL: Flags =
        Flags(Self::CLASS_NL.0 | Self::ONE_LINE.0 | Self::PERL_X.0 | Self::UNICODE_GROUPS.0);
    /// Go `POSIX` (parse.go:73): POSIX syntax, no flags.
    pub const POSIX: Flags = Flags(0);
}

impl BitOr for Flags {
    type Output = Flags;
    /// Go `a | b`.
    fn bitor(self, rhs: Flags) -> Flags {
        Flags(self.0 | rhs.0)
    }
}

impl BitAnd for Flags {
    type Output = Flags;
    /// Go `a & b`.
    fn bitand(self, rhs: Flags) -> Flags {
        Flags(self.0 & rhs.0)
    }
}

impl Not for Flags {
    type Output = Flags;
    /// Go `^a` (used as `x &^ y` == `x & !y`).
    fn not(self) -> Flags {
        Flags(!self.0)
    }
}

/// Go `Op` (regexp.go:33). Discriminants match Go's `1 + iota`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Op {
    /// Go `OpNoMatch` (regexp.go:41): matches no strings.
    NoMatch = 1,
    /// Go `OpEmptyMatch` (regexp.go:42): matches empty string.
    EmptyMatch = 2,
    /// Go `OpLiteral` (regexp.go:43): matches `rune` sequence.
    Literal = 3,
    /// Go `OpCharClass` (regexp.go:44): `rune` is a range-pair list.
    CharClass = 4,
    /// Go `OpAnyCharNotNL` (regexp.go:45).
    AnyCharNotNL = 5,
    /// Go `OpAnyChar` (regexp.go:46).
    AnyChar = 6,
    /// Go `OpBeginLine` (regexp.go:47).
    BeginLine = 7,
    /// Go `OpEndLine` (regexp.go:48).
    EndLine = 8,
    /// Go `OpBeginText` (regexp.go:49).
    BeginText = 9,
    /// Go `OpEndText` (regexp.go:50).
    EndText = 10,
    /// Go `OpWordBoundary` (regexp.go:51).
    WordBoundary = 11,
    /// Go `OpNoWordBoundary` (regexp.go:52).
    NoWordBoundary = 12,
    /// Go `OpCapture` (regexp.go:53).
    Capture = 13,
    /// Go `OpStar` (regexp.go:54).
    Star = 14,
    /// Go `OpPlus` (regexp.go:55).
    Plus = 15,
    /// Go `OpQuest` (regexp.go:56).
    Quest = 16,
    /// Go `OpRepeat` (regexp.go:57).
    Repeat = 17,
    /// Go `OpConcat` (regexp.go:58).
    Concat = 18,
    /// Go `OpAlternate` (regexp.go:59).
    Alternate = 19,
    /// Go `opPseudo` (regexp.go:62): where pseudo-ops start.
    Pseudo = 128,
}

impl fmt::Display for Op {
    /// Go `Op.String` (op_string.go:41): name without the `Op` prefix, `opPseudo` for 128.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Op::NoMatch => f.write_str("NoMatch"),
            Op::EmptyMatch => f.write_str("EmptyMatch"),
            Op::Literal => f.write_str("Literal"),
            Op::CharClass => f.write_str("CharClass"),
            Op::AnyCharNotNL => f.write_str("AnyCharNotNL"),
            Op::AnyChar => f.write_str("AnyChar"),
            Op::BeginLine => f.write_str("BeginLine"),
            Op::EndLine => f.write_str("EndLine"),
            Op::BeginText => f.write_str("BeginText"),
            Op::EndText => f.write_str("EndText"),
            Op::WordBoundary => f.write_str("WordBoundary"),
            Op::NoWordBoundary => f.write_str("NoWordBoundary"),
            Op::Capture => f.write_str("Capture"),
            Op::Star => f.write_str("Star"),
            Op::Plus => f.write_str("Plus"),
            Op::Quest => f.write_str("Quest"),
            Op::Repeat => f.write_str("Repeat"),
            Op::Concat => f.write_str("Concat"),
            Op::Alternate => f.write_str("Alternate"),
            Op::Pseudo => f.write_str("opPseudo"),
        }
    }
}

/// Go `Regexp` (regexp.go:18). Go's `Sub0`/`Rune0` storage is omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regexp {
    /// Operator.
    pub op: Op,
    pub flags: Flags,
    /// Subexpressions, if any.
    pub sub: Vec<Regexp>,
    /// Matched runes, for Literal and CharClass (CharClass: lo, hi pairs). Go `rune` values
    /// (`int32`): a class range can start or end at a surrogate (`\p{Cs}`, `\p{Assigned}`), which
    /// a `char` cannot hold.
    pub rune: Vec<u32>,
    /// Min for Repeat.
    pub min: i32,
    /// Max for Repeat (-1 is no limit).
    pub max: i32,
    /// Capture index, for Capture.
    pub cap: i32,
    /// Capture name, for Capture.
    pub name: String,
}

impl Regexp {
    /// Go `Regexp.Simplify` (simplify.go:14).
    pub fn simplify(&self) -> Regexp {
        simplify::simplify(self)
    }

    /// Go `Regexp.MaxCap` (regexp.go:437).
    pub fn max_cap(&self) -> i32 {
        regexp::max_cap(self)
    }

    /// Go `Regexp.CapNames` (regexp.go:451).
    pub fn cap_names(&self) -> Vec<String> {
        regexp::cap_names(self)
    }

    /// Go `Regexp.Equal` (regexp.go:64).
    pub fn equal(&self, other: &Regexp) -> bool {
        regexp::equal(self, other)
    }
}

impl fmt::Display for Regexp {
    /// Go `Regexp.String` (regexp.go:384).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        regexp::write_string(self, f)
    }
}

/// Go `ErrorCode` (parse.go:27), as an enum of Go's string constants (parse.go:29-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Go `ErrInternalError` (parse.go:30).
    InternalError,
    /// Go `ErrInvalidCharClass` (parse.go:33).
    InvalidCharClass,
    /// Go `ErrInvalidCharRange` (parse.go:34).
    InvalidCharRange,
    /// Go `ErrInvalidEscape` (parse.go:35).
    InvalidEscape,
    /// Go `ErrInvalidNamedCapture` (parse.go:36).
    InvalidNamedCapture,
    /// Go `ErrInvalidPerlOp` (parse.go:37).
    InvalidPerlOp,
    /// Go `ErrInvalidRepeatOp` (parse.go:38).
    InvalidRepeatOp,
    /// Go `ErrInvalidRepeatSize` (parse.go:39).
    InvalidRepeatSize,
    /// Go `ErrInvalidUTF8` (parse.go:40).
    InvalidUTF8,
    /// Go `ErrMissingBracket` (parse.go:41).
    MissingBracket,
    /// Go `ErrMissingParen` (parse.go:42).
    MissingParen,
    /// Go `ErrMissingRepeatArgument` (parse.go:43).
    MissingRepeatArgument,
    /// Go `ErrTrailingBackslash` (parse.go:44).
    TrailingBackslash,
    /// Go `ErrUnexpectedParen` (parse.go:45).
    UnexpectedParen,
    /// Go `ErrNestingDepth` (parse.go:46).
    NestingDepth,
    /// Go `ErrLarge` (parse.go:47).
    Large,
}

impl ErrorCode {
    /// Go `ErrorCode.String` (parse.go:51): the code's exact Go string.
    pub fn text(&self) -> &'static str {
        match self {
            ErrorCode::InternalError => "regexp/syntax: internal error",
            ErrorCode::InvalidCharClass => "invalid character class",
            ErrorCode::InvalidCharRange => "invalid character class range",
            ErrorCode::InvalidEscape => "invalid escape sequence",
            ErrorCode::InvalidNamedCapture => "invalid named capture",
            ErrorCode::InvalidPerlOp => "invalid or unsupported Perl syntax",
            ErrorCode::InvalidRepeatOp => "invalid nested repetition operator",
            ErrorCode::InvalidRepeatSize => "invalid repeat count",
            ErrorCode::InvalidUTF8 => "invalid UTF-8",
            ErrorCode::MissingBracket => "missing closing ]",
            ErrorCode::MissingParen => "missing closing )",
            ErrorCode::MissingRepeatArgument => "missing argument to repetition operator",
            ErrorCode::TrailingBackslash => "trailing backslash at end of expression",
            ErrorCode::UnexpectedParen => "unexpected )",
            ErrorCode::NestingDepth => "expression nests too deeply",
            ErrorCode::Large => "expression too large",
        }
    }
}

impl fmt::Display for ErrorCode {
    /// Go `ErrorCode.String` (parse.go:51) == `text()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text())
    }
}

/// Go `Error` (parse.go:17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub expr: String,
}

impl fmt::Display for Error {
    /// Go `Error.Error` (parse.go:22): "error parsing regexp: " + code + ": `" + expr + "`".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error parsing regexp: {}: `{}`", self.code, self.expr)
    }
}

impl std::error::Error for Error {}

/// Go `Parse` (parse.go:887).
pub fn parse(s: &str, flags: Flags) -> Result<Regexp, Error> {
    parse::parse(s, flags)
}

/// Go `minFold` (parse.go:2014): smallest rune with a nontrivial `SimpleFold`.
/// `TestFoldConstants` (parse_test.go:433) compares it.
pub const MIN_FOLD: char = '\u{0041}';
/// Go `maxFold` (parse.go:2015): largest rune with a nontrivial `SimpleFold`.
pub const MAX_FOLD: char = '\u{1e943}';

/// Go `InstOp` (prog.go:25). Discriminants match Go's `iota`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InstOp {
    /// Go `InstAlt` (prog.go:28).
    Alt = 0,
    /// Go `InstAltMatch` (prog.go:29).
    AltMatch = 1,
    /// Go `InstCapture` (prog.go:30).
    Capture = 2,
    /// Go `InstEmptyWidth` (prog.go:31).
    EmptyWidth = 3,
    /// Go `InstMatch` (prog.go:32).
    Match = 4,
    /// Go `InstFail` (prog.go:33).
    Fail = 5,
    /// Go `InstNop` (prog.go:34).
    Nop = 6,
    /// Go `InstRune` (prog.go:35).
    Rune = 7,
    /// Go `InstRune1` (prog.go:36).
    Rune1 = 8,
    /// Go `InstRuneAny` (prog.go:37).
    RuneAny = 9,
    /// Go `InstRuneAnyNotNL` (prog.go:38).
    RuneAnyNotNL = 10,
}

impl fmt::Display for InstOp {
    /// Go `InstOp.String` (prog.go:55): `instOpNames` entry ("InstAlt", ...).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InstOp::Alt => "InstAlt",
            InstOp::AltMatch => "InstAltMatch",
            InstOp::Capture => "InstCapture",
            InstOp::EmptyWidth => "InstEmptyWidth",
            InstOp::Match => "InstMatch",
            InstOp::Fail => "InstFail",
            InstOp::Nop => "InstNop",
            InstOp::Rune => "InstRune",
            InstOp::Rune1 => "InstRune1",
            InstOp::RuneAny => "InstRuneAny",
            InstOp::RuneAnyNotNL => "InstRuneAnyNotNL",
        })
    }
}

/// Go `EmptyOp` (prog.go:63): a mixture of zero-width assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyOp(pub u8);

impl EmptyOp {
    /// Go `EmptyBeginLine` (prog.go:66).
    pub const BEGIN_LINE: EmptyOp = EmptyOp(1 << 0);
    /// Go `EmptyEndLine` (prog.go:67).
    pub const END_LINE: EmptyOp = EmptyOp(1 << 1);
    /// Go `EmptyBeginText` (prog.go:68).
    pub const BEGIN_TEXT: EmptyOp = EmptyOp(1 << 2);
    /// Go `EmptyEndText` (prog.go:69).
    pub const END_TEXT: EmptyOp = EmptyOp(1 << 3);
    /// Go `EmptyWordBoundary` (prog.go:70).
    pub const WORD_BOUNDARY: EmptyOp = EmptyOp(1 << 4);
    /// Go `EmptyNoWordBoundary` (prog.go:71).
    pub const NO_WORD_BOUNDARY: EmptyOp = EmptyOp(1 << 5);
}

/// Go `Inst` (prog.go:115).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inst {
    pub op: InstOp,
    /// All but Match, Fail.
    pub out: u32,
    /// Alt, AltMatch, Capture, EmptyWidth.
    pub arg: u32,
    /// Go `rune` values, as in `Regexp::rune`.
    pub rune: Vec<u32>,
}

/// Go `Prog` (prog.go:18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prog {
    pub inst: Vec<Inst>,
    /// Index of start instruction.
    pub start: usize,
    /// Number of Capture insts in the regexp.
    pub num_cap: usize,
}

impl fmt::Display for Prog {
    /// Go `Prog.String` (prog.go:122), via `dumpProg` (prog.go:296).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        prog::dump(self, f)
    }
}

/// Go `Compile` (compile.go:71). Expects a simplified regexp.
pub fn compile(re: &Regexp) -> Result<Prog, Error> {
    compile::compile(re)
}

/// Go `EmptyOpContext` (prog.go:80). `None` is Go's -1 (beginning/end of text).
pub fn empty_op_context(r1: Option<char>, r2: Option<char>) -> EmptyOp {
    prog::empty_op_context(r1, r2)
}

/// Go `IsWordChar` (prog.go:108): ASCII `[A-Za-z0-9_]` only.
pub fn is_word_char(r: char) -> bool {
    prog::is_word_char(r)
}
