// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! sdcflows' `BSplineApprox` (`sdcflows/interfaces/bspline.py`, sdcflows 2.17.1; Apache-2.0):
//! the B-spline approximation of a field map, its coefficients, the smooth field, the fitting
//! error and the extrapolated field, with numpy's and SciPy's arithmetic where sdcflows uses
//! them (numpy 2.5.4 and 2.3.5: `median`, `mean` and their pairwise float32 sum,
//! `numpy/lib/_function_base_impl.py`, `numpy/_core/_methods.py`,
//! `numpy/_core/src/umath/loops_utils.h.src`; SciPy 1.18.1 and 1.15.3: `scipy.stats.mode`;
//! BSD-3-Clause, notices in [`crate::npy`] and [`crate::ridge`]).
//!
//! What `_run_interface` does, in order, and what this module computes of it:
//!
//! 1. The control grids of every spacing, from the **original** image (`bspline_grid`).
//! 2. `need_resize` when a header zoom is below `zooms_min`: sdcflows resamples the field map
//!    and the mask (`resample_to_zooms`, cubic) and fits on that grid. The resampling is the
//!    caller's ([`Fitting::Resampled`] holds its result); everything else is here.
//! 3. The mask (`dataobj > 1e-4`, all voxels without one; the caller's), the centre
//!    ([`Recenter`]: numpy's median or mean, SciPy's mode, or none) subtracted from the data, the
//!    data outside the mask set to 0.
//! 4. The collocation matrix of every level on the fitting grid and scikit-learn's
//!    `Ridge(alpha, fit_intercept=False)` fit ([`crate::ridge`]). sdcflows fits up to three
//!    times while `max(|coef|) >= 1e4`; the fits are identical, so one is computed and the
//!    debug message repeated as sdcflows logs it; a failed fit is sdcflows' `RuntimeError`.
//! 5. On the original grid (re-read and recentred, not zeroed, after a resize), the field
//!    inside the mask (`colmat[mask] @ coef`, 0 outside), the error `(data - field) * mask`
//!    and, with `extrapolate`, the field outside the mask (`colmat[~mask] @ coef`), or the
//!    field itself when the mask holds every voxel.
//!
//! The header work and the files (`_field`, `_coeff000`, `_error`, `_extra`) are the Python
//! wrapper's.

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::linalg::Mat4;

use crate::bspline::{self, Collocation, ControlGrid, WeightType};
use crate::ridge::{self, RidgeFit};
use crate::{SdcError, npy};

/// `BSplineApprox`'s `recenter` input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Recenter {
    /// `np.median(data[mask])` (the default).
    #[default]
    Median,
    /// `scipy.stats.mode(data[mask]).mode`.
    Mode,
    /// `np.mean(data[mask])`.
    Mean,
    /// `False` (or `True`, which matches none of the modes): no centring.
    None,
}

/// A field-map grid: its data (float32, C order), shape, affine and mask (C order; `None`:
/// every voxel).
#[derive(Clone, Copy, Debug)]
pub struct Grid<'a> {
    pub data: &'a [f32],
    pub shape: [usize; 3],
    pub affine: &'a Mat4,
    pub mask: Option<&'a [bool]>,
}

/// Where sdcflows fits: the field map's own grid, or the grid `resample_to_zooms` gave it
/// (`need_resize`), with the resampled data and mask.
#[derive(Clone, Copy, Debug)]
pub enum Fitting<'a> {
    Original,
    Resampled(Grid<'a>),
}

/// The approximation's inputs (`BSplineApprox`'s, after loading).
#[derive(Clone, Debug)]
pub struct ApproxParams {
    /// The header zooms of the field map (`header.get_zooms()`, all of them).
    pub zooms: Vec<f64>,
    /// `bs_spacing`: one control-point spacing (mm) per level.
    pub spacings: Vec<[f64; 3]>,
    pub ridge_alpha: f64,
    pub recenter: Recenter,
    pub extrapolate: bool,
    /// The input's name in sdcflows' error message (`in_data`).
    pub name: String,
    /// SDC-01 (weights beyond the control grid) corrected or reproduced; the control grids
    /// cover the field map, so only a resampled grid larger than them can reach it.
    pub defects: UpstreamDefects,
    /// Fit with the weights evaluated at every product instead of stored
    /// ([`bspline::StoredCollocation`], the default): O(voxels + knots) memory, slower at one
    /// thread; the same bits.
    pub low_memory: bool,
}

