// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The watershed from markers by the image foresting transform, ported from SciPy 1.18.1
//! `scipy/ndimage/src/ni_measure.c` (`NI_WatershedIFT`; the same file in SciPy 1.15.3) and
//! `scipy/ndimage/_measurements.py` (`watershed_ift`); BSD-3-Clause, the notice is in the
//! crate's root.
//!
//! SciPy's algorithm keeps one doubly linked queue per cost level, `0..=max(input)`. The markers
//! start in queue 0: the first marker of the array in C order alone, then each object marker
//! (`> 0`) at the queue's front and each background marker (`< 0`) at its end; every other
//! element has the cost `max(input) + 1`. The levels are emptied in increasing order, each from
//! its front. A dequeued element `v` is done; each neighbour `p` not done yet whose cost
//! `max(cost(v), |input(p) - input(v)|)` is below its own takes that cost and `v`'s label (read
//! from the output) and moves to the front of that cost's queue, or to its end when the label is
//! negative.
//!
//! [`watershed_ift_run`] runs it as SciPy does (`faithful`) or corrected, and reports what the
//! faithful run did that the corrected one does not ([`WatershedEvents`]):
//! - **Neighbours across an edge.** SciPy adds the structure's offsets to the C-order index and
//!   checks only that the sum lies in the array, so the last element of a row neighbours the
//!   first of the next and an axis of length 1 has neighbours along it. Corrected, a neighbour
//!   has every coordinate inside the array.
//! - **An element alone in its queue stays in it.** SciPy takes an element out of its queue
//!   when its cost drops only if one of its links is set (`if (p->next || p->prev)`), so an
//!   element alone in its queue is left there: the queue keeps pointing at it, and an element
//!   queued later at that level is linked to it, which can join two queues, cut elements off
//!   every queue (they are never processed) or process them at the wrong level. Corrected, an
//!   element in a queue always leaves it.
//! - **32-bit labels.** SciPy reads markers and labels as C `int`s: a marker beyond 32 bits is
//!   truncated (its value, and its sign, change). Corrected, labels keep their 64 bits.
//! - **Byte offsets in `int`.** SciPy's input and output offsets are C `int`s of bytes, so above
//!   2^31 bytes it reads and writes outside its arrays; larmorx's offsets are 64-bit in both
//!   modes (an out-of-bounds access is never reproduced) and the event says SciPy's would be
//!   outside.
//!
//! Labels are stored in the output's signed type (C's conversion, wrapping) in both modes, as
//! SciPy stores them; the sign that decides a neighbour's place in its queue is the stored
//! label's, while the markers' places are decided by the markers' own values.

use crate::{Footprint, NdimageError, c_strides, unravel};

/// An input element type of the watershed (SciPy takes `uint8` and `uint16` only).
pub trait WatershedInput: Copy + Send + Sync {
    /// The value as SciPy reads it (`npy_intp`, then `int`).
    fn level(self) -> i32;
}

impl WatershedInput for u8 {
    #[inline]
    fn level(self) -> i32 {
        i32::from(self)
    }
}

impl WatershedInput for u16 {
    #[inline]
    fn level(self) -> i32 {
        i32::from(self)
    }
}

/// A marker element type (SciPy's wrapper takes the signed integers only).
pub trait MarkerValue: Copy + Send + Sync {
    /// The marker's value.
    fn value(self) -> i64;
}

macro_rules! marker_value {
    ($($t:ty),*) => {$(
        impl MarkerValue for $t {
            #[inline]
            fn value(self) -> i64 {
                self as i64
            }
        }
    )*};
}
marker_value!(i8, i16, i32, i64);

/// What a faithful run did that a corrected one does not (each may or may not change the
/// labels; [`watershed_ift`] counts the elements whose labels differ).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WatershedEvents {
    /// An element's cost and label were set through a neighbour across an edge of the array.
    pub across_edge: bool,
    /// An element alone in its queue had its cost lowered and stayed in that queue.
    pub stale_queue: bool,
    /// A marker beyond 32 bits was truncated.
    pub truncated: bool,
    /// SciPy's byte offsets would overflow its `int`s (more than 2^31 bytes in the input or the
    /// output): SciPy reads and writes outside its arrays.
    pub oversize: bool,
}

