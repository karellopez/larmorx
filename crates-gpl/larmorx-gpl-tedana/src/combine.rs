// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! `tedana.combine.make_optcom`: the optimally combined series, a weighted average over each
//! voxel's good echoes (two for a voxel with one), per volume:
//!
//! - `t2s` (Posse et al. 1999): weights `TE · exp(-TE / T2*)` from the full T2* map (per volume
//!   for `fitmode ts`, where weights that are all zero become ones);
//! - `paid` (Poser et al. 2006): weights `TE · mean / std` of each echo's series (0 where the
//!   standard deviation is 0).
//!
//! numpy's `average(data, axis=1, weights=w)`: the weights' sum and the sum of `data · w` in
//! float64, each in numpy's order (in index order along the echoes; the weights' sum pairwise
//! for `fitmode ts`, whose weights numpy holds echo-innermost; pairwise for both with a single
//! volume), then their quotient; a zero sum of weights anywhere in a group of voxels is numpy's
//! `ZeroDivisionError` for the whole call.
//!
//! Defect TED-15: with `paid`, a voxel whose good echoes all have a temporal standard deviation
//! of 0 (a constant voxel; every voxel of a one-volume series) has weights summing to 0, so
//! numpy's `ZeroDivisionError` stops t2smap. By default a constant voxel gets equal weights (as
//! tedana gives all-zero `t2s` weights in `fitmode ts`), and a one-volume series is refused
//! with a clear message (PAID needs the series' variation).

use larmorx_core::defects::{Notices, TED_15};
use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::reduce::{mean_axis, pairwise_sum, std_axis};
use rayon::prelude::*;

use crate::decay::Libraries;
use crate::log::{Log, PyError, Site};
use crate::maths::Maths;

/// How the echoes are combined (`--combmode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombMode {
    T2s,
    Paid,
}

/// The order of one of numpy's sums along the echoes.
#[derive(Clone, Copy)]
enum Order {
    InOrder,
    Pairwise,
}

fn sum(values: &[f64], order: Order) -> f64 {
    match order {
        Order::InOrder => values.iter().fold(0.0f64, |s, &x| s.add(x)),
        Order::Pairwise => 0.0f64.add(pairwise_sum(values, 0, values.len(), 1)),
    }
}

/// `echos_to_run` and the voxels of each: counts present, 2 added when 1 is, those `>= 2`;
/// for 2 the voxels with one or two good echoes.
fn groups(am: &[i64]) -> Vec<(usize, Vec<usize>)> {
    let mut u: Vec<i64> = am.to_vec();
    u.sort_unstable();
    u.dedup();
    if u.contains(&1) && !u.contains(&2) {
        u.push(2);
        u.sort_unstable();
    }
    u.into_iter()
        .filter(|&e| e >= 2)
        .map(|e| {
            let idx = (0..am.len())
                .filter(|&v| {
                    if e == 2 {
                        am[v] > 0 && am[v] <= 2
                    } else {
                        am[v] == e
                    }
                })
                .collect();
            (e as usize, idx)
        })
        .collect()
}

