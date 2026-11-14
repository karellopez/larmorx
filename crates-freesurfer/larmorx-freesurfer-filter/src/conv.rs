// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrifilter.cpp`'s convolutions and window statistics (queue item 3.1-42): the Gaussian
//! kernel `MRIgaussian1d` (2368), the one-dimensional convolutions `MRIconvolve1d` (3006) with
//! `MRIconvolve1dByte` (3321), `MRIconvolve1dShort` (3464) and `MRIconvolve1dInt` (3550), the
//! separable `MRIconvolveGaussian` (2564), the matrix smoothers `MRIgaussianSmoothNI` (6517, item
//! 3.1-43's line), `MRIgaussianSmooth` (2643) and `MRImaskedGaussianSmooth` (2665), the window
//! statistics `MRImean` (2034), `MRIstd` (2429) and `MRIstdNonzero` (2496), and the pyramid
//! reductions `MRIreduce` (2891), `MRIreduce2D` (2943), `MRIreduce1d` (3722) and `MRIreduceSlice`
//! (3852) with `MRImodifySampledHeader` (2743).
//!
//! The sums are the official build's: single precision, each tap `total + k * v` in the source's
//! order ([`sse`] for the NaNs' bits), the integer outputs rounded by `nint`. `mrifilter.cpp`'s
//! `FZERO` is `fabs(f) < FLT_EPSILON` with `<float.h>`'s epsilon (its preprocessed source).
//! `MRIgaussianSmoothNI`'s kernel sums are `long double` (x87), added in the order of one thread.
//! FS-545 to FS-553.

use std::io::Write;

use larmorx_core::defects::{
    FS_545, FS_546, FS_547, FS_548, FS_549, FS_550, FS_551, FS_552, FS_553,
};
use larmorx_core::math::Libm;
use larmorx_core::x87::F80;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{gaussian_matrix, gaussian_matrix2, matrix_identity};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add, div, div64, mul, mul64, sqrt64, sub};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MAX_INDEX, MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_sequence,
    mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, reinit_cache};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_clone, mri_copy_frame};
use larmorx_freesurfer_volume::orient::mri_set_voxel_to_ras_xform;
use larmorx_freesurfer_volume::resize::mri_set_resolution;
use larmorx_freesurfer_volume::stats::mri_calc_cras_for_sampled_volume;

use crate::mem;

/// `MRI_HEIGHT`, `MRI_WIDTH`, `MRI_DEPTH` (`mri.h`): the axes of the one-dimensional filters.
pub const MRI_HEIGHT: i32 = 0;
pub const MRI_WIDTH: i32 = 1;
pub const MRI_DEPTH: i32 = 2;

/// `FLT_EPSILON` as `mrifilter.cpp` sees it (`<float.h>`'s).
const FLT_EPSILON: f32 = f32::EPSILON;

/// `FZERO(f)` of a `float` in `mrifilter.cpp`.
fn fzero(f: f32) -> bool {
    f.abs() < FLT_EPSILON
}

/// The extent of a volume.
fn ext(m: &Mri) -> (i32, i32, i32) {
    (m.vg.width, m.vg.height, m.vg.depth)
}

fn count(w: i32, h: i32, d: i32) -> usize {
    (w.max(0) as usize) * (h.max(0) as usize) * (d.max(0) as usize)
}

/// `MRIgaussian1d(sigma, max_len)`: a float volume of `len` x 1 x 1, `len = nint(8 sigma) + 1`
/// made odd, cut to `max_len` when that is positive and smaller, at least 1 (then the single weight
/// 1); the weights `exp(-x² / (2 sigma²))` within two sigma of the centre, `(4 - |x| / sigma)⁴ /
/// (16 e²)` out to four sigma, 0 beyond (Nitzberg and Shiota's splined bump), normalised to sum 1
/// (`float` sums; `exp` and `pow` in `double`, GLIBC-07).
pub fn mri_gaussian1d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sigma: f32,
    max_len: i32,
) -> Result<Mri, Fatal> {
    let mut len = nint(g, f64::from(mul(8.0, sigma)));
    len = len.wrapping_add(1);
    if len % 2 == 0 {
        len = len.wrapping_add(1);
    }
    if max_len > 0 && max_len < len {
        len = max_len;
    }
    let half = len / 2;
    if len < 1 {
        len = 1;
    }
    let mut m = mri_alloc(g, out, err, len, 1, 1, MRI_FLOAT)?;
    let set = |m: &mut Mri, x: usize, v: f32| {
        m.chunk.as_mut().expect("voxels")[4 * x..4 * x + 4].copy_from_slice(&v.to_le_bytes());
    };
    let get = |m: &Mri, x: usize| -> f32 {
        f32::from_le_bytes(
            m.chunk.as_ref().expect("voxels")[4 * x..4 * x + 4]
                .try_into()
                .expect("4"),
        )
    };
    if len <= 1 {
        set(&mut m, 0, 1.0);
        return Ok(m);
    }
    let libm = Libm::new(g.mode());
    let two_sigma = mul(2.0, sigma);
    // 1.0f / (16.0f * (float)(M_E * M_E)), folded by the compiler
    let e2 = (std::f64::consts::E * std::f64::consts::E) as f32;
    let c = 1.0f32 / (16.0f32 * e2);
    let four_sigma = mul(4.0, sigma);
    let mut norm = 0.0f32;
    for x in 0..len {
        let fx = (x - half) as f32;
        let a = fx.abs();
        let k = if a <= two_sigma {
            let q = div(mul(-fx, fx), mul(two_sigma, sigma));
            libm.exp(f64::from(q)) as f32
        } else if two_sigma < a && a <= four_sigma {
            let base = 4.0 - f64::from(a) / f64::from(sigma);
            mul(c, libm.pow(base, 4.0) as f32)
        } else {
            0.0
        };
        set(&mut m, x as usize, k);
        norm = add(norm, k);
    }
    for x in 0..len as usize {
        let v = get(&m, x);
        set(&mut m, x, div(v, norm));
    }
    note_glibc07(g, &libm, "exp and pow");
    Ok(m)
}

/// The boundary index table's entry `j` of an axis of `n` voxels (`xi`, `yi`, `zi`: the nearest
/// voxel of the axis).
fn clamp_index(j: i32, n: i32) -> i32 {
    if j <= 0 {
        0
    } else if j >= n {
        n - 1
    } else {
        j
    }
}

/// FS-549: a kernel whose half-length passes the index tables' margin.
fn long_kernel(g: &mut Globals, what: &str, len: usize) {
    if len / 2 > MAX_INDEX as usize {
        g.notices.add(
            &FS_549,
            format!(
                "{what}: a kernel of {len} taps reads past the boundary index tables (memory contents); taken as the edge voxel"
            ),
        );
    }
}

