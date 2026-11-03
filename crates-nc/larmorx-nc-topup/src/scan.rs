// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topup_costfunctions.{h,cpp}, the
// classes TopupScan and TopupScanManager). Copyright (C) 2012 University of Oxford (Jesper
// Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (see LICENSE).
//! One scan of topup's input (`TopupScan`: its regridded, subsampled and smoothed copies, and
//! for a field and movement parameters the scan resampled, its derivatives, the Jacobian
//! determinant and the mask of valid voxels), and the scans together (`TopupScanManager`: their
//! mean, the movement parameters each one estimates).
//!
//! FSL holds the stages as shared pointers, a stage that changes nothing pointing to the
//! previous one; here a stage that is `None` is the previous one.

use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::basisfield::{FieldIndex, SplineField};
use larmorx_nc_core::interp_partial::{ExtrapParams, Interpolator};
use larmorx_nc_core::newimagefns;
use larmorx_nc_core::volume::{Extrapolation, FslError, Interpolation, Volume};
use larmorx_nc_core::warpfns;

use crate::Console;
use crate::fileio::movepar2matrix;
use crate::vol;

/// `TopupException(msg)`: `what()` is `"Topup::" + msg`.
pub fn topup_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "TOPUP::TopupException",
        what: format!("Topup::{msg}"),
    }
}

/// `SizeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeType {
    Original,
    Regridded,
    Subsampled,
    Target,
}

/// `TopupInterpolationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpType {
    Linear,
    Spline,
}

fn translate_interp(it: InterpType) -> Interpolation {
    match it {
        InterpType::Linear => Interpolation::Trilinear,
        InterpType::Spline => Interpolation::Spline,
    }
}

/// `TopupScan`.
#[derive(Clone, Debug)]
pub struct Scan {
    orig: Volume,
    regrid: Option<Volume>,
    subsamp: Option<Volume>,
    smooth: Option<Volume>,
    resampled: Volume,
    derivs: Volume,
    jac: Volume,
    mask: Volume<i8>,
    pevec: [f64; 3],
    rotime: f64,
    uptodate: bool,
    mp: [f64; 6],
    tp: bool,
    nthr: u32,
}

impl Scan {
    /// `TopupScan(scan, pevec, rotime, nthreads)`: the scan extrapolated periodically, valid
    /// along the phase-encoding direction when it is one of x and y.
    pub fn new(scan: &Volume, pevec: [f64; 3], rotime: f64, nthr: u32) -> Result<Scan, FslError> {
        if pevec[2] != 0.0 {
            return Err(topup_error(
                "TopupScan::TopupScan: third element of pevec must be zero",
            ));
        }
        let mut orig = scan.clone();
        orig.extrap = Extrapolation::Periodic;
        orig.ep_valid = if pevec[0] != 0.0 && pevec[1] == 0.0 {
            [true, false, false]
        } else if pevec[0] == 0.0 && pevec[1] != 0.0 {
            [false, true, false]
        } else {
            [false, false, false]
        };
        Ok(Scan {
            orig,
            regrid: None,
            subsamp: None,
            smooth: None,
            resampled: Volume::new7([0, 0, 0, 0, 1, 1, 1]),
            derivs: Volume::new7([0, 0, 0, 0, 1, 1, 1]),
            jac: Volume::new7([0, 0, 0, 0, 1, 1, 1]),
            mask: Volume::new7([0, 0, 0, 0, 1, 1, 1]),
            pevec,
            rotime,
            uptodate: false,
            mp: [0.0; 6],
            tp: false,
            nthr,
        })
    }

    fn regrid(&self) -> &Volume {
        self.regrid.as_ref().unwrap_or(&self.orig)
    }

    fn subsamp(&self) -> &Volume {
        self.subsamp.as_ref().unwrap_or_else(|| self.regrid())
    }

    fn smooth_vol(&self) -> &Volume {
        self.smooth.as_ref().unwrap_or_else(|| self.subsamp())
    }

    pub fn has_beta(&self) -> bool {
        self.pevec[0] != 0.0
    }

    pub fn has_gamma(&self) -> bool {
        self.pevec[1] != 0.0
    }

    pub fn pevec(&self) -> [f64; 3] {
        self.pevec
    }

    pub fn up_to_date(&self) -> bool {
        self.uptodate
    }

    pub fn set_up_to_date(&mut self, flag: bool) {
        self.uptodate = flag;
    }

    pub fn set_trace_print(&mut self, val: bool) {
        self.tp = val;
    }

    pub fn movement_parameters(&self) -> [f64; 6] {
        self.mp
    }

    /// `ImageSize(type)`.
    pub fn image_size(&self, con: &Console, t: SizeType) -> Result<[u32; 3], FslError> {
        con.trace(self.tp, "Entering TopupScan::ImageSize");
        let sz = |v: &Volume| [v.xsize() as u32, v.ysize() as u32, v.zsize() as u32];
        let size = match t {
            SizeType::Original => sz(&self.orig),
            SizeType::Regridded => sz(self.regrid()),
            SizeType::Subsampled => sz(self.subsamp()),
            SizeType::Target => {
                let ss = self.target_ss("TopupScan::ImageSize(Target)")?;
                let o = &self.orig;
                [
                    (o.xsize() / ss) as u32,
                    (o.ysize() / ss) as u32,
                    (o.zsize() / ss) as u32,
                ]
            }
        };
        con.trace(self.tp, "Leaving TopupScan::ImageSize");
        Ok(size)
    }

