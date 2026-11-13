//! Port of Go 1.27.4 `sort/zsortfunc.go` (`pdqsort_func`, behind `sort.Slice`) and
//! `slices/zsortanyfunc.go` (`pdqsortCmpFunc`, behind `slices.SortFunc`), plus
//! `slices.BinarySearchFunc`. Call sites: `sort.Slice` at 14 sites, `slices.SortFunc` at
//! `build.go:155`, `slices.BinarySearchFunc` at `csharp.go:625`. Stable sorts
//! (`sort.SliceStable`, `slices.SortStableFunc`) are std `sort_by`, a plan decision: a stable
//! sort's result is unique for a consistent comparator.
//!
//! The two Go files are the same algorithm with the same call order (`data.Less(i, j)` there,
//! `cmp(data[i], data[j]) < 0` here), so both entry points share one port written over a
//! `less(&x[i], &x[j])` closure. Every `less` call passes references into the slice, in Go's
//! argument order, so the call trace matches Go's.

use std::cmp::Ordering;

#[derive(PartialEq)]
enum Hint {
    Unknown,
    Increasing,
    Decreasing,
}

/// Go `bits.Len(uint(n))`.
fn bits_len(n: usize) -> usize {
    (usize::BITS - n.leading_zeros()) as usize
}

/// Go `sort.slice` entry: `pdqsort(data, 0, n, bits.Len(n))`.
fn sort_by_less<T>(x: &mut [T], less: &mut impl FnMut(&T, &T) -> bool) {
    let n = x.len();
    pdqsort(x, 0, n, bits_len(n), less);
}

/// Go `sort.Slice` (`pdqsort_func`, `limit = bits.Len(len)`): unstable sort by `less`.
pub fn slice<T>(x: &mut [T], mut less: impl FnMut(&T, &T) -> bool) {
    sort_by_less(x, &mut less);
}

/// Go `slices.SortFunc` (`pdqsortCmpFunc`): the same algorithm with `less = cmp(a, b) == Less`.
pub fn sort_func<T>(x: &mut [T], mut cmp: impl FnMut(&T, &T) -> Ordering) {
    sort_by_less(x, &mut |a: &T, b: &T| cmp(a, b) == Ordering::Less);
}

/// Go `slices.BinarySearchFunc`: the first position where `cmp(x[i], target)` is not `Less`, and
/// whether that element compares `Equal`. `U: ?Sized` (the plan says `<T, U>`) so a `&str`
/// target works.
pub fn binary_search_func<T, U: ?Sized>(
    x: &[T],
    target: &U,
    mut cmp: impl FnMut(&T, &U) -> Ordering,
) -> (usize, bool) {
    let n = x.len();
    let (mut i, mut j) = (0, n);
    while i < j {
        let h = (i + j) >> 1;
        if cmp(&x[h], target) == Ordering::Less {
            i = h + 1;
        } else {
            j = h;
        }
    }
    (i, i < n && cmp(&x[i], target) == Ordering::Equal)
}

/// Go `sort.reverseRange` (`zsortfunc.go`), reverses `x[a..b]`. Public only so the ported
/// `TestReverseRange` can reach it.
#[doc(hidden)]
pub fn reverse_range<T>(x: &mut [T], a: usize, b: usize) {
    // Go: i, j := a, b-1; for i < j. `j` here is Go's j+1, so b == 0 cannot underflow.
    let (mut i, mut j) = (a, b);
    while i + 1 < j {
        j -= 1;
        x.swap(i, j);
        i += 1;
    }
}

/// insertionSort sorts data[a:b] using insertion sort.
fn insertion_sort<T>(data: &mut [T], a: usize, b: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    for i in a + 1..b {
        let mut j = i;
        while j > a && less(&data[j], &data[j - 1]) {
            data.swap(j, j - 1);
            j -= 1;
        }
    }
}

/// siftDown implements the heap property on data[lo:hi]. `first` is an offset into the array
/// where the root of the heap lies.
fn sift_down<T>(
    data: &mut [T],
    lo: usize,
    hi: usize,
    first: usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) {
    let mut root = lo;
    loop {
        let mut child = 2 * root + 1;
        if child >= hi {
            break;
        }
        if child + 1 < hi && less(&data[first + child], &data[first + child + 1]) {
            child += 1;
        }
        if !less(&data[first + root], &data[first + child]) {
            return;
        }
        data.swap(first + root, first + child);
        root = child;
    }
}

