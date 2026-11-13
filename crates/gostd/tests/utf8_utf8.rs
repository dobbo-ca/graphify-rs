//! Port of unicode/utf8/utf8_test.go (Go 1.27.4).
//!
//! TestConstants: not ported: gostd has no MaxRune; Rust has char::MAX and char::REPLACEMENT_CHARACTER
//! TestFullRune: not ported: FullRune is not in the gostd API (not reached by graphify-go)
//! TestEncodeRune: not ported: Rust std encodes runes (char::encode_utf8)
//! TestAppendRune: not ported: Rust std encodes runes (String::push)
//! TestDecodeRune: ported
//! TestDecodeSurrogateRune: ported
//! TestSequencing: ported
//! TestRuntimeConversion: not ported: checks the Go runtime (`[]rune(s)`, `range`), not utf8
//! TestDecodeInvalidSequence: ported
//! TestNegativeRune: not ported: a Rust char cannot be negative; std encodes runes
//! TestRuneCount: ported
//! TestRuneCountNonASCIIAllocation: not ported: Go allocation count (testing.AllocsPerRun), no Rust equivalent
//! TestRuneLen: not ported: Rust std has char::len_utf8; a char cannot be a surrogate or out of range
//! TestValid: ported (checks std::str::from_utf8 against Go's expected value)
//! TestValidRune: not ported: a Rust char is always a valid rune (char::from_u32 checks)
//!
//! Go has separate []byte and string variants of each function; Rust takes
//! &[u8] only, so each pair collapses to one call.
//!
//! decode_rune_per_byte is not in Go: it pins the per-byte U+FFFD rule from the gostd plan.

use gostd::utf8::*;

const RUNE_ERROR: char = '\u{FFFD}';

const UTF8MAP: &[(char, &[u8])] = &[
    ('\u{0000}', b"\x00"),
    ('\u{0001}', b"\x01"),
    ('\u{007e}', b"\x7e"),
    ('\u{007f}', b"\x7f"),
    ('\u{0080}', b"\xc2\x80"),
    ('\u{0081}', b"\xc2\x81"),
    ('\u{00bf}', b"\xc2\xbf"),
    ('\u{00c0}', b"\xc3\x80"),
    ('\u{00c1}', b"\xc3\x81"),
    ('\u{00c8}', b"\xc3\x88"),
    ('\u{00d0}', b"\xc3\x90"),
    ('\u{00e0}', b"\xc3\xa0"),
    ('\u{00f0}', b"\xc3\xb0"),
    ('\u{00f8}', b"\xc3\xb8"),
    ('\u{00ff}', b"\xc3\xbf"),
    ('\u{0100}', b"\xc4\x80"),
    ('\u{07ff}', b"\xdf\xbf"),
    ('\u{0400}', b"\xd0\x80"),
    ('\u{0800}', b"\xe0\xa0\x80"),
    ('\u{0801}', b"\xe0\xa0\x81"),
    ('\u{1000}', b"\xe1\x80\x80"),
    ('\u{d000}', b"\xed\x80\x80"),
    ('\u{d7ff}', b"\xed\x9f\xbf"), // last code point before surrogate half.
    ('\u{e000}', b"\xee\x80\x80"), // first code point after surrogate half.
    ('\u{fffe}', b"\xef\xbf\xbe"),
    ('\u{ffff}', b"\xef\xbf\xbf"),
    ('\u{10000}', b"\xf0\x90\x80\x80"),
    ('\u{10001}', b"\xf0\x90\x80\x81"),
    ('\u{40000}', b"\xf1\x80\x80\x80"),
    ('\u{10fffe}', b"\xf4\x8f\xbf\xbe"),
    ('\u{10ffff}', b"\xf4\x8f\xbf\xbf"),
    ('\u{FFFD}', b"\xef\xbf\xbd"),
];

// Go rows carry the rune 0xd800/0xdfff, which Rust cannot represent as a char;
// the test only checks (RuneError, 1), so only the bytes are kept.
const SURROGATE_MAP: &[&[u8]] = &[
    b"\xed\xa0\x80", // surrogate min decodes to (RuneError, 1)
    b"\xed\xbf\xbf", // surrogate max decodes to (RuneError, 1)
];

const TEST_STRINGS: &[&[u8]] = &[
    b"",
    b"abcd",
    "☺☻☹".as_bytes(),
    "日a本b語ç日ð本Ê語þ日¥本¼語i日©".as_bytes(),
    "日a本b語ç日ð本Ê語þ日¥本¼語i日©日a本b語ç日ð本Ê語þ日¥本¼語i日©日a本b語ç日ð本Ê語þ日¥本¼語i日©"
        .as_bytes(),
    b"\x80\x80\x80\x80",
];

