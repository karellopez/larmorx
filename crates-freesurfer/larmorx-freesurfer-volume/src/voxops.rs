// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Voxelwise operations, crops and segmentation lists of `utils/mri2.cpp` (3.1-37's piece d):
//! `mri_framepower`, `mri_binarize`, `mri_reshape`, `MRIsqrt`, `MRImax`, `MRImultiplyConst`,
//! `MRIaddConst`, `MRIframeBinarize`, `MRIvote`, `MRIsquare`, `MRIsquareRoot`, `MRIcrop`,
//! `MRIreverseSlices`, `MRIcutEndSlices`, `MRIsegIdList`, `MRIsegIdListNot0` and
//! `MRIhalfCosBias`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §14):
//! - **FS-480**: the vote over a voxel's frames excludes zeros from every run but the last: a
//!   last run of zeros that is the longest wins with `reproduce`; by default it is excluded too,
//!   noted.
//! - **FS-481**: the reshape of a volume of 64-bit integers (`long`) copies 32-bit integers at the
//!   voxels' indices (halves of the first voxels, the rest left 0) with `reproduce`; by default the
//!   whole voxels, noted.

use std::io::Write;

use larmorx_core::defects::{FS_480, FS_481};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt, sqrt64, sub64};
use larmorx_freesurfer_core::utils::unique_int_list;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, Mri, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::{p0_to_cras, xfm_crs2xyz};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::{Pair, mri_clear, mri_clone, mri_clone_by_space};
use crate::frames::mri_sort_in_place;
use crate::labels::int_of;
use crate::orient::mri_set_voxel_to_ras_xform;
use crate::rows::{self, Num};

/// `(double)` of a `float` as `cvtss2sd` makes it (a signalling NaN quieted).
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// `New(m)` or `Given` for a function that filled `fresh` or the caller's volume.
fn ret(fresh: Option<Mri>) -> Ret<Mri> {
    match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    }
}

/// `mri_framepower(vol, framepower)`: each frame's `float` voxels (read and written directly as
/// `float`) raised to its power: unchanged within 0.00001 of 1, `sqrtf` within 0.00001 of 0.5,
/// else `powf` (FreeSurfer's branch for 2 tests 0.5 again: never taken). `framepower` holds a
/// power per frame (else an error: FreeSurfer reads past it). 0.
pub fn mri_framepower(
    g: &mut Globals,
    err: &mut dyn Write,
    vol: &mut Mri,
    framepower: &[f32],
) -> Result<i32, Fatal> {
    const WHAT: &str = "mri_framepower";
    if (framepower.len() as i64) < i64::from(vol.nframes) {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "mri_framepower(): fewer powers than frames (FreeSurfer reads past them)\n",
        )?;
        return Ok(0);
    }
    let libm = Libm::new(g.mode());
    for f in 0..vol.nframes {
        let p = framepower[f as usize];
        if (f64::from(p) - 1.0).abs() < 0.00001 {
            continue;
        }
        let half = (f64::from(p) - 0.5).abs() < 0.00001;
        for r in 0..vol.vg.height {
            for c in 0..vol.vg.width {
                for s in 0..vol.vg.depth {
                    let sl = i64::from(s) + i64::from(f) * i64::from(vol.vg.depth);
                    let (rr, cc) = (i64::from(r), i64::from(c));
                    let val = rows::read(g, WHAT, vol, sl, rr, cc, MRI_FLOAT).f();
                    let v = if half { sqrt(val) } else { libm.powf(val, p) };
                    rows::write(g, WHAT, vol, sl, rr, cc, MRI_FLOAT, Num::F(v));
                }
            }
        }
    }
    note_glibc07(g, &libm, "powf");
    Ok(0)
}

