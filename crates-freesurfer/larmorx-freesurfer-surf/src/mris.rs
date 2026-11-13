// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface structure (`fs.surfio`, queue item 3.1-46), translated from FreeSurfer 8.2.0's
//! `include/mrisurf_FACE_VERTEX_MRIS_generated.h`: `FACE`, `VERTEX_TOPOLOGY`, `VERTEX` and `MRIS`
//! with their fields in FreeSurfer's order and C types (`char` is `i8`, as on x86-64; `uchar`
//! `u8`; `short` `i16`; `int` `i32`; `uint` `u32`).
//!
//! C's pointers become: an array a function allocates, `Option<Vec<T>>` (`None` is `NULL`, which
//! FreeSurfer's code tests in places: `mrisVertexNeighborIndex`); a matrix, `Option<Matrix>` or
//! `Option<DMatrix>`; a pointer to a vertex of the same surface (`v_temporal_pole` and the other
//! two poles), its vertex number; a `void *`, [`Opaque`]. The arrays' lengths are what was
//! allocated; FreeSurfer's counts (`num`, `vtotal`, `dist_capacity`, `nvertices`, ...) stay fields
//! of their own, as in C.
//!
//! Not yet here (they wait for their modules' structures, which are other items): `MRIS::lta`
//! (`LTA *`, fs.volume's transforms), `MRIS::ct` (`COLOR_TABLE *`) and `MRIS::vg` (`VOL_GEOM`), and
//! `MRIS::mri_sras2vox` (`MRI *`) (fs.mri, queue item 3.1-55). Nothing in queue items 3.1-45 and
//! 3.1-46 reads them; the reading and writing of surfaces (3.1-47) adds them.

use larmorx_freesurfer_core::dmatrix::DMatrix;
use larmorx_freesurfer_core::matrix::Matrix;

use crate::aaa::{
    AnglesPerTriangle, FaceNormCacheEntry, FaceNormDeferredEntry, MAX_CMDS, MriCorner, MriEdge,
    MrisAreaLabel, MrisStatus, Opaque, Strip, VerticesPerFace,
};

/// `FACE` (`face_type_`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Face {
    /// `v`: the vertex numbers, counter-clockwise seen from outside.
    pub v: VerticesPerFace,
    pub area: f32,
    pub angle: AnglesPerTriangle,
    pub orig_angle: AnglesPerTriangle,
    pub ripflag: i8,
    pub oripflag: i8,
    pub marked: i32,
    /// `norm` (`DMATRIX *`).
    pub norm: Option<DMatrix>,
    /// `gradNorm` (three `DMATRIX *`).
    #[allow(non_snake_case)]
    pub gradNorm: [Option<DMatrix>; 3],
}

/// `VERTEX_TOPOLOGY`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VertexTopology {
    /// `f`: `num` face numbers around the vertex.
    pub f: Option<Vec<i32>>,
    /// `n`: for each of those faces, the vertex's position in its `v` (`uchar`).
    pub n: Option<Vec<u8>>,
    /// `e`: the edge state of the neighbouring vertices.
    pub e: Option<Vec<i32>>,
    /// `v`: `vtotal` (or more) neighbouring vertex numbers, sorted by hops.
    pub v: Option<Vec<i32>>,
    /// The 1-hop, 1- or 2-hop, 1- to 3-hop neighbour counts.
    pub vnum: i16,
    pub v2num: i16,
    pub v3num: i16,
    /// The neighbours in use (a copy of the `v#num` of `nsizeCur`).
    pub vtotal: i16,
    /// `mris->nsizeMaxClock` when the `v#num` were filled.
    #[allow(non_snake_case)]
    pub nsizeMaxClock: i16,
    #[allow(non_snake_case)]
    pub nsizeMax: u8,
    #[allow(non_snake_case)]
    pub nsizeCur: u8,
    /// The number of faces around the vertex.
    pub num: u8,
}

