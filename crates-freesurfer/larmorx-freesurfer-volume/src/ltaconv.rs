// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A linear transform array converted between coordinates and composed with matrices
//! (`utils/transform.cpp`, for track A's `TransformVox2Ras`, `TransformRas2Vox` and
//! `TransformCompose`): `LTAvoxelToRasXform` (1898), `LTArasToVoxelXform` (1930) with
//! `LTAsetVolGeom` (4280), and `LTAcompose` (4844).
//!
//! The products are FreeSurfer's `MatrixMultiply` (`float`), the inverses its `MatrixInverse`.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-volume.md` §28):
//! - **FS-954** (rule A): `LTArasToVoxelXform` stores every converted matrix in the first
//!   transform; corrected: each in place (`reproduce`: FreeSurfer's).
//! - **FS-955** (rule C, kept): `LTAvoxelToRasXform` without volumes converts in the other
//!   direction (FreeSurfer keeps that output on purpose).
//! - **FS-956** (rule A): `LTAcompose` into another array with both matrices keeps only the right
//!   product; corrected: the right matrix multiplies the left product.
//! - **FS-957** (rule A): `LTArasToVoxelXform` with no destination volume crashes copying its
//!   geometry; corrected: the source's geometry for both (`reproduce`: an error).
//! - **FS-504** (an array without transforms where FreeSurfer reads one): an error.

use std::io::Write;

use larmorx_core::defects::{FS_954, FS_956, FS_957};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, lta_alloc};
use larmorx_freesurfer_mri::mri::{Mri, mri_copy_vol_geom_from_mri};

use crate::ltaops::outside;
use crate::orient::mri_voxel_xform_to_ras_xform;
use crate::vgeom::{vg_get_ras_to_voxel_xform, vg_get_voxel_to_ras_xform};
use crate::xform::mri_ras_xform_to_voxel_xform;

/// The transforms `num_xforms` counts, or FS-504's error when the array holds fewer.
fn count(g: &mut Globals, err: &mut dyn Write, lta: &Lta, what: &str) -> Result<usize, Fatal> {
    let n = lta.num_xforms.max(0) as usize;
    if n > lta.xforms.len() {
        outside(g, err, what, "an array of fewer transforms than it counts")?;
        return Ok(lta.xforms.len());
    }
    Ok(n)
}

/// `LTAvoxelToRasXform(lta, mri_src, mri_dst)`: each transform's matrix (taken as voxel to voxel)
/// converted to RAS: with `mri_src`, `MRIvoxelXformToRasXform(mri_src, mri_dst, M)` (`vox2ras(dst)
/// * M * ras2vox(src)`, the destination the source when there is none); without it, from the
/// transform's own geometries, `ras2vox(dst geometry) * (M * vox2ras(src geometry))`
/// (`VGgetVoxelToRasXform` of the source's geometry first, then `VGgetRasToVoxelXform` of the
/// destination's): FS-955's direction, kept. The array becomes `LINEAR_RAS_TO_RAS` (whatever its
/// type was). `NO_ERROR`.
pub fn lta_voxel_to_ras_xform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
    mri_src: Option<&Mri>,
    mri_dst: Option<&Mri>,
) -> Result<i32, Fatal> {
    let n = count(g, err, lta, "LTAvoxelToRasXform")?;
    for i in 0..n {
        let m = match mri_src {
            None => {
                let lt = &lta.xforms[i];
                let src_v2r = vg_get_voxel_to_ras_xform(g, out, err, &lt.src, 0)?;
                let dst_r2v = vg_get_ras_to_voxel_xform(g, out, err, &lt.dst, 0)?;
                let tmp =
                    matrix_multiply(g, out, err, lt.m_l.as_ref(), Some(&src_v2r), None)?.into_new();
                matrix_multiply(g, out, err, dst_r2v.as_ref(), tmp.as_ref(), None)?.into_new()
            }
            Some(s) => {
                let ml = lta.xforms[i].m_l.clone();
                mri_voxel_xform_to_ras_xform(g, out, err, s, mri_dst, ml.as_ref(), None)?
                    .into_new()
            }
        };
        lta.xforms[i].m_l = m;
    }
    lta.ltype = LINEAR_RAS_TO_RAS;
    Ok(NO_ERROR)
}