    /// The subsampling factor of the target grid, with `ImageSize`'s and `ImageVxs`' checks.
    fn target_ss(&self, what: &str) -> Result<i64, FslError> {
        let (r, s) = (self.regrid(), self.subsamp());
        let ss = r.xsize() / s.xsize();
        if r.ysize() / s.ysize() != ss || r.zsize() / s.zsize() != ss {
            return Err(topup_error(&format!("{what}: Inconsistent sub-sampling")));
        }
        let o = &self.orig;
        if o.xsize() % ss != 0 || o.ysize() % ss != 0 || o.zsize() % ss != 0 {
            return Err(topup_error(&format!(
                "{what}: Sub-sampling incompatible with original image size"
            )));
        }
        Ok(ss)
    }

    /// `ImageVxs(type)` (the target's: `ss * xdim` in `float`).
    pub fn image_vxs(&self, con: &Console, t: SizeType) -> Result<[f64; 3], FslError> {
        con.trace(self.tp, "Entering TopupScan::ImageVxs");
        let dm = |v: &Volume| {
            [
                f64::from(v.xdim()),
                f64::from(v.ydim()),
                f64::from(v.zdim()),
            ]
        };
        let vxs = match t {
            SizeType::Original => dm(&self.orig),
            SizeType::Regridded => dm(self.regrid()),
            SizeType::Subsampled => dm(self.subsamp()),
            SizeType::Target => {
                let ss = self.target_ss("TopupScan::ImageVxs(Target)")? as u32 as f32;
                let o = &self.orig;
                [
                    f64::from(ss * o.xdim()),
                    f64::from(ss * o.ydim()),
                    f64::from(ss * o.zdim()),
                ]
            }
        };
        con.trace(self.tp, "Leaving TopupScan::ImageVxs");
        Ok(vxs)
    }

    /// `GetResampled(field)`: the Jacobian times the resampled scan.
    pub fn get_resampled(
        &mut self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        con.trace(self.tp, "Entering TopupScan::GetResampled");
        self.update(con, field)?;
        con.trace(self.tp, "Leaving TopupScan::GetResampled");
        vol::mul(&self.jac, &self.resampled)
    }