/// A watershed's labels (C-ordered, in the output's type's range) and its events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watershed {
    /// The labels.
    pub labels: Vec<i64>,
    /// What the faithful run did differently (all false for a corrected run).
    pub events: WatershedEvents,
}

/// `v` stored into the output's signed integer type of `bits` bits (C's conversion: wrapping).
#[inline]
fn store(v: i64, bits: u32) -> i64 {
    match bits {
        8 => v as i8 as i64,
        16 => v as i16 as i64,
        32 => v as i32 as i64,
        _ => v,
    }
}

/// An index into the queues' links, with a null value.
trait Slot: Copy + Eq + Send + Sync {
    const NONE: Self;
    fn of(i: usize) -> Self;
    fn at(self) -> usize;
}

impl Slot for u32 {
    const NONE: u32 = u32::MAX;
    #[inline]
    fn of(i: usize) -> u32 {
        i as u32
    }
    #[inline]
    fn at(self) -> usize {
        self as usize
    }
}

impl Slot for usize {
    const NONE: usize = usize::MAX;
    #[inline]
    fn of(i: usize) -> usize {
        i
    }
    #[inline]
    fn at(self) -> usize {
        self
    }
}

/// SciPy's queues: the elements' links and each level's first and last element.
struct Queues<S: Slot> {
    next: Vec<S>,
    prev: Vec<S>,
    first: Vec<S>,
    last: Vec<S>,
}

impl<S: Slot> Queues<S> {
    fn new(size: usize, levels: usize) -> Self {
        Queues {
            next: vec![S::NONE; size],
            prev: vec![S::NONE; size],
            first: vec![S::NONE; levels],
            last: vec![S::NONE; levels],
        }
    }

    /// `p->next = first[l]; p->prev = NULL; ...; first[l] = p; if (!last[l]) last[l] = p;`
    #[inline]
    fn push_front(&mut self, l: usize, p: usize) {
        let f = self.first[l];
        self.next[p] = f;
        self.prev[p] = S::NONE;
        if f != S::NONE {
            self.prev[f.at()] = S::of(p);
        }
        self.first[l] = S::of(p);
        if self.last[l] == S::NONE {
            self.last[l] = S::of(p);
        }
    }

    /// `p->prev = last[l]; p->next = NULL; ...; last[l] = p; if (!first[l]) first[l] = p;`
    #[inline]
    fn push_back(&mut self, l: usize, p: usize) {
        let t = self.last[l];
        self.prev[p] = t;
        self.next[p] = S::NONE;
        if t != S::NONE {
            self.next[t.at()] = S::of(p);
        }
        self.last[l] = S::of(p);
        if self.first[l] == S::NONE {
            self.first[l] = S::of(p);
        }
    }

    /// SciPy's removal of `p` from queue `l` (its links are left as they are).
    #[inline]
    fn unlink(&mut self, l: usize, p: usize) {
        let (pr, nx) = (self.prev[p], self.next[p]);
        if self.first[l] == S::of(p) {
            self.first[l] = nx;
        }
        if self.last[l] == S::of(p) {
            self.last[l] = pr;
        }
        if pr != S::NONE {
            self.next[pr.at()] = nx;
        }
        if nx != S::NONE {
            self.prev[nx.at()] = pr;
        }
    }

    /// The first element of queue `l`, dequeued (its links cleared), as SciPy dequeues it.
    #[inline]
    fn pop(&mut self, l: usize) -> Option<usize> {
        let v = self.first[l];
        if v == S::NONE {
            return None;
        }
        let v = v.at();
        let nx = self.next[v];
        self.first[l] = nx;
        if nx != S::NONE {
            self.prev[nx.at()] = S::NONE;
        }
        self.prev[v] = S::NONE;
        self.next[v] = S::NONE;
        Some(v)
    }
}

