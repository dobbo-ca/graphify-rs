//! `csv`: gostd::csv::Writer against Go encoding/csv (see parity/go/oracle/csv.go). The input is a
//! record: fields separated by 0x1F; an empty input is a record with no fields.

use super::Op;
use gostd::csv::Writer;

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let text = std::str::from_utf8(input).expect("generated input is valid UTF-8");
    let fields: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.split('\x1f').collect()
    };
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    w.write(&fields).unwrap();
    w.flush().unwrap();
    drop(w);
    vec![("write", Ok(out))]
}
