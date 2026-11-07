// SPDX-License-Identifier: Apache-2.0
//! Clean-room originals of FSL's tools for larmorx (CLAUDE.md rule 1, `docs/licensing.md`).
//!
//! - [`bet`]: brain extraction compatible with FSL's `bet2` and the `bet` script. Written from
//!   `specs/bet.md`, S. M. Smith's paper (Human Brain Mapping 17(3), 2002), FSL's
//!   documentation and black-box runs of FSL 6.0.7.23's programs, without access to FSL's
//!   source code or its bit-exact replica (see `PROVENANCE.md`).
//! - [`betsurf`]: the skull and scalp surfaces of FSL's `betsurf` (`specs/bet.md` §6), written the
//!   same way.
//! - [`flirt`]: linear registration compatible with FSL's `flirt` and its matrix tool
//!   `convert_xfm`, written from `specs/flirt.md` the same way, on the main package's shared
//!   clean-room registration engine (`larmorx_mri::hmc`).
//! - [`fast`]: FAST's tissue segmentation (`fast`), written from `specs/fast.md` the same way.
//! - [`topup`]: topup's estimation of the susceptibility-induced field (`topup`), written from
//!   `specs/topup.md` the same way.
//! - [`maths`]: FSL's image calculator `fslmaths`, written from `specs/fslmaths.md` the same
//!   way.
//! - [`prelude`]: PRELUDE's phase unwrapping (`prelude`), written from `specs/prelude.md` and
//!   Jenkinson's paper (Magnetic Resonance in Medicine 49, 2003) the same way.
//! - [`data`]: FSL's standard images, found where the user has them or fetched once (FSL's
//!   data, read at run time, never shipped: `specs/bet.md` §5.4).
//! - [`cli`]: the command lines (`larmorx fsl bet2`, `bet`, `betsurf`, `flirt`, `convert_xfm`,
//!   FLIRT's other programs, `fast`, `topup`, `fslmaths`, `fslstats`, `fslroi`,
//!   `standard_space_roi`, `prelude`; FSL's arguments).
#![forbid(unsafe_code)]
// Fixed 3-vectors and 3 x 3 matrices read best indexed; comparisons written so that NaN takes
// the branch FSL's own comparisons give it.
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]

pub mod bet;
pub mod betsurf;
pub mod cli;
pub mod data;
pub mod fast;
pub mod flirt;
pub mod maths;
pub mod prelude;
pub mod topup;