/// `VERTEX` (`vertex_type_`).
#[allow(non_snake_case)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vertex {
    /// Distances to the neighbours in `xyz` (`vtotal` used, `dist_capacity` allocated).
    pub dist: Option<Vec<f32>>,
    /// Distances to the neighbours in `origxyz`.
    pub dist_orig: Option<Vec<f32>>,
    pub dist_capacity: i32,
    pub dist_orig_capacity: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub origx: f32,
    pub origy: f32,
    pub origz: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub pnx: f32,
    pub pny: f32,
    pub pnz: f32,
    pub wnx: f32,
    pub wny: f32,
    pub wnz: f32,
    pub onx: f32,
    pub ony: f32,
    pub onz: f32,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub odx: f32,
    pub ody: f32,
    pub odz: f32,
    pub tdx: f32,
    pub tdy: f32,
    pub tdz: f32,
    pub curv: f32,
    pub curvbak: f32,
    pub val: f32,
    pub imag_val: f32,
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub tx: f32,
    pub ty: f32,
    pub tz: f32,
    pub t2x: f32,
    pub t2y: f32,
    pub t2z: f32,
    pub targx: f32,
    pub targy: f32,
    pub targz: f32,
    pub pialx: f32,
    pub pialy: f32,
    pub pialz: f32,
    pub whitex: f32,
    pub whitey: f32,
    pub whitez: f32,
    pub l4x: f32,
    pub l4y: f32,
    pub l4z: f32,
    pub infx: f32,
    pub infy: f32,
    pub infz: f32,
    pub fx: f32,
    pub fy: f32,
    pub fz: f32,
    pub px: i32,
    pub qx: i32,
    pub py: i32,
    pub qy: i32,
    pub pz: i32,
    pub qz: i32,
    pub e1x: f32,
    pub e1y: f32,
    pub e1z: f32,
    pub e2x: f32,
    pub e2y: f32,
    pub e2z: f32,
    pub pe1x: f32,
    pub pe1y: f32,
    pub pe1z: f32,
    pub pe2x: f32,
    pub pe2y: f32,
    pub pe2z: f32,
    pub nc: f32,
    pub val2: f32,
    pub valbak: f32,
    pub val2bak: f32,
    pub stat: f32,
    pub undefval: i32,
    pub old_undefval: i32,
    pub fixedval: i32,
    pub fieldsign: f32,
    pub fsmask: f32,
    pub d: f32,
    pub annotation: i32,
    pub oripflag: i8,
    pub origripflag: i8,
    /// `vp`: the user's information.
    pub vp: Opaque,
    pub theta: f32,
    pub phi: f32,
    pub area: f32,
    pub origarea: f32,
    pub group_avg_area: f32,
    pub K: f32,
    pub H: f32,
    pub k1: f32,
    pub k2: f32,
    pub mean: f32,
    pub mean_imag: f32,
    pub std_error: f32,
    pub flags: u32,
    pub fno: i32,
    pub cropped: i32,
    pub marked: i16,
    pub marked2: i16,
    pub marked3: i16,
    pub neg: i8,
    pub border: i8,
    pub ripflag: i8,
}

