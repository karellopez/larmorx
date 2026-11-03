// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-miscmaths 2412.6 (splinterpolator.h). Copyright (C)
// 2008-2024 University of Oxford (Jesper Andersson et al.). Rust translation Copyright 2026 Karel
// Lopez Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `SPLINTERPOLATOR::Splinterpolator<T>`: B-spline interpolation of order 0 to 7 in one to five
//! dimensions, with the value's first derivatives, as fsl-miscmaths 2412.6 computes them.
//!
//! The coefficients are the data deconvolved along every dimension longer than one voxel
//! (Unser's recursive filter, one to three poles by order), each pass's results stored in `T`
//! before the next pass reads them; the interpolated value and its derivatives are sums of
//! coefficients times separable weights, in `double`, in FSL's loop order. Coefficients outside
//! the grid follow each dimension's extrapolation (`Zeros` and `SoftZeros`: zero; `Constant` and
//! `CorrectConstant`: the edge; `Mirror`; `Periodic`).
//!
//! newimage's spline interpolation ([`crate::interp_partial`]) builds one of these over a
//! volume. [`crate::interp::SplineCoefs`] is the earlier order-3 translation that mcflirt,
//! FLIRT and FAST use; for orders 3 and the extrapolations they meet both give the same bits
//! (unit test `same_as_spline_coefs`).
//!
//! Index arithmetic beyond `i32` (only coordinates of about 2^31 voxels, e.g. a NaN position,
//! whose start index x86-64 converts to `INT_MIN`): FSL's `Mirror` loop takes `|index| / size`
//! steps there and its products overflow `int`; the mapping here is the closed form those loops
//! compute wherever they do not overflow (unit test `indx2indx_loops`).

use crate::cnum;
use crate::sse;
use crate::volume::FslError;

/// `SPLINTERPOLATOR::ExtrapolationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtrapolationType {
    Zeros,
    Constant,
    Mirror,
    Periodic,
    SoftZeros,
    CorrectConstant,
}

/// `SplinterpolatorException(msg)`: `what()` is `"Splinterpolator::" + msg`.
pub fn splinterpolator_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "SPLINTERPOLATOR::SplinterpolatorException",
        what: format!("Splinterpolator::{msg}"),
    }
}

/// The element type `T` of a `Splinterpolator<T>`: how a value is read as `double`, how
/// `SplineColumn::Set` stores a deconvolved value, and `static_cast<T>(double)`.
pub trait SplineValue: Copy + Default + Send + Sync + std::fmt::Debug + 'static {
    fn to_f64(self) -> f64;
    /// `SplineColumn::Set`: `static_cast<T>(v)` for `float` and `double` (integer types round:
    /// `static_cast<T>(v + 0.5)`).
    fn from_column(v: f64) -> Self;
    /// `static_cast<T>(v)`.
    fn from_f64(v: f64) -> Self;
}

impl SplineValue for f32 {
    #[inline(always)]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline(always)]
    fn from_column(v: f64) -> f32 {
        v as f32
    }
    #[inline(always)]
    fn from_f64(v: f64) -> f32 {
        v as f32
    }
}

impl SplineValue for f64 {
    #[inline(always)]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline(always)]
    fn from_column(v: f64) -> f64 {
        v
    }
    #[inline(always)]
    fn from_f64(v: f64) -> f64 {
        v
    }
}

/// The arithmetic of the sums: plain (numbers) or with SSE's NaNs, operands in the source's
/// order. A sum that comes out NaN is computed again with [`Sse`] (same bits for numbers).
trait Arith {
    fn add(a: f64, b: f64) -> f64;
    fn sub(a: f64, b: f64) -> f64;
    fn mul(a: f64, b: f64) -> f64;
    fn div(a: f64, b: f64) -> f64;
}

struct Plain;
struct Sse;

impl Arith for Plain {
    #[inline(always)]
    fn add(a: f64, b: f64) -> f64 {
        a + b
    }
    #[inline(always)]
    fn sub(a: f64, b: f64) -> f64 {
        a - b
    }
    #[inline(always)]
    fn mul(a: f64, b: f64) -> f64 {
        a * b
    }
    #[inline(always)]
    fn div(a: f64, b: f64) -> f64 {
        a / b
    }
}

impl Arith for Sse {
    #[inline(always)]
    fn add(a: f64, b: f64) -> f64 {
        sse::add_f64(a, b)
    }
    #[inline(always)]
    fn sub(a: f64, b: f64) -> f64 {
        sse::sub_f64(a, b)
    }
    #[inline(always)]
    fn mul(a: f64, b: f64) -> f64 {
        sse::mul_f64(a, b)
    }
    #[inline(always)]
    fn div(a: f64, b: f64) -> f64 {
        sse::div_f64(a, b)
    }
}

/// `Splinterpolator<T>`: the coefficients and how to interpolate them.
#[derive(Clone, Debug)]
pub struct Splinterpolator<T: SplineValue> {
    coef: Vec<T>,
    order: u32,
    ndim: usize,
    prec: f64,
    /// Five sizes, 1 beyond `ndim`.
    dim: [u32; 5],
    /// One per dimension (`ndim` of them).
    et: Vec<ExtrapolationType>,
}

impl<T: SplineValue> Splinterpolator<T> {
    /// `Splinterpolator(data_or_coefs, dim, et, order, copy_low_order, nthr, prec,
    /// data_are_coefs)`: `data` holds the values (x fastest) of a grid of `dim` (one to five
    /// sizes), `et` one extrapolation per dimension. With `data_are_coefs` the values are the
    /// coefficients. FSL's thread count splits the deconvolution's lines among threads, which
    /// gives the same coefficients (each line is deconvolved alone); `copy_low_order` only
    /// decides whether orders 0 and 1 copy the data.
    pub fn new(
        data: &[T],
        dim: &[u32],
        et: &[ExtrapolationType],
        order: u32,
        prec: f64,
        data_are_coefs: bool,
    ) -> Result<Splinterpolator<T>, FslError> {
        // common_construction
        if dim.is_empty() {
            return Err(splinterpolator_error(
                "common_construction: data has zeros dimensions",
            ));
        }
        if dim.len() > 5 {
            return Err(splinterpolator_error(
                "common_construction: data cannot have more than 5 dimensions",
            ));
        }
        if dim.len() != et.len() {
            return Err(splinterpolator_error(
                "common_construction: dim and et must have the same size",
            ));
        }
        if dim.contains(&0) {
            return Err(splinterpolator_error(
                "common_construction: data cannot have zeros size in any direction",
            ));
        }
        if order > 7 {
            return Err(splinterpolator_error(
                "common_construction: spline order must be lesst than 7",
            ));
        }
        let mut d5 = [1u32; 5];
        d5[..dim.len()].copy_from_slice(dim);
        let ts: usize = d5.iter().map(|&d| d as usize).product();
        if data.len() < ts {
            // FSL reads past the caller's array (a pointer, no length): not reproduced.
            return Err(splinterpolator_error(
                "common_construction: zero data pointer",
            ));
        }
        let mut s = Splinterpolator {
            coef: data[..ts].to_vec(),
            order,
            ndim: dim.len(),
            prec,
            dim: d5,
            et: et.to_vec(),
        };
        // calc_coef
        if order >= 2 && !data_are_coefs {
            for cdir in 0..5 {
                if s.dim[cdir] > 1 {
                    s.deconv_along(cdir);
                }
            }
        }
        Ok(s)
    }

