//! Ported from slices/sort_test.go (GOSRC, Go 1.27.4, BSD-3-Clause). 13 Test funcs: 7 ported,
//! 6 not ported. `slices.Sort` and `slices.SortFunc` are driven through `gostd::sort::sort_func`
//! (`pdqsortCmpFunc`); `cmp.Compare` / `strings.Compare` become `Ord::cmp`.
//! - TestSortIntSlice, TestSortFuncIntSlice, TestSortStringSlice: ported through `sort_func`
//! - TestSortLarge_Random: ported; Go uses n = 1,000,000, or 10,000 under testing.Short(); this
//!   port uses the short size 10,000
//! - TestStability: ported against std `sort_by` (which graphify-rs uses for
//!   `slices.SortStableFunc`), Go's non-short n, m
//! - TestBinarySearch, TestBinarySearchFunc: ported, tables verbatim, against
//!   `binary_search_func`; Go's `BinarySearch` on ordered values is `binary_search_func` with
//!   `Ord::cmp`
//! - TestSortFloat64Slice: not ported: floats, graphify-go sorts no floats
//! - TestMinMax: not ported: slices.Min/Max/MinFunc/MaxFunc are not called by graphify-go
//! - TestMinMaxNaNs: not ported: slices.Min/Max with NaN floats, not called by graphify-go
//! - TestMinMaxPanics: not ported: slices.Min/Max panics on empty input, not called by graphify-go
//! - TestBinarySearchInts: not ported: int table for BinarySearch, same `binary_search_func`
//!   path as TestBinarySearch; graphify-go calls only BinarySearchFunc (csharp.go:625)
//! - TestBinarySearchFloats: not ported: BinarySearch on floats with NaN, not called by graphify-go
//!
//! Random data uses a local splitmix64 generator (gostd has no third-party crates).

use gostd::sort;

const INTS: [i64; 13] = [
    74, 59, 238, -784, 9845, 959, 905, 0, 0, 42, 7586, -5467984, 7586,
];
const STRS: [&str; 8] = ["", "Hello", "foo", "bar", "foo", "f00", "%*&^*&^&", "***"];

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
}

// slices/sort_test.go:22
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_int_slice() {
    let mut data = INTS;
    sort::sort_func(&mut data, |a, b| a.cmp(b));
    assert!(data.is_sorted(), "sorted {INTS:?}\n   got {data:?}");
}

// slices/sort_test.go:31
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_func_int_slice() {
    let mut data = INTS;
    sort::sort_func(&mut data, |a, b| (a - b).cmp(&0));
    assert!(data.is_sorted(), "sorted {INTS:?}\n   got {data:?}");
}

// slices/sort_test.go:49
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_string_slice() {
    let mut data = STRS;
    sort::sort_func(&mut data, |a, b| a.cmp(b));
    assert!(data.is_sorted(), "sorted {STRS:?}\n   got {data:?}");
}

// slices/sort_test.go:58 (testing.Short() size)
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_sort_large_random() {
    let n = 1_000_000 / 100;
    let mut rng = Rng(1);
    let mut data: Vec<i64> = (0..n).map(|_| rng.int_n(100) as i64).collect();
    assert!(!data.is_sorted(), "terrible rand.rand");
    sort::sort_func(&mut data, |a, b| a.cmp(b));
    assert!(data.is_sorted(), "sort didn't sort - 1M ints");
}

#[derive(Clone, Copy, Default)]
struct IntPair {
    a: usize,
    b: usize,
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

// Pairs compare on a only.
fn pairs_sorted(d: &[IntPair]) -> bool {
    d.windows(2).all(|w| w[1].a >= w[0].a)
}

// slices/sort_test.go:118 (Go's non-short n, m), against std sort_by. Go's lastA starts at -1;
// usize::MAX is the same "no element yet" value.
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

struct SearchCase<'a> {
    data: &'a [&'a str],
    target: &'a str,
    want_pos: usize,
    want_found: bool,
}

fn case<'a>(
    data: &'a [&'a str],
    target: &'a str,
    want_pos: usize,
    want_found: bool,
) -> SearchCase<'a> {
    SearchCase {
        data,
        target,
        want_pos,
        want_found,
    }
}

// slices/sort_test.go:284
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_binary_search() {
    let str1: &[&str] = &["foo"];
    let str2: &[&str] = &["ab", "ca"];
    let str3: &[&str] = &["mo", "qo", "vo"];
    let str4: &[&str] = &["ab", "ad", "ca", "xy"];

    // slice with repeating elements
    let str_repeats: &[&str] = &["ba", "ca", "da", "da", "da", "ka", "ma", "ma", "ta"];

    // slice with all element equal
    let str_same: &[&str] = &["xx", "xx", "xx"];

    let tests = [
        case(&[], "foo", 0, false),
        case(&[], "", 0, false),
        case(str1, "foo", 0, true),
        case(str1, "bar", 0, false),
        case(str1, "zx", 1, false),
        case(str2, "aa", 0, false),
        case(str2, "ab", 0, true),
        case(str2, "ad", 1, false),
        case(str2, "ca", 1, true),
        case(str2, "ra", 2, false),
        case(str3, "bb", 0, false),
        case(str3, "mo", 0, true),
        case(str3, "nb", 1, false),
        case(str3, "qo", 1, true),
        case(str3, "tr", 2, false),
        case(str3, "vo", 2, true),
        case(str3, "xr", 3, false),
        case(str4, "aa", 0, false),
        case(str4, "ab", 0, true),
        case(str4, "ac", 1, false),
        case(str4, "ad", 1, true),
        case(str4, "ax", 2, false),
        case(str4, "ca", 2, true),
        case(str4, "cc", 3, false),
        case(str4, "dd", 3, false),
        case(str4, "xy", 3, true),
        case(str4, "zz", 4, false),
        case(str_repeats, "da", 2, true),
        case(str_repeats, "db", 5, false),
        case(str_repeats, "ma", 6, true),
        case(str_repeats, "mb", 8, false),
        case(str_same, "xx", 0, true),
        case(str_same, "ab", 0, false),
        case(str_same, "zz", 3, false),
    ];
    for tt in &tests {
        // BinarySearch: ordered values, Ord::cmp.
        let (pos, found) = sort::binary_search_func(tt.data, &tt.target, |a, b| a.cmp(b));
        assert!(
            pos == tt.want_pos && found == tt.want_found,
            "BinarySearch({:?}) got ({pos}, {found}), want ({}, {})",
            tt.target,
            tt.want_pos,
            tt.want_found
        );

        // BinarySearchFunc with strings.Compare.
        let (pos, found) = sort::binary_search_func(tt.data, tt.target, |a, b| (**a).cmp(b));
        assert!(
            pos == tt.want_pos && found == tt.want_found,
            "BinarySearchFunc({:?}) got ({pos}, {found}), want ({}, {})",
            tt.target,
            tt.want_pos,
            tt.want_found
        );
    }
}

// slices/sort_test.go:422
#[test]
#[ignore = "graphify-rs-aa6.16"]
fn test_binary_search_func() {
    let data: [i64; 4] = [1, 10, 11, 2]; // sorted lexicographically
    let target: &str = "2";
    let (pos, found) = sort::binary_search_func(&data, target, |a: &i64, b: &str| {
        a.to_string().as_str().cmp(b)
    });
    assert!(
        pos == 3 && found,
        "BinarySearchFunc({data:?}, {target:?}, cmp) = {pos}, {found}, want 3, true"
    );
}