/// The convolution of a frame `a` (`[z][y][x]` of `w h d`) with `k` along `axis`, each tap's
/// voxel the nearest of the axis (the index tables), as `total + k[i] v` from tap 0.
fn convolve_plane(a: &[f32], (w, h, d): (i32, i32, i32), k: &[f32], axis: i32) -> Vec<f32> {
    let n = count(w, h, d);
    let mut o = vec![0.0f32; n];
    let halflen = (k.len() / 2) as i32;
    let (wu, hu) = (w as usize, h as usize);
    for z in 0..d {
        for y in 0..h {
            let row = (z as usize * hu + y as usize) * wu;
            for x in 0..w {
                let mut total = 0.0f32;
                for (i, &ki) in k.iter().enumerate() {
                    let j = i as i32 - halflen;
                    let idx = match axis {
                        MRI_WIDTH => row + clamp_index(x + j, w) as usize,
                        MRI_HEIGHT => {
                            (z as usize * hu + clamp_index(y + j, h) as usize) * wu + x as usize
                        }
                        _ => (clamp_index(z + j, d) as usize * hu + y as usize) * wu + x as usize,
                    };
                    total = add(total, mul(ki, a[idx]));
                }
                o[row + x as usize] = total;
            }
        }
    }
    o
}

/// What a convolution's integer output stores for a sum.
#[derive(Clone, Copy)]
enum OutType {
    Float,
    Byte,
    Short,
    Int,
}

impl OutType {
    fn mri(self) -> i32 {
        match self {
            OutType::Float => MRI_FLOAT,
            OutType::Byte => MRI_UCHAR,
            OutType::Short => MRI_SHORT,
            OutType::Int => MRI_INT,
        }
    }
    /// The stored bytes of a sum: the float itself, or `nint`'s int in the type's low bits.
    fn bytes(self, g: &mut Globals, v: f32) -> Vec<u8> {
        match self {
            OutType::Float => v.to_le_bytes().to_vec(),
            OutType::Byte => vec![nint(g, f64::from(v)) as u8],
            OutType::Short => (nint(g, f64::from(v)) as i16).to_le_bytes().to_vec(),
            OutType::Int => nint(g, f64::from(v)).to_le_bytes().to_vec(),
        }
    }
}

/// The values of a convolution stored into frame `f` of `dst` over the source's extent, rows of
/// the source's width (FS-546 where `dst` is smaller or has no such frame).
#[allow(clippy::too_many_arguments)]
fn store(
    g: &mut Globals,
    what: &str,
    dst: &mut Mri,
    f: i32,
    (w, h, d): (i32, i32, i32),
    vals: &[f32],
    ot: OutType,
) {
    let ty = ot.mri();
    if mem::fits(dst, ty, f, w, h, d) {
        let size = mem::elem_size(ty);
        let (dw, dh) = (dst.vg.width as usize, dst.vg.height as usize);
        let base = f as usize * dst.vox_per_vol as usize;
        let mut i = 0;
        for z in 0..d as usize {
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let b = ot.bytes(g, vals[i]);
                    let o = (base + (z * dh + y) * dw + x) * size;
                    dst.chunk.as_mut().expect("voxels")[o..o + size].copy_from_slice(&b);
                    i += 1;
                }
            }
        }
        return;
    }
    let base = i64::from(f) * i64::from(dst.vg.depth);
    let mut i = 0;
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let b = ot.bytes(g, vals[i]);
                mem::write(
                    g,
                    (&FS_546, &FS_546),
                    what,
                    dst,
                    base + i64::from(z),
                    i64::from(y),
                    i64::from(x),
                    &b,
                );
                i += 1;
            }
        }
    }
}

/// FS-546's check: whether the source's frame and the destination's extent and frame let every
/// access stay in the volumes; by default an error (`NULL` returned) where they do not.
#[allow(clippy::too_many_arguments)]
fn frames_ok(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    src: &Mri,
    dst: &Mri,
    src_frame: i32,
    dst_frame: i32,
) -> bool {
    let (w, h, d) = ext(src);
    let src_ok = (0..src.nframes).contains(&src_frame);
    let dst_ok = (0..dst.nframes).contains(&dst_frame)
        && w <= dst.vg.width
        && h <= dst.vg.height
        && d <= dst.vg.depth;
    if src_ok && dst_ok {
        return true;
    }
    let detail = if !src_ok {
        format!(
            "{what}: the source has no frame {src_frame} ({} frames)",
            src.nframes
        )
    } else {
        format!(
            "{what}: a destination of {}x{}x{} and {} frames cannot take frame {dst_frame} of a {w}x{h}x{d} source",
            dst.vg.width, dst.vg.height, dst.vg.depth, dst.nframes
        )
    };
    g.notices.add(&FS_546, detail.clone());
    if g.notices.reproduce() {
        return true;
    }
    log::error_printf(g, err, ERROR_BADPARM, &detail);
    false
}

/// The source's frame as the convolution reads it: direct reads of type `ty` (the code's, not
/// necessarily the volume's: FS-547 by default reads the source's own values instead).
#[allow(clippy::too_many_arguments)]
fn source_plane(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    src: &Mri,
    ty: i32,
    frame: i32,
) -> Result<Vec<f32>, Fatal> {
    if src.mtype != ty {
        g.notices.add(
            &FS_547,
            format!(
                "{what}: a source of type {} read as type {ty}{}",
                src.mtype,
                if g.notices.reproduce() {
                    ""
                } else {
                    "; its own values used"
                }
            ),
        );
        if !g.notices.reproduce() {
            return vox_plane(g, err, src, frame);
        }
    }
    Ok(mem::plane(g, (&FS_547, &FS_546), what, src, ty, frame))
}

/// The frame `f` of `m` by `MRIgetVoxVal` (`[z][y][x]`).
fn vox_plane(g: &mut Globals, err: &mut dyn Write, m: &Mri, f: i32) -> Result<Vec<f32>, Fatal> {
    let (w, h, d) = ext(m);
    let mut v = Vec::with_capacity(count(w, h, d));
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                v.push(mri_get_vox_val(g, err, m, x, y, z, f, 0)?);
            }
        }
    }
    Ok(v)
}

/// A destination: the caller's, or a new one.
fn result(fresh: Option<Mri>, ok: bool) -> Ret<Mri> {
    match (ok, fresh) {
        (false, _) => Ret::Null,
        (true, Some(m)) => Ret::New(m),
        (true, None) => Ret::Given,
    }
}

fn is_axis(axis: i32) -> bool {
    matches!(axis, MRI_WIDTH | MRI_HEIGHT | MRI_DEPTH)
}

