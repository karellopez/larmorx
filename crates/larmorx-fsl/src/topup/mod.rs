// SPDX-License-Identifier: Apache-2.0
//! topup's estimation of the susceptibility-induced field (FSL 6.0.7.23's `topup`, fsl-topup
//! 2203.5), a clean-room original written from `specs/topup.md`, J. L. R. Andersson, S. Skare
//! and J. Ashburner's paper ("How to correct susceptibility distortions in spin-echo
//! echo-planar images: application to diffusion tensor imaging", NeuroImage 20(2):870-888,
//! 2003), FSL's TopupUsersGuide and black-box runs of FSL's program (see `PROVENANCE.md` and
//! `docs/findings/fsl-topup-original.md`).
//!
//! - [`images`]: the intensity scaling, the regridding, each level's subsampling, smoothing and
//!   spline interpolation (§4.1, §4.4, §4.5);
//! - [`spline`]: the field's B-spline basis, its refits and the regulariser's Gram matrices
//!   (§4.2, §4.4, §4.6);
//! - [`model`]: one level's predictions, cost, gradient and Gauss-Newton Hessian (§4.3, §4.6);
//! - [`stencil`]: the Hessian's storage and its conjugate-gradient solve (§4.7, §11);
//! - [`minimise`]: Levenberg-Marquardt and scaled conjugate gradients (§4.7);
//! - [`apply`]: applytopup's corrections (§7);
//! - [`settings`]: larmorx's own settings of FSL's six configurations, by name (§2.5).
//!
//! The command line is [`crate::cli::topup`].

pub mod apply;
pub mod images;
pub mod minimise;
pub mod model;
pub mod movement;
pub mod pool;
pub mod settings;
pub mod spline;
pub mod stencil;
pub mod vb;

pub use model::{RegMod, Scan};

use images::{LevelImages, Prepared};
use model::{Eval, Level};
use spline::{Axis, Op1, apply3, refit_op};
use stencil::Hessian;

/// The minimiser of a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Minimiser {
    LevenbergMarquardt,
    ScaledConjugateGradients,
}

/// One level's settings (§2.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelSpec {
    pub warpres: f64,
    pub subsamp: usize,
    pub fwhm: f64,
    pub miter: usize,
    pub lambda: f64,
    pub estmov: bool,
    pub minmet: Minimiser,
}

/// The estimation's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub levels: Vec<LevelSpec>,
    pub ssqlambda: bool,
    pub regmod: RegMod,
    /// Each volume scaled to a mean of 100 by itself (`--scale=1`).
    pub scale: bool,
    pub regrid: bool,
    /// The voxels the regridding adds per axis (FSL: the largest subsampling; FSL-118's
    /// correction: their least common multiple).
    pub extra: usize,
    /// The Levenberg-Marquardt Hessian in single precision (`--numprec=float`).
    pub single: bool,
    /// FSL-117 reproduced: the membrane energy's Levenberg-Marquardt Hessian per mm.
    pub membrane_hessian_mm: bool,
    /// FSL-145 reproduced: FSL's quadratic count on an axis shorter than two knot spacings (one
    /// or two coefficients), and FSL's error when a level refits a single coefficient.
    pub fsl145: bool,
    /// FSL-146 reproduced: each axis' Jacobian term of the gradient over the volumes encoded
    /// along that axis only.
    pub fsl146: bool,
    /// FSL-201 reproduced: the movements through FSL's matrices, their derivatives FSL's forward
    /// differences.
    pub fsl201: bool,
    /// FSL-242 reproduced: a level of scaled conjugate gradients whose last iteration is rejected
    /// ends at that iteration's point (else at the current point).
    pub fsl242: bool,
    /// Trilinear interpolation of the images (`--interp=linear`).
    pub linear: bool,
    /// The field's B-spline order (`--splineorder`: 3 or 2).
    pub order: u8,
    /// glibc's `exp` in the smoothing (`reproduce`, GLIBC-07); correctly rounded otherwise.
    pub glibc: bool,
}

/// Whether the volumes are encoded along different sets of axes (FSL-146: the two modes'
/// gradients differ): some axis along which one volume's phase encoding has a component and
/// another's has none (the encoding decides, whatever the read-out time: FSL leaves out of an
/// axis' Jacobian term only the volumes whose encoding along it is 0, `specs/topup.md` §4.6).
pub fn meets_fsl146(scans: &[Scan]) -> bool {
    (0..3).any(|a| {
        let on = |s: &Scan| s.pe[a] != 0.0;
        scans.iter().any(on) && !scans.iter().all(on)
    })
}

