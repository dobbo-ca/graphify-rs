//! Added tests for the shared extractor helpers (coordinator review, 2026-11-13).

use std::borrow::Cow;

use crate::extract::builder::go_text;

/// A node range that ends inside a rune gives Go's bytes (one U+FFFD per bad byte, which
/// graph.json writes as Go writes the raw byte) and no panic; valid text is borrowed.
#[test]
fn go_text_mid_rune_range() {
    let src = "aé€😀".as_bytes(); // 61 | C3 A9 | E2 82 AC | F0 9F 98 80
    assert!(matches!(go_text(src), Cow::Borrowed("aé€😀")));
    assert_eq!(go_text(&src[..2]), "a\u{fffd}"); // a, then C3 cut
    assert_eq!(go_text(&src[2..3]), "\u{fffd}"); // a lone continuation byte
    assert_eq!(go_text(&src[3..5]), "\u{fffd}\u{fffd}"); // E2 82 cut: two RuneErrors in Go
    assert_eq!(go_text(&src[6..9]), "\u{fffd}\u{fffd}\u{fffd}"); // F0 9F 98 cut
    assert_eq!(go_text(&src[7..]), "\u{fffd}\u{fffd}\u{fffd}"); // 9F 98 80: no lead byte
}
