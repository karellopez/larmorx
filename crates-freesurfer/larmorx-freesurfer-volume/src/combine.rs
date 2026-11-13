// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Volumes combined and compared (`utils/mri.cpp`, 3.1-37's piece c; `utils/mri2.cpp`'s
//! `MRIdimMismatch`, piece d): `MRIrms`, `MRIcomputeVolumeFractions`,
//! `MRIcombineDistanceTransforms`, `MRIdiff2Vect`, `MRIapplyDiffVect`, `MRIdimMismatch` and
//! `MRIestimateTIV`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §13):
//! - **FS-475** (memory): the volume fractions count a voxel of label 0 into frame -1 of the
//!   fractions (labels are frame + 1): FreeSurfer reads and writes before the volume's frames;
//!   both modes leave label 0 out, noted.
//! - **FS-476**: the fractions' second pass (target voxels no source voxel reached) takes labels
//!   below the frame count only: the last label is never set there with `reproduce`; by default
//!   it is, noted.
//! - **FS-477**: the application of a difference list accepts a column, row or slice equal to the
//!   volume's extent and writes past it with `reproduce` (FreeSurfer's memory: FS-53); by default
//!   such an entry is out of bounds (the message and `NULL`), noted.
//! - **FS-478**: applied in place, the difference list's volume loses its colour table
//!   (`CTABdeepCopy: ct was NULL`) with `reproduce`; by default it keeps it, noted.
//! - **FS-479** (crash): the volume estimate from a transform file with no linear transform reads
//!   the first of none: an error in both modes.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_475, FS_476, FS_477, FS_478, FS_479};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul64, sqrt64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{MRI_INT, Mri, mri_copy_pulse_parameters};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::mri_clone;
use crate::frames::mri_clone_different_type;
use crate::labels::int_of;
use crate::ltaread::{LtaIo, lta_read_ex};

/// `(double)` of a `float` as `cvtss2sd` makes it (a signalling NaN quieted).
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// `MRIdimMismatch(v1, v2, frameflag)`: 1, 2 or 3 for the first of width, height, depth that
/// differs, 4 for the frames (with `frameflag`), else 0.
pub fn mri_dim_mismatch(v1: &Mri, v2: &Mri, frameflag: bool) -> i32 {
    if v1.vg.width != v2.vg.width {
        1
    } else if v1.vg.height != v2.vg.height {
        2
    } else if v1.vg.depth != v2.vg.depth {
        3
    } else if frameflag && v1.nframes != v2.nframes {
        4
    } else {
        0
    }
}

