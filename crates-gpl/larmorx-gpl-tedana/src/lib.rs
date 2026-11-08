// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! tedana's `t2smap` translated exactly to Rust: the T2* and S0 maps of multi-echo fMRI and
//! their optimal combination, with tedana 26.0.3's command line, messages, outputs and exit
//! statuses, bit for bit with tedana's Python and the numpy 2.5.4 and SciPy 1.18.1 it runs on
//! (numpy 2.3.5 and SciPy 1.15.3, the shared environment's, on request).
//!
//! - [`parser`], `t2smap`'s argument parser (argparse, through `larmorx_core::python`).
//! - [`log`], tedana's `GENERAL` logger on standard error, Python's warnings, exceptions.
//! - [`data`], the reference image, the mask and the masked multi-echo data
//!   (`tedana.io.load_ref_img`, `tedana.utils.load_mask`, `tedana.io.load_data_nilearn` with
//!   nibabel's and nilearn's reading).
//! - [`adaptive`], `tedana.utils.make_adaptive_mask`.
//! - [`decay`], `tedana.decay`'s fits: `fit_loglinear` (numpy's `lstsq`), `fit_monoexponential`
//!   (SciPy's `curve_fit`), `fit_decay`, `fit_decay_ts`.
//! - [`maps`], `tedana.decay.modify_t2s_s0_maps` and `rmse_of_fit_decay_ts`, `utils.unmask`.
//! - [`combine`], `tedana.combine.make_optcom`.
//! - [`outputs`], `tedana.io.OutputGenerator`: the files' names, images, table and JSON.
//! - [`workflow`], `tedana.workflows.t2smap` (`t2smap_workflow`, `_main`).
//!
//! The numerics tedana calls are numpy's and SciPy's, translated in the main package's
//! Apache-2.0 crates (`larmorx_core::numpy`, `larmorx_transform::numpy_linalg`,
//! `larmorx_optim`), which this crate uses: only tedana's own logic is here.
//!
//! **Licence.** tedana is distributed under the GNU Lesser General Public License version 2.1
//! (its `LICENSE`; no file carries another notice, and no later version is named). Section 3 of
//! that licence lets a copy be distributed under the ordinary GNU General Public License
//! instead, naming a version newer than 2; this translation does so, under the GNU General
//! Public License version 3 only (the text: `crates-gpl/LICENSE`), so that it can include the
//! Apache-2.0 crates it is built with. The change cannot be undone for this copy (section 3).
//!
//! **Modifications** (the GPL's section 5a): translated from Python to Rust in 2026 by Karel
//! Lopez Vilaret; the defects of tedana's that change results are corrected by default and
//! reproduced on request (`LARMORX_UPSTREAM_DEFECTS=reproduce`; the register's TED entries); the
//! maths default to correctly rounded functions (numpy's own and the C library's with
//! `reproduce`). `PROVENANCE.md` lists the files translated.
#![forbid(unsafe_code)]

pub mod adaptive;
pub mod combine;
pub mod data;
pub mod decay;
pub mod log;
pub mod maps;
pub mod outputs;
pub mod parser;
pub mod workflow;

pub use workflow::t2smap;

/// tedana's version, which this translation follows (`t2smap -v` prints it).
pub const TEDANA_VERSION: &str = "26.0.3";
