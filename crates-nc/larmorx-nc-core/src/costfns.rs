// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-newimage 2203.12 (costfns.{h,cc}). Copyright (C)
// 1999-2018 University of Oxford (Mark Jenkinson et al.). Rust translation Copyright 2026 Karel
// Lopez Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! FLIRT's cost functions (`NEWIMAGE::Costfn`) as mcflirt uses them: the cost of an affine
//! matrix between a reference volume and a test volume, computed over the reference's voxels
//! by trilinear (or, for `NormCorrSinc`, windowed-sinc) interpolation of the test volume.
//!
//! The costs: normalised correlation (with and without edge weighting, and with sinc
//! interpolation), least squares, the correlation ratio, Woods' criterion, mutual information
//! and normalised mutual information (fuzzy-binned, edge-weighted). Each is FSL's loop,
//! operation by operation in single precision, including FSL's quirks (the weighted
//! correlation's inflated count, FSL-02; Woods' squared bin counts, FSL-18).

use std::sync::OnceLock;

use larmorx_core::math;

use crate::arma::{self, Mat};
use crate::cnum;
use crate::volume::Volume;

/// `costfns` (costfns.h), the values mcflirt uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostType {
    Woods,
    CorrRatio,
    MutualInfo,
    NormCorr,
    NormMI,
    LeastSq,
    NormCorrSinc,
}

/// `Costfn`: a reference and a test volume, the histogram bins of the reference, and the
/// settings of the cost.
#[derive(Clone, Debug)]
pub struct Costfn {
    pub refvol: Volume,
    pub testvol: Volume,
    /// `testCog`: the test volume's centre of mass in scaled millimetres.
    pub test_cog: [f64; 3],
    pub costtype: CostType,
    pub smoothsize: f32,
    pub fuzzyfrac: f32,
    no_bins: i32,
    bin_a0: f32,
    bin_a1: f32,
    bindex: Vec<i32>,
    /// `p_count`: the number of cost evaluations.
    pub count: u64,
    /// FSL-14's correction: points whose reference or interpolated value is not finite are
    /// left out of the cost (FSL counts them, so one NaN makes the cost NaN).
    pub skip_nonfinite: bool,
}

impl Costfn {
    /// `Costfn(refv, inputv)`: the test volume's centre of mass is computed here (its warning, if
    /// any, is returned in `warnings`).
    pub fn new(refvol: Volume, testvol: Volume, warnings: &mut Vec<String>) -> Costfn {
        let test_cog = testvol.cog_scaled_mm(warnings);
        Costfn {
            refvol,
            testvol,
            test_cog,
            costtype: CostType::CorrRatio,
            smoothsize: 1.0,
            fuzzyfrac: 0.5,
            no_bins: 0,
            bin_a0: 0.0,
            bin_a1: 1.0,
            bindex: Vec::new(),
            count: 0,
            skip_nonfinite: false,
        }
    }

    /// `set_no_bins(n)`: the reference's bin of every voxel (`plnp`, the table of the unsmoothed
    /// mutual information, is not used by mcflirt's paths and is not built).
    pub fn set_no_bins(&mut self, n_bins: i32) {
        self.no_bins = n_bins;
        let (refmax0, refmin) = self.refvol.extrema();
        let mut refmax = refmax0;
        if refmax - refmin == 0.0 {
            refmax += 1.0;
        }
        self.bin_a1 = n_bins as f32 / (refmax - refmin);
        self.bin_a0 = (-refmin * n_bins as f32) / (refmax - refmin);
        let n = self.refvol.nvoxels() as usize;
        let mut bindex = Vec::with_capacity(n);
        for &v in &self.refvol.data[..n] {
            let mut b = cnum::f32_to_i32(v * self.bin_a1 + self.bin_a0);
            if b >= n_bins {
                b = n_bins - 1;
            }
            if b < 0 {
                b = 0;
            }
            bindex.push(b);
        }
        self.bindex = bindex;
    }

