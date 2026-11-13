// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Resampling a volume onto a template's grid (`utils/mri.cpp`, 3.1-37): `MRIresampleFill` and
//! `MRIresample`.
//!
//! Each template voxel `(di, dj, dk)` (the template's x outermost, z innermost, frames outermost
//! of all) is mapped into the source by `MRIgetResampleMatrix(src, template)` (a `float` matrix
//! product), and the source is sampled there: windowed sinc (half-width 5), cubic, cubic B-spline
//! or, read from the eight corners around the point, trilinear, nearest or the "weighted" choice
//! of a corner; or the template voxel is the label most of the source voxels it holds vote for
//! (`SAMPLE_VOTE`). The values are stored in the source's type, clipped to the type's range and
//! rounded with `nint`. The result gets the template's header, the source's pulse parameters and
//! direction cosines and centre made from the two volumes' geometries.
//!
//! The NaNs: the trilinear value is FreeSurfer's sum of eight products in `double` (the last one
//! in `float`, as C's types make it) in the official build's operand order, read from its object
//! code (`docs/findings/freesurfer-volume.md` §11), with SSE's rules
//! ([`larmorx_freesurfer_core::sse`]); a float voxel taken as it is (nearest, weighted) keeps its
//! bits, a signalling NaN included.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §11):
//! - **FS-326** (rule A): the votes are counted in a byte volume, so a label's count stops at 255:
//!   a label with more votes ties with one of 255 (the lower label wins). The default counts
//!   every vote; `reproduce` stops at 255. Noted in both modes.
//! - **FS-327** (crash): voting with a template whose orientation is unknown (no good RAS and no
//!   principal slice direction) or whose geometry does not invert: FreeSurfer writes into the
//!   matrix it did not get. An error in both modes (`NULL`).
//! - **FS-328** (rule A): a volume of 64-bit integers (`MRI_LONG`) is written as 32-bit integers at
//!   the voxel's index (halves of other voxels). The default writes the voxel; `reproduce`
//!   FreeSurfer's halves. Noted in both modes.
//!
//! Kept (rule C, findings §11): the fill value replaces the destination's zeros before the
//! sampling, which then writes every voxel of the six sampled types: it shows only in a volume of
//! another type (and is never non-zero on recon-all's paths).
//!
//! Cubic B-spline sampling (`SAMPLE_CUBIC_BSPLINE`) needs `fs.bspline` (wave 3.2): a port error
//! until it is ported.

// C's comparisons, as written (`if (v < 0) v = 0; if (v > 255) v = 255;`: the same values as
// `clamp`, NaN kept)
#![allow(clippy::manual_clamp)]

use std::collections::HashMap;
use std::io::Write;

use larmorx_core::defects::{FS_298, FS_326, FS_327, FS_328};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::sse::{add64, div, mul, mul64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, mri_alloc_sequence,
    mri_copy_header, mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::{Pair, mri_replace_values};
use crate::rows::{self, Num};
use crate::sample::{
    SAMPLE_CUBIC, SAMPLE_CUBIC_BSPLINE, SAMPLE_NEAREST, SAMPLE_SINC, SAMPLE_TRILINEAR, SAMPLE_VOTE,
    SAMPLE_WEIGHTED, cubic_sample_volume_frame, cvtt, sinc_sample_volume_frame,
};
use crate::xform::mri_get_resample_matrix;

const WHAT: &str = "MRIresampleFill";

/// `MRIresample(src, template, type)`: [`mri_resample_fill`] with a fill value of 0.
pub fn mri_resample(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    template: &Mri,
    resample_type: i32,
) -> Result<Option<Mri>, Fatal> {
    mri_resample_fill(g, out, err, src, template, resample_type, 0.0)
}

/// `(int) floor(x)` of a `float` (x86-64's `INT_MIN` for NaN and values beyond the `int` range).
fn ifloor(x: f32) -> i32 {
    cvtt(f64::from(x.floor()))
}

/// `MatrixPrint(stdout, m)` (`out` is FreeSurfer's standard output).
fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: &Matrix) {
    let mut buf: Vec<u8> = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, Some(m), 5);
    let _ = out.write_all(&buf);
}

