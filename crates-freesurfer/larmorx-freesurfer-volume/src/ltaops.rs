// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Operations on a linear transform array (`utils/transform.cpp`, 3.1-40, on `fs.mri`'s
//! [`Lta`]): `LTAfillInverse` (3568), `LTAinvert` (3530), `LTAconcat` (3396) with `LTAconcat2`
//! (3378), `LTAreduce` (3469), `LTAchangeType` (3695) with `LTAgetV2V` and `LTAgetR2R`,
//! `LTAmriIsSource` (4908), `LTAmriIsTarget` (4921) and `LTAprint` (3238).
//!
//! The products are FreeSurfer's `MatrixMultiply` (`float`), the inverses its `MatrixInverse`;
//! the geometries are compared with `vg_isEqual` ([`crate::vgeom::vg_is_equal`]). A transform's
//! assignment (`LT = LT`, C++'s implicit copy) copies the numbers and the matrices' pointers and
//! assigns the two geometries ([`VolGeom::assign`], FS-59).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §22):
//! - **FS-504** (memory, crash): no array, an array without transforms or a transform without a
//!   4 x 4 matrix where FreeSurfer reads one: an error in both modes.
//! - **FS-505** (rule A): `LTAfillInverse` into an inverse without a matrix: FreeSurfer's inverse
//!   is lost; by default it is kept.
//! - **FS-506** (rule A): `LTAchangeType` from `LINEAR_PHYSVOX_TO_PHYSVOX` to `LINEAR_RAS_TO_RAS`
//!   (and to `REGISTER_DAT` and `FSLREG_TYPE` through it): FreeSurfer converts the matrix as a
//!   RAS-to-RAS one to voxels and then scales it; by default the physical-voxel matrix is scaled
//!   to voxels and then converted to RAS.
//! - **FS-507** (rule A): `LTAchangeType` from `REGISTER_DAT` to `LINEAR_RAS_TO_RAS` converts the
//!   first transform once per transform; by default each transform once.
//! - **FS-508** (memory): a change to or from `FSLREG_TYPE` whose FSL conversion gives no matrix:
//!   FreeSurfer frees the transform's matrix and goes on using it; an error in both modes.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_504, FS_505, FS_506, FS_507, FS_508};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::lta::{
    FSLREG_TYPE, LINEAR_CORONAL_RAS_TO_CORONAL_RAS, LINEAR_PHYSVOX_TO_PHYSVOX, LINEAR_RAS_TO_RAS,
    LINEAR_VOX_TO_VOX, LinearTransform, Lta, REGISTER_DAT, lt_copy, lta_alloc, lta_copy,
};
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc_header, mri_copy_vol_geom_to_mri};
use larmorx_freesurfer_mri::tkreg::{
    fsl2tkreg, ras_from_surface_ras, tkreg_mtx, tkreg2fsl, tkreg2native,
};
use larmorx_freesurfer_mri::vol_geom::VolGeom;

use crate::vgeom::{get_vol_geom, vg_i_to_r, vg_is_equal, vg_r_to_i, vol_geom_text};

/// FS-504: FreeSurfer reads outside an array or through a missing matrix here; an error in both
/// modes (`<what>` names the function).
fn outside(g: &mut Globals, err: &mut dyn Write, what: &str, why: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_504,
        format!("{what}: {why} (FreeSurfer reads memory it does not own)"),
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}: {why} (FreeSurfer reads memory it does not own here)\n"),
    )
}

/// A transform as `calloc` leaves it: every number 0, no matrices, both geometries zeroed.
fn zeroed_lt() -> LinearTransform {
    let zero = || VolGeom {
        xsize: 0.0,
        ysize: 0.0,
        zsize: 0.0,
        x_r: 0.0,
        y_s: 0.0,
        z_a: 0.0,
        ..VolGeom::default()
    };
    LinearTransform {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        sigma: 0.0,
        m_l: None,
        m_dl: None,
        m_last_dl: None,
        ltype: 0,
        src: zero(),
        dst: zero(),
        label: 0,
    }
}

/// `*dst = *src` for transforms (C++'s implicit copy assignment): the numbers and the matrices'
/// pointers copied, the geometries assigned ([`VolGeom::assign`]).
fn lt_assign(g: &mut Globals, dst: &mut LinearTransform, src: &LinearTransform) {
    dst.x0 = src.x0;
    dst.y0 = src.y0;
    dst.z0 = src.z0;
    dst.sigma = src.sigma;
    dst.m_l = src.m_l.clone();
    dst.m_dl = src.m_dl.clone();
    dst.m_last_dl = src.m_last_dl.clone();
    dst.ltype = src.ltype;
    dst.src.assign(g, &src.src);
    dst.dst.assign(g, &src.dst);
    dst.label = src.label;
}

/// `LTAinvert`'s exchange of transforms `i` and `j` (`i < j`) through a `calloc`ed transform.
fn swap_lt(g: &mut Globals, v: &mut [LinearTransform], i: usize, j: usize) {
    let mut tmp = zeroed_lt();
    lt_assign(g, &mut tmp, &v[i]);
    let (a, b) = v.split_at_mut(j);
    lt_assign(g, &mut a[i], &b[0]);
    lt_assign(g, &mut v[j], &tmp);
}

