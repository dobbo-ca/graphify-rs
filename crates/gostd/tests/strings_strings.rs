//! Ported from strings/strings_test.go (GOSRC, Go 1.27.4) and bytes/bytes_test.go (TestTrimSpace).
//! strings_test.go has 41 Test funcs (7 ported, 34 not ported):
//! - TestLines: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestIndex: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestLastIndex: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestIndexAny: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestLastIndexAny: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestIndexByte: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestLastIndexByte: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestIndexRandom: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestIndexRune: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestSplit: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestSplitAfter: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestFields: ported (FieldsSeq half not ported: no stub equivalent)
//! - TestFieldsFunc: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestMap: ported (case 6 identity pointer check and case 7 invalid input left out, see below)
//! - TestToUpper: ported
//! - TestToLower: ported
//! - TestToValidUTF8: ported
//! - TestSpecialCase: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestTrimSpace: ported
//! - TestTrim: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestTrimFunc: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestIndexFunc: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestCaseConsistency: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestRepeat: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestRepeatCatchesOverflow: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestRunes: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestReadByte: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestReadRune: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestUnreadRuneError: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestReplace: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestTitle: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestContains: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestContainsAny: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestContainsRune: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestContainsFunc: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestEqualFold: ported
//! - TestCount: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestCut: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestCutLast: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestCutPrefix: not ported: not reached: not a Unicode-table function of graphify-go
//! - TestCutSuffix: not ported: not reached: not a Unicode-table function of graphify-go
//!
//! bytes_test.go: TestTrimSpace: ported as test_bytes_trim_space (all 20 rows, invalid UTF-8 kept;
//! Go's nil vs empty []byte distinction is not expressible with &[u8]).
//!
//! Table rows left out (&str API cannot hold invalid UTF-8):
//! - fieldstests: 1 row ("1\xFF\u2000\xFF2\xFF \xFF")
//! - trimSpaceTests (strings): 7 rows (" x\x80", " x\xc0", "x \xc0\xc0 ", "x \xc0", "x \xc0 ", the
//!   duplicate "x \xc0\xc0 ", "x ☺\xc0\xc0 ")
//! - upperTests/lowerTests/toValidUTF8Tests/EqualFoldTests: 0 rows
//! - TestMap: 2 cases (6: unsafe.StringData pointer identity; 7: "Hello\255World" invalid input)

use gostd::strings::*;
use gostd::unicode;

const SPACE: &str = "\t\x0b\r\x0c\n\u{85}\u{a0}\u{2000}\u{3000}";

#[test]

fn test_fields() {
    let faces = "☺☻☹";
    let tests: Vec<(String, Vec<&str>)> = vec![
        ("".into(), vec![]),
        (" ".into(), vec![]),
        (" \t ".into(), vec![]),
        ("\u{2000}".into(), vec![]),
        ("  abc  ".into(), vec!["abc"]),
        ("1 2 3 4".into(), vec!["1", "2", "3", "4"]),
        ("1  2  3  4".into(), vec!["1", "2", "3", "4"]),
        ("1\t\t2\t\t3\t4".into(), vec!["1", "2", "3", "4"]),
        (
            "1\u{2000}2\u{2001}3\u{2002}4".into(),
            vec!["1", "2", "3", "4"],
        ),
        ("\u{2000}\u{2001}\u{2002}".into(), vec![]),
        ("\n™\t™\n".into(), vec!["™", "™"]),
        ("\n\u{2000}1™2\u{2000} \u{2001} ™".into(), vec!["1™2", "™"]),
        (
            "\n1\u{FFFD} \u{FFFD}2\u{2000}3\u{FFFD}4".into(),
            vec!["1\u{FFFD}", "\u{FFFD}2", "3\u{FFFD}4"],
        ),
        // not ported: "1\xFF\u2000\xFF2\xFF \xFF" is invalid UTF-8, &str API
        (faces.into(), vec![faces]),
    ];
    for (s, want) in &tests {
        let a = fields(s);
        assert_eq!(&a, want, "Fields({s:?}) = {a:?}; want {want:?}");
    }
}