    /// `cost(affmat)` (voxel-to-voxel... in FSL's scaled-millimetre convention).
    pub fn cost(&mut self, affmat: &Mat) -> f32 {
        self.count += 1;
        let skip = self.skip_nonfinite;
        match self.costtype {
            CostType::NormCorr => {
                let c = if self.smoothsize > 0.0 {
                    p_normcorr_smoothed(&self.refvol, &self.testvol, affmat, self.smoothsize, false, skip)
                } else {
                    p_normcorr(&self.refvol, &self.testvol, affmat, skip)
                };
                (1.0 - f64::from(c.abs())) as f32
            }
            CostType::NormCorrSinc => {
                let c =
                    p_normcorr_smoothed(&self.refvol, &self.testvol, affmat, self.smoothsize, true, skip);
                (1.0 - f64::from(c.abs())) as f32
            }
            CostType::LeastSq => {
                if self.smoothsize > 0.0 {
                    p_leastsquares_smoothed(&self.refvol, &self.testvol, affmat, self.smoothsize, skip)
                } else {
                    p_leastsquares(&self.refvol, &self.testvol, affmat, skip)
                }
            }
            CostType::CorrRatio => {
                let c = if self.smoothsize > 0.0 {
                    p_corr_ratio_smoothed(
                        &self.refvol,
                        &self.testvol,
                        &self.bindex,
                        affmat,
                        self.no_bins,
                        self.smoothsize,
                        skip,
                    )
                } else {
                    p_corr_ratio(&self.refvol, &self.testvol, &self.bindex, affmat, self.no_bins, skip)
                };
                (1.0 - f64::from(c)) as f32
            }
            CostType::Woods => {
                p_woods_fn(&self.refvol, &self.testvol, &self.bindex, affmat, self.no_bins, skip)
            }
            CostType::MutualInfo | CostType::NormMI => {
                // smoothsize > 0 or fuzzyfrac > 0 (always 0.5 in mcflirt): the smoothed form.
                let (minv, maxv) = {
                    let (mx, mn) = self.testvol.extrema();
                    (mn, mx)
                };
                let (je, m1, m2) = calc_smoothed_entropy(
                    &self.refvol,
                    &self.testvol,
                    &self.bindex,
                    affmat,
                    minv,
                    maxv,
                    self.no_bins,
                    self.smoothsize,
                    self.fuzzyfrac,
                    skip,
                );
                if self.costtype == CostType::MutualInfo {
                    -(m1 + m2 - je)
                } else {
                    let normmi = if f64::from(je.abs()) < 1e-9 {
                        0.0
                    } else {
                        (m1 + m2) / je
                    };
                    -normmi
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// The voxel-to-voxel mapping

/// `vtest.sampling_mat().i() * aff.i() * vref.sampling_mat()`, as Armadillo evaluates it:
/// `solve(Stest, inv(aff) * Sref)`, the inverse by cofactors and the products in Armadillo's
/// small-matrix order. The 12 coefficients are returned as FSL stores them (`float`s).
pub fn voxel_map(vref: &Volume, vtest: &Volume, aff: &Mat) -> [f32; 12] {
    let ai = arma::inv(aff).unwrap_or_else(|_| Mat::from_rows(4, 4, &[f64::NAN; 16]));
    let b = arma::mul_tinysq(&ai, &vref.sampling_mat());
    let iaffbig = arma::solve(&vtest.sampling_mat(), &b)
        .unwrap_or_else(|_| Mat::from_rows(4, 4, &[f64::NAN; 16]));
    // a11 a12 a13 a14 / a21 .. a24 / a31 .. a34
    std::array::from_fn(|k| {
        let (r, c) = (k / 4, k % 4);
        iaffbig.at(r, c) as f32
    })
}

/// `in_interp_bounds(v, x, y, z)`: the truncated position and the next one inside the volume.
#[inline]
fn in_interp_bounds(v: &Volume, x: f32, y: f32, z: f32) -> bool {
    let ix = cnum::f32_to_i64(x);
    let iy = cnum::f32_to_i64(y);
    let iz = cnum::f32_to_i64(z);
    v.in_bounds(ix, iy, iz) && v.in_bounds(ix + 1, iy + 1, iz + 1)
}

/// `q_tri_interpolation(v, x, y, z)` (costfns.cc): trilinear with truncated indices and the pad
/// value outside `[0, n-1)`.
#[inline]
pub fn q_tri_interpolation(v: &Volume, x: f32, y: f32, z: f32) -> f32 {
    let ix = cnum::f32_to_i32(x);
    let iy = cnum::f32_to_i32(y);
    let iz = cnum::f32_to_i32(z);
    let dx = x - ix as f32;
    let dy = y - iy as f32;
    let dz = z - iz as f32;
    if ix >= 0
        && iy >= 0
        && iz >= 0
        && i64::from(ix) < v.xsize() - 1
        && i64::from(iy) < v.ysize() - 1
        && i64::from(iz) < v.zsize() - 1
    {
        let n = crate::interp::neighbours(v, i64::from(ix), i64::from(iy), i64::from(iz));
        crate::interp::q_tri(n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], dx, dy, dz)
    } else {
        v.padvalue
    }
}

/// `findrangex(...)`: the range of `x` for which the row's sample positions are inside the test
/// volume, from the analytic bounds, then trimmed by walking the row in float steps as the cost
/// loops will. Returns `(xmin, xmax)`; `xmin > xmax` is an empty row.
#[allow(clippy::too_many_arguments)]
pub fn findrangex(
    o1: f32,
    o2: f32,
    o3: f32,
    a11: f32,
    a21: f32,
    a31: f32,
    xb1: u32,
    xb2: f32,
    yb2: f32,
    zb2: f32,
) -> (u32, u32) {
    let mut xmin0: f32 = 0.0;
    let mut xmax0: f32 = xb1 as f32;
    let axis = |o: f32, a: f32, b2: f32| -> (f32, f32) {
        if f64::from(a.abs()) < 1.0e-8 {
            if 0.0 <= o && o <= b2 {
                (-1.0e8, 1.0e8)
            } else {
                (-1.0e8, -1.0e8)
            }
        } else {
            (-o / a, (b2 - o) / a)
        }
    };
    for (o, a, b2) in [(o1, a11, xb2), (o2, a21, yb2), (o3, a31, zb2)] {
        let (x1, x2) = axis(o, a, b2);
        let xmin = min_f(x1, x2);
        let xmax = max_f(x1, x2);
        xmin0 = max_f(xmin0, xmin);
        xmax0 = min_f(xmax0, xmax);
    }
    let (mut xmin1, mut xmax1) = if xmax0 < xmin0 {
        (1u32, 0u32)
    } else {
        (cnum::f32_to_u32(xmin0.ceil()), cnum::f32_to_u32(xmax0.floor()))
    };
    let (xmin_t, xmax_t) = (xmin1, xmax1);
    let mut x = o1;
    let mut y = o2;
    let mut z = o3;
    x += xmin_t as f32 * a11;
    y += xmin_t as f32 * a21;
    z += xmin_t as f32 * a31;
    let mut xc = xmin_t;
    while xc <= xmax_t {
        let inbounds = x <= xb2 && x >= 0.0 && y <= yb2 && y >= 0.0 && z <= zb2 && z >= 0.0;
        if xc == xmin1 && !inbounds {
            xmin1 += 1;
        } else if !inbounds {
            xmax1 = xc.wrapping_sub(1);
            return (xmin1, xmax1);
        }
        x += a11;
        y += a21;
        z += a31;
        if xc == u32::MAX {
            break;
        }
        xc += 1;
    }
    (xmin1, xmax1)
}

/// `MISCMATHS::Min(a, b)`: `a < b ? a : b`.
#[inline]
fn min_f(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `MISCMATHS::Max(a, b)`: `a > b ? a : b`.
#[inline]
fn max_f(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// The geometry every cost loop starts from: the coefficients, the reference's last indices and
/// the test volume's bounds `n - 1.0001`.
struct Geometry {
    a: [f32; 12],
    xb1: u32,
    yb1: u32,
    zb1: u32,
    xb2: f32,
    yb2: f32,
    zb2: f32,
}

impl Geometry {
    fn new(vref: &Volume, vtest: &Volume, aff: &Mat) -> Geometry {
        let a = voxel_map(vref, vtest, aff);
        Geometry {
            a,
            xb1: (vref.xsize() - 1) as u32,
            yb1: (vref.ysize() - 1) as u32,
            zb1: (vref.zsize() - 1) as u32,
            xb2: (f64::from(vtest.xsize() as f32) - 1.0001) as f32,
            yb2: (f64::from(vtest.ysize() as f32) - 1.0001) as f32,
            zb2: (f64::from(vtest.zsize() as f32) - 1.0001) as f32,
        }
    }

    /// For row `(y, z)`: the range of x and the positions at its first point.
    #[inline]
    fn row(&self, y: u32, z: u32) -> (u32, u32, f32, f32, f32) {
        let a = &self.a;
        let (yf, zf) = (y as f32, z as f32);
        let mut o1 = yf * a[1] + zf * a[2] + a[3];
        let mut o2 = yf * a[5] + zf * a[6] + a[7];
        let mut o3 = yf * a[9] + zf * a[10] + a[11];
        let (xmin, xmax) = findrangex(
            o1, o2, o3, a[0], a[4], a[8], self.xb1, self.xb2, self.yb2, self.zb2,
        );
        o1 += xmin as f32 * a[0];
        o2 += xmin as f32 * a[4];
        o3 += xmin as f32 * a[8];
        (xmin, xmax, o1, o2, o3)
    }

    /// The edge weight of a point (`smoothx = smoothsize / xdim`, ...).
    #[inline]
    fn weight(&self, o1: f32, o2: f32, o3: f32, sm: [f32; 3]) -> f32 {
        let mut w: f32 = 1.0;
        if o1 < sm[0] {
            w *= o1 / sm[0];
        } else if (self.xb2 - o1) < sm[0] {
            w *= (self.xb2 - o1) / sm[0];
        }
        if o2 < sm[1] {
            w *= o2 / sm[1];
        } else if (self.yb2 - o2) < sm[1] {
            w *= (self.yb2 - o2) / sm[1];
        }
        if o3 < sm[2] {
            w *= o3 / sm[2];
        } else if (self.zb2 - o3) < sm[2] {
            w *= (self.zb2 - o3) / sm[2];
        }
        if w < 0.0 {
            w = 0.0;
        }
        w
    }
}

/// `smoothx, smoothy, smoothz`.
fn smooth3(smoothsize: f32, vtest: &Volume) -> [f32; 3] {
    [
        smoothsize / vtest.xdim(),
        smoothsize / vtest.ydim(),
        smoothsize / vtest.zdim(),
    ]
}

/// Whether point `x` of a row is used: inner points always, end points when inside the
/// interpolation bounds.
#[inline]
fn use_point(vtest: &Volume, x: u32, xmin: u32, xmax: u32, o1: f32, o2: f32, o3: f32) -> bool {
    !(x == xmin || x == xmax) || in_interp_bounds(vtest, o1, o2, o3)
}

/// `p_normcorr(vref, vtest, aff)`: the correlation over the used points (count exact).
pub fn p_normcorr(vref: &Volume, vtest: &Volume, aff: &Mat, skip: bool) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let (mut sumx, mut sumy, mut sumx2, mut sumy2, mut sumxy) = (0f32, 0f32, 0f32, 0f32, 0f32);
    let (mut sumx_a, mut sumy_a, mut sumx2_a, mut sumy2_a, mut sumxy_a) =
        (0f32, 0f32, 0f32, 0f32, 0f32);
    let (mut sumx_b, mut sumy_b, mut sumx2_b, mut sumy2_b, mut sumxy_b) =
        (0f32, 0f32, 0f32, 0f32, 0f32);
    let mut num: i64 = 0;
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        num += 1;
                        let valx = vref.value(i64::from(x), i64::from(y), i64::from(z));
                        let valy = val;
                        sumx += valx;
                        sumx2 += valx * valx;
                        sumy += valy;
                        sumy2 += valy * valy;
                        sumxy += valx * valy;
                    }
                }
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
            sumx_a += sumx;
            sumy_a += sumy;
            sumx2_a += sumx2;
            sumy2_a += sumy2;
            sumxy_a += sumxy;
            sumx = 0.0;
            sumy = 0.0;
            sumx2 = 0.0;
            sumy2 = 0.0;
            sumxy = 0.0;
        }
        sumx_b += sumx_a;
        sumy_b += sumy_a;
        sumx2_b += sumx2_a;
        sumy2_b += sumy2_a;
        sumxy_b += sumxy_a;
        sumx_a = 0.0;
        sumy_a = 0.0;
        sumx2_a = 0.0;
        sumy2_a = 0.0;
        sumxy_a = 0.0;
    }
    let (sumx, sumy, sumx2, sumy2, sumxy) = (sumx_b, sumy_b, sumx2_b, sumy2_b, sumxy_b);
    let mut corr: f32 = 0.0;
    if num > 2 {
        let numf = num as f32;
        let numsq = numf * numf;
        let varxy = sumxy / (numf - 1.0) - (sumx * sumy) / numsq;
        let varx = sumx2 / (numf - 1.0) - (sumx * sumx) / numsq;
        let vary = sumy2 / (numf - 1.0) - (sumy * sumy) / numsq;
        if varx > 0.0 && vary > 0.0 {
            corr = varxy / varx.sqrt() / vary.sqrt();
        }
    }
    corr
}

/// `p_normcorr_smoothed` (and, with `sinc`, `p_normcorr_smoothed_sinc`): the edge-weighted
/// correlation, with FSL's count: `num` and `numA` are never reset, so the count is a running
/// sum of running sums of the weights (FSL-02).
pub fn p_normcorr_smoothed(
    vref: &Volume,
    vtest: &Volume,
    aff: &Mat,
    smoothsize: f32,
    sinc: bool,
    skip: bool,
) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let sm = smooth3(smoothsize, vtest);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let (mut sumx, mut sumy, mut sumx2, mut sumy2, mut sumxy) = (0f32, 0f32, 0f32, 0f32, 0f32);
    let (mut sumx_a, mut sumy_a, mut sumx2_a, mut sumy2_a, mut sumxy_a) =
        (0f32, 0f32, 0f32, 0f32, 0f32);
    let (mut sumx_b, mut sumy_b, mut sumx2_b, mut sumy2_b, mut sumxy_b) =
        (0f32, 0f32, 0f32, 0f32, 0f32);
    let (mut num, mut num_a, mut num_b) = (0f32, 0f32, 0f32);
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = if sinc {
                        q_sinc_interpolation(vtest, o1, o2, o3)
                    } else {
                        q_tri_interpolation(vtest, o1, o2, o3)
                    };
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        let weight = g.weight(o1, o2, o3, sm);
                        let valx = vref.value(i64::from(x), i64::from(y), i64::from(z));
                        let valy = val;
                        num += weight;
                        sumx += weight * valx;
                        sumx2 += weight * valx * valx;
                        sumy += weight * valy;
                        sumy2 += weight * valy * valy;
                        sumxy += weight * valx * valy;
                    }
                }
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
            num_a += num;
            sumx_a += sumx;
            sumy_a += sumy;
            sumx2_a += sumx2;
            sumy2_a += sumy2;
            sumxy_a += sumxy;
            sumx = 0.0;
            sumy = 0.0;
            sumx2 = 0.0;
            sumy2 = 0.0;
            sumxy = 0.0;
        }
        num_b += num_a;
        sumx_b += sumx_a;
        sumy_b += sumy_a;
        sumx2_b += sumx2_a;
        sumy2_b += sumy2_a;
        sumxy_b += sumxy_a;
        sumx_a = 0.0;
        sumy_a = 0.0;
        sumx2_a = 0.0;
        sumy2_a = 0.0;
        sumxy_a = 0.0;
    }
    normcorr_from_sums(num_b, sumx_b, sumy_b, sumx2_b, sumy2_b, sumxy_b)
}

