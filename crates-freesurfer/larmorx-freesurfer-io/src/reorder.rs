// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Reordering a volume's axes (`utils/mriio.cpp`, item 3.1-30): `MRIreorderVox2RAS` (the
//! voxel-to-RAS matrix of the reordered volume) and `MRIreorder` (the voxels moved: `mri_convert
//! -r`, `mri_robust_register`, `mri_robust_template`, the conform step's slice order); since
//! relay 44 `MRIreorder4` (the four dimensions reordered: `mri_convert --reorder4`, the item of
//! its program, 3.2-07, taken early).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 32):
//! - **FS-843** (crash): dimension numbers whose absolute values' product and sum both come to
//!   6 in 32-bit arithmetic without being 1, 2 and 3 pass FreeSurfer's check and are used as
//!   indices: by default refused with FreeSurfer's own message; with `reproduce` the program ends
//!   as FreeSurfer's (139).
//! - **FS-844** (memory): a given destination with fewer frames than the source: FreeSurfer reads
//!   its slice table past the end. Refused in both modes, noted.
//! - **FS-845** (out of bounds): a given destination of another voxel type is written with the
//!   source's voxel size at the destination's row starts (values misplaced; a smaller type
//!   written past its voxels). Refused by default; with `reproduce` the bytes go where FreeSurfer
//!   puts them while they stay in the destination's voxels (refused where they would not), noted.
//! - FS-58 (crash): a source or destination without voxels.
//! - **FS-937** (out of bounds): `MRIreorder4` with an order value outside 1 to 4 reads
//!   FreeSurfer's arrays of four dimensions and indices outside them (stack memory: the extents of
//!   the result and the voxels' places are memory contents). Refused in both modes, noted.
//! - **FS-938** (rule B): `MRIreorder4` with an order that is not a permutation (a value
//!   repeated): FreeSurfer makes a volume with that dimension twice and puts each voxel at the
//!   place its repeated index gives (a voxel written several times keeps the last source voxel's
//!   value, the rest stay 0). Refused by default; `reproduce` gives FreeSurfer's volume, noted.

use std::io::Write;

