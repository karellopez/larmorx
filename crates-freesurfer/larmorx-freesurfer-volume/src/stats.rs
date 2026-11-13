// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A volume's statistics and small header helpers (`utils/mri.cpp`, 3.1-37): `MRIstats`,
//! `MRIprintStats`, `MRItype2str`, `MRIcropBoundingBox` and `MRIcalcCRASforSampledVolume`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §12):
//! - **FS-329**: the statistics of a volume of 64-bit integers read 32-bit integers at the voxels'
//!   indices (halves of the first voxels); the default reads the voxels.
//! - **FS-330** (memory): the printed statistics of a volume without voxels, or of a type the
//!   statistics do not read, are values never written; 0 in both modes, noted.
//! - **FS-462** (crash): the centre of a volume sampled from one wider than it by a whole factor
//!   divides by the sampled volume's extent: an extent of 0 ends the program; an error in both
//!   modes.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_329, FS_330, FS_462};
use larmorx_freesurfer_core::affine::{AffineMatrix, set_affine_matrix};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, div, div64, mul, sqrt64, sub};
use larmorx_freesurfer_mri::mri::{
    MRI_BITMAP, MRI_DOUBLE_COMPLEX, MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_LONG, MRI_RGB,
    MRI_SHORT, MRI_TENSOR, MRI_UCHAR, MRI_USHRT, Mri, MriRegion,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i};

use crate::coords::center_of_mass;
use crate::rows;

/// `MRIstats`' outputs, each written only where FreeSurfer writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub min: f32,
    pub max: f32,
    pub n_voxels: i32,
    pub mean: f32,
    pub std: f32,
}

/// `MRIstats(mri, &min, &max, &n_voxels, &mean, &std)`: nothing written for a volume without
/// voxels. Otherwise `n_voxels` = width x height x depth x frames (32-bit, wrapping) and, for the
/// six types the function reads (bytes, shorts, unsigned shorts, 32-bit and 64-bit integers,
/// floats), `min` and `max` from the first voxel, then every voxel (x outermost, then y, z, the
/// frame innermost) compared (`<`, `>`: NaNs passed over) and summed with its square in `float`;
/// `mean = sum / n` and `std = sqrt((sq_sum - n mean mean) / (n - 1.0))` (`n` the count as a
/// `float`, the quotient in `double` rounded to `float`, its square root in `double` rounded to
/// `float`). Another type: `min` and `max` not written, the sums 0. `NO_ERROR`.
pub fn mri_stats(g: &mut Globals, mri: &Mri, s: &mut Stats) -> i32 {
    const WHAT: &str = "MRIstats";
    if mri.chunk.is_none() {
        return NO_ERROR;
    }
    let (w, h, d, nf) = (mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes);
    s.n_voxels = w.wrapping_mul(h).wrapping_mul(d).wrapping_mul(nf);
    let n = s.n_voxels as f32;
    let (mut sum, mut sq_sum) = (0.0f32, 0.0f32);
    let ty = match mri.mtype {
        MRI_LONG => {
            if i64::from(w) * i64::from(h) * i64::from(d) * i64::from(nf) > 0 {
                g.notices.add(
                    &FS_329,
                    "the statistics of a volume of 64-bit integers read 32-bit integers at the voxels' indices",
                );
            }
            if g.notices.reproduce() {
                Some(MRI_INT)
            } else {
                Some(MRI_LONG)
            }
        }
        t @ (MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT) => Some(t),
        _ => None,
    };
    if let Some(ty) = ty {
        let first = rows::read(g, WHAT, mri, 0, 0, 0, ty).f();
        (s.min, s.max) = (first, first);
        for i in 0..w {
            for j in 0..h {
                for k in 0..d {
                    for t in 0..nf {
                        let sl = i64::from(k) + i64::from(t) * i64::from(d);
                        let val = rows::read(g, WHAT, mri, sl, i64::from(j), i64::from(i), ty).f();
                        if val < s.min {
                            s.min = val;
                        }
                        if val > s.max {
                            s.max = val;
                        }
                        sum = add(sum, val);
                        sq_sum = add(sq_sum, mul(val, val));
                    }
                }
            }
        }
    }
    s.mean = div(sum, n);
    let var = sub(sq_sum, mul(mul(n, s.mean), s.mean));
    s.std = div64(f64::from(var), f64::from(n) - 1.0) as f32;
    s.std = sqrt64(f64::from(s.std)) as f32;
    NO_ERROR
}

/// `MRIprintStats(mri, stream)`: [`mri_stats`], then on `stream` the lines `<n> values`,
/// `min = <g>`, `max = <g>`, `mean = <g>`, `std = <g>` (`%g`) and, from `MRIcenterOfMass` at
/// threshold 0, `com = <g> <g> <g>`; `NO_ERROR`. FS-330: what [`mri_stats`] does not write is 0.
pub fn mri_print_stats(
    g: &mut Globals,
    stream: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
) -> Result<i32, Fatal> {
    let mut s = Stats::default();
    mri_stats(g, mri, &mut s);
    let read = matches!(
        mri.mtype,
        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG | MRI_FLOAT
    );
    if mri.chunk.is_none() || !read {
        g.notices.add(
            &FS_330,
            "the printed statistics of a volume without voxels or of a type they do not read are values never written (0 here)",
        );
    }
    let gf = |v: f32| cfmt::g(f64::from(v), 6);
    let _ = write!(
        stream,
        "{} values\nmin = {}\nmax = {}\nmean = {}\nstd = {}\n",
        s.n_voxels,
        gf(s.min),
        gf(s.max),
        gf(s.mean),
        gf(s.std)
    );
    let (_, com) = center_of_mass(g, err, mri, 0)?;
    let _ = writeln!(
        stream,
        "com = {} {} {}",
        cfmt::g(com[0], 6),
        cfmt::g(com[1], 6),
        cfmt::g(com[2], 6)
    );
    Ok(NO_ERROR)
}

