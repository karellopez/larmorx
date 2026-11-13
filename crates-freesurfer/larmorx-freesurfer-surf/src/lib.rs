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
//! - [`lta`]: `MRIS::lta`'s type, fs.mri's `LTA` (with `LTAcopy` and `LTAfree`).
//! - [`alloc`]: a surface's life: allocation, reallocation (FS-117), the metadata copy,
//!   destruction, the dumps.
//! - [`basemisc`]: the rip flags, the arrays, the curvature names, the registration's scales, the
//!   extra gradients, the NaN check, the column printing (FS-119), the vector searches, the
//!   progress bar (FS-118), the sort comparisons.
//! - [`edges`]: whether two edges cross on the sphere.
//! - [`mp`]: `MRIScomputeMetricProperties` and its default computation on the `MRIS_MP` copy
//!   (FS-120..122), the reproducible sums.
//! - [`mp_slow`]: the computation on the surface itself (`MRIScomputeNormals`,
//!   `MRIScomputeTriangleProperties`, `MRIScomputeAvgInterVertexDist`, the orientations).
//! - [`mp_faster`]: the faster computation `FS_FASTER_MP` selects (FS-123, FS-124).
//! - [`sfile`]: `mrisurf_io.cpp`'s small file readers and writers (the read frame, the
//!   triangle-property files, the distance, area-error, angle-error and curvature w files, the
//!   backup and imaginary value readers, the marks, the ASCII and VTK formats; FS-126..FS-134).
//! - [`curvio`]: the per-vertex value files (the curvature as text and binary, the float and area
//!   files, the file names, the decimation files, `FS_GII`; FS-136..FS-140, FS-148, FS-149).
//! - [`formats`]: the VTK reader, the movie.byu and icosahedron writers, the Talairach matrix, the
//!   MATLAB plots (FS-141..FS-147).
//! - [`scanf`]: one C `scanf` call (fs.core's `cscan::Scanf`, re-exported).
//! - [`stl`]: STL surfaces, read (binary or text) and written (FS-151, FS-152, FS-157..FS-160).
//! - [`surfread`]: the file-local surface readers (ASCII, movie.byu and its positions, the
//!   triangle file's positions) and the old writer's quadrangles (FS-147, FS-150..FS-156).
//! - [`trifile`]: the binary triangle and quadrangle surface files, read and written, with
//!   their tags (FS-346..FS-353).
#![forbid(unsafe_code)]

pub mod aaa;
pub mod alloc;
pub mod base;
pub mod basemisc;
pub mod coords;
pub mod curvio;
pub mod dist;
pub mod edges;
pub mod facenorm;
pub mod formats;
pub mod lta;
pub mod metric;
pub mod mp;
pub mod mp_faster;
pub mod mp_slow;
pub mod mris;
pub mod mris_mp;
pub mod mrisurf;
pub mod positions;
pub mod scanf;
pub mod sfile;
pub mod stl;
pub mod surfbase;
pub mod surfread;
pub mod topology;
pub mod trifile;
pub mod views;
