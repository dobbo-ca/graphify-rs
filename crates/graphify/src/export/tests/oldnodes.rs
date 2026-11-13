//! Port of `oldnodes_test.go` (3 tests and `FuzzScanOldNodes`, whose seeds are the table test
//! `scan_old_nodes_seeds`). No upstream file tests the streaming scan.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};

use gostd::json::{Error as JsonError, unmarshal};

use super::to_json_at;
use crate::export::oldnodes::{OldNode, scan_old_nodes};
use crate::model::{Graph, Node};
use crate::testutil::TempDir;

/// Go `existing struct { Nodes []oldNode }`.
#[derive(Default)]
struct Existing {
    nodes: Vec<OldNode>,
}

gostd::json_fields!(decode Existing, "struct { Nodes []struct { ID string \"json:\\\"id\\\"\"; SourceFile string \"json:\\\"source_file\\\"\" } \"json:\\\"nodes\\\"\" }", {
    nodes => "nodes",
});

/// Go `wholeOldNodes`: the decode scanOldNodes stands in for.
fn whole_old_nodes(doc: &[u8]) -> Result<Vec<OldNode>, JsonError> {
    unmarshal::<Existing>(doc).map(|e| e.nodes)
}

/// Go `iotest.OneByteReader`: reads one byte at a time.
struct OneByteReader<R>(R);

impl<R: Read> Read for OneByteReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.0.read(&mut buf[..1])
    }
}

/// Go `checkScan`: fails when scanOldNodes accepts doc but the whole decode disagrees. It
/// reports whether the scan accepted.
fn check_scan(doc: &[u8]) -> bool {
    let got = scan_old_nodes(doc);
    let one = scan_old_nodes(OneByteReader(doc));
    assert!(
        got == one,
        "scan depends on how the input is cut into reads: {doc:?}"
    );
    let Some(got) = got else {
        return false;
    };
    let want = whole_old_nodes(doc)
        .unwrap_or_else(|e| panic!("scan accepted what the decoder refuses ({e}): {doc:?}"));
    assert!(got == want, "scan got {got:?}, decoder {want:?}: {doc:?}");
    true
}

/// Go `scanSeeds`, but its last seed (a 1001-deep array) is made by `scan_seeds`.
const SCAN_SEEDS: &[&[u8]] = &[
    br#"{"nodes":[{"id":"a","source_file":"a.go"},{"id":"b"}],"links":[]}"#,
    b"{\n  \"directed\": false,\n  \"graph\": {\"n\": 1, \"m\": {\".go\": 2}},\n  \"nodes\": [\n    {\n      \"id\": \"a\",\n      \"label\": \"A\",\n      \"source_file\": \"p/a.go\",\n      \"community\": 3\n    }\n  ],\n  \"links\": [{\"weight\": 1.5e-3, \"x\": [true, false, null, -0, 12.5E+2]}],\n  \"hyperedges\": []\n}\n",
    br#"{"nodes":[]}"#,
    br#"{}"#,
    br#"{"links":[1,2]}"#,
    r#" { "nodes" : [ null , {} , {"id":"é\n\"x","source_file":"\\"} ] } "#.as_bytes(),
    br#"{"nodes":[{"id":"a"}],"nodes":[{"id":"b"}]}"#,
    br#"{"nodes":[{"id":"a","source_file":"s"},{"id":"c"}],"nodes":[{"id":"b"}]}"#,
    br#"{"Nodes":[{"id":"a"}]}"#,
    br#"{"nodes":[{"id":"a"}],"NODES":[]}"#,
    br#"{"nodes":null}"#,
    br#"{"nodes":{}}"#,
    br#"{"nodes":[{"id":1}]}"#,
    br#"{"nodes":[1]}"#,
    br#"{"nodes":[{"ID":"a","Source_File":"b"}]}"#,
    br#"{"nodes":[{"id":"a","id":"b"}]}"#,
    br#"{"nodes":[{"id":"a"}]}"#,
    br#"{"nodes":[{"id":"a"},]}"#,
    br#"{"nodes":[{"id":"a"}],}"#,
    br#"{"nodes":[{"id":"a"}]} x"#,
    br#"{"nodes":[{"id":"a"}]"#,
    br#"{"nodes":[{"id":"a"}]}{}"#,
    br#"[]"#,
    br#"null"#,
    b"",
    b"  ",
    br#"{"a":01}"#,
    br#"{"a":1.}"#,
    br#"{"a":-}"#,
    br#"{"a":1e}"#,
    br#"{"a":"\x"}"#,
    br#"{"a":"\u12g4"}"#,
    b"{\"a\":\"\x01\"}",
    b"{\"a\":\"\xff\"}",
    b"{\"nodes\":[{\"id\":\"\xff\"}]}",
    br#"{"a":tru}"#,
    br#"{"a":nul}"#,
    br#"{"a" 1}"#,
    br#"{"a":1 "b":2}"#,
    br#"{a:1}"#,
    br#"{"a":[1 2]}"#,
    br#"{"a":{"b":}}"#,
    br#"{"a":[[[[{"b":[{}]}]]]]}"#,
    b"{\"a\":1}\x00",
    b"{\"a\":1\x00}",
    r#"{"nodes":[{"id":"a","source_file":"s"}], "a":1e5, "b":-0.0, "c":0, "d":"😀\ud800"}"#
        .as_bytes(),
];

