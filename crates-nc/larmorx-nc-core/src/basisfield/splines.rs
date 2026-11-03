// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-basisfield 2203.4: fsl_splines.h). FSL is Copyright
// (C) 2007 University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `BASISFIELD::Spline1D<double>` and `Spline3D<double>` (fsl_splines.h): the quadratic and cubic
//! B-spline kernels sampled at the voxels of a field whose knots are an integer number of voxels
//! apart, the ranges of voxels and of overlapping splines, and the products of two kernels that
//! fill `JᵀJ` and the regularisers' Hessians.
//!
//! Unsigned arithmetic wraps where C++'s `unsigned int` does (a spline left of the field has a
//! negative offset computed in `unsigned int`, then read as `int`).

use crate::volume::FslError;

/// `FslSplinesException(msg)`: `what()` is `"FslSplines::" + msg`.
pub fn splines_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "BASISFIELD::FslSplinesException",
        what: format!("FslSplines::{msg}"),
    }
}

/// `Spline1D<double>`.
#[derive(Clone, Debug)]
pub struct Spline1D {
    order: u32,
    ksp: u32,
    deriv: u32,
    n: u32,
    sp: Vec<f64>,
    /// larmorx (FSL-145 corrected): the quadratic splines' coefficient count grows on every
    /// axis, short ones included ([`Spline1D::ncoef`]); `false` is FSL's count.
    general: bool,
}

impl Spline1D {
    /// `Spline1D(order, ksp, deriv)`.
    pub fn new(order: u32, ksp: u32, deriv: u32) -> Result<Spline1D, FslError> {
        let n = spline_size(order, ksp);
        let sp = spline_kernel(order, ksp, deriv, n);
        if !(2..=3).contains(&order) {
            return Err(splines_error(
                "Spline1D::Spline1D: only orders 2 and 3 implemented",
            ));
        }
        if ksp < 1 {
            return Err(splines_error(
                "Spline1D::Spline1D: knot-spacing has to be > 0",
            ));
        }
        if deriv > order {
            return Err(splines_error(
                "Spline1D::Spline1D: only differentiable order times",
            ));
        }
        Ok(Spline1D {
            order,
            ksp,
            deriv,
            n,
            sp,
            general: false,
        })
    }

    /// The same spline with the quadratic coefficient count corrected (`true`, FSL-145) or
    /// FSL's (`false`, as [`Spline1D::new`] makes it).
    #[must_use]
    pub fn with_general_count(mut self, on: bool) -> Spline1D {
        self.general = on;
        self
    }

    /// Whether the coefficient count is the corrected one (FSL-145).
    pub fn general_count(&self) -> bool {
        self.general
    }

    pub fn order(&self) -> u32 {
        self.order
    }
    pub fn knot_spacing(&self) -> u32 {
        self.ksp
    }
    pub fn derivative_order(&self) -> u32 {
        self.deriv
    }
    pub fn kernel_size(&self) -> u32 {
        self.n
    }
    /// `operator[](indx)` (zero offset).
    #[inline]
    pub fn at(&self, i: u32) -> f64 {
        self.sp[i as usize]
    }
    pub fn kernel(&self) -> &[f64] {
        &self.sp
    }

    /// `FullFOV(csz)`.
    pub fn full_fov(&self, csz: u32) -> u32 {
        if self.n % 2 == 1 {
            (csz - 1) * self.ksp + 1 + self.n - 1
        } else {
            (csz - 1) * self.ksp + 1 + self.n
        }
    }

    /// `NCoef(isz)`; with [`Spline1D::with_general_count`], the corrected quadratic count
    /// (FSL-145).
    pub fn ncoef(&self, isz: u32) -> u32 {
        if self.general {
            self.ncoef_general(isz)
        } else {
            self.ncoef_fsl(isz)
        }
    }

    /// FSL's `NCoef(isz)`: `OffsetIntoField(nc)-KernelSize()/2 < isz-1` is an int minus an
    /// unsigned int, compared as unsigned, so on a quadratic axis shorter than two knot
    /// spacings (one when the spacing is 2) the negative difference wraps to a large number and
    /// the count is not grown: 2 coefficients, 1 when the axis is shorter than one spacing
    /// (FSL-145).
    pub fn ncoef_fsl(&self, isz: u32) -> u32 {
        if self.ksp == 1 {
            isz
        } else if self.order == 3 {
            ((f64::from(isz + 1) / f64::from(self.ksp)).ceil() as u32) + 2
        } else {
            let mut nc = isz / self.ksp + self.order / 2;
            while (self.offset_into_field(nc) as u32).wrapping_sub(self.n / 2) < isz.wrapping_sub(1)
            {
                nc += 1;
            }
            nc
        }
    }

