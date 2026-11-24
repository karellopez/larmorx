// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The planes of least variance (`utils/mripolv.cpp`, the parts `mri_segment` reaches; queue item
//! 3.2-31, track D's relay 55): the 22 directions of the half sphere (`ic_*_vertices`) and their
//! in-plane basis vectors (`init_basis_vectors` 1244), the direction whose plane through a voxel
//! varies least (`MRIcentralPlaneOfLeastVarianceNormalVoxel` 3802), the median of a plane
//! (`MRIcpolvMedianAtVoxel` 4078), the ambiguous voxels classified along that direction
//! (`MRIcpolvMedianCurveVoxel` 3940, `MRIcpolvMedianCurveSegment` 3893), the trinary intensity
//! segmentation (`MRIintensitySegmentation` 3603) and the labels' mask (`MRImaskLabels` 3768);
//! relay 56: the thin strands thickened (`MRIthickenThinWMStrands`, the compiled branch at 1670),
//! the islands removed (`MRIremoveIslands` 4143), the planar holes filled (`MRIfillPlanarHoles`
//! 4187) with `MRIcpolvMaxWhiteAtVoxel` (4261) and `MRIcpolvAllQuadrantsFilled` (4327, its
//! compiled branch).
//!
//! Defects: FS-1216 (more than 200 positions along the normal), FS-1217 and FS-1218 (a plane's
//! window whose values FreeSurfer sorts unwritten or past its table), FS-1225 (labels of a
//! volume not of bytes read and written byte by byte).
//!
//! FreeSurfer scales its global tables `ic_x_vertices`, `ic_y_vertices`, `ic_z_vertices` in place
//! the first time the basis vectors are made (each direction divided by its largest component's
//! magnitude); every function here runs after that, so [`Basis`] holds the scaled table.

use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use larmorx_core::defects::{FS_1216, FS_1217, FS_1218, FS_1225};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::cstd::format_double;
use larmorx_freesurfer_core::diag::DIAG_SHOW;
use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE, diag_show_pct_done};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::{nint, resolve};
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, Tri, mri_binarize, mri_clear, mri_clone};
use larmorx_freesurfer_volume::coords::index_not_in_volume;
use larmorx_freesurfer_volume::deriv::neighbors_on;
use larmorx_freesurfer_volume::labels::WM_MIN_VAL;
use larmorx_freesurfer_volume::mriset::{mri_close, mri_dilate, mri_xor};
use larmorx_freesurfer_volume::sample::sample_volume;

use crate::classify::{MRI_AMBIGUOUS, MRI_NOT_WHITE, MRI_WHITE};
use crate::mem;
use crate::segment::{
    mri_segment, mri_segment_clear_ignore_flags, mri_segment_dilate, mri_segment_max,
    mri_segment_to_image,
};

/// The run's environment and working folder: what the diagnostic files (`DIAG_WRITE` with
/// `DIAG_VERBOSE`) are written with.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub env: &'a Environment,
    pub dir: Option<&'a Path>,
}

/// `THICKEN_FILL`, `NBHD_FILL` (`include/mri.h`).
pub const THICKEN_FILL: i32 = 200;
pub const NBHD_FILL: i32 = 210;
/// `TOO_THIN` (`mripolv.cpp`).
const TOO_THIN: i32 = 2;
/// The positions along the normal FreeSurfer's tables hold (`medians[200]`, `dists[200]`).
pub const MAX_MEDIANS: usize = 200;

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

/// `MRIgetVoxVal(m, x, y, z, 0)`: a voxel of a byte volume read directly (the same value), any
/// other through [`mri_get_vox_val`]. For a byte volume it is also `MRIvox(m, x, y, z)`.
#[inline]
pub fn vox_val(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    if m.mtype == MRI_UCHAR
        && x >= 0
        && y >= 0
        && z >= 0
        && x < m.vg.width
        && y < m.vg.height
        && z < m.vg.depth
        && let Some(c) = m.chunk.as_ref()
    {
        let o =
            (z as usize * m.vg.height as usize + y as usize) * m.vox_per_row as usize + x as usize;
        if let Some(b) = c.get(o) {
            return Ok(f32::from(*b));
        }
    }
    mri_get_vox_val(g, err, m, x, y, z, 0, 0)
}

/// `m->xi[x]`, `m->yi[y]`, `m->zi[z]`: the boundary index tables.
#[inline]
pub fn tables(m: &Mri, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    (
        Mri::index(m.xi.as_deref().unwrap_or(&[]), x),
        Mri::index(m.yi.as_deref().unwrap_or(&[]), y),
        Mri::index(m.zi.as_deref().unwrap_or(&[]), z),
    )
}

