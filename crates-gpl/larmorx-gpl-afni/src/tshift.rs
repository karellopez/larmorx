// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/3dTshift.c (the shifting loop of main),
// src/thd_dsetto1D.c (THD_extract_series, THD_extract_array), src/thd_1Dtodset.c
// (THD_insert_series), src/thd_initdblk.c (THD_need_brick_factor) and the SHORTIZE and
// BYTEIZE macros of src/mrilib.h.
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! 3dTshift's slice-timing correction, bit for bit.
//!
//! Every slice whose shift is at least 0.001 TR (or every slice, with `-rlt`/`-rlt+`) is
//! processed voxel pair by voxel pair, `(0, 1), (2, 3), ...` within the slice, as 3dTshift does:
//!
//! 1. each series is extracted as float (`THD_extract_series`: non-finite values become 0, a
//!    positive brick factor is applied);
//! 2. the range of the used points is recorded (`-ignore` points are left out);
//! 3. the linear trend is removed (`THD_linear_detrend`), or with `-no_detrend` the mean
//!    (`THD_const_detrend`, applied to the *first* voxel of the pair twice: AFNI's bug);
//! 4. the detrended range is recorded;
//! 5. both series are shifted together (`SHIFT_two_rows`);
//! 6. each value is clipped to the detrended range, the trend (or the mean) is added back and
//!    the result clipped to the input range;
//! 7. the series is stored back (`THD_insert_series`: divided by the brick factor in double,
//!    then `SHORTIZE`/`BYTEIZE` for integer data).
//!
//! Pairs are independent, so they run in parallel; the result does not depend on the number
//! of threads.

use larmorx_core::parallel::{self, ThreadPoolError};
use rayon::prelude::*;

use crate::csfft::{csfft_cox_handles, csfft_nextup_one35};
use crate::dataset::{BrickData, Dataset};
use crate::detrend::{const_detrend, linear_detrend};
use crate::shift::{Method, ShiftWork, shift_two_rows};

/// What is added back after shifting (`TS_rlt`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rlt {
    /// 0: the trend, then clipping to the input range (default).
    #[default]
    Trend,
    /// 1 (`-rlt`): nothing.
    Nothing,
    /// 2 (`-rlt+`, and `-no_detrend`): the mean (the trend's `f0`).
    Mean,
}

/// 3dTshift's settings after the command line and the dataset were read. Times are in the
/// units of `tr`.
#[derive(Clone, Debug, PartialEq)]
pub struct TshiftParams {
    /// `TS_TR`.
    pub tr: f32,
    /// `TS_tpat`: the time offset of each slice.
    pub tpat: Vec<f32>,
    /// `TS_tzero`: the common time origin.
    pub tzero: f32,
    /// `TS_ignore`.
    pub ignore: usize,
    pub method: Method,
    pub rlt: Rlt,
    /// `TS_detrend`: remove the linear trend (true), or only the mean (`-no_detrend`).
    pub detrend: bool,
    /// The hidden `-BAD` option: shift the wrong way.
    pub bad: bool,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
}

/// [`tshift`] could not run.
#[derive(Debug, thiserror::Error)]
pub enum TshiftError {
    #[error("{times} slice times for {slices} slices")]
    SliceTimes { times: usize, slices: usize },
    #[error("-ignore value {ignore} is too large")]
    Ignore { ignore: usize },
    #[error(
        "FFT length {0}: AFNI computes it with its fftn package, which larmorx-gpl does not \
         include (time series longer than 32764 points)"
    )]
    FftLength(usize),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// The fractional shift of slice `kk`: `-(tzero - tpat[kk]) / TR` in float (`-BAD` keeps the
/// sign of the "rightward" shift).
pub fn slice_shift(p: &TshiftParams, kk: usize) -> f32 {
    let fshift = (p.tzero - p.tpat[kk]) / p.tr;
    if p.bad { fshift } else { -fshift }
}

