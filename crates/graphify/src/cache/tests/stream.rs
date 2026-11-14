//! Port of `stream_test.go` (4 tests). Go compares `writeCache` with `json.Marshal` of the whole
//! document; byte parity with Go is a non-goal, so the Rust tests check the layout `loadSplit`
//! depends on, the key order, and the round trip through a whole decode. A Rust `BTreeMap` has
//! no nil form, so Go's `nil` case is dropped, and a Go string with a bad byte (`b"b\xff"`,
//! `"bad\xff.c"`) is its R3 form with U+FFFD.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};

use super::{WORKERS, entry};
use crate::cache::cache::{CacheFile, load_split, write_cache};
use crate::cache::{Cache, Entry, FILE_NAME, load, save};
use crate::extract::{Call, Result as ExtractResult};
use crate::model::{Edge, Node, Relation, append_json_string};
use crate::testutil::TempDir;
use gostd::json::{indent, unmarshal};

/// The `files` of one decode of the whole document (Go: the `whole` closure).
fn whole(data: &[u8]) -> Cache {
    unmarshal::<CacheFile>(data).expect("whole decode").files
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// Go `iotest.OneByteReader`.
struct OneByteReader<R>(R);

impl<R: Read> Read for OneByteReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(1);
        self.0.read(&mut buf[..n])
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// A key as the writer encodes it, plus the start of its entry.
fn key_needle(k: &str) -> Vec<u8> {
    let mut b = Vec::new();
    append_json_string(&mut b, k);
    b.extend_from_slice(br#":{"hash":""#);
    b
}

/// Go `TestWriteCacheMatchesMarshal`: `write_cache` writes the stamped document `loadSplit`
/// reads: head, entries in sorted key order, the same bytes when draining, and what a whole
/// decode reads back is what was written.
#[test]
fn test_write_cache_matches_marshal() {
    let entry_of = |i: usize| Entry {
        hash: format!("h{i}"),
        result: ExtractResult {
            nodes: vec![Node {
                id: format!("n{i}"),
                label: "<é&>\u{2028}".to_string(),
                attributes: BTreeMap::from([
                    ("b".to_string(), "2".to_string()),
                    ("a".to_string(), "1".to_string()),
                ]),
                ..Node::default()
            }],
            edges: vec![Edge {
                source: "a".to_string(),
                target: "b\u{FFFD}".to_string(),
                relation: Relation::CALLS,
                weight: 0.5,
                ..Edge::default()
            }],
            calls: vec![Call {
                callee: 1,
                recv: 2,
                ..Call::default()
            }],
            call_text: strs(&["", "f", "日本"]),
            ..ExtractResult::default()
        },
    };
    // Keys whose byte order differs from UTF-16 order, plus ones the encoder escapes.
    let keys = [
        "a.go",
        "A.go",
        "dir/b.go",
        "",
        "<x>&.js",
        "q\"uote\\.py",
        "é.md",
        "\u{FFFF}.ts",
        "\u{1F600}.ts",
        "\u{E000}.ts",
        "bad\u{FFFD}.c",
        "tab\t.h",
        "a",
        "a/a",
        "a.a",
    ];
    let mut cases: Vec<(&str, Cache)> = vec![
        ("empty", Cache::new()),
        ("one", Cache::from([("only.go".to_string(), entry_of(0))])),
        (
            "zero-entry",
            Cache::from([("z.go".to_string(), Entry::default())]),
        ),
    ];
    let many: Cache = keys
        .iter()
        .enumerate()
        .map(|(i, k)| ((*k).to_string(), entry_of(i)))
        .collect();
    cases.push(("many", many));
    for (name, c) in &cases {
        for stamp in ["v0.10.1-s12", "", "<dev>\"x"] {
            let mut got = Vec::new();
            write_cache(&mut got, stamp, &mut c.clone(), false).unwrap();

            // The head: `{"v":<stamp>,"files":{`, then the entries, then `}}`.
            let mut head = br#"{"v":"#.to_vec();
            append_json_string(&mut head, stamp);
            head.extend_from_slice(br#","files":{"#);
            assert!(
                got.starts_with(&head),
                "{name}, stamp {stamp:?}: head differs\n got: {}",
                got.escape_ascii()
            );
            assert!(
                got.ends_with(b"}}"),
                "{name}, stamp {stamp:?}: tail differs"
            );

            // Sorted key order: every entry starts after the one before it.
            let mut at = head.len();
            for k in c.keys() {
                let Some(i) = find(&got[at..], &key_needle(k)) else {
                    panic!("{name}, stamp {stamp:?}: key {k:?} is missing or out of order");
                };
                at += i + 1;
            }

            // What a whole decode reads back.
            let back = unmarshal::<CacheFile>(&got).expect("whole decode");
            assert!(
                back.v == stamp && back.files == *c,
                "{name}, stamp {stamp:?}: decoded cache differs"
            );

            // Draining writes the same bytes and empties the map.
            let mut d = c.clone();
            let mut drained = Vec::new();
            write_cache(&mut drained, stamp, &mut d, true).unwrap();
            assert!(
                drained == got && d.is_empty(),
                "{name}, stamp {stamp:?}: drained cache differs or {} entries left",
                d.len()
            );
        }
    }
}

struct FailWriter {
    n: i64,
}

impl Write for FailWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.n -= 1;
        if self.n < 0 {
            return Err(io::Error::other("disk full"));
        }
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Go `TestWriteCacheWriteError`: a write error part way through is returned: head, 50
/// entries, tail.
#[test]
fn test_write_cache_write_error() {
    let c: Cache = (0..50).map(|i| (format!("f{i}.go"), entry("h"))).collect();
    for n in [0, 1, 7, 51] {
        assert!(
            write_cache(&mut FailWriter { n }, "s", &mut c.clone(), true).is_err(),
            "writer failing after {n} writes: no error"
        );
    }
}

/// Go `TestLoadSplitMatchesWholeDecode`: load in pieces must return what one decode of the
/// whole file returns.
#[test]
fn test_load_split_matches_whole_decode() {
    let mut c = Cache::new();
    for i in 0..20000 {
        // Text that looks like an entry boundary, inside a key and a value.
        let k = format!("d{}/{}{}.go", i % 7, r#"}},"k":{"hash":""#, i);
        c.insert(
            k.clone(),
            Entry {
                hash: format!("h{i}"),
                result: ExtractResult {
                    nodes: vec![Node {
                        id: k,
                        label: r#"x}},"y":{"hash":"z"#.to_string(),
                        attributes: BTreeMap::from([("a".to_string(), "}}".to_string())]),
                        ..Node::default()
                    }],
                    calls: vec![Call {
                        callee: 1,
                        recv: 2,
                        ..Call::default()
                    }],
                    call_text: strs(&["", "f\\", "日本"]),
                    ..ExtractResult::default()
                },
            },
        );
    }
    // Entries longer than a piece, alone and side by side.
    for k in ["big0.go", "d3/big1.go", "d3/big2.go"] {
        c.insert(
            k.to_string(),
            Entry {
                hash: "h".to_string(),
                result: ExtractResult {
                    nodes: vec![Node {
                        id: k.to_string(),
                        label: r#"}},"y":{"hash":"z"#.repeat(60000),
                        ..Node::default()
                    }],
                    ..ExtractResult::default()
                },
            },
        );
    }
    let mut buf = Vec::new();
    write_cache(&mut buf, "s1", &mut c.clone(), false).unwrap();
    let Some(got) = load_split(buf.as_slice(), "s1", WORKERS) else {
        panic!("loadSplit gave up on a file writeCache wrote");
    };
    assert!(
        got == whole(&buf) && got.len() == c.len(),
        "split decode differs from the whole decode"
    );
    assert!(
        load_split(buf.as_slice(), "s2", WORKERS).is_none(),
        "loadSplit accepted another stamp"
    );

    // A file cut short, with a byte too many or a wrong last byte is not decoded in pieces.
    let mut one_more = buf.clone();
    one_more.push(b'}');
    let mut wrong_last = buf[..buf.len() - 1].to_vec();
    wrong_last.push(b'x');
    for b in [
        buf[..buf.len() - 1].to_vec(),
        buf[..buf.len() / 2].to_vec(),
        one_more,
        wrong_last,
    ] {
        assert!(
            load_split(b.as_slice(), "s1", WORKERS).is_none(),
            "loadSplit accepted a damaged file of {} bytes",
            b.len()
        );
    }
    for e in [
        Cache::new(),
        Cache::from([("a.go".to_string(), entry("h"))]),
    ] {
        buf.clear();
        write_cache(&mut buf, "s1", &mut e.clone(), false).unwrap();
        let got = load_split(OneByteReader(buf.as_slice()), "s1", WORKERS);
        assert!(
            got.as_ref().is_some_and(|g| *g == whole(&buf)),
            "split decode of {} entries differs from the whole decode",
            e.len()
        );
    }

    // A key written twice, pieces apart: the later entry wins, as in a whole decode.
    let mut dup = String::from(r#"{"v":"s1","files":{"a":{"hash":"1","result":{}}"#);
    for i in 0..20000 {
        dup.push_str(&format!(r#","f{i}":{{"hash":"h{i}","result":{{}}}}"#));
    }
    dup.push_str(r#","a":{"hash":"2","result":{}}}}"#);
    let got = load_split(dup.as_bytes(), "s1", WORKERS).unwrap_or_default();
    assert!(
        got.get("a").is_some_and(|e| e.hash == "2") && got == whole(dup.as_bytes()),
        "split decode of a repeated key differs from the whole decode"
    );

    // A hand-made file whose every likely cut is inside an entry: the pieces fail, and Load
    // falls back to the whole decode.
    let mut sb = String::from(r#"{"v":"s1","files":{"#);
    for i in 0..20000 {
        if i > 0 {
            sb.push(',');
        }
        sb.push_str(&format!(
            r#""f{i}":{{"hash":"h{i}","extra":{{"a":{{"b":{{}}}},"c":{{"hash":"x"}}}},"result":{{"GoMods":{{"m":"{i}"}}}}}}"#
        ));
    }
    sb.push_str("}}");
    let bad = sb.as_bytes();
    assert!(
        load_split(bad, "s1", WORKERS).is_none(),
        "loadSplit decoded pieces cut inside an entry"
    );
    let dir = TempDir::new();
    let path = dir.write(FILE_NAME, bad);
    let got = load(&path, "s1", WORKERS);
    assert!(
        got.len() == 20000 && got == whole(bad),
        "Load after a failed split differs from the whole decode"
    );
}

/// Go `TestLoadDropsBadCalls`: an entry whose calls number a text it does not hold is not
/// loaded, by either decoder, so the file is parsed again.
#[test]
fn test_load_drops_bad_calls() {
    let with_callee = |callee: u32| Entry {
        hash: "h".to_string(),
        result: ExtractResult {
            calls: vec![Call {
                callee,
                ..Call::default()
            }],
            call_text: strs(&["", "f"]),
            ..ExtractResult::default()
        },
    };
    let c = Cache::from([
        ("ok.go".to_string(), with_callee(1)),
        ("bad.go".to_string(), with_callee(2)),
    ]);
    let dir = TempDir::new();
    let path = dir.join(FILE_NAME);
    save(&path, "s", &c).unwrap();
    let got = load(&path, "s", WORKERS);
    assert!(
        got.len() == 1 && got.get("ok.go").is_some_and(|e| e.hash == "h"),
        "split load = {got:?}, want ok.go only"
    );
    // Not in writeCache's layout: the whole-file decoder.
    let mut data = Vec::new();
    indent(&mut data, &fs::read(&path).unwrap(), "", " ").unwrap();
    fs::write(&path, data).unwrap();
    let got = load(&path, "s", WORKERS);
    assert!(
        got.len() == 1 && got.get("ok.go").is_some_and(|e| e.hash == "h"),
        "whole load = {got:?}, want ok.go only"
    );
}
