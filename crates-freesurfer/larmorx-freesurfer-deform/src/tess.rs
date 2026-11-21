// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The tessellation of a volume (`utils/mrisurf_timeStep.cpp`: `MRIStessellate` 1321-1932 and its
//! helpers `TESSaddFace` 1934, `TESScheckFace` 1966, `TESSaddVertex` 2013 and `TESSfacep` 2030;
//! fs.deform, queue item 3.4-06, taken early by relay 48 of track D for `mri_binarize --surf`), and
//! `utils/mri2.cpp`'s `MRIvolTopoFix` (5752-5871), which tessellates the volume it fixes
//! (`mri_binarize --fix-vol-topo`; fs.volume's code, here because it needs the tessellation).
//!
//! The tessellation tiles the outside of the voxels of one value: every lattice corner where a
//! boundary face meets is a vertex (in slice, row, column order), every boundary face of a voxel a
//! quadrangle split into two triangles (`WHICH_FACE_SPLIT`), the vertices placed at the corners'
//! tkregister coordinates (`surfaceRASFromVoxel_`). FreeSurfer's lists hold at most
//! `MAXVERTICES` (10,000,000) vertices and `MAXFACES` (20,000,000) quadrangles; the end of either
//! ends the program (`<program>: max faces 20000000 exceeded`, `ERROR_NOMEMORY`). Its face tables
//! (`face_index_table0` and `1`, kept from one slice to the next) are reproduced as they are,
//! stale entries included; the port allocates the six planes of them FreeSurfer uses (it
//! allocates 600) and no vertex table (FreeSurfer writes one it never reads), and its lists grow
//! with what they hold (FreeSurfer's are allocated at their limits).
//!
//! FS-1021 and FS-1022 are `MRIvolTopoFix`'s defects as `mri_binarize` meets them.

use std::io::Write;

use larmorx_core::defects::{FS_1022, FS_1027};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NOMEMORY, Fatal};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{Mri, mri_copy_pulse_parameters};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::{SurfGlobals, VERTICES_PER_FACE};
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::coords::{self, Io};
use larmorx_freesurfer_surf::formats::MRIS_TRIANGULAR_SURFACE;
use larmorx_freesurfer_surf::mp::compute_metric_properties;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::nbhd::complete_topology;
use larmorx_freesurfer_surf::toprest::compute_euler_number;
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::coords::surface_ras_from_voxel;
use larmorx_freesurfer_volume::mriset::{
    NEAREST_NEIGHBOR_FACE, mri_dilate6, mri_erode_nn, mri_set_edges,
};
use larmorx_freesurfer_volume::topo::mri_vol_topo_erode_one;
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

/// `MAXVERTICES`.
pub const MAXVERTICES: i32 = 10_000_000;
/// `MAXFACES`.
pub const MAXFACES: i32 = 2 * MAXVERTICES;

/// `TESSfacep`'s twelve tests at a corner (slice, row, column offsets of the two voxels), in the
/// order of `MRIStessellate`'s condition.
const FACEP: [[i32; 6]; 12] = [
    [0, -1, -1, -1, -1, -1],
    [0, -1, 0, -1, -1, 0],
    [0, 0, 0, -1, 0, 0],
    [0, 0, -1, -1, 0, -1],
    [-1, 0, -1, -1, -1, -1],
    [-1, 0, 0, -1, -1, 0],
    [0, 0, 0, 0, -1, 0],
    [0, 0, -1, 0, -1, -1],
    [-1, -1, 0, -1, -1, -1],
    [-1, 0, 0, -1, 0, -1],
    [0, 0, 0, 0, 0, -1],
    [0, -1, 0, 0, -1, -1],
];