/// `MRIconvolve1d(mri_src, mri_dst, k, len, axis, src_frame, dst_frame)`: `mri_dst` frame
/// `dst_frame` the convolution of `mri_src`'s frame `src_frame` with `k` along `axis`. An axis of
/// one voxel copies the source (`MRIcopy`, every frame) instead. Without a destination, a float
/// volume of the source's extent (no header copied). A uchar, short or int destination goes to
/// [`mri_convolve1d_byte`], [`mri_convolve1d_short`], [`mri_convolve1d_int`]; another type than
/// float: `MRIconvolve1d: unsupported dst pixel format <type>` and `NULL`. A uchar or float source
/// repeats the edge voxel past the axis' ends; another type reads `MRIgetVoxVal` at the taps
/// (FS-545). An axis other than the three leaves the destination as it is. FS-546, FS-549.
#[allow(clippy::too_many_arguments)]
pub fn mri_convolve1d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = ext(src);
    if (axis == MRI_WIDTH && w == 1)
        || (axis == MRI_HEIGHT && h == 1)
        || (axis == MRI_DEPTH && d == 1)
    {
        return mri_copy(g, out, err, src, dst);
    }
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => fresh.insert(mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?),
    };
    let ot = match dm.mtype {
        MRI_UCHAR => OutType::Byte,
        MRI_SHORT => OutType::Short,
        MRI_INT => OutType::Int,
        MRI_FLOAT => OutType::Float,
        t => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRIconvolve1d: unsupported dst pixel format {t}"),
            );
            return Ok(Ret::Null);
        }
    };
    let ok = match ot {
        OutType::Float => convolve_float(g, err, src, dm, k, axis, src_frame, dst_frame)?,
        _ => convolve_typed(g, err, src, dm, k, axis, src_frame, dst_frame, ot)?,
    };
    Ok(result(fresh, ok))
}

/// `MRIconvolve1d`'s float destination.
#[allow(clippy::too_many_arguments)]
fn convolve_float(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
) -> Result<bool, Fatal> {
    const WHAT: &str = "MRIconvolve1d";
    if !is_axis(axis) {
        return Ok(true);
    }
    if !frames_ok(g, err, WHAT, src, dst, src_frame, dst_frame) {
        return Ok(false);
    }
    long_kernel(g, WHAT, k.len());
    let e = ext(src);
    let vals = if src.mtype == MRI_UCHAR || src.mtype == MRI_FLOAT {
        let a = mem::plane(g, (&FS_546, &FS_546), WHAT, src, src.mtype, src_frame);
        convolve_plane(&a, e, k, axis)
    } else {
        convolve_vox(g, err, src, k, axis, src_frame)?
    };
    store(g, WHAT, dst, dst_frame, e, &vals, OutType::Float);
    Ok(true)
}

/// `MRIconvolve1d`'s default branch: the taps read by `MRIgetVoxVal` at their own column, row or
/// slice, past the axis' ends too (FS-545: by default the edge voxel).
fn convolve_vox(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    k: &[f32],
    axis: i32,
    frame: i32,
) -> Result<Vec<f32>, Fatal> {
    let (w, h, d) = ext(src);
    let a = vox_plane(g, err, src, frame)?;
    let halflen = (k.len() / 2) as i32;
    let n_axis = match axis {
        MRI_WIDTH => w,
        MRI_HEIGHT => h,
        _ => d,
    };
    if k.len() > 1 {
        g.notices.add(
            &FS_545,
            format!(
                "MRIconvolve1d: a source of type {} past the axis' ends: {}",
                src.mtype,
                if g.notices.reproduce() {
                    "its outside value and the next row's voxels, as FreeSurfer"
                } else {
                    "the edge voxel"
                }
            ),
        );
    }
    if !g.notices.reproduce() {
        return Ok(convolve_plane(&a, (w, h, d), k, axis));
    }
    let (wu, hu) = (w as usize, h as usize);
    let mut o = vec![0.0f32; count(w, h, d)];
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let mut total = 0.0f32;
                for (i, &ki) in k.iter().enumerate() {
                    let j = i as i32 - halflen;
                    let (xx, yy, zz) = match axis {
                        MRI_WIDTH => (x + j, y, z),
                        MRI_HEIGHT => (x, y + j, z),
                        _ => (x, y, z + j),
                    };
                    let at = match axis {
                        MRI_WIDTH => xx,
                        MRI_HEIGHT => yy,
                        _ => zz,
                    };
                    let v = if (0..n_axis).contains(&at) {
                        a[(zz as usize * hu + yy as usize) * wu + xx as usize]
                    } else {
                        mri_get_vox_val(g, err, src, xx, yy, zz, frame, 0)?
                    };
                    total = add(total, mul(ki, v));
                }
                o[(z as usize * hu + y as usize) * wu + x as usize] = total;
            }
        }
    }
    Ok(o)
}

/// The integer-typed convolutions (`MRIconvolve1dByte`, `Short`, `Int`) into a given destination
/// of their type: the source read as the code's type (uchar for a uchar or float source in
/// `Byte`, which refuses other types; shorts or ints whatever the source in `Short` and `Int`:
/// FS-547), the sums rounded by `nint` into the type.
#[allow(clippy::too_many_arguments)]
fn convolve_typed(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
    ot: OutType,
) -> Result<bool, Fatal> {
    let (what, read_ty) = match ot {
        OutType::Byte => ("MRIconvolve1dByte", MRI_UCHAR),
        OutType::Short => ("MRIconvolve1dShort", MRI_SHORT),
        _ => ("MRIconvolve1dInt", MRI_INT),
    };
    if dst.mtype != ot.mri() {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{what}: unsupported dst pixel format {}", dst.mtype),
        );
        return Ok(false);
    }
    if matches!(ot, OutType::Byte) && src.mtype != MRI_UCHAR && src.mtype != MRI_FLOAT {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIconvolve1d: unsupported pixel format {}", src.mtype),
        );
        return Ok(false);
    }
    if !is_axis(axis) {
        return Ok(true);
    }
    if !frames_ok(g, err, what, src, dst, src_frame, dst_frame) {
        return Ok(false);
    }
    long_kernel(g, what, k.len());
    let e = ext(src);
    let a = source_plane(g, err, what, src, read_ty, src_frame)?;
    let vals = convolve_plane(&a, e, k, axis);
    store(g, what, dst, dst_frame, e, &vals, ot);
    Ok(true)
}

/// One of the typed convolutions called directly: without a destination, a new one of its type
/// (the source's extent).
#[allow(clippy::too_many_arguments)]
fn typed_entry(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
    ot: OutType,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = ext(src);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => fresh.insert(mri_alloc(g, out, err, w, h, d, ot.mri())?),
    };
    let ok = convolve_typed(g, err, src, dm, k, axis, src_frame, dst_frame, ot)?;
    Ok(result(fresh, ok))
}

/// `MRIconvolve1dByte(mri_src, mri_dst, k, len, axis, src_frame, dst_frame)`: a uchar destination
/// (new without one); `MRIconvolve1dByte: unsupported dst pixel format <type>` and `NULL` for
/// another; a source of a type but uchar and float: `MRIconvolve1d: unsupported pixel format
/// <type>` and `NULL`. A float source's bytes are read as uchar voxels (FS-547).
#[allow(clippy::too_many_arguments)]
pub fn mri_convolve1d_byte(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    typed_entry(
        g,
        out,
        err,
        src,
        dst,
        k,
        axis,
        src_frame,
        dst_frame,
        OutType::Byte,
    )
}

/// `MRIconvolve1dShort`: as [`mri_convolve1d_byte`] for a short destination, the source read as
/// shorts whatever its type (FS-547).
#[allow(clippy::too_many_arguments)]
pub fn mri_convolve1d_short(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    typed_entry(
        g,
        out,
        err,
        src,
        dst,
        k,
        axis,
        src_frame,
        dst_frame,
        OutType::Short,
    )
}

