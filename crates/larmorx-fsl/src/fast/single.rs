// SPDX-License-Identifier: Apache-2.0
//! The single-channel segmentation (`specs/fast.md` §4): the input's negative values, the mask
//! and the log image (§4.1), the initialisation (§4.3), the main loop of MRF iterations, bias
//! field estimates and class statistics (§4.5, §4.6), the class order and the hard segmentation
//! (§4.7), the partial volumes (§4.8, [`super::pve`]) and the member probabilities (§4.9).
//!
//! Every per-voxel quantity is kept for the mask's voxels only ([`Grid`]); the MRF sweeps run
//! in storage order and update in place (Gauss-Seidel, as FAST's), the rest runs in parallel
//! where each voxel is independent of the others.

use std::io::Write;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;

use larmorx_core::math::Libm;
use rayon::prelude::*;

use super::bias;
use super::grid::Grid;
use super::pve;
use super::rng::{FAST_SEED, GlibcRand};
use super::stats::{self, Gauss};
use crate::cli::fmt::g;

/// The options of one segmentation (§2.2).
#[derive(Clone, Debug)]
pub struct Params {
    /// `-n`: the number of classes (at least 1).
    pub n_classes: usize,
    /// `-I`: main-loop iterations with the bias field.
    pub bias_iterations: i64,
    /// `-l`: the bias field's smoothing extent (FWHM, mm).
    pub lowpass: f32,
    /// `-t`: 2 reverses the class order.
    pub image_type: i64,
    /// `-f`: the MRF weight while the bias field is estimated.
    pub f_hard: f32,
    /// `-N`.
    pub no_bias: bool,
    /// `-W`.
    pub init_iterations: i64,
    /// `-R`.
    pub mixel: f32,
    /// `-O`.
    pub fixed_iterations: i64,
    /// `-H`.
    pub hyper: f32,
    /// Not `--nopve`.
    pub pve: bool,
    /// `-s`'s initial class intensities when the file held exactly `n` of them.
    pub manual: Option<Vec<f32>>,
    /// `-v`.
    pub verbose: bool,
    /// Keep the member probabilities for `-p`.
    pub probabilities: bool,
    /// `-a`'s registered priors (CSF, grey, white matter) over the whole image (§6), when they
    /// are used: the initialisation is theirs.
    pub priors: Option<[Vec<f32>; 3]>,
    /// `-P`: the priors weight every MRF update.
    pub priors_throughout: bool,
    /// Several channels: FSL-98's pairing of the sorted classes (`reproduce`).
    pub fsl98_pairing: bool,
    /// The C library functions (GLIBC-07, §8: correctly rounded by default, glibc's with
    /// `reproduce`) and their witness.
    pub libm: std::sync::Arc<Libm>,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            n_classes: 3,
            bias_iterations: 4,
            lowpass: 20.0,
            image_type: 1,
            f_hard: 0.02,
            no_bias: false,
            init_iterations: 15,
            mixel: 0.3,
            fixed_iterations: 4,
            hyper: 0.1,
            pve: true,
            manual: None,
            verbose: false,
            probabilities: false,
            priors: None,
            priors_throughout: false,
            fsl98_pairing: false,
            libm: std::sync::Arc::default(),
        }
    }
}

/// Why a segmentation stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Two initial class means are equal (§4.3): FAST's exception, exit 255.
    KMeans,
    /// The image is too large for larmorx's indices.
    TooLarge,
    /// Several channels: a class's covariance is singular (FAST aborts: `inv(): matrix is
    /// singular`).
    Singular,
    /// Several channels of different sizes: the percentile of a second channel (FSL-104's
    /// reproduction: FAST aborts).
    Sizes,
}