/// `scipy.ndimage.watershed_ift(input, markers, structure)` with an output of `output_bits`
/// bits (8, 16, 32 or 64, signed), the arrays C-ordered: as SciPy computes it (`faithful`) or
/// corrected (the module's text). `structure` has `3` elements along each of the input's axes.
pub fn watershed_ift_run<T: WatershedInput, M: MarkerValue>(
    input: &[T],
    shape: &[usize],
    markers: &[M],
    structure: &Footprint,
    output_bits: u32,
    faithful: bool,
) -> Result<Watershed, NdimageError> {
    let size: usize = shape.iter().product();
    if input.len() != size || markers.len() != size {
        return Err(NdimageError::Shape(
            "input and markers must have equal shape".into(),
        ));
    }
    let ndim = shape.len();
    if structure.shape.len() != ndim {
        return Err(NdimageError::Shape(
            "structure and input must have equal rank".into(),
        ));
    }
    if structure.shape.iter().any(|&s| s != 3) {
        return Err(NdimageError::Shape(
            "structure dimensions must be equal to 3".into(),
        ));
    }
    if ndim > 7 {
        return Err(NdimageError::Shape("too many dimensions".into()));
    }
    if !matches!(output_bits, 8 | 16 | 32 | 64) {
        return Err(NdimageError::Shape(
            "the output must be a signed integer type".into(),
        ));
    }
    if size < u32::MAX as usize {
        run::<T, M, u32>(input, shape, markers, structure, output_bits, faithful)
    } else {
        run::<T, M, usize>(input, shape, markers, structure, output_bits, faithful)
    }
}

/// [`watershed_ift_run`] in the chosen mode, with the number of elements whose label the other
/// mode gives differently (the corrected run and the faithful one are both made; the events
/// are the faithful run's).
pub fn watershed_ift<T: WatershedInput, M: MarkerValue>(
    input: &[T],
    shape: &[usize],
    markers: &[M],
    structure: &Footprint,
    output_bits: u32,
    faithful: bool,
) -> Result<(Watershed, usize), NdimageError> {
    let scipy = watershed_ift_run(input, shape, markers, structure, output_bits, true)?;
    let fixed = watershed_ift_run(input, shape, markers, structure, output_bits, false)?;
    let differ = scipy
        .labels
        .iter()
        .zip(&fixed.labels)
        .filter(|(a, b)| a != b)
        .count();
    if faithful {
        Ok((scipy, differ))
    } else {
        Ok((
            Watershed {
                labels: fixed.labels,
                events: scipy.events,
            },
            differ,
        ))
    }
}

