// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_cc`'s segmentation from an aseg (`mri_cc.cpp` 1557-2662): `find_cc_with_aseg` (1557), the
//! cutting plane's score `cc_cutting_plane_correct` (2138), the fornix's removal from a sagittal
//! slice `remove_fornix_new` (2282) and the midline's voxels `find_voxels_close_to_both_hemis`
//! (2609). The path recon-all's `seg2cc` runs (`mri_cc -aseg aseg.auto_noCCseg.mgz -o
//! aseg.auto.mgz -lta transforms/cc_up.lta <subject>`).
//!
//! `find_cc_with_aseg` finds the midsagittal plane: the hemispheres' x extents (left white matter
//! and cortex, right), the voxel lists of each side in that slab (`VLSTcreateInRegion`), the
//! principal axes of the voxels near both cortices (the third axis the plane's normal), then a
//! search of rotations about z and y (in steps of 0.25 degree within 7 degrees of the best, the
//! window following the best as it moves, as FreeSurfer's loop conditions read it) and of the
//! plane's x for the most voxels on their own side; the volume rotated upright (an LTA, voxel to
//! voxel), the dark voxels (`norm` below 40) taken out, the white matter that borders the other
//! hemisphere's on the best slice grown into the callosum, thickened, its largest connected part
//! kept, the fornix removed slice by slice, closed, and the dark white matter added back.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-cc.md`):
//! - **FS-1201** (memory): more than 500 slices between the hemispheres' extents overrun the
//!   slice table: an error in both modes.
//! - **FS-1202** (rule A): a sagittal slice whose fornix columns give no thickness (none found, or
//!   a variance rounded below 0): FreeSurfer's maximum thickness is `(int)` of a NaN (`INT_MIN`)
//!   and the shaving's first row a 32-bit overflow; by default nothing is shaved there, noted
//!   where FreeSurfer's would erase voxels; `reproduce` FreeSurfer's.

use std::io::Write;

use larmorx_core::defects::{FS_1201, FS_1202};
use larmorx_core::math::{self, Libm};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cstd::{double_to_int_x86, fmt_f};
use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{
    Y_ROTATION, Z_ROTATION, matrix_identity, matrix_inverse, matrix_print, matrix_realloc_rotation,
};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_filter::segment::{mri_segment, mri_segment_max, mri_segment_to_image};
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, Lta, lta_alloc};
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri, MriRegion};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{
    Pair, Tri, mri_binarize, mri_clear, mri_clone, mri_replace_values, mri_subtract,
};
use larmorx_freesurfer_volume::coords::{MRI_SAGITTAL, SAMPLE_NEAREST, principle_components};
use larmorx_freesurfer_volume::labels::{mri_labels_in_nbhd, mri_labels_in_planar_nbhd};
use larmorx_freesurfer_volume::ltaops::lta_fill_inverse;
use larmorx_freesurfer_volume::ltawrite::lta_write;
use larmorx_freesurfer_volume::mriset::{mri_close, mri_copy_label, mri_dilate};
use larmorx_freesurfer_volume::resize::{mri_extract_plane, mri_fill_plane};
use larmorx_freesurfer_volume::vgeom::get_vol_geom;
use larmorx_freesurfer_volume::voxlist::vlst_create_in_region;
use larmorx_freesurfer_volume::xform::mri_linear_transform_interp;

use super::main::{Options, Run};

/// `Left_Cerebral_White_Matter` and the other labels of `cma.h`.
pub const LEFT_WM: i32 = 2;
pub const LEFT_CORTEX: i32 = 3;
pub const RIGHT_WM: i32 = 41;
pub const RIGHT_CORTEX: i32 = 42;
pub const FORNIX: i32 = 250;
/// `LABEL_IN_CC` (`mri_cc.cpp`).
pub const LABEL_IN_CC: i32 = 128;
/// `CC_VAL`.
pub const CC_VAL: i32 = 100;
/// `FORNIX_VAL`, `LABEL_ERASE` (`remove_fornix_new`).
const FORNIX_VAL: i32 = 32;
const LABEL_ERASE: i32 = 96;
/// `MAX_CENTRAL_SLICES`: the size of `find_cc_with_aseg`'s slice table.
const MAX_CENTRAL_SLICES: i64 = 500;
/// `CC_ANGLE_DELTA` (`RADIANS(.25)`), the rotation search's step.
const CC_ANGLE_DELTA: f64 = (2.0 * std::f64::consts::PI * 0.25) / 360.0;

/// `DEGREES(rad)`: `(360.0 * rad) / (2.0 * M_PI)`.
pub fn degrees(rad: f64) -> f64 {
    sse::div64(sse::mul64(360.0, rad), 2.0 * std::f64::consts::PI)
}

/// `RADIANS(deg)`: `(2.0 * M_PI * deg) / 360.0`.
pub fn radians(deg: f64) -> f64 {
    sse::div64(sse::mul64(2.0 * std::f64::consts::PI, deg), 360.0)
}

/// glibc's NaN rule of a function of one `double`: a NaN argument gives itself quieted; a NaN
/// result of a number (a domain error) the default NaN (negative on x86-64).
fn nan_of(r: f64, a: f64) -> f64 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else {
        sse::DEFAULT_NAN64
    }
}

/// `(int)` of a voxel's `float` value (`int label = MRIgetVoxVal(...)`): FS-17 beyond `int`.
fn label_of(g: &mut Globals, v: f32) -> i32 {
    float_to_int(g, f64::from(v), "mri_cc")
}

/// `(int)` of frame 0's voxel (`int label = MRIgetVoxVal(...)`).
fn lab(g: &mut Globals, r: &mut Run, mri: &Mri, x: i32, y: i32, z: i32) -> Result<i32, Fatal> {
    let v = get(g, r, mri, x, y, z)?;
    Ok(label_of(g, v))
}

/// Frame 0's voxel.
fn get(g: &mut Globals, r: &mut Run, mri: &Mri, x: i32, y: i32, z: i32) -> Result<f32, Fatal> {
    mri_get_vox_val(g, r.err, mri, x, y, z, 0, 0)
}