/// `MRIconvolve1dInt`: as [`mri_convolve1d_short`] for an int destination, ints.
#[allow(clippy::too_many_arguments)]
pub fn mri_convolve1d_int(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
    src_frame: i32,
    dst_frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    typed_entry(
        g,
        out,
        err,
        src,
        dst,
        k,
        axis,
        src_frame,
        dst_frame,
        OutType::Int,
    )
}

/// The kernel of a kernel volume (its first `width` floats; FS-548 for a volume that is not float:
/// an error in both modes).
fn kernel_of(g: &mut Globals, err: &mut dyn Write, kv: &Mri) -> Result<Option<Vec<f32>>, Fatal> {
    if kv.mtype != MRI_FLOAT || kv.chunk.is_none() {
        g.notices.add(
            &FS_548,
            format!("MRIconvolveGaussian: a kernel volume of type {}", kv.mtype),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIconvolveGaussian(): the kernel volume is of type {}, not float (FreeSurfer reads its memory as floats)\n",
                kv.mtype
            ),
        )?;
        return Ok(None);
    }
    let c = kv.chunk.as_ref().expect("voxels");
    let n = kv.vg.width.max(0) as usize;
    Ok(Some(
        (0..n)
            .map(|i| f32::from_le_bytes(c[4 * i..4 * i + 4].try_into().expect("4")))
            .collect(),
    ))
}

/// `MRIconvolveGaussian(mri_src, mri_dst, mri_gaussian)`: each frame convolved along the width,
/// the height and the depth with the kernel volume's weights (`MRIcopyFrame` of the frame into a
/// one-frame volume of the source's type, `MRIconvolve1d` width into it, height into the
/// destination's frame, depth back into it, `MRIcopyFrame` into the destination), then the
/// source's header. Without a destination, `MRIclone` of the source; a destination of another type
/// ends the program (`MRIconvolveGaussian: Source and destination types differ!`, exit 1); the
/// source as its own destination is smoothed through a clone and copied back. FS-548, FS-550.
pub fn mri_convolve_gaussian(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    kernel: &Mri,
) -> Result<Ret<Mri>, Fatal> {
    let Some(k) = kernel_of(g, err, kernel)? else {
        return Ok(Ret::Null);
    };
    match p {
        Pair::New(src) => {
            let mut dst = mri_clone(g, out, err, src, None)?.unwrap_new();
            gaussian_passes(g, out, err, src, &mut dst, &k)?;
            Ok(Ret::New(dst))
        }
        Pair::Given(src, dst) => {
            if src.mtype != dst.mtype {
                let _ = writeln!(
                    err,
                    "MRIconvolveGaussian: Source and destination types differ!"
                );
                return Err(Fatal::Exit(1));
            }
            gaussian_passes(g, out, err, src, dst, &k)?;
            Ok(Ret::Given)
        }
        Pair::Same(src) => {
            let mut tmp = mri_clone(g, out, err, src, None)?.unwrap_new();
            gaussian_passes(g, out, err, src, &mut tmp, &k)?;
            mri_copy(g, out, err, &tmp, Some(src))?;
            Ok(Ret::Given)
        }
    }
}

/// `MRIconvolveGaussian`'s passes into `dst`, then the source's header.
fn gaussian_passes(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    k: &[f32],
) -> Result<(), Fatal> {
    if src.nframes == 0 {
        g.notices.add(
            &FS_550,
            "MRIconvolveGaussian of a volume without frames (FreeSurfer divides by 0 here)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIconvolveGaussian(): the volume has no frames (FreeSurfer crashes here)\n",
        )?;
        return Ok(());
    }
    let mut tmp: Option<Mri> = None;
    for frame in 0..src.nframes {
        let r = match tmp.as_mut() {
            None => mri_copy_frame(g, out, err, Pair::New(src), frame, 0)?,
            Some(t) => mri_copy_frame(g, out, err, Pair::Given(src, t), frame, 0)?,
        };
        if let Ret::New(m) = r {
            tmp = Some(m);
        }
        let t = tmp.as_mut().expect("the frame's volume");
        mri_convolve1d(g, out, err, src, Some(t), k, MRI_WIDTH, frame, 0)?;
        mri_convolve1d(g, out, err, t, Some(dst), k, MRI_HEIGHT, 0, frame)?;
        mri_convolve1d(g, out, err, dst, Some(t), k, MRI_DEPTH, frame, 0)?;
        mri_copy_frame(g, out, err, Pair::Given(t, dst), 0, frame)?;
    }
    mri_copy_header(g, out, err, src, Some(dst))?;
    Ok(())
}

/// The external variables of `MRIgaussianSmoothNI`'s two-Gaussian mixture (`smni_cw1`,
/// `smni_cstd2`, ... in `mri.h`; weights 1 by default: one Gaussian), which `mri_fwhm` and
/// `mri_gtmpvc` set from their options.
#[derive(Clone, Copy, Debug)]
pub struct Smni {
    pub cw1: f32,
    pub cstd2: f32,
    pub rw1: f32,
    pub rstd2: f32,
    pub sw1: f32,
    pub sstd2: f32,
}

impl Default for Smni {
    fn default() -> Smni {
        Smni {
            cw1: 1.0,
            cstd2: 0.0,
            rw1: 1.0,
            rstd2: 0.0,
            sw1: 1.0,
            sstd2: 0.0,
        }
    }
}

/// `%Lg` of a `long double` (6 significant digits), from its value rounded to `double` (the
/// diagnostic lines only).
fn lg(v: F80) -> String {
    larmorx_core::cfmt::g(v.to_f64(), 6)
}

/// `MRIgaussianSmoothNI(src, cstd, rstd, sstd, targ)`: Gaussian smoothing along the columns, rows
/// and slices of every frame with standard deviations `cstd`, `rstd`, `sstd` (in mm, divided by the
/// voxel size; an axis is skipped where its deviation is not above 0), each line of voxels
/// multiplied by `GaussianMatrix(len, std, 1)` (with `smni`'s weights below 1, `GaussianMatrix2`'s
/// mixture), then every voxel divided by the kernel's sum (the products of the three middle rows'
/// weights, added in `long double`). Without a target, a float volume of the source's extent and
/// frames with the source copied into it (`MRIcopy`); a target of another extent or frames:
/// `ERROR: MRIgaussianSmoothNI: <width|height|depth|frames> dimension mismatch` and `NULL`; the
/// source as its own target is smoothed in place. With `Gdiag_no` above 0, FreeSurfer's diagnostic
/// lines (the threads: 1 here). FS-551.
#[allow(clippy::too_many_arguments)]
pub fn mri_gaussian_smooth_ni(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    cstd: f64,
    rstd: f64,
    sstd: f64,
    smni: &Smni,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh: Option<Mri> = None;
    let targ: &mut Mri = match p {
        Pair::New(src) => {
            let (w, h, d) = ext(src);
            let t = fresh.insert(mri_alloc_sequence(
                g,
                out,
                err,
                w,
                h,
                d,
                MRI_FLOAT,
                src.nframes,
            )?);
            mri_copy(g, out, err, src, Some(t))?;
            t
        }
        Pair::Given(src, t) => {
            for (name, a, b) in [
                ("width", src.vg.width, t.vg.width),
                ("height", src.vg.height, t.vg.height),
                ("depth", src.vg.depth, t.vg.depth),
                ("frames", src.nframes, t.nframes),
            ] {
                if a != b {
                    let _ = writeln!(out, "ERROR: MRIgaussianSmoothNI: {name} dimension mismatch");
                    return Ok(Ret::Null);
                }
            }
            mri_copy(g, out, err, src, Some(t))?;
            t
        }
        Pair::Same(t) => t,
    };
    if !smooth_ni(g, out, err, targ, cstd, rstd, sstd, smni)? {
        return Ok(Ret::Null);
    }
    Ok(result(fresh, true))
}

