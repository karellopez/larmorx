// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer 8.2.0's `MRIStransform` (`utils/mrisurf_metricProperties.cpp` 2166; track A's relay
//! 45 of FreeSurfer's foundations): a surface moved through a `TRANSFORM`, a GCA morph or a
//! linear transform array of one transform. It lives beside [`crate::transform`] (not in fs.surf,
//! which this crate builds on) because it needs the `TRANSFORM` type; its callers are
//! `mri_normalize -surface <surface> <transform>` (a transform other than `identity.nofile`) and
//! `mris_sphere`'s `-xform` with its volume.
//!
//! A morph: each vertex (tkregister RAS of the source volume, the morph's atlas when none is given)
//! to the source's voxels (FreeSurfer's float `MatrixMultiply`), sampled through the morph
//! (`GCAMsampleMorph`), then the destination's voxels (the morph's image when none is given) to
//! tkregister RAS (`MRIvoxelToSurfaceRAS`); a morph in RAS coordinates is first taken to the
//! destination's voxels (`GCAMrasToVox`). A linear array: RAS to RAS through `RASFromSurfaceRAS_`
//! and `surfaceRASFromRAS_` (or the matrix itself for a surface in scanner RAS), voxel to voxel
//! through `voxelFromSurfaceRAS_` and `surfaceRASFromVoxel_`; each vertex through
//! `TransformWithMatrix`. Then the surface's extent (`mrisComputeSurfaceDimensions`) and the
//! destination's geometry as the surface's (`getVolGeom`). Ripped vertices stay.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 41): FS-939 (a vertex
//! the morph cannot sample after one it could: the earlier vertex's position), FS-940 (before any:
//! memory contents), FS-941 (the c_(ras) checks compare each value with itself), FS-942 (a linear
//! transform of another type: a null matrix), FS-858 (a linear array given with the morph's type).

use std::io::Write;

use larmorx_core::defects::{FS_858, FS_939, FS_940, FS_941, FS_942};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, MORPH_3D_TYPE};
use larmorx_freesurfer_mri::mri::{
    MRI_UCHAR, Mri, mri_alloc, mri_alloc_header, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::tkreg::ras_from_surface_ras;
use larmorx_freesurfer_mri::vol_geom::{VolGeom, reinit_cache};
use larmorx_freesurfer_surf::coords::compute_surface_dimensions;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_volume::coords::{
    surface_ras_from_ras, surface_ras_from_voxel, voxel_from_surface_ras, voxel_to_surface_ras,
};
use larmorx_freesurfer_volume::talairachex::transform_with_matrix;
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

use crate::gcam::{GCAM_RAS, Gcam, gcam_ras_to_vox};
use crate::transform::{Transform, Xform, gcam_sample_morph};

/// The volumes `MRIStransform(mris, mri, transform, mri_dst)` is given.
pub enum Volumes<'a> {
    /// `mri` and `mri_dst`, each `NULL` when `None` (`mri_normalize`: both `NULL`).
    Separate(Option<&'a mut Mri>, Option<&'a mut Mri>),
    /// One volume as both (`mris_sphere`'s `MRIStransform(mris, mri, &transform, mri)`).
    Same(&'a mut Mri),
}

/// A volume of the call: the caller's, one made here, or none.
enum Slot<'a> {
    Absent,
    Given(&'a mut Mri),
    Own(Box<Mri>),
}

impl Slot<'_> {
    fn get(&mut self) -> Option<&mut Mri> {
        match self {
            Slot::Absent => None,
            Slot::Given(m) => Some(m),
            Slot::Own(m) => Some(m),
        }
    }

    fn is_absent(&self) -> bool {
        matches!(self, Slot::Absent)
    }
}

/// The two volumes, `dst` the source itself when `same`.
struct Vols<'a> {
    src: Slot<'a>,
    dst: Slot<'a>,
    same: bool,
}

