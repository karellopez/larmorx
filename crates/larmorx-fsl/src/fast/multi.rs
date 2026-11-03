// SPDX-License-Identifier: Apache-2.0
//! The multi-channel segmentation (`specs/fast.md` §5, `-S n` with n ≥ 2): the mask where every
//! channel is above 0, the per-channel log images, the initialisation by the square of the
//! summed differences (§5.2), the classes' means, covariances and inverses (§5.3), the Gaussian
//! likelihood (§5.4), the member probabilities (§5.5), the per-channel bias fields and the MRF
//! sweeps without random restart (§5.6), the class order with FSL-98's pairing (§5.7) and the
//! partial volumes in the log domain (§5.8).
//!
//! The channels of a voxel are stored together (`y[v * S + i]`), the posteriors as in
//! [`super::single`] (`n` per mask voxel and a row of zeros for the neighbours outside the
//! mask). The matrix arithmetic is larmorx's own (LU with partial pivoting, any size): the
//! original does not follow OpenBLAS's kernels (§8.6), so `-S 9` and more are segmented too.

use std::io::Write;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;

use larmorx_core::math;
use rayon::prelude::*;

use super::bias;
use super::grid::{Grid, NEIGHBOURS};
use super::pve::{self, Pve};
use super::single::{Params, Stop, cell, from_cells, log1, prior_rows, to_cells};
use super::stats::{self, PI_F32};
use crate::cli::fmt::g;

/// One class's statistics over the `S` channels (§5.3): the means, the covariance (row-major
/// `S × S`), its inverse and determinant.
#[derive(Clone, Debug)]
pub struct Class {
    pub mean: Vec<f64>,
    pub cov: Vec<f64>,
    pub inv: Vec<f64>,
    pub det: f64,
}

/// What a multi-channel segmentation computed (the classes in their final order).
#[derive(Clone, Debug)]
pub struct Multi {
    pub grid: Grid,
    pub n: usize,
    pub channels: usize,
    /// The hard segmentation (1..n, 0 where no `w p` is above `-1e-10`).
    pub seg: Vec<u8>,
    /// `-p`'s member probabilities (`n` per mask voxel).
    pub members: Vec<f32>,
    pub pve: Option<Pve>,
    /// Each channel's bias field `b_i` per mask voxel (`None` with `-N`).
    pub bias: Option<Vec<Vec<f32>>>,
    /// The tissue volumes (mm³) `-v` prints.
    pub volumes: Vec<f64>,
    /// The class order's permutation was not its own inverse (FSL-98 changes the result).
    pub cycle: bool,
    /// `-H nan` with `reproduce`: the β estimate read a table FAST never writes (FSL-99).
    pub unwritten_beta: bool,
}

/// The determinant and inverse of the `s × s` matrix `a` (row-major) by LU decomposition with
/// partial pivoting; `None` when a pivot is exactly 0 (singular). NaN entries give NaN results.
pub fn invert(a: &[f64], s: usize) -> Option<(f64, Vec<f64>)> {
    let mut lu = a.to_vec();
    let mut piv: Vec<usize> = (0..s).collect();
    let mut det = 1.0;
    for k in 0..s {
        let mut p = k;
        let mut best = lu[k * s + k].abs();
        for r in k + 1..s {
            let v = lu[r * s + k].abs();
            if v > best {
                best = v;
                p = r;
            }
        }
        if lu[p * s + k] == 0.0 {
            return None;
        }
        if p != k {
            for c in 0..s {
                lu.swap(k * s + c, p * s + c);
            }
            piv.swap(k, p);
            det = -det;
        }
        let d = lu[k * s + k];
        det *= d;
        for r in k + 1..s {
            let f = lu[r * s + k] / d;
            lu[r * s + k] = f;
            for c in k + 1..s {
                lu[r * s + c] -= f * lu[k * s + c];
            }
        }
    }
    let mut inv = vec![0.0; s * s];
    let mut x = vec![0.0; s];
    for col in 0..s {
        for (r, xr) in x.iter_mut().enumerate() {
            *xr = if piv[r] == col { 1.0 } else { 0.0 };
        }
        for r in 0..s {
            for c in 0..r {
                x[r] -= lu[r * s + c] * x[c];
            }
        }
        for r in (0..s).rev() {
            for c in r + 1..s {
                x[r] -= lu[r * s + c] * x[c];
            }
            x[r] /= lu[r * s + r];
        }
        for r in 0..s {
            inv[r * s + col] = x[r];
        }
    }
    Some((det, inv))
}

