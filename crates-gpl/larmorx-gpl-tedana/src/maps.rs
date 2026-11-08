// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! What `t2smap` makes of the fit: `tedana.decay.modify_t2s_s0_maps` (the floors and the cap,
//! the "full" and "limited" maps), `tedana.utils.unmask` and `tedana.decay.rmse_of_fit_decay_ts`
//! (the root mean squared error of the fit per voxel and its table per volume).
//!
//! - **Floors and caps.** Infinite T2* becomes 500, T2* `<= 0` becomes 1, a T2* so small that
//!   `exp(-TE / T2*)` is 0 for some echo is set to `-max(TE) / log(eps)` (`_apply_t2s_floor`,
//!   per volume for `fitmode ts`), S0 NaN becomes 0. The limited maps are copies with the
//!   voxels of one good echo set to 0; only the limited T2* gets the cap: values above ten
//!   times its 99.5th percentile (`scoreatpercentile(..., "lower")`, zeros included) are set to
//!   that percentile.
//! - **RMSE.** For each voxel with `n >= 2` good echoes, the predicted signal of the limited
//!   maps (`S0 · exp(-TE / T2*)` in float64, stored as float32) against the data of its first
//!   `n` echoes, every volume: `sqrt(mean over echoes of the squared residuals)` in float32.
//!   The map is the mean over volumes; the table per volume holds the mean, standard deviation
//!   and percentiles 0, 2, 25, 50, 75, 98, 100 over voxels (numpy's `nanpercentile`).
//!
//! Defects: TED-05, only the limited T2* map gets the cap, so the full map (from which the
//! combination is weighted, and which fMRIPrep keeps) holds values of up to days (measured: 2 to
//! 34 voxels of fMRIPrep's four multi-echo calls, up to 239,794 s): by default the full map gets
//! the same cap (as tedana 25.0.1 capped it). TED-14, the floor's debug message counts echoes
//! times voxels as voxels (`E · V`): by default it counts the voxels.

use larmorx_core::defects::{Notices, TED_05, TED_14};
use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::quantile::{
    Percentiles, nanpercentile_linear_axis0, scoreatpercentile_lower,
};
use larmorx_core::numpy::reduce::{mean_axis, nanstd_axis, nansum_axis};
use larmorx_core::python::number::format_fixed;
use rayon::prelude::*;

use crate::decay::Libraries;
use crate::log::{Log, Site};
use crate::maths::Maths;

/// The four maps over the fitted voxels, C-ordered `(voxels[, volumes])`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Maps {
    pub t2s_full: Vec<f64>,
    pub s0_full: Vec<f64>,
    pub t2s_limited: Vec<f64>,
    pub s0_limited: Vec<f64>,
}

/// `_apply_t2s_floor(t2s, echo_times)` of one column of voxels, in place.
fn apply_floor(
    t2s: &mut [f64],
    tes: &[f64],
    maths: &Maths,
    log: &mut Log<'_>,
    notices: &mut Notices,
) {
    let reproduce = notices.reproduce();
    let e = tes.len();
    let bad: Vec<bool> = t2s
        .iter()
        .map(|&t| t != 0.0 && tes.iter().any(|&te| maths.exp((-te).div(t)) == 0.0))
        .collect();
    let n_bad = bad.iter().filter(|&&b| b).count();
    if n_bad > 0 && log.enabled(crate::log::Level::Debug) {
        let n_vox = t2s.len();
        let counted = if reproduce { e * n_vox } else { n_vox };
        if e > 1 {
            notices.add(
                &TED_14,
                format!(
                    "the floor's message counts {n_vox} voxels; tedana prints {}",
                    e * n_vox
                ),
            );
        }
        let pct = (100 * n_bad) as f64 / counted as f64;
        log.debug(Site("decay", "_apply_t2s_floor", 58), || {
            format!(
                "T2* values for {n_bad}/{counted} voxels ({}%) have been identified as close to \
                 zero and have been adjusted",
                format_fixed(pct, 2)
            )
        });
    }
    let te_max = tes.iter().fold(f64::NEG_INFINITY, |m, &t| m.max(t));
    let floor = (-te_max).div(maths.log(f64::EPSILON));
    for (t, &b) in t2s.iter_mut().zip(&bad) {
        if b {
            *t = floor;
        }
    }
}

