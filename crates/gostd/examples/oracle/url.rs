//! `url`: gostd::url against Go net/url (see parity/go/oracle/url.go). Cases named `unescape-*`
//! run `path_unescape`; every other case runs `parse` and writes `%q` of the scheme, hostname
//! and port, one per line.

use super::Op;
use gostd::strconv::quote;
use gostd::url::{parse, path_unescape};

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    let s = std::str::from_utf8(input).expect("generated input is valid UTF-8");
    if name.starts_with("unescape-") {
        let res = path_unescape(s).map_err(|e| e.to_string());
        return vec![("path_unescape", res)];
    }
    let res = match parse(s) {
        Ok(u) => {
            let mut out = Vec::new();
            for field in [u.scheme.as_bytes(), u.hostname(), u.port()] {
                out.extend_from_slice(quote(field).as_bytes());
                out.push(b'\n');
            }
            Ok(out)
        }
        Err(e) => Err(e.to_string()),
    };
    vec![("parse", res)]
}