/// The final formula of the smoothed correlations: `sumxy/(num-1.0)` is a double division (the
/// literal), `(sumx*sumy)/(num*num)` a float one, their difference stored in a float.
#[inline]
pub fn normcorr_from_sums(num: f32, sumx: f32, sumy: f32, sumx2: f32, sumy2: f32, sumxy: f32) -> f32 {
    let mut corr: f32 = 0.0;
    if f64::from(num) > 2.0 {
        let nm1 = f64::from(num) - 1.0;
        let nn = num * num;
        let varxy = (f64::from(sumxy) / nm1 - f64::from((sumx * sumy) / nn)) as f32;
        let varx = (f64::from(sumx2) / nm1 - f64::from((sumx * sumx) / nn)) as f32;
        let vary = (f64::from(sumy2) / nm1 - f64::from((sumy * sumy) / nn)) as f32;
        if varx > 0.0 && vary > 0.0 {
            corr = varxy / varx.sqrt() / vary.sqrt();
        }
    }
    corr
}

/// `p_leastsquares`.
pub fn p_leastsquares(vref: &Volume, vtest: &Volume, aff: &Mat, skip: bool) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let (mut sum, mut sum_a, mut sum_b) = (0f32, 0f32, 0f32);
    let mut num: i64 = 0;
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        num += 1;
                        let valx = vref.value(i64::from(x), i64::from(y), i64::from(z));
                        let valy = val;
                        sum += (valx - valy) * (valx - valy);
                    }
                }
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
            sum_a += sum;
            sum = 0.0;
        }
        sum_b += sum_a;
        sum_a = 0.0;
    }
    if num > 1 {
        sum_b / num as f32
    } else {
        let d = max_ff(vref.max(), vtest.max()) - min_ff(vref.min(), vtest.min());
        d * d
    }
}

