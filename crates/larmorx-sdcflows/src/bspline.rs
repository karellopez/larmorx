// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The B-spline control grid and the tensor-product cubic B-spline weights of a sampling grid,
//! ported from sdcflows 2.17.1 (`sdcflows/interfaces/bspline.py`: `bspline_grid`;
//! `sdcflows/transform.py`: `grid_bspline_weights`; Apache-2.0) with SciPy's arithmetic
//! (`scipy/interpolate/src/__fitpack.cc`: `_find_interval`, `_deBoor_D`, `_evaluate_spline`;
//! `scipy/sparse/_construct.py`: `kron`, `hstack`; `scipy/sparse/sparsetools/csr.h` and
//! `csc.h`: `csr_matvec`, `csc_matvec`; SciPy 1.18.1, the same in 1.15.3 on these paths;
//! BSD-3-Clause, notice below).
//!
//! sdcflows evaluates, for each axis, every sample's knot coordinate, `BSpline(knots,
//! eye(K + 2), 3)` at it (de Boor's recursion in float64), keeps the knots within distance 2,
//! rounds to the matrix type (float32 by default) in a `lil_array`, which drops exact zeros, and
//! multiplies the three axes with `kron(kron(w0, w1), w2)`. scipy's `kron` goes through BSR when
//! its right operand has `2 * nnz >= rows * cols`, and then stores the right operand **densely**:
//! every knot of that axis, explicit zeros included. The matrix sdcflows then uses
//! (`sparse_hstack(levels).tocsr()`) has, in each row, the stored entries in ascending column
//! order. [`Collocation`] applies it (and its transpose) in exactly that order, from the
//! per-axis weights, without storing it; [`Collocation::to_csr`] stores it when a caller asks.
//!
//! Two defects of sdcflows' weights are corrected by default ([`UpstreamDefects::Fix`]; D13,
//! `docs/findings/sdcflows-fieldmap.md`), and reproduced bit for bit with
//! [`UpstreamDefects::Reproduce`]:
//! - **SDC-01**: a sample beyond the first or the last control point (a location outside
//!   `[0, K - 1]` within distance 2 of a kept knot) gets, through SciPy's extrapolation, the
//!   end interval's polynomial continued; corrected, the cubic B-spline's value (the same
//!   recursion on the uniform knots continued beyond the grid; inside `[0, K - 1]` the
//!   original's own intervals, so the same bits).
//! - **SDC-02**: on a sampling grid rotated relative to the control grid (sdcflows' alignment
//!   test fails: it warns and goes on), the tensor product of per-axis weights, each from one
//!   index of the voxel only, is not the field at the voxel; corrected, every sample's weights
//!   are the products of its three axes' weights at its own position ([`ExactLevel`]), in the
//!   same arithmetic (the location by nibabel's `apply_affine`, the values rounded to the matrix
//!   type, the products `(w0 * w1) * w2`).
//!
//! SciPy: Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers. All rights reserved.
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met: redistributions of source code
//! must retain the above copyright notice, this list of conditions and the following
//! disclaimer; redistributions in binary form must reproduce the above copyright notice, this
//! list of conditions and the following disclaimer in the documentation and/or other materials
//! provided with the distribution; neither the name of the copyright holder nor the names of
//! its contributors may be used to endorse or promote products derived from this software
//! without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT
//! HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//! LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY
//! DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
//! NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
//! OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
//! OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::linalg::Mat4;
use larmorx_core::parallel::with_threads;
use larmorx_transform::openblas::{inv4, matmul4};
use larmorx_transform::resample_series::{apply_affine, apply_affine_single};
use rayon::prelude::*;

use crate::SdcError;
use crate::npy;

/// sdcflows' default control-point spacings, in mm (`interfaces/bspline.py`).
pub const DEFAULT_ZOOMS_MM: [f64; 3] = [40.0, 40.0, 20.0];
/// Low-frequency spacings (`DEFAULT_LF_ZOOMS_MM`).
pub const DEFAULT_LF_ZOOMS_MM: [f64; 3] = [100.0, 100.0, 40.0];
/// High-frequency spacings (`DEFAULT_HF_ZOOMS_MM`), the ones fMRIPrep's field maps use.
pub const DEFAULT_HF_ZOOMS_MM: [f64; 3] = [16.0, 16.0, 10.0];

/// sdcflows' warning when the sampling grid is rotated relative to the control grid.
pub const NOT_ALIGNED: &str = "Image's and B-Spline's grids are not aligned.";

/// A B-spline control grid (sdcflows' coefficients image without its data): its shape and its
/// affine (voxel to RAS+ mm).
#[derive(Clone, Debug, PartialEq)]
pub struct ControlGrid {
    pub shape: [usize; 3],
    pub affine: Mat4,
}