/// `LTAfillInverse(lta)`: for each transform, its matrix's inverse (`MatrixInverse`) into the
/// inverse transform's matrix, or the program ends with `TransformInvert: xform noninvertible`
/// (`ERROR_BADPARM`); the inverse's source geometry is the transform's destination and its
/// destination the transform's source. FS-505 where the inverse has no matrix.
pub fn lta_fill_inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
) -> Result<(), Fatal> {
    for i in 0..lta.num_xforms.max(0) as usize {
        let xf = &lta.xforms[i];
        let inv = &mut lta.inv_xforms[i];
        let failed = match inv.m_l.as_mut() {
            Some(m) => matrix_inverse(g, out, err, xf.m_l.as_ref(), Some(m))?.is_null(),
            None => match matrix_inverse(g, out, err, xf.m_l.as_ref(), None)? {
                Ret::New(m) => {
                    g.notices.add(
                        &FS_505,
                        "LTAfillInverse: an inverse without a matrix: FreeSurfer's inverse is lost",
                    );
                    if !g.notices.reproduce() {
                        inv.m_l = Some(m);
                    }
                    false
                }
                _ => true,
            },
        };
        if failed {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "TransformInvert: xform noninvertible",
            )?;
        }
        inv.src.assign(g, &xf.dst);
        inv.dst.assign(g, &xf.src);
    }
    Ok(())
}

/// The arrays of `LTAinvert(lta, ltainv)`: a new inverse (`ltainv` `NULL`), into another array, or
/// in place (`ltainv == lta`).
pub enum InvertInto<'a> {
    New(&'a Lta),
    Given(&'a Lta, &'a mut Lta),
    Same(&'a mut Lta),
}

/// `LTAinvert`'s steps on the copy: the inverses filled, the transforms' order reversed in both
/// arrays (`n` from the source array), and the two arrays exchanged.
fn invert_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    l: &mut Lta,
    n: i32,
) -> Result<(), Fatal> {
    lta_fill_inverse(g, out, err, l)?;
    let (mut i, mut j) = (0i32, n.wrapping_sub(1));
    while i < n.wrapping_add(1) / 2 {
        if i != j {
            swap_lt(g, &mut l.inv_xforms, i as usize, j as usize);
            swap_lt(g, &mut l.xforms, i as usize, j as usize);
        }
        i += 1;
        j -= 1;
    }
    std::mem::swap(&mut l.xforms, &mut l.inv_xforms);
    Ok(())
}

/// `LTAinvert(lta, ltainv)`: `lta` copied into `ltainv` (`LTAcopy`: a new array when there is
/// none; `Null` after its message when the sizes differ) unless they are the same, then its
/// inverses filled ([`lta_fill_inverse`]), the transforms' order reversed and the inverse
/// transforms made the transforms: the inverse of the whole chain.
pub fn lta_invert(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    into: InvertInto,
) -> Result<Ret<Lta>, Fatal> {
    match into {
        InvertInto::New(l) => {
            let Some(mut c) = lta_copy(g, out, err, l, None)?.into_new() else {
                return Ok(Ret::Null);
            };
            invert_in_place(g, out, err, &mut c, l.num_xforms)?;
            Ok(Ret::New(c))
        }
        InvertInto::Given(l, d) => {
            if lta_copy(g, out, err, l, Some(d))?.is_null() {
                return Ok(Ret::Null);
            }
            invert_in_place(g, out, err, d, l.num_xforms)?;
            Ok(Ret::Given)
        }
        InvertInto::Same(l) => {
            let n = l.num_xforms;
            invert_in_place(g, out, err, l, n)?;
            Ok(Ret::Given)
        }
    }
}

/// `LTAconcat2(lta1, lta2, Reduce)`: [`lta_concat`] of the two.
pub fn lta_concat2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    lta1: &mut Lta,
    lta2: &mut Lta,
    reduce: bool,
) -> Result<Option<Lta>, Fatal> {
    lta_concat(g, out, err, env, &mut [lta1, lta2], reduce)
}

