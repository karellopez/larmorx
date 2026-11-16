// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A warp volume's change of format (`utils/warpfield.cpp`, track A's relay 39 of FreeSurfer's
//! foundations): `Warpfield::Warpfield(MRI *)` (55), `Warpfield::changeFormat` (659) and its four
//! `__changeFormatFrom_*` (678, 743, 816, 881), as `niiWrite` calls them to write a warp volume's
//! three frames as RAS displacements (the volume itself changed, as FreeSurfer changes it).
//!
//! A warp volume (intent `MGZ_INTENT_WARPMAP`) holds, at each atlas voxel `(c, r, s)`, the image
//! position it maps to, in one of four formats: the image voxel (`ABS_CRS`), its displacement from
//! `(c, r, s)` (`DISP_CRS`), its RAS position (`ABS_RAS`) or that position's displacement from the
//! atlas voxel's RAS position (`DISP_RAS`). The RAS positions use the image and atlas geometries
//! the volume carries, with their shears.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply_d};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_WARPMAP, Mri};
use larmorx_freesurfer_mri::tkreg::{get_ras2vox, get_vox2ras};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

/// `WarpfieldDTFMT`'s formats.
pub const WARPFIELD_DTFMT_ABS_CRS: i32 = 0;
pub const WARPFIELD_DTFMT_DISP_CRS: i32 = 1;
pub const WARPFIELD_DTFMT_ABS_RAS: i32 = 2;
pub const WARPFIELD_DTFMT_DISP_RAS: i32 = 3;

/// A column `(x, y, z, 1)` of `float`s.
fn col(v: [f32; 3]) -> Matrix {
    let mut m = Matrix::real(4, 1);
    m.set_relt(1, 1, v[0]);
    m.set_relt(2, 1, v[1]);
    m.set_relt(3, 1, v[2]);
    m.set_relt(4, 1, 1.0);
    m
}

/// `MatrixMultiplyD(m, v, out)`'s first three elements.
fn mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
    v: [f32; 3],
) -> Result<[f32; 3], Fatal> {
    let x = col(v);
    let mut y = Matrix::real(4, 1);
    matrix_multiply_d(g, out, err, m, Some(&x), Some(&mut y))?;
    Ok([y.relt(1, 1), y.relt(2, 1), y.relt(3, 1)])
}

/// `a - b` per coordinate (`float`).
fn minus(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [sse::sub(a[0], b[0]), sse::sub(a[1], b[1]), sse::sub(a[2], b[2])]
}

/// `a + b` per coordinate (`float`).
fn plus(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [sse::add(a[0], b[0]), sse::add(a[1], b[1]), sse::add(a[2], b[2])]
}

