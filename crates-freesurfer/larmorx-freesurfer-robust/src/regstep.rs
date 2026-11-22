// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `RegistrationStep<T>` (queue 3.2-03): one step of the robust symmetric registration (Reuter et
//! al., NeuroImage 2010), translated from FreeSurfer 8.2.0's `mri_robust_register/RegistrationStep.h`
//! for `float` and `double`:
//!
//! - [`RegistrationStep::construct_ab`] (`constructAb`): the linearised system `A p = b` of two
//!   images in the same space: the derivatives of their mean (`MyMRI::getPartials`), their
//!   smoothed difference (`getBlur`), optionally every second voxel at the fixed offsets of
//!   `getRand` (`subSample`), one row per voxel and frame that is inside both images, not NaN and
//!   not of a zero gradient (the model's [`Transformation::gradient`], the mean image as the
//!   intensity column), and the index image that maps each voxel to its row (or to why it has
//!   none: -10 not visited, -1 a zero gradient, -2 a NaN, -4 outside one image, -5 outside both);
//! - [`RegistrationStep::compute_registration_step`] (`computeRegistrationStep`): the robust
//!   ([`Regression::get_robust_est_w`]) or least-squares solution, the weights image (the squared
//!   robust weights in the source's space, 1 where a voxel has no row, 0 outside one image only)
//!   with its Gaussian-weighted checks, and the model's matrix of the solution with the intensity
//!   parameter.
//!
//! The registration that drives the steps (`RegRobust`, `Registration`) gives the step its
//! parameters ([`StepParams`]) and its transformation model (queue 3.2-03's second part).
//! `constructR` (the experimental restriction of the affine system to rigid parameters, `rtype
//! 2`) is never reached: `rtype` is always 1.
//!
//! Defects: **FS-1061** (an image of more than 2,147,483,647 voxels and frames: FreeSurfer's row
//! numbers in 32-bit integers; unreachable in memory, noted), **FS-1062** (subsampled: the model's
//! gradient takes the subsampled grid's positions; rule D, measured at `mri_robust_register`'s
//! replays: both modes as FreeSurfer until then).

use std::io::Write;

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::{FS_1061, FS_1062, FS_1069};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{DEFAULT_NAN64, add64, div64, mul64, sqrt64, sub64};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_RGB, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
    mri_alloc_sequence, mri_copy_header, mri_sizeof,
};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, Tri, mri_add, mri_scalar_mul, mri_subtract};
use larmorx_freesurfer_volume::resize::mri_set_resolution;

use crate::mymatrix::Mat4;
use crate::mymri::{get_blur, get_partials, get_rand, sub_sample, wide};
use crate::real::{Mat, Real};
use crate::regression::{Regression, SAT_R, SIG};
use crate::transformation::Transformation;

/// `Registration::Cost`: the registration's cost functions (the step solves `ROB` robustly and
/// any other by least squares).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Cost {
    Ls = 0,
    Rob,
    Mi,
    Nmi,
    Ecc,
    Ncc,
    Scr,
    Tb,
    Lncc,
    Sad,
    Sb,
}

impl Cost {
    /// The cost of its number (`(Registration::Cost) n`; `None` outside the enumeration).
    pub fn of(n: i32) -> Option<Cost> {
        use Cost::*;
        [Ls, Rob, Mi, Nmi, Ecc, Ncc, Scr, Tb, Lncc, Sad, Sb]
            .get(usize::try_from(n).ok()?)
            .copied()
    }
}

/// What `RegistrationStep(const RegRobust& R)` copies from the registration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepParams {
    /// The saturation (`sat`; a negative one: `SATr`, 4.685).
    pub sat: f64,
    pub iscale: bool,
    pub transonly: bool,
    pub rigid: bool,
    pub isoscale: bool,
    pub costfun: Cost,
    pub subsamplesize: i32,
    pub debug: i32,
    pub verbose: i32,
    pub iscalefinal: f64,
}