/// Whether FSL refits onto a grid of `axes` that has a single quadratic coefficient along some
/// axis (FSL-145 reproduced: the refit's two wrap-around rows need two coefficients, and FSL
/// stops with `Mat::operator(): index out of bounds`; `specs/topup.md` §4.2).
fn single_coefficient(axes: &[Axis; 3]) -> bool {
    axes.iter()
        .any(|a| a.order == 2 && a.fsl145 && a.ncoef() == 1)
}

/// Whether a level's or the output's layout has quadratic B-splines on an axis shorter than two
/// knot spacings (FSL-145: the two modes lay it differently).
pub fn meets_fsl145(params: &Params, dims: [usize; 3], voxel: [f64; 3]) -> bool {
    if params.order != 2 {
        return false;
    }
    let short = |n: [usize; 3], k: [usize; 3]| {
        (0..3).any(|a| Axis::of_order(n[a], k[a], 2).short_quadratic())
    };
    let level = |spec: &LevelSpec| {
        let s = spec.subsamp.max(1);
        short(
            dims.map(|n| (n / s).max(1)),
            knot_spacing(spec.warpres, voxel.map(|v| v * s as f64)),
        )
    };
    params.levels.iter().any(level)
        || params
            .levels
            .last()
            .is_some_and(|l| l.subsamp > 1 && short(dims, knot_spacing(l.warpres, voxel)))
}

/// The knot spacing (voxels) for a resolution of `warpres` mm on voxels of `voxel` mm (§4.2).
pub fn knot_spacing(warpres: f64, voxel: [f64; 3]) -> [usize; 3] {
    voxel.map(|v| {
        let k = (warpres / v).round();
        if k >= 1.0 { k as usize } else { 1 }
    })
}

/// A field: its axes (grid and knot spacing) and coefficients (x fastest).
#[derive(Clone, Debug)]
pub struct Field {
    pub axes: [Axis; 3],
    pub coef: Vec<f64>,
}

impl Field {
    pub fn zeros(axes: [Axis; 3]) -> Field {
        let n = axes.iter().map(Axis::ncoef).product();
        Field {
            axes,
            coef: vec![0.0; n],
        }
    }

    pub fn coef_dims(&self) -> [usize; 3] {
        self.axes.map(|a| a.ncoef())
    }

    /// The field sampled at the positions `xs` (voxels, per axis) and refitted on the grid
    /// `to` (§4.4).
    pub fn resample(&self, xs: [Vec<f64>; 3], to: [Axis; 3]) -> Field {
        let ops: [Op1; 3] = std::array::from_fn(|a| self.axes[a].rows(&xs[a], 0));
        let (values, dims) = apply3(
            &self.coef,
            self.coef_dims(),
            [Some(&ops[0]), Some(&ops[1]), Some(&ops[2])],
        );
        let refit: [Op1; 3] = std::array::from_fn(|a| refit_op(to[a]));
        let (coef, _) = apply3(
            &values,
            dims,
            [Some(&refit[0]), Some(&refit[1]), Some(&refit[2])],
        );
        Field { axes: to, coef }
    }

    /// The field on a grid `step` times finer or coarser (`step = new s / old s`): sampled at
    /// the new voxels' centres, `(step - 1) / 2 + j step` old voxels, and refitted with knot
    /// spacing `k` (§4.4).
    pub fn regrid(&self, n: [usize; 3], step: f64, k: [usize; 3]) -> Field {
        let xs = std::array::from_fn(|a| {
            (0..n[a])
                .map(|j| (step - 1.0) / 2.0 + j as f64 * step)
                .collect()
        });
        self.resample(xs, std::array::from_fn(|a| self.axes[a].like(n[a], k[a])))
    }

    /// The field with knot spacing `k` on the same grid (sampled at the voxels and refitted).
    pub fn respace(&self, k: [usize; 3]) -> Field {
        let n = self.axes.map(|a| a.n);
        let xs = std::array::from_fn(|a| (0..n[a]).map(|j| j as f64).collect());
        self.resample(xs, std::array::from_fn(|a| self.axes[a].like(n[a], k[a])))
    }
}