/// `MRIgaussianSmoothNI`'s smoothing of `targ` in place.
#[allow(clippy::too_many_arguments)]
fn smooth_ni(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    targ: &mut Mri,
    cstd: f64,
    rstd: f64,
    sstd: f64,
    smni: &Smni,
) -> Result<bool, Fatal> {
    let (w, h, d) = ext(targ);
    let nf = targ.nframes;
    let diag = g.gdiag_no > 0;
    if diag {
        let procs = std::thread::available_parallelism().map_or(1, |n| n.get());
        let _ = writeln!(
            out,
            "MRIgaussianSmoothNI(): {procs} avail. processors, running in 1 threads"
        );
        if smni.cw1 != 1.0 || smni.rw1 != 1.0 || smni.sw1 != 1.0 {
            let f = |v: f32| larmorx_core::cfmt::g(f64::from(v), 6);
            let _ = writeln!(
                out,
                "MRIgaussianSmoothNI(): G2: {} {} {}  {} {} {}",
                f(smni.cw1),
                f(smni.cstd2),
                f(smni.rw1),
                f(smni.rstd2),
                f(smni.sw1),
                f(smni.sstd2)
            );
        }
    }
    for (std, n) in [(cstd, w), (rstd, h), (sstd, d)] {
        if std > 0.0 && n == 0 {
            g.notices.add(
                &FS_551,
                "MRIgaussianSmoothNI along an axis of 0 voxels (FreeSurfer writes outside a matrix)",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRIgaussianSmoothNI(): an axis of 0 voxels (FreeSurfer writes outside a matrix here)\n",
            )?;
            return Ok(false);
        }
    }
    let verbose = diag && g.gdiag & larmorx_freesurfer_core::diag::DIAG_VERBOSE != 0;
    // the axes: (std, the axis' voxels, its voxel size, the mixture's weight and second std,
    // the line index printed, printed without DIAG_VERBOSE_ON)
    let axes = [
        (cstd, w, targ.vg.xsize, smni.cw1, smni.cstd2, verbose),
        (rstd, h, targ.vg.ysize, smni.rw1, smni.rstd2, diag),
        (sstd, d, targ.vg.zsize, smni.sw1, smni.sstd2, diag),
    ];
    let mut middle: [Option<Vec<f64>>; 3] = [None, None, None];
    for (ax, &(std, n, size, w1, std2, show)) in axes.iter().enumerate() {
        if std <= 0.0 || std.is_nan() {
            continue;
        }
        if ax == 1 && verbose {
            let _ = writeln!(out, "Smoothing rows");
        }
        let gm = if w1 == 1.0 {
            gaussian_matrix(g, out, err, n, (std / f64::from(size)) as f32, 1, None)?
        } else {
            gaussian_matrix2(
                g,
                out,
                err,
                n,
                (std / f64::from(size)) as f32,
                div(std2, size),
                w1,
                1,
                None,
            )?
        };
        let Ret::New(gm) = gm else {
            return Ok(false);
        };
        let mut v = matrix_alloc(g, out, err, n, 1, MATRIX_REAL)?.expect("a column");
        let mut vg = matrix_alloc(g, out, err, n, 1, MATRIX_REAL)?.expect("a column");
        // the lines: columns along x for each row, rows along y and slices along z for each column
        let outer = if ax == 0 { h } else { w };
        for o in 0..outer {
            if show {
                let _ = write!(out, "{o} ");
                if o % 10 == 9 {
                    let _ = writeln!(out);
                }
            }
            let (inner, other) = if ax == 2 { (h, d) } else { (d, n) };
            for s in 0..inner {
                for f in 0..nf {
                    let place = |i: i32| match ax {
                        0 => (i, o, s),
                        1 => (o, i, s),
                        _ => (o, s, i),
                    };
                    for i in 0..other {
                        let (x, y, z) = place(i);
                        let val = mri_get_vox_val(g, err, targ, x, y, z, f, 0)?;
                        v.set_relt(i + 1, 1, val);
                    }
                    matrix_multiply(g, out, err, Some(&gm), Some(&v), Some(&mut vg))?;
                    for i in 0..other {
                        let (x, y, z) = place(i);
                        mri_set_vox_val(g, err, targ, x, y, z, f, vg.relt(i + 1, 1), 0)?;
                    }
                }
            }
        }
        if show {
            let _ = writeln!(out);
        }
        let mid: Vec<f64> = if n > 1 {
            (0..n).map(|c| f64::from(gm.relt(n / 2, c + 1))).collect()
        } else {
            vec![1.0]
        };
        middle[ax] = Some(mid);
    }
    let stops = [w, h, d];
    let stop = |ax: usize, std: f64| if std > 0.0 { stops[ax] } else { 1 };
    let (cstop, rstop, sstop) = (stop(0, cstd), stop(1, rstd), stop(2, sstd));
    let mut scale = F80::from(0.0f64);
    let mut vmf = F80::from(0.0f64);
    for c in 0..cstop {
        let (mut aa, mut bb, mut cc) = (1.0f64, 1.0f64, 1.0f64);
        for r in 0..rstop {
            for s in 0..sstop {
                if cstd > 0.0 {
                    aa = middle[0].as_ref().expect("columns")[c as usize];
                }
                if rstd > 0.0 {
                    bb = middle[1].as_ref().expect("rows")[r as usize];
                }
                if sstd > 0.0 {
                    cc = middle[2].as_ref().expect("slices")[s as usize];
                }
                let val = mul64(mul64(aa, bb), cc);
                scale = scale + F80::from_f64(val);
                vmf = vmf + F80::from_f64(mul64(val, val));
            }
        }
    }
    if diag {
        let _ = writeln!(out, "MRIguassianSmoothNI(): scale = {}", lg(scale));
        let one = F80::from(1.0f64);
        let _ = writeln!(
            out,
            "MRIguassianSmoothNI(): VMF = {}, VRF = {}",
            lg(vmf),
            lg(one / vmf)
        );
    }
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                for f in 0..nf {
                    let val = f64::from(mri_get_vox_val(g, err, targ, c, r, s, f, 0)?);
                    let q = (F80::from_f64(val) / scale).to_f32();
                    mri_set_vox_val(g, err, targ, c, r, s, f, q, 0)?;
                }
            }
        }
    }
    Ok(true)
}

