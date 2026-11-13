//! Ported from sort/sort_test.go (GOSRC, Go 1.27.4, BSD-3-Clause). 21 Test funcs: 15 ported,
//! 6 not ported. sort.Interface types are driven through `gostd::sort::slice` with an equivalent
//! `less` closure (Go's `Less(i, j)` indexes the data, ours gets the two elements).
//! - TestSortIntSlice, TestSortStringSlice, TestInts, TestStrings, TestSlice: ported through `slice`
//! - TestSortLarge_Random: ported; Go uses n = 1,000,000, or 10,000 under testing.Short(); this
//!   port uses the short size 10,000
//! - TestBreakPatterns, TestReverseRange, TestNonDeterministicComparison, TestAdversary: ported
//! - TestSortBM: ported through `slice` with Go's sizes, tables and modes; the Swap budget
//!   (`maxswap`) is not ported: `slice` has no Swap to count
//! - TestCountSortOps: ported as a check that `slice` sorts; Go only logs Swap and Less counts
//!   (and skips unless verbose). Sizes are Go's testing.Short() sizes `countOpsSizes[:5]`
//! - TestStableInts, TestStability, TestCountStableOps: ported against std `sort_by`, which is the
//!   stable sort graphify-rs uses (this proves the std choice). TestCountStableOps checks only
//!   that it sorts, not Go's call count
//! - TestSortFloat64Slice: not ported: floats with NaN, graphify-go sorts no floats
//! - TestSortFloat64sCompareSlicesSort: not ported: floats with NaN, graphify-go sorts no floats
//! - TestFloat64s: not ported: sort.Float64s with NaN, graphify-go sorts no floats
//! - TestReverseSortIntSlice: not ported: sort.Reverse wraps a sort.Interface, graphify-go does
//!   not call it
//! - TestHeapsortBM: not ported: calls Heapsort (export_test.go) directly; graphify-go reaches
//!   heapSort only through pdqsort, which TestAdversary exercises
//! - TestStableBM: not ported: bounds the Swap count of sort.Stable's symMerge, which gostd does
//!   not port (std `sort_by`)
//!
//! Random data uses a local splitmix64 generator (gostd has no third-party crates); Go uses
//! math/rand/v2, and PCG(0, 0) in TestNonDeterministicComparison, whose exact stream does not
//! matter to these checks.

use gostd::sort;
use std::cell::Cell;

const INTS: [i64; 13] = [
    74, 59, 238, -784, 9845, 959, 905, 0, 0, 42, 7586, -5467984, 7586,
];
const STRINGS_DATA: [&str; 8] = ["", "Hello", "foo", "bar", "foo", "f00", "%*&^*&^&", "***"];

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn int_n(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn float32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }
}

fn lg(n: usize) -> usize {
    let mut i = 0;
    while 1usize << i < n {
        i += 1;
    }
    i
}

// sort_test.go:24
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_int_slice() {
    let mut data = INTS;
    sort::slice(&mut data, |a, b| a < b);
    assert!(data.is_sorted(), "sorted {INTS:?}\n   got {data:?}");
}

// sort_test.go:58
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_string_slice() {
    let mut data = STRINGS_DATA;
    sort::slice(&mut data, |a, b| a < b);
    assert!(data.is_sorted(), "sorted {STRINGS_DATA:?}\n   got {data:?}");
}

// sort_test.go:68
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_ints() {
    let mut data = INTS;
    sort::slice(&mut data, |a, b| a < b);
    assert!(data.is_sorted(), "sorted {INTS:?}\n   got {data:?}");
}

// sort_test.go:86
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_strings() {
    let mut data = STRINGS_DATA;
    sort::slice(&mut data, |a, b| a < b);
    assert!(data.is_sorted(), "sorted {STRINGS_DATA:?}\n   got {data:?}");
}

// sort_test.go:95
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_slice() {
    let mut data = STRINGS_DATA;
    sort::slice(&mut data, |a, b| a < b);
    assert!(data.is_sorted(), "sorted {STRINGS_DATA:?}\n   got {data:?}");
}

// sort_test.go:106 (testing.Short() size)
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_large_random() {
    let n = 1_000_000 / 100;
    let mut rng = Rng(1);
    let mut data: Vec<i64> = (0..n).map(|_| rng.int_n(100) as i64).collect();
    assert!(!data.is_sorted(), "terrible rand.rand");
    sort::slice(&mut data, |a, b| a < b);
    assert!(data.is_sorted(), "sort didn't sort - 1M ints");
}

// sort_test.go:141
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_break_patterns() {
    // Special slice used to trigger breakPatterns.
    let mut data = [10i64; 30];
    let len = data.len();
    data[len / 4] = 0;
    data[(len / 4) * 2] = 1;
    data[(len / 4) * 3] = 2;
    sort::slice(&mut data, |a, b| a < b);
    // Go only checks that Sort returns.
    assert!(data.is_sorted());
}

