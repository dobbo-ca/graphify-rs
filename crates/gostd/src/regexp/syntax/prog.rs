//! Port of Go 1.27.4 regexp/syntax/prog.go: Prog/Inst helpers, EmptyOpContext, IsWordChar, dumpProg.
//!
//! Go's `-1` rune ("no rune": beginning or end of text) is `None` in the `Option<char>` parameters.

use super::{EmptyOp, Flags, Inst, InstOp, Prog};
use crate::unicode::simple_fold;
use std::fmt;

/// Go `Prog.String` via `dumpProg` (prog.go:296): writes the dump into `f`.
pub(super) fn dump(p: &Prog, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (j, i) in p.inst.iter().enumerate() {
        let mut pc = j.to_string();
        if pc.len() < 3 {
            f.write_str(&"   "[pc.len()..])?;
        }
        if j == p.start {
            pc.push('*');
        }
        write!(f, "{pc}\t")?;
        dump_inst(f, i)?;
        f.write_str("\n")?;
    }
    Ok(())
}

/// Go `EmptyOpContext` (prog.go:80). `None` is Go's -1.
pub(super) fn empty_op_context(r1: Option<char>, r2: Option<char>) -> EmptyOp {
    let mut op = EmptyOp::NO_WORD_BOUNDARY.0;
    let mut boundary = 0u8;
    if r1.is_some_and(is_word_char) {
        boundary = 1;
    } else if r1 == Some('\n') {
        op |= EmptyOp::BEGIN_LINE.0;
    } else if r1.is_none() {
        op |= EmptyOp::BEGIN_TEXT.0 | EmptyOp::BEGIN_LINE.0;
    }
    if r2.is_some_and(is_word_char) {
        boundary ^= 1;
    } else if r2 == Some('\n') {
        op |= EmptyOp::END_LINE.0;
    } else if r2.is_none() {
        op |= EmptyOp::END_TEXT.0 | EmptyOp::END_LINE.0;
    }
    if boundary != 0 {
        // IsWordChar(r1) != IsWordChar(r2)
        op ^= EmptyOp::WORD_BOUNDARY.0 | EmptyOp::NO_WORD_BOUNDARY.0;
    }
    EmptyOp(op)
}

/// Go `IsWordChar` (prog.go:108).
pub(super) fn is_word_char(r: char) -> bool {
    r.is_ascii_alphanumeric() || r == '_'
}

/// Go `Flags(i.Arg)&FoldCase != 0` (`Flags` is `uint16`, so `Arg` is truncated).
fn fold_case(arg: u32) -> bool {
    (Flags(arg as u16) & Flags::FOLD_CASE) != Flags(0)
}

/// Go `unicode.SimpleFold` on a Go `rune`: values that are not scalar values fold to themselves.
fn fold_rune(r: u32) -> u32 {
    char::from_u32(r).map_or(r, |c| simple_fold(c) as u32)
}

impl Prog {
    /// Go `Prog.skipNop` (prog.go:129): follows any no-op or capturing instructions.
    pub(crate) fn skip_nop(&self, pc: u32) -> &Inst {
        let mut i = &self.inst[pc as usize];
        while i.op == InstOp::Nop || i.op == InstOp::Capture {
            i = &self.inst[i.out as usize];
        }
        i
    }

    /// Go `Prog.Prefix` (prog.go:150): a literal string that all matches for the regexp must start
    /// with, and whether that prefix is the entire match.
    pub fn prefix(&self) -> (String, bool) {
        let mut i = self.skip_nop(self.start as u32);

        // Avoid allocation of buffer if prefix is empty.
        if i.op() != InstOp::Rune || i.rune.len() != 1 {
            return (String::new(), i.op == InstOp::Match);
        }

        // Have prefix; gather characters.
        let mut buf = String::new();
        while i.op() == InstOp::Rune
            && i.rune.len() == 1
            && !fold_case(i.arg)
            && i.rune[0] != 0xFFFD
        {
            // Go `WriteRune` writes U+FFFD for a value that is not a scalar value.
            buf.push(char::from_u32(i.rune[0]).unwrap_or('\u{FFFD}'));
            i = self.skip_nop(i.out);
        }
        (buf, i.op == InstOp::Match)
    }

    /// Go `Prog.StartCond` (prog.go:169): the leading empty-width conditions that must be true in
    /// any match. Returns `^EmptyOp(0)` if no matches are possible.
    pub fn start_cond(&self) -> EmptyOp {
        let mut flag = 0u8;
        let mut i = &self.inst[self.start];
        loop {
            match i.op {
                InstOp::EmptyWidth => flag |= i.arg as u8,
                InstOp::Fail => return EmptyOp(!0),
                InstOp::Capture | InstOp::Nop => {} // skip
                _ => break,
            }
            i = &self.inst[i.out as usize];
        }
        EmptyOp(flag)
    }
}

impl Inst {
    /// Go `Inst.op` (prog.go:138): `self.op` with all the Rune special cases merged into `Rune`.
    pub(crate) fn op(&self) -> InstOp {
        match self.op {
            InstOp::Rune1 | InstOp::RuneAny | InstOp::RuneAnyNotNL => InstOp::Rune,
            op => op,
        }
    }

    /// Go `Inst.MatchRune` (prog.go:195): reports whether the instruction matches (and consumes)
    /// `r`. Only call when `self.op == InstOp::Rune`.
    pub fn match_rune(&self, r: char) -> bool {
        self.match_rune_pos(r).is_some()
    }