/// A log record sdcflows writes (`nipype.interface`), in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogRecord {
    Info(String),
    Debug(String),
}

/// What `BSplineApprox` computes.
#[derive(Clone, Debug)]
pub struct Approx {
    /// The control grids (the coefficients images' shapes and affines).
    pub grids: Vec<ControlGrid>,
    /// The coefficients of each level, C order of its grid.
    pub coefficients: Vec<Vec<f32>>,
    /// The centre subtracted (NaN when the mask is empty, as numpy's and SciPy's).
    pub center: f32,
    /// Voxels inside the mask on the fitting grid.
    pub n_inside: usize,
    /// The smooth field on the original grid (0 outside the mask).
    pub field: Vec<f32>,
    /// `(data - field) * mask`.
    pub error: Vec<f32>,
    /// The field with its extrapolation outside the mask; `None` without `extrapolate`. Equal
    /// to `field` when the mask holds every voxel (sdcflows then returns the field's file).
    pub extrapolated: Option<Vec<f32>>,
    /// `max(|coef|)`.
    pub extreme: f32,
    pub fit: RidgeFit,
    pub log: Vec<LogRecord>,
    /// Whether SDC-01 changed (or, reproduced, would have changed) the weights.
    pub beyond: bool,
}

/// numpy's pairwise float32 sum (`FLOAT_pairwise_sum`): blocks of up to 128 values summed in
/// eight lanes, halves split at multiples of eight; the reduction adds it to +0.
fn pairwise_sum(a: &[f32]) -> f32 {
    fn pw(a: &[f32]) -> f32 {
        let n = a.len();
        if n < 8 {
            let mut res = -0.0f32;
            for &v in a {
                res += v;
            }
            res
        } else if n <= 128 {
            let mut r: [f32; 8] = std::array::from_fn(|j| a[j]);
            let end = n - n % 8;
            let mut i = 8;
            while i < end {
                for (j, rj) in r.iter_mut().enumerate() {
                    *rj += a[i + j];
                }
                i += 8;
            }
            let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
            for &v in &a[i..] {
                res += v;
            }
            res
        } else {
            let mut n2 = n / 2;
            n2 -= n2 % 8;
            pw(&a[..n2]) + pw(&a[n2..])
        }
    }
    0.0 + pw(a)
}

/// `np.mean(a)` of float32 values (`numpy/_core/_methods.py`, `_mean`): the pairwise float32
/// sum divided by the count, an `intp`, so the quotient is taken in float64 and rounded to
/// float32 (`ret.dtype.type(ret / rcount)`).
pub fn mean_f32(a: &[f32]) -> f32 {
    if a.is_empty() {
        // 0.0 / 0 in float64: x86's default NaN (the sign bit set), rounded to float32.
        return X86_DEFAULT_NAN_F32;
    }
    (f64::from(pairwise_sum(a)) / a.len() as f64) as f32
}

/// `np.median(a)` of float32 values: the mean of an empty slice (x86's default NaN) when there
/// is none; a NaN of the values when one is NaN (`_median_nancheck` returns the partition's
/// last element; the first NaN here, the same bits when the NaNs are alike); else the mean of
/// the middle one or two values of the sorted order (`mean` of one or two float32 values:
/// `(0 + a [+ b]) / count`).
pub fn median_f32(a: &[f32]) -> f32 {
    if a.is_empty() {
        return X86_DEFAULT_NAN_F32;
    }
    if let Some(&nan) = a.iter().find(|v| v.is_nan()) {
        return nan;
    }
    let mut v = a.to_vec();
    let n = v.len();
    let hi = n / 2;
    let (_, &mut m_hi, _) = v.select_nth_unstable_by(hi, f32::total_cmp);
    if n % 2 == 1 {
        return mean_f32(&[m_hi]);
    }
    // the largest value below position `hi`
    let m_lo = v[..hi].iter().copied().fold(f32::NEG_INFINITY, |acc, x| {
        if x.total_cmp(&acc).is_gt() { x } else { acc }
    });
    mean_f32(&[m_lo, m_hi])
}

