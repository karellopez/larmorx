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

use super::images::{Grid, LevelImages, interp, sample_op, taps};
use super::spline::{Axis, Op1, apply3, band_op, gram};
use super::stencil::Stencil;

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

    /// `R c`.
    pub fn apply(&self, c: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; c.len()];
        for (ds, w) in terms(self.model) {
            let (r, _) = apply3(
                c,
                self.dims,
                [
                    Some(&self.ops[0][ds[0]]),
                    Some(&self.ops[1][ds[1]]),
                    Some(&self.ops[2][ds[2]]),
                ],
            );
            for (o, v) in out.iter_mut().zip(&r) {
                *o += w * v;
            }
        }
        out
    }

    /// `E = cᵀ R c` and `R c`.
    pub fn energy(&self, c: &[f64]) -> (f64, Vec<f64>) {
        let rc = self.apply(c);
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
    mask: Vec<bool>,
    pub n_mask: usize,
    pub lambda: f64,
    pub ssq: bool,
    pub reg: Regulariser,
}

/// The predictions at one coefficient vector.
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
}

impl Level {
    /// The level: `images` the volumes subsampled by `s` and smoothed, `input_voxel` the input's
    /// voxel sizes, `k` the knot spacings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        images: LevelImages,
        input_dims: [usize; 3],
        input_voxel: [f64; 3],
        s: usize,
        k: [usize; 3],
        scans: &[Scan],
        lambda: f64,
        ssq: bool,
        regmod: RegMod,
    ) -> Level {
        let nt = input_dims.map(|n| (n / s).max(1));
        let vt = input_voxel.map(|v| v * s as f64);
        let axes = std::array::from_fn(|a| Axis::new(nt[a], k[a]));
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
        // the mask: inside the grid (strictly) along each volume's other axes
        let mut mask = vec![true; nt[0] * nt[1] * nt[2]];
        for ev in &e {
            for z in 0..nt[2] {
                for y in 0..nt[1] {
                    for x in 0..nt[0] {
                        let p = [x, y, z];
                        let inside = (0..3).all(|a| ev[a] != 0.0 || (p[a] > 0 && p[a] + 1 < nt[a]));
                        if !inside {
                            mask[(z * nt[1] + y) * nt[0] + x] = false;
                        }
                    }
                }
            }
        }
        let n_mask = mask.iter().filter(|&&m| m).count();
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
                            sample_op(&xs, c.dims[a], false)
                        })
                    });
                    let (data, dims) = apply3(
                        &c.data,
                        c.dims,
                        [ops[0].as_ref(), ops[1].as_ref(), ops[2].as_ref()],
                    );
                    Sampler::Line {
                        axis,
                        lines: Grid { dims, data },
                    }
                } else {
                    Sampler::Full
                }
            })
            .collect();
        let reg = Regulariser::new(regmod, axes, vt);
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
            mask,
            n_mask,
            lambda,
            ssq,
            reg,
        }
    }

    pub fn n_coef(&self) -> usize {
        self.axes.iter().map(Axis::ncoef).product()
    }

    pub fn coef_dims(&self) -> [usize; 3] {
        self.axes.map(|a| a.ncoef())
    }

    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    /// The field (derivative along `deriv`, per level voxel) on the level's grid.
    pub fn field(&self, c: &[f64], deriv: Option<usize>) -> Vec<f64> {
        let ops: [&Op1; 3] = std::array::from_fn(|a| &self.basis[a][usize::from(deriv == Some(a))]);
        apply3(
            c,
            self.coef_dims(),
            [Some(ops[0]), Some(ops[1]), Some(ops[2])],
        )
        .0
    }

    /// `Σ_x w(x) φ(x)` for each coefficient, `φ` the basis (derivative along `deriv`).
    fn project(&self, w: &[f64], deriv: Option<usize>) -> Vec<f64> {
        let ops: [&Op1; 3] =
            std::array::from_fn(|a| &self.basis_t[a][usize::from(deriv == Some(a))]);
        apply3(w, self.nt, [Some(ops[0]), Some(ops[1]), Some(ops[2])]).0
    }

    /// Evaluates the cost at `c`.
    pub fn eval(&self, c: &[f64]) -> Eval {
        let h = self.field(c, None);
        let dh: [Option<Vec<f64>>; 3] =
            std::array::from_fn(|a| self.pe_axes[a].then(|| self.field(c, Some(a))));
        let [nx, ny, _] = self.nt;
        let nvox = h.len();
        let m = self.e.len();
        let mut u = Vec::with_capacity(m);
        let mut jac = Vec::with_capacity(m);
        let mut fv = Vec::with_capacity(m);
        let mut fdv = Vec::with_capacity(m);
        for (i, ev) in self.e.iter().enumerate() {
            let mut ui = vec![0.0; nvox];
            let mut ji = vec![0.0; nvox];
            let mut fi = vec![0.0; nvox];
            let mut fdi: [Option<Vec<f64>>; 3] =
                std::array::from_fn(|a| (ev[a] != 0.0).then(|| vec![0.0; nvox]));
            let want = std::array::from_fn(|a| ev[a] != 0.0);
            // per voxel, in parallel by slices (each value independent)
            let results: Vec<(f64, f64, f64, [f64; 3])> = (0..nvox)
                .into_par_iter()
                .map(|v| {
                    let x = v % nx;
                    let y = (v / nx) % ny;
                    let z = v / (nx * ny);
                    let hv = h[v];
                    let mut jv = 1.0;
                    for a in 0..3 {
                        if let Some(d) = &dh[a] {
                            jv += ev[a] * d[v];
                        }
                    }
                    let (f, d) = match &self.samplers[i] {
                        Sampler::Line { axis, lines } => {
                            let a = *axis;
                            let p = [x as f64, y as f64, z as f64][a] + hv * ev[a];
                            let pos = p * self.scale[a];
                            let n = lines.dims[a];
                            let (idx, w) = taps(pos, n, false);
                            let (_, dw) = taps(pos, n, true);
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
                            (f, d)
                        }
                        Sampler::Full => {
                            let p = [
                                (x as f64 + hv * ev[0]) * self.scale[0],
                                (y as f64 + hv * ev[1]) * self.scale[1],
                                (z as f64 + hv * ev[2]) * self.scale[2],
                            ];
                            let (f, d) = interp(&self.images.coef[i], p, want);
                            (
                                f,
                                [
                                    d[0] * self.scale[0],
                                    d[1] * self.scale[1],
                                    d[2] * self.scale[2],
                                ],
                            )
                        }
                    };
                    (jv * f, jv, f, d)
                })
                .collect();
            for (v, (uv, jv, f, d)) in results.into_iter().enumerate() {
                ui[v] = uv;
                ji[v] = jv;
                fi[v] = f;
                for a in 0..3 {
                    if let Some(arr) = &mut fdi[a] {
                        arr[v] = d[a];
                    }
                }
            }
            u.push(ui);
            jac.push(ji);
            fv.push(fi);
            fdv.push(fdi);
        }
        // SSD in storage order, per slice then in order
        let plane = nx * ny;
        let partial: Vec<f64> = (0..self.nt[2])
            .into_par_iter()
            .map(|z| {
                let mut s = 0.0;
                for v in z * plane..(z + 1) * plane {
                    if !self.mask[v] {
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
        let denom = (self.n_mask.max(1) * (m.max(2) - 1)) as f64;
        let ssd = partial.iter().sum::<f64>() / denom;
        let (energy, rc) = if self.lambda != 0.0 {
            self.reg.energy(c)
        } else {
            (0.0, vec![0.0; c.len()])
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

    /// The gradient of the cost at the evaluated point.
    pub fn gradient(&self, ev: &Eval, c: &[f64]) -> Vec<f64> {
        let nvox = ev.h.len();
        let m = self.e.len();
        let mut wa = vec![0.0; nvox];
        let mut wb: [Vec<f64>; 3] = std::array::from_fn(|a| {
            if self.pe_axes[a] {
                vec![0.0; nvox]
            } else {
                Vec::new()
            }
        });
        {
            let rows: Vec<(f64, [f64; 3])> = (0..nvox)
                .into_par_iter()
                .map(|v| {
                    if !self.mask[v] {
                        return (0.0, [0.0; 3]);
                    }
                    let mean = ev.u.iter().map(|ui| ui[v]).sum::<f64>() / m as f64;
                    let mut sa = 0.0;
                    let mut sb = [0.0; 3];
                    for i in 0..m {
                        let r = mean - ev.u[i][v];
                        let (alpha, beta) = self.alpha_beta(ev, i, v);
                        sa += r * alpha;
                        for a in 0..3 {
                            sb[a] += r * beta[a];
                        }
                    }
                    (sa, sb)
                })
                .collect();
            for (v, (sa, sb)) in rows.into_iter().enumerate() {
                wa[v] = sa;
                for a in 0..3 {
                    if self.pe_axes[a] {
                        wb[a][v] = sb[a];
                    }
                }
            }
        }
        let factor = -2.0 / (self.n_mask.max(1) * (m.max(2) - 1)) as f64;
        let mut g = self.project(&wa, None);
        for a in 0..3 {
            if self.pe_axes[a] {
                let p = self.project(&wb[a], Some(a));
                for (x, y) in g.iter_mut().zip(&p) {
                    *x += y;
                }
            }
        }
        let lam = ev.lambda_eff(self);
        for (k, x) in g.iter_mut().enumerate() {
            *x *= factor;
            if self.lambda != 0.0 {
                *x += 2.0 * lam * ev.rc[k];
            }
        }
        let _ = c;
        g
    }

    /// The Gauss-Newton Hessian of the cost at the evaluated point, with the regulariser's.
    pub fn hessian(&self, ev: &Eval) -> Stencil {
        let nvox = ev.h.len();
        let m = self.e.len();
        // the basis types present: 0 = B, 1 + a = ∂_a B
        let types: Vec<usize> = std::iter::once(0)
            .chain((0..3).filter(|&a| self.pe_axes[a]).map(|a| a + 1))
            .collect();
        let nt = types.len();
        // per voxel, the weights W_pq = Σ_i (c_ip - c̄_p)(c_iq - c̄_q), masked
        let weights: Vec<Vec<f64>> = (0..nvox)
            .into_par_iter()
            .map(|v| {
                let mut w = vec![0.0; nt * nt];
                if !self.mask[v] {
                    return w;
                }
                let mut comps = vec![vec![0.0; nt]; m];
                for (i, ci) in comps.iter_mut().enumerate() {
                    let (alpha, beta) = self.alpha_beta(ev, i, v);
                    for (j, &t) in types.iter().enumerate() {
                        ci[j] = if t == 0 { alpha } else { beta[t - 1] };
                    }
                }
                let mean: Vec<f64> = (0..nt)
                    .map(|j| comps.iter().map(|c| c[j]).sum::<f64>() / m as f64)
                    .collect();
                for c in &comps {
                    for p in 0..nt {
                        for q in 0..nt {
                            w[p * nt + q] += (c[p] - mean[p]) * (c[q] - mean[q]);
                        }
                    }
                }
                w
            })
            .collect();
        let dims = self.coef_dims();
        let mut h = Stencil::zeros(dims);
        let factor = 2.0 / (self.n_mask.max(1) * (m.max(2) - 1)) as f64;
        for (p, &tp) in types.iter().enumerate() {
            for (q, &tq) in types.iter().enumerate() {
                let w: Vec<f64> = weights.iter().map(|x| x[p * nt + q] * factor).collect();
                if w.iter().all(|&x| x == 0.0) {
                    continue;
                }
                let dp = |a: usize| usize::from(tp == a + 1);
                let dq = |a: usize| usize::from(tq == a + 1);
                h.add_weighted(
                    &w,
                    self.nt,
                    std::array::from_fn(|a| &self.basis[a][dp(a)]),
                    std::array::from_fn(|a| &self.basis[a][dq(a)]),
                );
            }
        }
        if self.lambda != 0.0 {
            self.reg.add_to(&mut h, 2.0 * ev.lambda_eff(self));
        }
        h
    }
}
