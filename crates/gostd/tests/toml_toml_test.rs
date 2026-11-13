//! Ported from BurntSushi/toml v1.4.0 toml_test.go (MIT, Copyright (c) 2013 TOML authors,
//! see `testdata/toml/COPYING`) and internal/toml-test (runner.go, json.go, version.go).
//! - TestToml (toml_test.go:249): ported as `test_toml`, the TOML 1.0 run, decode half only
//! - TestTomlNextFails (toml_test.go:261): ported as `test_toml_next_fails`
//! - errorTests (toml_test.go:23): ported as `error_tests` (the error text checks of
//!   `testError`, toml_test.go:441)
//!
//! Not ported:
//! - TestTomlNext (toml_test.go:254): TOML 1.1, not reached (graphify-go never sets
//!   BURNTSUSHI_TOML_110)
//! - the encode half of `runTomlTest` (`run(t, true)`, `parser.Encode`): not reached, graphify-go
//!   only decodes
//! - `metaTests`, `testMeta`: `MetaData` is not reached
//!
//! The cases are read at run time from `$GOSTD_GOTEST/toml-test/`, the `internal/toml-test/tests`
//! tree. As in Go's `Runner.List`, the files excluded for TOML 1.0 by `version.go:24-32` are not
//! run (5 valid cases; the `invalid/string/bad-hex-esc` entry is a glob without a wildcard and
//! matches no file, so no invalid case is dropped). That leaves 180 valid and 371 invalid cases
//! (185 and 371 `.toml` files in the tree); both counts are asserted, so a missing or short tree
//! fails loudly.
//!
//! Differences from the Go runner:
//! - Go skips 16 invalid cases (`SkipTests`, toml_test.go:315-338) that BurntSushi accepts or that
//!   depend on the Go version; they are skipped here too and the port must match Go on them
//!   (355 invalid cases are asserted to be errors).
//! - Go skips `valid/comment/noeol` and `valid/comment/nonascii` because the command runner
//!   cannot handle their output; here `decode` is called directly, so they are compared.
//! - Go's `cmpAsDatetimes` (json.go:200) parses the wanted string twice and never looks at the
//!   decoded value, so datetimes are only checked by tag there. Here the decoded `Datetime` is
//!   compared field by field with the wanted string, which is stricter.
//! - The `.json` files are read with a small local parser (no decode half in `gostd::json` yet).

use gostd::toml::{Datetime, DatetimeKind, Value, decode};
use std::collections::BTreeMap;
use std::path::Path;

/// `version.go:24-32`, the "1.0.0" excludes that match a file.
const EXCLUDED_1_0: [&str; 5] = [
    "valid/string/escape-esc",
    "valid/string/hex-escape",
    "valid/datetime/no-seconds",
    "valid/inline-table/newline",
    "valid/key/unicode",
];

/// `toml_test.go:315-338`, `Runner.SkipTests` for the invalid cases.
const SKIP_INVALID: [&str; 16] = [
    "invalid/datetime/offset-overflow-hour",
    "invalid/datetime/offset-overflow-minute",
    "invalid/array/extend-defined-aot",
    "invalid/inline-table/duplicate-key-3",
    "invalid/inline-table/overwrite-02",
    "invalid/inline-table/overwrite-07",
    "invalid/inline-table/overwrite-08",
    "invalid/spec/inline-table-2-0",
    "invalid/spec/table-9-1",
    "invalid/table/append-to-array-with-dotted-keys",
    "invalid/table/append-with-dotted-keys-1",
    "invalid/table/append-with-dotted-keys-2",
    "invalid/table/duplicate-key-dotted-table",
    "invalid/table/duplicate-key-dotted-table2",
    "invalid/table/redefine-2",
    "invalid/table/redefine-3",
];

/// `toml_test.go:261-267`, `TestTomlNextFails`.
const NEXT_FAILS: [&str; 5] = [
    "valid/string/escape-esc",
    "valid/datetime/no-seconds",
    "valid/string/hex-escape",
    "valid/inline-table/newline",
    "valid/key/unicode",
];