/// `MRIgaussianSmooth(src, std, norm, targ)`: `MRIgaussianSmoothNI` with `std` on the three axes;
/// `norm` 0 gives `ERROR: MRIgaussianSmooth: norm must be non-zero` and `NULL`.
pub fn mri_gaussian_smooth(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    std: f64,
    norm: i32,
) -> Result<Ret<Mri>, Fatal> {
    if norm == 0 {
        let _ = writeln!(out, "ERROR: MRIgaussianSmooth: norm must be non-zero");
        return Ok(Ret::Null);
    }
    mri_gaussian_smooth_ni(g, out, err, p, std, std, std, &Smni::default())
}

/// `MRImaskedGaussianSmooth(src, binmask, std, targ)`: without a mask, `MRIgaussianSmooth`; with
/// one, the source's voxels where the mask (frame 0) is below 0.5 zeroed in a copy (every frame),
/// the copy smoothed into the target, the mask smoothed alone, then the target zeroed outside the
/// mask and divided by the smoothed mask inside it (in `double`). The source as its own target is
/// allowed (the copy is made first). FS-552: a target the smoother refuses (another extent or
/// frames) ends FreeSurfer after the smoother's message; an error here.
pub fn mri_masked_gaussian_smooth(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    binmask: Option<&Mri>,
    std: f64,
) -> Result<Ret<Mri>, Fatal> {
    let Some(mask) = binmask else {
        return mri_gaussian_smooth(g, out, err, p, std, 1);
    };
    let (src_ref, mut targ): (&Mri, Option<&mut Mri>) = match p {
        Pair::New(s) => (s, None),
        Pair::Given(s, t) => (s, Some(t)),
        Pair::Same(t) => {
            // the copy is made before the target is touched: smooth a copy into the source
            let copy = mri_copy(g, out, err, t, None)?.unwrap_new();
            return masked_into(g, out, err, copy, mask, std, Some(t));
        }
    };
    let copy = mri_copy(g, out, err, src_ref, None)?.unwrap_new();
    masked_into(g, out, err, copy, mask, std, targ.take())
}

/// `MRImaskedGaussianSmooth` from the source's copy `srcmasked`.
fn masked_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mut srcmasked: Mri,
    mask: &Mri,
    std: f64,
    targ: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = ext(&srcmasked);
    let nf = srcmasked.nframes;
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let m = f64::from(mri_get_vox_val(g, err, mask, c, r, s, 0, 0)?);
                if m >= 0.5 {
                    continue;
                }
                for f in 0..nf {
                    mri_set_vox_val(g, err, &mut srcmasked, c, r, s, f, 0.0, 0)?;
                }
            }
        }
    }
    let given = targ.is_some();
    let mut fresh = None;
    let r = match targ {
        Some(t) => {
            let r = mri_gaussian_smooth(g, out, err, Pair::Given(&srcmasked, &mut *t), std, 1)?;
            (r, Some(t))
        }
        None => (
            mri_gaussian_smooth(g, out, err, Pair::New(&srcmasked), std, 1)?,
            None,
        ),
    };
    let t: &mut Mri = match r {
        (Ret::New(m), _) => fresh.insert(m),
        (Ret::Given, Some(t)) => t,
        _ => {
            g.notices.add(
                &FS_552,
                "MRImaskedGaussianSmooth with a target the smoother refused (FreeSurfer crashes here)",
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRImaskedGaussianSmooth(): the smoother refused the target (FreeSurfer crashes here)\n",
            )?;
            return Ok(Ret::Null);
        }
    };
    let smoothed = mri_gaussian_smooth(g, out, err, Pair::New(mask), std, 1)?;
    let Ret::New(masksm) = smoothed else {
        return Ok(Ret::Null);
    };
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let m = f64::from(mri_get_vox_val(g, err, mask, c, r, s, 0, 0)?);
                if m < 0.5 {
                    for f in 0..nf {
                        mri_set_vox_val(g, err, t, c, r, s, f, 0.0, 0)?;
                    }
                    continue;
                }
                let m = f64::from(mri_get_vox_val(g, err, &masksm, c, r, s, 0, 0)?);
                for f in 0..nf {
                    let v = f64::from(mri_get_vox_val(g, err, t, c, r, s, f, 0)?);
                    mri_set_vox_val(g, err, t, c, r, s, f, div64(v, m) as f32, 0)?;
                }
            }
        }
    }
    Ok(if given {
        Ret::Given
    } else {
        result(fresh, true)
    })
}

/// `MRImean(mri_src, mri_dst, wsize)`: each voxel of each frame the mean (`float` sum and count)
/// of the source's voxels in the `wsize` window (half-width `(wsize - 1) / 2`) that lie in the
/// volume; a voxel whose window holds none is left as it is. Without a destination, a float volume
/// of the source's extent and frames with its header.
pub fn mri_mean(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    wsize: i32,
) -> Result<Ret<Mri>, Fatal> {
    let whalf = (wsize - 1) / 2;
    let (w, h, d) = ext(src);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, w, h, d, MRI_FLOAT, src.nframes)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    for frame in 0..src.nframes {
        let a = vox_plane(g, err, src, frame)?;
        let (wu, hu) = (w as usize, h as usize);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let (mut num, mut val) = (0.0f32, 0.0f32);
                    for z0 in -whalf..=whalf {
                        if z + z0 < 0 || z + z0 >= d {
                            continue;
                        }
                        for y0 in -whalf..=whalf {
                            if y + y0 < 0 || y + y0 >= h {
                                continue;
                            }
                            for x0 in -whalf..=whalf {
                                if x + x0 < 0 || x + x0 >= w {
                                    continue;
                                }
                                let i = ((z + z0) as usize * hu + (y + y0) as usize) * wu
                                    + (x + x0) as usize;
                                // the official build: the voxel plus the sum (the NaNs' bits)
                                val = add(a[i], val);
                                num += 1.0;
                            }
                        }
                    }
                    if !fzero(num) {
                        val = div(val, num);
                        mri_set_vox_val(g, err, dm, x, y, z, frame, val, 0)?;
                    }
                }
            }
        }
    }
    Ok(result(fresh, true))
}

