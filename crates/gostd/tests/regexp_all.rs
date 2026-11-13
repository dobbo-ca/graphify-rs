//! Go 1.27.4 regexp/all_test.go
//!
//! TestGoodCompile: ported
//! TestBadCompile: ported (error text compared with `contains`, as Go does)
//! TestMatch: ported (MatchString for every row; the []byte `Match` path goes through `find_submatch`; 6 rows with invalid UTF-8 text run the byte path only)
//! TestMatchFunction: ported (the package-level `MatchString(pat, text)` is not in the Rust API, so it is a local helper over compile + match_string; 6 invalid-UTF-8 text rows left out, `&str` cannot hold them)
//! TestCopyMatch: not ported: Regexp.Copy is not reached
//! TestReplaceAll: ported (ReplaceAllString only; the []byte ReplaceAll path is not reached)
//! TestReplaceAllLiteral: not ported: ReplaceAllLiteral is not reached
//! TestReplaceAllFunc: ported (ReplaceAllStringFunc only; the []byte ReplaceAllFunc path is not reached)
//! TestQuoteMeta: ported (QuoteMeta part only; the literal and isLiteral columns belong to LiteralPrefix and are dropped)
//! TestLiteralPrefix: not ported: LiteralPrefix is not reached
//! TestSubexp: ported (NumSubexp and SubexpNames only; SubexpIndex is not in the API, so the indices column is dropped)
//! TestSplit: ported
//! TestParseAndCompile: ported
//! TestOnePassCutoff: not ported: compileOnePass, the onepass engine is deferred (graphify-rs-aa6.8)
//! TestSwitchBacktrack: not ported: maxBacktrackVector, the backtrack engine is deferred (graphify-rs-aa6.8)
//! TestDeepEqual: not ported: reflect.DeepEqual on Regexp internals has no Rust equivalent
//! TestMinInputLen: not ported: internal minInputLen is not reachable from the public API
//! TestUnmarshalText: not ported: MarshalText, UnmarshalText and AppendText are not reached
//!
//! Benchmarks are not ported. graphify_patterns_compile is not in Go: it compiles the 43
//! constant patterns of graphify-go v0.12.0. Skipped from the 47 call-site rows of the inventory:
//! internal/detect/gitignore.go:414 and :426 (patterns built at runtime from .gitignore content),
//! internal/query/query.go:148 (user pattern), and gitignore.go:507 (a QuoteMeta call, not a pattern).

use gostd::regexp::syntax::{self, Flags};
use gostd::regexp::{Regexp, quote_meta};

const GOOD_RE: &[&str] = &[
    r"",
    r".",
    r"^.$",
    r"a",
    r"a*",
    r"a+",
    r"a?",
    r"a|b",
    r"a*|b*",
    r"(a*|b)(c*|d)",
    r"[a-z]",
    r"[a-abc-c\-\]\[]",
    r"[a-z]+",
    r"[abc]",
    r"[^1234]",
    r"[^\n]",
    r"\!\\",
];