/// `modify_t2s_s0_maps(t2s, s0, adaptive_mask, tes)`: `t2s`, `s0` C-ordered `(voxels, cols)`
/// (`cols` 1 for `fitmode all`, the volumes for `ts`), `am` per voxel.
#[allow(clippy::too_many_arguments)]
pub fn modify_t2s_s0_maps(
    mut t2s: Vec<f64>,
    mut s0: Vec<f64>,
    am: &[i64],
    cols: usize,
    tes: &[f64],
    maths: &Maths,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Maps {
    for t in t2s.iter_mut() {
        if t.is_infinite() {
            *t = 500.0;
        }
        if *t <= 0.0 {
            *t = 1.0;
        }
    }
    if cols == 1 {
        apply_floor(&mut t2s, tes, maths, log, notices);
    } else {
        let n = am.len();
        for c in 0..cols {
            let mut col: Vec<f64> = (0..n).map(|v| t2s[v * cols + c]).collect();
            apply_floor(&mut col, tes, maths, log, notices);
            for (v, x) in col.into_iter().enumerate() {
                t2s[v * cols + c] = x;
            }
        }
    }
    for s in s0.iter_mut() {
        if s.is_nan() {
            *s = 0.0;
        }
    }
    let mut t2s_limited = t2s.clone();
    let mut s0_limited = s0.clone();
    for (v, &a) in am.iter().enumerate() {
        if a == 1 {
            t2s_limited[v * cols..(v + 1) * cols].fill(0.0);
            s0_limited[v * cols..(v + 1) * cols].fill(0.0);
        }
    }
    let cap = scoreatpercentile_lower(&t2s_limited, 99.5).expect("99.5 is a percentile");
    let cap10 = cap.mul(10.0);
    log.debug(Site("decay", "modify_t2s_s0_maps", 624), || {
        format!("Setting cap on T2* map at {}", format_fixed(cap10, 5))
    });
    for t in t2s_limited.iter_mut() {
        if *t > cap10 {
            *t = cap;
        }
    }
    // TED-05: tedana 26.0.3 caps only the limited map; the full map (the combination's weights,
    // the T2* map fMRIPrep uses) keeps values of up to days. By default it gets the same cap.
    let above = t2s.iter().filter(|&&t| t > cap10).count();
    if above > 0 {
        notices.add(
            &TED_05,
            format!(
                "{above} value(s) of the full T2* map above {} ms (ten times the cap) set to the \
                 cap, {} ms; tedana 26.0.3 caps only the limited map",
                format_fixed(cap10, 3),
                format_fixed(cap, 3)
            ),
        );
        if notices.fix() {
            for t in t2s.iter_mut() {
                if *t > cap10 {
                    *t = cap;
                }
            }
        }
    }
    Maps {
        t2s_full: t2s,
        s0_full: s0,
        t2s_limited,
        s0_limited,
    }
}

/// `utils.unmask(data, mask)`: `data` over the set entries of `mask`, rows of `cols` values,
/// zeros elsewhere.
pub fn unmask<T: Copy + Default>(data: &[T], mask: &[bool], cols: usize) -> Vec<T> {
    let mut out = vec![T::default(); mask.len() * cols];
    let mut k = 0;
    for (v, &m) in mask.iter().enumerate() {
        if m {
            out[v * cols..(v + 1) * cols].copy_from_slice(&data[k * cols..(k + 1) * cols]);
            k += 1;
        }
    }
    out
}

/// What `rmse_of_fit_decay_ts` returns.
#[derive(Clone, Debug)]
pub struct Rmse {
    /// `rmse_map` over the mask's voxels (float32; NaN where no volume has a value).
    pub map: Vec<f32>,
    /// Per volume: the mean and standard deviation over voxels (float32).
    pub mean: Vec<f32>,
    pub std: Vec<f32>,
    /// The percentiles 0, 2, 25, 50, 75, 98, 100 per volume.
    pub percentiles: Percentiles,
}

/// `rmse_of_fit_decay_ts(data=data_cat, tes, adaptive_mask, t2s, s0, fitmode)`: `data` the
/// `(voxels, echoes, volumes)` float32 array, `am` per voxel, `t2s` and `s0` the limited maps
/// over the voxels, C-ordered `(voxels, cols)` (`cols` 1 or the volumes).
#[allow(clippy::too_many_arguments)]
pub fn rmse_of_fit_decay_ts(
    data: &[f32],
    n_echos: usize,
    n_vols: usize,
    tes: &[f64],
    am: &[i64],
    t2s: &[f64],
    s0: &[f64],
    cols: usize,
    maths: &Maths,
    libraries: Libraries,
    n_threads: usize,
    log: &mut Log<'_>,
) -> Rmse {
    let n_samples = am.len();
    let ne = tes.len();
    let mut rmse = vec![f32::NAN; n_samples * n_vols];
    larmorx_core::parallel::with_threads(n_threads, || {
        rmse.par_chunks_mut(n_vols.max(1))
            .enumerate()
            .for_each(|(v, row)| {
                let n = am[v];
                if n < 2 || n as usize > ne {
                    return;
                }
                let n = n as usize;
                let mut sq = vec![0.0f32; n * n_vols];
                for e in 0..n {
                    let neg_te = -tes[e];
                    let src = &data[(v * n_echos + e) * n_vols..(v * n_echos + e + 1) * n_vols];
                    let mut constant = None;
                    for t in 0..n_vols {
                        let c = if cols == 1 { 0 } else { t };
                        let pred = match constant {
                            Some(p) => p,
                            None => {
                                let (s, tt) = (s0[v * cols + c], t2s[v * cols + c]);
                                let p = f32::from_f64(s.mul(maths.exp(neg_te.div(tt))));
                                if cols == 1 {
                                    constant = Some(p);
                                }
                                p
                            }
                        };
                        let d = src[t].sub(pred);
                        sq[e * n_vols + t] = d.mul(d);
                    }
                }
                let mean = mean_axis(&sq, [1, n, n_vols]);
                for (r, m) in row.iter_mut().zip(mean) {
                    *r = m.sqrt();
                }
            });
    })
    .expect("a thread pool");
    let sum_map = nansum_axis(&rmse, [n_samples, n_vols, 1]);
    let map: Vec<f32> = (0..n_samples)
        .map(|v| {
            let count = rmse[v * n_vols..(v + 1) * n_vols]
                .iter()
                .filter(|x| !x.is_nan())
                .count();
            if count > 0 {
                f32::from_f64(f64::from(sum_map[v]).div(count as f64))
            } else {
                f32::NAN
            }
        })
        .collect();
    let sum_ts = nansum_axis(&rmse, [1, n_samples, n_vols]);
    let mean: Vec<f32> = (0..n_vols)
        .map(|t| {
            let count = (0..n_samples)
                .filter(|&v| !rmse[v * n_vols + t].is_nan())
                .count();
            if count > 0 {
                f32::from_f64(f64::from(sum_ts[t]).div(count as f64))
            } else {
                f32::NAN
            }
        })
        .collect();
    let sd = nanstd_axis(&rmse, [1, n_samples, n_vols], 0);
    if sd.bad_dof {
        log.warn(
            "numpy/lib/_nanfunctions_impl.py",
            libraries.nanstd_line(),
            "RuntimeWarning",
            "Degrees of freedom <= 0 for slice.",
            "var = nanvar(a, axis=axis, dtype=dtype, out=out, ddof=ddof,",
        );
    }
    let percentiles = nanpercentile_linear_axis0(
        &rmse,
        n_samples,
        n_vols,
        &[0.0, 2.0, 25.0, 50.0, 75.0, 98.0, 100.0],
    )
    .expect("percentiles in [0, 100]");
    if percentiles.all_nan_column {
        log.warn(
            "numpy/lib/_nanfunctions_impl.py",
            libraries.nanquantile_line(),
            "RuntimeWarning",
            "All-NaN slice encountered",
            "return fnb._ureduce(a,",
        );
    }
    Rmse {
        map,
        mean,
        std: sd.values,
        percentiles,
    }
}