/// `MRIgetVoxVal(mri, mri->xi[x], mri->yi[y], mri->zi[z], 0)` (and `MRIvox` of a byte volume).
#[inline]
fn vox(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    let (xi, yi, zi) = tables(mri, x, y, z);
    vox_val(g, err, mri, xi, yi, zi)
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
/// the point sampled at the `(2 whalf + 1)^2` points of its window (`whalf = (wsize - 1) / 2`;
/// `yk` outer: `(float) x + (float) yk e2.x`, then `+ xk e1.x` in `float`, as a `double`) by
/// `MRIsampleVolume` (trilinear); FreeSurfer then sorts `wsize^2` values of its table (glibc's
/// `qsort` with `c1 > c2: 1, c1 < c2: -1, else 0`) and returns the element `wsize^2 / 2`. For an
/// even or negative `wsize` the table holds fewer samples than it sorts: the rest is memory
/// FreeSurfer never wrote, read here as 0 (FS-1217, `reproduce` only: the program refuses such a
/// window by default). More than [`MAXLEN`] values overrun the table: the caller refuses such a
/// window (FS-1218).
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
    let n = wsize.wrapping_mul(wsize).max(0) as usize;
    let mut vals: Vec<f32> = Vec::with_capacity(n.max(1));
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
    if vals.len() < n.max(1) {
        g.notices.add(
            &FS_1217,
            format!(
                "MRIcpolvMedianAtVoxel: a window of {wsize} sorts {n} values of which {} were sampled: the rest, memory FreeSurfer never wrote, read as 0",
                vals.len()
            ),
        );
        vals.resize(n.max(1), 0.0);
    }
    qsort(&mut vals[..n], |a: &f32, b: &f32| {
        if a > b {
            1i32
        } else if a < b {
            -1
        } else {
            0
        }
    });
    vals[n / 2]
}

/// `MRIcpolvMedianCurveVoxel(mri, labeled, x0, y0, z0, wsize, len, gray_hi, wm_low)`: the voxel's
/// label from the medians of the planes along the normal of its plane of least variance (from
/// `-len` to `len` in steps of `0.25f`, in `float`). FreeSurfer's tables hold [`MAX_MEDIANS`]
/// positions and a window of at most [`MAXLEN`] values: a voxel that needs more ends the run with
/// an error (FS-1216, FS-1218: FreeSurfer writes past them; the program refuses such settings by
/// default before it starts).
#[allow(clippy::too_many_arguments)]
pub fn cpolv_median_curve_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    ctx: Ctx,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    wsize: i32,
    len: f32,
    gray_hi: f32,
    wm_low: f32,
) -> Result<i32, Fatal> {
    if wsize.unsigned_abs() > 16 {
        g.notices.add(
            &FS_1218,
            format!(
                "MRIcpolvMedianCurveVoxel: a plane window of {wsize} needs {} values: FreeSurfer's table holds {MAXLEN} (it writes or reads past it)",
                i64::from(wsize) * i64::from(wsize)
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIcpolvMedianCurveVoxel: the plane window {wsize} holds more than {MAXLEN} values"
            ),
        )?;
    }
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
        if medians.len() == MAX_MEDIANS {
            g.notices.add(
                &FS_1216,
                format!(
                    "MRIcpolvMedianCurveVoxel: -polvlen {} takes more than {MAX_MEDIANS} positions along the normal: FreeSurfer writes past its tables",
                    format_double("%g", f64::from(len)).unwrap_or_default()
                ),
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "MRIcpolvMedianCurveVoxel: more than {MAX_MEDIANS} positions along the normal (len {})",
                    format_double("%g", f64::from(len)).unwrap_or_default()
                ),
            )?;
        }
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
        let _ = std::fs::write(resolve(ctx.dir, "med.plt"), text);
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

/// FS-1225: a labels volume not of bytes, read and written byte by byte (`MRIvox`).
const LABEL_BYTES: mem::Notes = (&FS_1225, &FS_1225);