/// `TESScheckFace`'s 24 calls at a corner, in order: the two voxels' offsets, the face direction
/// `f`, the corner's place `n` in the quadrangle and `prev_flag`.
const CHECK: [[i32; 9]; 24] = [
    [0, -1, -1, -1, -1, -1, 0, 2, 0],
    [0, -1, 0, -1, -1, 0, 0, 3, 0],
    [0, 0, 0, -1, 0, 0, 0, 0, 0],
    [0, 0, -1, -1, 0, -1, 0, 1, 0],
    [-1, 0, -1, -1, -1, -1, 2, 2, 1],
    [-1, 0, 0, -1, -1, 0, 2, 1, 1],
    [0, 0, 0, 0, -1, 0, 2, 0, 0],
    [0, 0, -1, 0, -1, -1, 2, 3, 0],
    [-1, -1, 0, -1, -1, -1, 4, 2, 1],
    [-1, 0, 0, -1, 0, -1, 4, 3, 1],
    [0, 0, 0, 0, 0, -1, 4, 0, 0],
    [0, -1, 0, 0, -1, -1, 4, 1, 0],
    [-1, -1, -1, 0, -1, -1, 1, 2, 1],
    [-1, -1, 0, 0, -1, 0, 1, 1, 1],
    [-1, 0, 0, 0, 0, 0, 1, 0, 1],
    [-1, 0, -1, 0, 0, -1, 1, 3, 1],
    [-1, -1, -1, -1, 0, -1, 3, 2, 1],
    [-1, -1, 0, -1, 0, 0, 3, 3, 1],
    [0, -1, 0, 0, 0, 0, 3, 0, 0],
    [0, -1, -1, 0, 0, -1, 3, 1, 0],
    [-1, -1, -1, -1, -1, 0, 5, 2, 1],
    [-1, 0, -1, -1, 0, 0, 5, 1, 1],
    [0, 0, -1, 0, 0, 0, 5, 0, 0],
    [0, -1, -1, 0, -1, 0, 5, 3, 0],
];

/// The file `MRIStessellate`'s `MRISsetXYZ` names.
fn time_step_cpp() -> String {
    format!(
        "{}/utils/mrisurf_timeStep.cpp",
        larmorx_freesurfer_core::diag::BUILD_ROOT
    )
}

/// A corner of the lattice (`tvertex_type`'s slice, row and column).
#[derive(Clone, Copy)]
struct TVertex {
    imnr: i32,
    i: i32,
    j: i32,
}

/// The tessellation's state: the volume, the value tiled, its lists and face tables.
struct Tess<'a> {
    mri: &'a Mri,
    value: i32,
    all_flag: bool,
    /// The quadrangles' four corners (`tface_type`'s `v`; zero until set, as `lcalloc` leaves them).
    faces: Vec<[i32; 4]>,
    vertices: Vec<TVertex>,
    table0: Vec<i32>,
    table1: Vec<i32>,
}