/// `p_leastsquares_smoothed` (its counts are reset per row and slice: no FSL-02 here).
pub fn p_leastsquares_smoothed(
    vref: &Volume,
    vtest: &Volume,
    aff: &Mat,
    smoothsize: f32,
    skip: bool,
) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let sm = smooth3(smoothsize, vtest);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let (mut sum, mut sum_a, mut sum_b) = (0f32, 0f32, 0f32);
    let (mut num, mut num_a, mut num_b) = (0f32, 0f32, 0f32);
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        let weight = g.weight(o1, o2, o3, sm);
                        let valx = vref.value(i64::from(x), i64::from(y), i64::from(z));
                        let valy = val;
                        num += weight;
                        sum += weight * (valx - valy) * (valx - valy);
                    }
                }
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
            sum_a += sum;
            sum = 0.0;
            num_a += num;
            num = 0.0;
        }
        sum_b += sum_a;
        sum_a = 0.0;
        num_b += num_a;
        num_a = 0.0;
    }
    if num_b > 1.0 {
        sum_b / num_b
    } else {
        let d = max_ff(vref.max(), vtest.max()) - min_ff(vref.min(), vtest.min());
        d * d
    }
}

#[inline]
fn max_ff(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

#[inline]
fn min_ff(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `p_corr_ratio_smoothed`: 1 - the weighted correlation ratio over the reference's bins.
pub fn p_corr_ratio_smoothed(
    vref: &Volume,
    vtest: &Volume,
    bindex: &[i32],
    aff: &Mat,
    no_bins: i32,
    smoothsize: f32,
    skip: bool,
) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let sm = smooth3(smoothsize, vtest);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let nb = no_bins.max(0) as usize;
    let mut sumy = vec![0f32; nb + 1];
    let mut sumy2 = vec![0f32; nb + 1];
    let mut numy = vec![0f32; nb + 1];
    let (nx, ny) = (vref.xsize() as usize, vref.ysize() as usize);
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut bptr = (z as usize * ny + y as usize) * nx + xmin as usize;
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        let b = bindex[bptr] as usize;
                        let weight = g.weight(o1, o2, o3, sm);
                        numy[b] += weight;
                        sumy[b] += weight * val;
                        sumy2[b] += weight * val * val;
                    }
                }
                bptr += 1;
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
        }
    }
    corr_ratio_finish(&mut numy, &mut sumy, &mut sumy2, nb, |n| f64::from(n) > 2.0)
}