/// `MRIcpolvMedianCurveSegment(mri, labeled, dst, wsize, len, gray_hi, wm_low)`: every voxel of
/// `labeled` that is ambiguous (128) relabelled by [`cpolv_median_curve_voxel`] in a copy of it
/// (slices outer, columns inner; the labels read and written as bytes, `MRIvox`, whatever the
/// type: FS-1225); the counts printed.
#[allow(clippy::too_many_arguments)]
pub fn cpolv_median_curve_segment(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: Ctx,
    mri: &Mri,
    labeled: &Mri,
    wsize: i32,
    len: f32,
    gray_hi: f32,
    wm_low: f32,
) -> Result<Mri, Fatal> {
    const WHAT: &str = "MRIcpolvMedianCurveSegment";
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
                let label = byte_read(g, WHAT, labeled, x, y, z) as i32;
                if label != MRI_AMBIGUOUS {
                    continue;
                }
                nlabeled += 1;
                let label = cpolv_median_curve_voxel(
                    g, err, ctx, mri, x, y, z, wsize, len, gray_hi, wm_low,
                )?;
                if label == MRI_WHITE {
                    non += 1;
                } else {
                    noff += 1;
                }
                let (zl, yl, xl) = (i64::from(z), i64::from(y), i64::from(x));
                mem::write(g, LABEL_BYTES, WHAT, &mut dst, zl, yl, xl, &[label as u8]);
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
                let val = vox_val(g, err, src, x, y, z)?;
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
                let m = vox_val(g, err, mask, x, y, z)?;
                let s = vox_val(g, err, src, x, y, z)?;
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
                let m = vox_val(g, err, mask, x, y, z)?;
                let s = vox_val(g, err, dst, x, y, z)?;
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

// ---- relay 56: the thin strands, the islands and the planar holes ------------------------------

/// `MRIwrite(m, fname)` of a diagnostic volume (its status not looked at).
fn write_diag(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: Ctx,
    m: &mut Mri,
    fname: &str,
) -> Result<(), Fatal> {
    mri_write(
        g,
        out,
        err,
        ctx.env,
        ctx.dir,
        m,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(())
}

fn diag_write_verbose(g: &Globals) -> bool {
    g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0
}

/// `MRIremoveIslands(src, NULL, wsize, thresh)`: a clone of the source (zeros) holding, at each
/// voxel of the slices from `whalf = wsize / 2` on, of their rows from `whalf` on (every column),
/// the source's value, set to 0 when it is not 0 and at least `thresh` of the `(2 whalf + 1)^3`
/// voxels around it (the index tables; the voxel itself included) are 0, or to `THICKEN_FILL`
/// (200) when it is 0 and at least `thresh` of them are not; the slices and rows before `whalf`
/// stay 0.
pub fn remove_islands(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    wsize: i32,
    thresh: i32,
) -> Result<Mri, Fatal> {
    let mut dst = mri_clone(g, out, err, src, None)?.unwrap_new();
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    let whalf = wsize / 2;
    for z in whalf..depth {
        for y in whalf..height {
            for x in 0..width {
                let (mut num_on, mut num_off) = (0i32, 0i32);
                for z0 in -whalf..=whalf {
                    for y0 in -whalf..=whalf {
                        for x0 in -whalf..=whalf {
                            if vox(g, err, src, x + x0, y + y0, z + z0)? == 0.0 {
                                num_off += 1;
                            } else {
                                num_on += 1;
                            }
                        }
                    }
                }
                let mut val = vox_val(g, err, src, x, y, z)?;
                if val != 0.0 && num_off >= thresh {
                    val = 0.0;
                } else if val == 0.0 && num_on >= thresh {
                    val = THICKEN_FILL as f32;
                }
                mri_set_vox_val(g, err, &mut dst, x, y, z, 0, val, 0)?;
            }
        }
    }
    Ok(dst)
}

/// `MRIsampleVolume(m, x, y, z, &val)` of a point (trilinear).
fn sample(g: &mut Globals, err: &mut dyn Write, m: &Mri, x: f64, y: f64, z: f64) -> f64 {
    let mut val = 0f64;
    sample_volume(g, err, m, x, y, z, &mut val);
    val
}

/// The search of `MRIthickenThinWMStrands` along its one direction (`nx = nz = 0`, `ny = 1`: the
/// rows), `sign` -1 downwards and 1 upwards: `nd` a `float` from 0 while `nd <= limit`, the point
/// `(x, y + sign nd, z)` sampled; the first below `WM_MIN_VAL` gives `nd - 0.5`, none `none`.
#[allow(clippy::too_many_arguments)]
fn strand_search(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
    sign: f64,
    limit: f32,
    none: f64,
) -> f64 {
    let mut nd = 0f32;
    while nd <= limit {
        // (double) x - nd * nx with nx 0, (double) y - nd * ny with ny 1
        let yf = f64::from(y) + sign * f64::from(nd);
        if sample(g, err, m, f64::from(x), yf, f64::from(z)) < f64::from(WM_MIN_VAL) {
            return f64::from(nd) - 0.5;
        }
        nd += 1.0;
    }
    none
}

/// `MRIthickenThinWMStrands(mri_T1, mri, mri, thickness, nsegments, wm_hi)` as `mri_segment` calls
/// it (the source its own destination: FreeSurfer's copy into it does nothing, and its later reads
/// of the source see what it has thickened). The compiled branch (`mripolv.cpp` 1670):
/// `MRIthickenThinWMStrands(): thickness=<t>, nsegments=<n>` on standard output; the islands
/// removed (`MRIremoveIslands(mri, NULL, 3, 24)`) and closed (`MRIclose`); the voxels above 0
/// thin along the rows (the only direction FreeSurfer allows: `nx = nz = 0`, `ny = 1`), the
/// first voxel below `WM_MIN_VAL` downwards and upwards within `thickness + 1` of the closed
/// volume (`down + up <= thickness`, each `nd - 0.5` or `thickness + 1`), marked 1 in a clone;
/// their connected segments (`MRIsegment(thin, 1, 255)`) dilated five times into the volume
/// (`MRIsegmentDilate`); then for the `nsegments` largest (`MRIsegmentMax`, each then ignored):
/// the segment's image of the closed volume (`MRIsegmentToImage` into the cleared clone); each of
/// its voxels at or above `WM_MIN_VAL` searched again in the volume within `TOO_THIN + 1` (3) and,
/// while `up + down <= TOO_THIN` (2), thickened on the nearer side (both when equal): the voxel
/// beyond the first non-white one (`up + 1.5`, `nint`) must lie in the volume (else `ERROR:
/// MRIthickenThinWMStrands: invalid x,y,z!` and the voxel's search ends) and be 0 (upwards also
/// inside the T1's extent: `MRIindexNotInVolume`); then the non-white one (`up + 0.5`) is set to
/// `THICKEN_FILL` (200) in the volume and the segment's image when the T1 there is below `wm_hi`;
/// each side's distance grows by 1 when it was considered; then the holes between the segment's
/// voxels filled (`NBHD_FILL`, 210: a voxel inside the volume's border that is 0 with a T1 below
/// `wm_hi`, between two of the segment's voxels along a row, a column or a slice, with no voxel of
/// the volume outside the segment among its 26 neighbours) until none is, and its planar holes
/// ([`fill_planar_holes_in_place`]); finally `  <n> segments, <filled> filled` (`%2d`) on
/// standard output and the ignore flags cleared. With `DIAG_WRITE` and `DIAG_VERBOSE`:
/// `seg<i>.mgh`, `dilated_seg<i>.mgh` and `thin.mgh`.
#[allow(clippy::too_many_arguments)]
pub fn thicken_thin_wm_strands_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: Ctx,
    t1: &Mri,
    dst: &mut Mri,
    thickness: i32,
    nsegments: i32,
    wm_hi: f32,
) -> Result<(), Fatal> {
    let _ = writeln!(
        out,
        "MRIthickenThinWMStrands(): thickness={thickness}, nsegments={nsegments}"
    );
    let (width, height, depth) = (dst.vg.width, dst.vg.height, dst.vg.depth);
    let _ = basis();
    let mut thin = mri_clone(g, out, err, dst, None)?.unwrap_new();
    let mut tmp = remove_islands(g, out, err, dst, 3, 27 - 3)?;
    mri_close(g, out, err, Pair::Same(&mut tmp))?;
    // MRIcopy(mri_src, mri_dst): the same volume, nothing copied
    let limit = thickness.wrapping_add(1) as f32;
    let none = f64::from(thickness.wrapping_add(1));
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                if vox_val(g, err, dst, x, y, z)? > 0.0 {
                    let down = strand_search(g, err, &tmp, x, y, z, -1.0, limit, none);
                    let up = strand_search(g, err, &tmp, x, y, z, 1.0, limit, none);
                    if up + down <= f64::from(thickness) {
                        mri_set_vox_val(g, err, &mut thin, x, y, z, 0, 1.0, 0)?;
                    }
                }
            }
        }
    }

    // now thicken the strands, careful not to connect them with others
    let mut total_filled = 0i32;
    let mut mriseg = mri_segment(g, out, err, &thin, 1.0, 255.0)?;
    if diag_write_verbose(g) {
        let mut img: Option<Mri> = None;
        for _ in 0..nsegments {
            let i = mri_segment_max(&mriseg);
            if i < 0 {
                break;
            }
            img = segment_image(g, out, err, dst, img, &mriseg, i)?;
            if let Some(m) = img.as_mut() {
                write_diag(g, out, err, ctx, m, &format!("seg{i}.mgh"))?;
            }
        }
    }
    for _ in 0..5 {
        mri_segment_dilate(g, out, err, &mut mriseg, dst)?;
    }
    if diag_write_verbose(g) {
        let mut img: Option<Mri> = None;
        for _ in 0..nsegments {
            let i = mri_segment_max(&mriseg);
            if i < 0 {
                break;
            }
            img = segment_image(g, out, err, dst, img, &mriseg, i)?;
            if let Some(m) = img.as_mut() {
                write_diag(g, out, err, ctx, m, &format!("dilated_seg{i}.mgh"))?;
            }
        }
    }

    let mut nseg = 0i32;
    while nseg < nsegments {
        let i = mri_segment_max(&mriseg);
        if i < 0 {
            break;
        }
        mri_clear(g, err, &mut thin)?;
        mri_segment_to_image(g, out, err, &tmp, Some(&mut thin), &mriseg, i)?;
        let mut nfilled = 0i32;
        let nvox = mriseg.segments[i as usize].voxels.len();
        for v in 0..nvox {
            let p = mriseg.segments[i as usize].voxels[v];
            let (x, y, z) = (i32::from(p.x), i32::from(p.y), i32::from(p.z));
            if vox_val(g, err, dst, x, y, z)? < WM_MIN_VAL as f32 {
                continue;
            }
            nfilled += thicken_voxel(g, err, t1, dst, &mut thin, x, y, z, wm_hi)?;
        }
        total_filled += nfilled;
        if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
            let _ = writeln!(
                err,
                "{nseg}: segment {i}, {} voxels, filled = {nfilled}, total = {total_filled}",
                mriseg.segments[i as usize].voxels.len()
            );
        }
        mriseg.segments[i as usize].ignore = 1;

        // the holes between the strand's voxels: a voxel filled lies between two of the
        // segment's, so only the box around them (inside the volume's border) is looked at; the
        // fills stay inside it
        let bx = nonzero_box(g, err, &thin)?;
        loop {
            let mut nfilled = 0i32;
            let Some(((x0, y0, z0), (x1, y1, z1))) = bx else {
                break;
            };
            for z in z0.max(1)..=z1.min(depth - 2) {
                for y in y0.max(1)..=y1.min(height - 2) {
                    for x in x0.max(1)..=x1.min(width - 2) {
                        if nbhd_fill(g, err, t1, dst, &thin, x, y, z, wm_hi)? {
                            mri_set_vox_val(g, err, dst, x, y, z, 0, NBHD_FILL as f32, 0)?;
                            mri_set_vox_val(g, err, &mut thin, x, y, z, 0, NBHD_FILL as f32, 0)?;
                            nfilled += 1;
                        }
                    }
                }
            }
            total_filled += nfilled;
            if nfilled <= 0 {
                break;
            }
        }
        fill_planar_holes_in_place(g, out, err, ctx, dst, &thin)?;
        nseg += 1;
    }
    let _ = writeln!(out, "  {nseg:2} segments, {total_filled} filled");
    mri_segment_clear_ignore_flags(&mut mriseg);
    drop(mriseg);
    if diag_write_verbose(g) {
        write_diag(g, out, err, ctx, &mut thin, "thin.mgh")?;
    }
    Ok(())
}