impl Tess<'_> {
    /// Frame 0 of voxel (`j`, `i`, `im`) (column, row, slice).
    fn val(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        j: i32,
        i: i32,
        im: i32,
    ) -> Result<f32, Fatal> {
        mri_get_vox_val(g, err, self.mri, j, i, im, 0, 0)
    }

    /// Whether both voxels are inside the volume.
    fn inside(&self, im0: i32, i0: i32, j0: i32, im1: i32, i1: i32, j1: i32) -> bool {
        let (w, h, d) = (self.mri.vg.width, self.mri.vg.height, self.mri.vg.depth);
        (0..d).contains(&im0)
            && (0..h).contains(&i0)
            && (0..w).contains(&j0)
            && (0..d).contains(&im1)
            && (0..h).contains(&i1)
            && (0..w).contains(&j1)
    }

    /// `TESSfacep(mri, im0, i0, j0, im1, i1, j1, value, all_flag)`: whether the face between the
    /// two voxels is a boundary of `value` (or of any change of value with `all_flag`).
    #[allow(clippy::too_many_arguments)]
    fn facep(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        im0: i32,
        i0: i32,
        j0: i32,
        im1: i32,
        i1: i32,
        j1: i32,
    ) -> Result<bool, Fatal> {
        if !self.inside(im0, i0, j0, im1, i1, j1) {
            return Ok(false);
        }
        let value = self.value as f32;
        if self.val(g, err, j0, i0, im0)? == self.val(g, err, j1, i1, im1)? {
            return Ok(false);
        }
        Ok(self.val(g, err, j0, i0, im0)? == value
            || self.val(g, err, j1, i1, im1)? == value
            || self.all_flag)
    }

    /// `TESSaddFace(mri, imnr, i, j, f, prev_flag, ...)`.
    fn add_face(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        i: i32,
        j: i32,
        f: i32,
        prev_flag: bool,
    ) -> Result<(), Fatal> {
        let (xnum, ynum) = (self.mri.vg.width as usize, self.mri.vg.height as usize);
        let pack = f as usize * ynum * xnum + i as usize * xnum + j as usize;
        let index = self.faces.len() as i32;
        if index >= MAXFACES - 1 {
            let m = format!("{}: max faces {MAXFACES} exceeded", g.progname);
            log::error_exit(g, err, ERROR_NOMEMORY, &m)?;
        }
        if prev_flag {
            self.table0[pack] = index;
        } else {
            self.table1[pack] = index;
        }
        self.faces.push([0; 4]);
        Ok(())
    }

    /// `TESScheckFace(mri, im0, i0, j0, im1, i1, j1, f, n, v_ind, prev_flag, all_flag, value,
    /// ...)`: a boundary face gets the corner `v_ind` as its `n`th vertex (made with its first).
    #[allow(clippy::too_many_arguments)]
    fn check_face(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        p: [i32; 6],
        f: i32,
        n: usize,
        v_ind: i32,
        prev_flag: bool,
    ) -> Result<(), Fatal> {
        let [im0, i0, j0, im1, i1, j1] = p;
        if !self.inside(im0, i0, j0, im1, i1, j1) {
            return Ok(());
        }
        let value = self.value as f32;
        let boundary = (self.all_flag
            && self.val(g, err, j0, i0, im0)? != self.val(g, err, j1, i1, im1)?)
            || (self.val(g, err, j0, i0, im0)? == value && self.val(g, err, j1, i1, im1)? != value);
        if !boundary {
            return Ok(());
        }
        if n == 0 {
            self.add_face(g, err, i0, j0, f, prev_flag)?;
        }
        let (xnum, ynum) = (self.mri.vg.width as usize, self.mri.vg.height as usize);
        let pack = f as usize * ynum * xnum + i0 as usize * xnum + j0 as usize;
        let f_ind = if prev_flag {
            self.table0[pack]
        } else {
            self.table1[pack]
        };
        // a table entry names a face of the list (FreeSurfer's list is allocated to MAXFACES,
        // whose entries past the faces made are zero: never named, the tables start at 0)
        self.faces[f_ind as usize][n] = v_ind;
        Ok(())
    }

    /// `TESSaddVertex(mri, imnr, i, j, ...)`: the corner's vertex number.
    fn add_vertex(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        imnr: i32,
        i: i32,
        j: i32,
    ) -> Result<i32, Fatal> {
        let index = self.vertices.len() as i32;
        if index >= MAXVERTICES - 1 {
            let m = format!("{}: max vertices {MAXVERTICES} exceeded", g.progname);
            log::error_exit(g, err, ERROR_NOMEMORY, &m)?;
        }
        self.vertices.push(TVertex { imnr, i, j });
        Ok(index)
    }
}

/// `WHICH_FACE_SPLIT(vno0, vno1)`: `nint(sqrt(1.9 vno0) + sqrt(3.5 vno1))`.
fn which_face_split(g: &mut Globals, vno0: i32, vno1: i32) -> i32 {
    nint(
        g,
        (1.9 * f64::from(vno0)).sqrt() + (3.5 * f64::from(vno1)).sqrt(),
    )
}