/// `toml_test.go:23-27`, `errorTests`: (path glob under `invalid/`, text the error must contain,
/// number of files the glob must match).
const ERROR_TESTS: [(&str, &str, usize); 3] = [
    ("encoding/bad-utf8*", "invalid UTF-8 byte", 6),
    (
        "encoding/utf16*",
        "files cannot contain NULL bytes; probably using UTF-16",
        3,
    ),
    ("string/multiline-bad-escape-2", r"invalid escape: '\ '", 1),
];

// ---- JSON (only what the toml-test .json files use: objects, arrays, strings) ----

#[derive(Debug)]
enum J {
    Str(String),
    Arr(Vec<J>),
    Obj(BTreeMap<String, J>),
}

struct JsonParser<'a> {
    b: &'a [u8],
    i: usize,
}

impl JsonParser<'_> {
    fn ws(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn value(&mut self) -> J {
        self.ws();
        match self.b.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut m = BTreeMap::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b'}' {
                        self.i += 1;
                        return J::Obj(m);
                    }
                    if self.b[self.i] == b',' {
                        self.i += 1;
                        self.ws();
                    }
                    let k = self.string();
                    self.ws();
                    assert_eq!(self.b[self.i], b':', "json: expected ':' at {}", self.i);
                    self.i += 1;
                    let v = self.value();
                    m.insert(k, v); // as Go: the last duplicate key wins
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        return J::Arr(a);
                    }
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                    a.push(self.value());
                }
            }
            Some(b'"') => J::Str(self.string()),
            c => panic!("json: unexpected {c:?} at {}", self.i),
        }
    }

    fn hex4(&mut self) -> u32 {
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4]).unwrap();
        self.i += 4;
        u32::from_str_radix(s, 16).unwrap()
    }

    fn string(&mut self) -> String {
        assert_eq!(self.b[self.i], b'"', "json: expected string at {}", self.i);
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).expect("json: invalid UTF-8"),
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let r = self.hex4();
                            if (0xD800..0xDC00).contains(&r) && self.b[self.i..].starts_with(b"\\u")
                            {
                                let save = self.i;
                                self.i += 2;
                                let r2 = self.hex4();
                                if (0xDC00..0xE000).contains(&r2) {
                                    char::from_u32(0x10000 + ((r - 0xD800) << 10) + (r2 - 0xDC00))
                                        .unwrap()
                                } else {
                                    self.i = save;
                                    '\u{FFFD}'
                                }
                            } else {
                                char::from_u32(r).unwrap_or('\u{FFFD}') // lone surrogate, as Go
                            }
                        }
                        _ => panic!("json: bad escape at {}", self.i),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
    }
}

fn parse_json(b: &[u8]) -> J {
    JsonParser { b, i: 0 }.value()
}

// ---- the toml-test comparison (json.go) ----

fn is_value(m: &BTreeMap<String, J>) -> bool {
    m.len() == 2
        && matches!(
            (m.get("type"), m.get("value")),
            (Some(J::Str(_)), Some(J::Str(_)))
        )
}

fn join(key: &str, k: &str) -> String {
    if key.is_empty() {
        k.to_string()
    } else {
        format!("{key}.{k}")
    }
}

