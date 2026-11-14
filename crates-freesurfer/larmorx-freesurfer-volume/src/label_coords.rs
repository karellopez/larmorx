// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A label's coordinates converted between tkregister ("surface") RAS, scanner RAS and voxels
//! (`utils/label.cpp`: `LabelToScannerRAS`, `LabelToSurfaceRAS` with `LabelFromScannerRAS`, its
//! other name, `LabelToVoxel`, `LabelVoxelToSurfaceRAS`), with the surface's own conversions they
//! reach (`utils/mrisurf_mri.cpp`: `MRISsurfaceRASToVoxel`, `MRISsurfaceRASFromVoxel`, over the
//! surface's volume geometry). Track A's module of fs.labels (3.1-31), here because it needs the
//! volume's matrices (`crate::coords`).
//!
//! D13 (larmorx_core::defects): FS-437 (a voxel label of a surface converted the other way).

use std::io::Write;

use larmorx_core::defects::FS_437;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::label::{
    LABEL_COORDS_SCANNER_RAS, LABEL_COORDS_TKREG_RAS, LABEL_COORDS_VOXEL, Label, label_copy,
};
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc_header, mri_copy_vol_geom_to_mri};
use larmorx_freesurfer_mri::tkreg::ras_from_surface_ras;
use larmorx_freesurfer_mri::vol_geom::{VolGeom, extract_r_to_i};

use crate::coords::{voxel_from_surface_ras, voxel_to_surface_ras};

/// Where a conversion writes: a new label (`NULL`), the source itself, or a given label.
#[derive(Debug)]
pub enum LabelDst {
    New,
    Same,
    Given(Box<Label>),
}

/// The streams and environment the conversions write to and read.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    /// The program's environment (`SUBJECTS_DIR` for `LabelCopy`'s new label).
    pub env: &'a Environment,
}

/// The working label of a conversion: `LabelCopy(lsrc, ldst)` unless the destination is the
/// source.
fn start(g: &mut Globals, io: &mut Io, lsrc: &Label, ldst: LabelDst) -> Result<Label, Fatal> {
    match ldst {
        LabelDst::Same => Ok(lsrc.clone()),
        LabelDst::New => label_copy(g, io.env, io.err, lsrc, None),
        LabelDst::Given(d) => label_copy(g, io.env, io.err, lsrc, Some(*d)),
    }
}

/// The destination of a nested conversion: the working label, or the source itself.
fn again(same: bool, work: Label) -> (LabelDst, Option<Label>) {
    if same {
        (LabelDst::Same, Some(work))
    } else {
        (LabelDst::Given(Box::new(work)), None)
    }
}

/// A real 4 x 1 vector `(0, 0, 0, 1)` (`VectorAlloc(4, MATRIX_REAL)`, element 4 set to 1).
fn vec4(g: &mut Globals, io: &mut Io) -> Result<Matrix, Fatal> {
    let mut v = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    v.set_relt(4, 1, 1.0);
    Ok(v)
}

/// The points `0..n` of `work` (the label's points, at most its places) through `m` in `float`
/// (`MatrixMultiply(m, v1, v2)` with `v1` the point and `v2` the result; `m` absent ends the
/// program). With `voxels`, each coordinate's `nint` becomes the point's voxel too.
fn map_points(
    g: &mut Globals,
    io: &mut Io,
    m: Option<&Matrix>,
    work: &mut Label,
    n: i32,
    voxels: bool,
) -> Result<(), Fatal> {
    let mut v1 = vec4(g, io)?;
    let mut v2 = vec4(g, io)?;
    // a label holds a place for each of its points (LabelAlloc, LabelRead, LabelCopy)
    let n = (n.max(0) as usize).min(work.lv.len());
    for i in 0..n {
        let p = work.lv[i];
        v1.set_relt(1, 1, p.x);
        v1.set_relt(2, 1, p.y);
        v1.set_relt(3, 1, p.z);
        matrix_multiply(g, io.out, io.err, m, Some(&v1), Some(&mut v2))?;
        let (x, y, z) = (v2.relt(1, 1), v2.relt(2, 1), v2.relt(3, 1));
        let q = &mut work.lv[i];
        (q.x, q.y, q.z) = (x, y, z);
        if voxels {
            q.xv = nint(g, f64::from(x));
            q.yv = nint(g, f64::from(y));
            q.zv = nint(g, f64::from(z));
        }
    }
    Ok(())
}

