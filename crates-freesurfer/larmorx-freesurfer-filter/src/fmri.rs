// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/fmriutils.cpp`'s frame functions (queue item 3.1-42): the frame-wise matrix product
//! `fMRImatrixMultiply` (49), the temporal covariance `fMRIcovariance` (120) and AR(1)
//! `fMRItemporalAR1` (193), the frame selections `fMRInskip` (521), `fMRIndrop` (572), `fMRIframe`
//! (623), `fMRIsubSample` (2371) and `fMRIextractFrames` (2412), the frame reductions `MRIvolMax`
//! (1450), `MRIvolMin` (1486), `MRIconjunct` (1531), `MRIvolMaxIndex` (1571), `MRIframeMean`
//! (1683), `MRIframeMedian` (1720) and `MRIframeSum` (1745), the cumulative sum `fMRIcumSum`
//! (2886), the time-course normalisation `MRIframeNorm` (2999), the ASL matrix `ASLinterpMatrix`
//! (2645), the volume-matrix conversions `fMRItoMatrix` (2681) and `fMRIfromMatrix` (2719), and
//! the spatial correlation matrix `fMRIspatialCorMatrix` (2790) with `matrix.cpp`'s
//! `MatrixDemean` (4039, the in-place form it calls).
//!
//! The sums are `double`, in the official build's operand orders (its object code;
//! `larmorx_freesurfer_core::sse` for the NaNs' bits): a covariance adds `(v2 - m) (v1 - m)` to
//! its sum, the frame mean and sum add each value to the sum, the cumulative sum adds the sum to
//! each value, the matrix product adds the output to `v * w`. FS-562 to FS-569.

use std::io::Write;

use larmorx_core::defects::{FS_562, FS_563, FS_564, FS_565, FS_566, FS_567, FS_568, FS_569};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_normalize_col, matrix_reshape, matrix_transpose};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div64, mul, mul64, sqrt, sub, sub64};
use larmorx_freesurfer_core::utils::median;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, Mri, mri_alloc, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters, mri_free,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{mri_clear, mri_clone_by_space};
use larmorx_freesurfer_volume::combine::mri_dim_mismatch;

use crate::mem;

/// `MRIgetVoxVal(m, c, r, s, f)`.
#[allow(clippy::too_many_arguments)]
fn gv(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f32, Fatal> {
    mri_get_vox_val(g, err, m, c, r, s, f, 0)
}

/// `MRIsetVoxVal(m, c, r, s, f, v)`.
#[allow(clippy::too_many_arguments)]
fn sv(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &mut Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
    v: f32,
) -> Result<(), Fatal> {
    mri_set_vox_val(g, err, m, c, r, s, f, v, 0)?;
    Ok(())
}

/// A destination: the caller's, or a new one.
fn result(fresh: Option<Mri>) -> Ret<Mri> {
    match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    }
}

fn same_space(a: &Mri, b: &Mri) -> bool {
    a.vg.width == b.vg.width && a.vg.height == b.vg.height && a.vg.depth == b.vg.depth
}

/// A new volume of `src`'s extent, `mtype` and `nframes`, with `src`'s header.
fn alloc_like(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    mtype: i32,
    nframes: i32,
) -> Result<Mri, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut m = mri_alloc_sequence(g, out, err, w, h, d, mtype, nframes)?;
    mri_copy_header(g, out, err, src, Some(&mut m))?;
    Ok(m)
}

/// `MRIFseq_vox(m, c, r, s, 0) = v`: a direct float write (FS-562 where `m` is not a float volume
/// holding the voxel).
fn put_direct(g: &mut Globals, what: &str, m: &mut Mri, c: i32, r: i32, s: i32, v: f32) {
    mem::write(
        g,
        (&FS_562, &FS_562),
        what,
        m,
        i64::from(s),
        i64::from(r),
        i64::from(c),
        &v.to_le_bytes(),
    );
}

/// `MRIFseq_vox(m, c, r, s, 0)` of a float volume holding the voxel.
fn get_direct(m: &Mri, c: i32, r: i32, s: i32) -> f32 {
    match mem::at(m, i64::from(s), i64::from(r), i64::from(c), 4) {
        mem::At::In(o) => mem::value(m, o, MRI_FLOAT),
        _ => unreachable!("a covariance volume of the input's extent"),
    }
}

/// Whether a `float` or `double` converts to `int` without x86-64's `INT_MIN` for NaN and values
/// outside the range.
fn int_range(x: f64) -> bool {
    !(x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0)
}

