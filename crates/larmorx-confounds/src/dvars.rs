// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! nipype's `compute_dvars` (nipype 1.12.0 `algorithms/confounds.py`, the same in every
//! environment) on the masked series, bit for bit with the libraries it calls in each
//! environment (numpy 2.5.4, 2.3.5 and 2.2.6's reductions and LAPACK; SciPy 1.18.1's and 1.15's
//! FFTs and 1 × 1 solve; nitime's AR(1)):
//!
//! 1. `mfunc = func[mask]` (float32, one row per voxel in the mask's C order: the caller's);
//!    unless `intensity_normalization == 0`, `(mfunc / np.median(mfunc)) * intensity_normalization`
//!    (float32; the factor a weak Python float, rounded to float32).
//! 2. `func_sd = (np.percentile(mfunc, 75, axis=1, method="lower") - np.percentile(..., 25,
//!    ...)) / 1.349` (float32); with `remove_zerovariance`, only the voxels with `func_sd >
//!    variance_tol` (a float32 comparison: the weak Python float rounded to float32) stay.
//! 3. `ar1 = apply_along_axis(_AR_est_YW, 1, regress_poly(0, mfunc)[0].astype(float32), 1)`:
//!    `regress_poly(0, ...)` is `data - X.dot(pinv(X).dot(data.T)).T` with `X = ones((T, 1))`:
//!    numpy's `pinv` (LAPACK `DGESDD`), `np.dot` of the `1 × T` row with the float64 copy of
//!    `data.T` (OpenBLAS's `dgemv_t`, columns in groups of four: one voxel alone is `0 +
//!    ddot`), the outer product with the ones (`dgemm` of depth one or `daxpy`: `beta + 0`),
//!    the float64 difference rounded to float32; then nitime's `AR_est_YW(x, 1)` per voxel
//!    ([`crate::nitime`]), in voxel order (the first voxel that fails stops the computation).
//! 4. `diff_sdhat = squeeze(sqrt(((1 - ar1) * 2).tolist())) * func_sd` (twice `1 - ar1` in
//!    float32, its float64 square root, times `func_sd` in float64), `diff_sd_mean =
//!    diff_sdhat.mean()` (numpy 2.2 sums in chunks of 8192).
//! 5. `func_diff = np.diff(mfunc, axis=1)` (float32), `dvars_nstd = sqrt(square(func_diff).mean(
//!    axis=0))` (float32, summed over the voxels in order, divided by the count in float64),
//!    `dvars_stdz = dvars_nstd / diff_sd_mean` (float64).
//! 6. Under `warnings.filterwarnings("error")`: `diff_vx_stdz = square(func_diff / diff_sdhat)`
//!    (float64) and `dvars_vx_stdz = sqrt(diff_vx_stdz.mean(axis=0))`: a floating-point warning
//!    of the division, the square or the sum stops the computation (`RuntimeWarning`, the first
//!    of divide by zero, overflow, invalid value, in numpy's order).
//!
//! **Corrected by default** ([`UpstreamDefects::Fix`]; [`UpstreamDefects::Reproduce`] gives
//! nipype's result):
//!
//! - PIPE-50: a voxel whose demeaned series is all zeros (`r0 = 0`) gets the AR(1) coefficient 0,
//!   the minimum-norm solution of `0 a = 0` (its predicted SD is `sqrt(2) · 0 = 0`, as its robust
//!   SD is 0); nipype gets NaN (SciPy 1.15: every standardised value NaN) or `LinAlgError`
//!   (SciPy 1.18).
//! - SCI-41: the other cases of SciPy 1.15's 1 × 1 solve take SciPy 1.18's checks (an infinite
//!   or NaN lag-1 autocorrelation is refused as SciPy 1.18 refuses it).
//! - PIPE-47: an AR(1) estimate above 1 (float32 rounding) is taken as 1 (a predicted SD of 0),
//!   not NaN.
//! - PIPE-46: the voxel-wise standardisation never stops the computation: the voxels whose
//!   predicted SD is 0 are left out of its mean (NaN when none is left), any other overflow stays
//!   IEEE's infinity.
//!
//! Upstream: nipype 1.12.0 (PyPI sdist `nipype-1.12.0.tar.gz`, SHA-256
//! `784098a8a54839836fa0eba2fa1c2040a365a272d0b28692578b5b15b475aeeb`), Copyright (c) 2009-2016,
//! Nipype developers, Apache-2.0; numpy 2.5.4 `numpy/_core/src/common/cblasfuncs.c`
//! (`cblas_matrixproduct`'s dispatch), `numpy/lib/_shape_base_impl.py` (`apply_along_axis`),
//! `numpy/_core/_methods.py` (`_mean`), Copyright (c) 2005-2025, NumPy Developers,
//! BSD-3-Clause (the notice: [`crate::nitime`], `PROVENANCE.md`, NOTICE). The logic is
//! re-expressed in Rust; no code was copied.

