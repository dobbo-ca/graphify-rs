//! Port of regexp/syntax/parse_test.go (Go 1.27.4).
//!
//! TestParseSimple: ported
//! TestParseFoldCase: ported
//! TestParseLiteral: ported
//! TestParseMatchNL: ported
//! TestParseNoMatchNL: ported
//! TestFoldConstants: ported (SimpleFold is gostd::unicode::simple_fold)
//! TestAppendRangeCollapse: not ported: internal appendRange (pub(crate), not reachable from an integration test)
//! TestParseInvalidRegexps: ported (4 invalid-UTF-8 rows left out: parse takes &str, so a Rust caller cannot pass them)
//! TestToStringEquivalentParse: ported (Regexp.String is Display for Regexp)
//! TestString: ported
//!
//! error_text_matches_go is not in Go: it pins the Display text of four parse errors.
//!
//! Test-only helpers dump, dump_regexp, mk_char_class and is_upper_fold are ports of the
//! Go helpers of the same names. mk_char_class walks every code point up to U+10FFFF as Go does;
//! a surrogate has no `char`, so each predicate gives Go's answer for it directly: a surrogate is
//! not in Lu, not in any script and not in Cn, so `\p{Assigned}` includes it. Go's `\p{Assigned}`
//! class therefore has a range starting at 0xd800 (Cn holds 0xd7fc-0xd7ff); `Regexp.rune` holds
//! Go rune values (`u32`) so that range is representable.

use std::fmt::Write;

use gostd::regexp::syntax::{Flags, MAX_FOLD, MIN_FOLD, Op, Regexp, parse};
use gostd::unicode::{category, is, is_upper, script, simple_fold};

type ParseTest = (String, String);

fn has(f: Flags, x: Flags) -> bool {
    (f & x) != Flags(0)
}

fn dump(re: &Regexp) -> String {
    let mut b = String::new();
    dump_regexp(&mut b, re);
    b
}

fn op_name(op: Op) -> Option<&'static str> {
    Some(match op {
        Op::NoMatch => "no",
        Op::EmptyMatch => "emp",
        Op::Literal => "lit",
        Op::CharClass => "cc",
        Op::AnyCharNotNL => "dnl",
        Op::AnyChar => "dot",
        Op::BeginLine => "bol",
        Op::EndLine => "eol",
        Op::BeginText => "bot",
        Op::EndText => "eot",
        Op::WordBoundary => "wb",
        Op::NoWordBoundary => "nwb",
        Op::Capture => "cap",
        Op::Star => "star",
        Op::Plus => "plus",
        Op::Quest => "que",
        Op::Repeat => "rep",
        Op::Concat => "cat",
        Op::Alternate => "alt",
        Op::Pseudo => return None,
    })
}

fn dump_regexp(b: &mut String, re: &Regexp) {
    match op_name(re.op) {
        None => {
            let n = re.op as u8;
            write!(b, "op{n}").unwrap();
        }
        Some(name) => match re.op {
            Op::Star | Op::Plus | Op::Quest | Op::Repeat => {
                if has(re.flags, Flags::NON_GREEDY) {
                    b.push('n');
                }
                b.push_str(name);
            }
            Op::Literal => {
                if re.rune.len() > 1 {
                    b.push_str("str");
                } else {
                    b.push_str("lit");
                }
                if has(re.flags, Flags::FOLD_CASE)
                    && re
                        .rune
                        .iter()
                        .any(|&r| char::from_u32(r).is_some_and(|c| simple_fold(c) as u32 != r))
                {
                    b.push_str("fold");
                }
            }
            _ => b.push_str(name),
        },
    }
    b.push('{');
    match re.op {
        Op::EndText => {
            if !has(re.flags, Flags::WAS_DOLLAR) {
                b.push_str(r"\z");
            }
        }
        // Go's WriteRune writes U+FFFD for a surrogate.
        Op::Literal => b.extend(
            re.rune
                .iter()
                .map(|&r| char::from_u32(r).unwrap_or('\u{FFFD}')),
        ),
        Op::Concat | Op::Alternate => {
            for sub in &re.sub {
                dump_regexp(b, sub);
            }
        }
        Op::Star | Op::Plus | Op::Quest => dump_regexp(b, &re.sub[0]),
        Op::Repeat => {
            write!(b, "{},{} ", re.min, re.max).unwrap();
            dump_regexp(b, &re.sub[0]);
        }
        Op::Capture => {
            if !re.name.is_empty() {
                b.push_str(&re.name);
                b.push(':');
            }
            dump_regexp(b, &re.sub[0]);
        }
        Op::CharClass => {
            let mut sep = "";
            for &[lo, hi] in re.rune.as_chunks::<2>().0 {
                b.push_str(sep);
                sep = " ";
                if lo == hi {
                    write!(b, "{lo:#x}").unwrap();
                } else {
                    write!(b, "{lo:#x}-{hi:#x}").unwrap();
                }
            }
        }
        _ => {}
    }
    b.push('}');
}