/// The final step of both correlation ratios: the last bin folded into the one before, the
/// bins' variances weighted by their counts, over the total variance.
fn corr_ratio_finish(
    numy: &mut [f32],
    sumy: &mut [f32],
    sumy2: &mut [f32],
    nb: usize,
    enough: impl Fn(f32) -> bool,
) -> f32 {
    if nb >= 1 {
        numy[nb - 1] += numy[nb];
        sumy[nb - 1] += sumy[nb];
        sumy2[nb - 1] += sumy2[nb];
    }
    numy[nb] = 0.0;
    sumy[nb] = 0.0;
    sumy2[nb] = 0.0;
    let (mut corr_ratio, mut var, mut totsumy, mut totsumy2, mut numtoty) =
        (0f32, 0f32, 0f32, 0f32, 0f32);
    for b in 0..nb {
        if enough(numy[b]) {
            numtoty += numy[b];
            totsumy += sumy[b];
            totsumy2 += sumy2[b];
            var = (sumy2[b] - sumy[b] * sumy[b] / numy[b]) / (numy[b] - 1.0);
            corr_ratio += var * numy[b];
        }
    }
    if numtoty > 0.0 {
        corr_ratio /= numtoty;
    }
    if numtoty > 1.0 {
        var = (totsumy2 - totsumy * totsumy / numtoty) / (numtoty - 1.0);
    }
    if var > 0.0 {
        corr_ratio /= var;
    }
    if numtoty <= 1.0 || var <= 0.0 {
        0.0
    } else {
        (1.0 - f64::from(corr_ratio)) as f32
    }
}

