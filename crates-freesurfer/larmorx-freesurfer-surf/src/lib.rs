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
//! separate process (`docs/licensing.md`). Its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates; the behaviour
//! is `specs/freesurfer-surfio.md`.
//!
//! - [`aaa`]: the primitive types and constants (`mrisurf_aaa.h`), `MRIS_Status` and its texts,
//!   the surface library's process-wide variables ([`aaa::SurfGlobals`]).
//! - [`mris`]: `FACE`, `VERTEX_TOPOLOGY`, `VERTEX`, `MRIS`.
//! - [`mris_mp`]: `MRIS_MP`, the metric-properties copy.
//! - [`views`]: the generated views' accessors ([`views::MetricSurface`] over `MRIS` and
//!   `MRIS_MP`), `MRISvertexCoord2XYZ_float`.
//! - [`topology`]: the neighbour counts (`modVnum` and its wrappers, `VERTEXvnum`,
//!   `mrisVertexNeighborIndex`, `vertexNeighbor`).
//! - [`base`]: `mrisurf_base.h`'s helpers and `Vec3`.
//! - [`surfbase`]: `mrisurf_base.cpp`'s borrowed temporaries, per-vertex floats and the
//!   integrator's file names (FS-111).
//! - [`mrisurf`]: `mrisurf.cpp`'s command lines (FS-112), the vertex hash, `MRISdiffSimple`.
//! - [`dist`]: the neighbour distances' storage (`mrisurf_base.cpp`).
//! - [`coords`]: the coordinate operations (`mrisurf_metricProperties.cpp`'s first part: setting,
//!   importing, copying, reversing, scaling (FS-113), moving, rotating, the bounding box).
//! - [`metric`]: averaging and smoothing the positions, the sphere's centre, the neighbour
//!   distances, the areas, the curvature operations, the saved coordinates; the original area,
//!   storing and restoring the metric properties.
//! - [`positions`]: the saved vertex positions and normals (FS-115), the canonical coordinates.
//! - [`facenorm`]: the face-normal cache (FS-116).
#![forbid(unsafe_code)]

pub mod aaa;
pub mod base;
pub mod coords;
pub mod dist;
pub mod facenorm;
pub mod metric;
pub mod mris;
pub mod mris_mp;
pub mod mrisurf;
pub mod positions;
pub mod surfbase;
pub mod topology;
pub mod views;