/// What a segmentation computed, per mask voxel (the classes in their final order).
#[derive(Clone, Debug)]
pub struct Single {
    pub grid: Grid,
    pub n: usize,
    /// The hard segmentation: 1..n, 0 where every posterior is NaN.
    pub seg: Vec<u8>,
    /// `-p`'s member probabilities (`n` per voxel), `None` where FAST never computed them.
    pub members: Option<Vec<f32>>,
    pub pve: Option<pve::Pve>,
    /// The bias field `b` in the log domain (`None` with `-N`).
    pub bias: Option<Vec<f32>>,
    /// `b` outside the mask: `(0 - mean) · 0` (§4.5's last step), ±0, or NaN when the mask's
    /// mean of the field is not finite (a class of NaN variance: FSL's field and corrections are
    /// then NaN over the whole image, observed).
    pub bias_outside: f32,
    /// The tissue volumes (mm³) `-v` prints.
    pub volumes: Vec<f64>,
    /// How many of FSL's masked sums ran over an empty mask (each prints `ERROR:: Empty mask
    /// image`, §8.3): 4 per class for each class statistics, 1 per bias field estimate, 1 per
    /// class for the tissue volumes (observed).
    pub empty_sums: usize,
}

/// §4.1's rule for negative values, in place: when the minimum is below 0, the minimum is
/// subtracted if the 2nd percentile of all voxels is below 0, else the negative values are set
/// to 0.
pub fn negative_rule(data: &mut [f32]) {
    let Some(&first) = data.first() else { return };
    let (mut lo, mut hi) = (first, first);
    for &v in data.iter() {
        if v < lo {
            lo = v;
        }
        if v > hi {
            hi = v;
        }
    }
    if !(lo < 0.0) {
        return;
    }
    if stats::percentile(data, 0.02) < 0.0 {
        data.par_iter_mut().for_each(|v| *v -= lo);
    } else {
        data.par_iter_mut().for_each(|v| {
            if *v < 0.0 || *v > hi {
                *v = 0.0;
            }
        });
    }
}

/// The image's maximum as FAST finds it (the first voxel, then every larger one).
pub fn maximum(data: &[f32]) -> f32 {
    let mut hi = data.first().copied().unwrap_or(0.0);
    for &v in data {
        if v > hi {
            hi = v;
        }
    }
    hi
}

/// FAST's mask (§4.1): `0 < v < max + 1`, the bound in single precision.
#[inline]
pub fn in_fsl_mask(v: f32, bound: f32) -> bool {
    v > 0.0 && v < bound
}

/// The corrected mask (FSL-103's default): above 0 and finite.
#[inline]
pub fn in_mask(v: f32) -> bool {
    v > 0.0 && v.is_finite()
}

/// `log(I + 1)` (§4.1), double precision rounded to single.
#[inline]
pub fn log1(v: f32, libm: &Libm) -> f32 {
    libm.log(f64::from(v) + 1.0) as f32
}

/// The member probabilities (§4.9): `max(p, 0)` normalised by its single-precision sum when the
/// sum is above 0.
pub fn members(post: &[f32], n: usize, nm: usize) -> Vec<f32> {
    let mut q = vec![0f32; nm * n];
    q.par_chunks_mut(n.max(1))
        .zip(post[..nm * n].par_chunks(n.max(1)))
        .for_each(|(q, p)| {
            let mut s = 0f32;
            for (qc, &pc) in q.iter_mut().zip(p) {
                *qc = if pc < 0.0 { 0.0 } else { pc };
                s += *qc;
            }
            if s > 0.0 {
                for qc in q.iter_mut() {
                    *qc /= s;
                }
            }
        });
    q
}

/// The priors of every mask voxel (§6.6): `(π_csf, π_gm, π_wm) / s` for three classes,
/// `(π_csf, π_gm + π_wm) / s` for two, `s = π_csf + π_gm + π_wm` (single); `1/n` each where `s ≤
/// 0`.
pub fn prior_rows(grid: &Grid, priors: &[Vec<f32>; 3], n: usize) -> Vec<f32> {
    let mut out = vec![0f32; grid.len() * n];
    let even = 1.0f32 / n as f32;
    for (row, &idx) in out.chunks_exact_mut(n.max(1)).zip(&grid.index) {
        let i = idx as usize;
        let (a, b, c) = (priors[0][i], priors[1][i], priors[2][i]);
        let s = a + b + c;
        if s <= 0.0 {
            row.fill(even);
        } else if n == 3 {
            row.copy_from_slice(&[a / s, b / s, c / s]);
        } else if n == 2 {
            row.copy_from_slice(&[a / s, (b + c) / s]);
        }
    }
    out
}

