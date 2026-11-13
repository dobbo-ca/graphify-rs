//! Port of Go 1.27.4 regexp/syntax/regexp.go: Regexp methods, Equal, String (writeRegexp), MaxCap, CapNames.
//!
//! Go keys its `flags` map by `*Regexp`; here the key is the node's address, which is stable
//! because the tree is only borrowed while `Display` runs. Go can share one node between several
//! parents (Simplify does); an owned Rust tree cannot, so such nodes are distinct keys here.

use super::parse::in_char_class;
use super::{Flags, MAX_FOLD, MIN_FOLD, Op, Regexp};
use crate::unicode;
use std::collections::HashMap;
use std::fmt;

/// Go `printFlags` (regexp.go:103): bit set of flags (and non-capturing parens) to print around a regexp.
type PrintFlags = u8;
/// Go `flagI` (regexp.go:107): `(?i:`.
const FLAG_I: PrintFlags = 1 << 0;
/// Go `flagM` (regexp.go:108): `(?m:`.
const FLAG_M: PrintFlags = 1 << 1;
/// Go `flagS` (regexp.go:109): `(?s:`.
const FLAG_S: PrintFlags = 1 << 2;
/// Go `flagOff` (regexp.go:110): `)`.
const FLAG_OFF: PrintFlags = 1 << 3;
/// Go `flagPrec` (regexp.go:111): `(?: )`.
const FLAG_PREC: PrintFlags = 1 << 4;
/// Go `negShift` (regexp.go:112): `flagI << NEG_SHIFT` is `(?-i:`.
const NEG_SHIFT: u32 = 5;
/// Go `unicode.MaxRune`.
const MAX_RUNE: u32 = 0x10FFFF;
/// `minFold` / `maxFold` as Go `rune` values.
const MIN_FOLD_RUNE: u32 = MIN_FOLD as u32;
const MAX_FOLD_RUNE: u32 = MAX_FOLD as u32;
/// Go `meta` (regexp.go:372).
const META: &str = r"\.+*?()|[]{}^$";

type FlagMap = HashMap<*const Regexp, PrintFlags>;

/// Go `unicode.SimpleFold` on a Go `rune`: values that are not a `char` (surrogates) have no fold orbit.
fn simple_fold(r: u32) -> u32 {
    char::from_u32(r).map_or(r, |c| unicode::simple_fold(c) as u32)
}

/// Go `Regexp.Equal` (regexp.go:64). Also called by parse.rs (`factor`, `cleanAlt`-adjacent code).
pub(super) fn equal(x: &Regexp, y: &Regexp) -> bool {
    if x.op != y.op {
        return false;
    }
    match x.op {
        Op::EndText => {
            // The parse flags remember whether this is \z or \Z.
            if x.flags & Flags::WAS_DOLLAR != y.flags & Flags::WAS_DOLLAR {
                return false;
            }
        }
        Op::Literal | Op::CharClass => {
            return x.flags & Flags::FOLD_CASE == y.flags & Flags::FOLD_CASE && x.rune == y.rune;
        }
        Op::Alternate | Op::Concat => {
            return x.sub.len() == y.sub.len()
                && x.sub.iter().zip(&y.sub).all(|(a, b)| equal(a, b));
        }
        Op::Star | Op::Plus | Op::Quest => {
            if x.flags & Flags::NON_GREEDY != y.flags & Flags::NON_GREEDY
                || !equal(&x.sub[0], &y.sub[0])
            {
                return false;
            }
        }
        Op::Repeat => {
            if x.flags & Flags::NON_GREEDY != y.flags & Flags::NON_GREEDY
                || x.min != y.min
                || x.max != y.max
                || !equal(&x.sub[0], &y.sub[0])
            {
                return false;
            }
        }
        Op::Capture => {
            if x.cap != y.cap || x.name != y.name || !equal(&x.sub[0], &y.sub[0]) {
                return false;
            }
        }
        _ => {}
    }
    true
}

