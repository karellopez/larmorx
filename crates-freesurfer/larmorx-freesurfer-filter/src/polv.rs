// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The planes of least variance (`utils/mripolv.cpp`, the parts `mri_segment` reaches; queue item
//! 3.2-31, track D's relay 55): the 22 directions of the half sphere (`ic_*_vertices`) and their
//! in-plane basis vectors (`init_basis_vectors` 1244), the direction whose plane through a voxel
//! varies least (`MRIcentralPlaneOfLeastVarianceNormalVoxel` 3802), the median of a plane
//! (`MRIcpolvMedianAtVoxel` 4078), the ambiguous voxels classified along that direction
//! (`MRIcpolvMedianCurveVoxel` 3940, `MRIcpolvMedianCurveSegment` 3893), the trinary intensity
//! segmentation (`MRIintensitySegmentation` 3603) and the labels' mask (`MRImaskLabels` 3768).
//!
//! FreeSurfer scales its global tables `ic_x_vertices`, `ic_y_vertices`, `ic_z_vertices` in place
//! the first time the basis vectors are made (each direction divided by its largest component's
//! magnitude); every function here runs after that, so [`Basis`] holds the scaled table.

use std::io::Write;
use std::sync::OnceLock;

use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::cstd::format_double;
use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE, diag_show_pct_done};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::mri_clone;
use larmorx_freesurfer_volume::sample::sample_volume;

use crate::classify::{MRI_AMBIGUOUS, MRI_NOT_WHITE, MRI_WHITE};

/// `NVERTICES` (`mri.h`).
pub const NVERTICES: usize = 22;

/// `ic_x_vertices`, `ic_y_vertices`, `ic_z_vertices` as FreeSurfer defines them (`mripolv.cpp`).
const IC_X: [f32; NVERTICES] = [
    0.0, 0.8944, 0.2764, -0.7236, -0.7236, 0.2764, -0.4253, -0.8507, -0.4253, 0.1625, -0.2629,
    0.5257, 0.6882, 0.1625, 0.6882, -0.2629, -0.5878, 0.0, -0.9511, -0.9511, -0.5878, 1.00,
];
const IC_Y: [f32; NVERTICES] = [
    0.0, 0.0, 0.8507, 0.5257, -0.5257, -0.8507, -0.3090, 0.0, 0.3090, -0.5000, -0.8090, 0.0,
    -0.5000, 0.5000, 0.5000, 0.8090, 0.8090, 1.0000, 0.3090, -0.3090, -0.8090, 0.00,
];
const IC_Z: [f32; NVERTICES] = [
    1.0000, 0.4472, 0.4472, 0.4472, 0.4472, 0.4472, 0.8507, 0.5257, 0.8507, 0.8507, 0.5257, 0.8507,
    0.5257, 0.8507, 0.5257, 0.5257, 0.0, 0.0, 0.0, 0.0, 0.0, 0.00,
];

/// `MAXLEN`: `MRIcpolvMedianAtVoxel`'s table of a plane's values.
pub const MAXLEN: usize = 256;

/// The directions (scaled as `init_basis_vectors` leaves `ic_*_vertices`) and the two in-plane
/// basis vectors of each (`e1_*_v`, `e2_*_v`).
#[derive(Debug)]
pub struct Basis {
    pub v: [[f32; 3]; NVERTICES],
    pub e1: [[f32; 3]; NVERTICES],
    pub e2: [[f32; 3]; NVERTICES],
}

/// `1.0f / ` the largest magnitude (`x` when larger than both, else `y` when larger than `z`, else
/// `z`).
fn inv_largest(x: f32, y: f32, z: f32) -> f32 {
    let (vx, vy, vz) = (x.abs(), y.abs(), z.abs());
    if vx > vy && vx > vz {
        1.0 / vx
    } else if vy > vz {
        1.0 / vy
    } else {
        1.0 / vz
    }
}

