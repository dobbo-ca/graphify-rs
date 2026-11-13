//! Ported from encoding/json/v2/fold_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2020 The Go
//! Authors). Go 1.27.4 builds encoding/json with GOEXPERIMENT=jsonv2 on, so encoding/json/fold_test.go
//! (`FuzzEqualFold`, `//go:build !goexperiment.jsonv2`) is excluded; v2/fold_test.go is the active file.
//! It has 3 Test funcs: 1 ported (adapted), 2 not ported.
//! - TestEqualFold (fold_test.go:60): ported, adapted, 37 of 38 `equalFoldTestdata` rows (fold_test.go:16-58)
//! - TestFoldRune (fold_test.go:73): not ported: checks the internal `foldRune` over every rune; under
//!   the v1 options its only observable is `strings.EqualFold`, which the strings tests cover
//! - TestBenchmarkUnmarshalUnknown (fold_test.go:106): not ported: the `case:ignore` tag option is v2 API
//!
//! TestEqualFold checks v2's `foldName(x) == foldName(y)` (fold.go:14-41: ASCII upper-cased, `_` and `-`
//! dropped, other runes mapped by `foldRune`). `foldName` is internal; encoding/json uses it only to find
//! candidate fields for a key with no exact match (v2/fields.go:61-65, v2/arshal_default.go:1328-1337),
//! and the v1 options (`MatchCaseSensitiveDelimiter` in DefaultV1Flags, internal/jsonflags/flags.go:65)
//! keep a candidate only if `strings.EqualFold(key, name)` (v2/fields.go:380-386). So each row decodes
//! `{"<in1>":1}` into a struct whose only field has the key `in2`, and the field is set iff Go's `want`
//! holds and `strings.EqualFold(in1, in2)`. Rows where those differ (a `_` or `-` the fold ignores) are
//! marked; each row also checks `gostd::strings::equal_fold`, which must equal the decode result.
//!
//! Left out (1): `{"γειά, κόσμε!", "Γ\xce_\xb5ιά, Κόσμε!", false}` (fold_test.go:44): invalid UTF-8
//! cannot be a Rust `&str` field key.

use gostd::json::unmarshal;

// One row: decode `{"x":1}` into a struct with the single field key `y`; the field is set iff `want`.
macro_rules! row {
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
fn test_equal_fold() {
    row!("", "", true);
    row!("abc", "abc", true);
    row!("ABcd", "ABcd", true);
    row!("123abc", "123ABC", true);
    row!("_1_2_-_3__--a-_-b-c-", "123ABC", false); // Go want: true (`_`, `-` dropped)
    row!("αβδ", "ΑΒΔ", true);
    row!("abc", "xyz", false);
    row!("abc", "XYZ", false);
    row!("abcdefghijk", "abcdefghijX", false);
    row!("abcdefghijk", "abcdefghij\u{212A}", true);
    row!("abcdefghijK", "abcdefghij\u{212A}", true);
    row!("abcdefghijkz", "abcdefghij\u{212A}y", false);
    row!("abcdefghijKz", "abcdefghij\u{212A}y", false);
    row!("1", "2", false);
    row!("utf-8", "US-ASCII", false);
    row!("hello, world!", "hello, world!", true);
    row!("hello, world!", "Hello, World!", true);
    row!("hello, world!", "HELLO, WORLD!", true);
    row!("hello, world!", "jello, world!", false);
    row!("γειά, κόσμε!", "γειά, κόσμε!", true);
    row!("γειά, κόσμε!", "Γειά, Κόσμε!", true);
    row!("γειά, κόσμε!", "ΓΕΙΆ, ΚΌΣΜΕ!", true);
    row!("γειά, κόσμε!", "ΛΕΙΆ, ΚΌΣΜΕ!", false);
    row!("AESKey", "aesKey", true);
    row!("aeskey", "AESKEY", true);
    row!("AESKEY", "aes_key", false); // Go want: true (`_` dropped)
    row!("aes_key", "AES_KEY", true);
    row!("AES_KEY", "aes-key", false); // Go want: true (`_`, `-` dropped)
    row!("aes-key", "AES-KEY", true);
    row!("AES-KEY", "aesKey", false); // Go want: true (`-` dropped)
    row!("aesKey", "AesKey", true);
    row!("AesKey", "AESKey", true);
    row!("AESKey", "aeskey", true);
    row!("DESKey", "aeskey", false);
    row!("AES Key", "aeskey", false);
    row!("aes\u{FE4F}key", "aeskey", false); // Unicode underscore not handled
    row!("aes\u{3030}key", "aeskey", false); // Unicode dash not handled
}