/// Whether slice `kk` is left alone: `fabs(fshift) < 0.001 && !TS_rlt` (compared in double).
pub fn skipped(p: &TshiftParams, fshift: f32) -> bool {
    f64::from(fshift.abs()) < 0.001 && p.rlt == Rlt::Trend
}

/// The FFT length 3dTshift uses for `ntt` time points: `csfft_nextup_one35(ntt + 4)`.
pub fn fft_length(ntt: usize) -> usize {
    csfft_nextup_one35(ntt + 4)
}

/// `SHORTIZE(xx)`: clamp to ±32767, then `rint` (round half to even).
#[inline]
pub fn shortize(xx: f32) -> i16 {
    if xx < -32767.0 {
        -32767
    } else if xx > 32767.0 {
        32767
    } else {
        // (short)rint(xx); a NaN becomes 0, as the x86 conversion gives.
        f64::from(xx).round_ties_even() as i16
    }
}

/// `BYTEIZE(xx)`: 0 below 0, 255 above 255, else `rintf`.
#[inline]
pub fn byteize(xx: f32) -> u8 {
    if f64::from(xx) < 0.0 {
        0
    } else if xx > 255.0 {
        255
    } else {
        xx.round_ties_even() as u8
    }
}

/// Runs 3dTshift's shifting loop on `ds` in place.
pub fn tshift(ds: &mut Dataset, p: &TshiftParams) -> Result<(), TshiftError> {
    let [nx, ny, nz] = ds.nxyz;
    let ntt = ds.nvals;
    if p.tpat.len() != nz {
        return Err(TshiftError::SliceTimes {
            times: p.tpat.len(),
            slices: nz,
        });
    }
    if ntt < 5 || p.ignore > ntt - 5 {
        return Err(TshiftError::Ignore { ignore: p.ignore });
    }
    let nup = fft_length(ntt);
    if p.method == Method::Fourier && !csfft_cox_handles(nup) {
        return Err(TshiftError::FftLength(nup));
    }
    let nxy = nx * ny;
    let nvox = nxy * nz;
    let need = ds.need_brick_factor();
    let factors = ds.factors.clone();
    let mut slice = vec![0.0f32; nxy * ntt];
    let data = &mut ds.data;
    parallel::with_threads(p.n_threads, || {
        for kk in 0..nz {
            let fshift = slice_shift(p, kk);
            if skipped(p, fshift) {
                continue;
            }
            for v in 0..nxy {
                extract(
                    data,
                    &factors,
                    need,
                    v + kk * nxy,
                    nvox,
                    &mut slice[v * ntt..(v + 1) * ntt],
                );
            }
            slice.par_chunks_mut(2 * ntt).with_min_len(8).for_each_init(
                ShiftWork::default,
                |work, pair| {
                    let (far, gar) = pair.split_at_mut(ntt);
                    let gar = (!gar.is_empty()).then_some(gar);
                    shift_pair(far, gar, fshift, nup, p, work);
                },
            );
            for v in 0..nxy {
                insert(
                    data,
                    &factors,
                    need,
                    v + kk * nxy,
                    nvox,
                    &slice[v * ntt..(v + 1) * ntt],
                );
            }
        }
    })?;
    Ok(())
}

/// `THD_extract_series(ind, dset, 0)`: the series as float, non-finite values set to 0
/// (`thd_floatscan`), multiplied by each positive brick factor when the dataset needs brick
/// factors, and scanned again (`MRI_floatscan`).
fn extract(
    data: &BrickData,
    factors: &[f32],
    need: bool,
    ind: usize,
    nvox: usize,
    far: &mut [f32],
) {
    for (t, v) in far.iter_mut().enumerate() {
        let x = match data {
            BrickData::Byte(a) => f32::from(a[ind + t * nvox]),
            BrickData::Short(a) => f32::from(a[ind + t * nvox]),
            BrickData::Float(a) => a[ind + t * nvox],
        };
        *v = if x.is_finite() { x } else { 0.0 };
    }
    if need {
        for (v, &fac) in far.iter_mut().zip(factors) {
            if f64::from(fac) > 0.0 {
                *v *= fac;
            }
        }
    }
    for v in far.iter_mut() {
        if !v.is_finite() {
            *v = 0.0;
        }
    }
}