/// `init_basis_vectors()`: for each direction `e3` (the table's), `e3` scaled by `1 /` its
/// largest magnitude; `e1 = (e3.z, e3.x, e3.y) x ...` as FreeSurfer writes it (`vx = e3.y`, `vy =
/// e3.z`, `vz = e3.x`; `e1 = (vy e3.z - vz e3.y, vz e3.x - vx e3.z, vx e3.y - vy e3.x)`) scaled the
/// same way; `e2 = (e1.y e3.z - e1.z e3.y, e1.x e3.z - e1.z e3.x, e1.y e3.x - e1.x e3.y)` (sic: its
/// second element's sign) scaled the same way; all in `float`, `e3` unscaled in the products.
pub fn basis() -> &'static Basis {
    static B: OnceLock<Basis> = OnceLock::new();
    B.get_or_init(|| {
        let mut b = Basis {
            v: [[0.0; 3]; NVERTICES],
            e1: [[0.0; 3]; NVERTICES],
            e2: [[0.0; 3]; NVERTICES],
        };
        for k in 0..NVERTICES {
            let (e3x, e3y, e3z) = (IC_X[k], IC_Y[k], IC_Z[k]);
            let len = inv_largest(e3x, e3y, e3z);
            b.v[k] = [e3x * len, e3y * len, e3z * len];
            let (vx, vy, vz) = (e3y, e3z, e3x);
            let e1x = vy * e3z - vz * e3y;
            let e1y = vz * e3x - vx * e3z;
            let e1z = vx * e3y - vy * e3x;
            let len = inv_largest(e1x, e1y, e1z);
            let (e1x, e1y, e1z) = (e1x * len, e1y * len, e1z * len);
            b.e1[k] = [e1x, e1y, e1z];
            let e2x = e1y * e3z - e1z * e3y;
            let e2y = e1x * e3z - e1z * e3x;
            let e2z = e1y * e3x - e1x * e3y;
            let len = inv_largest(e2x, e2y, e2z);
            b.e2[k] = [e2x * len, e2y * len, e2z * len];
        }
        b
    })
}

/// `FZERO(f)` of a `float` (C++'s `fabs(float)`): `|f| < FLT_EPSILON`.
fn fzero(f: f32) -> bool {
    f.abs() < f32::EPSILON
}

/// `MRIvox(mri, x, y, z)` through the index tables (`mri->xi[...]`): frame 0's byte.
fn vox(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    let xi = Mri::index(mri.xi.as_deref().unwrap_or(&[]), x);
    let yi = Mri::index(mri.yi.as_deref().unwrap_or(&[]), y);
    let zi = Mri::index(mri.zi.as_deref().unwrap_or(&[]), z);
    mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)
}

/// `MRIcentralPlaneOfLeastVarianceNormalVoxel(mri, wsize, x, y, z)`: over the directions in
/// order, the plane through `(x, y, z)` spanned by `e1` and `e2` sampled at `wsize x wsize`
/// points (`yk` outer, `xk` inner, each `-whalf` to `whalf`; the point `nint((float) x + (float) yk
/// e2.x + xk e1.x)` and the others in `float`, through the index tables, its byte); the plane's
/// variance `total_sq / n - mean^2` in `float`; the first direction of the smallest variance
/// (strictly below 100,000), stopping at the first whose variance is `FZERO`. -1 when none.
pub fn central_plane_of_least_variance_normal_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    let b = basis();
    let whalf = (wsize - 1) / 2;
    let mut min_vertex = -1i32;
    let mut min_var = 100000.0f32;
    for vertex in 0..NVERTICES {
        let [e1x, e1y, e1z] = b.e1[vertex];
        let [e2x, e2y, e2z] = b.e2[vertex];
        let (mut total, mut total_sq, mut nv) = (0f32, 0f32, 0f32);
        for yk in -whalf..=whalf {
            let xbase = x as f32 + yk as f32 * e2x;
            let ybase = y as f32 + yk as f32 * e2y;
            let zbase = z as f32 + yk as f32 * e2z;
            for xk in -whalf..=whalf {
                let xi = nint(g, f64::from(xbase + xk as f32 * e1x));
                let yi = nint(g, f64::from(ybase + xk as f32 * e1y));
                let zi = nint(g, f64::from(zbase + xk as f32 * e1z));
                let val = vox(g, err, mri, xi, yi, zi)?;
                total += val;
                total_sq += val * val;
                nv += 1.0;
            }
        }
        let varv = if nv == 0.0 {
            0.0
        } else {
            let avgv = total / nv;
            total_sq / nv - avgv * avgv
        };
        if varv < min_var {
            min_var = varv;
            min_vertex = vertex as i32;
        }
        if fzero(varv) {
            break;
        }
    }
    Ok(min_vertex)
}