/// `MRIstd` and `MRIstdNonzero`'s common loop (`nonzero`: voxels whose mean is 0 are skipped, and
/// so are window voxels that are 0).
#[allow(clippy::too_many_arguments)]
fn std_window(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    mean: &Mri,
    wsize: i32,
    nonzero: bool,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = ext(src);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    if dm.mtype != MRI_FLOAT {
        log::error_printf(g, err, ERROR_UNSUPPORTED, "MRIstd: dst must be MRI_FLOAT");
        // ErrorReturn(mri_dst, ...): the destination returned, unchanged
        return Ok(result(fresh, true));
    }
    let whalf = wsize / 2;
    let a = vox_plane(g, err, src, 0)?;
    let (wu, hu) = (w as usize, h as usize);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let m = mri_get_vox_val(g, err, mean, x, y, z, 0, 0)?;
                if nonzero && fzero(m) {
                    continue;
                }
                let mut wcubed = 0.0f32;
                let mut variance = 0.0f32;
                for z0 in -whalf..=whalf {
                    let zi = z + z0;
                    if zi < 0 || zi >= d {
                        continue;
                    }
                    for y0 in -whalf..=whalf {
                        let yi = y + y0;
                        if yi < 0 || yi >= h {
                            continue;
                        }
                        for x0 in -whalf..=whalf {
                            let xi = x + x0;
                            if xi < 0 || xi >= w {
                                continue;
                            }
                            let mut f = a[(zi as usize * hu + yi as usize) * wu + xi as usize];
                            if nonzero && fzero(f) {
                                continue;
                            }
                            wcubed += 1.0;
                            f = sub(f, m);
                            // the official build: the square plus the sum (the NaNs' bits)
                            variance = add(mul(f, f), variance);
                        }
                    }
                }
                let v = if wcubed == 0.0 {
                    0.0
                } else {
                    sqrt64(f64::from(div(variance, wcubed))) as f32
                };
                mri_set_vox_val(g, err, dm, x, y, z, 0, v, 0)?;
            }
        }
    }
    Ok(result(fresh, true))
}

/// `MRIstd(mri_src, mri_dst, mri_mean, wsize)`: each voxel the standard deviation (`float` sums,
/// `sqrt` in `double`) of the source's frame 0 in the `wsize` window (half-width `wsize / 2`, the
/// voxels in the volume) around `mri_mean`'s voxel; 0 where the window holds none. Without a
/// destination, a float volume with the source's header; a destination that is not float: `MRIstd:
/// dst must be MRI_FLOAT` and the destination returned as it is.
pub fn mri_std(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    mean: &Mri,
    wsize: i32,
) -> Result<Ret<Mri>, Fatal> {
    std_window(g, out, err, src, dst, mean, wsize, false)
}

/// `MRIstdNonzero`: as [`mri_std`] over the window's voxels that are not zero (`FZERO`), voxels
/// whose mean is zero left as they are.
pub fn mri_std_nonzero(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    mean: &Mri,
    wsize: i32,
) -> Result<Ret<Mri>, Fatal> {
    std_window(g, out, err, src, dst, mean, wsize, true)
}

/// `KERNEL_SIZE` and `kernel` of `mrifilter.cpp`'s reductions: `{0.25 - K_A / 2, 0.25, K_A, 0.25,
/// 0.25 - K_A / 2}` with `K_A` 0.4 (`float`).
pub const REDUCE_KERNEL: [f32; 5] = {
    let a = 0.4f32;
    let e = 0.25f32 - a / 2.0f32;
    [e, 0.25, a, 0.25, e]
};

/// `MRImodifySampledHeader(src, dst)` (file-local): the source's header, `thick`, `ps`, `scale` and
/// the voxel sizes doubled, the centre `MRIcalcCRASforSampledVolume`'s, the cached matrices
/// recomputed.
fn modify_sampled_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: &mut Mri,
) -> Result<(), Fatal> {
    mri_copy_header(g, out, err, src, Some(dst))?;
    dst.thick = mul(src.thick, 2.0);
    dst.ps = mul(src.ps, 2.0);
    dst.scale = src.scale.wrapping_mul(2);
    dst.vg.xsize = mul(src.vg.xsize, 2.0);
    dst.vg.ysize = mul(src.vg.ysize, 2.0);
    dst.vg.zsize = mul(src.vg.zsize, 2.0);
    let (c_r, c_a, c_s) = mri_calc_cras_for_sampled_volume(g, out, err, src, dst)?;
    dst.vg.c_a = c_a as f32;
    dst.vg.c_s = c_s as f32;
    dst.vg.c_r = c_r as f32;
    reinit_cache(g, out, err, &mut dst.vg)?;
    Ok(())
}

/// `MRIreduce1d(mri_src, mri_dst, k, len, axis)`: each destination voxel (frame 0) the sum of
/// `k[i]` times the source's voxel at twice the destination's place along the other axes and at
/// `2 x + i - (len - 1) / 2` along `axis`, cut to the axis (`MRIgetVoxVal`, `MRIsetVoxVal`).
/// Without a destination, one of half the source's extent and its type (`imnr1` set, the voxel
/// sizes doubled by `MRIsetResolution`). An axis other than the three leaves it as it is.
pub fn mri_reduce1d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (sw, sh, sd) = ext(src);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc(g, out, err, sw / 2, sh / 2, sd / 2, src.mtype)?;
            m.imnr1 = m.imnr0 + m.vg.depth - 1;
            let (xs, ys, zs) = (
                mul(src.vg.xsize, 2.0),
                mul(src.vg.ysize, 2.0),
                mul(src.vg.zsize, 2.0),
            );
            mri_set_resolution(g, out, err, &mut m, xs, ys, zs)?;
            fresh.insert(m)
        }
    };
    reduce_lines(g, err, src, dm, k, axis, true)?;
    Ok(result(fresh, true))
}

/// `MRIreduceSlice(mri_src, mri_dst, k, len, axis)`: as [`mri_reduce1d`] within each slice (the
/// slice index kept), along the width or the height; without a destination, a uchar volume of
/// half the source's width and height and its depth (the voxel sizes doubled in x and y).
pub fn mri_reduce_slice(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    k: &[f32],
    axis: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (sw, sh, sd) = ext(src);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc(g, out, err, sw / 2, sh / 2, sd, MRI_UCHAR)?;
            m.imnr1 = m.imnr0 + m.vg.depth - 1;
            let (xs, ys) = (mul(src.vg.xsize, 2.0), mul(src.vg.ysize, 2.0));
            mri_set_resolution(g, out, err, &mut m, xs, ys, src.vg.zsize)?;
            fresh.insert(m)
        }
    };
    if axis != MRI_DEPTH {
        reduce_lines(g, err, src, dm, k, axis, false)?;
    }
    Ok(result(fresh, true))
}

/// The reductions' loops (`twice_z`: the slice index doubled, as `MRIreduce1d`).
fn reduce_lines(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    k: &[f32],
    axis: i32,
    twice_z: bool,
) -> Result<(), Fatal> {
    if !is_axis(axis) {
        return Ok(());
    }
    let (sw, sh, sd) = ext(src);
    let (dw, dh, dd) = ext(dst);
    let halflen = (k.len() as i32 - 1) / 2;
    let edge = |j: i32, n: i32| {
        if j < 0 {
            0
        } else if j >= n {
            n - 1
        } else {
            j
        }
    };
    for z in 0..dd {
        for y in 0..dh {
            for x in 0..dw {
                let zbase = if twice_z { 2 * z } else { z };
                let mut total = 0.0f32;
                for (i, &ki) in k.iter().enumerate() {
                    let j = i as i32 - halflen;
                    let (xi, yi, zi) = match axis {
                        MRI_WIDTH => (edge(2 * x + j, sw), 2 * y, zbase),
                        MRI_HEIGHT => (2 * x, edge(2 * y + j, sh), zbase),
                        _ => (2 * x, 2 * y, edge(2 * z + j, sd)),
                    };
                    let val = mri_get_vox_val(g, err, src, xi, yi, zi, 0, 0)?;
                    total = add(total, mul(ki, val));
                }
                mri_set_vox_val(g, err, dst, x, y, z, 0, total, 0)?;
            }
        }
    }
    Ok(())
}

