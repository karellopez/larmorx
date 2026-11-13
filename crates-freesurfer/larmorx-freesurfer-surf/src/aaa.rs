// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface library's primitive types and constants (`fs.surfio`, queue items 3.1-45 and
//! 3.1-46), translated from FreeSurfer 8.2.0's `include/mrisurf_aaa.h` (constants, `MRIS_Status`,
//! the small structures), `include/base.h`'s `FloatXYZ` and `utils/mrisurf_base.cpp` lines 23-133
//! (`mris_sort_compare_float`, `MRIS_Status_text`, `MRIS_Status_distanceFormula`,
//! `areCompatible`, `checkOrigXYZCompatibleWkr`, `fix_vertex_area` and its two accessors).

use std::any::Any;
use std::fmt;
use std::io::Write;
use std::sync::Arc;

use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::dmatrix::DMatrix;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};

/// `MAX_NEIGHBORS`.
pub const MAX_NEIGHBORS: i32 = 1024;
/// `MRIS_MAX_NEIGHBORHOOD_LINKS`: the bound on `nlinks`.
pub const MRIS_MAX_NEIGHBORHOOD_LINKS: i32 = 50;
/// `MAX_SURFACES`.
pub const MAX_SURFACES: i32 = 20;
/// `TALAIRACH_COORDS`, `SPHERICAL_COORDS`, `ELLIPSOID_COORDS`.
pub const TALAIRACH_COORDS: i32 = 0;
pub const SPHERICAL_COORDS: i32 = 1;
pub const ELLIPSOID_COORDS: i32 = 2;
/// `VERTICES_PER_FACE`, `ANGLES_PER_TRIANGLE`.
pub const VERTICES_PER_FACE: usize = 3;
pub const ANGLES_PER_TRIANGLE: usize = 3;
/// The surfaces' standard names.
pub const INFLATED_NAME: &str = "inflated";
pub const SMOOTH_NAME: &str = "smoothwm";
pub const SPHERE_NAME: &str = "sphere";
pub const ORIG_NAME: &str = "orig";
pub const WHITE_MATTER_NAME: &str = "white";
pub const GRAY_MATTER_NAME: &str = "gray";
pub const LAYERIV_NAME: &str = "graymid";
pub const GRAYMID_NAME: &str = LAYERIV_NAME;
/// `MAX_CMDS`: the size of `MRIS::cmdlines`.
pub const MAX_CMDS: usize = 1000;
/// `NEW_VERSION_MAGIC_NUMBER` (16777215, the triangle files' new-version mark).
pub const NEW_VERSION_MAGIC_NUMBER: i32 = 16_777_215;
/// `VERTEX_SULCAL`.
pub const VERTEX_SULCAL: u32 = 0x0000_0001;

/// `WHICH_FACE_SPLIT(vno0, vno1)`: `1*nint(sqrt(1.9*vno0) + sqrt(3.5*vno1))`, in double (a
/// repeatable arbitrary choice from its parity).
pub fn which_face_split(g: &mut Globals, vno0: i32, vno1: i32) -> i32 {
    let a = (1.9 * f64::from(vno0)).sqrt();
    let b = (3.5 * f64::from(vno1)).sqrt();
    larmorx_freesurfer_core::utils::nint(g, a + b)
}

/// `vertices_per_face_t`: the three vertex numbers of a face, counter-clockwise seen from outside.
pub type VerticesPerFace = [i32; VERTICES_PER_FACE];
/// `angles_per_triangle_t`.
pub type AnglesPerTriangle = [f32; ANGLES_PER_TRIANGLE];

/// `copyAnglesPerTriangle(dst, src)`: `dst = src`.
pub fn copy_angles_per_triangle(dst: &mut AnglesPerTriangle, src: &AnglesPerTriangle) {
    *dst = *src;
}

/// `cmpAnglesPerTriangle(dst, src)`: `memcmp` of the two arrays' 12 bytes (their bits, so `-0` and
/// `+0` differ and equal NaNs compare equal): 0 when equal, else the difference of the first
/// differing bytes as unsigned values (`dst`'s minus `src`'s), which is what glibc 2.35's x86-64
/// `memcmp` returns for 12 bytes (observed through the probe on the reference machine:
/// `specs/freesurfer-surfio.md` §3). FreeSurfer's callers compare it with 0, or print it.
pub fn cmp_angles_per_triangle(dst: &AnglesPerTriangle, src: &AnglesPerTriangle) -> i32 {
    for (a, b) in dst.iter().zip(src) {
        let (a, b) = (a.to_bits().to_le_bytes(), b.to_bits().to_le_bytes());
        for k in 0..4 {
            if a[k] != b[k] {
                return i32::from(a[k]) - i32::from(b[k]);
            }
        }
    }
    0
}