/// sdcflows' `bspline_grid(img, control_zooms_mm)`: the control grid with the image's
/// orientation, `control_zooms_mm` between control points, covering the image's extent plus a
/// margin, centred on the image.
///
/// `zooms` are the image header's zooms, all of them (`img.header.get_zooms()`); sdcflows
/// divides the affine's 3×3 columns by them, so a header with other than three (or one) zooms
/// fails there with numpy's broadcasting error, which is returned.
pub fn bspline_grid(
    shape: [usize; 3],
    affine: &Mat4,
    zooms: &[f64],
    control_zooms_mm: [f64; 3],
) -> Result<ControlGrid, SdcError> {
    let zoom = |c: usize| -> f64 { if zooms.len() == 1 { zooms[0] } else { zooms[c] } };
    if zooms.len() != 3 && zooms.len() != 1 {
        return Err(SdcError::Invalid(format!(
            "operands could not be broadcast together with shapes (3,3) ({},) ",
            zooms.len()
        )));
    }
    // dir_cos = img.affine[:3, :3] / im_zooms; bs_affine[:3, :3] = control_zooms * dir_cos
    let mut bs: Mat4 = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for (r, row) in bs.iter_mut().enumerate().take(3) {
        for (c, v) in row.iter_mut().enumerate().take(3) {
            *v = control_zooms_mm[c] * (affine[r][c] / zoom(c));
        }
    }
    // bs_zooms = nb.affines.voxel_sizes(bs_affine): sqrt of the column sums of squares (rows
    // added in order).
    let bs_zooms: [f64; 3] = std::array::from_fn(|c| {
        ((bs[0][c] * bs[0][c] + bs[1][c] * bs[1][c]) + bs[2][c] * bs[2][c]).sqrt()
    });
    // im_extent = im_zooms * (im_shape - 1); bs_shape = (im_extent // bs_zooms + 3).astype(int)
    let mut bs_shape = [0usize; 3];
    for c in 0..3 {
        let extent = zoom(c) * (shape[c] as f64 - 1.0);
        let n = npy::floor_divide(extent, bs_zooms[c]) + 3.0;
        if !n.is_finite() || n < 0.0 {
            return Err(SdcError::Invalid(format!(
                "the control grid's size along axis {c} is not a valid count ({n})"
            )));
        }
        bs_shape[c] = n.trunc() as usize;
    }
    // Centre both grids: apply_affine(img.affine, 0.5 * (im_shape - 1))
    //                    - apply_affine(bs_affine, 0.5 * (bs_shape - 1))
    let half = |s: usize| 0.5 * (s as f64 - 1.0);
    let c_img = apply_affine_single(affine, [half(shape[0]), half(shape[1]), half(shape[2])]);
    let c_bs = apply_affine_single(
        &bs,
        [half(bs_shape[0]), half(bs_shape[1]), half(bs_shape[2])],
    );
    for r in 0..3 {
        bs[r][3] = c_img[r] - c_bs[r];
    }
    Ok(ControlGrid {
        shape: bs_shape,
        affine: bs,
    })
}

/// The type of the weights matrix (`grid_bspline_weights`' `dtype`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WeightType {
    #[default]
    Float32,
    Float64,
}

impl WeightType {
    fn round(self, v: f64) -> f64 {
        match self {
            WeightType::Float32 => f64::from(v as f32),
            WeightType::Float64 => v,
        }
    }

    /// `a * b` in the matrix type (numpy's element product of two arrays of that type).
    fn mul(self, a: f64, b: f64) -> f64 {
        match self {
            WeightType::Float32 => f64::from((a as f32) * (b as f32)),
            WeightType::Float64 => a * b,
        }
    }
}

/// SciPy's `_find_interval` for sdcflows' uniform knots `t[i] = i - 3`, `i = 0 .. K + 6`, with
/// extrapolation: the last `l` in `[3, K + 1]` with `t[l] <= x`, or 3 (`-1` for NaN). The search
/// result does not depend on where SciPy starts it.
fn find_interval(n_knots: usize, x: f64) -> Option<usize> {
    if x.is_nan() {
        return None;
    }
    let (k, n) = (3i64, n_knots as i64 + 2);
    // t[l] = l - 3 <= x  <=>  l <= floor(x) + 3
    let f = x.floor();
    let l = if f >= (n - 1 - 3) as f64 {
        n - 1
    } else if f < 0.0 {
        k
    } else {
        (f as i64 + 3).clamp(k, n - 1)
    };
    Some(l as usize)
}

/// The interval of `x` on sdcflows' uniform knots continued beyond the grid (`t[l] = l - 3` for
/// every integer `l`), for a location outside `[0, K - 1]`: `floor(x) + 3` (SDC-01's
/// correction: the cubic B-spline's own interval instead of the end one).
fn extended_interval(x: f64) -> i64 {
    x.floor() as i64 + 3
}

/// SciPy's `_deBoor_D(t, x, 3, ell, 0, result)` for sdcflows' uniform knots: the four cubic
/// B-splines `B[ell-3] .. B[ell]` at `x`, in float64, each operation as the C code does it.
/// `ell` may lie beyond SciPy's base intervals (the knots continued: SDC-01's correction).
fn deboor3(x: f64, ell: i64) -> [f64; 4] {
    let t = |i: i64| i as f64 - 3.0;
    let mut h = [0.0f64; 4];
    let mut hh = [0.0f64; 4];
    h[0] = 1.0;
    for j in 1..=3usize {
        hh[..j].copy_from_slice(&h[..j]);
        h[0] = 0.0;
        for n in 1..=j {
            let ind = ell + n as i64;
            let xb = t(ind);
            let xa = t(ind - j as i64);
            if xb == xa {
                h[n] = 0.0;
                continue;
            }
            let w = hh[n - 1] / (xb - xa);
            h[n - 1] += w * (xb - x);
            h[n] = w * (x - xa);
        }
    }
    h
}

/// The value of B-spline column `jp` (`BSpline(...)(x)[:, jp]`) at `x` in interval `ell`: the
/// recursion's value for the four splines non-zero there, `0.0 + 1 * w`, and +0 otherwise.
fn basis_value(work: &[f64; 4], ell: i64, jp: i64) -> f64 {
    if jp + 3 >= ell && jp <= ell {
        0.0 + work[(jp + 3 - ell) as usize]
    } else {
        0.0
    }
}