/// `fMRImatrixMultiply(inmri, M, outmri)`: frame `fout` of the output is the sum over the input's
/// frames `fin` of `M[fout + 1][fin + 1]` times frame `fin`, in `float`, added frame after frame
/// to the cleared output (`v * w + out`). A new output is float of `M->rows` frames with the
/// input's header; errors on standard output (`ERROR: fMRImatrixMultiply: ...`) and `Null`: the
/// input's frames not `M`'s columns (`input dimension mismatch`), an input that is not float
/// (`input is not MRI_FLOAT`), a given output of another extent or frame count (`output dimension
/// mismatch`) or not float (`output is not MRI_FLOAT`).
pub fn fmri_matrix_multiply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    inmri: &Mri,
    m: &Matrix,
    outmri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if inmri.nframes != m.cols {
        let _ = writeln!(out, "ERROR: fMRImatrixMultiply: input dimension mismatch");
        return Ok(Ret::Null);
    }
    if inmri.mtype != MRI_FLOAT {
        let _ = writeln!(out, "ERROR: fMRImatrixMultiply: input is not MRI_FLOAT");
        return Ok(Ret::Null);
    }
    let nframesout = m.rows;
    let mut fresh = None;
    let dst: &mut Mri = match outmri {
        None => fresh.insert(alloc_like(g, out, err, inmri, MRI_FLOAT, nframesout)?),
        Some(o) => {
            if !same_space(o, inmri) || o.nframes != nframesout {
                let _ = writeln!(out, "ERROR: fMRImatrixMultiply: output dimension mismatch");
                return Ok(Ret::Null);
            }
            if o.mtype != MRI_FLOAT {
                let _ = writeln!(out, "ERROR: fMRImatrixMultiply: output is not MRI_FLOAT");
                return Ok(Ret::Null);
            }
            o
        }
    };
    mri_clear(g, err, dst)?;
    let (w, h, d) = (dst.vg.width, dst.vg.height, dst.vg.depth);
    for fout in 0..dst.nframes {
        for fin in 0..inmri.nframes {
            let val = m.relt(fout + 1, fin + 1);
            for s in 0..d {
                let nin = i64::from(s) + i64::from(fin) * i64::from(inmri.vg.depth);
                let nout = i64::from(s) + i64::from(fout) * i64::from(d);
                for r in 0..h {
                    for c in 0..w {
                        let (mem::At::In(oi), mem::At::In(oo)) = (
                            mem::at(inmri, nin, i64::from(r), i64::from(c), 4),
                            mem::at(dst, nout, i64::from(r), i64::from(c), 4),
                        ) else {
                            unreachable!("float volumes of one extent")
                        };
                        let v = mem::value(inmri, oi, MRI_FLOAT);
                        let o = mem::value(dst, oo, MRI_FLOAT);
                        let t = add(mul(v, val), o);
                        dst.chunk.as_mut().expect("voxels")[oo..oo + 4]
                            .copy_from_slice(&t.to_le_bytes());
                    }
                }
            }
        }
    }
    Ok(result(fresh))
}

/// `fMRIcovariance(fmri, Lag, DOFAdjust, mask, covar)`: at each voxel (columns outermost) the sum
/// over frames `f < nframes - Lag` of `(v[f + Lag] - m) (v[f] - m)` in `double`, divided by
/// `DOFLag = (int)(nframes - DOFAdjust) - Lag` (the subtraction in `float`), stored as a float in
/// frame 0; `m` is the voxel's frame mean when `DOFAdjust` is negative (then taken as 1), else 0;
/// voxels whose mask value is below 0.5 get 0. A new output is float of one frame with the input's
/// header; a given one is written directly as floats whatever its type and extent (FS-562).
/// Errors on standard output and `Null`: `ERROR: fMRIcovariance: Lag out of bound <Lag> ` for a
/// lag outside `[0, nframes)`, `ERROR: fMRIcovariance: DOFLag = <n> <= 0 (DOF=<d>, Lag=<l>)`.
/// FS-565 (the conversion of a NaN or out-of-range DOF: by default saturated, NaN 0).
#[allow(clippy::too_many_arguments)]
pub fn fmri_covariance(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fmri: &Mri,
    lag: i32,
    dof_adjust: f32,
    mask: Option<&Mri>,
    covar: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "fMRIcovariance";
    let mut dof_adjust = dof_adjust;
    let remove_mean = dof_adjust < 0.0;
    if remove_mean {
        dof_adjust = 1.0;
    }
    if lag < 0 || lag >= fmri.nframes {
        let _ = writeln!(out, "ERROR: fMRIcovariance: Lag out of bound {lag} ");
        return Ok(Ret::Null);
    }
    let x = sub(fmri.nframes as f32, dof_adjust);
    let (dof, doflag) = if (-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        let dof = x.trunc() as i32;
        (dof, dof.wrapping_sub(lag))
    } else {
        g.notices.add(
            &FS_565,
            format!("{WHAT}: the number of degrees of freedom {x} is outside the int range"),
        );
        if g.notices.reproduce() {
            (i32::MIN, i32::MIN.wrapping_sub(lag))
        } else {
            let dof = if x.is_nan() {
                0
            } else if x > 0.0 {
                i32::MAX
            } else {
                i32::MIN
            };
            (dof, dof.saturating_sub(lag))
        }
    };
    if doflag <= 0 {
        let _ = writeln!(
            out,
            "ERROR: fMRIcovariance: DOFLag = {doflag} <= 0 (DOF={dof}, Lag={lag})"
        );
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let cv: &mut Mri = match covar {
        Some(c) => c,
        None => fresh.insert(alloc_like(g, out, err, fmri, MRI_FLOAT, 1)?),
    };
    let mean = if remove_mean {
        mri_frame_mean(g, out, err, fmri, None, &[])?.into_new()
    } else {
        None
    };
    let mut valmean = 0.0f64;
    for c in 0..fmri.vg.width {
        for r in 0..fmri.vg.height {
            for s in 0..fmri.vg.depth {
                if let Some(mk) = mask
                    && gv(g, err, mk, c, r, s, 0)? < 0.5
                {
                    put_direct(g, WHAT, cv, c, r, s, 0.0);
                    continue;
                }
                if let Some(mn) = mean.as_ref() {
                    valmean = f64::from(gv(g, err, mn, c, r, s, 0)?);
                }
                let mut sum = 0.0f64;
                for f in 0..fmri.nframes - lag {
                    let v1 = f64::from(gv(g, err, fmri, c, r, s, f)?);
                    let v2 = f64::from(gv(g, err, fmri, c, r, s, f + lag)?);
                    sum = add64(sum, mul64(sub64(v2, valmean), sub64(v1, valmean)));
                }
                put_direct(g, WHAT, cv, c, r, s, div64(sum, f64::from(doflag)) as f32);
            }
        }
    }
    Ok(result(fresh))
}

/// FS-564: a covariance FreeSurfer reads although it could not be computed.
fn missing_covariance(g: &mut Globals, err: &mut dyn Write, what: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_564,
        format!(
            "{what}: the covariance could not be computed (FreeSurfer reads a NULL volume here)"
        ),
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!(
            "{what}(): the temporal covariance could not be computed (FreeSurfer crashes here)\n"
        ),
    )
}

