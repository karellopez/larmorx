// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Constant and random volumes, the frames of a voxel normalised or sorted, slices reversed
//! (`utils/mri.cpp`, 3.1-37's piece c): `MRIrandn`, `MRIconst`, `MRInormalizeSequence`,
//! `MRInormalizeFrames`, `MRInormalizeFramesMean`, `MRInormalizeFramesFirst`, `MRIsort` (with
//! `CompareDoubles` and the C library's `qsort`), `MRIreverseSliceOrder` and
//! `MRIcloneDifferentType`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §13):
//! - **FS-472**: a value stored into a byte, short or unsigned short voxel through an `int`
//!   (`MRIconst`, `MRInormalizeSequence`) keeps the `int`'s low bits: past the type's range it
//!   wraps with `reproduce`, clamped to the range by default (FS-12's rule for the `int` itself).
//! - **FS-473** (memory): the reversal of the slices copies a b-value and b-vector per frame
//!   from tables of fewer rows than frames: past their rows FreeSurfer reads through pointers it
//!   does not own (a crash, or memory contents); both modes leave those entries 0, noted.

use std::io::Write;

use larmorx_core::defects::{FS_472, FS_473};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NONE, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::pdf::pdf_gaussian;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, mri_alloc_sequence,
    mri_copy_header,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::mri_clone;
use crate::labels::int_of;
use crate::rows::{self, Num, need_voxels};
use crate::sample::sample_volume_frame;

/// An `int` stored into a voxel of type `ty` (C's conversion to `unsigned char`, `short` or
/// `unsigned short` keeps the low bits; `int` and `long` keep the value): FS-472 past the type's
/// range, clamped by default.
fn narrow(g: &mut Globals, what: &str, i: i32, ty: i32) -> Num {
    let (lo, hi) = match ty {
        MRI_UCHAR => (0, 255),
        MRI_SHORT => (-32768, 32767),
        MRI_USHRT => (0, 65535),
        _ => return Num::I(i64::from(i)),
    };
    if i < lo || i > hi {
        g.notices.add(
            &FS_472,
            format!("{what} stores {i} into a voxel of {lo} to {hi}"),
        );
        if !g.notices.reproduce() {
            return Num::I(i64::from(i.clamp(lo, hi)));
        }
    }
    Num::I(i64::from(i))
}

/// The dimensions check of `MRIrandn` and `MRIconst`: `ERROR: <name>: dimension mismatch` on
/// standard output when they differ.
fn same_dims(out: &mut dyn Write, name: &str, m: &Mri, dims: [i32; 4]) -> bool {
    if [m.vg.width, m.vg.height, m.vg.depth, m.nframes] != dims {
        let _ = writeln!(out, "ERROR: {name}: dimension mismatch");
        return false;
    }
    true
}