impl<'a> Vols<'a> {
    fn new(v: Volumes<'a>) -> Vols<'a> {
        match v {
            Volumes::Separate(a, b) => Vols {
                src: a.map_or(Slot::Absent, Slot::Given),
                dst: b.map_or(Slot::Absent, Slot::Given),
                same: false,
            },
            Volumes::Same(m) => Vols {
                src: Slot::Given(m),
                dst: Slot::Absent,
                same: true,
            },
        }
    }

    fn src(&mut self) -> Option<&mut Mri> {
        self.src.get()
    }

    fn dst(&mut self) -> Option<&mut Mri> {
        if self.same {
            self.src.get()
        } else {
            self.dst.get()
        }
    }

    fn has_src(&self) -> bool {
        !self.src.is_absent()
    }

    fn has_dst(&self) -> bool {
        self.same || !self.dst.is_absent()
    }

    fn src_vg(&mut self) -> &mut VolGeom {
        &mut self.src().expect("MRIStransform's source volume").vg
    }

    fn dst_vg(&mut self) -> &mut VolGeom {
        &mut self.dst().expect("MRIStransform's destination volume").vg
    }
}

/// `WARNING:` and `n` stars, a newline.
fn banner(n: usize) -> String {
    format!("WARNING:{}\n", "*".repeat(n))
}

/// `FZERO(f)`: `fabs(f) < FLT_EPSILON` (false for a NaN).
fn fzero(f: f32) -> bool {
    f64::from(f).abs() < f64::from(f32::EPSILON)
}

/// FreeSurfer's c_(ras) test `!(FZERO(a.c_r - b.c_r) && FZERO(a.c_a - b.c_a) && FZERO(a.c_s -
/// b.c_s))` (float differences, the first false one decides).
fn centres_differ(a: &VolGeom, b: &VolGeom) -> bool {
    !(fzero(sse::sub(a.c_r, b.c_r))
        && fzero(sse::sub(a.c_a, b.c_a))
        && fzero(sse::sub(a.c_s, b.c_s)))
}

fn ptr_mut(r: Ret<Matrix>) -> Option<Matrix> {
    match r {
        Ret::New(m) => Some(m),
        Ret::Given | Ret::Null => None,
    }
}

/// `MRIStransform(mris, mri, transform, mri_dst)`: see the module. Returns `NO_ERROR`; the
/// failures end the program (`ErrorExit`, `ERROR_BADPARM`): a linear array of several transforms
/// (`we cannot handle multiple transforms`) or none (`transform does not have transform ;-) `), no
/// source volume when neither the transform's source geometry nor the surface's is valid (`When
/// mri == NULL, the transform must have the valid src info.`). Messages on standard error: the
/// c_(ras) warnings (FS-941), `INFO:try to get src info from transform.` or `... from the
/// surface.`, `INFO: Get dst info from transform.` (with `Gdiag`'s `DIAG_SHOW` and
/// `DIAG_VERBOSE`), the warning for a transform without a valid destination (the coronal volume
/// of centre 0 assumed), and the voxel-to-voxel warning for volumes of other extents (`%dm` as
/// FreeSurfer prints it).
#[allow(clippy::too_many_arguments)]
pub fn mris_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mris: &mut Mris,
    volumes: Volumes<'_>,
    transform: &mut Transform,
) -> Result<i32, Fatal> {
    let mut vols = Vols::new(volumes);
    let ttype = transform.ttype;
    match (&mut transform.xform, ttype == MORPH_3D_TYPE) {
        (Xform::Morph(gcam), true) => morph(g, out, err, env, mris, &mut vols, gcam)?,
        (Xform::Lta(lta), false) => linear(g, out, err, mris, &mut vols, lta)?,
        (Xform::Lta(_), true) => {
            g.notices.add(
                &FS_858,
                "MRIStransform of a linear transform array given the morph's type 11: FreeSurfer reads the array as a morph (memory contents or a crash)",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRIStransform: a linear transform with the morph's type 11 (FreeSurfer reads it as a morph here)\n",
            )?;
        }
        (Xform::Morph(_), false) => {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!("MRIStransform: a GCA morph in a transform of type {ttype}\n"),
            )?;
        }
    }
    Ok(NO_ERROR)
}