/// Frame 0's voxel set.
fn set(
    g: &mut Globals,
    r: &mut Run,
    mri: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
    v: f32,
) -> Result<(), Fatal> {
    mri_set_vox_val(g, r.err, mri, x, y, z, 0, v, 0)?;
    Ok(())
}

/// `MRIwrite(mri, fname)` of a diagnostic volume (its status not looked at).
pub(crate) fn write_volume(
    g: &mut Globals,
    r: &mut Run,
    mri: &mut Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        mri,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )
}

fn diag_write_verbose(g: &Globals) -> bool {
    g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0
}

/// A new matrix (`MatrixAlloc(rows, cols, MATRIX_REAL)`).
fn alloc(g: &mut Globals, r: &mut Run, rows: i32, cols: i32) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc(g, r.out, r.err, rows, cols, MATRIX_REAL)?.expect("a real matrix"))
}

/// `MatrixIdentity(n, NULL)`.
fn identity(g: &mut Globals, r: &mut Run, n: i32) -> Result<Matrix, Fatal> {
    Ok(matrix_identity(g, r.out, r.err, n, None)?.unwrap_new())
}

/// `MatrixMultiply(a, b, c)` into `c`.
fn multiply_into(
    g: &mut Globals,
    r: &mut Run,
    a: &Matrix,
    b: &Matrix,
    c: &mut Matrix,
) -> Result<(), Fatal> {
    matrix_multiply(g, r.out, r.err, Some(a), Some(b), Some(c))?;
    Ok(())
}

/// `MatrixReallocRotation(4, angle, which, m)` (the angle a `float`).
fn rotation(
    g: &mut Globals,
    r: &mut Run,
    angle: f64,
    which: i32,
    m: &mut Matrix,
) -> Result<(), Fatal> {
    matrix_realloc_rotation(g, r.out, r.err, 4, angle as f32, which, Some(m))?;
    Ok(())
}

/// `MatrixPrint(Gstdout, m)`.
pub(crate) fn print_matrix(g: &mut Globals, r: &mut Run, m: &Matrix) {
    let mut sink = std::io::sink();
    matrix_print(g, Some(&mut *r.out), &mut sink, r.err, Some(m), 5);
}

/// A side's voxel list as coordinates (`vl->xi[i]` and the others as `double`s) for the score.
struct Side {
    x: Vec<f64>,
    y: Vec<f64>,
    z: Vec<f64>,
}

/// `cc_cutting_plane_correct(mri_aseg, x0, y0, z0, dx, dy, dz, vl_left, vl_right, debug)` with
/// `debug` 0: the left list's voxels on the plane's positive side (`dot > 0.5`) plus the right
/// list's on its negative side (`dot < -0.5`), `dot = (xi - x0) dx + (yi - y0) dy + (zi - z0) dz`
/// in `double`, left to right.
#[allow(clippy::too_many_arguments)]
fn cutting_plane_correct(
    left: &Side,
    right: &Side,
    x0: f64,
    y0: f64,
    z0: f64,
    dx: f64,
    dy: f64,
    dz: f64,
) -> i32 {
    let mut correct = 0i32;
    for i in 0..left.x.len() {
        let dot = (left.x[i] - x0) * dx + (left.y[i] - y0) * dy + (left.z[i] - z0) * dz;
        if dot > 0.5 {
            correct += 1;
        }
    }
    for i in 0..right.x.len() {
        let dot = (right.x[i] - x0) * dx + (right.y[i] - y0) * dy + (right.z[i] - z0) * dz;
        if dot < -0.5 {
            correct += 1;
        }
    }
    correct
}

/// `CROSS(dst, a, b)`.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        sse::sub64(sse::mul64(a[1], b[2]), sse::mul64(a[2], b[1])),
        sse::sub64(sse::mul64(a[2], b[0]), sse::mul64(a[0], b[2])),
        sse::sub64(sse::mul64(a[0], b[1]), sse::mul64(a[1], b[0])),
    ]
}

/// `VLEN(v)`: `sqrt(v0 v0 + v1 v1 + v2 v2)`.
fn vlen(v: [f64; 3]) -> f64 {
    sse::sqrt64(sse::add64(
        sse::add64(sse::mul64(v[0], v[0]), sse::mul64(v[1], v[1])),
        sse::mul64(v[2], v[2]),
    ))
}

/// `SCALAR_MUL(dst, s, v)`.
fn scale(s: f64, v: [f64; 3]) -> [f64; 3] {
    [
        sse::mul64(s, v[0]),
        sse::mul64(s, v[1]),
        sse::mul64(s, v[2]),
    ]
}

/// `ISINT(x)`: `((float)(x) == (float)(int)(x))` (`macros.h`).
fn is_int(g: &mut Globals, x: f64) -> bool {
    let i = float_to_int(g, x, "ISINT");
    (x as f32) == (i as f32)
}

/// `(int)floor(v)` and `(int)ceil(v)` of a sample position (FS-17 beyond `int`).
fn floor_int(g: &mut Globals, v: f64) -> i32 {
    float_to_int(g, v.floor(), "mri_cc")
}

fn ceil_int(g: &mut Globals, v: f64) -> i32 {
    float_to_int(g, v.ceil(), "mri_cc")
}

