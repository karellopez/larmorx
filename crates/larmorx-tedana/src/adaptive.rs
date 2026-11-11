// SPDX-License-Identifier: Apache-2.0
//! The adaptive mask (`specs/t2smap.md` §6): per voxel, the number of echoes with good signal.

use larmorx_core::defects::{self, Notices};
use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::format::{array_str, scalar_str};
use larmorx_core::numpy::reduce::pairwise_sum;
use larmorx_core::python::number::float_repr;
use rayon::prelude::*;

use crate::input::Data;
use crate::log::{Log, PyError};
use crate::options::{Libraries, MaskType};

/// The data the adaptive mask and the fit see: every volume, or those `--exclude` leaves.
pub struct Used<'a> {
    pub data: &'a Data,
    /// The used volumes (`None`: all of them, in which case numpy's means are pairwise).
    pub volumes: Option<&'a [usize]>,
}

impl Used<'_> {
    pub fn n_used(&self) -> usize {
        self.volumes.map_or(self.data.n_t, <[usize]>::len)
    }

    /// The temporal mean of voxel `v`, echo `e` (§6.2 step 1, §13.2): `0 + pairwise` along the
    /// volumes, or with `--exclude` a plain sum in volume order (the volumes then an outer
    /// axis); the division by the count in float64.
    fn mean(&self, v: usize, e: usize) -> f32 {
        let s = self.data.series(v, e);
        let (sum, n) = match self.volumes {
            None => (0.0f32.add(pairwise_sum(s, 0, s.len(), 1)), s.len()),
            Some(vols) => {
                let mut acc = 0.0f32;
                for &t in vols {
                    acc = acc.add(s[t]);
                }
                (acc, vols.len())
            }
        };
        divide_count(sum, n)
    }

    /// Whether echo `e` of voxel `v` is good: none of its used samples NaN or ≤ 0 (§6.1).
    fn good(&self, v: usize, e: usize) -> bool {
        let s = self.data.series(v, e);
        let bad = |x: f32| x.is_nan() || x <= 0.0;
        match self.volumes {
            None => !s.iter().any(|&x| bad(x)),
            Some(vols) => !vols.iter().any(|&t| bad(s[t])),
        }
    }
}

/// The places of numpy's own warnings in its mean and standard deviation (Python's warnings
/// registry knows them apart by place: §12.3).
pub const MEAN_SUM: &str = "numpy.rs:mean_sum";
pub const MEAN_DIVIDE: &str = "numpy.rs:mean_divide";
pub const VAR_SUM: &str = "numpy.rs:var_sum";
pub const VAR_SUBTRACT: &str = "numpy.rs:var_subtract";

/// A float32 sum divided by an integer count (numpy's `true_divide` by an `intp`).
pub fn divide_count(sum: f32, n: usize) -> f32 {
    f32::from_f64(sum.to_f64().div(n as f64))
}

/// The run's adaptive mask: the counts and what was printed.
pub struct Adaptive {
    /// Per voxel of the mask, 0 to E.
    pub count: Vec<u8>,
}

