#![no_main]
use libfuzzer_sys::fuzz_target;

// Any input is a table or a ParseError, never a panic.
fuzz_target!(|data: &[u8]| {
    let _ = gostd::toml::decode(data);
});
