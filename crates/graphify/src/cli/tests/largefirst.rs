//! Port of `largefirst_test.go` (2 tests). Go swaps the `parseFile` var; [`assemble_with`] takes
//! it as a parameter. Go's `runtime.NumCPU()` workers are a parameter too (R5): these pass 8.

use std::sync::Mutex;
use std::time::Duration;

use super::{BIG_FILE, BIG_IN_FLIGHT, assemble, assemble_with, large_first};
use crate::cache::StatIndex;
use crate::extract::Result as ExtractResult;
use crate::testutil::TempDir;

const WORKERS: usize = 8;

// The large-file lane changes the parse order only: results come back in file order with the
// same content.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_assemble_large_first() {
    let root = TempDir::new();
    let body = |n: usize| format!("package p\n\nfunc F() {{}}\n{}", "// pad\n".repeat(n / 7));
    let files: Vec<String> = ["a.go", "big1.go", "b.go", "big2.go", "gone.go", "c.go"]
        .map(String::from)
        .to_vec();
    let sizes = [10, BIG_FILE, 20, BIG_FILE + 4096, 0, 30];
    for (f, n) in files.iter().zip(sizes) {
        if f != "gone.go" {
            root.write(f, body(n).as_bytes());
        }
    }
    assert_eq!(large_first(root.path(), &files), [3, 1], "large_first");
    let (results, new_cache, _, stats) =
        assemble(root.path(), &files, None, &StatIndex::new(), WORKERS);
    assert!(
        stats.parsed == 5 && new_cache.len() == 5,
        "parsed {}, cache {}, want 5 and 5",
        stats.parsed,
        new_cache.len()
    );
    for (i, f) in ["a.go", "big1.go", "b.go", "big2.go", "c.go"]
        .iter()
        .enumerate()
    {
        assert!(
            results[i] == new_cache[*f].result,
            "result {i} is not the result of {f}"
        );
    }
    assert!(
        results[0] != results[1],
        "results of different files are equal"
    );
}

// Any two of these files are over BIG_IN_FLIGHT, so they must parse one at a time however many
// workers reach the large-file lane.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_assemble_large_files_in_flight() {
    let root = TempDir::new();
    let files: Vec<String> = ["big1.go", "big2.go", "big3.go"].map(String::from).to_vec();
    for f in &files {
        root.write(f, "\0".repeat(BIG_IN_FLIGHT / 2 + 1).as_bytes());
    }
    // (held, peak)
    let state = Mutex::new((0usize, 0usize));
    let parse = |_: &str, src: &[u8]| -> ExtractResult {
        {
            let mut s = state.lock().unwrap();
            s.0 += src.len();
            s.1 = s.1.max(s.0);
        }
        std::thread::sleep(Duration::from_millis(50));
        state.lock().unwrap().0 -= src.len();
        ExtractResult::default()
    };
    let (_, _, _, stats) = assemble_with(
        root.path(),
        &files,
        None,
        &StatIndex::new(),
        WORKERS,
        &parse,
    );
    assert_eq!(stats.parsed, 3, "parsed");
    let peak = state.lock().unwrap().1;
    assert!(
        peak <= BIG_IN_FLIGHT,
        "{peak} bytes of large files in flight, limit {BIG_IN_FLIGHT}"
    );
}
