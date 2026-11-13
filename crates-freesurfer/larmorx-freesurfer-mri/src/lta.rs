// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The linear transform array (`include/transform.h`: `LINEAR_TRANSFORM`, `LINEAR_TRANSFORM_ARRAY`,
//! the transform type numbers) with its allocation, copy and free (`utils/transform.cpp`:
//! `LTAalloc` 630, `LTAcopy` 160, `LTcopy` 185, `LTAfree` 812, and `initVolGeom` 305 in
//! [`crate::vol_geom::init_vol_geom`]), taken early from `fs.volume`'s item 3.1-40 (the
//! orchestrator, 2026-11-13) so that a surface's transform (`MRIS::lta`, `fs.surfio`) is the real
//! structure; the rest of `transform.cpp` stays in 3.1-40.
//!
//! The C arrays `xforms` and `inv_xforms` are vectors of `num_xforms` transforms each (the field is
//! kept, as callers set it; the functions here keep the three equal). A transform's matrices are
//! `Option<Matrix>` (`NULL` possible). FS-195 (a copy's own type and label), FS-196 (a copy onto
//! missing or smaller matrices), FS-197 (freeing nothing), FS-198 (a copy from missing matrices).

use std::io::Write;

use larmorx_core::defects::{FS_195, FS_196, FS_197, FS_198};
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_copy};
use larmorx_freesurfer_core::matrix_ops::matrix_identity;
use larmorx_freesurfer_core::ret::Ret;

use crate::mri::Mri;
use crate::vol_geom::{VolGeom, init_vol_geom};
use crate::voxel::mri_bounding_box;

/// `TransformType`'s `MINC`.
pub const MINC: i32 = 0;
/// `TKREG`.
pub const TKREG: i32 = 1;
/// `GENERIC`.
pub const GENERIC: i32 = 2;
/// `UNKNOWN`: a new transform's type.
pub const UNKNOWN: i32 = -1;

/// `LINEAR_VOX_TO_VOX` (`LINEAR_VOXEL_TO_VOXEL`).
pub const LINEAR_VOX_TO_VOX: i32 = 0;
/// `LINEAR_RAS_TO_RAS`.
pub const LINEAR_RAS_TO_RAS: i32 = 1;
/// `LINEAR_PHYSVOX_TO_PHYSVOX`.
pub const LINEAR_PHYSVOX_TO_PHYSVOX: i32 = 2;
/// `TRANSFORM_ARRAY_TYPE`.
pub const TRANSFORM_ARRAY_TYPE: i32 = 10;
/// `MORPH_3D_TYPE`.
pub const MORPH_3D_TYPE: i32 = 11;
/// `MNI_TRANSFORM_TYPE`.
pub const MNI_TRANSFORM_TYPE: i32 = 12;
/// `MATLAB_ASCII_TYPE`.
pub const MATLAB_ASCII_TYPE: i32 = 13;
/// `REGISTER_DAT`.
pub const REGISTER_DAT: i32 = 14;
/// `FSLREG_TYPE`.
pub const FSLREG_TYPE: i32 = 15;
/// `LINEAR_CORONAL_RAS_TO_CORONAL_RAS` (`LINEAR_COR_TO_COR`).
pub const LINEAR_CORONAL_RAS_TO_CORONAL_RAS: i32 = 21;

/// `errno`'s `ENOMEM` on Linux (a failed `calloc`).
const ENOMEM: i32 = 12;

/// `LINEAR_TRANSFORM` (`LT`).
#[derive(Debug, PartialEq)]
pub struct LinearTransform {
    /// `x0`, `y0`, `z0`: the transform's centre.
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    /// `sigma`: its spread.
    pub sigma: f32,
    /// `m_L`: the matrix.
    pub m_l: Option<Matrix>,
    /// `m_dL`: the functional's gradient with respect to the matrix.
    pub m_dl: Option<Matrix>,
    /// `m_last_dL`: the last step (momentum).
    pub m_last_dl: Option<Matrix>,
    /// `type`.
    pub ltype: i32,
    pub src: VolGeom,
    pub dst: VolGeom,
    /// `label`: the label the transform applies to (0: all).
    pub label: i32,
}

/// `LINEAR_TRANSFORM_ARRAY` (`LTA`).
#[derive(Debug, PartialEq)]
pub struct Lta {
    pub num_xforms: i32,
    pub xforms: Vec<LinearTransform>,
    pub inv_xforms: Vec<LinearTransform>,
    /// `type`.
    pub ltype: i32,
    /// `fscale` (tkregister2's).
    pub fscale: f32,
    /// `subject` (tkregister2's), a `STRLEN` buffer with C's semantics.
    pub subject: CName,
}

