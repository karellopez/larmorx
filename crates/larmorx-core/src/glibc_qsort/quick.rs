// SPDX-License-Identifier: Apache-2.0
//! The quicksort order of `specs/glibc-qsort.md` §5 (glibc's fallback): a quicksort phase for
//! more than [`TAU`] elements (a median of three, the partition scans, an explicit stack of
//! pending ranges; Sedgewick 1978), then an insertion phase over the whole array, both with the
//! spec's exact conventions. Positions are signed: an inconsistent comparator can drive a scan
//! past the ends, and the sort stops before the call that would name such a position.

use super::{Blocks, QuickOrderError};

/// The threshold `τ`: a part of at most `τ + 1` elements is left to the insertion phase.
const TAU: isize = 4;

/// Sorts `v`; `lt(x, y)` is whether the comparator's result for `(x, y)` is negative.
pub(super) fn sort<B, F>(v: &mut B, lt: &mut F) -> Result<(), QuickOrderError>
where
    B: Blocks + ?Sized,
    F: FnMut(&B::Elem, &B::Elem) -> bool,
{
    let len = v.count();
    if len < 2 {
        return Ok(());
    }
    let last = len as isize - 1;
    // the comparator on positions p and q, refusing a position outside the array
    let mut less = |v: &B, p: isize, q: isize| -> Result<bool, QuickOrderError> {
        for position in [p, q] {
            if position < 0 || position > last {
                return Err(QuickOrderError::Outside { position, len });
            }
        }
        Ok(lt(v.elem(p as usize), v.elem(q as usize)))
    };
    let at = |p: isize| p as usize;

    // the quicksort phase, for more than τ elements
    if last >= TAU {
        let mut stack: Vec<(isize, isize)> = Vec::new();
        let (mut lo, mut hi) = (0, last);
        loop {
            if hi < lo {
                return Err(QuickOrderError::ReversedRange { lo, hi, len });
            }
            // the pivot: a median of three, `mid` following it
            let mut mid = lo + (hi - lo) / 2;
            if less(v, mid, lo)? {
                v.swap_at(at(mid), at(lo));
            }
            if less(v, hi, mid)? {
                v.swap_at(at(mid), at(hi));
                if less(v, mid, lo)? {
                    v.swap_at(at(mid), at(lo));
                }
            }
            // the partition
            let (mut l, mut r) = (lo + 1, hi - 1);
            loop {
                while less(v, l, mid)? {
                    l += 1;
                }
                while less(v, mid, r)? {
                    r -= 1;
                }
                if l < r {
                    v.swap_at(at(l), at(r));
                    if mid == l {
                        mid = r;
                    } else if mid == r {
                        mid = l;
                    }
                    l += 1;
                    r -= 1;
                } else if l == r {
                    l += 1;
                    r -= 1;
                    break;
                }
                if l > r {
                    break;
                }
            }
            // the next range
            let left_small = (0..=TAU).contains(&(r - lo));
            let right_small = (0..=TAU).contains(&(hi - l));
            match (left_small, right_small) {
                (true, true) => match stack.pop() {
                    None => break,
                    Some((a, b)) => (lo, hi) = (a, b),
                },
                (true, false) => lo = l,
                (false, true) => hi = r,
                (false, false) => {
                    if r - lo > hi - l {
                        stack.push((lo, r));
                        lo = l;
                    } else {
                        stack.push((l, hi));
                        hi = r;
                    }
                }
            }
        }
    }

    // the insertion phase: the first minimum of the first τ + 1 elements to the front
    let mut k = 0;
    for p in 1..=last.min(TAU) {
        if less(v, p, k)? {
            k = p;
        }
    }
    if k != 0 {
        v.swap_at(at(k), 0);
    }
    for p in 2..=last {
        let mut q = p - 1;
        while less(v, p, q)? {
            q -= 1;
        }
        q += 1;
        if q != p {
            v.shift_down(at(q), at(p));
        }
    }
    Ok(())
}