    /// larmorx (FSL-145 corrected): FSL's count with its test in signed arithmetic, so the
    /// quadratic count grows until the last spline's sampled extent reaches the last voxel on
    /// every axis (`ceil((2n + k - 4) / (2k)) + 2`); the cubic count and `k = 1` as FSL's.
    pub fn ncoef_general(&self, isz: u32) -> u32 {
        if self.ksp == 1 || self.order == 3 {
            return self.ncoef_fsl(isz);
        }
        let mut nc = isz / self.ksp + self.order / 2;
        while i64::from(self.offset_into_field(nc)) - i64::from(self.n / 2) < i64::from(isz) - 1 {
            nc += 1;
        }
        nc
    }

    /// `OffsetIntoField(cindx)`: the voxel of the spline's centre (unsigned products read as
    /// `int`).
    #[inline]
    pub fn offset_into_field(&self, cindx: u32) -> i32 {
        if self.ksp == 1 {
            cindx.wrapping_mul(self.ksp) as i32
        } else {
            cindx.wrapping_sub(self.order / 2).wrapping_mul(self.ksp) as i32
        }
    }

    /// `Range(cindx, isz, first, last)`: the voxels `first..last` the spline covers.
    #[inline]
    pub fn range(&self, cindx: u32, isz: u32) -> (u32, u32) {
        let oif = self.offset_into_field(cindx);
        let half = (self.n / 2) as i32;
        let first = 0.max(oif - half) as u32;
        let last = (isz as i32).min(oif + half + 1) as u32;
        (first, last)
    }

    /// `RangeOfSplines(vox, csz, first, last)`.
    pub fn range_of_splines(&self, vox: f64, csz: u32) -> (u32, u32) {
        let o0 = f64::from(self.offset_into_field(0));
        let half = (f64::from(self.n) + 1.0) / 2.0;
        let ksp = f64::from(self.ksp);
        let first = 0.max(crate::cnum::f64_to_i32(((vox - o0 - half) / ksp).ceil())) as u32;
        let tmp = (csz as i32).min(crate::cnum::f64_to_i32(((vox - o0 + half) / ksp).ceil()));
        let last = if tmp < 0 { 0 } else { tmp as u32 };
        (first, last)
    }

    /// `Offset(cindx, isz)`: the first index into the kernel that falls inside the field.
    #[inline]
    pub fn offset(&self, cindx: u32, _isz: u32) -> u32 {
        (-(0.min(self.offset_into_field(cindx) - (self.n as i32) / 2))) as u32
    }

    /// `RangeOfOverlappingSplines(cindx, isz, first, last)` (splines of the same kind).
    pub fn range_of_overlapping_splines(&self, cindx: u32, isz: u32) -> (u32, u32, bool) {
        let (first, last) = if self.ksp == 1 {
            let h = 2 * (self.order / 2);
            (
                0.max(cindx as i32 - h as i32) as u32,
                self.ncoef(isz).min(cindx + h + 1),
            )
        } else {
            (
                0.max(cindx as i32 - self.order as i32) as u32,
                self.ncoef(isz).min(cindx + self.order + 1),
            )
        };
        (first, last, last > first)
    }

    /// `RangeOfOverlappingSplines(cindx, isz, sp2, first, last)`: `last` is written only when a
    /// second overlapping spline is found, so it keeps the value it came in with otherwise (the
    /// caller's variable, as FSL's reference argument does).
    pub fn range_of_overlapping_splines_with(
        &self,
        cindx: u32,
        isz: u32,
        sp2: &Spline1D,
        first: &mut u32,
        last: &mut u32,
    ) -> bool {
        let (fme, lme) = self.range(cindx, isz);
        let mut ifirst: i32 = -1;
        for i in 0..sp2.ncoef(isz) {
            let (fsp2, lsp2) = sp2.range(i, isz);
            let overlap = lme
                .wrapping_sub(1)
                .min(lsp2.wrapping_sub(1))
                .wrapping_sub(fme.max(fsp2)) as i32;
            if overlap >= 0 {
                if ifirst == -1 {
                    ifirst = i as i32;
                } else {
                    *last = i + 1;
                }
            } else if ifirst != -1 {
                break;
            }
        }
        if ifirst == -1 {
            *first = 0;
            *last = 0;
            false
        } else {
            *first = ifirst as u32;
            true
        }
    }

