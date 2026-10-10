// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Spline interpolation at arbitrary coordinates: the coordinate-array path of SciPy 1.15.2's
//! `NI_GeometricTransform` (`scipy/ndimage/src/ni_interpolation.c`) and the prefiltering of
//! `map_coordinates` (`scipy/ndimage/_interpolation.py`).
//!
//! The per-point evaluation keeps SciPy's order exactly: the footprint is visited with the
//! last axis fastest, each coefficient is multiplied by the weights of axis 0, 1, ... in turn,
//! and the products are added to a sum that starts at zero.
//!
//! ni_interpolation.c: Copyright (C) 2003-2005 Peter J. Verveer
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. The name of the author may not be used to endorse or promote products derived from this
//!    software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES,
//! INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
//! PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
//! INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
//! LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
//! BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//! STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
//! USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use super::splines::interpolation_weights;
use super::{
    Extend, Mode, NdimageError, Sample, c_cast_i64, check_shape, filter_axis, floor, map_coordinate,
};

/// An input prepared for interpolation at arbitrary coordinates: SciPy's prepadding and
/// prefilter applied once, so that every later [`Spline::sample`] call reads coefficients.
#[derive(Clone, Debug)]
pub struct Spline<const D: usize> {
    /// The (padded, prefiltered) coefficients, Fortran order, as doubles.
    coefficients: Vec<f64>,
    /// Shape of `coefficients` (the input's plus the prepadding).
    dims: [i64; D],
    strides: [i64; D],
    order: u32,
    mode: Extend,
    spline_mode: Extend,
    cval: f64,
    npad: f64,
}

impl<const D: usize> Spline<D> {
    /// Prepares `input` (Fortran order, `shape`) as `map_coordinates` does before
    /// interpolating: with `prefilter` and `order > 1`, `nearest` and `grid-constant` pad 12
    /// voxels (edge values or `cval`), then the float64 spline filter runs along every axis
    /// with `mode`'s boundary rule. `parallel` filters lines on the current rayon pool.
    pub fn new<T: Sample>(
        input: &[T],
        shape: [usize; D],
        order: u32,
        mode: Mode,
        cval: f64,
        prefilter: bool,
        parallel: bool,
    ) -> Result<Self, NdimageError> {
        if order > 5 {
            return Err(NdimageError::Order(order));
        }
        if D == 0 {
            return Err(NdimageError::Coordinates);
        }
        check_shape(input.len(), &shape)?;
        if shape.contains(&0) {
            // numpy cannot pad an empty axis, and SciPy reads nothing from one.
            return Err(NdimageError::Shape {
                len: input.len(),
                shape: shape.to_vec(),
            });
        }
        let filtered = prefilter && order > 1;
        let npad = if filtered { mode.prepad() } else { 0 };
        let padded: [usize; D] = std::array::from_fn(|d| shape[d] + 2 * npad);
        let mut coefficients = if npad == 0 {
            input.iter().map(|v| v.to_f64()).collect::<Vec<f64>>()
        } else {
            pad(input, shape, npad, mode, cval)
        };
        if filtered {
            for axis in 0..D {
                filter_axis(
                    &mut coefficients,
                    &padded,
                    axis,
                    order,
                    mode.extend(),
                    parallel,
                );
            }
        }
        let mut strides = [0i64; D];
        let mut s = 1i64;
        for d in 0..D {
            strides[d] = s;
            s *= padded[d] as i64;
        }
        let extend = mode.extend();
        Ok(Spline {
            coefficients,
            dims: padded.map(|n| n as i64),
            strides,
            order,
            mode: extend,
            // `_get_spline_boundary_mode`: constant and wrap use mirror for the footprint.
            spline_mode: match extend {
                Extend::Constant | Extend::Wrap => Extend::Mirror,
                m => m,
            },
            cval,
            npad: npad as f64,
        })
    }

    /// The prepared coefficients (Fortran order) and their shape.
    pub fn coefficients(&self) -> (&[f64], [usize; D]) {
        (&self.coefficients, self.dims.map(|n| n as usize))
    }