/// Go `addSpan` (regexp.go:116): enables the flags `f` around `start..last`, by setting
/// `flags[start] = f` and `flags[last] |= flagOff` (maybe `start == last`).
fn add_span(start: &Regexp, last: &Regexp, f: PrintFlags, flags: &mut FlagMap) {
    flags.insert(std::ptr::from_ref(start), f);
    *flags.entry(std::ptr::from_ref(last)).or_insert(0) |= FLAG_OFF;
}

/// Go `calcFlags` (regexp.go:129): calculates the flags to print around each subexpression in
/// `re`, storing them in `flags[sub]`, and returns the flags that must (`must`) or can't (`cant`)
/// be active around `re`.
fn calc_flags(re: &Regexp, flags: &mut FlagMap) -> (PrintFlags, PrintFlags) {
    match re.op {
        Op::Literal => {
            // If literal is fold-sensitive, return (flagI, 0) or (0, flagI)
            // according to whether (?i) is active.
            // If literal is not fold-sensitive, return 0, 0.
            for &r in &re.rune {
                if (MIN_FOLD_RUNE..=MAX_FOLD_RUNE).contains(&r) && simple_fold(r) != r {
                    return if re.flags & Flags::FOLD_CASE != Flags(0) {
                        (FLAG_I, 0)
                    } else {
                        (0, FLAG_I)
                    };
                }
            }
            (0, 0)
        }
        Op::CharClass => {
            // If literal is fold-sensitive, return 0, flagI - (?i) has been compiled out.
            // If literal is not fold-sensitive, return 0, 0.
            for i in (0..re.rune.len()).step_by(2) {
                let lo = MIN_FOLD_RUNE.max(re.rune[i]);
                let hi = MAX_FOLD_RUNE.min(re.rune[i + 1]);
                for r in lo..=hi {
                    let mut f = simple_fold(r);
                    while f != r {
                        if !(lo..=hi).contains(&f) && !in_char_class(f, &re.rune) {
                            return (0, FLAG_I);
                        }
                        f = simple_fold(f);
                    }
                }
            }
            (0, 0)
        }
        // (?-s).
        Op::AnyCharNotNL => (0, FLAG_S),
        // (?s).
        Op::AnyChar => (FLAG_S, 0),
        // (?m)^ (?m)$
        Op::BeginLine | Op::EndLine => (FLAG_M, 0),
        Op::EndText => {
            if re.flags & Flags::WAS_DOLLAR != Flags(0) {
                // (?-m)$
                (0, FLAG_M)
            } else {
                (0, 0)
            }
        }
        Op::Capture | Op::Star | Op::Plus | Op::Quest | Op::Repeat => calc_flags(&re.sub[0], flags),
        Op::Concat | Op::Alternate => {
            // Gather the must and cant for each subexpression.
            // When we find a conflicting subexpression, insert the necessary
            // flags around the previously identified span and start over.
            let (mut must, mut cant, mut all_cant): (PrintFlags, PrintFlags, PrintFlags) =
                (0, 0, 0);
            let mut start = 0;
            let mut last = 0;
            let mut did = false;
            for (i, sub) in re.sub.iter().enumerate() {
                let (sub_must, sub_cant) = calc_flags(sub, flags);
                if must & sub_cant != 0 || sub_must & cant != 0 {
                    if must != 0 {
                        add_span(&re.sub[start], &re.sub[last], must, flags);
                    }
                    must = 0;
                    cant = 0;
                    start = i;
                    did = true;
                }
                must |= sub_must;
                cant |= sub_cant;
                all_cant |= sub_cant;
                if sub_must != 0 {
                    last = i;
                }
                if must == 0 && start == i {
                    start += 1;
                }
            }
            if !did {
                // No conflicts: pass the accumulated must and cant upward.
                return (must, cant);
            }
            if must != 0 {
                // Conflicts found; need to finish final span.
                add_span(&re.sub[start], &re.sub[last], must, flags);
            }
            (0, all_cant)
        }
        _ => (0, 0),
    }
}

