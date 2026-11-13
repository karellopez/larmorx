// SPDX-License-Identifier: GPL-3.0-or-later
// Translated from GSL 1.8 (the GNU Scientific Library; Copyright (C) 1996, 1997, 1998, 1999, 2000
// James Theiler, Brian Gough; GNU GPL version 2 or later) as FreeSurfer 8.2.0's utils/numerics.cpp
// transcribes it; translated to Rust in 2026 by Karel Lopez Vilaret, under the GNU GPL version 3
// or later.
//! GSL 1.8's RANLUX389 generator and random distributions as FreeSurfer 8.2.0 draws from them: a
//! **replica**, translated statement by statement from FreeSurfer's transcription of GSL
//! (`utils/numerics.cpp`, the "GSL library replacement for Freesurfer" of 2006: `sc_rng_*`,
//! `sc_uni*`, `sc_ran_*`, `sc_gamma_*`, `sc_ugauss`), bit for bit with FreeSurfer's official build
//! (GCC 8.4.0, x86-64): its operation order, its integer conversions, its NaNs (on every platform,
//! [`sse`]), glibc 2.35's `log` and `exp`.
//!
//! - [`rng`]: the generator ([`ScRng`]: `sc_rng_alloc`, `sc_rng_set`, `sc_rng_get`), its state
//!   ([`ScStatus`], `sc_inc_status`) and the uniform deviates (`sc_uni`, `sc_uni_pos`).
//! - [`randist`]: the distributions, as [`ScRng`]'s methods: `ran_flat`, `ran_gaussian`, `ugauss`,
//!   `ran_exponential`, `ran_gamma` (with `gamma_int`, `gamma_large`, `gamma_frac`), `ran_chisq`,
//!   `ran_fdist`, `ran_tdist`.
//!
//! **Where it stands.** GPL code never joins a FreeSurfer program (FreeSurfer's licence and the GPL
//! cannot share one: `docs/freesurfer/third-party.md` §9, P-08; the user's decision of 2026-11-12,
//! "No waiting for permissions"), so `larmorx-freesurfer`'s replicas (`mri_coreg`, `mris_expand`)
//! draw from larmorx's clean room, `larmorx_core::ranlux` (from `specs/ranlux.md` alone). This crate
//! is its bit-exact reference, alone in `larmorx-gpl`, as `crates-gpl/tools/` keeps glibc's
//! translations: no program links it.
//!
//! **The one exception to FreeSurfer's bits** is `tan` (`sc_gamma_large` only): larmorx keeps it
//! correctly rounded everywhere (the user's decision of 2026-11-04, GLIBC-07), so a gamma draw of
//! shape 12 or more may differ from FreeSurfer's in its last bits where its tangent's argument is
//! one of the 41,708 of 2²⁴ where glibc's `tan` is not correctly rounded (`specs/ranlux.md` §6).
//!
//! **Defects** (D13; `larmorx_core::defects`): FS-100 (gamma shapes outside [0, 2³²)) and FS-101
//! (the seeds whose state is all zero), corrected by default with a notice as the clean room does,
//! FreeSurfer's results under `reproduce`; draws that cannot give a number are
//! [`DrawError`]s, with the clean room's texts.
#![forbid(unsafe_code)]

pub mod randist;
pub mod rng;
pub mod sse;

pub use rng::{DrawError, ScRng, ScStatus, sc_inc_status};

#[cfg(test)]
mod tests;
