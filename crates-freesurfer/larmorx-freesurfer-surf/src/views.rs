// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The generated surface views (`fs.surfio`, queue items 3.1-45 and 3.1-46), translated from
//! FreeSurfer 8.2.0's `include/mrisurf_SurfaceFrom{MRIS,MRIS_MP,MRISPV}_generated_*.h` (made by
//! FreeSurfer's `utils_generator/Generator.cpp`) and `MRISvertexCoord2XYZ_float`/`_double`
//! (`utils/mrisurf_metricProperties.cpp` lines 3721-3758, which the `MRIS` views' `which_coords`
//! call).
//!
//! In C++ each representation of a surface (`MRIS`, `MRIS_MP`, `MRISPV`) has 13 namespaces of
//! `Face`, `Vertex` and `Surface` classes (`Existence` ... `AllM`), whose member functions read or
//! write one field each. The access levels are compile-time permissions only: a member function
//! reads and writes the same storage at every level where it exists (checked for all 6,331 bodies
//! by `validation/investigations/freesurfer-surfio/accessors.py`). The behaviour is therefore which
//! storage each accessor reaches, and that is what this module keeps:
//!
//! - [`MetricSurface`]: the accessors the `MRIS_MP` views offer, which FreeSurfer's code templated
//!   over both `MRIS` and `MRIS_MP` uses (`mrisurf_sseTerms.cpp`'s `SseTerms_DistortedSurfaces`,
//!   `mrisurf_metricProperties.cpp`'s templates), implemented for [`Mris`] (each accessor its
//!   vertex's, face's or surface's field) and for [`MpView`] (an `MRIS_MP` with the `MRIS` it was
//!   loaded from: the `v_*`/`f_*` arrays, and the three that read the `MRIS`: `status`, `patch` and
//!   `noscale`). Left out: the face-normal accessors (`norm` is a `DMATRIX *` in `MRIS` and a
//!   `FloatXYZ` in `MRIS_MP`) and `faceNormCacheEntries`/`faceNormDeferredEntries` (never filled in
//!   an `MRIS_MP`): no templated code calls them.
//! - The `MRIS` views' other accessors are the fields of [`Mris`], [`Vertex`], [`Face`] and
//!   [`crate::mris::VertexTopology`] themselves (a `Vertex::v(i)` is `vertices_topology[vno].v[i]`,
//!   `Face::v(i)` is `faces[fno].v[i]`, a pole's view its vertex number).
//! - `MRISPV`: no FreeSurfer 8.2.0 code creates one (no constructor, allocation or loader exists;
//!   the only functions taking one are `MRISfreeDistsButNotOrig`, an assertion "NYI", and
//!   `mrishash.cpp`'s template instances, never called with one), so its views are unreachable and
//!   the replica has none.
//! - `which_coords`: only `mrishash.cpp` calls it, on `MRIS` views (and on `MRISPV` ones, never
//!   built), where it is [`vertex_coord2xyz_float`]. The `MRIS_MP` views' `which_coords` (their own
//!   switch: [`which_coords_mp_cases`]) is never called; the table is kept for the spec.

use std::io::Write;
use std::sync::Arc;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};

use crate::aaa::{AnglesPerTriangle, MrisStatus};
use crate::mris::{Mris, Vertex};
use crate::mris_mp::MrisMp;

/// The vertex coordinate sets (`which`, `include/mrisurf.h`).
pub const ORIGINAL_VERTICES: i32 = 0;
pub const TMP_VERTICES: i32 = 2;
pub const CANONICAL_VERTICES: i32 = 3;
pub const CURRENT_VERTICES: i32 = 4;
pub const INFLATED_VERTICES: i32 = 5;
pub const FLATTENED_VERTICES: i32 = 6;
pub const PIAL_VERTICES: i32 = 7;
pub const TMP2_VERTICES: i32 = 8;
pub const WHITE_VERTICES: i32 = 9;
pub const TARGET_VERTICES: i32 = 10;
pub const LAYERIV_VERTICES: i32 = 11;
pub const VERTEX_NORMALS: i32 = 12;
pub const PIAL_NORMALS: i32 = 13;
pub const WHITE_NORMALS: i32 = 14;

/// The 13 access levels of the generated views (C++ namespaces), in the generator's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    Existence,
    Topology,
    XyzPosition,
    XyzPositionConsequences,
    Distort,
    Analysis,
    ExistenceM,
    TopologyM,
    XyzPositionM,
    XyzPositionConsequencesM,
    DistortM,
    AnalysisM,
    AllM,
}