    /// `GetMask(field)`.
    pub fn get_mask(
        &mut self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<Volume<i8>, FslError> {
        con.trace(self.tp, "Entering TopupScan::GetMask");
        self.update(con, field)?;
        con.trace(self.tp, "Leaving TopupScan::GetMask");
        Ok(self.mask.clone())
    }

    /// Volume `k` of the derivatives (`_derivs[k]`, with their properties).
    fn deriv(&self, k: i64) -> Result<Volume, FslError> {
        self.derivs.sub_volume(k)
    }

    /// `GetAlpha(field)`: the derivative of the resampled scan with respect to the field.
    pub fn get_alpha(
        &mut self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        con.trace(self.tp, "Entering TopupScan::GetAlpha");
        self.update(con, field)?;
        let ovxs = self.image_vxs(con, SizeType::Original)?;
        let svxs = self.image_vxs(con, SizeType::Subsampled)?;
        let sf: [f64; 3] = [0, 1, 2].map(|i| svxs[i] / ovxs[i]);
        let (p1, p2) = (self.pevec[0], self.pevec[1]);
        let rval = if p1 != 0.0 && p2 == 0.0 {
            let k = (self.rotime / sf[0] * p1) as f32;
            vol::mul(&vol::scaled(&self.deriv(0)?, k), &self.jac)?
        } else if p1 == 0.0 && p2 != 0.0 {
            let k = (self.rotime / sf[1] * p2) as f32;
            vol::mul(&vol::scaled(&self.deriv(1)?, k), &self.jac)?
        } else {
            let a = vol::divided(&vol::scaled(&self.deriv(0)?, p1 as f32), sf[0] as f32);
            let b = vol::divided(&vol::scaled(&self.deriv(1)?, p2 as f32), sf[1] as f32);
            vol::mul(
                &vol::scaled(&vol::add(&a, &b)?, self.rotime as f32),
                &self.jac,
            )?
        };
        con.trace(self.tp, "Leaving TopupScan::GetAlpha");
        Ok(rval)
    }

    /// `GetBeta(field)` (x) and `GetGamma(field)` (y): the read-out time over the subsampling
    /// times the resampled scan, or zeros without a phase-encoding component along `dir`.
    fn beta_gamma(
        &mut self,
        con: &Console,
        field: &mut SplineField,
        dir: usize,
    ) -> Result<Volume, FslError> {
        let name = if dir == 0 { "GetBeta" } else { "GetGamma" };
        con.trace(self.tp, &format!("Entering TopupScan::{name}"));
        self.update(con, field)?;
        let ovxs = self.image_vxs(con, SizeType::Original)?;
        let tvxs = self.image_vxs(con, SizeType::Target)?;
        let ss = (tvxs[dir] / ovxs[dir]) as f32;
        let rval = if self.pevec[dir] != 0.0 {
            let k = (self.rotime / f64::from(ss) * self.pevec[dir]) as f32;
            vol::scaled(&self.resampled, k)
        } else {
            let mut r = self.resampled.clone();
            vol::fill(&mut r, 0.0);
            r
        };
        con.trace(self.tp, &format!("Leaving TopupScan::{name}"));
        Ok(rval)
    }

    pub fn get_beta(&mut self, con: &Console, field: &mut SplineField) -> Result<Volume, FslError> {
        self.beta_gamma(con, field, 0)
    }

    pub fn get_gamma(
        &mut self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        self.beta_gamma(con, field, 1)
    }

    /// `GetMovementDerivative(i, field)`: the derivative of the resampled scan with respect to
    /// movement parameter `i` (0..5), by the difference of the two transforms at `mp` and at
    /// `mp + tiny` (1e-4 for translations, 1e-5 for rotations), times the Jacobian.
    pub fn get_movement_derivative(
        &mut self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        con.trace(self.tp, "Entering TopupScan::GetMovementDerivative");
        if i >= 6 {
            return Err(topup_error(
                "TopupScan::GetMovementDerivative: i must be in range 0...5",
            ));
        }
        self.update(con, field)?;
        let isz = self.image_size(con, SizeType::Target)?;
        let idim = self.image_vxs(con, SizeType::Target)?;
        let mut rval = vol::grid(
            i64::from(isz[0]),
            i64::from(isz[1]),
            i64::from(isz[2]),
            [idim[0] as f32, idim[1] as f32, idim[2] as f32],
        );
        let sdim = self.image_vxs(con, SizeType::Subsampled)?;
        let sf: [f64; 3] = [0, 1, 2].map(|k| idim[k] / sdim[k]);
        let tiny = if i > 2 { 1e-5 } else { 1e-4 };
        let mut p = self.mp;
        p[i] += tiny;
        let s = rval.sampling_mat();
        // inv(S) * inv(R) * S, as Armadillo evaluates it: solve(S, inv(R) * S)
        let t = |r: &Mat| -> Result<Mat, FslError> {
            let ri = arma::inv(r).map_err(|_| singular())?;
            arma::solve(&s, &arma::mul(&ri, &s, false)).map_err(|_| singular())
        };
        let t1 = t(&self.mp_to_matrix(con, &self.mp))?;
        let t2 = t(&self.mp_to_matrix(con, &p))?;
        let n = rval.nvoxels() as usize;
        let d = &self.derivs.data;
        let (nx, ny, nz) = (rval.xsize(), rval.ysize(), rval.zsize());
        for k in 0..nz {
            for j in 0..ny {
                for ii in 0..nx {
                    let x = Mat::col(&[ii as f64, j as f64, k as f64, 1.0]);
                    let y1 = arma::mul(&t1, &x, false);
                    let y2 = arma::mul(&t2, &x, false);
                    let idx = rval.idx(ii, j, k);
                    let v = sf[0] * (y2.get(0) - y1.get(0)) * f64::from(d[idx])
                        + sf[1] * (y2.get(1) - y1.get(1)) * f64::from(d[n + idx])
                        + sf[2] * (y2.get(2) - y1.get(2)) * f64::from(d[2 * n + idx]);
                    rval.data[idx] = v as f32;
                }
            }
        }
        vol::div_scalar(&mut rval, tiny as f32);
        let jac = self.get_jacobian(con, field)?;
        vol::mul_assign(&mut rval, &jac)?;
        con.trace(self.tp, "Leaving TopupScan::GetMovementDerivative");
        Ok(rval)
    }

    /// `GetJacobian(field)`.
    pub fn get_jacobian(
        &mut self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        con.trace(self.tp, "Entering TopupScan::GetJacobian");
        self.update(con, field)?;
        con.trace(self.tp, "Leaving TopupScan::GetJacobian");
        Ok(self.jac.clone())
    }

    /// `GetDisplacementField(field)`: the displacements (mm) along x, y and z (0).
    pub fn get_displacement_field(
        &self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        let isz = self.image_size(con, SizeType::Target)?;
        let idim = self.image_vxs(con, SizeType::Target)?;
        self.displacements(field, isz, idim)
    }

    /// The displacement field of `update` and `GetDisplacementField`: `df[0]` the field times
    /// `rotime · pevec(1) · xdim`, `df[1]` times `rotime · pevec(2) · ydim` (both in `float`),
    /// `df[2]` 0.
    fn displacements(
        &self,
        field: &mut SplineField,
        isz: [u32; 3],
        idim: [f64; 3],
    ) -> Result<Volume, FslError> {
        let (x, y, z) = (i64::from(isz[0]), i64::from(isz[1]), i64::from(isz[2]));
        let dims = [idim[0] as f32, idim[1] as f32, idim[2] as f32];
        let mut df = Volume::new(x, y, z, 3);
        df.setdims(dims[0], dims[1], dims[2]);
        df.pixdims[3] = 1.0;
        let mut f0 = vol::grid(x, y, z, dims);
        field.as_volume(&mut f0, FieldIndex::Field)?;
        let n = f0.data.len();
        let k1 = (self.rotime * self.pevec[1] * f64::from(self.orig.ydim())) as f32;
        let k0 = (self.rotime * self.pevec[0] * f64::from(self.orig.xdim())) as f32;
        for i in 0..n {
            df.data[n + i] = k1 * f0.data[i];
            df.data[i] = f0.data[i] * k0;
            df.data[2 * n + i] = 0.0;
        }
        Ok(df)
    }

    /// `SetMovementParameters(mp)`: out of date when one moves by more than 1e-9.
    pub fn set_movement_parameters(&mut self, con: &Console, mp: &[f64; 6]) {
        con.trace(self.tp, "Entering TopupScan::SetMovementParameters");
        for i in 0..6 {
            if (mp[i] - self.mp[i]).abs() > 1e-9 {
                self.uptodate = false;
                self.mp = *mp;
                break;
            }
        }
        con.trace(self.tp, "Leaving TopupScan::SetMovementParameters");
    }

    /// `SetInterpolationModel(it)`: every stage.
    pub fn set_interpolation_model(&mut self, con: &Console, it: InterpType) {
        con.trace(self.tp, "Entering TopupScan::SetInterpolationModel");
        let m = translate_interp(it);
        self.orig.interp = m;
        for v in [&mut self.regrid, &mut self.subsamp, &mut self.smooth]
            .into_iter()
            .flatten()
        {
            v.interp = m;
        }
        self.uptodate = false;
        con.trace(self.tp, "Leaving TopupScan::SetInterpolationModel");
    }

    /// `Smooth(fwhm)`: a Gaussian of `fwhm` mm (`sigma = fwhm / sqrt(8 ln 2)`) of the
    /// subsampled scan; 0 none.
    pub fn smooth(&mut self, con: &Console, fwhm: f64) {
        con.trace(self.tp, "Entering TopupScan::Smooth");
        if fwhm == 0.0 {
            self.smooth = None;
        } else {
            let sigma = (fwhm / (8.0 * std::f64::consts::LN_2).sqrt()) as f32;
            self.smooth = Some(newimagefns::smooth(self.subsamp(), sigma, 1));
        }
        self.uptodate = false;
        con.trace(self.tp, "Leaving TopupScan::Smooth");
    }

    /// `SubSample(ss)`: the mean of each `ss³` block of the regridded scan (summed in `float`
    /// in z, y, x order, divided as `double`).
    pub fn subsample(&mut self, con: &Console, ss: u32) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScan::SubSample");
        let r = self.regrid();
        let s = i64::from(ss);
        if r.xsize() % s != 0 || r.ysize() % s != 0 || r.zsize() % s != 0 {
            return Err(topup_error(
                "TopupScan::SubSample: invalid subsampling factor",
            ));
        }
        if ss == 1 {
            self.subsamp = None;
        } else {
            let ssf = ss as f32;
            let mut v = vol::grid(
                r.xsize() / s,
                r.ysize() / s,
                r.zsize() / s,
                [ssf * r.xdim(), ssf * r.ydim(), ssf * r.zdim()],
            );
            v.interp = r.interp;
            v.extrap = r.extrap;
            v.ep_valid = r.ep_valid;
            let div = f64::from(ss * ss * ss);
            for k in 0..v.zsize() {
                for j in 0..v.ysize() {
                    for i in 0..v.xsize() {
                        let mut acc = 0f32;
                        for kk in 0..s {
                            for jj in 0..s {
                                for ii in 0..s {
                                    acc += r.value(i * s + ii, j * s + jj, k * s + kk);
                                }
                            }
                        }
                        v.set(i, j, k, (f64::from(acc) / div) as f32);
                    }
                }
            }
            self.subsamp = Some(v);
        }
        self.uptodate = false;
        self.smooth = None;
        con.trace(self.tp, "Leaving TopupScan::SubSample");
        Ok(())
    }

