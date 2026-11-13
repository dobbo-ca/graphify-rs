//! Ported from `unicode/norm/normalize_test.go` (golang.org/x/text v0.27.0).
//!
//! Go Test funcs:
//! - TestDecomposeSegment: not ported: tests x/text internals not reached by graphify-go
//! - TestFirstBoundary: not ported: tests x/text internals not reached by graphify-go
//! - TestNextBoundary: not ported: tests x/text internals not reached by graphify-go
//! - TestDecomposeToLastBoundary: not ported: tests x/text internals not reached by graphify-go
//! - TestLastBoundary: not ported: tests x/text internals not reached by graphify-go
//! - TestSpan: not ported: tests x/text internals not reached by graphify-go
//! - TestIsNormal: not ported: tests x/text internals not reached by graphify-go
//! - TestIsNormalString: not ported: tests x/text internals not reached by graphify-go
//! - TestAppend: ported (NFKC and NFKD rows; see below)
//! - TestAppendString: ported (NFKC and NFKD rows; see below)
//! - TestBytes: ported (NFKC and NFKD rows)
//! - TestString: ported (NFKC and NFKD rows)
//! - TestLinking: not ported: tests x/text internals not reached by graphify-go
//!
//! The four ported tests run `runNormTests`: for each form f the table `appendTestsNF<f>`,
//! asserting `out == f(left + right)` (String, Bytes) or `out == f.Append(left, right)`
//! (Append, AppendString), plus a bootstrap check against every other form g:
//! `g.String(left + right) == fn(g, g.AppendString(nil, left), right)`.
//! Only NFKC and NFKD exist here, so f in {NFKC, NFKD} and g is the other one. The
//! `appendTestsNFC` rows (18) and the empty `appendTestsNFD` are not ported.
//!
//! Go's `Append(out, right)` treats `out` as already normalized, re-normalizing only its
//! last segment together with `right`; it is not defined as `f(left + right)`. The Rust API
//! has no append, so all four tests assert `f(left + right) == out`. Go's own table asserts
//! the same `out` for String(left + right) and Append(left, right) on every row, so the
//! expected strings are identical. The bootstrap check is expressed as
//! `g(g(left) + right) == g(left + right)`.
//!
//! Rows kept verbatim and in order: appendTestsNFKC 48 of 76, appendTestsNFKD 14 of 14.
//! The 28 NFKC rows left out have `left` or `right` that is not valid UTF-8 on its own
//! (some concatenate into valid UTF-8, but the parts cannot be `&str`); each is marked
//! `// not ported: invalid UTF-8` below.

use gostd::xtext::norm::{nfkc, nfkd};

const CGJ: &str = "\u{34F}";
const SEG_SIZE: usize = 128; // maxByteBufferSize = utf8.UTFMax * (maxNonStarters + 2)

type Row = (String, String, String);

fn rep(r: char, n: usize) -> String {
    r.to_string().repeat(n)
}

fn grave(n: usize) -> String {
    rep('\u{300}', n)
}

fn row(left: &str, right: &str, out: &str) -> Row {
    (left.to_owned(), right.to_owned(), out.to_owned())
}