/// `dᵀ M d` (double): the row `dᵀ M`, each element a left-to-right sum, times `d`.
#[inline]
fn quad(m: &[f64], d: &[f64]) -> f64 {
    let s = d.len();
    let mut total = 0.0;
    for j in 0..s {
        let mut row = 0.0;
        for i in 0..s {
            row += d[i] * m[i * s + j];
        }
        total += row * d[j];
    }
    total
}

/// `(2π)^S` in single precision, one factor at a time (§5.4).
pub fn two_pi(s: usize) -> f32 {
    (0..s).fold(1f32, |v, _| (v * 2.0) * PI_F32)
}

impl Class {
    /// The likelihood's normalisation `log(sqrt(|det C (2π)^S|))` (double).
    fn norm(&self, tp: f32) -> f64 {
        math::log((self.det * f64::from(tp)).abs().sqrt())
    }

    /// `G = ½ dᵀ C⁻¹ d + norm` (§5.4) at the channels `y`: `d` in double, the quadratic form
    /// rounded to single, the sum rounded to single.
    #[inline]
    fn neg_log(&self, y: &[f32], norm: f64, d: &mut [f64]) -> f32 {
        for (k, dk) in d.iter_mut().enumerate() {
            *dk = f64::from(y[k]) - self.mean[k];
        }
        let q = quad(&self.inv, d) as f32;
        (0.5 * f64::from(q) + norm) as f32
    }
}

/// The member probabilities (§5.5): `q = w p / Σ w p` with `w = 1/n` (single), 0 where the sum
/// is not above 0.
pub fn members(post: &[f32], n: usize, nm: usize) -> Vec<f32> {
    let w = 1.0f32 / n as f32;
    let mut q = vec![0f32; nm * n];
    q.par_chunks_mut(n)
        .zip(post[..nm * n].par_chunks(n))
        .for_each(|(q, p)| {
            let mut s = 0f32;
            for (qc, &pc) in q.iter_mut().zip(p) {
                *qc = w * pc;
                s += *qc;
            }
            for qc in q.iter_mut() {
                *qc = if s > 0.0 { *qc / s } else { 0.0 };
            }
        });
    q
}

/// The classes' statistics from the members `q` and the channels `y` (§5.3): `Σ q` in single,
/// the products `q y` in single summed in double; the covariances in double; a singular one
/// stops (FAST aborts). Every sum runs over the voxels in order; the sums are independent of
/// each other, so they run in parallel (one task each), with the serial bits.
pub fn class_stats(q: &[f32], y: &[f32], n: usize, s: usize) -> Result<Vec<Class>, Stop> {
    let nm = y.len().checked_div(s).unwrap_or(0);
    // Σ q_c (single) and Σ q_c y_ci (double) of each class
    let first: Vec<f64> = (0..n * (1 + s))
        .into_par_iter()
        .map(|k| {
            let (c, which) = (k / (1 + s), k % (1 + s));
            if which == 0 {
                let mut sw = 0f32;
                for v in 0..nm {
                    sw += q[v * n + c];
                }
                f64::from(sw)
            } else {
                let i = which - 1;
                let mut t = 0f64;
                for v in 0..nm {
                    t += f64::from(q[v * n + c] * y[v * s + i]);
                }
                t
            }
        })
        .collect();
    let means: Vec<Vec<f64>> = (0..n)
        .map(|c| {
            let sw = first[c * (1 + s)];
            (0..s)
                .map(|i| {
                    let t = first[c * (1 + s) + 1 + i];
                    if sw == 0.0 { 0.0 } else { t / sw }
                })
                .collect()
        })
        .collect();
    // Σ q_c (y_ci - μ_ci)(y_cj - μ_cj) of each class and pair (double)
    let second: Vec<f64> = (0..n * s * s)
        .into_par_iter()
        .map(|k| {
            let (c, i, j) = (k / (s * s), k / s % s, k % s);
            let (mi, mj) = (means[c][i], means[c][j]);
            let mut t = 0f64;
            for v in 0..nm {
                let qc = f64::from(q[v * n + c]);
                let di = f64::from(y[v * s + i]) - mi;
                let dj = f64::from(y[v * s + j]) - mj;
                t += qc * di * dj;
            }
            t
        })
        .collect();
    means
        .into_iter()
        .enumerate()
        .map(|(c, mean)| {
            let sw = first[c * (1 + s)];
            let cov: Vec<f64> = second[c * s * s..(c + 1) * s * s]
                .iter()
                .map(|&v| v / sw)
                .collect();
            let (det, inv) = invert(&cov, s).ok_or(Stop::Singular)?;
            Ok(Class {
                mean,
                cov,
                inv,
                det,
            })
        })
        .collect()
}