    /// The interpolated value at `coords` (array indices of the original input), as the double
    /// SciPy stores into its output (before the conversion to the output type).
    #[inline]
    pub fn sample(&self, coords: [f64; D]) -> f64 {
        let order = self.order as usize;
        let grid_constant = self.mode == Extend::GridConstant;
        let mut weights = [[0.0f64; 6]; D];
        // Per axis: the absolute coefficient index of each footprint position, and whether it
        // lies outside the array (grid-constant).
        let mut index = [[0i64; 6]; D];
        let mut outside = [[false; 6]; D];
        let mut any_outside = false;
        for hh in 0..D {
            let mut cc = coords[hh] + self.npad;
            let len = self.dims[hh];
            if self.mode != Extend::GridConstant && self.mode != Extend::Nearest {
                cc = map_coordinate(cc, len, self.mode);
            }
            if cc > -1.0 || self.mode == Extend::GridConstant || self.mode == Extend::Nearest {
                let start = if order & 1 == 1 {
                    c_cast_i64(floor(cc))
                } else {
                    c_cast_i64(floor(cc + 0.5))
                }
                .wrapping_sub((order / 2) as i64);
                if grid_constant {
                    for ll in 0..=order {
                        let idx = start.wrapping_add(ll as i64);
                        index[hh][ll] = idx;
                        let out = idx < 0 || idx >= len;
                        outside[hh][ll] = out;
                        any_outside |= out;
                    }
                } else if start < 0 || start.wrapping_add(order as i64) >= len {
                    for ll in 0..=order {
                        let idx = start.wrapping_add(ll as i64);
                        index[hh][ll] =
                            c_cast_i64(map_coordinate(idx as f64, len, self.spline_mode));
                    }
                } else {
                    for ll in 0..=order {
                        index[hh][ll] = start.wrapping_add(ll as i64);
                    }
                }
                interpolation_weights(cc, self.order, &mut weights[hh]);
            } else {
                // The constant border condition.
                return self.cval;
            }
        }
        self.sum(&index, &outside, any_outside, &weights)
    }

    /// The weighted sum over the footprint, last axis fastest.
    #[inline(always)]
    fn sum(
        &self,
        index: &[[i64; 6]; D],
        outside: &[[bool; 6]; D],
        any_outside: bool,
        weights: &[[f64; 6]; D],
    ) -> f64 {
        let order = self.order as usize;
        let mut pos = [0usize; D];
        let mut t = 0.0f64;
        loop {
            let mut coeff;
            let is_cval = any_outside && (0..D).any(|ll| outside[ll][pos[ll]]);
            if is_cval {
                coeff = self.cval;
            } else {
                let mut idx = 0i64;
                let mut valid = true;
                for ll in 0..D {
                    let i = index[ll][pos[ll]];
                    valid &= i >= 0 && i < self.dims[ll];
                    idx = idx.wrapping_add(i.wrapping_mul(self.strides[ll]));
                }
                if !valid {
                    // SciPy would read outside its buffer (NaN or huge coordinates).
                    return f64::NAN;
                }
                coeff = self.coefficients[idx as usize];
            }
            if order > 0 {
                for ll in 0..D {
                    coeff *= weights[ll][pos[ll]];
                }
            }
            t += coeff;
            // Next footprint position, last axis fastest.
            let mut d = D;
            loop {
                if d == 0 {
                    return t;
                }
                d -= 1;
                if pos[d] < order {
                    pos[d] += 1;
                    break;
                }
                pos[d] = 0;
            }
        }
    }
}

/// Interior points interpolated together in [`Spline::sample_batch`].
const GROUP: usize = 4;

impl Spline<3> {
    /// `out[i] = self.sample(points[i])` for many 3D points, faster.
    ///
    /// Points whose whole footprint lies inside the coefficient array (and, for the modes that
    /// map coordinates, inside `[0, n - 1]`) are evaluated four at a time with their sums
    /// interleaved, which keeps every point's own order of operations, so the results are
    /// identical to [`Spline::sample`]. Other points take the general path.
    pub fn sample_batch(&self, points: &[[f64; 3]], out: &mut [f64]) {
        assert_eq!(points.len(), out.len(), "one output per point");
        match self.order {
            0 => self.batch::<0>(points, out),
            1 => self.batch::<1>(points, out),
            2 => self.batch::<2>(points, out),
            3 => self.batch::<3>(points, out),
            4 => self.batch::<4>(points, out),
            _ => self.batch::<5>(points, out),
        }
    }

    #[inline(always)]
    fn batch<const O: usize>(&self, points: &[[f64; 3]], out: &mut [f64]) {
        let mut bases = [0usize; GROUP];
        let mut weights = [[[0.0f64; 6]; 3]; GROUP];
        let mut slots = [0usize; GROUP];
        let mut n = 0;
        for (i, p) in points.iter().enumerate() {
            match self.interior::<O>(*p, &mut weights[n]) {
                Some(base) => {
                    bases[n] = base;
                    slots[n] = i;
                    n += 1;
                    if n == GROUP {
                        let t = self.sum_group::<O>(&bases, &weights);
                        for v in 0..GROUP {
                            out[slots[v]] = t[v];
                        }
                        n = 0;
                    }
                }
                None => out[i] = self.sample(*p),
            }
        }
        for v in 0..n {
            out[slots[v]] = self.sum_one::<O>(bases[v], &weights[v]);
        }
    }