    /// `ReGrid(xsz, ysz, zsz)`: the scan interpolated onto `xsz × ysz × zsz` voxels spanning
    /// its field of view (less 1e-6 mm).
    pub fn re_grid(&mut self, con: &Console, xsz: i64, ysz: i64, zsz: i64) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScan::Regrid");
        let o = &self.orig;
        if xsz == o.xsize() && ysz == o.ysize() && zsz == o.zsize() {
            self.regrid = None;
        } else {
            let fov = |n: i64, d: f32| f64::from((n - 1) as f32 * d) - 1e-6;
            let mut v = vol::grid(
                xsz,
                ysz,
                zsz,
                [
                    (fov(o.xsize(), o.xdim()) / (xsz - 1) as f64) as f32,
                    (fov(o.ysize(), o.ydim()) / (ysz - 1) as f64) as f32,
                    (fov(o.zsize(), o.zdim()) / (zsz - 1) as f64) as f32,
                ],
            );
            v.interp = o.interp;
            v.extrap = o.extrap;
            v.ep_valid = o.ep_valid;
            let xfac = f64::from(v.xdim() / o.xdim());
            let ip = Interpolator::new(o)?;
            for k in 0..zsz {
                let z = f64::from(k as f32 * v.zdim() / o.zdim());
                for j in 0..ysz {
                    let y = f64::from(j as f32 * v.ydim() / o.ydim());
                    for i in 0..xsz {
                        let val = ip.interpolate(
                            (i as f64 * xfac) as f32,
                            y as f32,
                            z as f32,
                            &ExtrapParams::DEFAULT,
                        )?;
                        v.set(i, j, k, val);
                    }
                }
            }
            self.regrid = Some(v);
            self.uptodate = false;
            self.subsamp = None;
            self.smooth = None;
        }
        con.trace(self.tp, "Leaving TopupScan::ReGrid");
        Ok(())
    }

    /// `mp_to_matrix(mp)`: `MovePar2Matrix(mp, subsampled scan)`.
    fn mp_to_matrix(&self, con: &Console, mp: &[f64; 6]) -> Mat {
        con.trace(self.tp, "Entering TopupScan::mp_to_matrix");
        let m = movepar2matrix(mp, self.subsamp());
        con.trace(self.tp, "Leaving TopupScan::mp_to_matrix");
        m
    }

    /// `GetRigidBodyMatrix()`.
    pub fn rigid_body_matrix(&self, con: &Console) -> Mat {
        self.mp_to_matrix(con, &self.mp)
    }

    /// `update(field)`: the scan resampled by the field and the movement parameters (with its
    /// derivatives and mask of valid voxels, the first and last slices and, for a single
    /// phase-encoding direction, the first and last columns across it masked out) and the
    /// Jacobian determinant.
    fn update(&mut self, con: &Console, field: &mut SplineField) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScan::update");
        if !self.uptodate {
            con.trace(
                self.tp,
                "TopupScan::update: Things not up to date, procceeds with updating.",
            );
            let rb = self.mp_to_matrix(con, &self.mp);
            let isz = self.image_size(con, SizeType::Target)?;
            let idim = self.image_vxs(con, SizeType::Target)?;
            let df = self.displacements(field, isz, idim)?;
            let (x, y, z) = (i64::from(isz[0]), i64::from(isz[1]), i64::from(isz[2]));
            let dims = [idim[0] as f32, idim[1] as f32, idim[2] as f32];
            let mut resampled = vol::grid(x, y, z, dims);
            let mut mask = Volume::<i8>::new(x, y, z, 1);
            mask.setdims(dims[0], dims[1], dims[2]);
            let mut derivs = Volume::new(x, y, z, 3);
            derivs.setdims(dims[0], dims[1], dims[2]);
            derivs.pixdims[3] = 1.0;
            warpfns::general_transform_3partial(
                self.smooth_vol(),
                &rb,
                &df,
                &mut resampled,
                &mut derivs,
                Some(&mut mask),
                self.nthr,
            )?;
            for j in 0..y {
                for i in 0..x {
                    mask.set(i, j, 0, 0);
                    mask.set(i, j, z - 1, 0);
                }
            }
            let (p1, p2) = (self.pevec[0], self.pevec[1]);
            if p1 != 0.0 && p2 == 0.0 {
                for k in 0..z {
                    for i in 0..x {
                        mask.set(i, 0, k, 0);
                        mask.set(i, y - 1, k, 0);
                    }
                }
            } else if p1 == 0.0 && p2 != 0.0 {
                for k in 0..z {
                    for j in 0..y {
                        mask.set(0, j, k, 0);
                        mask.set(x - 1, j, k, 0);
                    }
                }
            }
            let mut jac = vol::grid(x, y, z, dims);
            if (p1 != 0.0 && p2 == 0.0) || (p1 == 0.0 && p2 != 0.0) {
                let mut dfield = field.clone();
                let sf = if p1 != 0.0 {
                    self.rotime * p1 * f64::from(self.orig.xdim())
                } else {
                    self.rotime * p2 * f64::from(self.orig.ydim())
                };
                dfield.scale_field(sf);
                let fi = if p1 != 0.0 {
                    FieldIndex::Dfdx
                } else {
                    FieldIndex::Dfdy
                };
                warpfns::deffield2jacobian_1d(&mut dfield, fi, &mut jac)?;
            } else {
                let mut xc = field.clone();
                let mut yc = field.clone();
                let mut zc = field.clone();
                xc.scale_field(self.rotime * p1 * f64::from(self.orig.xdim()));
                yc.scale_field(self.rotime * p2 * f64::from(self.orig.ydim()));
                zc.scale_field(0.0);
                warpfns::deffield2jacobian(&mut xc, &mut yc, &mut zc, &mut jac)?;
            }
            self.resampled = resampled;
            self.mask = mask;
            self.derivs = derivs;
            self.jac = jac;
            self.uptodate = true;
        }
        con.trace(self.tp, "Leaving TopupScan::update");
        Ok(())
    }
}

