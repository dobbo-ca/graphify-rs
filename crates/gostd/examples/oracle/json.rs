//! `json`: gostd::json against Go encoding/json (see parity/go/oracle/json.go).

use super::Op;
use gostd::json;

fn records(mut input: &[u8]) -> Vec<&[u8]> {
    let mut v = Vec::new();
    while let Some((len, rest)) = input.split_first_chunk::<4>() {
        let (s, rest) = rest.split_at(u32::from_be_bytes(*len) as usize);
        v.push(s);
        input = rest;
    }
    v
}

fn line(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(b);
    out.push(b'\n');
}

fn strings(input: &[u8]) -> Vec<Op> {
    let (mut html, mut nohtml) = (Vec::new(), Vec::new());
    for s in records(input) {
        json::append_string(&mut html, s, true);
        html.push(b'\n');
        json::append_string(&mut nohtml, s, false);
        nohtml.push(b'\n');
    }
    vec![("html", Ok(html)), ("nohtml", Ok(nohtml))]
}

fn floats(input: &[u8]) -> Vec<Op> {
    let text = std::str::from_utf8(input).expect("json floats are ASCII");
    let mut out = Vec::new();
    for l in text.strip_suffix('\n').unwrap_or(text).split('\n') {
        let f = f64::from_bits(u64::from_str_radix(l, 16).unwrap());
        if let Err(e) = json::append_float(&mut out, f) {
            out.extend_from_slice(e.to_string().as_bytes());
        }
        out.push(b'\n');
    }
    vec![("json", Ok(out))]
}

fn docs(input: &[u8]) -> Vec<Op> {
    let (mut valid, mut compact, mut compact_html, mut indent) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for d in records(input) {
        line(&mut valid, if json::valid(d) { b"1" } else { b"0" });
        let mut c = Vec::new();
        match json::compact(&mut c, d, false) {
            Ok(()) => {
                line(&mut compact, &c);
                let mut h = Vec::new();
                json::html_escape(&mut h, &c);
                line(&mut compact_html, &h);
            }
            Err(e) => {
                line(&mut compact, e.to_string().as_bytes());
                line(&mut compact_html, e.to_string().as_bytes());
            }
        }
        let mut i = Vec::new();
        let rec = match json::indent(&mut i, d, ">", "\t") {
            Ok(()) => i,
            Err(e) => [&[0][..], e.to_string().as_bytes()].concat(),
        };
        indent.extend_from_slice(&(rec.len() as u32).to_be_bytes());
        indent.extend_from_slice(&rec);
    }
    vec![
        ("valid", Ok(valid)),
        ("compact", Ok(compact)),
        ("compact_html", Ok(compact_html)),
        ("indent", Ok(indent)),
    ]
}

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    if name == "strings" {
        strings(input)
    } else if name == "floats" {
        floats(input)
    } else if name.starts_with("docs-") {
        docs(input)
    } else {
        Vec::new()
    }
}