/// One sample's kept weights on one axis: the knots within distance 2 whose weight, rounded to
/// the matrix type, is not zero, ascending; and whether SDC-01 changes them (a location beyond
/// `[0, K - 1]` whose kept weights differ between the end polynomial and the B-spline).
#[derive(Clone, Copy, Debug, Default)]
struct SampleWeights {
    len: usize,
    knots: [usize; 4],
    values: [f64; 4],
    beyond: bool,
}

impl SampleWeights {
    fn of(x: f64, n_knots: usize, dtype: WeightType, defects: UpstreamDefects) -> SampleWeights {
        let mut out = SampleWeights::default();
        let Some(ell) = find_interval(n_knots, x) else {
            return out;
        };
        let ell = ell as i64;
        let inside = (0.0..=(n_knots as f64 - 1.0)).contains(&x);
        let work = deboor3(x, ell);
        let (ext_ell, ext) = if inside {
            (ell, None)
        } else {
            let e = extended_interval(x);
            (e, Some(deboor3(x, e)))
        };
        // distance = |locs - knots[3:-3]| (float32 knots, exact), within_support < 2: the
        // knots floor(x) - 1 ..= floor(x) + 2, those of the grid
        let f = x.floor();
        let lo = (f - 1.0).max(0.0);
        let hi = (f + 2.0).min(n_knots as f64 - 1.0);
        if !(lo <= hi) {
            return out;
        }
        for kk in (lo as usize)..=(hi as usize) {
            if (x - kk as f64).abs() >= 2.0 {
                continue;
            }
            let jp = kk as i64 + 1;
            let original = dtype.round(basis_value(&work, ell, jp));
            let v = match &ext {
                None => original,
                Some(w) => {
                    let corrected = dtype.round(basis_value(w, ext_ell, jp));
                    if corrected.to_bits() != original.to_bits() {
                        out.beyond = true;
                    }
                    if defects.fix() { corrected } else { original }
                }
            };
            if v != 0.0 {
                out.knots[out.len] = kk;
                out.values[out.len] = v;
                out.len += 1;
            }
        }
        out
    }
}

/// One axis' weights: for every sample, the knots within distance 2 whose weight, rounded to
/// the matrix type, is not zero (`lil_array` drops exact zeros), in ascending knot order.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisWeights {
    pub n_samples: usize,
    pub n_knots: usize,
    /// Row pointers into `knots` and `values` (`n_samples + 1`).
    pub indptr: Vec<usize>,
    pub knots: Vec<usize>,
    /// The weights, rounded to the matrix type (held as float64).
    pub values: Vec<f64>,
    /// How many samples lie beyond the control grid with weights SDC-01 changes.
    pub beyond: usize,
}

impl AxisWeights {
    /// The weights of samples at knot coordinates `locs` (sdcflows' `locs` for this axis), with
    /// SDC-01 corrected or reproduced (`defects`).
    pub fn from_locations(
        locs: &[f64],
        n_knots: usize,
        dtype: WeightType,
        defects: UpstreamDefects,
    ) -> AxisWeights {
        let mut indptr = Vec::with_capacity(locs.len() + 1);
        let mut knots = Vec::new();
        let mut values = Vec::new();
        let mut beyond = 0;
        indptr.push(0);
        for &x in locs {
            let w = SampleWeights::of(x, n_knots, dtype, defects);
            beyond += usize::from(w.beyond);
            knots.extend_from_slice(&w.knots[..w.len]);
            values.extend_from_slice(&w.values[..w.len]);
            indptr.push(knots.len());
        }
        AxisWeights {
            n_samples: locs.len(),
            n_knots,
            indptr,
            knots,
            values,
            beyond,
        }
    }

    pub fn nnz(&self) -> usize {
        self.knots.len()
    }

    fn row(&self, i: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let r = self.indptr[i]..self.indptr[i + 1];
        self.knots[r.clone()]
            .iter()
            .copied()
            .zip(self.values[r].iter().copied())
    }
}

/// An axis' entries as the final matrix stores them: its own entries, or every knot (explicit
/// zeros included) when scipy's `kron` stored this axis densely. Rows (sample → knots) and
/// columns (knot → samples), both ascending.
#[derive(Clone, Debug)]
struct AxisPattern {
    n_knots: usize,
    row_ptr: Vec<usize>,
    row_knot: Vec<usize>,
    row_val: Vec<f64>,
    col_ptr: Vec<usize>,
    col_sample: Vec<usize>,
    col_val: Vec<f64>,
}

impl AxisPattern {
    fn new(w: &AxisWeights, dense: bool) -> AxisPattern {
        let (mut row_ptr, mut row_knot, mut row_val) = (vec![0], Vec::new(), Vec::new());
        for i in 0..w.n_samples {
            if dense {
                let mut full = vec![0.0f64; w.n_knots];
                for (k, v) in w.row(i) {
                    full[k] = v;
                }
                for (k, v) in full.into_iter().enumerate() {
                    row_knot.push(k);
                    row_val.push(v);
                }
            } else {
                for (k, v) in w.row(i) {
                    row_knot.push(k);
                    row_val.push(v);
                }
            }
            row_ptr.push(row_knot.len());
        }
        // Transpose (counting sort keeps the samples ascending in every column).
        let mut count = vec![0usize; w.n_knots + 1];
        for &k in &row_knot {
            count[k + 1] += 1;
        }
        for k in 0..w.n_knots {
            count[k + 1] += count[k];
        }
        let col_ptr = count.clone();
        let mut next = count;
        let mut col_sample = vec![0usize; row_knot.len()];
        let mut col_val = vec![0.0f64; row_knot.len()];
        for i in 0..w.n_samples {
            for e in row_ptr[i]..row_ptr[i + 1] {
                let k = row_knot[e];
                col_sample[next[k]] = i;
                col_val[next[k]] = row_val[e];
                next[k] += 1;
            }
        }
        AxisPattern {
            n_knots: w.n_knots,
            row_ptr,
            row_knot,
            row_val,
            col_ptr,
            col_sample,
            col_val,
        }
    }