/// Armadillo's error for a singular matrix in `inv(A) * B`.
fn singular() -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: "matrix multiplication: problem with matrix inverse; suggest to use solve() instead"
            .to_owned(),
    }
}

/// `TopupScanManager`.
#[derive(Clone, Debug)]
pub struct ScanManager {
    scans: Vec<Scan>,
    mask: Volume<i8>,
    mean: Volume,
    mean_alpha: Volume,
    mean_beta: Volume,
    mean_gamma: Volume,
    up_to_date: bool,
    regrid_sz: [u32; 3],
    ss: u32,
    /// `_it` (FSL never initialises it: `None` until the first `SetInterpolationModel`, which
    /// always sets the scans' method; the result is the same whatever FSL's memory held).
    it: Option<InterpType>,
    fwhm: f64,
    mpi: Vec<Vec<usize>>,
    hasx: bool,
    hasy: bool,
    tp: bool,
}

impl ScanManager {
    /// `TopupScanManager(scans, pevecs, rotimes, nthreads)`: the first scan the reference (no
    /// movement); each other estimates six parameters, or five (the translation along the
    /// phase-encoding direction left out) for the first scan whose phase-encoding sign differs
    /// from the first's, when all scans share one phase-encoding axis.
    pub fn new(
        scans: &Volume,
        pevecs: &[[f64; 3]],
        rotimes: &[f64],
        nthr: u32,
    ) -> Result<ScanManager, FslError> {
        let nt = scans.tsize() as usize;
        if nt != pevecs.len() || nt != rotimes.len() {
            return Err(topup_error(
                "TopupScanManager::TopupScanManage: Mismatched input parameters",
            ));
        }
        let mut s = Vec::with_capacity(nt);
        for i in 0..nt {
            let v = scans.sub_volume(i as i64)?;
            s.push(Scan::new(&v, pevecs[i], rotimes[i], nthr)?);
        }
        let regrid_sz = [
            scans.xsize() as u32,
            scans.ysize() as u32,
            scans.zsize() as u32,
        ];
        let diff = |a: f64, b: f64| (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0);
        let (mut hasx, mut hasy) = (false, false);
        let (mut fx, mut fy) = (0.0f64, 0.0f64);
        let mut index_five = 0usize;
        for (i, sc) in s.iter().enumerate() {
            let p = sc.pevec;
            if p[0] != 0.0 {
                hasx = true;
                if fx == 0.0 {
                    fx = p[0];
                } else if diff(fx, p[0]) && index_five == 0 {
                    index_five = i;
                }
            }
            if p[1] != 0.0 {
                hasy = true;
                if fy == 0.0 {
                    fy = p[1];
                } else if diff(fy, p[1]) && index_five == 0 {
                    index_five = i;
                }
            }
        }
        let mut mpi = vec![Vec::new(); nt];
        for (i, m) in mpi.iter_mut().enumerate().skip(1) {
            *m = if !(hasx && hasy) && i == index_five {
                if hasx {
                    (1..6).collect()
                } else {
                    vec![0, 2, 3, 4, 5]
                }
            } else {
                (0..6).collect()
            };
        }
        let empty = || Volume::new7([0, 0, 0, 0, 1, 1, 1]);
        Ok(ScanManager {
            scans: s,
            mask: Volume::<i8>::new7([0, 0, 0, 0, 1, 1, 1]),
            mean: empty(),
            mean_alpha: empty(),
            mean_beta: empty(),
            mean_gamma: empty(),
            up_to_date: false,
            regrid_sz,
            ss: 1,
            it: None,
            fwhm: 0.0,
            mpi,
            hasx,
            hasy,
            tp: false,
        })
    }