/// What the estimation reports as it goes: every cost evaluation (with its level, iteration and
/// attempt, as FSL's `--debug` files number them), gradient and Hessian, and each new level.
pub trait Progress {
    fn evaluated(&mut self, index: usize, level: &Level, e: &Eval, iter: usize, attempt: usize);
    fn next_level(&mut self, index: usize);
    fn gradient(&mut self, _index: usize, _level: &Level, _e: &Eval, _g: &[f64], _iter: usize) {}
    fn hessian(&mut self, _index: usize, _level: &Level, _e: &Eval, _h: &Hessian, _iter: usize) {}
    /// A level begins (`index` from 0) or ends.
    fn level_started(&mut self, _index: usize) {}
    fn level_finished(&mut self, _index: usize) {}
}

/// A level's [`minimise::Report`] forwarding to the run's [`Progress`].
struct LevelReport<'a> {
    progress: &'a mut dyn Progress,
    index: usize,
}

impl minimise::Report for LevelReport<'_> {
    fn evaluated(&mut self, level: &Level, e: &Eval, iter: usize, attempt: usize) {
        self.progress.evaluated(self.index, level, e, iter, attempt);
    }
    fn gradient(&mut self, level: &Level, e: &Eval, g: &[f64], iter: usize) {
        self.progress.gradient(self.index, level, e, g, iter);
    }
    fn hessian(&mut self, level: &Level, e: &Eval, h: &Hessian, iter: usize) {
        self.progress.hessian(self.index, level, e, h, iter);
    }
}

/// The result: the field on the input's grid and the prepared volumes.
#[derive(Clone, Debug)]
pub struct Estimate {
    pub field: Field,
    pub prepared: Prepared,
    pub warpres_last: f64,
    /// Each volume's movement (tx, ty, tz, rx, ry, rz; the first volume's 0).
    pub movements: Vec<[f64; 6]>,
    /// The smoothing (FWHM, mm) of the outputs' predictions: the last level's when it was not
    /// subsampled, else none (§6: a subsampled last level's smoothing and subsampling are
    /// removed; observed, `thr/openblas`: one level of FWHM 4 mm, FSL's `--iout` is smoothed).
    pub fwhm_out: f64,
    /// The run's choice of `exp` (glibc's with `reproduce`).
    pub glibc: bool,
    /// FSL-201 reproduced: the outputs' predictions moved through FSL's matrices.
    pub fsl201: bool,
    /// Some level of scaled conjugate gradients ended on a rejected iteration (FSL-242: the two
    /// modes' results differ).
    pub fsl242_met: bool,
}