/// `MRIresampleFill(src, template, type, fill_val)`: a source without a good RAS prints
/// `MRIresample(): WARNING: ras_good_flag is not set, changing orientation\nto default.` on
/// standard output first; `None` when [`mri_get_resample_matrix`] gives none. The destination has
/// the template's extent, the source's type and `min(source frames, template frames)` frames, its
/// zeros replaced by `fill_val` (`MRIreplaceValues`), then the template's header and the source's
/// pulse parameters. See the [module documentation](self) for the sampling and the defects.
#[allow(clippy::too_many_arguments)]
pub fn mri_resample_fill(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    template: &Mri,
    resample_type: i32,
    fill_val: f32,
) -> Result<Option<Mri>, Fatal> {
    if src.vg.ras_good_flag == 0 {
        let _ = out.write_all(
            b"MRIresample(): WARNING: ras_good_flag is not set, changing orientation\nto default.\n",
        );
    }
    let Some(m) = mri_get_resample_matrix(g, out, err, src, template)? else {
        return Ok(None);
    };
    let diag = g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0;
    if diag {
        let _ = out.write_all(b"MRIresample() matrix is:\n");
        print_matrix(g, out, err, &m);
    }
    let nframes_target = src.nframes.min(template.nframes);
    let (tw, th, td) = (template.vg.width, template.vg.height, template.vg.depth);
    let mut dest = mri_alloc_sequence(g, out, err, tw, th, td, src.mtype, nframes_target)?;
    mri_replace_values(g, out, err, Pair::Same(&mut dest), 0.0, fill_val)?;
    mri_copy_header(g, out, err, template, Some(&mut dest))?;
    mri_copy_pulse_parameters(src, &mut dest);
    let mut sp = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    let mut dp = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    dp.set_relt(4, 1, 1.0);
    sp.set_relt(4, 1, 1.0);
    if resample_type == SAMPLE_CUBIC_BSPLINE {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            "MRIresampleFill: cubic B-spline sampling needs fs.bspline (not yet ported)\n",
        )?;
        return Ok(None);
    }
    if resample_type == SAMPLE_VOTE {
        if !vote(g, out, err, src, template, &mut dest, &mut sp, &mut dp)? {
            return Ok(None);
        }
    } else {
        sample_all(
            g,
            out,
            err,
            src,
            &mut dest,
            &m,
            &mut sp,
            &mut dp,
            nframes_target,
            resample_type,
        )?;
    }
    header(g, out, err, src, &mut dest, &m, diag)?;
    Ok(Some(dest))
}

/// The eight corners' values (`val000` ... `val111`, x the first digit), as FreeSurfer keeps them
/// from voxel to voxel: a source type none of the six reads leaves them as they were (0).
#[derive(Clone, Copy, Default)]
struct Corners([f32; 8]);

/// A source voxel as `(float) MRI?seq_vox(src, ...)` reads it: the type's element, a float as it
/// is, a 64-bit integer as FreeSurfer's 32-bit read at the voxel's index (FS-298).
fn read_vox(g: &mut Globals, src: &Mri, sl: i64, y: i64, x: i64) -> f32 {
    match src.mtype {
        MRI_LONG => {
            let half = rows::read(g, WHAT, src, sl, y, x, MRI_INT);
            let full = rows::read(g, WHAT, src, sl, y, x, MRI_LONG);
            let (Num::I(h), Num::I(f)) = (half, full) else {
                unreachable!("integer reads")
            };
            if h != f {
                g.notices.add(
                    &FS_298,
                    format!("{WHAT} reads a 64-bit integer voxel as a 32-bit integer"),
                );
                if g.notices.reproduce() {
                    return h as f32;
                }
            }
            f as f32
        }
        ty => rows::read(g, WHAT, src, sl, y, x, ty).f(),
    }
}

