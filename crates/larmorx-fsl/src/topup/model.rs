// SPDX-License-Identifier: Apache-2.0
//! One level of the estimation (`specs/topup.md` §4.3, §4.6): the predicted undistorted volumes,
//! the mask, the cost, its gradient and the Gauss-Newton Hessian.
//!
//! Each volume `i` is displaced along its phase encoding by `h · t_i · p_i` (in the input's
//! voxels; `e_i` below converts to the level's voxels), sampled from its smoothed, subsampled
//! image and modulated by the Jacobian `J_i = 1 + e_i · ∇h`. The cost is the mean squared
//! difference of the volumes from their mean inside the mask plus `λ'` times the bending energy,
//! `λ' = λ · SSD` with `--ssqlambda=1` (observed: `-v`'s `Reg` is `λ · SSD · E`).

use rayon::prelude::*;

use super::images::{Grid, LevelImages, interp, sample_op_of, taps_both};
use super::movement::{Motion, free_parameters};
use super::pool::Pool;
use super::spline::{Axis, Op1, apply3, band_op, gram};
use super::stencil::{Hessian, Stencil};

/// One volume's acquisition: the phase-encoding vector (voxel axes; `pe[2]` is 0) and the
/// total read-out time (s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scan {
    pub pe: [f64; 3],
    pub t: f64,
}

/// The regulariser (§4.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegMod {
    Bending,
    Membrane,
}

/// The regulariser's terms: the derivative orders along x, y, z and their weights.
fn terms(m: RegMod) -> &'static [([usize; 3], f64)] {
    match m {
        RegMod::Bending => &[
            ([2, 0, 0], 1.0),
            ([0, 2, 0], 1.0),
            ([0, 0, 2], 1.0),
            ([1, 1, 0], 2.0),
            ([1, 0, 1], 2.0),
            ([0, 1, 1], 2.0),
        ],
        RegMod::Membrane => &[([1, 0, 0], 1.0), ([0, 1, 0], 1.0), ([0, 0, 1], 1.0)],
    }
}

/// The field's regulariser on a grid: `E(c) = cᵀ R c`, `R` a sum of products of 1D Gram
/// matrices.
#[derive(Clone, Debug)]
pub struct Regulariser {
    pub model: RegMod,
    /// `grams[a][d]`: axis `a`'s Gram matrix of derivative `d` (banded).
    grams: [[Vec<[f64; 7]>; 3]; 3],
    ops: [[Op1; 3]; 3],
    dims: [usize; 3],
}

impl Regulariser {
    pub fn new(model: RegMod, axes: [Axis; 3], voxel: [f64; 3]) -> Regulariser {
        let grams: [[Vec<[f64; 7]>; 3]; 3] =
            std::array::from_fn(|a| std::array::from_fn(|d| gram(axes[a], d, voxel[a])));
        let ops = std::array::from_fn(|a| std::array::from_fn(|d| band_op(&grams[a][d])));
        Regulariser {
            model,
            grams,
            ops,
            dims: axes.map(|a| a.ncoef()),
        }
    }

    /// `R c`: the terms in order, each `w · Z(Y(X c))` (the passes x, y, z) added to the sum;
    /// the terms' x passes computed once per derivative order.
    pub fn apply(&self, c: &[f64]) -> Vec<f64> {
        self.apply_in(c, &Pool::default())
    }

    /// [`Regulariser::apply`] with its arrays from `pool`.
    pub fn apply_in(&self, c: &[f64], pool: &Pool) -> Vec<f64> {
        let dims = self.dims;
        let mut out = pool.zeros(c.len());
        let mut xs: [Option<Vec<f64>>; 3] = [None, None, None];
        for (ds, _) in terms(self.model) {
            if xs[ds[0]].is_none() {
                xs[ds[0]] = Some(pool.apply_axis(&self.ops[0][ds[0]], c, dims, 0).0);
            }
        }
        for (ds, w) in terms(self.model) {
            let x = xs[ds[0]].as_ref().expect("computed above");
            let (y, _) = pool.apply_axis(&self.ops[1][ds[1]], x, dims, 1);
            self.ops[2][ds[2]].accumulate_z(&y, dims, &mut out, *w);
            pool.give(y);
        }
        for x in xs.into_iter().flatten() {
            pool.give(x);
        }
        out
    }

    /// `E = cᵀ R c` and `R c`.
    pub fn energy(&self, c: &[f64]) -> (f64, Vec<f64>) {
        self.energy_in(c, &Pool::default())
    }

    /// [`Regulariser::energy`] with its arrays from `pool`.
    pub fn energy_in(&self, c: &[f64], pool: &Pool) -> (f64, Vec<f64>) {
        let rc = self.apply_in(c, pool);
        let e = dot(c, &rc);
        (e, rc)
    }