/// `new Warpfield(mri)` then `changeFormat(newformat)`: the volume's format set to `newformat`;
/// when it differs from the old one, every atlas voxel's three frames converted (in `float`, the
/// matrix products in `double` through `MatrixMultiplyD`): to and from voxels with the image
/// geometry's voxel-to-RAS matrix and its inverse (both with shears), displacements with the atlas
/// voxel or the atlas geometry's voxel-to-RAS matrix. A volume of another intent ends the program
/// (`[ERROR] Can't create Warpfield object with non MGZ_INTENT_WARPMAP MRI` on standard output,
/// status 1).
pub fn change_format(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Mri,
    newformat: i32,
) -> Result<(), Fatal> {
    if m.intent != MGZ_INTENT_WARPMAP {
        let _ = out
            .write_all(b"[ERROR] Can't create Warpfield object with non MGZ_INTENT_WARPMAP MRI\n");
        return Err(Fatal::Exit(1));
    }
    let image = m.gcamorph_image_vg.clone();
    let atlas = m.gcamorph_atlas_vg.clone();
    let src_ras2vox = get_ras2vox(g, out, err, &image, 0, true)?;
    let src_vox2ras = get_vox2ras(g, out, err, &image, 0, true)?;
    let _ = get_ras2vox(g, out, err, &atlas, 0, true)?;
    let dst_vox2ras = get_vox2ras(g, out, err, &atlas, 0, true)?;
    let format = m.warp_field_format;
    m.warp_field_format = newformat;
    if format == newformat {
        return Ok(());
    }
    if !(WARPFIELD_DTFMT_ABS_CRS..=WARPFIELD_DTFMT_DISP_RAS).contains(&format) {
        return Ok(());
    }
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    for c in 0..w.max(0) {
        for r in 0..h.max(0) {
            for s in 0..d.max(0) {
                let mut v = [0f32; 3];
                for (k, x) in v.iter_mut().enumerate() {
                    *x = mri_get_vox_val(g, err, m, c, r, s, k as i32, 0)?;
                }
                let atlas_crs = [c as f32, r as f32, s as f32];
                let new = match format {
                    WARPFIELD_DTFMT_ABS_CRS => {
                        if newformat == WARPFIELD_DTFMT_DISP_CRS {
                            Some(minus(v, atlas_crs))
                        } else {
                            let image_ras = mul(g, out, err, Some(&src_vox2ras), v)?;
                            if newformat == WARPFIELD_DTFMT_ABS_RAS {
                                Some(image_ras)
                            } else if newformat == WARPFIELD_DTFMT_DISP_RAS {
                                let atlas_ras = mul(g, out, err, Some(&dst_vox2ras), atlas_crs)?;
                                Some(minus(image_ras, atlas_ras))
                            } else {
                                None
                            }
                        }
                    }
                    WARPFIELD_DTFMT_DISP_CRS => {
                        let image_crs = plus(v, atlas_crs);
                        if newformat == WARPFIELD_DTFMT_ABS_CRS {
                            Some(image_crs)
                        } else {
                            let image_ras = mul(g, out, err, Some(&src_vox2ras), image_crs)?;
                            if newformat == WARPFIELD_DTFMT_ABS_RAS {
                                Some(image_ras)
                            } else if newformat == WARPFIELD_DTFMT_DISP_RAS {
                                let atlas_ras = mul(g, out, err, Some(&dst_vox2ras), atlas_crs)?;
                                Some(minus(image_ras, atlas_ras))
                            } else {
                                None
                            }
                        }
                    }
                    WARPFIELD_DTFMT_ABS_RAS => {
                        if newformat == WARPFIELD_DTFMT_DISP_RAS {
                            let atlas_ras = mul(g, out, err, Some(&dst_vox2ras), atlas_crs)?;
                            Some(minus(v, atlas_ras))
                        } else {
                            let image_crs = mul(g, out, err, src_ras2vox.as_ref(), v)?;
                            if newformat == WARPFIELD_DTFMT_ABS_CRS {
                                Some(image_crs)
                            } else if newformat == WARPFIELD_DTFMT_DISP_CRS {
                                Some(minus(image_crs, atlas_crs))
                            } else {
                                None
                            }
                        }
                    }
                    _ => {
                        let atlas_ras = mul(g, out, err, Some(&dst_vox2ras), atlas_crs)?;
                        let image_ras = plus(v, atlas_ras);
                        if newformat == WARPFIELD_DTFMT_ABS_RAS {
                            Some(image_ras)
                        } else {
                            let image_crs = mul(g, out, err, src_ras2vox.as_ref(), image_ras)?;
                            if newformat == WARPFIELD_DTFMT_ABS_CRS {
                                Some(image_crs)
                            } else if newformat == WARPFIELD_DTFMT_DISP_CRS {
                                Some(minus(image_crs, atlas_crs))
                            } else {
                                None
                            }
                        }
                    }
                };
                if let Some(nv) = new {
                    for (k, x) in nv.into_iter().enumerate() {
                        mri_set_vox_val(g, err, m, c, r, s, k as i32, x, 0)?;
                    }
                }
            }
        }
    }
    Ok(())
}