    fn row(&self, i: usize) -> (&[usize], &[f64]) {
        let r = self.row_ptr[i]..self.row_ptr[i + 1];
        (&self.row_knot[r.clone()], &self.row_val[r])
    }

    fn col(&self, k: usize) -> (&[usize], &[f64]) {
        let r = self.col_ptr[k]..self.col_ptr[k + 1];
        (&self.col_sample[r.clone()], &self.col_val[r])
    }
}

/// The weights of one control grid on one sampling grid: sdcflows' `grid_bspline_weights`.
#[derive(Clone, Debug)]
pub struct LevelWeights {
    pub axes: [AxisWeights; 3],
    /// Whether scipy's `kron` stored axes 1 and 2 densely (axis 0 is never).
    pub dense: [bool; 3],
    /// Whether sdcflows' alignment test passed (else it warns [`NOT_ALIGNED`] and goes on: SDC-02).
    pub aligned: bool,
    pub dtype: WeightType,
    /// How many samples get weights SDC-01 changes (beyond the control grid): per axis for a
    /// separable level, samples for an [`ExactLevel`].
    pub beyond: usize,
    /// SDC-02 corrected: the weights at every sample's own position (a rotated sampling grid).
    pub exact: Option<ExactLevel>,
    patterns: Option<[AxisPattern; 3]>,
}

/// The weights of a sampling grid rotated relative to the control grid, at every sample's own
/// position (SDC-02's correction): the sample's three coordinates in the control grid
/// (`apply_affine(inv(ctrl) @ target, ijk)`, nibabel's arithmetic), each axis' kept weights at
/// its coordinate (as [`AxisWeights`], SDC-01 corrected), and every product `(w0 * w1) * w2` in
/// the matrix type, in ascending column order. Computed for each row when it is used.
#[derive(Clone, Debug)]
pub struct ExactLevel {
    to_grid: Mat4,
    sample_shape: [usize; 3],
    knot_shape: [usize; 3],
    single: bool,
    nnz: usize,
}

impl ExactLevel {
    fn new(
        to_grid: Mat4,
        sample_shape: [usize; 3],
        knot_shape: [usize; 3],
        dtype: WeightType,
    ) -> (ExactLevel, usize) {
        let n: usize = sample_shape.iter().product();
        let mut level = ExactLevel {
            to_grid,
            sample_shape,
            knot_shape,
            single: n == 1,
            nnz: 0,
        };
        let (mut nnz, mut beyond) = (0, 0);
        for s in 0..n {
            let (w, b) = level.axes(s, dtype);
            nnz += w.iter().map(|a| a.len).product::<usize>();
            beyond += usize::from(b);
        }
        level.nnz = nnz;
        (level, beyond)
    }

    /// Sample `s`'s weights on the three axes (SDC-01 corrected) and whether SDC-01 changes them.
    fn axes(&self, s: usize, dtype: WeightType) -> ([SampleWeights; 3], bool) {
        let [_, n1, n2] = self.sample_shape;
        let idx = [s / (n1 * n2), (s / n2) % n1, s % n2];
        let p: [f64; 3] = std::array::from_fn(|a| match dtype {
            WeightType::Float32 => f64::from(idx[a] as f32),
            WeightType::Float64 => idx[a] as f64,
        });
        let q = if self.single {
            apply_affine_single(&self.to_grid, p)
        } else {
            apply_affine(&self.to_grid, p)
        };
        let w: [SampleWeights; 3] = std::array::from_fn(|a| {
            SampleWeights::of(q[a], self.knot_shape[a], dtype, UpstreamDefects::Fix)
        });
        let beyond = w.iter().any(|a| a.beyond) && w.iter().all(|a| a.len > 0);
        (w, beyond)
    }

    fn for_row(&self, s: usize, dtype: WeightType, off: usize, mut f: impl FnMut(usize, f64)) {
        let (w, _) = self.axes(s, dtype);
        let [_, k1n, k2n] = self.knot_shape;
        for i0 in 0..w[0].len {
            for i1 in 0..w[1].len {
                let w01 = dtype.mul(w[0].values[i0], w[1].values[i1]);
                let base = off + (w[0].knots[i0] * k1n + w[1].knots[i1]) * k2n;
                for i2 in 0..w[2].len {
                    f(base + w[2].knots[i2], dtype.mul(w01, w[2].values[i2]));
                }
            }
        }
    }
}

impl LevelWeights {
    /// The knot count of this level (the matrix's columns).
    pub fn n_coefficients(&self) -> usize {
        self.axes.iter().map(|a| a.n_knots).product()
    }

    /// The sample count (the matrix's rows).
    pub fn n_samples(&self) -> usize {
        self.axes.iter().map(|a| a.n_samples).product()
    }

    /// The number of entries the matrix stores (explicit zeros included).
    pub fn nnz(&self) -> usize {
        if let Some(e) = &self.exact {
            return e.nnz;
        }
        match &self.patterns {
            None => 0,
            Some(p) => p.iter().map(|a| a.row_knot.len()).product(),
        }
    }
}