fn append_tests_nfkc() -> Vec<Row> {
    vec![
        // empty buffers
        row("", "", ""),
        row("a", "", "a"),
        row("", "a", "a"),
        row("", "\u{41}\u{307}\u{304}", "\u{1E0}"),
        // segment split across buffers
        row("", "a\u{300}b", "\u{E0}b"),
        row("a", "\u{300}b", "\u{E0}b"),
        row("a", "\u{300}\u{316}", "\u{E0}\u{316}"),
        row("a", "\u{316}\u{300}", "\u{E0}\u{316}"),
        row("a", "\u{300}a\u{300}", "\u{E0}\u{E0}"),
        row("a", "\u{300}a\u{300}a\u{300}", "\u{E0}\u{E0}\u{E0}"),
        row("a", "\u{300}aaa\u{300}aaa\u{300}", "\u{E0}aa\u{E0}aa\u{E0}"),
        row("a\u{300}", "\u{327}", "\u{E0}\u{327}"),
        row("a\u{327}", "\u{300}", "\u{E0}\u{327}"),
        row("a\u{316}", "\u{300}", "\u{E0}\u{316}"),
        row("\u{41}\u{307}", "\u{304}", "\u{1E0}"),
        // Hangul
        row("", "\u{110B}\u{1173}", "\u{C73C}"),
        row("", "\u{1103}\u{1161}", "\u{B2E4}"),
        row("", "\u{110B}\u{1173}\u{11B7}", "\u{C74C}"),
        row("", "\u{320E}", "\x28\u{AC00}\x29"),
        row("", "\x28\u{1100}\u{1161}\x29", "\x28\u{AC00}\x29"),
        row("\u{1103}", "\u{1161}", "\u{B2E4}"),
        row("\u{110B}", "\u{1173}\u{11B7}", "\u{C74C}"),
        row("\u{110B}\u{1173}", "\u{11B7}", "\u{C74C}"),
        row("\u{C73C}", "\u{11B7}", "\u{C74C}"),
        // UTF-8 encoding split across buffers
        // not ported: invalid UTF-8 ("a\xCC", "\x80", ...)
        // not ported: invalid UTF-8 ("a\xCC", "\x80b", ...)
        // not ported: invalid UTF-8 ("a\xCC", "\x80à", ...)
        // not ported: invalid UTF-8 ("a\xCC", "\x80\x80", ...)
        // not ported: invalid UTF-8 ("a\xCC", "\x80\xCC", ...)
        // not ported: invalid UTF-8 ("a̖\xCC", "\x80à̖", ...)
        // ending in incomplete UTF-8 encoding
        // not ported: invalid UTF-8 ("", "\xCC", ...)
        // not ported: invalid UTF-8 ("a", "\xCC", ...)
        // not ported: invalid UTF-8 ("a", "b\xCC", ...)
        // not ported: invalid UTF-8 ("Ȧ", "\xCC", ...)
        // illegal runes
        // not ported: invalid UTF-8 ("", "\x80", ...)
        // not ported: invalid UTF-8 ("", "\x80\x80\x80", ...)
        // not ported: invalid UTF-8 ("", "\xCC\x80\x80\x80", ...)
        // not ported: invalid UTF-8 ("", "a\x80", ...)
        // not ported: invalid UTF-8 ("", "a\x80\x80\x80", ...)
        // not ported: invalid UTF-8 ("", "a\x80\x80\x80\x80\x80\x80", ...)
        // not ported: invalid UTF-8 ("a", "\x80\x80\x80", ...)
        // overflow
        // not ported: invalid UTF-8 ("", Repeat("\x80", 33), ...)
        // not ported: invalid UTF-8 (Repeat("\x80", 33), "", ...)
        // not ported: invalid UTF-8 (Repeat("\x80", 33), Repeat("\x80", 33), ...)
        // overflow of combining characters
        row("", &grave(34), &format!("{}{CGJ}{}", grave(30), grave(4))),
        row("", &grave(36), &format!("{}{CGJ}{}", grave(30), grave(6))),
        row(
            &grave(29),
            &grave(5),
            &format!("{}{CGJ}{}", grave(30), grave(4)),
        ),
        row(
            &grave(30),
            &grave(4),
            &format!("{}{CGJ}{}", grave(30), grave(4)),
        ),
        row(
            &grave(30),
            &grave(3),
            &format!("{}{CGJ}{}", grave(30), grave(3)),
        ),
        // not ported: invalid UTF-8 (grave(30) + "\xCC", "\x80", ...)
        row(
            "",
            &format!("\u{FDFA}{}", grave(14)),
            &format!(
                "\u{635}\u{644}\u{649} \u{627}\u{644}\u{644}\u{647} \u{639}\u{644}\u{64a}\u{647} \u{648}\u{633}\u{644}\u{645}{}",
                grave(14)
            ),
        ),
        row(
            "",
            &format!("\u{FDFA}{}\u{316}", grave(28)),
            &format!(
                "\u{635}\u{644}\u{649} \u{627}\u{644}\u{644}\u{647} \u{639}\u{644}\u{64a}\u{647} \u{648}\u{633}\u{644}\u{645}\u{316}{}",
                grave(28)
            ),
        ),
        // - First rune has a trailing non-starter.
        row(
            "\u{d5}",
            &grave(30),
            &format!("\u{d5}{}{CGJ}{}", grave(29), grave(1)),
        ),
        // - U+FF9E decomposes into a non-starter in compatibility mode. A CGJ must be
        //   inserted even when FF9E starts a new segment.
        row(
            "\u{ff9e}",
            &grave(30),
            &format!("\u{3099}{}{CGJ}{}", grave(29), grave(1)),
        ),
        row(
            &grave(30),
            "\u{ff9e}",
            &format!("{}{CGJ}\u{3099}", grave(30)),
        ),
        // - Many non-starter decompositions in a row causing overflow.
        row(
            "",
            &rep('\u{340}', 31),
            &format!("{}{CGJ}\u{300}", rep('\u{300}', 30)),
        ),
        row(
            "",
            &rep('\u{FF9E}', 31),
            &format!("{}{CGJ}\u{3099}", rep('\u{3099}', 30)),
        ),
        row(
            "",
            &format!("\u{644}\u{625}{}", rep('\u{300}', 31)),
            &format!("\u{644}\u{625}{}{CGJ}\u{300}\u{300}", rep('\u{300}', 29)),
        ),
        row(
            "",
            &format!("\u{fef9}{}", rep('\u{300}', 31)),
            &format!(
                "\u{644}\u{625}{}{CGJ}{}",
                rep('\u{300}', 29),
                rep('\u{300}', 2)
            ),
        ),
        row(
            "",
            &format!("\u{fef9}{}", rep('\u{300}', 31)),
            &format!(
                "\u{644}\u{625}{}{CGJ}{}",
                rep('\u{300}', 29),
                rep('\u{300}', 2)
            ),
        ),
        // U+0F81 TIBETAN VOWEL SIGN REVERSED II splits into two modifiers.
        row(
            "",
            &format!("\u{f7f}{}\u{f81}", rep('\u{f71}', 29)),
            &format!("\u{f7f}{}{CGJ}\u{f71}\u{f80}", rep('\u{f71}', 29)),
        ),
        row(
            "",
            &format!("\u{f7f}{}\u{f81}", rep('\u{f71}', 28)),
            &format!("\u{f7f}{}\u{f80}", rep('\u{f71}', 29)),
        ),
        row(
            "",
            &format!("\u{f7f}{}", rep('\u{f81}', 16)),
            &format!(
                "\u{f7f}{}{}{CGJ}\u{f71}\u{f80}",
                rep('\u{f71}', 15),
                rep('\u{f80}', 15)
            ),
        ),
        // weird UTF-8
        // not ported: invalid UTF-8 ("à\xE1", "\x86", ...)
        row("a\u{300}\u{11B7}", "\u{300}", "\u{E0}\u{11B7}\u{300}"),
        row(
            "a\u{300}\u{11B7}\u{300}",
            "\u{300}",
            "\u{E0}\u{11B7}\u{300}\u{300}",
        ),
        // not ported: invalid UTF-8 ("̀", "\xF8\x80\x80\x80\x80̀", ...)
        // not ported: invalid UTF-8 ("̀", "\xFC\x80\x80\x80\x80\x80̀", ...)
        // not ported: invalid UTF-8 ("\xF8\x80\x80\x80\x80̀", "̀", ...)
        // not ported: invalid UTF-8 ("\xFC\x80\x80\x80\x80\x80̀", "̀", ...)
        // not ported: invalid UTF-8 ("\xF8\x80\x80\x80", "\x80̀̀", ...)
        row("", &"a\u{316}\u{300}".repeat(6), &"\u{E0}\u{316}".repeat(6)),
        // large input.
        row(
            "",
            &"a\u{300}\u{316}".repeat(31),
            &"\u{E0}\u{316}".repeat(31),
        ),
        row(
            "",
            &"a\u{300}\u{316}".repeat(4000),
            &"\u{E0}\u{316}".repeat(4000),
        ),
        // not ported: invalid UTF-8 ("", Repeat("\x80\x80", 4000), ...)
        row("", "\u{41}\u{307}\u{304}", "\u{1E0}"),
    ]
}

