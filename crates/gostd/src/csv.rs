//! Go 1.27.4 `encoding/csv` writer (`writer.go`): default comma, `\n` line ends, no `UseCRLF`, no
//! reader. Call site: graphify-go `formats.go:147-186` (`csv.NewWriter`, `Write`, `Flush`, `Error`).
#![allow(unused_variables, dead_code)] // stubs; removed in PR B

use std::io;

/// Go `csv.Writer`: buffered, so `flush` must run to reach the inner writer.
pub struct Writer<W: io::Write> {
    w: W,
}

impl<W: io::Write> Writer<W> {
    /// Go `csv.NewWriter`.
    pub fn new(w: W) -> Self {
        todo!("graphify-rs-aa6.13")
    }

    /// Go `Writer.Write`: one record, quoted where Go quotes.
    pub fn write(&mut self, record: &[&str]) -> io::Result<()> {
        todo!("graphify-rs-aa6.13")
    }

    /// Go `Writer.Flush` plus `Writer.Error`: the error of the flush, if any.
    pub fn flush(&mut self) -> io::Result<()> {
        todo!("graphify-rs-aa6.13")
    }
}
