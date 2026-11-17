// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Writing a GCA morph (track A's relay 39 of FreeSurfer's foundations): `MRIwriteType`'s branch of
//! type `MGH_MORPH` (mriio.cpp 1179-1190), with `GCAMinitVolGeom` (gcamorph.cpp 1383), `GCAMinit`
//! without an atlas (1417), `GCAMreadWarpFromMRI` (15111), `GCAMwrite` (352), `__m3zWrite` (374),
//! `__warpfieldWrite` (365) and the parts of `Warpfield::convert` (warpfield.cpp 132),
//! `Warpfield::setWarp` (590) and `Warpfield::write` (567) it reaches.
//!
//! A three-frame volume written under an `.m3z` or `.m3d` name (or with the type `MGH_MORPH`)
//! becomes a morph of the volume's grid: each node at its own voxel (through the identity voxel
//! transform), displaced by the volume's three frames, with the volume's geometry as both the
//! image's and the atlas's; then `GCAMwrite`, which writes `.m3z`/`.m3d` names as the morph format
//! (compressed when the name contains `.m3z`) and `.mgz`/`.nii` names as a warp volume (absolute
//! voxel coordinates, `Warpfield::convert`).
//!
//! Defect FS-856 (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 34): the branch
//! reports success whatever the writer did.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_856;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_print};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_io::fstagsio::vol_geom_write;
use larmorx_freesurfer_io::{fio, identify, mriio, tags};
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, MORPH_3D_TYPE};
use larmorx_freesurfer_mri::mri::{
    MGH_VERSION, MGZ_INTENT_UNKNOWN, MGZ_INTENT_WARPMAP, MRI_FLOAT, Mri, mri_alloc,
    mri_alloc_sequence, mri_copy_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::tkreg::{get_ras2vox, get_vox2ras};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::sample::{SAMPLE_TRILINEAR, sample_volume_frame_type};
use larmorx_freesurfer_volume::vgeom::get_vol_geom;
use larmorx_io::znz::ZnzFile;

use crate::gcam::{
    GCAM_AREA_INVALID, GCAM_RAS, GCAM_VALID, GCAM_VERSION, GCAM_VOX, Gcam, ORIGINAL_POSITIONS,
    SAVED_ORIGINAL_POSITIONS, gcam_alloc, gcam_compute_metric_properties,
    gcam_compute_original_properties, gcam_copy_node_positions, gcam_ras_to_vox,
};
use crate::transform::{transform_alloc, transform_free, transform_invert, transform_sample};

/// `WarpfieldDTFMT`'s formats.
pub const WARPFIELD_DTFMT_ABS_CRS: i32 = 0;
pub const WARPFIELD_DTFMT_DISP_CRS: i32 = 1;
pub const WARPFIELD_DTFMT_ABS_RAS: i32 = 2;
pub const WARPFIELD_DTFMT_DISP_RAS: i32 = 3;

/// `GCAMinitVolGeom(gcam, mri_image, mri_atlas)`: the image geometry from `mri_image`, the atlas
/// geometry from `mri_atlas` (`getVolGeom`). `NO_ERROR`.
pub fn gcam_init_vol_geom(gcam: &mut Gcam, mri_image: &Mri, mri_atlas: &Mri) -> i32 {
    get_vol_geom(&mri_image.vg, &mut gcam.image);
    get_vol_geom(&mri_atlas.vg, &mut gcam.atlas);
    // both geometries given now (FS-855's memory contents gone)
    gcam.geom_unset = false;
    NO_ERROR
}

/// `GCAMinit(gcam, mri_image, NULL, NULL, 0)`: a morph without an atlas, through the identity
/// voxel transform `TransformAlloc(LINEAR_VOX_TO_VOX, mri_image)` (inverted): the inputs are the
/// image's frames, the image geometry the image's; each node `(x, y, z)` valid, its node
/// coordinates its own, its original and current positions the transform of its voxel (`float`);
/// spacing 1; the affine matrix a copy of the transform's and `det` its determinant; the metric
/// properties, each node's original areas its areas (a valid node with neither area above 0
/// becomes `GCAM_AREA_INVALID`); the inverse index volumes on the image's grid (the transform's
/// sample of each voxel, over the spacing); the original positions saved, the original properties
/// and the metric properties again; type `GCAM_VOX`. With `Gx`, `Gy`, `Gz` at a node,
/// `node(<x>,<y>,<z>) --> MRI (<ox>, <oy>, <oz>)` on standard output. `NO_ERROR`.
pub fn gcam_init_no_gca(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    gcam: &mut Gcam,
    mri_image: &Mri,
) -> Result<i32, Fatal> {
    let mut transform = transform_alloc(g, out, err, LINEAR_VOX_TO_VOX, Some(mri_image))?
        .expect("a linear transform");
    gcam.ninputs = mri_image.nframes;
    get_vol_geom(&mri_image.vg, &mut gcam.image);
    let (width, height, depth) = (gcam.width, gcam.height, gcam.depth);
    transform_invert(g, out, err, env, dir, &mut transform, Some(mri_image))?;
    let m_l = transform.lta().xforms[0].m_l.clone();
    let mut v1 = Matrix::real(4, 1);
    v1.set_relt(4, 1, 1.0);
    let mut v2 = v1.clone();
    for x in 0..width.max(0) {
        v1.set_relt(1, 1, x as f32);
        for y in 0..height.max(0) {
            v1.set_relt(2, 1, y as f32);
            for z in 0..depth.max(0) {
                v1.set_relt(3, 1, z as f32);
                matrix_multiply(g, out, err, m_l.as_ref(), Some(&v1), Some(&mut v2))?;
                let (ox, oy, oz) = (v2.relt(1, 1), v2.relt(2, 1), v2.relt(3, 1));
                let n = gcam.node_mut(x, y, z);
                n.xn = x;
                n.yn = y;
                n.zn = z;
                n.invalid = GCAM_VALID;
                n.origx = f64::from(ox);
                n.x = n.origx;
                n.origy = f64::from(oy);
                n.y = n.origy;
                n.origz = f64::from(oz);
                n.z = n.origz;
                n.log_p = 0.0;
                if x == g.gx && y == g.gy && z == g.gz {
                    let _ = writeln!(
                        out,
                        "node({x},{y},{z}) --> MRI ({}, {}, {})",
                        fmt_f(f64::from(ox), 2, 1),
                        fmt_f(f64::from(oy), 2, 1),
                        fmt_f(f64::from(oz), 2, 1)
                    );
                }
            }
        }
    }
    gcam.spacing = 1;
    if transform.ttype != MORPH_3D_TYPE {
        let a = matrix_copy(g, out, err, m_l.as_ref(), None)?.into_new();
        gcam.det = a
            .as_ref()
            .map_or(0.0, |m| f64::from(matrix_determinant(g, err, m)));
        gcam.m_affine = a;
    }
    gcam_compute_metric_properties(g, out, err, env, gcam)?;
    for n in &mut gcam.nodes {
        n.orig_area = n.area;
        n.orig_area1 = n.area1;
        n.orig_area2 = n.area2;
        if n.orig_area1 <= 0.0 && n.orig_area2 <= 0.0 && n.invalid == GCAM_VALID {
            n.invalid = GCAM_AREA_INVALID;
        }
    }
    let (iw, ih, id) = (mri_image.vg.width, mri_image.vg.height, mri_image.vg.depth);
    let mut xind = mri_alloc(g, out, err, iw, ih, id, MRI_FLOAT)?;
    let mut yind = mri_alloc(g, out, err, iw, ih, id, MRI_FLOAT)?;
    let mut zind = mri_alloc(g, out, err, iw, ih, id, MRI_FLOAT)?;
    gcam.mri_xind = None;
    gcam.mri_yind = None;
    gcam.mri_zind = None;
    let sp = gcam.spacing as f32;
    for x in 0..iw.max(0) {
        for y in 0..ih.max(0) {
            for z in 0..id.max(0) {
                let (_, [ox, oy, oz]) = transform_sample(
                    g,
                    out,
                    err,
                    env,
                    &mut transform,
                    x as f32,
                    y as f32,
                    z as f32,
                )?;
                mri_set_vox_val(g, err, &mut xind, x, y, z, 0, sse::div(ox, sp), 0)?;
                mri_set_vox_val(g, err, &mut yind, x, y, z, 0, sse::div(oy, sp), 0)?;
                mri_set_vox_val(g, err, &mut zind, x, y, z, 0, sse::div(oz, sp), 0)?;
            }
        }
    }
    gcam.mri_xind = Some(xind);
    gcam.mri_yind = Some(yind);
    gcam.mri_zind = Some(zind);
    gcam_copy_node_positions(g, err, gcam, ORIGINAL_POSITIONS, SAVED_ORIGINAL_POSITIONS);
    gcam_compute_original_properties(g, out, err, env, gcam)?;
    gcam_compute_metric_properties(g, out, err, env, gcam)?;
    gcam.mtype = GCAM_VOX;
    let mut t = Some(transform);
    transform_free(g, &mut t);
    Ok(NO_ERROR)
}

/// `GCAMreadWarpFromMRI(gcam, mri_warp, DeformationFlag)`: a warp volume of the morph's grid
/// (`GCAMreadWarpFromMRI(): warp and gcam have the same dims` on standard output) gives each node
/// its voxel's three frames; another grid (`... do not have the same dims`) the frames' samples at
/// the node's voxel times the spacing (`MRIsampleVolumeFrameType`, trilinear). With the flag the
/// values are displacements added to the original positions (`double`), else the positions.
/// `NO_ERROR`.
pub fn gcam_read_warp_from_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gcam: &mut Gcam,
    mri_warp: &Mri,
    deformation: bool,
) -> Result<i32, Fatal> {
    let set = |n: &mut crate::gcam::GcamNode, d: [f64; 3]| {
        if deformation {
            n.x = sse::add64(n.origx, d[0]);
            n.y = sse::add64(n.origy, d[1]);
            n.z = sse::add64(n.origz, d[2]);
        } else {
            (n.x, n.y, n.z) = (d[0], d[1], d[2]);
        }
    };
    let (w, h, d) = (gcam.width, gcam.height, gcam.depth);
    if w == mri_warp.vg.width && h == mri_warp.vg.height && d == mri_warp.vg.depth {
        let _ = out.write_all(b"GCAMreadWarpFromMRI(): warp and gcam have the same dims\n");
        for xp in 0..w.max(0) {
            for yp in 0..h.max(0) {
                for zp in 0..d.max(0) {
                    let mut v = [0f64; 3];
                    for (f, slot) in v.iter_mut().enumerate() {
                        *slot =
                            f64::from(mri_get_vox_val(g, err, mri_warp, xp, yp, zp, f as i32, 0)?);
                    }
                    set(gcam.node_mut(xp, yp, zp), v);
                }
            }
        }
        return Ok(NO_ERROR);
    }
    let _ = out.write_all(b"GCAMreadWarpFromMRI(): warp and gcam do not have the same dims\n");
    let sp = gcam.spacing;
    for xp in 0..w.max(0) {
        let xv = xp.wrapping_mul(sp);
        for yp in 0..h.max(0) {
            let yv = yp.wrapping_mul(sp);
            for zp in 0..d.max(0) {
                let zv = zp.wrapping_mul(sp);
                let mut v = [0f64; 3];
                for (f, slot) in v.iter_mut().enumerate() {
                    sample_volume_frame_type(
                        g,
                        err,
                        mri_warp,
                        f64::from(xv),
                        f64::from(yv),
                        f64::from(zv),
                        f as i32,
                        SAMPLE_TRILINEAR,
                        slot,
                    );
                }
                set(gcam.node_mut(xp, yp, zp), v);
            }
        }
    }
    Ok(NO_ERROR)
}