    /// Go `Inst.MatchRunePos` (prog.go:204): the index of the matching rune pair (or, when
    /// `rune.len() == 1`, rune singleton), or `None` (Go's -1). Only call when
    /// `self.op == InstOp::Rune`.
    pub fn match_rune_pos(&self, r: char) -> Option<usize> {
        let r = r as u32;
        let rune = &self.rune;

        match rune.len() {
            0 => return None,

            1 => {
                // Special case: single-rune slice is from literal string, not char class.
                let r0 = rune[0];
                if r == r0 {
                    return Some(0);
                }
                if fold_case(self.arg) {
                    let mut r1 = fold_rune(r0);
                    while r1 != r0 {
                        if r == r1 {
                            return Some(0);
                        }
                        r1 = fold_rune(r1);
                    }
                }
                return None;
            }

            2 => {
                if (rune[0]..=rune[1]).contains(&r) {
                    return Some(0);
                }
                return None;
            }

            4 | 6 | 8 => {
                // Linear search for a few pairs.
                // Should handle ASCII well.
                let (pairs, _) = rune.as_chunks::<2>();
                for (j, [lo, hi]) in pairs.iter().enumerate() {
                    if r < *lo {
                        return None;
                    }
                    if r <= *hi {
                        return Some(j);
                    }
                }
                return None;
            }

            _ => {}
        }

        // Otherwise binary search.
        let mut lo = 0;
        let mut hi = rune.len() / 2;
        while lo < hi {
            let m = (lo + hi) >> 1;
            let c = rune[2 * m];
            if c <= r {
                if r <= rune[2 * m + 1] {
                    return Some(m);
                }
                lo = m + 1;
            } else {
                hi = m;
            }
        }
        None
    }

    /// Go `Inst.MatchEmptyWidth` (prog.go:266): reports whether the instruction matches an empty
    /// string between the runes `before` and `after` (`None` is Go's -1). Only call when
    /// `self.op == InstOp::EmptyWidth`.
    pub fn match_empty_width(&self, before: Option<char>, after: Option<char>) -> bool {
        let word = |r: Option<char>| r.is_some_and(is_word_char);
        match EmptyOp(self.arg as u8) {
            EmptyOp::BEGIN_LINE => before == Some('\n') || before.is_none(),
            EmptyOp::END_LINE => after == Some('\n') || after.is_none(),
            EmptyOp::BEGIN_TEXT => before.is_none(),
            EmptyOp::END_TEXT => after.is_none(),
            EmptyOp::WORD_BOUNDARY => word(before) != word(after),
            EmptyOp::NO_WORD_BOUNDARY => word(before) == word(after),
            _ => panic!("unknown empty width arg"),
        }
    }
}

impl fmt::Display for Inst {
    /// Go `Inst.String` (prog.go:284).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        dump_inst(f, self)
    }
}

/// Go `dumpInst` (prog.go:316).
fn dump_inst(f: &mut fmt::Formatter<'_>, i: &Inst) -> fmt::Result {
    match i.op {
        InstOp::Alt => write!(f, "alt -> {}, {}", i.out, i.arg),
        InstOp::AltMatch => write!(f, "altmatch -> {}, {}", i.out, i.arg),
        InstOp::Capture => write!(f, "cap {} -> {}", i.arg, i.out),
        InstOp::EmptyWidth => write!(f, "empty {} -> {}", i.arg, i.out),
        InstOp::Match => f.write_str("match"),
        InstOp::Fail => f.write_str("fail"),
        InstOp::Nop => write!(f, "nop -> {}", i.out),
        InstOp::Rune => {
            // Go also prints "rune <nil>" first when `i.Rune == nil` ("shouldn't happen"); a Rust
            // `Vec` cannot tell nil from empty, so that branch is omitted.
            write!(f, "rune {}", quote_to_ascii(&i.rune))?;
            if fold_case(i.arg) {
                f.write_str("/i")?;
            }
            write!(f, " -> {}", i.out)
        }
        InstOp::Rune1 => write!(f, "rune1 {} -> {}", quote_to_ascii(&i.rune), i.out),
        InstOp::RuneAny => write!(f, "any -> {}", i.out),
        InstOp::RuneAnyNotNL => write!(f, "anynotnl -> {}", i.out),
    }
}

/// Go `strconv.QuoteToASCII(string(r))` (quote.go:`appendQuotedWith`, `appendEscapedRune`):
/// `string([]rune)` turns values that are not scalar values into U+FFFD.
fn quote_to_ascii(runes: &[u32]) -> String {
    let mut s = String::from("\"");
    for &r in runes {
        let r = if char::from_u32(r).is_some() {
            r
        } else {
            0xFFFD
        };
        if r == '"' as u32 || r == '\\' as u32 {
            s.push('\\');
            s.push(r as u8 as char);
        } else if (0x20..0x7F).contains(&r) {
            s.push(r as u8 as char);
        } else {
            match r {
                0x07 => s.push_str("\\a"),
                0x08 => s.push_str("\\b"),
                0x0C => s.push_str("\\f"),
                0x0A => s.push_str("\\n"),
                0x0D => s.push_str("\\r"),
                0x09 => s.push_str("\\t"),
                0x0B => s.push_str("\\v"),
                _ if r < 0x20 || r == 0x7F => s.push_str(&format!("\\x{r:02x}")),
                _ if r < 0x10000 => s.push_str(&format!("\\u{r:04x}")),
                _ => s.push_str(&format!("\\U{r:08x}")),
            }
        }
    }
    s.push('"');
    s
}