    /// `SplineValueAtVoxel(vox, cindx)`.
    pub fn spline_value_at_voxel(&self, vox: f64, cindx: u32) -> f64 {
        let x = (vox - f64::from(self.offset_into_field(cindx))) / f64::from(self.ksp);
        kernel_value(x, self.order, self.ksp, self.deriv)
    }

    /// `operator()(double x)`: the kernel at `x` knot spacings from its centre.
    pub fn value(&self, x: f64) -> f64 {
        kernel_value(x, self.order, self.ksp, self.deriv)
    }
}

/// `spline_size(order, ksp)`.
fn spline_size(order: u32, ksp: u32) -> u32 {
    if order.is_multiple_of(2) && !ksp.is_multiple_of(2) {
        (order + 1) * ksp
    } else {
        ((order + 1) * ksp).wrapping_sub(1)
    }
}

/// `spline_kernel()`: the kernel sampled at each voxel, centred.
fn spline_kernel(order: u32, ksp: u32, deriv: u32, spsz: u32) -> Vec<f64> {
    let coa = f64::from(spsz.wrapping_sub(1)) / 2.0;
    (0..spsz)
        .map(|i| {
            let x = (f64::from(i) - coa) / f64::from(ksp);
            kernel_value(x, order, ksp, deriv)
        })
        .collect()
}

/// `kernel_value(x, order, ksp, deriv)`.
pub fn kernel_value(x: f64, order: u32, ksp: u32, deriv: u32) -> f64 {
    let ax = x.abs();
    let k = f64::from(ksp);
    let k2 = f64::from(ksp.wrapping_mul(ksp));
    match order {
        0 | 1 => match deriv {
            0 => {
                if ax < 1.0 {
                    return 1.0 - ax;
                }
                return 0.0;
            }
            1 => {
                if ax < 1e-6 {
                    return 0.0;
                } else if (ax - 1.0).abs() < 1e-6 {
                    return -(x / ax) * 0.5 / k;
                } else if ax < 1.0 {
                    return -(x / ax) / k;
                }
            }
            _ => {}
        },
        2 => match deriv {
            0 => {
                if ax <= 0.5 {
                    return 0.75 - ax * ax;
                } else if ax < 1.5 {
                    return 0.5 * (1.5 - ax) * (1.5 - ax);
                }
                return 0.0;
            }
            1 => {
                if ax < 1e-6 {
                    return 0.0;
                } else if ax <= 0.5 {
                    return -(x / ax) * 2.0 * (ax / k);
                } else if ax < 1.5 {
                    return (x / ax) * 0.5 * ((2.0 * ax - 3.0) / k);
                }
                return 0.0;
            }
            2 => {
                if (ax - 1.5).abs() < 1e-6 {
                    return 0.5 / k2;
                } else if (ax - 0.5).abs() < 1e-6 {
                    return -0.5 / k2;
                } else if ax < 0.5 {
                    return -2.0 / k2;
                } else if ax < 1.5 {
                    return 1.0 / k2;
                }
                return 0.0;
            }
            _ => {}
        },
        3 => match deriv {
            0 => {
                if ax <= 1.0 {
                    return (2.0 / 3.0) + (ax * ax) * (ax / 2.0 - 1.0);
                } else if ax < 2.0 {
                    return (1.0 / 6.0) * (2.0 - ax) * (2.0 - ax) * (2.0 - ax);
                }
                return 0.0;
            }
            1 => {
                if ax < 1e-6 {
                    return 0.0;
                } else if ax <= 1.0 {
                    return (x / ax) * (1.5 * ax * ax - 2.0 * ax) / k;
                } else if ax < 2.0 {
                    return (x / ax) * (-0.5 * (2.0 - ax) * (2.0 - ax)) / k;
                }
                return 0.0;
            }
            2 => {
                if ax <= 1.0 {
                    return (3.0 * ax - 2.0) / k2;
                } else if ax < 2.0 {
                    return (2.0 - ax) / k2;
                }
                return 0.0;
            }
            _ => {}
        },
        _ => {}
    }
    0.0
}