/// The MRF sweeps' state: the posteriors (`n` per mask voxel, then a row of zeros for the
/// neighbours outside the mask).
struct Mrf<'a> {
    grid: &'a Grid,
    n: usize,
    post: Vec<f32>,
    weights: [f64; 18],
    libm: &'a Libm,
}

/// Posteriors as the in-place sweeps share them between threads (the bits of each `f32`).
pub(crate) fn to_cells(v: Vec<f32>) -> Vec<AtomicU32> {
    v.into_iter().map(|x| AtomicU32::new(x.to_bits())).collect()
}

pub(crate) fn from_cells(v: Vec<AtomicU32>) -> Vec<f32> {
    v.into_iter()
        .map(|x| f32::from_bits(x.into_inner()))
        .collect()
}

#[inline(always)]
pub(crate) fn cell(v: &[AtomicU32], i: usize) -> f32 {
    f32::from_bits(v[i].load(Relaxed))
}

/// One voxel's MRF update (§4.6): `U_c = Σ_k w_k p_kc` (double, the neighbours in §4.2's
/// order), `e_c = f32(exp(β U_c - G_c))` (times `π_c` with `-P`), `p_c = e_c / Σ e` (double,
/// rounded), 0 where the sum is not above 0; `u` and `e` hold `n` values.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn mrf_voxel(
    grid: &Grid,
    w: &[f64; 18],
    cells: &[AtomicU32],
    neg_g: &[f64],
    b: f64,
    priors: Option<&[f32]>,
    i: usize,
    u: &mut [f64],
    e: &mut [f32],
    libm: &Libm,
) {
    let n = u.len();
    let nb = grid.neighbours(i);
    u.fill(0.0);
    for (k, &j) in nb.iter().enumerate() {
        let wk = w[k];
        for (c, uc) in u.iter_mut().enumerate() {
            *uc += wk * f64::from(cell(cells, j * n + c));
        }
    }
    let mut s = 0f64;
    for c in 0..n {
        let mut v = libm.exp(b * u[c] - neg_g[i * n + c]) as f32;
        if let Some(pr) = priors {
            v *= pr[i * n + c];
        }
        e[c] = v;
        s += f64::from(v);
    }
    for c in 0..n {
        let p = if s > 0.0 {
            (f64::from(e[c]) / s) as f32
        } else {
            0.0
        };
        cells[i * n + c].store(p.to_bits(), Relaxed);
    }
}

/// One in-place sweep (§4.6) over the mask in storage order ([`Grid::gauss_seidel`]); the
/// usual class counts with their scratch on the stack (the same operations).
fn mrf_sweep(
    grid: &Grid,
    w: &[f64; 18],
    cells: &[AtomicU32],
    neg_g: &[f64],
    b: f64,
    priors: Option<&[f32]>,
    n: usize,
    libm: &Libm,
) {
    #[allow(clippy::too_many_arguments)]
    fn fixed<const N: usize>(
        grid: &Grid,
        w: &[f64; 18],
        cells: &[AtomicU32],
        neg_g: &[f64],
        b: f64,
        priors: Option<&[f32]>,
        libm: &Libm,
    ) {
        grid.gauss_seidel(|r| {
            let (mut u, mut e) = ([0f64; N], [0f32; N]);
            for i in r {
                mrf_voxel(grid, w, cells, neg_g, b, priors, i, &mut u, &mut e, libm);
            }
        });
    }
    match n {
        2 => fixed::<2>(grid, w, cells, neg_g, b, priors, libm),
        3 => fixed::<3>(grid, w, cells, neg_g, b, priors, libm),
        4 => fixed::<4>(grid, w, cells, neg_g, b, priors, libm),
        _ => grid.gauss_seidel(|r| {
            let (mut u, mut e) = (vec![0f64; n], vec![0f32; n]);
            for i in r {
                mrf_voxel(grid, w, cells, neg_g, b, priors, i, &mut u, &mut e, libm);
            }
        }),
    }
}

