#![no_main]
use gostd::json;
use libfuzzer_sys::fuzz_target;

// The syntax path (valid, compact, indent). A json_roundtrip target (unmarshal, marshal, unmarshal)
// comes with the decode half, graphify-rs-aa6.10.
// `valid` is true iff `compact` succeeds. For valid input, compacting the indented text gives the
// compact text, and compacting the compact text changes nothing.
fuzz_target!(|data: &[u8]| {
    let mut compact = Vec::new();
    let ok = json::compact(&mut compact, data, false).is_ok();
    assert_eq!(json::valid(data), ok);
    if !ok {
        return;
    }
    let mut indented = Vec::new();
    json::indent(&mut indented, data, "", "\t").expect("indent of valid input");
    let mut again = Vec::new();
    json::compact(&mut again, &indented, false).expect("compact of indented text");
    assert!(again == compact, "compact(indent(x)) != compact(x)");
    again.clear();
    json::compact(&mut again, &compact, false).expect("compact of compact text");
    assert!(again == compact, "compact is not idempotent");
});