fn bad_re() -> Vec<(String, &'static str)> {
    vec![
        (r"*".into(), "missing argument to repetition operator: `*`"),
        (r"+".into(), "missing argument to repetition operator: `+`"),
        (r"?".into(), "missing argument to repetition operator: `?`"),
        (r"(abc".into(), "missing closing ): `(abc`"),
        (r"abc)".into(), "unexpected ): `abc)`"),
        (r"x[a-z".into(), "missing closing ]: `[a-z`"),
        (r"[z-a]".into(), "invalid character class range: `z-a`"),
        (r"abc\".into(), "trailing backslash at end of expression"),
        (r"a**".into(), "invalid nested repetition operator: `**`"),
        (r"a*+".into(), "invalid nested repetition operator: `*+`"),
        (r"\x".into(), "invalid escape sequence: `\\x`"),
        (r"\pL".repeat(27000), "expression too large"),
    ]
}

/// Go compileTest: an empty `error` means compiling must succeed.
fn compile_test(expr: &str, error: &str) -> Option<Regexp> {
    match Regexp::compile(expr) {
        Ok(re) => {
            assert!(error.is_empty(), "compiling `{expr}`; missing error");
            Some(re)
        }
        Err(err) => {
            assert!(
                !error.is_empty(),
                "compiling `{expr}`; unexpected error: {err}"
            );
            let got = err.to_string();
            assert!(
                got.contains(error),
                "compiling `{expr}`; wrong error: {got}; want {error}"
            );
            None
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_good_compile() {
    for expr in GOOD_RE {
        compile_test(expr, "");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_bad_compile() {
    for (expr, err) in bad_re() {
        compile_test(&expr, err);
    }
}

struct FindTest {
    pat: &'static str,
    text: &'static str,
    max: i32,
    matches: Vec<Vec<i32>>,
}

/// Go build: n matches with len(x)/n submatches each.
fn build(n: usize, x: &[i32]) -> Vec<Vec<i32>> {
    let run_length = x.len() / n;
    let mut j = 0;
    let mut ret = Vec::new();
    for _ in 0..n {
        ret.push(x[j..j + run_length].to_vec());
        j += run_length;
        assert!(j <= x.len(), "invalid build entry");
    }
    ret
}

fn ft(pat: &'static str, text: &'static str, max: i32, matches: Vec<Vec<i32>>) -> FindTest {
    FindTest {
        pat,
        text,
        max,
        matches,
    }
}

fn find_tests() -> Vec<FindTest> {
    vec![
        ft(r"", r"", -1, build(1, &[0, 0])),
        ft(r"^abcdefg", "abcdefg", -1, build(1, &[0, 7])),
        ft(r"a+", "baaab", -1, build(1, &[1, 4])),
        ft(r"a", "bababaab", -1, build(4, &[1, 2, 3, 4, 5, 6, 6, 7])),
        ft(r"a", "bababaab", 0, vec![]),
        ft(r"a", "bababaab", 1, build(1, &[1, 2])),
        ft(r"a", "bababaab", 2, build(2, &[1, 2, 3, 4])),
        ft(r"a", "bababaab", 3, build(3, &[1, 2, 3, 4, 5, 6])),
        ft(r"a", "bababaab", 4, build(4, &[1, 2, 3, 4, 5, 6, 6, 7])),
        ft(r"a", "bababaab", 5, build(4, &[1, 2, 3, 4, 5, 6, 6, 7])),
        ft("abcd..", "abcdef", -1, build(1, &[0, 6])),
        ft(r"a", "a", -1, build(1, &[0, 1])),
        ft(r"x", "y", -1, vec![]),
        ft(r"b", "abc", -1, build(1, &[1, 2])),
        ft(r".", "a", -1, build(1, &[0, 1])),
        ft(r".*", "abcdef", -1, build(1, &[0, 6])),
        ft(r"^", "abcde", -1, build(1, &[0, 0])),
        ft(r"$", "abcde", -1, build(1, &[5, 5])),
        ft(r"^abcd$", "abcd", -1, build(1, &[0, 4])),
        ft(r"^bcd'", "abcdef", -1, vec![]),
        ft(r"^abcd$", "abcde", -1, vec![]),
        ft(r"a+", "baaab", -1, build(1, &[1, 4])),
        ft(r"a*", "baaab", -1, build(3, &[0, 0, 1, 4, 5, 5])),
        ft(r"[a-z]+", "abcd", -1, build(1, &[0, 4])),
        ft(r"[^a-z]+", "ab1234cd", -1, build(1, &[2, 6])),
        ft(r"[a\-\]z]+", "az]-bcz", -1, build(2, &[0, 4, 6, 7])),
        ft(r"[^\n]+", "abcd\n", -1, build(1, &[0, 4])),
        ft(r"[日本語]+", "日本語日本語", -1, build(1, &[0, 18])),
        ft(r"日本語+", "日本語", -1, build(1, &[0, 9])),
        ft(r"日本語+", "日本語語語語", -1, build(1, &[0, 18])),
        ft(r"()", "", -1, build(1, &[0, 0, 0, 0])),
        ft(r"(a)", "a", -1, build(1, &[0, 1, 0, 1])),
        ft(r"(.)(.)", "日a", -1, build(1, &[0, 4, 0, 3, 3, 4])),
        ft(r"(.*)", "", -1, build(1, &[0, 0, 0, 0])),
        ft(r"(.*)", "abcd", -1, build(1, &[0, 4, 0, 4])),
        ft(r"(..)(..)", "abcd", -1, build(1, &[0, 4, 0, 2, 2, 4])),
        ft(
            r"(([^xyz]*)(d))",
            "abcd",
            -1,
            build(1, &[0, 4, 0, 4, 0, 3, 3, 4]),
        ),
        ft(
            r"((a|b|c)*(d))",
            "abcd",
            -1,
            build(1, &[0, 4, 0, 4, 2, 3, 3, 4]),
        ),
        ft(
            r"(((a|b|c)*)(d))",
            "abcd",
            -1,
            build(1, &[0, 4, 0, 4, 0, 3, 2, 3, 3, 4]),
        ),
        ft(r"\a\f\n\r\t\v", "\x07\x0c\n\r\t\x0b", -1, build(1, &[0, 6])),
        ft(
            r"[\a\f\n\r\t\v]+",
            "\x07\x0c\n\r\t\x0b",
            -1,
            build(1, &[0, 6]),
        ),
        ft(r"a*(|(b))c*", "aacc", -1, build(1, &[0, 4, 2, 2, -1, -1])),
        ft(r"(.*).*", "ab", -1, build(1, &[0, 2, 0, 2])),
        ft(r"[.]", ".", -1, build(1, &[0, 1])),
        ft(r"/$", "/abc/", -1, build(1, &[4, 5])),
        ft(r"/$", "/abc", -1, vec![]),
        // multiple matches
        ft(r".", "abc", -1, build(3, &[0, 1, 1, 2, 2, 3])),
        ft(
            r"(.)",
            "abc",
            -1,
            build(3, &[0, 1, 0, 1, 1, 2, 1, 2, 2, 3, 2, 3]),
        ),
        ft(r".(.)", "abcd", -1, build(2, &[0, 2, 1, 2, 2, 4, 3, 4])),
        ft(r"ab*", "abbaab", -1, build(3, &[0, 3, 3, 4, 4, 6])),
        ft(
            r"a(b*)",
            "abbaab",
            -1,
            build(3, &[0, 3, 1, 3, 3, 4, 4, 4, 4, 6, 5, 6]),
        ),
        // fixed bugs
        ft(r"ab$", "cab", -1, build(1, &[1, 3])),
        ft(r"axxb$", "axxcb", -1, vec![]),
        ft(r"data", "daXY data", -1, build(1, &[5, 9])),
        ft(r"da(.)a$", "daXY data", -1, build(1, &[5, 9, 7, 8])),
        ft(r"zx+", "zzx", -1, build(1, &[1, 3])),
        ft(r"ab$", "abcab", -1, build(1, &[3, 5])),
        ft(r"(aa)*$", "a", -1, build(1, &[1, 1, -1, -1])),
        ft(r"(?:.|(?:.a))", "", -1, vec![]),
        ft(r"(?:A(?:A|a))", "Aa", -1, build(1, &[0, 2])),
        ft(r"(?:A|(?:A|a))", "a", -1, build(1, &[0, 1])),
        ft(r"(a){0}", "", -1, build(1, &[0, 0, -1, -1])),
        ft(r"(?-s)(?:(?:^).)", "\n", -1, vec![]),
        ft(r"(?s)(?:(?:^).)", "\n", -1, build(1, &[0, 1])),
        ft(r"(?:(?:^).)", "\n", -1, vec![]),
        ft(r"\b", "x", -1, build(2, &[0, 0, 1, 1])),
        ft(r"\b", "xx", -1, build(2, &[0, 0, 2, 2])),
        ft(r"\b", "x y", -1, build(4, &[0, 0, 1, 1, 2, 2, 3, 3])),
        ft(r"\b", "xx yy", -1, build(4, &[0, 0, 2, 2, 3, 3, 5, 5])),
        ft(r"\B", "x", -1, vec![]),
        ft(r"\B", "xx", -1, build(1, &[1, 1])),
        ft(r"\B", "x y", -1, vec![]),
        ft(r"\B", "xx yy", -1, build(2, &[1, 1, 4, 4])),
        ft(
            r"(|a)*",
            "aa",
            -1,
            build(3, &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]),
        ),
        ft(r"0A|0[aA]", "0a", -1, build(1, &[0, 2])),
        ft(r"0[aA]|0A", "0a", -1, build(1, &[0, 2])),
        // RE2 tests
        ft(r"[^\S\s]", "abcd", -1, vec![]),
        ft(r"[^\S[:space:]]", "abcd", -1, vec![]),
        ft(r"[^\D\d]", "abcd", -1, vec![]),
        ft(r"[^\D[:digit:]]", "abcd", -1, vec![]),
        ft(r"(?i)\W", "x", -1, vec![]),
        ft(r"(?i)\W", "k", -1, vec![]),
        ft(r"(?i)\W", "s", -1, vec![]),
        // can backslash-escape any punctuation
        ft(
            r##"\!\"\#\$\%\&\'\(\)\*\+\,\-\.\/\:\;\<\=\>\?\@\[\\\]\^\_\{\|\}\~"##,
            r##"!"#$%&'()*+,-./:;<=>?@[\]^_{|}~"##,
            -1,
            build(1, &[0, 31]),
        ),
        ft(
            r##"[\!\"\#\$\%\&\'\(\)\*\+\,\-\.\/\:\;\<\=\>\?\@\[\\\]\^\_\{\|\}\~]+"##,
            r##"!"#$%&'()*+,-./:;<=>?@[\]^_{|}~"##,
            -1,
            build(1, &[0, 31]),
        ),
        ft(r"\`", "`", -1, build(1, &[0, 1])),
        ft(r"[\`]+", "`", -1, build(1, &[0, 1])),
        // long set of matches (longer than startSize)
        ft(
            ".",
            "qwertyuiopasdfghjklzxcvbnm1234567890",
            -1,
            build(
                36,
                &[
                    0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
                    12, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22,
                    22, 23, 23, 24, 24, 25, 25, 26, 26, 27, 27, 28, 28, 29, 29, 30, 30, 31, 31, 32,
                    32, 33, 33, 34, 34, 35, 35, 36,
                ],
            ),
        ),
    ]
}

/// The Go findTests rows whose text is not valid UTF-8; Rust can only reach them through the byte API.
/// Pattern, text bytes, max, expected match indices.
type ByteFindTest = (&'static str, &'static [u8], i32, Vec<Vec<i32>>);

fn find_tests_invalid_utf8() -> Vec<ByteFindTest> {
    vec![
        ("\u{fffd}", b"\xff", -1, build(1, &[0, 1])),
        ("\u{fffd}", b"hello\xffworld", -1, build(1, &[5, 6])),
        (r".*", b"hello\xffworld", -1, build(1, &[0, 11])),
        (r"\x{fffd}", b"\xc2\x00", -1, build(1, &[0, 1])),
        ("[\u{fffd}]", b"\xff", -1, build(1, &[0, 1])),
        (r"[\x{fffd}]", b"\xc2\x00", -1, build(1, &[0, 1])),
    ]
}

fn match_test(test: &FindTest) {
    if test.max == 0 {
        return;
    }
    let Some(re) = compile_test(test.pat, "") else {
        return;
    };
    let want = !test.matches.is_empty();
    let m = re.match_string(test.text);
    assert_eq!(
        m, want,
        "MatchString failure on pat: `{}` text: `{}`",
        test.pat, test.text
    );
    // now try bytes
    let m = re.find_submatch(test.text.as_bytes()).is_some();
    assert_eq!(
        m, want,
        "Match failure on pat: `{}` text: `{}`",
        test.pat, test.text
    );
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_match() {
    for test in find_tests() {
        match_test(&test);
    }
    for (pat, text, max, matches) in find_tests_invalid_utf8() {
        if max == 0 {
            continue;
        }
        let re = compile_test(pat, "").expect("compiles");
        let m = re.find_submatch(text).is_some();
        assert_eq!(
            m,
            !matches.is_empty(),
            "Match failure on pat: `{pat}` text: {text:?}"
        );
    }
}

/// Go package-level `MatchString(pattern, s)`.
fn match_string(pattern: &str, s: &str) -> Result<bool, syntax::Error> {
    Regexp::compile(pattern).map(|re| re.match_string(s))
}

fn match_function_test(test: &FindTest) {
    if match_string(test.pat, test.text).is_ok() {
        return;
    }
    // Go only checks when err != nil, and then m is false.
    assert!(
        test.matches.is_empty(),
        "Match failure on pat: `{}` text: `{}`",
        test.pat,
        test.text
    );
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_match_function() {
    for test in find_tests() {
        match_function_test(&test);
    }
}

const REPLACE_TESTS: &[(&str, &str, &str, &str)] = &[
    // Test empty input and/or replacement, with pattern that matches the empty string.
    ("", "", "", ""),
    ("", "x", "", "x"),
    ("", "", "abc", "abc"),
    ("", "x", "abc", "xaxbxcx"),
    // Test empty input and/or replacement, with pattern that does not match the empty string.
    ("b", "", "", ""),
    ("b", "x", "", ""),
    ("b", "", "abc", "ac"),
    ("b", "x", "abc", "axc"),
    ("y", "", "", ""),
    ("y", "x", "", ""),
    ("y", "", "abc", "abc"),
    ("y", "x", "abc", "abc"),
    // Multibyte characters -- verify that we don't try to match in the middle
    // of a character.
    ("[a-c]*", "x", "\u{65e5}", "x\u{65e5}x"),
    ("[^\u{65e5}]", "x", "abc\u{65e5}def", "xxx\u{65e5}xxx"),
    // Start and end of a string.
    ("^[a-c]*", "x", "abcdabc", "xdabc"),
    ("[a-c]*$", "x", "abcdabc", "abcdx"),
    ("^[a-c]*$", "x", "abcdabc", "abcdabc"),
    ("^[a-c]*", "x", "abc", "x"),
    ("[a-c]*$", "x", "abc", "x"),
    ("^[a-c]*$", "x", "abc", "x"),
    ("^[a-c]*", "x", "dabce", "xdabce"),
    ("[a-c]*$", "x", "dabce", "dabcex"),
    ("^[a-c]*$", "x", "dabce", "dabce"),
    ("^[a-c]*", "x", "", "x"),
    ("[a-c]*$", "x", "", "x"),
    ("^[a-c]*$", "x", "", "x"),
    ("^[a-c]+", "x", "abcdabc", "xdabc"),
    ("[a-c]+$", "x", "abcdabc", "abcdx"),
    ("^[a-c]+$", "x", "abcdabc", "abcdabc"),
    ("^[a-c]+", "x", "abc", "x"),
    ("[a-c]+$", "x", "abc", "x"),
    ("^[a-c]+$", "x", "abc", "x"),
    ("^[a-c]+", "x", "dabce", "dabce"),
    ("[a-c]+$", "x", "dabce", "dabce"),
    ("^[a-c]+$", "x", "dabce", "dabce"),
    ("^[a-c]+", "x", "", ""),
    ("[a-c]+$", "x", "", ""),
    ("^[a-c]+$", "x", "", ""),
    // Other cases.
    ("abc", "def", "abcdefg", "defdefg"),
    ("bc", "BC", "abcbcdcdedef", "aBCBCdcdedef"),
    ("abc", "", "abcdabc", "d"),
    ("x", "xXx", "xxxXxxx", "xXxxXxxXxXxXxxXxxXx"),
    ("abc", "d", "", ""),
    ("abc", "d", "abc", "d"),
    (".+", "x", "abc", "x"),
    ("[a-c]*", "x", "def", "xdxexfx"),
    ("[a-c]+", "x", "abcbcdcdedef", "xdxdedef"),
    ("[a-c]*", "x", "abcbcdcdedef", "xdxdxexdxexfx"),
    // Substitutions
    ("a+", "($0)", "banana", "b(a)n(a)n(a)"),
    ("a+", "(${0})", "banana", "b(a)n(a)n(a)"),
    ("a+", "(${0})$0", "banana", "b(a)an(a)an(a)a"),
    ("a+", "(${0})$0", "banana", "b(a)an(a)an(a)a"),
    (
        "hello, (.+)",
        "goodbye, ${1}",
        "hello, world",
        "goodbye, world",
    ),
    ("hello, (.+)", "goodbye, $1x", "hello, world", "goodbye, "),
    (
        "hello, (.+)",
        "goodbye, ${1}x",
        "hello, world",
        "goodbye, worldx",
    ),
    (
        "hello, (.+)",
        "<$0><$1><$2><$3>",
        "hello, world",
        "<hello, world><world><><>",
    ),
    (
        "hello, (?P<noun>.+)",
        "goodbye, $noun!",
        "hello, world",
        "goodbye, world!",
    ),
    (
        "hello, (?P<noun>.+)",
        "goodbye, ${noun}",
        "hello, world",
        "goodbye, world",
    ),
    ("(?P<x>hi)|(?P<x>bye)", "$x$x$x", "hi", "hihihi"),
    ("(?P<x>hi)|(?P<x>bye)", "$x$x$x", "bye", "byebyebye"),
    ("(?P<x>hi)|(?P<x>bye)", "$xyz", "hi", ""),
    ("(?P<x>hi)|(?P<x>bye)", "${x}yz", "hi", "hiyz"),
    ("(?P<x>hi)|(?P<x>bye)", "hello $$x", "hi", "hello $x"),
    ("a+", "${oops", "aaa", "${oops"),
    ("a+", "$$", "aaa", "$"),
    ("a+", "$", "aaa", "$"),
    // Substitution when subexpression isn't found
    ("(x)?", "$1", "123", "123"),
    ("abc", "$1", "123", "123"),
    // Substitutions involving a (x){0}
    ("(a)(b){0}(c)", ".$1|$3.", "xacxacx", "x.a|c.x.a|c.x"),
    ("(a)(((b))){0}c", ".$1.", "xacxacx", "x.a.x.a.x"),
    (
        "((a(b){0}){3}){5}(h)",
        "y caramb$2",
        "say aaaaaaaaaaaaaaaah",
        "say ay caramba",
    ),
    (
        "((a(b){0}){3}){5}h",
        "y caramb$2",
        "say aaaaaaaaaaaaaaaah",
        "say ay caramba",
    ),
];

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_replace_all() {
    for &(pattern, replacement, input, output) in REPLACE_TESTS {
        let re = Regexp::compile(pattern)
            .unwrap_or_else(|err| panic!("Unexpected error compiling {pattern:?}: {err}"));
        let actual = re.replace_all_string(input, replacement);
        assert_eq!(
            actual, output,
            "{pattern:?}.ReplaceAllString({input:?},{replacement:?})"
        );
    }
}

/// Go replaceFuncTests; the replacement function is the same in all three rows: "x" + s + "y".
const REPLACE_FUNC_TESTS: &[(&str, &str, &str)] = &[
    ("[a-c]", "defabcdef", "defxayxbyxcydef"),
    ("[a-c]+", "defabcdef", "defxabcydef"),
    ("[a-c]*", "defabcdef", "xydxyexyfxabcydxyexyfxy"),
];

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_replace_all_func() {
    for &(pattern, input, output) in REPLACE_FUNC_TESTS {
        let re = Regexp::compile(pattern)
            .unwrap_or_else(|err| panic!("Unexpected error compiling {pattern:?}: {err}"));
        let actual = re.replace_all_string_func(input, |s| format!("x{s}y"));
        assert_eq!(actual, output, "{pattern:?}.ReplaceFunc({input:?},fn)");
    }
}

/// Go metaTests, columns pattern and output (literal and isLiteral are for LiteralPrefix).
const META_TESTS: &[(&str, &str)] = &[
    (r"", r""),
    (r"foo", r"foo"),
    (r"日本語+", r"日本語\+"),
    (r"foo\.\$", r"foo\\\.\\\$"), // has meta but no operator
    (r"foo.\$", r"foo\.\\\$"),    // has escaped operators and real operators
    (
        r"!@#$%^&*()_+-=[{]}\|,<.>/?~",
        r"!@#\$%\^&\*\(\)_\+-=\[\{\]\}\\\|,<\.>/\?~",
    ),
];

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_quote_meta() {
    for &(pattern, output) in META_TESTS {
        // Verify that QuoteMeta returns the expected string.
        let quoted = quote_meta(pattern);
        assert_eq!(quoted, output, "QuoteMeta(`{pattern}`)");

        // Verify that the quoted string is in fact treated as expected
        // by Compile -- i.e. that it matches the original, unquoted string.
        if !pattern.is_empty() {
            let re = Regexp::compile(&quoted).unwrap_or_else(|err| {
                panic!("Unexpected error compiling QuoteMeta(`{pattern}`): {err}")
            });
            let src = format!("abc{pattern}def");
            let replaced = re.replace_all_string(&src, "xyz");
            assert_eq!(
                replaced, "abcxyzdef",
                "QuoteMeta(`{pattern}`).Replace(`{src}`,`xyz`)"
            );
        }
    }
}

/// Go subexpCase, columns input, num, names (indices needs SubexpIndex, which is not in the API).
const SUBEXP_CASES: &[(&str, usize, Option<&[&str]>)] = &[
    (r"", 0, None),
    (r".*", 0, None),
    (r"abba", 0, None),
    (r"ab(b)a", 1, Some(&["", ""])),
    (r"ab(.*)a", 1, Some(&["", ""])),
    (r"(.*)ab(.*)a", 2, Some(&["", "", ""])),
    (r"(.*)(ab)(.*)a", 3, Some(&["", "", "", ""])),
    (r"(.*)((a)b)(.*)a", 4, Some(&["", "", "", "", ""])),
    (r"(.*)(\(ab)(.*)a", 3, Some(&["", "", "", ""])),
    (r"(.*)(\(a\)b)(.*)a", 3, Some(&["", "", "", ""])),
    (
        r"(?P<foo>.*)(?P<bar>(a)b)(?P<foo>.*)a",
        4,
        Some(&["", "foo", "bar", "", "foo"]),
    ),
];

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_subexp() {
    for &(input, num, want_names) in SUBEXP_CASES {
        let re = Regexp::must_compile(input);
        let n = re.num_subexp();
        assert_eq!(n, num, "{input:?}: NumSubexp");
        let names = re.subexp_names();
        assert_eq!(names.len(), 1 + n, "{input:?}: len(SubexpNames)");
        if let Some(want) = want_names {
            assert_eq!(names, want, "{input:?}: SubexpNames");
        }
    }
}

type SplitTest = (&'static str, &'static str, isize, &'static [&'static str]);

const SPLIT_TESTS: &[SplitTest] = &[
    ("foo:and:bar", ":", -1, &["foo", "and", "bar"]),
    ("foo:and:bar", ":", 1, &["foo:and:bar"]),
    ("foo:and:bar", ":", 2, &["foo", "and:bar"]),
    ("foo:and:bar", "foo", -1, &["", ":and:bar"]),
    ("foo:and:bar", "bar", -1, &["foo:and:", ""]),
    ("foo:and:bar", "baz", -1, &["foo:and:bar"]),
    ("baabaab", "a", -1, &["b", "", "b", "", "b"]),
    ("baabaab", "a*", -1, &["b", "b", "b"]),
    ("baabaab", "ba*", -1, &["", "", "", ""]),
    ("foobar", "f*b*", -1, &["", "o", "o", "a", "r"]),
    ("foobar", "f+.*b+", -1, &["", "ar"]),
    ("foobooboar", "o{2}", -1, &["f", "b", "boar"]),
    ("a,b,c,d,e,f", ",", 3, &["a", "b", "c,d,e,f"]),
    ("a,b,c,d,e,f", ",", 0, &[]),
    (",", ",", -1, &["", ""]),
    (",,,", ",", -1, &["", "", "", ""]),
    ("", ",", -1, &[""]),
    ("", ".*", -1, &[""]),
    ("", ".+", -1, &[""]),
    ("", "", -1, &[]),
    ("foobar", "", -1, &["f", "o", "o", "b", "a", "r"]),
    ("abaabaccadaaae", "a*", 5, &["", "b", "b", "c", "cadaaae"]),
    (":x:y:z:", ":", -1, &["", "x", "y", "z", ""]),
];

/// Go strings.SplitN. An empty sep explodes into UTF-8 sequences; the table only uses it with n < 0.
fn strings_split_n<'s>(s: &'s str, sep: &str, n: isize) -> Vec<&'s str> {
    if n == 0 {
        return Vec::new();
    }
    if sep.is_empty() {
        assert!(n < 0, "SplitN with empty sep is only needed for n < 0");
        return s
            .char_indices()
            .map(|(i, c)| &s[i..i + c.len_utf8()])
            .collect();
    }
    if n < 0 {
        s.split(sep).collect()
    } else {
        s.splitn(n as usize, sep).collect()
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_split() {
    for (i, &(s, r, n, out)) in SPLIT_TESTS.iter().enumerate() {
        let re =
            Regexp::compile(r).unwrap_or_else(|err| panic!("#{i}: {r:?}: compile error: {err}"));

        let split = re.split(s, n);
        assert_eq!(split, out, "#{i}: {r:?}: got {split:?}; want {out:?}");

        if quote_meta(r) == r {
            let strsplit = strings_split_n(s, r, n);
            assert_eq!(
                split, strsplit,
                "#{i}: Split({s:?}, {r:?}, {n}): regexp vs strings mismatch"
            );
        }
    }
}

// The following sequence of Match calls used to panic. See issue #12980.
#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_parse_and_compile() {
    let expr = "a$";
    let s = "a\nb";

    for (i, (re_flags, exp_match)) in [
        (Flags::PERL | Flags::ONE_LINE, false),
        (Flags::PERL & !Flags::ONE_LINE, true),
    ]
    .into_iter()
    .enumerate()
    {
        let parsed =
            syntax::parse(expr, re_flags).unwrap_or_else(|err| panic!("{i}: parse: {err}"));
        let re = Regexp::compile(&parsed.to_string())
            .unwrap_or_else(|err| panic!("{i}: compile: {err}"));
        let m = re.match_string(s);
        assert_eq!(
            m,
            exp_match,
            "{i}: {:?}.MatchString({s:?})={m}; expected={exp_match}",
            re.as_str()
        );
    }
}

/// The 43 constant patterns of graphify-go v0.12.0 (file:line, pattern).
const GRAPHIFY_PATTERNS: &[(&str, &str)] = &[
    ("cmd/graphify/save_result.go:26", r"[^0-9a-zA-Z_]+"),
    ("cmd/graphify/serve.go:453", r"^[0-9a-f]{7,40}$"),
    ("internal/security/security.go:147", r"[\x00-\x1f\x7f]"),
    (
        "internal/detect/detect.go:449",
        r"(?i)(^|[\\/])\.(env|envrc)(\.|$)",
    ),
    (
        "internal/detect/detect.go:450",
        r"(?i)\.(pem|key|p12|pfx|cert|crt|der|p8)$",
    ),
    (
        "internal/detect/detect.go:451",
        r"(id_rsa|id_dsa|id_ecdsa|id_ed25519)(\.pub)?$",
    ),
    (
        "internal/detect/detect.go:452",
        r"(?i)(\.netrc|\.pgpass|\.htpasswd)$",
    ),
    (
        "internal/detect/detect.go:488",
        r"(?i)(?:credential|secret|passwd|password|private_key)s?",
    ),
    ("internal/detect/detect.go:492", r"[-_\s]+"),
    ("internal/extract/manifest.go:158", r"^module\s+(\S+)"),
    ("internal/extract/manifest.go:159", r"^require\s*\("),
    ("internal/extract/manifest.go:160", r"^(\S+)\s+v\S+"),
    (
        "internal/extract/manifest.go:161",
        r"^require\s+(\S+)\s+v\S+",
    ),
    ("internal/extract/manifest.go:240", r"[\s<>=!~;\[\(]"),
    ("internal/extract/manifest.go:306", r"\$\{([^}]+)\}"),
    (
        "internal/extract/cpp.go:30",
        r"^(?:class|struct)\s+((?:[A-Z][A-Z0-9_]*\s+)+)([A-Za-z_]\w*(?:\s+final)?\s*)([:{])",
    ),
    ("internal/extract/python.go:252", r"^[A-Z]\w*$"),
    ("internal/extract/python.go:715", r"(?m)^revision\s*[:=]"),
    (
        "internal/extract/mcpconfig.go:170",
        r"^@[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*(?:@[\w.\-+]+)?$",
    ),
    (
        "internal/extract/mcpconfig.go:171",
        r"^[a-z0-9][a-z0-9._-]*-mcp(?:-[a-z0-9._-]+)?$|^mcp-[a-z0-9][a-z0-9._-]*$",
    ),
    ("internal/extract/mcpconfig.go:172", r"^-{1,2}\w"),
    (
        "internal/extract/component.go:29",
        r#"(?i)(<script\b(?:"[^"]*"|'[^']*'|[^>"'])*>)([\s\S]*?)(</script\s*>)"#,
    ),
    (
        "internal/extract/component.go:32",
        r#"(?i)\blang\s*=\s*['"]?([A-Za-z]+)['"]?"#,
    ),
    (
        "internal/extract/component.go:36",
        r"\A\s*---\s*\r?\n([\s\S]*?)\r?\n---\s*(?:\r?\n|\z)",
    ),
    (
        "internal/extract/component.go:39",
        r#"import\(\s*['"]([^'"]+)['"]\s*\)"#,
    ),
    (
        "internal/extract/resolve.go:1057",
        r"^[A-Za-z_][A-Za-z0-9_]*(?:(?:::|\.)[A-Za-z_][A-Za-z0-9_]*)*$",
    ),
    ("internal/extract/resolve.go:1060", r"::|\."),
    (
        "internal/extract/terraform.go:197",
        r"(?i)(password|passwd|secret|token|api[-_]?key|access[-_]?key|private[-_]?key|credential|client[-_]?secret|connection[-_]?string|sas[-_]?token|auth|bearer|passphrase)",
    ),
    ("internal/extract/terraform.go:202", r"://[^/@\s]*:[^@\s]+@"),
    (
        "internal/extract/terraform.go:205",
        r"^[A-Za-z_][\w.\[\]*-]*$",
    ),
    ("internal/query/ask.go:61", r"[\p{L}\p{N}_]+"),
    (
        "internal/extract/markdown.go:15",
        r"\[[^\]]*\]\((?:<([^>]+)>|([^)\s]+))",
    ),
    (
        "internal/extract/markdown.go:19",
        r"^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?",
    ),
    (
        "internal/extract/markdown.go:23",
        r"\[\[([^\]|#\\]+)\\?(?:[#|][^\]]*)?\]\]",
    ),
    ("internal/extract/markdown.go:27", r"^(#{1,6})\s+(.+)"),
    // An interpreted Go string "`([^`\n]+)`": the \n is a real newline.
    ("internal/extract/markdown.go:31", "`([^`\n]+)`"),
    ("internal/extract/javascript.go:1428", r"^\s*(//|/\*|\*)"),
    (
        "internal/extract/javascript.go:1433",
        r"(?i)\b(ADR[- ]?\d{1,5}|RFC[- ]?\d{1,5})\b",
    ),
    (
        "internal/extract/javascript.go:1434",
        r"([A-Za-z]+)[- ]?(\d+)",
    ),
    (
        "internal/extract/csharp_resolve.go:529",
        r"(?i)^test_|_test\..|\.test\..|\.spec\..|_spec\..|\.tests\.ps1$",
    ),
    (
        "internal/extract/csharp_resolve.go:530",
        r"Tests?\.java$|Tests\.cs$",
    ),
    ("internal/idutil/idutil.go:17", r"[^\p{L}\p{N}_]+"),
    ("internal/idutil/idutil.go:21", r"_+"),
];

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn graphify_patterns_compile() {
    assert_eq!(GRAPHIFY_PATTERNS.len(), 43);
    for (site, pattern) in GRAPHIFY_PATTERNS {
        if let Err(err) = Regexp::compile(pattern) {
            panic!("{site}: {pattern:?} did not compile: {err}");
        }
    }
}