/// `Spline3D<double>`: the tensor product of three [`Spline1D`]s, with its kernel `_osp3D` and the
/// possibly pre-multiplied copy `_sp3D`.
#[derive(Clone, Debug)]
pub struct Spline3D {
    v: [Spline1D; 3],
    osp: Vec<f64>,
    sp: Vec<f64>,
    premul: bool,
}

impl Spline3D {
    /// `Spline3D(order, ksp, deriv)`.
    pub fn new(order: u32, ksp: [u32; 3], deriv: [u32; 3]) -> Result<Spline3D, FslError> {
        let v = [
            Spline1D::new(order, ksp[0], deriv[0])?,
            Spline1D::new(order, ksp[1], deriv[1])?,
            Spline1D::new(order, ksp[2], deriv[2])?,
        ];
        let (n0, n1, n2) = (v[0].n, v[1].n, v[2].n);
        let mut osp = vec![0.0; (n0 * n1 * n2) as usize];
        for k in 0..n2 {
            for j in 0..n1 {
                let bi = k * n1 * n0 + j * n0;
                for i in 0..n0 {
                    osp[(bi + i) as usize] = v[2].at(k) * v[1].at(j) * v[0].at(i);
                }
            }
        }
        let sp = osp.clone();
        Ok(Spline3D {
            v,
            osp,
            sp,
            premul: false,
        })
    }

    /// The same spline with the quadratic coefficient count corrected (`true`, FSL-145) or
    /// FSL's (`false`) along every direction.
    #[must_use]
    pub fn with_general_count(mut self, on: bool) -> Spline3D {
        for v in &mut self.v {
            v.general = on;
        }
        self
    }

    /// Whether the coefficient counts are the corrected ones (FSL-145).
    pub fn general_count(&self) -> bool {
        self.v[0].general
    }

    /// The copy constructor: a pre-multiplied spline's values are copied with it; otherwise the
    /// kernel is rebuilt from the 1D splines, which drops a scaling by `*=` or `/=` (as FSL's
    /// copy does).
    pub fn copy(&self) -> Spline3D {
        let mut c = self.clone();
        if !self.premul {
            c.sp = c.osp.clone();
        }
        c
    }

    pub fn order(&self) -> u32 {
        self.v[0].order
    }
    pub fn knot_spacing(&self, dir: usize) -> u32 {
        self.v[dir].ksp
    }
    pub fn knot_spacings(&self) -> [u32; 3] {
        [self.v[0].ksp, self.v[1].ksp, self.v[2].ksp]
    }
    pub fn derivative_order(&self) -> [u32; 3] {
        [self.v[0].deriv, self.v[1].deriv, self.v[2].deriv]
    }
    pub fn kernel_size(&self, dir: usize) -> u32 {
        self.v[dir].n
    }
    pub fn total_kernel_size(&self) -> u32 {
        self.v[0].n * self.v[1].n * self.v[2].n
    }
    pub fn ncoef(&self, dir: usize, isz: u32) -> u32 {
        self.v[dir].ncoef(isz)
    }
    pub fn dim(&self, dir: usize) -> &Spline1D {
        &self.v[dir]
    }
    pub fn is_premul(&self) -> bool {
        self.premul
    }
    /// `operator[](indx)`: the (possibly pre-multiplied) kernel, vectorised.
    #[inline]
    pub fn at(&self, i: u32) -> f64 {
        self.sp[i as usize]
    }
    pub fn values(&self) -> &[f64] {
        &self.sp
    }

    /// `operator()(double x, double y, double z)`.
    pub fn value(&self, x: f64, y: f64, z: f64) -> f64 {
        self.v[0].value(x) * self.v[1].value(y) * self.v[2].value(z)
    }

    /// `SplineValueAtVoxel(vox, cindx)`: z, then y, then x.
    pub fn spline_value_at_voxel(&self, vox: [f64; 3], cindx: [u32; 3]) -> f64 {
        self.v[2].spline_value_at_voxel(vox[2], cindx[2])
            * self.v[1].spline_value_at_voxel(vox[1], cindx[1])
            * self.v[0].spline_value_at_voxel(vox[0], cindx[0])
    }