    pub fn n_scans(&self) -> usize {
        self.scans.len()
    }

    pub fn n_movement_parameters(&self) -> usize {
        self.mpi.iter().map(Vec::len).sum()
    }

    /// `NoOfMovementParametersForScan(scan)`.
    pub fn n_movement_parameters_for(&self, s: usize) -> Result<usize, FslError> {
        self.mpi.get(s).map(Vec::len).ok_or_else(|| {
            topup_error("TopupScanManager::NoOfMovementParametersForScan: scan index out of range")
        })
    }

    pub fn has_beta(&self) -> bool {
        self.hasx
    }

    pub fn has_gamma(&self) -> bool {
        self.hasy
    }

    pub fn subsampling(&self) -> u32 {
        self.ss
    }

    pub fn fwhm(&self) -> f64 {
        self.fwhm
    }

    pub fn interpolation_model(&self) -> Option<InterpType> {
        self.it
    }

    pub fn trace_print(&self) -> bool {
        self.tp
    }

    /// `SetTracePrint(val)`: the manager and every scan.
    pub fn set_trace_print(&mut self, val: bool) {
        self.tp = val;
        for s in &mut self.scans {
            s.set_trace_print(val);
        }
    }

    pub fn image_size(&self, con: &Console, t: SizeType) -> Result<[u32; 3], FslError> {
        self.scans[0].image_size(con, t)
    }

    pub fn image_vxs(&self, con: &Console, t: SizeType) -> Result<[f64; 3], FslError> {
        self.scans[0].image_vxs(con, t)
    }

    /// `GetMovementParameters()`: the parameters being estimated, scan by scan.
    pub fn movement_parameters(&self, con: &Console) -> Vec<f64> {
        con.trace(self.tp, "Entering TopupScanManager::GetMovementParameters");
        let mut ovec = Vec::with_capacity(self.n_movement_parameters());
        for (s, m) in self.scans.iter().zip(&self.mpi) {
            let mp = s.movement_parameters();
            ovec.extend(m.iter().map(|&j| mp[j]));
        }
        con.trace(self.tp, "Leaving TopupScanManager::GetMovementParameters");
        ovec
    }

    /// `GetAllMovementParameters()`: six per scan.
    pub fn all_movement_parameters(&self, con: &Console) -> Vec<f64> {
        con.trace(
            self.tp,
            "Entering TopupScanManager::GetAllMovementParameters",
        );
        let v = self
            .scans
            .iter()
            .flat_map(|s| s.movement_parameters())
            .collect();
        con.trace(
            self.tp,
            "Leaving TopupScanManager::GetAllMovementParameters",
        );
        v
    }

    /// `GetRigidBodyMatrix(i)`.
    pub fn rigid_body_matrix(&self, con: &Console, i: usize) -> Result<Mat, FslError> {
        con.trace(self.tp, "Entering TopupScanManager::GetRigidBodyMatrix");
        let s = self.scans.get(i).ok_or_else(|| {
            topup_error("TopupScanManager::GetRigidBodyMatrix: index i out of range")
        })?;
        let m = s.rigid_body_matrix(con);
        con.trace(self.tp, "Leaving TopupScanManager::GetRigidBodyMatrix");
        Ok(m)
    }