/// `cc_cutting_plane_correct`'s `debug` part (`Gdiag` with `DIAG_WRITE` and `DIAG_VERBOSE`): the
/// plane sampled every half voxel within half the slice's width and height on its two in-plane
/// axes (`e1 = n x (dy, dx, dx)`, `e2 = n x e1`, each normalised), the voxels a quarter voxel to
/// either side marked 160 and 100 in a clone written to `plane<debug>.mgz`; each pair of the left
/// white matter and the right adds 10 to the score.
#[allow(clippy::too_many_arguments)]
fn cutting_plane_debug(
    g: &mut Globals,
    r: &mut Run,
    aseg: &Mri,
    x0: f64,
    y0: f64,
    z0: f64,
    n: [f64; 3],
    debug: i32,
) -> Result<i32, Fatal> {
    const SDIST: f64 = 0.25;
    let mut mri_debug = mri_clone(g, r.out, r.err, aseg, None)?.unwrap_new();
    let e2 = [n[1], n[0], n[0]];
    let e1 = cross(n, e2);
    let len = vlen(e1);
    let e1 = scale(sse::div64(1.0, len), e1);
    let e2 = cross(n, e1);
    let len = vlen(e2);
    let e2 = scale(sse::div64(1.0, len), e2);
    let width = aseg.vg.depth;
    let height = aseg.vg.height;
    let mut correct = 0i32;
    let mut x = f64::from(-width / 2);
    while x <= f64::from(width / 2) {
        let mut y = f64::from(-height / 2);
        while y <= f64::from(height / 2) {
            let xf = sse::add64(sse::add64(x0, sse::mul64(e1[0], x)), sse::mul64(e2[0], y));
            let yf = sse::add64(sse::add64(y0, sse::mul64(e1[1], x)), sse::mul64(e2[1], y));
            let zf = sse::add64(sse::add64(z0, sse::mul64(e1[2], x)), sse::mul64(e2[2], y));
            let l = [
                floor_int(g, sse::add64(xf, sse::mul64(SDIST, n[0]))),
                floor_int(g, sse::add64(yf, sse::mul64(SDIST, n[1]))),
                floor_int(g, sse::add64(zf, sse::mul64(SDIST, n[2]))),
            ];
            let rr = if is_int(g, xf) {
                [
                    floor_int(g, sse::sub64(xf, sse::mul64(SDIST, n[0]))),
                    floor_int(g, sse::sub64(yf, sse::mul64(SDIST, n[1]))),
                    floor_int(g, sse::sub64(zf, sse::mul64(SDIST, n[2]))),
                ]
            } else {
                [
                    ceil_int(g, sse::sub64(xf, sse::mul64(SDIST, n[0]))),
                    ceil_int(g, sse::sub64(yf, sse::mul64(SDIST, n[1]))),
                    ceil_int(g, sse::sub64(zf, sse::mul64(SDIST, n[2]))),
                ]
            };
            let inside = |p: [i32; 3]| {
                p[0] >= 0
                    && p[0] < aseg.vg.width
                    && p[1] >= 0
                    && p[1] < aseg.vg.height
                    && p[2] >= 0
                    && p[2] < aseg.vg.depth
            };
            if inside(l) {
                let label_left = lab(g, r, aseg, l[0], l[1], l[2])?;
                if inside(rr) {
                    set(g, r, &mut mri_debug, l[0], l[1], l[2], 160.0)?;
                    set(g, r, &mut mri_debug, rr[0], rr[1], rr[2], 100.0)?;
                    let label_right = lab(g, r, aseg, rr[0], rr[1], rr[2])?;
                    if label_left == LEFT_WM && label_right == RIGHT_WM {
                        correct += 10;
                    }
                }
            }
            y += 0.5;
        }
        x += 0.5;
    }
    let fname = format!("plane{debug}.mgz");
    write_volume(g, r, &mut mri_debug, &fname)?;
    Ok(correct)
}

/// `find_voxels_close_to_both_hemis(mri_aseg, lh_label, rh_label, wsize)`: a clone of zeros with 1
/// at each voxel whose window of half-width `(wsize - 1) / 2` (through the index tables; x
/// outermost, z innermost, stopping once both are found) holds both labels.
pub fn find_voxels_close_to_both_hemis(
    g: &mut Globals,
    r: &mut Run,
    aseg: &Mri,
    lh_label: i32,
    rh_label: i32,
    wsize: i32,
) -> Result<Mri, Fatal> {
    let mut dst = mri_clone(g, r.out, r.err, aseg, None)?.unwrap_new();
    let whalf = (wsize - 1) / 2;
    let (w, h, d) = (aseg.vg.width, aseg.vg.height, aseg.vg.depth);
    let xt = aseg.xi.as_deref().unwrap_or(&[]).to_vec();
    let yt = aseg.yi.as_deref().unwrap_or(&[]).to_vec();
    let zt = aseg.zi.as_deref().unwrap_or(&[]).to_vec();
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let (mut num_lh, mut num_rh) = (0, 0);
                'window: for xk in -whalf..=whalf {
                    let xi = Mri::index(&xt, x + xk);
                    for yk in -whalf..=whalf {
                        let yi = Mri::index(&yt, y + yk);
                        for zk in -whalf..=whalf {
                            let zi = Mri::index(&zt, z + zk);
                            let label = lab(g, r, aseg, xi, yi, zi)?;
                            if label == lh_label {
                                num_lh += 1;
                            } else if label == rh_label {
                                num_rh += 1;
                            }
                            if num_lh > 0 && num_rh > 0 {
                                break 'window;
                            }
                        }
                    }
                }
                if num_lh > 0 && num_rh > 0 {
                    set(g, r, &mut dst, x, y, z, 1.0)?;
                }
            }
        }
    }
    Ok(dst)
}

/// What `find_cc_with_aseg` gives back: the callosum (a clone of the aseg's type: 128 inside,
/// rotated upright), the transform (voxel to voxel, upright from the volume, its inverse filled),
/// and the centre (`*pxc`, `*pyc`, `*pzc`).
pub struct Found {
    pub cc: Mri,
    pub lta: Lta,
    pub xc: f64,
    pub yc: f64,
    pub zc: f64,
    /// `-lta`'s transform was not written (`LTAwrite` failed: FS-1204).
    pub lta_unwritten: bool,
}