/// `fMRItemporalAR1(fmri, DOFAdjust, mask, ar1)`: the lag-1 covariance over the lag-0 one
/// ([`fmri_covariance`] with the same adjustment and mask; their messages first), in `double`,
/// stored as a float in frame 0; masked voxels 0; a voxel of variance 0 gets 0 (FS-563: FreeSurfer
/// divides by it). A new output is float of one frame with the input's header; a given one is
/// written directly as floats (FS-562). FS-564: a covariance that could not be computed ends the
/// program at the first voxel not masked out.
#[allow(clippy::too_many_arguments)]
pub fn fmri_temporal_ar1(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fmri: &Mri,
    dof_adjust: f32,
    mask: Option<&Mri>,
    ar1: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "fMRItemporalAR1";
    let var = fmri_covariance(g, out, err, fmri, 0, dof_adjust, mask, None)?.into_new();
    let covar = fmri_covariance(g, out, err, fmri, 1, dof_adjust, mask, None)?.into_new();
    let mut fresh = None;
    let a: &mut Mri = match ar1 {
        Some(a) => a,
        None => fresh.insert(alloc_like(g, out, err, fmri, MRI_FLOAT, 1)?),
    };
    for c in 0..fmri.vg.width {
        for r in 0..fmri.vg.height {
            for s in 0..fmri.vg.depth {
                if let Some(mk) = mask
                    && gv(g, err, mk, c, r, s, 0)? < 0.5
                {
                    put_direct(g, WHAT, a, c, r, s, 0.0);
                    continue;
                }
                let (Some(var), Some(covar)) = (var.as_ref(), covar.as_ref()) else {
                    missing_covariance(g, err, WHAT)?;
                    return Ok(Ret::Null);
                };
                let voxvar = f64::from(get_direct(var, c, r, s));
                if voxvar == 0.0 {
                    g.notices.add(
                        &FS_563,
                        format!(
                            "{WHAT}: voxel ({c}, {r}, {s}) has variance 0; FreeSurfer divides by it"
                        ),
                    );
                    if !g.notices.reproduce() {
                        put_direct(g, WHAT, a, c, r, s, 0.0);
                        continue;
                    }
                }
                let voxcovar = f64::from(get_direct(covar, c, r, s));
                put_direct(g, WHAT, a, c, r, s, div64(voxcovar, voxvar) as f32);
            }
        }
    }
    Ok(result(fresh))
}

/// The frame copy of `fMRInskip`, `fMRIndrop` and `fMRIframe`: `nframesout` frames, output frame
/// `k` from input frame `k + first`, through `MRIgetVoxVal`/`MRIsetVoxVal`, into a new volume of
/// the input's type (header; `pulse`: and pulse parameters) or a given one of the input's extent,
/// frame count and type (else `ERROR: <what>: output dimension mismatch` or `... structure type
/// mismatch`), cleared first.
#[allow(clippy::too_many_arguments)]
fn copy_frames(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    what: &str,
    inmri: &Mri,
    first: i32,
    nframesout: i32,
    pulse: bool,
    outmri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let dst: &mut Mri = match outmri {
        None => {
            let mut d = alloc_like(g, out, err, inmri, inmri.mtype, nframesout)?;
            if pulse {
                mri_copy_pulse_parameters(inmri, &mut d);
            }
            fresh.insert(d)
        }
        Some(o) => {
            if !same_space(o, inmri) || o.nframes != nframesout {
                let _ = writeln!(out, "ERROR: {what}: output dimension mismatch");
                return Ok(Ret::Null);
            }
            if o.mtype != inmri.mtype {
                let _ = writeln!(out, "ERROR: {what}: structure type mismatch");
                return Ok(Ret::Null);
            }
            o
        }
    };
    mri_clear(g, err, dst)?;
    for fout in 0..dst.nframes {
        let fin = fout.wrapping_add(first);
        for s in 0..dst.vg.depth {
            for r in 0..dst.vg.height {
                for c in 0..dst.vg.width {
                    let v = gv(g, err, inmri, c, r, s, fin)?;
                    sv(g, err, dst, c, r, s, fout, v)?;
                }
            }
        }
    }
    Ok(result(fresh))
}

/// `fMRInskip(inmri, nskip, outmri)`: the frames after the first `nskip` (`ERROR: fMRInskip:
/// nskip >= nframes` and `Null` when none is left).
pub fn fmri_nskip(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    inmri: &Mri,
    nskip: i32,
    outmri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if inmri.nframes <= nskip {
        let _ = writeln!(out, "ERROR: fMRInskip: nskip >= nframes");
        return Ok(Ret::Null);
    }
    let n = inmri.nframes.wrapping_sub(nskip);
    copy_frames(g, out, err, "fMRInskip", inmri, nskip, n, false, outmri)
}

/// `fMRIndrop(inmri, ndrop, outmri)`: the frames before the last `ndrop` (`ERROR: fMRIndrop:
/// ndrop >= nframes` and `Null` when none is left).
pub fn fmri_ndrop(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    inmri: &Mri,
    ndrop: i32,
    outmri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if inmri.nframes <= ndrop {
        let _ = writeln!(out, "ERROR: fMRIndrop: ndrop >= nframes");
        return Ok(Ret::Null);
    }
    let n = inmri.nframes.wrapping_sub(ndrop);
    copy_frames(g, out, err, "fMRIndrop", inmri, 0, n, false, outmri)
}