/// `mri_binarize(vol, thresh, tail, invert, volbin, &nover)`: each voxel (rows outermost, then
/// columns, slices, frames; `float`; negated for a `tail` beginning `neg`, its magnitude for
/// `abs`, as is for `pos` or none, the first three letters in any case) above `thresh` gives 1 (0
/// with `invert`), else the other, written with `MRIsetVoxVal`; `nover` counts the 1s. `NOT
/// INVERTING` or `INVERTING` on standard output first. A new `float` volume (the header and pulse
/// parameters) for `Pair::New`. Another tail: `mri_binarize: tail = <tail> unrecoginzed` on
/// standard error and `Null`.
#[allow(clippy::too_many_arguments)]
pub fn mri_binarize(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    thresh: f32,
    tail: Option<&[u8]>,
    invert: bool,
    nover: &mut i32,
) -> Result<Ret<Mri>, Fatal> {
    let tail = tail.unwrap_or(b"positive");
    let is = |w: &[u8]| tail.len() >= 3 && tail[..3].eq_ignore_ascii_case(w);
    let code = if is(b"pos") {
        1
    } else if is(b"neg") {
        2
    } else if is(b"abs") {
        3
    } else {
        let mut line = b"mri_binarize: tail = ".to_vec();
        line.extend_from_slice(tail);
        line.extend_from_slice(b" unrecoginzed\n");
        let _ = err.write_all(&line);
        return Ok(Ret::Null);
    };
    let mut fresh = None;
    let (src, d): (Option<&Mri>, &mut Mri) = match p {
        Pair::New(v) => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                v.vg.width,
                v.vg.height,
                v.vg.depth,
                MRI_FLOAT,
                v.nframes,
            )?;
            mri_copy_header(g, out, err, v, Some(&mut m))?;
            mri_copy_pulse_parameters(v, &mut m);
            (Some(v), fresh.insert(m))
        }
        Pair::Given(v, d) => (Some(v), d),
        Pair::Same(v) => (None, v),
    };
    let (on, off) = if invert { (0, 1) } else { (1, 0) };
    let _ = writeln!(
        out,
        "{}",
        if invert { "INVERTING" } else { "NOT INVERTING" }
    );
    *nover = 0;
    let dims = {
        let v = src.unwrap_or(&*d);
        (v.vg.height, v.vg.width, v.vg.depth, v.nframes)
    };
    for r in 0..dims.0 {
        for c in 0..dims.1 {
            for s in 0..dims.2 {
                for f in 0..dims.3 {
                    let mut val = mri_get_vox_val(g, err, src.unwrap_or(&*d), c, r, s, f, 0)?;
                    match code {
                        2 => val = -val,
                        3 => val = val.abs(),
                        _ => {}
                    }
                    let b = if val > thresh { on } else { off };
                    if b != 0 {
                        *nover += 1;
                    }
                    mri_set_vox_val(g, err, d, c, r, s, f, b as f32, 0)?;
                }
            }
        }
    }
    Ok(ret(fresh))
}