/// `MRIStessellate(mri, value, all_flag)`: the surface tiling the outside of the voxels of
/// `value` (frame 0; with `all_flag`, of every change of value): a triangular surface of the
/// corners and split quadrangles, its vertex coordinates the corners' (`vox - 0.5`) through
/// `surfaceRASFromVoxel_`, its face lists and their places (each vertex's faces in face order),
/// the volume's geometry, then `mrisCompleteTopology` and `MRIScomputeMetricProperties`. With
/// `Gdiag_no` above 0, `slice <imnr>: <nv> vertices, <nf> faces` every 10 slices once any exist,
/// and `MRIStessellate: using vox2ras matrix:` with the matrix.
pub fn mris_tessellate(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mri: &mut Mri,
    value: i32,
    all_flag: bool,
) -> Result<Mris, Fatal> {
    let (xnum, ynum, numimg) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let plane = (xnum.max(0) as usize) * (ynum.max(0) as usize);
    let mut t = Tess {
        mri,
        value,
        all_flag,
        faces: Vec::new(),
        vertices: Vec::new(),
        table0: vec![0; 6 * plane],
        table1: vec![0; 6 * plane],
    };
    for imnr in 0..=numimg {
        if g.gdiag_no > 0 && (!t.vertices.is_empty() || !t.faces.is_empty()) && imnr % 10 == 0 {
            let _ = writeln!(
                io.out,
                "slice {imnr}: {} vertices, {} faces",
                t.vertices.len(),
                t.faces.len()
            );
        }
        for i in 0..=ynum {
            for j in 0..=xnum {
                let mut any = false;
                for p in &FACEP {
                    if t.facep(
                        g,
                        io.err,
                        imnr + p[0],
                        i + p[1],
                        j + p[2],
                        imnr + p[3],
                        i + p[4],
                        j + p[5],
                    )? {
                        any = true;
                        break;
                    }
                }
                if !any {
                    continue;
                }
                let v_ind = t.add_vertex(g, io.err, imnr, i, j)?;
                for c in &CHECK {
                    let p = [
                        imnr + c[0],
                        i + c[1],
                        j + c[2],
                        imnr + c[3],
                        i + c[4],
                        j + c[5],
                    ];
                    t.check_face(g, io.err, p, c[6], c[7] as usize, v_ind, c[8] != 0)?;
                }
            }
        }
        t.table0.copy_from_slice(&t.table1);
    }
    let (faces, vertices) = (
        std::mem::take(&mut t.faces),
        std::mem::take(&mut t.vertices),
    );
    let nv = vertices.len() as i32;
    let nquads = faces.len() as i32;
    let mut surf = alloc::alloc(g, io.out, io.err, sg, nv, 2 * nquads)?;
    surf.r#type = MRIS_TRIANGULAR_SURFACE;
    let m = surface_ras_from_voxel(g, io.out, io.err, &mut mri.vg)?;
    if g.gdiag_no > 0 {
        let _ = writeln!(io.out, "MRIStessellate: using vox2ras matrix:");
        let mut so = Vec::new();
        matrix_print(g, Some(&mut so), io.out, io.err, Some(&m), 5);
        let _ = io.out.write_all(&so);
    }
    let mut vv = Matrix::real(4, 1);
    vv.set_relt(4, 1, 1.0);
    let mut vw = Matrix::real(4, 1);
    let file = time_step_cpp();
    for (k, v) in vertices.iter().enumerate() {
        vv.set_relt(1, 1, (f64::from(v.j) - 0.5) as f32);
        vv.set_relt(2, 1, (f64::from(v.i) - 0.5) as f32);
        vv.set_relt(3, 1, (f64::from(v.imnr) - 0.5) as f32);
        matrix_multiply(g, io.out, io.err, Some(&m), Some(&vv), Some(&mut vw))?;
        let (x, y, z) = (vw.relt(1, 1), vw.relt(2, 1), vw.relt(3, 1));
        coords::set_xyz(g, io, &mut surf, k as i32, x, y, z, &file, 1861)?;
    }
    for (k, quad) in faces.iter().enumerate() {
        let fno = 2 * k;
        let which = which_face_split(g, quad[0], quad[1]);
        let (a, b) = if which / 2 * 2 == which {
            ([quad[0], quad[1], quad[3]], [quad[2], quad[3], quad[1]])
        } else {
            ([quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]])
        };
        surf.face_mut(fno).v = a;
        surf.face_mut(fno + 1).v = b;
        for n in 0..VERTICES_PER_FACE {
            for f in [fno, fno + 1] {
                let v = surf.face(f).v[n] as usize;
                let tp = surf.topology_mut(v);
                tp.num = tp.num.wrapping_add(1);
            }
        }
    }
    for k in 0..vertices.len() {
        let tp = surf.topology_mut(k);
        let num = usize::from(tp.num);
        tp.f = Some(vec![0; num]);
        tp.n = Some(vec![0; num]);
        tp.num = 0;
    }
    for fno in 0..2 * faces.len() {
        for n in 0..VERTICES_PER_FACE {
            let v = surf.face(fno).v[n] as usize;
            let tp = surf.topology_mut(v);
            let at = usize::from(tp.num);
            if let Some(f) = tp.f.as_mut().and_then(|f| f.get_mut(at)) {
                *f = fno as i32;
            }
            tp.num = tp.num.wrapping_add(1);
        }
    }
    for k in 0..vertices.len() {
        let num = usize::from(surf.topology(k).num);
        for n in 0..num {
            let fno = surf.topology(k).f.as_ref().map_or(0, |f| f[n]) as usize;
            let fv = surf.face(fno).v;
            for (i, &x) in fv.iter().enumerate() {
                if x == k as i32
                    && let Some(p) = surf.topology_mut(k).n.as_mut()
                {
                    p[n] = i as u8;
                }
            }
        }
    }
    // mrisCheckVertexFaceTopology: always true in FreeSurfer 8.2.0
    get_vol_geom(&mri.vg, &mut surf.vg);
    complete_topology(g, sg, io, &mut surf)?;
    compute_metric_properties(g, sg, io, &mut surf)?;
    Ok(surf)
}