/// `p_corr_ratio` (counts as integers, converted to float where FSL converts them).
pub fn p_corr_ratio(
    vref: &Volume,
    vtest: &Volume,
    bindex: &[i32],
    aff: &Mat,
    no_bins: i32,
    skip: bool,
) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let nb = no_bins.max(0) as usize;
    let mut sumy = vec![0f32; nb + 1];
    let mut sumy2 = vec![0f32; nb + 1];
    let mut numy = vec![0i32; nb + 1];
    let (nx, ny) = (vref.xsize() as usize, vref.ysize() as usize);
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut bptr = (z as usize * ny + y as usize) * nx + xmin as usize;
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        let b = bindex[bptr] as usize;
                        numy[b] += 1;
                        sumy[b] += val;
                        sumy2[b] += val * val;
                    }
                }
                bptr += 1;
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
        }
    }
    if nb >= 1 {
        numy[nb - 1] += numy[nb];
        sumy[nb - 1] += sumy[nb];
        sumy2[nb - 1] += sumy2[nb];
    }
    numy[nb] = 0;
    sumy[nb] = 0.0;
    sumy2[nb] = 0.0;
    let (mut corr_ratio, mut var, mut totsumy, mut totsumy2) = (0f32, 0f32, 0f32, 0f32);
    let mut numtoty: i32 = 0;
    for b in 0..nb {
        if numy[b] > 2 {
            numtoty += numy[b];
            totsumy += sumy[b];
            totsumy2 += sumy2[b];
            var = (sumy2[b] - sumy[b] * sumy[b] / numy[b] as f32) / (numy[b] - 1) as f32;
            corr_ratio += var * numy[b] as f32;
        }
    }
    if numtoty > 0 {
        corr_ratio /= numtoty as f32;
    }
    if numtoty > 1 {
        var = (totsumy2 - totsumy * totsumy / numtoty as f32) / (numtoty - 1) as f32;
    }
    if var > 0.0 {
        corr_ratio /= var;
    }
    if numtoty <= 1 || var <= 0.0 {
        0.0
    } else {
        (1.0 - f64::from(corr_ratio)) as f32
    }
}

/// `p_woods_fn`: the count-weighted coefficients of variation of the test values in the
/// reference's bins, with FSL's squared counts (FSL-18).
pub fn p_woods_fn(
    vref: &Volume,
    vtest: &Volume,
    bindex: &[i32],
    aff: &Mat,
    no_bins: i32,
    skip: bool,
) -> f32 {
    let g = Geometry::new(vref, vtest, aff);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let nb = no_bins.max(0) as usize;
    let mut sum = vec![0f32; nb + 1];
    let mut sum2 = vec![0f32; nb + 1];
    let mut num = vec![0i32; nb + 1];
    let (nx, ny) = (vref.xsize() as usize, vref.ysize() as usize);
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut bptr = (z as usize * ny + y as usize) * nx + xmin as usize;
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        let b = bindex[bptr] as usize;
                        num[b] += 1;
                        sum[b] += val;
                        sum2[b] += val * val;
                    }
                }
                bptr += 1;
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
        }
    }
    let (mut woods, mut var): (f32, f32);
    woods = 0.0;
    let mut numtot: i32 = 0;
    for b in 0..=nb {
        if num[b] > 2 {
            numtot += num[b];
            var = (sum2[b] - sum[b] * sum[b] / num[b] as f32) / (num[b] - 1) as f32;
            let stdev = if var > 0.0 { var.sqrt() } else { 0.0 };
            let n = num[b] as f32;
            if sum[b] > 0.0 {
                woods += (n * n) * stdev / sum[b];
            } else {
                woods += (n * n) * stdev;
            }
        }
    }
    if numtot > 0 {
        woods /= numtot as f32;
        woods
    } else {
        1e10
    }
}

