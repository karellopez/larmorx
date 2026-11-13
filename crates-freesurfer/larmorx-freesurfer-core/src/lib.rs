// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-core: `fs.core` (queue item 3.1-22) and `fs.maths` (3.1-23, 3.1-24) of
//! FreeSurfer 8.2.0's replica (docs/freesurfer/waves.md section 4): FreeSurfer's messages,
//! diagnostics, program name, environment, version stamps, the ROMP switch, timers, paths, signals
//! and the licence check's messages (`fs.core`); its matrices (float and long double), the numerics
//! wrappers over VNL and Cephes, `fs_powell`, `fs_lbfgs`, histograms, statistics, PDFs and random
//! fields (`fs.maths`).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
//!
//! - [`run`]: what the program `larmorx-freesurfer` gives a program's replica (its settings, an
//!   environment table, the entry point every program crate registers); larmorx's own plumbing.
//! - `fs.core` (queue item 3.1-22; the behaviour: `specs/freesurfer-core.md`): [`globals`]
//!   (FreeSurfer's process-wide state, one value per run), [`log`] (messages, fatal errors, the
//!   error codes; FS-01), [`diag`] (the diagnostic bits and `DiagInit`; FS-02, FS-03),
//!   [`machine`] (byte swaps), [`cmdargs`], [`base`]; [`random`] (the seeded random numbers;
//!   FS-04, FS-05), [`utils`] (lines, strings, file names, rounding, lists, order statistics, the
//!   fast arctangent, memory reports, logic-problem reports, temporary files; FS-06, FS-09 to
//!   FS-12), [`version`] (`--version`, `--all-info`, the stamps; FS-07), [`fsenv`] (FS-08),
//!   [`timer`], [`romp`], [`chklc`] (no licence check), [`xdebug`], [`fio`]'s table.
//! - The C library's behaviour they need, larmorx's own code: [`cstd`] (conversions), [`cglob`]
//!   (`glob`, `fnmatch`), [`ctime`] (`gmtime`, `localtime` with the time-zone files,
//!   `strftime("%c")`), [`system`] (the user's and the machine's names).
//! - `fs.spline` (queue item 3.1-54): [`spline`], John Burkardt's 2006 cubic spline as FreeSurfer
//!   changed it (`SplineCubicSet`, `SplineCubicValue`, `d3_np_fs`, `OpenSpline`, `OpenSplint`; FS-103).
#![forbid(unsafe_code)]

pub mod affine;
pub mod argparse;
pub mod base;
pub mod cglob;
pub mod chklc;
pub mod cmdargs;
pub mod cname;
pub mod cscan;
pub mod cstd;
pub mod ctime;
pub mod diag;
pub mod dmatrix;
pub mod fio;
pub mod fnv;
pub mod fs_vnl;
pub mod fsenv;
pub mod globals;
pub mod help_xml;
pub mod histo;
pub mod log;
pub mod machine;
pub mod matrix;
pub mod matrix_decomp;
pub mod matrix_io;
pub mod matrix_ops;
pub mod numerics;
pub mod pdf;
pub mod prime;
pub mod random;
pub mod randomfields;
pub mod ret;
pub mod romp;
pub mod run;
pub mod spline;
pub mod sse;
pub mod system;
pub mod timer;
pub mod utils;
pub mod version;
pub mod xdebug;
