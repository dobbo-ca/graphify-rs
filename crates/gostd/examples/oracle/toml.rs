//! `toml`: gostd::toml::decode against BurntSushi/toml v1.4.0 Decode into `map[string]any` (see
//! parity/go/oracle/toml.go for the inputs and the `dump` format). One line per leaf, sorted
//! bytewise, each `<key path>\t<type>\t<value>`; a decode error is its text, and a `BUG:` error
//! (Go would panic) is `panic`.

use super::Op;
use gostd::strconv;
use gostd::toml::{Datetime, DatetimeKind, Table, Value, decode};

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let res = match decode(input) {
        Ok(t) => Ok(dump(&t)),
        Err(e) if e.message.starts_with("BUG: ") => Err("panic".to_string()),
        Err(e) => Err(e.to_string()),
    };
    vec![("dump", res)]
}

fn dump(t: &Table) -> Vec<u8> {
    let mut lines = Vec::new();
    walk_table("", t, &mut lines);
    lines.sort();
    let mut out = Vec::new();
    for l in lines {
        out.extend_from_slice(l.as_bytes());
        out.push(b'\n');
    }
    out
}

fn join(path: &str, seg: &str) -> String {
    if path.is_empty() {
        seg.to_string()
    } else {
        format!("{path}.{seg}")
    }
}

fn walk_table(path: &str, t: &Table, lines: &mut Vec<String>) {
    if t.is_empty() && !path.is_empty() {
        lines.push(format!("{path}\ttable\t0"));
    }
    for (k, v) in t {
        walk(&join(path, &strconv::quote(k.as_bytes())), v, lines);
    }
}

fn walk(path: &str, v: &Value, lines: &mut Vec<String>) {
    match v {
        Value::String(s) => lines.push(format!("{path}\tstring\t{}", strconv::quote(s.as_bytes()))),
        Value::Integer(i) => lines.push(format!("{path}\tinteger\t{i}")),
        Value::Float(f) => lines.push(format!(
            "{path}\tfloat\t{}",
            strconv::format_float(*f, b'g')
        )),
        Value::Bool(b) => lines.push(format!("{path}\tbool\t{b}")),
        Value::Datetime(d) => {
            let (typ, text) = datetime(d);
            lines.push(format!("{path}\t{typ}\t{text}"));
        }
        Value::Array(a) => {
            lines.push(format!("{path}\tarray\t{}", a.len()));
            for (i, e) in a.iter().enumerate() {
                walk(&join(path, &format!("[{i}]")), e, lines);
            }
        }
        Value::ArrayOfTables(a) => {
            lines.push(format!("{path}\taot\t{}", a.len()));
            for (i, e) in a.iter().enumerate() {
                walk_table(&join(path, &format!("[{i}]")), e, lines);
            }
        }
        Value::Table(t) => walk_table(path, t, lines),
    }
}

/// The toml-test type and the Go `Format` of the layouts of the plan:
/// `2006-01-02T15:04:05.999999999Z07:00`, `2006-01-02T15:04:05.999999999`, `2006-01-02` and
/// `15:04:05.999999999` (the fraction has its trailing zeros trimmed, or is dropped when zero).
fn datetime(d: &Datetime) -> (&'static str, String) {
    let date = format!("{:04}-{:02}-{:02}", d.year, d.month, d.day);
    let mut time = format!("{:02}:{:02}:{:02}", d.hour, d.minute, d.second);
    if d.nanosecond != 0 {
        time.push('.');
        time.push_str(format!("{:09}", d.nanosecond).trim_end_matches('0'));
    }
    match d.kind {
        DatetimeKind::OffsetDateTime => {
            let mut s = format!("{date}T{time}");
            match d.offset_seconds {
                None | Some(0) => s.push('Z'),
                Some(off) => {
                    let mins = off / 60;
                    let sign = if mins < 0 { '-' } else { '+' };
                    s.push_str(&format!(
                        "{sign}{:02}:{:02}",
                        mins.abs() / 60,
                        mins.abs() % 60
                    ));
                }
            }
            ("datetime", s)
        }
        DatetimeKind::LocalDateTime => ("datetime-local", format!("{date}T{time}")),
        DatetimeKind::LocalDate => ("date-local", date),
        DatetimeKind::LocalTime => ("time-local", time),
    }
}