/// `fMRIframe(inmri, frame, outmri)`: one frame (0-based; `ERROR: fMRIframe: frame >= nframes`
/// and `Null` past the last); a new output also takes the pulse parameters.
pub fn fmri_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    inmri: &Mri,
    frame: i32,
    outmri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if inmri.nframes <= frame {
        let _ = writeln!(out, "ERROR: fMRIframe: frame >= nframes");
        return Ok(Ret::Null);
    }
    copy_frames(g, out, err, "fMRIframe", inmri, frame, 1, true, outmri)
}

/// A one-frame destination of `MRIvolMax` and its kin: the caller's (of the input's extent, else
/// `ERROR: <name>: dimension mismatch` and `None`), or a new one of `mtype` with the input's
/// header.
#[allow(clippy::too_many_arguments)]
fn one_frame_dst<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    name: &str,
    invol: &Mri,
    mtype: i32,
    given: Option<&'a mut Mri>,
    fresh: &'a mut Option<Mri>,
) -> Result<Option<&'a mut Mri>, Fatal> {
    let d: &mut Mri = match given {
        Some(d) => d,
        None => {
            let (w, h, dd) = (invol.vg.width, invol.vg.height, invol.vg.depth);
            let mut m = mri_alloc(g, out, err, w, h, dd, mtype)?;
            mri_copy_header(g, out, err, invol, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    if !same_space(d, invol) {
        let _ = writeln!(out, "ERROR: {name}: dimension mismatch");
        return Ok(None);
    }
    Ok(Some(d))
}

/// `MRIvolMax` (`max`) and `MRIvolMin`: the largest (smallest) value over the frames at each
/// voxel, from frame 0, a frame replacing it only when larger (smaller): NaNs never replace and a
/// NaN in frame 0 stays.
fn vol_extreme(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    dst: Option<&mut Mri>,
    max: bool,
) -> Result<Ret<Mri>, Fatal> {
    let name = if max { "MRIvolMax" } else { "MRIvolMin" };
    let mut fresh = None;
    let Some(d) = one_frame_dst(g, out, err, name, invol, invol.mtype, dst, &mut fresh)? else {
        return Ok(Ret::Null);
    };
    for c in 0..invol.vg.width {
        for r in 0..invol.vg.height {
            for s in 0..invol.vg.depth {
                let mut m = f64::from(gv(g, err, invol, c, r, s, 0)?);
                for f in 1..invol.nframes {
                    let v = f64::from(gv(g, err, invol, c, r, s, f)?);
                    if (max && m < v) || (!max && m > v) {
                        m = v;
                    }
                }
                sv(g, err, d, c, r, s, 0, m as f32)?;
            }
        }
    }
    Ok(result(fresh))
}

/// `MRIvolMax(invol, out)`: the maximum over the frames at each voxel; a new output has the
/// input's type; `ERROR: MRIvolMax: dimension mismatch` and `Null` for a given one of another
/// extent.
pub fn mri_vol_max(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    vol_extreme(g, out, err, invol, dst, true)
}

/// `MRIvolMin(invol, out)`: as [`mri_vol_max`], the minimum.
pub fn mri_vol_min(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    vol_extreme(g, out, err, invol, dst, false)
}

/// `MRIconjunct(invol, out)`: at each voxel the frame value of the smallest magnitude with its
/// sign (`|v| * sign`, the sign 1 for a value above 0 and -1 otherwise, so 0 gives -0); from frame
/// 0, a later frame replacing it only when its magnitude is smaller. A new output is float.
pub fn mri_conjunct(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let Some(d) = one_frame_dst(
        g,
        out,
        err,
        "MRIconjunct",
        invol,
        MRI_FLOAT,
        dst,
        &mut fresh,
    )?
    else {
        return Ok(Ret::Null);
    };
    let sign = |v: f32| if v > 0.0 { 1.0f64 } else { -1.0 };
    for c in 0..invol.vg.width {
        for r in 0..invol.vg.height {
            for s in 0..invol.vg.depth {
                let v = gv(g, err, invol, c, r, s, 0)?;
                let mut min = f64::from(v).abs();
                let mut minsign = sign(v);
                for f in 0..invol.nframes {
                    let v = gv(g, err, invol, c, r, s, f)?;
                    let a = f64::from(v).abs();
                    if min > a {
                        min = a;
                        minsign = sign(v);
                    }
                }
                sv(g, err, d, c, r, s, 0, mul64(min, minsign) as f32)?;
            }
        }
    }
    Ok(result(fresh))
}

/// `MRIvolMaxIndex(invol, base, mask, out)`: at each voxel the first frame of the largest value
/// (a later frame replacing it only when larger) plus `base`; 0 where the mask is below 0.5 (and
/// before each voxel is looked at). A new output is int; `ERROR: MRIvolMax: dimension mismatch`
/// and `Null` for a given one of another extent.
#[allow(clippy::too_many_arguments)]
pub fn mri_vol_max_index(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    base: i32,
    mask: Option<&Mri>,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let Some(d) = one_frame_dst(g, out, err, "MRIvolMax", invol, MRI_INT, dst, &mut fresh)? else {
        return Ok(Ret::Null);
    };
    for c in 0..invol.vg.width {
        for r in 0..invol.vg.height {
            for s in 0..invol.vg.depth {
                sv(g, err, d, c, r, s, 0, 0.0)?;
                if let Some(mk) = mask
                    && f64::from(gv(g, err, mk, c, r, s, 0)?) < 0.5
                {
                    continue;
                }
                let mut max = f64::from(gv(g, err, invol, c, r, s, 0)?);
                let mut index = 0i32;
                for f in 1..invol.nframes {
                    let v = f64::from(gv(g, err, invol, c, r, s, f)?);
                    if max < v {
                        max = v;
                        index = f;
                    }
                }
                sv(g, err, d, c, r, s, 0, index.wrapping_add(base) as f32)?;
            }
        }
    }
    Ok(result(fresh))
}

/// A new float volume of one frame with `vol`'s header, or the caller's.
fn frame_dst<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    given: Option<&'a mut Mri>,
    fresh: &'a mut Option<Mri>,
) -> Result<&'a mut Mri, Fatal> {
    Ok(match given {
        Some(d) => d,
        None => fresh.insert(alloc_like(g, out, err, vol, MRI_FLOAT, 1)?),
    })
}

