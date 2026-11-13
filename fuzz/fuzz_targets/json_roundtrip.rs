#![no_main]
use gostd::json::{self, Value};
use libfuzzer_sys::fuzz_target;

// unmarshal into the dynamic tree; on success marshal it and unmarshal again: the second tree
// equals the first. Invalid UTF-8 and lone surrogates become U+FFFD on the first decode and stay
// so. Numbers outside f64 fail the first decode, so marshal never sees NaN or an infinity; -0
// marshals as "-0" and compares equal to 0.
fuzz_target!(|data: &[u8]| {
    let Ok(first) = json::unmarshal::<Value>(data) else {
        return;
    };
    let out = json::marshal(&first).expect("marshal of a decoded value");
    let second = json::unmarshal::<Value>(&out).expect("unmarshal of marshal output");
    assert!(first == second, "unmarshal(marshal(v)) != v");
});