/// `G_c` of every mask voxel and class (§5.4).
fn neg_logs(classes: &[Class], y: &[f32], s: usize, nm: usize) -> Vec<f32> {
    let n = classes.len();
    let tp = two_pi(s);
    let norms: Vec<f64> = classes.iter().map(|c| c.norm(tp)).collect();
    let mut out = vec![0f32; nm * n];
    out.par_chunks_mut(n.max(1))
        .zip(y.par_chunks(s))
        .for_each_init(
            || vec![0f64; s],
            |d, (row, yv)| {
                for (c, g) in row.iter_mut().enumerate() {
                    *g = classes[c].neg_log(yv, norms[c], d);
                }
            },
        );
    out
}

/// The likelihoods `exp(-G_c)` (double `exp`) normalised over the classes (single), 0 where
/// their sum is not above 0 (§5.2).
fn likelihoods(classes: &[Class], y: &[f32], s: usize, post: &mut [f32]) {
    let n = classes.len();
    let nm = y.len() / s;
    let gs = neg_logs(classes, y, s, nm);
    // a batch of voxels at a time (`math::exp_in_place`: the same bits as one by one)
    const BATCH: usize = 64;
    post[..nm * n]
        .par_chunks_mut(n * BATCH)
        .zip(gs.par_chunks(n * BATCH))
        .for_each(|(rows, gr)| {
            let mut e = [0f64; 8 * BATCH];
            let mut e_vec = Vec::new();
            let e: &mut [f64] = if gr.len() <= e.len() {
                &mut e[..gr.len()]
            } else {
                e_vec.resize(gr.len(), 0.0);
                &mut e_vec
            };
            for (v, &gc) in e.iter_mut().zip(gr) {
                *v = -f64::from(gc);
            }
            math::exp_in_place(e);
            for (row, er) in rows.chunks_exact_mut(n).zip(e.chunks_exact(n)) {
                let mut t = 0f32;
                for (p, &ec) in row.iter_mut().zip(er) {
                    *p = ec as f32;
                    t += *p;
                }
                for p in row.iter_mut() {
                    *p = if t > 0.0 { *p / t } else { 0.0 };
                }
            }
        });
}

/// The multi-channel MRF weights (§5.1): §4.2's, the distances in single precision.
pub fn weights(voxel: [f32; 3]) -> [f32; 18] {
    let [dx, dy, dz] = voxel;
    let mut w = [0f32; 18];
    for (k, o) in NEIGHBOURS.iter().enumerate() {
        w[k] = match (o[0] != 0, o[1] != 0, o[2] != 0) {
            (true, false, false) => 1.0 / dx,
            (false, true, false) => 1.0 / dy,
            (false, false, true) => 1.0 / dz,
            (false, true, true) => 1.0 / (dz * dz + dy * dy).sqrt(),
            (true, false, true) => 1.0 / (dz * dz + dx * dx).sqrt(),
            (true, true, false) => 1.0 / (dx * dx + dy * dy).sqrt(),
            _ => 0.0,
        };
    }
    w
}

/// The multi-channel bias field's kernels (§5.1): `σ = sqrt(l / d)` in single precision.
pub fn kernels(lowpass: f32, voxel: [f32; 3]) -> [Vec<f64>; 3] {
    voxel.map(|d| bias::kernel((lowpass / d).sqrt()))
}