/// sdcflows' `grid_bspline_weights(target_nii, ctrl_nii, dtype)`: the cubic B-spline weights of
/// every voxel of the sampling grid (`target_shape`, `target_affine`) for every control point of
/// the grid (`ctrl_shape`, `ctrl_affine`). `defects`: SDC-01 and SDC-02 corrected
/// ([`UpstreamDefects::Fix`]: the B-spline beyond the grid, the exact weights of a rotated
/// grid) or sdcflows' weights bit for bit ([`UpstreamDefects::Reproduce`]); the returned
/// [`LevelWeights::beyond`] and [`LevelWeights::aligned`] say whether they changed the result.
pub fn grid_bspline_weights(
    target_shape: [usize; 3],
    target_affine: &Mat4,
    ctrl_shape: [usize; 3],
    ctrl_affine: &Mat4,
    dtype: WeightType,
    defects: UpstreamDefects,
) -> Result<LevelWeights, SdcError> {
    let aligned = npy::grids_aligned(ctrl_affine, target_affine);
    let to_grid = matmul4(&inv4(ctrl_affine).ok_or(SdcError::Singular)?, target_affine);
    let axes: [AxisWeights; 3] = std::array::from_fn(|axis| {
        let n = target_shape[axis];
        // coords: zeros (3, L) of the matrix type with arange on this axis' row; locs =
        // apply_affine(target_to_grid, coords.T)[:, axis] (dgemm, or dgemv for one sample).
        let locs: Vec<f64> = (0..n)
            .map(|i| {
                let mut p = [0.0f64; 3];
                p[axis] = match dtype {
                    WeightType::Float32 => f64::from(i as f32),
                    WeightType::Float64 => i as f64,
                };
                let q = if n == 1 {
                    apply_affine_single(&to_grid, p)
                } else {
                    apply_affine(&to_grid, p)
                };
                q[axis]
            })
            .collect();
        AxisWeights::from_locations(&locs, ctrl_shape[axis], dtype, defects)
    });
    // kron(w0, w1), then kron(that, w2): an operand B with 2 * nnz >= rows * cols is stored
    // densely (BSR); a kron with an empty operand is the zero matrix.
    let dense_of = |a: &AxisWeights| 2 * a.nnz() >= a.n_samples * a.n_knots;
    let dense = [false, dense_of(&axes[1]), dense_of(&axes[2])];
    if !aligned && defects.fix() {
        let (exact, beyond) = ExactLevel::new(to_grid, target_shape, ctrl_shape, dtype);
        return Ok(LevelWeights {
            axes,
            dense: [false; 3],
            aligned,
            dtype,
            beyond,
            exact: Some(exact),
            patterns: None,
        });
    }
    let empty = axes.iter().any(|a| a.nnz() == 0);
    let beyond = if empty {
        0
    } else {
        axes.iter().map(|a| a.beyond).sum()
    };
    let patterns = if empty {
        None
    } else {
        Some(std::array::from_fn(|a| {
            AxisPattern::new(&axes[a], dense[a])
        }))
    };
    Ok(LevelWeights {
        axes,
        dense,
        aligned,
        dtype,
        beyond,
        exact: None,
        patterns,
    })
}

/// sdcflows' collocation matrix: the weights of one or more control grids on one sampling grid,
/// side by side (`sparse_hstack([grid_bspline_weights(target, level) for level in levels])
/// .tocsr()`).
#[derive(Clone, Debug)]
pub struct Collocation {
    levels: Vec<LevelWeights>,
    sample_shape: [usize; 3],
    offsets: Vec<usize>,
    n_cols: usize,
}

/// A stored sparse matrix in scipy's CSR layout (rows sorted by column).
#[derive(Clone, Debug, PartialEq)]
pub struct Csr<T> {
    pub shape: [usize; 2],
    pub indptr: Vec<usize>,
    pub indices: Vec<usize>,
    pub data: Vec<T>,
}

impl Collocation {
    /// The levels side by side; every level must have been computed on the same sampling grid.
    pub fn new(levels: Vec<LevelWeights>) -> Result<Collocation, SdcError> {
        let first = levels
            .first()
            .ok_or_else(|| SdcError::Invalid("need at least one array to concatenate".into()))?;
        let sample_shape = std::array::from_fn(|a| first.axes[a].n_samples);
        if levels.iter().any(|l| {
            (0..3).any(|a| l.axes[a].n_samples != sample_shape[a]) || l.dtype != first.dtype
        }) {
            return Err(SdcError::Invalid(
                "the levels' weights were computed on different sampling grids".into(),
            ));
        }
        let mut offsets = Vec::with_capacity(levels.len());
        let mut n_cols = 0;
        for l in &levels {
            offsets.push(n_cols);
            n_cols += l.n_coefficients();
        }
        Ok(Collocation {
            levels,
            sample_shape,
            offsets,
            n_cols,
        })
    }

    pub fn levels(&self) -> &[LevelWeights] {
        &self.levels
    }

    /// `(rows, cols)`: samples (C order of the sampling grid) by control points (each level's
    /// C order, level after level).
    pub fn shape(&self) -> [usize; 2] {
        [self.sample_shape.iter().product(), self.n_cols]
    }

    /// The number of stored entries, explicit zeros included (`.nnz` of sdcflows' matrix).
    pub fn nnz(&self) -> usize {
        self.levels.iter().map(LevelWeights::nnz).sum()
    }

    fn unravel(&self, s: usize) -> [usize; 3] {
        let [_, n1, n2] = self.sample_shape;
        [s / (n1 * n2), (s / n2) % n1, s % n2]
    }