/// The adaptive mask of §6, its messages in order, the defects' corrections (TED-01, TED-12).
pub fn adaptive_mask(
    used: &Used<'_>,
    masktypes: &[MaskType],
    libraries: Libraries,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Adaptive, PyError> {
    let data = used.data;
    let (n_vox, n_echo) = (data.n_vox, data.n_echo);
    let methods: Vec<MaskType> = masktypes
        .iter()
        .copied()
        .filter(|m| *m != MaskType::None)
        .collect();
    if methods.is_empty() {
        log.warning(
            "utils:make_adaptive_mask:113",
            "No methods provided for adaptive mask generation. Only removing voxels with \
             negative or NaN values",
        );
    }
    // the base count: the leading good echoes
    let mut count: Vec<u8> = (0..n_vox)
        .into_par_iter()
        .with_min_len(256)
        .map(|v| {
            let mut c = 0u8;
            for e in 0..n_echo {
                if !used.good(v, e) {
                    break;
                }
                c += 1;
            }
            c
        })
        .collect();
    if !methods.is_empty() {
        if used.n_used() == 0 {
            // numpy's warnings of a mean of no value (TED-13, TED-16 in reproduce)
            let text = match libraries {
                Libraries::Newest => "Mean of empty slice",
                Libraries::Shared => "Mean of empty slice.",
            };
            log.py_warning("adaptive.rs:60", "RuntimeWarning", text);
            // a mask of no voxel leaves nothing to divide: the first warning alone (§12.3,
            // observed)
            if n_vox > 0 {
                log.py_warning(
                    MEAN_DIVIDE,
                    "RuntimeWarning",
                    "invalid value encountered in divide",
                );
            }
        }
        let means: Vec<f32> = (0..n_vox)
            .into_par_iter()
            .with_min_len(256)
            .flat_map_iter(|v| (0..n_echo).map(move |e| used.mean(v, e)))
            .collect();
        // a series holding both infinities sums to NaN: numpy's `reduce` warning (observed);
        // a NaN among them (which numpy's order may meet first) is taken to silence it
        let both = (0..n_vox).into_par_iter().with_min_len(256).any(|v| {
            (0..n_echo).any(|e| {
                let s = used.data.series(v, e);
                let mut seen = (false, false, false);
                let mut look = |x: f32| {
                    seen.0 |= x == f32::INFINITY;
                    seen.1 |= x == f32::NEG_INFINITY;
                    seen.2 |= x.is_nan();
                };
                match used.volumes {
                    None => s.iter().for_each(|&x| look(x)),
                    Some(vols) => vols.iter().for_each(|&t| look(s[t])),
                }
                seen.0 && seen.1 && !seen.2
            })
        });
        if both {
            log.py_warning(
                MEAN_SUM,
                "RuntimeWarning",
                "invalid value encountered in reduce",
            );
        }
        let m = |v: usize, e: usize| means[v * n_echo + e];
        if methods.contains(&MaskType::Dropout) {
            let thr = dropout_thresholds(&means, n_vox, n_echo, libraries, notices)?;
            let text = if n_echo == 1 {
                scalar_str(thr[0])
            } else {
                array_str(&thr)
            };
            log.info(
                "utils:make_adaptive_mask:167",
                &format!("Echo-wise intensity thresholds for adaptive mask: {text}"),
            );
            if n_echo == 1 {
                return Err(PyError::new(
                    "IndexError",
                    "invalid index to scalar variable.",
                ));
            }
            for (v, c) in count.iter_mut().enumerate() {
                let mut d = 0u8;
                for (e, &t) in thr.iter().enumerate() {
                    if m(v, e).abs() > t {
                        d = (e + 1) as u8;
                    }
                }
                *c = (*c).min(d);
            }
        }
        if methods.contains(&MaskType::Decay) {
            for (v, c) in count.iter_mut().enumerate() {
                let mut d = n_echo as u8;
                for e in 1..n_echo {
                    if m(v, e).sub(m(v, e - 1)) >= 0.0 {
                        d = e as u8;
                        break;
                    }
                }
                *c = (*c).min(d);
            }
        }
    }
    let removed = count.iter().filter(|&&c| c < 1).count();
    if removed > 0 {
        log.warning(
            "utils:make_adaptive_mask:195",
            &format!(
                "{removed} voxels in user-defined mask do not have good signal. Removing voxels \
                 from mask."
            ),
        );
    }
    Ok(Adaptive { count })
}

/// §6.2 steps 2 and 3: the exemplar's thresholds (E float32 values).
fn dropout_thresholds(
    means: &[f32],
    n_vox: usize,
    n_echo: usize,
    libraries: Libraries,
    notices: &mut Notices,
) -> Result<Vec<f32>, PyError> {
    let first: Vec<f32> = (0..n_vox).map(|v| means[v * n_echo]).collect();
    let nonzero: Vec<f32> = first.iter().copied().filter(|&x| x != 0.0).collect();
    let argmax_empty = || PyError::value("attempt to get argmax of an empty sequence");
    let nan_present = nonzero.iter().any(|x| x.is_nan());
    let perc = if notices.fix() {
        let finite: Vec<f32> = nonzero.iter().copied().filter(|x| x.is_finite()).collect();
        // an empty mask or first-echo means all 0 are not TED-01's: tedana's IndexError below,
        // in both modes (observed, random testing 2026-11-11)
        if finite.is_empty() && !nonzero.is_empty() {
            return Err(PyError::value(
                "no voxel of the mask has a finite non-zero first-echo mean, so the dropout \
                 thresholds cannot be set (tedana fails with: attempt to get argmax of an empty \
                 sequence)",
            ));
        }
        if finite.len() != nonzero.len() {
            let tedana = percentile_higher(&nonzero, libraries);
            let ours = percentile_higher(&finite, libraries);
            if nan_present || tedana.map(f32::to_bits) != ours.map(f32::to_bits) {
                notices.add(
                    &defects::TED_01,
                    "the dropout thresholds are taken from the voxels whose first-echo mean is \
                     finite (a NaN mean made tedana fail)",
                );
            }
            ours
        } else {
            percentile_higher(&finite, libraries)
        }
    } else {
        if nan_present {
            notices.add(
                &defects::TED_01,
                "a NaN first-echo mean makes the dropout percentile NaN",
            );
        }
        percentile_higher(&nonzero, libraries)
    };
    let Some(perc) = perc else {
        return Err(PyError::new(
            "IndexError",
            "index -1 is out of bounds for axis 0 with size 0",
        ));
    };
    let exemplars: Vec<usize> = (0..n_vox).filter(|&v| first[v] == perc).collect();
    if exemplars.is_empty() {
        return Err(argmax_empty());
    }
    let thirds = |v: usize| -> Vec<f32> {
        (0..n_echo)
            .map(|e| means[v * n_echo + e].div(3.0))
            .collect()
    };
    if exemplars.len() == 1 {
        return Ok(thirds(exemplars[0]));
    }
    // the candidates with the largest float32 sum of the thirds (numpy's argmax: the first
    // largest, a NaN wins)
    let mut best: Option<(usize, f32)> = None;
    for &v in &exemplars {
        let t = thirds(v);
        let s = 0.0f32.add(pairwise_sum(&t, 0, t.len(), 1));
        match best {
            None => best = Some((v, s)),
            Some((_, b)) if b.is_nan() => {}
            Some((_, b)) if s.is_nan() || s > b => best = Some((v, s)),
            _ => {}
        }
    }
    Ok(thirds(best.expect("several exemplars").0))
}

/// numpy's `percentile(a, 33, method="higher")` of float32 values (§13.6): numpy 2.5.4's rank
/// `ceil((n - 1) * 0.33)` in float64; numpy 2.3.5's with `q / 100` and the product in float32.
fn percentile_higher(a: &[f32], libraries: Libraries) -> Option<f32> {
    match libraries {
        Libraries::Newest => larmorx_core::numpy::quantile::percentile_higher(a, 33.0),
        Libraries::Shared => {
            if a.is_empty() {
                return None;
            }
            if let Some(&nan) = a.iter().find(|x| x.is_nan()) {
                return Some(nan);
            }
            let q = 33.0f32 / 100.0f32;
            let rank = ((a.len() - 1) as f32 * q).ceil() as usize;
            let mut v = a.to_vec();
            Some(
                *v.select_nth_unstable_by(rank, |x, y| x.partial_cmp(y).expect("no NaN"))
                    .1,
            )
        }
    }
}

/// `--n-independent-echos k`'s warnings (§6.5), TED-11.
pub fn independent_echo_warnings(
    count: &[u8],
    n_echo: usize,
    k: i128,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<(), PyError> {
    if k == 0 {
        notices.add(&defects::TED_11, "--n-independent-echos 0");
        return Err(if notices.fix() {
            PyError::value(
                "--n-independent-echos must be a positive number of echoes (tedana fails with \
                 ZeroDivisionError)",
            )
        } else {
            PyError::new("ZeroDivisionError", "division by zero")
        });
    }
    let n1 = count.iter().filter(|&&c| c >= 1).count();
    let e = n_echo as f64;
    let kf = k as f64;
    // each percentage is its own place of numpy's warning: with no fit voxel both warn (§6.5,
    // observed)
    let share = |t: f64, place: &'static str, log: &mut Log<'_>| -> (usize, f64) {
        let n = count.iter().filter(|&&c| c >= 1 && (c as f64) < t).count();
        if n1 == 0 {
            log.py_warning(
                place,
                "RuntimeWarning",
                "invalid value encountered in scalar divide",
            );
        }
        (n, 100.0 * n as f64 / n1 as f64)
    };
    let round1 = |x: f64| (x * 10.0).round_ties_even() / 10.0;
    let t3 = (2.0 * e / kf).floor();
    let (n3, p3) = share(t3, "adaptive.rs:p3", log);
    if p3 > 0.0 {
        log.warning(
            "utils:make_adaptive_mask:215",
            &format!(
                "{n3} voxels ({}%) have fewer than {} good voxels. These voxels will be used in \
                 all analyses, but might not include 3 independent echo measurements.",
                float_repr(round1(p3)),
                float_repr(t3.round_ties_even())
            ),
        );
    }
    if k > 3 {
        let td = ((kf - 1.0) * e / kf).floor();
        let (nd, pd) = share(td, "adaptive.rs:pd", log);
        log.warning(
            "utils:make_adaptive_mask:231",
            &format!(
                "{nd} voxels ({}%) have fewer than {} good voxels. The degrees of freedom for \
                 fits across echoes will remain {k} even if there might be fewer independent \
                 echo measurements.",
                float_repr(round1(pd)),
                float_repr(td.round_ties_even())
            ),
        );
    }
    Ok(())
}
