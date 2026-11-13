//! Go 1.27.4 regexp/find_test.go
//!
//! TestFind: not ported: Find is not reached by graphify-go
//! TestFindString: ported (FindString is not in the Rust API; the result is derived from
//!   find_string_index, with "" for no match, exactly as Go's FindString returns)
//! TestFindIndex: not ported: FindIndex is not reached by graphify-go
//! TestFindStringIndex: ported
//! TestFindReaderIndex: not ported: the Reader variants are not reached
//! TestFindAll: not ported: FindAll is not reached by graphify-go
//! TestFindAllString: ported
//! TestFindAllIndex: not ported: FindAllIndex is not reached by graphify-go
//! TestFindAllStringIndex: ported
//! TestFindSubmatch: ported (byte API)
//! TestFindStringSubmatch: ported
//! TestFindSubmatchIndex: ported (byte API)
//! TestFindStringSubmatchIndex: not ported: FindStringSubmatchIndex is not reached
//! TestFindReaderSubmatchIndex: not ported: the Reader variants are not reached
//! TestFindAllSubmatch: not ported: FindAllSubmatch is not reached by graphify-go
//! TestFindAllStringSubmatch: ported
//! TestFindAllSubmatchIndex: ported (byte API)
//! TestFindAllStringSubmatchIndex: not ported: FindAllStringSubmatchIndex is not reached
//!
//! Mapping from Go to the Rust API:
//! - Go nil vs empty slice: a nil `[]T` result is an empty `Vec`; no match is `None` for the
//!   single-match methods and an empty `Vec` for the `all` methods. Go's `matches == nil` row
//!   (no match expected) is `None` in the table.
//! - Go index slices ([]int with -1 for an unmatched group) are `Option<(usize, usize)>`,
//!   `None` being the unmatched group. Go's string submatch result "" for an unmatched group
//!   is `None` here (stricter than Go, which cannot tell it from an empty match); `[]byte` nil
//!   is `None`.
//! - Go's `len(got) != cap(got)` capacity checks have no Rust counterpart and are dropped.
//! - findTests rows whose text is not valid UTF-8 can only go to the byte API
//!   (find_submatch, find_submatch_index, find_all_submatch_index); the str API tests skip them
//!   (6 of 94 rows: the `U+FFFD` and `\x{fffd}` rows with `\xff` / `\xc2\x00` text).
//!
//! findTests: 94 rows kept, 0 left out of the table; 88 are valid UTF-8 and run in the str
//! tests, all 94 run in the byte tests.

use gostd::regexp::Regexp;

struct FindTest {
    pat: &'static str,
    text: &'static [u8],
    max: isize,
    matches: Option<Vec<Vec<isize>>>,
}

impl FindTest {
    // Go FindTest.String.
    fn desc(&self) -> String {
        format!(
            "pat: {:?} text: {:?}",
            self.pat,
            String::from_utf8_lossy(self.text)
        )
    }

    // The text as a &str, or None when it is not valid UTF-8.
    fn text_str(&self) -> Option<&'static str> {
        std::str::from_utf8(self.text).ok()
    }
}

// build is a helper to construct a Vec<Vec<isize>> by extracting n sequences from x.
// This represents n matches with len(x)/n submatches each.
fn build(n: usize, x: &[isize]) -> Option<Vec<Vec<isize>>> {
    let run_length = x.len() / n;
    let mut ret = Vec::with_capacity(n);
    let mut j = 0;
    for _ in 0..n {
        let mut v = vec![0; run_length];
        let end = (j + run_length).min(x.len());
        v[..end - j].copy_from_slice(&x[j..end]);
        ret.push(v);
        j += run_length;
        assert!(j <= x.len(), "invalid build entry");
    }
    Some(ret)
}

fn row(
    pat: &'static str,
    text: &'static str,
    max: isize,
    matches: Option<Vec<Vec<isize>>>,
) -> FindTest {
    FindTest {
        pat,
        text: text.as_bytes(),
        max,
        matches,
    }
}

// Same as row, for text that is not valid UTF-8.
fn rowb(
    pat: &'static str,
    text: &'static [u8],
    max: isize,
    matches: Option<Vec<Vec<isize>>>,
) -> FindTest {
    FindTest {
        pat,
        text,
        max,
        matches,
    }
}