    /// Calls `f(column, weight)` for the stored entries of row `s`, in ascending column order;
    /// the weight is the kron product `(w0 * w1) * w2` in the matrix type.
    fn for_row(&self, s: usize, mut f: impl FnMut(usize, f64)) {
        let [i0, i1, i2] = self.unravel(s);
        for (level, off) in self.levels.iter().zip(&self.offsets) {
            if let Some(e) = &level.exact {
                e.for_row(s, level.dtype, *off, &mut f);
                continue;
            }
            let Some(p) = &level.patterns else { continue };
            let (k1n, k2n) = (p[1].n_knots, p[2].n_knots);
            let (r0k, r0v) = p[0].row(i0);
            let (r1k, r1v) = p[1].row(i1);
            let (r2k, r2v) = p[2].row(i2);
            for (&k0, &w0) in r0k.iter().zip(r0v) {
                for (&k1, &w1) in r1k.iter().zip(r1v) {
                    let w01 = level.dtype.mul(w0, w1);
                    let base = off + (k0 * k1n + k1) * k2n;
                    for (&k2, &w2) in r2k.iter().zip(r2v) {
                        f(base + k2, level.dtype.mul(w01, w2));
                    }
                }
            }
        }
    }

    /// Calls `f(sample, weight)` for the stored entries of column `c`, in ascending sample order.
    fn for_col(&self, c: usize, mut f: impl FnMut(usize, f64)) {
        let li = self.offsets.partition_point(|&o| o <= c) - 1;
        let level = &self.levels[li];
        let Some(p) = &level.patterns else { return };
        let local = c - self.offsets[li];
        let (k1n, k2n) = (p[1].n_knots, p[2].n_knots);
        let (k0, k1, k2) = (local / (k1n * k2n), (local / k2n) % k1n, local % k2n);
        let [_, n1, n2] = self.sample_shape;
        let (c0s, c0v) = p[0].col(k0);
        let (c1s, c1v) = p[1].col(k1);
        let (c2s, c2v) = p[2].col(k2);
        for (&i0, &w0) in c0s.iter().zip(c0v) {
            for (&i1, &w1) in c1s.iter().zip(c1v) {
                let w01 = level.dtype.mul(w0, w1);
                let base = (i0 * n1 + i1) * n2;
                for (&i2, &w2) in c2s.iter().zip(c2v) {
                    f(base + i2, level.dtype.mul(w01, w2));
                }
            }
        }
    }

    /// `X @ v` for float32 `X` and `v`, as scipy's `csr_matvec` computes each row: a float32
    /// sum from +0 of the rounded products, in ascending column order. `rows` restricts it to
    /// those rows (`X[mask] @ v`, in their order); `None` is every row.
    pub fn matvec_f32(
        &self,
        v: &[f32],
        rows: Option<&[usize]>,
        n_threads: usize,
    ) -> Result<Vec<f32>, SdcError> {
        self.check_f32()?;
        if v.len() != self.n_cols {
            return Err(SdcError::Invalid(format!(
                "dimension mismatch: the matrix has {} columns, the vector {} values",
                self.n_cols,
                v.len()
            )));
        }
        let n = rows.map_or(self.shape()[0], <[usize]>::len);
        let row = |r: usize| -> f32 {
            let s = rows.map_or(r, |rs| rs[r]);
            let mut sum = 0.0f32;
            self.for_row(s, |c, w| sum += (w as f32) * v[c]);
            sum
        };
        Ok(with_threads(n_threads, || {
            (0..n).into_par_iter().with_min_len(1024).map(row).collect()
        })?)
    }

