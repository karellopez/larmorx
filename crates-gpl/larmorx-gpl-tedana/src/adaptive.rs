// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! `tedana.utils.make_adaptive_mask(data, n_independent_echos, threshold=1, methods)`: for each
//! voxel, the number of echoes with good signal.
//!
//! - The base count: the leading echoes whose values are, at every volume, neither NaN nor
//!   `<= 0`.
//! - `dropout`: the temporal means (float32, numpy's pairwise sums); the 33rd percentile
//!   (`method="higher"`) of the first echo's non-zero means; the voxels whose first-echo mean
//!   equals it (the one with the largest sum of means when several); a third of their means is
//!   each echo's threshold; the last echo whose mean's magnitude exceeds its threshold.
//! - `decay`: the first echo whose mean does not decrease from the previous one (all echoes if
//!   none).
//! - The minimum of the base count and the methods' (dropout counted twice, as tedana appends it
//!   twice); counts below the threshold set to 0, with a warning.
//!
//! TED-01: a NaN mean in the first echo inside the mask makes the percentile NaN, so no voxel
//! equals it and tedana fails (`ValueError: attempt to get argmax of an empty sequence`); by
//! default the percentile and the exemplars are taken over the finite means only (those voxels
//! are dropped by the base count anyway), `reproduce` raises tedana's error.

use larmorx_core::defects::{Notices, TED_01, TED_11};
use larmorx_core::numpy::format::{array_str, scalar_str};
use larmorx_core::numpy::quantile::percentile_higher;
use larmorx_core::numpy::reduce::{add_reduce_axis, mean_axis};
use larmorx_core::python::number::float_repr;

use crate::decay::Libraries;
use crate::log::{Log, PyError, Site};

/// numpy's `np.round(x, decimals=1)` of a float64: `rint(x * 10) / 10`.
fn np_round1(x: f64) -> f64 {
    (x * 10.0).round_ties_even() / 10.0
}

/// `data.mean(axis=-1)` of `(voxels, echoes, volumes)`. numpy sums along the volumes pairwise
/// when they are the innermost axis in memory (a copy of `data_cat`), and in order when they are
/// the outermost (`data_cat[:, :, use_volumes]`, the copy `--exclude` makes: numpy puts the
/// indexed axis first in memory): `time_outer`.
fn temporal_means(
    data: &[f32],
    n_samples: usize,
    n_echos: usize,
    n_vols: usize,
    time_outer: bool,
) -> Vec<f32> {
    if !time_outer {
        return mean_axis(data, [n_samples * n_echos, n_vols, 1]);
    }
    // In chunks of voxels: each chunk transposed to (volumes, voxels·echoes), then numpy's mean
    // over its outer axis (the same sums in order, element by element).
    let rows = n_samples * n_echos;
    let mut out = Vec::with_capacity(rows);
    let chunk = 8192;
    let mut start = 0;
    while start < rows {
        let end = (start + chunk).min(rows);
        let w = end - start;
        let mut t = vec![0.0f32; w * n_vols];
        for r in 0..w {
            for k in 0..n_vols {
                t[k * w + r] = data[(start + r) * n_vols + k];
            }
        }
        out.extend(mean_axis(&t, [1, n_vols, w]));
        start = end;
    }
    out
}