/// `MRIreduce(mri_src, mri_dst)`: the source (width and height above 1, else the program ends with
/// `MRIreduce: insufficient dimension (<w>, <h>, <d>)`) convolved along the width and the height
/// with the reduction kernel and reduced along the depth by [`mri_reduce1d`] (a source of one slice:
/// convolved along the width and reduced along the height); the source's header, the voxel sizes
/// doubled (the depth's kept for one slice), the start and end coordinates the source's, the
/// voxel-to-RAS matrix the source's times the doubling. FS-553: without a destination, a source of
/// one slice gives a volume of depth 0 (by default the reduced slice, depth 1).
pub fn mri_reduce(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = ext(src);
    if w <= 1 || h <= 1 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIreduce: insufficient dimension ({w}, {h}, {d})"),
        )?;
        return Ok(Ret::Null);
    }
    let k = &REDUCE_KERNEL;
    let mtmp1 = mri_convolve1d(g, out, err, src, None, k, MRI_WIDTH, 0, 0)?;
    let Ret::New(mtmp1) = mtmp1 else {
        return Ok(Ret::Null);
    };
    let given = dst.is_some();
    let r = if d > 1 {
        let mtmp2 = mri_convolve1d(g, out, err, &mtmp1, None, k, MRI_HEIGHT, 0, 0)?;
        let Ret::New(mtmp2) = mtmp2 else {
            return Ok(Ret::Null);
        };
        reduce_into(g, out, err, &mtmp2, dst, k, MRI_DEPTH, false)?
    } else {
        reduce_into(g, out, err, &mtmp1, dst, k, MRI_HEIGHT, true)?
    };
    let (mut fresh, dm): (Option<Mri>, Option<&mut Mri>) = match r {
        (Ret::New(m), _) => (Some(m), None),
        (_, Some(d)) => (None, Some(d)),
        _ => return Ok(Ret::Null),
    };
    let dm: &mut Mri = match dm {
        Some(d) => d,
        None => fresh.as_mut().expect("a destination"),
    };
    mri_copy_header(g, out, err, src, Some(dm))?;
    let zs = if d == 1 {
        src.vg.zsize
    } else {
        mul(src.vg.zsize, 2.0)
    };
    mri_set_resolution(
        g,
        out,
        err,
        dm,
        mul(src.vg.xsize, 2.0),
        mul(src.vg.ysize, 2.0),
        zs,
    )?;
    dm.xstart = src.xstart;
    dm.ystart = src.ystart;
    dm.zstart = src.zstart;
    dm.xend = src.xend;
    dm.yend = src.yend;
    dm.zend = src.zend;
    let v2r = extract_i_to_r(g, out, err, &src.vg)?;
    let mut sc = matrix_identity(g, out, err, 4, None)?.unwrap_new();
    sc.set_relt(1, 1, 2.0);
    sc.set_relt(2, 2, 2.0);
    sc.set_relt(3, 3, if d == 1 { 1.0 } else { 2.0 });
    let m = matrix_multiply(g, out, err, Some(&v2r), Some(&sc), None)?.unwrap_new();
    mri_set_voxel_to_ras_xform(g, out, err, env, dm, &m)?;
    Ok(if given {
        Ret::Given
    } else {
        result(fresh, true)
    })
}

/// `MRIreduce1d` of `MRIreduce`, its destination returned with it (`one_slice`: FS-553's case).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reduce_into<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&'a mut Mri>,
    k: &[f32],
    axis: i32,
    one_slice: bool,
) -> Result<(Ret<Mri>, Option<&'a mut Mri>), Fatal> {
    match dst {
        Some(d) => {
            let r = mri_reduce1d(g, out, err, src, Some(&mut *d), k, axis)?;
            Ok((r, Some(d)))
        }
        None => {
            if one_slice {
                g.notices.add(
                    &FS_553,
                    if g.notices.reproduce() {
                        "MRIreduce of one slice: a volume of depth 0, as FreeSurfer"
                    } else {
                        "MRIreduce of one slice: the reduced slice (FreeSurfer returns a volume of depth 0)"
                    },
                );
                if !g.notices.reproduce() {
                    let (sw, sh, _) = ext(src);
                    let mut m = mri_alloc(g, out, err, sw / 2, sh / 2, 1, src.mtype)?;
                    m.imnr1 = m.imnr0 + m.vg.depth - 1;
                    let (xs, ys, zs) = (
                        mul(src.vg.xsize, 2.0),
                        mul(src.vg.ysize, 2.0),
                        mul(src.vg.zsize, 2.0),
                    );
                    mri_set_resolution(g, out, err, &mut m, xs, ys, zs)?;
                    mri_reduce1d(g, out, err, src, Some(&mut m), k, axis)?;
                    return Ok((Ret::New(m), None));
                }
            }
            Ok((mri_reduce1d(g, out, err, src, None, k, axis)?, None))
        }
    }
}

/// `MRIreduce2D(mri_src, mri_dst)`: the source (width and height above 1, else `MRIreduce2D:
/// insufficient dimension (<w>, <h>)` ends the program) convolved along the width and reduced
/// along the height within each slice ([`mri_reduce_slice`]: a uchar volume without a
/// destination), then the sampled header (`MRImodifySampledHeader`). The source's cached matrices
/// are filled when it has none (as FreeSurfer's).
pub fn mri_reduce2d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, _) = ext(src);
    if w <= 1 || h <= 1 {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIreduce2D: insufficient dimension ({w}, {h})"),
        )?;
        return Ok(Ret::Null);
    }
    let k = &REDUCE_KERNEL;
    let Ret::New(mtmp1) = mri_convolve1d(g, out, err, src, None, k, MRI_WIDTH, 0, 0)? else {
        return Ok(Ret::Null);
    };
    match dst {
        Some(d) => {
            mri_reduce_slice(g, out, err, &mtmp1, Some(&mut *d), k, MRI_HEIGHT)?;
            modify_sampled_header(g, out, err, src, d)?;
            Ok(Ret::Given)
        }
        None => {
            let mut m = mri_reduce_slice(g, out, err, &mtmp1, None, k, MRI_HEIGHT)?.unwrap_new();
            modify_sampled_header(g, out, err, src, &mut m)?;
            Ok(Ret::New(m))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reduction_kernel() {
        let e = 0.25f32 - 0.4f32 / 2.0f32;
        assert_eq!(REDUCE_KERNEL, [e, 0.25, 0.4, 0.25, e]);
    }
}