const INVALID_SEQUENCE_TESTS: &[&[u8]] = &[
    b"\xed\xa0\x80\x80", // surrogate min
    b"\xed\xbf\xbf\x80", // surrogate max
    // xx
    b"\x91\x80\x80\x80",
    // s1
    b"\xC2\x7F\x80\x80",
    b"\xC2\xC0\x80\x80",
    b"\xDF\x7F\x80\x80",
    b"\xDF\xC0\x80\x80",
    // s2
    b"\xE0\x9F\xBF\x80",
    b"\xE0\xA0\x7F\x80",
    b"\xE0\xBF\xC0\x80",
    b"\xE0\xC0\x80\x80",
    // s3
    b"\xE1\x7F\xBF\x80",
    b"\xE1\x80\x7F\x80",
    b"\xE1\xBF\xC0\x80",
    b"\xE1\xC0\x80\x80",
    // s4
    b"\xED\x7F\xBF\x80",
    b"\xED\x80\x7F\x80",
    b"\xED\x9F\xC0\x80",
    b"\xED\xA0\x80\x80",
    // s5
    b"\xF0\x8F\xBF\xBF",
    b"\xF0\x90\x7F\xBF",
    b"\xF0\x90\x80\x7F",
    b"\xF0\xBF\xBF\xC0",
    b"\xF0\xBF\xC0\x80",
    b"\xF0\xC0\x80\x80",
    // s6
    b"\xF1\x7F\xBF\xBF",
    b"\xF1\x80\x7F\xBF",
    b"\xF1\x80\x80\x7F",
    b"\xF1\xBF\xBF\xC0",
    b"\xF1\xBF\xC0\x80",
    b"\xF1\xC0\x80\x80",
    // s7
    b"\xF4\x7F\xBF\xBF",
    b"\xF4\x80\x7F\xBF",
    b"\xF4\x80\x80\x7F",
    b"\xF4\x8F\xBF\xC0",
    b"\xF4\x8F\xC0\x80",
    b"\xF4\x90\x80\x80",
];

const RUNE_COUNT_TESTS: &[(&[u8], usize)] = &[
    (b"abcd", 4),
    ("☺☻☹".as_bytes(), 3),
    (b"1,2,3,4", 7),
    (b"\xe2\x00", 2),
    (b"\xe2\x80", 2),
    (b"a\xe2\x80", 3),
];

const VALID_TESTS: &[(&[u8], bool)] = &[
    (b"", true),
    (b"a", true),
    (b"abc", true),
    ("Ж".as_bytes(), true),
    ("ЖЖ".as_bytes(), true),
    ("брэд-ЛГТМ".as_bytes(), true),
    ("☺☻☹".as_bytes(), true),
    (b"aa\xe2", false),
    (&[66, 250], false),
    (&[66, 250, 67], false),
    ("a\u{FFFD}b".as_bytes(), true),
    (b"\xF4\x8F\xBF\xBF", true),      // U+10FFFF
    (b"\xF4\x90\x80\x80", false),     // U+10FFFF+1; out of range
    (b"\xF7\xBF\xBF\xBF", false),     // 0x1FFFFF; out of range
    (b"\xFB\xBF\xBF\xBF\xBF", false), // 0x3FFFFFF; out of range
    (b"\xc0\x80", false),             // U+0000 encoded in two bytes: incorrect
    (b"\xed\xa0\x80", false),         // U+D800 high surrogate (sic)
    (b"\xed\xbf\xbf", false),         // U+DFFF low surrogate (sic)
];

/// Stand-in for Go's `for _, r := range s` (std, independent of gostd):
/// decodes one rune at the start of `b`; an invalid byte gives (U+FFFD, 1).
fn range_decode(b: &[u8]) -> (char, usize) {
    let head = &b[..b.len().min(4)];
    let valid = match std::str::from_utf8(head) {
        Ok(s) => s,
        Err(e) => std::str::from_utf8(&head[..e.valid_up_to()]).unwrap(),
    };
    match valid.chars().next() {
        Some(c) => (c, c.len_utf8()),
        None => (RUNE_ERROR, 1),
    }
}

fn test_sequence(s: &[u8]) {
    let mut index: Vec<(usize, char)> = Vec::new();
    let mut si = 0;
    while si < s.len() {
        let (r, size) = range_decode(&s[si..]);
        index.push((si, r));
        let (r1, size1) = decode_rune(&s[si..]);
        assert_eq!(
            r1,
            r,
            "DecodeRune({:?}) = {:#06x}, want {:#06x}",
            &s[si..],
            r1 as u32,
            r as u32
        );
        assert_eq!(
            size1,
            size,
            "DecodeRune({:?}) size {size1}, want {size}",
            &s[si..]
        );
        si += size1;
    }
    let mut j = index.len();
    si = s.len();
    while si > 0 {
        j -= 1;
        let (r1, size1) = decode_last_rune(&s[..si]);
        assert_eq!(
            r1, index[j].1,
            "DecodeLastRune({s:?}, {si}) = {:#06x}, want {:#06x}",
            r1 as u32, index[j].1 as u32
        );
        si -= size1;
        assert_eq!(
            si, index[j].0,
            "DecodeLastRune({s:?}) index mismatch at {si}, want {}",
            index[j].0
        );
    }
    assert_eq!(si, 0, "DecodeLastRune({s:?}) finished at {si}, not 0");
}

