// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-surf: `fs.surfio` (wave 3.1), `fs.surf` (wave 3.4), `fs.mars` (later) of
//! FreeSurfer 8.2.0's replica (docs/freesurfer/waves.md section 4): the surface structure and its
//! I/O, with the metric properties every read computes (`fs.surfio`); the rest of the surface
//! library: topology, values, volume-surface sampling, projections, the surface hash, realms,
//! icosahedra, spherical maps, surface clusters, gcsa and the surface-gradient code (`fs.surf`).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Empty until its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
#![forbid(unsafe_code)]