/// `FloatXYZ` (`include/base.h`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatXyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `MRIS_XYZ`.
pub type MrisXyz = FloatXyz;

/// `MRIS_HASH`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MrisHash {
    pub hash: u64,
}

/// `MRIS_Status`: the kind of surface. A C enumeration, so any `int` can be stored in it (the
/// functions below meet values outside the list as FreeSurfer does).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MrisStatus(pub i32);

impl MrisStatus {
    pub const SURFACE: MrisStatus = MrisStatus(0);
    pub const PATCH: MrisStatus = MrisStatus(1);
    pub const PLANE: MrisStatus = MrisStatus(2);
    pub const ELLIPSOID: MrisStatus = MrisStatus(3);
    pub const SPHERE: MrisStatus = MrisStatus(4);
    pub const PARAMETERIZED_SPHERE: MrisStatus = MrisStatus(5);
    pub const RIGID_BODY: MrisStatus = MrisStatus(6);
    pub const SPHERICAL_PATCH: MrisStatus = MrisStatus(7);
    pub const UNORIENTED_SPHERE: MrisStatus = MrisStatus(8);
    pub const PIAL_SURFACE: MrisStatus = MrisStatus(9);
    /// `MRIS_Status__end`.
    pub const END: MrisStatus = MrisStatus(10);
    /// `MRIS_CUT` (= `MRIS_PATCH`).
    pub const CUT: MrisStatus = MrisStatus(1);
}

/// The names and distance formulas of `MRIS_Status_ELTS`, in order.
const STATUS_ELTS: [(&str, i32); 10] = [
    ("MRIS_SURFACE", 0),
    ("MRIS_PATCH", 0),
    ("MRIS_PLANE", 0),
    ("MRIS_ELLIPSOID", 0),
    ("MRIS_SPHERE", 1),
    ("MRIS_PARAMETERIZED_SPHERE", 1),
    ("MRIS_RIGID_BODY", 0),
    ("MRIS_SPHERICAL_PATCH", 0),
    ("MRIS_UNORIENTED_SPHERE", 0),
    ("MRIS_PIAL_SURFACE", 0),
];

/// `MRIS_Status_DistanceFormula` (`_0` the plane formula, `_1` the sphere's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceFormula {
    Formula0,
    Formula1,
}

/// The source file `cheapAssert` names in `mrisurf_base.cpp`'s failures (the official build's).
fn base_cpp() -> String {
    format!("{}/utils/mrisurf_base.cpp", diag::BUILD_ROOT)
}

fn status_index(s: MrisStatus) -> Option<usize> {
    usize::try_from(s.0).ok().filter(|&i| i < STATUS_ELTS.len())
}

/// `MRIS_Status_text(s)`: the enumerator's name; a value outside the list fails
/// `cheapAssert(false)` (`mrisurf_base.cpp`, line 50), which ends the program (FS-03).
pub fn status_text(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: MrisStatus,
) -> Result<&'static str, Fatal> {
    match status_index(s) {
        Some(i) => Ok(STATUS_ELTS[i].0),
        None => Err(diag::assert_failed(g, out, err, &base_cpp(), 50, "false")),
    }
}

/// `MRIS_Status_distanceFormula(s)`; a value outside the list fails `cheapAssert(false)`
/// (line 63).
pub fn status_distance_formula(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: MrisStatus,
) -> Result<DistanceFormula, Fatal> {
    match status_index(s) {
        Some(i) if STATUS_ELTS[i].1 == 1 => Ok(DistanceFormula::Formula1),
        Some(_) => Ok(DistanceFormula::Formula0),
        None => Err(diag::assert_failed(g, out, err, &base_cpp(), 63, "false")),
    }
}

/// `areCompatible(s1, s2)`: the two kinds use the same distance formula (`s1`'s checked first).
pub fn are_compatible(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s1: MrisStatus,
    s2: MrisStatus,
) -> Result<bool, Fatal> {
    let a = status_distance_formula(g, out, err, s1)?;
    let b = status_distance_formula(g, out, err, s2)?;
    Ok(a == b)
}

/// `checkOrigXYZCompatibleWkr(s1, s2, file, line)`: nothing when compatible, else the debug
/// message `using incompatible <s1> and <s2>` (shown with `DIAG_VERBOSE`).
pub fn check_orig_xyz_compatible(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s1: MrisStatus,
    s2: MrisStatus,
) -> Result<(), Fatal> {
    if are_compatible(g, out, err, s1, s2)? {
        return Ok(());
    }
    let t1 = status_text(g, out, err, s1)?;
    let t2 = status_text(g, out, err, s2)?;
    log::debug(g, err, &format!("using incompatible {t1} and {t2}"));
    Ok(())
}