/// The sampling loop of every type but the vote.
#[allow(clippy::too_many_arguments)]
fn sample_all(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dest: &mut Mri,
    m: &Matrix,
    sp: &mut Matrix,
    dp: &mut Matrix,
    nframes_target: i32,
    resample_type: i32,
) -> Result<(), Fatal> {
    let (tw, th, td) = (dest.vg.width, dest.vg.height, dest.vg.depth);
    let (sw, sh, sd) = (src.vg.width, src.vg.height, src.vg.depth);
    let sampled = matches!(
        src.mtype,
        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG | MRI_FLOAT
    );
    let mut c = Corners::default();
    let mut val = 0.0f32;
    // FreeSurfer's `double pval`: the samplers leave it as it was when they fail (a source type
    // they refuse, whose destination nothing is stored into)
    let mut pval = 0.0f64;
    for nframe in 0..nframes_target {
        for di in 0..tw {
            for dj in 0..th {
                for dk in 0..td {
                    dp.set_relt(1, 1, di as f32);
                    dp.set_relt(2, 1, dj as f32);
                    dp.set_relt(3, 1, dk as f32);
                    matrix_multiply(g, out, err, Some(m), Some(&*dp), Some(&mut *sp))?;
                    let si_ff = sp.relt(1, 1);
                    let sj_ff = sp.relt(2, 1);
                    let sk_ff = sp.relt(3, 1);
                    let (si, sj, sk) = (ifloor(si_ff), ifloor(sj_ff), ifloor(sk_ff));
                    let si_f = sub(si_ff, si as f32);
                    let sj_f = sub(sj_ff, sj as f32);
                    let sk_f = sub(sk_ff, sk as f32);
                    let (x, y, z) = (f64::from(si_ff), f64::from(sj_ff), f64::from(sk_ff));
                    if resample_type == SAMPLE_SINC {
                        sinc_sample_volume_frame(g, err, src, x, y, z, nframe, 5, &mut pval)?;
                        val = pval as f32;
                    } else if resample_type == SAMPLE_CUBIC {
                        cubic_sample_volume_frame(g, err, src, x, y, z, nframe, &mut pval);
                        val = pval as f32;
                    } else {
                        if sampled {
                            c = corners(g, src, si, sj, sk, nframe, sw, sh, sd);
                        }
                        if resample_type == SAMPLE_TRILINEAR {
                            val = trilinear(si_f, sj_f, sk_f, &c);
                        }
                        if resample_type == SAMPLE_NEAREST {
                            val = nearest(si_f, sj_f, sk_f, &c);
                        }
                        if resample_type == SAMPLE_WEIGHTED {
                            val = weighted(si_f, sj_f, sk_f, &c);
                        }
                    }
                    store(g, dest, di, dj, dk, nframe, val);
                }
            }
        }
    }
    Ok(())
}

/// The eight corners of `(si, sj, sk)` in `frame`, each 0 where its column, row or slice is
/// outside the source.
#[allow(clippy::too_many_arguments)]
fn corners(
    g: &mut Globals,
    src: &Mri,
    si: i32,
    sj: i32,
    sk: i32,
    frame: i32,
    sw: i32,
    sh: i32,
    sd: i32,
) -> Corners {
    let ig = [si >= 0 && si < sw, {
        let s = si.wrapping_add(1);
        s >= 0 && s < sw
    }];
    let jg = [sj >= 0 && sj < sh, {
        let s = sj.wrapping_add(1);
        s >= 0 && s < sh
    }];
    let kg = [sk >= 0 && sk < sd, {
        let s = sk.wrapping_add(1);
        s >= 0 && s < sd
    }];
    let mut c = [0.0f32; 8];
    for (n, v) in c.iter_mut().enumerate() {
        let (a, b, k) = (n >> 2, (n >> 1) & 1, n & 1);
        if ig[a] && jg[b] && kg[k] {
            let sl = i64::from(sk) + k as i64 + i64::from(frame) * i64::from(sd);
            let y = i64::from(sj) + b as i64;
            let x = i64::from(si) + a as i64;
            *v = read_vox(g, src, sl, y, x);
        }
    }
    Corners(c)
}

