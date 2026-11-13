// SPDX-License-Identifier: Apache-2.0
//! The C library's `qsort` and `qsort_r` as the GNU C Library 2.35 (x86-64) leaves an array: a
//! **clean-room implementation** of `specs/glibc-qsort.md` (FreeSurfer's foundations, the GNU
//! family), written from that spec, the publications it cites (Knuth's merge sort, Sedgewick's
//! quicksort) and black-box runs of the C library's functions only.
//!
//! A correct sort is not enough to give glibc's arrangement: elements the comparator calls equal
//! often differ (a structure sorted by one field, `+0.0` and `−0.0`, NaNs of different payloads),
//! and FreeSurfer's comparators are not always consistent (some never return 0). So every function
//! here makes **exactly glibc's comparator calls, in glibc's order**, and leaves glibc's
//! arrangement, for any comparator whose result depends only on the two elements' contents:
//!
//! - [`merge_order`] (§4, glibc's usual method and the one the replicas call; [`qsort`] and
//!   [`qsort_r`] are it under the C names): a top-down merge sort, the left part `⌊m / 2⌋`
//!   elements, a tie (a result ≤ 0) taking the left part's element; a temporary copy of the slice
//!   (elements of more than 32 bytes are ordered through their positions, then moved once, as glibc
//!   orders them through pointers: the same calls on the same contents);
//! - [`quick_order`] (§5, glibc's fallback when it cannot have the temporary array): a quicksort
//!   with a median of three, an explicit stack and a final insertion sort, in place; where an
//!   inconsistent comparator would drive it outside the array (where glibc reads and may write
//!   memory that is not the array's, Qualys 2024), it stops with a [`QuickOrderError`] at that
//!   call, after exactly the calls before it (D13: never reproduced);
//! - [`choose`] and [`qsort_as_glibc`] (§3): glibc's choice between the two from the temporary
//!   array's size, the page size, the physical memory and whether the allocation succeeds, for a
//!   caller that wants it; by default (D13: a result that depends on the machine's memory is never
//!   reproduced) the replicas call [`merge_order`].
//!
//! Each has a variant over blocks of bytes (`_bytes`: `n` elements of `size` bytes, as C's
//! interface takes them). The comparator returns an [`Ordering`] or a signed integer (only its
//! sign is used, [`Comparison`]); a context, `qsort_r`'s third argument, is whatever the closure
//! captures. Index arithmetic only: the same result on every platform, no threads, no global
//! state.

mod merge;
mod quick;
#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::fmt;

/// A comparator's result: only its sign is used (C's `int`, or an [`Ordering`]).
pub trait Comparison {
    /// The sign as an ordering (`Less` for a negative value).
    fn ordering(self) -> Ordering;
}

impl Comparison for Ordering {
    fn ordering(self) -> Ordering {
        self
    }
}

macro_rules! signed_comparison {
    ($($t:ty),*) => {$(
        impl Comparison for $t {
            fn ordering(self) -> Ordering {
                self.cmp(&0)
            }
        }
    )*};
}
signed_comparison!(i8, i16, i32, i64, isize);

/// Elements of more than this many bytes are ordered through their positions (§3, §4): glibc's
/// temporary array then holds two 8-byte pointers per element and one spare element.
pub const DIRECT_LIMIT: usize = 32;

/// The stack limit of §3: a temporary array of fewer bytes needs no allocation.
pub const STACK_LIMIT: u128 = 1024;

/// The physical memory glibc assumes when the system cannot report it (§3), in pages.
pub const UNKNOWN_PHYS_PAGES: u64 = (1 << 63) - 1;

/// Why [`quick_order`] stopped: glibc's fallback would pass the comparator memory outside the
/// array (possible only with an inconsistent comparator). The calls before it were made, exactly
/// glibc's; the slice is then in an unspecified arrangement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickOrderError {
    /// The next call would name position `position` of an array of `len` elements.
    Outside {
        /// The position (negative or at least `len`).
        position: isize,
        /// The number of elements.
        len: usize,
    },
    /// The next range ends before it starts, so its pivot lies outside the array.
    ReversedRange {
        /// The range's first position.
        lo: isize,
        /// The range's last position (below `lo`).
        hi: isize,
        /// The number of elements.
        len: usize,
    },
}

impl fmt::Display for QuickOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            QuickOrderError::Outside { position, len } => write!(
                f,
                "glibc's quicksort order would pass the comparator position {position}, outside \
                 the array of {len} elements (an inconsistent comparator): not reproduced"
            ),
            QuickOrderError::ReversedRange { lo, hi, len } => write!(
                f,
                "glibc's quicksort order reached the range {lo}..{hi} of an array of {len} \
                 elements, whose pivot lies outside the array (an inconsistent comparator): not \
                 reproduced"
            ),
        }
    }
}

