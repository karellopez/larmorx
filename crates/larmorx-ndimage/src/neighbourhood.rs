// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The offsets of a footprint's elements around every position, with SciPy's boundary rules
//! (`NI_InitFilterOffsets`, `scipy/ndimage/src/ni_support.c`, SciPy 1.18.1; BSD-3-Clause, the
//! notice is in the crate's root).
//!
//! SciPy computes one table of offsets per border region and switches between them as its
//! iterator moves; the value read for a footprint element at a position is always the input at
//! the element's coordinate mapped by the mode, axis by axis, or `cval` (the constant modes) when
//! one axis falls outside. This module computes the same mapping per axis once (a table of
//! `length × footprint length` coordinates) and adds the strides.

use crate::{Extend, Footprint, Layout};

/// The flag of a coordinate outside the array in the constant modes.
pub(crate) const BORDER: usize = usize::MAX;

/// `NI_InitFilterOffsets`' boundary condition of one axis: the coordinate `cc` (relative to the
/// axis' start) mapped into `[0, len)`, or [`BORDER`].
pub(crate) fn map_coordinate(mut cc: isize, len: usize, mode: Extend) -> usize {
    let len_i = len as isize;
    match mode {
        Extend::Mirror => {
            if cc < 0 {
                if len <= 1 {
                    cc = 0;
                } else {
                    let sz2 = 2 * len_i - 2;
                    cc += sz2 * (-cc / sz2);
                    cc = if cc <= 1 - len_i { cc + sz2 } else { -cc };
                }
            } else if cc >= len_i {
                if len <= 1 {
                    cc = 0;
                } else {
                    let sz2 = 2 * len_i - 2;
                    cc -= sz2 * (cc / sz2);
                    if cc >= len_i {
                        cc = sz2 - cc;
                    }
                }
            }
        }
        Extend::Reflect => {
            if cc < 0 {
                if len <= 1 {
                    cc = 0;
                } else {
                    let sz2 = 2 * len_i;
                    if cc < -sz2 {
                        cc += sz2 * (-cc / sz2);
                    }
                    // SciPy: `cc < -len ? cc + sz2 : -cc - 1`, which gives -1 where the reduction
                    // left 0 (cc a multiple of 2·sz2 below 0); the reflection's value is index 0.
                    cc = if cc == 0 {
                        0
                    } else if cc < -len_i {
                        cc + sz2
                    } else {
                        -cc - 1
                    };
                }
            } else if cc >= len_i {
                if len <= 1 {
                    cc = 0;
                } else {
                    let sz2 = 2 * len_i;
                    cc -= sz2 * (cc / sz2);
                    if cc >= len_i {
                        cc = sz2 - cc - 1;
                    }
                }
            }
        }
        Extend::Wrap => {
            if cc < 0 {
                if len <= 1 {
                    cc = 0;
                } else {
                    cc += len_i * (-cc / len_i);
                    if cc < 0 {
                        cc += len_i;
                    }
                }
            } else if cc >= len_i {
                if len <= 1 {
                    cc = 0;
                } else {
                    cc -= len_i * (cc / len_i);
                }
            }
        }
        Extend::Nearest => {
            if cc < 0 {
                cc = 0;
            } else if cc >= len_i {
                cc = len_i - 1;
            }
        }
        Extend::Constant => {
            if cc < 0 || cc >= len_i {
                return BORDER;
            }
        }
    }
    cc as usize
}

/// A footprint's true elements around every position of an input layout.
pub(crate) struct Neighbourhood {
    ndim: usize,
    /// The input's shape and strides.
    shape: Vec<usize>,
    strides: Vec<usize>,
    /// Per element, its footprint coordinate on every axis (`elems[e * ndim + d]`).
    elems: Vec<usize>,
    /// Per element, its offset relative to the position where no axis crosses a border.
    rel: Vec<isize>,
    /// Per axis, the footprint's length and centre (`fshape / 2 + origin`; outside the
    /// footprint for the binary operations' unchecked origins).
    fshape: Vec<usize>,
    centre: Vec<isize>,
    /// Per axis, the mapped coordinate of every (position, footprint coordinate).
    tables: Vec<Vec<usize>>,
}

impl Neighbourhood {
    /// The neighbourhood of `footprint` (checked shapes, valid origins) over `layout`.
    pub(crate) fn new(
        layout: &Layout,
        footprint: &Footprint,
        origins: &[isize],
        mode: Extend,
    ) -> Neighbourhood {
        let ndim = layout.ndim();
        let fshape = footprint.shape.clone();
        let centre: Vec<isize> = fshape
            .iter()
            .zip(origins)
            .map(|(&f, &o)| (f / 2) as isize + o)
            .collect();
        let mut elems = Vec::new();
        let mut rel = Vec::new();
        let mut k = vec![0usize; ndim];
        for (lin, &on) in footprint.mask.iter().enumerate() {
            if !on {
                continue;
            }
            crate::unravel(lin, &fshape, &mut k);
            let mut r = 0isize;
            for d in 0..ndim {
                r += (k[d] as isize - centre[d]) * layout.strides[d] as isize;
            }
            elems.extend_from_slice(&k);
            rel.push(r);
        }
        let tables = (0..ndim)
            .map(|d| {
                let (len, fs, c) = (layout.shape[d], fshape[d], centre[d]);
                let mut t = Vec::with_capacity(len * fs);
                for p in 0..len {
                    for kd in 0..fs {
                        t.push(map_coordinate(p as isize + kd as isize - c, len, mode));
                    }
                }
                t
            })
            .collect();
        Neighbourhood {
            ndim,
            shape: layout.shape.clone(),
            strides: layout.strides.clone(),
            elems,
            rel,
            fshape,
            centre,
            tables,
        }
    }