/// `RASFromSurfaceRAS_(mri, NULL)`: tkregister RAS to scanner RAS; a tkregister vox2ras that
/// cannot be inverted ends the program (`MatrixMultiply(): m2 is null`).
fn tk_to_scanner(g: &mut Globals, io: &mut Io, vg: &VolGeom) -> Result<Option<Matrix>, Fatal> {
    Ok(ras_from_surface_ras(g, io.out, io.err, vg, None)?.into_new())
}

/// `LabelToScannerRAS(lsrc, mri, ldst)`: the points from tkregister RAS (voxels first made
/// tkregister RAS, [`label_to_surface_ras`]) to scanner RAS, space `scanner`; a label already in
/// scanner RAS is only copied. The matrix is made before the copy (a volume whose tkregister
/// vox2ras cannot be inverted ends the program even for a label already in scanner RAS), and again
/// after the voxels' conversion. `surface`: the geometry of the label's surface (`mris->vg`) when
/// it has one (`lsrc.mris`); `None` reads as a geometry that is not valid.
pub fn label_to_scanner_ras(
    g: &mut Globals,
    io: &mut Io,
    lsrc: &Label,
    mri: &mut Mri,
    surface: Option<&VolGeom>,
    ldst: LabelDst,
) -> Result<Label, Fatal> {
    tk_to_scanner(g, io, &mri.vg)?;
    let same = matches!(ldst, LabelDst::Same);
    let mut work = start(g, io, lsrc, ldst)?;
    if lsrc.coords == LABEL_COORDS_SCANNER_RAS {
        return Ok(work);
    }
    if lsrc.coords == LABEL_COORDS_VOXEL {
        let (dst, cur) = again(same, work);
        work = label_to_surface_ras(g, io, cur.as_ref().unwrap_or(lsrc), mri, surface, dst)?;
    }
    let m = tk_to_scanner(g, io, &mri.vg)?;
    map_points(g, io, m.as_ref(), &mut work, lsrc.n_points, false)?;
    work.space = b"scanner".to_vec();
    work.coords = LABEL_COORDS_SCANNER_RAS;
    Ok(work)
}

/// `LabelToSurfaceRAS(lsrc, mri, ldst)` (`LabelFromScannerRAS`): a label in tkregister RAS only
/// copied; voxels through [`label_voxel_to_surface_ras`]; scanner RAS through the inverse of
/// `RASFromSurfaceRAS_` (a matrix that cannot be inverted ends the program in the product), space
/// `TkReg`. Other coordinates end the program: `LabelToSurfaceRAS: unsupported coords (was <n>)`
/// (`ERROR_UNSUPPORTED`). The matrix is made first, for every label. `surface` as in
/// [`label_to_scanner_ras`].
pub fn label_to_surface_ras(
    g: &mut Globals,
    io: &mut Io,
    lsrc: &Label,
    mri: &mut Mri,
    surface: Option<&VolGeom>,
    ldst: LabelDst,
) -> Result<Label, Fatal> {
    let m = tk_to_scanner(g, io, &mri.vg)?;
    let same = matches!(ldst, LabelDst::Same);
    let mut work = start(g, io, lsrc, ldst)?;
    match lsrc.coords {
        LABEL_COORDS_TKREG_RAS => return Ok(work),
        LABEL_COORDS_VOXEL => {
            let (dst, cur) = again(same, work);
            return label_voxel_to_surface_ras(
                g,
                io,
                cur.as_ref().unwrap_or(lsrc),
                mri,
                surface,
                dst,
            );
        }
        LABEL_COORDS_SCANNER_RAS => {}
        c => {
            log::error_exit(
                g,
                io.err,
                ERROR_UNSUPPORTED,
                &format!("LabelToSurfaceRAS: unsupported coords (was {c})"),
            )?;
        }
    }
    let inv = matrix_inverse(g, io.out, io.err, m.as_ref(), None)?.into_new();
    map_points(g, io, inv.as_ref(), &mut work, lsrc.n_points, false)?;
    work.space = b"TkReg".to_vec();
    work.coords = LABEL_COORDS_TKREG_RAS;
    Ok(work)
}

