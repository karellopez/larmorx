// SPDX-License-Identifier: GPL-3.0-or-later AND LGPL-2.1-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The translations of glibc 2.35's `sinf`, `cosf`, `tanhf`, `expm1f` and `cbrtf` that the AFNI
//! replicas used while they were part of `larmorx-gpl` (until 2026-10-29), kept unchanged as the
//! reference the verification tools compare with: each was checked against glibc on all 2^32
//! inputs ([`sincosf`]: input by input, the FMA build; [`floatmath`]: `tanhf` input by input and
//! both functions by a hash of all results against `floatmath-verify/glibc_reference.c`).
//!
//! The replicas now live in the main package (`crates/larmorx-afni-replica`, Apache-2.0), with
//! `sinf` and `cosf` translated from Arm's Optimized Routines (MIT) and proved equal to
//! [`sincosf`] (`sincosf-verify`), `tanhf` and `expm1f` from fdlibm (Sun's notice), and
//! CORE-MATH's `cbrtf` (MIT) where 3dUnifize `-EPI` calls `cbrtf` (`cbrtf-measure` measures what
//! glibc's last bit changes there).
#![forbid(unsafe_code)]

pub mod floatmath;
pub mod sincosf;
