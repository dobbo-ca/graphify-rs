//! Regression tests for graphify-rs-aa6.23: the pike VM's epsilon closure (Go `machine.add`,
//! exec.go:317) must not use the call stack. Go recurses there on a goroutine stack that grows to
//! 1 GB; a Rust worker thread has 2 MiB.

use gostd::regexp::Regexp;

/// The fuzz.yml crash (run 43459768460): `( ... ){704}` over a ~100-way alternation with empty
/// branches, so the closure walks all 704 copies. Input format: pattern, `\n`, subject.
const CRASH: &[u8] = include_bytes!(
    "../../../fuzz/seeds/regexp_compile_match/crash-0507ddb0673efe9b953c3247eb962cf7602aa218"
);

fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn fuzz_crash_closure_on_2mib_stack() {
    let nl = CRASH.iter().position(|&b| b == b'\n').unwrap();
    let pat = std::str::from_utf8(&CRASH[..nl]).unwrap().to_string();
    let rest = CRASH[nl + 1..].to_vec();
    // The test is that this returns: before the fix it overflowed the stack.
    on_small_stack(move || {
        let re = Regexp::compile(&pat).unwrap();
        re.find_all_submatch_index(&rest, -1)
    });
}

/// 1000 copies (Go's repeat limit) of a group that can match empty: a closure about 4,000
/// instructions deep. The only match starts at 0, where every copy takes an `a`; group 1 is the
/// last copy's `a`.
#[test]
fn empty_alternation_repeat_on_2mib_stack() {
    let got = on_small_stack(|| {
        let re = Regexp::compile("(?:(|a)){1000}b").unwrap();
        let s = "a".repeat(1000) + "b";
        re.find_all_submatch_index(s.as_bytes(), -1)
    });
    assert_eq!(got, vec![vec![Some((0, 1001)), Some((999, 1000))]]);
}