fn find_tests() -> Vec<FindTest> {
    vec![
        row("", "", -1, build(1, &[0, 0])),
        row(r"^abcdefg", "abcdefg", -1, build(1, &[0, 7])),
        row(r"a+", "baaab", -1, build(1, &[1, 4])),
        row(r"a", "bababaab", -1, build(4, &[1, 2, 3, 4, 5, 6, 6, 7])),
        row(r"a", "bababaab", 0, None),
        row(r"a", "bababaab", 1, build(1, &[1, 2])),
        row(r"a", "bababaab", 2, build(2, &[1, 2, 3, 4])),
        row(r"a", "bababaab", 3, build(3, &[1, 2, 3, 4, 5, 6])),
        row(r"a", "bababaab", 4, build(4, &[1, 2, 3, 4, 5, 6, 6, 7])),
        row(r"a", "bababaab", 5, build(4, &[1, 2, 3, 4, 5, 6, 6, 7])),
        row("abcd..", "abcdef", -1, build(1, &[0, 6])),
        row(r"a", "a", -1, build(1, &[0, 1])),
        row(r"x", "y", -1, None),
        row(r"b", "abc", -1, build(1, &[1, 2])),
        row(r".", "a", -1, build(1, &[0, 1])),
        row(r".*", "abcdef", -1, build(1, &[0, 6])),
        row(r"^", "abcde", -1, build(1, &[0, 0])),
        row(r"$", "abcde", -1, build(1, &[5, 5])),
        row(r"^abcd$", "abcd", -1, build(1, &[0, 4])),
        row(r"^bcd'", "abcdef", -1, None),
        row(r"^abcd$", "abcde", -1, None),
        row(r"a+", "baaab", -1, build(1, &[1, 4])),
        row(r"a*", "baaab", -1, build(3, &[0, 0, 1, 4, 5, 5])),
        row(r"[a-z]+", "abcd", -1, build(1, &[0, 4])),
        row(r"[^a-z]+", "ab1234cd", -1, build(1, &[2, 6])),
        row(r"[a\-\]z]+", "az]-bcz", -1, build(2, &[0, 4, 6, 7])),
        row(r"[^\n]+", "abcd\n", -1, build(1, &[0, 4])),
        row(r"[日本語]+", "日本語日本語", -1, build(1, &[0, 18])),
        row(r"日本語+", "日本語", -1, build(1, &[0, 9])),
        row(r"日本語+", "日本語語語語", -1, build(1, &[0, 18])),
        row(r"()", "", -1, build(1, &[0, 0, 0, 0])),
        row(r"(a)", "a", -1, build(1, &[0, 1, 0, 1])),
        row(r"(.)(.)", "日a", -1, build(1, &[0, 4, 0, 3, 3, 4])),
        row(r"(.*)", "", -1, build(1, &[0, 0, 0, 0])),
        row(r"(.*)", "abcd", -1, build(1, &[0, 4, 0, 4])),
        row(r"(..)(..)", "abcd", -1, build(1, &[0, 4, 0, 2, 2, 4])),
        row(
            r"(([^xyz]*)(d))",
            "abcd",
            -1,
            build(1, &[0, 4, 0, 4, 0, 3, 3, 4]),
        ),
        row(
            r"((a|b|c)*(d))",
            "abcd",
            -1,
            build(1, &[0, 4, 0, 4, 2, 3, 3, 4]),
        ),
        row(
            r"(((a|b|c)*)(d))",
            "abcd",
            -1,
            build(1, &[0, 4, 0, 4, 0, 3, 2, 3, 3, 4]),
        ),
        row(r"\a\f\n\r\t\v", "\x07\x0c\n\r\t\x0b", -1, build(1, &[0, 6])),
        row(
            r"[\a\f\n\r\t\v]+",
            "\x07\x0c\n\r\t\x0b",
            -1,
            build(1, &[0, 6]),
        ),
        row(r"a*(|(b))c*", "aacc", -1, build(1, &[0, 4, 2, 2, -1, -1])),
        row(r"(.*).*", "ab", -1, build(1, &[0, 2, 0, 2])),
        row(r"[.]", ".", -1, build(1, &[0, 1])),
        row(r"/$", "/abc/", -1, build(1, &[4, 5])),
        row(r"/$", "/abc", -1, None),
        // multiple matches
        row(r".", "abc", -1, build(3, &[0, 1, 1, 2, 2, 3])),
        row(
            r"(.)",
            "abc",
            -1,
            build(3, &[0, 1, 0, 1, 1, 2, 1, 2, 2, 3, 2, 3]),
        ),
        row(r".(.)", "abcd", -1, build(2, &[0, 2, 1, 2, 2, 4, 3, 4])),
        row(r"ab*", "abbaab", -1, build(3, &[0, 3, 3, 4, 4, 6])),
        row(
            r"a(b*)",
            "abbaab",
            -1,
            build(3, &[0, 3, 1, 3, 3, 4, 4, 4, 4, 6, 5, 6]),
        ),
        // fixed bugs
        row(r"ab$", "cab", -1, build(1, &[1, 3])),
        row(r"axxb$", "axxcb", -1, None),
        row(r"data", "daXY data", -1, build(1, &[5, 9])),
        row(r"da(.)a$", "daXY data", -1, build(1, &[5, 9, 7, 8])),
        row(r"zx+", "zzx", -1, build(1, &[1, 3])),
        row(r"ab$", "abcab", -1, build(1, &[3, 5])),
        row(r"(aa)*$", "a", -1, build(1, &[1, 1, -1, -1])),
        row(r"(?:.|(?:.a))", "", -1, None),
        row(r"(?:A(?:A|a))", "Aa", -1, build(1, &[0, 2])),
        row(r"(?:A|(?:A|a))", "a", -1, build(1, &[0, 1])),
        row(r"(a){0}", "", -1, build(1, &[0, 0, -1, -1])),
        row(r"(?-s)(?:(?:^).)", "\n", -1, None),
        row(r"(?s)(?:(?:^).)", "\n", -1, build(1, &[0, 1])),
        row(r"(?:(?:^).)", "\n", -1, None),
        row(r"\b", "x", -1, build(2, &[0, 0, 1, 1])),
        row(r"\b", "xx", -1, build(2, &[0, 0, 2, 2])),
        row(r"\b", "x y", -1, build(4, &[0, 0, 1, 1, 2, 2, 3, 3])),
        row(r"\b", "xx yy", -1, build(4, &[0, 0, 2, 2, 3, 3, 5, 5])),
        row(r"\B", "x", -1, None),
        row(r"\B", "xx", -1, build(1, &[1, 1])),
        row(r"\B", "x y", -1, None),
        row(r"\B", "xx yy", -1, build(2, &[1, 1, 4, 4])),
        row(
            r"(|a)*",
            "aa",
            -1,
            build(3, &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]),
        ),
        row(r"0A|0[aA]", "0a", -1, build(1, &[0, 2])),
        row(r"0[aA]|0A", "0a", -1, build(1, &[0, 2])),
        // RE2 tests
        row(r"[^\S\s]", "abcd", -1, None),
        row(r"[^\S[:space:]]", "abcd", -1, None),
        row(r"[^\D\d]", "abcd", -1, None),
        row(r"[^\D[:digit:]]", "abcd", -1, None),
        row(r"(?i)\W", "x", -1, None),
        row(r"(?i)\W", "k", -1, None),
        row(r"(?i)\W", "s", -1, None),
        // can backslash-escape any punctuation
        row(
            r##"\!\"\#\$\%\&\'\(\)\*\+\,\-\.\/\:\;\<\=\>\?\@\[\\\]\^\_\{\|\}\~"##,
            r##"!"#$%&'()*+,-./:;<=>?@[\]^_{|}~"##,
            -1,
            build(1, &[0, 31]),
        ),
        row(
            r##"[\!\"\#\$\%\&\'\(\)\*\+\,\-\.\/\:\;\<\=\>\?\@\[\\\]\^\_\{\|\}\~]+"##,
            r##"!"#$%&'()*+,-./:;<=>?@[\]^_{|}~"##,
            -1,
            build(1, &[0, 31]),
        ),
        row(r"\`", "`", -1, build(1, &[0, 1])),
        row(r"[\`]+", "`", -1, build(1, &[0, 1])),
        rowb("\u{fffd}", b"\xff", -1, build(1, &[0, 1])),
        rowb("\u{fffd}", b"hello\xffworld", -1, build(1, &[5, 6])),
        rowb(r".*", b"hello\xffworld", -1, build(1, &[0, 11])),
        rowb(r"\x{fffd}", b"\xc2\x00", -1, build(1, &[0, 1])),
        rowb("[\u{fffd}]", b"\xff", -1, build(1, &[0, 1])),
        rowb(r"[\x{fffd}]", b"\xc2\x00", -1, build(1, &[0, 1])),
        // long set of matches (longer than startSize)
        row(
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

fn idx(want: &[isize], k: usize) -> Option<(usize, usize)> {
    if want[2 * k] == -1 {
        None
    } else {
        Some((want[2 * k] as usize, want[2 * k + 1] as usize))
    }
}

// Go testFindIndex.
fn test_find_index(test: &FindTest, result: Option<(usize, usize)>) {
    match (&test.matches, result) {
        _ if test.max == 0 => {} // do not know whether to match or not; skip
        (None, None) => {}
        (None, Some(r)) => panic!("got match {r:?}, want none: {}", test.desc()),
        (Some(_), None) => panic!("got no match, want one: {}", test.desc()),
        (Some(m), Some(r)) => {
            let want = (m[0][0] as usize, m[0][1] as usize);
            assert_eq!(r, want, "{}", test.desc());
        }
    }
}

// Go testFindAllIndex.
fn test_find_all_index(test: &FindTest, result: Vec<(usize, usize)>) {
    match &test.matches {
        None => assert!(
            result.is_empty(),
            "got match {result:?}, want none: {}",
            test.desc()
        ),
        Some(m) => {
            assert!(
                !result.is_empty(),
                "got no match, want one: {}",
                test.desc()
            );
            assert_eq!(
                result.len(),
                m.len(),
                "got {} matches, want {}: {}",
                result.len(),
                m.len(),
                test.desc()
            );
            for (k, e) in m.iter().enumerate() {
                let want = (e[0] as usize, e[1] as usize);
                assert_eq!(result[k], want, "match {k}: {}", test.desc());
            }
        }
    }
}

// Go testSubmatchBytes.
fn test_submatch_bytes(test: &FindTest, n: usize, submatches: &[isize], result: &[Option<&[u8]>]) {
    assert_eq!(
        submatches.len(),
        result.len() * 2,
        "match {n}: got {} submatches, want {}: {}",
        result.len(),
        submatches.len() / 2,
        test.desc()
    );
    for (k, got) in result.iter().enumerate() {
        let want = idx(submatches, k).map(|(a, b)| &test.text[a..b]);
        assert_eq!(*got, want, "match {n}: {}", test.desc());
    }
}

// Go testSubmatchString.
fn test_submatch_string(test: &FindTest, n: usize, submatches: &[isize], result: &[Option<&str>]) {
    assert_eq!(
        submatches.len(),
        result.len() * 2,
        "match {n}: got {} submatches, want {}: {}",
        result.len(),
        submatches.len() / 2,
        test.desc()
    );
    let text = test.text_str().unwrap();
    for (k, got) in result.iter().enumerate() {
        let want = idx(submatches, k).map(|(a, b)| &text[a..b]);
        assert_eq!(*got, want, "match {n}: {}", test.desc());
    }
}

// Go testSubmatchIndices.
fn test_submatch_indices(
    test: &FindTest,
    n: usize,
    want: &[isize],
    result: &[Option<(usize, usize)>],
) {
    assert_eq!(
        want.len(),
        result.len() * 2,
        "match {n}: got {} matches, want {}: {}",
        result.len(),
        want.len() / 2,
        test.desc()
    );
    for (k, got) in result.iter().enumerate() {
        assert_eq!(*got, idx(want, k), "match {n}: {}", test.desc());
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_string() {
    for test in &find_tests() {
        let Some(text) = test.text_str() else {
            continue;
        };
        let result = Regexp::must_compile(test.pat)
            .find_string_index(text)
            .map_or("", |(a, b)| &text[a..b]);
        match &test.matches {
            _ if test.max == 0 => {} // do not know whether to match or not; skip
            None => assert!(
                result.is_empty(),
                "got match {result:?}, want none: {}",
                test.desc()
            ),
            Some(m) if result.is_empty() => {
                // Tricky because an empty result has two meanings: no match or empty match.
                assert_eq!(m[0][0], m[0][1], "got no match, want one: {}", test.desc());
            }
            Some(m) => {
                let want = &text[m[0][0] as usize..m[0][1] as usize];
                assert_eq!(result, want, "{}", test.desc());
            }
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_string_index() {
    for test in &find_tests() {
        let Some(text) = test.text_str() else {
            continue;
        };
        test_find_index(test, Regexp::must_compile(test.pat).find_string_index(text));
    }
}

// Now come the simple All cases.

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_all_string() {
    for test in &find_tests() {
        let Some(text) = test.text_str() else {
            continue;
        };
        let result = Regexp::must_compile(test.pat).find_all_string(text, test.max);
        match &test.matches {
            None => assert!(
                result.is_empty(),
                "got match {result:?}, want none: {}",
                test.desc()
            ),
            Some(m) => {
                assert!(
                    !result.is_empty(),
                    "got no match, want one: {}",
                    test.desc()
                );
                assert_eq!(
                    result.len(),
                    m.len(),
                    "got {} matches, want {}: {}",
                    result.len(),
                    m.len(),
                    test.desc()
                );
                for (k, e) in m.iter().enumerate() {
                    let want = &text[e[0] as usize..e[1] as usize];
                    assert_eq!(result[k], want, "{}", test.desc());
                }
            }
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_all_string_index() {
    for test in &find_tests() {
        let Some(text) = test.text_str() else {
            continue;
        };
        test_find_all_index(
            test,
            Regexp::must_compile(test.pat).find_all_string_index(text, test.max),
        );
    }
}

// Now come the Submatch cases.

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_submatch() {
    for test in &find_tests() {
        let result = Regexp::must_compile(test.pat).find_submatch(test.text);
        match (&test.matches, &result) {
            _ if test.max == 0 => {} // do not know whether to match or not; skip
            (None, None) => {}
            (None, Some(r)) => panic!("got match {r:?}, want none: {}", test.desc()),
            (Some(_), None) => panic!("got no match, want one: {}", test.desc()),
            (Some(m), Some(r)) => test_submatch_bytes(test, 0, &m[0], r),
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_string_submatch() {
    for test in &find_tests() {
        let Some(text) = test.text_str() else {
            continue;
        };
        let result = Regexp::must_compile(test.pat).find_string_submatch(text);
        match (&test.matches, &result) {
            _ if test.max == 0 => {} // do not know whether to match or not; skip
            (None, None) => {}
            (None, Some(r)) => panic!("got match {r:?}, want none: {}", test.desc()),
            (Some(_), None) => panic!("got no match, want one: {}", test.desc()),
            (Some(m), Some(r)) => test_submatch_string(test, 0, &m[0], r),
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_submatch_index() {
    for test in &find_tests() {
        let result = Regexp::must_compile(test.pat).find_submatch_index(test.text);
        match (&test.matches, &result) {
            _ if test.max == 0 => {} // do not know whether to match or not; skip
            (None, None) => {}
            (None, Some(r)) => panic!("got match {r:?}, want none: {}", test.desc()),
            (Some(_), None) => panic!("got no match, want one: {}", test.desc()),
            (Some(m), Some(r)) => test_submatch_indices(test, 0, &m[0], r),
        }
    }
}

// Now come the monster AllSubmatch cases.

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_all_string_submatch() {
    for test in &find_tests() {
        let Some(text) = test.text_str() else {
            continue;
        };
        let result = Regexp::must_compile(test.pat).find_all_string_submatch(text, test.max);
        match &test.matches {
            None => assert!(
                result.is_empty(),
                "got match {result:?}, want none: {}",
                test.desc()
            ),
            Some(m) => {
                assert!(
                    !result.is_empty(),
                    "got no match, want one: {}",
                    test.desc()
                );
                assert_eq!(
                    result.len(),
                    m.len(),
                    "got {} matches, want {}: {}",
                    result.len(),
                    m.len(),
                    test.desc()
                );
                for (k, matched) in m.iter().enumerate() {
                    test_submatch_string(test, k, matched, &result[k]);
                }
            }
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.7"]
fn test_find_all_submatch_index() {
    for test in &find_tests() {
        let result = Regexp::must_compile(test.pat).find_all_submatch_index(test.text, test.max);
        match &test.matches {
            None => assert!(
                result.is_empty(),
                "got match {result:?}, want none: {}",
                test.desc()
            ),
            Some(m) => {
                assert!(
                    !result.is_empty(),
                    "got no match, want one: {}",
                    test.desc()
                );
                assert_eq!(
                    result.len(),
                    m.len(),
                    "got {} matches, want {}: {}",
                    result.len(),
                    m.len(),
                    test.desc()
                );
                for (k, matched) in m.iter().enumerate() {
                    test_submatch_indices(test, k, matched, &result[k]);
                }
            }
        }
    }
}