fn scan_seeds() -> Vec<Vec<u8>> {
    let mut seeds: Vec<Vec<u8>> = SCAN_SEEDS.iter().map(|s| s.to_vec()).collect();
    seeds.push(format!(r#"{{"a":{}{}}}"#, "[".repeat(1001), "]".repeat(1001)).into_bytes());
    seeds
}

// Go's FuzzScanOldNodes seeds, run as a table test: the scan must give the same answer however
// the input is cut into reads, and what it accepts must decode to the same nodes. The
// cargo-fuzz target comes with the code PR.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn scan_old_nodes_seeds() {
    let seeds = scan_seeds();
    assert_eq!(seeds.len(), 47, "Go has 47 seeds");
    let mut accepted = 0;
    for doc in &seeds {
        if check_scan(doc) {
            accepted += 1;
        }
    }
    // Seeds 1, 2, 3, 4, 5 and 6 are plain documents the scan takes.
    assert!(accepted >= 6, "the scan accepted only {accepted} seeds");
}

/// A deterministic generator for the mutations (SplitMix64). Go uses `math/rand` seed 1; the
/// mutants differ, the check is the same.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Go `rng.Intn(n)`.
    fn intn(&mut self, n: usize) -> usize {
        (self.next_u64() >> 33) as usize % n
    }
}

// Byte edits of valid documents: the scan must refuse or agree, never differ.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_scan_old_nodes_mutations() {
    let mut rng = Rng(1);
    const ALPHABET: &[u8] = b"{}[]\",:\\ \n0123456789-+.eEtrufalsn\x00\x1f\xffu";
    let mut accepted = 0;
    for seed in &scan_seeds()[..6] {
        for _ in 0..20000 {
            let mut doc = seed.clone();
            for _ in 0..1 + rng.intn(2) {
                if doc.is_empty() {
                    break;
                }
                let i = rng.intn(doc.len());
                match rng.intn(4) {
                    0 => doc[i] = ALPHABET[rng.intn(ALPHABET.len())],
                    1 => {
                        doc.remove(i);
                    }
                    2 => doc.insert(i, ALPHABET[rng.intn(ALPHABET.len())]),
                    _ => doc.truncate(i),
                }
            }
            if check_scan(&doc) {
                accepted += 1;
            }
        }
    }
    assert!(
        accepted >= 1000,
        "only {accepted} mutants accepted: the test checks nothing"
    );
}

// Nodes past one decode batch, the later ones without a source: a batch
// must not inherit fields from the one before.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_scan_old_nodes_batches() {
    let mut b = String::from(r#"{"nodes":["#);
    const N: usize = 20000;
    for i in 0..N {
        if i > 0 {
            b.push_str(", ");
        }
        if i < N / 2 {
            write!(
                b,
                r#"{{"id":"n{i}","source_file":"dir/file{i}.go","pad":[{i},{{"x":"y"}}]}}"#
            )
            .expect("write to String");
        } else {
            write!(b, r#"{{"id":"n{i}"}}"#).expect("write to String");
        }
    }
    b.push_str(r#"],"links":[]}"#);
    assert!(
        b.len() >= 3 * 256 * 1024 / 2,
        "document of {} bytes does not span the batches",
        b.len()
    );
    let doc = b.as_bytes();
    let got = scan_old_nodes(doc);
    let want = whole_old_nodes(doc).unwrap_or_default();
    let got_len = got.as_ref().map_or(0, Vec::len);
    assert!(
        got.as_ref() == Some(&want) && got_len == N,
        "ok={}, {got_len} nodes, equal={}",
        got.is_some(),
        got.as_ref() == Some(&want)
    );
}

// The scan must take what ToJSON writes, or the whole decode it replaces
// would quietly come back.
#[test]
#[ignore = "graphify-rs-ihc.12"]
fn test_scan_old_nodes_takes_to_json() {
    let dir = TempDir::new();
    let path = dir.join("graph.json");
    let mut g = Graph::new();
    g.add_node(Node {
        id: "a".to_string(),
        label: "a".to_string(),
        file_type: "code".to_string(),
        source_file: "pkg/a.go".to_string(),
        ..Node::default()
    });
    g.add_node(Node {
        id: "b \"q\" \u{e9}<".to_string(),
        label: "b".to_string(),
        file_type: "code".to_string(),
        source_file: "pkg/b.go".to_string(),
        ..Node::default()
    });
    let communities = BTreeMap::from([(0, vec!["a".to_string()])]);
    to_json_at(&g, &communities, &path, "abc", true).expect("ToJSON");
    let doc = fs::read(&path).expect("read");
    assert!(
        check_scan(&doc),
        "scan refused ToJSON output: {}",
        String::from_utf8(doc.clone()).expect("graph.json is UTF-8")
    );
    let got = scan_old_nodes(doc.as_slice()).unwrap_or_default();
    assert_eq!(got.len(), 2, "got {} nodes, want 2", got.len());
}