impl std::error::Error for QuickOrderError {}

/// The two orders of glibc's `qsort`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// §4, [`merge_order`].
    Merge,
    /// §5, [`quick_order`].
    Quick,
}

/// glibc's choice for one sort (§3), with the allocation it asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The temporary array is under [`STACK_LIMIT`] bytes: the merge order, no allocation.
    Stack,
    /// One allocation of the temporary array, granted: the merge order.
    Allocated,
    /// The temporary array would exceed a quarter of the physical memory: the quicksort order,
    /// no allocation.
    TooLarge,
    /// One allocation of the temporary array, refused: the quicksort order.
    AllocationRefused,
}

impl Choice {
    /// The order this choice sorts in.
    #[must_use]
    pub fn order(self) -> Order {
        match self {
            Choice::Stack | Choice::Allocated => Order::Merge,
            Choice::TooLarge | Choice::AllocationRefused => Order::Quick,
        }
    }

    /// How many times glibc asks the allocator for the temporary array (0 or 1).
    #[must_use]
    pub fn requests(self) -> u32 {
        match self {
            Choice::Allocated | Choice::AllocationRefused => 1,
            Choice::Stack | Choice::TooLarge => 0,
        }
    }
}

/// What glibc reads from the system for its choice (§3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    /// The page size in bytes (`sysconf(_SC_PAGESIZE)`).
    pub page_size: u64,
    /// The number of physical pages (`sysconf(_SC_PHYS_PAGES)`), `None` when the system cannot
    /// report it ([`UNKNOWN_PHYS_PAGES`] is then used).
    pub phys_pages: Option<u64>,
    /// Whether the allocation of the temporary array succeeds.
    pub allocation_succeeds: bool,
}

impl Machine {
    /// The reference machine of the spec (4096-byte pages, 32,763,452 of them: 124 GiB), with
    /// every allocation granted.
    pub const REFERENCE: Machine = Machine {
        page_size: 4096,
        phys_pages: Some(32_763_452),
        allocation_succeeds: true,
    };
}

/// The bytes of glibc's temporary array for `n` elements of `size` bytes (§3): `n · size` up to
/// [`DIRECT_LIMIT`] bytes per element, else `16 n + size`.
#[must_use]
pub fn temporary_bytes(n: usize, size: usize) -> u128 {
    if size <= DIRECT_LIMIT {
        n as u128 * size as u128
    } else {
        16 * n as u128 + size as u128
    }
}

/// glibc's choice of §3 for `n` elements of `size` bytes on `machine`.
///
/// # Panics
/// When `machine.page_size` is 0.
#[must_use]
pub fn choose(n: usize, size: usize, machine: &Machine) -> Choice {
    assert!(machine.page_size > 0, "the page size must be positive");
    let t = temporary_bytes(n, size);
    if t < STACK_LIMIT {
        return Choice::Stack;
    }
    let phys = machine.phys_pages.unwrap_or(UNKNOWN_PHYS_PAGES);
    if t / u128::from(machine.page_size) > u128::from(phys / 4) {
        Choice::TooLarge
    } else if machine.allocation_succeeds {
        Choice::Allocated
    } else {
        Choice::AllocationRefused
    }
}

/// `n` elements of `size` bytes in one byte slice: the blocks the `_bytes` functions sort.
struct ByteBlocks<'a> {
    data: &'a mut [u8],
    size: usize,
}