/// `mris_sort_compare_float(a, b)`: 1, -1 or 0 by `>` then `<` (NaNs compare 0).
pub fn mris_sort_compare_float(a: f32, b: f32) -> i32 {
    if a > b {
        1
    } else if a < b {
        -1
    } else {
        0
    }
}

/// `MRIS_AREA_LABEL`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MrisAreaLabel {
    /// `name[STRLEN]`.
    pub name: String,
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub label: i32,
}

/// `FaceNormCacheEntry`: a face's cached normal and original area.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceNormCacheEntry {
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub orig_area: f32,
}

/// `FaceNormDeferredEntry`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceNormDeferredEntry {
    pub deferred: i8,
}

/// `MRI_EDGE`: an edge with its two ends, the two opposite vertices and their metrics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MriEdge {
    pub edgeno: i32,
    /// The 2 ends, then the 2 opposites.
    pub vtxno: [i32; 4],
    pub faceno: [i32; 2],
    /// `corner[nthvtx][faceno]`.
    pub corner: [[u8; 2]; 4],
    pub len: f64,
    pub dot: f64,
    pub angle: f64,
    #[allow(non_snake_case)]
    pub J: f64,
    pub u: [f64; 3],
    pub area: [f64; 2],
    pub maxarea: f64,
    #[allow(non_snake_case)]
    pub gradU: Option<DMatrix>,
    #[allow(non_snake_case)]
    pub gradDot: [Option<DMatrix>; 4],
}

/// `MRI_CORNER`: a corner of a triangle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MriCorner {
    pub cornerno: i32,
    /// Vertex 0 is the source.
    pub vtxno: [i32; 3],
    pub faceno: i32,
    pub edgeno: [i32; 2],
    pub edgedir: [i32; 2],
    pub dot: f64,
    pub angle: f64,
    #[allow(non_snake_case)]
    pub J: f64,
    #[allow(non_snake_case)]
    pub gradDot: [Option<DMatrix>; 3],
}

/// `STRIP`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Strip {
    pub nvertices: i32,
    pub vertex_indices: Option<Vec<u32>>,
}

/// A `void *` field (`vp`, `user_parms`, `mht`, `temps`): whatever the caller hangs there,
/// shared on a copy as C copies the pointer.
#[derive(Clone, Default)]
pub struct Opaque(pub Option<Arc<dyn Any + Send + Sync>>);

impl fmt::Debug for Opaque {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "Opaque(set)"
        } else {
            "Opaque(NULL)"
        })
    }
}

