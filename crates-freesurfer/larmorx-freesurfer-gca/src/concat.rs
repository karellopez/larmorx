// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GCA morphs composed with each other and with linear transforms, and a morph's inverse as a
//! morph (`utils/gcamorph.cpp` of FreeSurfer 8.2.0; track A's relay 43 of FreeSurfer's
//! foundations): `GCAMconcat2` (16269), `GCAMconcat3` (16343), `GCAMfillInverse` (16462) and
//! `GCAMfreeInverse` (7647). `TransformConcat` and `TransformInvertReplace` (`crate::transform`) go
//! through them; `mri_warp_convert` calls `GCAMconcat3` itself.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 39): FS-934 (a linear
//! transform array whose transforms do not chain: a crash), FS-935 (a morph of spacing 0 composed
//! with a linear transform: a division by zero), FS-936 (a morph inverted without its inverse and
//! without a readable image volume: no morph, a crash at its next use), FS-17 (the inverse's node
//! coordinates as `int`s).

use std::io::Write;

use larmorx_core::defects::{FS_934, FS_935, FS_936};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply_d};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, Lta, lta_alloc};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::ltaops::{
    InvertInto, lta_change_type, lta_fill_inverse, lta_invert, lta_reduce,
};
use larmorx_freesurfer_volume::ltawrite::lta_dump_vol_geom;
use larmorx_freesurfer_volume::vgeom::{vg_is_equal, vg_is_not_equal_thresh, vg_print};

use crate::gcam::{
    GCAM_POSITION_INVALID, GCAM_RAS, GCAM_VOX, Gcam, gcam_alloc, gcam_invert, gcam_ras_to_vox,
};
use crate::transform::gcam_sample_morph;

/// `GCAMfreeInverse(gcam)`: the inverse's index volumes dropped; `NO_ERROR`.
pub fn gcam_free_inverse(gcam: &mut Gcam) -> i32 {
    gcam.mri_xind = None;
    gcam.mri_yind = None;
    gcam.mri_zind = None;
    NO_ERROR
}

/// A morph whose positions are in RAS taken to voxels, after `<what>(): converting ... from
/// GCAM_RAS to GCAM_VOX` (standard output).
fn to_vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
    line: &str,
) -> Result<(), Fatal> {
    if gcam.mtype == GCAM_RAS {
        let _ = writeln!(out, "{line}");
        gcam_ras_to_vox(g, out, err, env, gcam, None)?;
    }
    Ok(())
}

/// What a node of a composed morph becomes where its point falls outside the morph sampled
/// (`GCAM_POSITION_INVALID`, its current and original positions 0: the reader's mark of a node
/// without a position).
fn invalidate(n: &mut crate::gcam::GcamNode) {
    n.invalid = GCAM_POSITION_INVALID;
    n.x = 0.0;
    n.y = 0.0;
    n.z = 0.0;
    n.origx = 0.0;
    n.origy = 0.0;
    n.origz = 0.0;
}

/// `GCAMconcat2(gcam1, gcam2, gcam2)`: the morph `source -> gcam1 -> gcam2 -> atlas` written into
/// `gcam2` (`TransformConcat`'s call). `gcam1`'s atlas must be `gcam2`'s image (`vg_isEqual`;
/// `ERROR: GCAMconcat2(): geometry does not match`, the program ends). Morphs in RAS are taken to
/// voxels first (`GCAMconcat2(): converting GCAM 1 from GCAM_RAS to GCAM_VOX`, `... GCAM 2 ...`).
/// The output's image becomes `gcam1`'s (`VOL_GEOM`'s assignment), its inverse is dropped, its
/// spacing stays `gcam2`'s and its type `GCAM_VOX`. Each node of `gcam2` (x slowest): its position
/// (`float`s) sampled through `gcam1` ([`gcam_sample_morph`]): outside it the node becomes
/// `GCAM_POSITION_INVALID` with zero positions; else the sample is its position, its node
/// coordinates its own, its original position its coordinates times the spacing, its label kept.
pub fn gcam_concat2_into_second(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam1: &mut Gcam,
    gcam2: &mut Gcam,
) -> Result<(), Fatal> {
    if vg_is_equal(g, &gcam1.atlas, &gcam2.image) == 0 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "ERROR: GCAMconcat2(): geometry does not match",
        )?;
    }
    to_vox(
        g,
        out,
        err,
        env,
        gcam1,
        "GCAMconcat2(): converting GCAM 1 from GCAM_RAS to GCAM_VOX",
    )?;
    to_vox(
        g,
        out,
        err,
        env,
        gcam2,
        "GCAMconcat2(): converting GCAM 2 from GCAM_RAS to GCAM_VOX",
    )?;
    gcam2.image.assign(g, &gcam1.image);
    gcam_free_inverse(gcam2);
    gcam2.mtype = GCAM_VOX;
    let sp = gcam2.spacing;
    for c in 0..gcam2.width.max(0) {
        for r in 0..gcam2.height.max(0) {
            for s in 0..gcam2.depth.max(0) {
                let n2 = gcam2.node(c, r, s);
                let (xd, yd, zd) = (n2.x as f32, n2.y as f32, n2.z as f32);
                let (code, [xdd, ydd, zdd]) = gcam_sample_morph(g, err, gcam1, xd, yd, zd)?;
                let n = gcam2.node_mut(c, r, s);
                if code != NO_ERROR {
                    invalidate(n);
                    continue;
                }
                n.x = f64::from(xdd);
                n.y = f64::from(ydd);
                n.z = f64::from(zdd);
                n.xn = c;
                n.yn = r;
                n.zn = s;
                n.origx = f64::from(c.wrapping_mul(sp));
                n.origy = f64::from(r.wrapping_mul(sp));
                n.origz = f64::from(s.wrapping_mul(sp));
            }
        }
    }
    Ok(())
}

