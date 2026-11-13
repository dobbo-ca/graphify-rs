//! Ported from strconv/quote_test.go (GOSRC/strconv/quote_test.go, Go 1.27.4).
//!
//! - TestIsPrint: ported
//! - TestIsGraphic: not ported: is_graphic is not reached by graphify-go
//! - TestQuote: ported (quotetests: 8 rows kept, 0 left out)
//! - TestQuoteToASCII: not ported: QuoteToASCII is not reached by graphify-go
//! - TestQuoteToGraphic: not ported: QuoteToGraphic is not reached by graphify-go
//! - TestQuoteRune: not ported: QuoteRune is not reached by graphify-go
//! - TestQuoteRuneToASCII: not ported: QuoteRuneToASCII is not reached by graphify-go
//! - TestQuoteRuneToGraphic: not ported: QuoteRuneToGraphic is not reached by graphify-go
//! - TestCanBackquote: not ported: CanBackquote is not reached by graphify-go
//! - TestUnquote: not ported: Unquote is not reached by graphify-go
//! - TestUnquoteInvalidUTF8: not ported: Unquote is not reached by graphify-go
//!
//! Extra test (not in Go): quote_invalid_utf8_per_byte.

use gostd::strconv::{append_quote, is_print, quote};
use gostd::unicode;

// Verify that our IsPrint agrees with unicode.IsPrint.
#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_is_print() {
    let mut n = 0;
    // Go loops over every rune including surrogates; a Rust char cannot be a
    // surrogate, so those 2048 values are skipped.
    for r in (0..=0x10FFFFu32).filter_map(char::from_u32) {
        if is_print(r) != unicode::is_print(r) {
            n += 1;
            assert!(
                n <= 10,
                "IsPrint disagrees with unicode.IsPrint (more than 10 runes)"
            );
        }
    }
    assert_eq!(n, 0, "IsPrint disagrees with unicode.IsPrint for {n} runes");
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn test_quote() {
    // (in, out)
    let quotetests: [(&[u8], &str); 8] = [
        (b"\x07\x08\x0c\r\n\t\x0b", r#""\a\b\f\r\n\t\v""#),
        (b"\\", r#""\\""#),
        (b"abc\xffdef", r#""abc\xffdef""#),
        ("\u{263a}".as_bytes(), r#""☺""#),
        ("\u{10ffff}".as_bytes(), r#""\U0010ffff""#),
        (b"\x04", r#""\x04""#),
        // Some non-printable but graphic runes.
        (
            "!\u{a0}!\u{2000}!\u{3000}!".as_bytes(),
            r#""!\u00a0!\u2000!\u3000!""#,
        ),
        (b"\x7f", r#""\x7f""#),
    ];
    for (input, out) in quotetests {
        assert_eq!(quote(input), out, "Quote({input:?})");
        let mut dst = b"abc".to_vec();
        append_quote(&mut dst, input);
        assert_eq!(
            String::from_utf8(dst).unwrap(),
            format!("abc{out}"),
            "AppendQuote(\"abc\", {input:?})"
        );
    }
}

#[test]
#[ignore = "graphify-rs-aa6.5"]
fn quote_invalid_utf8_per_byte() {
    assert_eq!(quote(b"\xE2\x82"), r#""\xe2\x82""#);
    assert_eq!(quote("\u{7f}".as_bytes()), r#""\x7f""#);
    assert_eq!(quote("a\u{2028}".as_bytes()), r#""a\u2028""#);
}