/// `CompareJSON`: the wanted tagged JSON against a decoded value.
fn cmp(want: &J, have: &Value, key: &str) -> Result<(), String> {
    match want {
        J::Obj(m) if is_value(m) => cmp_value(m, have, key),
        J::Obj(m) => {
            let Value::Table(t) = have else {
                return Err(format!("key {key:?}: want a table, have {have:?}"));
            };
            for k in m.keys() {
                if !t.contains_key(k) {
                    return Err(format!(
                        "could not find key {:?} in parser output",
                        join(key, k)
                    ));
                }
            }
            for k in t.keys() {
                if !m.contains_key(k) {
                    return Err(format!(
                        "could not find key {:?} in expected output",
                        join(key, k)
                    ));
                }
            }
            for (k, w) in m {
                cmp(w, &t[k], &join(key, k))?;
            }
            Ok(())
        }
        J::Arr(a) => {
            let items: Vec<Value> = match have {
                Value::Array(v) => v.clone(),
                Value::ArrayOfTables(v) => v.iter().cloned().map(Value::Table).collect(),
                _ => return Err(format!("key {key:?}: want an array, have {have:?}")),
            };
            if a.len() != items.len() {
                return Err(format!(
                    "array lengths differ for key {key:?}: want {}, have {}",
                    a.len(),
                    items.len()
                ));
            }
            for (w, h) in a.iter().zip(&items) {
                cmp(w, h, key)?;
            }
            Ok(())
        }
        J::Str(_) => Err(format!("key {key:?}: bare string in expected output")),
    }
}

fn cmp_value(m: &BTreeMap<String, J>, have: &Value, key: &str) -> Result<(), String> {
    let (J::Str(ty), J::Str(val)) = (&m["type"], &m["value"]) else {
        unreachable!("is_value")
    };
    let ok = match (ty.as_str(), have) {
        ("string", Value::String(s)) => s == val,
        ("integer", Value::Integer(i)) => i.to_string() == *val,
        // Go lower-cases both sides of a bool.
        ("bool", Value::Bool(b)) => b.to_string() == val.to_lowercase(),
        ("float", Value::Float(f)) => {
            let w = val.to_lowercase();
            if w.ends_with("nan") {
                f.is_nan() // json.go:155: the sign of a NaN is ignored
            } else {
                w.parse::<f64>().expect("BUG IN TEST CASE: float") == *f
            }
        }
        ("datetime" | "datetime-local" | "date-local" | "time-local", Value::Datetime(d)) => {
            *d == parse_datetime(ty, val, d)
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!("key {key:?}: want {ty} {val:?}, have {have:?}"))
    }
}

/// The wanted `Datetime` for a tagged value. `datetimeRepl` first: ' ' and 't'
/// become 'T', 'z' becomes 'Z'. A `LocalTime` has no date, so its date fields are taken from
/// `have`.
fn parse_datetime(kind: &str, s: &str, have: &Datetime) -> Datetime {
    let s = s.replace([' ', 't'], "T").replace('z', "Z");
    let mut d = Datetime {
        kind: DatetimeKind::LocalDate,
        year: 0,
        month: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        nanosecond: 0,
        offset_seconds: None,
    };
    let (date, time) = match kind {
        "date-local" => (Some(s.as_str()), None),
        "time-local" => (None, Some(s.as_str())),
        _ => {
            let (a, b) = s.split_once('T').expect("BUG IN TEST CASE: datetime");
            (Some(a), Some(b))
        }
    };
    if let Some(date) = date {
        let mut p = date.split('-').map(|x| x.parse::<i32>().unwrap());
        d.year = p.next().unwrap();
        d.month = p.next().unwrap() as u8;
        d.day = p.next().unwrap() as u8;
    } else {
        (d.year, d.month, d.day) = (have.year, have.month, have.day);
    }
    if let Some(time) = time {
        let zone = if kind == "datetime" {
            time.find(['Z', '+', '-'])
        } else {
            None
        };
        let (clock, zone) = zone.map_or((time, None), |i| (&time[..i], Some(&time[i..])));
        let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
        let mut p = hms.split(':').map(|x| x.parse::<u8>().unwrap());
        d.hour = p.next().unwrap();
        d.minute = p.next().unwrap();
        d.second = p.next().unwrap();
        d.nanosecond = format!("{frac:0<9}").parse().unwrap();
        d.offset_seconds = zone.map(|z| {
            if z == "Z" {
                return 0;
            }
            let (h, m) = z[1..].split_once(':').unwrap();
            let secs = h.parse::<i32>().unwrap() * 3600 + m.parse::<i32>().unwrap() * 60;
            if z.starts_with('-') { -secs } else { secs }
        });
    }
    d.kind = match kind {
        "datetime" => DatetimeKind::OffsetDateTime,
        "datetime-local" => DatetimeKind::LocalDateTime,
        "date-local" => DatetimeKind::LocalDate,
        _ => DatetimeKind::LocalTime,
    };
    d
}