use larmorx_core::defects::{FS_58, FS_843, FS_844, FS_845, FS_937, FS_938};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::matrix_multiply;
use larmorx_freesurfer_core::matrix_ops::{matrix_const_val, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_FLOAT_COMPLEX, MRI_INT, MRI_LONG, MRI_RGB, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
    mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::{set_vox2ras_from_matrix, xfm_crs2xyz};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

/// `XDIM`, `YDIM`, `ZDIM` (`mri.h`).
pub const XDIM: i32 = 1;
pub const YDIM: i32 = 2;
pub const ZDIM: i32 = 3;

/// The types whose rows FreeSurfer's slice table points at their own places (the others' rows
/// all point at the first voxel, FS-166).
const ROWS_STEPPED: [i32; 8] = [
    MRI_UCHAR,
    MRI_SHORT,
    MRI_USHRT,
    MRI_RGB,
    MRI_INT,
    MRI_LONG,
    MRI_FLOAT,
    MRI_FLOAT_COMPLEX,
];

/// `ISIGN(a)` (`macros.h`): 1 for 0 and above, -1 below.
fn isign(a: i32) -> i32 {
    if a >= 0 { 1 } else { -1 }
}

/// Whether the three numbers are 1, 2 and 3 in some order and with any signs.
fn is_permutation(xdim: i32, ydim: i32, zdim: i32) -> bool {
    let mut a = [
        xdim.unsigned_abs(),
        ydim.unsigned_abs(),
        zdim.unsigned_abs(),
    ];
    a.sort_unstable();
    a == [1, 2, 3]
}

/// FreeSurfer's check of the dimension numbers: the product and the sum of their `abs` values
/// (32-bit, `abs(INT_MIN)` itself) both 6.
fn passes_check(xdim: i32, ydim: i32, zdim: i32) -> bool {
    let (a, b, c) = (
        xdim.wrapping_abs(),
        ydim.wrapping_abs(),
        zdim.wrapping_abs(),
    );
    a.wrapping_mul(b).wrapping_mul(c) == 6 && a.wrapping_add(b).wrapping_add(c) == 6
}

/// `MRIreorderVox2RAS(mri_src, mri_dst, xdim, ydim, zdim)`: the destination's voxel-to-RAS
/// matrix set to the source's (`MRIxfmCRS2XYZ(src, 0)`) times `Q`, the 4 x 4 matrix (all
/// `float`, from `MatrixConstVal(0)`) that maps a destination voxel to the source voxel it came
/// from: row `k` holds `ISIGN(dim_k)` in column `abs(dim_k)` and, for a negative `dim_k`, the
/// source's extent minus 1 along that axis in column 4 (`Q[4][4] = 1`), the product
/// `MatrixMultiply`'s (each element summed in `float`); then `MRIsetVox2RASFromMatrix(dst, M)`,
/// which keeps the destination's voxel sizes (printing its warning when the columns' lengths
/// differ from them by more than 0.001). With `Gdiag` not 0, `Msrc`, `Q`, `Qinv` and `Mdst` are
/// printed on standard output first, each after its title line (`Msrc ------------------------`,
/// ...), with `MatrixPrint`. Returns 0. Numbers that are not 1, 2 and 3 with signs (FreeSurfer
/// writes outside its matrix; [`mri_reorder`] refuses them first) are refused here with
/// FreeSurfer's `ERROR: replicated/incorrect dimension number`, returning 1 with the
/// destination unchanged (FS-843).
#[allow(clippy::too_many_arguments)]
pub fn mri_reorder_vox2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    xdim: i32,
    ydim: i32,
    zdim: i32,
) -> Result<i32, Fatal> {
    if !is_permutation(xdim, ydim, zdim) {
        g.notices.add(
            &FS_843,
            format!(
                "MRIreorderVox2RAS({xdim}, {ydim}, {zdim}): dimension numbers that are not 1, 2 and 3 (FreeSurfer writes outside its matrix): refused"
            ),
        );
        let _ = out.write_all(b"ERROR: replicated/incorrect dimension number\n");
        return Ok(1);
    }
    let msrc = xfm_crs2xyz(g, out, err, &src.vg, 0, false)?;
    let mut q = matrix_const_val(g, out, err, 0.0, 4, 4, None)?.unwrap_new();
    q.set_relt(1, xdim.abs(), isign(xdim) as f32);
    q.set_relt(2, ydim.abs(), isign(ydim) as f32);
    q.set_relt(3, zdim.abs(), isign(zdim) as f32);
    q.set_relt(4, 4, 1.0);
    if xdim < 0 {
        q.set_relt(1, 4, src.vg.width.wrapping_sub(1) as f32);
    }
    if ydim < 0 {
        q.set_relt(2, 4, src.vg.height.wrapping_sub(1) as f32);
    }
    if zdim < 0 {
        q.set_relt(3, 4, src.vg.depth.wrapping_sub(1) as f32);
    }
    let mdst = matrix_multiply(g, out, err, Some(&msrc), Some(&q), None)?.unwrap_new();
    if g.gdiag > 0 {
        let _ = out.write_all(b"Msrc ------------------------\n");
        print_on_out(g, out, err, Some(&msrc));
        let _ = out.write_all(b"Q ------------------------\n");
        print_on_out(g, out, err, Some(&q));
        let _ = out.write_all(b"Qinv ------------------------\n");
        let qinv = matrix_inverse(g, out, err, Some(&q), None)?.into_new();
        print_on_out(g, out, err, qinv.as_ref());
        let _ = out.write_all(b"Mdst ------------------------\n");
        print_on_out(g, out, err, Some(&mdst));
    }
    set_vox2ras_from_matrix(g, out, err, &mut dst.vg, &mdst)?;
    Ok(0)
}