/// `THD_insert_series(ind, dset, ntt, MRI_float, far, 0)`: each value times `1.0/factor` (in
/// double, when the dataset needs brick factors and the factor is not 0), then stored in the
/// dataset's type.
fn insert(data: &mut BrickData, factors: &[f32], need: bool, ind: usize, nvox: usize, far: &[f32]) {
    for (t, &v) in far.iter().enumerate() {
        let v = if need {
            let fac = if f64::from(factors[t]) != 0.0 {
                1.0 / f64::from(factors[t])
            } else {
                1.0
            };
            (f64::from(v) * fac) as f32
        } else {
            v
        };
        match data {
            BrickData::Float(a) => a[ind + t * nvox] = v,
            BrickData::Short(a) => a[ind + t * nvox] = shortize(v),
            BrickData::Byte(a) => a[ind + t * nvox] = byteize(v),
        }
    }
}

/// The range of a series as 3dTshift scans it (`if <min ... else if >max`).
fn range(x: &[f32]) -> (f32, f32) {
    let (mut lo, mut hi) = (x[0], x[0]);
    for &v in &x[1..] {
        if v < lo {
            lo = v;
        } else if v > hi {
            hi = v;
        }
    }
    (lo, hi)
}

/// What 3dTshift records about one series before shifting it.
struct Prep {
    /// `ffmin`, `ffmax` (with the default `-rlt` only).
    full: (f32, f32),
    /// `f0`, `f1`.
    trend: (f32, f32),
    /// `fmin`, `fmax`: the range after detrending.
    after: (f32, f32),
}

/// One pass of 3dTshift's voxel loop: the pair `far`, `gar` (the second voxel, absent for the
/// last voxel of a slice with an odd number of voxels).
fn shift_pair(
    far: &mut [f32],
    mut gar: Option<&mut [f32]>,
    fshift: f32,
    nup: usize,
    p: &TshiftParams,
    work: &mut ShiftWork,
) {
    let ig = p.ignore;
    let restore_full = p.rlt == Rlt::Trend;
    let full = |x: &[f32]| {
        if restore_full {
            range(&x[ig..])
        } else {
            (0.0, 0.0)
        }
    };
    // First voxel.
    let f_full = full(far);
    let f_trend = if p.detrend {
        linear_detrend(&mut far[ig..]).unwrap_or((0.0, 0.0))
    } else {
        (const_detrend(&mut far[ig..]).unwrap_or(0.0), 0.0)
    };
    let f = Prep {
        full: f_full,
        trend: f_trend,
        after: range(&far[ig..]),
    };
    // Second voxel.
    let g = gar.as_deref_mut().map(|gar| {
        let g_full = full(gar);
        let g_trend = if p.detrend {
            linear_detrend(&mut gar[ig..]).unwrap_or((0.0, 0.0))
        } else {
            // 3dTshift.c:632-633: `THD_const_detrend( ntt-ignore , far+ignore , &g0 )`: the
            // first voxel is de-meaned again, its new mean (about 0) becomes g0, and the second
            // voxel keeps its mean.
            (const_detrend(&mut far[ig..]).unwrap_or(0.0), 0.0)
        };
        Prep {
            full: g_full,
            trend: g_trend,
            after: range(&gar[ig..]),
        }
    });
    shift_two_rows(
        p.method,
        nup,
        fshift,
        &mut far[ig..],
        fshift,
        gar.as_deref_mut().map(|g| &mut g[ig..]),
        work,
    );
    restore(&mut far[ig..], &f, p.rlt);
    if let (Some(gar), Some(g)) = (gar, g) {
        restore(&mut gar[ig..], &g, p.rlt);
    }
}