/// `LTAreduce(lta)` for `GCAMconcat3`: the array reduced to one transform; FS-934: transforms that
/// do not chain (`ERROR: LTAreduce(): LTs <n-1> and <n> do not match`) give FreeSurfer no array,
/// which it then reads (a crash): the program ends in both modes.
fn reduce(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &Lta,
    which: i32,
) -> Result<Lta, Fatal> {
    match lta_reduce(g, out, err, lta)? {
        Some(l) => Ok(l),
        None => {
            g.notices.add(
                &FS_934,
                format!(
                    "GCAMconcat3: LTA {which} whose transforms do not chain (FreeSurfer reads the array it did not get and crashes)"
                ),
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "GCAMconcat3(): LTA {which} could not be reduced (FreeSurfer crashes here)"
                ),
            )?;
            Err(Fatal::Exit(139))
        }
    }
}

/// `width / spacing` of `GCAMconcat3` (`int` division); FS-935: a spacing of 0 (or the division
/// that overflows) is a division fault in FreeSurfer: the program ends in both modes.
fn per_node(g: &mut Globals, err: &mut dyn Write, n: i32, sp: i32) -> Result<i32, Fatal> {
    if sp == 0 || (n == i32::MIN && sp == -1) {
        g.notices.add(
            &FS_935,
            format!(
                "GCAMconcat3: the atlas extent {n} divided by the morph's spacing {sp} (a division fault in FreeSurfer)"
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "GCAMconcat3(): the morph's spacing is {sp}: FreeSurfer divides by it and crashes here"
            ),
        )?;
        return Err(Fatal::Exit(136));
    }
    Ok(n / sp)
}

