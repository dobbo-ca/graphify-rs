//! `xml-dec`: gostd::xml::unmarshal_element against Go `xml.Unmarshal` into graphify's `xmlElem`
//! (see parity/go/oracle/xml.go).

use super::Op;
use gostd::xml::{Element, unmarshal_element};
use std::fmt::Write;

/// The same escaping as `xmlDecEsc` in xml.go: `\`, `|`, newline and CR escaped, other bytes
/// outside 0x20..0x7e as `\xHH`.
fn esc(out: &mut String, s: &str) {
    for &c in s.as_bytes() {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'|' => out.push_str("\\|"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            0..0x20 | 0x7f.. => write!(out, "\\x{c:02x}").unwrap(),
            _ => out.push(char::from(c)),
        }
    }
}

/// One line per element, pre-order: `depth space|local|chardata`.
fn dump(out: &mut String, e: &Element, depth: usize) {
    write!(out, "{depth} ").unwrap();
    esc(out, &e.name.space);
    out.push('|');
    esc(out, &e.name.local);
    out.push('|');
    esc(out, &e.chardata);
    out.push('\n');
    for c in &e.children {
        dump(out, c, depth + 1);
    }
}

pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    // Inputs nested 10,000 deep recurse in `dump` and in the drop of the tree: give them room.
    let out = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(s, || match unmarshal_element(input) {
                Ok(e) => {
                    let mut o = String::new();
                    dump(&mut o, &e, 0);
                    o
                }
                // graphify-go discards the error (parsePom), so only "an error" is compared.
                Err(_) => "error".to_string(),
            })
            .unwrap()
            .join()
            .unwrap()
    });
    vec![("elem", Ok(out.into_bytes()))]
}