/// `MatrixPrint(stdout, m)`.
fn print_on_out(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&larmorx_freesurfer_core::matrix::Matrix>,
) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, m, 5);
    let _ = out.write_all(&buf);
}

/// The voxel size of the types `MRIreorder` moves (`None` for the others).
fn moved_size(mtype: i32) -> Option<usize> {
    match mtype {
        MRI_SHORT | MRI_USHRT => Some(2),
        MRI_FLOAT | MRI_INT => Some(4),
        MRI_UCHAR => Some(1),
        _ => None,
    }
}

/// `errno = 0; ErrorReturn(NULL, (code, message))`.
fn fail(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Ret<Mri> {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    Ret::Null
}

/// `MRIreorder(mri_src, mri_dst, xdim, ydim, zdim)`: the volume with its axes reordered: the
/// source's axis `x` becomes the destination's axis `abs(xdim)` (1 columns, 2 rows, 3 slices),
/// reversed when `xdim` is negative, and so for `y` and `z`; every frame. With `Gdiag` not 0 it
/// first prints `MRIreorder() -----------` and `xdim=<x> ydim=<y> zdim=<z>` on standard output.
/// Numbers whose `abs` values' product or sum (32-bit) is not 6 print `ERROR: replicated/incorrect
/// dimension number` on standard output and give `Null` (FS-843: the other numbers that pass).
/// Without a destination, a new volume (`MRIallocSequence` of the reordered extent, the source's
/// type and frames; `MRIcopyHeader`; the voxel sizes reordered too); then
/// [`mri_reorder_vox2ras`] (on a given destination too, before its checks), with `Gdiag` the
/// lines `src <w> <h> <d>, <xsize> <ysize> <zsize>` and `dst ...` (`%f`). A given destination
/// whose extent does not receive the source's axes gives `Null` after an error (`MRIreorder:
/// incorrect dst width`, `... dst width != src height`, `... dst height != src height`, `... dst
/// depth != height`, `... dst depth`, FreeSurfer's texts, `errno` 0). The voxels of `MRI_UCHAR`,
/// `MRI_SHORT`, `MRI_USHRT`, `MRI_INT` and `MRI_FLOAT` volumes are moved as they are (a float's
/// bits kept); another type gives `Null` after `MRIreorder: unsupported voxel format <type>` at
/// the first voxel (a volume without voxels is returned as it is). `New` or `Given`. FS-844 (a
/// destination with fewer frames), FS-845 (a destination of another type), FS-58 (no voxels).
#[allow(clippy::too_many_arguments)]
pub fn mri_reorder(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    xdim: i32,
    ydim: i32,
    zdim: i32,
) -> Result<Ret<Mri>, Fatal> {
    use larmorx_core::cfmt::f as cf;
    if g.gdiag > 0 {
        let _ = out.write_all(b"MRIreorder() -----------\n");
        let _ = writeln!(out, "xdim={xdim} ydim={ydim} zdim={zdim}");
    }
    if !passes_check(xdim, ydim, zdim) {
        let _ = out.write_all(b"ERROR: replicated/incorrect dimension number\n");
        return Ok(Ret::Null);
    }
    if !is_permutation(xdim, ydim, zdim) {
        g.notices.add(
            &FS_843,
            format!(
                "MRIreorder({xdim}, {ydim}, {zdim}): dimension numbers that pass FreeSurfer's check only through 32-bit overflow (FreeSurfer writes outside its arrays and crashes)"
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = out.write_all(b"ERROR: replicated/incorrect dimension number\n");
        return Ok(Ret::Null);
    }
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    let srcdims = [width, height, depth];
    let srcsizes = [src.vg.xsize, src.vg.ysize, src.vg.zsize];
    let (ax, ay, az) = (
        xdim.unsigned_abs() as usize - 1,
        ydim.unsigned_abs() as usize - 1,
        zdim.unsigned_abs() as usize - 1,
    );
    let mut dstdims = [0i32; 3];
    let mut dstsizes = [0f32; 3];
    dstdims[ax] = srcdims[0];
    dstdims[ay] = srcdims[1];
    dstdims[az] = srcdims[2];
    dstsizes[ax] = srcsizes[0];
    dstsizes[ay] = srcsizes[1];
    dstsizes[az] = srcsizes[2];
    let mut fresh: Option<Mri> = None;
    let given = dst.is_some();
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc_sequence(
                g,
                out,
                err,
                dstdims[0],
                dstdims[1],
                dstdims[2],
                src.mtype,
                src.nframes,
            )?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            m.vg.xsize = dstsizes[0];
            m.vg.ysize = dstsizes[1];
            m.vg.zsize = dstsizes[2];
            fresh.insert(m)
        }
    };
    mri_reorder_vox2ras(g, out, err, src, d, xdim, ydim, zdim)?;
    if g.gdiag > 0 {
        for (what, v) in [("src", &src.vg), ("dst", &d.vg)] {
            let _ = writeln!(
                out,
                "{what} {} {} {}, {} {} {}",
                v.width,
                v.height,
                v.depth,
                cf(f64::from(v.xsize), 6),
                cf(f64::from(v.ysize), 6),
                cf(f64::from(v.zsize), 6)
            );
        }
    }
    // the checks of the destination's extent, in FreeSurfer's order and texts
    let dext = [d.vg.width, d.vg.height, d.vg.depth];
    let checks: [(usize, &str, &str, &str); 3] = [
        (
            ax,
            "MRIreorder: incorrect dst width",
            "MRIreorder: incorrect dst width",
            "MRIreorder: incorrect dst width",
        ),
        (
            ay,
            "MRIreorder: incorrect dst width != src height",
            "MRIreorder: incorrect dst height != src height",
            "MRIreorder: incorrect dst depth != height",
        ),
        (
            az,
            "MRIreorder: incorrect dst depth",
            "MRIreorder: incorrect dst depth",
            "MRIreorder: incorrect dst depth",
        ),
    ];
    for (k, (axis, m0, m1, m2)) in checks.into_iter().enumerate() {
        if dext[axis] != srcdims[k] {
            return Ok(fail(g, err, ERROR_BADPARM, [m0, m1, m2][axis]));
        }
    }
    let nvox = i64::from(width.max(0))
        * i64::from(height.max(0))
        * i64::from(depth.max(0))
        * i64::from(src.nframes.max(0));
    if nvox == 0 {
        return Ok(match fresh {
            Some(m) => Ret::New(m),
            None => Ret::Given,
        });
    }
    let Some(size) = moved_size(src.mtype) else {
        return Ok(fail(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIreorder: unsupported voxel format {}", src.mtype),
        ));
    };
    if src.chunk.is_none() || d.chunk.is_none() {
        g.notices.add(
            &FS_58,
            "MRIreorder of or into a volume without voxels (FreeSurfer crashes)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIreorder(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
        return Ok(Ret::Null);
    }
    if given && d.nframes < src.nframes {
        g.notices.add(
            &FS_844,
            format!(
                "MRIreorder: a destination of {} frames for a source of {} (FreeSurfer reads its slice table past the end): refused",
                d.nframes, src.nframes
            ),
        );
        return Ok(fail(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIreorder: dst has fewer frames ({}) than src ({})",
                d.nframes, src.nframes
            ),
        ));
    }
    let dsize = d.bytes_per_vox;
    if given && d.mtype != src.mtype {
        let stepped = ROWS_STEPPED.contains(&d.mtype);
        let fits = stepped && size <= dsize;
        g.notices.add(
            &FS_845,
            format!(
                "MRIreorder: a destination of type {} for a source of type {} (FreeSurfer writes the source's voxel size at the destination's rows{})",
                d.mtype,
                src.mtype,
                if fits { "" } else { ": past its voxels" }
            ),
        );
        if !g.notices.reproduce() || !stepped || !placed_inside(src, d, size, [ax, ay, az]) {
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIreorder: dst type {} differs from src type {}",
                    d.mtype, src.mtype
                ),
            ));
        }
    }
    move_voxels(src, d, size, [xdim, ydim, zdim]);
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// Whether every write of [`move_voxels`] stays inside the destination's voxels: the last
/// destination row of each frame's last slice, at its last column, plus the source's voxel size.
fn placed_inside(src: &Mri, d: &Mri, size: usize, axes: [usize; 3]) -> bool {
    let srcdims = [src.vg.width, src.vg.height, src.vg.depth];
    let mut far = [0i64; 3];
    for (k, &a) in axes.iter().enumerate() {
        far[a] = i64::from(srcdims[k]) - 1;
    }
    let (w, h, dd) = (
        i64::from(d.vg.width),
        i64::from(d.vg.height),
        i64::from(d.vg.depth),
    );
    let f = i64::from(src.nframes) - 1;
    let row = ((f * dd + far[2]) * h + far[1]) * w * d.bytes_per_vox as i64;
    let end = row + far[0] * size as i64 + size as i64;
    end <= d.chunk.as_ref().map_or(0, |c| c.len() as i64)
}