/// `mri_tmp = MRIsegmentToImage(mri_src, mri_tmp, mriseg, i)` (a clone the first time).
fn segment_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    img: Option<Mri>,
    mriseg: &crate::segment::MriSegmentation,
    i: i32,
) -> Result<Option<Mri>, Fatal> {
    match img {
        Some(mut m) => {
            mri_segment_to_image(g, out, err, src, Some(&mut m), mriseg, i)?;
            Ok(Some(m))
        }
        None => Ok(mri_segment_to_image(g, out, err, src, None, mriseg, i)?.into_new()),
    }
}

/// The smallest box holding every voxel of frame 0 that is not 0 (`None` when there is none).
type Box3 = ((i32, i32, i32), (i32, i32, i32));
fn nonzero_box(g: &mut Globals, err: &mut dyn Write, m: &Mri) -> Result<Option<Box3>, Fatal> {
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let (mut lo, mut hi) = (
        (i32::MAX, i32::MAX, i32::MAX),
        (i32::MIN, i32::MIN, i32::MIN),
    );
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if vox_val(g, err, m, x, y, z)? != 0.0 {
                    lo = (lo.0.min(x), lo.1.min(y), lo.2.min(z));
                    hi = (hi.0.max(x), hi.1.max(y), hi.2.max(z));
                }
            }
        }
    }
    Ok((lo.0 <= hi.0).then_some((lo, hi)))
}