#[test]

fn test_map() {
    let ten = |c: char| c.to_string().repeat(10);
    let a = ten('a');
    // 1. Grow.
    let m = map(&a, |_| Some('\u{10FFFF}'));
    let expect = ten('\u{10FFFF}');
    assert_eq!(m, expect, "growing: expected {expect:?} got {m:?}");
    // 2. Shrink
    let m = map(&ten('\u{10FFFF}'), |_| Some('a'));
    assert_eq!(m, a, "shrinking: expected {a:?} got {m:?}");
    // 3. Rot13
    let rot13 = |r: char| {
        Some(match r {
            'a'..='z' => (((r as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((r as u8 - b'A' + 13) % 26) + b'A') as char,
            _ => r,
        })
    };
    let m = map("a to zed", rot13);
    assert_eq!(m, "n gb mrq", "rot13: expected \"n gb mrq\" got {m:?}");
    // 4. Rot13^2
    let m = map(&map("a to zed", rot13), rot13);
    assert_eq!(m, "a to zed", "rot13: expected \"a to zed\" got {m:?}");
    // 5. Drop
    let m = map("Hello, 세계", |r| {
        unicode::is(&unicode::LATIN, r).then_some(r)
    });
    assert_eq!(m, "Hello", "drop: expected \"Hello\" got {m:?}");
    // 6. Identity: not ported: unsafe.StringData pointer identity, Rust String is always a copy
    // 7. Invalid UTF-8 input: not ported: "Hello\255World" is invalid UTF-8, Rust char cannot
    //    be invalid, so Map's RuneError replacement cannot be reached
    // 8. RuneSelf and MaxRune encoding
    let encode = |r: char| {
        Some(match r {
            '\u{80}' => '\u{10FFFF}',
            '\u{10FFFF}' => '\u{80}',
            _ => r,
        })
    };
    let s = "\u{80}\u{10FFFF}";
    let r = "\u{10FFFF}\u{80}";
    let m = map(s, encode);
    assert_eq!(
        m, r,
        "encoding not handled correctly: expected {r:?} got {m:?}"
    );
    let m = map(r, encode);
    assert_eq!(
        m, s,
        "encoding not handled correctly: expected {s:?} got {m:?}"
    );
    // 9. Mapping in front, middle and back
    let m = map("   abc    123   ", |r| (!unicode::is_space(r)).then_some(r));
    assert_eq!(m, "abc123", "trimSpaces: expected \"abc123\" got {m:?}");
}

const UPPER_TESTS: &[(&str, &str)] = &[
    ("", ""),
    ("ONLYUPPER", "ONLYUPPER"),
    ("abc", "ABC"),
    ("AbC123", "ABC123"),
    ("azAZ09_", "AZAZ09_"),
    (
        "longStrinGwitHmixofsmaLLandcAps",
        "LONGSTRINGWITHMIXOFSMALLANDCAPS",
    ),
    (
        "RENAN BASTOS 93 AOSDAJDJAIDJAIDAJIaidsjjaidijadsjiadjiOOKKO",
        "RENAN BASTOS 93 AOSDAJDJAIDJAIDAJIAIDSJJAIDIJADSJIADJIOOKKO",
    ),
    (
        "long\u{250}string\u{250}with\u{250}nonascii\u{2C6F}chars",
        "LONG\u{2C6F}STRING\u{2C6F}WITH\u{2C6F}NONASCII\u{2C6F}CHARS",
    ),
    (
        "\u{250}\u{250}\u{250}\u{250}\u{250}",
        "\u{2C6F}\u{2C6F}\u{2C6F}\u{2C6F}\u{2C6F}",
    ), // grows one byte per char
    ("a\u{80}\u{10FFFF}", "A\u{80}\u{10FFFF}"), // test utf8.RuneSelf and utf8.MaxRune
];

const LOWER_TESTS: &[(&str, &str)] = &[
    ("", ""),
    ("abc", "abc"),
    ("AbC123", "abc123"),
    ("azAZ09_", "azaz09_"),
    (
        "longStrinGwitHmixofsmaLLandcAps",
        "longstringwithmixofsmallandcaps",
    ),
    (
        "renan bastos 93 AOSDAJDJAIDJAIDAJIaidsjjaidijadsjiadjiOOKKO",
        "renan bastos 93 aosdajdjaidjaidajiaidsjjaidijadsjiadjiookko",
    ),
    (
        "LONG\u{2C6F}STRING\u{2C6F}WITH\u{2C6F}NONASCII\u{2C6F}CHARS",
        "long\u{250}string\u{250}with\u{250}nonascii\u{250}chars",
    ),
    (
        "\u{2C6D}\u{2C6D}\u{2C6D}\u{2C6D}\u{2C6D}",
        "\u{251}\u{251}\u{251}\u{251}\u{251}",
    ), // shrinks one byte per char
    ("A\u{80}\u{10FFFF}", "a\u{80}\u{10FFFF}"), // test utf8.RuneSelf and utf8.MaxRune
];

#[test]

fn test_to_upper() {
    for (inp, out) in UPPER_TESTS {
        let actual = to_upper(inp);
        assert_eq!(actual, *out, "ToUpper({inp:?}) = {actual:?}; want {out:?}");
    }
}

#[test]

fn test_to_lower() {
    for (inp, out) in LOWER_TESTS {
        let actual = to_lower(inp);
        assert_eq!(actual, *out, "ToLower({inp:?}) = {actual:?}; want {out:?}");
    }
}

#[test]

fn test_to_valid_utf8() {
    let tests: &[(&[u8], &str, &str)] = &[
        (b"", "\u{FFFD}", ""),
        (b"abc", "\u{FFFD}", "abc"),
        (b"\xEF\xB7\x9D", "\u{FFFD}", "\u{FDDD}"),
        (b"a\xffb", "\u{FFFD}", "a\u{FFFD}b"),
        (b"a\xffb\xEF\xBF\xBD", "X", "aXb\u{FFFD}"),
        (
            b"a\xE2\x98\xBA\xffb\xE2\x98\xBA\xC0\xAFc\xE2\x98\xBA\xff",
            "",
            "a☺b☺c☺",
        ),
        (
            b"a\xE2\x98\xBA\xffb\xE2\x98\xBA\xC0\xAFc\xE2\x98\xBA\xff",
            "日本語",
            "a☺日本語b☺日本語c☺日本語",
        ),
        (b"\xC0\xAF", "\u{FFFD}", "\u{FFFD}"),
        (b"\xE0\x80\xAF", "\u{FFFD}", "\u{FFFD}"),
        (b"\xed\xa0\x80", "abc", "abc"),
        (b"\xed\xbf\xbf", "\u{FFFD}", "\u{FFFD}"),
        (b"\xF0\x80\x80\xaf", "☺", "☺"),
        (b"\xF8\x80\x80\x80\xAF", "\u{FFFD}", "\u{FFFD}"),
        (b"\xFC\x80\x80\x80\x80\xAF", "\u{FFFD}", "\u{FFFD}"),
    ];
    for (inp, repl, out) in tests {
        let got = to_valid_utf8(inp, repl);
        assert_eq!(
            got, *out,
            "ToValidUTF8({inp:?}, {repl:?}) = {got:?}; want {out:?}"
        );
    }
}

#[test]

fn test_trim_space() {
    let sp = SPACE;
    let tests: Vec<(String, &str)> = vec![
        ("".into(), ""),
        ("abc".into(), "abc"),
        (format!("{sp}abc{sp}"), "abc"),
        (" ".into(), ""),
        (" \t\r\n \t\t\r\r\n\n ".into(), ""),
        (" \t\r\n x\t\t\r\r\n\n ".into(), "x"),
        (
            " \u{2000}\t\r\n x\t\t\r\r\ny\n \u{3000}".into(),
            "x\t\t\r\r\ny",
        ),
        ("1 \t\r\n2".into(), "1 \t\r\n2"),
        // not ported (7 rows): invalid UTF-8, &str API:
        // " x\x80", " x\xc0", "x \xc0\xc0 ", "x \xc0", "x \xc0 ", "x \xc0\xc0 ", "x ☺\xc0\xc0 "
        ("x ☺ ".into(), "x ☺"),
    ];
    for (inp, out) in &tests {
        let actual = trim_space(inp);
        assert_eq!(
            actual, *out,
            "TrimSpace({inp:?}) = {actual:?}; want {out:?}"
        );
    }
}

#[test]

fn test_bytes_trim_space() {
    let sp = SPACE.as_bytes();
    let wrap = [sp, b"abc", sp].concat();
    let tests: Vec<(Vec<u8>, Vec<u8>)> = vec![
        (b"".to_vec(), b"".to_vec()),
        (b"  a".to_vec(), b"a".to_vec()),
        (b"b  ".to_vec(), b"b".to_vec()),
        (b"abc".to_vec(), b"abc".to_vec()),
        (wrap, b"abc".to_vec()),
        (b" ".to_vec(), b"".to_vec()),
        ("\u{3000} ".as_bytes().to_vec(), b"".to_vec()),
        (" \u{3000}".as_bytes().to_vec(), b"".to_vec()),
        (b" \t\r\n \t\t\r\r\n\n ".to_vec(), b"".to_vec()),
        (b" \t\r\n x\t\t\r\r\n\n ".to_vec(), b"x".to_vec()),
        (
            " \u{2000}\t\r\n x\t\t\r\r\ny\n \u{3000}"
                .as_bytes()
                .to_vec(),
            b"x\t\t\r\r\ny".to_vec(),
        ),
        (b"1 \t\r\n2".to_vec(), b"1 \t\r\n2".to_vec()),
        (b" x\x80".to_vec(), b"x\x80".to_vec()),
        (b" x\xc0".to_vec(), b"x\xc0".to_vec()),
        (b"x \xc0\xc0 ".to_vec(), b"x \xc0\xc0".to_vec()),
        (b"x \xc0".to_vec(), b"x \xc0".to_vec()),
        (b"x \xc0 ".to_vec(), b"x \xc0".to_vec()),
        (b"x \xc0\xc0 ".to_vec(), b"x \xc0\xc0".to_vec()),
        (
            b"x \xe2\x98\xba\xc0\xc0 ".to_vec(),
            b"x \xe2\x98\xba\xc0\xc0".to_vec(),
        ),
        ("x ☺ ".as_bytes().to_vec(), "x ☺".as_bytes().to_vec()),
    ];
    for (inp, out) in &tests {
        let actual = bytes_trim_space(inp);
        assert_eq!(
            actual,
            &out[..],
            "TrimSpace({inp:?}) = {actual:?}; want {out:?}"
        );
    }
}

#[test]

fn test_equal_fold() {
    let tests: &[(&str, &str, bool)] = &[
        ("abc", "abc", true),
        ("ABcd", "ABcd", true),
        ("123abc", "123ABC", true),
        ("αβδ", "ΑΒΔ", true),
        ("abc", "xyz", false),
        ("abc", "XYZ", false),
        ("abcdefghijk", "abcdefghijX", false),
        ("abcdefghijk", "abcdefghij\u{212A}", true),
        ("abcdefghijK", "abcdefghij\u{212A}", true),
        ("abcdefghijkz", "abcdefghij\u{212A}y", false),
        ("abcdefghijKz", "abcdefghij\u{212A}y", false),
        ("1", "2", false),
        ("utf-8", "US-ASCII", false),
    ];
    for &(s, t, out) in tests {
        assert_eq!(
            equal_fold(s, t),
            out,
            "EqualFold({s:?}, {t:?}) = {}, want {out}",
            !out
        );
        assert_eq!(
            equal_fold(t, s),
            out,
            "EqualFold({t:?}, {s:?}) = {}, want {out}",
            !out
        );
    }
}
