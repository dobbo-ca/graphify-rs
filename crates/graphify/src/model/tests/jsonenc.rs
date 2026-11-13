//! The plan's `append_json_float_thresholds` (Task 6): `jsonenc.go:28-34`.

use crate::model::{append_json_float, append_json_string};

fn float(f: f64) -> Result<String, gostd::json::Error> {
    let mut b = Vec::new();
    append_json_float(&mut b, f)?;
    Ok(String::from_utf8(b).unwrap())
}

#[test]
#[ignore = "graphify-rs-ihc.7"]
fn append_json_float_thresholds() {
    // 'f' form for 0 and 1e-6 <= |f| < 1e21, else encoding/json's 'e' form with "e-07" made "e-7".
    for (f, want) in [
        (0.0, "0"),
        (-0.0, "-0"),
        (1e-6, "0.000001"),
        (1e-7, "1e-7"),
        (-1e-7, "-1e-7"),
        (1.5, "1.5"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (123456789e30, "1.23456789e+38"),
    ] {
        assert_eq!(float(f).unwrap(), want, "{f:e}");
    }
    for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(float(f).is_err(), "{f} must be an error");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.7"]
fn append_json_string_matches_marshal() {
    for s in [
        "plain",
        "",
        "a\"b\\c",
        "<&>",
        "tab\there",
        "é",
        "\u{2028}",
        "\x00\x7f",
    ] {
        let mut b = Vec::new();
        append_json_string(&mut b, s);
        assert_eq!(b, gostd::json::marshal(s).unwrap(), "{s:?}");
    }
}