/// `find_cc_with_aseg(mri_aseg_orig, NULL, &lta, &xc, &yc, &zc, thick, mri_norm, mri_fornix)`
/// (the module's documentation). `fornix` (a clone of the aseg) receives the fornix (250) in the
/// upright frame.
pub fn find_cc_with_aseg(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    aseg_orig: &Mri,
    norm: &Mri,
    fornix: &mut Mri,
) -> Result<Found, Fatal> {
    let thick = o.dxi;
    let mut aseg = mri_copy(g, r.out, r.err, aseg_orig, None)?.unwrap_new();
    if g.gdiag & DIAG_VERBOSE != 0 {
        g.gdiag |= DIAG_VERBOSE | DIAG_WRITE;
    }
    let mut lta = lta_alloc(g, r.out, r.err, 1, None)?;
    lta.xforms[0].ltype = LINEAR_VOX_TO_VOX;

    // the leftwards extent of the right hemisphere and the rightwards extent of the left
    let (w, h, d) = (aseg.vg.width, aseg.vg.height, aseg.vg.depth);
    let mut xleft_min = w;
    let mut xright_max = 0;
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let label = lab(g, r, &aseg, x, y, z)?;
                if label == LEFT_WM || label == LEFT_CORTEX {
                    if x < xleft_min {
                        xleft_min = x;
                    }
                } else if (label == RIGHT_WM || label == RIGHT_CORTEX) && x > xright_max {
                    xright_max = x;
                }
            }
        }
    }
    let xmin = xleft_min.min(xright_max).wrapping_sub(1);
    let xmax = xleft_min.max(xright_max).wrapping_add(1);
    let region = MriRegion {
        x: xmin.wrapping_sub(1),
        y: 0,
        z: 0,
        dx: xmax.wrapping_sub(xmin).wrapping_add(2),
        dy: h,
        dz: d,
    };
    let (left, right, nl, nr) = {
        let side = |vl: &larmorx_freesurfer_volume::voxlist::VoxelList| {
            let n = vl.nvox.max(0) as usize;
            Side {
                x: vl.xi[..n].iter().map(|&v| f64::from(v)).collect(),
                y: vl.yi[..n].iter().map(|&v| f64::from(v)).collect(),
                z: vl.zi[..n].iter().map(|&v| f64::from(v)).collect(),
            }
        };
        let vl_left = vlst_create_in_region(
            g,
            r.out,
            r.err,
            &aseg,
            LEFT_WM as f32,
            LEFT_CORTEX as f32,
            None,
            o.skip,
            false,
            &region,
        )?;
        let vl_right = vlst_create_in_region(
            g,
            r.out,
            r.err,
            &aseg,
            RIGHT_WM as f32,
            RIGHT_CORTEX as f32,
            None,
            o.skip,
            false,
            &region,
        )?;
        (side(&vl_left), side(&vl_right), vl_left.nvox, vl_right.nvox)
    };
    let _ = writeln!(
        r.out,
        "{nl} voxels in left wm, {nr} in right wm, xrange [{xmin}, {xmax}]"
    );

    // the principal components of the voxels near both cortices
    let mut midline = find_voxels_close_to_both_hemis(g, r, &aseg, LEFT_CORTEX, RIGHT_CORTEX, 5)?;
    if g.gdiag & DIAG_WRITE != 0 {
        write_volume(g, r, &mut midline, "m.mgz")?;
    }
    let mut m_evectors = alloc(g, r, 3, 3)?;
    let mut evalues = [0f32; 3];
    let (_, means, _) = principle_components(
        g,
        r.out,
        r.err,
        &midline,
        Some(&mut m_evectors),
        &mut evalues,
        0,
    )?;
    drop(midline);

    let mut dx = f64::from(m_evectors.relt(1, 3));
    let mut dy = f64::from(m_evectors.relt(2, 3));
    let mut dz = f64::from(m_evectors.relt(3, 3));
    if dx < 0.0 {
        dx = -dx;
        dy = -dy;
        dz = -dz;
    }
    let x0 = means[0];
    let y0 = float_to_int(g, means[1], "mri_cc");
    let z0 = float_to_int(g, means[2], "mri_cc");
    let (yf0, zf0) = (f64::from(y0), f64::from(z0));
    let correct = cutting_plane_correct(&left, &right, x0, yf0, zf0, dx, dy, dz);
    let mut max_correct = correct;
    let mut x0_best = x0;
    let (y0_best, z0_best) = (y0, z0);
    let libm = Libm::quiet(g.notices.mode());
    let mut zrot_best = -nan_of(math::asin(dy), dy);
    let c = nan_of(libm.cos(zrot_best), zrot_best);
    let a = sse::div64(dz, c);
    let mut yrot_best = nan_of(math::asin(a), a);

    let mut m = alloc(g, r, 4, 4)?;
    let mut m_yrot = identity(g, r, 4)?;
    let mut m_zrot = identity(g, r, 4)?;
    let mut v1 = alloc(g, r, 4, 1)?;
    let mut v2 = alloc(g, r, 4, 1)?;
    v1.set_relt(4, 1, 1.0);
    v2.set_relt(4, 1, 1.0);
    let max_cc_rot = o.max_cc_rot;
    let mut scale = 1i32;
    while scale >= 1 {
        let s = f64::from(scale);
        let _ = writeln!(
            r.out,
            "searching rotation angles z=[{} {}], y=[{} {}]",
            fmt_f(
                degrees(sse::sub64(zrot_best, sse::mul64(s, max_cc_rot))),
                2,
                0
            ),
            fmt_f(
                degrees(sse::add64(zrot_best, sse::mul64(s, max_cc_rot))),
                2,
                0
            ),
            fmt_f(
                degrees(sse::sub64(yrot_best, sse::mul64(s, max_cc_rot))),
                2,
                0
            ),
            fmt_f(
                degrees(sse::add64(yrot_best, sse::mul64(s, max_cc_rot))),
                2,
                0
            ),
        );
        let step = sse::mul64(s, CC_ANGLE_DELTA);
        let mut zrot = sse::sub64(zrot_best, sse::mul64(s, max_cc_rot));
        while zrot <= sse::add64(zrot_best, sse::mul64(s, max_cc_rot)) {
            let _ = write!(
                r.out,
                "\rsearching scale {scale} Z rot {}  ",
                fmt_f(degrees(zrot), 2, 1)
            );
            let _ = r.out.flush();
            rotation(g, r, zrot, Z_ROTATION, &mut m_zrot)?;
            let mut yrot = sse::sub64(yrot_best, sse::mul64(s, max_cc_rot));
            while yrot <= sse::add64(yrot_best, sse::mul64(s, max_cc_rot)) {
                // the vector (1, 0, 0) rotated
                rotation(g, r, yrot, Y_ROTATION, &mut m_yrot)?;
                multiply_into(g, r, &m_yrot, &m_zrot, &mut m)?;
                v1.set_relt(1, 1, 1.0);
                v1.set_relt(2, 1, 0.0);
                v1.set_relt(3, 1, 0.0);
                multiply_into(g, r, &m, &v1, &mut v2)?;
                let (ddx, ddy, ddz) = (
                    f64::from(v2.relt(1, 1)),
                    f64::from(v2.relt(2, 1)),
                    f64::from(v2.relt(3, 1)),
                );
                let mut xx = f64::from(xmin);
                while xx <= f64::from(xmax) {
                    let correct = cutting_plane_correct(&left, &right, xx, yf0, zf0, ddx, ddy, ddz);
                    if correct > max_correct {
                        x0_best = xx;
                        max_correct = correct;
                        yrot_best = yrot;
                        zrot_best = zrot;
                    }
                    xx += 1.0;
                }
                yrot = sse::add64(yrot, step);
            }
            zrot = sse::add64(zrot, step);
        }
        rotation(g, r, yrot_best, Y_ROTATION, &mut m_yrot)?;
        rotation(g, r, zrot_best, Z_ROTATION, &mut m_zrot)?;
        multiply_into(g, r, &m_yrot, &m_zrot, &mut m)?;
        v1.set_relt(1, 1, 1.0);
        v1.set_relt(2, 1, 0.0);
        v1.set_relt(3, 1, 0.0);
        multiply_into(g, r, &m, &v1, &mut v2)?;
        dx = f64::from(v2.relt(1, 1));
        dy = f64::from(v2.relt(2, 1));
        dz = f64::from(v2.relt(3, 1));
        max_correct = -1;
        scale /= 2;
    }
    let _ = max_correct;
    if diag_write_verbose(g) {
        let mut c = cutting_plane_debug(
            g,
            r,
            &aseg,
            x0_best,
            f64::from(y0_best),
            f64::from(z0_best),
            [dx, dy, dz],
            2,
        )?;
        c += cutting_plane_correct(
            &left,
            &right,
            x0_best,
            f64::from(y0_best),
            f64::from(z0_best),
            dx,
            dy,
            dz,
        );
        let _ = c;
    }
    drop((left, right));
    let _ = writeln!(
        r.out,
        "global minimum found at slice {}, rotations ({}, {})",
        fmt_f(x0_best, 2, 1),
        fmt_f(degrees(yrot_best), 2, 2),
        fmt_f(degrees(zrot_best), 2, 2)
    );

    // the transform: to the origin's 128, rotated, to the best plane
    let mut m_trans = identity(g, r, 4)?;
    m_trans.set_relt(1, 4, x0_best as f32);
    m_trans.set_relt(2, 4, y0_best as f32);
    m_trans.set_relt(3, 4, z0_best as f32);
    let mut m_trans_inv = identity(g, r, 4)?;
    m_trans_inv.set_relt(1, 4, -128.0);
    m_trans_inv.set_relt(2, 4, -128.0);
    m_trans_inv.set_relt(3, 4, -128.0);
    multiply_into(g, r, &m_zrot, &m_trans_inv, &mut m)?;
    let m_tmp = matrix_multiply(g, r.out, r.err, Some(&m_yrot), Some(&m), None)?.unwrap_new();
    multiply_into(g, r, &m_trans, &m_tmp, &mut m)?;
    {
        let ml = lta.xforms[0].m_l.as_mut();
        matrix_inverse(g, r.out, r.err, Some(&m), ml)?;
    }
    let _ = writeln!(
        r.out,
        "final transformation (x={}, yr={}, zr={}):",
        fmt_f(x0_best, 2, 1),
        fmt_f(degrees(yrot_best), 2, 3),
        fmt_f(degrees(zrot_best), 2, 3)
    );
    if let Some(ml) = lta.xforms[0].m_l.clone() {
        print_matrix(g, r, &ml);
    }
    if diag_write_verbose(g) {
        lta_write(g, r.out, r.err, r.env, r.dir, &lta, "test.lta")?;
    }

    // the aseg's voxels darker than norm_thresh in norm.mgz are not callosum
    for x in 0..norm.vg.width {
        for y in 0..norm.vg.height {
            for z in 0..norm.vg.depth {
                if get(g, r, norm, x, y, z)? < o.norm_thresh as f32 {
                    set(g, r, &mut aseg, x, y, z, 0.0)?;
                }
            }
        }
    }
    let ml = lta.xforms[0].m_l.clone();
    let tmp =
        mri_linear_transform_interp(g, r.out, r.err, &aseg, None, ml.as_ref(), SAMPLE_NEAREST)?
            .unwrap_new();
    mri_copy(g, r.out, r.err, &tmp, Some(&mut aseg))?;
    drop(tmp);
    if diag_write_verbose(g) {
        write_volume(g, r, &mut aseg, "asegx.mgz")?;
    }
    let mut cc = mri_clone(g, r.out, r.err, &aseg, None)?.unwrap_new();

    // the x range again, in the upright frame
    let mut xleft_min = aseg.vg.width;
    let mut xright_max = 0;
    for x in 0..aseg.vg.width {
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                let label = lab(g, r, &aseg, x, y, z)?;
                if label == LEFT_WM {
                    if x < xleft_min {
                        xleft_min = x;
                    }
                } else if label == RIGHT_WM && x > xright_max {
                    xright_max = x;
                }
            }
        }
    }
    if xleft_min > xright_max {
        log::error_exit(
            g,
            r.err,
            ERROR_UNSUPPORTED,
            &format!(
                "{}: no WM voxels found with norm > {} -- check skull stripping\n",
                g.progname, o.norm_thresh
            ),
        )?;
    }
    let xmin = xleft_min.min(xright_max).wrapping_sub(1);
    let xmax = xleft_min.max(xright_max).wrapping_add(1);
    let _ = writeln!(
        r.out,
        "updating x range to be [{xmin}, {xmax}] in xformed coordinates"
    );
    let nslices = i64::from(xmax) - i64::from(xmin) + 1;
    if nslices > MAX_CENTRAL_SLICES {
        g.notices.add(
            &FS_1201,
            format!(
                "mri_cc: {nslices} slices between the hemispheres, beyond its table of {MAX_CENTRAL_SLICES}"
            ),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "mri_cc: {nslices} slices between the hemispheres' extents, more than its table of {MAX_CENTRAL_SLICES} (FreeSurfer writes past it here)\n"
            ),
        )?;
    }

    // the white matter that borders the other hemisphere's, slice by slice
    let mut slice_voxels = vec![0i32; nslices.max(0) as usize];
    for x in xmin..=xmax {
        let i = (x - xmin) as usize;
        slice_voxels[i] = 0;
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                let label = lab(g, r, &aseg, x, y, z)?;
                let other = if label == LEFT_WM {
                    RIGHT_WM
                } else if label == RIGHT_WM {
                    LEFT_WM
                } else {
                    continue;
                };
                if mri_labels_in_nbhd(g, r.err, &aseg, x, y, z, 1, other)? > 0 {
                    set(g, r, &mut cc, x, y, z, LABEL_IN_CC as f32)?;
                    slice_voxels[i] += 1;
                }
            }
        }
    }
    if diag_write_verbose(g) {
        write_volume(g, r, &mut cc, "s1.mgz")?;
    }
    let mut best = 0usize;
    for x in (xmin + 1)..=xmax {
        let i = (x - xmin) as usize;
        if slice_voxels[i] > slice_voxels[best] {
            best = i;
        }
    }
    let best_slice = best as i32 + xmin;
    let _ = writeln!(r.out, "best xformed slice {best_slice}");

    // the other slices erased (for now)
    for x in xmin..=xmax {
        if x == best_slice {
            continue;
        }
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                set(g, r, &mut cc, x, y, z, 0.0)?;
            }
        }
    }

    // the central slice grown over the connected white matter
    loop {
        let mut changed = false;
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                if get(g, r, &cc, best_slice, y, z)? > 0.0 {
                    continue;
                }
                let label = lab(g, r, &aseg, best_slice, y, z)?;
                if label != LEFT_WM && label != RIGHT_WM {
                    continue;
                }
                if mri_labels_in_planar_nbhd(
                    g,
                    r.err,
                    &cc,
                    best_slice,
                    y,
                    z,
                    1,
                    LABEL_IN_CC,
                    MRI_SAGITTAL,
                )? > 1
                {
                    set(g, r, &mut cc, best_slice, y, z, LABEL_IN_CC as f32)?;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // thickened one slice at a time
    if diag_write_verbose(g) {
        write_volume(g, r, &mut cc, "s2.mgz")?;
    }
    let mut tmp = mri_copy(g, r.out, r.err, &cc, None)?.unwrap_new();
    for i in 1..=thick {
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                for x in [best_slice.wrapping_add(i), best_slice.wrapping_sub(i)] {
                    let label = lab(g, r, &aseg, x, y, z)?;
                    if label == LEFT_WM || label == RIGHT_WM {
                        let v = if mri_labels_in_nbhd(g, r.err, &cc, x, y, z, 1, LABEL_IN_CC)? > 0 {
                            LABEL_IN_CC as f32
                        } else {
                            0.0
                        };
                        set(g, r, &mut tmp, x, y, z, v)?;
                    }
                }
            }
        }
        mri_copy(g, r.out, r.err, &tmp, Some(&mut cc))?;
    }
    if diag_write_verbose(g) {
        write_volume(g, r, &mut cc, "s3.mgz")?;
    }

    // its largest connected part (of the volume dilated four times)
    mri_dilate(g, r.out, r.err, Pair::Given(&cc, &mut tmp))?;
    for _ in 0..3 {
        mri_dilate(g, r.out, r.err, Pair::Same(&mut tmp))?;
    }
    let mseg = mri_segment(g, r.out, r.err, &tmp, 1.0, 255.0)?;
    let i = mri_segment_max(&mseg);
    drop(tmp);
    let part = mri_segment_to_image(g, r.out, r.err, &cc, None, &mseg, i)?.unwrap_new();
    mri_copy(g, r.out, r.err, &part, Some(&mut cc))?;
    drop(part);
    drop(mseg);
    if diag_write_verbose(g) {
        write_volume(g, r, &mut cc, "s4.mgz")?;
    }
    let xc = f64::from(best_slice);
    let mut yc = f64::from(y0_best);
    let mut zc = f64::from(z0_best);

    // the fornix removed slice by slice
    let mut tmp = mri_clone(g, r.out, r.err, &cc, None)?.unwrap_new();
    if diag_write_verbose(g) {
        write_volume(g, r, &mut cc, "cc_with_fornix.mgz")?;
    }
    mri_copy(g, r.out, r.err, &cc, Some(fornix))?;
    for x in best_slice.wrapping_sub(thick)..=best_slice.wrapping_add(thick) {
        let slice = mri_extract_plane(g, r.out, r.err, &cc, None, MRI_SAGITTAL, x)?.unwrap_new();
        let edited = remove_fornix_new(g, r, &slice)?;
        mri_fill_plane(g, r.err, &edited, &mut tmp, MRI_SAGITTAL, x, CC_VAL)?;
    }
    mri_subtract(g, r.out, r.err, Tri::First(fornix, &tmp))?;
    mri_binarize(
        g,
        r.out,
        r.err,
        Pair::Same(fornix),
        (LABEL_IN_CC - 1) as f32,
        0.0,
        FORNIX as f32,
    )?;
    if diag_write_verbose(g) {
        write_volume(g, r, fornix, "fornix.mgz")?;
    }
    mri_close(g, r.out, r.err, Pair::Given(&tmp, &mut cc))?;
    drop(tmp);

    // the white matter taken out as dark added back
    for x in best_slice.wrapping_sub(thick)..=best_slice.wrapping_add(thick) {
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                if get(g, r, fornix, x, y, z)? > 0.0 {
                    continue;
                }
                let label = lab(g, r, aseg_orig, x, y, z)?;
                if (label == LEFT_WM || label == RIGHT_WM)
                    && get(g, r, &cc, x, y, z)? == 0.0
                    && mri_labels_in_planar_nbhd(
                        g,
                        r.err,
                        &cc,
                        x,
                        y,
                        z,
                        1,
                        LABEL_IN_CC,
                        MRI_SAGITTAL,
                    )? > 1
                {
                    set(g, r, &mut cc, x, y, z, LABEL_IN_CC as f32)?;
                }
            }
        }
    }

    // the centre: on the best slice, a voxel whose neighbourhood is the most in the callosum (the
    // scan's `break` leaves the rows only: the last row with such a voxel wins)
    let mut changed = false;
    let mut i = 27;
    loop {
        for y in 0..cc.vg.height {
            for z in 0..cc.vg.depth {
                let label = lab(g, r, &cc, best_slice, y, z)?;
                if label != 0
                    && mri_labels_in_nbhd(g, r.err, &cc, best_slice, y, z, 1, LABEL_IN_CC)? >= i
                {
                    changed = true;
                    yc = f64::from(y);
                    zc = f64::from(z);
                    break;
                }
            }
        }
        i -= 1;
        if changed || i <= 0 {
            break;
        }
    }

    let mut lta_unwritten = false;
    if o.write_lta {
        let mut lta2 = lta_alloc(g, r.out, r.err, 1, Some(norm))?;
        let mut ml = lta.xforms[0].m_l.clone().expect("the transform's matrix");
        ml.set_relt(
            1,
            4,
            sse::add(ml.relt(1, 4), 128i32.wrapping_sub(best_slice) as f32),
        );
        lta2.xforms[0].m_l = Some(ml);
        lta2.xforms[0].ltype = LINEAR_VOX_TO_VOX;
        get_vol_geom(&norm.vg, &mut lta2.xforms[0].src);
        get_vol_geom(&norm.vg, &mut lta2.xforms[0].dst);
        lta_unwritten = lta_write(g, r.out, r.err, r.env, r.dir, &lta2, &o.lta_fname)? != 0;
    }
    lta_fill_inverse(g, r.out, r.err, &mut lta)?;
    Ok(Found {
        cc,
        lta,
        xc,
        yc,
        zc,
        lta_unwritten,
    })
}