/// A `VOL_GEOM` as `calloc` leaves it: every field 0, no name, no matrices.
fn zeroed_vol_geom() -> VolGeom {
    VolGeom {
        xsize: 0.0,
        ysize: 0.0,
        zsize: 0.0,
        x_r: 0.0,
        y_s: 0.0,
        z_a: 0.0,
        ..VolGeom::default()
    }
}

/// One transform of `LTAalloc`: centred at `(x0, y0, z0)`, `sigma` 10000, the identity, two 4 x 4
/// matrices of zeros, both geometries `initVolGeom`'s, type `UNKNOWN`, label 0.
fn new_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    x0: f32,
    y0: f32,
    z0: f32,
) -> Result<LinearTransform, Fatal> {
    let m_l = matrix_identity(g, out, err, 4, None)?.into_new();
    let m_dl = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?;
    let m_last_dl = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?;
    let mut src = zeroed_vol_geom();
    let mut dst = zeroed_vol_geom();
    init_vol_geom(&mut src);
    init_vol_geom(&mut dst);
    Ok(LinearTransform {
        x0,
        y0,
        z0,
        sigma: 10000.0,
        m_l,
        m_dl,
        m_last_dl,
        ltype: UNKNOWN,
        src,
        dst,
        label: 0,
    })
}

/// `calloc(n, sizeof(LT))` of `LTAalloc`: room for `n` transforms, or the error exit
/// `LTAalloc(<n>): could not allocate <what>` (`ERROR_NOMEMORY`, after `Cannot allocate memory`)
/// for a negative `n` or one beyond memory.
fn transforms(
    g: &mut Globals,
    err: &mut dyn Write,
    n: i32,
    what: &str,
) -> Result<Vec<LinearTransform>, Fatal> {
    let mut v = Vec::new();
    if n < 0 || v.try_reserve_exact(n as usize).is_err() {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NOMEMORY,
            &format!("LTAalloc({n}): could not allocate {what}"),
        )?;
    }
    Ok(v)
}

/// `LTAalloc(nxforms, mri)`: an array of `nxforms` transforms and as many inverse transforms, each
/// as [`new_transform`] makes it, centred at the middle of `mri`'s bounding box of voxels above 70
/// (`MRIboundingBox`; in whole voxels: the corner plus half the extent, rounded towards 0) or at 0
/// without `mri`; the array's type 0 (`LINEAR_VOX_TO_VOX`), `fscale` 0, no subject. A negative
/// count (or one beyond memory) ends the program: [`transforms`].
pub fn lta_alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    nxforms: i32,
    mri: Option<&Mri>,
) -> Result<Lta, Fatal> {
    let (x0, y0, z0) = match mri {
        Some(m) => {
            let (_, b) = mri_bounding_box(g, err, m, 70)?;
            (
                b.x.wrapping_add(b.dx / 2) as f32,
                b.y.wrapping_add(b.dy / 2) as f32,
                b.z.wrapping_add(b.dz / 2) as f32,
            )
        }
        None => (0.0, 0.0, 0.0),
    };
    let mut xforms = transforms(g, err, nxforms, "xforms")?;
    let mut inv_xforms = transforms(g, err, nxforms, "inverse xforms")?;
    for _ in 0..nxforms.max(0) {
        xforms.push(new_transform(g, out, err, x0, y0, z0)?);
        inv_xforms.push(new_transform(g, out, err, x0, y0, z0)?);
    }
    Ok(Lta {
        num_xforms: nxforms,
        xforms,
        inv_xforms,
        ltype: 0,
        fscale: 0.0,
        subject: CName::default(),
    })
}

/// `MatrixCopy(src, dst)` as `LTcopy` calls it, ignoring its result: into the destination's matrix
/// when it has the room; FS-196 where it is missing or smaller (FreeSurfer's copy is lost: by
/// default the destination takes a copy of the source's matrix); FS-198 where the source has none
/// (FreeSurfer frees the destination's matrix and keeps referring to it: both modes drop it).
fn copy_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: Option<&Matrix>,
    dst: &mut Option<Matrix>,
    name: &str,
) -> Result<(), Fatal> {
    match (src, dst.as_mut()) {
        (Some(s), Some(d)) => {
            if matrix_copy(g, out, err, Some(s), Some(d))?.is_null() {
                g.notices.add(
                    &FS_196,
                    format!(
                        "LTcopy: the destination's {name} ({} x {}) is smaller than the source's ({} x {}): FreeSurfer keeps the old numbers",
                        d.rows, d.cols, s.rows, s.cols
                    ),
                );
                if !g.notices.reproduce() {
                    *dst = Some(s.clone());
                }
            }
        }
        (Some(s), None) => {
            let copy = matrix_copy(g, out, err, Some(s), None)?.into_new();
            g.notices.add(
                &FS_196,
                format!("LTcopy: the destination has no {name}: FreeSurfer's copy is lost"),
            );
            if !g.notices.reproduce() {
                *dst = copy;
            }
        }
        (None, Some(_)) => {
            g.notices.add(
                &FS_198,
                format!(
                    "LTcopy: the source has no {name}: FreeSurfer frees the destination's but keeps referring to it"
                ),
            );
            *dst = None;
        }
        (None, None) => {}
    }
    Ok(())
}