use crate::nitime::{CrosscovPlan, SolveError, SolveRelease, scipy_solve_1x1};
use crate::scipy_fft::{FftError, FftLibrary};
use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::numpy::quantile::{RankMethod, median, percentile_rank_axis};
use larmorx_core::numpy::reduce::{SumOrder, mean_ordered};
use larmorx_core::numpy::{NpFloat, NumpyRelease};
use larmorx_core::parallel::{ThreadPoolError, with_threads};
use larmorx_transform::numpy_linalg::blocked::Lapack;
use larmorx_transform::numpy_linalg::svd::numpy_pinv;
use larmorx_transform::openblas::level2::{ddot, dgemv};
use larmorx_transform::openblas::level3::Trans;
use larmorx_transform::openblas::matmul::Array2;
use rayon::prelude::*;

/// Voxels per parallel chunk of the AR(1) loop (a multiple of four: `dgemv_t` computes the
/// columns in groups of four, so a chunk's columns take the kernels they take in the whole
/// product).
const CHUNK: usize = 256;

/// The settings of [`compute_dvars`]: nipype's arguments and the environment's libraries.
#[derive(Clone, Copy, Debug)]
pub struct DvarsOptions {
    /// `remove_zerovariance` (the function's default `False`; `ComputeDVARS`' `True`).
    pub remove_zerovariance: bool,
    /// `variance_tol` (the function's default 0; `ComputeDVARS`' 1e-7).
    pub variance_tol: f64,
    /// `intensity_normalization` (default 1000; 0 disables the scaling).
    pub intensity_normalization: f64,
    /// numpy's percentile arithmetic.
    pub numpy: NumpyRelease,
    /// numpy's summation order (numpy 2.2: [`SumOrder::Buffered`]).
    pub sum_order: SumOrder,
    /// numpy's LAPACK (`pinv`).
    pub lapack: Lapack,
    /// SciPy's FFT library.
    pub fft: FftLibrary,
    /// SciPy's 1 × 1 solve.
    pub solve: SolveRelease,
    /// D13's switch.
    pub defects: UpstreamDefects,
    /// Threads (0: all logical CPUs); the result does not depend on it.
    pub n_threads: usize,
}

impl Default for DvarsOptions {
    fn default() -> Self {
        DvarsOptions {
            remove_zerovariance: false,
            variance_tol: 0.0,
            intensity_normalization: 1000.0,
            numpy: NumpyRelease::default(),
            sum_order: SumOrder::default(),
            lapack: Lapack::default(),
            fft: FftLibrary::default(),
            solve: SolveRelease::default(),
            defects: UpstreamDefects::Fix,
            n_threads: 1,
        }
    }
}

