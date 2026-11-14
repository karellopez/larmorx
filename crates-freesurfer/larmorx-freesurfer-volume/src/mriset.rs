// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Morphology of `utils/mriset.cpp` (3.1-38): `MRIdilate` and `MRIdilateUchar` (taken first:
//! fs.filter needs them). (FreeSurfer runs `MRIdilate`'s slices in parallel; each voxel is its
//! own, so the order does not matter.)

use std::io::Write;

use larmorx_core::defects::{FS_493, FS_494, FS_495, FS_496};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::cscan;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub64};
use larmorx_freesurfer_core::utils::{most_frequent_int_list, nint};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_INT, MRI_UCHAR, Mri, mri_alloc_sequence, mri_copy_header, mri_copy_pulse_parameters,
    mri_free,
};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::{
    Pair, Tri, Vols, check_dims, clone_into, fequal, getv, mri_clone, mri_clone_by_space,
    mri_replace_values_uchar, setv,
};
use crate::combine::mri_dim_mismatch;
use crate::deriv::{Axis, index};
use crate::frames::compare_doubles;
use crate::labels::{WM_MIN_VAL, int_of};
use crate::orient::mri_get_voxel_to_voxel_xform;
use crate::vgeom::{get_vol_geom, vg_is_not_equal_thresh};
use crate::rows::{self, Num, need_voxels};
use crate::sample::{SAMPLE_NEAREST, cvtt, sample_volume_frame, sample_volume_frame_type};
use crate::xform::mri_set_values;

/// The bounding box of the voxels `above_zero` says are above 0 (in any of `nframes` frames),
/// grown by one voxel and clipped to the volume: `[xmin, xmax, ymin, ymax, zmin, zmax]`, starting
/// from `[w - 1, 0, h - 1, 0, d - 1, 0]` as FreeSurfer's does.
fn grown_box(
    w: i32,
    h: i32,
    d: i32,
    nframes: i32,
    mut above_zero: impl FnMut(i32, i32, i32, i32) -> Result<bool, Fatal>,
) -> Result<[i32; 6], Fatal> {
    let (mut xmin, mut xmax, mut ymin, mut ymax, mut zmin, mut zmax) =
        (w - 1, 0, h - 1, 0, d - 1, 0);
    for f in 0..nframes {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if above_zero(x, y, z, f)? {
                        xmin = xmin.min(x - 1);
                        xmax = xmax.max(x + 1);
                        ymin = ymin.min(y - 1);
                        ymax = ymax.max(y + 1);
                        zmin = zmin.min(z - 1);
                        zmax = zmax.max(z + 1);
                    }
                }
            }
        }
    }
    Ok([
        xmin.max(0),
        xmax.min(w - 1),
        ymin.max(0),
        ymax.min(h - 1),
        zmin.max(0),
        zmax.min(d - 1),
    ])
}

/// The destination of a [`Pair`]: `MRIclone` of the source when none is given, then
/// `MRIcheckVolDims`; for the source itself a clone to write into (`same`).
enum Dst<'a> {
    New(Mri),
    Given(&'a mut Mri),
    Same(&'a mut Mri, Mri),
}

fn resolve<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair<'a>,
) -> Result<(Option<&'a Mri>, Dst<'a>), Fatal> {
    Ok(match p {
        Pair::New(s) => {
            let d = mri_clone(g, out, err, s, None)?
                .into_new()
                .expect("MRIclone without a destination");
            check_dims(g, err, s, &d)?;
            (Some(s), Dst::New(d))
        }
        Pair::Given(s, d) => {
            check_dims(g, err, s, d)?;
            (Some(s), Dst::Given(d))
        }
        Pair::Same(m) => {
            let d = mri_clone(g, out, err, m, None)?
                .into_new()
                .expect("MRIclone without a destination");
            (None, Dst::Same(m, d))
        }
    })
}

/// The source and the volume written of a resolved destination.
fn parts<'b>(src: Option<&'b Mri>, dst: &'b mut Dst) -> (&'b Mri, &'b mut Mri) {
    match dst {
        Dst::New(d) => (src.expect("a source"), d),
        Dst::Given(d) => (src.expect("a source"), d),
        Dst::Same(m, d) => (m, d),
    }
}

/// The end of a dilation: for the source itself, `MRIcopy(dst, src)` (voxels and header).
fn finish(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dst: Dst,
) -> Result<Ret<Mri>, Fatal> {
    Ok(match dst {
        Dst::New(d) => Ret::New(d),
        Dst::Given(_) => Ret::Given,
        Dst::Same(m, d) => {
            mri_copy(g, out, err, &d, Some(m))?;
            Ret::Given
        }
    })
}

/// A byte of frame 0 through FreeSurfer's byte pointer (`MRIvox`).
fn byte(g: &mut Globals, what: &str, m: &Mri, x: i32, y: i32, z: i32) -> u8 {
    match rows::read(
        g,
        what,
        m,
        i64::from(z),
        i64::from(y),
        i64::from(x),
        MRI_UCHAR,
    ) {
        Num::I(i) => i as u8,
        Num::F(f) => f as u8,
    }
}

/// `MRIdilateUchar(mri_src, mri_dst)`: each voxel of frame 0 inside the bounding box of the
/// nonzero bytes, grown by one voxel, becomes the largest byte of its 3 x 3 x 3 neighbourhood
/// (the index tables clamp it at the faces), stored as a byte (into a destination of another
/// type, the byte at the voxel's index in the row, as FreeSurfer's byte pointer writes it);
/// the destination's other voxels are left as they are. Without a destination, a clone of the
/// source (`MRIclone`: zeros); a destination of another extent ends the program
/// (`MRIcheckVolDims`); the source itself is dilated into a clone, copied back (`MRIcopy`).
pub fn mri_dilate_uchar(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIdilateUchar";
    let (src_own, mut dst) = resolve(g, out, err, p)?;
    {
        let (src, d) = parts(src_own, &mut dst);
        let (w, h, dd) = (src.vg.width, src.vg.height, src.vg.depth);
        let [xmin, xmax, ymin, ymax, zmin, zmax] =
            grown_box(
                w,
                h,
                dd,
                1,
                |x, y, z, _| Ok(byte(g, WHAT, src, x, y, z) > 0),
            )?;
        for z in zmin..=zmax {
            for y in ymin..=ymax {
                for x in xmin..=xmax {
                    let mut max_val = 0u8;
                    for z0 in -1..=1 {
                        let zi = index(g, err, WHAT, src, Axis::Z, z + z0)?;
                        for y0 in -1..=1 {
                            let yi = index(g, err, WHAT, src, Axis::Y, y + y0)?;
                            for x0 in -1..=1 {
                                let xi = index(g, err, WHAT, src, Axis::X, x + x0)?;
                                max_val = max_val.max(byte(g, WHAT, src, xi, yi, zi));
                            }
                        }
                    }
                    let (z, y, x) = (i64::from(z), i64::from(y), i64::from(x));
                    rows::write(g, WHAT, d, z, y, x, MRI_UCHAR, Num::I(i64::from(max_val)));
                }
            }
        }
    }
    finish(g, out, err, dst)
}

/// A nearest sample of frame `f` at a voxel (`MRIsampleVolumeFrameType`, `SAMPLE_NEAREST`).
fn nearest(g: &mut Globals, err: &mut dyn Write, m: &Mri, x: i32, y: i32, z: i32, f: i32) -> f64 {
    let mut val = 0.0f64;
    let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
    sample_volume_frame_type(g, err, m, x, y, z, f, SAMPLE_NEAREST, &mut val);
    val
}

/// `MRIdilate(mri_src, mri_dst)`: a byte source goes to [`mri_dilate_uchar`]. Otherwise the
/// bounding box of the voxels above 0 in any frame (nearest samples, `MRIsampleVolumeFrameType`),
/// grown by one voxel; in it, each voxel of each frame becomes the largest of 0 and its
/// 3 x 3 x 3 neighbourhood's samples (the index tables clamp it at the faces; a NaN never
/// wins), written with `MRIsetVoxVal` (the destination's type). The destination as for
/// [`mri_dilate_uchar`].
pub fn mri_dilate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIdilate";
    let src_type = match &p {
        Pair::New(s) | Pair::Given(s, _) => s.mtype,
        Pair::Same(m) => m.mtype,
    };
    if src_type == MRI_UCHAR {
        return mri_dilate_uchar(g, out, err, p);
    }
    let (src_own, mut dst) = resolve(g, out, err, p)?;
    {
        let (src, d) = parts(src_own, &mut dst);
        let (w, h, dd, nf) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
        let [xmin, xmax, ymin, ymax, zmin, zmax] = grown_box(w, h, dd, nf, |x, y, z, f| {
            Ok(nearest(g, err, src, x, y, z, f) > 0.0)
        })?;
        for f in 0..nf {
            for z in zmin..=zmax {
                for y in ymin..=ymax {
                    for x in xmin..=xmax {
                        let mut max_val = 0.0f64;
                        for z0 in -1..=1 {
                            let zi = index(g, err, WHAT, src, Axis::Z, z + z0)?;
                            for y0 in -1..=1 {
                                let yi = index(g, err, WHAT, src, Axis::Y, y + y0)?;
                                for x0 in -1..=1 {
                                    let xi = index(g, err, WHAT, src, Axis::X, x + x0)?;
                                    let val = nearest(g, err, src, xi, yi, zi, f);
                                    if val > max_val {
                                        max_val = val;
                                    }
                                }
                            }
                        }
                        mri_set_vox_val(g, err, d, x, y, z, f, max_val as f32, 0)?;
                    }
                }
            }
        }
    }
    finish(g, out, err, dst)
}

// ---- relay 23: the rest of mriset.cpp's reached functions -------------------------------------

/// A byte of frame 0 written through FreeSurfer's byte pointer (`MRIvox(m, x, y, z) = v`).
fn put_byte(g: &mut Globals, what: &str, m: &mut Mri, x: i32, y: i32, z: i32, v: u8) {
    let (z, y, x) = (i64::from(z), i64::from(y), i64::from(x));
    rows::write(g, what, m, z, y, x, MRI_UCHAR, Num::I(i64::from(v)));
}

