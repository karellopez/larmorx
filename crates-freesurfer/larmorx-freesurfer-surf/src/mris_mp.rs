// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIS_MP`, the metric-properties copy of a surface (`fs.surfio`, queue item 3.1-45),
//! translated from FreeSurfer 8.2.0's `include/mrisurf_MRIS_MPPropertiesInVectors.h` and the field
//! lists of `include/mrisurf_MRIS_MP.h`: one array per vertex or face property, filled from an
//! `MRIS` by `MRISMP_load` (queue item 3.1-51) and read through the views ([`crate::views`]).
//!
//! C's pointers become: an array, `Option<Vec<T>>` (`None` is `NULL`; several arrays the struct
//! declares are never filled by FreeSurfer, so stay `NULL`: `f_v`, `v_origx..z`, `v_dx..z`, the
//! face-normal caches); an input array (`MRIS_MP__LIST_V_IN`, `MRIS_MP__LIST_F_IN`), shared by
//! the copies `MRISMP_copy` makes, `Option<Arc<Vec<T>>>` ([`In`]: C shares the pointer and counts
//! the copies in `in_ref_count`; `Arc` counts them). The pointers into the `MRIS` it was loaded
//! from (`underlyingMRIS`, `vertices_topology`, `vertices`, `faces`, and `v_dist_orig`, which
//! "keep pointing to the original ones in the MRIS") are not stored: a view pairs the `MRIS_MP`
//! with its `MRIS` and reads them there.

use std::sync::Arc;

use crate::aaa::{AnglesPerTriangle, FloatXyz, MrisStatus, VerticesPerFace};

/// An input array of an `MRIS_MP`, shared by its copies.
pub type In<T> = Option<Arc<Vec<T>>>;

/// `MRIS_MP`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MrisMp {
    /// `faces_topology`: each face's three vertex numbers, copied from the `MRIS` by
    /// `MRISMP_load` ("a hack because mrisurf doesn't have a FACE_TOPOLOGY yet").
    pub faces_topology: Option<Vec<VerticesPerFace>>,
    pub v_vsize: In<i32>,
    /// `v_dist_buffer`: each vertex's distance buffer, kept by `MRISMP_makeDist2`.
    pub v_dist_buffer: Option<Vec<Option<Vec<f32>>>>,
    pub f_norm_orig_area: In<f32>,
    pub f_norm_set: Option<Vec<i8>>,
    pub f_v: Option<Vec<VerticesPerFace>>,
    pub f_area: Option<Vec<f32>>,
    pub f_angle: Option<Vec<AnglesPerTriangle>>,
    pub f_orig_angle: In<AnglesPerTriangle>,
    pub f_ripflag: In<i8>,
    pub f_norm: Option<Vec<FloatXyz>>,
    /// `v_dist`: the distances to the neighbours in `xyz` (`vtotal` used).
    pub v_dist: Option<Vec<Option<Vec<f32>>>>,
    pub v_dist_capacity: Option<Vec<i32>>,
    pub v_x: Option<Vec<f32>>,
    pub v_y: Option<Vec<f32>>,
    pub v_z: Option<Vec<f32>>,
    pub v_origx: Option<Vec<f32>>,
    pub v_origy: Option<Vec<f32>>,
    pub v_origz: Option<Vec<f32>>,
    pub v_nx: Option<Vec<f32>>,
    pub v_ny: Option<Vec<f32>>,
    pub v_nz: Option<Vec<f32>>,
    pub v_wnx: In<f32>,
    pub v_wny: In<f32>,
    pub v_wnz: In<f32>,
    pub v_dx: Option<Vec<f32>>,
    pub v_dy: Option<Vec<f32>>,
    pub v_dz: Option<Vec<f32>>,
    pub v_curv: Option<Vec<f32>>,
    pub v_cx: Option<Vec<f32>>,
    pub v_cy: Option<Vec<f32>>,
    pub v_cz: Option<Vec<f32>>,
    pub v_pialx: In<f32>,
    pub v_pialy: In<f32>,
    pub v_pialz: In<f32>,
    pub v_whitex: In<f32>,
    pub v_whitey: In<f32>,
    pub v_whitez: In<f32>,
    pub v_area: Option<Vec<f32>>,
    pub v_origarea: Option<Vec<f32>>,
    pub v_fno: Option<Vec<i32>>,
    pub v_neg: Option<Vec<i8>>,
    pub v_border: Option<Vec<i8>>,
    pub v_ripflag: In<i8>,
    pub nvertices: i32,
    pub nfaces: i32,
    pub xctr: f32,
    pub yctr: f32,
    pub zctr: f32,
    pub xlo: f32,
    pub ylo: f32,
    pub zlo: f32,
    pub xhi: f32,
    pub yhi: f32,
    pub zhi: f32,
    pub total_area: f32,
    pub avg_vertex_area: f64,
    pub avg_vertex_dist: f64,
    pub std_vertex_dist: f64,
    pub orig_area: f32,
    pub neg_area: f32,
    pub neg_orig_area: f32,
    pub radius: f64,
    pub status: MrisStatus,
    pub origxyz_status: MrisStatus,
    /// `patch`: declared, never read (the views read the `MRIS`'s).
    pub patch: i32,
    pub nsize: i8,
    pub dist_nsize: i8,
    /// `noscale`: declared, never read (the views read the `MRIS`'s).
    pub noscale: i32,
}