impl PartialEq for Opaque {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// The surface library's process-wide variables (FreeSurfer's globals of `mrisurf.cpp` and
/// `mrisurf_base.cpp`), one value per run beside [`Globals`].
#[derive(Clone, Debug)]
pub struct SurfGlobals {
    /// `UnitizeNormalFace` (1): `mrisNormalFace` makes its normal unit length.
    pub unitize_normal_face: i32,
    /// `RmsValErrorRecord` (0).
    pub rms_val_error_record: i32,
    /// `fix_vertex_area` (1).
    pub fix_vertex_area: i32,
    /// `mrisurf_activeRealmTreesSize` (0).
    pub active_realm_trees_size: i32,
    /// `modVnum`'s `static short maxVnumSeen` (30).
    pub max_vnum_seen: i16,
    /// `NEG_AREA_K` (10.0, `mrisurf_base.cpp`).
    pub neg_area_k: f64,
    /// `mrisurf_surface_names[3]` (`"inflated"`, `"smoothwm"`, `"smoothwm"`): the surfaces whose
    /// curvatures are computed when [`SurfGlobals::curvature_names`] has none
    /// (`mrisurf_base.cpp` line 937).
    pub surface_names: [Option<Vec<u8>>; 3],
    /// `curvature_names[3]` (`"inflated.H"`, `"sulc"`, `NULL`): the curvature files read instead
    /// (line 938).
    pub curvature_names: [Option<Vec<u8>>; 3],
    /// `sigmas` (`mrisurf_base.cpp` line 969): the spherical registration's blurring scales
    /// ([`SIGMAS_DEFAULT`] until `MRISsetRegistrationSigmas`).
    pub sigmas: Vec<f32>,
    /// `nsigmas` (a `double`, line 970): their count.
    pub nsigmas: f64,
    /// `count_MRIScomputeMetricProperties_calls` (`mrisurf_metricProperties.cpp` line 1839): the
    /// calls of `MRIScomputeMetricProperties` so far (not those `FS_FASTER_MP` sends elsewhere),
    /// which `MRISMP_unload`'s comparison lines print.
    pub mp_calls: i32,
    /// `comparisonWkr`'s `static int count` (`mrisurf_metricProperties_fast.h` line 333): the
    /// comparison lines printed so far (the first also prints `b <file>:335  needed`).
    pub mp_comparisons: i32,
    /// FS-120's correction, on request (not FreeSurfer's; `false` by default): with
    /// [`larmorx_core::defects::UpstreamDefects::Fix`], the default computation of the metric
    /// properties writes the faces' angles back into the surface, as FreeSurfer's two other
    /// computations do. `false` keeps FreeSurfer's behaviour (rule D: kept until its effect on
    /// `mris_sphere` and `mris_register` is measured with their ports).
    pub fs120_write_face_angles: bool,
    /// [`larmorx_core::defects::FS_368`]: with this set and
    /// [`larmorx_core::defects::UpstreamDefects::Fix`], `MRIScomputeSurfaceNormals` restores the
    /// current positions it saved. `false` keeps FreeSurfer's behaviour (rule D: the surface is
    /// left at the positions whose normals were computed, until its callers' ports measure it).
    pub fs368_restore_current: bool,
    /// `mris_readval_frame` (`mrisurf_io.cpp` line 64, -1): the frame `MRISreadValues` reads
    /// from a volume, `MRISsetReadFrame`'s.
    pub readval_frame: i32,
    /// `mrisAddEdgeWkr`'s `static int maxVnum` (20, `mrisurf_topology.cpp` line 396).
    pub add_edge_max_vnum: i32,
    /// `mrisCompleteTopology`'s choice, read at its first call (`static bool use_new`: whether
    /// `mrisCompleteTopology_new` is set).
    pub complete_topology_new: Option<bool>,
    /// `MRISfindNeighborsAtVertex`'s `static int laterTime`: its calls so far.
    pub find_nbrs_calls: i32,
    /// `MRISfindNeighborsAtVertex`'s searches (`use_new`, `use_old`), chosen at its first call.
    pub find_nbrs_use: (bool, bool),
    /// `MRISfindNeighborsAtVertex`'s `static bool shownTesting`.
    pub find_nbrs_shown_testing: bool,
    /// `MRISfindNeighborsAtVertex_old`'s `static bool laterTime` (its first-call line shown).
    pub find_nbrs_old_shown: bool,
    /// `MRISfindNeighborsAtVertex_newWkr`'s set of vertices (`tempForEachThread[0].status`:
    /// one byte per vertex, kept from call to call, zeroed whole when it grows).
    pub find_nbrs_status: Vec<u8>,
    /// `mrisCheckVertexVertexTopologyWkr`'s `forceAlways` (`FREESURFER_always_check_vv` set), read
    /// at its first call.
    pub check_vv_force_always: Option<bool>,
    /// `mrisCheckVertexVertexTopologyWkr`'s `checkDist` (`FREESURFER_checkDist` set), read once.
    pub check_vv_check_dist: Option<bool>,
}

/// `sigmas_default` (`mrisurf_base.cpp` line 962).
pub const SIGMAS_DEFAULT: [f32; 4] = [4.0, 2.0, 1.0, 0.5];

impl Default for SurfGlobals {
    fn default() -> Self {
        SurfGlobals {
            unitize_normal_face: 1,
            rms_val_error_record: 0,
            fix_vertex_area: 1,
            active_realm_trees_size: 0,
            max_vnum_seen: 30,
            neg_area_k: 10.0,
            surface_names: [
                Some(b"inflated".to_vec()),
                Some(b"smoothwm".to_vec()),
                Some(b"smoothwm".to_vec()),
            ],
            curvature_names: [Some(b"inflated.H".to_vec()), Some(b"sulc".to_vec()), None],
            sigmas: SIGMAS_DEFAULT.to_vec(),
            nsigmas: SIGMAS_DEFAULT.len() as f64,
            mp_calls: 0,
            mp_comparisons: 0,
            fs120_write_face_angles: false,
            fs368_restore_current: false,
            readval_frame: -1,
            add_edge_max_vnum: 20,
            complete_topology_new: None,
            find_nbrs_calls: 0,
            find_nbrs_use: (false, false),
            find_nbrs_shown_testing: false,
            find_nbrs_old_shown: false,
            find_nbrs_status: Vec::new(),
            check_vv_force_always: None,
            check_vv_check_dist: None,
        }
    }
}

/// `MRISsetFixVertexAreaValue(value)`: sets `fix_vertex_area` and returns it.
pub fn set_fix_vertex_area_value(sg: &mut SurfGlobals, value: i32) -> i32 {
    sg.fix_vertex_area = value;
    sg.fix_vertex_area
}

/// `MRISgetFixVertexAreaValue()`.
pub fn get_fix_vertex_area_value(sg: &SurfGlobals) -> i32 {
    sg.fix_vertex_area
}

/// `hasActiveRealmTrees()` (`include/mrisurf_base.h`).
pub fn has_active_realm_trees(sg: &SurfGlobals) -> bool {
    sg.active_realm_trees_size > 0
}