/// The trilinear value, as the official build computes it: the first seven products in `double`
/// (the C expression's `1.0 - f` makes them so), the last, `si_f * sj_f * sk_f * val111`, in
/// `float`, summed left to right; each product's operands in the object code's order.
fn trilinear(si_f: f32, sj_f: f32, sk_f: f32, c: &Corners) -> f32 {
    let v = |n: usize| f64::from(c.0[n]);
    let (d0, d1, d2) = (f64::from(si_f), f64::from(sj_f), f64::from(sk_f));
    let a0 = sub64(1.0, d0);
    let a1 = sub64(1.0, d1);
    let a2 = sub64(1.0, d2);
    let p01 = mul64(a0, a1);
    let x5 = mul64(d1, a0);
    let x7 = mul64(a1, d0);
    let mut acc = add64(mul64(mul64(p01, a2), v(0)), mul64(mul64(p01, d2), v(1)));
    acc = add64(acc, mul64(mul64(a2, x5), v(2)));
    acc = add64(acc, mul64(mul64(x5, d2), v(3)));
    acc = add64(acc, mul64(mul64(a2, x7), v(4)));
    acc = add64(acc, mul64(mul64(d2, x7), v(5)));
    let f10 = mul(sj_f, si_f);
    acc = add64(acc, mul64(mul64(a2, f64::from(f10)), v(6)));
    let t8 = mul(mul(f10, sk_f), c.0[7]);
    add64(acc, f64::from(t8)) as f32
}

/// The nearest corner: each fraction below 0.5 takes the lower one (NaN the upper).
fn nearest(si_f: f32, sj_f: f32, sk_f: f32, c: &Corners) -> f32 {
    // `if (f < 0.5)` the lower corner, else (NaN included) the upper one
    let upper = |f: f32| if f < 0.5 { 0 } else { 1 };
    c.0[(upper(si_f) << 2) | (upper(sj_f) << 1) | upper(sk_f)]
}

/// `SAMPLE_WEIGHTED` ("unfinished" in FreeSurfer): each corner weighs the sum of the inverse
/// squared distances `1 / (1 - f)^2` or `1 / f^2` of its three coordinates (in `float`, the
/// first written `1. / (1 - 2 f + f^2)` in `double`); corners of equal value pool their weights
/// into the first of them; the corner with the largest pooled weight (the first on ties) gives
/// the value.
fn weighted(si_f: f32, sj_f: f32, sk_f: f32, c: &Corners) -> f32 {
    let inv1 = |f: f32| (1.0 / f64::from(1.0 - 2.0 * f + f * f)) as f32;
    let inv0 = |f: f32| 1.0 / (f * f);
    let (ii2, ij2, ik2) = (inv1(si_f), inv1(sj_f), inv1(sk_f));
    let (isi, isj, isk) = (inv0(si_f), inv0(sj_f), inv0(sk_f));
    let wx = [ii2, isi];
    let wy = [ij2, isj];
    let wz = [ik2, isk];
    let mut wc = [0.0f32; 8];
    for (n, w) in wc.iter_mut().enumerate() {
        *w = wx[n >> 2] + wy[(n >> 1) & 1] + wz[n & 1];
    }
    // wi[n]: the first corner of n's value (C's chain of equality tests, the lowest match last)
    let mut wi = [0usize; 8];
    for (n, w) in wi.iter_mut().enumerate() {
        *w = n;
        for k in (0..n).rev() {
            if c.0[n] == c.0[k] {
                *w = k;
            }
        }
    }
    let mut w = [0.0f32; 8];
    for n in 0..8 {
        w[wi[n]] += wc[n];
    }
    let mut mwi = 0;
    for n in 1..8 {
        if w[n] > w[mwi] {
            mwi = n;
        }
    }
    c.0[mwi]
}

/// `(int) nint(val)` stored in the destination's type after the type's clipping; nothing for a
/// type the function does not write.
fn store(g: &mut Globals, dest: &mut Mri, di: i32, dj: i32, dk: i32, frame: i32, val: f32) {
    let sl = i64::from(dk) + i64::from(frame) * i64::from(dest.vg.depth);
    let (y, x) = (i64::from(dj), i64::from(di));
    let mut v = val;
    let ty = dest.mtype;
    match ty {
        MRI_UCHAR => {
            if f64::from(v) < 0.0 {
                v = 0.0;
            }
            if v > 255.0 {
                v = 255.0;
            }
        }
        MRI_SHORT => {
            if f64::from(v) < -32768.0 {
                v = -32768.0;
            }
            if f64::from(v) > 32767.0 {
                v = 32767.0;
            }
        }
        MRI_USHRT => {
            if v < 0.0 {
                v = 0.0;
            }
            if v > 65535.0 {
                v = 65535.0;
            }
        }
        MRI_INT => {
            // INT_MIN and INT_MAX as floats: -2^31 and 2^31
            if v < -2147483648.0 {
                v = -2147483648.0;
            }
            if v > 2147483648.0 {
                v = 2147483648.0;
            }
        }
        MRI_LONG => {
            // LONG_MIN and LONG_MAX as floats: -2^63 and 2^63
            if v < i64::MIN as f32 {
                v = i64::MIN as f32;
            }
            if v > i64::MAX as f32 {
                v = i64::MAX as f32;
            }
        }
        MRI_FLOAT => {
            rows::write(g, WHAT, dest, sl, y, x, MRI_FLOAT, Num::F(v));
            return;
        }
        _ => return,
    }
    let k = i64::from(nint(g, f64::from(v)));
    let ety = if ty == MRI_LONG {
        g.notices.add(
            &FS_328,
            format!("{WHAT} writes a 64-bit integer volume as 32-bit integers"),
        );
        if g.notices.reproduce() {
            MRI_INT
        } else {
            MRI_LONG
        }
    } else {
        ty
    };
    rows::write(g, WHAT, dest, sl, y, x, ety, Num::I(k));
}