/// `scipy.stats.mode(a).mode` of float32 values: the smallest of the most frequent values,
/// all NaNs counted as one value after every number (`np.nan` for no values; a NaN of the
/// values when they are the most frequent); a zero mode takes the more frequent sign of the
/// zeros (numpy's unstable sort leaves the first one undefined).
pub fn mode_f32(a: &[f32]) -> f32 {
    if a.is_empty() {
        return f32::NAN;
    }
    let n_nan = a.iter().filter(|v| v.is_nan()).count();
    let mut v: Vec<f32> = a.iter().copied().filter(|x| !x.is_nan()).collect();
    v.sort_unstable_by(f32::total_cmp);
    let (mut best, mut best_count) = (f32::NAN, 0usize);
    let mut i = 0;
    while i < v.len() {
        let mut j = i + 1;
        while j < v.len() && v[j] == v[i] {
            j += 1;
        }
        if j - i > best_count {
            best = v[i];
            best_count = j - i;
        }
        i = j;
    }
    if n_nan > best_count {
        return a.iter().copied().find(|v| v.is_nan()).unwrap_or(f32::NAN);
    }
    if best == 0.0 {
        // +0 and -0 are one value; which of them np.unique's sort (numpy's unstable SIMD
        // quicksort) puts first is not defined: the more frequent sign (+0 on a tie), which is
        // numpy's on the trial's cases.
        let negative = v
            .iter()
            .filter(|x| **x == 0.0 && x.is_sign_negative())
            .count();
        let positive = v.iter().filter(|x| **x == 0.0).count() - negative;
        return if negative > positive { -0.0 } else { 0.0 };
    }
    best
}

/// The NaN an x86 CPU makes for an invalid operation such as `0.0 / 0.0` (the "default NaN":
/// quiet, the sign bit set), as numpy's `mean` of an empty slice returns it on x86-64.
pub const X86_DEFAULT_NAN_F32: f32 = f32::from_bits(0xffc0_0000);

fn center_of(data: &[f32], mask: Option<&[bool]>, recenter: Recenter) -> f32 {
    if recenter == Recenter::None {
        return 0.0;
    }
    let inside: Vec<f32> = match mask {
        None => data.to_vec(),
        Some(m) => data
            .iter()
            .zip(m)
            .filter(|(_, k)| **k)
            .map(|(v, _)| *v)
            .collect(),
    };
    match recenter {
        Recenter::Median => median_f32(&inside),
        Recenter::Mean => mean_f32(&inside),
        Recenter::Mode => mode_f32(&inside),
        Recenter::None => 0.0,
    }
}

fn rows_where(mask: Option<&[bool]>, n: usize, value: bool) -> Vec<usize> {
    match mask {
        None => {
            if value {
                (0..n).collect()
            } else {
                Vec::new()
            }
        }
        Some(m) => (0..n).filter(|&i| m[i] == value).collect(),
    }
}

fn collocation(
    shape: [usize; 3],
    affine: &Mat4,
    grids: &[ControlGrid],
    defects: UpstreamDefects,
    beyond: &mut bool,
) -> Result<Collocation, SdcError> {
    let mut levels = Vec::with_capacity(grids.len());
    for g in grids {
        let w = bspline::grid_bspline_weights(
            shape,
            affine,
            g.shape,
            &g.affine,
            WeightType::Float32,
            defects,
        )?;
        *beyond |= w.beyond > 0;
        levels.push(w);
    }
    Collocation::new(levels)
}

fn check_grid(g: &Grid<'_>) -> Result<usize, SdcError> {
    let n: usize = g.shape.iter().product();
    if g.data.len() != n || g.mask.is_some_and(|m| m.len() != n) {
        return Err(SdcError::Invalid(
            "the data and the mask must hold one value per voxel of the grid".into(),
        ));
    }
    Ok(n)
}

/// sdcflows' `BSplineApprox` on a loaded field map (`original`: its grid, data and mask) and,
/// after a resize, the resampled grid it fits on.
pub fn bspline_approx(
    original: Grid<'_>,
    fitting: Fitting<'_>,
    params: &ApproxParams,
    n_threads: usize,
) -> Result<Approx, SdcError> {
    let mut log = Vec::new();
    let mut out = bspline_approx_logged(original, fitting, params, n_threads, &mut log)?;
    out.log = log;
    Ok(out)
}