/// `MRItype2str(type)`: `uchar`, `short`, `ushrt`, `int`, `long`, `float`, `bitmap`, `tensor`,
/// `RGB`, or `unknown data type` (the complex types included).
pub fn mri_type2str(ty: i32) -> &'static str {
    match ty {
        MRI_UCHAR => "uchar",
        MRI_SHORT => "short",
        MRI_USHRT => "ushrt",
        MRI_INT => "int",
        MRI_LONG => "long",
        MRI_FLOAT => "float",
        MRI_BITMAP => "bitmap",
        MRI_TENSOR => "tensor",
        MRI_RGB => "RGB",
        MRI_FLOAT_COMPLEX | MRI_DOUBLE_COMPLEX => "unknown data type",
        _ => "unknown data type",
    }
}

/// `MRIcropBoundingBox(mri, box)`: the box's start raised to 0 and its extent lowered to what is
/// left of the volume from its start (32-bit arithmetic); `NO_ERROR`.
pub fn mri_crop_bounding_box(mri: &Mri, b: &mut MriRegion) -> i32 {
    b.x = b.x.max(0);
    b.y = b.y.max(0);
    b.z = b.z.max(0);
    b.dx = mri.vg.width.wrapping_sub(b.x).min(b.dx);
    b.dy = mri.vg.height.wrapping_sub(b.y).min(b.dy);
    b.dz = mri.vg.depth.wrapping_sub(b.z).min(b.dz);
    NO_ERROR
}

/// `MRIcalcCRASforSampledVolume(src, dst, &r, &a, &s)`: the sampling factors `src.width /
/// dst.width` (and height, depth; C's integer division), the destination's centre voxel
/// `dst.width / 2` (integer), its place in the source `centre * factor + (factor - 1.0) / 2.0`
/// (in `double`, rounded to `float`), and that place's scanner RAS through the source's cached
/// voxel-to-RAS matrix (as `float`s widened to `double`); a source without cached matrices gets
/// both first. With `Gdiag & DIAG_SHOW` and `DIAG_VERBOSE_ON`, `c_ras for sample volume is (...)
/// compared with the src (...)` on standard error. FS-462: a destination extent of 0 (or a
/// division that overflows) ends FreeSurfer; an error here.
pub fn mri_calc_cras_for_sampled_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: &Mri,
) -> Result<(f64, f64, f64), Fatal> {
    let pairs = [
        (src.vg.width, dst.vg.width),
        (src.vg.height, dst.vg.height),
        (src.vg.depth, dst.vg.depth),
    ];
    let mut sample = [0i32; 3];
    for (k, (a, b)) in pairs.into_iter().enumerate() {
        match a.checked_div(b) {
            Some(q) => sample[k] = q,
            None => {
                g.notices.add(
                    &FS_462,
                    "the centre of a sampled volume divides by its extent of 0 (FreeSurfer ends here)",
                );
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    "MRIcalcCRASforSampledVolume(): an extent of the sampled volume is 0 (FreeSurfer crashes here)\n",
                )?;
                return Ok((0.0, 0.0, 0.0));
            }
        }
    }
    let centre = [dst.vg.width / 2, dst.vg.height / 2, dst.vg.depth / 2];
    let at = |k: usize| {
        f64::from(centre[k].wrapping_mul(sample[k])) + (f64::from(sample[k]) - 1.0) / 2.0
    };
    let (sx, sy, sz) = (at(0), at(1), at(2));
    if src.vg.i_to_r.is_none() {
        let tmp = extract_i_to_r(g, out, err, &src.vg)?;
        let mut am = AffineMatrix::default();
        set_affine_matrix(err, &mut am, &tmp)?;
        src.vg.i_to_r = Some(am);
        src.vg.r_to_i = extract_r_to_i(g, out, err, &src.vg)?;
    }
    // AffineMV, inlined as in MRIcalcCRASforExtractedVolume
    let (x, y, z) = (sx as f32, sy as f32, sz as f32);
    let m = src.vg.i_to_r.as_ref().expect("cached").mat;
    let lane = |i: usize| {
        add(
            add(add(mul(x, m[i]), mul(y, m[4 + i])), mul(z, m[8 + i])),
            m[12 + i],
        )
    };
    let (pr, pa, ps) = (
        f64::from(lane(0)),
        f64::from(lane(1)),
        f64::from(lane(2)),
    );
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let f = |x: f64| cfmt::f(x, 6);
        let _ = writeln!(
            err,
            "c_ras for sample volume is ({}, {}, {}) compared with the src ({}, {}, {})",
            f(pr),
            f(pa),
            f(ps),
            f(f64::from(src.vg.c_r)),
            f(f64::from(src.vg.c_a)),
            f(f64::from(src.vg.c_s))
        );
    }
    Ok((pr, pa, ps))
}