/// The morph's part of [`mris_transform`].
fn morph(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mris: &mut Mris,
    vols: &mut Vols<'_>,
    gcam: &mut Gcam,
) -> Result<(), Fatal> {
    if !vols.has_dst() {
        let (w, h, d) = (gcam.image.width, gcam.image.height, gcam.image.depth);
        let mut m = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?;
        use_vol_geom_to_mri(g, out, err, &gcam.image, &mut m.vg)?;
        vols.dst = Slot::Own(Box::new(m));
    }
    if !vols.has_src() {
        let (w, h, d) = (gcam.atlas.width, gcam.atlas.height, gcam.atlas.depth);
        let mut m = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?;
        use_vol_geom_to_mri(g, out, err, &gcam.atlas, &mut m.vg)?;
        vols.src = Slot::Own(Box::new(m));
    }
    if gcam.mtype == GCAM_RAS {
        let dst = vols.dst().map(|m| &*m);
        gcam_ras_to_vox(g, out, err, env, gcam, dst)?;
    }
    let mut v1 = Matrix::real(4, 1);
    v1.set_relt(4, 1, 1.0);
    let mut v2 = Matrix::real(4, 1);
    v2.set_relt(4, 1, 1.0);
    let to_vox = voxel_from_surface_ras(g, out, err, vols.src_vg())?;
    // GCAMsampleMorph leaves its outputs as they were when it cannot sample: FreeSurfer's three
    // floats keep the last sample, memory contents before the first (FS-939, FS-940)
    let mut last: Option<[f32; 3]> = None;
    for vno in 0..mris.nvertices.max(0) as usize {
        let (x, y, z, rip) = {
            let v = mris.vertex(vno);
            (v.x, v.y, v.z, v.ripflag)
        };
        if rip != 0 {
            continue;
        }
        v1.set_relt(1, 1, x);
        v1.set_relt(2, 1, y);
        v1.set_relt(3, 1, z);
        matrix_multiply(g, out, err, to_vox.as_ref(), Some(&v1), Some(&mut v2))?;
        let (xv, yv, zv) = (
            f64::from(v2.relt(1, 1)),
            f64::from(v2.relt(2, 1)),
            f64::from(v2.relt(3, 1)),
        );
        let (code, p) = gcam_sample_morph(g, err, gcam, xv as f32, yv as f32, zv as f32)?;
        let p = if code == NO_ERROR {
            last = Some(p);
            p
        } else {
            unsampled(g, err, vno, xv, yv, zv, last)?
        };
        let (_, s) = voxel_to_surface_ras(
            g,
            out,
            err,
            vols.dst_vg(),
            f64::from(p[0]),
            f64::from(p[1]),
            f64::from(p[2]),
        )?;
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (s[0] as f32, s[1] as f32, s[2] as f32);
    }
    compute_surface_dimensions(mris);
    get_vol_geom(vols.dst_vg(), &mut mris.vg);
    Ok(())
}

/// A vertex the morph cannot sample (outside its nodes, or a corner node with an invalid
/// position). FreeSurfer keeps the previous sample (FS-939), or memory contents before the first
/// (FS-940: 0 here with `reproduce`); larmorx ends the program with the vertex's number by
/// default (rule B, as FreeSurfer's own `GCAMmorphSurf` refuses such a vertex).
fn unsampled(
    g: &mut Globals,
    err: &mut dyn Write,
    vno: usize,
    xv: f64,
    yv: f64,
    zv: f64,
    last: Option<[f32; 3]>,
) -> Result<[f32; 3], Fatal> {
    let point = format!("({xv}, {yv}, {zv})");
    match last {
        Some(_) => g.notices.add(
            &FS_939,
            format!(
                "MRIStransform: vertex {vno} at voxel {point} is outside the morph or next to an invalid node: FreeSurfer gives it the previous vertex's sample"
            ),
        ),
        None => g.notices.add(
            &FS_940,
            format!(
                "MRIStransform: vertex {vno} at voxel {point} is outside the morph or next to an invalid node before any vertex was sampled: FreeSurfer's position is memory contents (0 here)"
            ),
        ),
    }
    if g.notices.reproduce() {
        return Ok(last.unwrap_or([0.0; 3]));
    }
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!(
            "MRIStransform: vertex {vno} (voxel {point}) is outside the morph or next to an invalid node: the morph does not move it\n"
        ),
    )?;
    Ok(last.unwrap_or([0.0; 3]))
}

/// The geometry FreeSurfer copies into a volume header it makes (`MRIallocHeader` of the extent,
/// one frame of bytes): the direction cosines, the centre, the voxel sizes, `ras_good_flag = 1`,
/// then the cached matrices (`MRIreInitCache`) when `reinit`.
fn header_from(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    reinit: bool,
) -> Result<Mri, Fatal> {
    let mut m = mri_alloc_header(g, out, err, vg.width, vg.height, vg.depth, MRI_UCHAR, 1)?;
    copy_geometry(vg, &mut m.vg);
    if reinit {
        reinit_cache(g, out, err, &mut m.vg)?;
    }
    Ok(m)
}

/// The direction cosines, centre and voxel sizes of `s` into `d`, `ras_good_flag = 1`.
fn copy_geometry(s: &VolGeom, d: &mut VolGeom) {
    (d.x_r, d.y_r, d.z_r, d.c_r) = (s.x_r, s.y_r, s.z_r, s.c_r);
    (d.x_a, d.y_a, d.z_a, d.c_a) = (s.x_a, s.y_a, s.z_a, s.c_a);
    (d.x_s, d.y_s, d.z_s, d.c_s) = (s.x_s, s.y_s, s.z_s, s.c_s);
    (d.xsize, d.ysize, d.zsize) = (s.xsize, s.ysize, s.zsize);
    d.ras_good_flag = 1;
}