impl<'a> ByteBlocks<'a> {
    fn new(data: &'a mut [u8], size: usize) -> ByteBlocks<'a> {
        assert!(size > 0, "the element size must be positive");
        assert!(
            data.len().is_multiple_of(size),
            "the data must hold whole elements of {size} bytes"
        );
        ByteBlocks { data, size }
    }
}

/// The element moves both orders make (positions are element indices).
trait Blocks {
    type Elem: ?Sized;
    fn count(&self) -> usize;
    fn elem(&self, i: usize) -> &Self::Elem;
    fn swap_at(&mut self, i: usize, j: usize);
    /// The element in position `p` moves to position `q < p`, those of `q..p` up by one.
    fn shift_down(&mut self, q: usize, p: usize);
}

/// Copies between two stores of the same shape (the merge order's array and its copy).
trait Copyable: Blocks {
    fn put(&mut self, i: usize, from: &Self, j: usize);
    /// `len` elements from `from[j..]` into `self[i..]`.
    fn put_range(&mut self, i: usize, from: &Self, j: usize, len: usize);
    /// Rearranges the elements so that position `k` receives the element that was in
    /// `perm[k]` (`perm` a permutation; it is overwritten).
    fn permute(&mut self, perm: &mut [usize]);
}

impl<T> Blocks for [T] {
    type Elem = T;
    fn count(&self) -> usize {
        self.len()
    }
    fn elem(&self, i: usize) -> &T {
        &self[i]
    }
    fn swap_at(&mut self, i: usize, j: usize) {
        self.swap(i, j);
    }
    fn shift_down(&mut self, q: usize, p: usize) {
        self[q..=p].rotate_right(1);
    }
}

/// A position [`Copyable::permute`] has placed.
const PLACED: usize = usize::MAX;

impl<T: Copy> Copyable for [T] {
    fn put(&mut self, i: usize, from: &Self, j: usize) {
        self[i] = from[j];
    }
    fn put_range(&mut self, i: usize, from: &Self, j: usize, len: usize) {
        self[i..i + len].copy_from_slice(&from[j..j + len]);
    }
    fn permute(&mut self, perm: &mut [usize]) {
        for start in 0..perm.len() {
            if perm[start] == PLACED || perm[start] == start {
                continue;
            }
            let spare = self[start];
            let mut k = start;
            loop {
                let from = perm[k];
                perm[k] = PLACED;
                if from == start {
                    self[k] = spare;
                    break;
                }
                self[k] = self[from];
                k = from;
            }
        }
    }
}

impl Blocks for ByteBlocks<'_> {
    type Elem = [u8];
    fn count(&self) -> usize {
        self.data.len() / self.size
    }
    fn elem(&self, i: usize) -> &[u8] {
        &self.data[i * self.size..(i + 1) * self.size]
    }
    fn swap_at(&mut self, i: usize, j: usize) {
        if i == j {
            return;
        }
        let s = self.size;
        let (a, b) = (i.min(j), i.max(j));
        let (left, right) = self.data.split_at_mut(b * s);
        left[a * s..(a + 1) * s].swap_with_slice(&mut right[..s]);
    }
    fn shift_down(&mut self, q: usize, p: usize) {
        let s = self.size;
        self.data[q * s..(p + 1) * s].rotate_right(s);
    }
}

impl Copyable for ByteBlocks<'_> {
    fn put(&mut self, i: usize, from: &Self, j: usize) {
        let s = self.size;
        self.data[i * s..(i + 1) * s].copy_from_slice(&from.data[j * s..(j + 1) * s]);
    }
    fn put_range(&mut self, i: usize, from: &Self, j: usize, len: usize) {
        let s = self.size;
        self.data[i * s..(i + len) * s].copy_from_slice(&from.data[j * s..(j + len) * s]);
    }
    fn permute(&mut self, perm: &mut [usize]) {
        let s = self.size;
        let mut spare = vec![0u8; s];
        for start in 0..perm.len() {
            if perm[start] == PLACED || perm[start] == start {
                continue;
            }
            spare.copy_from_slice(&self.data[start * s..(start + 1) * s]);
            let mut k = start;
            loop {
                let from = perm[k];
                perm[k] = PLACED;
                if from == start {
                    self.data[k * s..(k + 1) * s].copy_from_slice(&spare);
                    break;
                }
                self.data.copy_within(from * s..(from + 1) * s, k * s);
                k = from;
            }
        }
    }
}

/// glibc's merge order (§4): sorts `v` in place, making exactly glibc's comparator calls in
/// glibc's order; a tie (a result ≤ 0) takes the left part's element, so the sort is stable for
/// a consistent comparator. Needs a copy of the slice (elements of more than [`DIRECT_LIMIT`]
/// bytes: two positions per element instead, as glibc's pointers).
pub fn merge_order<T: Copy, R: Comparison>(v: &mut [T], mut cmp: impl FnMut(&T, &T) -> R) {
    let n = v.len();
    if n < 2 {
        return;
    }
    let mut gt = |x: &T, y: &T| cmp(x, y).ordering() == Ordering::Greater;
    if size_of::<T>() <= DIRECT_LIMIT {
        let mut copy = v.to_vec();
        merge::sort(v, &mut copy[..], n, &mut gt);
    } else {
        let mut perm = merge::through_positions(&*v, n, &mut gt);
        v.permute(&mut perm);
    }
}