fn heap_sort<T>(data: &mut [T], a: usize, b: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    let first = a;
    let lo = 0;
    let hi = b - a;

    // Build heap with greatest element at top.
    for i in (0..=(hi - 1) / 2).rev() {
        sift_down(data, i, hi, first, less);
    }

    // Pop elements, largest first, into end of data.
    for i in (0..hi).rev() {
        data.swap(first, first + i);
        sift_down(data, lo, i, first, less);
    }
}

/// pdqsort sorts data[a:b]. `limit` is the number of allowed bad (very unbalanced) pivots
/// before falling back to heapsort.
fn pdqsort<T>(
    data: &mut [T],
    mut a: usize,
    mut b: usize,
    mut limit: usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) {
    const MAX_INSERTION: usize = 12;

    let mut was_balanced = true; // whether the last partitioning was reasonably balanced
    let mut was_partitioned = true; // whether the slice was already partitioned

    loop {
        let length = b - a;

        if length <= MAX_INSERTION {
            insertion_sort(data, a, b, less);
            return;
        }

        // Fall back to heapsort if too many bad choices were made.
        if limit == 0 {
            heap_sort(data, a, b, less);
            return;
        }

        // If the last partitioning was imbalanced, we need to breaking patterns.
        if !was_balanced {
            break_patterns(data, a, b);
            limit -= 1;
        }

        let (mut pivot, mut hint) = choose_pivot(data, a, b, less);
        if hint == Hint::Decreasing {
            reverse_range(data, a, b);
            // The chosen pivot was pivot-a elements after the start of the array.
            // After reversing it is pivot-a elements before the end of the array.
            pivot = (b - 1) - (pivot - a);
            hint = Hint::Increasing;
        }

        // The slice is likely already sorted.
        if was_balanced
            && was_partitioned
            && hint == Hint::Increasing
            && partial_insertion_sort(data, a, b, less)
        {
            return;
        }

        // Probably the slice contains many duplicate elements, partition the slice into
        // elements equal to and elements greater than the pivot.
        if a > 0 && !less(&data[a - 1], &data[pivot]) {
            let mid = partition_equal(data, a, b, pivot, less);
            a = mid;
            continue;
        }

        let (mid, already_partitioned) = partition(data, a, b, pivot, less);
        was_partitioned = already_partitioned;

        let (left_len, right_len) = (mid - a, b - mid);
        let balance_threshold = length / 8;
        if left_len < right_len {
            was_balanced = left_len >= balance_threshold;
            pdqsort(data, a, mid, limit, less);
            a = mid + 1;
        } else {
            was_balanced = right_len >= balance_threshold;
            pdqsort(data, mid + 1, b, limit, less);
            b = mid;
        }
    }
}