/// `GCAMconcat3(lta1, gcam, lta2, NULL)`: the morph `source -> lta1 -> gcam -> lta2 -> atlas` as a
/// new morph. `lta2` (reduced to one transform, `LTAreduce`; FS-934) must start at `gcam`'s atlas
/// (`vg_isEqual`), or end there (`WARNING: GCAMconcat3(): inverting LTA 2 to match geometry`, then
/// inverted); else `Atlas volume geom <threshold %g> (<differs from its source>, <from its
/// destination>)`, the atlas geometry (`vg_print`, standard error) and `ERROR: GCAMconcat3(): LTA 2
/// geometry does not match atlas src or dst` end the program; without `lta2` the identity on the
/// atlas. `lta1` likewise must end at `gcam`'s image, or start there (`WARNING: GCAMconcat3():
/// inverting LTA 1 to match geometry`); else `gcam->image vol geom`, the image geometry
/// (`LTAdumpVolGeom`) and `ERROR: GCAMconcat3(): LTA 1 geometry does not match`; without it the
/// identity on the image. Both become voxel to voxel with their inverses filled. The new morph has
/// `lta2`'s destination extent divided by `gcam`'s spacing (`int`; FS-935) along each axis
/// (`GCAMalloc`), `lta1`'s source as image, `lta2`'s destination as atlas, `gcam`'s spacing, type
/// `GCAM_VOX` (`gcam` in RAS is taken to voxels first: `GCAMconcat3(): converting from GCAM_RAS to
/// GCAM_VOX`). Each node `(c, r, s)` (c slowest): its original position `(c, r, s)` times the
/// spacing (`float`), through `lta2`'s inverse (`MatrixMultiplyD`), sampled through `gcam`
/// ([`gcam_sample_morph`]: outside, the node `GCAM_POSITION_INVALID` with zero positions), through
/// `lta1`'s inverse: its position; its node coordinates its own.
#[allow(clippy::too_many_arguments)]
pub fn gcam_concat3(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    lta1: Option<&Lta>,
    gcam: &mut Gcam,
    lta2: Option<&Lta>,
) -> Result<Gcam, Fatal> {
    let mut l2 = match lta2 {
        Some(l) => {
            let mut r = reduce(g, out, err, l, 2)?;
            if vg_is_equal(g, &gcam.atlas, &r.xforms[0].src) == 0 {
                if vg_is_equal(g, &gcam.atlas, &r.xforms[0].dst) != 0 {
                    let _ = writeln!(
                        out,
                        "WARNING: GCAMconcat3(): inverting LTA 2 to match geometry"
                    );
                    lta_invert(g, out, err, InvertInto::Same(&mut r))?;
                } else {
                    let t = g.vg_is_equal_threshold;
                    let a = vg_is_not_equal_thresh(&gcam.atlas, &r.xforms[0].src, t);
                    let b = vg_is_not_equal_thresh(&gcam.atlas, &r.xforms[0].dst, t);
                    let _ = writeln!(
                        out,
                        "Atlas volume geom {} ({a}, {b}) ",
                        larmorx_core::cfmt::g(t, 6)
                    );
                    vg_print(err, &gcam.atlas);
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        "ERROR: GCAMconcat3(): LTA 2 geometry does not match atlas src or dst",
                    )?;
                }
            }
            r
        }
        None => {
            // `src = dst = gcam->atlas`: the destination assigned first, the source from it
            let mut r = lta_alloc(g, out, err, 1, None)?;
            r.xforms[0].dst.assign(g, &gcam.atlas);
            let dst = r.xforms[0].dst.clone();
            r.xforms[0].src.assign(g, &dst);
            r
        }
    };
    let mut l1 = match lta1 {
        Some(l) => {
            let mut r = reduce(g, out, err, l, 1)?;
            if vg_is_equal(g, &r.xforms[0].dst, &gcam.image) == 0 {
                if vg_is_equal(g, &r.xforms[0].src, &gcam.image) != 0 {
                    let _ = writeln!(
                        out,
                        "WARNING: GCAMconcat3(): inverting LTA 1 to match geometry"
                    );
                    lta_invert(g, out, err, InvertInto::Same(&mut r))?;
                } else {
                    let _ = writeln!(out, "gcam->image vol geom");
                    lta_dump_vol_geom(out, &gcam.image);
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        "ERROR: GCAMconcat3(): LTA 1 geometry does not match",
                    )?;
                }
            }
            r
        }
        None => {
            // `src = dst = gcam->image`: the destination assigned first, the source from it
            let mut r = lta_alloc(g, out, err, 1, None)?;
            r.xforms[0].dst.assign(g, &gcam.image);
            let dst = r.xforms[0].dst.clone();
            r.xforms[0].src.assign(g, &dst);
            r
        }
    };
    lta_change_type(g, out, err, env, &mut l2, LINEAR_VOX_TO_VOX)?;
    lta_change_type(g, out, err, env, &mut l1, LINEAR_VOX_TO_VOX)?;
    lta_fill_inverse(g, out, err, &mut l2)?;
    lta_fill_inverse(g, out, err, &mut l1)?;
    let sp = gcam.spacing;
    let width = per_node(g, err, l2.xforms[0].dst.width, sp)?;
    let height = per_node(g, err, l2.xforms[0].dst.height, sp)?;
    let depth = per_node(g, err, l2.xforms[0].dst.depth, sp)?;
    let mut o = gcam_alloc(g, err, width, height, depth)?;
    to_vox(
        g,
        out,
        err,
        env,
        gcam,
        "GCAMconcat3(): converting from GCAM_RAS to GCAM_VOX",
    )?;
    o.image.assign(g, &l1.xforms[0].src);
    o.atlas.assign(g, &l2.xforms[0].dst);
    gcam_free_inverse(&mut o);
    o.spacing = sp;
    o.mtype = GCAM_VOX;
    let mut orig = Matrix::real(4, 1);
    let mut v = Matrix::real(4, 1);
    let mut w = Matrix::real(4, 1);
    orig.set_relt(4, 1, 1.0);
    w.set_relt(4, 1, 1.0);
    let spf = sp as f32;
    for c in 0..width.max(0) {
        for r in 0..height.max(0) {
            for s in 0..depth.max(0) {
                orig.set_relt(1, 1, sse::mul(c as f32, spf));
                orig.set_relt(2, 1, sse::mul(r as f32, spf));
                orig.set_relt(3, 1, sse::mul(s as f32, spf));
                let m2 = l2.inv_xforms[0].m_l.clone();
                matrix_multiply_d(g, out, err, m2.as_ref(), Some(&orig), Some(&mut v))?;
                let (code, p) =
                    gcam_sample_morph(g, err, gcam, v.relt(1, 1), v.relt(2, 1), v.relt(3, 1))?;
                if code != NO_ERROR {
                    invalidate(o.node_mut(c, r, s));
                    continue;
                }
                w.set_relt(1, 1, p[0]);
                w.set_relt(2, 1, p[1]);
                w.set_relt(3, 1, p[2]);
                let m1 = l1.inv_xforms[0].m_l.clone();
                matrix_multiply_d(g, out, err, m1.as_ref(), Some(&w), Some(&mut v))?;
                let n = o.node_mut(c, r, s);
                n.xn = c;
                n.yn = r;
                n.zn = s;
                n.x = f64::from(v.relt(1, 1));
                n.y = f64::from(v.relt(2, 1));
                n.z = f64::from(v.relt(3, 1));
                n.origx = f64::from(orig.relt(1, 1));
                n.origy = f64::from(orig.relt(2, 1));
                n.origz = f64::from(orig.relt(3, 1));
            }
        }
    }
    Ok(o)
}

