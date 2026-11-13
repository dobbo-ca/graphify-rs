//! Port of `jsonenc.go`: the JSON pieces graph.json is written from by hand.

/// Go `model.AppendJSONString`: `s` as a JSON string, `encoding/json` escaping.
pub fn append_json_string(dst: &mut Vec<u8>, s: &str) {
    todo!("graphify-rs-ihc.7")
}

/// Go `model.AppendJSONFloat` (`jsonenc.go:28-34`): `'f'` form for 0 and 1e-6 <= |f| < 1e21,
/// else `encoding/json`'s `'e'` form; NaN and infinities are an error.
pub fn append_json_float(dst: &mut Vec<u8>, f: f64) -> Result<(), gostd::json::Error> {
    todo!("graphify-rs-ihc.7")
}