fn append_tests_nfkd() -> Vec<Row> {
    vec![
        row(
            "",
            &format!("a{}", grave(64)),
            &format!("a{}{CGJ}{}{CGJ}{}", grave(30), grave(30), grave(4)),
        ),
        // segment overflow on unchanged character
        row(
            "",
            &format!("a{}\u{316}", grave(64)),
            &format!("a{}{CGJ}{}{CGJ}\u{316}{}", grave(30), grave(30), grave(4)),
        ),
        // segment overflow on unchanged character + start value
        row(
            "",
            &format!("a{}\u{316}", grave(98)),
            &format!(
                "a{}{CGJ}{}{CGJ}{}{CGJ}\u{316}{}",
                grave(30),
                grave(30),
                grave(30),
                grave(8)
            ),
        ),
        // segment overflow on decomposition. (U+0340 decomposes to U+0300.)
        row(
            "",
            &format!("a{}\u{340}", grave(59)),
            &format!("a{}{CGJ}{}", grave(30), grave(30)),
        ),
        // segment overflow on non-starter decomposition
        row(
            "",
            &format!("a{}\u{340}{}\u{320}", grave(33), grave(30)),
            &format!("a{}{CGJ}{}{CGJ}\u{320}{}", grave(30), grave(30), grave(4)),
        ),
        // start value after ASCII overflow
        row(
            "",
            &format!("{}{}\u{320}", rep('a', SEG_SIZE), grave(32)),
            &format!(
                "{}{}{CGJ}\u{320}{}",
                rep('a', SEG_SIZE),
                grave(30),
                grave(2)
            ),
        ),
        // Jamo overflow
        row(
            "",
            &format!("\u{1100}\u{1161}{}\u{320}{}", grave(30), grave(2)),
            &format!("\u{1100}\u{1161}{}{CGJ}\u{320}{}", grave(29), grave(3)),
        ),
        // Hangul
        row("", "\u{ac00}", "\u{1100}\u{1161}"),
        // Hangul overflow
        row(
            "",
            &format!("\u{ac00}{}\u{320}", grave(32)),
            &format!("\u{1100}\u{1161}{}{CGJ}\u{320}{}", grave(29), grave(3)),
        ),
        // Hangul overflow in Hangul mode.
        row(
            "",
            &format!("\u{ac00}\u{ac00}{}\u{320}", grave(32)),
            &format!(
                "\u{1100}\u{1161}\u{1100}\u{1161}{}{CGJ}\u{320}{}",
                grave(29),
                grave(3)
            ),
        ),
        // Hangul overflow in Hangul mode.
        row(
            "",
            &format!("{}{}\u{320}", "\u{ac00}".repeat(3), grave(32)),
            &format!(
                "{}{}{CGJ}\u{320}{}",
                "\u{1100}\u{1161}".repeat(3),
                grave(29),
                grave(3)
            ),
        ),
        // start value after cc=0
        row(
            "",
            &format!("您您{}\u{320}", grave(34)),
            &format!("您您{}{CGJ}\u{320}{}", grave(30), grave(4)),
        ),
        // start value after normalization
        row(
            "",
            &format!("\u{300}\u{320}a{}\u{320}", grave(34)),
            &format!("\u{320}\u{300}a{}{CGJ}\u{320}{}", grave(30), grave(4)),
        ),
        // U+0F81 TIBETAN VOWEL SIGN REVERSED II splits into two modifiers.
        row(
            "",
            &format!("a\u{f7f}{}\u{f81}", rep('\u{f71}', 29)),
            &format!("a\u{f7f}{}{CGJ}\u{f71}\u{f80}", rep('\u{f71}', 29)),
        ),
    ]
}