/// `LTAconcat(ltaArray, nLTAs, Reduce)`: every array's inverses filled first; a new array of all
/// their transforms, the first array's copied (`LTcopy`) and its type, `fscale` and subject taken;
/// each later array changed to that type when it has another ([`lta_change_type`]: the caller's
/// array is changed), then its transforms copied when its first source is the last copied
/// destination, else its inverse transforms in reverse order when its last destination is that
/// destination (`WARNING: LTAconcat(): inverting LTA <n> to match geometry` on standard output),
/// else `ERROR: LTAconcat(): LTAs <n-1> and <n> do not match`, both arrays printed
/// ([`lta_print`]) between `LTA <k> -------------------` lines and a line of dashes, and `None`.
/// With `reduce`, the result is [`lta_reduce`]'s of the new array. FS-504.
pub fn lta_concat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    ltas: &mut [&mut Lta],
    reduce: bool,
) -> Result<Option<Lta>, Fatal> {
    let mut ntot = 0i32;
    for l in ltas.iter_mut() {
        ntot = ntot.wrapping_add(l.num_xforms);
        lta_fill_inverse(g, out, err, l)?;
    }
    let mut lta = lta_alloc(g, out, err, ntot, None)?;
    if ltas.is_empty() {
        outside(g, err, "LTAconcat", "no transform arrays")?;
        return Ok(None);
    }
    let mut k = 0usize;
    {
        let lta1 = &ltas[0];
        for m in 0..lta1.num_xforms.max(0) as usize {
            lt_copy(g, out, err, &lta1.xforms[m], Some(&mut lta.xforms[k]))?;
            k += 1;
        }
        lta.ltype = lta1.ltype;
        lta.fscale = lta1.fscale;
        lta.subject.strcpy(&lta1.subject);
    }
    for n in 1..ltas.len() {
        if ltas[n].ltype != lta.ltype {
            lta_change_type(g, out, err, env, ltas[n], lta.ltype)?;
        }
        let lta1 = &ltas[n];
        let nx = lta1.num_xforms;
        if k == 0 || nx <= 0 {
            outside(g, err, "LTAconcat", "a transform array without transforms")?;
            return Ok(None);
        }
        let prev = &lta.xforms[k - 1].dst;
        let do_inv = if vg_is_equal(g, prev, &lta1.xforms[0].src) == 1 {
            false
        } else if vg_is_equal(g, prev, &lta1.xforms[nx as usize - 1].dst) == 1 {
            let _ = writeln!(
                out,
                "WARNING: LTAconcat(): inverting LTA {n} to match geometry"
            );
            true
        } else {
            let _ = writeln!(
                out,
                "ERROR: LTAconcat(): LTAs {} and {n} do not match",
                n - 1
            );
            let _ = writeln!(out, "LTA {} -------------------", n - 1);
            lta_print(g, err, out, ltas[n - 1])?;
            let _ = writeln!(out, "LTA {n} -------------------");
            lta_print(g, err, out, ltas[n])?;
            let _ = out.write_all(b"--------------------------\n");
            return Ok(None);
        };
        if do_inv {
            for m in (0..nx as usize).rev() {
                lt_copy(g, out, err, &lta1.inv_xforms[m], Some(&mut lta.xforms[k]))?;
                k += 1;
            }
        } else {
            for m in 0..nx as usize {
                lt_copy(g, out, err, &lta1.xforms[m], Some(&mut lta.xforms[k]))?;
                k += 1;
            }
        }
    }
    if reduce {
        return lta_reduce(g, out, err, &lta);
    }
    Ok(Some(lta))
}

/// `LTAreduce(lta0)`: a copy of `lta0` with its inverses filled; a new array of one transform, the
/// copy's type, `fscale` and subject, its transform the first one (`LTcopy`) multiplied on the
/// left by each next transform's matrix (`M * old`) when that transform's source is the previous
/// one's destination, by its inverse's matrix when its destination is (`WARNING: LTAreduce():
/// inverting LT <n> to match geometry`), else `ERROR: LTAreduce(): LTs <n-1> and <n> do not
/// match` and `None`; its destination the last transform's destination (its source when
/// inverted). The new array's inverse is not filled. FS-504.
pub fn lta_reduce(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta0: &Lta,
) -> Result<Option<Lta>, Fatal> {
    let Some(mut lta) = lta_copy(g, out, err, lta0, None)?.into_new() else {
        return Ok(None);
    };
    lta_fill_inverse(g, out, err, &mut lta)?;
    let mut ltar = lta_alloc(g, out, err, 1, None)?;
    ltar.ltype = lta.ltype;
    ltar.fscale = lta.fscale;
    ltar.subject.strcpy(&lta.subject);
    if lta.num_xforms <= 0 {
        outside(g, err, "LTAreduce", "a transform array without transforms")?;
        return Ok(None);
    }
    lt_copy(g, out, err, &lta.xforms[0], Some(&mut ltar.xforms[0]))?;
    let mut last: Option<(usize, bool)> = None;
    for n in 1..lta.num_xforms as usize {
        let (prev, lt) = (&lta.xforms[n - 1], &lta.xforms[n]);
        let do_inv = if vg_is_equal(g, &prev.dst, &lt.src) == 1 {
            false
        } else if vg_is_equal(g, &prev.dst, &lt.dst) == 1 {
            let _ = writeln!(
                out,
                "WARNING: LTAreduce(): inverting LT {n} to match geometry"
            );
            true
        } else {
            let _ = writeln!(
                out,
                "ERROR: LTAreduce(): LTs {} and {n} do not match",
                n - 1
            );
            return Ok(None);
        };
        let m = if do_inv {
            lta.inv_xforms[n].m_l.as_ref()
        } else {
            lt.m_l.as_ref()
        };
        // MatrixMultiply(M, lt0->m_L, lt0->m_L): FreeSurfer copies the input it writes into
        let old = ltar.xforms[0].m_l.clone();
        matrix_multiply(g, out, err, m, old.as_ref(), ltar.xforms[0].m_l.as_mut())?;
        last = Some((n, do_inv));
    }
    // with one transform, `lt` is `lt0` itself: its destination assigned to itself
    if let Some((n, do_inv)) = last {
        let lt = &lta.xforms[n];
        let d = if do_inv { &lt.src } else { &lt.dst };
        ltar.xforms[0].dst.assign(g, d);
    }
    Ok(Some(ltar))
}