/// `calc_smoothed_entropy`: the joint and marginal entropies with fuzzy binning of the test
/// values and edge weights. Returns `(joint, marginal 1, marginal 2)`.
#[allow(clippy::too_many_arguments)]
pub fn calc_smoothed_entropy(
    vref: &Volume,
    vtest: &Volume,
    bindex: &[i32],
    aff: &Mat,
    mintest: f32,
    maxtest: f32,
    no_bins: i32,
    smoothsize: f32,
    fuzzyfrac: f32,
    skip: bool,
) -> (f32, f32, f32) {
    let g = Geometry::new(vref, vtest, aff);
    let sm = smooth3(smoothsize, vtest);
    let (a11, a21, a31) = (g.a[0], g.a[4], g.a[8]);
    let nb = no_bins.max(0) as usize;
    let mut jointhist = vec![0f32; (nb + 1) * (nb + 1)];
    let mut marghist1 = vec![0f32; nb + 1];
    let mut marghist2 = vec![0f32; nb + 1];
    let b1 = no_bins as f32 / (maxtest - mintest);
    let b0 = (-mintest * no_bins as f32) / (maxtest - mintest);
    let (nx, ny) = (vref.xsize() as usize, vref.ysize() as usize);
    let nbl = i64::from(no_bins);
    for z in 0..=g.zb1 {
        for y in 0..=g.yb1 {
            let (xmin, xmax, mut o1, mut o2, mut o3) = g.row(y, z);
            let mut bptr = (z as usize * ny + y as usize) * nx + xmin as usize;
            let mut x = xmin;
            while x <= xmax {
                if use_point(vtest, x, xmin, xmax, o1, o2, o3) {
                    let val = q_tri_interpolation(vtest, o1, o2, o3);
                    if !skip || (val.is_finite() && vref.value(i64::from(x), i64::from(y), i64::from(z)).is_finite()) {
                        let geomweight = g.weight(o1, o2, o3, sm);
                        let a = i64::from(bindex[bptr]);
                        let mut bidx = val * b1 + b0;
                        let mut bcentre = cnum::f32_to_i64(bidx);
                        let mut bplus = bcentre.wrapping_add(1);
                        let mut bminus = bcentre.wrapping_sub(1);
                        if bcentre >= nbl {
                            bcentre = nbl - 1;
                            bplus = bcentre;
                        }
                        if bcentre < 0 {
                            bcentre = 0;
                            bminus = bcentre;
                        }
                        if bplus >= nbl {
                            bplus = nbl - 1;
                        }
                        if bminus < 0 {
                            bminus = 0;
                        }
                        bidx = (bidx - cnum::f32_to_i32(bidx) as f32).abs();
                        let (wcentre, wplus, wminus): (f32, f32, f32);
                        if bidx < fuzzyfrac {
                            wcentre = (0.5 + 0.5 * f64::from(bidx / fuzzyfrac)) as f32;
                            wminus = 1.0 - wcentre;
                            wplus = 0.0;
                        } else if f64::from(bidx) > 1.0 - f64::from(fuzzyfrac) {
                            wcentre = (0.5 + 0.5 * ((1.0 - f64::from(bidx)) / f64::from(fuzzyfrac)))
                                as f32;
                            wplus = 1.0 - wcentre;
                            wminus = 0.0;
                        } else {
                            wcentre = 1.0;
                            wplus = 0.0;
                            wminus = 0.0;
                        }
                        let row = (a * (nbl + 1)) as usize;
                        // A NaN test value makes FSL's bin indices wrap and write outside the
                        // histograms (undefined behaviour, FSL-21): such updates are dropped.
                        let ok = |b: i64| (0..=nbl).contains(&b);
                        if ok(bcentre) {
                            jointhist[row + bcentre as usize] += geomweight * wcentre;
                            marghist2[bcentre as usize] += geomweight * wcentre;
                        }
                        if ok(bplus) {
                            jointhist[row + bplus as usize] += geomweight * wplus;
                            marghist2[bplus as usize] += geomweight * wplus;
                        }
                        if ok(bminus) {
                            jointhist[row + bminus as usize] += geomweight * wminus;
                            marghist2[bminus as usize] += geomweight * wminus;
                        }
                        marghist1[a as usize] += geomweight;
                    }
                }
                bptr += 1;
                o1 += a11;
                o2 += a21;
                o3 += a31;
                if x == u32::MAX {
                    break;
                }
                x += 1;
            }
        }
    }
    let nvoxels = vref.ysize() * vref.xsize() * vref.zsize();
    let nv = nvoxels as f32;
    let (mut jointentropy, mut margentropy1, mut margentropy2) = (0f32, 0f32, 0f32);
    for &n in &jointhist {
        if n > 0.0 {
            let p = n / nv;
            jointentropy += -p * crate::glibc_float::logf(p);
        }
    }
    for &n in &marghist1 {
        if n > 0.0 {
            let p = n / nv;
            margentropy1 += -p * crate::glibc_float::logf(p);
        }
    }
    let mut noverlap: f32 = 0.0;
    for &n in &marghist2 {
        if n > 0.0 {
            noverlap += n;
            let p = n / nv;
            margentropy2 += -p * crate::glibc_float::logf(p);
        }
    }
    if noverlap > 0.0 {
        let nratio = nv / noverlap;
        let l = crate::glibc_float::logf(nratio);
        jointentropy = nratio * jointentropy - l;
        margentropy1 = nratio * margentropy1 - l;
        margentropy2 = nratio * margentropy2 - l;
    } else {
        let l = math::log(f64::from(no_bins));
        jointentropy = (2.0 * l) as f32;
        margentropy1 = l as f32;
        margentropy2 = l as f32;
    }
    (jointentropy, margentropy1, margentropy2)
}