/// `MRIrms(in, out)`: frame 0 of `out` (`MRIsetVoxVal`, read back between frames) becomes the
/// root mean square of `in`'s frames (a volume without frames is read as one): frame by frame
/// (frames outermost, then z, y, x) `vin * vin + vout` in `double` (the last frame `vout + vin *
/// vin`, the official build's operand order), the last divided by the frame count and its
/// square root taken.
pub fn mri_rms(
    g: &mut Globals,
    err: &mut dyn Write,
    input: &Mri,
    out: &mut Mri,
) -> Result<(), Fatal> {
    let nframes = if input.nframes == 0 { 1 } else { input.nframes };
    for f in 0..nframes {
        for z in 0..input.vg.depth {
            for y in 0..input.vg.height {
                for x in 0..input.vg.width {
                    let vin = widen(mri_get_vox_val(g, err, input, x, y, z, f, 0)?);
                    let vout = if f != 0 {
                        widen(mri_get_vox_val(g, err, out, x, y, z, 0, 0)?)
                    } else {
                        0.0
                    };
                    let sq = mul64(vin, vin);
                    let mut v = if f == nframes - 1 && f != 0 {
                        add64(vout, sq)
                    } else {
                        add64(sq, vout)
                    };
                    if f == nframes - 1 {
                        v = sqrt64(div64(v, f64::from(nframes)));
                    }
                    mri_set_vox_val(g, err, out, x, y, z, 0, v as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `MRIcomputeVolumeFractions(mri_src, m_vox2vox, mri_seg, mri_fractions)`: each segmentation
/// voxel mapped by `m_vox2vox` (`float`, rounded with `nint`) into the source's grid adds 1 to the
/// target's count (an `int` volume like the source) and 1 to frame `label - 1` of the fractions
/// for a label from 1 to the frame count (FS-475 for label 0); then each target voxel with a count
/// has its fractions divided by it (`float`), and one without takes the label at its point mapped
/// back (the inverse, `nint`) and sets that label's frame to 1 (labels below the frame count with
/// `reproduce`: FS-476). A matrix with no inverse: `MatrixPrint` of it on standard output and the
/// error `MRIcomputePartialVolumeFractions: non-invertible vox2vox matrix` (`ERROR_BADPARM`).
#[allow(clippy::too_many_arguments)]
pub fn mri_compute_volume_fractions(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    m_vox2vox: &Matrix,
    seg: &Mri,
    fractions: &mut Mri,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIcomputeVolumeFractions";
    let m_inv = match matrix_inverse(g, out, err, Some(m_vox2vox), None)? {
        Ret::New(m) => m,
        _ => {
            let mut buf = Vec::new();
            matrix_print(
                g,
                Some(&mut buf as &mut dyn Write),
                &mut std::io::sink(),
                err,
                Some(m_vox2vox),
                5,
            );
            let _ = out.write_all(&buf);
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRIcomputePartialVolumeFractions: non-invertible vox2vox matrix",
            )?;
            return Ok(ERROR_BADPARM);
        }
    };
    let mut counts = mri_clone_different_type(g, out, err, src, MRI_INT)?;
    let mut v1 = Matrix::real(4, 1);
    let mut v2 = Matrix::real(4, 1);
    v1.set_relt(4, 1, 1.0);
    v2.set_relt(4, 1, 1.0);
    let nf = fractions.nframes;
    let mult = |g: &mut Globals,
                out: &mut dyn Write,
                err: &mut dyn Write,
                m: &Matrix,
                v1: &Matrix,
                v2: &mut Matrix|
     -> Result<(), Fatal> {
        matrix_multiply(g, out, err, Some(m), Some(v1), Some(v2))?;
        Ok(())
    };
    let round = |g: &mut Globals, v2: &Matrix| {
        let r = |g: &mut Globals, k: i32| nint(g, f64::from(v2.relt(k, 1)));
        [r(g, 1), r(g, 2), r(g, 3)]
    };
    for x in 0..seg.vg.width {
        v1.set_relt(1, 1, x as f32);
        for y in 0..seg.vg.height {
            v1.set_relt(2, 1, y as f32);
            for z in 0..seg.vg.depth {
                v1.set_relt(3, 1, z as f32);
                mult(g, out, err, m_vox2vox, &v1, &mut v2)?;
                let [xs, ys, zs] = round(g, &v2);
                if xs >= 0
                    && ys >= 0
                    && zs >= 0
                    && xs < src.vg.width
                    && ys < src.vg.height
                    && zs < src.vg.depth
                {
                    let val = mri_get_vox_val(g, err, &counts, xs, ys, zs, 0, 0)?;
                    mri_set_vox_val(g, err, &mut counts, xs, ys, zs, 0, add(val, 1.0), 0)?;
                    let lv = mri_get_vox_val(g, err, seg, x, y, z, 0, 0)?;
                    let label = int_of(g, WHAT, f64::from(lv));
                    if label >= 0 && label <= nf {
                        if label == 0 {
                            g.notices.add(
                                &FS_475,
                                format!("{WHAT} counts label 0 into frame -1 of the fractions"),
                            );
                            continue;
                        }
                        let val = mri_get_vox_val(g, err, fractions, xs, ys, zs, label - 1, 0)?;
                        mri_set_vox_val(
                            g,
                            err,
                            fractions,
                            xs,
                            ys,
                            zs,
                            label - 1,
                            add(val, 1.0),
                            0,
                        )?;
                    }
                }
            }
        }
    }
    for x in 0..src.vg.width {
        for y in 0..src.vg.height {
            for z in 0..src.vg.depth {
                let count = mri_get_vox_val(g, err, &counts, x, y, z, 0, 0)?;
                if count >= 1.0 {
                    for label in 0..nf {
                        let val = mri_get_vox_val(g, err, fractions, x, y, z, label, 0)?;
                        mri_set_vox_val(g, err, fractions, x, y, z, label, div(val, count), 0)?;
                    }
                } else {
                    v1.set_relt(1, 1, x as f32);
                    v1.set_relt(2, 1, y as f32);
                    v1.set_relt(3, 1, z as f32);
                    // FreeSurfer multiplies twice
                    mult(g, out, err, &m_inv, &v1, &mut v2)?;
                    mult(g, out, err, &m_inv, &v1, &mut v2)?;
                    let [xs, ys, zs] = round(g, &v2);
                    if xs >= 0
                        && ys >= 0
                        && zs >= 0
                        && xs < seg.vg.width
                        && ys < seg.vg.height
                        && zs < seg.vg.depth
                    {
                        let lv = mri_get_vox_val(g, err, seg, xs, ys, zs, 0, 0)?;
                        let label = int_of(g, WHAT, f64::from(lv));
                        let mut take = label >= 0 && label < nf;
                        if label == nf && label > 0 {
                            g.notices.add(
                                &FS_476,
                                format!(
                                    "{WHAT} leaves the last label {label} out of its second pass"
                                ),
                            );
                            take = !g.notices.reproduce();
                        }
                        if take && label == 0 {
                            g.notices.add(
                                &FS_475,
                                format!("{WHAT} sets frame -1 of the fractions for label 0"),
                            );
                            take = false;
                        }
                        if take {
                            mri_set_vox_val(g, err, fractions, x, y, z, label - 1, 1.0, 0)?;
                        }
                    }
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRIcombineDistanceTransforms(mri_src1, mri_src2, mri_dst)`: each voxel of every frame (of the
/// destination's extent; a clone of the first when none is given) the first's value, replaced by
/// the second's where the second is negative and the first positive, both positive and the
/// second smaller, or both negative and the second larger.
pub fn mri_combine_distance_transforms(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src1: &Mri,
    src2: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => fresh.insert(mri_clone(g, out, err, src1, None)?.unwrap_new()),
    };
    for f in 0..d.nframes {
        for x in 0..d.vg.width {
            for y in 0..d.vg.height {
                for z in 0..d.vg.depth {
                    let mut v1 = mri_get_vox_val(g, err, src1, x, y, z, f, 0)?;
                    let v2 = mri_get_vox_val(g, err, src2, x, y, z, f, 0)?;
                    if (v2 < 0.0 && v1 > 0.0) || (v2 > 0.0 && v1 > v2) || (v2 < 0.0 && v1 < v2) {
                        v1 = v2;
                    }
                    mri_set_vox_val(g, err, d, x, y, z, f, v1, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIdiff2Vect(mri1, mri2, thresh, mask)`: the voxels (columns outermost, then rows, slices)
/// within the mask (frame 0 not below 0.5) whose frame-0 values (`double`) differ by `thresh` or
/// more (`fabs(v1 - v2) < thresh` skipped), each `[c, r, s, v1, v2]`. Dimension mismatches (the
/// frames not compared): `MRIdiff2Vect(): ERROR: dimension mismatch` (`... with mask`) on standard
/// output and `None`.
pub fn mri_diff2vect(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri1: &Mri,
    mri2: &Mri,
    thresh: f64,
    mask: Option<&Mri>,
) -> Result<Option<Vec<[f64; 5]>>, Fatal> {
    if mri_dim_mismatch(mri1, mri2, false) != 0 {
        let _ = writeln!(out, "MRIdiff2Vect(): ERROR: dimension mismatch");
        return Ok(None);
    }
    if let Some(m) = mask {
        if mri_dim_mismatch(mri1, m, false) != 0 {
            let _ = writeln!(out, "MRIdiff2Vect(): ERROR: dimension mismatch with mask");
            return Ok(None);
        }
    }
    let mut v = Vec::new();
    for c in 0..mri1.vg.width {
        for r in 0..mri1.vg.height {
            for s in 0..mri1.vg.depth {
                if let Some(m) = mask {
                    if mri_get_vox_val(g, err, m, c, r, s, 0, 0)? < 0.5 {
                        continue;
                    }
                }
                let a = widen(mri_get_vox_val(g, err, mri1, c, r, s, 0, 0)?);
                let b = widen(mri_get_vox_val(g, err, mri2, c, r, s, 0, 0)?);
                if (a - b).abs() < thresh {
                    continue;
                }
                v.push([f64::from(c), f64::from(r), f64::from(s), a, b]);
            }
        }
    }
    Ok(Some(v))
}

/// Where [`mri_apply_diff_vect`] writes: a new copy of `mri1`, a given volume, or `mri1` itself.
pub enum DiffDst<'a> {
    New,
    Given(&'a mut Mri),
    InPlace,
}

/// `MRIapplyDiffVect(mri1, mask, diffvec, napply, mri2)`: `mri1` copied into the destination
/// (`MRIcopy` and its pulse parameters; in place: nothing copied), `mri1`'s colour table deep-copied
/// into it (FS-478 in place), then each entry's value `[4]` written at its column, row, slice
/// (`(int)` of `[0..3]`; FS-12) of frame 0 where the mask (frame 0) is not below 0.5, counting the
/// entries applied. An index below 0 or past the extent (FS-477 at the extent): `MRIapplyDiffVect():
/// voxel index out of bounds <c> <r> <s>` on standard output and `Null` (entries before it applied).
/// A destination or mask of other dimensions: `MRIapplyDiffVect(): ERROR: dimension mismatch`
/// (`... with mask`) and `Null`.
#[allow(clippy::too_many_arguments)]
pub fn mri_apply_diff_vect(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri1: &mut Mri,
    mask: Option<&Mri>,
    diffvec: &[[f64; 5]],
    napply: &mut i32,
    dst: DiffDst<'_>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIapplyDiffVect";
    *napply = 0;
    let mut fresh = None;
    let in_place = matches!(dst, DiffDst::InPlace);
    let mut given_ref = None;
    match dst {
        DiffDst::New => match mri_copy(g, out, err, mri1, None)? {
            Ret::New(mut m) => {
                mri_copy_pulse_parameters(mri1, &mut m);
                fresh = Some(m);
            }
            _ => return Ok(Ret::Null),
        },
        DiffDst::Given(d) => {
            if let Ret::Null = mri_copy(g, out, err, mri1, Some(&mut *d))? {
                return Ok(Ret::Null);
            }
            mri_copy_pulse_parameters(mri1, d);
            given_ref = Some(d);
        }
        DiffDst::InPlace => {}
    }
    // the colour table, then the dimension check
    if in_place {
        if mri1.ct.is_some() {
            g.notices.add(
                &FS_478,
                format!("{WHAT} in place frees the volume's colour table before copying it"),
            );
            if g.notices.reproduce() {
                mri1.ct = None;
                mri1.ct = ctab_deep_copy(g, err, None);
            }
        }
    } else {
        let ct = ctab_deep_copy_if(g, err, mri1);
        let d2: &mut Mri = match (fresh.as_mut(), given_ref.as_deref_mut()) {
            (Some(m), _) => m,
            (None, Some(m)) => m,
            _ => unreachable!("a destination"),
        };
        if let Some(ct) = ct {
            d2.ct = Some(ct);
        }
    }
    {
        let d2: &Mri = match (&fresh, &given_ref) {
            (Some(m), _) => m,
            (None, Some(m)) => m,
            _ => &*mri1,
        };
        if mri_dim_mismatch(mri1, d2, false) != 0 {
            let _ = writeln!(out, "MRIapplyDiffVect(): ERROR: dimension mismatch");
            return Ok(Ret::Null);
        }
    }
    if let Some(m) = mask {
        if mri_dim_mismatch(mri1, m, false) != 0 {
            let _ = writeln!(
                out,
                "MRIapplyDiffVect(): ERROR: dimension mismatch with mask"
            );
            return Ok(Ret::Null);
        }
    }
    let (w, h, dd) = (mri1.vg.width, mri1.vg.height, mri1.vg.depth);
    for e in diffvec {
        let c = int_of(g, WHAT, e[0]);
        let r = int_of(g, WHAT, e[1]);
        let s = int_of(g, WHAT, e[2]);
        let mut out_of = c < 0 || r < 0 || s < 0 || c > w || r > h || s > dd;
        if !out_of && (c == w || r == h || s == dd) {
            g.notices.add(
                &FS_477,
                format!("{WHAT} takes the index ({c}, {r}, {s}) at the volume's extent"),
            );
            out_of = !g.notices.reproduce();
        }
        if out_of {
            let _ = writeln!(
                out,
                "MRIapplyDiffVect(): voxel index out of bounds {c} {r} {s}"
            );
            return Ok(Ret::Null);
        }
        if let Some(m) = mask {
            if mri_get_vox_val(g, err, m, c, r, s, 0, 0)? < 0.5 {
                continue;
            }
        }
        let d2: &mut Mri = match (fresh.as_mut(), given_ref.as_deref_mut()) {
            (Some(m), _) => m,
            (None, Some(m)) => m,
            _ => &mut *mri1,
        };
        mri_set_vox_val(g, err, d2, c, r, s, 0, e[4] as f32, 0)?;
        *napply += 1;
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `CTABdeepCopy(mri1->ct)` when the volume has a colour table.
fn ctab_deep_copy_if(
    g: &mut Globals,
    err: &mut dyn Write,
    mri1: &Mri,
) -> Option<larmorx_freesurfer_mri::ctab::ColorTable> {
    mri1.ct.as_ref()?;
    ctab_deep_copy(g, err, mri1.ct.as_ref())
}

/// `MRIestimateTIV(theLtaFile, theScaleFactor, theAtlasDet)`: the transform read
/// (`LTAreadEx`; none: `<Progname>: could not open atlas transform file <name>` as a fatal error,
/// `ERROR_NOFILE`), the determinant of its first linear transform (`MatrixDeterminant`, `float`),
/// and `(scale * 1000) / det` in `double`. Returns `(icv, det)`.
#[allow(clippy::too_many_arguments)]
pub fn mri_estimate_tiv(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    lta_file: &str,
    scale: f64,
    io: &mut dyn LtaIo,
) -> Result<(f64, f64), Fatal> {
    let Some(lta) = lta_read_ex(g, out, err, dir, lta_file, io)? else {
        let msg = format!(
            "{}: could not open atlas transform file {lta_file}",
            g.progname
        );
        log::error_exit(g, err, ERROR_NOFILE, &msg)?;
        return Ok((0.0, 0.0));
    };
    let Some(m) = lta.xforms.first().and_then(|lt| lt.m_l.as_ref()) else {
        g.notices.add(
            &FS_479,
            format!("MRIestimateTIV reads the first transform of {lta_file}, which has none (FreeSurfer ends here)"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIestimateTIV(): {lta_file} holds no linear transform (FreeSurfer crashes here)\n"
            ),
        )?;
        return Ok((0.0, 0.0));
    };
    let det = f64::from(matrix_determinant(g, err, m));
    let icv = div64(mul64(scale, 1000.0), det);
    Ok((icv, det))
}