/// The voxels moved: for each frame, source slice, row and column (in that order of loops), the
/// destination place the axes and signs give; each voxel's bytes copied whole. The destination's
/// row starts are its own; a voxel's place in the row is the source's voxel size times the column
/// (the same as the destination's but for FS-845's `reproduce`).
fn move_voxels(src: &Mri, d: &mut Mri, size: usize, dims: [i32; 3]) {
    let [xdim, ydim, zdim] = dims;
    let (width, height, depth) = (
        src.vg.width as usize,
        src.vg.height as usize,
        src.vg.depth as usize,
    );
    let (dw, dh, dd) = (
        d.vg.width as usize,
        d.vg.height as usize,
        d.vg.depth as usize,
    );
    let dbytes = d.bytes_per_vox;
    let s = src.chunk.as_ref().expect("checked");
    let dc = d.chunk.as_mut().expect("checked");
    let mut p = [0usize; 3];
    let ax = |v: i32| v.unsigned_abs() as usize - 1;
    for f in 0..src.nframes as usize {
        for zs in 0..depth {
            p[ax(zdim)] = if zdim < 0 { depth - zs - 1 } else { zs };
            for ys in 0..height {
                p[ax(ydim)] = if ydim < 0 { height - ys - 1 } else { ys };
                let srow = ((f * depth + zs) * height + ys) * width * size;
                for xs in 0..width {
                    p[ax(xdim)] = if xdim < 0 { width - xs - 1 } else { xs };
                    let [xd, yd, zd] = p;
                    let o = ((f * dd + zd) * dh + yd) * dw * dbytes + xd * size;
                    let si = srow + xs * size;
                    dc[o..o + size].copy_from_slice(&s[si..si + size]);
                }
            }
        }
    }
}