/// [`bspline_approx`] with its log records pushed to `log` as sdcflows writes them, also when
/// it fails (sdcflows logs the fit's description and every attempt before raising). The
/// returned [`Approx::log`] is empty.
pub fn bspline_approx_logged(
    original: Grid<'_>,
    fitting: Fitting<'_>,
    params: &ApproxParams,
    n_threads: usize,
    log: &mut Vec<LogRecord>,
) -> Result<Approx, SdcError> {
    let n_orig = check_grid(&original)?;
    let mut grids = Vec::with_capacity(params.spacings.len());
    for s in &params.spacings {
        grids.push(bspline::bspline_grid(
            original.shape,
            original.affine,
            &params.zooms,
            *s,
        )?);
    }
    if grids.is_empty() {
        return Err(SdcError::Invalid(
            "need at least one array to concatenate".into(),
        ));
    }
    let fit_grid = match fitting {
        Fitting::Original => original,
        Fitting::Resampled(g) => g,
    };
    let n_fit = check_grid(&fit_grid)?;
    let center = center_of(fit_grid.data, fit_grid.mask, params.recenter);
    let mut y: Vec<f32> = fit_grid.data.iter().map(|v| v - center).collect();
    if let Some(m) = fit_grid.mask {
        for (v, &k) in y.iter_mut().zip(m) {
            if !k {
                *v = 0.0;
            }
        }
    }
    let n_inside = fit_grid
        .mask
        .map_or(n_fit, |m| m.iter().filter(|k| **k).count());
    let mut beyond = false;
    let colmat = collocation(
        fit_grid.shape,
        fit_grid.affine,
        &grids,
        params.defects,
        &mut beyond,
    )?;
    let mut knots: Vec<String> = grids
        .iter()
        .map(|g| format!("{}x{}x{}", g.shape[0], g.shape[1], g.shape[2]))
        .collect();
    if let Some(last) = knots.last_mut() {
        *last = format!("and {last}");
    }
    let [s0, s1, s2] = fit_grid.shape;
    log.push(LogRecord::Info(format!(
        "Approximating B-Splines grids ({} [knots]) on a grid of {s0}x{s1}x{s2} ({n_fit}) \
         voxels, of which {n_inside} fall within the mask.",
        knots.join(", ")
    )));
    if !y.iter().all(|v| v.is_finite()) {
        let what = if y.iter().any(|v| v.is_nan()) {
            "NaN".to_string()
        } else {
            "infinity or a value too large for dtype('float32')".to_string()
        };
        return Err(SdcError::Invalid(format!("Input y contains {what}.")));
    }
    let stored = if params.low_memory {
        None
    } else {
        colmat.store(n_threads)?
    };
    let fit = match &stored {
        Some(m) => ridge::ridge_sparse_cg(m, &y, params.ridge_alpha, 1e-4, None, n_threads)?,
        None => ridge::ridge_sparse_cg(&colmat, &y, params.ridge_alpha, 1e-4, None, n_threads)?,
    };
    // The stored matrix also evaluates the fit when it was made on the original grid (every row
    // in one product; a row's value does not depend on the rows beside it).
    let evaluated = match (&stored, &fitting) {
        (Some(m), Fitting::Original) => Some(m.matvec_f32(&fit.coef, n_threads)?),
        _ => None,
    };
    drop(stored);
    let extreme = fit.coef.iter().fold(0.0f32, |acc, c| {
        let a = c.abs();
        if acc.is_nan() || a.is_nan() {
            f32::NAN
        } else if a > acc {
            a
        } else {
            acc
        }
    });
    let ok = extreme < 1e4;
    for attempt in 0..(if ok { 1 } else { 3 }) {
        log.push(LogRecord::Debug(format!(
            "Model fit attempt {attempt}: max(|coeffs|) = {}",
            // an f-string formats numpy's float32 through Python's float
            npy::py_repr(f64::from(extreme))
        )));
    }
    if !ok {
        return Err(SdcError::Runtime(format!(
            "Spline fit of input file {} failed. Extreme value {} detected in spline \
             coefficients.",
            params.name,
            npy::py_format_2e(f64::from(extreme))
        )));
    }
    let mut coefficients = Vec::with_capacity(grids.len());
    let mut index = 0;
    for g in &grids {
        let n: usize = g.shape.iter().product();
        coefficients.push(fit.coef[index..index + n].to_vec());
        index += n;
    }
    // Evaluation on the original grid.
    let (data, mask, colmat) = match fitting {
        Fitting::Original => (y, original.mask, colmat),
        Fitting::Resampled(_) => {
            let d: Vec<f32> = original.data.iter().map(|v| v - center).collect();
            let c = collocation(
                original.shape,
                original.affine,
                &grids,
                params.defects,
                &mut beyond,
            )?;
            (d, original.mask, c)
        }
    };
    let inside = rows_where(mask, n_orig, true);
    let mut field = vec![0.0f32; n_orig];
    match &evaluated {
        Some(all) => {
            for &r in &inside {
                field[r] = all[r];
            }
        }
        None => {
            for (&r, v) in
                inside
                    .iter()
                    .zip(colmat.matvec_f32(&fit.coef, Some(&inside), n_threads)?)
            {
                field[r] = v;
            }
        }
    }
    let error: Vec<f32> = (0..n_orig)
        .map(|i| {
            let m = mask.is_none_or(|m| m[i]);
            (data[i] - field[i]) * if m { 1.0 } else { 0.0 }
        })
        .collect();
    let extrapolated = if !params.extrapolate {
        None
    } else if inside.len() == n_orig {
        Some(field.clone())
    } else {
        let outside = rows_where(mask, n_orig, false);
        let mut extra = field.clone();
        match &evaluated {
            Some(all) => {
                for &r in &outside {
                    extra[r] = all[r];
                }
            }
            None => {
                for (&r, v) in
                    outside
                        .iter()
                        .zip(colmat.matvec_f32(&fit.coef, Some(&outside), n_threads)?)
                {
                    extra[r] = v;
                }
            }
        }
        Some(extra)
    };
    Ok(Approx {
        grids,
        coefficients,
        center,
        n_inside,
        field,
        error,
        extrapolated,
        extreme,
        fit,
        log: Vec::new(),
        beyond,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numpy_reductions() {
        assert_eq!(median_f32(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median_f32(&[4.0, 1.0, 3.0, 2.0]), 2.5);
        assert!(median_f32(&[1.0, f32::NAN]).is_nan());
        assert_eq!(median_f32(&[-0.0]).to_bits(), 0.0f32.to_bits());
        assert_eq!(mode_f32(&[2.0, 1.0, 2.0, 1.0, 3.0]), 1.0);
        assert!(mode_f32(&[f32::NAN, f32::NAN, 1.0]).is_nan());
        assert_eq!(mode_f32(&[f32::NAN, 1.0]), 1.0);
        assert_eq!(mean_f32(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        // 9 values: the eight lanes, then the ninth (not the first value plus the rest).
        let a: Vec<f32> = (0..9).map(|i| 1.0 + (i as f32) * 1e-7).collect();
        let r: Vec<f32> = a[..8].to_vec();
        let want =
            0.0 + ((((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]))) + a[8]);
        assert_eq!(pairwise_sum(&a).to_bits(), want.to_bits());
    }

    #[test]
    fn numpys_nan_of_no_values_and_of_nan_values() {
        // np.median / np.mean of an empty slice: 0.0 / 0, x86's default NaN (sign bit set).
        assert_eq!(median_f32(&[]).to_bits(), 0xffc0_0000);
        assert_eq!(mean_f32(&[]).to_bits(), 0xffc0_0000);
        // a NaN among the values is returned as it is (here a positive one)
        assert_eq!(
            median_f32(&[1.0, f32::NAN, 2.0]).to_bits(),
            f32::NAN.to_bits()
        );
        // scipy.stats.mode of no values: np.nan
        assert_eq!(mode_f32(&[]).to_bits(), f32::NAN.to_bits());
        let neg = f32::from_bits(0xffc0_0001);
        assert_eq!(mode_f32(&[neg, neg, 1.0]).to_bits(), neg.to_bits());
        // a zero mode: the more frequent sign
        assert_eq!(mode_f32(&[-0.0, 0.0, 0.0, 1.0]).to_bits(), 0.0f32.to_bits());
        assert_eq!(
            mode_f32(&[-0.0, -0.0, 0.0, 1.0]).to_bits(),
            (-0.0f32).to_bits()
        );
    }
}