    /// `operator*=(s)` (also `operator/=(s)` as `*= 1.0/s`).
    pub fn scale(&mut self, s: f64) -> Result<(), FslError> {
        if self.premul {
            return Err(splines_error(
                "Spline3D::operator*=: You should not combine multiplication by scalar and image",
            ));
        }
        for v in self.sp.iter_mut() {
            *v *= s;
        }
        Ok(())
    }

    /// `operator/=(s)`.
    pub fn divide(&mut self, s: f64) -> Result<(), FslError> {
        self.scale(1.0 / s)
    }

    /// `RangeInField(cindx, isz, first, last)`.
    #[inline]
    pub fn range_in_field(&self, cindx: [u32; 3], isz: [u32; 3]) -> ([u32; 3], [u32; 3]) {
        let mut first = [0; 3];
        let mut last = [0; 3];
        for i in 0..3 {
            (first[i], last[i]) = self.v[i].range(cindx[i], isz[i]);
        }
        (first, last)
    }

    /// `OffsetIntoKernel(indx, isz, offs)`.
    #[inline]
    pub fn offset_into_kernel(&self, cindx: [u32; 3], isz: [u32; 3]) -> [u32; 3] {
        [
            self.v[0].offset(cindx[0], isz[0]),
            self.v[1].offset(cindx[1], isz[1]),
            self.v[2].offset(cindx[2], isz[2]),
        ]
    }

    /// `RangeOfSplines(vox, csz, first, last)`.
    pub fn range_of_splines(&self, vox: [f64; 3], csz: [u32; 3]) -> ([u32; 3], [u32; 3]) {
        let mut first = [0; 3];
        let mut last = [0; 3];
        for i in 0..3 {
            (first[i], last[i]) = self.v[i].range_of_splines(vox[i], csz[i]);
        }
        (first, last)
    }

    /// `RangeOfOverlappingSplines(cindx, isz, first, last)` (same kind). `first` and `last` are
    /// written up to the first direction with no overlap, as FSL's loop stops there.
    pub fn range_of_overlapping_splines(
        &self,
        cindx: [u32; 3],
        isz: [u32; 3],
        first: &mut [u32; 3],
        last: &mut [u32; 3],
    ) -> bool {
        for i in 0..3 {
            let (f, l, ok) = self.v[i].range_of_overlapping_splines(cindx[i], isz[i]);
            first[i] = f;
            last[i] = l;
            if !ok {
                return false;
            }
        }
        true
    }

    /// `RangeOfOverlappingSplines(cindx, isz, sp2, first, last)`.
    pub fn range_of_overlapping_splines_with(
        &self,
        cindx: [u32; 3],
        isz: [u32; 3],
        sp2: &Spline3D,
        first: &mut [u32; 3],
        last: &mut [u32; 3],
    ) -> bool {
        for i in 0..3 {
            if !self.v[i].range_of_overlapping_splines_with(
                cindx[i],
                isz[i],
                &sp2.v[i],
                &mut first[i],
                &mut last[i],
            ) {
                return false;
            }
        }
        true
    }

    /// `NumberOfOverlappingSplines(cindx, isz)`.
    pub fn number_of_overlapping_splines(&self, cindx: [u32; 3], isz: [u32; 3]) -> u32 {
        let mut nos = 1u32;
        for i in 0..3 {
            let (f, l, ok) = self.v[i].range_of_overlapping_splines(cindx[i], isz[i]);
            if !ok {
                return 0;
            }
            nos = nos.wrapping_mul(l - f);
        }
        nos
    }

    /// `Premul(cindx, isz, ima)`: the kernel times the image over the part inside the field, the
    /// rest zero.
    pub fn premul(&mut self, cindx: [u32; 3], isz: [u32; 3], ima: &[f32]) {
        let (fi, li) = self.range_in_field(cindx, isz);
        let so = self.offset_into_kernel(cindx, isz);
        let (n0, n1) = (self.v[0].n, self.v[1].n);
        self.sp.iter_mut().for_each(|v| *v = 0.0);
        let mut sk = so[2];
        for ik in fi[2]..li[2] {
            let mut sj = so[1];
            for ij in fi[1]..li[1] {
                let ibi = ik * isz[1] * isz[0] + ij * isz[0];
                let sbi = sk * n1 * n0 + sj * n0;
                let mut si = so[0];
                for ii in fi[0]..li[0] {
                    let s = (sbi + si) as usize;
                    self.sp[s] = self.osp[s] * f64::from(ima[(ibi + ii) as usize]);
                    si += 1;
                }
                sj += 1;
            }
            sk += 1;
        }
        self.premul = true;
    }