/// `GCAMfillInverse(gcam)`: the morph's inverse as a new morph of the image's extent
/// (`Allocating inv_gcam...(<w>, <h>, <d>)`, `GCAMalloc`), whose image is `gcam`'s atlas and atlas
/// `gcam`'s image. Without its inverse's index volumes, `gcam` is inverted on the header of the
/// volume its image geometry names (`MRIreadHeader`; `GCAMfillInverse: Must invert gcam
/// explicitely! `, [`gcam_invert`]); a name that cannot be read gives `ERROR: reading <name>` and
/// no morph (FS-936 in the caller). Each node `(x, y, z)` of the image grid: its original and
/// current positions the index volumes' values there, its node coordinates the same as `int`s
/// (`(int)` of the `float`: FS-17).
pub fn gcam_fill_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&std::path::Path>,
    gcam: &mut Gcam,
) -> Result<Option<Gcam>, Fatal> {
    let (width, height, depth) = (gcam.image.width, gcam.image.height, gcam.image.depth);
    let _ = writeln!(out, "Allocating inv_gcam...({width}, {height}, {depth})");
    let mut inv = gcam_alloc(g, err, width, height, depth)?;
    inv.image.assign(g, &gcam.atlas);
    inv.atlas.assign(g, &gcam.image);
    if gcam.mri_xind.is_none() || gcam.mri_yind.is_none() || gcam.mri_zind.is_none() {
        let name = String::from_utf8_lossy(gcam.image.fname.c_bytes()).into_owned();
        let Some(mri) =
            mriio::mri_read_header(g, out, err, env, dir, &name, MRI_VOLUME_TYPE_UNKNOWN)?
        else {
            let _ = writeln!(out, "ERROR: reading {name}");
            return Ok(None);
        };
        let _ = writeln!(out, "GCAMfillInverse: Must invert gcam explicitely! ");
        gcam_invert(g, out, err, env, dir, gcam, Some(&mri))?;
    }
    let (xi, yi, zi) = (
        gcam.mri_xind.as_ref().expect("the inverse"),
        gcam.mri_yind.as_ref().expect("the inverse"),
        gcam.mri_zind.as_ref().expect("the inverse"),
    );
    for x in 0..width.max(0) {
        for y in 0..height.max(0) {
            for z in 0..depth.max(0) {
                let a = mri_get_vox_val(g, err, xi, x, y, z, 0, 0)?;
                let b = mri_get_vox_val(g, err, yi, x, y, z, 0, 0)?;
                let c = mri_get_vox_val(g, err, zi, x, y, z, 0, 0)?;
                let xn = float_to_int(g, f64::from(a), "GCAMfillInverse");
                let yn = float_to_int(g, f64::from(b), "GCAMfillInverse");
                let zn = float_to_int(g, f64::from(c), "GCAMfillInverse");
                let n = inv.node_mut(x, y, z);
                n.origx = f64::from(a);
                n.origy = f64::from(b);
                n.origz = f64::from(c);
                n.x = f64::from(a);
                n.y = f64::from(b);
                n.z = f64::from(c);
                n.xn = xn;
                n.yn = yn;
                n.zn = zn;
            }
        }
    }
    Ok(Some(inv))
}

/// FS-936: `TransformInvertReplace` of a morph whose inverse could not be made (no index volumes
/// and no readable image volume): FreeSurfer leaves the transform without a morph, which its next
/// use reads (a crash): the program ends here in both modes.
pub(crate) fn no_inverse(g: &mut Globals, err: &mut dyn Write) -> Result<(), Fatal> {
    g.notices.add(
        &FS_936,
        "TransformInvertReplace of a morph whose inverse could not be made: FreeSurfer keeps no morph and crashes at its next use",
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        "TransformInvertReplace: the morph's inverse could not be made (FreeSurfer crashes at the transform's next use)",
    )
}