/// `__m3zWrite(gcam, fname)`: the morph format, compressed when the name contains `.m3z`: the
/// version 1.0, the grid's extents, the spacing and `exp_k` (`float`), each node's original and
/// current positions (`float`) and node coordinates, `TAG_GCAMORPH_GEOM` and the image and atlas
/// geometries (`VOL_GEOM::write`, no shears), `TAG_GCAMORPH_TYPE` and the type,
/// `TAG_GCAMORPH_LABELS` and each node's label, and with an affine matrix `TAG_MGH_XFORM` and the
/// matrix (`znzWriteMatrix`, tag 0). A file that cannot be made: `GCAMwrite(<f>): could not open
/// file` (`ERROR_BADPARM`).
pub fn m3z_write(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    gcam: &Gcam,
    fname: &str,
) -> i32 {
    let gzipped = fname.contains(".m3z");
    let path = utils::resolve(dir, fname);
    let Some(mut fp) = ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"wb", gzipped) else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("GCAMwrite({fname}): could not open file"),
        );
        return ERROR_BADPARM;
    };
    let f = &mut fp;
    fio::znz_write_float(f, GCAM_VERSION as f32);
    fio::znz_write_int(f, gcam.width);
    fio::znz_write_int(f, gcam.height);
    fio::znz_write_int(f, gcam.depth);
    fio::znz_write_int(f, gcam.spacing);
    fio::znz_write_float(f, gcam.exp_k as f32);
    for n in &gcam.nodes {
        for v in [n.origx, n.origy, n.origz, n.x, n.y, n.z] {
            fio::znz_write_float(f, v as f32);
        }
        for v in [n.xn, n.yn, n.zn] {
            fio::znz_write_int(f, v);
        }
    }
    fio::znz_write_int(f, tags::TAG_GCAMORPH_GEOM);
    if gcam.geom_unset {
        // relay 42: the geometries' validity and sizes written as memory holds them in FreeSurfer
        crate::gcam::unset_geometry(g, "the morph file's geometry tag");
    }
    vol_geom_write(f, &gcam.image, false, true);
    vol_geom_write(f, &gcam.atlas, false, true);
    fio::znz_write_int(f, tags::TAG_GCAMORPH_TYPE);
    fio::znz_write_int(f, gcam.mtype);
    fio::znz_write_int(f, tags::TAG_GCAMORPH_LABELS);
    for n in &gcam.nodes {
        fio::znz_write_int(f, n.label);
    }
    if let Some(a) = &gcam.m_affine {
        fio::znz_write_int(f, tags::TAG_MGH_XFORM);
        tags::znz_write_matrix(g, f, a, 0);
    }
    fp.close();
    NO_ERROR
}