    /// `MulByOther(cindx1, cindx2, isz, sp2)`: the sum over the voxels both splines cover of
    /// this (possibly pre-multiplied) kernel times `sp2`'s, z, then y, then x.
    pub fn mul_by_other(
        &self,
        cindx1: [u32; 3],
        cindx2: [u32; 3],
        isz: [u32; 3],
        sp2: &Spline3D,
    ) -> f64 {
        let mut f1 = [0u32; 3];
        let mut f2 = [0u32; 3];
        let mut l1 = [0u32; 3];
        for d in 0..3 {
            let (if1, il1) = self.v[d].range(cindx1[d], isz[d]);
            let o1 = self.v[d].offset(cindx1[d], isz[d]);
            let (if2, il2) = sp2.v[d].range(cindx2[d], isz[d]);
            let o2 = sp2.v[d].offset(cindx2[d], isz[d]);
            f1[d] = (0.max(if2 as i32 - if1 as i32) as u32).wrapping_add(o1);
            f2[d] = (0.max(if1 as i32 - if2 as i32) as u32).wrapping_add(o2);
            l1[d] = il1.min(il2).wrapping_sub(if1).wrapping_add(o1);
        }
        let (n0, n1) = (self.v[0].n, self.v[1].n);
        let (m0, m1) = (sp2.v[0].n, sp2.v[1].n);
        let mut prod = 0.0f64;
        let mut k2 = f2[2];
        for k1 in f1[2]..l1[2] {
            let bi1 = k1 * n1 * n0;
            let bi2 = k2 * m1 * m0;
            let mut j2 = f2[1];
            for j1 in f1[1]..l1[1] {
                let bi12 = bi1 + j1 * n0;
                let bi22 = bi2 + j2 * m0;
                let mut i2 = f2[0];
                for i1 in f1[0]..l1[0] {
                    prod += self.sp[(bi12 + i1) as usize] * sp2.sp[(bi22 + i2) as usize];
                    i2 += 1;
                }
                j2 += 1;
            }
            k2 += 1;
        }
        prod
    }

    /// `NzMax(isz)`: the most non-zero elements of `AᵀA` (the capacity FSL allocates).
    pub fn nz_max(&self, isz: [u32; 3]) -> u32 {
        let mut nzmax = 0u32;
        for k in 0..self.v[2].ncoef(isz[2]) {
            let (f, l, _) = self.v[2].range_of_overlapping_splines(k, isz[2]);
            let zn = l.wrapping_sub(f);
            for j in 0..self.v[1].ncoef(isz[1]) {
                let (f, l, _) = self.v[1].range_of_overlapping_splines(j, isz[1]);
                let yn = l.wrapping_sub(f);
                for i in 0..self.v[0].ncoef(isz[0]) {
                    let (f, l, _) = self.v[0].range_of_overlapping_splines(i, isz[0]);
                    nzmax = nzmax.wrapping_add(zn.wrapping_mul(yn).wrapping_mul(l.wrapping_sub(f)));
                }
            }
        }
        nzmax
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_kernel_ksp_one() {
        let s = Spline1D::new(3, 1, 0).unwrap();
        assert_eq!(s.kernel_size(), 3);
        // (2/3) + 1 * (1/2 - 1): one rounding below 1/6, as FSL computes it.
        assert_eq!(
            s.kernel(),
            &[0.16666666666666663, 2.0 / 3.0, 0.16666666666666663]
        );
        assert_eq!(s.ncoef(10), 10);
        let s = Spline1D::new(3, 4, 0).unwrap();
        assert_eq!(s.kernel_size(), 15);
        assert_eq!(s.ncoef(64), 19);
        assert_eq!(s.offset_into_field(0), -4);
        assert_eq!(s.range(0, 64), (0, 4));
        assert_eq!(s.offset(0, 64), 11);
    }

    #[test]
    fn errors() {
        assert_eq!(
            Spline1D::new(4, 2, 0).unwrap_err().what,
            "FslSplines::Spline1D::Spline1D: only orders 2 and 3 implemented"
        );
        assert!(Spline1D::new(3, 2, 4).is_err());
    }
}