/// partition does one quicksort partition. Let p = data[pivot]. Moves elements in data[a:b]
/// around, so that data[i]<p and data[j]>=p for i<newpivot and j>newpivot. On return,
/// data[newpivot] = p.
fn partition<T>(
    data: &mut [T],
    a: usize,
    b: usize,
    pivot: usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> (usize, bool) {
    data.swap(a, pivot);
    // i and j are inclusive of the elements remaining to be partitioned (j stays >= a).
    let (mut i, mut j) = (a + 1, b - 1);

    while i <= j && less(&data[i], &data[a]) {
        i += 1;
    }
    while i <= j && !less(&data[j], &data[a]) {
        j -= 1;
    }
    if i > j {
        data.swap(j, a);
        return (j, true);
    }
    data.swap(i, j);
    i += 1;
    j -= 1;

    loop {
        while i <= j && less(&data[i], &data[a]) {
            i += 1;
        }
        while i <= j && !less(&data[j], &data[a]) {
            j -= 1;
        }
        if i > j {
            break;
        }
        data.swap(i, j);
        i += 1;
        j -= 1;
    }
    data.swap(j, a);
    (j, false)
}

/// partitionEqual partitions data[a:b] into elements equal to data[pivot] followed by elements
/// greater than data[pivot]. It assumed that data[a:b] does not contain elements smaller than
/// the data[pivot].
fn partition_equal<T>(
    data: &mut [T],
    a: usize,
    b: usize,
    pivot: usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> usize {
    data.swap(a, pivot);
    let (mut i, mut j) = (a + 1, b - 1);

    loop {
        while i <= j && !less(&data[a], &data[i]) {
            i += 1;
        }
        while i <= j && less(&data[a], &data[j]) {
            j -= 1;
        }
        if i > j {
            break;
        }
        data.swap(i, j);
        i += 1;
        j -= 1;
    }
    i
}

/// partialInsertionSort partially sorts a slice, returns true if the slice is sorted at the end.
fn partial_insertion_sort<T>(
    data: &mut [T],
    a: usize,
    b: usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> bool {
    const MAX_STEPS: usize = 5; // maximum number of adjacent out-of-order pairs that will get shifted
    const SHORTEST_SHIFTING: usize = 50; // don't shift any elements on short arrays
    let mut i = a + 1;
    for _ in 0..MAX_STEPS {
        while i < b && !less(&data[i], &data[i - 1]) {
            i += 1;
        }

        if i == b {
            return true;
        }

        if b - a < SHORTEST_SHIFTING {
            return false;
        }

        data.swap(i, i - 1);

        // Shift the smaller one to the left.
        if i - a >= 2 {
            let mut j = i - 1;
            while j >= 1 {
                if !less(&data[j], &data[j - 1]) {
                    break;
                }
                data.swap(j, j - 1);
                j -= 1;
            }
        }
        // Shift the greater one to the right.
        if b - i >= 2 {
            let mut j = i + 1;
            while j < b {
                if !less(&data[j], &data[j - 1]) {
                    break;
                }
                data.swap(j, j - 1);
                j += 1;
            }
        }
    }
    false
}

/// breakPatterns scatters some elements around in an attempt to break some patterns that might
/// cause imbalanced partitions in quicksort.
fn break_patterns<T>(data: &mut [T], a: usize, b: usize) {
    let length = b - a;
    if length >= 8 {
        let mut random = length as u64; // xorshift(length)
        let modulus = 1usize << bits_len(length); // nextPowerOfTwo(length)

        for idx in a + (length / 4) * 2 - 1..=a + (length / 4) * 2 + 1 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let mut other = (random as usize) & (modulus - 1);
            if other >= length {
                other -= length;
            }
            data.swap(idx, a + other);
        }
    }
}

/// choosePivot chooses a pivot in data[a:b].
///
/// [0,8): chooses a static pivot.
/// [8,shortestNinther): uses the simple median-of-three method.
/// [shortestNinther,∞): uses the Tukey ninther method.
fn choose_pivot<T>(
    data: &[T],
    a: usize,
    b: usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> (usize, Hint) {
    const SHORTEST_NINTHER: usize = 50;
    const MAX_SWAPS: usize = 4 * 3;

    let l = b - a;

    let mut swaps = 0;
    let mut i = a + l / 4;
    let mut j = a + l / 4 * 2;
    let mut k = a + l / 4 * 3;

    if l >= 8 {
        if l >= SHORTEST_NINTHER {
            // Tukey ninther method, the idea came from Rust's implementation.
            i = median_adjacent(data, i, &mut swaps, less);
            j = median_adjacent(data, j, &mut swaps, less);
            k = median_adjacent(data, k, &mut swaps, less);
        }
        // Find the median among i, j, k and stores it into j.
        j = median(data, i, j, k, &mut swaps, less);
    }

    match swaps {
        0 => (j, Hint::Increasing),
        MAX_SWAPS => (j, Hint::Decreasing),
        _ => (j, Hint::Unknown),
    }
}

/// order2 returns x,y where data[x] <= data[y], where x,y=a,b or x,y=b,a.
fn order2<T>(
    data: &[T],
    a: usize,
    b: usize,
    swaps: &mut usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> (usize, usize) {
    if less(&data[b], &data[a]) {
        *swaps += 1;
        return (b, a);
    }
    (a, b)
}

/// median returns x where data[x] is the median of data[a],data[b],data[c], where x is a, b,
/// or c.
fn median<T>(
    data: &[T],
    a: usize,
    b: usize,
    c: usize,
    swaps: &mut usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> usize {
    let (a, b) = order2(data, a, b, swaps, less);
    let (b, _c) = order2(data, b, c, swaps, less);
    let (_a, b) = order2(data, a, b, swaps, less);
    b
}

/// medianAdjacent finds the median of data[a - 1], data[a], data[a + 1] and stores the index
/// into a.
fn median_adjacent<T>(
    data: &[T],
    a: usize,
    swaps: &mut usize,
    less: &mut impl FnMut(&T, &T) -> bool,
) -> usize {
    median(data, a - 1, a, a + 1, swaps, less)
}