/// `LTAgetV2V(mod, src, dst)`: `mod` (RAS to RAS) made voxel to voxel, `inv(dst vox2ras) mod (src
/// vox2ras)` in two products; a destination whose vox2ras cannot be inverted ends the program
/// (`LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i.`).
fn get_v2v(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Option<Matrix>,
    src: &VolGeom,
    dst: &VolGeom,
) -> Result<(), Fatal> {
    let s_i2r = vg_i_to_r(g, out, err, src)?;
    let d_r2i = vg_r_to_i(g, out, err, dst)?;
    if d_r2i.is_none() {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i.",
        )?;
    }
    let tmp = matrix_multiply(g, out, err, m.as_ref(), Some(&s_i2r), None)?.into_new();
    matrix_multiply(g, out, err, d_r2i.as_ref(), tmp.as_ref(), m.as_mut())?;
    Ok(())
}

/// `LTAgetR2R(mod, src, dst)`: `mod` (voxel to voxel) made RAS to RAS, `(dst vox2ras) mod inv(src
/// vox2ras)`; a source whose vox2ras cannot be inverted ends the program (`LTAgetR2R: passed
/// volumes did not have the info on r_to_i or i_to_r`).
fn get_r2r(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Option<Matrix>,
    src: &VolGeom,
    dst: &VolGeom,
) -> Result<(), Fatal> {
    let s_r2i = vg_r_to_i(g, out, err, src)?;
    let d_i2r = vg_i_to_r(g, out, err, dst)?;
    if s_r2i.is_none() {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "LTAgetR2R: passed volumes did not have the info on r_to_i or i_to_r",
        )?;
    }
    let tmp = matrix_multiply(g, out, err, m.as_ref(), s_r2i.as_ref(), None)?.into_new();
    matrix_multiply(g, out, err, Some(&d_i2r), tmp.as_ref(), m.as_mut())?;
    Ok(())
}

/// The identity with the three voxel sizes (or their reciprocals, `1. / size` in `double` stored
/// as `float`) on its diagonal.
fn sizes(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    reciprocal: bool,
) -> Result<Matrix, Fatal> {
    let mut m = matrix_identity(g, out, err, 4, None)?.unwrap_new();
    for (k, s) in [vg.xsize, vg.ysize, vg.zsize].into_iter().enumerate() {
        let v = if reciprocal { sse::div(1.0, s) } else { s };
        m.set_relt(k as i32 + 1, k as i32 + 1, v);
    }
    Ok(m)
}

/// `m = left (m right)`, FreeSurfer's two products `tmp = MatrixMultiply(m, right, NULL);
/// MatrixMultiply(left, tmp, m)`.
fn scale(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &mut Option<Matrix>,
    left: &Matrix,
    right: &Matrix,
) -> Result<(), Fatal> {
    let tmp = matrix_multiply(g, out, err, m.as_ref(), Some(right), None)?.into_new();
    matrix_multiply(g, out, err, Some(left), tmp.as_ref(), m.as_mut())?;
    Ok(())
}