/// The vote (`SAMPLE_VOTE`): every source voxel (frames outermost, then x, y, z) votes, with its
/// value rounded and clipped to [0, 255], for the template voxel `floor(A (x, y, z, 1))`, `A`
/// the template-to-source matrix of the other direction with 0.5 added to its translations;
/// every frame of the template then gets the label of most votes (the lowest on ties) over all
/// the source's frames. `false` for FS-327.
#[allow(clippy::too_many_arguments)]
fn vote(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    template: &Mri,
    dest: &mut Mri,
    sp: &mut Matrix,
    dp: &mut Matrix,
) -> Result<bool, Fatal> {
    let _ = out.write_all(b"resampling using voting...\n");
    let (tw, th, td) = (template.vg.width, template.vg.height, template.vg.depth);
    let mut votes = mri_alloc_sequence(g, out, err, tw, th, td, MRI_UCHAR, 256)?;
    let Some(mut minv) = mri_get_resample_matrix(g, out, err, template, src)? else {
        g.notices.add(
            &FS_327,
            format!("{WHAT}: voting with a template whose voxel-to-RAS matrix FreeSurfer cannot use (FreeSurfer crashes)"),
        );
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("{WHAT}: no matrix from the template to the source for the vote"),
        );
        return Ok(false);
    };
    for r in 1..=3 {
        let t = minv.relt(r, 4);
        minv.set_relt(r, 4, add64(f64::from(t), 0.5) as f32);
    }
    let rep = g.notices.reproduce();
    // FS-326's correction: the votes beyond 255 (template voxel and label)
    let mut extra: HashMap<(i32, i32, i32, i32), u32> = HashMap::new();
    for nframe in 0..src.nframes {
        for si in 0..src.vg.width {
            for sj in 0..src.vg.height {
                for sk in 0..src.vg.depth {
                    sp.set_relt(1, 1, si as f32);
                    sp.set_relt(2, 1, sj as f32);
                    sp.set_relt(3, 1, sk as f32);
                    matrix_multiply(g, out, err, Some(&minv), Some(&*sp), Some(&mut *dp))?;
                    let di = ifloor(dp.relt(1, 1));
                    let dj = ifloor(dp.relt(2, 1));
                    let dk = ifloor(dp.relt(3, 1));
                    if di < 0
                        || dj < 0
                        || dk < 0
                        || di > votes.vg.width - 1
                        || dj > votes.vg.height - 1
                        || dk > votes.vg.depth - 1
                    {
                        continue;
                    }
                    let s = mri_get_vox_val(g, err, src, si, sj, sk, nframe, 0)?;
                    let mut val = nint(g, f64::from(s)) as f32;
                    if val < 0.0 {
                        val = 0.0;
                    } else if val > 255.0 {
                        val = 255.0;
                    }
                    let label = val as i32;
                    let count = cvtt(f64::from(mri_get_vox_val(
                        g, err, &votes, di, dj, dk, label, 0,
                    )?));
                    if count >= 255 {
                        g.notices.add(
                            &FS_326,
                            format!("{WHAT}: a label's votes for a voxel pass 255"),
                        );
                        if !rep {
                            *extra.entry((di, dj, dk, label)).or_insert(0) += 1;
                        }
                    }
                    mri_set_vox_val(g, err, &mut votes, di, dj, dk, label, (count + 1) as f32, 0)?;
                }
            }
        }
    }
    for nframe in 0..template.nframes {
        for di in 0..tw {
            for dj in 0..th {
                for dk in 0..td {
                    let (mut max_label, mut max_count) = (-1i32, -1i64);
                    for label in 0..256 {
                        let mut count = i64::from(cvtt(f64::from(mri_get_vox_val(
                            g, err, &votes, di, dj, dk, label, 0,
                        )?)));
                        if let Some(&e) = extra.get(&(di, dj, dk, label)) {
                            count += i64::from(e);
                        }
                        if count > max_count {
                            max_count = count;
                            max_label = label;
                        }
                    }
                    mri_set_vox_val(g, err, dest, di, dj, dk, nframe, max_label as f32, 0)?;
                }
            }
        }
    }
    Ok(true)
}