/// `(int)` of a value read as an `int` voxel (`val = (int)MRIgetVoxVal(...)`).
fn ival(g: &mut Globals, r: &mut Run, mri: &Mri, x: i32, y: i32) -> Result<i32, Fatal> {
    let v = get(g, r, mri, x, y, 0)?;
    Ok(label_of(g, v))
}

/// `nint(MRIgetVoxVal(...))` of a slice's voxel.
fn nval(g: &mut Globals, r: &mut Run, mri: &Mri, x: i32, y: i32) -> Result<i32, Fatal> {
    let v = get(g, r, mri, x, y, 0)?;
    Ok(nint(g, f64::from(v)))
}

/// `remove_fornix_new(mri_slice, NULL)`: the sagittal slice (columns along the volume's slices,
/// rows its rows) with the fornix taken off the callosum: the second run of voxels down each
/// column marked fornix, the callosum grown back from its two last columns over a third of its
/// length, the fornix voxels with no callosum within 3 columns erased and those touching an
/// erased one too (the rest callosum again), the erased ones right of three callosum columns in
/// the last 11 rows kept, the columns from three before the fornix's first one shaved below the
/// mean thickness plus three deviations; the erased set to 0 and the largest connected part kept.
pub fn remove_fornix_new(g: &mut Globals, r: &mut Run, slice: &Mri) -> Result<Mri, Fatal> {
    let mut edited = mri_clone(g, r.out, r.err, slice, None)?.unwrap_new();
    let (width, height) = (slice.vg.width, slice.vg.height);
    let mut xmin = width - 1;
    let (mut xmax, mut ymax) = (0, 0);
    for x in 0..width {
        for y in 0..height {
            if get(g, r, slice, x, y, 0)? > 0.0 {
                xmin = xmin.min(x);
                xmax = xmax.max(x);
                ymax = ymax.max(y);
            }
        }
    }
    mri_copy(g, r.out, r.err, slice, Some(&mut edited))?;

    // down each column: the first run is callosum, the second fornix
    for x in xmin..=xmax {
        let mut edges_found = 0;
        let mut last_val = 0;
        for y in 0..height {
            let val = ival(g, r, slice, x, y)?;
            if val > 0 && last_val == 0 {
                edges_found += 1;
            }
            last_val = val;
            if val != 0 && edges_found > 1 {
                set(g, r, &mut edited, x, y, 0, FORNIX_VAL as f32)?;
            }
        }
    }

    // seeds in the callosum's two last columns, grown over a third of its length
    let mut tmp = mri_clone(g, r.out, r.err, slice, None)?.unwrap_new();
    for x in (xmax - 1)..=xmax {
        for y in 0..height {
            if ival(g, r, slice, x, y)? > 0 {
                set(g, r, &mut tmp, x, y, 0, LABEL_IN_CC as f32)?;
            }
        }
    }
    let mut x = xmax - 2;
    while x >= xmax - 2 * (xmax - xmin) / 3 {
        loop {
            let mut changed = 0;
            for y in 0..height {
                let val = ival(g, r, &edited, x, y)?;
                if mri_labels_in_nbhd(g, r.err, &tmp, x, y, 0, 1, LABEL_IN_CC)? > 0 && val > 0 {
                    let was = ival(g, r, &tmp, x, y)?;
                    set(g, r, &mut tmp, x, y, 0, LABEL_IN_CC as f32)?;
                    if was != LABEL_IN_CC {
                        changed += 1;
                    }
                }
            }
            if changed == 0 {
                break;
            }
        }
        x -= 1;
    }
    mri_copy_label(g, r.err, &edited, &mut tmp, LABEL_IN_CC)?;
    mri_copy_label(g, r.err, &tmp, &mut edited, LABEL_IN_CC)?;

    // fornix voxels with no callosum within 3 columns either way: erased
    for x in xmin..=xmax {
        for y in 0..height {
            if ival(g, r, &edited, x, y)? != FORNIX_VAL {
                continue;
            }
            let mut found = false;
            for i in -3..=3 {
                let x1 = x + i;
                if x < 0 || x >= edited.vg.width {
                    continue;
                }
                if ival(g, r, &tmp, x1, y)? == LABEL_IN_CC {
                    found = true;
                    break;
                }
            }
            if !found {
                set(g, r, &mut edited, x, y, 0, LABEL_ERASE as f32)?;
            }
        }
    }

    // fornix voxels touching an erased one are fornix too; the others callosum
    for x in xmin..=xmax {
        for y in 0..height {
            if ival(g, r, &edited, x, y)? != FORNIX_VAL {
                continue;
            }
            let v = if mri_labels_in_nbhd(g, r.err, &edited, x, y, 0, 1, LABEL_ERASE)? > 0 {
                LABEL_ERASE
            } else {
                LABEL_IN_CC
            };
            set(g, r, &mut edited, x, y, 0, v as f32)?;
        }
    }

    {
        let (mut mean_thickness, mut std_thickness) = (0.0f64, 0.0f64);
        let mut num = 0i32;
        let mut min_x_fornix = width - 1;
        if diag_write_verbose(g) {
            let mut s = mri_copy(g, r.out, r.err, slice, None)?.unwrap_new();
            write_volume(g, r, &mut s, "s.mgz")?;
            write_volume(g, r, &mut edited, "ed.mgz")?;
        }

        // erased voxels directly anterior to three callosum voxels: callosum
        for x in xmin..=xmax {
            for y in (ymax - 10)..=ymax {
                if nval(g, r, &edited, x, y)? != LABEL_ERASE {
                    continue;
                }
                if nval(g, r, &edited, x - 1, y)? == LABEL_IN_CC
                    && nval(g, r, &edited, x - 2, y)? == LABEL_IN_CC
                    && nval(g, r, &edited, x - 3, y)? == LABEL_IN_CC
                {
                    set(g, r, &mut edited, x, y, 0, LABEL_IN_CC as f32)?;
                }
            }
        }

        let mut min_x_fornix_set = false;
        for x in xmin..=xmax {
            for y in 0..height {
                let val = ival(g, r, slice, x, y)?;
                let val2 = ival(g, r, &edited, x, y)?;
                if val == LABEL_IN_CC && val2 == LABEL_ERASE {
                    min_x_fornix = min_x_fornix.min(x);
                    min_x_fornix_set = true;
                    let (mut first_on, mut last_on) = (-1, -1);
                    let mut y1 = y;
                    while y1 >= 0 {
                        if ival(g, r, &edited, x, y1)? == LABEL_IN_CC {
                            if first_on < 0 {
                                first_on = y1;
                            } else {
                                last_on = y1;
                            }
                        }
                        y1 -= 1;
                    }
                    let thickness = first_on - last_on + 1;
                    mean_thickness += f64::from(thickness);
                    std_thickness += f64::from(thickness * thickness);
                    num += 1;
                    break;
                }
            }
        }
        if !min_x_fornix_set {
            let _ = writeln!(r.out, "WARNING: min_x_fornix not set");
        } else {
            let _ = writeln!(r.out, "min_x_fornix = {min_x_fornix}");
        }
        let n = f64::from(num);
        mean_thickness = sse::div64(mean_thickness, n);
        std_thickness = sse::sqrt64(sse::sub64(
            sse::div64(std_thickness, n),
            sse::mul64(mean_thickness, mean_thickness),
        ));
        let limit = sse::add64(mean_thickness, sse::mul64(3.0, std_thickness)).ceil();
        // FS-1202: no thickness (a NaN): FreeSurfer's `(int)` gives INT_MIN
        let no_thickness = limit.is_nan() || limit >= 2147483648.0 || limit <= -2147483649.0;
        let max_thickness = double_to_int_x86(limit);
        let mut would_erase = 0usize;
        for x in (min_x_fornix - 3)..=min_x_fornix {
            let (mut first_on, mut last_on, mut first_off) = (-1, -1, -1);
            let mut y = height - 1;
            while y >= 0 {
                if ival(g, r, &edited, x, y)? == LABEL_IN_CC {
                    if first_on < 0 {
                        first_on = y;
                    } else {
                        last_on = y;
                    }
                } else if first_on >= 0 && first_off < 0 {
                    first_off = y;
                }
                y -= 1;
            }
            let thickness = first_on - last_on + 1;
            if thickness >= max_thickness {
                let mut ystart = if first_off > last_on {
                    // two vertical runs of white: leftover fornix
                    first_off
                } else {
                    last_on.wrapping_add(max_thickness).wrapping_sub(1)
                };
                ystart = ystart.max(0);
                for y1 in ystart..height {
                    if ival(g, r, &edited, x, y1)? == LABEL_IN_CC {
                        if no_thickness && !g.notices.reproduce() {
                            would_erase += 1;
                        } else {
                            set(g, r, &mut edited, x, y1, 0, LABEL_ERASE as f32)?;
                        }
                    }
                }
            }
        }
        if no_thickness && (would_erase > 0 || g.notices.reproduce()) {
            g.notices.add(
                &FS_1202,
                format!(
                    "mri_cc: a sagittal slice's fornix gives no thickness (FreeSurfer's maximum {max_thickness}){}",
                    if g.notices.reproduce() {
                        String::new()
                    } else {
                        format!(": {would_erase} voxels FreeSurfer would shave kept")
                    }
                ),
            );
        }
    }

    mri_replace_values(
        g,
        r.out,
        r.err,
        Pair::Same(&mut edited),
        LABEL_ERASE as f32,
        0.0,
    )?;
    drop(tmp);

    // the unconnected parts removed
    let mseg = mri_segment(g, r.out, r.err, &edited, 1.0, 255.0)?;
    let i = mri_segment_max(&mseg);
    mri_clear(g, r.err, &mut edited)?;
    mri_segment_to_image(g, r.out, r.err, slice, Some(&mut edited), &mseg, i)?;
    Ok(edited)
}
