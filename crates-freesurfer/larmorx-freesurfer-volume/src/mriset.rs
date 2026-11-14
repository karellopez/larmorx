// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Morphology of `utils/mriset.cpp` (3.1-38): `MRIdilate` and `MRIdilateUchar` (taken first:
//! fs.filter needs them). (FreeSurfer runs `MRIdilate`'s slices in parallel; each voxel is its
//! own, so the order does not matter.)

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use crate::arith::{Pair, check_dims, mri_clone};
use crate::deriv::{Axis, index};
use crate::rows::{self, Num};
use crate::sample::{SAMPLE_NEAREST, sample_volume_frame_type};

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
