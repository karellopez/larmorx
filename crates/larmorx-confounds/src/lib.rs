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
//!
//! This crate is the **port** (a replica of permissive upstreams, in the main package). The
//! clean-room original of the same steps, planned later (the user, 2026-11-06 and 2026-11-11),
//! lives in another crate.
#![forbid(unsafe_code)]

pub mod scipy_fft;