/// `MRIreorder4(mri, order)`: a new volume whose dimension `n` (columns, rows, slices, frames) is
/// the source's dimension `order[n]` (1 to 4), of the source's type, with the source's header
/// copied unchanged (`MRIcopyHeader`: voxel sizes and the voxel-to-RAS matrix stay the source's).
/// Each voxel moves by `MRIgetVoxVal` and `MRIsetVoxVal` (its float, a NaN's payload kept: the
/// official build passes the float through), the source's columns outermost and frames innermost.
/// `None` when the volume cannot be made (`mri_convert`: `error reordering all axes`, exit 1);
/// FS-937 and FS-938 above.
pub fn mri_reorder4(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    order: [i32; 4],
) -> Result<Option<Mri>, Fatal> {
    if let Some(&v) = order.iter().find(|&&v| !(1..=4).contains(&v)) {
        g.notices.add(
            &FS_937,
            format!(
                "MRIreorder4: the order value {v} is outside 1 to 4 (FreeSurfer reads stack memory for the result's extents and the voxels' places): refused"
            ),
        );
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIreorder4: order value {v} outside 1 to 4"),
        );
        return Ok(None);
    }
    let mut seen = [false; 4];
    let repeated = order
        .iter()
        .any(|&v| std::mem::replace(&mut seen[v as usize - 1], true));
    if repeated {
        g.notices.add(
            &FS_938,
            format!(
                "MRIreorder4({} {} {} {}): an order that is not a permutation (FreeSurfer makes a volume with a dimension twice{})",
                order[0],
                order[1],
                order[2],
                order[3],
                if g.notices.reproduce() {
                    ", as here"
                } else {
                    "): refused"
                }
            ),
        );
        if !g.notices.reproduce() {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIreorder4: order {} {} {} {} is not a permutation of 1 2 3 4",
                    order[0], order[1], order[2], order[3]
                ),
            );
            return Ok(None);
        }
    }
    let olddims = [mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes];
    let newdims = order.map(|v| olddims[v as usize - 1]);
    let mut result = mri_alloc_sequence(
        g, out, err, newdims[0], newdims[1], newdims[2], mri.mtype, newdims[3],
    )?;
    mri_copy_header(g, out, err, mri, Some(&mut result))?;
    for c0 in 0..olddims[0] {
        for r0 in 0..olddims[1] {
            for s0 in 0..olddims[2] {
                for f0 in 0..olddims[3] {
                    let dold = [c0, r0, s0, f0];
                    let d = order.map(|v| dold[v as usize - 1]);
                    let v = mri_get_vox_val(g, err, mri, c0, r0, s0, f0, 0)?;
                    mri_set_vox_val(g, err, &mut result, d[0], d[1], d[2], d[3], v, 0)?;
                }
            }
        }
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn vol(g: &mut Globals, w: i32, h: i32, d: i32, nf: i32, t: i32) -> Mri {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri_alloc_sequence(g, &mut o, &mut e, w, h, d, t, nf).unwrap();
        let c = m.chunk.as_mut().unwrap();
        for (i, b) in c.iter_mut().enumerate() {
            *b = (i * 7 + 3) as u8;
        }
        m
    }

    #[test]
    fn reverses_and_swaps() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let src = vol(&mut g, 3, 2, 2, 1, MRI_UCHAR);
        let r = mri_reorder(&mut g, &mut o, &mut e, &src, None, -1, 2, 3)
            .unwrap()
            .unwrap_new();
        let (a, b) = (src.chunk.as_ref().unwrap(), r.chunk.as_ref().unwrap());
        assert_eq!(&b[..3], &[a[2], a[1], a[0]]);
        let r = mri_reorder(&mut g, &mut o, &mut e, &src, None, 2, 1, 3)
            .unwrap()
            .unwrap_new();
        assert_eq!((r.vg.width, r.vg.height, r.vg.depth), (2, 3, 2));
        let b = r.chunk.as_ref().unwrap();
        // destination (y, x): source column x moves to destination row x
        assert_eq!(b[1], a[3]);
        assert_eq!(b[2], a[1]);
    }

    #[test]
    fn reorders_four_dimensions() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        // 3 columns, 2 rows, 1 slice, 2 frames; the frames become the columns
        let src = vol(&mut g, 3, 2, 1, 2, MRI_UCHAR);
        let r = mri_reorder4(&mut g, &mut o, &mut e, &src, [4, 1, 2, 3])
            .unwrap()
            .unwrap();
        assert_eq!(
            (r.vg.width, r.vg.height, r.vg.depth, r.nframes),
            (2, 3, 2, 1)
        );
        assert_eq!((r.vg.xsize, r.vg.ysize), (src.vg.xsize, src.vg.ysize));
        for (c, rr, f) in [(0, 0, 0), (2, 1, 1), (1, 0, 1)] {
            let a = mri_get_vox_val(&mut g, &mut e, &src, c, rr, 0, f, 0).unwrap();
            let b = mri_get_vox_val(&mut g, &mut e, &r, f, c, rr, 0, 0).unwrap();
            assert_eq!(a, b);
        }
        assert!(g.notices.is_empty());
    }

    #[test]
    fn reorder4_refusals() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let src = vol(&mut g, 2, 2, 1, 1, MRI_UCHAR);
            // outside 1 to 4: refused in both modes
            let r = mri_reorder4(&mut g, &mut o, &mut e, &src, [1, 2, 5, 4]).unwrap();
            assert!(r.is_none() && g.notices.contains(&FS_937));
            // a value repeated: refused by default, FreeSurfer's volume with reproduce
            let r = mri_reorder4(&mut g, &mut o, &mut e, &src, [1, 1, 3, 4]).unwrap();
            assert!(g.notices.contains(&FS_938));
            match mode {
                UpstreamDefects::Fix => assert!(r.is_none()),
                UpstreamDefects::Reproduce => {
                    let r = r.unwrap();
                    assert_eq!((r.vg.width, r.vg.height), (2, 2));
                    // (c, c) holds the last row's voxel of column c; (1, 0) stays 0
                    let last = mri_get_vox_val(&mut g, &mut e, &src, 1, 1, 0, 0, 0).unwrap();
                    let diag = mri_get_vox_val(&mut g, &mut e, &r, 1, 1, 0, 0, 0).unwrap();
                    assert_eq!(diag, last);
                    let off = mri_get_vox_val(&mut g, &mut e, &r, 1, 0, 0, 0, 0).unwrap();
                    assert_eq!(off, 0.0);
                }
            }
        }
    }

    #[test]
    fn checks_the_numbers() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let src = vol(&mut g, 2, 2, 2, 1, MRI_UCHAR);
            let r = mri_reorder(&mut g, &mut o, &mut e, &src, None, 1, 1, 3).unwrap();
            assert!(r.is_null());
            assert!(o.ends_with(b"ERROR: replicated/incorrect dimension number\n"));
            let r = mri_reorder(
                &mut g, &mut o, &mut e, &src, None, 99577, 2147399842, 2147467883,
            );
            assert!(g.notices.contains(&FS_843));
            match mode {
                UpstreamDefects::Fix => assert!(r.unwrap().is_null()),
                _ => assert!(matches!(r, Err(Fatal::Exit(139)))),
            }
        }
    }

    #[test]
    fn refuses_other_types_and_fewer_frames() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let src = vol(&mut g, 2, 2, 2, 2, MRI_SHORT);
        let mut d = vol(&mut g, 2, 2, 2, 1, MRI_SHORT);
        assert!(
            mri_reorder(&mut g, &mut o, &mut e, &src, Some(&mut d), 1, 2, 3)
                .unwrap()
                .is_null()
        );
        assert!(g.notices.contains(&FS_844));
        let mut d = vol(&mut g, 2, 2, 2, 2, MRI_FLOAT);
        assert!(
            mri_reorder(&mut g, &mut o, &mut e, &src, Some(&mut d), 1, 2, 3)
                .unwrap()
                .is_null()
        );
        assert!(g.notices.contains(&FS_845));
        let long = vol(&mut g, 1, 1, 1, 1, MRI_LONG);
        assert!(
            mri_reorder(&mut g, &mut o, &mut e, &long, None, 1, 2, 3)
                .unwrap()
                .is_null()
        );
        assert!(String::from_utf8_lossy(&e).contains("MRIreorder: unsupported voxel format 2"));
    }
}