/// `mri_reshape(vol, ncols, nrows, nslices, nframes)`: a new volume of the given shape, the
/// input's type, header and pulse parameters, its voxels the input's in memory order (columns
/// fastest; copied directly for byte, short, unsigned short, `float`, `int` and `long`, nothing
/// for other types). An input without frames is first given one (in place). Another element count
/// (`int` products): `ERROR: mri_reshape: number of elements cannot change` and `  nv1 = <n1>, nv1
/// = <n2>` on standard output and `None`.
pub fn mri_reshape(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &mut Mri,
    shape: [i32; 4],
) -> Result<Option<Mri>, Fatal> {
    const WHAT: &str = "mri_reshape";
    let [nc, nr, ns, nf] = shape;
    if vol.nframes == 0 {
        vol.nframes = 1;
    }
    let nv1 = vol
        .vg
        .width
        .wrapping_mul(vol.vg.height)
        .wrapping_mul(vol.vg.depth)
        .wrapping_mul(vol.nframes);
    let nv2 = nc.wrapping_mul(nr).wrapping_mul(ns).wrapping_mul(nf);
    if nv1 != nv2 {
        let _ = writeln!(out, "ERROR: mri_reshape: number of elements cannot change");
        let _ = writeln!(out, "  nv1 = {nv1}, nv1 = {nv2}");
        return Ok(None);
    }
    let mut o = mri_alloc_sequence(g, out, err, nc, nr, ns, vol.mtype, nf)?;
    mri_copy_header(g, out, err, vol, Some(&mut o))?;
    mri_copy_pulse_parameters(vol, &mut o);
    let mut ty = vol.mtype;
    let direct = rows::elem_size(ty) > 0;
    if ty == MRI_LONG && nv1 > 0 {
        g.notices.add(
            &FS_481,
            "mri_reshape copies a 64-bit integer volume as 32-bit integers at the voxels' indices",
        );
        if g.notices.reproduce() {
            ty = MRI_INT;
        }
    }
    let (mut c2, mut r2, mut s2, mut f2) = (0i32, 0i32, 0i32, 0i32);
    for f in 0..vol.nframes {
        for s in 0..vol.vg.depth {
            for r in 0..vol.vg.height {
                for c in 0..vol.vg.width {
                    if direct {
                        let sl = i64::from(s) + i64::from(f) * i64::from(vol.vg.depth);
                        let v = rows::read(g, WHAT, vol, sl, i64::from(r), i64::from(c), ty);
                        let sl2 = i64::from(s2) + i64::from(f2) * i64::from(o.vg.depth);
                        rows::write(g, WHAT, &mut o, sl2, i64::from(r2), i64::from(c2), ty, v);
                    }
                    c2 += 1;
                    if c2 == nc {
                        c2 = 0;
                        r2 += 1;
                        if r2 == nr {
                            r2 = 0;
                            s2 += 1;
                            if s2 == ns {
                                s2 = 0;
                                f2 += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(Some(o))
}

/// `MRIsqrt(invol, outvol)`: [`mri_square_root`] without a mask.
pub fn mri_sqrt(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    invol: &Mri,
    outvol: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    mri_square_root(g, out, err, invol, None, outvol)
}

/// `MRImax(mri1, mri2, out)`: the larger of the two voxels (`std::max`: the first unless it is
/// below the second; the `float` values themselves), frames outermost, then columns, rows,
/// slices. A new volume of the first's type and header without `out`; an `out` of another shape:
/// `ERROR: MRImax: dimension mismatch` on standard output and `Null`.
pub fn mri_max(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri1: &Mri,
    mri2: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (cols, rows_, slices, frames) =
        (mri1.vg.width, mri1.vg.height, mri1.vg.depth, mri1.nframes);
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, cols, rows_, slices, mri1.mtype, frames)?;
            mri_copy_header(g, out, err, mri1, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    if [d.vg.width, d.vg.height, d.vg.depth, d.nframes] != [cols, rows_, slices, frames] {
        let _ = writeln!(out, "ERROR: MRImax: dimension mismatch");
        return Ok(Ret::Null);
    }
    for f in 0..frames {
        for c in 0..cols {
            for r in 0..rows_ {
                for s in 0..slices {
                    let v1 = mri_get_vox_val(g, err, mri1, c, r, s, f, 0)?;
                    let v2 = mri_get_vox_val(g, err, mri2, c, r, s, f, 0)?;
                    let m = if v1 < v2 { v2 } else { v1 };
                    mri_set_vox_val(g, err, d, c, r, s, f, m, 0)?;
                }
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRImultiplyConst(src, vconst, dst)` (`mul`) and `MRIaddConst(src, vconst, dst)`: each voxel
/// (columns outermost, then rows, slices, frames) `v * vconst` or `v + vconst` in `double`. A new
/// `float` volume with the source's header without `dst`.
#[allow(clippy::too_many_arguments)]
pub fn mri_const_op(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    vconst: f64,
    mul: bool,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                src.vg.width,
                src.vg.height,
                src.vg.depth,
                MRI_FLOAT,
                src.nframes,
            )?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    for c in 0..src.vg.width {
        for r in 0..src.vg.height {
            for s in 0..src.vg.depth {
                for f in 0..src.nframes {
                    let v = widen(mri_get_vox_val(g, err, src, c, r, s, f, 0)?);
                    let w = if mul {
                        mul64(v, vconst)
                    } else {
                        add64(v, vconst)
                    };
                    mri_set_vox_val(g, err, d, c, r, s, f, w as f32, 0)?;
                }
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRIframeBinarize(mri, thresh, mask)`: frame 0 of the mask 1 where every frame's magnitude
/// (`double`) is above `thresh`, else 0; a given mask is pruned (voxels below 0.5 left as they
/// are), else a new cleared `float` one-frame volume.
pub fn mri_frame_binarize(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    thresh: f64,
    mask: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let premask = mask.is_some();
    let m: &mut Mri = match mask {
        Some(m) => m,
        None => {
            let mut m = mri_clone_by_space(g, out, err, mri, MRI_FLOAT, 1)?;
            mri_clear(g, err, &mut m)?;
            fresh.insert(m)
        }
    };
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                if premask && f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?) < 0.5 {
                    continue;
                }
                let mut n = 0;
                for f in 0..mri.nframes {
                    let val = f64::from(mri_get_vox_val(g, err, mri, c, r, s, f, 0)?);
                    if val.abs() > thresh {
                        n += 1;
                    }
                }
                let b = if n == mri.nframes { 1.0 } else { 0.0 };
                mri_set_vox_val(g, err, m, c, r, s, 0, b, 0)?;
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRIvote(in, mask, vote, Exclude0)`: `in`'s frames sorted in place (`MRIsort(in, mask, in)`,
/// between `MRIvote: sorting` and `MRIvote: done sorting` on standard output), then at each voxel
/// in the mask the value of the longest run of equal values (the first of the longest; with
/// `exclude0` zeros excluded, but from the last run with `reproduce`: FS-480) into frame 0 and
/// its run length over the frame count into frame 1. A new two-frame volume of `in`'s type and
/// header without `vote`; a `vote` of another type (`ERROR: MRIvote: type mismatch`) or shape
/// (`... dimension mismatch`): `Null`.
pub fn mri_vote(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    input: &mut Mri,
    mask: Option<&Mri>,
    vote: Option<&mut Mri>,
    exclude0: bool,
) -> Result<Ret<Mri>, Fatal> {
    let _ = writeln!(out, "MRIvote: sorting");
    mri_sort_in_place(g, err, input, mask)?;
    let _ = writeln!(out, "MRIvote: done sorting");
    let (nc, nr, ns, nf) = (
        input.vg.width,
        input.vg.height,
        input.vg.depth,
        input.nframes,
    );
    let mut fresh = None;
    let v: &mut Mri = match vote {
        Some(v) => v,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, nc, nr, ns, input.mtype, 2)?;
            mri_copy_header(g, out, err, input, Some(&mut m))?;
            m.nframes = 2;
            fresh.insert(m)
        }
    };
    if input.mtype != v.mtype {
        let _ = writeln!(out, "ERROR: MRIvote: type mismatch");
        return Ok(Ret::Null);
    }
    if [v.vg.width, v.vg.height, v.vg.depth, v.nframes] != [nc, nr, ns, 2] {
        let _ = writeln!(out, "ERROR: MRIvote: dimension mismatch");
        return Ok(Ret::Null);
    }
    for c in 0..nc {
        for r in 0..nr {
            for s in 0..ns {
                if let Some(mk) = mask {
                    if mri_get_vox_val(g, err, mk, c, r, s, 0, 0)? < 0.5 {
                        continue;
                    }
                }
                let mut vmax = 0.0f64;
                let mut runlenmax = 0;
                let mut v0 = widen(mri_get_vox_val(g, err, input, c, r, s, 0, 0)?);
                let mut f0 = 0;
                let mut f = 1;
                while f < nf {
                    let vv = widen(mri_get_vox_val(g, err, input, c, r, s, f, 0)?);
                    if v0 != vv {
                        let runlen = f - f0;
                        let skip = v0 == 0.0 && exclude0;
                        if runlenmax < runlen && !skip {
                            runlenmax = runlen;
                            vmax = v0;
                        }
                        v0 = vv;
                        f0 = f;
                    }
                    f += 1;
                }
                let runlen = f - f0;
                if runlenmax < runlen {
                    let mut take = true;
                    if v0 == 0.0 && exclude0 {
                        g.notices.add(
                            &FS_480,
                            "MRIvote: the last run, of zeros, is the longest and is not excluded",
                        );
                        take = g.notices.reproduce();
                    }
                    if take {
                        runlenmax = runlen;
                        vmax = v0;
                    }
                }
                mri_set_vox_val(g, err, v, c, r, s, 0, vmax as f32, 0)?;
                let frac = div64(f64::from(runlenmax), f64::from(nf));
                mri_set_vox_val(g, err, v, c, r, s, 1, frac as f32, 0)?;
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRIsquare(in, mask, out)`: each frame `val * val` (`double`), `val` 0 where the mask is not
/// above 0.5 and voxels below 0.5 in the mask skipped; a clone of `in` without `out`.
pub fn mri_square(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    input: &Mri,
    mask: Option<&Mri>,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => fresh.insert(mri_clone(g, out, err, input, None)?.unwrap_new()),
    };
    let mut mval = 1.0f64;
    for c in 0..input.vg.width {
        for r in 0..input.vg.height {
            for s in 0..input.vg.depth {
                if let Some(m) = mask {
                    mval = f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?);
                    if f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?) < 0.5 {
                        continue;
                    }
                }
                for f in 0..input.nframes {
                    let val = if mval > 0.5 {
                        widen(mri_get_vox_val(g, err, input, c, r, s, f, 0)?)
                    } else {
                        0.0
                    };
                    mri_set_vox_val(g, err, d, c, r, s, f, mul64(val, val) as f32, 0)?;
                }
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRIsquareRoot(in, mask, out)`: each frame `sqrt(fabs(val))` (`double`), `val` 0 where the mask
/// is not above 0.5; a new `float` volume with `in`'s header without `out`.
pub fn mri_square_root(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    input: &Mri,
    mask: Option<&Mri>,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                input.vg.width,
                input.vg.height,
                input.vg.depth,
                MRI_FLOAT,
                input.nframes,
            )?;
            mri_copy_header(g, out, err, input, Some(&mut m))?;
            m.mtype = MRI_FLOAT;
            fresh.insert(m)
        }
    };
    let mut mval = 1.0f64;
    for c in 0..input.vg.width {
        for r in 0..input.vg.height {
            for s in 0..input.vg.depth {
                if let Some(m) = mask {
                    mval = f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?);
                }
                for f in 0..input.nframes {
                    let val = if mval > 0.5 {
                        widen(mri_get_vox_val(g, err, input, c, r, s, f, 0)?)
                    } else {
                        0.0
                    };
                    mri_set_vox_val(g, err, d, c, r, s, f, sqrt64(val.abs()) as f32, 0)?;
                }
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRIcrop(mri, c1, r1, s1, c2, r2, s2)`: a new volume of the inclusive box, the input's type,
/// frames and header, its first voxel's RAS (`MRIxfmCRS2XYZ` times the start point, `float`) as
/// its `P0` (`MRIp0ToCRAS`), its voxels copied (`MRIgetVoxVal`/`MRIsetVoxVal`). A start or end
/// point outside the volume: `MRIcrop(): start point <c> <r> <s> out of range` (or `end point`)
/// on standard output and `None`.
#[allow(clippy::too_many_arguments)]
pub fn mri_crop(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    p1: [i32; 3],
    p2: [i32; 3],
) -> Result<Option<Mri>, Fatal> {
    let outside = |p: [i32; 3]| {
        p[0] < 0
            || p[0] >= mri.vg.width
            || p[1] < 0
            || p[1] >= mri.vg.height
            || p[2] < 0
            || p[2] >= mri.vg.depth
    };
    if outside(p1) {
        let _ = writeln!(
            out,
            "MRIcrop(): start point {} {} {} out of range",
            p1[0], p1[1], p1[2]
        );
        return Ok(None);
    }
    if outside(p2) {
        let _ = writeln!(
            out,
            "MRIcrop(): end point {} {} {} out of range",
            p2[0], p2[1], p2[2]
        );
        return Ok(None);
    }
    let n = [p2[0] - p1[0] + 1, p2[1] - p1[1] + 1, p2[2] - p1[2] + 1];
    let mut crop = mri_alloc_sequence(g, out, err, n[0], n[1], n[2], mri.mtype, mri.nframes)?;
    mri_copy_header(g, out, err, mri, Some(&mut crop))?;
    let v2r = xfm_crs2xyz(g, out, err, &mri.vg, 0, false)?;
    let mut crs = Matrix::real(4, 1);
    crs.set_relt(1, 1, p1[0] as f32);
    crs.set_relt(2, 1, p1[1] as f32);
    crs.set_relt(3, 1, p1[2] as f32);
    crs.set_relt(4, 1, 1.0);
    let p0 = matrix_multiply(g, out, err, Some(&v2r), Some(&crs), None)?.unwrap_new();
    let (x, y, z) = (p0.relt(1, 1), p0.relt(2, 1), p0.relt(3, 1));
    p0_to_cras(
        g,
        out,
        err,
        &mut crop.vg,
        f64::from(x),
        f64::from(y),
        f64::from(z),
    )?;
    for c in 0..crop.vg.width {
        for r in 0..crop.vg.height {
            for s in 0..crop.vg.depth {
                for f in 0..crop.nframes {
                    let v = mri_get_vox_val(g, err, mri, c + p1[0], r + p1[1], s + p1[2], f, 0)?;
                    // `double v` between a float read and a float write: dropped by the compiler
                    mri_set_vox_val(g, err, &mut crop, c, r, s, f, v, 0)?;
                }
            }
        }
    }
    Ok(Some(crop))
}

/// `MRIreverseSlices(in, out)`: a copy of `in` (`MRIcopy`) with its slices in the reverse order
/// and its voxel-to-RAS matrix `in`'s times the inverse of the slice flip (`MRIsetVoxelToRasXform`:
/// its warnings, its environment variable). (FreeSurfer refuses `out == in`: `ERROR:
/// MRIreverseSlices(): cannot be done in-place`; two distinct volumes here.)
pub fn mri_reverse_slices(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    input: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => {
            if let Ret::Null = mri_copy(g, out, err, input, Some(&mut *d))? {
                return Ok(Ret::Null);
            }
            d
        }
        None => match mri_copy(g, out, err, input, None)? {
            Ret::New(m) => fresh.insert(m),
            _ => return Ok(Ret::Null),
        },
    };
    let sin = xfm_crs2xyz(g, out, err, &input.vg, 0, false)?;
    let mut m = Matrix::real(4, 4);
    m.set_relt(1, 1, 1.0);
    m.set_relt(2, 2, 1.0);
    m.set_relt(3, 3, -1.0);
    m.set_relt(3, 4, (f64::from(input.vg.depth) - 1.0) as f32);
    m.set_relt(4, 4, 1.0);
    let Ret::New(inv) = matrix_inverse(g, out, err, Some(&m), None)? else {
        let _ = writeln!(out, "ERROR: inverting M");
        return Ok(Ret::Null);
    };
    let sout = matrix_multiply(g, out, err, Some(&sin), Some(&inv), None)?.unwrap_new();
    mri_set_voxel_to_ras_xform(g, out, err, env, d, &sout)?;
    let depth = d.vg.depth;
    for s in 0..depth {
        for c in 0..d.vg.width {
            for r in 0..d.vg.height {
                for f in 0..d.nframes {
                    let v = mri_get_vox_val(g, err, input, c, r, s, f, 0)?;
                    let s2 = int_of(
                        g,
                        "MRIreverseSlices",
                        sub64(f64::from(input.vg.depth) - 1.0, f64::from(s)),
                    );
                    mri_set_vox_val(g, err, d, c, r, s2, f, v, 0)?;
                }
            }
        }
    }
    Ok(ret(fresh))
}

/// `MRIcutEndSlices(mri, ncut)`: a new volume without `ncut` slices at each end (the input's
/// type, frames and header). No slice left: `ERROR: MRIcutEndSlices(): ncut = <n>, input only
/// has <d> ` on standard output and `None`.
pub fn mri_cut_end_slices(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    ncut: i32,
) -> Result<Option<Mri>, Fatal> {
    let nslices = mri.vg.depth.wrapping_sub(ncut.wrapping_mul(2));
    if nslices <= 0 {
        let _ = writeln!(
            out,
            "ERROR: MRIcutEndSlices(): ncut = {ncut}, input only has {} ",
            mri.vg.depth
        );
        return Ok(None);
    }
    let mut o = mri_alloc_sequence(
        g,
        out,
        err,
        mri.vg.width,
        mri.vg.height,
        nslices,
        mri.mtype,
        mri.nframes,
    )?;
    mri_copy_header(g, out, err, mri, Some(&mut o))?;
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            let mut scut = 0;
            for s in ncut..mri.vg.depth - ncut {
                for f in 0..mri.nframes {
                    let v = mri_get_vox_val(g, err, mri, c, r, s, f, 0)?;
                    mri_set_vox_val(g, err, &mut o, c, r, scut, f, v, 0)?;
                }
                scut += 1;
            }
        }
    }
    Ok(Some(o))
}

/// `MRIsegIdList(seg, &nlist, frame)`: the distinct `(int)` values (FS-12) of the frame, sorted
/// (`unqiue_int_list`; FS-10 for a volume without voxels).
pub fn mri_seg_id_list(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Mri,
    frame: i32,
) -> Result<Vec<i32>, Fatal> {
    let mut tmp = Vec::new();
    for c in 0..seg.vg.width {
        for r in 0..seg.vg.height {
            for s in 0..seg.vg.depth {
                let v = mri_get_vox_val(g, err, seg, c, r, s, frame, 0)?;
                tmp.push(int_of(g, "MRIsegIdList", f64::from(v)));
            }
        }
    }
    Ok(unique_int_list(g, &mut tmp))
}

/// `MRIsegIdListNot0(seg, &nsegs, frame)`: [`mri_seg_id_list`] without 0.
pub fn mri_seg_id_list_not0(
    g: &mut Globals,
    err: &mut dyn Write,
    seg: &Mri,
    frame: i32,
) -> Result<Vec<i32>, Fatal> {
    let mut l = mri_seg_id_list(g, err, seg, frame)?;
    l.retain(|&v| v != 0);
    Ok(l)
}

/// `MRIhalfCosBias(in, alpha, out)`: a copy of `in` (`MRIcopy`) whose slice `s` is scaled by
/// `w = (alpha * cos(pi s / (depth - 1)) + (2 - alpha)) / 2` (`double`, glibc's `cos`; the
/// official build's operand orders).
pub fn mri_half_cos_bias(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    input: &Mri,
    alpha: f64,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => {
            if let Ret::Null = mri_copy(g, out, err, input, Some(&mut *d))? {
                return Ok(Ret::Null);
            }
            d
        }
        None => match mri_copy(g, out, err, input, None)? {
            Ret::New(m) => fresh.insert(m),
            _ => return Ok(Ret::Null),
        },
    };
    let libm = Libm::new(g.mode());
    for s in 0..d.vg.depth {
        let arg = div64(
            mul64(std::f64::consts::PI, f64::from(s)),
            sub64(f64::from(d.vg.depth), 1.0),
        );
        let cs = libm.cos(arg);
        let w = mul64(add64(mul64(cs, alpha), sub64(2.0, alpha)), 0.5);
        for c in 0..d.vg.width {
            for r in 0..d.vg.height {
                for f in 0..d.nframes {
                    let v = widen(mri_get_vox_val(g, err, input, c, r, s, f, 0)?);
                    // the official build: v * w (v the destination)
                    mri_set_vox_val(g, err, d, c, r, s, f, mul64(v, w) as f32, 0)?;
                }
            }
        }
    }
    note_glibc07(g, &libm, "cos");
    Ok(ret(fresh))
}