impl Mrf<'_> {
    /// One MRF iteration (§4.6): random posteriors, then five sweeps in storage order, in place.
    #[allow(clippy::too_many_arguments)]
    fn iterate(
        &mut self,
        yb: &[f32],
        classes: &[Gauss],
        beta: f32,
        rng: &mut GlibcRand,
        priors: Option<&[f32]>,
        out: &mut dyn Write,
        verbose: bool,
    ) {
        let n = self.n;
        let nm = self.grid.len();
        // the random posteriors: the sequence in order, voxel by voxel, class by class (drawn
        // serially), each voxel's normalisation in parallel
        for p in self.post[..nm * n].iter_mut() {
            *p = rng.next_value() as f32;
        }
        self.post[..nm * n].par_chunks_mut(n).for_each(|row| {
            let mut s = 0f32;
            for p in row.iter_mut() {
                *p /= 2_147_483_647f32;
                s += *p;
            }
            if s > 0.0 {
                for p in row.iter_mut() {
                    *p /= s;
                }
            }
        });
        // -G for each voxel and class: fixed through the sweeps
        let mut neg_g = vec![0f64; nm * n];
        neg_g
            .par_chunks_mut(n)
            .zip(yb.par_iter())
            .for_each(|(row, &y)| {
                for (v, c) in row.iter_mut().zip(classes) {
                    *v = f64::from(c.neg_log(y));
                }
            });
        let b = f64::from(beta);
        let cells = to_cells(std::mem::take(&mut self.post));
        for sweep in 0..5 {
            mrf_sweep(
                self.grid,
                &self.weights,
                &cells,
                &neg_g,
                b,
                priors,
                n,
                self.libm,
            );
            if verbose {
                let total = weights_total(self.grid, &self.weights, n, |k| cell(&cells, k));
                let _ = writeln!(
                    out,
                    "Tanaka-inner-loop-iteration={sweep} MRFWeightsTotal={} beta={}",
                    g(f64::from(total as f32)),
                    g(f64::from(beta))
                );
            }
        }
        self.post = from_cells(cells);
    }

    fn weights_total(&self) -> f64 {
        weights_total(self.grid, &self.weights, self.n, |k| self.post[k])
    }
}

/// `-v`'s `MRFWeightsTotal` after a sweep: `Σ_M Σ_c U_c p_c` over the posteriors as they stand
/// (`post(k)`, serial; observed: the first sweep's total is already near the last's, so it is
/// not accumulated during the sweep, whose neighbours ahead still hold random values).
fn weights_total(grid: &Grid, w: &[f64; 18], n: usize, post: impl Fn(usize) -> f32) -> f64 {
    let mut total = 0f64;
    for i in 0..grid.len() {
        let nb = grid.neighbours(i);
        for c in 0..n {
            let mut u = 0f64;
            for (k, &j) in nb.iter().enumerate() {
                u += w[k] * f64::from(post(j * n + c));
            }
            total += u * f64::from(post(i * n + c));
        }
    }
    total
}

/// Runs `f` on a pool of `threads` threads (tests).
#[cfg(test)]
pub(crate) fn with_pool<R: Send>(threads: usize, f: impl FnOnce() -> R + Send) -> R {
    larmorx_core::parallel::with_threads(threads, f).unwrap()
}

/// The classes' Gaussians from their statistics.
fn gaussians(s: &[(f32, f32)], libm: &Libm) -> Vec<Gauss> {
    s.iter().map(|&(m, v)| Gauss::new(m, v, libm)).collect()
}

/// `-v`'s line of the classes' means and standard deviations (§4.10).
fn class_line(out: &mut dyn Write, s: &[(f32, f32)], libm: &Libm) {
    let mut line = String::new();
    for (c, &(m, v)) in s.iter().enumerate() {
        let sd = v.sqrt();
        let mean = libm.expf(m);
        let spread = (libm.expf(m + sd) - libm.expf(m - sd)) / 2.0;
        line.push_str(&format!(
            " CLASS {} MEAN {} STDDEV {}",
            c + 1,
            g(f64::from(mean)),
            g(f64::from(spread))
        ));
    }
    let _ = writeln!(out, "{line}");
}