    /// The base coefficient index and the weights of a point whose footprint needs no boundary
    /// handling, or `None`.
    #[inline(always)]
    fn interior<const O: usize>(&self, p: [f64; 3], w: &mut [[f64; 6]; 3]) -> Option<usize> {
        let maps = !matches!(self.mode, Extend::GridConstant | Extend::Nearest);
        let mut base = 0i64;
        for d in 0..3 {
            let cc = p[d] + self.npad;
            let len = self.dims[d];
            // Inside [0, n - 1], `map_coordinate` returns the coordinate unchanged.
            if maps && !(cc >= 0.0 && cc <= (len - 1) as f64) {
                return None;
            }
            let f = if O & 1 == 1 {
                floor(cc)
            } else {
                floor(cc + 0.5)
            };
            if !(f >= 0.0 && f < len as f64) {
                return None;
            }
            let start = f as i64 - (O / 2) as i64;
            if start < 0 || start + O as i64 >= len {
                return None;
            }
            base += start * self.strides[d];
            interpolation_weights(cc, O as u32, &mut w[d]);
        }
        Some(base as usize)
    }

    #[inline(always)]
    fn sum_group<const O: usize>(
        &self,
        bases: &[usize; GROUP],
        w: &[[[f64; 6]; 3]; GROUP],
    ) -> [f64; GROUP] {
        let c = &self.coefficients[..];
        let (s1, s2) = (self.strides[1] as usize, self.strides[2] as usize);
        // Weights by axis and tap, the four points side by side (vectorisable).
        let wt: [[[f64; GROUP]; 6]; 3] =
            std::array::from_fn(|d| std::array::from_fn(|k| std::array::from_fn(|v| w[v][d][k])));
        let mut t = [0.0f64; GROUP];
        for a in 0..=O {
            let w0 = wt[0][a];
            for b in 0..=O {
                let w1 = wt[1][b];
                let rows: [usize; GROUP] = std::array::from_fn(|v| bases[v] + a + b * s1);
                for e in 0..=O {
                    let w2 = wt[2][e];
                    let off = e * s2;
                    let mut coeff: [f64; GROUP] = std::array::from_fn(|v| c[rows[v] + off]);
                    if O > 0 {
                        for v in 0..GROUP {
                            coeff[v] *= w0[v];
                        }
                        for v in 0..GROUP {
                            coeff[v] *= w1[v];
                        }
                        for v in 0..GROUP {
                            coeff[v] *= w2[v];
                        }
                    }
                    for v in 0..GROUP {
                        t[v] += coeff[v];
                    }
                }
            }
        }
        t
    }

    #[inline(always)]
    fn sum_one<const O: usize>(&self, base: usize, w: &[[f64; 6]; 3]) -> f64 {
        let c = &self.coefficients;
        let (s1, s2) = (self.strides[1] as usize, self.strides[2] as usize);
        let mut t = 0.0f64;
        for a in 0..=O {
            for b in 0..=O {
                for e in 0..=O {
                    let mut coeff = c[base + a + b * s1 + e * s2];
                    if O > 0 {
                        coeff *= w[0][a];
                        coeff *= w[1][b];
                        coeff *= w[2][e];
                    }
                    t += coeff;
                }
            }
        }
        t
    }
}

/// `np.pad(input, npad, mode='constant', constant_values=cval)` (grid-constant) or
/// `mode='edge'` (nearest), read as doubles.
fn pad<T: Sample, const D: usize>(
    input: &[T],
    shape: [usize; D],
    npad: usize,
    mode: Mode,
    cval: f64,
) -> Vec<f64> {
    let padded: [usize; D] = std::array::from_fn(|d| shape[d] + 2 * npad);
    let total: usize = padded.iter().product();
    let fill = T::pad_value(cval);
    let nearest = mode == Mode::Nearest;
    let (nx, px) = (shape[0], padded[0]);
    let mut out = vec![fill; total];
    // Row by row along the first axis: the source row of each padded row (clamped for
    // `nearest`), or none (constant padding).
    let mut pos = [0usize; D];
    for row in out.chunks_mut(px) {
        let mut src = 0usize;
        let mut stride = nx;
        let mut inside = true;
        for d in 1..D {
            let p = pos[d] as i64 - npad as i64;
            let n = shape[d] as i64;
            if p < 0 || p >= n {
                inside = false;
            }
            src += p.clamp(0, n - 1) as usize * stride;
            stride *= shape[d];
        }
        if inside || nearest {
            let line = &input[src..src + nx];
            for (o, v) in row[npad..npad + nx].iter_mut().zip(line) {
                *o = v.to_f64();
            }
            if nearest {
                let (first, last) = (line[0].to_f64(), line[nx - 1].to_f64());
                row[..npad].fill(first);
                row[npad + nx..].fill(last);
            }
        }
        for d in 1..D {
            pos[d] += 1;
            if pos[d] < padded[d] {
                break;
            }
            pos[d] = 0;
        }
    }
    out
}
