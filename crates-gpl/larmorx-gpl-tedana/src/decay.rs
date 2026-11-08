// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! `tedana.decay`'s fits of the monoexponential decay `S(TE) = S0 · exp(-TE / T2*)`:
//!
//! - `fit_loglinear`: for each adaptive-mask count `n` (a count of 1 fitted with two echoes),
//!   one least-squares problem for all voxels of that count, `log(|S| + 1)` (float32) against
//!   `[1, -TE]` repeated per volume: numpy's `lstsq` (LAPACK `dgelsd` with OpenBLAS's kernels,
//!   `larmorx_transform::numpy_linalg::lstsq`, one thread's result at any thread count);
//!   `T2* = 1 / slope`, `S0 = exp(intercept)`.
//! - `fit_monoexponential`: the log-linear estimates as starting values, then per voxel SciPy's
//!   `curve_fit` with bounds `((min(S), 0), (inf, inf))` (`larmorx_optim::curve_fit2`: the
//!   trust-region reflective method); a voxel whose fit raises (data not finite, a start outside
//!   the bounds, the evaluation budget spent) keeps its log-linear estimate and is a failure.
//! - `fit_decay`, `fit_decay_ts`: the fit over all volumes, or per volume.
//!
//! Defects (D13; `reproduce` gives tedana's results): TED-07, the `--verbose` variance maps are
//! swapped (tedana writes the variance of S0 as T2*'s and the reverse); TED-08, a failure (and
//! the variances) of a voxel of count 1, fitted with two echoes, is not reported; TED-09, an
//! infinite sample in one voxel makes every voxel of its count NaN in the log-linear fit (one
//! least-squares problem for all of them); by default that voxel alone is NaN.

use larmorx_core::defects::{Notices, TED_09};
use larmorx_core::math::{self, Libm};
use larmorx_core::numpy::umath;
use larmorx_core::parallel;
use larmorx_optim::{Scipy, curve_fit2};
use larmorx_transform::numpy_linalg::{LapackRelease, lstsq};
use rayon::prelude::*;

use crate::log::{Log, PyError, Site};
use crate::parser::FitType;

/// The numpy and SciPy whose arithmetic the fits follow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Libraries {
    /// numpy 2.5.4 (LAPACK 3.12.1) and SciPy 1.18.1: tedana 26.0.3's newest environment.
    #[default]
    Newest,
    /// numpy 2.3.5 (LAPACK 3.12.0) and SciPy 1.15.3: the shared environment.
    Shared,
}

impl Libraries {
    pub fn lapack(self) -> LapackRelease {
        match self {
            Libraries::Newest => LapackRelease::V3_12_1,
            Libraries::Shared => LapackRelease::V3_12_0,
        }
    }
    fn scipy(self) -> Scipy {
        match self {
            Libraries::Newest => Scipy::V1_18_1,
            Libraries::Shared => Scipy::V1_15_3,
        }
    }
}

/// What a fit runs with.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub libraries: Libraries,
    /// Threads for the per-voxel fits (0: all logical CPUs); the results do not depend on it.
    pub n_threads: usize,
}

/// The maths of a mode: numpy's own (`reproduce`) or correctly rounded (the default).
fn log32(reproduce: bool, x: f32) -> f32 {
    if reproduce {
        umath::log_f32(x)
    } else {
        math::logf(x)
    }
}

fn exp(reproduce: bool, x: f64) -> f64 {
    if reproduce {
        umath::exp_f64(x)
    } else {
        math::exp(x)
    }
}

/// The fit's maps, over the fitted voxels (`(voxels,)`, or `(voxels, volumes)` C-ordered for
/// `fit_decay_ts`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fit {
    pub t2s: Vec<f64>,
    pub s0: Vec<f64>,
    /// `curvefit` only.
    pub failures: Option<Vec<bool>>,
    /// The variances and covariance of `curve_fit`'s estimates (`curvefit` only), named by what
    /// they hold (TED-07: tedana writes them swapped; `reproduce` swaps them back on output).
    pub t2s_var: Option<Vec<f64>>,
    pub s0_var: Option<Vec<f64>>,
    pub covar: Option<Vec<f64>>,
    /// Voxels of count 1 (fitted with two echoes) whose `curve_fit` failed, and those it fitted
    /// (TED-08: tedana reports neither in the failures and variance maps).
    pub count1_failed: usize,
    pub count1_fitted: usize,
}