/// `runNormTests` with `fn` reduced to `f(left + right)` (see the module comment).
fn run_norm_tests(name: &str) {
    type Form = fn(&str) -> String;
    let forms: [(&str, Form, Form, Vec<Row>); 2] = [
        ("NFKC", nfkc, nfkd, append_tests_nfkc()),
        ("NFKD", nfkd, nfkc, append_tests_nfkd()),
    ];
    for (fname, f, g, tests) in forms {
        for (i, (left, right, out)) in tests.iter().enumerate() {
            let joined = format!("{left}{right}");
            assert_eq!(&f(&joined), out, "{name}/{fname}/{i}");
            // Bootstrap by normalizing input with the other form.
            let want = g(&joined);
            let have = g(&format!("{}{right}", g(left)));
            assert_eq!(have, want, "{name}/{fname}/{i}/bootstrap");
        }
    }
}

#[test]
fn test_append() {
    run_norm_tests("Append");
}

#[test]
fn test_append_string() {
    run_norm_tests("AppendString");
}

#[test]
fn test_bytes() {
    run_norm_tests("Bytes");
}

#[test]
fn test_string() {
    run_norm_tests("String");
}

// Not a Go test. Values from composition.go: maxNonStarters = 30, streamSafe.next counts
// the leading non-starters of each input rune and reports ssOverflow when the count
// exceeds 30, so the 31st U+0301 gets a CGJ before it. The count runs on the decomposed
// input (`a` has no trailing non-starters), so NFKC also splits after `a` + 30 U+0301,
// then composes `a` + U+0301 to U+00E1, leaving 29 U+0301 before the CGJ. To be
// confirmed by the oracle case xtext/cgj-31.
#[test]
fn stream_safe_cgj() {
    let s = "a".to_owned() + &"\u{301}".repeat(31);
    assert_eq!(
        nfkd(&s),
        "a".to_owned() + &"\u{301}".repeat(30) + "\u{34F}\u{301}"
    );
    assert_eq!(
        nfkc(&s),
        "\u{E1}".to_owned() + &"\u{301}".repeat(29) + "\u{34F}\u{301}"
    );
}