/// One bias field estimate (§4.5): `b` per mask voxel, from the uncorrected log image `y`, and
/// its value outside the mask (`(0 - mean) · 0`: NaN when the mean is not finite).
fn bias_field(
    grid: &Grid,
    post: &[f32],
    y: &[f32],
    s: &[(f32, f32)],
    kernels: &[Vec<f64>; 3],
    len: usize,
) -> (Vec<f32>, f32) {
    let n = s.len();
    // each mask voxel's weight and residual (in parallel), then over the image
    let wr: Vec<(f32, f32)> = (0..grid.len())
        .into_par_iter()
        .map(|i| {
            let (mut ws, mut rs) = (0f32, 0f32);
            for (c, &(m, var)) in s.iter().enumerate() {
                let wc = post[i * n + c] / var;
                ws += wc;
                rs += wc * (y[i] - m);
            }
            (ws, rs)
        })
        .collect();
    let w = grid.expand(|i| wr[i].0, 0.0);
    let r = grid.expand(|i| wr[i].1, 0.0);
    drop(wr);
    let k = [&kernels[0][..], &kernels[1][..], &kernels[2][..]];
    let w = bias::smooth(w, grid.shape, k);
    let r = bias::smooth(r, grid.shape, k);
    let mut b: Vec<f32> = grid
        .index
        .par_iter()
        .map(|&idx| r[idx as usize] / w[idx as usize])
        .collect();
    let mean = (stats::sum_par(b.len(), len, |i| f64::from(b[i])) / (b.len().max(1) as f64)) as f32;
    b.par_iter_mut().for_each(|x| *x -= mean);
    (b, (0.0 - mean) * 0.0)
}

/// The bias field's kernels for one channel (§4.5): `σ = 0.51 l / d` voxels along each axis.
pub fn kernels(lowpass: f32, voxel: [f32; 3], libm: &Libm) -> [Vec<f64>; 3] {
    voxel.map(|d| bias::kernel(((0.51 * f64::from(lowpass)) / f64::from(d)) as f32, libm))
}