/// `MRIcpolvMedianAtVoxel(mri, vertex, x, y, z, wsize)`: the plane of direction `vertex` through
/// the point sampled at `wsize x wsize` points (`yk` outer: `(float) x + (float) yk e2.x`, then `+
/// xk e1.x` in `float`, as a `double`) by `MRIsampleVolume` (trilinear), sorted (glibc's `qsort`
/// with `c1 > c2: 1, c1 < c2: -1, else 0`), the element `wsize^2 / 2`. More than [`MAXLEN`]
/// samples overrun FreeSurfer's table: the caller refuses such a window.
#[allow(clippy::too_many_arguments)]
pub fn cpolv_median_at_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    vertex: usize,
    x: f32,
    y: f32,
    z: f32,
    wsize: i32,
) -> f32 {
    let b = basis();
    let [e1x, e1y, e1z] = b.e1[vertex];
    let [e2x, e2y, e2z] = b.e2[vertex];
    let whalf = (wsize - 1) / 2;
    let mut vals: Vec<f32> = Vec::with_capacity((wsize.max(0) * wsize.max(0)) as usize);
    for yk in -whalf..=whalf {
        let xbase = x + yk as f32 * e2x;
        let ybase = y + yk as f32 * e2y;
        let zbase = z + yk as f32 * e2z;
        for xk in -whalf..=whalf {
            let xr = f64::from(xbase + xk as f32 * e1x);
            let yr = f64::from(ybase + xk as f32 * e1y);
            let zr = f64::from(zbase + xk as f32 * e1z);
            let mut rval = 0f64;
            sample_volume(g, err, mri, xr, yr, zr, &mut rval);
            vals.push(rval as f32);
        }
    }
    qsort(&mut vals, |a: &f32, b: &f32| {
        if a > b {
            1i32
        } else if a < b {
            -1
        } else {
            0
        }
    });
    let k = (wsize * wsize / 2) as usize;
    vals.get(k).copied().unwrap_or(0.0)
}

/// `MRIcpolvMedianCurveVoxel(mri, labeled, x0, y0, z0, wsize, len, gray_hi, wm_low)`: the voxel's
/// label from the medians of the planes along the normal of its plane of least variance.
#[allow(clippy::too_many_arguments)]
pub fn cpolv_median_curve_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    wsize: i32,
    len: f32,
    gray_hi: f32,
    wm_low: f32,
) -> Result<i32, Fatal> {
    let b = basis();
    let vertex = central_plane_of_least_variance_normal_voxel(g, err, mri, wsize, x0, y0, z0)?;
    let vi = usize::try_from(vertex).unwrap_or(0);
    let [mut nx, mut ny, mut nz] = b.v[vi];
    let dist = f64::from(nx * nx + ny * ny + nz * nz).sqrt() as f32;
    nx /= dist;
    ny /= dist;
    nz /= dist;
    let (mut white_dist, mut gray_dist) = (1000.0f32, 1000.0f32);
    let mut what = MRI_AMBIGUOUS;
    let (mut gray_val, mut white_val, mut val) = (-1.0f32, -1.0f32, -1.0f32);
    let mut medians: Vec<f32> = Vec::new();
    let mut dists: Vec<f32> = Vec::new();
    let diag = g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0;
    let mut dist = -len;
    while dist <= len {
        let x = x0 as f32 + nx * dist;
        let y = y0 as f32 + ny * dist;
        let z = z0 as f32 + nz * dist;
        let median = cpolv_median_at_voxel(g, err, mri, vi, x, y, z, wsize);
        medians.push(median);
        dists.push(dist);
        if median >= gray_hi && dist.abs() < white_dist {
            white_val = median;
            white_dist = dist.abs();
        } else if median <= wm_low && dist.abs() < gray_dist {
            gray_val = median;
            gray_dist = dist.abs();
        }
        if fzero(dist) {
            val = median;
            if median >= gray_hi {
                what = MRI_WHITE;
            } else if median <= wm_low {
                what = MRI_NOT_WHITE;
            }
            if !diag && what != MRI_AMBIGUOUS {
                return Ok(what);
            }
        }
        dist += 0.25;
    }
    if diag {
        let mut text = String::new();
        for (d, m) in dists.iter().zip(&medians) {
            text.push_str(&format!(
                "{}  {}\n",
                larmorx_freesurfer_core::cstd::fmt_f(f64::from(*d), 2, 2),
                larmorx_freesurfer_core::cstd::fmt_f(f64::from(*m), 2, 3)
            ));
        }
        let _ = std::fs::write("med.plt", text);
    }
    if white_dist > len {
        for (m, d) in medians.iter().zip(&dists) {
            if *m > white_val && *m > wm_low {
                white_val = *m;
                white_dist = d.abs();
            }
        }
        if fzero(white_dist) {
            what = MRI_WHITE;
        }
    }
    if gray_dist > len {
        gray_val = 10000.0;
        for (m, d) in medians.iter().zip(&dists) {
            if *m < gray_val && *m < gray_hi {
                gray_val = *m;
                gray_dist = d.abs();
            }
        }
        if fzero(gray_dist) {
            what = MRI_NOT_WHITE;
        }
    }
    if what == MRI_AMBIGUOUS {
        if f64::from(white_dist) < f64::from(gray_dist) - 0.75 {
            what = MRI_WHITE;
        } else if f64::from(gray_dist) < f64::from(white_dist) - 0.75 {
            what = MRI_NOT_WHITE;
        }
    }
    if what == MRI_AMBIGUOUS {
        if f64::from(white_dist) > 2.0 {
            what = MRI_NOT_WHITE;
        } else {
            let white_grad = (white_val - val).abs();
            let gray_grad = (gray_val - val).abs();
            what = if white_grad > gray_grad {
                MRI_NOT_WHITE
            } else {
                MRI_WHITE
            };
        }
    }
    Ok(what)
}