/// One voxel of a segment of `MRIthickenThinWMStrands`: the voxels it fills.
#[allow(clippy::too_many_arguments)]
fn thicken_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    t1: &Mri,
    dst: &mut Mri,
    thin: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
    wm_hi: f32,
) -> Result<i32, Fatal> {
    let too = f64::from(TOO_THIN);
    let limit = (TOO_THIN + 1) as f32;
    let none = f64::from(TOO_THIN + 1);
    let mut down_dist = strand_search(g, err, dst, x, y, z, -1.0, limit, none);
    let mut up_dist = strand_search(g, err, dst, x, y, z, 1.0, limit, none);
    let mut thin_now = up_dist + down_dist <= too;
    let mut nfilled = 0i32;
    let (w, h, d) = (dst.vg.width, dst.vg.height, dst.vg.depth);
    let outside =
        |xv: i32, yv: i32, zv: i32| xv >= w || xv < 0 || yv >= h || yv < 0 || zv >= d || zv < 0;
    while thin_now {
        let (mut up_added, mut down_added) = (false, false);
        if up_dist <= down_dist {
            // nint((double) x + (up_dist + 1.5) * nx) with nx 0 is x
            let yv = nint(g, f64::from(y) + (up_dist + 1.5));
            if outside(x, yv, z) {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    "ERROR: MRIthickenThinWMStrands: invalid x,y,z!\n",
                );
                break;
            }
            if index_not_in_volume(t1, f64::from(x), f64::from(yv), f64::from(z)) == 0
                && vox_val(g, err, dst, x, yv, z)? == 0.0
            {
                up_added = true;
                let yv = nint(g, f64::from(y) + (up_dist + 0.5));
                if vox_val(g, err, t1, x, yv, z)? < wm_hi {
                    mri_set_vox_val(g, err, dst, x, yv, z, 0, THICKEN_FILL as f32, 0)?;
                    mri_set_vox_val(g, err, thin, x, yv, z, 0, THICKEN_FILL as f32, 0)?;
                    nfilled += 1;
                }
            }
        }
        if up_dist >= down_dist {
            let yv = nint(g, f64::from(y) - (down_dist + 1.5));
            if outside(x, yv, z) {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    "ERROR: MRIthickenThinWMStrands: invalid x,y,z!\n",
                );
                break;
            }
            if vox_val(g, err, dst, x, yv, z)? == 0.0 {
                down_added = true;
                let yv = nint(g, f64::from(y) - (down_dist + 0.5));
                if vox_val(g, err, t1, x, yv, z)? < wm_hi {
                    mri_set_vox_val(g, err, dst, x, yv, z, 0, THICKEN_FILL as f32, 0)?;
                    mri_set_vox_val(g, err, thin, x, yv, z, 0, THICKEN_FILL as f32, 0)?;
                    nfilled += 1;
                }
            }
        }
        if up_added {
            up_dist += 1.0;
        }
        if down_added {
            down_dist += 1.0;
        }
        thin_now = up_dist + down_dist <= too;
        if !up_added && !down_added {
            break;
        }
    }
    Ok(nfilled)
}