/// `Warpfield::setWarp(c, r, s, fcs, frs, fss, label)` into the warp volume: the label, and the
/// image voxel `(fcs, frs, fss)` in the volume's format: as it is, as the displacement from
/// `(c, r, s)` (`float`), as its RAS position (the image's voxel-to-RAS matrix, `MatrixMultiplyD`)
/// or as the displacement of that from the atlas voxel's RAS position.
#[allow(clippy::too_many_arguments)]
fn set_warp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    warp: &mut Mri,
    src_vox2ras: &Matrix,
    dst_vox2ras: &Matrix,
    c: i32,
    r: i32,
    s: i32,
    f: [f32; 3],
    label: i32,
) -> Result<(), Fatal> {
    let fmt = warp.warp_field_format;
    let (h, d) = (warp.vg.height as usize, warp.vg.depth as usize);
    if let Some(l) = warp.gcamorph_label.as_mut() {
        l[(c as usize * h + r as usize) * d + s as usize] = label;
    }
    let vals = if fmt == WARPFIELD_DTFMT_ABS_CRS {
        Some(f)
    } else if fmt == WARPFIELD_DTFMT_DISP_CRS {
        Some([
            sse::sub(f[0], c as f32),
            sse::sub(f[1], r as f32),
            sse::sub(f[2], s as f32),
        ])
    } else if fmt == WARPFIELD_DTFMT_ABS_RAS || fmt == WARPFIELD_DTFMT_DISP_RAS {
        let mut image_crs = Matrix::real(4, 1);
        for (k, v) in f.iter().enumerate() {
            image_crs.set_relt(k as i32 + 1, 1, *v);
        }
        image_crs.set_relt(4, 1, 1.0);
        let mut image_ras = Matrix::real(4, 1);
        matrix_multiply_d(
            g,
            out,
            err,
            Some(src_vox2ras),
            Some(&image_crs),
            Some(&mut image_ras),
        )?;
        if fmt == WARPFIELD_DTFMT_ABS_RAS {
            Some([
                image_ras.relt(1, 1),
                image_ras.relt(2, 1),
                image_ras.relt(3, 1),
            ])
        } else {
            let mut atlas_crs = Matrix::real(4, 1);
            atlas_crs.set_relt(1, 1, c as f32);
            atlas_crs.set_relt(2, 1, r as f32);
            atlas_crs.set_relt(3, 1, s as f32);
            atlas_crs.set_relt(4, 1, 1.0);
            let mut atlas_ras = Matrix::real(4, 1);
            matrix_multiply_d(
                g,
                out,
                err,
                Some(dst_vox2ras),
                Some(&atlas_crs),
                Some(&mut atlas_ras),
            )?;
            Some([
                sse::sub(image_ras.relt(1, 1), atlas_ras.relt(1, 1)),
                sse::sub(image_ras.relt(2, 1), atlas_ras.relt(2, 1)),
                sse::sub(image_ras.relt(3, 1), atlas_ras.relt(3, 1)),
            ])
        }
    } else {
        None
    };
    if let Some(v) = vals {
        for (k, x) in v.into_iter().enumerate() {
            mri_set_vox_val(g, err, warp, c, r, s, k as i32, x, 0)?;
        }
    }
    Ok(())
}