/// A volume header of the geometry's extent (`MRIallocHeader(width, height, depth, MRI_UCHAR,
/// 1)`) with its voxel sizes, direction cosines and centre (`MRIcopyVolGeomToMRI`).
fn header_of(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Mri, Fatal> {
    let mut m = mri_alloc_header(g, out, err, vg.width, vg.height, vg.depth, MRI_UCHAR, 1)?;
    mri_copy_vol_geom_to_mri(&mut m, vg);
    Ok(m)
}

/// `MatrixCopy(new, m_L)` of a computed matrix into a transform's. FS-508: no computed matrix
/// (FreeSurfer's `MatrixCopy(NULL, m_L)` frees the transform's matrix, which it goes on using): an
/// error in both modes.
fn copy_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    new: Option<&Matrix>,
    m: &mut Option<Matrix>,
) -> Result<(), Fatal> {
    if new.is_none() && m.is_some() {
        g.notices.add(
            &FS_508,
            "LTAchangeType: FSL's conversion gave no matrix (FreeSurfer frees the transform's matrix and goes on using it)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "LTAchangeType: the FSL conversion gave no matrix (FreeSurfer frees the transform's matrix and goes on using it here)\n",
        )?;
    }
    matrix_copy(g, out, err, new, m.as_mut())?;
    Ok(())
}

/// With `Gdiag_no > 0`: `transformed matrix:` and the first transform's matrix on standard output
/// (`MatrixPrint(Gstdout, ...)`, `Gstdout` as `DiagInit` sets it). FS-504.
fn diag_print(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &Lta,
) -> Result<(), Fatal> {
    if g.gdiag_no > 0 {
        let _ = out.write_all(b"transformed matrix:\n");
        let Some(lt) = lta.xforms.first().filter(|_| lta.num_xforms > 0) else {
            return outside(
                g,
                err,
                "LTAchangeType",
                "a transform array without transforms",
            );
        };
        let mut buf: Vec<u8> = Vec::new();
        matrix_print(g, Some(&mut buf), out, err, lt.m_l.as_ref(), 5);
        let _ = out.write_all(&buf);
    }
    Ok(())
}

/// `LTAchangeType(lta, ltatype)`: nothing when the array has that type already; otherwise every
/// transform's geometries must be valid (else `LTAchangeType: src geometry must be valid` or `dst`,
/// `ERROR_BADPARM`), each matrix is converted between the types `LINEAR_RAS_TO_RAS`,
/// `LINEAR_VOX_TO_VOX`, `LINEAR_PHYSVOX_TO_PHYSVOX`, `REGISTER_DAT` (tkregister's, from the
/// destination to the source), `FSLREG_TYPE` and from `LINEAR_CORONAL_RAS_TO_CORONAL_RAS`
/// (through `LINEAR_RAS_TO_RAS` where FreeSurfer has no direct conversion; another target ends the
/// program: `LTAchangeType: you are requesting ras-to-ras to <n> ` and the like), the array takes
/// the new type, and its inverses are filled ([`lta_fill_inverse`]); an array of a type not listed
/// keeps its matrices and type and gets its inverses filled. The FSL conversions read
/// `FSLOUTPUTTYPE` from `env`. FS-504, FS-506, FS-507.
pub fn lta_change_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    lta: &mut Lta,
    ltatype: i32,
) -> Result<(), Fatal> {
    if lta.ltype == ltatype {
        return Ok(());
    }
    let n = lta.num_xforms.max(0) as usize;
    for lt in &lta.xforms[..n] {
        if lt.src.valid == 0 {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "LTAchangeType: src geometry must be valid\n",
            )?;
        }
        if lt.dst.valid == 0 {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "LTAchangeType: dst geometry must be valid\n",
            )?;
        }
    }
    let unsupported = |g: &mut Globals, err: &mut dyn Write, what: &str| {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!("LTAchangeType{what} to {ltatype} "),
        )
    };
    match lta.ltype {
        LINEAR_RAS_TO_RAS => match ltatype {
            LINEAR_VOX_TO_VOX => {
                for lt in &mut lta.xforms[..n] {
                    get_v2v(g, out, err, &mut lt.m_l, &lt.src, &lt.dst)?;
                }
                lta.ltype = LINEAR_VOX_TO_VOX;
            }
            LINEAR_PHYSVOX_TO_PHYSVOX => {
                for lt in &mut lta.xforms[..n] {
                    let s_isize = sizes(g, out, err, &lt.src, true)?;
                    let d_size = sizes(g, out, err, &lt.dst, false)?;
                    get_v2v(g, out, err, &mut lt.m_l, &lt.src, &lt.dst)?;
                    scale(g, out, err, &mut lt.m_l, &d_size, &s_isize)?;
                }
                lta.ltype = LINEAR_PHYSVOX_TO_PHYSVOX;
            }
            REGISTER_DAT => {
                for lt in &mut lta.xforms[..n] {
                    let mreg = tkreg_mtx(g, out, err, &lt.dst, &lt.src, lt.m_l.as_ref())?;
                    copy_into(g, out, err, Some(&mreg), &mut lt.m_l)?;
                }
                lta.ltype = REGISTER_DAT;
            }
            FSLREG_TYPE => {
                for lt in &mut lta.xforms[..n] {
                    let mreg = tkreg_mtx(g, out, err, &lt.dst, &lt.src, lt.m_l.as_ref())?;
                    let mfsl = tkreg2fsl(g, out, err, env, &lt.dst, &lt.src, Some(&mreg))?;
                    copy_into(g, out, err, mfsl.as_ref(), &mut lt.m_l)?;
                }
                lta.ltype = FSLREG_TYPE;
            }
            _ => unsupported(g, err, ": you are requesting ras-to-ras")?,
        },
        LINEAR_VOX_TO_VOX => match ltatype {
            LINEAR_RAS_TO_RAS => {
                for lt in &mut lta.xforms[..n] {
                    get_r2r(g, out, err, &mut lt.m_l, &lt.src, &lt.dst)?;
                }
                lta.ltype = LINEAR_RAS_TO_RAS;
            }
            LINEAR_PHYSVOX_TO_PHYSVOX => {
                for lt in &mut lta.xforms[..n] {
                    let s_isize = sizes(g, out, err, &lt.src, true)?;
                    let d_size = sizes(g, out, err, &lt.dst, false)?;
                    scale(g, out, err, &mut lt.m_l, &d_size, &s_isize)?;
                }
                lta.ltype = LINEAR_PHYSVOX_TO_PHYSVOX;
            }
            REGISTER_DAT | FSLREG_TYPE => {
                lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
                lta_change_type(g, out, err, env, lta, ltatype)?;
            }
            _ => unsupported(g, err, ": you are requesting vox-to-vox")?,
        },
        LINEAR_PHYSVOX_TO_PHYSVOX => match ltatype {
            LINEAR_VOX_TO_VOX => {
                for lt in &mut lta.xforms[..n] {
                    let s_size = sizes(g, out, err, &lt.src, false)?;
                    let d_isize = sizes(g, out, err, &lt.dst, true)?;
                    scale(g, out, err, &mut lt.m_l, &d_isize, &s_size)?;
                }
                lta.ltype = LINEAR_VOX_TO_VOX;
            }
            LINEAR_RAS_TO_RAS => {
                if n > 0 {
                    g.notices.add(
                        &FS_506,
                        "LTAchangeType: physical-voxel to RAS-to-RAS: FreeSurfer converts the matrix as RAS-to-RAS before scaling it",
                    );
                }
                let reproduce = g.notices.reproduce();
                for lt in &mut lta.xforms[..n] {
                    let s_size = sizes(g, out, err, &lt.src, false)?;
                    let d_isize = sizes(g, out, err, &lt.dst, true)?;
                    if reproduce {
                        get_v2v(g, out, err, &mut lt.m_l, &lt.src, &lt.dst)?;
                        scale(g, out, err, &mut lt.m_l, &d_isize, &s_size)?;
                    } else {
                        scale(g, out, err, &mut lt.m_l, &d_isize, &s_size)?;
                        get_r2r(g, out, err, &mut lt.m_l, &lt.src, &lt.dst)?;
                    }
                }
                lta.ltype = LINEAR_RAS_TO_RAS;
            }
            REGISTER_DAT | FSLREG_TYPE => {
                lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
                lta_change_type(g, out, err, env, lta, ltatype)?;
            }
            _ => unsupported(g, err, ": you are requesting physvox-to-physvox")?,
        },
        REGISTER_DAT => {
            match ltatype {
                LINEAR_RAS_TO_RAS => {
                    if n > 1 {
                        g.notices.add(
                            &FS_507,
                            format!(
                                "LTAchangeType: tkregister to RAS-to-RAS of {n} transforms: FreeSurfer converts the first {n} times"
                            ),
                        );
                    }
                    let reproduce = g.notices.reproduce();
                    for i in 0..n {
                        let lt = &mut lta.xforms[if reproduce { 0 } else { i }];
                        let movmri = header_of(g, out, err, &lt.src)?;
                        let refmri = header_of(g, out, err, &lt.dst)?;
                        let mreg =
                            tkreg2native(g, out, err, &refmri.vg, &movmri.vg, lt.m_l.as_ref())?;
                        copy_into(g, out, err, Some(&mreg), &mut lt.m_l)?;
                    }
                    lta.ltype = LINEAR_RAS_TO_RAS;
                }
                LINEAR_VOX_TO_VOX => {
                    lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
                    lta_change_type(g, out, err, env, lta, LINEAR_VOX_TO_VOX)?;
                }
                FSLREG_TYPE => {
                    for lt in &mut lta.xforms[..n] {
                        let mfsl = tkreg2fsl(g, out, err, env, &lt.dst, &lt.src, lt.m_l.as_ref())?;
                        copy_into(g, out, err, mfsl.as_ref(), &mut lt.m_l)?;
                    }
                    lta.ltype = FSLREG_TYPE;
                }
                _ => unsupported(g, err, " unsupported: you are requesting REGISTER_DAT")?,
            }
            diag_print(g, out, err, lta)?;
        }
        LINEAR_CORONAL_RAS_TO_CORONAL_RAS => {
            match ltatype {
                LINEAR_RAS_TO_RAS => {
                    for lt in &mut lta.xforms[..n] {
                        let tmp = header_of(g, out, err, &lt.dst)?;
                        let sras2ras = ras_from_surface_ras(g, out, err, &tmp.vg, None)?.into_new();
                        // m_L = MatrixMultiply(m_sras2ras, m_L, m_L)
                        let old = lt.m_l.clone();
                        matrix_multiply(
                            g,
                            out,
                            err,
                            sras2ras.as_ref(),
                            old.as_ref(),
                            lt.m_l.as_mut(),
                        )?;
                    }
                    lta.ltype = LINEAR_RAS_TO_RAS;
                }
                REGISTER_DAT | FSLREG_TYPE => {
                    lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
                    lta_change_type(g, out, err, env, lta, ltatype)?;
                }
                _ => unsupported(
                    g,
                    err,
                    " unsupported: you are requesting COR_RAS_TO_COR_RAS",
                )?,
            }
            diag_print(g, out, err, lta)?;
        }
        FSLREG_TYPE => {
            match ltatype {
                LINEAR_RAS_TO_RAS | LINEAR_VOX_TO_VOX => {
                    lta_change_type(g, out, err, env, lta, REGISTER_DAT)?;
                    lta_change_type(g, out, err, env, lta, ltatype)?;
                }
                REGISTER_DAT => {
                    for lt in &mut lta.xforms[..n] {
                        let mreg = fsl2tkreg(g, out, err, env, &lt.dst, &lt.src, lt.m_l.as_ref())?;
                        copy_into(g, out, err, mreg.as_ref(), &mut lt.m_l)?;
                    }
                    lta.ltype = REGISTER_DAT;
                }
                _ => unsupported(g, err, " unsupported: you are requesting FSLREG_TYPE")?,
            }
            diag_print(g, out, err, lta)?;
        }
        _ => {}
    }
    lta_fill_inverse(g, out, err, lta)
}

