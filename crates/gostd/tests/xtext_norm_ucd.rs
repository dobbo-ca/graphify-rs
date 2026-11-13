//! Ported from `unicode/norm/ucd_test.go` (golang.org/x/text v0.27.0), NFKC and NFKD columns only.
//!
//! - `TestStandardTests`: ported (NFKC/NFKD conformance of `NormalizationTest.txt` parts 0, 2, 3;
//!   only the `String` form is checked: Go's Bytes/Iter/Transform/Append/IsNormal checks are not ported)
//! - `TestCharacterByCharacter`: ported (NFKC/NFKD identity for every code point not listed in Part 1,
//!   plus the Part 1 conformance rows; surrogates are skipped, they are not `char`)
//! - `TestPerformance`: not ported: Go timing test on NFC.Append, NFC not called by graphify-go

use gostd::xtext::norm::{nfkc, nfkd};

struct Test {
    name: String,
    r: u32,
    cols: [String; 5],
}

fn cp_string(hex: &str) -> String {
    hex.split(' ')
        .map(|h| {
            char::from_u32(u32::from_str_radix(h, 16).expect("bad hex"))
                .expect("surrogate in NormalizationTest.txt")
        })
        .collect()
}

/// Parts indexed by their `@PartN` number (Go indexes by order of appearance, which is the same).
fn load() -> Vec<Vec<Test>> {
    let dir = std::env::var("GOSTD_UCD")
        .expect("GOSTD_UCD is not set: the test stage bind-mounts the ucd stage at /ucd");
    let path = std::path::Path::new(&dir).join("15.0.0/NormalizationTest.txt");
    let data = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut parts: Vec<Vec<Test>> = Vec::new();
    for line in data.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("@Part") {
            let n = rest[..1].parse::<usize>().expect("bad part number");
            assert_eq!(n, parts.len(), "parts out of order");
            parts.push(Vec::new());
            continue;
        }
        let (fields, name) = line
            .split_once(" # ")
            .unwrap_or_else(|| panic!("failed to parse: {line}"));
        let cols: Vec<&str> = fields.split(';').collect();
        assert!(
            cols.len() == 6 && cols[5].is_empty(),
            "failed to parse: {line}"
        );
        let r = u32::from_str_radix(cols[0].split(' ').next().unwrap(), 16).expect("bad hex");
        let cols = std::array::from_fn(|i| cp_string(cols[i]));
        parts.last_mut().expect("test before @Part").push(Test {
            name: name.to_string(),
            r,
            cols,
        });
    }
    parts
}

fn check(
    errs: &mut Vec<String>,
    label: &str,
    form: &str,
    f: fn(&str) -> String,
    gold: &str,
    input: &str,
) {
    let got = f(input);
    if got != gold {
        errs.push(format!("{label}: {form}({input:?})={got:?}; want {gold:?}"));
    }
}

fn conformance(errs: &mut Vec<String>, t: &Test) {
    for input in &t.cols {
        check(errs, &t.name, "NFKC", nfkc, &t.cols[3], input);
        check(errs, &t.name, "NFKD", nfkd, &t.cols[4], input);
    }
}

fn finish(errs: Vec<String>) {
    assert!(
        errs.is_empty(),
        "{} failures, first 20:\n{}",
        errs.len(),
        errs[..errs.len().min(20)].join("\n")
    );
}

#[test]
fn test_standard_tests() {
    let parts = load();
    let mut errs = Vec::new();
    for j in [0, 2, 3] {
        for t in &parts[j] {
            conformance(&mut errs, t);
        }
    }
    finish(errs);
}

#[test]
fn test_character_by_character() {
    let parts = load();
    let mut errs = Vec::new();
    let mut last = 0u32;
    // The final `None` is Go's special case: stop at 0x2FA1E rather than 0x10FFFF.
    for t in parts[1].iter().map(Some).chain([None]) {
        let r = t.map_or(0x2FA1E, |t| t.r);
        for cp in last + 1..r {
            // Surrogates are not `char`: skipped (Go checks U+FFFD for them).
            if let Some(c) = char::from_u32(cp) {
                let s = c.to_string();
                check(&mut errs, "Part1 unlisted", "NFKC", nfkc, &s, &s);
                check(&mut errs, "Part1 unlisted", "NFKD", nfkd, &s, &s);
            }
        }
        last = r;
        if let Some(t) = t {
            conformance(&mut errs, t);
        }
    }
    finish(errs);
}