    /// `scale · R` added to a stencil matrix.
    pub fn add_to(&self, h: &mut Stencil, scale: f64) {
        let [nx, ny, nz] = self.dims;
        h.data
            .par_chunks_mut(343 * nx * ny)
            .enumerate()
            .for_each(|(kz, slab)| {
                for ky in 0..ny {
                    for kx in 0..nx {
                        let row = &mut slab[(ky * nx + kx) * 343..(ky * nx + kx + 1) * 343];
                        for (ds, w) in terms(self.model) {
                            let gx = &self.grams[0][ds[0]][kx];
                            let gy = &self.grams[1][ds[1]][ky];
                            let gz = &self.grams[2][ds[2]][kz];
                            for dz in 0..7 {
                                if gz[dz] == 0.0 {
                                    continue;
                                }
                                for dy in 0..7 {
                                    let gzy = gz[dz] * gy[dy];
                                    if gzy == 0.0 {
                                        continue;
                                    }
                                    for dx in 0..7 {
                                        row[(dz * 7 + dy) * 7 + dx] += scale * w * gzy * gx[dx];
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = nz;
            });
    }
}

/// A dot product summed in fixed blocks, in order (independent of the thread count).
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    let partial: Vec<f64> = a
        .par_chunks(4096)
        .zip(b.par_chunks(4096))
        .map(|(x, y)| x.iter().zip(y).map(|(p, q)| p * q).sum::<f64>())
        .collect();
    partial.iter().sum()
}

/// How a volume is interpolated.
#[derive(Clone, Debug)]
enum Sampler {
    /// Displaced along one axis only: the spline contracted along the two others at the
    /// level's fixed positions (a line per voxel column; 4 taps per voxel).
    Line { axis: usize, lines: Grid },
    /// The general case: 64 taps per voxel.
    Full,
}

/// One level.
#[derive(Clone, Debug)]
pub struct Level {
    /// The level's grid (the input's divided by the subsampling) and voxel sizes.
    pub nt: [usize; 3],
    pub vt: [f64; 3],
    pub axes: [Axis; 3],
    basis: [[Op1; 2]; 3],
    basis_t: [[Op1; 2]; 3],
    images: LevelImages,
    samplers: Vec<Sampler>,
    /// Level voxel → image voxel.
    scale: [f64; 3],
    /// Per volume: the displacement (level voxels) per Hz along each axis.
    pub e: Vec<[f64; 3]>,
    /// The axes along which some volume is displaced.
    pe_axes: [bool; 3],
    /// The centre (mm) of the images interpolated: the movements rotate about it.
    centre: [f64; 3],
    /// Per volume: its movement (`None`: none estimated and none fixed).
    motions: Vec<Option<Motion>>,
    /// The movement parameters estimated, `(volume, parameter)`, after the coefficients.
    pub free: Vec<(usize, usize)>,
    /// Per volume: whether its movement is estimated.
    estimated: Vec<bool>,
    pub lambda: f64,
    pub ssq: bool,
    pub reg: Regulariser,
    /// The membrane energy per mm (FSL-117: FSL's Levenberg-Marquardt Hessian of it).
    reg_mm: Option<Regulariser>,
    /// FSL-117 reproduced: Levenberg-Marquardt's Hessian of the membrane energy per mm.
    pub membrane_hessian_mm: bool,
    /// The Levenberg-Marquardt Hessian in single precision (`--numprec=float`).
    pub single: bool,
    /// FSL-146 reproduced: each axis' Jacobian term of the gradient summed over the volumes
    /// displaced along that axis only.
    pub fsl146: bool,
    /// The large arrays kept for reuse by the level's evaluations.
    pool: Pool,
}

/// The predictions at one parameter vector (the coefficients, then [`Level::free`]).
#[derive(Clone, Debug)]
pub struct Eval {
    pub h: Vec<f64>,
    pub dh: [Option<Vec<f64>>; 3],
    /// Per volume: `u`, `J`, `f` and the derivative of `f` along each displaced axis (level
    /// voxels).
    pub u: Vec<Vec<f64>>,
    pub jac: Vec<Vec<f64>>,
    pub f: Vec<Vec<f64>>,
    pub fd: Vec<[Option<Vec<f64>>; 3]>,
    /// Per volume whose movement is estimated: `∂f / ∂P` along x, y, z (per mm).
    pub fm: Vec<Option<[Vec<f64>; 3]>>,
    /// Per volume: the movement in force.
    pub motions: Vec<Option<Motion>>,
    /// The voxels where every volume's interpolation stayed inside the grid (§4.3).
    pub mask: Vec<bool>,
    pub n_mask: usize,
    pub ssd: f64,
    pub energy: f64,
    pub rc: Vec<f64>,
    pub cost: f64,
}

impl Eval {
    /// `λ'`.
    pub fn lambda_eff(&self, l: &Level) -> f64 {
        if l.ssq { l.lambda * self.ssd } else { l.lambda }
    }

    /// The movement parameters of every volume.
    pub fn movements(&self) -> Vec<[f64; 6]> {
        self.motions
            .iter()
            .map(|m| m.map_or([0.0; 6], |m| m.p))
            .collect()
    }
}

/// One slice of the gradient's voxel weights (of `B`, then of `∂_a B` along the displaced axes),
/// written in place.
type GradientSlice<'a> = (&'a mut [f64], [Option<&'a mut [f64]>; 3]);

/// One slice of a volume's predictions in [`Level::eval`], written in place.
struct Slices<'a> {
    u: &'a mut [f64],
    j: &'a mut [f64],
    f: &'a mut [f64],
    fd: [Option<&'a mut [f64]>; 3],
    fm: Option<[&'a mut [f64]; 3]>,
    mask: &'a mut [bool],
}

impl<'a> Slices<'a> {
    /// The arrays cut into slices of `plane` values.
    fn split(
        plane: usize,
        u: &'a mut [f64],
        j: &'a mut [f64],
        f: &'a mut [f64],
        fd: &'a mut [Option<Vec<f64>>; 3],
        fm: &'a mut Option<[Vec<f64>; 3]>,
        mask: &'a mut [bool],
    ) -> Vec<Slices<'a>> {
        let mut uc = u.chunks_mut(plane);
        let mut jc = j.chunks_mut(plane);
        let mut fc = f.chunks_mut(plane);
        let mut mc = mask.chunks_mut(plane);
        let mut dc = fd
            .each_mut()
            .map(|o| o.as_mut().map(|v| v.chunks_mut(plane)));
        let mut mmc = fm
            .as_mut()
            .map(|a| a.each_mut().map(|v| v.chunks_mut(plane)));
        let mut out = Vec::new();
        while let (Some(u), Some(j), Some(f), Some(mask)) =
            (uc.next(), jc.next(), fc.next(), mc.next())
        {
            out.push(Slices {
                u,
                j,
                f,
                fd: dc.each_mut().map(|o| o.as_mut().and_then(Iterator::next)),
                fm: mmc.as_mut().map(|a| {
                    a.each_mut()
                        .map(|c| c.next().expect("as many slices as the others"))
                }),
                mask,
            });
        }
        out
    }
}

/// Whether voxel `p` lies strictly inside the grid `nt` along every axis volume `e` is not
/// displaced along (§4.3; observed, findings 2).
#[inline]
fn inside_static(p: [usize; 3], nt: [usize; 3], e: [f64; 3]) -> bool {
    (0..3).all(|a| e[a] != 0.0 || (p[a] > 0 && p[a] + 1 < nt[a]))
}

impl Level {
    /// The level: `images` the volumes subsampled by `s` and smoothed, `input_voxel` the input's
    /// voxel sizes, `field` the field's axes (its knot spacings and order), `movements` each
    /// volume's movement at the start and
    /// `estimate` whether they are estimated (§4.3).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        images: LevelImages,
        input_dims: [usize; 3],
        input_voxel: [f64; 3],
        s: usize,
        field: [Axis; 3],
        scans: &[Scan],
        lambda: f64,
        ssq: bool,
        regmod: RegMod,
        movements: &[[f64; 6]],
        estimate: bool,
    ) -> Level {
        let nt = input_dims.map(|n| (n / s).max(1));
        let vt = input_voxel.map(|v| v * s as f64);
        let axes: [Axis; 3] = std::array::from_fn(|a| field[a].like(nt[a], field[a].k));
        let basis: [[Op1; 2]; 3] = std::array::from_fn(|a| {
            let ax: Axis = axes[a];
            [ax.at_voxels(0), ax.at_voxels(1)]
        });
        let basis_t = std::array::from_fn(|a| [basis[a][0].transpose(), basis[a][1].transpose()]);
        let scale = std::array::from_fn(|a| vt[a] / images.voxel[a]);
        let e: Vec<[f64; 3]> = scans
            .iter()
            .map(|sc| std::array::from_fn(|a| sc.t * sc.pe[a] * input_voxel[a] / vt[a]))
            .collect();
        let mut pe_axes = [false; 3];
        for ev in &e {
            for a in 0..3 {
                if ev[a] != 0.0 {
                    pe_axes[a] = true;
                }
            }
        }
        let centre: [f64; 3] =
            std::array::from_fn(|a| (images.dims[a] as f64 - 1.0) * images.voxel[a] / 2.0);
        let free = if estimate {
            free_parameters(&scans.iter().map(|s| s.pe).collect::<Vec<_>>())
        } else {
            Vec::new()
        };
        let estimated: Vec<bool> = (0..scans.len())
            .map(|i| free.iter().any(|&(j, _)| j == i))
            .collect();
        let motions: Vec<Option<Motion>> = (0..scans.len())
            .map(|i| {
                let p = movements.get(i).copied().unwrap_or([0.0; 6]);
                (estimated[i] || p.iter().any(|&x| x != 0.0)).then(|| Motion::new(p, centre))
            })
            .collect();
        let samplers = e
            .iter()
            .zip(&images.coef)
            .map(|(ev, c)| {
                let moving: Vec<usize> = (0..3).filter(|&a| ev[a] != 0.0).collect();
                if moving.len() == 1 {
                    let axis = moving[0];
                    let ops: [Option<Op1>; 3] = std::array::from_fn(|a| {
                        (a != axis).then(|| {
                            let xs: Vec<f64> = (0..nt[a]).map(|j| j as f64 * scale[a]).collect();
                            sample_op_of(&xs, c.dims[a], false, c.linear)
                        })
                    });
                    let (data, dims) = apply3(
                        &c.data,
                        c.dims,
                        [ops[0].as_ref(), ops[1].as_ref(), ops[2].as_ref()],
                    );
                    Sampler::Line {
                        axis,
                        lines: Grid {
                            dims,
                            data,
                            linear: c.linear,
                        },
                    }
                } else {
                    Sampler::Full
                }
            })
            .collect();
        // the membrane energy's cost and gradient measure its derivatives per voxel, the bending
        // energy's per mm (FSL-117; observed: -v's Reg)
        let reg = match regmod {
            RegMod::Membrane => Regulariser::new(regmod, axes, [1.0; 3]),
            RegMod::Bending => Regulariser::new(regmod, axes, vt),
        };
        let reg_mm = (regmod == RegMod::Membrane).then(|| Regulariser::new(regmod, axes, vt));
        Level {
            nt,
            vt,
            axes,
            basis,
            basis_t,
            images,
            samplers,
            scale,
            e,
            pe_axes,
            centre,
            motions,
            free,
            estimated,
            lambda,
            ssq,
            reg,
            reg_mm,
            membrane_hessian_mm: false,
            single: false,
            fsl146: false,
            pool: Pool::default(),
        }
    }

    pub fn n_coef(&self) -> usize {
        self.axes.iter().map(Axis::ncoef).product()
    }

    /// The level's arrays kept for reuse.
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// The number of parameters: the coefficients and the free movement parameters.
    pub fn n_params(&self) -> usize {
        self.n_coef() + self.free.len()
    }

    pub fn coef_dims(&self) -> [usize; 3] {
        self.axes.map(|a| a.ncoef())
    }

    /// The parameter vector of the coefficients `c` and the level's starting movements.
    pub fn start(&self, c: Vec<f64>) -> Vec<f64> {
        let mut theta = c;
        for &(i, k) in &self.free {
            theta.push(self.motions[i].map_or(0.0, |m| m.p[k]));
        }
        theta
    }

    /// The movements of every volume at the parameter vector `theta`.
    pub fn movements(&self, theta: &[f64]) -> Vec<[f64; 6]> {
        let mut out: Vec<[f64; 6]> = self
            .motions
            .iter()
            .map(|m| m.map_or([0.0; 6], |m| m.p))
            .collect();
        let nc = self.n_coef();
        for (j, &(i, k)) in self.free.iter().enumerate() {
            out[i][k] = theta[nc + j];
        }
        out
    }

    /// The field (derivative along `deriv`, per level voxel) on the level's grid.
    pub fn field(&self, c: &[f64], deriv: Option<usize>) -> Vec<f64> {
        let ops: [&Op1; 3] = std::array::from_fn(|a| &self.basis[a][usize::from(deriv == Some(a))]);
        apply3(
            &c[..self.n_coef()],
            self.coef_dims(),
            [Some(ops[0]), Some(ops[1]), Some(ops[2])],
        )
        .0
    }

    /// The field and its derivatives along the displaced axes (per level voxel), as
    /// [`Level::field`] gives them, the passes they share computed once.
    fn fields(&self, c: &[f64]) -> (Vec<f64>, [Option<Vec<f64>>; 3]) {
        let c = &c[..self.n_coef()];
        let dims = self.coef_dims();
        let b = &self.basis;
        let pool = &self.pool;
        let (x0, d1) = pool.apply_axis(&b[0][0], c, dims, 0);
        let (y0, d2) = pool.apply_axis(&b[1][0], &x0, d1, 1);
        let h = pool.apply_axis(&b[2][0], &y0, d2, 2).0;
        let mut dh: [Option<Vec<f64>>; 3] = [None, None, None];
        if self.pe_axes[0] {
            let (x1, d1) = pool.apply_axis(&b[0][1], c, dims, 0);
            let (y, d2) = pool.apply_axis(&b[1][0], &x1, d1, 1);
            dh[0] = Some(pool.apply_axis(&b[2][0], &y, d2, 2).0);
            pool.give(x1);
            pool.give(y);
        }
        if self.pe_axes[1] {
            let (y, d2) = pool.apply_axis(&b[1][1], &x0, d1, 1);
            dh[1] = Some(pool.apply_axis(&b[2][0], &y, d2, 2).0);
            pool.give(y);
        }
        if self.pe_axes[2] {
            dh[2] = Some(pool.apply_axis(&b[2][1], &y0, d2, 2).0);
        }
        pool.give(x0);
        pool.give(y0);
        (h, dh)
    }

    /// `Σ_x w(x) φ(x)` for each coefficient, `φ` the basis (derivative along `deriv`).
    fn project(&self, w: &[f64], deriv: Option<usize>) -> Vec<f64> {
        let ops: [&Op1; 3] =
            std::array::from_fn(|a| &self.basis_t[a][usize::from(deriv == Some(a))]);
        // apply3's passes (x, y, z), the intermediate arrays from the pool
        let pool = &self.pool;
        let (x, d1) = pool.apply_axis(ops[0], w, self.nt, 0);
        let (y, d2) = pool.apply_axis(ops[1], &x, d1, 1);
        let out = pool.apply_axis(ops[2], &y, d2, 2).0;
        pool.give(x);
        pool.give(y);
        out
    }

    /// Gives an evaluation's arrays back for reuse by the next ones.
    pub fn recycle(&self, e: Eval) {
        let pool = &self.pool;
        pool.give(e.h);
        for v in e.dh.into_iter().flatten() {
            pool.give(v);
        }
        for v in e.u.into_iter().chain(e.jac).chain(e.f) {
            pool.give(v);
        }
        for fd in e.fd {
            for v in fd.into_iter().flatten() {
                pool.give(v);
            }
        }
        for fm in e.fm.into_iter().flatten() {
            for v in fm {
                pool.give(v);
            }
        }
        pool.give(e.rc);
    }

    /// The position (mm) of level voxel `v`.
    #[inline]
    fn position(&self, v: usize) -> [f64; 3] {
        let [nx, ny, _] = self.nt;
        let p = [v % nx, (v / nx) % ny, v / (nx * ny)];
        std::array::from_fn(|a| p[a] as f64 * self.vt[a])
    }

    /// Evaluates the cost at `theta` (the coefficients, then the free movement parameters).
    pub fn eval(&self, theta: &[f64]) -> Eval {
        let nc = self.n_coef();
        let c = &theta[..nc];
        let mov = self.movements(theta);
        let motions: Vec<Option<Motion>> = self
            .motions
            .iter()
            .zip(&mov)
            .map(|(m, p)| m.map(|_| Motion::new(*p, self.centre)))
            .collect();
        let (h, dh) = self.fields(c);
        let [nx, ny, _] = self.nt;
        let nvox = h.len();
        let m = self.e.len();
        let mut u = Vec::with_capacity(m);
        let mut jac = Vec::with_capacity(m);
        let mut fv = Vec::with_capacity(m);
        let mut fdv = Vec::with_capacity(m);
        let mut fmv = Vec::with_capacity(m);
        let mut mask = vec![true; nvox];
        let plane = nx * ny;
        for (i, ev) in self.e.iter().enumerate() {
            // every value of these is written below, slice by slice
            let pool = &self.pool;
            let mut ui = pool.take(nvox);
            let mut ji = pool.take(nvox);
            let mut fi = pool.take(nvox);
            let mut fdi: [Option<Vec<f64>>; 3] =
                std::array::from_fn(|a| (ev[a] != 0.0).then(|| pool.take(nvox)));
            let est = self.estimated[i];
            let mut fmi: Option<[Vec<f64>; 3]> =
                est.then(|| std::array::from_fn(|_| pool.take(nvox)));
            let motion = motions[i].filter(|m| !m.is_identity());
            let want: [bool; 3] = std::array::from_fn(|a| est || ev[a] != 0.0);
            let e_mm: [f64; 3] = std::array::from_fn(|a| ev[a] * self.vt[a]);
            let line = match (&self.samplers[i], est, motion) {
                (Sampler::Line { axis, lines }, false, None) => Some((*axis, lines)),
                _ => None,
            };
            // displaced along more than one axis (an oblique phase encoding): no axis wraps; a
            // sample outside the level image along a displaced axis marks the voxel invalid
            // (observed: first_step.py, FSL's debug masks; findings 32)
            let oblique = ev.iter().filter(|x| **x != 0.0).count() > 1;
            let n_img = self.images.dims;
            let on_grid = |p: [f64; 3]| {
                !oblique
                    || (0..3)
                        .all(|a| ev[a] == 0.0 || (p[a] >= 0.0 && p[a] <= (n_img[a] - 1) as f64))
            };
            // per voxel (each value independent), slice by slice, written in place
            let slices = Slices::split(
                plane, &mut ui, &mut ji, &mut fi, &mut fdi, &mut fmi, &mut mask,
            );
            // the Jacobian's terms, in axis order
            let jterms: Vec<(f64, &[f64])> = (0..3)
                .filter_map(|b| dh[b].as_deref().map(|d| (ev[b], d)))
                .collect();
            slices.into_par_iter().enumerate().for_each(|(z, mut o)| {
                if let Some((a, lines)) = line {
                    // displaced along axis `a` alone: the spline's line through the voxel (the
                    // general branch below, with its invariants taken out of the loop)
                    let n = lines.dims[a];
                    let stride = [1, lines.dims[0], lines.dims[0] * lines.dims[1]][a];
                    let (sa, ea) = (self.scale[a], ev[a]);
                    let ny = plane / nx.max(1);
                    let mut fd_a = o.fd[a].take();
                    for y in 0..ny {
                        for x in 0..nx {
                            let k = y * nx + x;
                            let v = z * plane + k;
                            let hv = h[v];
                            let mut jv = 1.0;
                            for (e, d) in &jterms {
                                jv += e * d[v];
                            }
                            let p = [x, y, z][a] as f64 + hv * ea;
                            let (idx, w, dw) = taps_both(p * sa, n, lines.linear);
                            let mut base_p = [x, y, z];
                            base_p[a] = 0;
                            let base =
                                (base_p[2] * lines.dims[1] + base_p[1]) * lines.dims[0] + base_p[0];
                            let mut f = 0.0;
                            let mut df = 0.0;
                            for j in 0..4 {
                                let val = lines.data[base + idx[j] * stride];
                                f += w[j] * val;
                                df += dw[j] * val;
                            }
                            o.u[k] = jv * f;
                            o.j[k] = jv;
                            o.f[k] = f;
                            if let Some(arr) = &mut fd_a {
                                arr[k] = df * sa;
                            }
                            if !inside_static([x, y, z], self.nt, *ev) {
                                o.mask[k] = false;
                            }
                        }
                    }
                    return;
                }
                for (k, (y, x)) in (0..plane / nx.max(1))
                    .flat_map(|y| (0..nx).map(move |x| (y, x)))
                    .enumerate()
                {
                    let v = z * plane + k;
                    let hv = h[v];
                    let mut jv = 1.0;
                    for a in 0..3 {
                        if let Some(d) = &dh[a] {
                            jv += ev[a] * d[v];
                        }
                    }
                    let (f, d, dm, valid) = if let Some((a, lines)) = line {
                        let p = [x as f64, y as f64, z as f64][a] + hv * ev[a];
                        let pos = p * self.scale[a];
                        let n = lines.dims[a];
                        let (idx, w, dw) = taps_both(pos, n, lines.linear);
                        let stride = [1, lines.dims[0], lines.dims[0] * lines.dims[1]][a];
                        let mut base_p = [x, y, z];
                        base_p[a] = 0;
                        let base =
                            (base_p[2] * lines.dims[1] + base_p[1]) * lines.dims[0] + base_p[0];
                        let mut f = 0.0;
                        let mut df = 0.0;
                        for j in 0..4 {
                            let val = lines.data[base + idx[j] * stride];
                            f += w[j] * val;
                            df += dw[j] * val;
                        }
                        let mut d = [0.0; 3];
                        d[a] = df * self.scale[a];
                        (f, d, [0.0; 3], inside_static([x, y, z], self.nt, *ev))
                    } else {
                        let (p, valid) = match &motion {
                            None => {
                                let p = [
                                    (x as f64 + hv * ev[0]) * self.scale[0],
                                    (y as f64 + hv * ev[1]) * self.scale[1],
                                    (z as f64 + hv * ev[2]) * self.scale[2],
                                ];
                                (p, inside_static([x, y, z], self.nt, *ev) && on_grid(p))
                            }
                            Some(mo) => {
                                let q = mo.apply(self.position(v));
                                let pm: [f64; 3] = std::array::from_fn(|a| q[a] + hv * e_mm[a]);
                                let valid = (0..3).all(|a| {
                                    ev[a] != 0.0 || {
                                        let r = pm[a] / self.vt[a];
                                        r > 0.0 && r < (self.nt[a] - 1) as f64
                                    }
                                });
                                let p = std::array::from_fn(|a| pm[a] / self.images.voxel[a]);
                                (p, valid && on_grid(p))
                            }
                        };
                        let (f, d) = interp(&self.images.coef[i], p, want);
                        (
                            f,
                            std::array::from_fn(|a| d[a] * self.scale[a]),
                            std::array::from_fn(|a| d[a] / self.images.voxel[a]),
                            valid,
                        )
                    };
                    o.u[k] = jv * f;
                    o.j[k] = jv;
                    o.f[k] = f;
                    for a in 0..3 {
                        if let Some(arr) = &mut o.fd[a] {
                            arr[k] = d[a];
                        }
                    }
                    if let Some(fm) = &mut o.fm {
                        for a in 0..3 {
                            fm[a][k] = dm[a];
                        }
                    }
                    if !valid {
                        o.mask[k] = false;
                    }
                }
            });
            u.push(ui);
            jac.push(ji);
            fv.push(fi);
            fdv.push(fdi);
            fmv.push(fmi);
        }
        let n_mask = mask.iter().filter(|&&b| b).count();
        // SSD in storage order, per slice then in order
        let plane = nx * ny;
        let partial: Vec<f64> = (0..self.nt[2])
            .into_par_iter()
            .map(|z| {
                let mut s = 0.0;
                for v in z * plane..(z + 1) * plane {
                    if !mask[v] {
                        continue;
                    }
                    let mean = u.iter().map(|ui| ui[v]).sum::<f64>() / m as f64;
                    for ui in &u {
                        let r = mean - ui[v];
                        s += r * r;
                    }
                }
                s
            })
            .collect();
        let denom = (n_mask.max(1) * (m.max(2) - 1)) as f64;
        let ssd = partial.iter().sum::<f64>() / denom;
        let (energy, rc) = if self.lambda != 0.0 {
            self.reg.energy_in(c, &self.pool)
        } else {
            (0.0, self.pool.zeros(c.len()))
        };
        let lam = if self.ssq {
            self.lambda * ssd
        } else {
            self.lambda
        };
        Eval {
            h,
            dh,
            u,
            jac,
            f: fv,
            fd: fdv,
            fm: fmv,
            motions,
            mask,
            n_mask,
            ssd,
            energy,
            rc,
            cost: ssd + lam * energy,
        }
    }

    /// The per-volume weights of the basis functions in `∂u_i/∂c`: `α_i` (of `B`) and `β_ia` (of
    /// `∂_a B`), at voxel `v`.
    fn alpha_beta(&self, ev: &Eval, i: usize, v: usize) -> (f64, [f64; 3]) {
        let e = self.e[i];
        let mut alpha = 0.0;
        let mut beta = [0.0; 3];
        for a in 0..3 {
            if e[a] != 0.0 {
                if let Some(fd) = &ev.fd[i][a] {
                    alpha += e[a] * fd[v];
                }
                beta[a] = e[a] * ev.f[i][v];
            }
        }
        (ev.jac[i][v] * alpha, beta)
    }

    /// `∂u_i / ∂p` of each free movement parameter, masked (0 outside the mask).
    fn movement_derivatives(&self, ev: &Eval) -> Vec<Vec<f64>> {
        self.free
            .iter()
            .map(|&(i, k)| {
                let (Some(fm), Some(mo)) = (&ev.fm[i], ev.motions[i]) else {
                    return vec![0.0; ev.h.len()];
                };
                (0..ev.h.len())
                    .into_par_iter()
                    .map(|v| {
                        if !ev.mask[v] {
                            return 0.0;
                        }
                        let dp = mo.derivative(self.position(v), k);
                        ev.jac[i][v] * (fm[0][v] * dp[0] + fm[1][v] * dp[1] + fm[2][v] * dp[2])
                    })
                    .collect()
            })
            .collect()
    }

    /// A sum over the voxels in slices, in a fixed order.
    fn masked_sum(&self, f: impl Fn(usize) -> f64 + Sync) -> f64 {
        let plane = self.nt[0] * self.nt[1];
        let partial: Vec<f64> = (0..self.nt[2])
            .into_par_iter()
            .map(|z| (z * plane..(z + 1) * plane).map(&f).sum::<f64>())
            .collect();
        partial.iter().sum()
    }

    /// The gradient of the cost at the evaluated point (the coefficients, then the free
    /// movement parameters).
    pub fn gradient(&self, ev: &Eval, c: &[f64]) -> Vec<f64> {
        let nvox = ev.h.len();
        let m = self.e.len();
        // FSL-146 reproduced on the axes along which some volumes are displaced and others not
        // (elsewhere its term is 0 but for rounding, which would change the default's bits)
        let fsl146: [bool; 3] = std::array::from_fn(|a| {
            self.fsl146 && self.e.iter().any(|e| e[a] != 0.0) && !self.e.iter().all(|e| e[a] != 0.0)
        });
        let fsl146_any = fsl146.iter().any(|&b| b);
        let mut wa = self.pool.zeros(nvox);
        let mut wb: [Vec<f64>; 3] = std::array::from_fn(|a| {
            if self.pe_axes[a] {
                self.pool.zeros(nvox)
            } else {
                Vec::new()
            }
        });
        {
            // per voxel, slice by slice, written in place
            let plane = self.nt[0] * self.nt[1];
            let mut bc = wb
                .each_mut()
                .map(|w| (!w.is_empty()).then(|| w.chunks_mut(plane)));
            let slices: Vec<GradientSlice<'_>> = wa
                .chunks_mut(plane)
                .map(|a| {
                    (
                        a,
                        bc.each_mut().map(|o| o.as_mut().and_then(Iterator::next)),
                    )
                })
                .collect();
            slices
                .into_par_iter()
                .enumerate()
                .for_each(|(z, (wa, mut wb))| {
                    for (k, out) in wa.iter_mut().enumerate() {
                        let v = z * plane + k;
                        if !ev.mask[v] {
                            continue;
                        }
                        let mean = ev.u.iter().map(|ui| ui[v]).sum::<f64>() / m as f64;
                        let mut sa = 0.0;
                        let mut sb = [0.0; 3];
                        // FSL-146 reproduced: per axis, the residuals and the Jacobian terms
                        // of the volumes displaced along it
                        let mut rs = [0.0; 3];
                        let mut bs = [0.0; 3];
                        for i in 0..m {
                            let r = mean - ev.u[i][v];
                            let (alpha, beta) = self.alpha_beta(ev, i, v);
                            sa += r * alpha;
                            for a in 0..3 {
                                sb[a] += r * beta[a];
                            }
                            if fsl146_any {
                                for a in 0..3 {
                                    if fsl146[a] && self.e[i][a] != 0.0 {
                                        rs[a] += r;
                                        bs[a] += beta[a];
                                    }
                                }
                            }
                        }
                        if fsl146_any {
                            // FSL's sum over the displaced volumes of (ū - u_i)(∂u_i - ∂ū)
                            // along a: the volumes not displaced along a leave out their
                            // (ū - u_j)(-∂ū) (observed: first_step.py)
                            for a in 0..3 {
                                if fsl146[a] {
                                    sb[a] -= bs[a] / m as f64 * rs[a];
                                }
                            }
                        }
                        *out = sa;
                        for a in 0..3 {
                            if let Some(w) = &mut wb[a] {
                                w[k] = sb[a];
                            }
                        }
                    }
                });
        }
        let factor = -2.0 / (ev.n_mask.max(1) * (m.max(2) - 1)) as f64;
        let mut g = self.project(&wa, None);
        for a in 0..3 {
            if self.pe_axes[a] {
                let p = self.project(&wb[a], Some(a));
                for (x, y) in g.iter_mut().zip(&p) {
                    *x += y;
                }
                self.pool.give(p);
            }
        }
        self.pool.give(wa);
        for w in wb {
            self.pool.give(w);
        }
        let lam = ev.lambda_eff(self);
        for (k, x) in g.iter_mut().enumerate() {
            *x *= factor;
            if self.lambda != 0.0 {
                *x += 2.0 * lam * ev.rc[k];
            }
        }
        if !self.free.is_empty() {
            let d = self.movement_derivatives(ev);
            for (p, &(i, _)) in self.free.iter().enumerate() {
                let s = self.masked_sum(|v| {
                    if d[p][v] == 0.0 {
                        return 0.0;
                    }
                    let mean = ev.u.iter().map(|ui| ui[v]).sum::<f64>() / m as f64;
                    (mean - ev.u[i][v]) * d[p][v]
                });
                g.push(factor * s);
            }
        }
        let _ = c;
        g
    }

    /// The Gauss-Newton Hessian's voxel weights for `--debug=2` (findings 6): for the basis
    /// types `B`, `∂_x B`, `∂_y B`, the upper triangle (00, 01, 02, 11, 12, 22) of `Σ_i (c_ip -
    /// c̄_p)(c_iq - c̄_q)`, unmasked.
    pub fn voxel_weights(&self, ev: &Eval) -> [Vec<f64>; 6] {
        let nvox = ev.h.len();
        let m = self.e.len();
        let rows: Vec<[f64; 6]> = (0..nvox)
            .into_par_iter()
            .map(|v| {
                let comps: Vec<[f64; 3]> = (0..m)
                    .map(|i| {
                        let (a, b) = self.alpha_beta(ev, i, v);
                        [a, b[0], b[1]]
                    })
                    .collect();
                let mean: [f64; 3] =
                    std::array::from_fn(|j| comps.iter().map(|c| c[j]).sum::<f64>() / m as f64);
                let mut w = [0.0; 6];
                for c in &comps {
                    let d: [f64; 3] = std::array::from_fn(|j| c[j] - mean[j]);
                    let pairs = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
                    for (k, (p, q)) in pairs.iter().enumerate() {
                        w[k] += d[*p] * d[*q];
                    }
                }
                w
            })
            .collect();
        std::array::from_fn(|k| rows.iter().map(|r| r[k]).collect())
    }

    /// The Gauss-Newton Hessian of the cost at the evaluated point, with the regulariser's.
    pub fn hessian(&self, ev: &Eval) -> Hessian {
        let nvox = ev.h.len();
        let m = self.e.len();
        // the basis types present: 0 = B, 1 + a = ∂_a B
        let types: Vec<usize> = std::iter::once(0)
            .chain((0..3).filter(|&a| self.pe_axes[a]).map(|a| a + 1))
            .collect();
        let nt = types.len();
        // per voxel, the weights W_pq = Σ_i (c_ip - c̄_p)(c_iq - c̄_q), masked
        let nn = nt * nt;
        let mut weights = self.pool.zeros(nvox * nn);
        let plane = self.nt[0] * self.nt[1];
        weights
            .par_chunks_mut(plane * nn)
            .enumerate()
            .for_each(|(z, slab)| {
                let mut comps = vec![0.0; m * nt];
                let mut mean = vec![0.0; nt];
                for (k, w) in slab.chunks_mut(nn).enumerate() {
                    let v = z * plane + k;
                    if !ev.mask[v] {
                        continue;
                    }
                    for i in 0..m {
                        let (alpha, beta) = self.alpha_beta(ev, i, v);
                        for (j, &t) in types.iter().enumerate() {
                            comps[i * nt + j] = if t == 0 { alpha } else { beta[t - 1] };
                        }
                    }
                    for (j, mj) in mean.iter_mut().enumerate() {
                        *mj = (0..m).map(|i| comps[i * nt + j]).sum::<f64>() / m as f64;
                    }
                    for c in comps.chunks(nt) {
                        for p in 0..nt {
                            for q in 0..nt {
                                w[p * nt + q] += (c[p] - mean[p]) * (c[q] - mean[q]);
                            }
                        }
                    }
                }
            });
        let dims = self.coef_dims();
        let mut h = Stencil::zeros_in(dims, &self.pool);
        let factor = 2.0 / (ev.n_mask.max(1) * (m.max(2) - 1)) as f64;
        for (p, &tp) in types.iter().enumerate() {
            for (q, &tq) in types.iter().enumerate() {
                let w: Vec<f64> = weights.chunks(nn).map(|x| x[p * nt + q] * factor).collect();
                if w.iter().all(|&x| x == 0.0) {
                    continue;
                }
                let dp = |a: usize| usize::from(tp == a + 1);
                let dq = |a: usize| usize::from(tq == a + 1);
                h.add_weighted_in(
                    &w,
                    self.nt,
                    std::array::from_fn(|a| &self.basis[a][dp(a)]),
                    std::array::from_fn(|a| &self.basis[a][dq(a)]),
                    &self.pool,
                );
                self.pool.give(w);
            }
        }
        self.pool.give(weights);
        if self.lambda != 0.0 {
            let reg = match (&self.reg_mm, self.membrane_hessian_mm) {
                (Some(r), true) => r,
                _ => &self.reg,
            };
            reg.add_to(&mut h, 2.0 * ev.lambda_eff(self));
        }
        let nm = self.free.len();
        let mut cm = Vec::with_capacity(nm);
        let mut mm = vec![0.0; nm * nm];
        if nm > 0 {
            let d = self.movement_derivatives(ev);
            // the coefficients' block: Σ_v (∂u_j/∂c - mean_i ∂u_i/∂c) ∂u_j/∂p
            for (p, &(j, _)) in self.free.iter().enumerate() {
                let rows: Vec<(f64, [f64; 3])> = (0..nvox)
                    .into_par_iter()
                    .map(|v| {
                        if d[p][v] == 0.0 {
                            return (0.0, [0.0; 3]);
                        }
                        let mut ma = 0.0;
                        let mut mb = [0.0; 3];
                        for i in 0..m {
                            let (a, b) = self.alpha_beta(ev, i, v);
                            ma += a;
                            for x in 0..3 {
                                mb[x] += b[x];
                            }
                        }
                        let (aj, bj) = self.alpha_beta(ev, j, v);
                        let dv = d[p][v] * factor;
                        (
                            (aj - ma / m as f64) * dv,
                            std::array::from_fn(|x| (bj[x] - mb[x] / m as f64) * dv),
                        )
                    })
                    .collect();
                let wa: Vec<f64> = rows.iter().map(|r| r.0).collect();
                let mut col = self.project(&wa, None);
                for a in 0..3 {
                    if self.pe_axes[a] {
                        let wb: Vec<f64> = rows.iter().map(|r| r.1[a]).collect();
                        for (x, y) in col.iter_mut().zip(&self.project(&wb, Some(a))) {
                            *x += y;
                        }
                    }
                }
                cm.push(col);
            }
            // the movements' block: Σ_v ∂u_j/∂p ∂u_k/∂q (δ_jk - 1/m)
            for (p, &(j, _)) in self.free.iter().enumerate() {
                for (q, &(k, _)) in self.free.iter().enumerate().skip(p) {
                    let w = if j == k { 1.0 } else { 0.0 } - 1.0 / m as f64;
                    let s = self.masked_sum(|v| d[p][v] * d[q][v]);
                    mm[p * nm + q] = factor * w * s;
                    mm[q * nm + p] = factor * w * s;
                }
            }
        }
        Hessian { cc: h, cm, mm }
    }
}

#[cfg(test)]
mod tests {
    use super::super::images::{level_images, prepare};
    use super::*;

    /// Two smooth volumes (plain arithmetic) encoded along y and x, and a small random field.
    fn level(fsl146: bool) -> (Level, Vec<f64>) {
        let dims = [12, 10, 4];
        let voxel = [2.0, 2.0, 3.0];
        let vol = |sx: f64, sy: f64| -> Vec<f32> {
            let mut v = Vec::new();
            for z in 0..dims[2] {
                for y in 0..dims[1] {
                    for x in 0..dims[0] {
                        let (u, w) = (x as f64 - 5.5 + sx, y as f64 - 4.5 + sy);
                        let r2 = u * u / 20.0 + w * w / 12.0 + (z as f64 - 1.5).powi(2) / 4.0;
                        v.push((100.0 / (1.0 + r2) + 3.0 * u) as f32);
                    }
                }
            }
            v
        };
        let volumes = vec![vol(0.0, 0.6), vol(0.5, 0.0)];
        let scans = [
            Scan {
                pe: [0.0, 1.0, 0.0],
                t: 0.05,
            },
            Scan {
                pe: [1.0, 0.0, 0.0],
                t: 0.05,
            },
        ];
        let p = prepare(&volumes, dims, voxel, false, None, false);
        let images = level_images(&p, 1, 2.0, false);
        let field: [Axis; 3] = std::array::from_fn(|a| Axis::of_order(dims[a], 3, 3));
        let mut l = Level::new(
            images,
            dims,
            voxel,
            1,
            field,
            &scans,
            0.0,
            false,
            RegMod::Bending,
            &[],
            false,
        );
        l.fsl146 = fsl146;
        let mut s = 7u64;
        let c = (0..l.n_coef())
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 33) as f64 / (1u64 << 31) as f64 - 0.5) * 4.0
            })
            .collect();
        (l, c)
    }

    #[test]
    fn the_gradient_is_the_costs_and_fsl146_is_not() {
        let (l, c) = level(false);
        let ev = l.eval(&c);
        let g = l.gradient(&ev, &c);
        let (lr, _) = level(true);
        let gr = lr.gradient(&lr.eval(&c), &c);
        let mut worst = 0.0f64;
        let mut worst_fsl = 0.0f64;
        let scale = g.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        for k in (0..c.len()).step_by(7) {
            let eps = 1e-4;
            let (mut a, mut b) = (c.clone(), c.clone());
            a[k] += eps;
            b[k] -= eps;
            let (ea, eb) = (l.eval(&a), l.eval(&b));
            assert_eq!(ea.n_mask, eb.n_mask);
            let fd = (ea.cost - eb.cost) / (2.0 * eps);
            worst = worst.max((fd - g[k]).abs());
            worst_fsl = worst_fsl.max((fd - gr[k]).abs());
        }
        // the default's gradient is the cost's (central differences, to their own error)
        assert!(worst < 1e-5 * scale, "default: {worst} of {scale}");
        // FSL-146's is not: volumes encoded along different axes
        assert!(worst_fsl > 1e-2 * scale, "FSL-146: {worst_fsl} of {scale}");
    }
}