/// Writes `lo-hi` (or just `lo`, or `lo` `hi` when adjacent) of a class range, as in the
/// two identical loops of Go `writeRegexp` (regexp.go:285, regexp.go:296).
fn escape_range(b: &mut String, lo: u32, hi: u32) {
    escape(b, lo, lo == '-' as u32);
    if lo != hi {
        if hi != lo + 1 {
            b.push('-');
        }
        escape(b, hi, hi == '-' as u32);
    }
}

/// Go `writeRegexp` (regexp.go:225): writes the Perl syntax for the regular expression `re` to `b`.
/// Go's two `defer b.WriteString(")")` become closing parens written after the switch (both are `)`).
fn write_regexp(b: &mut String, re: &Regexp, f: PrintFlags, flags: &FlagMap) {
    let mut f = f | flags.get(&std::ptr::from_ref(re)).copied().unwrap_or(0);
    if f & FLAG_PREC != 0 && f & !(FLAG_OFF | FLAG_PREC) != 0 && f & FLAG_OFF != 0 {
        // flagPrec is redundant with other flags being added and terminated
        f &= !FLAG_PREC;
    }
    if f & !(FLAG_OFF | FLAG_PREC) != 0 {
        b.push_str("(?");
        if f & FLAG_I != 0 {
            b.push('i');
        }
        if f & FLAG_M != 0 {
            b.push('m');
        }
        if f & FLAG_S != 0 {
            b.push('s');
        }
        if f & ((FLAG_M | FLAG_S) << NEG_SHIFT) != 0 {
            b.push('-');
            if f & (FLAG_M << NEG_SHIFT) != 0 {
                b.push('m');
            }
            if f & (FLAG_S << NEG_SHIFT) != 0 {
                b.push('s');
            }
        }
        b.push(':');
    }
    if f & FLAG_PREC != 0 {
        b.push_str("(?:");
    }

    match re.op {
        Op::NoMatch => b.push_str(r"[^\x00-\x{10FFFF}]"),
        Op::EmptyMatch => b.push_str("(?:)"),
        Op::Literal => {
            for &r in &re.rune {
                escape(b, r, false);
            }
        }
        Op::CharClass => {
            if !re.rune.len().is_multiple_of(2) {
                b.push_str("[invalid char class]");
            } else {
                b.push('[');
                if re.rune.is_empty() {
                    b.push_str(r"^\x00-\x{10FFFF}");
                } else if re.rune[0] == 0
                    && re.rune[re.rune.len() - 1] == MAX_RUNE
                    && re.rune.len() > 2
                {
                    // Contains 0 and MaxRune. Probably a negated class.
                    // Print the gaps.
                    b.push('^');
                    for i in (1..re.rune.len() - 1).step_by(2) {
                        escape_range(b, re.rune[i] + 1, re.rune[i + 1] - 1);
                    }
                } else {
                    for i in (0..re.rune.len()).step_by(2) {
                        escape_range(b, re.rune[i], re.rune[i + 1]);
                    }
                }
                b.push(']');
            }
        }
        Op::AnyCharNotNL | Op::AnyChar => b.push('.'),
        Op::BeginLine => b.push('^'),
        Op::EndLine => b.push('$'),
        Op::BeginText => b.push_str(r"\A"),
        Op::EndText => {
            if re.flags & Flags::WAS_DOLLAR != Flags(0) {
                b.push('$');
            } else {
                b.push_str(r"\z");
            }
        }
        Op::WordBoundary => b.push_str(r"\b"),
        Op::NoWordBoundary => b.push_str(r"\B"),
        Op::Capture => {
            if re.name.is_empty() {
                b.push('(');
            } else {
                b.push_str("(?P<");
                b.push_str(&re.name);
                b.push('>');
            }
            if re.sub[0].op != Op::EmptyMatch {
                let sub_flags = flags
                    .get(&std::ptr::from_ref(&re.sub[0]))
                    .copied()
                    .unwrap_or(0);
                write_regexp(b, &re.sub[0], sub_flags, flags);
            }
            b.push(')');
        }
        Op::Star | Op::Plus | Op::Quest | Op::Repeat => {
            let mut p = 0;
            let sub = &re.sub[0];
            if sub.op as u8 > Op::Capture as u8 || sub.op == Op::Literal && sub.rune.len() > 1 {
                p = FLAG_PREC;
            }
            write_regexp(b, sub, p, flags);

            match re.op {
                Op::Star => b.push('*'),
                Op::Plus => b.push('+'),
                Op::Quest => b.push('?'),
                _ => {
                    b.push('{');
                    b.push_str(&re.min.to_string());
                    if re.max != re.min {
                        b.push(',');
                        if re.max >= 0 {
                            b.push_str(&re.max.to_string());
                        }
                    }
                    b.push('}');
                }
            }
            if re.flags & Flags::NON_GREEDY != Flags(0) {
                b.push('?');
            }
        }
        Op::Concat => {
            for sub in &re.sub {
                let p = if sub.op == Op::Alternate {
                    FLAG_PREC
                } else {
                    0
                };
                write_regexp(b, sub, p, flags);
            }
        }
        Op::Alternate => {
            for (i, sub) in re.sub.iter().enumerate() {
                if i > 0 {
                    b.push('|');
                }
                write_regexp(b, sub, 0, flags);
            }
        }
        Op::Pseudo => {
            b.push_str("<invalid op");
            b.push_str(&(re.op as u8).to_string());
            b.push('>');
        }
    }

    if f & FLAG_PREC != 0 {
        b.push(')');
    }
    if f & FLAG_OFF != 0 {
        b.push(')');
    }
}