/// `MRIS` (`MRI_SURFACE`), with the fields that wait for other items left out (the module's
/// documentation).
#[allow(non_snake_case)]
#[derive(Clone, Debug, PartialEq)]
pub struct Mris {
    #[allow(non_snake_case)]
    pub nverticesFrozen: i32,
    pub nvertices: i32,
    pub nfaces: i32,
    pub faceAttachmentDeferred: bool,
    pub nedges: i32,
    pub ncorners: i32,
    pub nstrips: i32,
    pub vertices_topology: Option<Vec<VertexTopology>>,
    pub vertices: Option<Vec<Vertex>>,
    /// `dist_storage`, `dist_orig_storage`: the vertices' distance buffers kept while their
    /// `dist` fields are emptied (`void **`).
    pub dist_storage: Option<Vec<Option<Vec<f32>>>>,
    pub dist_orig_storage: Option<Vec<Option<Vec<f32>>>>,
    pub tempsAssigned: i32,
    pub faces: Option<Vec<Face>>,
    pub edges: Option<Vec<MriEdge>>,
    pub corners: Option<Vec<MriCorner>>,
    pub faceNormCacheEntries: Option<Vec<FaceNormCacheEntry>>,
    pub faceNormDeferredEntries: Option<Vec<FaceNormDeferredEntry>>,
    pub strips: Option<Vec<Strip>>,
    pub xctr: f32,
    pub yctr: f32,
    pub zctr: f32,
    pub xlo: f32,
    pub ylo: f32,
    pub zlo: f32,
    pub xhi: f32,
    pub yhi: f32,
    pub zhi: f32,
    /// The centre of spherical expansion.
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    /// `v_temporal_pole`, `v_frontal_pole`, `v_occipital_pole` (`VERTEX *` into `vertices`): the
    /// vertex number, `None` for `NULL`.
    pub v_temporal_pole: Option<usize>,
    pub v_frontal_pole: Option<usize>,
    pub v_occipital_pole: Option<usize>,
    pub max_curv: f32,
    pub min_curv: f32,
    pub total_area: f32,
    pub avg_vertex_area: f64,
    pub avg_vertex_dist: f64,
    pub std_vertex_dist: f64,
    pub orig_area: f32,
    pub neg_area: f32,
    pub neg_orig_area: f32,
    pub zeros: i32,
    pub hemisphere: i32,
    pub initialized: i32,
    pub SRASToTalSRAS_: Option<Matrix>,
    pub TalSRASToSRAS_: Option<Matrix>,
    pub free_transform: i32,
    pub radius: f64,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    /// `fname[STRLEN]`: the file the surface was read from.
    pub fname: String,
    pub Hmin: f32,
    pub Hmax: f32,
    pub Kmin: f32,
    pub Kmax: f32,
    pub Ktotal: f64,
    pub status: MrisStatus,
    pub origxyz_status: MrisStatus,
    pub patch: i32,
    pub nlabels: i32,
    pub labels: Option<Vec<MrisAreaLabel>>,
    pub nsize: i8,
    pub vtotalsMightBeTooBig: u8,
    pub nsizeMaxClock: i16,
    pub max_nsize: i8,
    pub dist_nsize: i8,
    pub dist_orig_nsize: i8,
    pub dist_alloced_flags: i8,
    pub avg_nbrs: f32,
    pub vp: Opaque,
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
    pub da: f32,
    pub db: f32,
    pub dg: f32,
    pub r#type: i32,
    pub max_vertices: i32,
    pub max_faces: i32,
    /// `subject_name[STRLEN]`.
    pub subject_name: String,
    pub canon_area: f32,
    pub noscale: i32,
    pub dx2: Option<Vec<f32>>,
    pub dy2: Option<Vec<f32>>,
    pub dz2: Option<Vec<f32>>,
    pub orig_xyzspace: i32,
    pub useRealRAS: i32,
    /// `cmdlines[MAX_CMDS]` (`char *` each).
    pub cmdlines: Vec<Option<String>>,
    pub ncmds: i32,
    pub group_avg_surface_area: f32,
    pub group_avg_vtxarea_loaded: i32,
    pub triangle_links_removed: i32,
    pub user_parms: Opaque,
    pub m_sras2vox: Option<Matrix>,
    pub mht: Opaque,
    pub temps: Opaque,
}