/// `LTAsetVolGeom(lta, mri_src, mri_dst)`: each transform's source and destination geometries
/// copied from the volumes (`MRIcopyVolGeomFromMRI`). `NO_ERROR`.
pub fn lta_set_vol_geom(lta: &mut Lta, mri_src: &Mri, mri_dst: &Mri) -> i32 {
    let n = (lta.num_xforms.max(0) as usize).min(lta.xforms.len());
    for lt in &mut lta.xforms[..n] {
        mri_copy_vol_geom_from_mri(mri_src, &mut lt.src);
        mri_copy_vol_geom_from_mri(mri_dst, &mut lt.dst);
    }
    NO_ERROR
}

/// `LTArasToVoxelXform(lta, mri_src, mri_dst)`: nothing for an array of `LINEAR_VOX_TO_VOX`;
/// otherwise each transform's matrix (taken as RAS to RAS) converted to voxels,
/// `MRIrasXformToVoxelXform(mri_src, mri_dst, M)` (`ras2vox(dst) * (M * vox2ras(src))`, the
/// destination the source when there is none; `NULL` when a product fails), each in place (FS-954:
/// FreeSurfer stores them all in the first transform, `reproduce`); then the geometries
/// ([`lta_set_vol_geom`]; with no destination volume FS-957: the source's for both, `reproduce`
/// an error where FreeSurfer crashes) and the type `LINEAR_VOX_TO_VOX`. `NO_ERROR`.
pub fn lta_ras_to_voxel_xform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
    mri_src: &Mri,
    mri_dst: Option<&Mri>,
) -> Result<i32, Fatal> {
    if lta.ltype == LINEAR_VOX_TO_VOX {
        return Ok(NO_ERROR);
    }
    let n = count(g, err, lta, "LTArasToVoxelXform")?;
    if n > 1 {
        g.notices.add(
            &FS_954,
            format!(
                "LTArasToVoxelXform of {n} transforms: {}",
                if g.notices.reproduce() {
                    "every converted matrix stored in the first transform"
                } else {
                    "each converted in place"
                }
            ),
        );
    }
    let first_only = g.notices.reproduce();
    for i in 0..n {
        let ml = lta.xforms[i].m_l.clone();
        let m = mri_ras_xform_to_voxel_xform(g, out, err, mri_src, mri_dst, ml.as_ref())?;
        let at = if first_only { 0 } else { i };
        lta.xforms[at].m_l = m;
    }
    match mri_dst {
        Some(d) => {
            lta_set_vol_geom(lta, mri_src, d);
        }
        None => {
            g.notices.add(
                &FS_957,
                "LTArasToVoxelXform with no destination volume (FreeSurfer crashes copying its geometry)",
            );
            if g.notices.reproduce() {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    "LTArasToVoxelXform: no destination volume (FreeSurfer reads through a null volume here)\n",
                )?;
            }
            lta_set_vol_geom(lta, mri_src, mri_src);
        }
    }
    lta.ltype = LINEAR_VOX_TO_VOX;
    Ok(NO_ERROR)
}

/// The products of `LTAcompose` from the source matrix `l` into the destination's matrix `d`:
/// `left * l` when `left` is given, then `l * right` when `right` is given (FS-956: into another
/// array FreeSurfer's second product starts again from `l`, `reproduce`; corrected and in place,
/// from the first product). A destination without a matrix keeps none (FreeSurfer's products go
/// into new matrices it drops).
#[allow(clippy::too_many_arguments)]
fn compose_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    l: Option<&Matrix>,
    left: Option<&Matrix>,
    right: Option<&Matrix>,
    d: &mut Option<Matrix>,
    chained: bool,
) -> Result<(), Fatal> {
    let mut first: Option<Matrix> = None;
    if let Some(left) = left {
        match d.as_mut() {
            Some(m) => {
                matrix_multiply(g, out, err, Some(left), l, Some(m))?;
                first = Some(m.clone());
            }
            None => first = matrix_multiply(g, out, err, Some(left), l, None)?.into_new(),
        }
    }
    if let Some(right) = right {
        let base = if left.is_some() && chained {
            first.as_ref()
        } else {
            l
        };
        let base = base.cloned();
        match d.as_mut() {
            Some(m) => {
                matrix_multiply(g, out, err, base.as_ref(), Some(right), Some(m))?;
            }
            None => {
                matrix_multiply(g, out, err, base.as_ref(), Some(right), None)?;
            }
        }
    }
    Ok(())
}

