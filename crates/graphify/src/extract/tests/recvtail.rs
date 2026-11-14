//! graphify-go `internal/extract/recvtail_test.go`: `TestRecvTailMatchesRegexp` and the seeds of
//! `FuzzRecvTail` as one table (plan Port rules). Go compares `recvTail` with
//! `regexp.MustCompile("[\p{L}\p{N}_]+$").FindString`; std has no `\p{L}`, so the table holds
//! what the expression gives for each seed. An invalid UTF-8 seed goes through
//! `String::from_utf8_lossy`: an invalid byte is U+FFFD, which is no letter or number, as in
//! Go's scan and regexp.

use crate::extract::resolve::recv_tail;

#[test]
#[ignore = "graphify-rs-ihc.18"]
fn recv_tail_matches_regexp_seeds() {
    // (seed, what `[\p{L}\p{N}_]+$` finds in it)
    let seeds: &[(&[u8], &str)] = &[
        (b"", ""),
        (b"a", "a"),
        (b"a.b", "b"),
        (b"a.b.c_1", "c_1"),
        (b"f()", ""),
        (b"a[0]", ""),
        (b"self", "self"),
        (b"$this", "this"),
        (b"@self", "self"),
        (b"super()", ""),
        (b"a.", ""),
        (b".", ""),
        (b"_", "_"),
        (b"__x__", "__x__"),
        (b"a b", "b"),
        (b"x\n", ""),
        (b"ns::Type", "Type"),
        (b"ptr->field", "field"),
        ("na\u{ef}ve".as_bytes(), "na\u{ef}ve"),
        (
            "obj.\u{65e5}\u{672c}\u{8a9e}".as_bytes(),
            "\u{65e5}\u{672c}\u{8a9e}",
        ),
        ("x.\u{663}\u{664}".as_bytes(), "\u{663}\u{664}"),
        ("a.\u{216b}".as_bytes(), "\u{216b}"),
        ("\u{e9}".as_bytes(), "\u{e9}"),
        ("a.\u{301}".as_bytes(), ""),   // a combining mark is no letter
        ("a\u{200b}b".as_bytes(), "b"), // a zero-width space is no letter
        ("x.\u{bd}".as_bytes(), "\u{bd}"),
        (b"\xff", ""),
        (b"a\xff", ""),
        (b"\xffa", "a"),
        (b"a.\xc3", ""),
        (b"\xc3a", "a"),
        (b"\xe4\xb8", ""),
        (b"\xe4\xb8\xc3\xa9", "\u{e9}"),
        (b"\xc3\xa9\xe4\xb8", ""),
        (b"\xf0\x9f\x98", ""),
        (b"a\xf0\x9f\x98\x80b", "b"),
        (b"\xc0\x80a", "a"),
        (b"\xed\xa0\x80z", "z"),
        (b"a\xef\xbf\xbd", ""),
        (b"\xef\xbf\xbdb", "b"),
    ];
    for (seed, want) in seeds {
        let s = String::from_utf8_lossy(seed);
        let got = recv_tail(&s);
        assert_eq!(
            got, *want,
            "recv_tail({seed:?}) = {got:?}, regexp gives {want:?}"
        );
    }
}