// ------------------------------------------------------------------------------------------------
// The sinc interpolation of the stage-4 cost

/// The kernel table of `q_setupkernel` (Hanning-windowed sinc, half-width 3, 201 samples),
/// built on first use as FSL's static table is.
fn sinc_table() -> &'static [f32; 201] {
    static TABLE: OnceLock<[f32; 201]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let w = 3;
        let mut t = [0f32; 201];
        for (n, v) in t.iter_mut().enumerate() {
            // x=(n-100)/100.0*Globalkernelwidth: double, then float.
            let x = ((n as f64 - 100.0) / 100.0 * f64::from(w)) as f32;
            *v = q_sinc(x) * q_hanning(x, w);
        }
        t
    })
}

/// `q_sinc(x)`.
fn q_sinc(x: f32) -> f32 {
    if f64::from(x.abs()) < 1e-7 {
        return (1.0 - f64::from(x.abs())) as f32;
    }
    let y = (std::f64::consts::PI * f64::from(x)) as f32;
    crate::kernel::sinf(y) / y
}

/// `q_hanning(x, w)`.
fn q_hanning(x: f32, w: i32) -> f32 {
    if x.abs() > w as f32 {
        0.0
    } else {
        (0.5 + 0.5 * math::cos(std::f64::consts::PI * f64::from(x) / f64::from(w))) as f32
    }
}

/// `q_kernelval(x, w)`.
fn q_kernelval(x: f32, w: i32) -> f32 {
    if x.abs() > w as f32 {
        return 0.0;
    }
    let mut dn = (f64::from(x / w as f32) * 100.0 + 100.0) as f32;
    let n = cnum::f32_to_i32(dn.floor());
    dn -= n as f32;
    if n >= 200 {
        return 0.0;
    }
    if n < 0 {
        return 0.0;
    }
    let t = sinc_table();
    // kernel[n]*(1.0-dn) is double (the literal); kernel[n+1]*dn is a float product.
    (f64::from(t[n as usize]) * (1.0 - f64::from(dn)) + f64::from(t[n as usize + 1] * dn)) as f32
}

/// `q_sinc_interpolation(v, x, y, z)`: the windowed sinc over the voxels inside the volume,
/// normalised by the kernel's sum; the volume's background value when that sum is about 0.
pub fn q_sinc_interpolation(v: &Volume, x: f32, y: f32, z: f32) -> f32 {
    let w = 3i32;
    let ix0 = cnum::f32_to_i32(x.floor());
    let iy0 = cnum::f32_to_i32(y.floor());
    let iz0 = cnum::f32_to_i32(z.floor());
    let mut sincz = [0f32; 7];
    let mut sincy = [0f32; 7];
    let mut sincx = [0f32; 7];
    for d in -w..=w {
        let k = (d + w) as usize;
        sincz[k] = q_kernelval(z - iz0 as f32 + d as f32, w);
        sincy[k] = q_kernelval(y - iy0 as f32 + d as f32, w);
        sincx[k] = q_kernelval(x - ix0 as f32 + d as f32, w);
    }
    let x1a = 0.max(ix0 - w);
    let x1b = ((v.xsize() - 1) as i32).min(ix0 + w);
    let y1a = 0.max(iy0 - w);
    let y1b = ((v.ysize() - 1) as i32).min(iy0 + w);
    let z1a = 0.max(iz0 - w);
    let z1b = ((v.zsize() - 1) as i32).min(iz0 + w);
    let mut convsum: f32 = 0.0;
    let mut kersum: f32 = 0.0;
    for z1 in z1a..=z1b {
        let zj = (iz0 - z1 + w) as usize;
        for y1 in y1a..=y1b {
            let yj = (iy0 - y1 + w) as usize;
            for x1 in x1a..=x1b {
                let xj = (ix0 - x1 + w) as usize;
                let sincfac = sincx[xj] * sincy[yj] * sincz[zj];
                convsum += v.value(i64::from(x1), i64::from(y1), i64::from(z1)) * sincfac;
                kersum += sincfac;
            }
        }
    }
    if f64::from(kersum.abs()) > 1e-9 {
        convsum / kersum
    } else {
        v.backgroundval()
    }
}