// ---- the case tree ----

fn root() -> String {
    let dir = std::env::var("GOSTD_GOTEST").expect(
        "GOSTD_GOTEST is not set: the test stage bind-mounts the testinputs stage at /gotest",
    );
    format!("{dir}/toml-test")
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap())
        .collect();
    names.sort_by_key(|e| e.file_name());
    for e in names {
        let name = e.file_name().into_string().expect("non-UTF-8 file name");
        let rel = format!("{rel}/{name}");
        if e.file_type().unwrap().is_dir() {
            walk(&e.path(), &rel, out);
        } else if let Some(case) = rel.strip_suffix(".toml") {
            out.push(case.to_string());
        }
    }
}

/// Every case path under `sub` ("valid" or "invalid"), such as `valid/array/array`, sorted.
fn cases(sub: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(&Path::new(&root()).join(sub), sub, &mut out);
    out
}

fn read(case: &str, ext: &str) -> Vec<u8> {
    let p = format!("{}/{case}.{ext}", root());
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

#[test]
#[ignore = "graphify-rs-aa6.11"]
fn test_toml() {
    let valid: Vec<String> = cases("valid")
        .into_iter()
        .filter(|c| !EXCLUDED_1_0.contains(&c.as_str()))
        .collect();
    let invalid = cases("invalid");
    assert_eq!(valid.len(), 180, "valid cases in {} (185 minus 5)", root());
    assert_eq!(invalid.len(), 371, "invalid cases in {}", root());

    let mut failures = Vec::new();
    for case in &valid {
        let want = parse_json(&read(case, "json"));
        match decode(&read(case, "toml")) {
            Err(e) => failures.push(format!("{case}: {e}")),
            Ok(t) => {
                if let Err(e) = cmp(&want, &Value::Table(t), "") {
                    failures.push(format!("{case}: {e}"));
                }
            }
        }
    }
    let mut checked = 0;
    for case in &invalid {
        if SKIP_INVALID.contains(&case.as_str()) {
            continue;
        }
        checked += 1;
        if decode(&read(case, "toml")).is_ok() {
            failures.push(format!(
                "{case}: expected an error, but no error was reported"
            ));
        }
    }
    assert_eq!(checked, 355, "invalid cases checked (371 minus 16 skipped)");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "graphify-rs-aa6.11"]
fn test_toml_next_fails() {
    // TOML 1.1 syntax must fail by default.
    for case in NEXT_FAILS {
        assert!(
            decode(&read(case, "toml")).is_err(),
            "{case}: expected an error"
        );
    }
}

#[test]
#[ignore = "graphify-rs-aa6.11"]
fn error_tests() {
    let invalid = cases("invalid");
    for (pat, want, n) in ERROR_TESTS {
        // `filepath.Match` with at most a trailing '*' (no '/' in the tail).
        let hits: Vec<&String> = invalid
            .iter()
            .filter(|c| {
                let path = c.strip_prefix("invalid/").unwrap();
                match pat.strip_suffix('*') {
                    Some(prefix) => path.starts_with(prefix) && !path[prefix.len()..].contains('/'),
                    None => path == pat,
                }
            })
            .collect();
        assert_eq!(hits.len(), n, "files matching {pat}");
        for case in hits {
            let err = decode(&read(case, "toml")).expect_err(case);
            let text = err.to_string();
            assert!(
                text.contains(want),
                "{case}\nwrong error message\nhave: {text}\nwant: {want}"
            );
        }
    }
}