    /// The number of true footprint elements.
    pub(crate) fn len(&self) -> usize {
        self.rel.len()
    }

    /// The elements that stay inside the array on every axis but the last at the line through
    /// `p` (constant modes): their relative offsets (`rel`) and displacements along the last axis
    /// (`dx`), in the footprint's order; returns whether any element falls outside on those axes.
    pub(crate) fn line_elements(
        &self,
        p: &[usize],
        rel: &mut Vec<isize>,
        dx: &mut Vec<isize>,
    ) -> bool {
        rel.clear();
        dx.clear();
        let last = self.ndim - 1;
        let mut outside = false;
        for e in 0..self.rel.len() {
            let k = &self.elems[e * self.ndim..(e + 1) * self.ndim];
            let inside = (0..last).all(|d| {
                let c = p[d] as isize + k[d] as isize - self.centre[d];
                c >= 0 && c < self.shape[d] as isize
            });
            if inside {
                rel.push(self.rel[e]);
                dx.push(k[last] as isize - self.centre[last]);
            } else {
                outside = true;
            }
        }
        outside
    }

    /// The input offset of position `p`.
    #[inline]
    pub(crate) fn base(&self, p: &[usize]) -> usize {
        p.iter().zip(&self.strides).map(|(i, s)| i * s).sum()
    }

    /// Every true element's offset relative to the position, in the footprint's order (valid
    /// where [`Neighbourhood::interior`] holds).
    #[inline]
    pub(crate) fn relative(&self) -> &[isize] {
        &self.rel
    }

    /// Whether the footprint around `p` stays inside the array on every axis.
    #[inline]
    pub(crate) fn interior(&self, p: &[usize]) -> bool {
        (0..self.ndim).all(|d| {
            let p = p[d] as isize;
            p - self.centre[d] >= 0
                && p + (self.fshape[d] as isize - 1 - self.centre[d]) < self.shape[d] as isize
        })
    }

    /// The positions `x` along the last axis where the footprint around `p` (its last
    /// coordinate replaced by `x`) stays inside the array on every axis: `lo..hi` (empty when
    /// another axis crosses a border).
    #[inline]
    pub(crate) fn line_interior(&self, p: &[usize]) -> (usize, usize) {
        let last = self.ndim - 1;
        let others = (0..last).all(|d| {
            let p = p[d] as isize;
            p - self.centre[d] >= 0
                && p + (self.fshape[d] as isize - 1 - self.centre[d]) < self.shape[d] as isize
        });
        let lo = self.centre[last].max(0);
        let hi = self.shape[last] as isize - (self.fshape[last] as isize - 1 - self.centre[last]);
        if !others || hi <= lo {
            return (0, 0);
        }
        (lo as usize, hi as usize)
    }

    /// The last axis' stride.
    #[inline]
    pub(crate) fn last_stride(&self) -> usize {
        self.strides[self.ndim - 1]
    }

    /// The 3-D footprint's mapped coordinates around `p` on each axis (`[k]` for footprint
    /// coordinate `k`, [`BORDER`] outside in the constant modes), with the input's strides.
    #[inline]
    pub(crate) fn axes3(&self, p: &[usize]) -> ([&[usize]; 3], [usize; 3]) {
        let t = |d: usize| {
            let f = self.fshape[d];
            &self.tables[d][p[d] * f..(p[d] + 1) * f]
        };
        (
            [t(0), t(1), t(2)],
            [self.strides[0], self.strides[1], self.strides[2]],
        )
    }

    /// The input offset of every element around `p` (`out[e]`, [`BORDER`] outside in the
    /// constant modes).
    #[inline]
    pub(crate) fn offsets(&self, p: &[usize], out: &mut [usize]) {
        if self.interior(p) {
            let base = self.base(p);
            for (o, &r) in out.iter_mut().zip(&self.rel) {
                *o = (base as isize + r) as usize;
            }
            return;
        }
        for (e, o) in out.iter_mut().enumerate() {
            let k = &self.elems[e * self.ndim..(e + 1) * self.ndim];
            let mut off = 0usize;
            let mut outside = false;
            for d in 0..self.ndim {
                let c = self.tables[d][p[d] * self.fshape[d] + k[d]];
                if c == BORDER {
                    outside = true;
                    break;
                }
                off += c * self.strides[d];
            }
            *o = if outside { BORDER } else { off };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_rules() {
        // length 4: reflect d c b a | a b c d | d c b a
        let r: Vec<usize> = (-6..10)
            .map(|c| map_coordinate(c, 4, Extend::Reflect))
            .collect();
        assert_eq!(r, [2, 3, 3, 2, 1, 0, 0, 1, 2, 3, 3, 2, 1, 0, 0, 1]);
        let m: Vec<usize> = (-4..8)
            .map(|c| map_coordinate(c, 4, Extend::Mirror))
            .collect();
        assert_eq!(m, [2, 3, 2, 1, 0, 1, 2, 3, 2, 1, 0, 1]);
        let w: Vec<usize> = (-5..9)
            .map(|c| map_coordinate(c, 4, Extend::Wrap))
            .collect();
        assert_eq!(w, [3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0]);
        assert_eq!(map_coordinate(-1, 4, Extend::Constant), BORDER);
        assert_eq!(map_coordinate(5, 4, Extend::Nearest), 3);
        // SciPy's reflect maps -16 (two periods of 8) to -1; larmorx to 0
        assert_eq!(map_coordinate(-16, 4, Extend::Reflect), 0);
        assert_eq!(map_coordinate(-8, 4, Extend::Reflect), 0);
        assert_eq!(map_coordinate(-9, 4, Extend::Reflect), 0);
    }
}