/// `LTAmriIsSource(lta, mri)`: 1 when the volume's geometry (`getVolGeom`: marked valid) equals
/// the first transform's source (`vg_isEqual`), else 0. FS-504.
pub fn lta_mri_is_source(
    g: &mut Globals,
    err: &mut dyn Write,
    lta: &Lta,
    mri: &Mri,
) -> Result<i32, Fatal> {
    let mut vg = VolGeom::default();
    get_vol_geom(&mri.vg, &mut vg);
    if lta.num_xforms <= 0 {
        outside(
            g,
            err,
            "LTAmriIsSource",
            "a transform array without transforms",
        )?;
        return Ok(0);
    }
    Ok(vg_is_equal(g, &lta.xforms[0].src, &vg))
}

/// `LTAmriIsTarget(lta, mri)`: 1 when the volume's geometry equals the last transform's
/// destination, else 0. FS-504.
pub fn lta_mri_is_target(
    g: &mut Globals,
    err: &mut dyn Write,
    lta: &Lta,
    mri: &Mri,
) -> Result<i32, Fatal> {
    let mut vg = VolGeom::default();
    get_vol_geom(&mri.vg, &mut vg);
    if lta.num_xforms <= 0 {
        outside(
            g,
            err,
            "LTAmriIsTarget",
            "a transform array without transforms",
        )?;
        return Ok(0);
    }
    Ok(vg_is_equal(
        g,
        &lta.xforms[lta.num_xforms as usize - 1].dst,
        &vg,
    ))
}

