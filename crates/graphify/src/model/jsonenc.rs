//! Port of `jsonenc.go`: what encoding/json writes for the plain cases, by hand.

use gostd::json::{append_float, append_string};

/// Go `model.AppendJSONString`: `s` quoted as `encoding/json` does (HTML escaping on).
/// Printable ASCII with nothing to escape is copied.
pub fn append_json_string(dst: &mut Vec<u8>, s: &str) {
    let plain = s
        .bytes()
        .all(|c| (0x20..0x7f).contains(&c) && !matches!(c, b'"' | b'\\' | b'<' | b'>' | b'&'));
    if !plain {
        append_string(dst, s.as_bytes(), true);
        return;
    }
    dst.push(b'"');
    dst.extend_from_slice(s.as_bytes());
    dst.push(b'"');
}

/// Go `model.AppendJSONFloat` (`jsonenc.go:28-34`): `'f'` form for 0 and 1e-6 <= |f| < 1e21,
/// else `encoding/json`'s form; NaN and infinities are its error.
pub fn append_json_float(dst: &mut Vec<u8>, f: f64) -> Result<(), gostd::json::Error> {
    let a = f.abs();
    if a == 0.0 || (1e-6..1e21).contains(&a) {
        gostd::strconv::append_float(dst, f, b'f');
        return Ok(());
    }
    append_float(dst, f)
}