    /// `X.T @ y` for float32 `X` and `y`, as scipy computes it through the transposed (CSC)
    /// matrix (`csc_matvec`): for every column, a float32 sum from +0 of the rounded products in
    /// ascending sample order.
    pub fn rmatvec_f32(&self, y: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError> {
        self.check_f32()?;
        if y.len() != self.shape()[0] {
            return Err(SdcError::Invalid(format!(
                "dimension mismatch: the matrix has {} rows, the vector {} values",
                self.shape()[0],
                y.len()
            )));
        }
        if self.levels.iter().any(|l| l.exact.is_some()) {
            // An exact level has no column view: scatter the rows in sample order, which adds
            // every column's products in ascending sample order from +0, as `for_col` does.
            let mut out = vec![0.0f32; self.n_cols];
            for (s, &ys) in y.iter().enumerate() {
                self.for_row(s, |c, w| out[c] += (w as f32) * ys);
            }
            return Ok(out);
        }
        let col = |c: usize| -> f32 {
            let mut sum = 0.0f32;
            self.for_col(c, |s, w| sum += (w as f32) * y[s]);
            sum
        };
        Ok(with_threads(n_threads, || {
            (0..self.n_cols)
                .into_par_iter()
                .with_min_len(16)
                .map(col)
                .collect()
        })?)
    }

    fn check_f32(&self) -> Result<(), SdcError> {
        if self.levels.iter().any(|l| l.dtype != WeightType::Float32) {
            return Err(SdcError::Invalid(
                "the products are ported for float32 weights".into(),
            ));
        }
        Ok(())
    }

    /// The matrix as scipy stores it (`.tocsr()`: rows in order, columns ascending, explicit
    /// zeros kept), with values in the matrix type (float32 values held as float64).
    pub fn to_csr(&self, n_threads: usize) -> Result<Csr<f64>, SdcError> {
        let [n_rows, n_cols] = self.shape();
        let rows: Vec<(Vec<usize>, Vec<f64>)> = with_threads(n_threads, || {
            (0..n_rows)
                .into_par_iter()
                .with_min_len(1024)
                .map(|s| {
                    let (mut cols, mut vals) = (Vec::new(), Vec::new());
                    self.for_row(s, |c, w| {
                        cols.push(c);
                        vals.push(w);
                    });
                    (cols, vals)
                })
                .collect()
        })?;
        let nnz = rows.iter().map(|r| r.0.len()).sum();
        let mut indptr = Vec::with_capacity(n_rows + 1);
        let mut indices = Vec::with_capacity(nnz);
        let mut data = Vec::with_capacity(nnz);
        indptr.push(0);
        for (c, v) in rows {
            indices.extend(c);
            data.extend(v);
            indptr.push(indices.len());
        }
        Ok(Csr {
            shape: [n_rows, n_cols],
            indptr,
            indices,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(z: [f64; 3], t: [f64; 3]) -> Mat4 {
        [
            [z[0], 0.0, 0.0, t[0]],
            [0.0, z[1], 0.0, t[1]],
            [0.0, 0.0, z[2], t[2]],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// The closed-form cubic B-spline (Unser 1999, eq. 6), for checking the recursion's values.
    fn beta3(u: f64) -> f64 {
        let a = u.abs();
        if a < 1.0 {
            2.0 / 3.0 - a * a + a * a * a / 2.0
        } else if a < 2.0 {
            (2.0 - a).powi(3) / 6.0
        } else {
            0.0
        }
    }

    #[test]
    fn deboor_matches_the_cubic_bspline_inside_the_knots() {
        let n_knots = 7;
        for i in 0..=600 {
            let x = i as f64 / 100.0; // 0 .. 6 = K - 1
            let ell = find_interval(n_knots, x).unwrap() as i64;
            let w = deboor3(x, ell);
            for kk in 0..n_knots {
                let v = basis_value(&w, ell, kk as i64 + 1);
                assert!((v - beta3(x - kk as f64)).abs() < 1e-14, "x {x} knot {kk}");
            }
            let sum: f64 = w.iter().sum();
            assert!((sum - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn extrapolation_continues_the_end_pieces() {
        // sdcflows' weights outside the knots come from the end polynomial pieces (BSpline's
        // extrapolate=True): at -1.5 the knot-0 weight is 2/3 - u² + u³/2 at u = -1.5.
        let ell = find_interval(5, -1.5).unwrap() as i64;
        assert_eq!(ell, 3);
        let v = basis_value(&deboor3(-1.5, ell), ell, 1);
        assert!((v - (2.0 / 3.0 - 2.25 - 1.6875)).abs() < 1e-12, "{v}");
        // SDC-01: reproduced, that value; corrected, the cubic B-spline's (2 - 1.5)^3 / 6.
        let r = SampleWeights::of(-1.5, 5, WeightType::Float64, UpstreamDefects::Reproduce);
        let c = SampleWeights::of(-1.5, 5, WeightType::Float64, UpstreamDefects::Fix);
        assert!(r.beyond && c.beyond);
        assert_eq!((r.len, c.len), (1, 1));
        assert_eq!(r.values[0], v);
        assert!((c.values[0] - beta3(1.5)).abs() < 1e-15, "{}", c.values[0]);
    }

    #[test]
    fn the_corrected_basis_is_the_bspline_beyond_the_grid_and_the_same_bits_inside() {
        let n = 6;
        for i in -250..=750 {
            let x = i as f64 / 100.0; // -2.5 .. 7.5 = K + 1.5
            let r = SampleWeights::of(x, n, WeightType::Float64, UpstreamDefects::Reproduce);
            let c = SampleWeights::of(x, n, WeightType::Float64, UpstreamDefects::Fix);
            if (0.0..=5.0).contains(&x) {
                assert!(!r.beyond && !c.beyond);
                assert_eq!(r.len, c.len);
                for j in 0..r.len {
                    assert_eq!(r.values[j].to_bits(), c.values[j].to_bits(), "x {x}");
                }
            }
            for j in 0..c.len {
                let want = beta3(x - c.knots[j] as f64);
                assert!(
                    (c.values[j] - want).abs() < 1e-14,
                    "x {x} knot {}",
                    c.knots[j]
                );
            }
        }
    }

    #[test]
    fn a_rotated_grid_is_evaluated_at_every_sample_position() {
        // SDC-02: a target rotated 30 degrees about z; corrected, each row's weights are the
        // products of the B-splines at the sample's own coordinates in the control grid.
        let ctrl = diag([16.0, 16.0, 10.0], [-40.0, -40.0, -20.0]);
        let (c, sn) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let target: Mat4 = [
            [2.0 * c, -2.5 * sn, 0.0, -12.0],
            [2.0 * sn, 2.5 * c, 0.0, -10.0],
            [0.0, 0.0, 3.0, -9.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (ts, ks) = ([9, 8, 5], [6, 6, 5]);
        let r = grid_bspline_weights(
            ts,
            &target,
            ks,
            &ctrl,
            WeightType::Float64,
            UpstreamDefects::Reproduce,
        )
        .unwrap();
        let f = grid_bspline_weights(
            ts,
            &target,
            ks,
            &ctrl,
            WeightType::Float64,
            UpstreamDefects::Fix,
        )
        .unwrap();
        assert!(!r.aligned && r.exact.is_none());
        assert!(f.exact.is_some());
        let col = Collocation::new(vec![f]).unwrap();
        let csr = col.to_csr(2).unwrap();
        assert_eq!(csr.indptr.len(), 9 * 8 * 5 + 1);
        assert_eq!(col.nnz(), csr.data.len());
        let to_grid = matmul4(&inv4(&ctrl).unwrap(), &target);
        for s in [0usize, 17, 133, 359] {
            let idx = [s / 40, (s / 5) % 8, s % 5];
            let q = apply_affine(&to_grid, std::array::from_fn(|a| idx[a] as f64));
            for e in csr.indptr[s]..csr.indptr[s + 1] {
                let k = csr.indices[e];
                let kk = [k / 30, (k / 5) % 6, k % 5];
                let want: f64 = (0..3).map(|a| beta3(q[a] - kk[a] as f64)).product();
                assert!((csr.data[e] - want).abs() < 1e-14, "sample {s} knot {k}");
            }
            // ascending columns
            assert!(
                csr.indices[csr.indptr[s]..csr.indptr[s + 1]]
                    .windows(2)
                    .all(|w| w[0] < w[1])
            );
        }
        // the transposed product scatters in sample order: the same bits as the stored matrix
        let y: Vec<f32> = (0..csr.shape[0])
            .map(|i| (i % 5) as f32 * 0.25 - 0.5)
            .collect();
        let f32col = Collocation::new(vec![
            grid_bspline_weights(
                ts,
                &target,
                ks,
                &ctrl,
                WeightType::Float32,
                UpstreamDefects::Fix,
            )
            .unwrap(),
        ])
        .unwrap();
        let c32 = f32col.to_csr(1).unwrap();
        let mut want = vec![0.0f32; c32.shape[1]];
        for s in 0..c32.shape[0] {
            for e in c32.indptr[s]..c32.indptr[s + 1] {
                want[c32.indices[e]] += (c32.data[e] as f32) * y[s];
            }
        }
        let got = f32col.rmatvec_f32(&y, 3).unwrap();
        assert!(
            want.iter()
                .zip(&got)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
    }

    #[test]
    fn interval_search() {
        assert_eq!(find_interval(5, f64::NAN), None);
        assert_eq!(find_interval(5, -10.0), Some(3));
        assert_eq!(find_interval(5, 0.0), Some(3));
        assert_eq!(find_interval(5, 0.999), Some(3));
        assert_eq!(find_interval(5, 1.0), Some(4));
        assert_eq!(find_interval(5, 3.999), Some(6));
        assert_eq!(find_interval(5, 4.0), Some(6)); // t[n] = K - 1: the last interval
        assert_eq!(find_interval(5, 1e300), Some(6));
        assert_eq!(find_interval(5, f64::INFINITY), Some(6));
    }

    #[test]
    fn grid_covers_and_centres_the_image() {
        let aff = diag([3.75, 3.75, 4.0], [-118.125, -118.125, -86.0]);
        let g = bspline_grid([64, 64, 44], &aff, &[3.75, 3.75, 4.0], DEFAULT_HF_ZOOMS_MM).unwrap();
        // 3.75 * 63 = 236.25 // 16 = 14, + 3 = 17; 4 * 43 = 172 // 10 = 17, + 3 = 20
        assert_eq!(g.shape, [17, 17, 20]);
        assert_eq!(g.affine[0][0], 16.0);
        assert_eq!(g.affine[2][2], 10.0);
        assert!(bspline_grid([4, 4, 4], &aff, &[1.0, 1.0, 1.0, 2.0], DEFAULT_ZOOMS_MM).is_err());
    }

    #[test]
    fn products_follow_the_stored_pattern() {
        let target = diag([2.0, 2.5, 3.0], [-19.0, -21.25, -9.0]);
        let g = bspline_grid([20, 18, 7], &target, &[2.0, 2.5, 3.0], DEFAULT_HF_ZOOMS_MM).unwrap();
        let w = grid_bspline_weights(
            [20, 18, 7],
            &target,
            g.shape,
            &g.affine,
            WeightType::Float32,
            UpstreamDefects::Reproduce,
        )
        .unwrap();
        assert!(w.aligned);
        assert_eq!(g.shape, [5, 5, 4]);
        // probe_colmat.py (sdcflows on SciPy 1.15.3 and 1.18.1): 191,520 stored, 71,136 zeros
        assert_eq!(w.dense, [false, true, true]);
        let col = Collocation::new(vec![w]).unwrap();
        assert_eq!(col.nnz(), 191_520);
        let csr = col.to_csr(2).unwrap();
        assert_eq!(csr.data.iter().filter(|&&v| v == 0.0).count(), 71_136);
        // X @ v and X.T @ y against the stored matrix, summed in scipy's order.
        let v: Vec<f32> = (0..col.shape()[1]).map(|i| (i % 7) as f32 - 3.0).collect();
        let y: Vec<f32> = (0..col.shape()[0]).map(|i| (i % 5) as f32 * 0.25).collect();
        let xv = col.matvec_f32(&v, None, 3).unwrap();
        let xty = col.rmatvec_f32(&y, 3).unwrap();
        let mut want_xty = vec![0.0f32; col.shape()[1]];
        for s in 0..col.shape()[0] {
            let mut sum = 0.0f32;
            for e in csr.indptr[s]..csr.indptr[s + 1] {
                sum += (csr.data[e] as f32) * v[csr.indices[e]];
                want_xty[csr.indices[e]] += (csr.data[e] as f32) * y[s];
            }
            assert_eq!(sum.to_bits(), xv[s].to_bits());
        }
        for (a, b) in want_xty.iter().zip(&xty) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        // Thread counts do not change the bits.
        assert_eq!(col.rmatvec_f32(&y, 1).unwrap(), xty);
        assert_eq!(col.matvec_f32(&v, None, 1).unwrap(), xv);
    }
}