impl Level {
    pub const ALL: [Level; 13] = [
        Level::Existence,
        Level::Topology,
        Level::XyzPosition,
        Level::XyzPositionConsequences,
        Level::Distort,
        Level::Analysis,
        Level::ExistenceM,
        Level::TopologyM,
        Level::XyzPositionM,
        Level::XyzPositionConsequencesM,
        Level::DistortM,
        Level::AnalysisM,
        Level::AllM,
    ];
}

/// `MRISvertexCoord2XYZ_float(v, which, &x, &y, &z)`: the vertex's coordinates of the set
/// `which` (current, original, canonical, temporary, second temporary, inflated, flattened, pial,
/// white, and the current, pial and white normals); any other `which` (`TARGET_VERTICES` and
/// `LAYERIV_VERTICES` included) is `ErrorExit(ERROR_UNSUPPORTED, "MRISvertexCoord2XYZ_float:
/// unsupported which %d")`. The `MRIS` views' `which_coords` at every level.
pub fn vertex_coord2xyz_float(
    g: &mut Globals,
    err: &mut dyn Write,
    v: &Vertex,
    which: i32,
) -> Result<(f32, f32, f32), Fatal> {
    Ok(match which {
        ORIGINAL_VERTICES => (v.origx, v.origy, v.origz),
        TMP_VERTICES => (v.tx, v.ty, v.tz),
        CANONICAL_VERTICES => (v.cx, v.cy, v.cz),
        CURRENT_VERTICES => (v.x, v.y, v.z),
        INFLATED_VERTICES => (v.infx, v.infy, v.infz),
        PIAL_VERTICES => (v.pialx, v.pialy, v.pialz),
        TMP2_VERTICES => (v.t2x, v.t2y, v.t2z),
        FLATTENED_VERTICES => (v.fx, v.fy, v.fz),
        WHITE_VERTICES => (v.whitex, v.whitey, v.whitez),
        VERTEX_NORMALS => (v.nx, v.ny, v.nz),
        PIAL_NORMALS => (v.pnx, v.pny, v.pnz),
        WHITE_NORMALS => (v.wnx, v.wny, v.wnz),
        _ => {
            log::error_exit(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRISvertexCoord2XYZ_float: unsupported which {which}"),
            )?;
            // With exceptions on and a code of 0 ErrorExit returns; ERROR_UNSUPPORTED is never 0.
            unreachable!("ErrorExit with a non-zero code always ends the call")
        }
    })
}

/// `MRISvertexCoord2XYZ_double`: [`vertex_coord2xyz_float`] widened to double.
pub fn vertex_coord2xyz_double(
    g: &mut Globals,
    err: &mut dyn Write,
    v: &Vertex,
    which: i32,
) -> Result<(f64, f64, f64), Fatal> {
    let (x, y, z) = vertex_coord2xyz_float(g, err, v, which)?;
    Ok((f64::from(x), f64::from(y), f64::from(z)))
}

/// The cases of the `MRIS_MP` views' own `which_coords` at each level (`which`, the field
/// prefix), in the generated switch's order; any other `which` sets the three outputs to 0 and is
/// `ErrorExit(ERROR_UNSUPPORTED, "which_coords: unsupported which %d")`. Never called by
/// FreeSurfer 8.2.0 (the module's documentation).
pub fn which_coords_mp_cases(level: Level) -> &'static [(i32, &'static str)] {
    const XYZ: &[(i32, &str)] = &[
        (CURRENT_VERTICES, ""),
        (ORIGINAL_VERTICES, "orig"),
        (CANONICAL_VERTICES, "c"),
    ];
    const CONSEQ: &[(i32, &str)] = &[
        (CURRENT_VERTICES, ""),
        (ORIGINAL_VERTICES, "orig"),
        (VERTEX_NORMALS, "n"),
        (CANONICAL_VERTICES, "c"),
    ];
    const DISTORT: &[(i32, &str)] = &[
        (CURRENT_VERTICES, ""),
        (ORIGINAL_VERTICES, "orig"),
        (VERTEX_NORMALS, "n"),
        (WHITE_NORMALS, "wn"),
        (CANONICAL_VERTICES, "c"),
        (PIAL_VERTICES, "pial"),
        (WHITE_VERTICES, "white"),
    ];
    match level {
        Level::Existence | Level::Topology | Level::ExistenceM | Level::TopologyM => &[],
        Level::XyzPosition | Level::XyzPositionM => XYZ,
        Level::XyzPositionConsequences | Level::XyzPositionConsequencesM => CONSEQ,
        _ => DISTORT,
    }
}