/// `echos_to_run`: the counts present, 2 added when 1 is, those `>= 2`.
fn echos_to_run(am: &[i64]) -> Vec<i64> {
    let mut u: Vec<i64> = am.to_vec();
    u.sort_unstable();
    u.dedup();
    if u.contains(&1) && !u.contains(&2) {
        u.push(2);
        u.sort_unstable();
    }
    u.retain(|&e| e >= 2);
    u
}

fn group(am: &[i64], echo_num: i64, loglin: bool) -> Vec<usize> {
    (0..am.len())
        .filter(|&v| {
            if echo_num == 2 {
                (!loglin || am[v] > 0) && am[v] <= 2
            } else {
                am[v] == echo_num
            }
        })
        .collect()
}

/// `fit_loglinear(data_cat, echo_times, adaptive_mask)`: `(t2s, s0)` over the voxels.
#[allow(clippy::too_many_arguments)]
pub fn fit_loglinear(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    tes: &[f64],
    am: &[i64],
    ctx: &Context,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<(Vec<f64>, Vec<f64>), PyError> {
    let md = am.len();
    let mut t2s = vec![0.0f64; md];
    let mut s0 = vec![0.0f64; md];
    let reproduce = notices.reproduce();
    for echo_num in echos_to_run(am) {
        let n = echo_num as usize;
        let idx = group(am, echo_num, true);
        if n > n_echos || n > tes.len() {
            // np.column_stack([np.ones(n), echo_times[:n]]) of unequal lengths.
            return Err(PyError::value_error(
                format!(
                    "all the input array dimensions except for the concatenation axis must \
                     match exactly, but along dimension 0, the array at index 0 has size {n} and \
                     the array at index 1 has size {}",
                    n.min(tes.len())
                ),
                "tedana/decay.py, in fit_loglinear",
            ));
        }
        let m = n * n_vols;
        let a: Vec<f64> = (0..m)
            .flat_map(|r| [1.0, -tes[r / n_vols.max(1)]])
            .collect();
        // B = log(|data| + 1) as float64, column-major (one column per voxel, rows e·T + t).
        let column = |v: usize, col: &mut [f64]| {
            for e in 0..n {
                let src = &data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols];
                for (t, &x) in src.iter().enumerate() {
                    col[e * n_vols + t] = f64::from(log32(reproduce, x.abs() + 1.0));
                }
            }
        };
        // TED-09: a column holding an infinite logarithm (an infinite sample) is solved apart.
        let mut solve_idx = idx.clone();
        let mut alone = Vec::new();
        if !reproduce {
            let infinite = |v: usize| {
                (0..n).any(|e| {
                    data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols]
                        .iter()
                        .any(|x| x.is_infinite())
                })
            };
            let (bad, good): (Vec<usize>, Vec<usize>) = idx.iter().partition(|&&v| infinite(v));
            if !bad.is_empty() {
                notices.add(
                    &TED_09,
                    format!(
                        "{} voxel(s) with an infinite sample fitted alone (NaN); tedana's \
                         {n}-echo fit makes all {} voxels of that count NaN",
                        bad.len(),
                        idx.len()
                    ),
                );
                alone = bad;
                solve_idx = good;
            }
        } else if idx.iter().any(|&v| {
            (0..n).any(|e| {
                data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols]
                    .iter()
                    .any(|x| x.is_infinite())
            })
        }) {
            notices.add(
                &TED_09,
                format!(
                    "an infinite sample makes all {} voxels of the {n}-echo fit NaN",
                    idx.len()
                ),
            );
        }
        for &v in &alone {
            t2s[v] = f64::NAN;
            s0[v] = f64::NAN;
        }
        if solve_idx.is_empty() {
            continue;
        }
        let nrhs = solve_idx.len();
        let mut b = vec![0.0f64; m * nrhs];
        b.par_chunks_mut(m.max(1))
            .zip(solve_idx.par_iter())
            .for_each(|(col, &v)| column(v, col));
        let sol = lstsq(
            &a,
            m,
            2,
            &mut b,
            nrhs,
            None,
            ctx.libraries.lapack(),
            ctx.n_threads,
        )
        .map_err(|e| {
            PyError::new(
                "numpy.linalg.LinAlgError",
                e.to_string(),
                "numpy/linalg/_linalg.py, in lstsq",
            )
        })?;
        let mut div0 = false;
        let mut overflow = false;
        for (j, &v) in solve_idx.iter().enumerate() {
            let (b0, b1) = (sol.x[j], sol.x[nrhs + j]);
            if b1 == 0.0 {
                div0 = true;
            }
            t2s[v] = 1.0 / b1;
            let e = exp(reproduce, b0);
            if e.is_infinite() && b0.is_finite() {
                overflow = true;
            }
            s0[v] = e;
        }
        if div0 {
            log.warn(
                "tedana/decay.py",
                371,
                "RuntimeWarning",
                "divide by zero encountered in divide",
                "t2s = 1.0 / betas[1, :].T",
            );
        }
        if overflow {
            log.warn(
                "tedana/decay.py",
                372,
                "RuntimeWarning",
                "overflow encountered in exp",
                "s0 = np.exp(betas[0, :]).T",
            );
        }
    }
    Ok((t2s, s0))
}