/// Whether `MRIthickenThinWMStrands`' neighbourhood filter fills `(x, y, z)`: 0 in the volume
/// with a T1 below `wm_hi`, between two of the segment's voxels along a row, a column or a slice,
/// and no voxel of its 27 (itself included) on in the volume but not in the segment.
#[allow(clippy::too_many_arguments)]
fn nbhd_fill(
    g: &mut Globals,
    err: &mut dyn Write,
    t1: &Mri,
    dst: &Mri,
    thin: &Mri,
    x: i32,
    y: i32,
    z: i32,
    wm_hi: f32,
) -> Result<bool, Fatal> {
    // !MRIgetVoxVal(dst) && MRIgetVoxVal(T1) < wm_hi (a NaN T1 fails it)
    let below = vox_val(g, err, t1, x, y, z)?.partial_cmp(&wm_hi) == Some(std::cmp::Ordering::Less);
    if vox_val(g, err, dst, x, y, z)? != 0.0 || !below {
        return Ok(false);
    }
    let on =
        |g: &mut Globals, err: &mut dyn Write, xx: i32, yy: i32, zz: i32| -> Result<bool, Fatal> {
            Ok(vox_val(g, err, thin, xx, yy, zz)? != 0.0)
        };
    let between = (on(g, err, x + 1, y, z)? && on(g, err, x - 1, y, z)?)
        || (on(g, err, x, y + 1, z)? && on(g, err, x, y - 1, z)?)
        || (on(g, err, x, y, z + 1)? && on(g, err, x, y, z - 1)?);
    if !between {
        return Ok(false);
    }
    for nz in -1..=1 {
        for ny in -1..=1 {
            for nx in -1..=1 {
                if vox_val(g, err, dst, x + nx, y + ny, z + nz)? != 0.0
                    && vox_val(g, err, thin, x + nx, y + ny, z + nz)? == 0.0
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// `MRIbinarize(segment, NULL, WM_MIN_VAL, 0, 1)`: a byte volume's voxels compared directly (the
/// same values), any other through [`mri_binarize`].
fn binarize_strand(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    segment: &Mri,
) -> Result<Mri, Fatal> {
    if segment.mtype == MRI_UCHAR && segment.chunk.is_some() {
        let mut strand = mri_clone(g, out, err, segment, None)?.unwrap_new();
        if let (Some(src), Some(dst)) = (segment.chunk.as_ref(), strand.chunk.as_mut())
            && src.len() == dst.len()
        {
            for (d, s) in dst.iter_mut().zip(src) {
                *d = u8::from(i32::from(*s) >= WM_MIN_VAL);
            }
            return Ok(strand);
        }
    }
    Ok(mri_binarize(g, out, err, Pair::New(segment), WM_MIN_VAL as f32, 0.0, 1.0)?.unwrap_new())
}

/// `MRIxor(strand, border, border, 1, 255)`: two byte volumes of one extent and frame compared
/// directly (each on within 1..255; 1 where exactly one is), any others through [`mri_xor`].
fn xor_border(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    strand: &Mri,
    border: &mut Mri,
) -> Result<(), Fatal> {
    let same = strand.mtype == MRI_UCHAR
        && border.mtype == MRI_UCHAR
        && strand.nframes == 1
        && border.nframes == 1
        && (strand.vg.width, strand.vg.height, strand.vg.depth)
            == (border.vg.width, border.vg.height, border.vg.depth);
    if same
        && let (Some(a), Some(b)) = (strand.chunk.as_ref(), border.chunk.as_mut())
        && a.len() == b.len()
    {
        for (d, s) in b.iter_mut().zip(a) {
            *d = u8::from((*s >= 1) != (*d >= 1));
        }
        return Ok(());
    }
    mri_xor(g, out, err, Tri::Second(strand, border), 1, 255)?;
    Ok(())
}

/// `MRIvox(m, x, y, z)` of a voxel of the volume: its byte (a byte volume's read directly; any
/// other type's byte as FreeSurfer reads it, FS-1225).
#[inline]
fn byte_read(g: &mut Globals, what: &str, m: &Mri, x: i32, y: i32, z: i32) -> f32 {
    if m.mtype == MRI_UCHAR
        && let Some(c) = m.chunk.as_ref()
        && (0..m.vg.width).contains(&x)
        && (0..m.vg.height).contains(&y)
        && (0..m.vg.depth).contains(&z)
    {
        let o =
            (z as usize * m.vg.height as usize + y as usize) * m.vox_per_row as usize + x as usize;
        if let Some(b) = c.get(o) {
            return f32::from(*b);
        }
    }
    let (zl, yl, xl) = (i64::from(z), i64::from(y), i64::from(x));
    mem::read(g, LABEL_BYTES, what, m, zl, yl, xl, MRI_UCHAR)
}

/// `MRIfillPlanarHoles(mri, mri_segment, mri, mseg)` as `MRIthickenThinWMStrands` calls it (the
/// volume its own destination: FreeSurfer's copy does nothing): the segment's image binarised at
/// `WM_MIN_VAL` to 0 and 1 (`MRIbinarize`), its border (dilated twice, `MRIdilate`, minus the
/// strand: `MRIxor` in 1..255); then, until a pass fills none, each voxel (slices outer) of the
/// border that is at most `WM_MIN_VAL` in the volume (`MRIvox`), whose direction of the most
/// strand voxels in a 5 x 5 plane ([`cpolv_max_white_at_voxel`]) has strand on its four sides
/// ([`cpolv_all_quadrants_filled`]) and whose face neighbours on in the strand (at least 1) are
/// as many as those on in the volume (at least `WM_MIN_VAL`, `MRIneighborsOn`), set to
/// `THICKEN_FILL` (200) in the volume, 1 in the strand and 0 in the border. With `DIAG_WRITE` and
/// `DIAG_VERBOSE`: `writing binary strand and border images...` on standard error and
/// `binary_strand.mgh`, `strand_border.mgh`; with `DIAG_SHOW` and `DIAG_VERBOSE`: `<n> planar
/// holes filled`.
pub fn fill_planar_holes_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: Ctx,
    dst: &mut Mri,
    segment: &Mri,
) -> Result<(), Fatal> {
    let (width, height, depth) = (dst.vg.width, dst.vg.height, dst.vg.depth);
    let _ = basis();
    let mut strand = binarize_strand(g, out, err, segment)?;
    let mut border = mri_dilate(g, out, err, Pair::New(&strand))?.unwrap_new();
    mri_dilate(g, out, err, Pair::Same(&mut border))?;
    xor_border(g, out, err, &strand, &mut border)?;
    let mut total_filled = 0i32;
    if diag_write_verbose(g) {
        let _ = writeln!(err, "writing binary strand and border images...");
        write_diag(g, out, err, ctx, &mut strand, "binary_strand.mgh")?;
        write_diag(g, out, err, ctx, &mut border, "strand_border.mgh")?;
    }
    const WHAT: &str = "MRIfillPlanarHoles";
    // the voxels a pass looks at further, in its order (slices outer): a voxel's own test reads
    // only its own place in the volume and the border, which only filling it changes
    let mut candidates: Vec<(i32, i32, i32)> = Vec::new();
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                if byte_read(g, WHAT, dst, x, y, z) > WM_MIN_VAL as f32
                    || byte_read(g, WHAT, &border, x, y, z) == 0.0
                {
                    continue;
                }
                candidates.push((x, y, z));
            }
        }
    }
    loop {
        let mut nfilled = 0i32;
        let mut kept = Vec::with_capacity(candidates.len());
        for &(x, y, z) in &candidates {
            let vertex = cpolv_max_white_at_voxel(g, err, &strand, x, y, z, 5)?;
            if cpolv_all_quadrants_filled(g, err, &strand, x, y, z, vertex, 5)
                && neighbors_on(g, err, &strand, x, y, z, 1)?
                    == neighbors_on(g, err, dst, x, y, z, WM_MIN_VAL)?
            {
                mri_set_vox_val(g, err, dst, x, y, z, 0, THICKEN_FILL as f32, 0)?;
                mri_set_vox_val(g, err, &mut strand, x, y, z, 0, 1.0, 0)?;
                mri_set_vox_val(g, err, &mut border, x, y, z, 0, 0.0, 0)?;
                nfilled += 1;
                // THICKEN_FILL is above WM_MIN_VAL and the border 0: no longer looked at
                if byte_read(g, WHAT, dst, x, y, z) > WM_MIN_VAL as f32
                    || byte_read(g, WHAT, &border, x, y, z) == 0.0
                {
                    continue;
                }
            }
            kept.push((x, y, z));
        }
        candidates = kept;
        total_filled += nfilled;
        if nfilled <= 0 {
            break;
        }
    }
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(err, "{total_filled} planar holes filled");
    }
    Ok(())
}