/// Clip to the detrended range, then restore per `rlt` (`far[jj] += f0 + (jj-ignore)*f1`,
/// float) and clip to the input range.
fn restore(x: &mut [f32], s: &Prep, rlt: Rlt) {
    let (fmin, fmax) = s.after;
    let (f0, f1) = s.trend;
    for (jj, v) in x.iter_mut().enumerate() {
        if *v < fmin {
            *v = fmin;
        } else if *v > fmax {
            *v = fmax;
        }
        match rlt {
            Rlt::Trend => {
                *v += f0 + jj as f32 * f1;
                if *v < s.full.0 {
                    *v = s.full.0;
                } else if *v > s.full.1 {
                    *v = s.full.1;
                }
            }
            Rlt::Mean => *v += f0,
            Rlt::Nothing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::Geometry;

    #[test]
    fn storing_rounds_half_to_even_and_clamps() {
        assert_eq!(shortize(2.5), 2);
        assert_eq!(shortize(3.5), 4);
        assert_eq!(shortize(-2.5), -2);
        assert_eq!(shortize(-40000.0), -32767);
        assert_eq!(shortize(40000.0), 32767);
        assert_eq!(shortize(f32::NAN), 0);
        assert_eq!(byteize(-3.0), 0);
        assert_eq!(byteize(255.7), 255);
        assert_eq!(byteize(0.5), 0);
        assert_eq!(byteize(1.5), 2);
    }

    fn dataset(nx: usize, nz: usize, nt: usize) -> Dataset {
        let n = nx * nz * nt;
        let data: Vec<f32> = (0..n)
            .map(|i| ((i * 7919) % 1000) as f32 * 0.37 + 100.0)
            .collect();
        Dataset {
            nxyz: [nx, 1, nz],
            nvals: nt,
            data: BrickData::Float(data),
            factors: vec![0.0; nt],
            taxis: None,
            geometry: Geometry {
                matrix: [[0.0; 4]; 4],
                steps: [1.0; 3],
                code: 1,
            },
        }
    }

    fn params(nz: usize, method: Method, n_threads: usize) -> TshiftParams {
        TshiftParams {
            tr: 2.0,
            tpat: (0..nz).map(|k| k as f32 * 2.0 / nz as f32).collect(),
            tzero: 0.9,
            ignore: 2,
            method,
            rlt: Rlt::Trend,
            detrend: true,
            bad: false,
            n_threads,
        }
    }

    #[test]
    fn results_do_not_depend_on_the_thread_count() {
        for method in [Method::Fourier, Method::Heptic, Method::Wsinc9] {
            let mut a = dataset(37, 3, 40);
            let mut b = a.clone();
            tshift(&mut a, &params(3, method, 1)).unwrap();
            tshift(&mut b, &params(3, method, 4)).unwrap();
            assert_eq!(a.data, b.data, "{method:?}");
            assert_ne!(a.data, dataset(37, 3, 40).data);
        }
    }

    #[test]
    fn ignored_points_and_skipped_slices_are_kept() {
        let orig = dataset(4, 2, 20);
        let mut ds = orig.clone();
        let mut p = params(2, Method::Cubic, 1);
        p.tpat = vec![0.9, 0.0];
        tshift(&mut ds, &p).unwrap();
        let (BrickData::Float(a), BrickData::Float(b)) = (&orig.data, &ds.data) else {
            unreachable!()
        };
        let nvox = 8;
        for t in 0..20 {
            for v in 0..4 {
                // slice 0 has shift 0: untouched.
                assert_eq!(a[v + t * nvox], b[v + t * nvox]);
                // slice 1: the two ignored points are kept.
                if t < 2 {
                    assert_eq!(a[4 + v + t * nvox], b[4 + v + t * nvox]);
                }
            }
        }
    }

    #[test]
    fn checks() {
        let mut ds = dataset(2, 2, 6);
        let mut p = params(2, Method::Linear, 1);
        p.ignore = 2;
        assert!(matches!(
            tshift(&mut ds, &p),
            Err(TshiftError::Ignore { .. })
        ));
        p.tpat = vec![0.0];
        p.ignore = 0;
        assert!(matches!(
            tshift(&mut ds, &p),
            Err(TshiftError::SliceTimes { .. })
        ));
    }
}