/// `MatrixPrint(stdout, m)` (precision 5).
fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: &Matrix) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, Some(m), 5);
    let _ = out.write_all(&buf);
}

/// `Warpfield::convert(gcam, dataformat, 0)`: a morph of type `GCAM_RAS` first converted to voxels
/// (`converting GCAM from GCAM_RAS to GCAM_VOX`, `GCAMrasToVox`); `[INFO] Warpfield::convert():`
/// lines with the grid's and the two geometries' extents; a three-frame float volume of the grid
/// with the atlas geometry, the warp-map intent and version, the format, spacing, `exp_k`, the two
/// geometries and (with `[DEBUG]` lines and the matrix printed twice) the affine matrix; each node's
/// current position set through [`set_warp`] with its label; `[INFO] Warpfield::convert(): total
/// out of range voxel count: 0`.
fn warpfield_convert(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
    dataformat: i32,
) -> Result<Mri, Fatal> {
    if gcam.mtype == GCAM_RAS {
        let _ = out.write_all(b"converting GCAM from GCAM_RAS to GCAM_VOX\n");
        gcam_ras_to_vox(g, out, err, env, gcam, None)?;
    }
    if gcam.geom_unset {
        crate::gcam::unset_geometry(
            g,
            "the morph's printed sizes and its conversion to a warp volume",
        );
    }
    let _ = out.write_all(b"[INFO] Warpfield::convert(): converting GCAM ...\n");
    let _ = writeln!(
        out,
        "[INFO] Warpfield::convert(): gcam       [{} x {} x {}]",
        gcam.width, gcam.height, gcam.depth
    );
    let _ = writeln!(
        out,
        "[INFO] Warpfield::convert(): gcam image [{} x {} x {}]",
        gcam.image.width, gcam.image.height, gcam.image.depth
    );
    let _ = writeln!(
        out,
        "[INFO] Warpfield::convert(): gcam atlas [{} x {} x {}]",
        gcam.atlas.width, gcam.atlas.height, gcam.atlas.depth
    );
    let mut warp = mri_alloc_sequence(
        g,
        out,
        err,
        gcam.width,
        gcam.height,
        gcam.depth,
        MRI_FLOAT,
        3,
    )?;
    mri_copy_vol_geom_to_mri(&mut warp, &gcam.atlas);
    let version = ((MGZ_INTENT_WARPMAP & 0xffff) << 8) | MGH_VERSION;
    warp.intent = MGZ_INTENT_WARPMAP;
    warp.version = version;
    warp.warp_field_format = dataformat;
    warp.gcamorph_spacing = gcam.spacing;
    warp.gcamorph_exp_k = gcam.exp_k;
    warp.gcamorph_image_vg.assign(g, &gcam.image);
    warp.gcamorph_atlas_vg.assign(g, &gcam.atlas);
    if let Some(a) = &gcam.m_affine {
        let _ = writeln!(
            out,
            "[DEBUG] Warpfield::convert() gcam->m_affine (spacing={}, exp-k={}, det={}):",
            gcam.spacing,
            fmt_f(gcam.exp_k, 0, 2),
            fmt_f(gcam.det, 0, 2)
        );
        print_matrix(g, out, err, a);
        let c = matrix_copy(g, out, err, Some(a), None)?.into_new();
        let _ = writeln!(
            out,
            "[DEBUG] Warpfield::convert() __warpmap->gcamorphAffine (spacing={}, exp-k={})",
            warp.gcamorph_spacing,
            fmt_f(warp.gcamorph_exp_k, 0, 2)
        );
        if let Some(c) = &c {
            print_matrix(g, out, err, c);
        }
        warp.gcamorph_affine = c;
    }
    warp.init_gcamorph_label();
    let _ = get_ras2vox(g, out, err, &gcam.image, 0, false)?;
    let src_vox2ras = get_vox2ras(g, out, err, &gcam.image, 0, false)?;
    let _ = get_ras2vox(g, out, err, &gcam.atlas, 0, false)?;
    let dst_vox2ras = get_vox2ras(g, out, err, &gcam.atlas, 0, false)?;
    for c in 0..warp.vg.width.max(0) {
        for r in 0..warp.vg.height.max(0) {
            for s in 0..warp.vg.depth.max(0) {
                let n = gcam.node(c, r, s);
                let f = [n.x as f32, n.y as f32, n.z as f32];
                let label = n.label;
                set_warp(
                    g,
                    out,
                    err,
                    &mut warp,
                    &src_vox2ras,
                    &dst_vox2ras,
                    c,
                    r,
                    s,
                    f,
                    label,
                )?;
            }
        }
    }
    let _ = out.write_all(b"[INFO] Warpfield::convert(): total out of range voxel count: 0\n");
    Ok(warp)
}