// sort_test.go:153
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_reverse_range() {
    let mut data: [i64; 7] = [1, 2, 3, 4, 5, 6, 7];
    let len = data.len();
    sort::reverse_range(&mut data, 0, len);
    for i in (1..len).rev() {
        assert!(data[i] <= data[i - 1], "reverseRange didn't work");
    }

    let mut data1: [i64; 7] = [1, 2, 3, 4, 5, 6, 7];
    let data2: [i64; 7] = [1, 2, 5, 4, 3, 6, 7];
    sort::reverse_range(&mut data1, 2, 5);
    for (i, v) in data1.iter().enumerate() {
        assert_eq!(*v, data2[i], "reverseRange didn't work");
    }
}

// sort_test.go:191. Go's Less/Swap panic on out-of-range indexes; `slice` hands out elements,
// so this checks that sorting with a random `less` returns without panicking.
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_non_deterministic_comparison() {
    let mut rng = Rng(0);
    for _ in 0..10 {
        let mut data: Vec<usize> = (0..500).collect();
        sort::slice(&mut data, |_, _| rng.float32() < 0.5);
    }
}

// sort_test.go:522, testBentleyMcIlroy at sort_test.go:426 (Go's non-short sizes)
#[test]
#[allow(clippy::needless_range_loop, clippy::manual_memcpy)]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_bm() {
    const SAWTOOTH: usize = 0;
    const RAND: usize = 1;
    const STAGGER: usize = 2;
    const PLATEAU: usize = 3;
    const SHUFFLE: usize = 4;
    const N_DIST: usize = 5;

    const COPY: usize = 0;
    const REVERSE: usize = 1;
    const REVERSE_FIRST_HALF: usize = 2;
    const REVERSE_SECOND_HALF: usize = 3;
    const SORTED: usize = 4;
    const DITHER: usize = 5;
    const N_MODE: usize = 6;

    let sizes = [100usize, 1023, 1024, 1025];
    let dists = ["sawtooth", "rand", "stagger", "plateau", "shuffle"];
    let modes = ["copy", "reverse", "reverse1", "reverse2", "sort", "dither"];
    let mut rng = Rng(2);
    for &n in &sizes {
        let mut m = 1;
        while m < 2 * n {
            for dist in 0..N_DIST {
                let mut j = 0;
                let mut k = 1;
                let mut data = vec![0usize; n];
                for i in 0..n {
                    data[i] = match dist {
                        SAWTOOTH => i % m,
                        RAND => rng.int_n(m),
                        STAGGER => (i * m + i) % n,
                        PLATEAU => i.min(m),
                        SHUFFLE => {
                            if rng.int_n(m) != 0 {
                                j += 2;
                                j
                            } else {
                                k += 2;
                                k
                            }
                        }
                        _ => unreachable!(),
                    };
                }

                let mut mdata = vec![0usize; n];
                for mode in 0..N_MODE {
                    match mode {
                        COPY => {
                            for i in 0..n {
                                mdata[i] = data[i];
                            }
                        }
                        REVERSE => {
                            for i in 0..n {
                                mdata[i] = data[n - i - 1];
                            }
                        }
                        REVERSE_FIRST_HALF => {
                            for i in 0..n / 2 {
                                mdata[i] = data[n / 2 - i - 1];
                            }
                            for i in n / 2..n {
                                mdata[i] = data[i];
                            }
                        }
                        REVERSE_SECOND_HALF => {
                            for i in 0..n / 2 {
                                mdata[i] = data[i];
                            }
                            for i in n / 2..n {
                                mdata[i] = data[n - (i - n / 2) - 1];
                            }
                        }
                        SORTED => {
                            for i in 0..n {
                                mdata[i] = data[i];
                            }
                            // Ints is std sort_unstable here (plan decision).
                            mdata.sort_unstable();
                        }
                        DITHER => {
                            for i in 0..n {
                                mdata[i] = data[i] + i % 5;
                            }
                        }
                        _ => unreachable!(),
                    }

                    let desc = format!("n={n} m={m} dist={} mode={}", dists[dist], modes[mode]);
                    sort::slice(&mut mdata, |a, b| a < b);
                    assert!(mdata.is_sorted(), "{desc}: ints not sorted\n\t{mdata:?}");
                }
            }
            m *= 2;
        }
    }
}