/// Go `Regexp.String` (regexp.go:384): writes the same text into `f`.
pub(super) fn write_string(re: &Regexp, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut b = String::new();
    let mut flags = FlagMap::new();
    let (mut must, cant) = calc_flags(re, &mut flags);
    must |= (cant & !FLAG_I) << NEG_SHIFT;
    if must != 0 {
        must |= FLAG_OFF;
    }
    write_regexp(&mut b, re, must, &flags);
    f.write_str(&b)
}

/// Go `escape` (regexp.go:398). `r` is a Go `rune`; a surrogate is not printable.
fn escape(b: &mut String, r: u32, force: bool) {
    if let Some(c) = char::from_u32(r)
        && unicode::is_print(c)
    {
        if META.contains(c) || force {
            b.push('\\');
        }
        b.push(c);
        return;
    }

    match r {
        0x07 => b.push_str(r"\a"),
        0x0C => b.push_str(r"\f"),
        0x0A => b.push_str(r"\n"),
        0x0D => b.push_str(r"\r"),
        0x09 => b.push_str(r"\t"),
        0x0B => b.push_str(r"\v"),
        _ => {
            if r < 0x100 {
                b.push_str(&format!(r"\x{r:02x}"));
            } else {
                b.push_str(&format!(r"\x{{{r:x}}}"));
            }
        }
    }
}

/// Go `Regexp.MaxCap` (regexp.go:437).
pub(super) fn max_cap(re: &Regexp) -> i32 {
    let mut m = if re.op == Op::Capture { re.cap } else { 0 };
    for sub in &re.sub {
        m = m.max(max_cap(sub));
    }
    m
}

/// Go `Regexp.CapNames` (regexp.go:451): index 0 is "", index i is capture i's name.
pub(super) fn cap_names(re: &Regexp) -> Vec<String> {
    let mut names = vec![String::new(); max_cap(re) as usize + 1];
    fill_cap_names(re, &mut names);
    names
}

/// Go `Regexp.capNames` (regexp.go:457).
fn fill_cap_names(re: &Regexp, names: &mut [String]) {
    if re.op == Op::Capture {
        names[re.cap as usize].clone_from(&re.name);
    }
    for sub in &re.sub {
        fill_cap_names(sub, names);
    }
}