    /// The same, one extrapolation for every dimension (the constructor taking one
    /// `ExtrapolationType`).
    pub fn new_uniform(
        data: &[T],
        dim: &[u32],
        et: ExtrapolationType,
        order: u32,
    ) -> Result<Splinterpolator<T>, FslError> {
        Splinterpolator::new(data, dim, &vec![et; dim.len()], order, 1e-8, false)
    }

    pub fn ndim(&self) -> usize {
        self.ndim
    }

    pub fn order(&self) -> u32 {
        self.order
    }

    /// `Extrapolation(dim)`.
    pub fn extrapolation(&self, d: usize) -> Result<ExtrapolationType, FslError> {
        if d >= self.ndim {
            return Err(splinterpolator_error("Extrapolation: Invalid dimension"));
        }
        Ok(self.et[d])
    }

    /// `Size()`: five sizes (1 beyond the dimensions given).
    pub fn size(&self) -> [u32; 5] {
        self.dim
    }

    /// `Copy(dest)`: every coefficient.
    pub fn coefs(&self) -> &[T] {
        &self.coef
    }

    /// `Coef(indx)`: one coefficient (zero-offset indices, at most five).
    pub fn coef_at(&self, indx: &[u32]) -> Result<T, FslError> {
        if indx.is_empty() {
            return Err(splinterpolator_error("Coef: indx has zeros dimensions"));
        }
        if indx.len() > 5 {
            return Err(splinterpolator_error(
                "Coef: indx has more than 5 dimensions",
            ));
        }
        for (i, &x) in indx.iter().enumerate() {
            if x >= self.dim[i] {
                return Err(splinterpolator_error("Coef: indx out of range"));
            }
        }
        let n = indx.len();
        let mut lindx = indx[n - 1] as usize;
        for i in (0..n - 1).rev() {
            lindx = self.dim[i] as usize * lindx + indx[i] as usize;
        }
        Ok(self.coef[lindx])
    }

    /// `Coef(x, y, z)`.
    pub fn coef3(&self, x: u32, y: u32, z: u32) -> Result<T, FslError> {
        self.coef_at(&[x, y, z])
    }

    /// `operator()(coord)`: the value at `coord` (one coordinate per dimension).
    pub fn value(&self, coord: &[f32]) -> Result<T, FslError> {
        if coord.len() != self.ndim {
            return Err(splinterpolator_error("operator(): coord has wrong length"));
        }
        let mut dcoord = [0.0f64; 5];
        for (d, &c) in dcoord.iter_mut().zip(coord) {
            *d = f64::from(c);
        }
        Ok(T::from_f64(self.value_at(&dcoord)))
    }

    /// `operator()(x, y, z, t)`: the value at a position given by up to four coordinates.
    pub fn at(&self, x: f64, y: f64, z: f64, t: f64) -> Result<T, FslError> {
        let nd = self.ndim;
        if nd > 4 || (t != 0.0 && nd < 4) || (z != 0.0 && nd < 3) || (y != 0.0 && nd < 2) {
            return Err(splinterpolator_error(
                "operator(): input has wrong dimensionality",
            ));
        }
        Ok(T::from_f64(self.value_at(&[x, y, z, t, 0.0])))
    }

    /// `operator()(coord, dd, &dval)`: the value and its derivative along dimension `dd`.
    pub fn value_deriv(&self, coord: &[f32], dd: u32) -> Result<(T, T), FslError> {
        if coord.len() != self.ndim {
            return Err(splinterpolator_error("operator(): coord has wrong length"));
        }
        if dd as usize > self.ndim - 1 {
            return Err(splinterpolator_error(
                "operator(): derivative specified for invalid direction",
            ));
        }
        let mut dcoord = [0.0f64; 5];
        for (d, &c) in dcoord.iter_mut().zip(coord) {
            *d = f64::from(c);
        }
        let mut deriv = [0u32; 5];
        deriv[dd as usize] = 1;
        let mut dval = [0.0f64; 5];
        let r = self.value_and_derivatives_at(&dcoord, &deriv, &mut dval)?;
        Ok((T::from_f64(r), T::from_f64(dval[0])))
    }

    /// `operator()(x, y, z, dd, &dval)`: the value and its derivative along `dd` at a position
    /// of up to three coordinates.
    pub fn at_deriv(&self, x: f64, y: f64, z: f64, dd: u32) -> Result<(T, T), FslError> {
        let nd = self.ndim;
        if nd > 3 || (z != 0.0 && nd < 3) || (y != 0.0 && nd < 2) {
            return Err(splinterpolator_error(
                "operator(): input has wrong dimensionality",
            ));
        }
        if dd as usize > nd - 1 {
            return Err(splinterpolator_error(
                "operator(): derivative specified for invalid direction",
            ));
        }
        let mut deriv = [0u32; 5];
        deriv[dd as usize] = 1;
        let mut dval = [0.0f64; 5];
        let r = self.value_and_derivatives_at(&[x, y, z, 0.0, 0.0], &deriv, &mut dval)?;
        Ok((T::from_f64(r), T::from_f64(dval[0])))
    }

    /// `ValAndDerivs(coord, deriv, rderiv)`: the value and the derivatives along the dimensions
    /// whose `deriv` is non-zero, in dimension order. (FSL builds, but does not throw, an
    /// exception when `rderiv` has another length than the derivatives asked for, and then
    /// writes past it; the derivatives are returned here instead.)
    pub fn val_and_derivs_sel(
        &self,
        coord: &[f32],
        deriv: &[u32],
    ) -> Result<(T, Vec<T>), FslError> {
        if coord.len() != self.ndim || deriv.len() != self.ndim {
            return Err(splinterpolator_error(
                "ValAndDerivs: input has wrong dimensionality",
            ));
        }
        let mut lcoord = [0.0f64; 5];
        let mut lderiv = [0u32; 5];
        let mut nd = 0;
        for i in 0..coord.len() {
            lcoord[i] = f64::from(coord[i]);
            lderiv[i] = u32::from(deriv[i] != 0);
            nd += lderiv[i] as usize;
        }
        let mut dval = [0.0f64; 5];
        let r = self.value_and_derivatives_at(&lcoord, &lderiv, &mut dval)?;
        Ok((
            T::from_f64(r),
            dval[..nd].iter().map(|&v| T::from_f64(v)).collect(),
        ))
    }

    /// `ValAndDerivs(coord, rderiv)`: the value and every first derivative.
    pub fn val_and_derivs_all(&self, coord: &[f32]) -> Result<(T, Vec<T>), FslError> {
        let deriv = vec![1u32; self.ndim];
        self.val_and_derivs_sel(coord, &deriv)
    }