/// What `MRIvolTopoFix` returns.
pub enum TopoFix {
    /// Its input itself: the segmentation is topologically correct already.
    Same,
    /// The fixed segmentation.
    New(Box<Mri>),
    /// `NULL`: more dilations than allowed.
    Null,
}

/// The tessellations after which FreeSurfer's run is noted with FS-1027: each keeps about
/// 1.25 GB of address space, so `--fix-vol-topo` ends where the address space or the memory
/// limit runs out (observed: after 55 dilations under the oracle-output cache's limit, 865 in an
/// unlimited 4 GiB box); larmorx releases its lists.
pub const FS_1027_TESSELLATIONS: u32 = 32;

/// The Euler number of `vol`'s tessellation of `onval` (`count`: the tessellations of the run so
/// far, FS-1027).
fn euler(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    vol: &mut Mri,
    onval: i32,
    count: &mut u32,
) -> Result<i32, Fatal> {
    *count += 1;
    if *count == FS_1027_TESSELLATIONS {
        g.notices.add(
            &FS_1027,
            format!("MRIvolTopoFix: {FS_1027_TESSELLATIONS} tessellations or more (FreeSurfer keeps about 1.25 GB of address space for each, and runs out)"),
        );
    }
    let surf = mris_tessellate(g, sg, io, vol, onval, false)?;
    Ok(compute_euler_number(&surf).euler)
}