fn run<T: WatershedInput, M: MarkerValue, S: Slot>(
    input: &[T],
    shape: &[usize],
    markers: &[M],
    structure: &Footprint,
    bits: u32,
    faithful: bool,
) -> Result<Watershed, NdimageError> {
    let size = input.len();
    let ndim = shape.len();
    let mut events = WatershedEvents::default();
    let item = std::mem::size_of::<T>().max(bits as usize / 8);
    if faithful && size > 0 && (size - 1).saturating_mul(item) > i32::MAX as usize {
        events.oversize = true;
    }
    // The maximum of the input (0 for an empty one) and one queue per level.
    let maxval = input.iter().map(|v| v.level()).max().unwrap_or(0).max(0) as usize;
    let mut q = Queues::<S>::new(size, maxval + 1);
    let mut cost = vec![0u32; size];
    let mut done = vec![false; size];
    let mut labels = vec![0i64; size];
    // The markers: queue 0, objects at its front, background at its end.
    for jj in 0..size {
        let m = markers[jj].value();
        let label = if faithful { m as i32 as i64 } else { m };
        if label != m {
            events.truncated = true;
        }
        labels[jj] = store(label, bits);
        if label != 0 {
            cost[jj] = 0;
            if label > 0 {
                q.push_front(0, jj);
            } else {
                q.push_back(0, jj);
            }
        } else {
            cost[jj] = (maxval + 1) as u32;
        }
    }
    // The neighbours: the structure's true elements but its centre, in C order, as offsets of
    // the C-order index (SciPy: `int` sums, the zero offsets of axes of length 1 dropped and
    // zeros left at the end) with their coordinate steps.
    let strides = c_strides(shape);
    let ssize = structure.mask.len();
    let centre = ssize / 2;
    let nneigh = (0..ssize)
        .filter(|&k| structure.mask[k] && k != centre)
        .count();
    let mut offsets: Vec<i64> = vec![0; nneigh];
    let mut steps: Vec<Vec<i64>> = vec![vec![0; ndim]; nneigh];
    let mut k3 = vec![0usize; ndim];
    let mut jj = 0usize;
    for kk in 0..ssize {
        if !structure.mask[kk] {
            continue;
        }
        unravel(kk, &structure.shape, &mut k3);
        let step: Vec<i64> = k3.iter().map(|&c| c as i64 - 1).collect();
        if faithful {
            let mut offset: i32 = 0;
            for ll in 0..ndim {
                offset = (i64::from(offset) + step[ll] * strides[ll] as i64) as i32;
            }
            if offset != 0 {
                offsets[jj] = i64::from(offset);
                steps[jj] = step;
                jj += 1;
            }
        } else if kk != centre {
            offsets[jj] = step
                .iter()
                .zip(&strides)
                .map(|(&s, &st)| s * st as i64)
                .sum();
            steps[jj] = step;
            jj += 1;
        }
    }
    let used = jj;
    let mut coords = vec![0usize; ndim];
    // The propagation.
    for level in 0..=maxval {
        while let Some(v) = q.pop(level) {
            done[v] = true;
            let vcost = cost[v];
            let vval = input[v].level();
            if !faithful || !events.across_edge {
                unravel(v, shape, &mut coords);
            }
            for hh in 0..nneigh {
                let p = if faithful {
                    let pi = v as i64 + offsets[hh];
                    if pi < 0 || pi >= size as i64 {
                        continue;
                    }
                    pi as usize
                } else {
                    if hh >= used {
                        break;
                    }
                    let inside = steps[hh].iter().enumerate().all(|(d, &s)| {
                        let c = coords[d] as i64 + s;
                        c >= 0 && c < shape[d] as i64
                    });
                    if !inside {
                        continue;
                    }
                    (v as i64 + offsets[hh]) as usize
                };
                if done[p] {
                    continue;
                }
                let wvp = (input[p].level() - vval).unsigned_abs();
                let pcost = cost[p];
                let max = if vcost > wvp { vcost } else { wvp };
                if max >= pcost {
                    continue;
                }
                if faithful && !events.across_edge && hh < used {
                    let across = steps[hh].iter().enumerate().any(|(d, &s)| {
                        let c = coords[d] as i64 + s;
                        c < 0 || c >= shape[d] as i64
                    });
                    if across {
                        events.across_edge = true;
                    }
                }
                cost[p] = max;
                let label = if faithful {
                    labels[v] as i32 as i64
                } else {
                    labels[v]
                };
                labels[p] = store(label, bits);
                let pl = pcost as usize;
                if faithful {
                    // (an element with links is in a queue of a level: `pl <= maxval`)
                    if (q.next[p] != S::NONE || q.prev[p] != S::NONE) && pl <= maxval {
                        q.unlink(pl, p);
                    } else if pl <= maxval && q.first[pl] == S::of(p) {
                        events.stale_queue = true;
                    }
                } else if pl <= maxval {
                    // not done and with a cost of a level: in that level's queue
                    q.unlink(pl, p);
                }
                if label < 0 {
                    q.push_back(max as usize, p);
                } else {
                    q.push_front(max as usize, p);
                }
            }
        }
    }
    Ok(Watershed { labels, events })
}