/// `make_optcom(data, tes, adaptive_mask, t2s=t2s, combmode=combmode)`: `data` the
/// `(voxels, echoes, volumes)` float32 array, `am` per voxel, `t2s` the full map in ms
/// (C-ordered `(voxels, cols)`, `cols` 1 or the volumes; unused for `paid`; `volume_wise`: a map
/// per volume, `fitmode ts`, even of one volume). The combined
/// series `(voxels, volumes)`, float64 rounded to float32 (as t2smap saves it).
#[allow(clippy::too_many_arguments)]
pub fn make_optcom(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    tes: &[f64],
    am: &[i64],
    t2s: &[f64],
    cols: usize,
    volume_wise: bool,
    mode: CombMode,
    maths: &Maths,
    libraries: Libraries,
    n_threads: usize,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<(Vec<f32>, bool), PyError> {
    let place = "tedana/combine.py, in make_optcom";
    let n_samples = am.len();
    if tes.len() != n_echos {
        return Err(PyError::value_error(
            format!(
                "Number of echos provided does not match second dimension of input data: {} != \
                 {n_echos}",
                tes.len()
            ),
            place,
        ));
    }
    match mode {
        CombMode::Paid => {
            // t2smap always passes the T2* map.
            log.warning(
                Site("combine", "make_optcom", 181),
                "Argument 't2s' is not required if 'combmode' is 'paid'. 't2s' array will not be \
                 used.",
            );
            log.info(
                Site("combine", "make_optcom", 187),
                "Optimally combining data with parallel-acquired inhomogeneity desensitized \
                 (PAID) method",
            );
        }
        CombMode::T2s => log.info(
            Site("combine", "make_optcom", 202),
            // tedana: `t2s.ndim == 1`; `fitmode ts` gives a 2D map even for one volume.
            if !volume_wise {
                "Optimally combining data with voxel-wise T2* estimates"
            } else {
                "Optimally combining data with voxel- and volume-wise T2* estimates"
            },
        ),
    }
    let mut combined = vec![0.0f32; n_samples * n_vols];
    // a finite float64 value beyond float32's range (numpy's cast warning when it is written)
    let mut overflows = false;
    for (n, idx) in groups(am) {
        debug_assert!(n <= n_echos, "adaptive-mask counts never exceed the echoes");
        // weights per voxel: (n, cols_w) C-ordered, cols_w = 1 (tiled over volumes) or n_vols
        let (wcols, order_w, order_p) = match (mode, cols) {
            (CombMode::T2s, c) if c > 1 => (n_vols, Order::Pairwise, Order::InOrder),
            _ => (1, Order::InOrder, Order::InOrder),
        };
        let (order_w, order_p) = if n_vols == 1 {
            (Order::Pairwise, Order::Pairwise)
        } else {
            (order_w, order_p)
        };
        let weights: Vec<Vec<f64>> = idx
            .par_iter()
            .map(|&v| {
                let rows = &data[v * n_echos * n_vols..(v * n_echos + n) * n_vols];
                match mode {
                    CombMode::T2s => {
                        let mut w = vec![0.0f64; n * wcols];
                        for c in 0..wcols {
                            let t = t2s[v * cols + c.min(cols - 1)];
                            for e in 0..n {
                                w[e * wcols + c] = tes[e].mul(maths.exp((-tes[e]).div(t)));
                            }
                            if wcols > 1 && (0..n).all(|e| w[e * wcols + c] == 0.0) {
                                for e in 0..n {
                                    w[e * wcols + c] = 1.0;
                                }
                            }
                        }
                        w
                    }
                    CombMode::Paid => {
                        let mean = mean_axis(rows, [n, n_vols, 1]);
                        let std = std_axis(rows, [n, n_vols, 1], 0);
                        (0..n)
                            .map(|e| {
                                let snr = if std[e] != 0.0 {
                                    mean[e].div(std[e]).to_f64()
                                } else {
                                    0.0
                                };
                                snr.mul(tes[e])
                            })
                            .collect()
                    }
                }
            })
            .collect();
        if mode == CombMode::Paid {
            paid_warnings(data, n_echos, n_vols, n, &idx, libraries, log);
        }
        // numpy checks every sum of weights of the group before averaging.
        let mut zero = Vec::new();
        for (k, w) in weights.iter().enumerate() {
            for c in 0..wcols {
                let col: Vec<f64> = (0..n).map(|e| w[e * wcols + c]).collect();
                if sum(&col, order_w) == 0.0 {
                    zero.push(k);
                    break;
                }
            }
        }
        let mut weights = weights;
        if !zero.is_empty() {
            if mode == CombMode::Paid && n_vols > 1 {
                notices.add(
                    &TED_15,
                    format!(
                        "{} voxel(s) whose good echoes do not vary over time combined with equal \
                         weights; tedana fails with ZeroDivisionError",
                        zero.len()
                    ),
                );
            } else if mode == CombMode::Paid {
                notices.add(&TED_15, "a one-volume series with --combmode paid");
            }
            if notices.reproduce() || mode != CombMode::Paid {
                return Err(PyError::new(
                    "ZeroDivisionError",
                    "Weights sum to zero, can't be normalized",
                    "numpy/lib/_function_base_impl.py, in average",
                ));
            }
            if n_vols == 1 {
                return Err(PyError::value_error(
                    "--combmode paid needs more than one volume: its weights are each echo's \
                     temporal signal-to-noise ratio (tedana fails with ZeroDivisionError)",
                    place,
                ));
            }
            for &k in &zero {
                weights[k].fill(1.0);
            }
        }
        let out: Vec<(usize, Vec<f32>, bool)> =
            larmorx_core::parallel::with_threads(n_threads, || {
                idx.par_iter()
                    .zip(weights.par_iter())
                    .map(|(&v, w)| {
                        let rows = &data[v * n_echos * n_vols..(v * n_echos + n) * n_vols];
                        let mut res = vec![0.0f32; n_vols];
                        let mut wv = vec![0.0f64; n];
                        let mut pv = vec![0.0f64; n];
                        let mut scl_const = None;
                        let mut over = false;
                        for (t, r) in res.iter_mut().enumerate() {
                            let c = if wcols == 1 { 0 } else { t };
                            for e in 0..n {
                                wv[e] = w[e * wcols + c];
                                pv[e] = rows[e * n_vols + t].to_f64().mul(wv[e]);
                            }
                            let scl = match scl_const {
                                Some(s) => s,
                                None => {
                                    let s = sum(&wv, order_w);
                                    if wcols == 1 {
                                        scl_const = Some(s);
                                    }
                                    s
                                }
                            };
                            let value = sum(&pv, order_p).div(scl);
                            *r = f32::from_f64(value);
                            over |= value.is_finite() && r.is_infinite();
                        }
                        (v, res, over)
                    })
                    .collect()
            })
            .expect("a thread pool");
        for (v, res, over) in out {
            combined[v * n_vols..(v + 1) * n_vols].copy_from_slice(&res);
            overflows |= over;
        }
    }
    Ok((combined, overflows))
}

/// numpy's floating-point warnings of PAID's `data.mean(axis=-1)` and `data.std(axis=-1)` over
/// one group's series (each warning once per place): a series whose pairwise sum adds +inf and
/// -inf (both infinities, unless a NaN reached the sums first: `crate::adaptive::
/// sum_adds_opposite_infinities`) raises `invalid value encountered in reduce`, in the mean and
/// in the variance's mean; a series whose mean is infinite subtracts it from its infinite samples
/// of that sign to NaN (`invalid value encountered in subtract`).
fn paid_warnings(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    n: usize,
    idx: &[usize],
    libraries: Libraries,
    log: &mut Log<'_>,
) {
    let (mut both, mut single) = (false, false);
    for &v in idx {
        for e in 0..n {
            let row = &data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols];
            if !row.iter().any(|x| x.is_infinite()) {
                continue;
            }
            both |= crate::adaptive::sum_adds_opposite_infinities(row, true);
            let mean = mean_axis(row, [1, n_vols, 1])[0];
            single |= mean.is_infinite() && row.contains(&mean);
        }
    }
    if both {
        log.warn(
            "numpy/_core/_methods.py",
            libraries.methods_mean_sum_line(),
            "RuntimeWarning",
            "invalid value encountered in reduce",
            "ret = umr_sum(arr, axis, dtype, out, keepdims, where=where)",
        );
        log.warn(
            "numpy/_core/_methods.py",
            libraries.methods_var_sum_line(),
            "RuntimeWarning",
            "invalid value encountered in reduce",
            "arrmean = umr_sum(arr, axis, dtype, keepdims=True, where=where)",
        );
    }
    if single {
        let (line, source) = libraries.methods_var_subtract();
        log.warn(
            "numpy/_core/_methods.py",
            line,
            "RuntimeWarning",
            "invalid value encountered in subtract",
            source,
        );
    }
}
