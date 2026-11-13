//! Port of internal/strconv/fp_test.go (GOSRC/internal/strconv/fp_test.go, Go 1.27.4).
//!
//! TestFp: not ported: every testfp.txt line is a printf format `%b` or `%.Ne` (fixed precision, or binary exponent output); gostd only has shortest (prec -1) formatting
//! TestParseFloatTestdata: ported
//! TestFormatFloatTestdata: not ported: it only calls FormatFloat(f, 'e', i, 64) for i in 0..19 (fixed precision), and compares Go's optimized and unoptimized paths, which gostd does not have
//!
//! testfp.txt: 0 of 136 data lines kept (all fixed precision or %b). atof1k.txt: 1000 of 1000 lines kept.
//! ftoa1k.txt: 0 of 1000 lines kept (no prec -1 call in the Go test).

use gostd::strconv::parse_float;

const ATOF1K: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/strconv/atof1k.txt");

// Go compares ParseFloat with SetOptimize(false) against SetOptimize(true); gostd has one
// path, so the check kept is that every line of the testbase sample parses without error.
#[test]
fn test_parse_float_testdata() {
    let data = std::fs::read_to_string(ATOF1K).unwrap_or_else(|e| panic!("{ATOF1K}: {e}"));
    let mut fail = 0;
    for (i, line) in data.lines().enumerate() {
        let s = line.trim();
        if s.starts_with('#') || s.is_empty() {
            continue;
        }
        if let Err(err) = parse_float(s) {
            eprintln!("atof1k.txt:{}: ParseFloat({s:?}): {err}", i + 1);
            fail += 1;
            assert!(fail <= 100, "too many failures");
        }
    }
    assert_eq!(fail, 0, "ParseFloat failures");
}