/// `MRIunion(mri1, mri2, mri_dst)`: frame 0's bytes (FreeSurfer's byte pointers, whatever the
/// type), the larger of the two at each voxel. `MRIcheckVolDims(mri1, mri2)` first; without a
/// destination a clone of `mri1`.
pub fn mri_union(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIunion";
    let mut v = Vols::of_tri(t);
    check_dims(g, err, v.a(), v.b())?;
    clone_into(g, out, err, &mut v)?;
    for m in [v.a(), v.b(), v.dref()] {
        need_voxels(g, err, WHAT, m)?;
    }
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let v1 = byte(g, WHAT, v.a(), x, y, z);
                let v2 = byte(g, WHAT, v.b(), x, y, z);
                put_byte(g, WHAT, v.d(), x, y, z, v1.max(v2));
            }
        }
    }
    Ok(v.done())
}

/// `MRIintersect(mri1, mri2, mri_dst)`: for each frame of the destination, the smaller of the
/// two voxels truncated to `int`s (`MRIgetVoxVal`; FS-12 beyond the range), written with
/// `MRIsetVoxVal`. `MRIcheckVolDims(mri1, mri2)` first; without a destination a clone of `mri1`.
pub fn mri_intersect(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIintersect";
    let mut v = Vols::of_tri(t);
    check_dims(g, err, v.a(), v.b())?;
    clone_into(g, out, err, &mut v)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for f in 0..v.dref().nframes {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let a = getv(g, err, &v, true, x, y, z, f)?;
                    let v1 = int_of(g, WHAT, f64::from(a));
                    let b = getv(g, err, &v, false, x, y, z, f)?;
                    let v2 = int_of(g, WHAT, f64::from(b));
                    let m = if v1 < v2 { v1 } else { v2 };
                    setv(g, err, &mut v, x, y, z, f, m as f32)?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRIcomplement(mri_src, mri_dst)`: `!(int) voxel` (1 for 0, else 0) for every frame of the
/// source. `MRIcheckVolDims` with a destination; without one `MRIalloc` of one frame of the
/// source's extent and type with the source's header (`MRIcopyHeader`).
pub fn mri_complement(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIcomplement";
    let (mut v, _) = Vols::of_pair(p);
    if v.has_dst() {
        check_dims(g, err, v.a(), v.dref())?;
    } else {
        let s = v.a();
        let mut d = mri_alloc_sequence(
            g,
            out,
            err,
            s.vg.width,
            s.vg.height,
            s.vg.depth,
            s.mtype,
            1,
        )?;
        mri_copy_header(g, out, err, v.a(), Some(&mut d))?;
        v.fresh = Some(d);
    }
    let (w, h, d, nf) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth, v.a().nframes);
    for f in 0..nf {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let a = getv(g, err, &v, true, x, y, z, f)?;
                    let val = int_of(g, WHAT, f64::from(a));
                    setv(g, err, &mut v, x, y, z, f, if val == 0 { 1.0 } else { 0.0 })?;
                }
            }
        }
    }
    Ok(v.done())
}

/// A voxel by `MRIsampleVolumeFrame` (at whole coordinates: the nearest sample).
fn sample_at(g: &mut Globals, err: &mut dyn Write, m: &Mri, x: i32, y: i32, z: i32, f: i32) -> f64 {
    let mut val = 0.0f64;
    sample_volume_frame(g, err, m, f64::from(x), f64::from(y), f64::from(z), f, &mut val);
    val
}

/// `MRIand(mri1, mri2, mri_dst, thresh)`: for each frame of `mri1`, 1 where both voxels
/// (`MRIsampleVolumeFrame`, each taken as 0 below `thresh`) are not 0 (a NaN is not 0), else 0.
/// `MRIcheckVolDims(mri1, mri2)` first; without a destination a clone of `mri1`.
pub fn mri_and(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
    thresh: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut v = Vols::of_tri(t);
    check_dims(g, err, v.a(), v.b())?;
    clone_into(g, out, err, &mut v)?;
    let (w, h, d, nf) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth, v.a().nframes);
    let th = f64::from(thresh);
    for f in 0..nf {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mut v1 = sample_at(g, err, v.a(), x, y, z, f);
                    if v1 < th {
                        v1 = 0.0;
                    }
                    let mut v2 = sample_at(g, err, v.b(), x, y, z, f);
                    if v2 < th {
                        v2 = 0.0;
                    }
                    let on = v1 != 0.0 && v2 != 0.0;
                    setv(g, err, &mut v, x, y, z, f, if on { 1.0 } else { 0.0 })?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRIxor(mri1, mri2, mri_dst, t1, t2)`: frame 0; each voxel truncated to an `int` is "on" when
/// in `[t1, t2]`; 1 where exactly one of the two is on, else 0. `MRIcheckVolDims(mri1, mri2)`
/// first; without a destination a clone of `mri1`.
pub fn mri_xor(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: Tri,
    t1: i32,
    t2: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIxor";
    let mut v = Vols::of_tri(t);
    check_dims(g, err, v.a(), v.b())?;
    clone_into(g, out, err, &mut v)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let a = getv(g, err, &v, true, x, y, z, 0)?;
                let v1 = int_of(g, WHAT, f64::from(a));
                let b = getv(g, err, &v, false, x, y, z, 0)?;
                let v2 = int_of(g, WHAT, f64::from(b));
                let (o1, o2) = (v1 >= t1 && v1 <= t2, v2 >= t1 && v2 <= t2);
                setv(g, err, &mut v, x, y, z, 0, f32::from(u8::from(o1 ^ o2)))?;
            }
        }
    }
    Ok(v.done())
}