    /// `ValAndDerivs(x, y, z, rderiv)`: the value and its three derivatives (three dimensions).
    pub fn val_and_derivs3(&self, x: f64, y: f64, z: f64) -> Result<(T, [T; 3]), FslError> {
        if self.ndim != 3 {
            return Err(splinterpolator_error(
                "ValAndDerivs: input has wrong dimensionality",
            ));
        }
        let mut dval = [0.0f64; 5];
        let r = self.value_and_derivatives_at(&[x, y, z, 0.0, 0.0], &[1, 1, 1, 0, 0], &mut dval)?;
        Ok((
            T::from_f64(r),
            [
                T::from_f64(dval[0]),
                T::from_f64(dval[1]),
                T::from_f64(dval[2]),
            ],
        ))
    }

    /// `Deriv(indx, dd)`: the continuous derivative along `dd` at a grid point (orders 2 to 7).
    /// (FSL builds, but does not throw, an exception when `indx` has another length than the
    /// dimensions, and then reads past it: refused here.)
    pub fn deriv(&self, indx: &[u32], dd: u32) -> Result<T, FslError> {
        if indx.len() != self.ndim {
            return Err(splinterpolator_error(
                "Deriv: Input indx of wrong dimension",
            ));
        }
        if dd as usize > self.ndim - 1 {
            return Err(splinterpolator_error(
                "Deriv: derivative specified for invalid direction",
            ));
        }
        let mut lindx = [0u32; 5];
        lindx[..self.ndim].copy_from_slice(indx);
        let mut deriv = [0u32; 5];
        deriv[dd as usize] = 1;
        let mut dval = [0.0f64; 5];
        self.derivatives_at_i(&lindx, &deriv, &mut dval)?;
        Ok(T::from_f64(dval[0]))
    }

    /// `DerivXYZ(i, j, k, dd)`.
    pub fn deriv_xyz(&self, i: u32, j: u32, k: u32, dd: u32) -> Result<T, FslError> {
        if self.ndim != 3 || dd > 2 {
            return Err(splinterpolator_error(
                "DerivXYZ: Input has wrong dimensionality",
            ));
        }
        let mut deriv = [0u32; 5];
        deriv[dd as usize] = 1;
        let mut dval = [0.0f64; 5];
        self.derivatives_at_i(&[i, j, k, 0, 0], &deriv, &mut dval)?;
        Ok(T::from_f64(dval[0]))
    }

    /// `Grad3D(i, j, k, ...)`: the three derivatives at a grid point (FSL's check of three
    /// dimensions builds but does not throw its exception: other dimensionalities give their
    /// derivatives and zeros, as FSL does).
    pub fn grad3d(&self, i: u32, j: u32, k: u32) -> Result<[T; 3], FslError> {
        let mut dval = [0.0f64; 5];
        self.derivatives_at_i(&[i, j, k, 0, 0], &[1, 1, 1, 0, 0], &mut dval)?;
        Ok([
            T::from_f64(dval[0]),
            T::from_f64(dval[1]),
            T::from_f64(dval[2]),
        ])
    }

    /// `Grad(indx, grad)`: every derivative at a grid point. (FSL builds, but does not throw,
    /// an exception for an `indx` of another length than the dimensions: refused here.)
    pub fn grad(&self, indx: &[u32]) -> Result<Vec<T>, FslError> {
        if indx.len() != self.ndim {
            return Err(splinterpolator_error(
                "Grad: Input indx or grad of wrong dimension",
            ));
        }
        let mut lindx = [0u32; 5];
        let mut deriv = [0u32; 5];
        for i in 0..self.ndim {
            lindx[i] = indx[i];
            deriv[i] = 1;
        }
        let mut dval = [0.0f64; 5];
        self.derivatives_at_i(&lindx, &deriv, &mut dval)?;
        Ok(dval[..self.ndim].iter().map(|&v| T::from_f64(v)).collect())
    }

    // --------------------------------------------------------------------------------------
    // The sums

    /// `value_at(coord)`.
    fn value_at(&self, coord: &[f64; 5]) -> f64 {
        if self.should_be_zero(coord) {
            return 0.0;
        }
        let mut wgts = [[0.0f64; 8]; 5];
        let mut start = [0i32; 5];
        let ni = self.get_start_indicies(coord, &mut start);
        self.get_wgts(coord, &start, &mut wgts);
        let val = self.value_sum::<Plain>(&start, &wgts, ni);
        if val.is_nan() {
            return self.value_sum::<Sse>(&start, &wgts, ni);
        }
        val
    }

    #[inline(always)]
    fn value_sum<A: Arith>(&self, start: &[i32; 5], wgts: &[[f64; 8]; 5], ni: usize) -> f64 {
        let nd = self.ndim;
        let ext = |d: usize| if nd > d { ni } else { 1 };
        let mut inds = [0i32; 5];
        let mut val = 0.0f64;
        for m in 0..ext(4) {
            inds[4] = start[4] + m as i32;
            for l in 0..ext(3) {
                inds[3] = start[3] + l as i32;
                for k in 0..ext(2) {
                    inds[2] = start[2] + k as i32;
                    let wgt1 = wgts[4][m] * wgts[3][l] * wgts[2][k];
                    for j in 0..ext(1) {
                        inds[1] = start[1] + j as i32;
                        let wgt2 = wgt1 * wgts[1][j];
                        for i in 0..ni {
                            inds[0] = start[0] + i as i32;
                            let c = self.coef_idx(&inds).to_f64();
                            val = A::add(val, A::mul(A::mul(c, wgts[0][i]), wgt2));
                        }
                    }
                }
            }
        }
        val
    }