/// `RegistrationStep<T>`: the step's settings and what its last call left.
pub struct RegistrationStep<T: Real> {
    pub params: StepParams,
    /// `rtype` (always 1).
    rtype: i32,
    floatsvd: bool,
    /// `getMd()`.
    pub md: (Mat4, f64),
    /// `getWeights()`: the weights image of the last robust step.
    pub mri_weights: Option<Mri>,
    /// `getwcheck()`, `getwchecksqrt()`, `getzeroweights()`.
    pub wcheck: f64,
    pub wchecksqrt: f64,
    pub zeroweights: f64,
    /// The index image of the last system (each voxel's row, or why it has none).
    pub mri_indexing: Option<Mri>,
    /// The last solution.
    pub pvec: Vec<T>,
}

/// A `float` voxel of a float image (`MRIFseq_vox`).
#[inline]
fn fvox(m: &Mri, x: i32, y: i32, z: i32, f: i32) -> f32 {
    let i = (f as u64 * m.vox_per_vol
        + z as u64 * m.vox_per_slice
        + y as u64 * m.vox_per_row
        + x as u64) as usize;
    let c = m.chunk.as_ref().expect("a float image's voxels");
    f32::from_le_bytes(c[4 * i..4 * i + 4].try_into().expect("4 bytes"))
}