/// `LTcopy(lt, ltcp)`: `lt`'s centre, spread, matrices (`MatrixCopy` into `ltcp`'s) and
/// geometries (C++'s assignment: FS-59) into `ltcp` (FS-196, FS-198); `true` (the destination
/// returned). FS-195:
/// FreeSurfer copies neither the type nor the label (by default copied). Without a destination:
/// `ERROR: LTcopy() destination LT cannot be NULL` on standard output and `false` (`NULL`).
pub fn lt_copy(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lt: &LinearTransform,
    ltcp: Option<&mut LinearTransform>,
) -> Result<bool, Fatal> {
    let Some(ltcp) = ltcp else {
        let _ = out.write_all(b"ERROR: LTcopy() destination LT cannot be NULL\n");
        return Ok(false);
    };
    ltcp.x0 = lt.x0;
    ltcp.y0 = lt.y0;
    ltcp.z0 = lt.z0;
    ltcp.sigma = lt.sigma;
    copy_matrix(g, out, err, lt.m_l.as_ref(), &mut ltcp.m_l, "m_L")?;
    copy_matrix(g, out, err, lt.m_dl.as_ref(), &mut ltcp.m_dl, "m_dL")?;
    copy_matrix(
        g,
        out,
        err,
        lt.m_last_dl.as_ref(),
        &mut ltcp.m_last_dl,
        "m_last_dL",
    )?;
    ltcp.src.assign(g, &lt.src);
    ltcp.dst.assign(g, &lt.dst);
    if ltcp.ltype != lt.ltype || ltcp.label != lt.label {
        g.notices.add(
            &FS_195,
            format!(
                "LTcopy: the copy's type {} and label {} (the source's {} and {})",
                ltcp.ltype, ltcp.label, lt.ltype, lt.label
            ),
        );
        if !g.notices.reproduce() {
            ltcp.ltype = lt.ltype;
            ltcp.label = lt.label;
        }
    }
    Ok(true)
}

/// `LTAcopy(lta, ltacp)`: `lta` into `ltacp` (a new array of `lta`'s size from `LTAalloc` when
/// there is none): the type, `fscale`, the subject (`strcpy`) and every transform and inverse
/// transform ([`lt_copy`]). A destination with another number of transforms:
/// `LTAcopy(): ERROR: number of xforms does not match (<n>,<m>)` on standard output and `NULL`
/// (the destination unchanged).
pub fn lta_copy(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &Lta,
    ltacp: Option<&mut Lta>,
) -> Result<Ret<Lta>, Fatal> {
    let mut fresh = None;
    let dst: &mut Lta = match ltacp {
        Some(d) => d,
        None => fresh.insert(lta_alloc(g, out, err, lta.num_xforms, None)?),
    };
    if lta.num_xforms != dst.num_xforms {
        let _ = writeln!(
            out,
            "LTAcopy(): ERROR: number of xforms does not match ({},{})",
            lta.num_xforms, dst.num_xforms
        );
        return Ok(Ret::Null);
    }
    dst.ltype = lta.ltype;
    dst.fscale = lta.fscale;
    dst.subject.strcpy(&lta.subject);
    for i in 0..lta.num_xforms.max(0) as usize {
        lt_copy(g, out, err, &lta.xforms[i], Some(&mut dst.xforms[i]))?;
        lt_copy(
            g,
            out,
            err,
            &lta.inv_xforms[i],
            Some(&mut dst.inv_xforms[i]),
        )?;
    }
    Ok(match fresh {
        Some(l) => Ret::New(l),
        None => Ret::Given,
    })
}