/// `LTAcompose(lta_src, m_left, m_right, lta_dst)` into another array: with no `lta_dst`, a new
/// array of one transform (`LTAalloc(1, NULL)`: its type `LINEAR_VOX_TO_VOX`) with the source's
/// first transform's geometries; then the first transform's matrix of the destination becomes
/// `m_left * L` (`L` the source's first matrix) when `m_left` is given, then `L * m_right` when
/// `m_right` is given (FS-956: with both, FreeSurfer's result is `L * m_right`, `reproduce`;
/// corrected `(m_left * L) * m_right`). The destination's type and other transforms are not
/// changed. FS-504 for an array without transforms.
#[allow(clippy::too_many_arguments)]
pub fn lta_compose(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta_src: &Lta,
    m_left: Option<&Matrix>,
    m_right: Option<&Matrix>,
    lta_dst: Option<&mut Lta>,
) -> Result<Ret<Lta>, Fatal> {
    const WHAT: &str = "LTAcompose";
    if lta_src.xforms.is_empty() {
        outside(g, err, WHAT, "a source array without transforms")?;
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let given = lta_dst.is_some();
    let dst: &mut Lta = match lta_dst {
        Some(d) => d,
        None => {
            let mut d = lta_alloc(g, out, err, 1, None)?;
            d.xforms[0].src = lta_src.xforms[0].src.clone();
            d.xforms[0].dst = lta_src.xforms[0].dst.clone();
            fresh.insert(d)
        }
    };
    if dst.xforms.is_empty() {
        outside(g, err, WHAT, "a destination array without transforms")?;
        return Ok(Ret::Null);
    }
    let chained = m_left.is_some() && m_right.is_some() && !g.notices.reproduce();
    if m_left.is_some() && m_right.is_some() {
        g.notices.add(
            &FS_956,
            if chained {
                "LTAcompose with a left and a right matrix: (left * L) * right"
            } else {
                "LTAcompose with a left and a right matrix: L * right (the left product overwritten)"
            },
        );
    }
    let l = lta_src.xforms[0].m_l.clone();
    compose_into(
        g,
        out,
        err,
        l.as_ref(),
        m_left,
        m_right,
        &mut dst.xforms[0].m_l,
        chained,
    )?;
    Ok(match fresh {
        Some(d) => Ret::New(d),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `LTAcompose(lta, m_left, m_right, lta)` in place (`TransformCompose` of a transform into
/// itself): the first transform's matrix becomes `m_left * L`, then that times `m_right`
/// (FreeSurfer's products into their own input multiply a copy of it, so both apply).
pub fn lta_compose_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
    m_left: Option<&Matrix>,
    m_right: Option<&Matrix>,
) -> Result<(), Fatal> {
    if lta.xforms.is_empty() {
        outside(g, err, "LTAcompose", "an array without transforms")?;
        return Ok(());
    }
    if let Some(left) = m_left {
        let l = lta.xforms[0].m_l.clone();
        compose_into(
            g,
            out,
            err,
            l.as_ref(),
            Some(left),
            None,
            &mut lta.xforms[0].m_l,
            false,
        )?;
    }
    if let Some(right) = m_right {
        let l = lta.xforms[0].m_l.clone();
        compose_into(
            g,
            out,
            err,
            l.as_ref(),
            None,
            Some(right),
            &mut lta.xforms[0].m_l,
            false,
        )?;
    }
    Ok(())
}
