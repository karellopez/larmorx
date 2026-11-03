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
//! - [`minimise`]: Levenberg-Marquardt and scaled conjugate gradients (§4.7).
//!
//! The command line is [`crate::cli::topup`].

pub mod images;
pub mod minimise;
pub mod model;
pub mod movement;
pub mod spline;
pub mod stencil;

pub use model::{RegMod, Scan};

use images::{LevelImages, Prepared};
use model::{Eval, Level};
use spline::{Axis, Op1, apply3, refit_op};

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
    /// The Levenberg-Marquardt Hessian in single precision (`--numprec=float`).
    pub single: bool,
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
        self.resample(xs, std::array::from_fn(|a| Axis::new(n[a], k[a])))
    }

    /// The field with knot spacing `k` on the same grid (sampled at the voxels and refitted).
    pub fn respace(&self, k: [usize; 3]) -> Field {
        let n = self.axes.map(|a| a.n);
        let xs = std::array::from_fn(|a| (0..n[a]).map(|j| j as f64).collect());
        self.resample(xs, std::array::from_fn(|a| Axis::new(n[a], k[a])))
    }
}

/// What the estimation reports as it goes: every cost evaluation and each new level.
pub trait Progress {
    fn evaluated(&mut self, level: &Level, e: &Eval);
    fn next_level(&mut self, index: usize);
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
) -> Estimate {
    let smax = params.levels.iter().map(|l| l.subsamp).max().unwrap_or(1);
    let prepared = images::prepare(
        volumes,
        dims,
        voxel,
        params.scale,
        params.regrid.then_some(smax),
    );
    let mut field: Option<Field> = None;
    let mut prev: Option<LevelSpec> = None;
    let mut cached: Option<(usize, f64, LevelImages)> = None;
    let mut mov = vec![[0.0; 6]; scans.len()];
    for (l, spec) in params.levels.iter().enumerate() {
        let s = spec.subsamp.max(1);
        let nt = dims.map(|n| (n / s).max(1));
        let vt = voxel.map(|v| v * s as f64);
        let k = knot_spacing(spec.warpres, vt);
        let mut f = match (field.take(), prev) {
            (Some(f), Some(p)) => {
                progress.next_level(l);
                let mut f = f;
                if p.subsamp != s {
                    let step = s as f64 / p.subsamp as f64;
                    f = f.regrid(nt, step, knot_spacing(p.warpres, vt));
                }
                // refitted when the resolution changes, even to the same knot spacing (observed:
                // the regularisation `-v` prints at the next level's start)
                if f.axes.map(|a| a.k) != k || p.warpres.to_bits() != spec.warpres.to_bits() {
                    f = f.respace(k);
                }
                f
            }
            _ => Field::zeros(std::array::from_fn(|a| Axis::new(nt[a], k[a]))),
        };
        let imgs = match &cached {
            Some((cs, cf, im)) if *cs == s && cf.to_bits() == spec.fwhm.to_bits() => im.clone(),
            _ => {
                let im = images::level_images(&prepared, s, spec.fwhm);
                cached = Some((s, spec.fwhm, im.clone()));
                im
            }
        };
        let mut level = Level::new(
            imgs,
            dims,
            voxel,
            s,
            f.axes.map(|a| a.k),
            scans,
            spec.lambda,
            params.ssqlambda,
            params.regmod,
            &mov,
            spec.estmov,
        );
        level.single = params.single;
        let mut rep = |lv: &Level, e: &Eval| progress.evaluated(lv, e);
        let theta = level.start(std::mem::take(&mut f.coef));
        let theta = match spec.minmet {
            Minimiser::LevenbergMarquardt => {
                minimise::levenberg_marquardt(&level, theta, spec.miter, &mut rep)
            }
            Minimiser::ScaledConjugateGradients => {
                minimise::scaled_conjugate_gradients(&level, theta, spec.miter, &mut rep)
            }
        };
        mov = level.movements(&theta);
        f.coef = theta[..level.n_coef()].to_vec();
        field = Some(f);
        prev = Some(*spec);
    }
    let last = prev.expect("at least one level");
    let mut f = field.expect("at least one level");
    if last.subsamp > 1 {
        let k = knot_spacing(last.warpres, voxel);
        f = f.regrid(dims, 1.0 / last.subsamp as f64, k);
        if f.axes.map(|a| a.k) != k {
            f = f.respace(k);
        }
    }
    Estimate {
        field: f,
        prepared,
        warpres_last: last.warpres,
        movements: mov,
        fwhm_out: if last.subsamp > 1 { 0.0 } else { last.fwhm },
    }
}

/// The predictions at full resolution (§6): the regridded volumes (smoothed as the last level's
/// when it was not subsampled, [`Estimate::fwhm_out`]) interpolated on the input's grid.
pub fn full_resolution(est: &Estimate, scans: &[Scan], regmod: RegMod) -> (Level, Eval) {
    let p = &est.prepared;
    let imgs = images::level_images(p, 1, est.fwhm_out);
    let level = Level::new(
        imgs,
        p.dims,
        p.voxel,
        1,
        est.field.axes.map(|a| a.k),
        scans,
        0.0,
        false,
        regmod,
        &est.movements,
        false,
    );
    let ev = level.eval(&est.field.coef);
    (level, ev)
}