/// Go mkCharClass: dump of the OpCharClass whose ranges cover every code point where `f` is true.
fn mk_char_class(f: impl Fn(u32) -> bool) -> String {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let mut lo: Option<u32> = None;
    for i in 0..=0x10FFFFu32 {
        if f(i) {
            if lo.is_none() {
                lo = Some(i);
            }
        } else if let Some(l) = lo.take() {
            ranges.push((l, i - 1));
        }
    }
    if let Some(l) = lo {
        ranges.push((l, 0x10FFFF));
    }
    let mut b = String::from("cc{");
    let mut sep = "";
    for (lo, hi) in ranges {
        b.push_str(sep);
        sep = " ";
        if lo == hi {
            write!(b, "{lo:#x}").unwrap();
        } else {
            write!(b, "{lo:#x}-{hi:#x}").unwrap();
        }
    }
    b.push('}');
    b
}

fn is_upper_fold(r: u32) -> bool {
    let Some(r) = char::from_u32(r) else {
        return false;
    };
    if is_upper(r) {
        return true;
    }
    let mut c = simple_fold(r);
    while c != r {
        if is_upper(c) {
            return true;
        }
        c = simple_fold(c);
    }
    false
}

fn test_parse_dump(tests: &[ParseTest], flags: Flags) {
    let mut errs = Vec::new();
    for (regexp, want) in tests {
        let re = match parse(regexp, flags) {
            Ok(re) => re,
            Err(err) => {
                errs.push(format!("Parse({regexp:?}): {err}"));
                continue;
            }
        };
        if want.is_empty() {
            // It parsed. That's all we care about.
            continue;
        }
        let d = dump(&re);
        if d != *want {
            errs.push(format!("Parse({regexp:?}).Dump() = {d:?} want {want:?}"));
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

fn to_tests(rows: &[(&str, &str)]) -> Vec<ParseTest> {
    rows.iter()
        .map(|&(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

/// Go parseTests (parse_test.go:19), all rows kept, in order except the mkCharClass rows and the
/// strings.Repeat rows, which are appended at the end.
fn parse_tests() -> Vec<ParseTest> {
    let mut v = to_tests(&[
        // Base cases
        (r"a", r"lit{a}"),
        (r"a.", r"cat{lit{a}dot{}}"),
        (r"a.b", r"cat{lit{a}dot{}lit{b}}"),
        (r"ab", r"str{ab}"),
        (r"a.b.c", r"cat{lit{a}dot{}lit{b}dot{}lit{c}}"),
        (r"abc", r"str{abc}"),
        (r"a|^", r"alt{lit{a}bol{}}"),
        (r"a|b", r"cc{0x61-0x62}"),
        (r"(a)", r"cap{lit{a}}"),
        (r"(a)|b", r"alt{cap{lit{a}}lit{b}}"),
        (r"a*", r"star{lit{a}}"),
        (r"a+", r"plus{lit{a}}"),
        (r"a?", r"que{lit{a}}"),
        (r"a{2}", r"rep{2,2 lit{a}}"),
        (r"a{2,3}", r"rep{2,3 lit{a}}"),
        (r"a{2,}", r"rep{2,-1 lit{a}}"),
        (r"a*?", r"nstar{lit{a}}"),
        (r"a+?", r"nplus{lit{a}}"),
        (r"a??", r"nque{lit{a}}"),
        (r"a{2}?", r"nrep{2,2 lit{a}}"),
        (r"a{2,3}?", r"nrep{2,3 lit{a}}"),
        (r"a{2,}?", r"nrep{2,-1 lit{a}}"),
        // Malformed { } are treated as literals.
        (r"x{1001", r"str{x{1001}"),
        (r"x{9876543210", r"str{x{9876543210}"),
        (r"x{9876543210,", r"str{x{9876543210,}"),
        (r"x{2,1", r"str{x{2,1}"),
        (r"x{1,9876543210", r"str{x{1,9876543210}"),
        (r"", r"emp{}"),
        (r"|", r"emp{}"), // alt{emp{}emp{}} but got factored
        (r"|x|", r"alt{emp{}lit{x}emp{}}"),
        (r".", r"dot{}"),
        (r"^", r"bol{}"),
        (r"$", r"eol{}"),
        (r"\|", r"lit{|}"),
        (r"\(", r"lit{(}"),
        (r"\)", r"lit{)}"),
        (r"\*", r"lit{*}"),
        (r"\+", r"lit{+}"),
        (r"\?", r"lit{?}"),
        (r"{", r"lit{{}"),
        (r"}", r"lit{}}"),
        (r"\.", r"lit{.}"),
        (r"\^", r"lit{^}"),
        (r"\$", r"lit{$}"),
        (r"\\", r"lit{\}"),
        (r"[ace]", r"cc{0x61 0x63 0x65}"),
        (r"[abc]", r"cc{0x61-0x63}"),
        (r"[a-z]", r"cc{0x61-0x7a}"),
        (r"[a]", r"lit{a}"),
        (r"\-", r"lit{-}"),
        (r"-", r"lit{-}"),
        (r"\_", r"lit{_}"),
        (r"abc", r"str{abc}"),
        (r"abc|def", r"alt{str{abc}str{def}}"),
        (r"abc|def|ghi", r"alt{str{abc}str{def}str{ghi}}"),
        // Posix and Perl extensions
        (r"[[:lower:]]", r"cc{0x61-0x7a}"),
        (r"[a-z]", r"cc{0x61-0x7a}"),
        (r"[^[:lower:]]", r"cc{0x0-0x60 0x7b-0x10ffff}"),
        (r"[[:^lower:]]", r"cc{0x0-0x60 0x7b-0x10ffff}"),
        (r"(?i)[[:lower:]]", r"cc{0x41-0x5a 0x61-0x7a 0x17f 0x212a}"),
        (r"(?i)[a-z]", r"cc{0x41-0x5a 0x61-0x7a 0x17f 0x212a}"),
        (
            r"(?i)[^[:lower:]]",
            r"cc{0x0-0x40 0x5b-0x60 0x7b-0x17e 0x180-0x2129 0x212b-0x10ffff}",
        ),
        (
            r"(?i)[[:^lower:]]",
            r"cc{0x0-0x40 0x5b-0x60 0x7b-0x17e 0x180-0x2129 0x212b-0x10ffff}",
        ),
        (r"\d", r"cc{0x30-0x39}"),
        (r"\D", r"cc{0x0-0x2f 0x3a-0x10ffff}"),
        (r"\s", r"cc{0x9-0xa 0xc-0xd 0x20}"),
        (r"\S", r"cc{0x0-0x8 0xb 0xe-0x1f 0x21-0x10ffff}"),
        (r"\w", r"cc{0x30-0x39 0x41-0x5a 0x5f 0x61-0x7a}"),
        (
            r"\W",
            r"cc{0x0-0x2f 0x3a-0x40 0x5b-0x5e 0x60 0x7b-0x10ffff}",
        ),
        (
            r"(?i)\w",
            r"cc{0x30-0x39 0x41-0x5a 0x5f 0x61-0x7a 0x17f 0x212a}",
        ),
        (
            r"(?i)\W",
            r"cc{0x0-0x2f 0x3a-0x40 0x5b-0x5e 0x60 0x7b-0x17e 0x180-0x2129 0x212b-0x10ffff}",
        ),
        (r"[^\\]", r"cc{0x0-0x5b 0x5d-0x10ffff}"),
        // Unicode, negatives, and a double negative.
        (r"\p{Braille}", r"cc{0x2800-0x28ff}"),
        (r"\P{Braille}", r"cc{0x0-0x27ff 0x2900-0x10ffff}"),
        (r"\p{^Braille}", r"cc{0x0-0x27ff 0x2900-0x10ffff}"),
        (r"\P{^Braille}", r"cc{0x2800-0x28ff}"),
        (
            r"\pZ",
            r"cc{0x20 0xa0 0x1680 0x2000-0x200a 0x2028-0x2029 0x202f 0x205f 0x3000}",
        ),
        (r"[\p{Braille}]", r"cc{0x2800-0x28ff}"),
        (r"[\P{Braille}]", r"cc{0x0-0x27ff 0x2900-0x10ffff}"),
        (r"[\p{^Braille}]", r"cc{0x0-0x27ff 0x2900-0x10ffff}"),
        (r"[\P{^Braille}]", r"cc{0x2800-0x28ff}"),
        (
            r"[\pZ]",
            r"cc{0x20 0xa0 0x1680 0x2000-0x200a 0x2028-0x2029 0x202f 0x205f 0x3000}",
        ),
        (r"\p{Any}", r"dot{}"),
        (r"\p{^Any}", r"cc{}"),
        (r"(?i)\p{ascii}", r"cc{0x0-0x7f 0x17f 0x212a}"),
        // Hex, octal.
        (r"[\012-\234]\141", r"cat{cc{0xa-0x9c}lit{a}}"),
        (r"[\x{41}-\x7a]\x61", r"cat{cc{0x41-0x7a}lit{a}}"),
        // More interesting regular expressions.
        (r"a{,2}", r"str{a{,2}}"),
        (r"\.\^\$\\", r"str{.^$\}"),
        (r"[a-zABC]", r"cc{0x41-0x43 0x61-0x7a}"),
        (r"[^a]", r"cc{0x0-0x60 0x62-0x10ffff}"),
        (r"[α-ε☺]", r"cc{0x3b1-0x3b5 0x263a}"), // utf-8
        (r"a*{", r"cat{star{lit{a}}lit{{}}"),
        // Test precedences
        (r"(?:ab)*", r"star{str{ab}}"),
        (r"(ab)*", r"star{cap{str{ab}}}"),
        (r"ab|cd", r"alt{str{ab}str{cd}}"),
        (r"a(b|c)d", r"cat{lit{a}cap{cc{0x62-0x63}}lit{d}}"),
        // Test flattening.
        (r"(?:a)", r"lit{a}"),
        (r"(?:ab)(?:cd)", r"str{abcd}"),
        (
            r"(?:a+b+)(?:c+d+)",
            r"cat{plus{lit{a}}plus{lit{b}}plus{lit{c}}plus{lit{d}}}",
        ),
        (
            r"(?:a+|b+)|(?:c+|d+)",
            r"alt{plus{lit{a}}plus{lit{b}}plus{lit{c}}plus{lit{d}}}",
        ),
        (r"(?:a|b)|(?:c|d)", r"cc{0x61-0x64}"),
        (r"a|.", r"dot{}"),
        (r".|a", r"dot{}"),
        (
            r"(?:[abc]|A|Z|hello|world)",
            r"alt{cc{0x41 0x5a 0x61-0x63}str{hello}str{world}}",
        ),
        (r"(?:[abc]|A|Z)", r"cc{0x41 0x5a 0x61-0x63}"),
        // Test Perl quoted literals
        (r"\Q+|*?{[\E", r"str{+|*?{[}"),
        (r"\Q+\E+", r"plus{lit{+}}"),
        (r"\Qab\E+", r"cat{lit{a}plus{lit{b}}}"),
        (r"\Q\\E", r"lit{\}"),
        (r"\Q\\\E", r"str{\\}"),
        // Test Perl \A and \z
        (r"(?m)^", r"bol{}"),
        (r"(?m)$", r"eol{}"),
        (r"(?-m)^", r"bot{}"),
        (r"(?-m)$", r"eot{}"),
        (r"(?m)\A", r"bot{}"),
        (r"(?m)\z", r"eot{\z}"),
        (r"(?-m)\A", r"bot{}"),
        (r"(?-m)\z", r"eot{\z}"),
        // Test named captures
        (r"(?P<name>a)", r"cap{name:lit{a}}"),
        (r"(?<name>a)", r"cap{name:lit{a}}"),
        // Case-folded literals
        (r"[Aa]", r"litfold{A}"),
        (r"[\x{100}\x{101}]", r"litfold{Ā}"),
        (r"[Δδ]", r"litfold{Δ}"),
        // Strings
        (r"abcde", r"str{abcde}"),
        (r"[Aa][Bb]cd", r"cat{strfold{AB}str{cd}}"),
        // Factoring.
        (
            r"abc|abd|aef|bcx|bcy",
            r"alt{cat{lit{a}alt{cat{lit{b}cc{0x63-0x64}}str{ef}}}cat{str{bc}cc{0x78-0x79}}}",
        ),
        (
            r"ax+y|ax+z|ay+w",
            r"cat{lit{a}alt{cat{plus{lit{x}}lit{y}}cat{plus{lit{x}}lit{z}}cat{plus{lit{y}}lit{w}}}}",
        ),
        // Bug fixes.
        (r"(?:.)", r"dot{}"),
        (r"(?:x|(?:xa))", r"cat{lit{x}alt{emp{}lit{a}}}"),
        (r"(?:.|(?:.a))", r"cat{dot{}alt{emp{}lit{a}}}"),
        (r"(?:A(?:A|a))", r"cat{lit{A}litfold{A}}"),
        (r"(?:A|a)", r"litfold{A}"),
        (r"A|(?:A|a)", r"litfold{A}"),
        (r"(?s).", r"dot{}"),
        (r"(?-s).", r"dnl{}"),
        (r"(?:(?:^).)", r"cat{bol{}dot{}}"),
        (r"(?-s)(?:(?:^).)", r"cat{bol{}dnl{}}"),
        (r"[\s\S]a", r"cat{cc{0x0-0x10ffff}lit{a}}"),
        // RE2 prefix_tests
        (r"abc|abd", r"cat{str{ab}cc{0x63-0x64}}"),
        (r"a(?:b)c|abd", r"cat{str{ab}cc{0x63-0x64}}"),
        (
            r"abc|abd|aef|bcx|bcy",
            r"alt{cat{lit{a}alt{cat{lit{b}cc{0x63-0x64}}str{ef}}}cat{str{bc}cc{0x78-0x79}}}",
        ),
        (r"abc|x|abd", r"alt{str{abc}lit{x}str{abd}}"),
        (r"(?i)abc|ABD", r"cat{strfold{AB}cc{0x43-0x44 0x63-0x64}}"),
        (r"[ab]c|[ab]d", r"cat{cc{0x61-0x62}cc{0x63-0x64}}"),
        (r".c|.d", r"cat{dot{}cc{0x63-0x64}}"),
        (
            r"x{2}|x{2}[0-9]",
            r"cat{rep{2,2 lit{x}}alt{emp{}cc{0x30-0x39}}}",
        ),
        (
            r"x{2}y|x{2}[0-9]y",
            r"cat{rep{2,2 lit{x}}alt{lit{y}cat{cc{0x30-0x39}lit{y}}}}",
        ),
        (
            r"a.*?c|a.*?b",
            r"cat{lit{a}alt{cat{nstar{dot{}}lit{c}}cat{nstar{dot{}}lit{b}}}}",
        ),
        // Valid repetitions.
        (r"((((((((((x{2}){2}){2}){2}){2}){2}){2}){2}){2}))", r""),
        (r"((((((((((x{1}){2}){2}){2}){2}){2}){2}){2}){2}){2})", r""),
    ]);

    // Unicode classes whose expected dump Go computes with mkCharClass.
    let upper = |r: u32| char::from_u32(r).is_some_and(is_upper);
    let canadian = script("Canadian_Aboriginal").unwrap();
    let cn = category("Cn").unwrap();
    v.extend([
        (r"\p{Lu}".to_string(), mk_char_class(upper)),
        (r"\p{Uppercase_Letter}".to_string(), mk_char_class(upper)),
        (r"\p{upper case-let ter}".to_string(), mk_char_class(upper)),
        (
            r"\p{__upper case-let ter}".to_string(),
            mk_char_class(upper),
        ),
        (
            r"\p{Canadian_Aboriginal}".to_string(),
            mk_char_class(|r| char::from_u32(r).is_some_and(|c| is(canadian, c))),
        ),
        (r"[\p{Lu}]".to_string(), mk_char_class(upper)),
        (r"(?i)[\p{Lu}]".to_string(), mk_char_class(is_upper_fold)),
        // A surrogate is in no Cn, so it is assigned.
        (
            r"\p{Assigned}".to_string(),
            mk_char_class(|r| char::from_u32(r).is_none_or(|c| !is(cn, c))),
        ),
        (
            r"\p{^Assigned}".to_string(),
            mk_char_class(|r| char::from_u32(r).is_some_and(|c| is(cn, c))),
        ),
    ]);

    // Valid nesting.
    v.push(("(".repeat(999) + &")".repeat(999), String::new()));
    v.push(("(?:".repeat(999) + &")*".repeat(999), String::new()));
    v.push((format!("({})", "|".repeat(12345)), String::new())); // not nested at all
    v
}

fn test_flags() -> Flags {
    Flags::MATCH_NL | Flags::PERL_X | Flags::UNICODE_GROUPS
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_parse_simple() {
    test_parse_dump(&parse_tests(), test_flags());
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_parse_fold_case() {
    test_parse_dump(
        &to_tests(&[
            (r"AbCdE", r"strfold{ABCDE}"),
            (r"[Aa]", r"litfold{A}"),
            (r"a", r"litfold{A}"),
            // 0x17F is an old English long s (looks like an f) and folds to s.
            // 0x212A is the Kelvin symbol and folds to k.
            (r"A[F-g]", r"cat{litfold{A}cc{0x41-0x7a 0x17f 0x212a}}"), // [Aa][A-z...]
            (r"[[:upper:]]", r"cc{0x41-0x5a 0x61-0x7a 0x17f 0x212a}"),
            (r"[[:lower:]]", r"cc{0x41-0x5a 0x61-0x7a 0x17f 0x212a}"),
        ]),
        Flags::FOLD_CASE,
    );
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_parse_literal() {
    test_parse_dump(
        &to_tests(&[("(|)^$.[*+?]{5,10},\\", "str{(|)^$.[*+?]{5,10},\\}")]),
        Flags::LITERAL,
    );
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_parse_match_nl() {
    test_parse_dump(
        &to_tests(&[
            (r".", r"dot{}"),
            ("\n", "lit{\n}"),
            (r"[^a]", r"cc{0x0-0x60 0x62-0x10ffff}"),
            (r"[a\n]", r"cc{0xa 0x61}"),
        ]),
        Flags::MATCH_NL,
    );
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_parse_no_match_nl() {
    test_parse_dump(
        &to_tests(&[
            (r".", r"dnl{}"),
            ("\n", "lit{\n}"),
            (r"[^a]", r"cc{0x0-0x9 0xb-0x60 0x62-0x10ffff}"),
            (r"[a\n]", r"cc{0xa 0x61}"),
        ]),
        Flags::POSIX,
    );
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_fold_constants() {
    // Surrogates are skipped; Go's SimpleFold returns them unchanged.
    let mut folds = ('\0'..=char::MAX).filter(|&c| simple_fold(c) != c);
    let first = folds.next();
    let last = folds.next_back().or(first);
    assert_eq!(first, Some(MIN_FOLD), "MIN_FOLD should be {first:?}");
    assert_eq!(last, Some(MAX_FOLD), "MAX_FOLD should be {last:?}");
}

/// Go invalidRegexps (parse_test.go:464). Left out: "\xff", "[\xff]", "[\\\xff]", "\\\xff"
/// (invalid UTF-8; parse takes &str).
fn invalid_regexps() -> Vec<String> {
    let mut v: Vec<String> = [
        r"(",
        r")",
        r"(a",
        r"a)",
        r"(a))",
        r"(a|b|",
        r"a|b|)",
        r"(a|b|))",
        r"(a|b",
        r"a|b)",
        r"(a|b))",
        r"[a-z",
        r"([a-z)",
        r"[a-z)",
        r"([a-z]))",
        r"x{1001}",
        r"x{9876543210}",
        r"x{2,1}",
        r"x{1,9876543210}",
        // not ported: "\xff", "[\xff]", "[\\\xff]", "\\\xff" (invalid UTF-8 cannot be a &str)
        r"(?P<name>a",
        r"(?P<name>",
        r"(?P<name",
        r"(?P<x y>a)",
        r"(?P<>a)",
        r"(?<name>a",
        r"(?<name>",
        r"(?<name",
        r"(?<x y>a)",
        r"(?<>a)",
        r"[a-Z]",
        r"(?i)[a-Z]",
        r"\Q\E*",
        r"a{100000}",                                           // too much repetition
        r"a{100000,}",                                          // too much repetition
        r"((((((((((x{2}){2}){2}){2}){2}){2}){2}){2}){2}){2})", // too much repetition
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.push("(".repeat(1000) + &")".repeat(1000)); // too deep
    v.push("(?:".repeat(1000) + &")*".repeat(1000)); // too deep
    v.push(format!("({}){{1000}}", "(xx?)".repeat(1000))); // too long
    v.push("(xx?){1000}".repeat(1000)); // too long
    v.push(r"\pL".repeat(27000)); // too many runes
    v
}

const ONLY_PERL: &[&str] = &[
    r"[a-b-c]",
    r"\Qabc\E",
    r"\Q*+?{[\E",
    r"\Q\\E",
    r"\Q\\\E",
    r"\Q\\\\E",
    r"\Q\\\\\E",
    r"(?:a)",
    r"(?P<name>a)",
];

const ONLY_POSIX: &[&str] = &["a++", "a**", "a?*", "a+*", "a{1}*", ".{1}{2}.{3}"];

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_parse_invalid_regexps() {
    let mut errs = Vec::new();
    let mut want_err = |regexp: &str, flags: Flags, name: &str| {
        if let Ok(re) = parse(regexp, flags) {
            errs.push(format!(
                "Parse({regexp:?}, {name}) = {}, should have failed",
                dump(&re)
            ));
        }
    };
    for regexp in invalid_regexps() {
        want_err(&regexp, Flags::PERL, "Perl");
        want_err(&regexp, Flags::POSIX, "POSIX");
    }
    for regexp in ONLY_PERL {
        want_err(regexp, Flags::POSIX, "POSIX");
    }
    for regexp in ONLY_POSIX {
        want_err(regexp, Flags::PERL, "Perl");
    }
    for regexp in ONLY_PERL {
        if let Err(err) = parse(regexp, Flags::PERL) {
            errs.push(format!("Parse({regexp:?}, Perl): {err}"));
        }
    }
    for regexp in ONLY_POSIX {
        if let Err(err) = parse(regexp, Flags::POSIX) {
            errs.push(format!("Parse({regexp:?}, POSIX): {err}"));
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_to_string_equivalent_parse() {
    let mut errs = Vec::new();
    for (regexp, want) in parse_tests() {
        let re = match parse(&regexp, test_flags()) {
            Ok(re) => re,
            Err(err) => {
                errs.push(format!("Parse({regexp:?}): {err}"));
                continue;
            }
        };
        if want.is_empty() {
            // It parsed. That's all we care about.
            continue;
        }
        let d = dump(&re);
        if d != want {
            errs.push(format!("Parse({regexp:?}).Dump() = {d:?} want {want:?}"));
            continue;
        }

        let s = re.to_string();
        if s != regexp {
            // If ToString didn't return the original regexp,
            // it must have found one with fewer parens.
            // Unfortunately we can't check the length here, because
            // ToString produces "\\{" for a literal brace,
            // but "{" is a shorter equivalent in some contexts.
            let nre = match parse(&s, test_flags()) {
                Ok(nre) => nre,
                Err(err) => {
                    errs.push(format!("Parse({regexp:?}.String() = {s:?}): {err}"));
                    continue;
                }
            };
            let nd = dump(&nre);
            if d != nd {
                errs.push(format!("Parse({regexp:?}) -> {s:?}; {d:?} vs {nd:?}"));
            }

            let ns = nre.to_string();
            if s != ns {
                errs.push(format!("Parse({regexp:?}) -> {s:?} -> {ns:?}"));
            }
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

/// Go stringTests (parse_test.go:601).
const STRING_TESTS: &[(&str, &str)] = &[
    (r"x(?i:ab*c|d?e)1", r"x(?i:AB*C|D?E)1"),
    (r"x(?i:ab*cd?e)1", r"x(?i:AB*CD?E)1"),
    (r"0(?i:ab*c|d?e)1", r"(?i:0(?:AB*C|D?E)1)"),
    (r"0(?i:ab*cd?e)1", r"(?i:0AB*CD?E1)"),
    (r"x(?i:ab*c|d?e)", r"x(?i:AB*C|D?E)"),
    (r"x(?i:ab*cd?e)", r"x(?i:AB*CD?E)"),
    (r"0(?i:ab*c|d?e)", r"(?i:0(?:AB*C|D?E))"),
    (r"0(?i:ab*cd?e)", r"(?i:0AB*CD?E)"),
    (r"(?i:ab*c|d?e)1", r"(?i:(?:AB*C|D?E)1)"),
    (r"(?i:ab*cd?e)1", r"(?i:AB*CD?E1)"),
    (r"(?i:ab)[123](?i:cd)", r"(?i:AB[1-3]CD)"),
    (r"(?i:ab*c|d?e)", r"(?i:AB*C|D?E)"),
    (r"[Aa][Bb]", r"(?i:AB)"),
    (r"[Aa][Bb]*[Cc]", r"(?i:AB*C)"),
    (r"A(?:[Bb][Cc]|[Dd])[Zz]", r"A(?i:(?:BC|D)Z)"),
    (r"[Aa](?:[Bb][Cc]|[Dd])Z", r"(?i:A(?:BC|D))Z"),
];

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_string() {
    let mut errs = Vec::new();
    for &(re, want) in STRING_TESTS {
        match parse(re, Flags::PERL) {
            Err(err) => errs.push(format!("Parse({re:?}): {err}")),
            Ok(parsed) => {
                let out = parsed.to_string();
                if out != want {
                    errs.push(format!("Parse({re:?}).String() = {out:?}, want {want:?}"));
                }
            }
        }
    }
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

/// Not in Go. Expected values read from parse.go (Go 1.27.4):
/// - "a(b": the stack still holds a paren marker at the end of parse (parse.go:1091-1095),
///   ErrMissingParen with the whole pattern.
/// - "x{1001}": parseRepeat accepts it, but max > 1000 (parse.go:1004-1005) gives
///   ErrInvalidRepeatSize with only the repeat text "{1001}", not the whole pattern.
/// - backslash-8: parseEscape has no case for '8' (not an octal digit), so it falls out of the
///   switch to parse.go:1556, ErrInvalidEscape with s[:len(s)-len(t)], the backslash and the 8.
/// - "[a": parseClass runs out of input in parseClassChar (parse.go:1561-1563),
///   ErrMissingBracket with wholeClass, which is the class from "[" to the end: "[a".
#[test]
#[ignore = "graphify-rs-aa6.6"]
fn error_text_matches_go() {
    let cases = [
        ("a(b", "error parsing regexp: missing closing ): `a(b`"),
        (
            "x{1001}",
            "error parsing regexp: invalid repeat count: `{1001}`",
        ),
        (
            r"\8",
            r"error parsing regexp: invalid escape sequence: `\8`",
        ),
        ("[a", "error parsing regexp: missing closing ]: `[a`"),
    ];
    for (pattern, want) in cases {
        let err = parse(pattern, Flags::PERL).unwrap_err();
        assert_eq!(err.to_string(), want, "pattern {pattern:?}");
    }
}
