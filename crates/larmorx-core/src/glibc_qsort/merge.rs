// SPDX-License-Identifier: Apache-2.0
//! The merge order of `specs/glibc-qsort.md` §4: a top-down merge sort (Knuth vol. 3 §5.2.4),
//! the left part `⌊m / 2⌋` elements, the left part sorted before the right, the merge calling the
//! comparator with the left part's element first and taking it on a result ≤ 0.
//!
//! The array and its copy take turns as source and destination (each level merges the halves
//! its two calls left in the other store), so no level copies back: the same calls on the same
//! contents as a sort that merges into a buffer and copies the result back.

use super::Copyable;

/// Sorts the `n` elements of `a`, with `b` an exact copy of `a` on entry (it is scrambled);
/// `gt(x, y)` is whether the comparator's result for `(x, y)` is positive.
pub(super) fn sort<S, F>(a: &mut S, b: &mut S, n: usize, gt: &mut F)
where
    S: Copyable + ?Sized,
    F: FnMut(&S::Elem, &S::Elem) -> bool,
{
    split_merge(b, a, 0, n, gt);
}

/// Sorts positions `lo..hi` into `dst`; on entry `src` and `dst` hold the same elements there.
fn split_merge<S, F>(src: &mut S, dst: &mut S, lo: usize, hi: usize, gt: &mut F)
where
    S: Copyable + ?Sized,
    F: FnMut(&S::Elem, &S::Elem) -> bool,
{
    let m = hi - lo;
    if m < 2 {
        return;
    }
    if m == 2 {
        // two parts of one element: one call
        if gt(src.elem(lo), src.elem(lo + 1)) {
            dst.put(lo, src, lo + 1);
            dst.put(lo + 1, src, lo);
        }
        return;
    }
    let mid = lo + m / 2;
    // each half sorted into `src` (the roles swap one level down), the left one first
    split_merge(dst, src, lo, mid, gt);
    split_merge(dst, src, mid, hi, gt);
    S::merge_halves(src, dst, lo, mid, hi, gt);
}

/// Merges the sorted `src[lo..mid]` and `src[mid..hi]` into `dst[lo..hi]`: the element-by-element
/// merge any store can make ([`Copyable::merge_halves`]'s default).
pub(super) fn merge_by_element<S, F>(
    src: &S,
    dst: &mut S,
    lo: usize,
    mid: usize,
    hi: usize,
    gt: &mut F,
) where
    S: Copyable + ?Sized,
    F: FnMut(&S::Elem, &S::Elem) -> bool,
{
    let (mut i, mut j, mut k) = (lo, mid, lo);
    while i < mid && j < hi {
        if gt(src.elem(i), src.elem(j)) {
            dst.put(k, src, j);
            j += 1;
        } else {
            dst.put(k, src, i);
            i += 1;
        }
        k += 1;
    }
    // the rest of one part, with no further call
    if i < mid {
        dst.put_range(k, src, i, mid - i);
    } else if j < hi {
        dst.put_range(k, src, j, hi - j);
    }
}

/// The same merge on slices of `Copy` elements (the same calls in the same order: one per output
/// element until a part runs out).
pub(super) fn merge_slices<T: Copy, F: FnMut(&T, &T) -> bool>(
    left: &[T],
    right: &[T],
    out: &mut [T],
    gt: &mut F,
) {
    let (mut i, mut j, mut k) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        if gt(&left[i], &right[j]) {
            out[k] = right[j];
            j += 1;
        } else {
            out[k] = left[i];
            i += 1;
        }
        k += 1;
    }
    if i < left.len() {
        out[k..].copy_from_slice(&left[i..]);
    } else {
        out[k..].copy_from_slice(&right[j..]);
    }
}

/// The merge order of `store`'s `n` elements through their positions (glibc's pointers for
/// elements of more than 32 bytes): the same calls, the elements left in place; returns, for
/// each position, the position of the element that goes there.
pub(super) fn through_positions<S, F>(store: &S, n: usize, gt: &mut F) -> Vec<usize>
where
    S: Copyable + ?Sized,
    F: FnMut(&S::Elem, &S::Elem) -> bool,
{
    let mut a: Vec<usize> = (0..n).collect();
    let mut b = a.clone();
    sort(&mut a[..], &mut b[..], n, &mut |&x: &usize, &y: &usize| {
        gt(store.elem(x), store.elem(y))
    });
    a
}

/// The merge order of `v` through references to its elements (glibc's pointers for elements of
/// more than 32 bytes): the same calls, the elements left in place; returns, for each position,
/// the position of the element that goes there. Two references per element at the peak, as
/// glibc's two pointers.
pub(super) fn through_references<T: Copy, F: FnMut(&T, &T) -> bool>(
    v: &[T],
    gt: &mut F,
) -> Vec<usize> {
    let n = v.len();
    let mut a: Vec<&T> = v.iter().collect();
    let mut b = a.clone();
    sort(&mut a[..], &mut b[..], n, &mut |x: &&T, y: &&T| gt(x, y));
    drop(b);
    // a reference's position from its address (`size_of::<T>()` is above 32 here)
    let base = v.as_ptr().addr();
    a.iter()
        .map(|r| (std::ptr::from_ref::<T>(r).addr() - base) / size_of::<T>())
        .collect()
}