/// `%2.2f%%` of `100.0f * (float) n / (float) total`.
fn pct(n: i32, total: i32) -> String {
    larmorx_freesurfer_core::cstd::fmt_f(f64::from(100.0f32 * n as f32 / total as f32), 2, 2)
}

/// `MRIcpolvMedianCurveSegment(mri, labeled, dst, wsize, len, gray_hi, wm_low)`: every voxel of
/// `labeled` that is ambiguous (128) relabelled by [`cpolv_median_curve_voxel`] in a copy of it
/// (slices outer, columns inner); the counts printed.
#[allow(clippy::too_many_arguments)]
pub fn cpolv_median_curve_segment(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    labeled: &Mri,
    wsize: i32,
    len: f32,
    gray_hi: f32,
    wm_low: f32,
) -> Result<Mri, Fatal> {
    let g4 = |v: f32| format_double("%g", f64::from(v)).unwrap_or_default();
    let _ = writeln!(
        out,
        "MRIcpolvMedianCurveSegment(): wsize={wsize}, len={}, gmhi={}, wmlow={}",
        g4(len),
        g4(gray_hi),
        g4(wm_low)
    );
    let mut dst = mri_copy(g, out, err, labeled, None)?.unwrap_new();
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let (mut non, mut noff, mut nlabeled) = (0i32, 0i32, 0i32);
    for z in 0..depth {
        diag_show_pct_done(g, err, z as f32 / (depth - 1) as f32, 5);
        for y in 0..height {
            for x in 0..width {
                let label = vox(g, err, labeled, x, y, z)? as i32;
                if label != MRI_AMBIGUOUS {
                    continue;
                }
                nlabeled += 1;
                let label =
                    cpolv_median_curve_voxel(g, err, mri, x, y, z, wsize, len, gray_hi, wm_low)?;
                if label == MRI_WHITE {
                    non += 1;
                } else {
                    noff += 1;
                }
                mri_set_vox_val(g, err, &mut dst, x, y, z, 0, label as f32, 0)?;
            }
        }
    }
    let total = width.wrapping_mul(height).wrapping_mul(depth);
    let _ = writeln!(
        out,
        "  {nlabeled:8} voxels processed ({}%)",
        pct(nlabeled, total)
    );
    let _ = writeln!(out, "  {non:8} voxels white ({}%)", pct(non, total));
    let _ = writeln!(out, "  {noff:8} voxels non-white ({}%)", pct(noff, total));
    Ok(dst)
}