/// `compute_dvars`' three series (`T - 1` values each) and what the defects did.
#[derive(Clone, Debug, PartialEq)]
pub struct Dvars {
    /// `dvars_stdz`, the standardised DVARS (float64).
    pub std: Vec<f64>,
    /// `dvars_nstd`, the DVARS (float32).
    pub nstd: Vec<f32>,
    /// `dvars_vx_stdz`, the voxel-wise standardised DVARS (float64).
    pub vxstd: Vec<f64>,
    /// The voxels used (after `remove_zerovariance`).
    pub voxels: usize,
    /// The AR(1) coefficients of those voxels (nipype's `ar1`).
    pub ar1: Vec<f32>,
    /// What the defects did to this result.
    pub notices: Vec<Notice>,
}

/// How `compute_dvars` fails, with the original's exception and message.
#[derive(Debug, thiserror::Error)]
pub enum DvarsError {
    /// `IndexError` of `np.percentile` along an empty time axis.
    #[error("index -1 is out of bounds for axis 0 with size 0")]
    EmptySeries,
    /// `ValueError` of `np.apply_along_axis` when no voxel is left.
    #[error("Cannot apply_along_axis when any iteration dimensions are 0")]
    NoVoxels,
    /// The AR(1) solve's error at voxel `voxel` (`sci41`: SciPy 1.15 itself would not have
    /// raised it, SCI-41's correction did).
    #[error("{error}")]
    Solve {
        error: SolveError,
        voxel: usize,
        sci41: bool,
    },
    /// `ValueError` of numpy's broadcasting: one volume, SciPy 1.18's empty AR coefficients and
    /// `n >= 2` voxels.
    #[error("operands could not be broadcast together with shapes ({0},0) ({0},) ")]
    Broadcast(usize),
    /// The `RuntimeWarning` raised in the voxel-wise standardisation (`reproduce`).
    #[error("{0}")]
    FloatingPoint(&'static str),
    /// The FFT plans (a series of more than 32768 volumes with SciPy 1.18).
    #[error(transparent)]
    Fft(#[from] FftError),
    /// numpy's `pinv` failed (not reached: `X` is a column of ones).
    #[error("SVD did not converge")]
    Pinv,
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// numpy's floating-point status of one ufunc call (the flags it reports).
#[derive(Clone, Copy, Debug, Default)]
struct Flags {
    divide: bool,
    overflow: bool,
    invalid: bool,
}

impl Flags {
    fn or(self, o: Flags) -> Flags {
        Flags {
            divide: self.divide || o.divide,
            overflow: self.overflow || o.overflow,
            invalid: self.invalid || o.invalid,
        }
    }

    /// numpy's message for the first flag it handles (divide, overflow, then invalid).
    fn message(
        self,
        divide: &'static str,
        overflow: &'static str,
        invalid: &'static str,
    ) -> Option<&'static str> {
        if self.divide {
            Some(divide)
        } else if self.overflow {
            Some(overflow)
        } else if self.invalid {
            Some(invalid)
        } else {
            None
        }
    }
}

/// `a / b` in float64 with the IEEE flags it raises (underflow is ignored, as numpy's default).
#[inline(always)]
fn div_flags(a: f64, b: f64, f: &mut Flags) -> f64 {
    let r = NpFloat::div(a, b);
    if !a.is_nan() && !b.is_nan() {
        if b == 0.0 {
            if a == 0.0 {
                f.invalid = true;
            } else if a.is_finite() {
                f.divide = true;
            }
        } else if a.is_infinite() && b.is_infinite() {
            f.invalid = true;
        } else if r.is_infinite() && a.is_finite() {
            f.overflow = true;
        }
    }
    r
}

/// The per-voxel part of one chunk: the AR(1) coefficient and the predicted SD of each voxel,
/// or the first error with its voxel.
struct ChunkOut {
    ar1: Vec<f32>,
    sdhat: Vec<f64>,
    error: Option<(usize, SolveError, bool)>,
    zero_series: usize,
    above_one: usize,
}

/// nipype's `compute_dvars(in_file, in_mask, remove_zerovariance, intensity_normalization,
/// variance_tol)` on `mfunc`, the `n × t` float32 rows of the masked series (C order, the
/// voxels in the mask's C order), with the libraries and the switch of `o` (module
/// documentation).
///
/// # Errors
/// The original's failures ([`DvarsError`]), and in [`UpstreamDefects::Reproduce`] the
/// voxel-wise standardisation's warnings and SciPy's errors that the corrections avoid.
pub fn compute_dvars(
    mfunc: Vec<f32>,
    n: usize,
    t: usize,
    o: &DvarsOptions,
) -> Result<Dvars, DvarsError> {
    assert_eq!(mfunc.len(), n * t, "mfunc holds n × t values");
    with_threads(o.n_threads, || compute(mfunc, n, t, o))?
}

fn compute(mut mfunc: Vec<f32>, n: usize, t: usize, o: &DvarsOptions) -> Result<Dvars, DvarsError> {
    let mut notices = Notices::new(o.defects);
    // 1. Intensity normalisation.
    if o.intensity_normalization != 0.0 {
        let med = median(&mfunc);
        let factor = o.intensity_normalization as f32;
        mfunc
            .par_iter_mut()
            .for_each(|x| *x = NpFloat::mul(NpFloat::div(*x, med), factor));
    }
    // 2. The robust SD and the zero-variance voxels.
    let q75 = percentile_rank_axis(&mfunc, [n, t, 1], 75.0, RankMethod::Lower, o.numpy)
        .map_err(|_| DvarsError::EmptySeries)?;
    let q25 = percentile_rank_axis(&mfunc, [n, t, 1], 25.0, RankMethod::Lower, o.numpy)
        .map_err(|_| DvarsError::EmptySeries)?;
    let div = 1.349f32;
    let mut func_sd: Vec<f32> = q75
        .iter()
        .zip(&q25)
        .map(|(&a, &b)| NpFloat::div(NpFloat::sub(a, b), div))
        .collect();
    let mut n = n;
    if o.remove_zerovariance {
        let tol = o.variance_tol as f32;
        let keep: Vec<bool> = func_sd.iter().map(|&s| s > tol).collect();
        let kept = keep.iter().filter(|&&k| k).count();
        if kept < n {
            let mut rows = Vec::with_capacity(kept * t);
            for (i, &k) in keep.iter().enumerate() {
                if k {
                    rows.extend_from_slice(&mfunc[i * t..(i + 1) * t]);
                }
            }
            mfunc = rows;
            func_sd = func_sd
                .into_iter()
                .zip(&keep)
                .filter(|(_, k)| **k)
                .map(|(s, _)| s)
                .collect();
            n = kept;
        }
    }
    if n == 0 {
        return Err(DvarsError::NoVoxels);
    }
    let fix = o.defects == UpstreamDefects::Fix;
    // 3. The AR(1) coefficients, voxel by voxel, in parallel chunks.
    let plan = CrosscovPlan::new(o.fft, t)?;
    let pinv = if t >= 2 {
        let ones = vec![1.0f64; t];
        let x = Array2 {
            data: &ones,
            off: 0,
            rows: t,
            cols: 1,
            s0: 1,
            s1: 1,
        };
        numpy_pinv(o.lapack, x, 1e-15).map_err(|_| DvarsError::Pinv)?
    } else {
        Vec::new()
    };
    let chunks: Vec<ChunkOut> = mfunc
        .par_chunks(CHUNK * t)
        .zip(func_sd.par_chunks(CHUNK))
        .enumerate()
        .map(|(c, (rows, sds))| ar_chunk(c * CHUNK, rows, sds, t, n, &pinv, &plan, o, fix))
        .collect();
    let mut ar1 = Vec::with_capacity(n);
    let mut sdhat = Vec::with_capacity(n);
    let (mut zero_series, mut above_one) = (0usize, 0usize);
    for ch in chunks {
        if let Some((voxel, error, sci41)) = ch.error {
            if sci41 {
                notices.add(
                    &defects::SCI_41,
                    format!(
                        "voxel {voxel}'s lag-1 autocorrelation is not finite: refused, as SciPy \
                         1.18 refuses it (SciPy 1.15 divides)"
                    ),
                );
            }
            return Err(DvarsError::Solve {
                error,
                voxel,
                sci41,
            });
        }
        ar1.extend(ch.ar1);
        sdhat.extend(ch.sdhat);
        zero_series += ch.zero_series;
        above_one += ch.above_one;
    }
    if t == 1 {
        // SciPy 1.18's empty coefficients: an (n, 0) array against func_sd's (n,).
        if n >= 2 {
            return Err(DvarsError::Broadcast(n));
        }
        return Ok(Dvars {
            std: Vec::new(),
            nstd: Vec::new(),
            vxstd: Vec::new(),
            voxels: n,
            ar1,
            notices: notices.into_vec(),
        });
    }
    if zero_series > 0 {
        let what = match o.solve {
            SolveRelease::V1_15 => "nipype's NaN makes every standardised DVARS NaN",
            SolveRelease::V1_18 => "nipype stops with LinAlgError: A singular matrix detected.",
        };
        notices.add(
            &defects::PIPE_50,
            format!(
                "{zero_series} voxels have an all-zero demeaned series: their AR(1) is 0 ({what})"
            ),
        );
    }
    if above_one > 0 {
        notices.add(
            &defects::PIPE_47,
            format!(
                "{above_one} voxels have an AR(1) estimate above 1: taken as 1 (nipype's NaN \
                 makes every standardised DVARS NaN)"
            ),
        );
    }
    // 4. The mean predicted SD.
    let diff_sd_mean = mean_ordered(&sdhat, o.sum_order);
    // 5. DVARS and the standardised DVARS.
    let tm = t - 1;
    let cols = column_blocks(tm);
    let sq_sum: Vec<f32> = if tm == 1 {
        let sq: Vec<f32> = (0..n)
            .map(|j| {
                let d = NpFloat::sub(mfunc[j * t + 1], mfunc[j * t]);
                NpFloat::mul(d, d)
            })
            .collect();
        vec![larmorx_core::numpy::reduce::add_reduce_ordered(
            &sq,
            o.sum_order,
        )]
    } else {
        cols.par_iter()
            .flat_map_iter(|&(c0, c1)| {
                let mut acc = vec![0.0f32; c1 - c0];
                for j in 0..n {
                    let row = &mfunc[j * t..(j + 1) * t];
                    for (k, a) in acc.iter_mut().enumerate() {
                        let d = NpFloat::sub(row[c0 + k + 1], row[c0 + k]);
                        *a = NpFloat::add(*a, NpFloat::mul(d, d));
                    }
                }
                acc
            })
            .collect()
    };
    let count = n as f64;
    let nstd: Vec<f32> = sq_sum
        .iter()
        .map(|&s| NpFloat::sqrt(f32::from_f64(NpFloat::div(s.to_f64(), count))))
        .collect();
    let std: Vec<f64> = nstd
        .iter()
        .map(|&v| NpFloat::div(v.to_f64(), diff_sd_mean))
        .collect();
    // 6. The voxel-wise standardisation.
    let include: Vec<bool> = if fix {
        sdhat.iter().map(|&s| s != 0.0).collect()
    } else {
        vec![true; n]
    };
    let used = include.iter().filter(|&&k| k).count();
    let blocks: Vec<(Vec<f64>, Flags, Flags, Flags)> = if tm == 1 {
        let (mut fd, mut fs) = (Flags::default(), Flags::default());
        let sq: Vec<f64> = (0..n)
            .filter(|&j| include[j])
            .map(|j| {
                let d = NpFloat::sub(mfunc[j * t + 1], mfunc[j * t]).to_f64();
                let q = div_flags(d, sdhat[j], &mut fd);
                square_flags(q, &mut fs)
            })
            .collect();
        let s = larmorx_core::numpy::reduce::add_reduce_ordered(&sq, o.sum_order);
        let fr = Flags {
            overflow: s.is_infinite() && sq.iter().all(|v| v.is_finite()),
            ..Flags::default()
        };
        vec![(vec![s], fd, fs, fr)]
    } else {
        cols.par_iter()
            .map(|&(c0, c1)| {
                let (mut fd, mut fs, mut fr) =
                    (Flags::default(), Flags::default(), Flags::default());
                let mut acc = vec![0.0f64; c1 - c0];
                for j in (0..n).filter(|&j| include[j]) {
                    let row = &mfunc[j * t..(j + 1) * t];
                    let s = sdhat[j];
                    for (k, a) in acc.iter_mut().enumerate() {
                        let d = NpFloat::sub(row[c0 + k + 1], row[c0 + k]).to_f64();
                        let q = div_flags(d, s, &mut fd);
                        let v = square_flags(q, &mut fs);
                        let r = NpFloat::add(*a, v);
                        if r.is_infinite() && a.is_finite() && v.is_finite() {
                            fr.overflow = true;
                        }
                        *a = r;
                    }
                }
                (acc, fd, fs, fr)
            })
            .collect()
    };
    let (mut fd, mut fs, mut fr) = (Flags::default(), Flags::default(), Flags::default());
    let mut vx_sum = Vec::with_capacity(tm);
    for (acc, d, s, r) in blocks {
        vx_sum.extend(acc);
        fd = fd.or(d);
        fs = fs.or(s);
        fr = fr.or(r);
    }
    let raised = fd
        .message(
            "divide by zero encountered in divide",
            "overflow encountered in divide",
            "invalid value encountered in divide",
        )
        .or_else(|| fs.message("", "overflow encountered in square", ""))
        .or_else(|| fr.message("", "overflow encountered in reduce", ""));
    let left = n - used;
    if !fix {
        if let Some(msg) = raised {
            return Err(DvarsError::FloatingPoint(msg));
        }
    } else if left > 0 {
        notices.add(
            &defects::PIPE_46,
            format!(
                "the voxel-wise standardised DVARS leaves out {left} voxels whose predicted SD is \
                 0 (nipype stops with a RuntimeWarning of the division)"
            ),
        );
    } else if let Some(msg) = raised {
        notices.add(
            &defects::PIPE_46,
            format!(
                "the voxel-wise standardisation's floating-point warning does not stop the \
                 computation (nipype stops with RuntimeWarning: {msg})"
            ),
        );
    }
    let ucount = used as f64;
    let vxstd: Vec<f64> = vx_sum
        .iter()
        .map(|&s| NpFloat::sqrt(NpFloat::div(s, ucount)))
        .collect();
    Ok(Dvars {
        std,
        nstd,
        vxstd,
        voxels: n,
        ar1,
        notices: notices.into_vec(),
    })
}

/// `q * q` with the overflow flag.
#[inline(always)]
fn square_flags(q: f64, f: &mut Flags) -> f64 {
    let v = NpFloat::mul(q, q);
    if v.is_infinite() && q.is_finite() {
        f.overflow = true;
    }
    v
}

/// The time columns split into blocks for the threads (each column summed over the voxels in
/// order by one thread).
fn column_blocks(tm: usize) -> Vec<(usize, usize)> {
    let step = 16usize;
    (0..tm.div_ceil(step))
        .map(|b| (b * step, ((b + 1) * step).min(tm)))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn ar_chunk(
    first: usize,
    rows: &[f32],
    sds: &[f32],
    t: usize,
    n_total: usize,
    pinv: &[f64],
    plan: &CrosscovPlan,
    o: &DvarsOptions,
    fix: bool,
) -> ChunkOut {
    let c = sds.len();
    let mut out = ChunkOut {
        ar1: Vec::with_capacity(c),
        sdhat: Vec::with_capacity(c),
        error: None,
        zero_series: 0,
        above_one: 0,
    };
    // regress_poly(0, ...): the coefficients of this chunk's voxels.
    let betas: Vec<f64> = if t == 1 {
        Vec::new()
    } else {
        let a: Vec<f64> = rows.iter().map(|&v| v.to_f64()).collect();
        if n_total == 1 {
            vec![0.0 + ddot(t, pinv, 0, 1, &a, 0, 1)]
        } else {
            let mut y = vec![0.0f64; c];
            dgemv(Trans::T, t, c, 1.0, &a, 0, t, pinv, 0, 1, 0.0, &mut y, 0, 1);
            y
        }
    };
    let mut res = vec![0.0f32; t];
    for j in 0..c {
        let row = &rows[j * t..(j + 1) * t];
        if t == 1 {
            res[0] = NpFloat::sub(row[0], row[0]);
        } else {
            let hat = NpFloat::add(betas[j], 0.0);
            for (r, &x) in res.iter_mut().zip(row) {
                *r = f32::from_f64(NpFloat::sub(x.to_f64(), hat));
            }
        }
        let (r0, r1) = plan.autocorr_01(&res);
        let ar = match r1 {
            None => match o.solve {
                SolveRelease::V1_15 => Err((SolveError::RowsMismatch, false)),
                SolveRelease::V1_18 if !r0.is_finite() => Err((SolveError::NotFinite, false)),
                SolveRelease::V1_18 => Ok(None),
            },
            Some(r1) if fix => {
                if r0 == 0.0 {
                    out.zero_series += 1;
                    Ok(Some(0.0))
                } else {
                    match scipy_solve_1x1(SolveRelease::V1_18, r0, r1) {
                        Ok(a) => Ok(Some(a)),
                        Err(e) => {
                            let original = scipy_solve_1x1(o.solve, r0, r1).is_ok();
                            Err((e, original))
                        }
                    }
                }
            }
            Some(r1) => scipy_solve_1x1(o.solve, r0, r1)
                .map(Some)
                .map_err(|e| (e, false)),
        };
        match ar {
            Err((e, sci41)) => {
                out.error = Some((first + j, e, sci41));
                return out;
            }
            Ok(None) => {}
            Ok(Some(mut a)) => {
                if fix && a > 1.0 {
                    out.above_one += 1;
                    a = 1.0;
                }
                out.ar1.push(a);
                let twice = NpFloat::mul(NpFloat::sub(1.0f32, a), 2.0f32);
                let s = NpFloat::sqrt(twice.to_f64());
                out.sdhat.push(NpFloat::mul(s, sds[j].to_f64()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(n: usize, t: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        let mut next = move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 40) as f64 / (1u64 << 24) as f64) - 0.5
        };
        let mut v = Vec::with_capacity(n * t);
        for j in 0..n {
            let mut walk = 1000.0 + j as f64;
            for _ in 0..t {
                walk += next() * 4.0;
                v.push((walk + next() * 10.0) as f32);
            }
        }
        v
    }

    #[test]
    fn threads_do_not_change_the_result() {
        let (n, t) = (1031, 37);
        let x = series(n, t, 7);
        let mut o = DvarsOptions {
            remove_zerovariance: true,
            variance_tol: 1e-7,
            ..DvarsOptions::default()
        };
        let one = compute_dvars(x.clone(), n, t, &o).unwrap();
        o.n_threads = 5;
        let five = compute_dvars(x, n, t, &o).unwrap();
        assert_eq!(one, five);
        assert_eq!(one.std.len(), t - 1);
        assert!(one.std.iter().all(|v| v.is_finite() && *v > 0.0));
        assert!(one.notices.is_empty());
    }

    #[test]
    fn the_defects_of_a_constant_voxel() {
        let (n, t) = (9, 12);
        let mut x = series(n, t, 3);
        for v in &mut x[..t] {
            *v = 0.0;
        }
        let mut o = DvarsOptions {
            solve: SolveRelease::V1_18,
            ..DvarsOptions::default()
        };
        let fixed = compute_dvars(x.clone(), n, t, &o).unwrap();
        assert!(fixed.std.iter().all(|v| v.is_finite()));
        let ids: Vec<&str> = fixed.notices.iter().map(|n| n.defect.id).collect();
        assert_eq!(ids, ["PIPE-50", "PIPE-46"]);
        o.defects = UpstreamDefects::Reproduce;
        assert!(matches!(
            compute_dvars(x.clone(), n, t, &o),
            Err(DvarsError::Solve {
                error: SolveError::Singular,
                voxel: 0,
                ..
            })
        ));
        o.solve = SolveRelease::V1_15;
        let r = compute_dvars(x, n, t, &o).unwrap();
        assert!(r.std.iter().all(|v| v.is_nan()));
        assert!(r.nstd.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn failures() {
        let o = DvarsOptions::default();
        assert!(matches!(
            compute_dvars(Vec::new(), 3, 0, &o),
            Err(DvarsError::EmptySeries)
        ));
        assert!(matches!(
            compute_dvars(Vec::new(), 0, 5, &o),
            Err(DvarsError::NoVoxels)
        ));
        let x = series(4, 2, 1);
        // Two volumes: every robust SD is 0, so the voxel-wise division by zero.
        let r = DvarsOptions {
            defects: UpstreamDefects::Reproduce,
            ..o
        };
        assert!(matches!(
            compute_dvars(x.clone(), 4, 2, &r),
            Err(DvarsError::FloatingPoint(
                "divide by zero encountered in divide"
            ))
        ));
        let f = compute_dvars(x, 4, 2, &o).unwrap();
        assert!(f.vxstd[0].is_nan());
        let one = series(3, 1, 2);
        assert!(matches!(
            compute_dvars(one.clone(), 3, 1, &o),
            Err(DvarsError::Broadcast(3))
        ));
        let o15 = DvarsOptions {
            solve: SolveRelease::V1_15,
            ..o
        };
        assert!(matches!(
            compute_dvars(one, 3, 1, &o15),
            Err(DvarsError::Solve {
                error: SolveError::RowsMismatch,
                ..
            })
        ));
    }

    #[test]
    fn an_ar1_estimate_above_one() {
        // One full period of a sine over 40,000 volumes, amplitude 3: the float32 FFT's rounding
        // gives it an AR(1) estimate of 1.0000001 (found once by a sweep of 60 sines of 30,000
        // and 40,000 volumes, amplitudes 1 to 1000 and phases 0 to 0.1, which also finds
        // (40,000, 10, 0.01); the sweep is validation/investigations/confounds/
        // probe_ar1_above_one.py's, which measures nitime's).
        let o = DvarsOptions {
            fft: FftLibrary::Pocketfft,
            solve: SolveRelease::V1_15,
            defects: UpstreamDefects::Reproduce,
            n_threads: 2,
            ..DvarsOptions::default()
        };
        let (t, amp, phase) = (40_000usize, 3.0f64, 0.0f64);
        // the sine first, then three BOLD-like voxels (a mean predicted SD above 0)
        let mut x: Vec<f32> = (0..t)
            .map(|i| {
                let a = 2.0 * std::f64::consts::PI * i as f64 / t as f64 + phase;
                (1000.0 + amp * larmorx_core::math::sin(a)) as f32
            })
            .collect();
        x.extend(series(3, t, 11));
        // above 1, nipype's predicted SD is NaN (an estimate of exactly 1 stops it: PIPE-46)
        let r = compute_dvars(x.clone(), 4, t, &o).unwrap();
        assert!(r.ar1[0] > 1.0, "{}", r.ar1[0]);
        assert!(r.std.iter().all(|v| v.is_nan()), "nipype's NaN");
        let f = compute_dvars(
            x,
            4,
            t,
            &DvarsOptions {
                defects: UpstreamDefects::Fix,
                ..o
            },
        )
        .unwrap();
        assert_eq!(f.ar1[0], 1.0);
        let ids: Vec<&str> = f.notices.iter().map(|n| n.defect.id).collect();
        assert_eq!(ids, ["PIPE-47", "PIPE-46"]);
        assert!(f.std.iter().all(|v| v.is_finite()));
        assert!(f.vxstd.iter().all(|v| v.is_finite()));
    }
}