/// `MRIcpolvMaxWhiteAtVoxel(mri, x, y, z, wsize)`: the last direction (of the 22, in order) whose
/// `wsize x wsize` plane through the voxel (as [`central_plane_of_least_variance_normal_voxel`]
/// samples it: `nint` of the `float` point, the index tables) holds the most values above 0.5
/// (`>=`: a later direction with as many wins; 0 when none has any).
pub fn cpolv_max_white_at_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    wsize: i32,
) -> Result<usize, Fatal> {
    let b = basis();
    let whalf = (wsize - 1) / 2;
    let (mut max_count, mut peak) = (0i32, 0usize);
    for vertex in 0..NVERTICES {
        let [e1x, e1y, e1z] = b.e1[vertex];
        let [e2x, e2y, e2z] = b.e2[vertex];
        let mut num = 0i32;
        for yk in -whalf..=whalf {
            let xbase = x as f32 + yk as f32 * e2x;
            let ybase = y as f32 + yk as f32 * e2y;
            let zbase = z as f32 + yk as f32 * e2z;
            for xk in -whalf..=whalf {
                let xi = nint(g, f64::from(xbase + xk as f32 * e1x));
                let yi = nint(g, f64::from(ybase + xk as f32 * e1y));
                let zi = nint(g, f64::from(zbase + xk as f32 * e1z));
                if f64::from(vox(g, err, mri, xi, yi, zi)?) > 0.5 {
                    num += 1;
                }
            }
        }
        if num >= max_count {
            peak = vertex;
            max_count = num;
        }
    }
    Ok(peak)
}