/// `MRIframeMean(vol, volmn, FrameWeight)`: the sum over the frames (each value times its weight
/// when weights are given) in `double`, divided by the number of frames, in frame 0 of a new float
/// volume with the input's header or of the caller's (by `MRIsetVoxVal`). `ERROR: MRIframeMean():
/// dim mismatch: weight=<n>, vol=<nframes>` and `Null` for weights of another count.
pub fn mri_frame_mean(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    volmn: Option<&mut Mri>,
    weights: &[f64],
) -> Result<Ret<Mri>, Fatal> {
    if !weights.is_empty() && weights.len() != vol.nframes as usize {
        let _ = writeln!(
            out,
            "ERROR: MRIframeMean(): dim mismatch: weight={}, vol={}",
            weights.len() as i32,
            vol.nframes
        );
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let d = frame_dst(g, out, err, vol, volmn, &mut fresh)?;
    for c in 0..vol.vg.width {
        for r in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                let mut v = 0.0f64;
                for f in 0..vol.nframes {
                    let mut val = f64::from(gv(g, err, vol, c, r, s, f)?);
                    if !weights.is_empty() {
                        val = mul64(val, weights[f as usize]);
                    }
                    v = add64(v, val);
                }
                sv(
                    g,
                    err,
                    d,
                    c,
                    r,
                    s,
                    0,
                    div64(v, f64::from(vol.nframes)) as f32,
                )?;
            }
        }
    }
    Ok(result(fresh))
}

/// `MRIframeMedian(vol, volmn)`: the median of the frames (`median`) at each voxel, in frame 0 of
/// a new float volume with the input's header or of the caller's.
pub fn mri_frame_median(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    volmn: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d = frame_dst(g, out, err, vol, volmn, &mut fresh)?;
    let mut t = vec![0.0f32; vol.nframes.max(0) as usize];
    for c in 0..vol.vg.width {
        for r in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                for f in 0..vol.nframes {
                    t[f as usize] = gv(g, err, vol, c, r, s, f)?;
                }
                let m = median(g, &mut t);
                sv(g, err, d, c, r, s, 0, m)?;
            }
        }
    }
    Ok(result(fresh))
}

/// `MRIframeSum(vol, volsum)`: the sum over the frames in `double`, in frame 0 of a new float
/// volume with the input's header or of the caller's.
pub fn mri_frame_sum(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    volsum: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d = frame_dst(g, out, err, vol, volsum, &mut fresh)?;
    for c in 0..vol.vg.width {
        for r in 0..vol.vg.height {
            for s in 0..vol.vg.depth {
                let mut v = 0.0f64;
                for f in 0..vol.nframes {
                    v = add64(v, f64::from(gv(g, err, vol, c, r, s, f)?));
                }
                sv(g, err, d, c, r, s, 0, v as f32)?;
            }
        }
    }
    Ok(result(fresh))
}

/// `fMRIsubSample(f, Start, Delta, Stop, fsub)`: the frames `Start, Start + Delta, ...` below
/// `Stop` (the frame count when `Stop` is negative) into consecutive frames of a new float volume
/// (`MRIcloneBySpace`) of `ceil((Stop - Start) / Delta)` frames or of the caller's. Errors on
/// standard output and `Null`: `ERROR: fMRIsubSample(): Start > Stop`, `... Delta == 0`, `...
/// frame mismatch (<n>, <frames>)` for a volume of another frame count (a negative count clones
/// the input's). FS-565 (a count outside the `int` range; by default an error), FS-569 (a step
/// that never reaches `Stop`: an error).
#[allow(clippy::too_many_arguments)]
pub fn fmri_sub_sample(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    f: &Mri,
    start: i32,
    delta: i32,
    stop: i32,
    fsub: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "fMRIsubSample";
    let stop = if stop < 0 { f.nframes } else { stop };
    if start > stop {
        let _ = writeln!(out, "ERROR: fMRIsubSample(): Start > Stop");
        return Ok(Ret::Null);
    }
    if delta == 0 {
        let _ = writeln!(out, "ERROR: fMRIsubSample(): Delta == 0");
        return Ok(Ret::Null);
    }
    let q = ((f64::from(stop) - f64::from(start)) / f64::from(delta)).ceil();
    let nframessub = if int_range(q) {
        q as i32
    } else {
        g.notices.add(
            &FS_565,
            format!("{WHAT}: a frame count of {q} is outside the int range"),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                out,
                "ERROR: fMRIsubSample(): {q} frames is outside the int range"
            );
            return Ok(Ret::Null);
        }
        i32::MIN
    };
    let mut fresh = None;
    let d: &mut Mri = match fsub {
        Some(d) => d,
        None => fresh.insert(mri_clone_by_space(g, out, err, f, MRI_FLOAT, nframessub)?),
    };
    if nframessub != d.nframes {
        let _ = writeln!(
            out,
            "ERROR: fMRIsubSample(): frame mismatch ({nframessub}, {})",
            d.nframes
        );
        return Ok(Ret::Null);
    }
    // the frames visited: FreeSurfer's loop runs without end (reading before the volume's memory
    // until it crashes) when the step is negative and Start below Stop, or when the frame number
    // passes the int range (FS-569)
    let mut frames = Vec::new();
    if f.vg.width > 0 && f.vg.height > 0 && f.vg.depth > 0 {
        let mut frame = start;
        while frame < stop {
            frames.push(frame);
            match frame.checked_add(delta) {
                Some(x) if delta > 0 => frame = x,
                _ => {
                    g.notices.add(
                        &FS_569,
                        format!(
                            "{WHAT}: Start {start}, Delta {delta}, Stop {stop} never reach Stop"
                        ),
                    );
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        "fMRIsubSample(): the frames never reach Stop (FreeSurfer reads without end and crashes here)\n",
                    )?;
                    return Ok(Ret::Null);
                }
            }
        }
    }
    for c in 0..f.vg.width {
        for r in 0..f.vg.height {
            for s in 0..f.vg.depth {
                for (k, &frame) in frames.iter().enumerate() {
                    let v = f64::from(gv(g, err, f, c, r, s, frame)?);
                    sv(g, err, d, c, r, s, k as i32, v as f32)?;
                }
            }
        }
    }
    Ok(result(fresh))
}