/// The destination's geometry: the direction cosines the columns of `src_i_to_r * m` divided by
/// the voxel sizes (`float`s), the centre `src_i_to_r * (m * (width / 2, height / 2, depth / 2,
/// 1))`, `ras_good_flag` 1 (the cached matrices left as `MRIcopyHeader` made them). With
/// `Gdiag`'s show and verbose bits, where RAS (0, 0, 0) falls in both volumes' voxels (`%2.0f`).
fn header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dest: &mut Mri,
    m: &Matrix,
    diag: bool,
) -> Result<(), Fatal> {
    let old_v2r = extract_i_to_r(g, out, err, &src.vg)?;
    let v2r = matrix_multiply(g, out, err, Some(&old_v2r), Some(m), None)?.into_new();
    let e = |r: i32, c: i32| v2r.as_ref().map_or(0.0, |v| v.relt(r, c));
    let vg = &mut dest.vg;
    vg.x_r = div(e(1, 1), vg.xsize);
    vg.x_a = div(e(2, 1), vg.xsize);
    vg.x_s = div(e(3, 1), vg.xsize);
    vg.y_r = div(e(1, 2), vg.ysize);
    vg.y_a = div(e(2, 2), vg.ysize);
    vg.y_s = div(e(3, 2), vg.ysize);
    vg.z_r = div(e(1, 3), vg.zsize);
    vg.z_a = div(e(2, 3), vg.zsize);
    vg.z_s = div(e(3, 3), vg.zsize);
    let mut v_vox = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    v_vox.set_relt(4, 1, 1.0);
    v_vox.set_relt(1, 1, (f64::from(vg.width) / 2.0) as f32);
    v_vox.set_relt(2, 1, (f64::from(vg.height) / 2.0) as f32);
    v_vox.set_relt(3, 1, (f64::from(vg.depth) / 2.0) as f32);
    let v_vox2 = matrix_multiply(g, out, err, Some(m), Some(&v_vox), None)?.into_new();
    let v_ras = matrix_multiply(g, out, err, Some(&old_v2r), v_vox2.as_ref(), None)?.into_new();
    let r = |i: i32| v_ras.as_ref().map_or(0.0, |v| v.relt(i, 1));
    dest.vg.c_r = r(1);
    dest.vg.c_a = r(2);
    dest.vg.c_s = r(3);
    dest.vg.ras_good_flag = 1;
    if diag {
        let new_r2v = extract_r_to_i(g, out, err, &dest.vg)?;
        let old_r2v = extract_r_to_i(g, out, err, &src.vg)?;
        let mut v_ras = v_ras;
        if let Some(v) = v_ras.as_mut() {
            v.set_relt(1, 1, 0.0);
            v.set_relt(2, 1, 0.0);
            v.set_relt(3, 1, 0.0);
        }
        for (which, r2v) in [("old", &old_r2v), ("new", &new_r2v)] {
            matrix_multiply(g, out, err, r2v.as_ref(), v_ras.as_ref(), Some(&mut v_vox))?;
            let f = |i: i32| {
                larmorx_core::cfmt::pad(&larmorx_core::cfmt::f(f64::from(v_vox.relt(i, 1)), 0), 2)
            };
            let _ = writeln!(out, "{which} RAS (0,0,0) -> ({}, {}, {})", f(1), f(2), f(3));
        }
    }
    Ok(())
}