/// `LabelFromScannerRAS`: [`label_to_surface_ras`] (FreeSurfer's other name for it).
pub fn label_from_scanner_ras(
    g: &mut Globals,
    io: &mut Io,
    lsrc: &Label,
    mri: &mut Mri,
    surface: Option<&VolGeom>,
    ldst: LabelDst,
) -> Result<Label, Fatal> {
    label_to_surface_ras(g, io, lsrc, mri, surface, ldst)
}

/// `LabelToVoxel(lsrc, mri, ldst)`: a label in voxels only copied; scanner RAS first made
/// tkregister RAS ([`label_to_surface_ras`]); then every point through `voxelFromSurfaceRAS_` (the
/// volume's cached matrices filled where missing; a matrix that cannot be inverted ends the
/// program in the product), each coordinate's `nint` its voxel, space `voxel`. Labels in other
/// coordinates are taken as tkregister RAS. `surface` as in [`label_to_scanner_ras`].
pub fn label_to_voxel(
    g: &mut Globals,
    io: &mut Io,
    lsrc: &Label,
    mri: &mut Mri,
    surface: Option<&VolGeom>,
    ldst: LabelDst,
) -> Result<Label, Fatal> {
    let same = matches!(ldst, LabelDst::Same);
    let mut work = start(g, io, lsrc, ldst)?;
    if lsrc.coords == LABEL_COORDS_VOXEL {
        return Ok(work);
    }
    if lsrc.coords == LABEL_COORDS_SCANNER_RAS {
        let (dst, cur) = again(same, work);
        work = label_to_surface_ras(g, io, cur.as_ref().unwrap_or(lsrc), mri, surface, dst)?;
    }
    let m = voxel_from_surface_ras(g, io.out, io.err, &mut mri.vg)?;
    map_points(g, io, m.as_ref(), &mut work, lsrc.n_points, true)?;
    work.space = b"voxel".to_vec();
    work.coords = LABEL_COORDS_VOXEL;
    Ok(work)
}

/// `LabelVoxelToSurfaceRAS(lsrc, mri, ldst)`: each point, a voxel of `mri`, in tkregister RAS
/// (`MRIvoxelToSurfaceRAS`, its matrices made again for each point), space `TkReg`. FS-437: for a
/// label of a surface (`lsrc.mris`) FreeSurfer converts each point the other way, as a surface
/// RAS point to a voxel ([`mris_surface_ras_to_voxel`]); by default it is converted from the voxel
/// to the surface's RAS ([`mris_surface_ras_from_voxel`]). `surface` as in
/// [`label_to_scanner_ras`].
pub fn label_voxel_to_surface_ras(
    g: &mut Globals,
    io: &mut Io,
    lsrc: &Label,
    mri: &mut Mri,
    surface: Option<&VolGeom>,
    ldst: LabelDst,
) -> Result<Label, Fatal> {
    let mut work = start(g, io, lsrc, ldst)?;
    let n = (lsrc.n_points.max(0) as usize).min(lsrc.lv.len());
    if lsrc.mris.is_some() && n > 0 {
        g.notices.add(
            &FS_437,
            "a voxel label of a surface made tkregister RAS: FreeSurfer converts its points the other way",
        );
    }
    for i in 0..n {
        let p = lsrc.lv[i];
        let (x, y, z) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
        let s = if lsrc.mris.is_some() {
            if g.notices.reproduce() {
                mris_surface_ras_to_voxel(g, io, surface, mri, x, y, z)?
            } else {
                mris_surface_ras_from_voxel(g, io, surface, mri, x, y, z)?
            }
        } else {
            voxel_to_surface_ras(g, io.out, io.err, &mut mri.vg, x, y, z)?.1
        };
        let q = &mut work.lv[i];
        (q.x, q.y, q.z) = (s[0] as f32, s[1] as f32, s[2] as f32);
    }
    work.coords = LABEL_COORDS_TKREG_RAS;
    work.space = b"TkReg".to_vec();
    Ok(work)
}