/// Writes a float voxel (`MRIFseq_vox(...) = v`).
#[inline]
fn set_fvox(m: &mut Mri, x: i32, y: i32, z: i32, f: i32, v: f32) {
    let i = (f as u64 * m.vox_per_vol
        + z as u64 * m.vox_per_slice
        + y as u64 * m.vox_per_row
        + x as u64) as usize;
    let c = m.chunk.as_mut().expect("a float image's voxels");
    c[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
}

/// The byte offset of `MRILseq_vox(m, x, y, z, f)`: FreeSurfer's `long32` is a 32-bit `int`, so
/// an index image of `MRI_LONG` (8-byte voxels) is read and written as 32-bit integers at 4-byte
/// steps along each row, as an `MRI_INT` one.
#[inline]
fn lvox_at(m: &Mri, x: i32, y: i32, z: i32, f: i32) -> usize {
    let row = f as u64 * m.vox_per_vol + z as u64 * m.vox_per_slice + y as u64 * m.vox_per_row;
    (row * mri_sizeof(m.mtype) as u64 + 4 * x as u64) as usize
}

#[inline]
fn lvox(m: &Mri, x: i32, y: i32, z: i32, f: i32) -> i32 {
    let o = lvox_at(m, x, y, z, f);
    let c = m.chunk.as_ref().expect("the index image's voxels");
    i32::from_le_bytes(c[o..o + 4].try_into().expect("4 bytes"))
}

#[inline]
fn set_lvox(m: &mut Mri, x: i32, y: i32, z: i32, f: i32, v: i32) {
    let o = lvox_at(m, x, y, z, f);
    let c = m.chunk.as_mut().expect("the index image's voxels");
    c[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// `MRIgetVoxVal(m, x, y, z, 0)` of a voxel inside the image, read straight from the voxels for
/// the types it reads (the same value; the step's loops visit every voxel twice); `None` for an
/// image without voxels or of another type (the caller then asks `mri_get_vox_val`).
fn frame0_reader(m: &Mri) -> Option<impl Fn(i32, i32, i32) -> f32 + '_> {
    let c = m.chunk.as_deref()?;
    let size = match m.mtype {
        MRI_UCHAR => 1u64,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_RGB | MRI_INT | MRI_FLOAT => 4,
        MRI_LONG => 8,
        _ => return None,
    };
    let (vpr, vps, ty) = (m.vox_per_row, m.vox_per_slice, m.mtype);
    Some(move |x: i32, y: i32, z: i32| {
        let o = ((x as u64 + y as u64 * vpr + z as u64 * vps) * size) as usize;
        match ty {
            MRI_UCHAR => f32::from(c[o]),
            MRI_SHORT => f32::from(i16::from_le_bytes([c[o], c[o + 1]])),
            MRI_USHRT => f32::from(u16::from_le_bytes([c[o], c[o + 1]])),
            MRI_FLOAT => f32::from_le_bytes(c[o..o + 4].try_into().expect("4 bytes")),
            MRI_LONG => i64::from_le_bytes(c[o..o + 8].try_into().expect("8 bytes")) as f32,
            _ => i32::from_le_bytes(c[o..o + 4].try_into().expect("4 bytes")) as f32,
        }
    })
}

/// `MyMRI::getRand(randpos)` as `(int)(2.0 * getRand(randpos))`, then `randpos++`.
#[inline]
fn rand_offset(randpos: &mut i32) -> i32 {
    let v = (2.0 * get_rand(randpos)) as i32;
    *randpos += 1;
    v
}

/// `fabs(v - outside) <= eps`: `v` (a `float` voxel) widened, the difference and the bound in
/// `double`.
#[inline]
fn at_outside(v: f32, outside: f64, eps: f64) -> bool {
    sub64(wide(v), outside).abs() <= eps
}

impl<T: Real> RegistrationStep<T> {
    /// `RegistrationStep<T>(R)`: the registration's settings, `floatsvd` off, no images.
    pub fn new(params: StepParams) -> RegistrationStep<T> {
        RegistrationStep {
            params,
            rtype: 1,
            floatsvd: false,
            md: ([[0.0; 4]; 4], 0.0),
            mri_weights: None,
            wcheck: 0.0,
            wchecksqrt: 0.0,
            zeroweights: 0.0,
            mri_indexing: None,
            pvec: Vec::new(),
        }
    }

    /// `setFloatSVD(fsvd)`: the weighted least squares in `float` (the registration always sets
    /// it).
    pub fn set_float_svd(&mut self, fsvd: bool) {
        self.floatsvd = fsvd;
    }

    /// `computeRegistrationStep(mriS, mriT)`: the step's matrix and intensity parameter for two
    /// images in the same space, with `trans` the registration's model (its parameters set to the
    /// solution). No model: `ERROR in RegistrationStep: no transform specified ...` on standard
    /// error and status 1; a system with an infinite or NaN entry: ` A or b constain NAN or
    /// infinity values!!` on standard error and status 1.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_registration_step(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        trans: Option<&mut Transformation>,
        mri_s: &mut Mri,
        mri_t: &mut Mri,
    ) -> Result<(Mat4, f64), Fatal> {
        let Some(trans) = trans else {
            let _ = writeln!(err, "ERROR in RegistrationStep: no transform specified ...");
            return Err(Fatal::Exit(1));
        };
        let p = self.params;
        debug_assert!(!(p.rigid && self.rtype == 2), "rtype is always 1");
        let (a, mut b) = self.construct_ab(g, out, err, trans, mri_s, mri_t)?;
        if p.verbose > 1 {
            let _ = write!(out, "   - checking A and b for nan ...");
        }
        if !a.is_finite() || !b.iter().all(|x| x.finite()) {
            let _ = writeln!(err, " A or b constain NAN or infinity values!!");
            return Err(Fatal::Exit(1));
        }
        if p.verbose > 1 {
            let _ = writeln!(out, "  DONE");
        }
        let lastweight;
        let mut w: Vec<T> = Vec::new();
        if p.costfun == Cost::Rob {
            {
                let mut r = Regression::new(&a, &mut b);
                r.set_verbose(p.verbose);
                r.set_float_svd(self.floatsvd);
                if p.verbose > 1 {
                    let _ = write!(
                        out,
                        "   - compute robust estimate ( sat {} )...",
                        ostream(p.sat)
                    );
                }
                let sat = if p.sat < 0.0 { SAT_R } else { p.sat };
                self.pvec = r.get_robust_est_w(g, out, err, &mut w, sat, SIG)?;
                lastweight = r.lastweight;
            }
            drop(a);
            drop(b);
            if p.verbose > 1 {
                let _ = writeln!(out, "  DONE");
            }
            self.weights_image(g, out, err, mri_s, &w)?;
            if p.verbose > 1 {
                let _ = writeln!(
                    out,
                    "   - Weight Check: {}  wsqrt: {}",
                    ostream(self.wcheck),
                    ostream(self.wchecksqrt)
                );
            }
        } else {
            if p.verbose > 1 {
                let _ = write!(out, "   - compute least squares estimate ...");
            }
            {
                let mut r = Regression::new(&a, &mut b);
                r.set_verbose(p.verbose);
                r.set_float_svd(self.floatsvd);
                self.pvec = r.get_ls_est(g, err)?;
                lastweight = r.lastweight;
            }
            drop(a);
            drop(b);
            if p.verbose > 1 {
                let _ = writeln!(out, "  DONE");
            }
            self.mri_weights = None;
        }
        self.zeroweights = lastweight;
        let mut second = 0.0;
        if p.iscale {
            second = self.pvec[self.pvec.len() - 1].wide();
        }
        // setParameters of vnl_vector<float> copies the first DOF entries; of vnl_vector<double>
        // the vector itself, then resizes it (FS-1063 when the intensity entry makes it longer)
        trans.set_parameters(g, &self.pvec, T::SINGLE);
        let first = trans.matrix(g, err)?;
        self.md = (first, second);
        Ok(self.md)
    }

    /// The weights image of a robust step (its checks into `wcheck`, `wchecksqrt`).
    fn weights_image(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mri_s: &Mri,
        w: &[T],
    ) -> Result<(), Fatal> {
        let (width, height, depth, nframes) = (
            mri_s.vg.width,
            mri_s.vg.height,
            mri_s.vg.depth,
            mri_s.nframes,
        );
        if let Some(m) = &self.mri_weights
            && (m.vg.width != width || m.vg.height != height || m.vg.depth != depth)
        {
            self.mri_weights = None;
        }
        if self.mri_weights.is_none() {
            let mut m = mri_alloc_sequence(g, out, err, width, height, depth, MRI_FLOAT, nframes)?;
            mri_copy_header(g, out, err, mri_s, Some(&mut m))?;
            m.mtype = MRI_FLOAT;
            mri_set_resolution(
                g,
                out,
                err,
                &mut m,
                mri_s.vg.xsize,
                mri_s.vg.ysize,
                mri_s.vg.zsize,
            )?;
            m.outside_val = 1.0;
            self.mri_weights = Some(m);
        }
        let libm = Libm::new(g.mode());
        let index = self
            .mri_indexing
            .as_ref()
            .expect("the index image of the step's system");
        let weights = self.mri_weights.as_mut().expect("the weights image");
        let mut sigma = f64::from(width);
        if f64::from(height) > sigma {
            sigma = f64::from(height);
        }
        if f64::from(depth) > sigma {
            sigma = f64::from(depth);
        }
        sigma = div64(sigma, 6.0);
        let sigma22 = mul64(mul64(2.0, sigma), sigma);
        let factor = div64(1.0, sqrt64(mul64(std::f64::consts::PI, sigma22)));
        let (mut dsum, mut wcheck, mut wchecksqrt) = (0.0f64, 0.0f64, 0.0f64);
        let (hw, hh, hd) = (
            mul64(0.5, f64::from(width)),
            mul64(0.5, f64::from(height)),
            mul64(0.5, f64::from(depth)),
        );
        for f in 0..nframes {
            for z in 0..depth {
                for x in 0..width {
                    for y in 0..height {
                        let val = lvox(index, x, y, z, f);
                        if val < 0 {
                            set_fvox(weights, x, y, z, f, 1.0);
                        }
                        if val == -4 {
                            set_fvox(weights, x, y, z, f, 0.0);
                        }
                        if val >= 0 {
                            let wv = w[val as usize];
                            let wtemp = wv.sse_mul(wv).wide();
                            set_fvox(weights, x, y, z, f, f32::narrow(wtemp));
                            let xx = sub64(f64::from(x), hw);
                            let yy = sub64(f64::from(y), hh);
                            let zz = sub64(f64::from(z), hd);
                            let distance2 =
                                add64(add64(mul64(xx, xx), mul64(yy, yy)), mul64(zz, zz));
                            let arg = div64(-distance2, sigma22);
                            let e = libm.exp(arg);
                            let e = if e.is_nan() {
                                if arg.is_nan() {
                                    f64::from_bits(arg.to_bits() | 0x0008_0000_0000_0000)
                                } else {
                                    DEFAULT_NAN64
                                }
                            } else {
                                e
                            };
                            let gauss = mul64(factor, e);
                            dsum = add64(dsum, gauss);
                            wcheck = add64(wcheck, mul64(gauss, sub64(1.0, wtemp)));
                            wchecksqrt = add64(wchecksqrt, mul64(gauss, sub64(1.0, wv.wide())));
                        }
                    }
                }
            }
        }
        note_glibc07(g, &libm, "exp");
        self.wcheck = div64(wcheck, dsum);
        self.wchecksqrt = div64(wchecksqrt, dsum);
        Ok(())
    }

    /// `constructAb(mriS, mriT, A, b)`: the step's system (see the module's notes) and its index
    /// image. Source and target must have the same size, frames and type (FreeSurfer's asserts,
    /// compiled out). One 2-D and one 3-D image: `ERROR: both source and target need to be 2D or
    /// 3D` on standard output and status 1; no usable voxel: FreeSurfer's advice (`ERROR: All
    /// entries are zero! Images do not overlap (anymore?).` and nine more lines) on standard error
    /// and status 1. A system above 3,800 MB (FreeSurfer's estimate of the solver's peak) prints
    /// `     -- WARNING: mem usage large: <MB>Mb mem + 6 MRI` and `          Maybe use --subsample
    /// <int> ` on standard output.
    #[allow(clippy::too_many_arguments)]
    pub fn construct_ab(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        trans: &Transformation,
        mri_s: &mut Mri,
        mri_t: &mut Mri,
    ) -> Result<(Mat<T>, Vec<T>), Fatal> {
        let p = self.params;
        let verbose = p.verbose;
        if verbose > 1 {
            let _ = writeln!(out, "   - constructAb: ");
        }
        if mri_s.nframes == 0 {
            mri_s.nframes = 1;
        }
        if mri_t.nframes == 0 {
            mri_t.nframes = 1;
        }
        let mut is2d = false;
        if mri_s.vg.depth == 1 || mri_t.vg.depth == 1 {
            if mri_t.vg.depth != mri_s.vg.depth {
                let _ = writeln!(out, "ERROR: both source and target need to be 2D or 3D");
                return Err(Fatal::Exit(1));
            }
            is2d = true;
        }
        let (vs, vt) = (&mri_s.vg, &mri_t.vg);
        if (vs.width, vs.height, vs.depth, mri_s.nframes, mri_s.mtype)
            != (vt.width, vt.height, vt.depth, mri_t.nframes, mri_t.mtype)
        {
            // FreeSurfer's size asserts are compiled out: it reads the target past its voxels
            g.notices.add(
                &FS_1069,
                "a registration step of two images of different sizes, frames or types (FreeSurfer reads past one)",
            );
            let _ = writeln!(
                err,
                "RegistrationStep::constructAb: the source ({} x {} x {}, {} frames, type {}) and the target ({} x {} x {}, {} frames, type {}) differ (FreeSurfer reads past one of them here)",
                vs.width,
                vs.height,
                vs.depth,
                mri_s.nframes,
                mri_s.mtype,
                vt.width,
                vt.height,
                vt.depth,
                mri_t.nframes,
                mri_t.mtype
            );
            return Err(Fatal::Exit(1));
        }
        let (sw, sh, sd, snf) = (
            mri_s.vg.width,
            mri_s.vg.height,
            mri_s.vg.depth,
            mri_s.nframes,
        );
        let ss = i64::from(sw) * i64::from(sh) * i64::from(sd) * i64::from(snf);
        self.mri_indexing = None;
        let itype = if ss > i64::from(i32::MAX) {
            g.notices.add(
                &FS_1061,
                format!("a registration step of {ss} voxels and frames: FreeSurfer's row numbers are 32-bit integers"),
            );
            if verbose > 1 {
                let _ = writeln!(out, "     -- using LONG for indexing ... ");
                let mu = (ss as f64) * 8.0 / (1024.0 * 1024.0);
                let _ = write!(
                    out,
                    "     -- allocating {}Mb mem for indexing ... ",
                    ostream(mu)
                );
            }
            MRI_LONG
        } else {
            if verbose > 1 {
                let _ = writeln!(out, "     -- using INT for indexing ... ");
                let mu = div64(mul64(ss as f64, 4.0), 1024.0 * 1024.0);
                let _ = write!(
                    out,
                    "     -- allocating {}Mb mem for indexing ... ",
                    ostream(mu)
                );
            }
            MRI_INT
        };
        let mut index = mri_alloc_sequence(g, out, err, sw, sh, sd, itype, snf)?;
        index.outside_val = -10.0;
        if verbose > 1 {
            let _ = writeln!(out, " done!");
        }
        for z in 0..sd {
            for x in 0..sw {
                for y in 0..sh {
                    for f in 0..snf {
                        set_lvox(&mut index, x, y, z, f, -10);
                    }
                }
            }
        }
        let ssz = p.subsamplesize;
        let dosubsample = ssz > 0 && sw > ssz && sh > ssz && (sd > ssz || sd == 1);
        if verbose > 1 {
            let _ = write!(out, "     -- compute derivatives ... ");
        }
        let mut spth = mri_alloc_sequence(g, out, err, sw, sh, sd, MRI_FLOAT, snf)?;
        mri_add(g, out, err, Tri::Given(mri_s, mri_t, &mut spth))?;
        mri_scalar_mul(g, out, err, Pair::Same(&mut spth), 0.5)?;
        let parts = get_partials(g, out, err, &spth)?;
        drop(spth);
        let mut smt = mri_alloc_sequence(g, out, err, sw, sh, sd, MRI_FLOAT, snf)?;
        mri_subtract(g, out, err, Tri::Given(mri_s, mri_t, &mut smt))?;
        mymri_blur_in_place(g, out, err, &mut smt)?;
        if verbose > 1 {
            let _ = writeln!(out, " done!");
        }
        let (fx, fy, fz, ft, smt) = if dosubsample {
            if verbose > 1 {
                let _ = write!(out, "     -- subsample ... ");
            }
            let sub = |g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: Mri| {
                sub_sample(g, out, err, &m, None, false, 0)
                    .map(|r| r.into_new().expect("subSample's new image"))
            };
            let fx = sub(g, out, err, parts.fx)?;
            let fy = sub(g, out, err, parts.fy)?;
            let fz = match parts.fz {
                Some(m) => Some(sub(g, out, err, m)?),
                None => None,
            };
            let ft = sub(g, out, err, parts.blur)?;
            let smt = sub(g, out, err, smt)?;
            if verbose > 1 {
                let _ = writeln!(out, " done! ");
            }
            g.notices.add(
                &FS_1062,
                "a subsampled step: the model's gradient at the subsampled grid's positions (as FreeSurfer)",
            );
            (fx, fy, fz, ft, smt)
        } else {
            (parts.fx, parts.fy, parts.fz, parts.blur, smt)
        };
        let (fxw, fxh, fxd, fxf) = (fx.vg.width, fx.vg.height, fx.vg.depth, fx.nframes);
        let n = i64::from(fxw) * i64::from(fxh) * i64::from(fxd) * i64::from(fxf);
        if verbose > 1 {
            let _ = write!(
                out,
                "     -- size {fxw} x {fxh} x {fxd} x {fxf} = {}",
                n as i32
            );
        }
        let eps = 0.00001f64;
        let oepss = add64(eps, div64(mri_s.outside_val, 255.0));
        let oepst = add64(eps, div64(mri_t.outside_val, 255.0));
        let zero_fz = (eps / 2.0) as f32;
        let (rs, rt) = (frame0_reader(mri_s), frame0_reader(mri_t));
        let mut counti: i64 = 0;
        let (mut ocount, mut ncount, mut zcount) = (0i32, 0i32, 0i32);
        let mut randpos = 0i32;
        let grid = |randpos: &mut i32, x: i32, y: i32, z: i32| -> (i32, i32, i32) {
            if dosubsample {
                let dx = rand_offset(randpos);
                let dy = rand_offset(randpos);
                let xp1 = 2 * x + dx;
                let yp1 = 2 * y + dy;
                let zp1 = if is2d {
                    z
                } else {
                    let dz = rand_offset(randpos);
                    2 * z + dz
                };
                (xp1, yp1, zp1)
            } else {
                (x, y, z)
            }
        };
        let fvals = |x, y, z, f| {
            let ftval = fvox(&ft, x, y, z, f);
            let fxval = fvox(&fx, x, y, z, f);
            let fyval = fvox(&fy, x, y, z, f);
            let fzval = match &fz {
                Some(m) if !is2d => fvox(m, x, y, z, f),
                _ => zero_fz,
            };
            (ftval, fxval, fyval, fzval)
        };
        for z in 0..fxd {
            for x in 0..fxw {
                for y in 0..fxh {
                    let (xp1, yp1, zp1) = grid(&mut randpos, x, y, z);
                    let sv = match &rs {
                        Some(r) => r(xp1, yp1, zp1),
                        None => mri_get_vox_val(g, err, mri_s, xp1, yp1, zp1, 0, 0)?,
                    };
                    let tv = match &rt {
                        Some(r) => r(xp1, yp1, zp1),
                        None => mri_get_vox_val(g, err, mri_t, xp1, yp1, zp1, 0, 0)?,
                    };
                    if at_outside(sv, mri_s.outside_val, oepss)
                        || at_outside(tv, mri_t.outside_val, oepst)
                    {
                        ocount = ocount.wrapping_add(fxf);
                        continue;
                    }
                    for f in 0..fxf {
                        let (ftval, fxval, fyval, fzval) = fvals(x, y, z, f);
                        if fxval.is_nan() || fyval.is_nan() || fzval.is_nan() || ftval.is_nan() {
                            ncount += 1;
                            continue;
                        }
                        if wide(fxval).abs() < eps
                            && wide(fyval).abs() < eps
                            && wide(fzval).abs() < eps
                        {
                            zcount += 1;
                            continue;
                        }
                        counti += 1;
                    }
                }
            }
        }
        if verbose > 1 && n > counti {
            let _ = writeln!(out, "  need only: {counti}");
        }
        if counti == 0 {
            let _ = write!(
                err,
                "\n ERROR: All entries are zero! Images do not overlap (anymore?).\n    This can have several reasons (i.e. different modalities, different \n    intensity scales, large non-linearities, too diff. voxel sizes ...)\n    Try calling with --noinit (if the original images are well aligned)\n    Maybe use --ixform <init.lta> with an approx. alignment\n    obtained from tkregister or another registration program.\n    Or do some prior intensity correction? \n    You can also try to switch off symmetric reg. via --nosym \n    or manually play around with the --sat parameter. \n\n"
            );
            return Err(Fatal::Exit(1));
        }
        if verbose > 1 {
            let _ = writeln!(
                out,
                "     -- nans: {ncount} zeros: {zcount} outside: {ocount}"
            );
        }
        let mut pnum = trans.dof();
        if p.iscale {
            pnum += 1;
        }
        let tsize = if T::SINGLE { 4.0 } else { 8.0 };
        let amu = div64(
            mul64(mul64(counti as f64, (pnum + 1) as f64), tsize),
            1024.0 * 1024.0,
        );
        let bmu = div64(mul64(counti as f64, tsize), 1024.0 * 1024.0);
        if verbose > 1 {
            let _ = write!(
                out,
                "     -- allocating {}Mb mem for A and b ... ",
                ostream(add64(amu, bmu))
            );
        }
        let rows = counti as usize;
        let mut a = Mat::<T>::zeros(rows, pnum);
        let mut b = vec![T::ZERO; rows];
        if verbose > 1 {
            let _ = writeln!(out, " done! ");
        }
        let mut maxmu = add64(mul64(5.0, amu), mul64(7.0, bmu));
        let mut fstr = "";
        if self.floatsvd {
            maxmu = add64(add64(amu, mul64(3.0, bmu)), mul64(2.0, add64(amu, bmu)));
            fstr = "-float";
        }
        if verbose > 1 {
            let _ = writeln!(
                out,
                "         (MAX usage in SVD{fstr} will be > {}Mb mem + 6 MRI) ",
                ostream(maxmu)
            );
        }
        if maxmu > 3800.0 {
            let _ = writeln!(
                out,
                "     -- WARNING: mem usage large: {}Mb mem + 6 MRI",
                ostream(maxmu)
            );
            let _ = writeln!(out, "          Maybe use --subsample <int> ");
        }
        let mut count = 0usize;
        let mut grad = [0.0f64; 12];
        randpos = 0;
        for z in 0..fxd {
            for x in 0..fxw {
                for y in 0..fxh {
                    let (xp1, yp1, zp1) = grid(&mut randpos, x, y, z);
                    let sv = match &rs {
                        Some(r) => r(xp1, yp1, zp1),
                        None => mri_get_vox_val(g, err, mri_s, xp1, yp1, zp1, 0, 0)?,
                    };
                    let tv = match &rt {
                        Some(r) => r(xp1, yp1, zp1),
                        None => mri_get_vox_val(g, err, mri_t, xp1, yp1, zp1, 0, 0)?,
                    };
                    let s_out = at_outside(sv, mri_s.outside_val, oepss);
                    let t_out = at_outside(tv, mri_t.outside_val, oepst);
                    if s_out || t_out {
                        let outval = if s_out && t_out { -5 } else { -4 };
                        for f in 0..fxf {
                            set_lvox(&mut index, xp1, yp1, zp1, f, outval);
                        }
                        continue;
                    }
                    for f in 0..fxf {
                        let (ftval, fxval, fyval, fzval) = fvals(x, y, z, f);
                        if fxval.is_nan() || fyval.is_nan() || fzval.is_nan() || ftval.is_nan() {
                            set_lvox(&mut index, xp1, yp1, zp1, f, -2);
                            continue;
                        }
                        if wide(fxval).abs() < eps
                            && wide(fyval).abs() < eps
                            && wide(fzval).abs() < eps
                        {
                            set_lvox(&mut index, xp1, yp1, zp1, f, -1);
                            continue;
                        }
                        // the row number stored as FreeSurfer's 32-bit integer
                        set_lvox(&mut index, xp1, yp1, zp1, f, count as i32);
                        let dof = trans.gradient_into(
                            err, x as u32, fxval, y as u32, fyval, z as u32, fzval, &mut grad,
                        )?;
                        for (pno, &gv) in grad[..dof].iter().enumerate() {
                            *a.at_mut(count, pno) = T::narrow(gv);
                        }
                        if p.iscale {
                            *a.at_mut(count, dof) = T::of_f32(ftval);
                        }
                        b[count] = T::of_f32(fvox(&smt, x, y, z, f));
                        count += 1;
                    }
                }
            }
        }
        self.mri_indexing = Some(index);
        Ok((a, b))
    }
}

/// `MyMRI::getBlur(m, m)` in place.
fn mymri_blur_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
) -> Result<(), Fatal> {
    get_blur(g, out, err, Pair::Same(m))?;
    Ok(())
}
