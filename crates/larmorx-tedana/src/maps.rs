// SPDX-License-Identifier: Apache-2.0
//! Floors, limited maps and the cap (`specs/t2smap.md` §8), TED-05, TED-14, TED-18.

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use larmorx_core::numpy::quantile::scoreatpercentile_lower;
use larmorx_core::python::number::format_fixed;
use rayon::prelude::*;

use crate::fit::Maps;
use crate::log::Log;

/// float64 `log(eps)`, eps = 2⁻⁵² (§8 step 2).
pub const LOG_EPS: f64 = -36.04365338911715;

/// The maps carried to the RMSE, the combination and the files (§8): on the mask's voxels,
/// `[v * cols + t]`, 0 outside the fit voxels; T2* in ms.
#[derive(Clone, Debug)]
pub struct Final {
    pub cols: usize,
    pub t2s_full: Vec<f64>,
    pub s0_full: Vec<f64>,
    pub t2s_limited: Vec<f64>,
    pub s0_limited: Vec<f64>,
    /// The log-linear S0 with the other exponential, after the same floor (NaN: not written as
    /// a log-linear value), for GLIBC-07's count (§15.2).
    pub s0_other: Vec<f64>,
}

/// §8 on the fit's maps: floors (per volume's column in `ts`), the limited maps, the cap.
pub fn finish(
    mut maps: Maps,
    count: &[u8],
    tes: &[f64],
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Final {
    let cols = maps.cols;
    let reproduce = notices.reproduce();
    let fit: Vec<usize> = (0..count.len()).filter(|&v| count[v] >= 1).collect();
    let n_echo_all = tes.len();
    let te_min_neg = tes.iter().map(|&t| -t).fold(f64::INFINITY, f64::min);
    let floor_value = te_min_neg / LOG_EPS;
    for t in 0..cols {
        // 1. the infinite and non-positive values
        for &v in &fit {
            let x = &mut maps.t2s[v * cols + t];
            if *x == f64::INFINITY {
                *x = 500.0;
            } else if *x <= 0.0 {
                *x = 1.0;
            }
        }
        // 2. the underflow floor: an echo's exp(−TE / T2*) exactly 0
        let under: Vec<bool> = fit
            .par_iter()
            .with_min_len(256)
            .map(|&v| {
                let x = maps.t2s[v * cols + t];
                x != 0.0
                    && tes.iter().any(|&te| {
                        let a = -te / x;
                        (if reproduce {
                            math::exp_glibc(a)
                        } else {
                            math::exp(a)
                        }) == 0.0
                    })
            })
            .collect();
        let b = under.iter().filter(|&&u| u).count();
        for (j, &v) in fit.iter().enumerate() {
            if under[j] {
                maps.t2s[v * cols + t] = floor_value;
            }
        }
        if b > 0 && log.debugging() {
            let v_count = fit.len();
            let denom = if notices.fix() {
                v_count
            } else {
                n_echo_all * v_count
            };
            log.debug(
                "decay:_apply_t2s_floor:58",
                &format!(
                    "T2* values for {b}/{denom} voxels ({}%) have been identified as close to \
                     zero and have been adjusted",
                    format_fixed(100.0 * b as f64 / denom as f64, 2)
                ),
            );
            if n_echo_all > 1 {
                notices.add(
                    &defects::TED_14,
                    format!(
                        "the floor's message counts {v_count} voxels; tedana prints {}",
                        n_echo_all * v_count
                    ),
                );
            }
        }
        // 3. NaN S0 → 0
        for &v in &fit {
            let k = v * cols + t;
            // (a NaN log-linear S0 is NaN with either function: its `s0_other` stays NaN and
            // is not counted)
            if maps.s0[k].is_nan() {
                maps.s0[k] = 0.0;
            }
        }
    }
    // 4. the limited maps
    let mut t2s_lim = maps.t2s.clone();
    let mut s0_lim = maps.s0.clone();
    for &v in &fit {
        if count[v] == 1 {
            for t in 0..cols {
                t2s_lim[v * cols + t] = 0.0;
                s0_lim[v * cols + t] = 0.0;
            }
        }
    }
    // 5. the cap: the 99.5th percentile ("lower") of every limited value of the fit voxels
    let values: Vec<f64> = fit
        .iter()
        .flat_map(|&v| (0..cols).map(move |t| v * cols + t))
        .map(|k| t2s_lim[k])
        .collect();
    let c = scoreatpercentile_lower(&values, 99.5).expect("99.5 is a percentile");
    log.debug(
        "decay:modify_t2s_s0_maps:624",
        &format!("Setting cap on T2* map at {}", format_fixed(10.0 * c, 5)),
    );
    let above = |x: f64| x > 10.0 * c;
    let k_lim = values.iter().filter(|&&x| above(x)).count();
    let ted18 = c <= 0.0;
    if ted18 && k_lim > 0 {
        notices.add(
            &defects::TED_18,
            format!(
                "the limited T2* map's cap is {} ms (its 99.5th percentile), so tedana sets its \
                 {k_lim} value(s) above it to it",
                format_fixed(c, 3)
            ),
        );
    }
    if !(ted18 && notices.fix()) {
        for &v in &fit {
            for t in 0..cols {
                let x = &mut t2s_lim[v * cols + t];
                if above(*x) {
                    *x = c;
                }
            }
        }
    }
    // TED-05: the full map capped too, by default (not when TED-18's cap is 0 or less)
    if !ted18 {
        let k_full = fit
            .iter()
            .flat_map(|&v| (0..cols).map(move |t| v * cols + t))
            .filter(|&k| above(maps.t2s[k]))
            .count();
        if k_full > 0 {
            notices.add(
                &defects::TED_05,
                format!(
                    "{k_full} value(s) of the full T2* map above {} ms (ten times the cap) set to \
                     the cap, {} ms; tedana 26.0.3 caps only the limited map",
                    format_fixed(10.0 * c, 3),
                    format_fixed(c, 3)
                ),
            );
            if notices.fix() {
                for &v in &fit {
                    for t in 0..cols {
                        let x = &mut maps.t2s[v * cols + t];
                        if above(*x) {
                            *x = c;
                        }
                    }
                }
            }
        }
    }
    let s0_other = maps.s0_other;
    Final {
        cols,
        t2s_full: maps.t2s,
        s0_full: maps.s0,
        t2s_limited: t2s_lim,
        s0_limited: s0_lim,
        s0_other,
    }
}

/// GLIBC-07's counts (§15.2): the written log-linear S0 values (float32) that the other
/// exponential would change, in the full and the limited S0 maps.
pub fn glibc07_counts(f: &Final, count: &[u8]) -> (usize, usize) {
    let cols = f.cols;
    let mut full = 0;
    let mut limited = 0;
    for (v, &c) in count.iter().enumerate() {
        if c < 1 {
            continue;
        }
        for t in 0..cols {
            let k = v * cols + t;
            let other = f.s0_other[k];
            if other.is_nan() {
                continue;
            }
            let differs = (f.s0_full[k] as f32).to_bits() != (other as f32).to_bits();
            if differs {
                full += 1;
                if c >= 2 {
                    limited += 1;
                }
            }
        }
    }
    (full, limited)
}