/// `MRIcpolvAllQuadrantsFilled(mri, x, y, z, vertex, wsize)` (its compiled branch): along `+e1`,
/// `-e1`, `+e2` and `-e2` of the direction's plane, from 0.75 in steps of 0.75 while the distance
/// is at most `whalf = (wsize - 1) / 2` (`float`), the first point (`(float) x + dist e`) whose
/// trilinear sample is above 0.5; true when each of the four has one. `mri_segment`'s window (5)
/// always samples; a window too small to sample at all (`whalf` below 0.75) would test a value
/// FreeSurfer never set: 0 here.
#[allow(clippy::too_many_arguments)]
pub fn cpolv_all_quadrants_filled(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    vertex: usize,
    wsize: i32,
) -> bool {
    let b = basis();
    let whalf = ((wsize - 1) / 2) as f32;
    let [e1x, e1y, e1z] = b.e1[vertex];
    let [e2x, e2y, e2z] = b.e2[vertex];
    let mut val = 0f64;
    for (ex, ey, ez, sign) in [
        (e1x, e1y, e1z, 1.0f32),
        (e1x, e1y, e1z, -1.0),
        (e2x, e2y, e2z, 1.0),
        (e2x, e2y, e2z, -1.0),
    ] {
        let mut dist = 0.75f32;
        while dist <= whalf {
            let (xf, yf, zf) = if sign > 0.0 {
                (
                    x as f32 + dist * ex,
                    y as f32 + dist * ey,
                    z as f32 + dist * ez,
                )
            } else {
                (
                    x as f32 - dist * ex,
                    y as f32 - dist * ey,
                    z as f32 - dist * ez,
                )
            };
            val = sample(g, err, mri, f64::from(xf), f64::from(yf), f64::from(zf));
            if val > 0.5 {
                break;
            }
            dist += 0.75;
        }
        if val <= 0.5 {
            return false;
        }
    }
    true
}