/// An `MRIS_MP` with the `MRIS` it was loaded from (`mp->underlyingMRIS`): what the `MRIS_MP`
/// views read.
pub struct MpView<'a> {
    pub mp: &'a mut MrisMp,
    pub mris: &'a mut Mris,
}

fn arr<'a, T>(a: &'a Option<Vec<T>>, what: &str) -> &'a [T] {
    a.as_deref()
        .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL"))
}

fn arr_mut<'a, T>(a: &'a mut Option<Vec<T>>, what: &str) -> &'a mut [T] {
    a.as_deref_mut()
        .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL"))
}

fn shared<'a, T>(a: &'a Option<Arc<Vec<T>>>, what: &str) -> &'a [T] {
    a.as_deref()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL"))
}

/// A write into an input array: FreeSurfer's copies share it (none of its code writes one); the
/// replica writes this copy's.
fn shared_mut<'a, T: Clone>(a: &'a mut Option<Arc<Vec<T>>>, what: &str) -> &'a mut [T] {
    Arc::make_mut(
        a.as_mut()
            .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL")),
    )
    .as_mut_slice()
}

pub use accessors::MetricSurface;

/// The accessor table: vertex fields (read-write, then read-only) and surface fields.
mod accessors {
    use super::*;

    macro_rules! table {
        (
            vertex_rw { $($vf:ident / $vset:ident : $vt:ty = $vm:ident, $vmp:ident),* $(,)? }
            vertex_ro { $($rf:ident : $rt:ty = $rm:ident, $rmp:ident),* $(,)? }
            surface_rw { $($sf:ident / $sset:ident : $st:ty),* $(,)? }
            surface_mris { $($mf:ident / $mset:ident : $mt:ty),* $(,)? }
            surface_ro { $($of:ident : $ot:ty),* $(,)? }
            surface_mris_ro { $($nf:ident : $nt:ty),* $(,)? }
        ) => {
            /// The `MRIS_MP` views' accessors (the module's documentation), by vertex or face
            /// number; vertex accessors are named `v_<field>`, face ones `f_<field>`, surface ones
            /// by the field. Reading a `NULL` array panics, where FreeSurfer reads through a null
            /// pointer.
            #[allow(non_snake_case)]
            pub trait MetricSurface {
                fn f_v(&self, fno: usize, i: usize) -> i32;
                fn set_f_v(&mut self, fno: usize, i: usize, to: i32);
                fn f_area(&self, fno: usize) -> f32;
                fn set_f_area(&mut self, fno: usize, to: f32);
                fn f_angle(&self, fno: usize) -> AnglesPerTriangle;
                fn set_f_angle(&mut self, fno: usize, to: AnglesPerTriangle);
                fn f_orig_angle(&self, fno: usize) -> AnglesPerTriangle;
                fn set_f_orig_angle(&mut self, fno: usize, to: AnglesPerTriangle);
                fn f_ripflag(&self, fno: usize) -> i8;
                fn set_f_ripflag(&mut self, fno: usize, to: i8);
                fn v_dist(&self, vno: usize, i: usize) -> f32;
                fn v_dist_orig(&self, vno: usize, i: usize) -> f32;
                fn v_dist_capacity(&self, vno: usize) -> i32;
                $( fn $vf(&self, vno: usize) -> $vt; fn $vset(&mut self, vno: usize, to: $vt); )*
                $( fn $rf(&self, vno: usize) -> $rt; )*
                $( fn $sf(&self) -> $st; fn $sset(&mut self, to: $st); )*
                $( fn $mf(&self) -> $mt; fn $mset(&mut self, to: $mt); )*
                $( fn $of(&self) -> $ot; )*
                $( fn $nf(&self) -> $nt; )*
            }

            #[allow(non_snake_case)]
            impl MetricSurface for Mris {
                fn f_v(&self, fno: usize, i: usize) -> i32 { self.face(fno).v[i] }
                fn set_f_v(&mut self, fno: usize, i: usize, to: i32) { self.face_mut(fno).v[i] = to; }
                fn f_area(&self, fno: usize) -> f32 { self.face(fno).area }
                fn set_f_area(&mut self, fno: usize, to: f32) { self.face_mut(fno).area = to; }
                fn f_angle(&self, fno: usize) -> AnglesPerTriangle { self.face(fno).angle }
                fn set_f_angle(&mut self, fno: usize, to: AnglesPerTriangle) {
                    self.face_mut(fno).angle = to;
                }
                fn f_orig_angle(&self, fno: usize) -> AnglesPerTriangle { self.face(fno).orig_angle }
                fn set_f_orig_angle(&mut self, fno: usize, to: AnglesPerTriangle) {
                    self.face_mut(fno).orig_angle = to;
                }
                fn f_ripflag(&self, fno: usize) -> i8 { self.face(fno).ripflag }
                fn set_f_ripflag(&mut self, fno: usize, to: i8) { self.face_mut(fno).ripflag = to; }
                fn v_dist(&self, vno: usize, i: usize) -> f32 {
                    self.vertex(vno).dist.as_ref().expect("VERTEX::dist is NULL")[i]
                }
                fn v_dist_orig(&self, vno: usize, i: usize) -> f32 {
                    self.vertex(vno).dist_orig.as_ref().expect("VERTEX::dist_orig is NULL")[i]
                }
                fn v_dist_capacity(&self, vno: usize) -> i32 { self.vertex(vno).dist_capacity }
                $(
                    fn $vf(&self, vno: usize) -> $vt { self.vertex(vno).$vm }
                    fn $vset(&mut self, vno: usize, to: $vt) { self.vertex_mut(vno).$vm = to; }
                )*
                $( fn $rf(&self, vno: usize) -> $rt { self.vertex(vno).$rm } )*
                $(
                    fn $sf(&self) -> $st { self.$sf }
                    fn $sset(&mut self, to: $st) { self.$sf = to; }
                )*
                $(
                    fn $mf(&self) -> $mt { self.$mf }
                    fn $mset(&mut self, to: $mt) { self.$mf = to; }
                )*
                $( fn $of(&self) -> $ot { self.$of } )*
                $( fn $nf(&self) -> $nt { self.$nf } )*
            }

            #[allow(non_snake_case)]
            impl MetricSurface for MpView<'_> {
                fn f_v(&self, fno: usize, i: usize) -> i32 { arr(&self.mp.f_v, "f_v")[fno][i] }
                fn set_f_v(&mut self, fno: usize, i: usize, to: i32) {
                    arr_mut(&mut self.mp.f_v, "f_v")[fno][i] = to;
                }
                fn f_area(&self, fno: usize) -> f32 { arr(&self.mp.f_area, "f_area")[fno] }
                fn set_f_area(&mut self, fno: usize, to: f32) {
                    arr_mut(&mut self.mp.f_area, "f_area")[fno] = to;
                }
                fn f_angle(&self, fno: usize) -> AnglesPerTriangle {
                    arr(&self.mp.f_angle, "f_angle")[fno]
                }
                fn set_f_angle(&mut self, fno: usize, to: AnglesPerTriangle) {
                    arr_mut(&mut self.mp.f_angle, "f_angle")[fno] = to;
                }
                fn f_orig_angle(&self, fno: usize) -> AnglesPerTriangle {
                    shared(&self.mp.f_orig_angle, "f_orig_angle")[fno]
                }
                fn set_f_orig_angle(&mut self, fno: usize, to: AnglesPerTriangle) {
                    shared_mut(&mut self.mp.f_orig_angle, "f_orig_angle")[fno] = to;
                }
                fn f_ripflag(&self, fno: usize) -> i8 { shared(&self.mp.f_ripflag, "f_ripflag")[fno] }
                fn set_f_ripflag(&mut self, fno: usize, to: i8) {
                    shared_mut(&mut self.mp.f_ripflag, "f_ripflag")[fno] = to;
                }
                fn v_dist(&self, vno: usize, i: usize) -> f32 {
                    arr(&self.mp.v_dist, "v_dist")[vno]
                        .as_ref()
                        .expect("MRIS_MP::v_dist[vno] is NULL")[i]
                }
                /// `v_dist_orig[vno][i]`: the `MRIS`'s own `dist_orig` (the module's documentation).
                fn v_dist_orig(&self, vno: usize, i: usize) -> f32 {
                    self.mris.vertex(vno).dist_orig.as_ref().expect("VERTEX::dist_orig is NULL")[i]
                }
                fn v_dist_capacity(&self, vno: usize) -> i32 {
                    arr(&self.mp.v_dist_capacity, "v_dist_capacity")[vno]
                }
                $(
                    fn $vf(&self, vno: usize) -> $vt {
                        mp_get!(self.mp.$vmp, stringify!($vmp))[vno]
                    }
                    fn $vset(&mut self, vno: usize, to: $vt) {
                        mp_set!(self.mp.$vmp, stringify!($vmp))[vno] = to;
                    }
                )*
                $( fn $rf(&self, vno: usize) -> $rt { mp_get!(self.mp.$rmp, stringify!($rmp))[vno] } )*
                $(
                    fn $sf(&self) -> $st { self.mp.$sf }
                    fn $sset(&mut self, to: $st) { self.mp.$sf = to; }
                )*
                $(
                    fn $mf(&self) -> $mt { self.mris.$mf }
                    fn $mset(&mut self, to: $mt) { self.mris.$mf = to; }
                )*
                $( fn $of(&self) -> $ot { self.mp.$of } )*
                $( fn $nf(&self) -> $nt { self.mris.$nf } )*
            }
        };
    }

    /// Reads an `MRIS_MP` array, owned or shared.
    macro_rules! mp_get {
        ($a:expr, $what:expr) => {
            MpArray::get(&$a, $what)
        };
    }
    macro_rules! mp_set {
        ($a:expr, $what:expr) => {
            MpArray::get_mut(&mut $a, $what)
        };
    }

    /// An `MRIS_MP` array: owned (`Option<Vec<T>>`) or shared by the copies (`In<T>`).
    trait MpArray<T> {
        fn get(&self, what: &str) -> &[T];
        fn get_mut(&mut self, what: &str) -> &mut [T];
    }

    impl<T> MpArray<T> for Option<Vec<T>> {
        fn get(&self, what: &str) -> &[T] {
            arr(self, what)
        }
        fn get_mut(&mut self, what: &str) -> &mut [T] {
            arr_mut(self, what)
        }
    }

    impl<T: Clone> MpArray<T> for Option<Arc<Vec<T>>> {
        fn get(&self, what: &str) -> &[T] {
            shared(self, what)
        }
        fn get_mut(&mut self, what: &str) -> &mut [T] {
            shared_mut(self, what)
        }
    }

    table! {
        vertex_rw {
            v_x / set_v_x: f32 = x, v_x,
            v_y / set_v_y: f32 = y, v_y,
            v_z / set_v_z: f32 = z, v_z,
            v_nx / set_v_nx: f32 = nx, v_nx,
            v_ny / set_v_ny: f32 = ny, v_ny,
            v_nz / set_v_nz: f32 = nz, v_nz,
            v_wnx / set_v_wnx: f32 = wnx, v_wnx,
            v_wny / set_v_wny: f32 = wny, v_wny,
            v_wnz / set_v_wnz: f32 = wnz, v_wnz,
            v_dx / set_v_dx: f32 = dx, v_dx,
            v_dy / set_v_dy: f32 = dy, v_dy,
            v_dz / set_v_dz: f32 = dz, v_dz,
            v_curv / set_v_curv: f32 = curv, v_curv,
            v_cx / set_v_cx: f32 = cx, v_cx,
            v_cy / set_v_cy: f32 = cy, v_cy,
            v_cz / set_v_cz: f32 = cz, v_cz,
            v_pialx / set_v_pialx: f32 = pialx, v_pialx,
            v_pialy / set_v_pialy: f32 = pialy, v_pialy,
            v_pialz / set_v_pialz: f32 = pialz, v_pialz,
            v_whitex / set_v_whitex: f32 = whitex, v_whitex,
            v_whitey / set_v_whitey: f32 = whitey, v_whitey,
            v_whitez / set_v_whitez: f32 = whitez, v_whitez,
            v_area / set_v_area: f32 = area, v_area,
            v_origarea / set_v_origarea: f32 = origarea, v_origarea,
            v_fno / set_v_fno: i32 = fno, v_fno,
            v_neg / set_v_neg: i8 = neg, v_neg,
            v_border / set_v_border: i8 = border, v_border,
            v_ripflag / set_v_ripflag: i8 = ripflag, v_ripflag,
        }
        vertex_ro {
            v_origx: f32 = origx, v_origx,
            v_origy: f32 = origy, v_origy,
            v_origz: f32 = origz, v_origz,
        }
        surface_rw {
            xctr / set_xctr: f32,
            yctr / set_yctr: f32,
            zctr / set_zctr: f32,
            xlo / set_xlo: f32,
            ylo / set_ylo: f32,
            zlo / set_zlo: f32,
            xhi / set_xhi: f32,
            yhi / set_yhi: f32,
            zhi / set_zhi: f32,
            total_area / set_total_area: f32,
            avg_vertex_area / set_avg_vertex_area: f64,
            avg_vertex_dist / set_avg_vertex_dist: f64,
            std_vertex_dist / set_std_vertex_dist: f64,
            orig_area / set_orig_area: f32,
            neg_area / set_neg_area: f32,
            neg_orig_area / set_neg_orig_area: f32,
            origxyz_status / set_origxyz_status: MrisStatus,
        }
        surface_mris {
            status / set_status: MrisStatus,
            patch / set_patch: i32,
        }
        surface_ro {
            nvertices: i32,
            nfaces: i32,
            radius: f64,
        }
        surface_mris_ro {
            noscale: i32,
        }
    }
}