/// `MRIintensitySegmentation(src, min_wm_mask, labeled, wm_low, wm_hi, gray_hi)`: each voxel of
/// frame 0 (slices outer, columns inner) labelled 1 below `wm_low` or above `wm_hi`, 128 up to
/// `gray_hi`, else 255 (`float` comparisons); with a minimal white-matter mask only the mask's
/// voxels above 0.5 are set (to 255) and the others left. Into `labeled` (a clone of `src` when
/// none is given); the counts printed.
#[allow(clippy::too_many_arguments)]
pub fn intensity_segmentation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    min_wm_mask: Option<&Mri>,
    labeled: Option<&mut Mri>,
    wm_low: f32,
    wm_hi: f32,
    gray_hi: f32,
) -> Result<Ret<Mri>, Fatal> {
    let g4 = |v: f32| format_double("%g", f64::from(v)).unwrap_or_default();
    let _ = writeln!(
        out,
        "MRIintensitySegmentation() wm_low={}, wm_hi={}, gray_hi={}",
        g4(wm_low),
        g4(wm_hi),
        g4(gray_hi)
    );
    let mut fresh = None;
    let dst: &mut Mri = match labeled {
        Some(d) => d,
        None => fresh.insert(mri_clone(g, out, err, src, None)?.unwrap_new()),
    };
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    let (mut nwhite, mut nblack, mut nambiguous, mut nmask) = (0i32, 0i32, 0i32, 0i32);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                if let Some(mask) = min_wm_mask {
                    let m = f64::from(mri_get_vox_val(g, err, mask, x, y, z, 0, 0)?);
                    if m > 0.5 {
                        mri_set_vox_val(g, err, dst, x, y, z, 0, MRI_WHITE as f32, 0)?;
                        nmask += 1;
                        nwhite += 1;
                    }
                    continue;
                }
                let val = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                let label = if val < wm_low || val > wm_hi {
                    nblack += 1;
                    MRI_NOT_WHITE
                } else if val <= gray_hi {
                    nambiguous += 1;
                    MRI_AMBIGUOUS
                } else {
                    nwhite += 1;
                    MRI_WHITE
                };
                mri_set_vox_val(g, err, dst, x, y, z, 0, label as f32, 0)?;
            }
        }
    }
    let _ = writeln!(
        out,
        "white = {nwhite}, nonwhite = {nblack}, ambig = {nambiguous}, nmask = {nmask}"
    );
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRImaskLabels(src, mask, NULL)`: each voxel (slices outer) 128 where the mask is 128, the
/// source's value where it is 255, else 0 (`float` comparisons), into a clone of `src`.
pub fn mask_labels(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    mask: &Mri,
) -> Result<Mri, Fatal> {
    let mut dst = mri_clone(g, out, err, src, None)?.unwrap_new();
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let m = mri_get_vox_val(g, err, mask, x, y, z, 0, 0)?;
                let s = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                mri_set_vox_val(g, err, &mut dst, x, y, z, 0, masked(m, s), 0)?;
            }
        }
    }
    Ok(dst)
}

/// `MRImaskLabels(dst, mask, dst)`: [`mask_labels`] in place (each voxel read before it is
/// written).
pub fn mask_labels_in_place(
    g: &mut Globals,
    err: &mut dyn Write,
    dst: &mut Mri,
    mask: &Mri,
) -> Result<(), Fatal> {
    let (width, height, depth) = (dst.vg.width, dst.vg.height, dst.vg.depth);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let m = mri_get_vox_val(g, err, mask, x, y, z, 0, 0)?;
                let s = mri_get_vox_val(g, err, dst, x, y, z, 0, 0)?;
                mri_set_vox_val(g, err, dst, x, y, z, 0, masked(m, s), 0)?;
            }
        }
    }
    Ok(())
}

/// A voxel of `MRImaskLabels`: 128 for a mask of 128, the source for 255, else 0.
fn masked(m: f32, s: f32) -> f32 {
    if m == MRI_AMBIGUOUS as f32 {
        MRI_AMBIGUOUS as f32
    } else if m == MRI_WHITE as f32 {
        s
    } else {
        0.0
    }
}