/// Segments one channel. `data` is the image after §4.1's negative-value rule (radiological
/// frame, x fastest), `in_mask` its mask; messages go to `out` (standard output).
pub fn segment(
    data: &[f32],
    shape: [usize; 3],
    voxel: [f32; 3],
    in_mask: impl Fn(usize) -> bool,
    p: &Params,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Single, Stop> {
    let grid = Grid::new(shape, voxel, in_mask).ok_or(Stop::TooLarge)?;
    let n = p.n_classes;
    let nm = grid.len();
    let len = stats::block_len(grid.voxels());
    let y: Vec<f32> = grid
        .index
        .iter()
        .map(|&i| log1(data[i as usize], &p.libm))
        .collect();
    // FSL's masked sums over an empty mask (each prints a message)
    let mut sums = 0usize;
    let mut post = vec![0f32; (nm + 1) * n];
    let stats_now: Vec<(f32, f32)>;
    let mut init_members: Option<Vec<f32>> = None;
    let pri = p.priors.as_ref().map(|pr| prior_rows(&grid, pr, n));
    if let Some(pr) = &pri {
        // §6.6: the initialisation from the priors, no iterations
        if p.verbose {
            let _ = writeln!(out, "Beginning prior-based initialisation");
        }
        post[..nm * n].copy_from_slice(pr);
        stats_now = stats::class_stats(&post[..nm * n], &y, n, len);
        sums += 4 * n;
    } else {
        stats_now = initialise(
            p,
            &y,
            n,
            nm,
            len,
            &mut post,
            &mut init_members,
            &mut sums,
            out,
        )?;
    }
    finish(
        p,
        grid,
        y,
        len,
        post,
        stats_now,
        init_members,
        pri,
        sums,
        voxel,
        out,
        err,
    )
}

/// §4.3: the initial means, the hard assignment and the initialisation iterations; the class
/// statistics of the last.
#[allow(clippy::too_many_arguments)]
fn initialise(
    p: &Params,
    y: &[f32],
    n: usize,
    nm: usize,
    len: usize,
    post: &mut [f32],
    init_members: &mut Option<Vec<f32>>,
    sums: &mut usize,
    out: &mut dyn Write,
) -> Result<Vec<(f32, f32)>, Stop> {
    let means: Vec<f32> = match &p.manual {
        Some(s) if s.len() == n => s.iter().map(|&v| p.libm.logf(v)).collect(),
        _ => {
            let sorted = stats::sorted(y.to_vec());
            let step = 1.0 / (n as f32 + 1.0);
            (1..=n)
                .map(|c| stats::percentile_of_sorted(&sorted, c as f32 * step))
                .collect()
        }
    };
    if p.verbose {
        for (c, m) in means.iter().enumerate() {
            let _ = writeln!(out, "{} {}", c + 1, g(f64::from(*m)));
        }
    }
    for a in 0..n {
        for b in a + 1..n {
            if means[a] == means[b] {
                return Err(Stop::KMeans);
            }
        }
    }
    let mut stats_now: Vec<(f32, f32)> = means.iter().map(|&m| (m, 0.0)).collect();
    // hard assignment to the nearest mean
    post[..nm * n]
        .par_chunks_mut(n)
        .zip(y.par_iter())
        .for_each(|(row, &yi)| {
            let (mut best, mut low) = (0usize, 1e31f32);
            for (c, &m) in means.iter().enumerate() {
                let d = (yi - m) * (yi - m);
                if d < low {
                    low = d;
                    best = c;
                }
            }
            row[best] = 1.0;
        });
    // the initialisation iterations: -O plus -W of them
    let init = (p.fixed_iterations + p.init_iterations).max(0);
    for it in 0..init {
        if p.verbose {
            let _ = writeln!(out, "KMeans Iteration {it}");
        }
        if !p.pve && p.probabilities && it == init - 1 {
            *init_members = Some(members(post, n, nm));
        }
        stats_now = stats::class_stats(&post[..nm * n], y, n, len);
        *sums += 4 * n;
        let gs = gaussians(&stats_now, &p.libm);
        post[..nm * n]
            .par_chunks_mut(n)
            .zip(y.par_iter())
            .for_each(|(row, &yi)| {
                let mut s = 0f32;
                for (pc, c) in row.iter_mut().zip(&gs) {
                    let l = p.libm.expf(-c.neg_log(yi));
                    *pc = if l < 0.0 { 0.0 } else { l };
                    s += *pc;
                }
                if s > 0.0 {
                    for pc in row.iter_mut() {
                        *pc /= s;
                    }
                }
            });
    }
    Ok(stats_now)
}

/// §4.6 to §4.9: the main loop, the class order, the hard segmentation, the partial volumes.
#[allow(clippy::too_many_arguments)]
fn finish(
    p: &Params,
    grid: Grid,
    y: Vec<f32>,
    len: usize,
    post: Vec<f32>,
    mut stats_now: Vec<(f32, f32)>,
    init_members: Option<Vec<f32>>,
    pri: Option<Vec<f32>>,
    mut sums: usize,
    voxel: [f32; 3],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Single, Stop> {
    let n = p.n_classes;
    let nm = grid.len();
    let kernels = kernels(p.lowpass, voxel, &p.libm);
    let mut bias: Option<Vec<f32>> = None;
    let bias_outside = std::cell::Cell::new(0f32);
    let mut yb = y.clone();
    let estimate = |post: &[f32], s: &[(f32, f32)], yb: &mut Vec<f32>| -> Vec<f32> {
        let (b, outside) = bias_field(&grid, post, &y, s, &kernels, len);
        bias_outside.set(outside);
        yb.par_iter_mut()
            .zip(y.par_iter())
            .zip(b.par_iter())
            .for_each(|((v, &y0), &bi)| *v = y0 - bi);
        b
    };
    if !p.no_bias {
        bias = Some(estimate(&post, &stats_now, &mut yb));
        sums += 1;
    }
    for &(m, v) in &stats_now {
        if m.is_nan() || v.is_nan() {
            let _ = writeln!(out, "MeaNsK variance nan");
        }
    }
    let mut mrf = Mrf {
        grid: &grid,
        n,
        post,
        weights: grid.weights.map(f64::from),
        libm: &p.libm,
    };
    let mut rng = GlibcRand::new(FAST_SEED);
    let pr = pri.as_deref().filter(|_| p.priors_throughout);
    let mut beta = p.f_hard;
    for it in 0..p.bias_iterations.max(0) {
        if p.verbose {
            let _ = writeln!(
                out,
                "Tanaka Iteration {it} bias field {}",
                p.bias_iterations
            );
        }
        mrf.iterate(
            &yb,
            &gaussians(&stats_now, &p.libm),
            beta,
            &mut rng,
            pr,
            out,
            p.verbose,
        );
        if p.verbose {
            class_line(out, &stats_now, &p.libm);
        }
        if !p.no_bias {
            bias = Some(estimate(&mrf.post, &stats_now, &mut yb));
            sums += 1;
        }
        stats_now = stats::class_stats(&mrf.post[..nm * n], &yb, n, len);
        sums += 4 * n;
    }
    for it in 0..p.fixed_iterations.max(0) {
        if p.verbose {
            let _ = writeln!(
                out,
                "Tanaka Iteration {it} hyperparameter {}",
                p.fixed_iterations
            );
        }
        mrf.iterate(
            &yb,
            &gaussians(&stats_now, &p.libm),
            beta,
            &mut rng,
            pr,
            out,
            p.verbose,
        );
        if p.verbose {
            class_line(out, &stats_now, &p.libm);
        }
        beta = if p.hyper.is_nan() {
            // FSL-99 (reproduce): a NaN -H passes FAST's check for a negative value and β is
            // estimated against a table of zeros (observed: the lines below, then β = 0)
            let total = mrf.weights_total() as f32;
            let _ = writeln!(
                out,
                " TOTAL {} RHS {}",
                g(f64::from(total)),
                "0 ".repeat(15)
            );
            for k in 1..=14 {
                let v = g(f64::from(k as f32 * 0.05));
                let _ = writeln!(err, "{v} {v}");
            }
            0.0
        } else {
            p.hyper
        };
        if p.verbose {
            let _ = writeln!(out, " BETA {}", g(f64::from(beta)));
        }
        stats_now = stats::class_stats(&mrf.post[..nm * n], &yb, n, len);
        sums += 4 * n;
    }
    let post = mrf.post;
    // §4.7: the classes sorted by their means (descending for -t 2)
    let mut order: Vec<f32> = stats_now.iter().map(|s| s.0).collect();
    order.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if p.image_type == 2 {
        order.reverse();
    }
    let perm: Vec<usize> = order
        .iter()
        .enumerate()
        .map(|(k, &m)| (0..n).rev().find(|&j| stats_now[j].0 == m).unwrap_or(k))
        .collect();
    let mut sorted_post = vec![0f32; nm * n];
    sorted_post
        .par_chunks_mut(n)
        .zip(post[..nm * n].par_chunks(n))
        .for_each(|(dst, src)| {
            for (k, &j) in perm.iter().enumerate() {
                dst[k] = src[j];
            }
        });
    drop(post);
    let seg: Vec<u8> = sorted_post
        .par_chunks(n)
        .map(|row| {
            let (mut best, mut high) = (0u8, -1e-10f32);
            for (c, &v) in row.iter().enumerate() {
                if v > high {
                    high = v;
                    best = (c + 1) as u8;
                }
            }
            best
        })
        .collect();
    let vox = f64::from(voxel[0]) * f64::from(voxel[1]) * f64::from(voxel[2]);
    let (members_out, pve, volumes) = if p.pve {
        if p.verbose {
            let _ = writeln!(out, " Starting Partial Volume Estimation ");
        }
        let q = members(&sorted_post, n, nm);
        let x: Vec<f32> = yb.par_iter().map(|&v| p.libm.expf(v)).collect();
        let r = pve::estimate(&grid, &sorted_post, &x, n, p.mixel, len, &p.libm);
        sums += 5 * n;
        let volumes: Vec<f64> = (0..n)
            .map(|c| stats::sum_par(nm, len, |i| f64::from(r.frac[i * n + c])) * vox)
            .collect();
        (p.probabilities.then_some(q), Some(r), volumes)
    } else {
        sums += n;
        let volumes: Vec<f64> = (0..n)
            .map(|c| stats::sum_par(nm, len, |i| f64::from(sorted_post[i * n + c])) * vox)
            .collect();
        (init_members, None, volumes)
    };
    if p.verbose {
        let mut total = 0f64;
        for (c, v) in volumes.iter().enumerate() {
            let _ = write!(out, "\n tissue {} {}", c + 1, g(*v));
            total += v;
        }
        let _ = writeln!(out, "\n total tissue {}", g(total));
    }
    Ok(Single {
        grid,
        n,
        seg,
        members: members_out,
        pve,
        bias,
        bias_outside: bias_outside.get(),
        volumes,
        empty_sums: if nm == 0 { sums } else { 0 },
    })
}