/// numpy's `np.min` of a float32 column: the smallest value, NaN if any is NaN.
fn np_min(values: impl Iterator<Item = f32>) -> f32 {
    let mut m = f32::INFINITY;
    let mut first = true;
    for x in values {
        if x.is_nan() {
            return x;
        }
        if first || x < m {
            m = x;
            first = false;
        }
    }
    m
}

/// One voxel's `curve_fit`: `Ok((s0, t2s, pcov))` or a failure.
type VoxelFit = Option<([f64; 2], [[f64; 2]; 2])>;

/// `fit_monoexponential(data_cat, echo_times, adaptive_mask, n_threads=n_threads)`.
#[allow(clippy::too_many_arguments)]
pub fn fit_monoexponential(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    tes: &[f64],
    am: &[i64],
    ctx: &Context,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Fit, PyError> {
    let md = am.len();
    let (mut t2s_init, mut s0_init) =
        fit_loglinear(data, n_echos, n_vols, tes, am, ctx, log, notices)?;
    let reproduce = notices.reproduce();
    let libm = Libm::quiet(notices.mode());
    let scipy = ctx.libraries.scipy();
    let mut t2s = vec![0.0f64; md];
    let mut s0 = vec![0.0f64; md];
    let mut failures = vec![false; md];
    let mut t2s_var = vec![0.0f64; md];
    let mut s0_var = vec![0.0f64; md];
    let mut covar = vec![0.0f64; md];
    let (mut count1_failed, mut count1_fitted) = (0usize, 0usize);
    for echo_num in echos_to_run(am) {
        let n = (echo_num as usize).min(n_echos).min(tes.len());
        let idx = group(am, echo_num, false);
        let x: Vec<f64> = (0..n * n_vols).map(|r| tes[r / n_vols.max(1)]).collect();
        let bar = log.progress(&format!("{echo_num}-echo monoexponential"), idx.len());
        let fit_one = |v: usize| -> VoxelFit {
            let y: Vec<f64> = (0..n)
                .flat_map(|e| {
                    data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols]
                        .iter()
                        .map(|&s| f64::from(s))
                })
                .collect();
            let lb0 = f64::from(np_min((0..n).flat_map(|e| {
                data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols]
                    .iter()
                    .copied()
            })));
            // The model's exp once per echo time (the same arguments give the same bits).
            let mut model = |_x: &[f64], p: &[f64; 2], out: &mut [f64]| {
                for e in 0..n {
                    let val = p[0] * exp(reproduce, -tes[e] / p[1]);
                    out[e * n_vols..(e + 1) * n_vols].fill(val);
                }
            };
            curve_fit2(
                &mut model,
                &x,
                &y,
                [s0_init[v], t2s_init[v]],
                [lb0, 0.0],
                [f64::INFINITY, f64::INFINITY],
                scipy,
                &libm,
            )
            .ok()
            .map(|r| (r.popt, r.pcov))
        };
        let results: Vec<VoxelFit> = parallel::with_threads(ctx.n_threads, || {
            idx.par_iter().map(|&v| fit_one(v)).collect()
        })
        .map_err(|e| PyError::new("RuntimeError", e.to_string(), "joblib Parallel"))?;
        log.progress_done(bar);
        let mut fail_count = 0usize;
        for (&v, r) in idx.iter().zip(&results) {
            match r {
                None => fail_count += 1,
                Some((popt, _)) => {
                    s0_init[v] = popt[0];
                    t2s_init[v] = popt[1];
                }
            }
        }
        if fail_count > 0 {
            let total = idx.len();
            log.debug(Site("decay", "fit_monoexponential", 261), || {
                format!(
                    "With {echo_num} echoes, monoexponential fit failed on {fail_count}/{total} \
                     ({:.2}%) voxel(s), used log linear estimate instead",
                    100.0 * fail_count as f64 / total as f64
                )
            });
        }
        for (&v, r) in idx.iter().zip(&results) {
            t2s[v] = t2s_init[v];
            s0[v] = s0_init[v];
            if am[v] == 1 {
                if r.is_none() {
                    count1_failed += 1;
                } else {
                    count1_fitted += 1;
                }
                // TED-08: tedana keeps no failure or variance for a voxel of count 1.
                if reproduce {
                    continue;
                }
            }
            match r {
                None => failures[v] = true,
                Some((_, pcov)) => {
                    t2s_var[v] = pcov[1][1];
                    s0_var[v] = pcov[0][0];
                    covar[v] = pcov[0][1];
                }
            }
        }
    }
    Ok(Fit {
        t2s,
        s0,
        failures: Some(failures),
        t2s_var: Some(t2s_var),
        s0_var: Some(s0_var),
        covar: Some(covar),
        count1_failed,
        count1_fitted,
    })
}