/// `fMRIextractFrames(mri, framelist)`: the listed frames, in order, into a new volume of the
/// input's type (`MRIcloneBySpace`, pulse parameters copied). `ERROR: fMRIextractFrames():
/// framelist is empty` or `... f=<f> >= <nframes>` and `None`; negative frames are read as
/// `MRIgetVoxVal` reads them.
pub fn fmri_extract_frames(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    framelist: &[i32],
) -> Result<Option<Mri>, Fatal> {
    if framelist.is_empty() {
        let _ = writeln!(out, "ERROR: fMRIextractFrames(): framelist is empty");
        return Ok(None);
    }
    for &f in framelist {
        if f >= mri.nframes {
            let _ = writeln!(out, "ERROR: fMRIextractFrames(): f={f} >= {}", mri.nframes);
            return Ok(None);
        }
    }
    let mut ss = mri_clone_by_space(g, out, err, mri, mri.mtype, framelist.len() as i32)?;
    mri_copy_pulse_parameters(mri, &mut ss);
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                for (n, &f) in framelist.iter().enumerate() {
                    let v = f64::from(gv(g, err, mri, c, r, s, f)?);
                    sv(g, err, &mut ss, c, r, s, n as i32, v as f32)?;
                }
            }
        }
    }
    Ok(Some(ss))
}

/// `*MATRIX_RELT(m, r, c) = v` where the place may lie past the matrix's numbers: written there
/// under `reproduce` when it is inside the allocation, dropped by default (FS-566).
fn put_relt(g: &mut Globals, what: &str, m: &mut Matrix, r: i32, c: i32, v: f32) {
    if r >= 1 && r <= m.rows && c >= 1 && c <= m.cols {
        m.set_relt(r, c, v);
        return;
    }
    g.notices.add(
        &FS_566,
        format!(
            "{what}: element ({r}, {c}) of a {} x {} matrix",
            m.rows, m.cols
        ),
    );
    if g.notices.reproduce()
        && let Some(i) = m.relt_index(r, c)
    {
        m.set_raw(i, v);
    }
}

/// `ASLinterpMatrix(ntp)`: the `ceil(ntp / 2)` x `ntp` matrix that subtracts from each tag (the
/// odd time points, 1-based) the mean of its neighbouring controls: row 1 `+1` at column 1 and
/// `-1` at 2; for an odd `ntp` the last row `-1` before and `+1` at its tag; the other rows `-0.5,
/// +1, -0.5` around their tag. FS-566 (one time point: the `-1` falls past the matrix).
pub fn asl_interp_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ntp: i32,
) -> Result<Option<Matrix>, Fatal> {
    const WHAT: &str = "ASLinterpMatrix";
    let is_odd = ntp % 2 != 0;
    let nrows = (f64::from(ntp) / 2.0).ceil() as i32;
    let Some(mut m) = matrix_alloc(g, out, err, nrows, ntp, MATRIX_REAL)? else {
        return Ok(None);
    };
    for r in 1..=nrows {
        let c0 = 2 * (r - 1) + 1;
        if r == 1 {
            put_relt(g, WHAT, &mut m, r, c0, 1.0);
            put_relt(g, WHAT, &mut m, r, c0 + 1, -1.0);
            continue;
        }
        if r == nrows && is_odd {
            put_relt(g, WHAT, &mut m, r, c0, 1.0);
            put_relt(g, WHAT, &mut m, r, c0 - 1, -1.0);
            continue;
        }
        put_relt(g, WHAT, &mut m, r, c0 - 1, -0.5);
        put_relt(g, WHAT, &mut m, r, c0, 1.0);
        put_relt(g, WHAT, &mut m, r, c0 + 1, -0.5);
    }
    Ok(Some(m))
}