#[test]

fn test_decode_rune() {
    for &(want, b) in UTF8MAP {
        let (r, size) = decode_rune(b);
        assert!(
            r == want && size == b.len(),
            "DecodeRune({b:?}) = {:#06x}, {size} want {:#06x}, {}",
            r as u32,
            want as u32,
            b.len()
        );

        // trailing byte after the sequence must not matter
        let mut s = b.to_vec();
        s.push(0);
        let (r, size) = decode_rune(&s);
        assert!(
            r == want && size == b.len(),
            "DecodeRune({s:?}) = {:#06x}, {size} want {:#06x}, {}",
            r as u32,
            want as u32,
            b.len()
        );

        // make sure missing bytes fail
        let mut wantsize = 1;
        if wantsize >= b.len() {
            wantsize = 0;
        }
        let short = &b[..b.len() - 1];
        let (r, size) = decode_rune(short);
        assert!(
            r == RUNE_ERROR && size == wantsize,
            "DecodeRune({short:?}) = {:#06x}, {size} want {:#06x}, {wantsize}",
            r as u32,
            RUNE_ERROR as u32
        );

        // make sure bad sequences fail
        let mut bad = b.to_vec();
        let last = bad.len() - 1;
        if bad.len() == 1 {
            bad[0] = 0x80;
        } else {
            bad[last] = 0x7F;
        }
        let (r, size) = decode_rune(&bad);
        assert!(
            r == RUNE_ERROR && size == 1,
            "DecodeRune({bad:?}) = {:#06x}, {size} want {:#06x}, 1",
            r as u32,
            RUNE_ERROR as u32
        );
    }
}

#[test]

fn test_decode_surrogate_rune() {
    for &b in SURROGATE_MAP {
        let (r, size) = decode_rune(b);
        assert!(
            r == RUNE_ERROR && size == 1,
            "DecodeRune({b:?}) = {:x}, {size} want {:x}, 1",
            r as u32,
            RUNE_ERROR as u32
        );
    }
}

// Check that DecodeRune and DecodeLastRune correspond to the equivalent range loop.
#[test]

fn test_sequencing() {
    for &ts in TEST_STRINGS {
        for &(_, m) in UTF8MAP {
            let cases = [[ts, m].concat(), [m, ts].concat(), [ts, m, ts].concat()];
            for s in &cases {
                test_sequence(s);
            }
        }
    }
}

#[test]

fn test_decode_invalid_sequence() {
    for &s in INVALID_SEQUENCE_TESTS {
        let (r1, _) = decode_rune(s);
        assert_eq!(
            r1, RUNE_ERROR,
            "DecodeRune({s:x?}) = {:#06x}, want {:#06x}",
            r1 as u32, RUNE_ERROR as u32
        );
        let (r3, _) = range_decode(s);
        assert_eq!(
            r1, r3,
            "DecodeRune({s:x?}) = {:#06x} mismatch with range decode = {:#06x}",
            r1 as u32, r3 as u32
        );
    }
}

#[test]

fn test_rune_count() {
    for &(input, want) in RUNE_COUNT_TESTS {
        let out = rune_count(input);
        assert_eq!(out, want, "RuneCount({input:?}) = {out}, want {want}");
    }
}

#[test]

fn test_valid() {
    let mut tests: Vec<(Vec<u8>, bool)> = VALID_TESTS
        .iter()
        .map(|&(b, ok)| (b.to_vec(), ok))
        .collect();
    for i in 0..100 {
        let a = "a".repeat(i).into_bytes();
        let bs = "b".repeat(i).into_bytes();
        tests.push((a.clone(), true));
        tests.push(([a.as_slice(), "Ж".as_bytes()].concat(), true));
        tests.push(([a.as_slice(), b"\xe2"].concat(), false));
        tests.push(([a.as_slice(), "Ж".as_bytes(), bs.as_slice()].concat(), true));
        tests.push(([a.as_slice(), b"\xe2", bs.as_slice()].concat(), false));
    }
    for (input, want) in &tests {
        let got = std::str::from_utf8(input).is_ok();
        assert_eq!(got, *want, "Valid({input:?}) = {got}; want {want}");
    }
}

#[test]

fn decode_rune_per_byte() {
    assert_eq!(decode_rune(b"\xE2\x82"), ('\u{FFFD}', 1));
    assert_eq!(rune_count(b"\xE2\x82a"), 3);
    assert_eq!(decode_last_rune(b"a\x82"), ('\u{FFFD}', 1));
    assert_eq!(decode_rune(b""), ('\u{FFFD}', 0));
    assert_eq!(decode_last_rune(b""), ('\u{FFFD}', 0));
}