/// The surface's tkregister RAS to the volume's voxels: `inv(vox2ras(mri)) RASFromSurfaceRAS_`, the
/// latter of a header made from the surface's geometry when it is valid (`MRIallocHeader` of its
/// extent, `uchar`, one frame, `MRIcopyVolGeomToMRI`), else of `mri`.
fn sras2vox(
    g: &mut Globals,
    io: &mut Io,
    surface: Option<&VolGeom>,
    mri: &Mri,
) -> Result<Option<Matrix>, Fatal> {
    let sras2ras = match surface.filter(|vg| vg.valid != 0) {
        Some(vg) => {
            let mut tmp = mri_alloc_header(
                g, io.out, io.err, vg.width, vg.height, vg.depth, MRI_UCHAR, 1,
            )?;
            mri_copy_vol_geom_to_mri(&mut tmp, vg);
            tk_to_scanner(g, io, &tmp.vg)?
        }
        None => tk_to_scanner(g, io, &mri.vg)?,
    };
    let ras2vox = extract_r_to_i(g, io.out, io.err, &mri.vg)?;
    Ok(matrix_multiply(g, io.out, io.err, ras2vox.as_ref(), sras2ras.as_ref(), None)?.into_new())
}

/// `m * (x, y, z, 1)` in `float` (`MatrixMultiply` into a 4 x 1 vector), its first three
/// elements.
#[allow(clippy::too_many_arguments)]
fn apply(
    g: &mut Globals,
    io: &mut Io,
    m: Option<&Matrix>,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], Fatal> {
    let mut v1 = vec4(g, io)?;
    let mut v2 = vec4(g, io)?;
    v1.set_relt(1, 1, x as f32);
    v1.set_relt(2, 1, y as f32);
    v1.set_relt(3, 1, z as f32);
    matrix_multiply(g, io.out, io.err, m, Some(&v1), Some(&mut v2))?;
    Ok([
        f64::from(v2.relt(1, 1)),
        f64::from(v2.relt(2, 1)),
        f64::from(v2.relt(3, 1)),
    ])
}

/// `MRISsurfaceRASToVoxel(mris, mri, r, a, s)` with the surface's geometry `surface` (`mris->vg`;
/// `None` or not valid: `mri`'s): a point of the surface's tkregister RAS as a voxel of `mri`.
pub fn mris_surface_ras_to_voxel(
    g: &mut Globals,
    io: &mut Io,
    surface: Option<&VolGeom>,
    mri: &Mri,
    r: f64,
    a: f64,
    s: f64,
) -> Result<[f64; 3], Fatal> {
    let m = sras2vox(g, io, surface, mri)?;
    apply(g, io, m.as_ref(), r, a, s)
}

/// `MRISsurfaceRASFromVoxel(mris, mri, x, y, z)` with the surface's geometry as in
/// [`mris_surface_ras_to_voxel`]: a voxel of `mri` in the surface's tkregister RAS (the inverse of
/// that conversion's matrix; one that cannot be inverted ends the program in the product).
pub fn mris_surface_ras_from_voxel(
    g: &mut Globals,
    io: &mut Io,
    surface: Option<&VolGeom>,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], Fatal> {
    let m = sras2vox(g, io, surface, mri)?;
    let inv = matrix_inverse(g, io.out, io.err, m.as_ref(), None)?.into_new();
    apply(g, io, inv.as_ref(), x, y, z)
}
