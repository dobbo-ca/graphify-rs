//! Ported from encoding/json/fold_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2013 The Go Authors).
//! fold_test.go has no Test funcs, only one fuzz target:
//! - FuzzEqualFold (fold_test.go:13): ported, seeds only (`fuzz_equal_fold_seeds`); the fuzzing itself is
//!   not ported
//!
//! Go's fuzz body checks `foldName(x) == foldName(y)` against `bytes.EqualFold(x, y)`. `foldName` is
//! internal, so the observable here is field matching in decode: `{"<x>":1}` is decoded into a struct whose
//! only field has the key `y`, and the field is set iff Go's `foldName(x) == foldName(y)`.
//! The rule (fold.go:15-50): each ASCII byte is upper-cased (`a`-`z` only); each multi-byte rune becomes
//! `foldRune`, the smallest rune of its `unicode.SimpleFold` orbit (the loop follows `SimpleFold` until it
//! stops increasing, so e.g. K, k and U+212A KELVIN SIGN all map to `K`). Other bytes are kept. Two names
//! match iff their folded forms are equal. Each row also checks `gostd::strings::equal_fold`, which Go's
//! fuzz body uses as the oracle (`bytes.EqualFold`).
//!
//! The seed table has 25 rows (fold_test.go:16-40): 25 kept, 0 left out (all are valid UTF-8 and plain
//! JSON keys, no escaping needed). The expected result of each row is derived by hand from the rule above.

use gostd::json::unmarshal;

// One row: decode `{"x":1}` into a struct with the single field key `y`; the field is set iff `want`.
macro_rules! seed {
    ($x:literal, $y:literal, $want:literal) => {{
        #[derive(Default)]
        struct Keyed {
            v: i64,
        }
        gostd::json_fields!(decode Keyed, "json.Keyed", { v => $y });
        let data = concat!("{\"", $x, "\":1}");
        let got: Keyed = unmarshal(data.as_bytes())
            .unwrap_or_else(|e| panic!("Unmarshal({data}) error: {e}"));
        assert_eq!(got.v == 1, $want, "decode {data} into key {:?}", $y);
        assert_eq!(
            gostd::strings::equal_fold($x, $y),
            $want,
            "equal_fold({:?}, {:?})",
            $x,
            $y
        );
    }};
}

#[test]
#[ignore = "graphify-rs-aa6.10"]
fn fuzz_equal_fold_seeds() {
    seed!("", "", true);
    seed!("123abc", "123ABC", true);
    seed!("αβδ", "ΑΒΔ", true);
    seed!("abc", "xyz", false);
    seed!("abc", "XYZ", false);
    seed!("1", "2", false);
    seed!("hello, world!", "hello, world!", true);
    seed!("hello, world!", "Hello, World!", true);
    seed!("hello, world!", "HELLO, WORLD!", true);
    seed!("hello, world!", "jello, world!", false);
    seed!("γειά, κόσμε!", "γειά, κόσμε!", true);
    seed!("γειά, κόσμε!", "Γειά, Κόσμε!", true);
    seed!("γειά, κόσμε!", "ΓΕΙΆ, ΚΌΣΜΕ!", true);
    seed!("γειά, κόσμε!", "ΛΕΙΆ, ΚΌΣΜΕ!", false);
    seed!("AESKey", "aesKey", true);
    seed!("AESKEY", "aes_key", false);
    seed!("aes_key", "AES_KEY", true);
    seed!("AES_KEY", "aes-key", false);
    seed!("aes-key", "AES-KEY", true);
    seed!("AES-KEY", "aesKey", false);
    seed!("aesKey", "AesKey", true);
    seed!("AesKey", "AESKey", true);
    seed!("AESKey", "aeskey", true);
    seed!("DESKey", "aeskey", false);
    seed!("AES Key", "aeskey", false);
}
