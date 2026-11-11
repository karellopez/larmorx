// SPDX-License-Identifier: Apache-2.0
//! The RMSE map and the confounds table (`specs/t2smap.md` §9).

use larmorx_core::math;
use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::quantile::nanpercentile_linear_axis0;
use larmorx_core::numpy::reduce::{nanstd_axis, nansum_axis, pairwise_sum};
use rayon::prelude::*;

use crate::adaptive::divide_count;
use crate::input::Data;
use crate::log::Log;
use crate::maps::Final;

/// The table's columns, in order (§9 step 3).
pub const COLUMNS: [&str; 9] = [
    "rmse_mean",
    "rmse_std",
    "rmse_min",
    "rmse_percentile02",
    "rmse_percentile25",
    "rmse_median",
    "rmse_percentile75",
    "rmse_percentile98",
    "rmse_max",
];
const PERCENTILES: [f64; 7] = [0.0, 2.0, 25.0, 50.0, 75.0, 98.0, 100.0];

/// The RMSE of every voxel and volume (§9 step 1), `(S, T)` float32, NaN below two echoes.
pub fn series(data: &Data, count: &[u8], f: &Final, tes: &[f64], reproduce: bool) -> Vec<f32> {
    let (n_t, cols) = (data.n_t, f.cols);
    let exp = |a: f64| {
        if reproduce {
            math::exp_glibc(a)
        } else {
            math::exp(a)
        }
    };
    let mut out = vec![f32::NAN; data.n_vox * n_t];
    out.par_chunks_mut(n_t.max(1))
        .enumerate()
        .with_min_len(64)
        .for_each(|(v, row)| {
            let n = count[v] as usize;
            if n < 2 || n_t == 0 {
                return;
            }
            let mut p = [0.0f32; 256];
            let predict = |p: &mut [f32; 256], t: usize| {
                let k = v * cols + t;
                let (s0, t2) = (f.s0_limited[k], f.t2s_limited[k]);
                for e in 0..n {
                    p[e] = s0.mul(exp((-tes[e]).div(t2))) as f32;
                }
            };
            if cols == 1 {
                predict(&mut p, 0);
            }
            for (t, r) in row.iter_mut().enumerate() {
                if cols > 1 {
                    predict(&mut p, t);
                }
                let mut sq = [0.0f32; 256];
                for (e, &pe) in p.iter().enumerate().take(n) {
                    let d = data.series(v, e)[t].sub(pe);
                    sq[e] = d.mul(d);
                }
                // the echoes an outer axis: a plain sum; pairwise for a single volume (§13.2)
                let acc = if n_t == 1 {
                    0.0f32.add(pairwise_sum(&sq[..n], 0, n, 1))
                } else {
                    sq[..n].iter().fold(0.0f32, |a, &b| a.add(b))
                };
                *r = divide_count(acc, n).sqrt();
            }
        });
    out
}

/// Whether a limited T2* of 0 is divided by in the predictions (numpy's warning, §9 step 1).
pub fn divides_by_zero(count: &[u8], f: &Final) -> bool {
    count
        .iter()
        .enumerate()
        .any(|(v, &c)| c >= 2 && (0..f.cols).any(|t| f.t2s_limited[v * f.cols + t] == 0.0))
}

/// The RMSE map (§9 step 2): per voxel numpy's `nansum` over the volumes (pairwise) divided
/// by the number of numbers; NaN when none.
pub fn map(r: &[f32], n_vox: usize, n_t: usize) -> Vec<f32> {
    (0..n_vox)
        .into_par_iter()
        .with_min_len(256)
        .map(|v| {
            let row = &r[v * n_t..(v + 1) * n_t];
            let filled: Vec<f32> = row
                .iter()
                .map(|&x| if x.is_nan() { 0.0 } else { x })
                .collect();
            let numbers = row.iter().filter(|x| !x.is_nan()).count();
            let s = 0.0f32.add(pairwise_sum(&filled, 0, n_t, 1));
            divide_count(s, numbers)
        })
        .collect()
}

/// The confounds table (§9 step 3): one row per volume of the nine columns (float64), and
/// whether the percentile columns are float32 values (volume 0 without a number).
pub struct Table {
    pub rows: Vec<[f64; 9]>,
    pub percentiles_f32: bool,
}

pub fn table(r: &[f32], n_vox: usize, n_t: usize, log: &mut Log<'_>) -> Table {
    // rmse_mean: numpy's nansum over the voxels: plain float32 sums in voxel order (the voxels
    // an outer axis), pairwise for a single volume (§13.2)
    let sums = nansum_axis(r, [1, n_vox, n_t]);
    let numbers: Vec<usize> = (0..n_t)
        .map(|t| (0..n_vox).filter(|&v| !r[v * n_t + t].is_nan()).count())
        .collect();
    let std = nanstd_axis(r, [1, n_vox, n_t], 0);
    let pct = nanpercentile_linear_axis0(r, n_vox, n_t, &PERCENTILES).expect("valid percentiles");
    // a volume holding an infinite RMSE (an S0 above float64's range): numpy's nanstd subtracts
    // its infinite mean from it, and the 100th percentile's interpolation subtracts it from
    // itself (observed)
    let infinite = |t: usize| (0..n_vox).any(|v| r[v * n_t + t] == f32::INFINITY);
    let first_inf = (0..n_t).find(|&t| infinite(t));
    let first_all_nan = (0..n_t).find(|&t| numbers[t] == 0);
    if first_inf.is_some() {
        log.py_warning(
            "rmse.rs:nanstd",
            "RuntimeWarning",
            "invalid value encountered in subtract",
        );
    }
    if std.bad_dof {
        log.py_warning(
            "rmse.rs:120",
            "RuntimeWarning",
            "Degrees of freedom <= 0 for slice.",
        );
    }
    let lerp = |log: &mut Log<'_>| {
        log.py_warning(
            "rmse.rs:percentile",
            "RuntimeWarning",
            "invalid value encountered in subtract",
        );
    };
    // the percentiles column by column (volume by volume): the first volume decides the order
    if first_inf.is_some_and(|i| first_all_nan.is_none_or(|n| i < n)) {
        lerp(log);
    }
    if pct.all_nan_column {
        log.py_warning("rmse.rs:121", "RuntimeWarning", "All-NaN slice encountered");
    }
    if first_inf.is_some() {
        lerp(log);
    }
    let rows = (0..n_t)
        .map(|t| {
            let mut row = [0.0f64; 9];
            row[0] = divide_count(sums[t], numbers[t]) as f64;
            row[1] = std.values[t] as f64;
            for i in 0..7 {
                row[2 + i] = pct.values[i * n_t + t];
            }
            row
        })
        .collect();
    Table {
        rows,
        percentiles_f32: pct.float32,
    }
}