/// The adaptive mask: `(mask, adaptive_mask)` as tedana returns them (`adaptive_mask` int64,
/// `mask = adaptive_mask != 0`).
#[allow(clippy::too_many_arguments)]
pub fn make_adaptive_mask(
    data: &[f32],
    n_samples: usize,
    n_echos: usize,
    n_vols: usize,
    n_independent_echos: Option<i128>,
    methods: &[String],
    time_outer: bool,
    libraries: Libraries,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<(Vec<bool>, Vec<i64>), PyError> {
    let threshold = 1i64;
    if methods.len() == 1 && methods[0].eq_ignore_ascii_case("none") {
        log.warning(
            Site("utils", "make_adaptive_mask", 113),
            "No methods provided for adaptive mask generation. Only removing voxels with \
             negative or NaN values",
        );
    }
    let has = |m: &str| methods.iter().any(|x| x == m);
    let at = |v: usize, e: usize| v * n_echos * n_vols + e * n_vols;

    // The base count.
    let mut base = vec![0i64; n_samples];
    for (v, b) in base.iter_mut().enumerate() {
        let mut count = 0i64;
        for e in 0..n_echos {
            let series = &data[at(v, e)..at(v, e) + n_vols];
            if series.iter().any(|&x| x.is_nan() || x <= 0.0) {
                break;
            }
            count += 1;
        }
        *b = count;
    }
    let mut masks: Vec<Vec<i64>> = vec![base];

    let echo_means = if has("dropout") || has("decay") {
        // numpy warns of a mean over an empty axis whatever the result's size (no voxels
        // included), and of the division 0 / 0 only where there are values to divide.
        if n_vols == 0 {
            log.warn(
                "tedana/utils.py",
                145,
                "RuntimeWarning",
                libraries.mean_of_empty_slice(),
                "echo_means = data.mean(axis=-1)  # temporal mean of echos",
            );
            if n_samples * n_echos > 0 {
                log.warn(
                    "numpy/_core/_methods.py",
                    libraries.methods_divide_line(),
                    "RuntimeWarning",
                    "invalid value encountered in divide",
                    "ret = um.true_divide(",
                );
            }
        }
        temporal_means(data, n_samples, n_echos, n_vols, time_outer)
    } else {
        Vec::new()
    };

    if has("dropout") {
        let first: Vec<f32> = (0..n_samples)
            .map(|v| echo_means[v * n_echos])
            .filter(|&m| m != 0.0)
            .collect();
        let has_nan = first.iter().any(|m| m.is_nan());
        let pool: Vec<f32> = if has_nan && notices.fix() {
            first.iter().copied().filter(|m| m.is_finite()).collect()
        } else {
            first
        };
        if pool.is_empty() && has_nan && notices.fix() {
            notices.add(&TED_01, "no voxel of the mask has a finite first-echo mean");
            return Err(PyError::value_error(
                "no voxel of the mask has a finite non-zero first-echo mean, so the dropout \
                 thresholds cannot be set (tedana fails with: attempt to get argmax of an empty \
                 sequence)",
                "tedana/utils.py, in make_adaptive_mask",
            ));
        }
        let perc = percentile_higher(&pool, 33.0).ok_or_else(|| {
            PyError::new(
                "IndexError",
                "index -1 is out of bounds for axis 0 with size 0",
                "numpy/lib/_function_base_impl.py, in percentile",
            )
        })?;
        let exemplars: Vec<usize> = (0..n_samples)
            .filter(|&v| echo_means[v * n_echos] == perc)
            .collect();
        if has_nan {
            notices.add(
                &TED_01,
                if notices.fix() {
                    "the dropout thresholds are taken from the voxels whose first-echo mean is \
                     finite (a NaN mean made tedana fail)"
                } else {
                    "a NaN first-echo mean makes the dropout percentile NaN"
                },
            );
        }
        // lthrs = squeeze(echo_means[perc_val].T) / 3
        let lthrs: Vec<f32> = match exemplars.len() {
            0 if n_echos == 1 => {
                // squeeze((1, 0)) is (0,): an empty threshold list, then lthrs[0] fails.
                log.info(
                    Site("utils", "make_adaptive_mask", 167),
                    "Echo-wise intensity thresholds for adaptive mask: []",
                );
                return Err(PyError::new(
                    "IndexError",
                    "index 0 is out of bounds for axis 0 with size 0",
                    "tedana/utils.py, in make_adaptive_mask",
                ));
            }
            0 => {
                return Err(PyError::value_error(
                    "attempt to get argmax of an empty sequence",
                    "tedana/utils.py, in make_adaptive_mask",
                ));
            }
            1 => (0..n_echos)
                .map(|e| echo_means[exemplars[0] * n_echos + e] / 3.0)
                .collect(),
            k => {
                // (E, k) Fortran-ordered: exemplar-major in memory.
                let cols: Vec<f32> = exemplars
                    .iter()
                    .flat_map(|&v| (0..n_echos).map(move |e| (v, e)))
                    .map(|(v, e)| echo_means[v * n_echos + e] / 3.0)
                    .collect();
                if n_echos == 1 {
                    // squeeze gives (k,): ndim 1, kept as it is.
                    cols
                } else {
                    let sums = add_reduce_axis(&cols, [k, n_echos, 1]);
                    // numpy's argmax: the first NaN, else the first largest.
                    let best = sums.iter().position(|s| s.is_nan()).unwrap_or_else(|| {
                        let mut b = 0;
                        for (j, s) in sums.iter().enumerate() {
                            if *s > sums[b] {
                                b = j;
                            }
                        }
                        b
                    });
                    cols[best * n_echos..(best + 1) * n_echos].to_vec()
                }
            }
        };
        if n_echos == 1 && exemplars.len() == 1 {
            // A numpy scalar: `lthrs[echo_idx]` fails.
            log.info(
                Site("utils", "make_adaptive_mask", 167),
                &format!(
                    "Echo-wise intensity thresholds for adaptive mask: {}",
                    scalar_str(lthrs[0])
                ),
            );
            return Err(PyError::new(
                "IndexError",
                "invalid index to scalar variable.",
                "tedana/utils.py, in make_adaptive_mask",
            ));
        }
        log.info(
            Site("utils", "make_adaptive_mask", 167),
            &format!(
                "Echo-wise intensity thresholds for adaptive mask: {}",
                array_str(&lthrs)
            ),
        );
        if lthrs.len() < n_echos {
            return Err(PyError::new(
                "IndexError",
                format!(
                    "index {} is out of bounds for axis 0 with size {}",
                    lthrs.len(),
                    lthrs.len()
                ),
                "tedana/utils.py, in make_adaptive_mask",
            ));
        }
        let mut dropout = vec![0i64; n_samples];
        for e in 0..n_echos {
            for (v, d) in dropout.iter_mut().enumerate() {
                if echo_means[v * n_echos + e].abs() > lthrs[e] {
                    *d = (e + 1) as i64;
                }
            }
        }
        masks.push(dropout.clone());
        masks.push(dropout);
    }

    if has("decay") {
        let mut last = vec![0i64; n_samples];
        for (v, l) in last.iter_mut().enumerate() {
            // echo_diffs = [-1, diff(means)], float64; the first index where it is >= 0.
            let mut first = 0usize;
            for e in 1..n_echos {
                let d = echo_means[v * n_echos + e] - echo_means[v * n_echos + e - 1];
                if f64::from(d) >= 0.0 {
                    first = e;
                    break;
                }
            }
            *l = if first == 0 {
                n_echos as i64
            } else {
                first as i64
            };
        }
        masks.push(last);
    }

    let mut adaptive: Vec<i64> = (0..n_samples)
        .map(|v| masks.iter().map(|m| m[v]).min().expect("the base count"))
        .collect();
    let n_bad = adaptive.iter().filter(|&&a| a < threshold).count();
    if n_bad > 0 {
        log.warning(
            Site("utils", "make_adaptive_mask", 195),
            &format!(
                "{n_bad} voxels in user-defined mask do not have good signal. Removing voxels \
                 from mask."
            ),
        );
        for a in &mut adaptive {
            if *a < threshold {
                *a = 0;
            }
        }
    }

    if let Some(n_ind) = n_independent_echos {
        if n_ind == 0 {
            // `2 * n_echos / n_independent_echos`: Python's ZeroDivisionError.
            notices.add(&TED_11, "--n-independent-echos 0");
            return Err(if notices.fix() {
                PyError::value_error(
                    "--n-independent-echos must be a positive number of echoes (tedana fails \
                     with ZeroDivisionError)",
                    "tedana/utils.py, in make_adaptive_mask",
                )
            } else {
                PyError::new(
                    "ZeroDivisionError",
                    "division by zero",
                    "tedana/utils.py, in make_adaptive_mask",
                )
            });
        }
        let good = adaptive.iter().filter(|&&a| a >= 1).count() as f64;
        let t3 = (2.0 * n_echos as f64 / n_ind as f64).floor();
        let n3 = adaptive
            .iter()
            .filter(|&&a| (a as f64) < t3 && a >= 1)
            .count();
        // numpy int64 scalars: 0 / 0 (no good voxel) is NaN, with numpy's warning.
        let zero_good = |log: &mut Log<'_>, line: u32, source: &str| {
            if good == 0.0 {
                log.warn(
                    "tedana/utils.py",
                    line,
                    "RuntimeWarning",
                    "invalid value encountered in scalar divide",
                    source,
                );
            }
        };
        zero_good(
            log,
            213,
            "perc_3dof_voxels = 100 * n_3dof_voxels / np.sum(adaptive_mask >= 1)",
        );
        let perc3 = 100.0 * n3 as f64 / good;
        if perc3 > 0.0 {
            log.warning(
                Site("utils", "make_adaptive_mask", 215),
                &format!(
                    "{n3} voxels ({}%) have fewer than {} good voxels. These voxels will be used \
                     in all analyses, but might not include 3 independent echo measurements.",
                    float_repr(np_round1(perc3)),
                    float_repr(t3.round_ties_even())
                ),
            );
        }
        if n_ind > 3 {
            let tdof = ((n_ind - 1) as f64 * n_echos as f64 / n_ind as f64).floor();
            let ndof = adaptive
                .iter()
                .filter(|&&a| (a as f64) < tdof && a >= 1)
                .count();
            zero_good(
                log,
                230,
                "perc_dof_voxels = 100 * n_dof_voxels / np.sum(adaptive_mask >= 1)",
            );
            let percdof = 100.0 * ndof as f64 / good;
            log.warning(
                Site("utils", "make_adaptive_mask", 231),
                &format!(
                    "{ndof} voxels ({}%) have fewer than {} good voxels. The degrees of freedom \
                     for fits across echoes will remain {n_ind} even if there might be fewer \
                     independent echo measurements.",
                    float_repr(np_round1(percdof)),
                    float_repr(tdof.round_ties_even())
                ),
            );
        }
    }
    let mask = adaptive.iter().map(|&a| a != 0).collect();
    Ok((mask, adaptive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporal_means_follow_the_memory_order() {
        // Nine volumes: pairwise (8 accumulators) and in-order sums can differ in the last bit.
        let data: Vec<f32> = (0..2 * 9)
            .map(|i| 1.0 + (i as f32) * 1.1e-7 * 1e6)
            .collect();
        let pairwise = temporal_means(&data, 2, 1, 9, false);
        let ordered = temporal_means(&data, 2, 1, 9, true);
        for v in 0..2 {
            let mut s = 0.0f32;
            for t in 0..9 {
                s += data[v * 9 + t];
            }
            assert_eq!(ordered[v], (f64::from(s) / 9.0) as f32);
        }
        assert_eq!(pairwise.len(), 2);
    }
}
