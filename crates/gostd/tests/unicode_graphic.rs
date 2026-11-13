//! Ported from unicode/graphic_test.go (Go 1.27.4).
//!
//! - TestIsControlLatin1: not ported: is_control is not reached by graphify-go
//! - TestIsLetterLatin1: ported
//! - TestIsUpperLatin1: ported
//! - TestIsLowerLatin1: not ported: is_lower is not reached by graphify-go
//! - TestNumberLatin1: ported
//! - TestIsPrintLatin1: ported
//! - TestIsGraphicLatin1: not ported: is_graphic is not reached by graphify-go
//! - TestIsPunctLatin1: not ported: is_punct is not reached by graphify-go
//! - TestIsSpaceLatin1: ported
//! - TestIsSymbolLatin1: not ported: is_symbol is not reached by graphify-go

use gostd::unicode::*;

/// Go's `In(r, PrintRanges...)` with `PrintRanges = {L, M, N, P, S}`.
fn in_print_ranges(r: char) -> bool {
    [&L, &M, &N, &P, &S].into_iter().any(|t| is(t, r))
}

#[test]

fn test_is_letter_latin1() {
    for i in 0..=0xFFu32 {
        let c = char::from_u32(i).unwrap();
        let got = is_letter(c);
        let want = is(&L, c);
        assert_eq!(got, want, "U+{i:04X} incorrect: got {got}; want {want}");
    }
}

#[test]

fn test_is_upper_latin1() {
    for i in 0..=0xFFu32 {
        let c = char::from_u32(i).unwrap();
        let got = is_upper(c);
        let want = is(&LU, c);
        assert_eq!(got, want, "U+{i:04X} incorrect: got {got}; want {want}");
    }
}

#[test]

fn test_number_latin1() {
    for i in 0..=0xFFu32 {
        let c = char::from_u32(i).unwrap();
        let got = is_number(c);
        let want = is(&N, c);
        assert_eq!(got, want, "U+{i:04X} incorrect: got {got}; want {want}");
    }
}

#[test]

fn test_is_print_latin1() {
    for i in 0..=0xFFu32 {
        let c = char::from_u32(i).unwrap();
        let got = is_print(c);
        let mut want = in_print_ranges(c);
        if c == ' ' {
            want = true;
        }
        assert_eq!(got, want, "U+{i:04X} incorrect: got {got}; want {want}");
    }
}

#[test]

fn test_is_space_latin1() {
    for i in 0..=0xFFu32 {
        let c = char::from_u32(i).unwrap();
        let got = is_space(c);
        let want = is(&WHITE_SPACE, c);
        assert_eq!(got, want, "U+{i:04X} incorrect: got {got}; want {want}");
    }
}