/// The coronal direction cosines (`x_r = -1`, `z_a = 1`, `y_s = -1`).
fn coronal(d: &mut VolGeom) {
    (d.x_r, d.y_r, d.z_r) = (-1.0, 0.0, 0.0);
    (d.x_a, d.y_a, d.z_a) = (0.0, 0.0, 1.0);
    (d.x_s, d.y_s, d.z_s) = (0.0, -1.0, 0.0);
}

/// The linear array's part of [`mris_transform`].
fn linear(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    vols: &mut Vols<'_>,
    lta: &Lta,
) -> Result<(), Fatal> {
    let dst_not_given = !vols.has_dst();
    if lta.num_xforms > 1 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "we cannot handle multiple transforms\n",
        )?;
    }
    if lta.num_xforms == 0 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "transform does not have transform ;-) \n",
        )?;
    }
    let Some(lt) = lta.xforms.first() else {
        // a negative count: FreeSurfer reads past the array (as FS-859)
        g.notices.add(
            &larmorx_core::defects::FS_859,
            "MRIStransform of a transform array of a negative count: FreeSurfer reads past the array",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIStransform: the transform array holds no transform (FreeSurfer reads past it here)\n",
        )?;
        return Ok(());
    };
    if lta.ltype == LINEAR_RAS_TO_RAS {
        centre_warnings(g, err, mris, vols, lt.src.valid == 1, &lt.src);
    }
    if !vols.has_src() && lt.src.valid == 1 {
        let _ = err.write_all(b"INFO:try to get src info from transform.\n");
        vols.src = Slot::Own(Box::new(header_from(g, out, err, &lt.src, true)?));
    } else if !vols.has_src() && mris.vg.valid == 1 {
        let _ = err.write_all(b"INFO:try to get src info from the surface.\n");
        let vg = mris.vg.clone();
        vols.src = Slot::Own(Box::new(header_from(g, out, err, &vg, true)?));
    } else if !vols.has_src() {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "When mri == NULL, the transform must have the valid src info.\n",
        )?;
        return Ok(());
    }
    if dst_not_given && lt.dst.valid == 1 {
        if g.gdiag & DIAG_SHOW != 0 && verbose_on(g) {
            let _ = err.write_all(b"INFO: Get dst info from transform.\n");
        }
        vols.dst = Slot::Own(Box::new(header_from(g, out, err, &lt.dst, false)?));
    } else if dst_not_given {
        let _ = err.write_all(banner(57).as_bytes());
        let _ =
            err.write_all(b"WARNING: transform does not have valid destination volume.       \n");
        let _ =
            err.write_all(b"WARNING: The standard CORONAL volume with c_(ras) = 0 is assumed.\n");
        let _ = err.write_all(banner(57).as_bytes());
        let (w, h, d) = (lt.dst.width, lt.dst.height, lt.dst.depth);
        let mut m = mri_alloc_header(g, out, err, w, h, d, MRI_UCHAR, 1)?;
        coronal(&mut m.vg);
        (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (0.0, 0.0, 0.0);
        (m.vg.xsize, m.vg.ysize, m.vg.zsize) = (1.0, 1.0, 1.0);
        m.vg.ras_good_flag = 1;
        vols.dst = Slot::Own(Box::new(m));
    }
    if dst_not_given {
        // the destination is taken as the source's extent and voxel sizes, coronal: only its
        // centre is kept
        let s = vols.src_vg().clone();
        let d = vols.dst_vg();
        (d.width, d.height, d.depth) = (s.width, s.height, s.depth);
        (d.xsize, d.ysize, d.zsize) = (s.xsize, s.ysize, s.zsize);
        coronal(d);
    }
    reinit_cache(g, out, err, vols.dst_vg())?;
    let mut sras: Option<Matrix> = None;
    if lta.ltype == LINEAR_RAS_TO_RAS {
        if mris.useRealRAS != 0 {
            sras = ptr_mut(matrix_copy(g, out, err, lt.m_l.as_ref(), None)?);
        } else {
            let r_from_s = ptr_mut(ras_from_surface_ras(g, out, err, vols.src_vg(), None)?);
            let s_from_r = surface_ras_from_ras(g, out, err, vols.dst_vg())?;
            let m = ptr_mut(matrix_multiply(
                g,
                out,
                err,
                lt.m_l.as_ref(),
                r_from_s.as_ref(),
                None,
            )?);
            sras = ptr_mut(matrix_multiply(
                g,
                out,
                err,
                s_from_r.as_ref(),
                m.as_ref(),
                None,
            )?);
        }
    } else if lta.ltype == LINEAR_VOX_TO_VOX {
        let s = vols.src_vg().clone();
        let d = vols.dst_vg().clone();
        if s.width != d.width || s.height != d.height || s.depth != d.depth {
            let _ = err.write_all(banner(56).as_bytes());
            let _ = err
                .write_all(b"WARNING:voxel-to-voxel transform must have the same volume sizes.\n");
            let _ = writeln!(
                err,
                "WARNING:You gave src ({}, {}m, {}) vs. dst ({}, {}, {}).",
                s.width, s.height, s.depth, d.width, d.height, d.depth
            );
            let _ = err.write_all(b"WARNING:The result of this transform is most likely wrong.\n");
            let _ = err.write_all(banner(56).as_bytes());
        }
        let to_vox = voxel_from_surface_ras(g, out, err, vols.src_vg())?;
        let from_vox = surface_ras_from_voxel(g, out, err, vols.dst_vg())?;
        let m = ptr_mut(matrix_multiply(
            g,
            out,
            err,
            lt.m_l.as_ref(),
            to_vox.as_ref(),
            None,
        )?);
        sras = ptr_mut(matrix_multiply(
            g,
            out,
            err,
            Some(&from_vox),
            m.as_ref(),
            None,
        )?);
    }
    for vno in 0..mris.nvertices.max(0) as usize {
        let (x, y, z, rip) = {
            let v = mris.vertex(vno);
            (v.x, v.y, v.z, v.ripflag)
        };
        if rip != 0 {
            continue;
        }
        let Some(m) = sras.as_ref() else {
            g.notices.add(
                &FS_942,
                format!(
                    "MRIStransform of a linear transform of type {}: FreeSurfer applies a null matrix to the vertices (a crash)",
                    lta.ltype
                ),
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIStransform: a linear transform of type {} (only voxel-to-voxel and RAS-to-RAS are applied; FreeSurfer crashes here)\n",
                    lta.ltype
                ),
            )?;
            return Ok(());
        };
        let p = transform_with_matrix(m, f64::from(x), f64::from(y), f64::from(z));
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (p[0] as f32, p[1] as f32, p[2] as f32);
    }
    compute_surface_dimensions(mris);
    get_vol_geom(vols.dst_vg(), &mut mris.vg);
    Ok(())
}