/// Runs the estimation (§4) on `volumes` (each `dims`, x fastest, radiological frame) with voxel
/// sizes `voxel` and acquisitions `scans`.
pub fn estimate(
    volumes: &[Vec<f32>],
    dims: [usize; 3],
    voxel: [f64; 3],
    scans: &[Scan],
    params: &Params,
    progress: &mut dyn Progress,
) -> Result<Estimate, LevelError> {
    let prepared = images::prepare(
        volumes,
        dims,
        voxel,
        params.scale,
        params.regrid.then_some(params.extra),
        params.linear,
    );
    let mut field: Option<Field> = None;
    let mut prev: Option<LevelSpec> = None;
    let mut cached: Option<(usize, f64, LevelImages)> = None;
    let mut fsl242_met = false;
    let mut mov = vec![[0.0; 6]; scans.len()];
    for (l, spec) in params.levels.iter().enumerate() {
        let s = spec.subsamp.max(1);
        if prepared.reg_dims.iter().any(|&n| n % s != 0) {
            // FSL-118 reproduced: the regridded grid does not divide by this level's subsampling
            return Err(LevelError::Subsampling(l + 1));
        }
        let nt = dims.map(|n| (n / s).max(1));
        let vt = voxel.map(|v| v * s as f64);
        let k = knot_spacing(spec.warpres, vt);
        let mut f = match (field.take(), prev) {
            (Some(f), Some(p)) => {
                progress.next_level(l);
                let mut f = f;
                if p.subsamp != s {
                    let step = s as f64 / p.subsamp as f64;
                    let kp = knot_spacing(p.warpres, vt);
                    let to: [Axis; 3] = std::array::from_fn(|a| f.axes[a].like(nt[a], kp[a]));
                    if single_coefficient(&to) {
                        return Err(LevelError::SingleCoefficient(l + 1));
                    }
                    f = f.regrid(nt, step, kp);
                }
                // refitted when the resolution changes, even to the same knot spacing (observed:
                // the regularisation `-v` prints at the next level's start)
                if f.axes.map(|a| a.k) != k || p.warpres.to_bits() != spec.warpres.to_bits() {
                    let to: [Axis; 3] = std::array::from_fn(|a| f.axes[a].like(nt[a], k[a]));
                    if single_coefficient(&to) {
                        return Err(LevelError::SingleCoefficient(l + 1));
                    }
                    f = f.respace(k);
                }
                f
            }
            _ => {
                let axes: [Axis; 3] = std::array::from_fn(|a| {
                    Axis::of_order(nt[a], k[a], params.order).with_fsl145(params.fsl145)
                });
                // the first level's field is made on the input's grid and, when the level is
                // subsampled, refitted onto the level's (§4.4): it stays 0, but FSL's refit of a
                // single coefficient stops it there (FSL-145)
                if s > 1 && single_coefficient(&axes) {
                    return Err(LevelError::SingleCoefficient(1));
                }
                Field::zeros(axes)
            }
        };
        let imgs = match &cached {
            Some((cs, cf, im)) if *cs == s && cf.to_bits() == spec.fwhm.to_bits() => im.clone(),
            _ => {
                let im = images::level_images(&prepared, s, spec.fwhm, params.glibc);
                cached = Some((s, spec.fwhm, im.clone()));
                im
            }
        };
        let mut level = Level::new(
            imgs,
            dims,
            voxel,
            s,
            f.axes,
            scans,
            spec.lambda,
            params.ssqlambda,
            params.regmod,
            &mov,
            spec.estmov,
        );
        level.single = params.single;
        level.membrane_hessian_mm = params.membrane_hessian_mm;
        level.fsl146 = params.fsl146;
        level.fsl201 = params.fsl201;
        progress.level_started(l);
        let mut rep = LevelReport {
            progress: &mut *progress,
            index: l,
        };
        let theta = level.start(std::mem::take(&mut f.coef));
        let theta = match spec.minmet {
            Minimiser::LevenbergMarquardt => {
                minimise::levenberg_marquardt(&level, theta, spec.miter, &mut rep)
            }
            Minimiser::ScaledConjugateGradients => {
                let end = minimise::scaled_conjugate_gradients(
                    &level,
                    theta,
                    spec.miter,
                    params.fsl242,
                    &mut rep,
                );
                fsl242_met |= end.last_rejected;
                end.x
            }
        };
        mov = level.movements(&theta);
        f.coef = theta[..level.n_coef()].to_vec();
        progress.level_finished(l);
        field = Some(f);
        prev = Some(*spec);
    }
    let last = prev.expect("at least one level");
    let mut f = field.expect("at least one level");
    if last.subsamp > 1 {
        let k = knot_spacing(last.warpres, voxel);
        let to: [Axis; 3] = std::array::from_fn(|a| f.axes[a].like(dims[a], k[a]));
        if single_coefficient(&to) {
            return Err(LevelError::SingleCoefficient(params.levels.len() + 1));
        }
        f = f.regrid(dims, 1.0 / last.subsamp as f64, k);
        if f.axes.map(|a| a.k) != k {
            f = f.respace(k);
        }
    }
    Ok(Estimate {
        field: f,
        prepared,
        warpres_last: last.warpres,
        movements: mov,
        fwhm_out: if last.subsamp > 1 { 0.0 } else { last.fwhm },
        glibc: params.glibc,
        fsl201: params.fsl201,
        fsl242_met,
    })
}

/// Why a level could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LevelError {
    /// The level (1-based) whose subsampling does not divide the regridded grid (FSL-118).
    Subsampling(usize),
    /// The level (1-based; one past the last: the outputs' grid) whose refit has a single
    /// quadratic coefficient along an axis (FSL-145 reproduced).
    SingleCoefficient(usize),
}

/// The predictions at full resolution (§6): the regridded volumes (smoothed as the last level's
/// when it was not subsampled, [`Estimate::fwhm_out`]) interpolated on the input's grid.
pub fn full_resolution(est: &Estimate, scans: &[Scan], regmod: RegMod) -> (Level, Eval) {
    let p = &est.prepared;
    let imgs = images::level_images(p, 1, est.fwhm_out, est.glibc);
    let level = Level::new(
        imgs,
        p.dims,
        p.voxel,
        1,
        est.field.axes,
        scans,
        0.0,
        false,
        regmod,
        &est.movements,
        false,
    );
    let mut level = level;
    level.fsl201 = est.fsl201;
    let ev = level.eval(&est.field.coef);
    (level, ev)
}
