//! Ported from encoding/csv/writer_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2011 The Go
//! Authors) and encoding/csv/fuzz_test.go.
//! - TestWrite (writer_test.go:52): ported (writeTests rows, `WriteAll` as `write` per record then `flush`)
//! - TestError (writer_test.go:77): ported, adapted (Go `Error()` is the `io::Result` of `flush`)
//! - FuzzRoundtrip (fuzz_test.go:15): not ported: it has no `f.Add` seeds and no testdata/fuzz corpus, and it needs
//!   the reader (graphify-go calls only the writer)
//! - quote_rules: added (no Go source), the plan's quoting cases
//!
//! writeTests has 29 rows (writer_test.go:14-50): 21 ported, 8 left out:
//! - UseCRLF: 3 rows (indexes 1, 10, 11)
//! - custom Comma: 3 rows (indexes 26, 27, 28; 28 is the errInvalidDelim row, Comma '"')
//! - invalid UTF-8 in `&str`: 2 rows (indexes 24, 25, "x09\x41\xb4\x1c")

use gostd::csv::Writer;
use std::io;

// (writeTests index, input records, output)
#[allow(clippy::type_complexity)]
const WRITE_TESTS: &[(usize, &[&[&str]], &str)] = &[
    (0, &[&["abc"]], "abc\n"),
    (2, &[&["\"abc\""]], "\"\"\"abc\"\"\"\n"),
    (3, &[&["a\"b"]], "\"a\"\"b\"\n"),
    (4, &[&["\"a\"b\""]], "\"\"\"a\"\"b\"\"\"\n"),
    (5, &[&[" abc"]], "\" abc\"\n"),
    (6, &[&["abc,def"]], "\"abc,def\"\n"),
    (7, &[&["abc", "def"]], "abc,def\n"),
    (8, &[&["abc"], &["def"]], "abc\ndef\n"),
    (9, &[&["abc\ndef"]], "\"abc\ndef\"\n"),
    (12, &[&["abc\rdef"]], "\"abc\rdef\"\n"),
    (13, &[&[""]], "\n"),
    (14, &[&["", ""]], ",\n"),
    (15, &[&["", "", ""]], ",,\n"),
    (16, &[&["", "", "a"]], ",,a\n"),
    (17, &[&["", "a", ""]], ",a,\n"),
    (18, &[&["", "a", "a"]], ",a,a\n"),
    (19, &[&["a", "", ""]], "a,,\n"),
    (20, &[&["a", "", "a"]], "a,,a\n"),
    (21, &[&["a", "a", ""]], "a,a,\n"),
    (22, &[&["a", "a", "a"]], "a,a,a\n"),
    (23, &[&["\\."]], "\"\\.\"\n"),
];

fn write_all(records: &[&[&str]]) -> String {
    let mut out = Vec::new();
    {
        let mut w = Writer::new(&mut out);
        for r in records {
            w.write(r).unwrap();
        }
        w.flush().unwrap();
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn test_write() {
    for (n, input, want) in WRITE_TESTS {
        let out = write_all(input);
        assert_eq!(out, *want, "#{n}: out={out:?} want {want:?}");
    }
}

/// Go `errorWriter`: every write fails with "Test".
struct ErrorWriter;

impl io::Write for ErrorWriter {
    fn write(&mut self, _b: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("Test"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("Test"))
    }
}

#[test]
fn test_error() {
    let mut b = Vec::new();
    let mut f = Writer::new(&mut b);
    let _ = f.write(&["abc"]);
    f.flush().unwrap();

    // Go: Write is buffered and its result ignored; Flush fails and Error() reports it.
    let mut f = Writer::new(ErrorWriter);
    let _ = f.write(&["abc"]);
    let err = f.flush().expect_err("Error should not be nil");
    assert_eq!(err.to_string(), "Test");
}

#[test]
fn quote_rules() {
    // (record, output)
    let tests: &[(&[&str], &str)] = &[
        (&[""], "\n"),
        (&["\\."], "\"\\.\"\n"),
        (&["\u{a0}x"], "\"\u{a0}x\"\n"),
        (&["\u{3000}x"], "\"\u{3000}x\"\n"),
        (&["a\"b"], "\"a\"\"b\"\n"),
    ];
    for (rec, want) in tests {
        let out = write_all(&[*rec]);
        assert_eq!(out, *want, "{rec:?}");
    }
}