/// One voxel's update of the multi-channel MRF (§5.6): `p_c ← expf(β U_c - G_c)` (single,
/// `U_c` summed in double and rounded to single; with `-P` times `π_c`), normalised by their
/// single sum, 0 where it is not above 0; `u` and `e` hold `n` values.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn sweep_voxel(
    grid: &Grid,
    w: &[f64; 18],
    cells: &[AtomicU32],
    gs: &[f32],
    beta: f32,
    priors: Option<&[f32]>,
    i: usize,
    u: &mut [f64],
    e: &mut [f32],
) {
    let n = u.len();
    let nb = grid.neighbours(i);
    u.fill(0.0);
    for (k, &j) in nb.iter().enumerate() {
        for (c, uc) in u.iter_mut().enumerate() {
            *uc += w[k] * f64::from(cell(cells, j * n + c));
        }
    }
    let mut t = 0f32;
    for c in 0..n {
        let mut v = math::expf(beta * (u[c] as f32) - gs[i * n + c]);
        if let Some(pr) = priors {
            v *= pr[i * n + c];
        }
        e[c] = v;
        t += v;
    }
    for c in 0..n {
        let p = if t > 0.0 { e[c] / t } else { 0.0 };
        cells[i * n + c].store(p.to_bits(), Relaxed);
    }
}

/// Five in-place MRF sweeps in storage order (§5.6), each on the pipeline of
/// [`Grid::gauss_seidel`] (the serial bits); the usual class counts with their scratch on the
/// stack.
fn sweeps(
    grid: &Grid,
    post: &mut Vec<f32>,
    gs: &[f32],
    beta: f32,
    priors: Option<&[f32]>,
    n: usize,
) {
    fn fixed<const N: usize>(
        grid: &Grid,
        w: &[f64; 18],
        cells: &[AtomicU32],
        gs: &[f32],
        beta: f32,
        priors: Option<&[f32]>,
    ) {
        grid.gauss_seidel(|r| {
            let (mut u, mut e) = ([0f64; N], [0f32; N]);
            for i in r {
                sweep_voxel(grid, w, cells, gs, beta, priors, i, &mut u, &mut e);
            }
        });
    }
    let w: [f64; 18] = grid.weights.map(f64::from);
    let cells = to_cells(std::mem::take(post));
    for _ in 0..5 {
        match n {
            2 => fixed::<2>(grid, &w, &cells, gs, beta, priors),
            3 => fixed::<3>(grid, &w, &cells, gs, beta, priors),
            4 => fixed::<4>(grid, &w, &cells, gs, beta, priors),
            _ => grid.gauss_seidel(|r| {
                let (mut u, mut e) = (vec![0f64; n], vec![0f32; n]);
                for i in r {
                    sweep_voxel(grid, &w, &cells, gs, beta, priors, i, &mut u, &mut e);
                }
            }),
        }
    }
    *post = from_cells(cells);
}

/// One estimate of every channel's bias field (§5.6) from the members `q` and the currently
/// corrected channels `yb`: `W_i = Σ_c q_c / C_c(i,i)`, `R_i = Σ_c (q_c / C_c(i,i))(y_bi - μ_ci)`
/// (the weights `q_c / C_c(i,i)` in single precision, `W` summed in single, each product of
/// `R` in double rounded to single as it is added), both smoothed; `b_i = R_s / W_s`, 0 where
/// `W_s` is 0.
fn bias_fields(
    grid: &Grid,
    q: &[f32],
    yb: &[f32],
    classes: &[Class],
    k: &[Vec<f64>; 3],
    s: usize,
) -> Vec<Vec<f32>> {
    let n = classes.len();
    let ks = [&k[0][..], &k[1][..], &k[2][..]];
    (0..s)
        .map(|i| {
            // each mask voxel's weight and residual (in parallel), then over the image
            let wr: Vec<(f32, f32)> = (0..grid.len())
                .into_par_iter()
                .map(|m| {
                    let (mut ws, mut rs) = (0f32, 0f32);
                    for (c, cl) in classes.iter().enumerate() {
                        // the weight in single precision (the double quotient rounded), summed
                        // in single; the residual's product with it in double, rounded at each
                        // sum (observed: bit-identical to FSL's field with a one-tap kernel)
                        let t = (f64::from(q[m * n + c]) / cl.cov[i * s + i]) as f32;
                        ws += t;
                        rs = (f64::from(rs)
                            + f64::from(t) * (f64::from(yb[m * s + i]) - cl.mean[i]))
                            as f32;
                    }
                    (ws, rs)
                })
                .collect();
            let w = grid.expand(|m| wr[m].0, 0.0);
            let r = grid.expand(|m| wr[m].1, 0.0);
            drop(wr);
            let w = bias::smooth(w, grid.shape, ks);
            let r = bias::smooth(r, grid.shape, ks);
            grid.index
                .par_iter()
                .map(|&idx| {
                    let ws = w[idx as usize];
                    if ws == 0.0 { 0.0 } else { r[idx as usize] / ws }
                })
                .collect()
        })
        .collect()
}