// sort_test.go:588. Go's adversary reads and writes `data[i]` by position. Elements here are
// `Cell`s so `less` can freeze them, and the position is recovered from the element's address
// (the sort must pass references into the slice, as Go's index-based Less does).
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_adversary() {
    const SIZE: usize = 10000; // large enough to distinguish between O(n^2) and O(n*log(n))
    let maxcmp = SIZE * lg(SIZE) * 4; // the factor 4 was found by trial and error
    let gas = SIZE - 1; // special value for unset elements, higher than everything else
    let mut data: Vec<Cell<usize>> = (0..SIZE).map(|_| Cell::new(gas)).collect();
    let base = data.as_ptr().addr();
    let ncmp = Cell::new(0usize); // number of comparisons (calls to Less)
    let nsolid = Cell::new(0usize); // elements set to non-gas values
    let candidate = Cell::new(0usize); // guess at current pivot
    sort::slice(&mut data, |a: &Cell<usize>, b: &Cell<usize>| {
        let i = (std::ptr::from_ref(a).addr() - base) / size_of::<Cell<usize>>();
        let j = (std::ptr::from_ref(b).addr() - base) / size_of::<Cell<usize>>();
        assert!(
            ncmp.get() < maxcmp,
            "used {} comparisons sorting adversary data with size {SIZE}",
            ncmp.get()
        );
        ncmp.set(ncmp.get() + 1);

        if a.get() == gas && b.get() == gas {
            if i == candidate.get() {
                // freeze i
                a.set(nsolid.get());
                nsolid.set(nsolid.get() + 1);
            } else {
                // freeze j
                b.set(nsolid.get());
                nsolid.set(nsolid.get() + 1);
            }
        }

        if a.get() == gas {
            candidate.set(i);
        } else if b.get() == gas {
            candidate.set(j);
        }

        a.get() < b.get()
    });
    // Check data is fully populated and sorted.
    for (i, v) in data.iter().enumerate() {
        assert_eq!(v.get(), i, "adversary data not fully sorted");
    }
}

const COUNT_OPS_SIZES: [usize; 9] = [100, 300, 1000, 3000, 10000, 30000, 100000, 300000, 1000000];

// countOps at sort_test.go:691 (testing.Short() sizes): sorts random data with `n / 5` values
// and prints the Less count; Go only logs.
fn count_ops(name: &str, algo: impl Fn(&mut [usize], &Cell<usize>)) {
    let mut rng = Rng(3);
    for &n in &COUNT_OPS_SIZES[..5] {
        let mut data: Vec<usize> = (0..n).map(|_| rng.int_n(n / 5)).collect();
        let ncmp = Cell::new(0usize);
        algo(&mut data, &ncmp);
        assert!(data.is_sorted(), "{name} did not sort {n} ints");
        println!("{name} {n:>8} elements: {:>10} Less", ncmp.get());
    }
}

// sort_test.go:715
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_count_sort_ops() {
    count_ops("Sort  ", |data, ncmp| {
        sort::slice(data, |a, b| {
            ncmp.set(ncmp.get() + 1);
            a < b
        });
    });
}

// sort_test.go:714, against std sort_by
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_count_stable_ops() {
    count_ops("Stable", |data, ncmp| {
        data.sort_by(|a, b| {
            ncmp.set(ncmp.get() + 1);
            a.cmp(b)
        });
    });
}

// sort_test.go:601, against std sort_by
#[test]
#[allow(clippy::unnecessary_sort_by)]
#[ignore = "graphify-rs-aa6.16"]
fn test_stable_ints() {
    let mut data = INTS;
    data.sort_by(|a, b| a.cmp(b));
    assert!(data.is_sorted(), "nsorted {INTS:?}\n   got {data:?}");
}

#[derive(Clone, Copy, Default)]
struct IntPair {
    a: usize,
    b: usize,
}

// IntPairs compare on a only.
fn pairs_sorted(d: &[IntPair]) -> bool {
    d.windows(2).all(|w| w[1].a >= w[0].a)
}

// Record initial order in B.
fn init_b(d: &mut [IntPair]) {
    for (i, p) in d.iter_mut().enumerate() {
        p.b = i;
    }
}

// InOrder checks if a-equal elements were not reordered.
fn in_order(d: &[IntPair]) -> bool {
    let (mut last_a, mut last_b) = (usize::MAX, 0);
    for p in d {
        if last_a != p.a {
            last_a = p.a;
            last_b = p.b;
            continue;
        }
        if p.b <= last_b {
            return false;
        }
        last_b = p.b;
    }
    true
}

// sort_test.go:642 (Go's non-short n, m), against std sort_by. Go's lastA starts at -1; usize::MAX
// is the same "no element yet" value.
#[test]
#[allow(clippy::unnecessary_sort_by)]
#[ignore = "graphify-rs-aa6.16"]
fn test_stability() {
    let (n, m) = (100000, 1000);
    let mut data = vec![IntPair::default(); n];
    let mut rng = Rng(4);
    let stable = |d: &mut [IntPair]| d.sort_by(|x, y| x.a.cmp(&y.a));

    // random distribution
    for p in data.iter_mut() {
        p.a = rng.int_n(m);
    }
    assert!(!pairs_sorted(&data), "terrible rand.rand");
    init_b(&mut data);
    stable(&mut data);
    assert!(pairs_sorted(&data), "Stable didn't sort {n} ints");
    assert!(in_order(&data), "Stable wasn't stable on {n} ints");

    // already sorted
    init_b(&mut data);
    stable(&mut data);
    assert!(
        pairs_sorted(&data),
        "Stable shuffled sorted {n} ints (order)"
    );
    assert!(
        in_order(&data),
        "Stable shuffled sorted {n} ints (stability)"
    );

    // sorted reversed
    let len = data.len();
    for (i, p) in data.iter_mut().enumerate() {
        p.a = len - i;
    }
    init_b(&mut data);
    stable(&mut data);
    assert!(pairs_sorted(&data), "Stable didn't sort {n} ints");
    assert!(in_order(&data), "Stable wasn't stable on {n} ints");
}