/// `fit_decay(data, tes, adaptive_mask, fittype, n_threads=n_threads)`.
#[allow(clippy::too_many_arguments)]
pub fn fit_decay(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    tes: &[f64],
    am: &[i64],
    fittype: FitType,
    ctx: &Context,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Fit, PyError> {
    match fittype {
        FitType::Loglin => {
            let (t2s, s0) = fit_loglinear(data, n_echos, n_vols, tes, am, ctx, log, notices)?;
            Ok(Fit {
                t2s,
                s0,
                ..Fit::default()
            })
        }
        FitType::Curvefit => fit_monoexponential(data, n_echos, n_vols, tes, am, ctx, log, notices),
    }
}

/// `fit_decay_ts(data, tes, adaptive_mask, fittype, n_threads=n_threads)`: one fit per volume;
/// the maps are `(voxels, volumes)`, C-ordered.
#[allow(clippy::too_many_arguments)]
pub fn fit_decay_ts(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    tes: &[f64],
    am: &[i64],
    fittype: FitType,
    ctx: &Context,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Fit, PyError> {
    let md = am.len();
    let curve = fittype == FitType::Curvefit;
    let zeros = || vec![0.0f64; md * n_vols];
    let mut out = Fit {
        t2s: zeros(),
        s0: zeros(),
        failures: curve.then(|| vec![false; md * n_vols]),
        t2s_var: curve.then(zeros),
        s0_var: curve.then(zeros),
        covar: curve.then(zeros),
        ..Fit::default()
    };
    let mut vol = vec![0.0f32; md * n_echos];
    for t in 0..n_vols {
        for (k, slot) in vol.iter_mut().enumerate() {
            *slot = data[k * n_vols + t];
        }
        let f = fit_decay(&vol, n_echos, 1, tes, am, fittype, ctx, log, notices)?;
        let put = |dst: &mut Vec<f64>, src: &[f64]| {
            for v in 0..md {
                dst[v * n_vols + t] = src[v];
            }
        };
        put(&mut out.t2s, &f.t2s);
        put(&mut out.s0, &f.s0);
        out.count1_failed += f.count1_failed;
        out.count1_fitted += f.count1_fitted;
        if curve {
            let fl = f.failures.as_ref().expect("curvefit");
            let dst = out.failures.as_mut().expect("curvefit");
            for v in 0..md {
                dst[v * n_vols + t] = fl[v];
            }
            put(
                out.t2s_var.as_mut().expect("curvefit"),
                f.t2s_var.as_ref().expect("cf"),
            );
            put(
                out.s0_var.as_mut().expect("curvefit"),
                f.s0_var.as_ref().expect("cf"),
            );
            put(
                out.covar.as_mut().expect("curvefit"),
                f.covar.as_ref().expect("cf"),
            );
        }
    }
    Ok(out)
}