/// `-v`'s lines of the classes (§5.6): per channel ` CHANNEL i` and each class's `MEAN e^μ
/// STDDEV (e^(μ+σ) - e^(μ-σ))/2` (double).
fn class_lines(out: &mut dyn Write, classes: &[Class], s: usize) {
    for i in 0..s {
        let mut line = format!(" CHANNEL {}", i + 1);
        for (c, cl) in classes.iter().enumerate() {
            let m = cl.mean[i];
            let sd = cl.cov[i * s + i].sqrt();
            let spread = (math::exp(m + sd) - math::exp(m - sd)) / 2.0;
            line.push_str(&format!(
                " CLASS {} MEAN {} STDDEV {}",
                c + 1,
                g(math::exp(m)),
                g(spread)
            ));
        }
        let _ = writeln!(out, "{line}");
    }
}

/// Segments `channels.len()` channels (channel 1 first; each after §4.1's negative rule, on
/// channel 1's grid, radiological frame, x fastest). `in_mask` is the mask (every channel above
/// 0); `sizes_differ` makes the percentile of a second channel stop as FAST does (FSL-104's
/// reproduction).
pub fn segment(
    channels: &[&[f32]],
    shape: [usize; 3],
    voxel: [f32; 3],
    in_mask: impl Fn(usize) -> bool,
    p: &Params,
    sizes_differ: bool,
    out: &mut dyn Write,
) -> Result<Multi, Stop> {
    let s = channels.len();
    let mut grid = Grid::new(shape, voxel, in_mask).ok_or(Stop::TooLarge)?;
    grid.weights = weights(voxel);
    let n = p.n_classes;
    let nm = grid.len();
    let mut y = vec![0f32; nm * s];
    y.par_chunks_mut(s)
        .zip(grid.index.par_iter())
        .for_each(|(row, &idx)| {
            for (i, v) in row.iter_mut().enumerate() {
                *v = log1(channels[i][idx as usize]);
            }
        });
    let mut post = vec![0f32; (nm + 1) * n];
    let pri = p.priors.as_ref().map(|pr| prior_rows(&grid, pr, n));
    let mut classes: Vec<Class>;
    let mut q: Vec<f32>;
    if let Some(pr) = &pri {
        if p.verbose {
            let _ = writeln!(out, "Beginning prior-based initialisation");
        }
        post[..nm * n].copy_from_slice(pr);
        q = members(&post, n, nm);
        classes = class_stats(&q, &y, n, s)?;
    } else {
        // §5.2: the initial means, channel by channel
        let mut means = vec![vec![0f64; n]; s];
        let given = p.manual.as_ref().filter(|v| v.len() == n * s);
        for (i, m) in means.iter_mut().enumerate() {
            match given {
                Some(v) => {
                    for (c, mc) in m.iter_mut().enumerate() {
                        *mc = f64::from(math::logf(v[i * n + c]));
                    }
                }
                None => {
                    if i > 0 && sizes_differ {
                        return Err(Stop::Sizes);
                    }
                    let sorted = stats::sorted((0..nm).map(|v| y[v * s + i]).collect());
                    let step = 1.0 / (n as f32 + 1.0);
                    for (c, mc) in m.iter_mut().enumerate() {
                        *mc =
                            f64::from(stats::percentile_of_sorted(&sorted, (c + 1) as f32 * step));
                    }
                }
            }
            if p.verbose {
                for (c, mc) in m.iter().enumerate() {
                    let _ = writeln!(out, "{} {} {}", i + 1, c + 1, g(*mc));
                }
            }
        }
        for m in &means {
            for a in 0..n {
                for b in a + 1..n {
                    if m[a] == m[b] {
                        return Err(Stop::KMeans);
                    }
                }
            }
        }
        // the hard assignment by the smallest square of the summed differences
        post[..nm * n]
            .par_chunks_mut(n)
            .zip(y.par_chunks(s))
            .for_each(|(row, yv)| {
                let (mut best, mut low) = (0usize, 1e31f32);
                for c in 0..n {
                    // the differences and their products in double, the sum single (observed:
                    // single-precision products move the assignment of a few voxels)
                    let mut d = 0f32;
                    for i in 0..s {
                        let a = f64::from(yv[i]) - means[i][c];
                        for j in 0..s {
                            let b = f64::from(yv[j]) - means[j][c];
                            d = (f64::from(d) + a * b) as f32;
                        }
                    }
                    if d < low {
                        low = d;
                        best = c;
                    }
                }
                row[best] = 1.0;
            });
        for it in 0..p.init_iterations.max(0) {
            if p.verbose {
                let _ = writeln!(out, "KMeans Iteration {it}");
            }
            q = members(&post, n, nm);
            classes = class_stats(&q, &y, n, s)?;
            likelihoods(&classes, &y, s, &mut post);
        }
        q = members(&post, n, nm);
        classes = class_stats(&q, &y, n, s)?;
        likelihoods(&classes, &y, s, &mut post);
        q = members(&post, n, nm);
    }
    // §5.6: the main loop
    let k = kernels(p.lowpass, voxel);
    let mut yb = y.clone();
    let mut bias: Option<Vec<Vec<f32>>> = None;
    let estimate = |q: &[f32], classes: &[Class], yb: &mut Vec<f32>| -> Vec<Vec<f32>> {
        let b = bias_fields(&grid, q, yb, classes, &k, s);
        yb.par_chunks_mut(s)
            .zip(y.par_chunks(s))
            .enumerate()
            .for_each(|(v, (row, y0))| {
                for i in 0..s {
                    row[i] = y0[i] - b[i][v];
                }
            });
        b
    };
    if !p.no_bias {
        bias = Some(estimate(&q, &classes, &mut yb));
    }
    let pr = pri.as_deref().filter(|_| p.priors_throughout);
    let mut beta = p.f_hard;
    let mut unwritten_beta = false;
    for it in 0..p.bias_iterations.max(0) {
        if p.verbose {
            let _ = writeln!(
                out,
                "Tanaka Iteration {it} bias field {}",
                p.bias_iterations
            );
        }
        let gs = neg_logs(&classes, &yb, s, nm);
        sweeps(&grid, &mut post, &gs, beta, pr, n);
        if p.verbose {
            class_lines(out, &classes, s);
        }
        q = members(&post, n, nm);
        if !p.no_bias {
            bias = Some(estimate(&q, &classes, &mut yb));
        }
        classes = class_stats(&q, &yb, n, s)?;
    }
    for it in 0..p.fixed_iterations.max(0) {
        if p.verbose {
            let _ = writeln!(
                out,
                "Tanaka Iteration {it} hyperparameter {}",
                p.fixed_iterations
            );
        }
        let gs = neg_logs(&classes, &yb, s, nm);
        sweeps(&grid, &mut post, &gs, beta, pr, n);
        if p.verbose {
            class_lines(out, &classes, s);
        }
        q = members(&post, n, nm);
        beta = if p.hyper.is_nan() {
            // FSL-99 (reproduce): FAST estimates β against a table it never writes; larmorx
            // takes it as zeros (one channel's table), which gives 0
            unwritten_beta = true;
            0.0
        } else {
            p.hyper
        };
        if p.verbose {
            let _ = writeln!(out, " BETA {}", g(f64::from(beta)));
        }
        classes = class_stats(&q, &yb, n, s)?;
    }
    // §5.7: the class order (channel 1's means; -t 2 by their negation)
    let key: Vec<f64> = classes
        .iter()
        .map(|c| {
            if p.image_type == 2 {
                -c.mean[0]
            } else {
                c.mean[0]
            }
        })
        .collect();
    let mut perm: Vec<usize> = (0..n).collect();
    perm.sort_by(|&a, &b| {
        key[a]
            .partial_cmp(&key[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut inv = vec![0usize; n];
    for (k, &c) in perm.iter().enumerate() {
        inv[c] = k;
    }
    let cycle = perm != inv;
    let src: Vec<usize> = if p.fsl98_pairing {
        inv.clone()
    } else {
        perm.clone()
    };
    let sorted: Vec<Class> = (0..n)
        .map(|k| {
            let mut c = classes[src[k]].clone();
            c.mean[0] = classes[perm[k]].mean[0];
            c
        })
        .collect();
    let mut sp = vec![0f32; nm * n];
    sp.par_chunks_mut(n)
        .zip(post[..nm * n].par_chunks(n))
        .for_each(|(dst, row)| {
            for k in 0..n {
                dst[k] = row[src[k]];
            }
        });
    drop(post);
    // the hard segmentation: the largest w p
    let w = 1.0f32 / n as f32;
    let seg: Vec<u8> = sp
        .par_chunks(n)
        .map(|row| {
            let (mut best, mut high) = (0u8, -1e-10f32);
            for (c, &v) in row.iter().enumerate() {
                if w * v > high {
                    high = w * v;
                    best = (c + 1) as u8;
                }
            }
            best
        })
        .collect();
    let members_out = members(&sp, n, nm);
    let vox = f64::from(voxel[0]) * f64::from(voxel[1]) * f64::from(voxel[2]);
    let pve_out = if p.pve {
        if p.verbose {
            let _ = writeln!(out, "Partial Volume Estimation ");
        }
        Some(partial_volumes(&grid, &yb, &sorted, n, s, p.mixel))
    } else {
        None
    };
    let source = pve_out.as_ref().map_or(&sp, |r| &r.frac);
    let volumes: Vec<f64> = (0..n)
        .map(|c| (0..nm).map(|i| f64::from(source[i * n + c])).sum::<f64>() * vox)
        .collect();
    if p.verbose && p.pve {
        // the tissue volumes follow the partial volumes only (observed: none with --nopve)
        let mut total = 0f64;
        for (c, v) in volumes.iter().enumerate() {
            let _ = write!(out, "\n tissue {} {}", c + 1, g(*v));
            total += v;
        }
        let _ = writeln!(out, "\n total tissue {}", g(total));
    }
    Ok(Multi {
        grid,
        n,
        channels: s,
        seg,
        members: members_out,
        pve: pve_out,
        bias,
        volumes,
        cycle,
        unwritten_beta,
    })
}

/// One fraction of a mixed type (§5.8): `δ`, `1 - δ`, the mixture's mean, inverse covariance,
/// determinant (single) and `log det` (single).
struct Frac {
    delta: f32,
    rest: f32,
    mean: Vec<f64>,
    inv: Vec<f64>,
    norm: f32,
    log_det: f32,
}

/// The fractions of several channels (§8.7): from 0 by `δ + 1/100` in single precision while
/// `δ ≤ 1`.
pub fn fractions() -> Vec<f32> {
    let step = 1.0f32 / 100.0;
    let mut out = Vec::with_capacity(101);
    let mut d = 0f32;
    while d <= 1.0 {
        out.push(d);
        d += step;
    }
    out
}

/// `G'`'s normalisation in single precision: `logf(sqrtf(|det · (2π)^S|))`.
#[inline]
fn norm_single(det: f32, tp: f32) -> f32 {
    math::logf((det * tp).abs().sqrt())
}

/// The partial volumes of several channels (§5.8) in the log domain with the last statistics.
fn partial_volumes(
    grid: &Grid,
    yb: &[f32],
    classes: &[Class],
    n: usize,
    s: usize,
    mixel: f32,
) -> Pve {
    let nm = grid.len();
    if n <= 1 {
        return Pve {
            frac: vec![0.0; nm * n],
            seg: vec![1; nm],
            mixel: vec![0; nm],
        };
    }
    let pairs = pve::mixtures(n);
    let nt = n + pairs.len();
    let tp = two_pi(s);
    let step = 1.0f32 / 100.0;
    let deltas = fractions();
    let tables: Vec<Vec<Frac>> = pairs
        .iter()
        .map(|&(a, b)| {
            let (ca, cb) = (&classes[a], &classes[b]);
            deltas
                .iter()
                .map(|&d| {
                    let rest = 1.0 - d;
                    let (dd, rr) = (f64::from(d), f64::from(rest));
                    let mean: Vec<f64> =
                        (0..s).map(|i| dd * ca.mean[i] + rr * cb.mean[i]).collect();
                    let cov: Vec<f64> = (0..s * s)
                        .map(|k| dd * dd * ca.cov[k] + rr * rr * cb.cov[k])
                        .collect();
                    let (det, inv) = invert(&cov, s).unwrap_or((f64::NAN, vec![f64::NAN; s * s]));
                    let det = det as f32;
                    Frac {
                        delta: d,
                        rest,
                        mean,
                        inv,
                        norm: norm_single(det, tp),
                        log_det: math::logf(det),
                    }
                })
                .collect()
        })
        .collect();
    let pure_norm: Vec<f64> = classes.iter().map(|c| c.norm(tp)).collect();
    let pure_single: Vec<f32> = classes
        .iter()
        .map(|c| norm_single(c.det as f32, tp))
        .collect();
    // a batch of voxels at a time: each term's `exp` for the whole batch at once
    // (`math::exp_in_place`, the same bits as one by one), each voxel's sum in fraction order
    const BATCH: usize = 64;
    let mut like = vec![0f32; nm * nt];
    like.par_chunks_mut(nt * BATCH)
        .zip(yb.par_chunks(s * BATCH))
        .for_each_init(
            || vec![0f64; s],
            |d, (rows, ys)| {
                let k = ys.len() / s;
                let mut g = [0f64; BATCH];
                for (c, cl) in classes.iter().enumerate() {
                    for (gl, yv) in g.iter_mut().zip(ys.chunks_exact(s)) {
                        let gc = if n == 3 {
                            cl.neg_log(yv, pure_norm[c], d)
                        } else {
                            for (k, dk) in d.iter_mut().enumerate() {
                                *dk = f64::from(yv[k]) - cl.mean[k];
                            }
                            0.5 * (quad(&cl.inv, d) as f32) + pure_single[c]
                        };
                        *gl = -f64::from(gc);
                    }
                    math::exp_in_place(&mut g[..k]);
                    for (row, &e) in rows.chunks_exact_mut(nt).zip(&g[..k]) {
                        row[c] = e as f32;
                    }
                }
                for (t, table) in tables.iter().enumerate() {
                    let mut acc = [0f32; BATCH];
                    for f in table {
                        for (gl, yv) in g.iter_mut().zip(ys.chunks_exact(s)) {
                            for (k, dk) in d.iter_mut().enumerate() {
                                *dk = f64::from(yv[k]) - f.mean[k];
                            }
                            let gp = 0.5 * (quad(&f.inv, d) as f32) + f.norm;
                            *gl = -f64::from(gp);
                        }
                        math::exp_in_place(&mut g[..k]);
                        for (al, &e) in acc.iter_mut().zip(&g[..k]) {
                            *al = (f64::from(*al) + e * f64::from(step)) as f32;
                        }
                    }
                    for (row, &al) in rows.chunks_exact_mut(nt).zip(&acc[..k]) {
                        row[n + t] = al;
                    }
                }
            },
        );
    let types = pve::icm(grid, &like, n, &pairs, mixel);
    let mut frac = vec![0f32; nm * n];
    frac.par_chunks_mut(n)
        .zip(types.par_iter())
        .zip(yb.par_chunks(s))
        .for_each_init(
            || vec![0f64; s],
            |d, ((f, &t), yv)| {
                let t = t as usize;
                if t < n {
                    f[t] = 1.0;
                    return;
                }
                let (a, b) = pairs[t - n];
                let (mut best, mut low) = (None, 1e13f32);
                for fr in &tables[t - n] {
                    for (k, dk) in d.iter_mut().enumerate() {
                        *dk = f64::from(yv[k]) - fr.mean[k];
                    }
                    let e = (quad(&fr.inv, d) as f32) + fr.log_det;
                    if e < low {
                        low = e;
                        best = Some(fr);
                    }
                }
                let (da, db) = best.map_or((0.0, 1.0), |fr| (fr.delta, fr.rest));
                f[a] = da;
                f[b] = db;
            },
        );
    let seg = pve::pveseg(&frac, n);
    Pve {
        frac,
        seg,
        mixel: types,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices() {
        let a = [4.0, 3.0, 6.0, 3.0];
        let (det, inv) = invert(&a, 2).unwrap();
        assert!((det - -6.0).abs() < 1e-12);
        assert!((inv[0] - -0.5).abs() < 1e-12 && (inv[1] - 0.5).abs() < 1e-12);
        assert!((inv[2] - 1.0).abs() < 1e-12 && (inv[3] - (-2.0 / 3.0)).abs() < 1e-12);
        assert!(invert(&[1.0, 1.0, 1.0, 1.0], 2).is_none());
        let (_, i) = invert(&[f64::NAN; 4], 2).unwrap();
        assert!(i.iter().all(|v| v.is_nan()));
        assert!((quad(&[2.0, 0.0, 0.0, 3.0], &[1.0, 2.0]) - 14.0).abs() < 1e-12);
        assert_eq!(two_pi(1), 2.0 * PI_F32);
        let f = fractions();
        assert!(f.len() == 100 || f.len() == 101);
    }
}