    /// `HasBeta(i)` / `HasGamma(i)` of scan `i`.
    fn scan_has(&self, con: &Console, i: usize, gamma: bool) -> Result<bool, FslError> {
        let name = if gamma { "HasGamma" } else { "HasBeta" };
        con.trace(self.tp, &format!("Entering TopupScanManager::{name}"));
        let s = self.scans.get(i).ok_or_else(|| {
            topup_error(&format!("TopupScanManager::{name}: index i out of range"))
        })?;
        con.trace(self.tp, &format!("Leaving TopupScanManager::{name}"));
        Ok(if gamma { s.has_gamma() } else { s.has_beta() })
    }

    pub fn has_beta_of(&self, con: &Console, i: usize) -> Result<bool, FslError> {
        self.scan_has(con, i, false)
    }

    pub fn has_gamma_of(&self, con: &Console, i: usize) -> Result<bool, FslError> {
        self.scan_has(con, i, true)
    }

    fn checked(&self, con: &Console, i: usize, name: &str) -> Result<(), FslError> {
        con.trace(self.tp, &format!("Entering TopupScanManager::{name}"));
        if i >= self.scans.len() {
            return Err(topup_error(&format!(
                "TopupScanManager::{name}: index i out of range"
            )));
        }
        con.trace(self.tp, &format!("Leaving TopupScanManager::{name}"));
        Ok(())
    }

    /// `GetScan(i, field)`: the Jacobian times the resampled scan.
    pub fn get_scan(
        &mut self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        self.checked(con, i, "GetScan")?;
        self.scans[i].get_resampled(con, field)
    }

    pub fn get_alpha(
        &mut self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        self.checked(con, i, "GetAlpha")?;
        self.scans[i].get_alpha(con, field)
    }

    pub fn get_beta(
        &mut self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        self.checked(con, i, "GetBeta")?;
        self.scans[i].get_beta(con, field)
    }

    pub fn get_gamma(
        &mut self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        self.checked(con, i, "GetGamma")?;
        self.scans[i].get_gamma(con, field)
    }

    pub fn get_jacobian(
        &mut self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        self.checked(con, i, "GetJacobian")?;
        self.scans[i].get_jacobian(con, field)
    }

    /// `GetMovementDerivative(scan, deriv, field)`: `deriv` indexes the scan's estimated
    /// parameters.
    pub fn get_movement_derivative(
        &mut self,
        con: &Console,
        scan: usize,
        deriv: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        con.trace(self.tp, "Entering TopupScanManager::GetMovementDerivative");
        if scan >= self.scans.len() {
            return Err(topup_error(
                "TopupScanManager::GetMovementDerivative: index scan out of range",
            ));
        }
        if deriv >= self.mpi[scan].len() {
            return Err(topup_error(
                "TopupScanManager::GetMovementDerivative: index deriv out of range",
            ));
        }
        con.trace(self.tp, "Leaving TopupScanManager::GetMovementDerivative");
        let p = self.mpi[scan][deriv];
        self.scans[scan].get_movement_derivative(con, p, field)
    }

    /// `GetDisplacementField(scanindx, field)`.
    pub fn get_displacement_field(
        &self,
        con: &Console,
        i: usize,
        field: &mut SplineField,
    ) -> Result<Volume, FslError> {
        let s = self.scans.get(i).ok_or_else(|| {
            topup_error("TopupScanManager::GetDisplacementField: index scanindx out of range")
        })?;
        s.get_displacement_field(con, field)
    }

    /// `GetMean(field)` and `GetMask(field)`, `GetMeanAlpha`, ... after `update`.
    pub fn mean(&mut self, con: &Console, field: &mut SplineField) -> Result<&Volume, FslError> {
        self.update(con, field)?;
        Ok(&self.mean)
    }

    /// Updates the means and returns them: the mean, its alpha, beta and gamma, and the mask.
    /// `getters`: how many of FSL's `GetMean`, `GetMeanAlpha`, ..., `GetMask` the caller makes,
    /// each an `update(field)` (with `--trace`, each prints its entry and exit).
    pub fn means(
        &mut self,
        con: &Console,
        field: &mut SplineField,
        getters: usize,
    ) -> Result<(Volume, Volume, Volume, Volume, Volume<i8>), FslError> {
        for _ in 0..getters.max(1) {
            self.update(con, field)?;
        }
        Ok((
            self.mean.clone(),
            self.mean_alpha.clone(),
            self.mean_beta.clone(),
            self.mean_gamma.clone(),
            self.mask.clone(),
        ))
    }

    pub fn mask(
        &mut self,
        con: &Console,
        field: &mut SplineField,
    ) -> Result<&Volume<i8>, FslError> {
        self.update(con, field)?;
        Ok(&self.mask)
    }

    /// `FieldUpdated()`.
    pub fn field_updated(&mut self, con: &Console) {
        con.trace(self.tp, "Entering TopupScanManager::FieldUpdated");
        self.up_to_date = false;
        for s in &mut self.scans {
            s.set_up_to_date(false);
        }
        con.trace(self.tp, "Leaving TopupScanManager::FieldUpdated");
    }

