// SPDX-License-Identifier: Apache-2.0
//! fMRIPrep's confound estimation (the confounds' part), ported from nipype 1.12.0
//! (`nipype/algorithms/confounds.py`), niworkflows 1.15.4 and fMRIPrep 25.2.7 (Apache-2.0, the
//! NiPreps and nipype developers), with the arithmetic of the libraries they call (nitime,
//! SciPy, numpy; BSD-3-Clause, notices in the modules that follow them and in
//! `PROVENANCE.md`).
//!
//! The results are those of the Python originals bit for bit (CLAUDE.md rule 8: the source is
//! the specification), and the same on every platform: the operation order of each upstream
//! routine is kept, with x86-64's SSE arithmetic (NaNs included).
//!
//! - [`scipy_fft`]: SciPy's `fftpack.fft` / `ifft` of single-precision arrays of power-of-two
//!   length, as SciPy 1.15 (pocketfft) and SciPy 1.18.1 (ducc0) compute them (nitime's
//!   `fftconvolve` under nipype's DVARS).
//! - [`nitime`]: nitime's `crosscov`/`autocorr` (`fftconvolve` with numpy's complex64 product)
//!   and `AR_est_YW(x, 1)` with each SciPy release's 1 × 1 `linalg.solve`.
//! - [`dvars`]: nipype's `compute_dvars` on the masked series (the three DVARS series, the
//!   corrections PIPE-46, PIPE-47, PIPE-50 and SCI-41).
//! - [`compcor`]: nipype's CompCor numerics (`_cosine_drift`, `regress_poly`, `cosine_filter`,
//!   `_compute_tSTD`, `TCompCor`'s high-variance mask, `compute_noise_components`; the corrections
//!   PIPE-44, PIPE-45, PIPE-52, PIPE-54, PIPE-55, PIPE-57 and PIPE-59, GLIBC-07's cosine).
//! - [`nipy`]: nipype's `normalize_mc_params(params, "NIPY")`, nipy 0.6.1's `to_matrix44` and
//!   `aff2euler` with nibabel's `mat2euler` (BSD-3-Clause and MIT: the notices in the module),
//!   for `FramewiseDisplacement(parameter_source="NIPY")`.
//! - [`expansion`]: numpy's float64 `a ** o` for the integer exponents of niworkflows'
//!   `ExpandModel` (`exponential_terms`): `x * x` for 2, glibc's `pow` (`reproduce`) or the
//!   correctly rounded one (the default, GLIBC-07) for the others.
//! - [`motion`]: fMRIPrep's `FSLMotionParams` (25.2.7 and 25.2.6) and `FSLRMSDeviation` on the
//!   head-motion transforms: nitransforms' `to_ras` and `from_ras`, SciPy's `Rotation.from_matrix`
//!   and `as_euler` (BSD-3-Clause and MIT: the notices in the module).
//! - [`pandas`]: pandas' default float parser of `read_csv` (`precise_xstrtod`, pandas 3.0.6
//!   and 2.2.3; BSD-3-Clause: the notice in the module) with the correctly rounded reading of
//!   the same texts (PD-01), for the tables fMRIPrep's confound steps read.
//! - [`signals`]: niworkflows' `SignalExtraction` means (`data[mask, :].mean(axis=0)` in numpy's
//!   summation order), read where the voxels are, without numpy's masked copy.
//!
//! This crate is the **port** (a replica of permissive upstreams, in the main package). The
//! clean-room original of the same steps, planned later (the user, 2026-11-06 and 2026-11-11),
//! lives in another crate.
#![forbid(unsafe_code)]

pub mod compcor;
pub mod dvars;
pub mod expansion;
pub mod motion;
pub mod nipy;
pub mod nitime;
pub mod pandas;
pub mod scipy_fft;
pub mod signals;