/// `fMRItoMatrix(fmri, M)`: frame `f` of voxel `n` (slices outermost, columns fastest) into
/// element `(f + 1, n + 1)` of a new real `nframes` x `nvox` matrix or of the caller's. FS-567: a
/// given matrix with fewer rows or columns (FreeSurfer writes into other elements and past it;
/// by default `ERROR: fMRItoMatrix: the matrix is <r> x <c>, the volume needs <f> x <n>` and
/// `Null`).
pub fn fmri_to_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fmri: &Mri,
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    const WHAT: &str = "fMRItoMatrix";
    let nvox = fmri
        .vg
        .width
        .wrapping_mul(fmri.vg.height)
        .wrapping_mul(fmri.vg.depth);
    let mut fresh = None;
    let mm: &mut Matrix = match m {
        Some(m) => m,
        None => match matrix_alloc(g, out, err, fmri.nframes, nvox, MATRIX_REAL)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    let small = mm.rows < fmri.nframes || mm.cols < nvox;
    if small {
        g.notices.add(
            &FS_567,
            format!(
                "{WHAT}: a {} x {} matrix for {} frames of {nvox} voxels",
                mm.rows, mm.cols, fmri.nframes
            ),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                out,
                "ERROR: fMRItoMatrix: the matrix is {} x {}, the volume needs {} x {nvox}",
                mm.rows, mm.cols, fmri.nframes
            );
            return Ok(Ret::Null);
        }
    }
    let mut nthcol = 0i32;
    for s in 0..fmri.vg.depth {
        for r in 0..fmri.vg.height {
            for c in 0..fmri.vg.width {
                for f in 0..fmri.nframes {
                    let v = f64::from(gv(g, err, fmri, c, r, s, f)?) as f32;
                    if !small {
                        mm.set_relt(f + 1, nthcol + 1, v);
                    } else if f < mm.rows
                        && let Some(i) = mm.relt_index(f + 1, nthcol + 1)
                    {
                        // a row FreeSurfer has a pointer for: its element, or past it
                        mm.set_raw(i, v);
                    }
                }
                nthcol += 1;
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// FS-568: `fMRIfromMatrix`'s crash.
fn from_matrix_crash(g: &mut Globals, err: &mut dyn Write, why: &str) -> Result<i32, Fatal> {
    g.notices.add(&FS_568, format!("fMRIfromMatrix: {why}"));
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("fMRIfromMatrix(): {why} (FreeSurfer crashes here)\n"),
    )?;
    Ok(1)
}

/// `fMRIfromMatrix(M, fmri, mask)`: the inverse of [`fmri_to_matrix`] over the voxels whose mask
/// value is 0.5 or more (all without a mask): row `f + 1` of the voxel's column into frame `f`
/// (`MRIsetVoxVal`). A matrix whose columns are a multiple `k` of the voxels is first reshaped to
/// `k` rows of `nvox` (its first elements, row by row). 0, or 1 after `fMRIfromMatrix(): mask
/// dimension mismatch`, `ERROR: fMRIfromMatrix: dim mismatch nvox=<n>, mat <rows> <cols>` or
/// `ERROR: fMRIfromMatrix: dim mismatch` (rows not the frames). FS-568 (no voxel in the mask:
/// FreeSurfer divides by 0; an error).
pub fn fmri_from_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    fmri: &mut Mri,
    mask: Option<&Mri>,
) -> Result<i32, Fatal> {
    let mut nvox = 0i32;
    if let Some(mk) = mask {
        if mri_dim_mismatch(fmri, mk, false) != 0 {
            let _ = writeln!(out, "fMRIfromMatrix(): mask dimension mismatch");
            return Ok(1);
        }
        for s in 0..mk.vg.depth {
            for r in 0..mk.vg.height {
                for c in 0..mk.vg.width {
                    if gv(g, err, mk, c, r, s, 0)? < 0.5 {
                        continue;
                    }
                    nvox += 1;
                }
            }
        }
    } else {
        nvox = fmri
            .vg
            .width
            .wrapping_mul(fmri.vg.height)
            .wrapping_mul(fmri.vg.depth);
    }
    let reshaped;
    let muse: &Matrix = if nvox != m.cols {
        if nvox == 0 {
            return from_matrix_crash(g, err, "the mask selects no voxel");
        }
        if m.cols % nvox != 0 {
            let _ = writeln!(
                out,
                "ERROR: fMRIfromMatrix: dim mismatch nvox={nvox}, mat {} {}",
                m.rows, m.cols
            );
            return Ok(1);
        }
        let nr = m.cols / nvox;
        match matrix_reshape(g, out, err, m, None, nr, nvox)? {
            Ret::New(r) => {
                reshaped = r;
                &reshaped
            }
            _ => return from_matrix_crash(g, err, "the matrix could not be reshaped"),
        }
    } else {
        m
    };
    if muse.cols != nvox || muse.rows != fmri.nframes {
        let _ = writeln!(out, "ERROR: fMRIfromMatrix: dim mismatch");
        return Ok(1);
    }
    let mut nthcol = 0i32;
    for s in 0..fmri.vg.depth {
        for r in 0..fmri.vg.height {
            for c in 0..fmri.vg.width {
                if let Some(mk) = mask
                    && gv(g, err, mk, c, r, s, 0)? < 0.5
                {
                    continue;
                }
                for f in 0..fmri.nframes {
                    let v = f64::from(muse.relt(f + 1, nthcol + 1));
                    sv(g, err, fmri, c, r, s, f, v as f32)?;
                }
                nthcol += 1;
            }
        }
    }
    Ok(0)
}

/// `MatrixDemean(M, M)` (`utils/matrix.cpp` 4039, the in-place call): each column's mean (the
/// `double` sum of its elements from the first row, over the rows) subtracted from its elements
/// in `double`, stored as floats.
pub fn matrix_demean_in_place(m: &mut Matrix) {
    for c in 1..=m.cols {
        let mut vsum = 0.0f64;
        for r in 1..=m.rows {
            vsum = add64(vsum, f64::from(m.relt(r, c)));
        }
        let vmean = div64(vsum, f64::from(m.rows));
        for r in 1..=m.rows {
            let v = sub64(f64::from(m.relt(r, c)), vmean) as f32;
            m.set_relt(r, c, v);
        }
    }
}

/// `fMRIspatialCorMatrix(fmri)`: the voxels' Pearson correlations: the series as a matrix
/// ([`fmri_to_matrix`]), each column demeaned and divided by its length, `M' M` in `double`
/// (`MatrixMultiplyD`), into a new float volume of the input's extent with one frame per voxel
/// ([`fmri_from_matrix`]); a progress line on standard output before each step
/// (`fMRIspatialCorMatrix: creating matrix`, `demeaing`, `normalizing`, `creating matrix
/// transpose`, `multiplying`, `allocating SCM`, `filling SCM`).
pub fn fmri_spatial_cor_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fmri: &Mri,
) -> Result<Option<Mri>, Fatal> {
    let _ = writeln!(out, "fMRIspatialCorMatrix: creating matrix");
    let Ret::New(mut m) = fmri_to_matrix(g, out, err, fmri, None)? else {
        return Ok(None);
    };
    let _ = writeln!(out, "fMRIspatialCorMatrix: demeaing");
    matrix_demean_in_place(&mut m);
    let _ = writeln!(out, "fMRIspatialCorMatrix: normalizing");
    let copy = m.clone();
    if matrix_normalize_col(g, out, err, &copy, Some(&mut m), None)?.is_null() {
        return Ok(None);
    }
    let _ = writeln!(out, "fMRIspatialCorMatrix: creating matrix transpose");
    let Ret::New(mt) = matrix_transpose(g, out, err, &m, None)? else {
        return Ok(None);
    };
    let _ = writeln!(out, "fMRIspatialCorMatrix: multiplying");
    let Ret::New(mtm) = matrix_multiply_d(g, out, err, Some(&mt), Some(&m), None)? else {
        return Ok(None);
    };
    let _ = writeln!(out, "fMRIspatialCorMatrix: allocating SCM");
    let (w, h, d) = (fmri.vg.width, fmri.vg.height, fmri.vg.depth);
    let nvox = w.wrapping_mul(h).wrapping_mul(d);
    let mut scm = mri_alloc_sequence(g, out, err, w, h, d, MRI_FLOAT, nvox)?;
    let _ = writeln!(out, "fMRIspatialCorMatrix: filling SCM");
    fmri_from_matrix(g, out, err, &mtm, &mut scm, None)?;
    Ok(Some(scm))
}