/// `MRIsetEdges(in, SetVal, out)`: `MRIcopy(in, out)` (`NULL` where it gives none), the pulse
/// parameters and colour table copied, then every voxel of every frame on the six faces set to
/// `SetVal` (as a `float`, `MRIsetVoxVal`).
pub fn mri_set_edges(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    set_val: f64,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let o: &mut Mri = match p {
        Pair::New(s) => {
            let Some(m) = mri_copy(g, out, err, s, None)?.into_new() else {
                return Ok(Ret::Null);
            };
            let o = fresh.insert(m);
            mri_copy_pulse_parameters(s, o);
            if s.ct.is_some() {
                o.ct = ctab_deep_copy(g, err, s.ct.as_ref());
            }
            o
        }
        Pair::Given(s, d) => {
            if matches!(mri_copy(g, out, err, s, Some(d))?, Ret::Null) {
                return Ok(Ret::Null);
            }
            mri_copy_pulse_parameters(s, d);
            if s.ct.is_some() {
                d.ct = ctab_deep_copy(g, err, s.ct.as_ref());
            }
            d
        }
        // MRIcopy of a volume into itself returns at once; its own pulse parameters and colour
        // table copied into it change nothing
        Pair::Same(m) => {
            if m.ct.is_some() {
                let ct = ctab_deep_copy(g, err, m.ct.as_ref());
                m.ct = ct;
            }
            m
        }
    };
    let val = set_val as f32;
    let (w, h, d, nf) = (o.vg.width, o.vg.height, o.vg.depth, o.nframes);
    for k in 0..2 {
        let c = if k == 0 { 0 } else { w - 1 };
        for r in 0..h {
            for s in 0..d {
                for f in 0..nf {
                    mri_set_vox_val(g, err, o, c, r, s, f, val, 0)?;
                }
            }
        }
        let r = if k == 0 { 0 } else { h - 1 };
        for c in 0..w {
            for s in 0..d {
                for f in 0..nf {
                    mri_set_vox_val(g, err, o, c, r, s, f, val, 0)?;
                }
            }
        }
        let s = if k == 0 { 0 } else { d - 1 };
        for c in 0..w {
            for r in 0..h {
                for f in 0..nf {
                    mri_set_vox_val(g, err, o, c, r, s, f, val, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `NEAREST_NEIGHBOR_FACE` (`mri.h`).
pub const NEAREST_NEIGHBOR_FACE: i32 = 1;
/// `NEAREST_NEIGHBOR_EDGE` (`mri.h`).
pub const NEAREST_NEIGHBOR_EDGE: i32 = 2;
/// `NEAREST_NEIGHBOR_CORNER` (`mri.h`).
pub const NEAREST_NEIGHBOR_CORNER: i32 = 3;

/// `MRIerodeNN(in, out, NNDef, ErodeEdges)`: frame 0; each voxel the smallest of itself and its
/// neighbours in the volume (faces, edges or corners by `NNDef`; any other value: all 26), in
/// `double`; with `ErodeEdges` 0 where any neighbour (of the 26, before `NNDef` is applied) is
/// outside the volume (then smaller neighbours inside still win). In place: `ERROR:
/// MRIerodeNN(): cannot be done in-place` on standard output and `NULL`. Without a destination,
/// one frame of the input's extent and type with its header, pulse parameters and colour table;
/// a given one of other dimensions: `ERROR: MRIerodeNN(): seg/out dim mismatch` and `NULL`, else
/// it is cleared first (`MRIsetValues(out, 0)`).
pub fn mri_erode_nn(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    nn_def: i32,
    erode_edges: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let (src, o): (&Mri, &mut Mri) = match p {
        Pair::Same(_) => {
            let _ = writeln!(out, "ERROR: MRIerodeNN(): cannot be done in-place");
            return Ok(Ret::Null);
        }
        Pair::New(s) => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                s.vg.width,
                s.vg.height,
                s.vg.depth,
                s.mtype,
                1,
            )?;
            mri_copy_header(g, out, err, s, Some(&mut m))?;
            mri_copy_pulse_parameters(s, &mut m);
            if s.ct.is_some() {
                m.ct = ctab_deep_copy(g, err, s.ct.as_ref());
            }
            (s, fresh.insert(m))
        }
        Pair::Given(s, d) => {
            if mri_dim_mismatch(s, d, false) != 0 {
                let _ = writeln!(out, "ERROR: MRIerodeNN(): seg/out dim mismatch");
                return Ok(Ret::Null);
            }
            mri_set_values(g, err, d, 0.0)?;
            (s, d)
        }
    };
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let mut valmin = f64::from(mri_get_vox_val(g, err, src, c, r, s, 0, 0)?);
                for dc in -1..=1i32 {
                    for dr in -1..=1i32 {
                        for ds in -1..=1i32 {
                            if c + dc < 0
                                || c + dc > w - 1
                                || r + dr < 0
                                || r + dr > h - 1
                                || s + ds < 0
                                || s + ds > d - 1
                            {
                                if erode_edges != 0 {
                                    valmin = 0.0;
                                }
                                continue;
                            }
                            let n = dc.abs() + dr.abs() + ds.abs();
                            if nn_def == NEAREST_NEIGHBOR_FACE && n > 1 {
                                continue;
                            }
                            if nn_def == NEAREST_NEIGHBOR_EDGE && n > 2 {
                                continue;
                            }
                            let val =
                                f64::from(mri_get_vox_val(g, err, src, c + dc, r + dr, s + ds, 0, 0)?);
                            if val < valmin {
                                valmin = val;
                            }
                        }
                    }
                }
                mri_set_vox_val(g, err, o, c, r, s, 0, valmin as f32, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIerode(mri_src, mri_dst)`: each voxel the smallest of its 3 x 3 x 3 neighbourhood (the
/// index tables clamp it at the faces). Bytes in and out: frame 0 through the byte pointers,
/// starting from 255; otherwise every frame of the source as `float`s from the voxel itself
/// (`MRIgetVoxVal`; a NaN never wins after the first), written with `MRIsetVoxVal`. The
/// destination as for [`mri_dilate_uchar`].
pub fn mri_erode(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIerode";
    let (src_own, mut dst) = resolve(g, out, err, p)?;
    {
        let (src, d) = parts(src_own, &mut dst);
        let (w, h, dd, nf) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
        if src.mtype != MRI_UCHAR || d.mtype != MRI_UCHAR {
            for f in 0..nf {
                for z in 0..dd {
                    for y in 0..h {
                        for x in 0..w {
                            let mut fmin = mri_get_vox_val(g, err, src, x, y, z, f, 0)?;
                            for z0 in -1..=1 {
                                let zi = index(g, err, WHAT, src, Axis::Z, z + z0)?;
                                for y0 in -1..=1 {
                                    let yi = index(g, err, WHAT, src, Axis::Y, y + y0)?;
                                    for x0 in -1..=1 {
                                        let xi = index(g, err, WHAT, src, Axis::X, x + x0)?;
                                        let fv = mri_get_vox_val(g, err, src, xi, yi, zi, f, 0)?;
                                        if fv < fmin {
                                            fmin = fv;
                                        }
                                    }
                                }
                            }
                            mri_set_vox_val(g, err, d, x, y, z, f, fmin, 0)?;
                        }
                    }
                }
            }
        } else {
            need_voxels(g, err, WHAT, src)?;
            need_voxels(g, err, WHAT, d)?;
            for z in 0..dd {
                for y in 0..h {
                    for x in 0..w {
                        let mut min_val = 255u8;
                        for z0 in -1..=1 {
                            let zi = index(g, err, WHAT, src, Axis::Z, z + z0)?;
                            for y0 in -1..=1 {
                                let yi = index(g, err, WHAT, src, Axis::Y, y + y0)?;
                                for x0 in -1..=1 {
                                    let xi = index(g, err, WHAT, src, Axis::X, x + x0)?;
                                    min_val = min_val.min(byte(g, WHAT, src, xi, yi, zi));
                                }
                            }
                        }
                        put_byte(g, WHAT, d, x, y, z, min_val);
                    }
                }
            }
        }
    }
    finish(g, out, err, dst)
}

/// `MRIerode2D(mri_src, mri_dst)`: frame 0; each voxel the smallest of its 3 x 3 neighbourhood
/// in its slice (`double`s of `MRIgetVoxVal`, from the voxel itself; the index tables clamp it),
/// written with `MRIsetVoxVal`. The destination as for [`mri_dilate_uchar`].
pub fn mri_erode_2d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIerode2D";
    let (src_own, mut dst) = resolve(g, out, err, p)?;
    {
        let (src, d) = parts(src_own, &mut dst);
        let (w, h, dd) = (src.vg.width, src.vg.height, src.vg.depth);
        for z in 0..dd {
            for y in 0..h {
                for x in 0..w {
                    let mut fmin = f64::from(mri_get_vox_val(g, err, src, x, y, z, 0, 0)?);
                    for y0 in -1..=1 {
                        let yi = index(g, err, WHAT, src, Axis::Y, y + y0)?;
                        for x0 in -1..=1 {
                            let xi = index(g, err, WHAT, src, Axis::X, x + x0)?;
                            let fv = f64::from(mri_get_vox_val(g, err, src, xi, yi, z, 0, 0)?);
                            if fv < fmin {
                                fmin = fv;
                            }
                        }
                    }
                    mri_set_vox_val(g, err, d, x, y, z, 0, fmin as f32, 0)?;
                }
            }
        }
    }
    finish(g, out, err, dst)
}

/// A label: frame 0's voxel truncated to an `int` (FS-12).
fn seg_label(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    m: &Mri,
    c: i32,
    r: i32,
    s: i32,
) -> Result<i32, Fatal> {
    let v = mri_get_vox_val(g, err, m, c, r, s, 0, 0)?;
    Ok(int_of(g, what, f64::from(v)))
}

/// A copy of a volume (`MRIcopy(src, NULL)`).
fn copy_new(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
) -> Result<Mri, Fatal> {
    Ok(mri_copy(g, out, err, src, None)?
        .into_new()
        .expect("MRIcopy without a destination"))
}

/// FS-493: a count that never reaches 1 by lowering (FreeSurfer loops without end): an error in
/// both modes.
fn never_ends(g: &mut Globals, err: &mut dyn Write, what: &str, n: i32) -> Result<(), Fatal> {
    g.notices.add(
        &FS_493,
        format!("{what} with a count of {n} (FreeSurfer never returns)"),
    );
    let msg = format!("{what}: a count of {n} never ends (FreeSurfer loops forever)\n");
    log::error_exit(g, err, ERROR_BADPARM, &msg)?;
    Ok(())
}

/// The volume a [`Pair`] writes into for the segmentation functions that refuse to work in
/// place (`seg == out`: `ERROR: <what>(): cannot be done in-place` and `None`): a given one, or
/// `MRIallocSequence` of one frame of the segmentation's extent and type.
fn seg_out<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    what: &str,
    p: Pair<'a>,
    fresh: &'a mut Option<Mri>,
) -> Result<Option<(&'a Mri, &'a mut Mri)>, Fatal> {
    Ok(match p {
        Pair::Same(_) => {
            let _ = writeln!(out, "ERROR: {what}(): cannot be done in-place");
            None
        }
        Pair::New(s) => {
            let m = mri_alloc_sequence(
                g,
                out,
                err,
                s.vg.width,
                s.vg.height,
                s.vg.depth,
                s.mtype,
                1,
            )?;
            Some((s, fresh.insert(m)))
        }
        Pair::Given(s, d) => Some((s, d)),
    })
}

/// The `Ret` of a segmentation function: the volume made here, or the one given.
fn ret_of(fresh: Option<Mri>) -> Ret<Mri> {
    match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    }
}

/// `MRIerodeSegmentation(seg, out, nErodes, nDiffThresh)`: `out` gets a copy of the segmentation
/// (`MRIcopy`), then `nErodes - 1` erosions of one step (each of the previous result), then the
/// last step: the voxels on the volume's faces become 0; each other voxel keeps its label
/// (truncated to an `int`, FS-12) unless more than `nDiffThresh` of its 6 face neighbours carry
/// another label, then 0. The segmentation's colour table is deep-copied into `out` when both
/// have one. In place: `ERROR: MRIerodeSegmentation(): cannot be done in-place` and `NULL`; a
/// given `out` of other dimensions: `ERROR: MRIerodeSegmentation(): seg/out dim mismatch` and
/// `NULL` (both on standard output); without one, one frame of the segmentation's extent and type.
/// FS-493 for a count of 0 or below.
pub fn mri_erode_segmentation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    n_erodes: i32,
    n_diff_thresh: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIerodeSegmentation";
    let mut fresh = None;
    let Some((seg, o)) = seg_out(g, out, err, WHAT, p, &mut fresh)? else {
        return Ok(Ret::Null);
    };
    if !erode_seg_into(g, out, err, seg, o, n_erodes, n_diff_thresh)? {
        return Ok(Ret::Null);
    }
    Ok(ret_of(fresh))
}

fn erode_seg_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    seg: &Mri,
    o: &mut Mri,
    n_erodes: i32,
    thr: i32,
) -> Result<bool, Fatal> {
    const WHAT: &str = "MRIerodeSegmentation";
    if mri_dim_mismatch(seg, o, false) != 0 {
        let _ = writeln!(out, "ERROR: MRIerodeSegmentation(): seg/out dim mismatch");
        return Ok(false);
    }
    mri_copy(g, out, err, seg, Some(o))?;
    if n_erodes < 1 {
        never_ends(g, err, WHAT, n_erodes)?;
    }
    let mut seg2 = copy_new(g, out, err, seg)?;
    for _ in 1..n_erodes {
        erode_seg_into(g, out, err, &seg2, o, 1, thr)?;
        mri_copy(g, out, err, o, Some(&mut seg2))?;
    }
    let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                if c == 0 || c == w - 1 || r == 0 || r == h - 1 || s == 0 || s == d - 1 {
                    mri_set_vox_val(g, err, o, c, r, s, 0, 0.0, 0)?;
                    continue;
                }
                let mut segid0 = seg_label(g, err, WHAT, &seg2, c, r, s)?;
                let mut ndiff = 0;
                for dc in -1..=1 {
                    if seg_label(g, err, WHAT, &seg2, c + dc, r, s)? != segid0 {
                        ndiff += 1;
                    }
                }
                for dr in -1..=1 {
                    if seg_label(g, err, WHAT, &seg2, c, r + dr, s)? != segid0 {
                        ndiff += 1;
                    }
                }
                for ds in -1..=1 {
                    if seg_label(g, err, WHAT, &seg2, c, r, s + ds)? != segid0 {
                        ndiff += 1;
                    }
                }
                if ndiff > thr {
                    segid0 = 0;
                }
                mri_set_vox_val(g, err, o, c, r, s, 0, segid0 as f32, 0)?;
            }
        }
    }
    if o.ct.is_some() && seg.ct.is_some() {
        o.ct = ctab_deep_copy(g, err, seg.ct.as_ref());
    }
    Ok(true)
}

/// `MRIdilateSegmentation(seg, out, nDils, mask, maskframe, maskthresh, &nchanges)`: `out` gets a
/// copy of the segmentation, then dilations of one step: each voxel of label 0 (truncated to an
/// `int`, FS-12) inside the mask (the mask's `maskframe` voxel not below `maskthresh`; every voxel
/// without one) whose 6 face neighbours in the volume carry labels other than 0 takes the most
/// frequent of them (`most_frequent_int_list`: the smallest of a tie). `nDils` steps (each of the
/// previous result; `nchanges` the last step's count of voxels changed), or with `nDils` -1 steps
/// until one changes nothing (`nchanges` the number of steps). In place, of other dimensions and
/// without `out`: as [`mri_erode_segmentation`] (`MRIdilateSegmentation()` in the messages).
/// FS-493 for a count of 0 or below -1.
#[allow(clippy::too_many_arguments)]
pub fn mri_dilate_segmentation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    n_dils: i32,
    mask: Option<&Mri>,
    maskframe: i32,
    maskthresh: f64,
    nchanges: &mut i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIdilateSegmentation";
    let mut fresh = None;
    let Some((seg, o)) = seg_out(g, out, err, WHAT, p, &mut fresh)? else {
        return Ok(Ret::Null);
    };
    let m = DilMask {
        mask,
        frame: maskframe,
        thresh: maskthresh,
    };
    if !dilate_seg_into(g, out, err, seg, o, n_dils, &m, nchanges)? {
        return Ok(Ret::Null);
    }
    Ok(ret_of(fresh))
}

struct DilMask<'a> {
    mask: Option<&'a Mri>,
    frame: i32,
    thresh: f64,
}

#[allow(clippy::too_many_arguments)]
fn dilate_seg_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    seg: &Mri,
    o: &mut Mri,
    n_dils: i32,
    m: &DilMask,
    pnchanges: &mut i32,
) -> Result<bool, Fatal> {
    const WHAT: &str = "MRIdilateSegmentation";
    if mri_dim_mismatch(seg, o, false) != 0 {
        let _ = writeln!(out, "ERROR: MRIdilateSegmentation(): seg/out dim mismatch");
        return Ok(false);
    }
    mri_copy(g, out, err, seg, Some(o))?;
    let mut seg2 = copy_new(g, out, err, seg)?;
    if n_dils == -1 {
        let (mut n, mut nch) = (0, 1);
        while nch != 0 {
            n += 1;
            dilate_seg_into(g, out, err, &seg2, o, 1, m, &mut nch)?;
            mri_copy(g, out, err, o, Some(&mut seg2))?;
        }
        *pnchanges = n;
        return Ok(true);
    }
    if n_dils < 1 {
        never_ends(g, err, WHAT, n_dils)?;
    }
    for _ in 1..n_dils {
        dilate_seg_into(g, out, err, &seg2, o, 1, m, pnchanges)?;
        mri_copy(g, out, err, o, Some(&mut seg2))?;
    }
    let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    let mut nch = 0;
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                if seg_label(g, err, WHAT, &seg2, c, r, s)? != 0 {
                    continue;
                }
                if let Some(mk) = m.mask {
                    let mval = f64::from(mri_get_vox_val(g, err, mk, c, r, s, m.frame, 0)?);
                    if mval < m.thresh {
                        continue;
                    }
                }
                let mut nbr = [0i32; 6];
                let mut nn = 0;
                for (dc, dr, ds) in [
                    (-1, 0, 0),
                    (1, 0, 0),
                    (0, -1, 0),
                    (0, 1, 0),
                    (0, 0, -1),
                    (0, 0, 1),
                ] {
                    let (cc, rr, ss) = (c + dc, r + dr, s + ds);
                    if cc < 0 || cc >= w || rr < 0 || rr >= h || ss < 0 || ss >= d {
                        continue;
                    }
                    let id = seg_label(g, err, WHAT, &seg2, cc, rr, ss)?;
                    if id == 0 {
                        continue;
                    }
                    nbr[nn] = id;
                    nn += 1;
                }
                if nn == 0 {
                    continue;
                }
                let (most, _) = most_frequent_int_list(g, &mut nbr[..nn]);
                mri_set_vox_val(g, err, o, c, r, s, 0, most as f32, 0)?;
                nch += 1;
            }
        }
    }
    *pnchanges = nch;
    Ok(true)
}

/// The bounding box of the voxels whose frame-0 byte (FreeSurfer's byte pointer) is `label`,
/// grown by one voxel and clipped to the volume (`[xmin, xmax, ymin, ymax, zmin, zmax]`).
fn label_box(g: &mut Globals, what: &str, m: &Mri, label: i32) -> [i32; 6] {
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let (mut xmin, mut xmax, mut ymin, mut ymax, mut zmin, mut zmax) = (w - 1, 0, h - 1, 0, d - 1, 0);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if i32::from(byte(g, what, m, x, y, z)) == label {
                    xmin = xmin.min(x - 1);
                    xmax = xmax.max(x + 1);
                    ymin = ymin.min(y - 1);
                    ymax = ymax.max(y + 1);
                    zmin = zmin.min(z - 1);
                    zmax = zmax.max(z + 1);
                }
            }
        }
    }
    [
        xmin.max(0),
        xmax.min(w - 1),
        ymin.max(0),
        ymax.min(h - 1),
        zmin.max(0),
        zmax.min(d - 1),
    ]
}

/// `mri_tmp = MRIcopy(mri_dst, mri_tmp)`: allocated the first time.
fn copy_tmp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    tmp: &mut Option<Mri>,
) -> Result<(), Fatal> {
    match tmp.as_mut() {
        Some(t) => {
            mri_copy(g, out, err, src, Some(t))?;
        }
        None => *tmp = Some(copy_new(g, out, err, src)?),
    }
    Ok(())
}

/// `MRIdilateThreshLabel(mri_src, mri_val, mri_dst, label, niter, thresh)`: see
/// [`mri_dilate_thresh_label_any`] (`mri_val >= thresh`).
#[allow(clippy::too_many_arguments)]
pub fn mri_dilate_thresh_label(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mri_val: &Mri,
    label: i32,
    niter: i32,
    thresh: i32,
) -> Result<Ret<Mri>, Fatal> {
    mri_dilate_thresh_label_any(g, out, err, p, mri_val, label, niter, thresh, false)
}

/// `MRIdilateInvThreshLabel(mri_src, mri_val, mri_dst, label, niter, thresh)`: see
/// [`mri_dilate_thresh_label_any`] (`mri_val <= thresh`).
#[allow(clippy::too_many_arguments)]
pub fn mri_dilate_inv_thresh_label(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mri_val: &Mri,
    label: i32,
    niter: i32,
    thresh: i32,
) -> Result<Ret<Mri>, Fatal> {
    mri_dilate_thresh_label_any(g, out, err, p, mri_val, label, niter, thresh, true)
}

/// The two threshold-limited label dilations, on frame 0's bytes (FreeSurfer's byte pointers):
/// `MRIcheckVolDims(mri_src, mri_val)`, the destination a copy of the source (`MRIcopy`); then up
/// to `niter` steps, each on a copy of the destination: in the bounding box of the source's
/// `label` grown by one voxel, each voxel whose `mri_val` byte is at least `thresh` (`inv`: at
/// most) becomes `label` (as a byte) when one of its 27 neighbourhood bytes is `label` (index
/// tables clamp it); for such a voxel the box grows to the last neighbour visited (the box's
/// later rows, columns and slices are visited, its earlier ones only by later rows and slices),
/// and a step that changes no voxel to `label` ends the iterations.
#[allow(clippy::too_many_arguments)]
fn mri_dilate_thresh_label_any(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mri_val: &Mri,
    label: i32,
    niter: i32,
    thresh: i32,
    inv: bool,
) -> Result<Ret<Mri>, Fatal> {
    let what = if inv {
        "MRIdilateInvThreshLabel"
    } else {
        "MRIdilateThreshLabel"
    };
    let (mut v, _) = Vols::of_pair(p);
    check_dims(g, err, v.a(), mri_val)?;
    v.copy_a(g, out, err)?;
    for m in [v.a(), mri_val, v.dref()] {
        need_voxels(g, err, what, m)?;
    }
    let mut tmp: Option<Mri> = None;
    for _ in 0..niter {
        copy_tmp(g, out, err, v.dref(), &mut tmp)?;
        let t = tmp.as_ref().expect("the copy");
        let [mut xmin, mut xmax, mut ymin, mut ymax, mut zmin, mut zmax] =
            label_box(g, what, v.a(), label);
        let mut nadded = 0;
        let mut z = zmin;
        while z <= zmax {
            let mut y = ymin;
            while y <= ymax {
                let mut x = xmin;
                while x <= xmax {
                    let mut out_val = byte(g, what, t, x, y, z);
                    let vv = i32::from(byte(g, what, mri_val, x, y, z));
                    if (!inv && vv >= thresh) || (inv && vv <= thresh) {
                        let (mut xi, mut yi, mut zi) = (0, 0, 0);
                        for z0 in -1..=1 {
                            zi = index(g, err, what, t, Axis::Z, z + z0)?;
                            for y0 in -1..=1 {
                                yi = index(g, err, what, t, Axis::Y, y + y0)?;
                                for x0 in -1..=1 {
                                    xi = index(g, err, what, t, Axis::X, x + x0)?;
                                    if i32::from(byte(g, what, t, xi, yi, zi)) == label {
                                        out_val = label as u8;
                                    }
                                }
                            }
                        }
                        if i32::from(out_val) == label {
                            xmin = xmin.min(xi);
                            xmax = xmax.max(xi);
                            ymin = ymin.min(yi);
                            ymax = ymax.max(yi);
                            zmin = zmin.min(zi);
                            zmax = zmax.max(zi);
                            if i32::from(byte(g, what, v.dref(), x, y, z)) != label {
                                nadded += 1;
                            }
                        }
                    }
                    put_byte(g, what, v.d(), x, y, z, out_val);
                    x += 1;
                }
                y += 1;
            }
            z += 1;
        }
        if nadded == 0 {
            break;
        }
    }
    // MRIfree(&mri_tmp): an error when no step ran
    mri_free(g, err, &mut tmp);
    Ok(v.done())
}

/// `MRIdilateLabelUchar(mri_src, mri_dst, label, niter)`: on frame 0's bytes; `MRIcheckVolDims`
/// with a destination, the destination a copy of the source; then `niter` steps, each on a copy
/// of the destination: in the bounding box of the source's `label` grown by one voxel, each voxel
/// becomes `label` when it or one of its 27 neighbourhood bytes is `label` (the search stops at
/// the first).
pub fn mri_dilate_label_uchar(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    label: i32,
    niter: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIdilateLabelUchar";
    let (mut v, _) = Vols::of_pair(p);
    if v.has_dst() {
        check_dims(g, err, v.a(), v.dref())?;
    }
    v.copy_a(g, out, err)?;
    need_voxels(g, err, WHAT, v.a())?;
    need_voxels(g, err, WHAT, v.dref())?;
    let mut tmp: Option<Mri> = None;
    for _ in 0..niter {
        copy_tmp(g, out, err, v.dref(), &mut tmp)?;
        let t = tmp.as_ref().expect("the copy");
        let [xmin, xmax, ymin, ymax, zmin, zmax] = label_box(g, WHAT, v.a(), label);
        for z in zmin..=zmax {
            for y in ymin..=ymax {
                for x in xmin..=xmax {
                    let mut out_val = byte(g, WHAT, t, x, y, z);
                    'search: for z0 in -1..=1 {
                        if i32::from(out_val) == label {
                            break;
                        }
                        let zi = index(g, err, WHAT, t, Axis::Z, z + z0)?;
                        for y0 in -1..=1 {
                            if i32::from(out_val) == label {
                                continue 'search;
                            }
                            let yi = index(g, err, WHAT, t, Axis::Y, y + y0)?;
                            for x0 in -1..=1 {
                                if i32::from(out_val) == label {
                                    break;
                                }
                                let xi = index(g, err, WHAT, t, Axis::X, x + x0)?;
                                if i32::from(byte(g, WHAT, t, xi, yi, zi)) == label {
                                    out_val = label as u8;
                                }
                            }
                        }
                    }
                    put_byte(g, WHAT, v.d(), x, y, z, out_val);
                }
            }
        }
    }
    // MRIfree(&mri_tmp): an error when no step ran
    mri_free(g, err, &mut tmp);
    Ok(v.done())
}

/// `MRIdilateLabel(mri_src, mri_dst, label, niter)`: a byte source goes to
/// [`mri_dilate_label_uchar`]. Otherwise as it for every frame, with `float` voxels
/// (`MRIgetVoxVal`, compared with `label` in `double`), the bounding box made from the copy's
/// voxels in any frame, written with `MRIsetVoxVal`.
pub fn mri_dilate_label(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    label: i32,
    niter: i32,
) -> Result<Ret<Mri>, Fatal> {
    let src_type = match &p {
        Pair::New(s) | Pair::Given(s, _) => s.mtype,
        Pair::Same(m) => m.mtype,
    };
    if src_type == MRI_UCHAR {
        if let Pair::Given(s, d) = &p {
            check_dims(g, err, s, d)?;
        }
        return mri_dilate_label_uchar(g, out, err, p, label, niter);
    }
    let (mut v, _) = Vols::of_pair(p);
    if v.has_dst() {
        check_dims(g, err, v.a(), v.dref())?;
    }
    v.copy_a(g, out, err)?;
    let lab = f64::from(label);
    let nf = v.a().nframes;
    let mut tmp: Option<Mri> = None;
    for _ in 0..niter {
        copy_tmp(g, out, err, v.dref(), &mut tmp)?;
        let t = tmp.as_ref().expect("the copy");
        let (w, h, d) = (t.vg.width, t.vg.height, t.vg.depth);
        let (mut xmin, mut xmax, mut ymin, mut ymax, mut zmin, mut zmax) =
            (w - 1, 0, h - 1, 0, d - 1, 0);
        for f in 0..nf {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if f64::from(mri_get_vox_val(g, err, t, x, y, z, f, 0)?) == lab {
                            xmin = xmin.min(x - 1);
                            xmax = xmax.max(x + 1);
                            ymin = ymin.min(y - 1);
                            ymax = ymax.max(y + 1);
                            zmin = zmin.min(z - 1);
                            zmax = zmax.max(z + 1);
                        }
                    }
                }
            }
        }
        let (xmin, xmax, ymin, ymax) = (xmin.max(0), xmax.min(w - 1), ymin.max(0), ymax.min(h - 1));
        let (zmin, zmax) = (zmin.max(0), zmax.min(d - 1));
        for f in 0..nf {
            for z in zmin..=zmax {
                for y in ymin..=ymax {
                    for x in xmin..=xmax {
                        let mut out_val = f64::from(mri_get_vox_val(g, err, t, x, y, z, f, 0)?);
                        'search: for z0 in -1..=1 {
                            if out_val == lab {
                                break;
                            }
                            let zi = index(g, err, "MRIdilateLabel", t, Axis::Z, z + z0)?;
                            for y0 in -1..=1 {
                                if out_val == lab {
                                    continue 'search;
                                }
                                let yi = index(g, err, "MRIdilateLabel", t, Axis::Y, y + y0)?;
                                for x0 in -1..=1 {
                                    if out_val == lab {
                                        break;
                                    }
                                    let xi = index(g, err, "MRIdilateLabel", t, Axis::X, x + x0)?;
                                    let val = f64::from(mri_get_vox_val(g, err, t, xi, yi, zi, f, 0)?);
                                    if val == lab {
                                        out_val = lab;
                                        break;
                                    }
                                }
                            }
                        }
                        mri_set_vox_val(g, err, v.d(), x, y, z, f, out_val as f32, 0)?;
                    }
                }
            }
        }
    }
    // MRIfree(&mri_tmp): an error when no step ran
    mri_free(g, err, &mut tmp);
    Ok(v.done())
}

/// `MRIopenN(mri_src, mri_dst, order)`: `order` erosions ([`mri_erode`], the first into a new
/// volume, the others in place), then `order` dilations ([`mri_dilate`], the first into the
/// destination, the others in place).
pub fn mri_open_n(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    order: i32,
) -> Result<Ret<Mri>, Fatal> {
    morph2(g, out, err, p, order, true)
}

/// `MRIopen(mri_src, mri_dst)`: [`mri_erode`] into a new volume, then [`mri_dilate`] into the
/// destination.
pub fn mri_open(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    morph2(g, out, err, p, 1, true)
}

/// `MRIclose(mri_src, mri_dst)`: [`mri_dilate`] into a new volume, then [`mri_erode`] into the
/// destination.
pub fn mri_close(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    morph2(g, out, err, p, 1, false)
}

type Morph = fn(&mut Globals, &mut dyn Write, &mut dyn Write, Pair) -> Result<Ret<Mri>, Fatal>;

/// An opening (`open`: erosions then dilations) or closing of `order` steps each.
fn morph2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    order: i32,
    open: bool,
) -> Result<Ret<Mri>, Fatal> {
    let (first, second): (Morph, Morph) = if open {
        (mri_erode, mri_dilate)
    } else {
        (mri_dilate, mri_erode)
    };
    let (src, dst): (&Mri, Option<&mut Mri>) = match p {
        Pair::New(s) => (s, None),
        Pair::Given(s, d) => (s, Some(d)),
        Pair::Same(m) => {
            // the source is read first (into a new volume), then written as the destination
            let mut tmp = first(g, out, err, Pair::New(m))?
                .into_new()
                .expect("a new volume");
            for _ in 1..order {
                first(g, out, err, Pair::Same(&mut tmp))?;
            }
            second(g, out, err, Pair::Given(&tmp, m))?;
            for _ in 1..order {
                second(g, out, err, Pair::Same(m))?;
            }
            return Ok(Ret::Given);
        }
    };
    let mut tmp = first(g, out, err, Pair::New(src))?
        .into_new()
        .expect("a new volume");
    for _ in 1..order {
        first(g, out, err, Pair::Same(&mut tmp))?;
    }
    match dst {
        Some(d) => {
            second(g, out, err, Pair::Given(&tmp, d))?;
            for _ in 1..order {
                second(g, out, err, Pair::Same(d))?;
            }
            Ok(Ret::Given)
        }
        None => {
            let r = second(g, out, err, Pair::New(&tmp))?;
            let Ret::New(mut d) = r else {
                return Ok(r);
            };
            for _ in 1..order {
                second(g, out, err, Pair::Same(&mut d))?;
            }
            Ok(Ret::New(d))
        }
    }
}

/// `MRIdilate6(mri_src, mri_dst)`: frame 0; each voxel the largest of its 6 face neighbours and
/// itself (from -1e10 as a `float`; the voxel itself read twice, as FreeSurfer's loop does; the
/// index tables clamp at the faces), written with `MRIsetVoxVal`. Without a destination a clone of
/// the source (`MRIclone`); `MRIcheckVolDims`. In place, voxels already written are read as
/// FreeSurfer's single pass reads them.
pub fn mri_dilate6(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIdilate6";
    let (mut v, _) = Vols::of_pair(p);
    clone_into(g, out, err, &mut v)?;
    check_dims(g, err, v.a(), v.dref())?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let mut xi = index(g, err, WHAT, v.a(), Axis::X, x)?;
                let mut yi = index(g, err, WHAT, v.a(), Axis::Y, y)?;
                let mut max_val = -1e10f32;
                for z1 in -1..=1 {
                    let zi = index(g, err, WHAT, v.a(), Axis::Z, z + z1)?;
                    let val = getv(g, err, &v, true, xi, yi, zi, 0)?;
                    if val > max_val {
                        max_val = val;
                    }
                }
                let zi = index(g, err, WHAT, v.a(), Axis::Z, z)?;
                for y1 in [-1, 1] {
                    yi = index(g, err, WHAT, v.a(), Axis::Y, y + y1)?;
                    let val = getv(g, err, &v, true, xi, yi, zi, 0)?;
                    if val > max_val {
                        max_val = val;
                    }
                }
                yi = index(g, err, WHAT, v.a(), Axis::Y, y)?;
                // FreeSurfer's test here looks at the finished y loop's counter: x1 = 0 is read too
                for x1 in -1..=1 {
                    xi = index(g, err, WHAT, v.a(), Axis::X, x + x1)?;
                    let val = getv(g, err, &v, true, xi, yi, zi, 0)?;
                    if val > max_val {
                        max_val = val;
                    }
                }
                setv(g, err, &mut v, x, y, z, 0, max_val)?;
            }
        }
    }
    Ok(v.done())
}

/// `MRIreplaceList(seg, srclist, targlist, nlist, mask, out)`: frame 0; each label (truncated to
/// an `int`, FS-12) found in `srclist` becomes the `targlist` entry at the same place (the first
/// match), others stay, and voxels where the mask is below 0.5 keep their label. Without `out`,
/// one frame of `MRI_INT` with the segmentation's header and pulse parameters; `out` of other
/// dimensions: `ERROR: MRIreplaceList(): seg/out dim mismatch`, a byte `out` with a target above
/// 255: `ERROR: MRIreplaceList(): output is UCHAR, but target is > 255` (both on standard output,
/// `NULL`). `nlist` is the shorter list's length.
pub fn mri_replace_list(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    srclist: &[i32],
    targlist: &[i32],
    mask: Option<&Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIreplaceList";
    let nlist = srclist.len().min(targlist.len());
    let (mut v, _) = Vols::of_pair(p);
    if !v.has_dst() {
        let s = v.a();
        let mut d = mri_alloc_sequence(
            g,
            out,
            err,
            s.vg.width,
            s.vg.height,
            s.vg.depth,
            MRI_INT,
            1,
        )?;
        mri_copy_header(g, out, err, v.a(), Some(&mut d))?;
        mri_copy_pulse_parameters(v.a(), &mut d);
        v.fresh = Some(d);
    }
    if mri_dim_mismatch(v.a(), v.dref(), false) != 0 {
        let _ = writeln!(out, "ERROR: MRIreplaceList(): seg/out dim mismatch");
        return Ok(v.null());
    }
    if v.dref().mtype == MRI_UCHAR && targlist[..nlist].iter().any(|&t| t > 255) {
        let _ = writeln!(out, "ERROR: MRIreplaceList(): output is UCHAR, but target is > 255");
        return Ok(v.null());
    }
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let segid = seg_label(g, err, WHAT, v.a(), c, r, s)?;
                if let Some(m) = mask
                    && mri_get_vox_val(g, err, m, c, r, s, 0, 0)? < 0.5
                {
                    setv(g, err, &mut v, c, r, s, 0, segid as f32)?;
                    continue;
                }
                let val = match srclist[..nlist].iter().position(|&x| x == segid) {
                    Some(n) => targlist[n],
                    None => segid,
                };
                setv(g, err, &mut v, c, r, s, 0, val as f32)?;
            }
        }
    }
    Ok(v.done())
}

/// `MRIreplaceValuesOnly(mri_src, mri_dst, in_val, out_val)`: `MRIcheckVolDims`; without a
/// destination a clone (zeros). Two byte volumes go to `MRIreplaceValuesUchar` with `in_val` and
/// `out_val` rounded (`nint`) to bytes; otherwise each voxel of each frame `FEQUAL` to `in_val`
/// (the `float` difference below `FLT_EPSILON`) is written as `out_val`, the others left as they
/// are in the destination.
pub fn mri_replace_values_only(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    in_val: f32,
    out_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    if v.has_dst() {
        check_dims(g, err, v.a(), v.dref())?;
    }
    clone_into(g, out, err, &mut v)?;
    if v.a().mtype == MRI_UCHAR && v.dref().mtype == MRI_UCHAR {
        let iv = nint(g, f64::from(in_val)) as u8;
        let ov = nint(g, f64::from(out_val)) as u8;
        return v.with_pair(|pp| mri_replace_values_uchar(g, out, err, pp, iv, ov));
    }
    let (w, h, d, nf) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth, v.a().nframes);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                for f in 0..nf {
                    let val = getv(g, err, &v, true, x, y, z, f)?;
                    if fequal(val, in_val) {
                        setv(g, err, &mut v, x, y, z, f, out_val)?;
                    }
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRImaskDifferentGeometry(mri_src, mri_mask, mri_dst, mask, out_val)`: each voxel of the
/// source (x outermost) is mapped into the mask by `MRIgetVoxelToVoxelXform(src, mask)` (a
/// `float` product, rounded with `nint`); where the mask's voxel there (frame 0, rounded) is
/// `mask`, or the point is outside the mask (as `mask`), every frame of the destination gets
/// `out_val`, else the source's voxel. Without a destination a clone of the source.
pub fn mri_mask_different_geometry(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mri_mask: &Mri,
    mask: i32,
    out_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    let m = mri_get_voxel_to_voxel_xform(g, out, err, v.a(), mri_mask)?;
    clone_into(g, out, err, &mut v)?;
    let mut v1 = Matrix::real(4, 1);
    let mut v2 = Matrix::real(4, 1);
    v1.set_relt(4, 1, 1.0);
    v2.set_relt(4, 1, 1.0);
    let (w, h, d, nf) = (
        v.dref().vg.width,
        v.dref().vg.height,
        v.dref().vg.depth,
        v.dref().nframes,
    );
    for x in 0..w {
        v1.set_relt(1, 1, x as f32);
        for y in 0..h {
            v1.set_relt(2, 1, y as f32);
            for z in 0..d {
                v1.set_relt(3, 1, z as f32);
                matrix_multiply(g, out, err, m.as_ref(), Some(&v1), Some(&mut v2))?;
                let xd = nint(g, f64::from(v2.relt(1, 1)));
                let yd = nint(g, f64::from(v2.relt(2, 1)));
                let zd = nint(g, f64::from(v2.relt(3, 1)));
                let (mw, mh, md) = (mri_mask.vg.width, mri_mask.vg.height, mri_mask.vg.depth);
                let mask_val = if xd < 0 || xd >= mw || yd < 0 || yd >= mh || zd < 0 || zd >= md {
                    mask
                } else {
                    let mv = mri_get_vox_val(g, err, mri_mask, xd, yd, zd, 0, 0)?;
                    nint(g, f64::from(mv))
                };
                for f in 0..nf {
                    let val = if mask_val == mask {
                        out_val
                    } else {
                        getv(g, err, &v, true, x, y, z, f)?
                    };
                    setv(g, err, &mut v, x, y, z, f, val)?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRImask(mri_src, mri_mask, mri_dst, mask, out_val)`: the environment's
/// `FS_MRIMASK_ALLOW_DIFF_GEOM` (`sscanf`'s `%d`; 1 without it; `MRImask(): AllowDiffGeom = <n>`
/// on standard output when `Gdiag` is above 0). When the two geometries differ
/// (`vg_isNotEqualThresh` at 1e-4): `INFO: MRImask() using MRImaskDifferentGeometry()` and
/// [`mri_mask_different_geometry`], or with it 0 `INFO: MRImask(): geometries differ but not using
/// MRImaskDifferentGeometry() ` and on. Then `MRIcheckVolDims(src, mask)`, a clone without a
/// destination, a destination of another type: `MRImask: src and dst must be same type`
/// (`ERROR_UNSUPPORTED`, `NULL`); each voxel where the mask (frame 0, truncated to an `int`,
/// FS-12) is `mask` gets `out_val` in every frame, the others the source's voxel.
#[allow(clippy::too_many_arguments)]
pub fn mri_mask(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    p: Pair,
    mri_mask: &Mri,
    mask: i32,
    out_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRImask";
    let mut allow = 1;
    if let Some(a) = env.get("FS_MRIMASK_ALLOW_DIFF_GEOM") {
        let mut st = cscan::Bytes::new(a.as_encoded_bytes());
        if let Some(n) = cscan::Call::new().i32(&mut st, &mut g.errno) {
            allow = n;
        }
    }
    if g.gdiag > 0 {
        let _ = writeln!(out, "MRImask(): AllowDiffGeom = {allow}");
    }
    let src_vg = match &p {
        Pair::New(s) | Pair::Given(s, _) => &s.vg,
        Pair::Same(m) => &m.vg,
    };
    let mut vg_src = VolGeom::default();
    let mut vg_mask = VolGeom::default();
    get_vol_geom(src_vg, &mut vg_src);
    get_vol_geom(&mri_mask.vg, &mut vg_mask);
    if vg_is_not_equal_thresh(&vg_src, &vg_mask, 1e-4) != 0 {
        if allow != 0 {
            let _ = writeln!(out, "INFO: MRImask() using MRImaskDifferentGeometry()");
            return mri_mask_different_geometry(g, out, err, p, mri_mask, mask, out_val);
        }
        let _ = writeln!(
            out,
            "INFO: MRImask(): geometries differ but not using MRImaskDifferentGeometry() "
        );
        let _ = out.flush();
    }
    let (mut v, _) = Vols::of_pair(p);
    check_dims(g, err, v.a(), mri_mask)?;
    clone_into(g, out, err, &mut v)?;
    if v.a().mtype != v.dref().mtype {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "MRImask: src and dst must be same type",
        );
        return Ok(v.null());
    }
    let (w, h, d, nf) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth, v.a().nframes);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let mv = mri_get_vox_val(g, err, mri_mask, x, y, z, 0, 0)?;
                let mask_val = int_of(g, WHAT, f64::from(mv));
                for f in 0..nf {
                    let val = if mask_val == mask {
                        out_val
                    } else {
                        getv(g, err, &v, true, x, y, z, f)?
                    };
                    setv(g, err, &mut v, x, y, z, f, val)?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRInMask(mask)`: the number of voxels of every frame above 0.5 (`double`).
pub fn mri_n_mask(g: &mut Globals, err: &mut dyn Write, mask: &Mri) -> Result<i32, Fatal> {
    let mut n = 0;
    for c in 0..mask.vg.width {
        for r in 0..mask.vg.height {
            for s in 0..mask.vg.depth {
                for f in 0..mask.nframes {
                    if f64::from(mri_get_vox_val(g, err, mask, c, r, s, f, 0)?) > 0.5 {
                        n += 1;
                    }
                }
            }
        }
    }
    Ok(n)
}

/// C's conversion of a `float` to a byte through a 32-bit integer (x86-64's `cvttss2si`, the low
/// byte): FS-494 beyond 0 to 255 or for NaN (clamped by default, NaN to 0).
fn byte_of(g: &mut Globals, what: &str, x: f32) -> u8 {
    if (0.0..256.0).contains(&x) {
        return x as u8;
    }
    g.notices
        .add(&FS_494, format!("{what}: the value {x} stored as a byte"));
    if g.notices.reproduce() {
        let i = if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
            i32::MIN
        } else {
            x as i32
        };
        return i as u8;
    }
    if x.is_nan() || x < 0.0 { 0 } else { 255 }
}

/// `MRIthresholdMask(mri_src, mri_mask, mri_dst, mask_threshold, out_val)`:
/// `MRIcheckVolDims(src, mask)`, a clone without a destination. Two byte volumes: frame 0's bytes
/// (FreeSurfer's byte pointers), the source's byte where the mask's byte is at least the threshold
/// (`float` comparison), else `out_val` as a byte (FS-494). Otherwise frame 0 as `float`s
/// (`MRIgetVoxVal`), `out_val` where the mask is below the threshold, written with
/// `MRIsetVoxVal`.
pub fn mri_threshold_mask(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mri_mask: &Mri,
    mask_threshold: f32,
    out_val: f32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIthresholdMask";
    let (mut v, _) = Vols::of_pair(p);
    check_dims(g, err, v.a(), mri_mask)?;
    clone_into(g, out, err, &mut v)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    if v.a().mtype == MRI_UCHAR && mri_mask.mtype == MRI_UCHAR {
        for m in [v.a(), mri_mask, v.dref()] {
            need_voxels(g, err, WHAT, m)?;
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let val = byte(g, WHAT, v.a(), x, y, z);
                    let mval = byte(g, WHAT, mri_mask, x, y, z);
                    let o = if f32::from(mval) >= mask_threshold {
                        val
                    } else {
                        byte_of(g, WHAT, out_val)
                    };
                    put_byte(g, WHAT, v.d(), x, y, z, o);
                }
            }
        }
    } else {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mut fval = getv(g, err, &v, true, x, y, z, 0)?;
                    let fmask = mri_get_vox_val(g, err, mri_mask, x, y, z, 0, 0)?;
                    if fmask < mask_threshold {
                        fval = out_val;
                    }
                    setv(g, err, &mut v, x, y, z, 0, fval)?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRIgrowLabel(mri, mri_filled, in_label, out_label)`: on frame 0's bytes. For `in_label` not
/// 0 the box of its voxels; for 0 the box of the voxels not 0, every voxel on or outside the box's
/// faces filled with `out_label` first. Then passes over the box until one fills nothing: each
/// neighbour (of 27, in the volume) of a filled voxel that is `in_label` in `mri` and not yet
/// filled is filled; after each pass `<n> voxels filled, total = <t>.` on standard error.
/// `MRIcheckVolDims(mri, mri_filled)`. `NO_ERROR`.
pub fn mri_grow_label(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    filled: &mut Mri,
    in_label: i32,
    out_label: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIgrowLabel";
    check_dims(g, err, mri, filled)?;
    need_voxels(g, err, WHAT, mri)?;
    need_voxels(g, err, WHAT, filled)?;
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut total = 0;
    let (mut xmin, mut ymin, mut zmin, mut xmax, mut ymax, mut zmax) = (w, h, d, 0, 0, 0);
    let mut see = |x: i32, y: i32, z: i32| {
        xmax = xmax.max(x);
        ymax = ymax.max(y);
        zmax = zmax.max(z);
        xmin = xmin.min(x);
        ymin = ymin.min(y);
        zmin = zmin.min(z);
    };
    let ob = out_label as u8;
    if in_label != 0 {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if i32::from(byte(g, WHAT, mri, x, y, z)) == in_label {
                        see(x, y, z);
                    }
                }
            }
        }
    } else {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if byte(g, WHAT, mri, x, y, z) != 0 {
                        see(x, y, z);
                    }
                }
            }
        }
    }
    if in_label == 0 {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if z <= zmin || z >= zmax || y <= ymin || y >= ymax || x <= xmin || x >= xmax {
                        total += 1;
                        put_byte(g, WHAT, filled, x, y, z, ob);
                    }
                }
            }
        }
    }
    loop {
        let mut nfilled = 0;
        for z in zmin..=zmax {
            for y in ymin..=ymax {
                for x in xmin..=xmax {
                    if i32::from(byte(g, WHAT, filled, x, y, z)) != out_label {
                        continue;
                    }
                    for zk in -1..=1 {
                        let zi = z + zk;
                        if zi < 0 || zi >= d {
                            continue;
                        }
                        for yk in -1..=1 {
                            let yi = y + yk;
                            if yi < 0 || yi >= h {
                                continue;
                            }
                            for xk in -1..=1 {
                                let xi = x + xk;
                                if xi < 0 || xi >= w {
                                    continue;
                                }
                                if i32::from(byte(g, WHAT, mri, xi, yi, zi)) == in_label
                                    && i32::from(byte(g, WHAT, filled, xi, yi, zi)) != out_label
                                {
                                    nfilled += 1;
                                    put_byte(g, WHAT, filled, xi, yi, zi, ob);
                                }
                            }
                        }
                    }
                }
            }
        }
        total += nfilled;
        let _ = writeln!(err, "{nfilled} voxels filled, total = {total}.");
        if nfilled <= 0 {
            break;
        }
    }
    Ok(NO_ERROR)
}

/// `MRIturnOnFG(mri, mri_fg, mri_bg)`: on frame 0's bytes; each voxel of `mri` where the
/// foreground is above 0 takes the foreground's byte; then each voxel where the foreground is 0
/// and `mri` is above 0 becomes 0 in `mri` and 1 in the background. `MRIcheckVolDims(mri, fg)`.
pub fn mri_turn_on_fg(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    fg: &Mri,
    bg: &mut Mri,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIturnOnFG";
    check_dims(g, err, mri, fg)?;
    for m in [&*mri, fg, &*bg] {
        need_voxels(g, err, WHAT, m)?;
    }
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let f = byte(g, WHAT, fg, x, y, z);
                if f > 0 {
                    put_byte(g, WHAT, mri, x, y, z, f);
                }
            }
        }
    }
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if byte(g, WHAT, fg, x, y, z) == 0 && byte(g, WHAT, mri, x, y, z) > 0 {
                    put_byte(g, WHAT, mri, x, y, z, 0);
                    put_byte(g, WHAT, bg, x, y, z, 1);
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `findLabel(mri, x, y, z)`: the most frequent byte above `WM_MIN_VAL` (the lowest of a tie) in
/// the cube of half-width 1 around the voxel (in the volume), the half-width grown to 4 while
/// none is found; 0 when none.
fn find_label(g: &mut Globals, m: &Mri, x: i32, y: i32, z: i32) -> u8 {
    const WHAT: &str = "MRIturnOffBG";
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let mut whalf = 1;
    loop {
        let mut counts = [0i32; 256];
        for zk in -whalf..=whalf {
            let zi = z + zk;
            if zi < 0 || zi >= d {
                continue;
            }
            for yk in -whalf..=whalf {
                let yi = y + yk;
                if yi < 0 || yi >= h {
                    continue;
                }
                for xk in -whalf..=whalf {
                    let xi = x + xk;
                    if xi < 0 || xi >= w {
                        continue;
                    }
                    let label = byte(g, WHAT, m, xi, yi, zi);
                    if i32::from(label) <= WM_MIN_VAL {
                        continue;
                    }
                    counts[usize::from(label)] += 1;
                }
            }
        }
        let (mut max_count, mut max_label) = (0, 0usize);
        for (label, &c) in counts.iter().enumerate() {
            if c > max_count {
                max_count = c;
                max_label = label;
            }
        }
        whalf += 1;
        if !(max_count == 0 && whalf < 5) {
            return max_label as u8;
        }
    }
}

/// `MRIturnOffBG(mri, mri_bg)`: on frame 0's bytes; each voxel where the background is above 0
/// becomes 0; then each voxel 0 in both takes [`find_label`]'s label (voxels already relabelled
/// count for the later ones). `MRIcheckVolDims(mri, bg)`. `NO_ERROR`.
pub fn mri_turn_off_bg(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    bg: &Mri,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIturnOffBG";
    check_dims(g, err, mri, bg)?;
    need_voxels(g, err, WHAT, mri)?;
    need_voxels(g, err, WHAT, bg)?;
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if byte(g, WHAT, bg, x, y, z) > 0 {
                    put_byte(g, WHAT, mri, x, y, z, 0);
                }
            }
        }
    }
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if byte(g, WHAT, bg, x, y, z) == 0 && byte(g, WHAT, mri, x, y, z) == 0 {
                    let l = find_label(g, mri, x, y, z);
                    put_byte(g, WHAT, mri, x, y, z, l);
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRIcopyLabel(mri_src, mri_dst, label)`: frame 0; each voxel of the source that is `label`
/// set to `label` in the destination; the number of them. Two byte volumes through the byte
/// pointers (`label` stored as a byte); otherwise `MRIgetVoxVal(src) == label` with `label` as a
/// `float`, written with `MRIsetVoxVal`. `MRIcheckVolDims`.
pub fn mri_copy_label(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    label: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIcopyLabel";
    check_dims(g, err, src, dst)?;
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut nvox = 0;
    if src.mtype == MRI_UCHAR && dst.mtype == MRI_UCHAR {
        need_voxels(g, err, WHAT, src)?;
        need_voxels(g, err, WHAT, dst)?;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if i32::from(byte(g, WHAT, src, x, y, z)) == label {
                        put_byte(g, WHAT, dst, x, y, z, label as u8);
                        nvox += 1;
                    }
                }
            }
        }
    } else {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if mri_get_vox_val(g, err, src, x, y, z, 0, 0)? == label as f32 {
                        mri_set_vox_val(g, err, dst, x, y, z, 0, label as f32, 0)?;
                        nvox += 1;
                    }
                }
            }
        }
    }
    Ok(nvox)
}

/// `MRItotalVoxelsOn(mri, thresh)`: the voxels of every frame at least `thresh` (as a `float`).
pub fn mri_total_voxels_on(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    thresh: i32,
) -> Result<i32, Fatal> {
    let mut n = 0;
    for f in 0..mri.nframes {
        for z in 0..mri.vg.depth {
            for y in 0..mri.vg.height {
                for x in 0..mri.vg.width {
                    if mri_get_vox_val(g, err, mri, x, y, z, f, 0)? >= thresh as f32 {
                        n += 1;
                    }
                }
            }
        }
    }
    Ok(n)
}

/// `MRIvoxelsInLabel(mri, label)`: the voxels of frame 0 whose value rounds (`nint`) to `label`.
pub fn mri_voxels_in_label(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    label: i32,
) -> Result<i32, Fatal> {
    let mut n = 0;
    for z in 0..mri.vg.depth {
        for y in 0..mri.vg.height {
            for x in 0..mri.vg.width {
                let v = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                if nint(g, f64::from(v)) == label {
                    n += 1;
                }
            }
        }
    }
    Ok(n)
}

/// `MRInot(mri_src, mri_dst)`: frame 0 (x outermost); 1 where the voxel rounds (`nint`) to 0,
/// else 0. `MRIcheckVolDims`; a clone without a destination.
pub fn mri_not(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    if v.has_dst() {
        check_dims(g, err, v.a(), v.dref())?;
    }
    clone_into(g, out, err, &mut v)?;
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let val = getv(g, err, &v, true, x, y, z, 0)?;
                let o = nint(g, f64::from(val)) == 0;
                setv(g, err, &mut v, x, y, z, 0, f32::from(u8::from(o)))?;
            }
        }
    }
    Ok(v.done())
}

/// `MRIcopyLabeledVoxels(mri_src, mri_labeled, mri_dst, label)`: frame 0; where the labelled
/// volume is `label` (as a `float`), the source's voxel written into the destination; the number
/// of them. `MRIcheckVolDims(src, labeled)`.
pub fn mri_copy_labeled_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    dst: &mut Mri,
    label: i32,
) -> Result<i32, Fatal> {
    check_dims(g, err, src, labeled)?;
    let mut n = 0;
    for z in 0..src.vg.depth {
        for y in 0..src.vg.height {
            for x in 0..src.vg.width {
                if mri_get_vox_val(g, err, labeled, x, y, z, 0, 0)? == label as f32 {
                    let val = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                    mri_set_vox_val(g, err, dst, x, y, z, 0, val, 0)?;
                    n += 1;
                }
            }
        }
    }
    Ok(n)
}

/// `MRIsetLabelValues(mri_src, mri_label, mri_dst, label, val)`: `MRIcheckVolDims(src, label)`,
/// the destination a copy of the source (`MRIcopy`); frame 0's voxels (x outermost) where the
/// label volume (truncated to an `int`, FS-12) is `label` set to `val`.
pub fn mri_set_label_values(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mri_label: &Mri,
    label: i32,
    val: f32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIsetLabelValues";
    let (mut v, _) = Vols::of_pair(p);
    check_dims(g, err, v.a(), mri_label)?;
    v.copy_a(g, out, err)?;
    let (w, h, d) = (v.dref().vg.width, v.dref().vg.height, v.dref().vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if seg_label(g, err, WHAT, mri_label, x, y, z)? == label {
                    setv(g, err, &mut v, x, y, z, 0, val)?;
                }
            }
        }
    }
    Ok(v.done())
}

/// `MRIlabelInVolume(mri_src, label)`: 1 when a voxel of frame 0 (x outermost; truncated to an
/// `int`, FS-12) is `label`, else 0.
pub fn mri_label_in_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    label: i32,
) -> Result<i32, Fatal> {
    for x in 0..src.vg.width {
        for y in 0..src.vg.height {
            for z in 0..src.vg.depth {
                if seg_label(g, err, "MRIlabelInVolume", src, x, y, z)? == label {
                    return Ok(1);
                }
            }
        }
    }
    Ok(0)
}

/// `MRIcomputeCentroid(mri)`: the centroid of frame 0 weighted by the voxels' values (x
/// outermost; voxels whose magnitude is below `FLT_EPSILON` skipped; sums in `double`), divided
/// by the sum of weights when it is above 0. `NO_ERROR` and `(xc, yc, zc)`.
pub fn mri_compute_centroid(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
) -> Result<(i32, [f64; 3]), Fatal> {
    let (mut xc, mut yc, mut zc, mut norm) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for x in 0..mri.vg.width {
        for y in 0..mri.vg.height {
            for z in 0..mri.vg.depth {
                let val = f64::from(mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?);
                if val.abs() < f64::from(f32::EPSILON) {
                    continue;
                }
                norm = add64(norm, val);
                xc = add64(mul64(f64::from(x), val), xc);
                yc = add64(mul64(f64::from(y), val), yc);
                zc = add64(mul64(f64::from(z), val), zc);
            }
        }
    }
    if norm > 0.0 {
        xc = div64(xc, norm);
        yc = div64(yc, norm);
        zc = div64(zc, norm);
    }
    Ok((NO_ERROR, [xc, yc, zc]))
}

/// `MRIbinMaskToCol(binmask, bincol)`: frame 0; each voxel's column where the mask is at least
/// 0.5, else 0. Without `bincol`, `MRIcloneBySpace(binmask, MRI_INT, -1)`; one of other
/// dimensions (frames included): `ERROR: MRIbinMaskToCol: input/output dim mismatch` on standard
/// output and `NULL`.
pub fn mri_bin_mask_to_col(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    let (mut v, _) = Vols::of_pair(p);
    if v.has_dst() {
        if mri_dim_mismatch(v.a(), v.dref(), true) != 0 {
            let _ = writeln!(out, "ERROR: MRIbinMaskToCol: input/output dim mismatch");
            return Ok(v.null());
        }
    } else {
        let m = mri_clone_by_space(g, out, err, v.a(), MRI_INT, -1)?;
        v.fresh = Some(m);
    }
    let (w, h, d) = (v.a().vg.width, v.a().vg.height, v.a().vg.depth);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let m = getv(g, err, &v, true, c, r, s, 0)?;
                let val = if m < 0.5 { 0 } else { c };
                setv(g, err, &mut v, c, r, s, 0, val as f32)?;
            }
        }
    }
    Ok(v.done())
}

/// `MRIpercentThresh(mri, mask, frame, pct)`: the voxels of `frame` (x outermost; where the mask
/// is not below 0.5) sorted ascending (the C library's `qsort` with `CompareDoubles`), and the
/// one at `round(n (100 - pct) / 100)` (`double`, truncated to an `int`). FS-495 and FS-496 for
/// an index at or past the list's end or outside it: by default the largest value (0 for an
/// empty list); with `reproduce` FreeSurfer's 0 from the list's unused part, and 0 (noted) where
/// FreeSurfer reads memory it does not own.
pub fn mri_percent_thresh(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    mask: Option<&Mri>,
    frame: i32,
    pct: f64,
) -> Result<f64, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let ntot = i64::from(w) * i64::from(d) * i64::from(h);
    let mut vlist = Vec::new();
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                if let Some(m) = mask
                    && mri_get_vox_val(g, err, m, c, r, s, 0, 0)? < 0.5
                {
                    continue;
                }
                vlist.push(f64::from(mri_get_vox_val(g, err, mri, c, r, s, frame, 0)?));
            }
        }
    }
    qsort(&mut vlist, compare_doubles);
    let nlist = vlist.len() as i64;
    let x = c_round(div64(mul64(nlist as f64, sub64(100.0, pct)), 100.0));
    let npct = cvtt(x);
    if npct >= 0 && i64::from(npct) < nlist {
        return Ok(vlist[npct as usize]);
    }
    let inside = npct >= 0 && i64::from(npct) < ntot;
    let def = if inside { &FS_495 } else { &FS_496 };
    g.notices.add(
        def,
        format!("MRIpercentThresh: the index {npct} of a list of {nlist} values ({ntot} allocated)"),
    );
    if g.notices.reproduce() {
        return Ok(0.0);
    }
    Ok(vlist.last().copied().unwrap_or(0.0))
}

/// C's `round(x)` (glibc: halves away from zero; a NaN passed on quieted).
fn c_round(x: f64) -> f64 {
    if x.is_nan() {
        f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
    } else {
        x.round()
    }
}

/// `MRIcountValInNbhd(mri, wsize, x, y, z, val)`: the voxels of frame 0 in the cube of half-width
/// `(wsize - 1) / 2` around `(x, y, z)` (the index tables clamp it; FS-323 past them) whose value
/// truncated to an `int` (FS-12) is `val`.
#[allow(clippy::too_many_arguments)]
pub fn mri_count_val_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
    val: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIcountValInNbhd";
    let whalf = (wsize - 1) / 2;
    let mut total = 0;
    for zk in -whalf..=whalf {
        let zi = index(g, err, WHAT, mri, Axis::Z, z + zk)?;
        for yk in -whalf..=whalf {
            let yi = index(g, err, WHAT, mri, Axis::Y, y + yk)?;
            for xk in -whalf..=whalf {
                let xi = index(g, err, WHAT, mri, Axis::X, x + xk)?;
                if seg_label(g, err, WHAT, mri, xi, yi, zi)? == val {
                    total += 1;
                }
            }
        }
    }
    Ok(total)
}
