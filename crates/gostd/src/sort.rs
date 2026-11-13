//! Port of Go 1.27.4 `sort/zsortfunc.go` (`pdqsort_func`, behind `sort.Slice`) and
//! `slices/zsortanyfunc.go` (`pdqsortCmpFunc`, behind `slices.SortFunc`), plus
//! `slices.BinarySearchFunc`. Call sites: `sort.Slice` at 14 sites, `slices.SortFunc` at
//! `build.go:155`, `slices.BinarySearchFunc` at `csharp.go:625`. Stable sorts
//! (`sort.SliceStable`, `slices.SortStableFunc`) are std `sort_by`, a plan decision: a stable
//! sort's result is unique for a consistent comparator.
#![allow(unused_variables)]

use std::cmp::Ordering;

/// Go `sort.Slice` (`pdqsort_func`, `limit = bits.Len(len)`): unstable sort by `less`.
pub fn slice<T>(x: &mut [T], less: impl FnMut(&T, &T) -> bool) {
    todo!("graphify-rs-aa6.16")
}

/// Go `slices.SortFunc` (`pdqsortCmpFunc`): the same algorithm with `less = cmp(a, b) == Less`.
pub fn sort_func<T>(x: &mut [T], cmp: impl FnMut(&T, &T) -> Ordering) {
    todo!("graphify-rs-aa6.16")
}

/// Go `slices.BinarySearchFunc`: the first position where `cmp(x[i], target)` is not `Less`, and
/// whether that element compares `Equal`. `U: ?Sized` (the plan says `<T, U>`) so a `&str`
/// target works.
pub fn binary_search_func<T, U: ?Sized>(
    x: &[T],
    target: &U,
    cmp: impl FnMut(&T, &U) -> Ordering,
) -> (usize, bool) {
    todo!("graphify-rs-aa6.16")
}

/// Go `sort.reverseRange` (`zsortfunc.go`), reverses `x[a..b]`. Public only so the ported
/// `TestReverseRange` can reach it.
#[doc(hidden)]
pub fn reverse_range<T>(x: &mut [T], a: usize, b: usize) {
    todo!("graphify-rs-aa6.16")
}