/// The two c_(ras) warnings of a RAS-to-RAS transform with a source volume: the transform's
/// source geometry (when valid) and the surface's (when valid) against the volume. FreeSurfer
/// compares each centre with itself (FS-941: the warnings appear only for a centre that is not
/// finite); larmorx compares them with the volume's centre by default.
fn centre_warnings(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    vols: &mut Vols<'_>,
    src_valid: bool,
    src: &VolGeom,
) {
    let Some(mri) = vols.src() else {
        return;
    };
    let mri = mri.vg.clone();
    let reproduce = g.notices.reproduce();
    for (which, other, valid) in [
        ("the transform src", src, src_valid),
        ("the surface stored volume", &mris.vg, mris.vg.valid == 1),
    ] {
        if !valid {
            continue;
        }
        let theirs = centres_differ(other, other);
        let ours = centres_differ(&mri, other);
        if theirs != ours {
            g.notices.add(
                &FS_941,
                format!(
                    "MRIStransform's c_(ras) check of the input volume and {which}: FreeSurfer compares the centre with itself ({} the warning)",
                    if ours { "no" } else { "a" }
                ),
            );
        }
        if !(if reproduce { theirs } else { ours }) {
            continue;
        }
        if which == "the transform src" {
            let _ = err.write_all(banner(52).as_bytes());
            let _ = err.write_all(
                b"WARNING: c_(ras) values are not equal for the input volume and the transform src.\n",
            );
            let _ = err.write_all(b"WARNING: The transformed surface position may be shifted.\n");
            let _ = err.write_all(banner(51).as_bytes());
        } else {
            let _ = err.write_all(banner(57).as_bytes());
            let _ = err.write_all(
                b"WARNING: c_(ras) values are not equal for the input volume and the surface stored volume.\n",
            );
            let _ = err.write_all(b"WARNING: The transformed surface position may be shifted.\n");
            let _ = err.write_all(banner(57).as_bytes());
        }
    }
}