/// `fMRIcumSum(inmri, mask, outmri)`: frame `f` the sum of frames 0 to `f` in `double` (each value
/// plus the running sum), over the voxels whose mask value is 0.5 or more (the others left as
/// they are), slices outermost. A new output is float with the input's frames and header; `ERROR:
/// fMRIcumSum(): output dimension mismatch` and `Null` for a given one of another extent or frame
/// count.
pub fn fmri_cum_sum(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    inmri: &Mri,
    mask: Option<&Mri>,
    outmri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match outmri {
        None => fresh.insert(alloc_like(g, out, err, inmri, MRI_FLOAT, inmri.nframes)?),
        Some(o) => {
            if !same_space(o, inmri) || o.nframes != inmri.nframes {
                let _ = writeln!(out, "ERROR: fMRIcumSum(): output dimension mismatch");
                return Ok(Ret::Null);
            }
            o
        }
    };
    for s in 0..d.vg.depth {
        for r in 0..d.vg.height {
            for c in 0..d.vg.width {
                if let Some(mk) = mask
                    && gv(g, err, mk, c, r, s, 0)? < 0.5
                {
                    continue;
                }
                let mut val = 0.0f64;
                for f in 0..d.nframes {
                    val = add64(f64::from(gv(g, err, inmri, c, r, s, f)?), val);
                    sv(g, err, d, c, r, s, f, val as f32)?;
                }
            }
        }
    }
    Ok(result(fresh))
}

/// `MRIframeNorm(src, mask, fnorm)`: each voxel's series minus its frame mean, divided by
/// `sqrtf(var (nframes - 1)) + DBL_MIN` (`var` the de-meaned covariance at lag 0 with one degree
/// of freedom removed, [`fmri_covariance`]; the product and root in `float`), over the voxels
/// whose mask value is 0.5 or more. A new output is float with the input's frames and header (a
/// given one is written by `MRIsetVoxVal`); `MRIfree: null pointer` as an error at the end when
/// the covariance could not be computed and every voxel was masked out. FS-564: a covariance that could not be computed (one
/// frame) ends the program at the first voxel not masked out.
pub fn mri_frame_norm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    mask: Option<&Mri>,
    fnorm: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match fnorm {
        Some(d) => d,
        None => fresh.insert(alloc_like(g, out, err, src, MRI_FLOAT, src.nframes)?),
    };
    let mn = mri_frame_mean(g, out, err, src, None, &[])?.unwrap_new();
    let mut var = fmri_covariance(g, out, err, src, 0, -1.0, mask, None)?.into_new();
    let n1 = src.nframes.wrapping_sub(1) as f32;
    for c in 0..src.vg.width {
        for r in 0..src.vg.height {
            for s in 0..src.vg.depth {
                if let Some(mk) = mask
                    && gv(g, err, mk, c, r, s, 0)? < 0.5
                {
                    continue;
                }
                let mnv = f64::from(gv(g, err, &mn, c, r, s, 0)?);
                let Some(var) = var.as_ref() else {
                    missing_covariance(g, err, "MRIframeNorm")?;
                    return Ok(Ret::Null);
                };
                let sss = f64::from(sqrt(mul(gv(g, err, var, c, r, s, 0)?, n1)));
                let den = add64(f64::MIN_POSITIVE, sss);
                for f in 0..src.nframes {
                    let v = f64::from(gv(g, err, src, c, r, s, f)?);
                    let v = div64(sub64(v, mnv), den);
                    sv(g, err, d, c, r, s, f, v as f32)?;
                }
            }
        }
    }
    // MRIfree(&mn), MRIfree(&var): a covariance never computed (every voxel masked out) gives
    // MRIfree's error message
    mri_free(g, err, &mut Some(mn));
    mri_free(g, err, &mut var);
    Ok(result(fresh))
}