/// `LTAfree(&lta)`: the array and its matrices freed, `lta` `NULL`; `NO_ERROR`. FS-197: with no
/// array (`*plta` `NULL`) FreeSurfer crashes; here nothing is freed (both modes).
pub fn lta_free(g: &mut Globals, plta: &mut Option<Lta>) -> i32 {
    if plta.is_none() {
        g.notices.add(
            &FS_197,
            "LTAfree of no transform array: nothing to free (FreeSurfer crashes)",
        );
    }
    *plta = None;
    NO_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    use crate::mri::{self, MRI_FLOAT, MRI_UCHAR};
    use crate::voxel;

    #[test]
    fn a_new_array_and_its_centre() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let l = lta_alloc(&mut g, &mut o, &mut e, 2, None).unwrap();
        assert_eq!(
            (l.num_xforms, l.xforms.len(), l.inv_xforms.len()),
            (2, 2, 2)
        );
        let t = &l.xforms[1];
        assert_eq!(
            (t.x0, t.sigma, t.ltype, t.label),
            (0.0, 10000.0, UNKNOWN, 0)
        );
        assert_eq!(t.m_l.as_ref().unwrap().relt(4, 4), 1.0);
        assert_eq!(t.m_dl.as_ref().unwrap().relt(1, 1), 0.0);
        assert_eq!((t.src.width, t.src.xsize, t.src.x_r), (256, 1.0, -1.0));
        assert_eq!(t.src.fname.c_bytes(), b"unknown");
        // a volume with voxels above 70 at x 1..2, y 0, z 3: the centre (1 + 2 / 2, 0, 3)
        let mut m = mri::mri_alloc(&mut g, &mut o, &mut e, 4, 2, 5, MRI_UCHAR).unwrap();
        voxel::mri_set_vox_val(&mut g, &mut e, &mut m, 1, 0, 3, 0, 71.0, 0).unwrap();
        voxel::mri_set_vox_val(&mut g, &mut e, &mut m, 2, 0, 3, 0, 200.0, 0).unwrap();
        let l = lta_alloc(&mut g, &mut o, &mut e, 1, Some(&m)).unwrap();
        assert_eq!(
            (l.xforms[0].x0, l.xforms[0].y0, l.xforms[0].z0),
            (2.0, 0.0, 3.0)
        );
        // an empty float volume: the corner at the last voxel, extents 2 - n
        let m = mri::mri_alloc(&mut g, &mut o, &mut e, 4, 3, 2, MRI_FLOAT).unwrap();
        let (r, b) = voxel::mri_bounding_box(&mut g, &mut e, &m, 70).unwrap();
        assert_eq!(
            (r, b.x, b.dx, b.y, b.dy, b.z, b.dz),
            (0, 3, -2, 2, -1, 1, 0)
        );
        assert!(o.is_empty() && e.is_empty());
        // a negative count ends the program
        let r = lta_alloc(&mut g, &mut o, &mut e, -1, None);
        assert_eq!(r.unwrap_err().status(), 254);
        assert!(String::from_utf8_lossy(&e).contains("LTAalloc(-1): could not allocate xforms"));
    }

    #[test]
    fn fs195_fs196_a_copy() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut a = lta_alloc(&mut g, &mut o, &mut e, 1, None).unwrap();
            a.ltype = LINEAR_RAS_TO_RAS;
            a.subject = CName::new(b"bert");
            a.xforms[0].ltype = LINEAR_RAS_TO_RAS;
            a.xforms[0].label = 17;
            a.xforms[0].m_l.as_mut().unwrap().set_relt(1, 4, 5.0);
            a.inv_xforms[0].m_dl = None;
            let c = lta_copy(&mut g, &mut o, &mut e, &a, None)
                .unwrap()
                .unwrap_new();
            assert_eq!(c.ltype, LINEAR_RAS_TO_RAS);
            assert_eq!(c.subject.c_bytes(), b"bert");
            assert_eq!(c.xforms[0].m_l.as_ref().unwrap().relt(1, 4), 5.0);
            assert!(c.inv_xforms[0].m_dl.is_none());
            let fixed = mode == UpstreamDefects::Fix;
            assert_eq!(
                c.xforms[0].ltype,
                if fixed { LINEAR_RAS_TO_RAS } else { UNKNOWN }
            );
            assert_eq!(c.xforms[0].label, if fixed { 17 } else { 0 });
            assert!(g.notices.contains(&FS_195) && g.notices.contains(&FS_198));
            // the other way: the destination lacks a matrix the source has
            let mut d = lta_alloc(&mut g, &mut o, &mut e, 1, None).unwrap();
            d.xforms[0].m_last_dl = None;
            lta_copy(&mut g, &mut o, &mut e, &a, Some(&mut d)).unwrap();
            assert_eq!(d.xforms[0].m_last_dl.is_some(), fixed);
            assert!(g.notices.contains(&FS_196));
            // another count: the message and NULL
            let mut d = lta_alloc(&mut g, &mut o, &mut e, 2, None).unwrap();
            assert!(
                lta_copy(&mut g, &mut o, &mut e, &a, Some(&mut d))
                    .unwrap()
                    .is_null()
            );
            assert_eq!(
                String::from_utf8_lossy(&o),
                "LTAcopy(): ERROR: number of xforms does not match (1,2)\n"
            );
            let mut p = Some(d);
            assert_eq!(lta_free(&mut g, &mut p), NO_ERROR);
            assert!(p.is_none() && !g.notices.contains(&FS_197));
            lta_free(&mut g, &mut p);
            assert!(g.notices.contains(&FS_197));
        }
    }
}