impl Default for Mris {
    /// Every field zero, every pointer `NULL` (FreeSurfer's surfaces start from `calloc`).
    fn default() -> Self {
        Mris {
            nverticesFrozen: 0,
            nvertices: 0,
            nfaces: 0,
            faceAttachmentDeferred: false,
            nedges: 0,
            ncorners: 0,
            nstrips: 0,
            vertices_topology: None,
            vertices: None,
            dist_storage: None,
            dist_orig_storage: None,
            tempsAssigned: 0,
            faces: None,
            edges: None,
            corners: None,
            faceNormCacheEntries: None,
            faceNormDeferredEntries: None,
            strips: None,
            xctr: 0.0,
            yctr: 0.0,
            zctr: 0.0,
            xlo: 0.0,
            ylo: 0.0,
            zlo: 0.0,
            xhi: 0.0,
            yhi: 0.0,
            zhi: 0.0,
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            v_temporal_pole: None,
            v_frontal_pole: None,
            v_occipital_pole: None,
            max_curv: 0.0,
            min_curv: 0.0,
            total_area: 0.0,
            avg_vertex_area: 0.0,
            avg_vertex_dist: 0.0,
            std_vertex_dist: 0.0,
            orig_area: 0.0,
            neg_area: 0.0,
            neg_orig_area: 0.0,
            zeros: 0,
            hemisphere: 0,
            initialized: 0,
            SRASToTalSRAS_: None,
            TalSRASToSRAS_: None,
            free_transform: 0,
            radius: 0.0,
            a: 0.0,
            b: 0.0,
            c: 0.0,
            fname: String::new(),
            Hmin: 0.0,
            Hmax: 0.0,
            Kmin: 0.0,
            Kmax: 0.0,
            Ktotal: 0.0,
            status: MrisStatus::SURFACE,
            origxyz_status: MrisStatus::SURFACE,
            patch: 0,
            nlabels: 0,
            labels: None,
            nsize: 0,
            vtotalsMightBeTooBig: 0,
            nsizeMaxClock: 0,
            max_nsize: 0,
            dist_nsize: 0,
            dist_orig_nsize: 0,
            dist_alloced_flags: 0,
            avg_nbrs: 0.0,
            vp: Opaque::default(),
            alpha: 0.0,
            beta: 0.0,
            gamma: 0.0,
            da: 0.0,
            db: 0.0,
            dg: 0.0,
            r#type: 0,
            max_vertices: 0,
            max_faces: 0,
            subject_name: String::new(),
            canon_area: 0.0,
            noscale: 0,
            dx2: None,
            dy2: None,
            dz2: None,
            orig_xyzspace: 0,
            useRealRAS: 0,
            cmdlines: vec![None; MAX_CMDS],
            ncmds: 0,
            group_avg_surface_area: 0.0,
            group_avg_vtxarea_loaded: 0,
            triangle_links_removed: 0,
            user_parms: Opaque::default(),
            m_sras2vox: None,
            mht: Opaque::default(),
            temps: Opaque::default(),
        }
    }
}

impl Mris {
    /// `mris->vertices[vno]` (a missing array or vertex is FreeSurfer's invalid read: a panic).
    pub fn vertex(&self, vno: usize) -> &Vertex {
        &self.vertices.as_ref().expect("MRIS::vertices is NULL")[vno]
    }

    pub fn vertex_mut(&mut self, vno: usize) -> &mut Vertex {
        &mut self.vertices.as_mut().expect("MRIS::vertices is NULL")[vno]
    }

    /// `mris->vertices_topology[vno]`.
    pub fn topology(&self, vno: usize) -> &VertexTopology {
        &self
            .vertices_topology
            .as_ref()
            .expect("MRIS::vertices_topology is NULL")[vno]
    }

    pub fn topology_mut(&mut self, vno: usize) -> &mut VertexTopology {
        &mut self
            .vertices_topology
            .as_mut()
            .expect("MRIS::vertices_topology is NULL")[vno]
    }

    /// `mris->faces[fno]`.
    pub fn face(&self, fno: usize) -> &Face {
        &self.faces.as_ref().expect("MRIS::faces is NULL")[fno]
    }

    pub fn face_mut(&mut self, fno: usize) -> &mut Face {
        &mut self.faces.as_mut().expect("MRIS::faces is NULL")[fno]
    }
}
