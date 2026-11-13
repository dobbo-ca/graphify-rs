//! Go 1.27.4 `encoding/csv` writer (`writer.go`): default comma, `\n` line ends, no `UseCRLF`, no
//! reader. Call site: graphify-go `formats.go:147-186` (`csv.NewWriter`, `Write`, `Flush`, `Error`).

use crate::unicode::is_space;
use std::io::{self, BufWriter, Write};

const COMMA: char = ',';

/// Go `csv.Writer`: buffered, so `flush` must run to reach the inner writer.
pub struct Writer<W: io::Write> {
    w: BufWriter<W>,
}

impl<W: io::Write> Writer<W> {
    /// Go `csv.NewWriter` (`bufio.NewWriter`: 4096 bytes).
    pub fn new(w: W) -> Self {
        Writer {
            w: BufWriter::with_capacity(4096, w),
        }
    }

    /// Go `Writer.Write`: one record, quoted where Go quotes.
    pub fn write(&mut self, record: &[&str]) -> io::Result<()> {
        for (n, field) in record.iter().enumerate() {
            if n > 0 {
                self.w.write_all(b",")?;
            }
            if !field_needs_quotes(field) {
                self.w.write_all(field.as_bytes())?;
                continue;
            }
            self.w.write_all(b"\"")?;
            let mut rest = *field;
            while let Some(i) = rest.find('"') {
                self.w.write_all(&rest.as_bytes()[..=i])?;
                self.w.write_all(b"\"")?;
                rest = &rest[i + 1..];
            }
            // Go copies \r and \n through unchanged when UseCRLF is false.
            self.w.write_all(rest.as_bytes())?;
            self.w.write_all(b"\"")?;
        }
        self.w.write_all(b"\n")
    }

    /// Go `Writer.Flush` plus `Writer.Error`: the error of the flush, if any.
    pub fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

/// Go `fieldNeedsQuotes` with the default comma.
fn field_needs_quotes(field: &str) -> bool {
    if field.is_empty() {
        return false;
    }
    if field == "\\." {
        return true;
    }
    if field.contains([COMMA, '"', '\r', '\n']) {
        return true;
    }
    field.chars().next().is_some_and(is_space)
}