/// `MRIrandn(ncols, nrows, nslices, nframes, avg, stddev, mri)`: every voxel (frames outermost,
/// then slices, rows, columns) `stddev * PDFgaussian() + avg` in `double`, stored as a `float`.
/// A new `float` volume when `mri` is `None`; a given one of other dimensions (`ERROR: MRIrandn:
/// dimension mismatch`) or not `float` (`ERROR: MRIrandn: structure passed is not MRI_FLOAT`):
/// `Null` after that line on standard output.
#[allow(clippy::too_many_arguments)]
pub fn mri_randn(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dims: [i32; 4],
    avg: f32,
    stddev: f32,
    mri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let [ncols, nrows, nslices, nframes] = dims;
    let mut fresh = None;
    let m: &mut Mri = match mri {
        None => fresh.insert(mri_alloc_sequence(
            g, out, err, ncols, nrows, nslices, MRI_FLOAT, nframes,
        )?),
        Some(m) => {
            if !same_dims(out, "MRIrandn", m, dims) {
                return Ok(Ret::Null);
            }
            if m.mtype != MRI_FLOAT {
                let _ = writeln!(out, "ERROR: MRIrandn: structure passed is not MRI_FLOAT");
                return Ok(Ret::Null);
            }
            m
        }
    };
    if ncols > 0 && nrows > 0 && nslices > 0 && nframes > 0 {
        need_voxels(g, err, "MRIrandn", m)?;
    }
    for f in 0..nframes {
        for s in 0..nslices {
            for r in 0..nrows {
                for c in 0..ncols {
                    // the official build: PDFgaussian() * stddev, then + avg
                    let p = pdf_gaussian(g);
                    let v = add64(mul64(p, f64::from(stddev)), f64::from(avg));
                    let sl = i64::from(s) + i64::from(f) * i64::from(m.vg.depth);
                    rows::write(
                        g,
                        "MRIrandn",
                        m,
                        sl,
                        i64::from(r),
                        i64::from(c),
                        MRI_FLOAT,
                        Num::F(v as f32),
                    );
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIconst(ncols, nrows, nslices, nframes, val, mri)`: every voxel `val` (`(type)nint(val)` for
/// the integer types, FS-472 for a byte, short or unsigned short; `val` for `float`; nothing for
/// other types). A new `float` volume when `mri` is `None`; a given one of other dimensions:
/// `ERROR: MRIconst: dimension mismatch` on standard output and `Null`.
pub fn mri_const(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dims: [i32; 4],
    val: f32,
    mri: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIconst";
    let [ncols, nrows, nslices, nframes] = dims;
    let mut fresh = None;
    let m: &mut Mri = match mri {
        None => fresh.insert(mri_alloc_sequence(
            g, out, err, ncols, nrows, nslices, MRI_FLOAT, nframes,
        )?),
        Some(m) => {
            if !same_dims(out, "MRIconst", m, dims) {
                return Ok(Ret::Null);
            }
            m
        }
    };
    let ty = m.mtype;
    let direct = matches!(
        ty,
        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG | MRI_FLOAT
    );
    if direct && nrows > 0 && nslices > 0 && nframes > 0 {
        need_voxels(g, err, WHAT, m)?;
    }
    let mut n: i64 = 0;
    for _f in 0..nframes {
        for _s in 0..nslices {
            for r in 0..nrows {
                if !direct {
                    continue;
                }
                for c in 0..ncols {
                    let v = if ty == MRI_FLOAT {
                        Num::F(val)
                    } else {
                        let i = nint(g, f64::from(val));
                        narrow(g, WHAT, i, ty)
                    };
                    rows::write(g, WHAT, m, n, i64::from(r), i64::from(c), ty, v);
                }
            }
            n += 1;
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRInormalizeSequence(mri, target)`: at each voxel the frames' norm (`MRIsampleVolumeFrame`'s
/// values squared and summed in `double`, its square root divided by `target`; 1 when below
/// `FLT_EPSILON`) divides every frame: short, unsigned short and byte voxels through an `int`
/// (FS-12, FS-472), `float` ones in `double`. Other types: `MRInormalizeSequence: unsupported
/// input type <t>` as an error and `ERROR_UNSUPPORTED` at the first voxel. `NO_ERROR`.
pub fn mri_normalize_sequence(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    target: f32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRInormalizeSequence";
    let (w, h, d, nf, ty) = (
        mri.vg.width,
        mri.vg.height,
        mri.vg.depth,
        mri.nframes,
        mri.mtype,
    );
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let mut norm = 0.0f64;
                for frame in 0..nf {
                    let mut val = 0.0f64;
                    let at = (f64::from(x), f64::from(y), f64::from(z));
                    sample_volume_frame(g, err, mri, at.0, at.1, at.2, frame, &mut val);
                    norm = add64(norm, mul64(val, val));
                }
                norm = div64(sqrt64(norm), f64::from(target));
                if norm.abs() < f64::from(f32::EPSILON) {
                    norm = 1.0;
                }
                for frame in 0..nf {
                    if !matches!(ty, MRI_SHORT | MRI_USHRT | MRI_FLOAT | MRI_UCHAR) {
                        log::error_printf(
                            g,
                            err,
                            ERROR_UNSUPPORTED,
                            &format!("MRInormalizeSequence: unsupported input type {ty}"),
                        );
                        return Ok(ERROR_UNSUPPORTED);
                    }
                    let sl = i64::from(z) + i64::from(frame) * i64::from(d);
                    let (yy, xx) = (i64::from(y), i64::from(x));
                    let v = rows::read(g, WHAT, mri, sl, yy, xx, ty);
                    let q = div64(v.f64(), norm);
                    let nv = if ty == MRI_FLOAT {
                        Num::F(q as f32)
                    } else {
                        let i = int_of(g, WHAT, q);
                        narrow(g, WHAT, i, ty)
                    };
                    rows::write(g, WHAT, mri, sl, yy, xx, ty, nv);
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRInormalizeFrames(mri)`: at each voxel (frame 0..n through `MRIgetVoxVal`, in `double`) the
/// mean and `var = sum2 / n - mean * mean`; every frame `(val - mean) / sqrt(var)` written back
/// (`MRIsetVoxVal`). `ERROR_NONE`.
pub fn mri_normalize_frames(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    let n = f64::from(mri.nframes);
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let (mut mean, mut var) = (0.0f64, 0.0f64);
                for f in 0..mri.nframes {
                    let val = widen(mri_get_vox_val(g, err, mri, c, r, s, f, 0)?);
                    mean = add64(mean, val);
                    var = add64(var, mul64(val, val));
                }
                mean = div64(mean, n);
                var = sub64(div64(var, n), mul64(mean, mean));
                let std = sqrt64(var);
                for f in 0..mri.nframes {
                    let val = widen(mri_get_vox_val(g, err, mri, c, r, s, f, 0)?);
                    let v = div64(sub64(val, mean), std);
                    mri_set_vox_val(g, err, mri, c, r, s, f, v as f32, 0)?;
                }
            }
        }
    }
    Ok(ERROR_NONE)
}

/// `MRInormalizeFramesMean(mri)`: every frame divided by the voxel's mean over its frames (in
/// `double`; 1 when its magnitude is below 0.000001). `ERROR_NONE`.
pub fn mri_normalize_frames_mean(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let mut mean = 0.0f64;
                for f in 0..mri.nframes {
                    let val = widen(mri_get_vox_val(g, err, mri, c, r, s, f, 0)?);
                    mean = add64(mean, val);
                }
                mean = div64(mean, f64::from(mri.nframes));
                if mean.abs() < 0.000001 {
                    mean = 1.0;
                }
                for f in 0..mri.nframes {
                    let val = widen(mri_get_vox_val(g, err, mri, c, r, s, f, 0)?);
                    mri_set_vox_val(g, err, mri, c, r, s, f, div64(val, mean) as f32, 0)?;
                }
            }
        }
    }
    Ok(ERROR_NONE)
}

/// `MRInormalizeFramesFirst(mri)`: every frame divided by the voxel's frame 0 (in `double`, read
/// once before the frames are rewritten). `ERROR_NONE`.
pub fn mri_normalize_frames_first(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let val1 = widen(mri_get_vox_val(g, err, mri, c, r, s, 0, 0)?);
                for f in 0..mri.nframes {
                    let val = widen(mri_get_vox_val(g, err, mri, c, r, s, f, 0)?);
                    mri_set_vox_val(g, err, mri, c, r, s, f, div64(val, val1) as f32, 0)?;
                }
            }
        }
    }
    Ok(ERROR_NONE)
}

/// `CompareDoubles(a, b)`: -1, +1 or 0 (NaN equal to everything).
pub fn compare_doubles(a: &f64, b: &f64) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

/// `MRIsort(in, mask, sorted)`: at each voxel (slices outermost, then rows, columns) whose mask
/// (frame 0) is not below 0.5, its frames (`double`) sorted by the C library's `qsort` with
/// `CompareDoubles` and written in that order (`MRIsetVoxVal`). A new volume of the input's type,
/// frames and header when `sorted` is `None`; a given one of other dimensions: `ERROR: MRIsort:
/// dimension mismatch` on standard output and `Null`.
pub fn mri_sort(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    input: &Mri,
    mask: Option<&Mri>,
    sorted: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (nc, nr, ns, nf) = (
        input.vg.width,
        input.vg.height,
        input.vg.depth,
        input.nframes,
    );
    let mut fresh = None;
    let d: &mut Mri = match sorted {
        None => {
            let mut m = mri_alloc_sequence(g, out, err, nc, nr, ns, input.mtype, nf)?;
            mri_copy_header(g, out, err, input, Some(&mut m))?;
            fresh.insert(m)
        }
        Some(m) => m,
    };
    if [d.vg.width, d.vg.height, d.vg.depth, d.nframes] != [nc, nr, ns, nf] {
        let _ = writeln!(out, "ERROR: MRIsort: dimension mismatch");
        return Ok(Ret::Null);
    }
    let mut vf = vec![0.0f64; usize::try_from(nf).unwrap_or(0)];
    for s in 0..ns {
        for r in 0..nr {
            for c in 0..nc {
                if let Some(mk) = mask {
                    if mri_get_vox_val(g, err, mk, c, r, s, 0, 0)? < 0.5 {
                        continue;
                    }
                }
                for (f, v) in vf.iter_mut().enumerate() {
                    *v = widen(mri_get_vox_val(g, err, input, c, r, s, f as i32, 0)?);
                }
                qsort(&mut vf, compare_doubles);
                for (f, &v) in vf.iter().enumerate() {
                    mri_set_vox_val(g, err, d, c, r, s, f as i32, v as f32, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIsort(mri, mask, mri)`: [`mri_sort`] into the input itself (each voxel's frames read, sorted
/// and written back).
pub fn mri_sort_in_place(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    mask: Option<&Mri>,
) -> Result<(), Fatal> {
    let (nc, nr, ns, nf) = (mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes);
    let mut vf = vec![0.0f64; usize::try_from(nf).unwrap_or(0)];
    for s in 0..ns {
        for r in 0..nr {
            for c in 0..nc {
                if let Some(mk) = mask {
                    if mri_get_vox_val(g, err, mk, c, r, s, 0, 0)? < 0.5 {
                        continue;
                    }
                }
                for (f, v) in vf.iter_mut().enumerate() {
                    *v = widen(mri_get_vox_val(g, err, mri, c, r, s, f as i32, 0)?);
                }
                qsort(&mut vf, compare_doubles);
                for (f, &v) in vf.iter().enumerate() {
                    mri_set_vox_val(g, err, mri, c, r, s, f as i32, v as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `(double)` of a `float` as x86-64's `cvtss2sd` makes it: a signalling NaN quieted (its payload
/// kept), whatever the compiler folds.
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// `MRIreverseSliceOrder(invol, outvol)`: a clone of `invol` (`MRIclone`) with its slices in the
/// reverse order (the geometry unchanged; every frame through `MRIgetVoxVal`/`MRIsetVoxVal`), new
/// b-value (frames x 1) and b-vector (frames x 3) tables when `invol` has them, filled from
/// `invol`'s (FS-473 past their rows), and `invol`'s b-vector space. (FreeSurfer refuses
/// `outvol == invol`: `ERROR: MRIreverseSliceOrder: cannot be done in-place`; two distinct
/// volumes here.)
pub fn mri_reverse_slice_order(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    outvol: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let o: &mut Mri = match outvol {
        None => fresh.insert(mri_clone(g, out, err, invol, None)?.unwrap_new()),
        Some(o) => {
            mri_clone(g, out, err, invol, Some(&mut *o))?;
            o
        }
    };
    let nf = invol.nframes;
    if invol.bvals.is_some() {
        o.bvals = Some(Matrix::real(nf, 1));
    }
    if invol.bvecs.is_some() {
        o.bvecs = Some(Matrix::real(nf, 3));
    }
    let mut s2 = invol.vg.depth;
    for s1 in 0..invol.vg.depth {
        s2 -= 1;
        for c in 0..invol.vg.width {
            for r in 0..invol.vg.height {
                for f in 0..nf {
                    // `double val` between a float read and a float write: the official build
                    // drops the round trip (a signalling NaN stays signalling)
                    let val = mri_get_vox_val(g, err, invol, c, r, s1, f, 0)?;
                    mri_set_vox_val(g, err, o, c, r, s2, f, val, 0)?;
                }
            }
        }
    }
    for f in 0..nf {
        if let (Some(src), Some(dst)) = (invol.bvals.as_ref(), o.bvals.as_mut()) {
            copy_entry(g, src, dst, f + 1, 1);
        }
        if let (Some(src), Some(dst)) = (invol.bvecs.as_ref(), o.bvecs.as_mut()) {
            for k in 1..=3 {
                copy_entry(g, src, dst, f + 1, k);
            }
        }
    }
    o.bvec_space = invol.bvec_space;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// One b-value or b-vector entry copied; FS-473 where the source table has no such entry.
fn copy_entry(g: &mut Globals, src: &Matrix, dst: &mut Matrix, r: i32, c: i32) {
    if src.relt_index(r, c).is_none() {
        g.notices.add(
            &FS_473,
            format!("MRIreverseSliceOrder reads entry ({r}, {c}) past a b-value or b-vector table"),
        );
        return;
    }
    dst.set_relt(r, c, src.relt(r, c));
}

/// `MRIcloneDifferentType(mri_src, type)`: `MRIclone` for the source's own type, else a new
/// volume of `type` (the source's extent and frames) with the source's header.
pub fn mri_clone_different_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    mtype: i32,
) -> Result<Mri, Fatal> {
    if mtype == src.mtype {
        return Ok(mri_clone(g, out, err, src, None)?.unwrap_new());
    }
    let mut d = mri_alloc_sequence(
        g,
        out,
        err,
        src.vg.width,
        src.vg.height,
        src.vg.depth,
        mtype,
        src.nframes,
    )?;
    mri_copy_header(g, out, err, src, Some(&mut d))?;
    Ok(d)
}