/// `__warpfieldWrite(gcam, fname, datainterp)`: [`warpfield_convert`], then `Warpfield::write`:
/// `MRIwrite` of the warp volume (`ERROR: Warpfield::write(<f>)` on standard output when it
/// fails). FreeSurfer returns 0 whatever the write did; the write's status here (FS-856's caller
/// chooses).
#[allow(clippy::too_many_arguments)]
fn warpfield_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    gcam: &mut Gcam,
    fname: &str,
    datainterp: i32,
) -> Result<i32, Fatal> {
    let mut warp = warpfield_convert(g, out, err, env, gcam, datainterp)?;
    let ret = mriio::mri_write(
        g,
        out,
        err,
        env,
        dir,
        &mut warp,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    if ret != 0 {
        let _ = writeln!(out, "ERROR: Warpfield::write({fname})");
    }
    Ok(ret)
}

/// What `GCAMwrite` did: its status as FreeSurfer returns it, and the writer's own status (the warp
/// volume's write, which FreeSurfer drops).
struct Written {
    status: i32,
    writer: i32,
    none: bool,
}

/// `GCAMwrite(gcam, fname, datainterp)`: `GCAMwrite(<f>)` on standard output; an `.m3z`/`.m3d`
/// name (`mri_identify`'s `MGH_MORPH`) [`m3z_write`], an `.mgz`/`.mgh` or `.nii` name the warp
/// volume ([`warpfield_write`]: status 0), any other `ERROR_BADPARM` (nothing written, no
/// message).
#[allow(clippy::too_many_arguments)]
fn gcam_write_inner(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    gcam: &mut Gcam,
    fname: &str,
    datainterp: i32,
) -> Result<Written, Fatal> {
    let _ = writeln!(out, "GCAMwrite({fname})");
    let ty = identify::mri_identify(g, dir, env, out, err, fname)?;
    if ty == identify::MGH_MORPH {
        let r = m3z_write(g, err, dir, gcam, fname);
        return Ok(Written {
            status: r,
            writer: r,
            none: false,
        });
    }
    if ty == identify::MRI_MGH_FILE || ty == identify::NII_FILE {
        let r = warpfield_write(g, out, err, env, dir, gcam, fname, datainterp)?;
        return Ok(Written {
            status: 0,
            writer: r,
            none: false,
        });
    }
    Ok(Written {
        status: ERROR_BADPARM,
        writer: ERROR_BADPARM,
        none: true,
    })
}

/// `GCAMwrite(gcam, fname, datainterp)` ([`gcam_write_inner`]): FreeSurfer's status (0 for a warp
/// volume whatever its write did).
#[allow(clippy::too_many_arguments)]
pub fn gcam_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    gcam: &mut Gcam,
    fname: &str,
    datainterp: i32,
) -> Result<i32, Fatal> {
    Ok(gcam_write_inner(g, out, err, env, dir, gcam, fname, datainterp)?.status)
}