/// `LTAprint(fp, lta)`: `type      = <n> ` with `# LINEAR_VOX_TO_VOX`, `# LINEAR_RAS_TO_RAS` or
/// `# REGISTER_DAT` for those types, `nxforms   = <n>`, each transform's `mean      = ` (its
/// centre, `%6.4f`), `sigma     = `, `1 4 4` and its matrix's rows (`%18.15le `), then each
/// transform's `src volume info` and `dst volume info` blocks (`writeVolGeom`), then `subject
/// <name>` when there is one and `fscale <f>` (`%f`) when it is above 0. `NO_ERROR`. FS-504.
pub fn lta_print(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut dyn Write,
    lta: &Lta,
) -> Result<i32, Fatal> {
    let mut s: Vec<u8> = Vec::new();
    s.extend_from_slice(format!("type      = {} ", lta.ltype).as_bytes());
    match lta.ltype {
        LINEAR_VOX_TO_VOX => s.extend_from_slice(b"# LINEAR_VOX_TO_VOX"),
        LINEAR_RAS_TO_RAS => s.extend_from_slice(b"# LINEAR_RAS_TO_RAS"),
        REGISTER_DAT => s.extend_from_slice(b"# REGISTER_DAT"),
        _ => {}
    }
    s.push(b'\n');
    s.extend_from_slice(format!("nxforms   = {}\n", lta.num_xforms).as_bytes());
    let n = lta.num_xforms.max(0) as usize;
    let f = |x: f32| fmt_f(f64::from(x), 6, 4);
    for lt in &lta.xforms[..n] {
        s.extend_from_slice(
            format!("mean      = {} {} {}\n", f(lt.x0), f(lt.y0), f(lt.z0)).as_bytes(),
        );
        s.extend_from_slice(format!("sigma     = {}\n", f(lt.sigma)).as_bytes());
        s.extend_from_slice(b"1 4 4\n");
        let Some(m) = lt.m_l.as_ref().filter(|m| m.rows >= 4 && m.cols >= 4) else {
            let _ = fp.write_all(&s);
            outside(g, err, "LTAprint", "a transform without a 4 x 4 matrix")?;
            return Ok(ERROR_BADPARM);
        };
        for r in 1..=4 {
            for c in 1..=4 {
                let v = cfmt::pad(&cfmt::e(f64::from(m.relt(r, c)), 15), 18);
                s.extend_from_slice(v.as_bytes());
                s.push(b' ');
            }
            s.push(b'\n');
        }
    }
    for lt in &lta.xforms[..n] {
        s.extend_from_slice(b"src volume info\n");
        s.extend_from_slice(&vol_geom_text(&lt.src));
        s.extend_from_slice(b"dst volume info\n");
        s.extend_from_slice(&vol_geom_text(&lt.dst));
    }
    if !lta.subject.is_empty() {
        s.extend_from_slice(b"subject ");
        s.extend_from_slice(lta.subject.c_bytes());
        s.push(b'\n');
    }
    if lta.fscale > 0.0 {
        s.extend_from_slice(format!("fscale {}\n", cfmt::f(f64::from(lta.fscale), 6)).as_bytes());
    }
    let _ = fp.write_all(&s);
    Ok(NO_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// A valid geometry of the given extent, voxel sizes and centre (coronal cosines).
    fn geom(w: i32, size: [f32; 3], c: [f32; 3]) -> VolGeom {
        VolGeom {
            valid: 1,
            width: w,
            height: w + 1,
            depth: w + 2,
            xsize: size[0],
            ysize: size[1],
            zsize: size[2],
            ras_good_flag: 1,
            c_r: c[0],
            c_a: c[1],
            c_s: c[2],
            ..VolGeom::default()
        }
    }

    /// A RAS-to-RAS array of `n` transforms (a rotation about z by a small angle and a shift)
    /// between two geometries.
    fn ras_lta(g: &mut Globals, n: i32) -> Lta {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut l = lta_alloc(g, &mut o, &mut e, n, None).unwrap();
        l.ltype = LINEAR_RAS_TO_RAS;
        for (k, lt) in l.xforms.iter_mut().enumerate() {
            let m = lt.m_l.as_mut().unwrap();
            m.set_relt(1, 1, 0.8);
            m.set_relt(1, 2, -0.6);
            m.set_relt(2, 1, 0.6);
            m.set_relt(2, 2, 0.8);
            m.set_relt(1, 4, 3.5 + k as f32);
            m.set_relt(3, 4, -2.0);
            lt.src = geom(8, [1.0, 1.25, 2.0], [1.0, -3.0, 5.0]);
            lt.dst = geom(10, [0.5, 1.0, 1.5], [-2.0, 4.0, 0.5]);
        }
        l
    }

    fn close(a: &Matrix, b: &Matrix, tol: f32) -> bool {
        (1..=4).all(|r| (1..=4).all(|c| (a.relt(r, c) - b.relt(r, c)).abs() <= tol))
    }

    #[test]
    fn physical_voxels_back_to_ras_by_default() {
        // FS-506: by default physvox -> RAS undoes RAS -> physvox; reproduce gives FreeSurfer's
        let env = Environment::default();
        for (mode, back) in [
            (UpstreamDefects::Fix, true),
            (UpstreamDefects::Reproduce, false),
        ] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut l = ras_lta(&mut g, 1);
            let r = l.xforms[0].m_l.clone().unwrap();
            lta_change_type(
                &mut g,
                &mut o,
                &mut e,
                &env,
                &mut l,
                LINEAR_PHYSVOX_TO_PHYSVOX,
            )
            .unwrap();
            lta_change_type(&mut g, &mut o, &mut e, &env, &mut l, LINEAR_RAS_TO_RAS).unwrap();
            assert_eq!(l.ltype, LINEAR_RAS_TO_RAS);
            let got = l.xforms[0].m_l.as_ref().unwrap();
            assert_eq!(close(got, &r, 1e-4), back, "{mode:?}");
            assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-506"));
        }
    }

    #[test]
    fn tkregister_to_ras_converts_every_transform_once_by_default() {
        // FS-507: two transforms; each one's round trip RAS -> REGISTER_DAT -> RAS by default
        let env = Environment::default();
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut l = ras_lta(&mut g, 2);
        let r: Vec<Matrix> = l.xforms.iter().map(|t| t.m_l.clone().unwrap()).collect();
        lta_change_type(&mut g, &mut o, &mut e, &env, &mut l, REGISTER_DAT).unwrap();
        lta_change_type(&mut g, &mut o, &mut e, &env, &mut l, LINEAR_RAS_TO_RAS).unwrap();
        for (t, r) in l.xforms.iter().zip(&r) {
            assert!(close(t.m_l.as_ref().unwrap(), r, 1e-3));
        }
        // reproduce: the first converted twice, the second not at all
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let mut l = ras_lta(&mut g, 2);
        lta_change_type(&mut g, &mut o, &mut e, &env, &mut l, REGISTER_DAT).unwrap();
        let reg1 = l.xforms[1].m_l.clone().unwrap();
        lta_change_type(&mut g, &mut o, &mut e, &env, &mut l, LINEAR_RAS_TO_RAS).unwrap();
        assert_eq!(l.xforms[1].m_l.as_ref().unwrap(), &reg1);
        assert!(!close(l.xforms[0].m_l.as_ref().unwrap(), &r[0], 1e-3));
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-507"));
    }

    #[test]
    fn an_inverse_without_a_matrix_gets_one_by_default() {
        // FS-505
        for (mode, kept) in [
            (UpstreamDefects::Fix, true),
            (UpstreamDefects::Reproduce, false),
        ] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut l = ras_lta(&mut g, 1);
            l.inv_xforms[0].m_l = None;
            lta_fill_inverse(&mut g, &mut o, &mut e, &mut l).unwrap();
            assert_eq!(l.inv_xforms[0].m_l.is_some(), kept);
            let (a, b) = (&l.inv_xforms[0].src, &l.xforms[0].dst);
            assert_eq!((a.width, a.xsize, a.c_r), (b.width, b.xsize, b.c_r));
        }
    }

    #[test]
    fn inverting_a_chain_reverses_it() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut l = ras_lta(&mut g, 3);
        l.xforms[2].label = 7;
        let r = match lta_invert(&mut g, &mut o, &mut e, InvertInto::New(&l)).unwrap() {
            Ret::New(r) => r,
            _ => panic!("a new array"),
        };
        // the last transform's inverse first (the inverse array's label travels with it); its
        // source the last destination, its matrix the inverse of the last matrix
        assert_eq!(r.inv_xforms[0].label, 7);
        let (a, b) = (&r.xforms[0].src, &l.xforms[2].dst);
        assert_eq!((a.width, a.xsize, a.c_r), (b.width, b.xsize, b.c_r));
        let id = matrix_multiply(
            &mut g,
            &mut o,
            &mut e,
            r.xforms[0].m_l.as_ref(),
            l.xforms[2].m_l.as_ref(),
            None,
        )
        .unwrap()
        .unwrap_new();
        let i4 = matrix_identity(&mut g, &mut o, &mut e, 4, None)
            .unwrap()
            .unwrap_new();
        assert!(close(&id, &i4, 1e-5));
    }

    #[test]
    fn empty_arrays_are_errors() {
        // FS-504
        let env = Environment::default();
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let l = lta_alloc(&mut g, &mut o, &mut e, 0, None).unwrap();
            assert!(lta_reduce(&mut g, &mut o, &mut e, &l).is_err());
            assert!(lta_concat(&mut g, &mut o, &mut e, &env, &mut [], false).is_err());
        }
    }
}