    /// `ReGrid(sz)`: each scan regridded, subsampled and smoothed again.
    pub fn re_grid(&mut self, con: &Console, sz: [u32; 3]) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScanManager::ReGrid");
        if sz != self.regrid_sz {
            self.regrid_sz = sz;
            self.up_to_date = false;
            let (ss, fwhm) = (self.ss, self.fwhm);
            for s in &mut self.scans {
                s.re_grid(con, i64::from(sz[0]), i64::from(sz[1]), i64::from(sz[2]))?;
                s.subsample(con, ss)?;
                s.smooth(con, fwhm);
            }
        }
        con.trace(self.tp, "Leaving TopupScanManager::ReGrid");
        Ok(())
    }

    /// `SubSample(ss)`: each scan subsampled and smoothed (with the current FWHM) again.
    pub fn subsample(&mut self, con: &Console, ss: u32) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScanManager::SubSample");
        if ss != self.ss {
            self.ss = ss;
            self.up_to_date = false;
            let fwhm = self.fwhm;
            for s in &mut self.scans {
                s.subsample(con, ss)?;
                s.smooth(con, fwhm);
            }
        }
        con.trace(self.tp, "Leaving TopupScanManager::SubSample");
        Ok(())
    }

    /// `Smooth(fwhm)`.
    pub fn smooth(&mut self, con: &Console, fwhm: f64) {
        con.trace(self.tp, "Entering TopupScanManager::Smooth");
        if fwhm != self.fwhm {
            self.fwhm = fwhm;
            self.up_to_date = false;
            for s in &mut self.scans {
                s.smooth(con, fwhm);
            }
        }
        con.trace(self.tp, "Leaving TopupScanManager::Smooth");
    }

    /// `SetMovementParameters(p)`: the estimated parameters, the others 0.
    pub fn set_movement_parameters(&mut self, con: &Console, p: &[f64]) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScanManager::SetMovementParameters");
        if p.len() != self.n_movement_parameters() {
            return Err(topup_error(
                "TopupScanManager::SetMovementParameters: wrong size parameter vector p",
            ));
        }
        let mut indx = 0usize;
        for i in 0..self.scans.len() {
            let m = &self.mpi[i];
            let tmp = &p[indx..indx + m.len()];
            indx += m.len();
            if m.len() == 6 {
                let mp = [tmp[0], tmp[1], tmp[2], tmp[3], tmp[4], tmp[5]];
                self.scans[i].set_movement_parameters(con, &mp);
            } else if !m.is_empty() {
                let mut mp = [0.0; 6];
                for (j, &k) in m.iter().enumerate() {
                    mp[k] = tmp[j];
                }
                self.scans[i].set_movement_parameters(con, &mp);
            }
            if !self.scans[i].up_to_date() {
                self.up_to_date = false;
            }
        }
        con.trace(self.tp, "Leaving TopupScanManager::SetMovementParameters");
        Ok(())
    }

    /// `SetInterpolationModel(it)`.
    pub fn set_interpolation_model(&mut self, con: &Console, it: InterpType) {
        con.trace(self.tp, "Entering TopupScanManager::SetInterpolationModel");
        if Some(it) != self.it {
            self.up_to_date = false;
            self.it = Some(it);
            for s in &mut self.scans {
                s.set_interpolation_model(con, it);
            }
        }
        con.trace(self.tp, "Leaving TopupScanManager::SetInterpolationModel");
    }

    /// `update(field)`: the mean of the resampled scans, of their alphas, betas and gammas, and
    /// the product of their masks.
    fn update(&mut self, con: &Console, field: &mut SplineField) -> Result<(), FslError> {
        con.trace(self.tp, "Entering TopupScanManager::update");
        if !self.up_to_date {
            con.trace(
                self.tp,
                "TopupScanManager::update: Things not up to date, proceeds with updating.",
            );
            let (hb, hg) = (self.hasx, self.hasy);
            let mut mean = self.scans[0].get_resampled(con, field)?;
            let mut mask = self.scans[0].get_mask(con, field)?;
            let mut ma = self.scans[0].get_alpha(con, field)?;
            let mut mb = if hb {
                self.scans[0].get_beta(con, field)?
            } else {
                std::mem::replace(&mut self.mean_beta, Volume::new7([0, 0, 0, 0, 1, 1, 1]))
            };
            let mut mg = if hg {
                self.scans[0].get_gamma(con, field)?
            } else {
                std::mem::replace(&mut self.mean_gamma, Volume::new7([0, 0, 0, 0, 1, 1, 1]))
            };
            for i in 1..self.scans.len() {
                vol::add_assign(&mut mean, &self.scans[i].get_resampled(con, field)?)?;
                let m = self.scans[i].get_mask(con, field)?;
                for (a, b) in mask.data.iter_mut().zip(&m.data) {
                    *a = a.wrapping_mul(*b);
                }
                vol::add_assign(&mut ma, &self.scans[i].get_alpha(con, field)?)?;
                if self.has_beta_of(con, i)? {
                    vol::add_assign(&mut mb, &self.scans[i].get_beta(con, field)?)?;
                }
                if self.has_gamma_of(con, i)? {
                    vol::add_assign(&mut mg, &self.scans[i].get_gamma(con, field)?)?;
                }
            }
            let n = self.scans.len() as f32;
            for v in [&mut mean, &mut ma, &mut mb, &mut mg] {
                vol::div_scalar(v, n);
            }
            self.mean = mean;
            self.mask = mask;
            self.mean_alpha = ma;
            self.mean_beta = mb;
            self.mean_gamma = mg;
            self.up_to_date = true;
        }
        con.trace(self.tp, "Leaving TopupScanManager::update");
        Ok(())
    }
}
