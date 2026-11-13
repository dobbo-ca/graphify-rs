//! Regression tests for graphify-rs-aa6.24: nested arrays and inline tables must not use one call
//! per level, to decode or to drop. Go decodes on a goroutine stack that grows to 1 GB; a Rust
//! worker thread has 2 MiB. Go's results and error texts for these shapes are checked by the
//! oracle inputs d0..d4 (parity/go/oracle/toml.go `tmDeep`).

use gostd::toml::{Value, decode};

/// The fuzz.yml crash (run 43459768460): `k = ` and 2,244 unclosed `[`.
const CRASH: &[u8] = include_bytes!(
    "../../../fuzz/seeds/toml_decode/crash-ae8219ba0a97c03688dc416776bfc711c6118b7d"
);

fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

/// The depth of single-element arrays under `v`, without recursion.
fn array_depth(mut v: &Value) -> usize {
    let mut n = 0;
    while let Value::Array(a) = v {
        n += 1;
        match a.first() {
            Some(e) => v = e,
            None => break,
        }
    }
    n
}

#[test]
fn fuzz_crash_on_2mib_stack() {
    assert!(on_small_stack(|| decode(CRASH).is_err()));
}

#[test]
fn deep_array_decodes_and_drops_on_2mib_stack() {
    const N: usize = 100_000;
    let depth = on_small_stack(|| {
        let src = format!("k = {}{}\n", "[".repeat(N), "]".repeat(N));
        let t = decode(src.as_bytes()).unwrap();
        array_depth(&t["k"])
        // `t` drops here, on the small stack.
    });
    assert_eq!(depth, N);
}

/// 3,000 levels: deep enough for one call per level to overflow 2 MiB, and small enough for the
/// O(depth^2) key strings that Go's `implicits` (and so ours) store per level.
#[test]
fn deep_unclosed_inline_tables_on_2mib_stack() {
    assert!(on_small_stack(|| decode(
        format!("k = {}", "{ a = ".repeat(3_000)).as_bytes()
    )
    .is_err()));
}

#[test]
fn deep_inline_tables_decode_on_2mib_stack() {
    const N: usize = 2_000;
    let depth = on_small_stack(|| {
        let src = format!("k = {}1{}\n", "{ a = ".repeat(N), " }".repeat(N));
        let t = decode(src.as_bytes()).unwrap();
        let mut v = &t["k"];
        let mut n = 0;
        while let Value::Table(t) = v {
            n += 1;
            v = &t["a"];
        }
        assert_eq!(*v, Value::Integer(1));
        n
    });
    assert_eq!(depth, N);
}

#[test]
fn deep_value_clones_on_2mib_stack() {
    const N: usize = 100_000;
    let (a, b) = on_small_stack(|| {
        let src = format!("k = {}{}\n", "[".repeat(N), "]".repeat(N));
        let t = decode(src.as_bytes()).unwrap();
        let c = t["k"].clone();
        (array_depth(&t["k"]), array_depth(&c))
    });
    assert_eq!((a, b), (N, N));
}

#[test]
fn clone_keeps_order_and_kinds() {
    let src = b"a = [1, [2.5, \"x\"], { y = true, z = [] }]\n[[t]]\nk = 1\n[[t]]\nk = 2\n";
    let t = decode(src).unwrap();
    for v in t.values() {
        assert_eq!(&v.clone(), v);
    }
}
