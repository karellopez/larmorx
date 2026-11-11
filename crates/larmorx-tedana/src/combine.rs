// SPDX-License-Identifier: Apache-2.0
//! The optimal combination (`specs/t2smap.md` §10), TED-15.

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::reduce::pairwise_sum;
use rayon::prelude::*;

use crate::adaptive::divide_count;
use crate::input::Data;
use crate::log::PyError;
use crate::maps::Final;
use crate::options::CombMode;

/// A float64 sum of `w` over the echoes: in order from +0 (the echoes an outer axis), or
/// numpy's pairwise sum (the echoes innermost); the two differ only from eight echoes on.
fn sum(w: &[f64], pairwise: bool) -> f64 {
    if pairwise {
        0.0f64.add(pairwise_sum(w, 0, w.len(), 1))
    } else {
        w.iter().fold(0.0f64, |a, &b| a.add(b))
    }
}

/// PAID's weight of one echo series: its temporal SNR (float32 mean over its float32 standard
/// deviation, 0 where that is 0) times the echo time.
fn paid_weight(s: &[f32], te: f64) -> f64 {
    let n = s.len();
    let mu = divide_count(0.0f32.add(pairwise_sum(s, 0, n, 1)), n);
    let sq: Vec<f32> = s
        .iter()
        .map(|&x| {
            let d = x.sub(mu);
            d.mul(d)
        })
        .collect();
    let sd = divide_count(0.0f32.add(pairwise_sum(&sq, 0, n, 1)), n).sqrt();
    let snr = if sd != 0.0 { mu.div(sd) } else { 0.0 };
    (snr as f64).mul(te)
}

/// numpy's warnings while PAID's means and standard deviations are computed (§12.3), over the
/// echoes each voxel's group combines: (`reduce`: a series holding both infinities, whose sum
/// is NaN, in the mean and again in the standard deviation; `subtract`: a series whose mean is
/// infinite, minus that mean). A series that also holds a NaN raises neither.
pub fn paid_warnings(data: &Data, count: &[u8]) -> (bool, bool) {
    (0..data.n_vox)
        .into_par_iter()
        .with_min_len(256)
        .filter(|&v| count[v] >= 1)
        .map(|v| {
            let mut out = (false, false);
            for e in 0..usize::from(count[v]).max(2) {
                let s = data.series(v, e);
                let pos = s.contains(&f32::INFINITY);
                let neg = s.contains(&f32::NEG_INFINITY);
                if s.iter().any(|x| x.is_nan()) {
                    continue;
                }
                out.0 |= pos && neg;
                out.1 |= pos != neg;
            }
            out
        })
        .reduce(|| (false, false), |a, b| (a.0 || b.0, a.1 || b.1))
}

/// The combined series `(S, T)` (float32 as written), §10.
pub fn combine(
    data: &Data,
    count: &[u8],
    f: &Final,
    tes: &[f64],
    mode: CombMode,
    notices: &mut Notices,
) -> Result<Vec<f32>, PyError> {
    let (n_t, cols) = (data.n_t, f.cols);
    let reproduce = notices.reproduce();
    let exp = |a: f64| {
        if reproduce {
            math::exp_glibc(a)
        } else {
            math::exp(a)
        }
    };
    if mode == CombMode::Paid && n_t == 1 && count.iter().any(|&c| c >= 1) {
        notices.add(&defects::TED_15, "a one-volume series with --combmode paid");
        return Err(if notices.fix() {
            PyError::value(
                "--combmode paid needs more than one volume: its weights are each echo's \
                 temporal signal-to-noise ratio (tedana fails with ZeroDivisionError)",
            )
        } else {
            zero_weights()
        });
    }
    let ts = cols > 1;
    // one volume: every sum is numpy's pairwise one (§13.2)
    let den_pairwise = ts || n_t == 1;
    let num_pairwise = n_t == 1;
    let fix = notices.fix();
    let mut out = vec![0.0f32; data.n_vox * n_t];
    // per voxel: (zero-sum volumes, equal weights used)
    let flags: Vec<(bool, bool)> = out
        .par_chunks_mut(n_t.max(1))
        .enumerate()
        .with_min_len(64)
        .map(|(v, row)| {
            let c = count[v] as usize;
            if c == 0 || n_t == 0 {
                return (false, false);
            }
            let n = c.max(2);
            let mut w = [0.0f64; 256];
            let mut prod = [0.0f64; 256];
            let voxel_weights = |w: &mut [f64; 256], t: usize| match mode {
                CombMode::Paid => {
                    for e in 0..n {
                        w[e] = paid_weight(data.series(v, e), tes[e]);
                    }
                }
                CombMode::T2s => {
                    let t2 = f.t2s_full[v * cols + t];
                    for e in 0..n {
                        w[e] = tes[e].mul(exp((-tes[e]).div(t2)));
                    }
                    if ts && w[..n].iter().all(|&x| x == 0.0) {
                        w[..n].fill(1.0);
                    }
                }
            };
            // PAID's weights and `all`'s are the same for every volume
            let per_volume = ts && mode == CombMode::T2s;
            if !per_volume {
                voxel_weights(&mut w, 0);
            }
            let mut zero = false;
            let mut equal = false;
            for (t, r) in row.iter_mut().enumerate() {
                if per_volume {
                    voxel_weights(&mut w, t);
                }
                let mut den = sum(&w[..n], den_pairwise);
                if den == 0.0 {
                    zero = true;
                    if !(fix && mode == CombMode::Paid) {
                        continue;
                    }
                    // TED-15: equal weights
                    w[..n].fill(1.0);
                    den = sum(&w[..n], den_pairwise);
                    equal = true;
                }
                for e in 0..n {
                    prod[e] = w[e].mul(data.series(v, e)[t] as f64);
                }
                *r = sum(&prod[..n], num_pairwise).div(den) as f32;
            }
            (zero, equal)
        })
        .collect();
    let zero = flags.iter().filter(|f| f.0).count();
    if zero > 0 {
        if mode == CombMode::Paid {
            // the notice counts the first echo-count group (ascending) that holds such voxels
            // (§15: the replica's numbers, observed)
            let first = (1..=data.n_echo)
                .find_map(|c| {
                    let k = (0..data.n_vox)
                        .filter(|&v| usize::from(count[v]) == c && flags[v].0)
                        .count();
                    (k > 0).then_some(k)
                })
                .unwrap_or(zero);
            let detail = if fix {
                format!(
                    "{first} voxel(s) whose good echoes do not vary over time combined with \
                     equal weights; tedana fails with ZeroDivisionError"
                )
            } else {
                format!(
                    "{first} voxel(s) whose good echoes do not vary over time: their weights sum \
                     to zero, and tedana fails with ZeroDivisionError"
                )
            };
            notices.add(&defects::TED_15, detail);
        }
        if !(fix && mode == CombMode::Paid) {
            return Err(zero_weights());
        }
    }
    Ok(out)
}

fn zero_weights() -> PyError {
    PyError::new(
        "ZeroDivisionError",
        "Weights sum to zero, can't be normalized",
    )
}