/// `MRIvolTopoFix(binseg0, onval, NULL, ndilationsmax, nerodesmax, ntopoerodesmax)`: the binary
/// segmentation (on value `onval`) with its holes filled so that its tessellation has Euler number
/// 2, keeping every voxel it had. Already 2: the input itself ([`TopoFix::Same`]). Otherwise a copy
/// (pulse parameters and colour table with it); `Found <n> voxels in binseg` (the voxels above
/// 0.5, with `WARNING: binseg has <n> edge voxels. You may want to add some padding if results are
/// not good` for those on the volume's edge); dilated (6 neighbours, the edges set to 0) until the
/// Euler number is 2 (`dilate n=<n> eno=<e>` each time; `  no defects found, breaking from
/// dilation`, or after `ndilationsmax` `  ERROR: number of dilations > ndilationsmax=<n>` and
/// [`TopoFix::Null`]); eroded (face neighbours, the original voxels kept) until the Euler number
/// changes or after `nerodesmax` (`erode n=<n> eno=<e>`; `  defects found, breaking from erode, and
/// reverting to previous` or `  WARNING: number of erodes > nerodesmax=<n>`), the last good one
/// kept; then eroded a voxel layer at a time where that keeps the Euler characteristic
/// (`MRIvolTopoErodeOne`, `n=<n> nhits = <h> eno=<e>`) until none is (or `  WARNING: number of
/// topoerodes > max=<n>`). FS-1022: FreeSurfer's erosion keeps every original voxel but the first
/// (in column, row, slice order); larmorx keeps it too, `reproduce` leaves it unprotected. FS-1027:
/// noted once the run has tessellated [`FS_1027_TESSELLATIONS`] times.
#[allow(clippy::too_many_arguments)]
pub fn mri_vol_topo_fix(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    binseg0: &mut Mri,
    onval: i32,
    ndilationsmax: i32,
    nerodesmax: i32,
    ntopoerodesmax: i32,
) -> Result<TopoFix, Fatal> {
    let mut count = 0u32;
    let mut eno = euler(g, sg, io, binseg0, onval, &mut count)?;
    if eno == 2 {
        return Ok(TopoFix::Same);
    }
    let mut binseg = mri_copy(g, io.out, io.err, binseg0, None)?.unwrap_new();
    mri_copy_pulse_parameters(binseg0, &mut binseg);
    if binseg0.ct.is_some() {
        binseg.ct = ctab_deep_copy(g, io.err, binseg0.ct.as_ref());
    }
    let mut crs: Vec<[i32; 3]> = Vec::new();
    let mut hitedge = 0;
    let (w, h, d) = (binseg.vg.width, binseg.vg.height, binseg.vg.depth);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let val = f64::from(mri_get_vox_val(g, io.err, &binseg, c, r, s, 0, 0)?);
                if val < 0.5 {
                    continue;
                }
                crs.push([c, r, s]);
                if c == 0 || c == w - 1 || r == 0 || r == h - 1 || s == 0 || s == d - 1 {
                    hitedge += 1;
                }
            }
        }
    }
    let _ = writeln!(io.out, "Found {} voxels in binseg", crs.len() as i32);
    if hitedge != 0 {
        let _ = writeln!(
            io.out,
            "WARNING: binseg has {hitedge} edge voxels. You may want to add some padding if results are not good"
        );
    }
    let mut mrisrc = mri_copy(g, io.out, io.err, &binseg, None)?.unwrap_new();
    mri_copy_pulse_parameters(&binseg, &mut mrisrc);

    let _ = write!(io.out, "\nDilating ============================\n");
    let mut n = 0;
    loop {
        n += 1;
        let _ = writeln!(io.out, "dilate n={n} eno={eno}");
        let mut mritmp = mri_dilate6(g, io.out, io.err, Pair::New(&binseg))?.unwrap_new();
        mri_set_edges(g, io.out, io.err, Pair::Same(&mut mritmp), 0.0)?;
        binseg = mritmp;
        eno = euler(g, sg, io, &mut binseg, onval, &mut count)?;
        if eno == 2 || n > ndilationsmax {
            if eno == 2 {
                let _ = writeln!(io.out, "  no defects found, breaking from dilation");
            } else {
                let _ = writeln!(
                    io.out,
                    "  ERROR: number of dilations > ndilationsmax={ndilationsmax}"
                );
                return Ok(TopoFix::Null);
            }
            break;
        }
    }

    // FS-1022: FreeSurfer protects the original voxels but the first from the erosion
    let first_kept = usize::from(g.notices.reproduce());
    let _ = write!(io.out, "\nEroding ============================\n");
    n = 0;
    loop {
        n += 1;
        let _ = writeln!(io.out, "erode n={n} eno={eno}");
        let mricopy = mri_copy(g, io.out, io.err, &binseg, None)?.unwrap_new();
        let mut mritmp = mri_erode_nn(
            g,
            io.out,
            io.err,
            Pair::New(&binseg),
            NEAREST_NEIGHBOR_FACE,
            1,
        )?
        .unwrap_new();
        if let Some(p) = crs.first() {
            let v0 = mri_get_vox_val(g, io.err, &mritmp, p[0], p[1], p[2], 0, 0)?;
            if v0 != onval as f32 {
                g.notices.add(
                    &FS_1022,
                    "MRIvolTopoFix: the erosion keeps the segmentation's first voxel too (FreeSurfer leaves it unprotected)",
                );
            }
        }
        for p in crs.iter().skip(first_kept) {
            mri_set_vox_val(g, io.err, &mut mritmp, p[0], p[1], p[2], 0, onval as f32, 0)?;
        }
        eno = euler(g, sg, io, &mut mritmp, onval, &mut count)?;
        if eno != 2 || n > nerodesmax {
            if eno != 2 {
                let _ = writeln!(
                    io.out,
                    "  defects found, breaking from erode, and reverting to previous"
                );
            } else {
                let _ = writeln!(
                    io.out,
                    "  WARNING: number of erodes > nerodesmax={nerodesmax}"
                );
            }
            binseg = mricopy;
            break;
        }
        binseg = mritmp;
    }

    let _ = write!(io.out, "\nTopoEroding ============================\n");
    n = 0;
    loop {
        n += 1;
        let nhits = mri_vol_topo_erode_one(g, io.out, io.err, &mut binseg, Some(&mrisrc))?;
        eno = euler(g, sg, io, &mut binseg, onval, &mut count)?;
        let _ = writeln!(io.out, "n={n} nhits = {nhits} eno={eno}");
        if nhits == 0 || n > ntopoerodesmax {
            if n > ntopoerodesmax {
                let _ = writeln!(
                    io.out,
                    "  WARNING: number of topoerodes > max={ntopoerodesmax}"
                );
            }
            break;
        }
    }
    mri_copy_pulse_parameters(&mrisrc, &mut binseg);
    Ok(TopoFix::New(Box::new(binseg)))
}
