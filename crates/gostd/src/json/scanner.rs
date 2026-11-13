//! Go `v2_scanner.go` (the active `Valid` and `checkValid` under GOEXPERIMENT=jsonv2).

use super::Error;
use super::jsontext::{Opts, append_format};

/// Go `Valid` (`v2_scanner.go:23`).
pub fn valid(data: &[u8]) -> bool {
    check_valid(data).is_ok()
}

/// Go `checkValid` (`v2_scanner.go:27`): `Ok` or the `SyntaxError`, with the decoder's texts
/// (`jsontext/decode.go`), duplicate names and invalid UTF-8 allowed.
pub(super) fn check_valid(data: &[u8]) -> Result<(), Error> {
    append_format(
        &mut Vec::new(),
        data,
        &Opts {
            decoder: true,
            indent: None,
            escape_html: false,
            escape_js: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{Error, check_valid};

    fn err(msg: &str, offset: usize) -> Result<(), Error> {
        Err(Error::Syntax {
            msg: msg.to_string(),
            offset: offset as i64,
        })
    }

    /// Decoder texts and offsets. `Valid`, `Unmarshal` and `Decoder.Decode` share Go's
    /// `jsontext` decoder, so the Go rows of the latter two are `checkValid`'s.
    #[test]
    fn check_valid_errors() {
        let rows: &[(&str, &str, usize)] = &[
            // v2_decode_test.go:480-496 (the Unmarshal syntax rows)
            ("", "unexpected end of JSON input", 0),
            (" \n\r\t", "unexpected end of JSON input", " \n\r\t".len()),
            ("[2, 3", "unexpected end of JSON input", "[2, 3".len()),
            (
                r#"{"X": "foo", "Y"}"#,
                "invalid character '}' after object key",
                r#"{"X": "foo", "Y"}"#.len(),
            ),
            (
                r#"{"X":12x}"#,
                "invalid character 'x' after object key:value pair",
                r#"{"X":12x"#.len(),
            ),
            (
                r#"{"F3": -}"#,
                "invalid character '}' in numeric literal",
                r#"{"F3": -}"#.len(),
            ),
            (
                "\x01 42",
                "invalid character '\\x01' looking for beginning of value",
                1,
            ),
            (
                " 42 \x01",
                "invalid character '\\x01' after top-level value",
                " 42 \x01".len(),
            ),
            (
                " \"string\" \x01",
                "invalid character '\\x01' after top-level value",
                " \"string\" \x01".len(),
            ),
            // v2_decode_test.go:484
            (
                "[1, 2, 3+]",
                "invalid character '+' after array element",
                "[1, 2, 3+".len(),
            ),
            // v2_decode_test.go:1195
            (
                "[1,2,true,4,5}",
                "invalid character '}' after array element",
                "[1,2,true,4,5}".len(),
            ),
            // v2_stream_test.go:438-446
            (
                r#"{ "\a" }"#,
                r"invalid escape sequence `\a` in string",
                r#"{ "\a"#.len(),
            ),
            (
                r" \a",
                r"invalid character '\\' looking for beginning of value",
                r" \".len(),
            ),
            (
                ",",
                "invalid character ',' looking for beginning of value",
                ",".len(),
            ),
            // v2_stream_test.go:538-546
            (
                r#"{"k","#,
                "invalid character ',' after object key",
                r#"{"k","#.len(),
            ),
            (
                r#"{"k"}"#,
                "invalid character '}' after object key",
                r#"{"k"}"#.len(),
            ),
            (
                "[0. ",
                "invalid character ' ' in numeric literal",
                "[0. ".len(),
            ),
            (
                "[0:",
                "invalid character ':' after array element",
                "[0:".len(),
            ),
            (
                "fal ",
                "invalid character ' ' in literal false (expecting 's')",
                "fal ".len(),
            ),
        ];
        for &(input, msg, offset) in rows {
            assert_eq!(check_valid(input.as_bytes()), err(msg, offset), "{input:?}");
        }
    }

    /// Added (no Go row): Go's decoder reports a truncated number at its start
    /// (`consumeNumber`, jsontext/decode.go:960-963); unexpected EOF adds nothing to the offset.
    #[test]
    fn check_valid_truncated_number() {
        assert_eq!(
            check_valid(b"[1, 2.5e"),
            err("unexpected end of JSON input", 4)
        );
        assert_eq!(check_valid(b"  "), err("unexpected end of JSON input", 2));
    }
}