/// [`merge_order`] over `data.len() / size` elements of `size` bytes each.
///
/// # Panics
/// When `size` is 0 or `data` does not hold whole elements.
pub fn merge_order_bytes<R: Comparison>(
    data: &mut [u8],
    size: usize,
    mut cmp: impl FnMut(&[u8], &[u8]) -> R,
) {
    let mut blocks = ByteBlocks::new(data, size);
    let n = blocks.count();
    if n < 2 {
        return;
    }
    let mut gt = |x: &[u8], y: &[u8]| cmp(x, y).ordering() == Ordering::Greater;
    if size <= DIRECT_LIMIT {
        let mut copy = blocks.data.to_vec();
        let mut scratch = ByteBlocks::new(&mut copy, size);
        merge::sort(&mut blocks, &mut scratch, n, &mut gt);
    } else {
        let mut perm = merge::through_positions(&blocks, n, &mut gt);
        blocks.permute(&mut perm);
    }
}

/// glibc's quicksort order (§5, its fallback): sorts `v` in place with no buffer, making exactly
/// glibc's comparator calls in glibc's order.
///
/// # Errors
/// [`QuickOrderError`] when the next call would pass the comparator memory outside the array
/// (an inconsistent comparator), after exactly the calls before it; `v` is then in an
/// unspecified arrangement.
pub fn quick_order<T, R: Comparison>(
    v: &mut [T],
    mut cmp: impl FnMut(&T, &T) -> R,
) -> Result<(), QuickOrderError> {
    quick::sort(v, &mut |x: &T, y: &T| {
        cmp(x, y).ordering() == Ordering::Less
    })
}

/// [`quick_order`] over `data.len() / size` elements of `size` bytes each.
///
/// # Errors
/// As [`quick_order`].
///
/// # Panics
/// When `size` is 0 or `data` does not hold whole elements.
pub fn quick_order_bytes<R: Comparison>(
    data: &mut [u8],
    size: usize,
    mut cmp: impl FnMut(&[u8], &[u8]) -> R,
) -> Result<(), QuickOrderError> {
    let mut blocks = ByteBlocks::new(data, size);
    quick::sort(&mut blocks, &mut |x: &[u8], y: &[u8]| {
        cmp(x, y).ordering() == Ordering::Less
    })
}

/// glibc's `qsort` with its choice of §3 on `machine`: the merge order or the quicksort order,
/// as [`choose`] decides. Returns the choice.
///
/// # Errors
/// As [`quick_order`], when the quicksort order is chosen.
///
/// # Panics
/// When `machine.page_size` is 0.
pub fn qsort_as_glibc<T: Copy, R: Comparison>(
    v: &mut [T],
    cmp: impl FnMut(&T, &T) -> R,
    machine: &Machine,
) -> Result<Choice, QuickOrderError> {
    let choice = choose(v.len(), size_of::<T>(), machine);
    match choice.order() {
        Order::Merge => merge_order(v, cmp),
        Order::Quick => quick_order(v, cmp)?,
    }
    Ok(choice)
}

/// [`qsort_as_glibc`] over `data.len() / size` elements of `size` bytes each.
///
/// # Errors
/// As [`quick_order`], when the quicksort order is chosen.
///
/// # Panics
/// When `size` or `machine.page_size` is 0, or `data` does not hold whole elements.
pub fn qsort_as_glibc_bytes<R: Comparison>(
    data: &mut [u8],
    size: usize,
    cmp: impl FnMut(&[u8], &[u8]) -> R,
    machine: &Machine,
) -> Result<Choice, QuickOrderError> {
    assert!(size > 0, "the element size must be positive");
    let choice = choose(data.len() / size, size, machine);
    match choice.order() {
        Order::Merge => merge_order_bytes(data, size, cmp),
        Order::Quick => quick_order_bytes(data, size, cmp)?,
    }
    Ok(choice)
}

/// C's `qsort` as FreeSurfer's programs get it: [`merge_order`] (§3: the merge order on any
/// machine that can run them).
pub fn qsort<T: Copy, R: Comparison>(v: &mut [T], cmp: impl FnMut(&T, &T) -> R) {
    merge_order(v, cmp);
}

/// C's `qsort_r`: [`qsort`] with a comparator that receives the caller's context `ctx`,
/// unchanged, in every call (§2: the same arrangement and calls as `qsort` with that comparator).
pub fn qsort_r<T: Copy, C: ?Sized, R: Comparison>(
    v: &mut [T],
    ctx: &mut C,
    mut cmp: impl FnMut(&T, &T, &mut C) -> R,
) {
    merge_order(v, |x, y| cmp(x, y, ctx));
}