/// `MRIwriteType(mri, fname, MGH_MORPH)`'s branch (after its three-frame check): a morph of the
/// volume's grid (`GCAMalloc`), both geometries the volume's (`GCAMinitVolGeom`), `GCAMinit`
/// without an atlas ([`gcam_init_no_gca`]), the volume's frames read as displacements
/// ([`gcam_read_warp_from_mri`]), then [`gcam_write`] with absolute voxel coordinates.
/// FreeSurfer's branch returns `NO_ERROR` whatever `GCAMwrite` did (FS-856): by default the
/// writer's status, with `MRIwriteType(): GCAMwrite(<f>) wrote nothing: the name is not .m3z,
/// .m3d, .mgz or .nii` (`ERROR_BADPARM`) for a name it does not write; `reproduce` returns 0.
pub fn write_morph(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    mri: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    let mut gcam = gcam_alloc(g, err, mri.vg.width, mri.vg.height, mri.vg.depth)?;
    gcam_init_vol_geom(&mut gcam, mri, mri);
    gcam_init_no_gca(g, out, err, env, dir, &mut gcam, mri)?;
    gcam_read_warp_from_mri(g, out, err, &mut gcam, mri, true)?;
    let w = gcam_write_inner(
        g,
        out,
        err,
        env,
        dir,
        &mut gcam,
        fname,
        WARPFIELD_DTFMT_ABS_CRS,
    )?;
    drop(gcam);
    if w.writer == NO_ERROR {
        return Ok(NO_ERROR);
    }
    g.notices.add(
        &FS_856,
        if g.notices.reproduce() {
            "MRIwriteType of a GCA morph whose writer failed or wrote nothing: success reported, as in FreeSurfer"
        } else {
            "MRIwriteType of a GCA morph whose writer failed or wrote nothing: its status returned (FreeSurfer reports success)"
        },
    );
    if g.notices.reproduce() {
        return Ok(NO_ERROR);
    }
    if w.none {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIwriteType(): GCAMwrite({fname}) wrote nothing: the name is not .m3z, .m3d, .mgz or .nii"
            ),
        );
    }
    Ok(w.writer)
}

/// Registers [`write_morph`] with fs.io, whose `MRIwriteType` writes morphs through it.
pub fn register_io() {
    larmorx_freesurfer_io::upper::register(|u| {
        u.write_morph = Some(write_morph);
    });
}