    /// `value_and_derivatives_at(coord, deriv, dval)`: the value, and in `dval` the derivatives
    /// along the dimensions whose `deriv` is set, in dimension order.
    fn value_and_derivatives_at(
        &self,
        coord: &[f64; 5],
        deriv: &[u32; 5],
        dval: &mut [f64; 5],
    ) -> Result<f64, FslError> {
        if self.should_be_zero(coord) {
            let n = self.n_nonzero(deriv);
            for v in dval.iter_mut().take(n) {
                *v = 0.0;
            }
            return Ok(0.0);
        }
        let mut wgts = [[0.0f64; 8]; 5];
        let mut dwgts = [[0.0f64; 8]; 5];
        let mut start = [0i32; 5];
        let ni = self.get_start_indicies(coord, &mut start);
        self.get_wgts(coord, &start, &mut wgts);
        self.get_dwgts(coord, &start, deriv, &mut dwgts)?;
        let mut dd = [0usize; 5];
        let mut nd = 0;
        for i in 0..self.ndim {
            if deriv[i] != 0 {
                dd[nd] = i;
                nd += 1;
            }
        }
        let val = self.deriv_sum::<Plain>(&start, &wgts, &dwgts, &dd, nd, ni, dval);
        if val.is_nan() || dval[..nd].iter().any(|v| v.is_nan()) {
            return Ok(self.deriv_sum::<Sse>(&start, &wgts, &dwgts, &dd, nd, ni, dval));
        }
        Ok(val)
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn deriv_sum<A: Arith>(
        &self,
        start: &[i32; 5],
        wgts: &[[f64; 8]; 5],
        dwgts: &[[f64; 8]; 5],
        dd: &[usize; 5],
        nd: usize,
        ni: usize,
        dval: &mut [f64; 5],
    ) -> f64 {
        let ndim = self.ndim;
        let ext = |d: usize| if ndim > d { ni } else { 1 };
        for v in dval.iter_mut().take(nd) {
            *v = 0.0;
        }
        let mut inds = [0i32; 5];
        let mut dwgt1 = [0.0f64; 5];
        let mut dwgt2 = [0.0f64; 5];
        let mut val = 0.0f64;
        for m in 0..ext(4) {
            inds[4] = start[4] + m as i32;
            for l in 0..ext(3) {
                inds[3] = start[3] + l as i32;
                for k in 0..ext(2) {
                    inds[2] = start[2] + k as i32;
                    let wgt1 = wgts[4][m] * wgts[3][l] * wgts[2][k];
                    get_dwgt1(wgts, dwgts, dd, nd, k, l, m, wgt1, &mut dwgt1);
                    for j in 0..ext(1) {
                        inds[1] = start[1] + j as i32;
                        let wgt2 = wgt1 * wgts[1][j];
                        for d in 0..nd {
                            dwgt2[d] = if dd[d] == 1 {
                                dwgt1[d] * dwgts[1][j]
                            } else {
                                dwgt1[d] * wgts[1][j]
                            };
                        }
                        for i in 0..ni {
                            inds[0] = start[0] + i as i32;
                            let c = self.coef_idx(&inds).to_f64();
                            val = A::add(val, A::mul(A::mul(c, wgts[0][i]), wgt2));
                            for d in 0..nd {
                                let add = if dd[d] == 0 {
                                    A::mul(A::mul(c, dwgts[0][i]), dwgt2[d])
                                } else {
                                    A::mul(A::mul(c, wgts[0][i]), dwgt2[d])
                                };
                                dval[d] = A::add(dval[d], add);
                            }
                        }
                    }
                }
            }
        }
        val
    }

    /// `derivatives_at_i(indx, deriv, dval)`: the derivatives at a grid point (the
    /// coefficients' indices mapped by `indx2indx` in every dimension, `Zeros` mirrored).
    fn derivatives_at_i(
        &self,
        indx: &[u32; 5],
        deriv: &[u32; 5],
        dval: &mut [f64; 5],
    ) -> Result<(), FslError> {
        let mut wgts = [[0.0f64; 8]; 5];
        let mut dwgts = [[0.0f64; 8]; 5];
        let mut inds = [0i32; 5];
        let ni = self.get_start_indicies_at_i(indx, &mut inds);
        self.get_wgts_at_i(indx, &inds, &mut wgts);
        self.get_dwgts_at_i(indx, &inds, deriv, &mut dwgts)?;
        let mut dd = [0usize; 5];
        let mut nd = 0;
        for i in 0..self.ndim {
            if deriv[i] != 0 {
                dd[nd] = i;
                nd += 1;
            }
        }
        self.at_i_sum::<Plain>(&inds, &wgts, &dwgts, &dd, nd, ni, dval);
        if dval[..nd].iter().any(|v| v.is_nan()) {
            self.at_i_sum::<Sse>(&inds, &wgts, &dwgts, &dd, nd, ni, dval);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn at_i_sum<A: Arith>(
        &self,
        inds: &[i32; 5],
        wgts: &[[f64; 8]; 5],
        dwgts: &[[f64; 8]; 5],
        dd: &[usize; 5],
        nd: usize,
        ni: usize,
        dval: &mut [f64; 5],
    ) {
        let ndim = self.ndim;
        let ext = |d: usize| if ndim > d { ni } else { 1 };
        for v in dval.iter_mut().take(nd) {
            *v = 0.0;
        }
        let mut dwgt1 = [0.0f64; 5];
        let mut dwgt2 = [0.0f64; 5];
        for m in 0..ext(4) {
            for l in 0..ext(3) {
                for k in 0..ext(2) {
                    let wgt1 = wgts[4][m] * wgts[3][l] * wgts[2][k];
                    get_dwgt1(wgts, dwgts, dd, nd, k, l, m, wgt1, &mut dwgt1);
                    let linear1 = self.indx2linear(
                        inds[2] + k as i32,
                        inds[3] + l as i32,
                        inds[4] + m as i32,
                    );
                    for j in 0..ext(1) {
                        for d in 0..nd {
                            dwgt2[d] = if dd[d] == 1 {
                                dwgt1[d] * dwgts[1][j]
                            } else {
                                dwgt1[d] * wgts[1][j]
                            };
                        }
                        let linear2 = self.add2linear(linear1, inds[1] + j as i32);
                        for i in 0..ni {
                            let c =
                                self.coef[linear2 + self.indx2indx(inds[0] + i as i32, 0)].to_f64();
                            for d in 0..nd {
                                let add = if dd[d] == 0 {
                                    A::mul(A::mul(c, dwgts[0][i]), dwgt2[d])
                                } else {
                                    A::mul(A::mul(c, wgts[0][i]), dwgt2[d])
                                };
                                dval[d] = A::add(dval[d], add);
                            }
                        }
                    }
                }
            }
        }
    }

    // --------------------------------------------------------------------------------------
    // Indices and weights

    /// `get_start_indicies(coord, sinds)`: the first coefficient with a non-zero weight in each
    /// dimension; the number of weights per dimension.
    fn get_start_indicies(&self, coord: &[f64; 5], sinds: &mut [i32; 5]) -> usize {
        let ni = self.order + 1;
        for i in 0..self.ndim {
            // A double assigned to an int (x86-64's conversion: INT_MIN when out of range).
            sinds[i] = if ni % 2 == 1 {
                cnum::f64_to_i32((coord[i] + 0.5).floor() - f64::from(ni / 2))
            } else {
                cnum::f64_to_i32(coord[i].ceil() - f64::from(ni / 2))
            };
        }
        for s in sinds.iter_mut().skip(self.ndim) {
            *s = 0;
        }
        ni as usize
    }

    /// `get_start_indicies_at_i(indx, sinds)`.
    fn get_start_indicies_at_i(&self, indx: &[u32; 5], sinds: &mut [i32; 5]) -> usize {
        let ni = if self.order % 2 == 1 {
            self.order
        } else {
            self.order + 1
        };
        for i in 0..self.ndim {
            // unsigned arithmetic assigned to an int
            sinds[i] = indx[i].wrapping_sub(self.order / 2) as i32;
        }
        for s in sinds.iter_mut().skip(self.ndim) {
            *s = 0;
        }
        ni as usize
    }

    /// `get_wgts(coord, sinds, wgts)`.
    fn get_wgts(&self, coord: &[f64; 5], sinds: &[i32; 5], wgts: &mut [[f64; 8]; 5]) {
        let ni = self.order as usize + 1;
        for dim in 0..self.ndim {
            for i in 0..ni {
                wgts[dim][i] =
                    self.get_wgt(coord[dim] - f64::from(sinds[dim].wrapping_add(i as i32)));
            }
        }
        for w in wgts.iter_mut().skip(self.ndim) {
            w[0] = 1.0;
        }
    }

    /// `get_wgts_at_i(indx, sinds, wgts)`.
    fn get_wgts_at_i(&self, indx: &[u32; 5], sinds: &[i32; 5], wgts: &mut [[f64; 8]; 5]) {
        let ni = if self.order % 2 == 1 {
            self.order
        } else {
            self.order + 1
        } as usize;
        for dim in 0..self.ndim {
            for i in 0..ni {
                let rel = indx[dim].wrapping_sub(sinds[dim].wrapping_add(i as i32) as u32) as i32;
                wgts[dim][i] = self.get_wgt_at_i(rel);
            }
        }
        for w in wgts.iter_mut().skip(self.ndim) {
            w[0] = 1.0;
        }
    }

    /// `get_dwgts(coord, sinds, deriv, dwgts)`.
    fn get_dwgts(
        &self,
        coord: &[f64; 5],
        sinds: &[i32; 5],
        deriv: &[u32; 5],
        dwgts: &mut [[f64; 8]; 5],
    ) -> Result<(), FslError> {
        let ni = self.order as usize + 1;
        for dim in 0..self.ndim {
            if deriv[dim] != 0 {
                match self.order {
                    1 => {
                        // "Not correct on original gridpoints"
                        dwgts[dim][0] = -1.0;
                        dwgts[dim][1] = 1.0;
                    }
                    2..=7 => {
                        for i in 0..ni {
                            dwgts[dim][i] = self.get_dwgt(
                                coord[dim] - f64::from(sinds[dim].wrapping_add(i as i32)),
                            )?;
                        }
                    }
                    _ => return Err(splinterpolator_error("get_dwgts: invalid order spline")),
                }
            }
        }
        Ok(())
    }

    /// `get_dwgts_at_i(indx, sinds, deriv, dwgts)`.
    fn get_dwgts_at_i(
        &self,
        indx: &[u32; 5],
        sinds: &[i32; 5],
        deriv: &[u32; 5],
        dwgts: &mut [[f64; 8]; 5],
    ) -> Result<(), FslError> {
        let ni = if self.order % 2 == 1 {
            self.order
        } else {
            self.order + 1
        } as usize;
        for dim in 0..self.ndim {
            if deriv[dim] != 0 {
                match self.order {
                    2..=7 => {
                        for i in 0..ni {
                            let rel = indx[dim]
                                .wrapping_sub(sinds[dim].wrapping_add(i as i32) as u32)
                                as i32;
                            dwgts[dim][i] = self.get_dwgt_at_i(rel)?;
                        }
                    }
                    _ => {
                        return Err(splinterpolator_error(
                            "get_dwgts_at_i: invalid order spline",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// `get_wgt_at_i(i)`: a spline's weight at an integer distance from its centre.
    fn get_wgt_at_i(&self, i: i32) -> f64 {
        let ai = i.unsigned_abs();
        match self.order {
            // FSL's value for orders 0 and 1 (1 off the centre, 0 at it); never used, as
            // derivatives at grid points need order 2 or more.
            0 | 1 => {
                if ai != 0 {
                    1.0
                } else {
                    0.0
                }
            }
            2 => match ai {
                0 => 0.75,
                1 => 0.125,
                _ => 0.0,
            },
            3 => match ai {
                0 => 0.666666666666667,
                1 => 0.166666666666667,
                _ => 0.0,
            },
            4 => match ai {
                0 => 0.598958333333333,
                1 => 0.197916666666667,
                2 => 0.002604166666667,
                _ => 0.0,
            },
            5 => match ai {
                0 => 0.55,
                1 => 0.216666666666667,
                2 => 0.008333333333333,
                _ => 0.0,
            },
            6 => match ai {
                0 => 0.511024305555556,
                1 => 0.228797743055556,
                2 => 0.015668402777779,
                3 => 8.680555555555556e-05,
                _ => 0.0,
            },
            _ => match ai {
                0 => 0.479365079365079,
                1 => 0.236309523809524,
                2 => 0.023809523809524,
                3 => 1.984126984126984e-04,
                _ => 0.0,
            },
        }
    }

    /// `get_dwgt_at_i(i)`: a spline's derivative at an integer distance from its centre.
    fn get_dwgt_at_i(&self, i: i32) -> Result<f64, FslError> {
        let ai = i.wrapping_abs();
        let sign = f64::from(if ai != 0 { i / ai } else { 1 });
        let v = |a1: f64, a2: f64, a3: f64| match ai {
            0 => 0.0,
            1 => sign * a1,
            2 => sign * a2,
            3 => sign * a3,
            _ => 0.0,
        };
        Ok(match self.order {
            2 | 3 => v(-0.5, 0.0, 0.0),
            4 => v(-0.458333333333333, -0.020833333333333, 0.0),
            5 => v(-0.416666666666667, -0.041666666666667, 0.0),
            6 => v(
                -0.376302083333333,
                -0.061458333333334,
                -2.604166666666667e-04,
            ),
            7 => v(-0.340277777777778, -0.077777777777778, -0.001388888888889),
            _ => return Err(splinterpolator_error("get_dwgt: invalid order spline")),
        })
    }

    /// `get_wgt(x)`: a spline's weight at distance `x` from its centre.
    fn get_wgt(&self, x: f64) -> f64 {
        let mut ax = x.abs();
        match self.order {
            0 => {
                if ax < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            1 => {
                if ax < 1.0 {
                    1.0 - ax
                } else {
                    0.0
                }
            }
            2 => {
                if ax < 0.5 {
                    0.75 - ax * ax
                } else if ax < 1.5 {
                    0.5 * (1.5 - ax) * (1.5 - ax)
                } else {
                    0.0
                }
            }
            3 => {
                if ax < 1.0 {
                    2.0 / 3.0 + 0.5 * ax * ax * (ax - 2.0)
                } else if ax < 2.0 {
                    ax = 2.0 - ax;
                    (1.0 / 6.0) * (ax * ax * ax)
                } else {
                    0.0
                }
            }
            4 => {
                if ax < 0.5 {
                    ax *= ax;
                    (115.0 / 192.0) + ax * ((2.0 * ax - 5.0) / 8.0)
                } else if ax < 1.5 {
                    (55.0 / 96.0) + ax * (ax * (ax * ((5.0 - ax) / 6.0) - 1.25) + 5.0 / 24.0)
                } else if ax < 2.5 {
                    ax -= 2.5;
                    ax *= ax;
                    (1.0 / 24.0) * ax * ax
                } else {
                    0.0
                }
            }
            5 => {
                if ax < 1.0 {
                    let xx = ax * ax;
                    0.55 + xx * (xx * ((3.0 - ax) / 12.0) - 0.5)
                } else if ax < 2.0 {
                    0.425 + ax * (ax * (ax * (ax * ((ax - 9.0) / 24.0) + 1.25) - 1.75) + 0.625)
                } else if ax < 3.0 {
                    ax = 3.0 - ax;
                    let xx = ax * ax;
                    (1.0 / 120.0) * ax * xx * xx
                } else {
                    0.0
                }
            }
            6 => {
                if ax < 0.5 {
                    ax *= ax;
                    (5887.0 / 11520.0) + ax * (ax * ((21.0 - 4.0 * ax) / 144.0) - 77.0 / 192.0)
                } else if ax < 1.5 {
                    7861.0 / 15360.0
                        + ax * (ax
                            * (ax * (ax * (ax * ((ax - 7.0) / 48.0) + 0.328125) - 35.0 / 288.0)
                                - 91.0 / 256.0)
                            - 7.0 / 768.0)
                } else if ax < 2.5 {
                    1379.0 / 7680.0
                        + ax * (ax
                            * (ax * (ax * (ax * ((14.0 - ax) / 120.0) - 0.65625) + 133.0 / 72.0)
                                - 2.5703125)
                            + 1267.0 / 960.0)
                } else if ax < 3.5 {
                    ax -= 3.5;
                    // `ax *= ax*ax`
                    ax = ax * (ax * ax);
                    (1.0 / 720.0) * ax * ax
                } else {
                    0.0
                }
            }
            _ => {
                if ax < 1.0 {
                    let xx = ax * ax;
                    151.0 / 315.0 + xx * (xx * (xx * ((ax - 4.0) / 144.0) + 1.0 / 9.0) - 1.0 / 3.0)
                } else if ax < 2.0 {
                    103.0 / 210.0
                        + ax * (ax
                            * (ax
                                * (ax * (ax * (ax * ((12.0 - ax) / 240.0) - 7.0 / 30.0) + 0.5)
                                    - 7.0 / 18.0)
                                - 0.1)
                            - 7.0 / 90.0)
                } else if ax < 3.0 {
                    ax * (ax
                        * (ax
                            * (ax
                                * (ax * (ax * ((ax - 20.0) / 720.0) + 7.0 / 30.0) - 19.0 / 18.0)
                                + 49.0 / 18.0)
                            - 23.0 / 6.0)
                        + 217.0 / 90.0)
                        - 139.0 / 630.0
                } else if ax < 4.0 {
                    ax = 4.0 - ax;
                    let xxx = ax * ax * ax;
                    (1.0 / 5040.0) * ax * xxx * xxx
                } else {
                    0.0
                }
            }
        }
    }

    /// `get_dwgt(x)`: a spline's first derivative at distance `x` from its centre.
    fn get_dwgt(&self, x: f64) -> Result<f64, FslError> {
        let mut ax = x.abs();
        // `(ax) ? static_cast<int>(x/ax) : 1` (a NaN is true; its conversion INT_MIN)
        let sign = f64::from(if ax != 0.0 || ax.is_nan() {
            cnum::f64_to_i32(x / ax)
        } else {
            1
        });
        Ok(match self.order {
            2 => {
                if ax < 0.5 {
                    sign * -2.0 * ax
                } else if ax < 1.5 {
                    sign * (-1.5 + ax)
                } else {
                    0.0
                }
            }
            3 => {
                if ax < 1.0 {
                    sign * (1.5 * ax * ax - 2.0 * ax)
                } else if ax < 2.0 {
                    ax = 2.0 - ax;
                    sign * -0.5 * ax * ax
                } else {
                    0.0
                }
            }
            4 => {
                if ax < 0.5 {
                    sign * (ax * ax * ax - 1.25 * ax)
                } else if ax < 1.5 {
                    sign * (5.0 / 24.0 - ax * (2.5 - ax * (2.5 - (2.0 / 3.0) * ax)))
                } else if ax < 2.5 {
                    ax -= 2.5;
                    sign * (1.0 / 6.0) * ax * ax * ax
                } else {
                    0.0
                }
            }
            5 => {
                if ax < 1.0 {
                    sign * ax * (ax * (ax * (1.0 - (5.0 / 12.0) * ax)) - 1.0)
                } else if ax < 2.0 {
                    sign * (0.625 - ax * (3.5 - ax * (3.75 - ax * (1.5 - (5.0 / 24.0) * ax))))
                } else if ax < 3.0 {
                    ax -= 3.0;
                    ax *= ax;
                    sign * (-1.0 / 24.0) * ax * ax
                } else {
                    0.0
                }
            }
            6 => {
                if ax < 0.5 {
                    let xx = ax * ax;
                    sign * ax * (xx * ((7.0 / 12.0) - (1.0 / 6.0) * xx) - (77.0 / 96.0))
                } else if ax < 1.5 {
                    let xx = ax * ax;
                    sign * (ax * (xx * (0.1250 * xx + 1.3125) - 0.7109375)
                        - xx * ((35.0 / 48.0) * xx + (35.0 / 96.0))
                        - (7.0 / 768.0))
                } else if ax < 2.5 {
                    let xx = ax * ax;
                    sign * ((1267.0 / 960.0)
                        - ax * (xx * (0.05 * xx + (21.0 / 8.0)) + (329.0 / 64.0))
                        + xx * ((7.0 / 12.0) * xx + (133.0 / 24.0)))
                } else if ax < 3.5 {
                    ax -= 3.5;
                    let xx = ax * ax;
                    sign * (1.0 / 120.0) * xx * xx * ax
                } else {
                    0.0
                }
            }
            7 => {
                if ax < 1.0 {
                    let xx = ax * ax;
                    sign * ax
                        * (xx * (xx * ((7.0 / 144.0) * ax - (1.0 / 6.0)) + 4.0 / 9.0) - 2.0 / 3.0)
                } else if ax < 2.0 {
                    let xx = ax * ax;
                    sign * (ax * (xx * (xx * 0.3 + 2.0) - 0.2)
                        - xx * (xx * (xx * (7.0 / 240.0) + (7.0 / 6.0)) + (7.0 / 6.0))
                        - (7.0 / 90.0))
                } else if ax < 3.0 {
                    let xx = ax * ax;
                    sign * (1.0 / 720.0)
                        * (xx - 4.0 * ax + 2.0)
                        * (7.0 * xx * xx - 92.0 * xx * ax + 458.0 * xx - 1024.0 * ax + 868.0)
                } else if ax < 4.0 {
                    ax = 4.0 - ax;
                    ax = ax * ax * ax;
                    sign * (-1.0 / 720.0) * ax * ax
                } else {
                    0.0
                }
            }
            _ => return Err(splinterpolator_error("get_dwgt: invalid order spline")),
        })
    }

    /// `indx2indx(indx, d)`: an index outside the grid mapped into it by dimension `d`'s
    /// extrapolation (`Zeros` and `SoftZeros` mirrored, as `Mirror`).
    fn indx2indx(&self, indx: i32, d: usize) -> usize {
        if d > self.ndim - 1 {
            return 0;
        }
        let dim = i64::from(self.dim[d]);
        let i = i64::from(indx);
        if i >= 0 && i < dim {
            return i as usize;
        }
        (match self.et[d] {
            ExtrapolationType::Constant | ExtrapolationType::CorrectConstant => {
                if i < 0 {
                    0
                } else {
                    dim - 1
                }
            }
            ExtrapolationType::Zeros | ExtrapolationType::SoftZeros | ExtrapolationType::Mirror => {
                // FSL's loops `indx = 2*dim*((indx+1)/dim) - 1 - indx` (below 0) and
                // `indx = 2*dim*(indx/dim) - 1 - indx` (from dim): a mirror of period 2*dim
                // that repeats the edge values.
                let p = i.rem_euclid(2 * dim);
                if p < dim { p } else { 2 * dim - 1 - p }
            }
            ExtrapolationType::Periodic => i.rem_euclid(dim),
        }) as usize
    }

    /// `indx2linear(k, l, m)`: the linear index of the start of row `(0, k, l, m)` of slice
    /// `k` (0 for fewer than three dimensions).
    fn indx2linear(&self, k: i32, l: i32, m: i32) -> usize {
        if self.ndim < 3 {
            return 0;
        }
        let d = |i: usize| self.dim[i] as usize;
        let mut lindx = 0usize;
        if self.ndim > 4 {
            lindx = self.indx2indx(m, 4);
        }
        if self.ndim > 3 {
            lindx = d(3) * lindx + self.indx2indx(l, 3);
        }
        d(0) * d(1) * (d(2) * lindx + self.indx2indx(k, 2))
    }

    /// `add2linear(lin, j)`.
    fn add2linear(&self, lin: usize, j: i32) -> usize {
        if self.ndim < 2 {
            lin
        } else {
            lin + self.dim[0] as usize * self.indx2indx(j, 1)
        }
    }

    /// `coef(indx)`: the coefficient at `indx`, 0 when a `Zeros`/`SoftZeros` dimension's index
    /// is outside the grid, else the index mapped by `indx2indx`.
    #[inline]
    fn coef_idx(&self, indx: &[i32; 5]) -> T {
        let mut idx = [0usize; 5];
        for i in 0..self.ndim {
            let inside = indx[i] >= 0 && (indx[i] as i64) < i64::from(self.dim[i]);
            if inside {
                idx[i] = indx[i] as usize;
            } else {
                match self.et[i] {
                    ExtrapolationType::SoftZeros | ExtrapolationType::Zeros => return T::default(),
                    _ => idx[i] = self.indx2indx(indx[i], i),
                }
            }
        }
        let mut lindx = idx[self.ndim - 1];
        for i in (0..self.ndim - 1).rev() {
            lindx = self.dim[i] as usize * lindx + idx[i];
        }
        self.coef[lindx]
    }

    /// `should_be_zero(coord)`: a `Zeros` dimension's coordinate outside `[0, size - 1]`.
    fn should_be_zero(&self, coord: &[f64; 5]) -> bool {
        (0..self.ndim).any(|i| {
            self.et[i] == ExtrapolationType::Zeros
                && (coord[i] < 0.0 || coord[i] > f64::from(self.dim[i] - 1))
        })
    }

    fn n_nonzero(&self, vec: &[u32; 5]) -> usize {
        (0..self.ndim).filter(|&i| vec[i] != 0).count()
    }

    // --------------------------------------------------------------------------------------
    // The deconvolution

    /// `deconv_along(dim)`: every line along `dim` deconvolved, the other dimensions in order
    /// (the lowest fastest).
    fn deconv_along(&mut self, dim: usize) {
        let mut rdim = [1usize; 4];
        let mut rstep = [1usize; 4];
        let mut mdim = 1usize;
        let mut mstep = 1usize;
        let mut j = 0;
        let mut ss = 1usize;
        for i in 0..5 {
            if i == dim {
                mdim = self.dim[i] as usize;
                mstep = ss;
            } else {
                rdim[j] = self.dim[i] as usize;
                rstep[j] = ss;
                j += 1;
            }
            ss *= self.dim[i] as usize;
        }
        let et = self.et[dim];
        let mut col = vec![0.0f64; mdim];
        let mut line = vec![0.0f64; mdim];
        for l in 0..rdim[3] {
            for k in 0..rdim[2] {
                for j in 0..rdim[1] {
                    for i in 0..rdim[0] {
                        let start = l * rstep[3] + k * rstep[2] + j * rstep[1] + i * rstep[0];
                        for (t, c) in col.iter_mut().enumerate() {
                            *c = self.coef[start + t * mstep].to_f64();
                        }
                        line.copy_from_slice(&col);
                        deconv::<Plain>(&mut col, self.order, et, self.prec);
                        if col.iter().any(|c| c.is_nan()) {
                            col.copy_from_slice(&line);
                            deconv::<Sse>(&mut col, self.order, et, self.prec);
                        }
                        for (t, &c) in col.iter().enumerate() {
                            self.coef[start + t * mstep] = T::from_column(c);
                        }
                    }
                }
            }
        }
    }
}

/// `get_dwgt1(...)`: the weight of slice `k` (and the higher dimensions) for each derivative.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn get_dwgt1(
    wgts: &[[f64; 8]; 5],
    dwgts: &[[f64; 8]; 5],
    dd: &[usize; 5],
    nd: usize,
    k: usize,
    l: usize,
    m: usize,
    wgt1: f64,
    dwgt1: &mut [f64; 5],
) {
    for i in 0..nd {
        dwgt1[i] = match dd[i] {
            2 => wgts[4][m] * wgts[3][l] * dwgts[2][k],
            3 => wgts[4][m] * dwgts[3][l] * wgts[2][k],
            4 => dwgts[4][m] * wgts[3][l] * wgts[2][k],
            _ => wgt1,
        };
    }
}

/// `SplineColumn::get_poles(order, z, sf)`: the poles and the scale factor (orders 2 to 7).
// The poles of orders 6 and 7 are written as FSL writes them (rounded to double as C rounds).
#[allow(clippy::excessive_precision)]
fn get_poles(order: u32) -> (Vec<f64>, u32) {
    match order {
        2 => (vec![2.0 * 2.0f64.sqrt() - 3.0], 8),
        3 => (vec![3.0f64.sqrt() - 2.0], 6),
        4 => (
            vec![
                (664.0 - 438976.0f64.sqrt()).sqrt() + 304.0f64.sqrt() - 19.0,
                (664.0 + 438976.0f64.sqrt()).sqrt() - 304.0f64.sqrt() - 19.0,
            ],
            384,
        ),
        5 => (
            vec![
                (135.0 / 2.0 - (17745.0f64 / 4.0).sqrt()).sqrt() + (105.0f64 / 4.0).sqrt()
                    - 13.0 / 2.0,
                (135.0 / 2.0 + (17745.0f64 / 4.0).sqrt()).sqrt()
                    - (105.0f64 / 4.0).sqrt()
                    - 13.0 / 2.0,
            ],
            120,
        ),
        6 => (
            vec![
                -0.488_294_589_303_044_755_130_118_038_883_789_062_112_279_161_239_38,
                -0.081_679_271_076_237_512_597_937_765_737_059_080_653_379_610_398_148,
                -0.001_414_151_808_325_817_751_087_243_976_558_592_527_864_169_055_346_7,
            ],
            46080,
        ),
        _ => (
            vec![
                -0.535_280_430_796_438_165_542_403_781_681_646_071_833_923_152_342_69,
                -0.122_554_615_192_326_690_515_272_264_359_357_343_605_486_549_427_30,
                -0.009_148_694_809_608_276_928_593_021_651_647_853_415_692_563_954_599_4,
            ],
            5040,
        ),
    }
}

/// `SplineColumn::Deconv(order, et, prec)`: one forward and one backward sweep per pole, then
/// the scale factor.
fn deconv<A: Arith>(col: &mut [f64], order: u32, et: ExtrapolationType, prec: f64) {
    let sz = col.len();
    let (z, sf) = get_poles(order);
    for &zp in &z {
        col[0] = init_fwd_sweep::<A>(col, zp, et, prec);
        let lv = col[sz - 1];
        for i in 1..sz {
            col[i] = A::add(col[i], A::mul(zp, col[i - 1]));
        }
        col[sz - 1] = init_bwd_sweep::<A>(col, zp, lv, et, prec);
        for i in (0..sz - 1).rev() {
            col[i] = A::mul(zp, A::sub(col[i + 1], col[i]));
        }
    }
    let sf = f64::from(sf);
    for c in col.iter_mut() {
        *c = A::mul(*c, sf);
    }
}

/// The number of terms of the sweeps' initial sums: `(unsigned)(log(prec)/log(|z|) + 1.5)`, at
/// most the column's length.
fn sweep_terms(z: f64, prec: f64, sz: usize) -> usize {
    let n = ((larmorx_core::math::log(prec) / larmorx_core::math::log(z.abs())) + 1.5) as u32;
    (n as usize).min(sz)
}

/// `SplineColumn::init_fwd_sweep(z, et, prec)`.
fn init_fwd_sweep<A: Arith>(col: &[f64], z: f64, et: ExtrapolationType, prec: f64) -> f64 {
    let sz = col.len();
    let n = sweep_terms(z, prec, sz);
    let mut iv = col[0];
    let mut z2i = z;
    match et {
        ExtrapolationType::Periodic => {
            for i in 1..n {
                iv = A::add(iv, A::mul(z2i, col[sz - i]));
                z2i *= z;
            }
        }
        ExtrapolationType::Mirror | ExtrapolationType::Constant | ExtrapolationType::Zeros => {
            for &c in col.iter().take(n).skip(1) {
                iv = A::add(iv, A::mul(z2i, c));
                z2i *= z;
            }
        }
        ExtrapolationType::SoftZeros | ExtrapolationType::CorrectConstant => {
            for &c in col.iter().take(n) {
                iv = A::add(iv, A::mul(z2i, c));
                z2i *= z;
            }
        }
    }
    iv
}

/// `SplineColumn::init_bwd_sweep(z, lv, et, prec)`.
fn init_bwd_sweep<A: Arith>(col: &[f64], z: f64, lv: f64, et: ExtrapolationType, prec: f64) -> f64 {
    let sz = col.len();
    match et {
        ExtrapolationType::Periodic => {
            let n = sweep_terms(z, prec, sz);
            let mut iv = A::mul(z, col[sz - 1]);
            let mut z2i = z * z;
            for i in 1..n {
                iv = A::add(iv, A::mul(z2i, col[i - 1]));
                z2i *= z;
            }
            A::div(iv, z2i - 1.0)
        }
        ExtrapolationType::Mirror | ExtrapolationType::Constant | ExtrapolationType::Zeros => {
            A::mul(-z / (1.0 - z * z), A::sub(A::mul(2.0, col[sz - 1]), lv))
        }
        ExtrapolationType::SoftZeros | ExtrapolationType::CorrectConstant => {
            A::mul(z / (z - 1.0), col[sz - 1])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interp::SplineCoefs;
    use crate::volume::{Extrapolation, Interpolation, Volume};

    /// The closed form of `indx2indx` is FSL's loops wherever their arithmetic stays in `int`.
    #[test]
    fn indx2indx_loops() {
        fn tdiv(a: i64, b: i64) -> i64 {
            a / b // Rust's division truncates toward zero, as C's
        }
        for dim in [1i64, 2, 3, 7, 32, 64] {
            let s = Splinterpolator::<f32>::new_uniform(
                &vec![0.0; dim as usize],
                &[dim as u32],
                ExtrapolationType::Mirror,
                3,
            )
            .unwrap();
            let p = Splinterpolator::<f32>::new_uniform(
                &vec![0.0; dim as usize],
                &[dim as u32],
                ExtrapolationType::Periodic,
                3,
            )
            .unwrap();
            for start in -20 * dim..20 * dim {
                let mut i = start;
                while i < 0 {
                    i = 2 * dim * tdiv(i + 1, dim) - 1 - i;
                }
                while i >= dim {
                    i = 2 * dim * tdiv(i, dim) - 1 - i;
                }
                assert_eq!(
                    s.indx2indx(start as i32, 0),
                    i as usize,
                    "mirror {dim} {start}"
                );
                let mut q = start;
                while q < 0 {
                    q += dim;
                }
                while q >= dim {
                    q -= dim;
                }
                assert_eq!(
                    p.indx2indx(start as i32, 0),
                    q as usize,
                    "periodic {dim} {start}"
                );
            }
        }
    }

    /// For order 3 and the extrapolations mcflirt, FLIRT and FAST meet, the earlier translation
    /// (`interp::SplineCoefs`) and this one give the same coefficients and values.
    #[test]
    fn same_as_spline_coefs() {
        let (nx, ny, nz) = (7usize, 6usize, 5usize);
        let mut seed = 12345u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / f64::from(1u32 << 31)) as f32
        };
        let data: Vec<f32> = (0..nx * ny * nz).map(|_| next() * 100.0 - 20.0).collect();
        for (ext, et) in [
            (Extrapolation::Zeropad, ExtrapolationType::Zeros),
            (Extrapolation::Extraslice, ExtrapolationType::Constant),
            (Extrapolation::Mirror, ExtrapolationType::Mirror),
        ] {
            let mut v: Volume = Volume::new(nx as i64, ny as i64, nz as i64, 1);
            v.data.copy_from_slice(&data);
            v.interp = Interpolation::Spline;
            v.extrap = ext;
            let old = SplineCoefs::new(&v);
            let new =
                Splinterpolator::new_uniform(&data, &[nx as u32, ny as u32, nz as u32], et, 3)
                    .unwrap();
            for (a, b) in old.coef.iter().zip(new.coefs()) {
                assert_eq!(a.to_bits(), b.to_bits());
            }
            for _ in 0..200 {
                let c = [
                    f64::from(next() * 9.0 - 1.0),
                    f64::from(next() * 8.0 - 1.0),
                    f64::from(next() * 7.0 - 1.0),
                ];
                let a = old.value_at(c);
                let b = new.value_at(&[c[0], c[1], c[2], 0.0, 0.0]);
                assert_eq!(a.to_bits(), b.to_bits(), "{ext:?} {c:?}");
            }
        }
    }
}
