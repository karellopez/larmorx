// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Coordinates and geometry of the `MRI` structure (`utils/mri.cpp`, 3.1-35's second piece): the
//! tkregister ("surface") RAS matrices and points, the slice direction and orientation string
//! (with `getSliceDirection` of `mriio.cpp`, taken here for `MRIvoxelToTalairach`), voxel to
//! Talairach, the centre of mass and principal components, the header comparison, the header dump
//! and its determinant, and `MRIsampleVolumeDerivativeSign`.

use std::io::Write;

use larmorx_freesurfer_core::affine::{AffineMatrix, set_affine_matrix};
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_decomp::matrix_eigen_system;
use larmorx_freesurfer_core::matrix_ops::{
    matrix_add, matrix_determinant, matrix_inverse, matrix_print, matrix_scalar_mul,
    matrix_transpose,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, Transform4 as Transform,
};
use larmorx_freesurfer_mri::tkreg::xfm_crs2xyz_tkreg;
use larmorx_freesurfer_mri::vol_geom::{VolGeom, extract_i_to_r, extract_r_to_i, voxel_to_world};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::rows;

/// `MRI_UNDEFINED`, `MRI_CORONAL`, `MRI_SAGITTAL`, `MRI_HORIZONTAL` (`MRI_AXIAL`).
pub const MRI_UNDEFINED: i32 = -1;
pub const MRI_CORONAL: i32 = 0;
pub const MRI_SAGITTAL: i32 = 1;
pub const MRI_AXIAL: i32 = 2;

/// `SAMPLE_NEAREST`.
pub const SAMPLE_NEAREST: i32 = 0;

/// `MRIsetSampleVolumeDerivativeSign(newsign)`: the sign kept, `Settting
/// MRIsampleVolumeDerivativeSign to <%g>` on standard output; the sign.
pub fn set_sample_volume_derivative_sign(g: &mut Globals, out: &mut dyn Write, newsign: f64) -> f64 {
    g.sample_volume_derivative_sign = newsign;
    let _ = writeln!(
        out,
        "Settting MRIsampleVolumeDerivativeSign to {}",
        larmorx_core::cfmt::g(newsign, 6)
    );
    newsign
}

/// The cached matrices computed where they are missing, as the tkregister functions do first
/// (`i_to_r__` from `extract_i_to_r`, `r_to_i__` from `extract_r_to_i`).
fn cache(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
) -> Result<(), Fatal> {
    if vg.i_to_r.is_none() {
        let tmp = extract_i_to_r(g, out, err, vg)?;
        let mut am = AffineMatrix::default();
        set_affine_matrix(err, &mut am, &tmp)?;
        vg.i_to_r = Some(am);
    }
    if vg.r_to_i.is_none() {
        vg.r_to_i = extract_r_to_i(g, out, err, vg)?;
    }
    Ok(())
}

/// The verbose diagnostic: `title` and the matrix (`MatrixPrint`) on standard output when
/// `Gdiag_no > 0` and `DIAG_VERBOSE` is on.
fn diag_print(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    title: &str,
    m: Option<&Matrix>,
) {
    if g.gdiag_no > 0 && (g.gdiag & DIAG_VERBOSE) != 0 {
        let _ = out.write_all(title.as_bytes());
        let mut buf = Vec::new();
        matrix_print(g, Some(&mut buf as &mut dyn Write), &mut std::io::sink(), err, m, 5);
        let _ = out.write_all(&buf);
    }
}

/// `surfaceRASFromVoxel_(mri)`: the cached matrices filled where missing, then the tkregister
/// vox2ras (`MRIxfmCRS2XYZtkreg`).
pub fn surface_ras_from_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
) -> Result<Matrix, Fatal> {
    cache(g, out, err, vg)?;
    let m = xfm_crs2xyz_tkreg(g, out, err, vg)?;
    diag_print(
        g,
        out,
        err,
        "surfaceRASFromVoxel_() vox2ras --------------------\n",
        Some(&m),
    );
    Ok(m)
}

/// `voxelFromSurfaceRAS_(mri)` (and `GetSurfaceRASToVoxelMatrix`): the cached matrices filled
/// where missing, then the inverse of the tkregister vox2ras; `None` when it cannot be inverted.
pub fn voxel_from_surface_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    cache(g, out, err, vg)?;
    let v2r = xfm_crs2xyz_tkreg(g, out, err, vg)?;
    let r2v = matrix_inverse(g, out, err, Some(&v2r), None)?.into_new();
    diag_print(
        g,
        out,
        err,
        "voxelFromSurfaceRAS_() ras2vox --------------------\n",
        r2v.as_ref(),
    );
    Ok(r2v)
}

/// `GetSurfaceRASToVoxelMatrix(mri)`: [`voxel_from_surface_ras`].
pub fn get_surface_ras_to_voxel_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    voxel_from_surface_ras(g, out, err, vg)
}

/// `surfaceRASFromRAS_(mri)`: the matrix from scanner RAS to tkregister RAS, `tkregister vox2ras
/// inv(vox2ras)`. A vox2ras that cannot be inverted ends the program in the product
/// (`MatrixMultiply(): m2 is null`).
pub fn surface_ras_from_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    let v2r = larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz(g, out, err, vg, 0, false)?;
    let v2tk = xfm_crs2xyz_tkreg(g, out, err, vg)?;
    match matrix_inverse(g, out, err, Some(&v2r), None)?.into_new() {
        Some(mut i) => {
            let c = i.clone();
            Ok(
                match matrix_multiply(g, out, err, Some(&v2tk), Some(&c), Some(&mut i))? {
                    Ret::Given => Some(i),
                    Ret::New(m) => Some(m),
                    Ret::Null => None,
                },
            )
        }
        None => Ok(matrix_multiply(g, out, err, Some(&v2tk), None, None)?.into_new()),
    }
}

/// `(x, y, z, 1)` as a real 4 x 1 vector (`V4_LOAD`: each a `float`).
fn vec4(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    x: f64,
    y: f64,
    z: f64,
) -> Result<Matrix, Fatal> {
    let mut v = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a real vector");
    v.set_relt(1, 1, x as f32);
    v.set_relt(2, 1, y as f32);
    v.set_relt(3, 1, z as f32);
    v.set_relt(4, 1, 1.0);
    Ok(v)
}

/// `m * (x, y, z, 1)` (`MatrixMultiply`; `m` absent ends the program), its first three elements.
#[allow(clippy::too_many_arguments)]
fn apply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
    x: f64,
    y: f64,
    z: f64,
) -> Result<(f64, f64, f64), Fatal> {
    let v = vec4(g, out, err, x, y, z)?;
    let r = match matrix_multiply(g, out, err, m, Some(&v), None)? {
        Ret::New(r) => r,
        _ => return Err(Fatal::Exit(1)),
    };
    Ok((
        f64::from(r.relt(1, 1)),
        f64::from(r.relt(2, 1)),
        f64::from(r.relt(3, 1)),
    ))
}

/// `MRIRASToSurfaceRAS(mri, xr, yr, zr, ...)`: a scanner RAS point in tkregister RAS
/// ([`surface_ras_from_ras`] times `(x, y, z, 1)` in `float`); `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn ras_to_surface_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    xr: f64,
    yr: f64,
    zr: f64,
) -> Result<(i32, [f64; 3]), Fatal> {
    let m = surface_ras_from_ras(g, out, err, vg)?;
    let (x, y, z) = apply(g, out, err, m.as_ref(), xr, yr, zr)?;
    Ok((NO_ERROR, [x, y, z]))
}

/// `MRIvoxelToSurfaceRAS(mri, xv, yv, zv, ...)`: a voxel in tkregister RAS
/// ([`surface_ras_from_voxel`] times `(x, y, z, 1)` in `float`; the cached matrices filled where
/// missing); `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn voxel_to_surface_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
    xv: f64,
    yv: f64,
    zv: f64,
) -> Result<(i32, [f64; 3]), Fatal> {
    let m = surface_ras_from_voxel(g, out, err, vg)?;
    let (x, y, z) = apply(g, out, err, Some(&m), xv, yv, zv)?;
    Ok((NO_ERROR, [x, y, z]))
}

/// `MRIsurfaceRASToVoxel(mri, xr, yr, zr, ...)`: a tkregister RAS point as a voxel
/// ([`voxel_from_surface_ras`] times `(x, y, z, 1)` in `float`; a matrix that cannot be inverted
/// ends the program, `MatrixMultiply(): m1 is null`); `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn surface_ras_to_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
    xr: f64,
    yr: f64,
    zr: f64,
) -> Result<(i32, [f64; 3]), Fatal> {
    let m = voxel_from_surface_ras(g, out, err, vg)?;
    let (x, y, z) = apply(g, out, err, m.as_ref(), xr, yr, zr)?;
    Ok((NO_ERROR, [x, y, z]))
}

/// `MRIdircosToOrientationString(mri, ostr)`: for each axis the letter of the largest of its
/// direction cosine's components (`R`/`L` for the first when it is larger than both others,
/// else `A`/`P` when the second is larger than the third, else `S`/`I`; by the sign), compared in
/// `float`; `???` and 1 when `ras_good_flag` is 0, else 0.
pub fn dircos_to_orientation_string(vg: &VolGeom) -> (i32, [u8; 3]) {
    if vg.ras_good_flag == 0 {
        return (1, *b"???");
    }
    let mdc = [
        [vg.x_r, vg.y_r, vg.z_r],
        [vg.x_a, vg.y_a, vg.z_a],
        [vg.x_s, vg.y_s, vg.z_s],
    ];
    let mut o = [0u8; 3];
    for (c, oc) in o.iter_mut().enumerate() {
        let (sag, cor, ax) = (mdc[0][c], mdc[1][c], mdc[2][c]);
        *oc = if sag.abs() > cor.abs() && sag.abs() > ax.abs() {
            if sag > 0.0 { b'R' } else { b'L' }
        } else if cor.abs() > ax.abs() {
            if cor > 0.0 { b'A' } else { b'P' }
        } else if ax > 0.0 {
            b'S'
        } else {
            b'I'
        };
    }
    (0, o)
}

/// `MRIsliceDirectionName(mri)`: `sagittal`, `coronal` or `axial` from the third letter of the
/// orientation string; `unknown` when `ras_good_flag` is 0.
pub fn slice_direction_name(vg: &VolGeom) -> &'static str {
    let (_, o) = dircos_to_orientation_string(vg);
    if vg.ras_good_flag == 0 {
        return "unknown";
    }
    match o[2] {
        b'L' | b'R' => "sagittal",
        b'P' | b'A' => "coronal",
        _ => "axial",
    }
}

/// `getSliceDirection(mri)` (`mriio.cpp`): `MRI_CORONAL`, `MRI_SAGITTAL` or `MRI_AXIAL` by
/// [`slice_direction_name`], `MRI_UNDEFINED` for `unknown`.
pub fn get_slice_direction(vg: &VolGeom) -> i32 {
    match slice_direction_name(vg) {
        "coronal" => MRI_CORONAL,
        "sagittal" => MRI_SAGITTAL,
        "axial" => MRI_AXIAL,
        _ => MRI_UNDEFINED,
    }
}

/// MINC's `transform_point(transform, x, y, z, ...)` as FreeSurfer's copy of volume_io is built
/// (`homogenous_transform_point` with `w = 1` inlined): each row's products summed left to right
/// in `double`, the weight's products taken coordinate first and its last element added first
/// (the official build's operand orders, for the NaN it passes on); the point divided by the
/// weight unless it is 0 or 1.
pub fn transform_point_fs(t: &Transform, x: f64, y: f64, z: f64) -> [f64; 3] {
    let row = |i: usize| {
        let s = add64(mul64(t[i][0], x), mul64(t[i][1], y));
        add64(add64(s, mul64(t[i][2], z)), t[i][3])
    };
    let (mut xt, mut yt, mut zt) = (row(0), row(1), row(2));
    let s = add64(mul64(x, t[3][0]), mul64(y, t[3][1]));
    let w = add64(t[3][3], add64(s, mul64(z, t[3][2])));
    if w != 0.0 && w != 1.0 {
        xt = div64(xt, w);
        yt = div64(yt, w);
        zt = div64(zt, w);
    }
    [xt, yt, zt]
}

/// `MRIvoxelToTalairach(mri, xv, yv, zv, ...)`: for a coronal volume ([`get_slice_direction`]) the
/// voxel's scanner RAS (`MRIvoxelToWorld`, the cached matrices filled where missing) through the
/// volume's linear Talairach transform when it has one ([`transform_point_fs`]), `NO_ERROR`;
/// another slice direction gives the error `MRIvoxelToTalairachVoxel: unsupported slice direction
/// <d>` and `ERROR_UNSUPPORTED`, the point untouched (`None`).
#[allow(clippy::too_many_arguments)]
pub fn voxel_to_talairach(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    xv: f64,
    yv: f64,
    zv: f64,
) -> Result<(i32, Option<[f64; 3]>), Fatal> {
    let dir = get_slice_direction(&mri.vg);
    if dir != MRI_CORONAL {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIvoxelToTalairachVoxel: unsupported slice direction {dir}"),
        );
        return Ok((ERROR_UNSUPPORTED, None));
    }
    let (xw, yw, zw) = voxel_to_world(g, out, err, &mut mri.vg, xv, yv, zv)?;
    let p = match &mri.linear_transform {
        Some(t) => transform_point_fs(t, xw, yw, zw),
        None => [xw, yw, zw],
    };
    Ok((NO_ERROR, Some(p)))
}

/// `MRIindexNotInVolume(mri, col, row, slice)`: 0 inside `[0, extent - 1]` on every axis, -1
/// within half a voxel of the edge (each coordinate rounded with `rint` to a `float` inside `[0,
/// extent)`), 1 outside (NaN included).
pub fn index_not_in_volume(mri: &Mri, col: f64, row: f64, slice: f64) -> i32 {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    if col >= 0.0
        && col <= f64::from(w - 1)
        && row >= 0.0
        && row <= f64::from(h - 1)
        && slice >= 0.0
        && slice <= f64::from(d - 1)
    {
        return 0;
    }
    let (c, r, s) = (
        col.round_ties_even() as f32,
        row.round_ties_even() as f32,
        slice.round_ties_even() as f32,
    );
    if c >= 0.0 && c < w as f32 && r >= 0.0 && r < h as f32 && s >= 0.0 && s < d as f32 {
        return -1;
    }
    1
}

/// `MRIsampleVolumeType(mri, x, y, z, &val, SAMPLE_NEAREST)`: `outside_val` for a point
/// unambiguously outside ([`index_not_in_volume`] 1); else the voxel of frame 0 nearest to it
/// (`nint`, clamped to the volume) as a `float`, read through the row pointers; another type
/// gives the error `MRIsampleVolumeType: unsupported volume type <t>`, 0 and `ERROR_UNSUPPORTED`.
pub fn sample_volume_nearest(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
) -> (i32, f64) {
    if index_not_in_volume(mri, x, y, z) == 1 {
        return (NO_ERROR, mri.outside_val);
    }
    let clamp = |v: i32, n: i32| {
        let v = if v < 0 { 0 } else { v };
        if v >= n { n - 1 } else { v }
    };
    let xv = clamp(nint(g, x), mri.vg.width);
    let yv = clamp(nint(g, y), mri.vg.height);
    let zv = clamp(nint(g, z), mri.vg.depth);
    let ty = mri.mtype;
    if !matches!(ty, MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT) {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIsampleVolumeType: unsupported volume type {ty}"),
        );
        return (ERROR_UNSUPPORTED, 0.0);
    }
    let v = rows::read(
        g,
        "MRIsampleVolumeType",
        mri,
        i64::from(zv),
        i64::from(yv),
        i64::from(xv),
        ty,
    );
    (NO_ERROR, f64::from(v.f()))
}

/// The weighted mean position of the voxels above `threshold` (`MRIcenterOfMass` and the first
/// pass of `MRIprincipleComponents`): `x * val` with `x` a `float`, sums in `double` in the voxel
/// order; zeros when the weight is not positive.
fn weighted_mean(
    vals: &mut dyn FnMut(&mut Globals, i32, i32, i32) -> Result<f64, Fatal>,
    g: &mut Globals,
    mri: &Mri,
    threshold: u8,
) -> Result<(f64, [f64; 3]), Fatal> {
    let (mut mx, mut my, mut mz, mut weight) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for z in 0..mri.vg.depth {
        for y in 0..mri.vg.height {
            for x in 0..mri.vg.width {
                let val = vals(g, x, y, z)?;
                if val > f64::from(threshold) {
                    weight = add64(weight, val);
                    mx = add64(mx, mul64(f64::from(x as f32), val));
                    my = add64(my, mul64(f64::from(y as f32), val));
                    mz = add64(mz, mul64(f64::from(z as f32), val));
                }
            }
        }
    }
    if weight > 0.0 {
        Ok((
            weight,
            [div64(mx, weight), div64(my, weight), div64(mz, weight)],
        ))
    } else {
        Ok((weight, [0.0; 3]))
    }
}

/// `MRIcenterOfMass(mri, means, threshold)`: the weighted mean position of the voxels of frame 0
/// above `threshold` ([`sample_volume_nearest`] at each voxel, so a type it does not read prints
/// its error once per voxel); zeros when no voxel is above it; `NO_ERROR`.
pub fn center_of_mass(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    threshold: u8,
) -> Result<(i32, [f64; 3]), Fatal> {
    let mut f = |g: &mut Globals, x: i32, y: i32, z: i32| {
        Ok(sample_volume_nearest(g, err, mri, f64::from(x), f64::from(y), f64::from(z)).1)
    };
    let (_, means) = weighted_mean(&mut f, g, mri, threshold)?;
    Ok((NO_ERROR, means))
}

/// `MRIprincipleComponents(mri, mEvectors, evalues, means, threshold)`: the weighted mean position
/// of the voxels of frame 0 above `threshold` (`MRIgetVoxVal`), then the covariance of their
/// weighted offsets (`((float)x - mean) * val` stored as `float`, each outer product added with
/// `MatrixMultiply` and `MatrixAdd`), scaled by `1 / weight` when the weight is positive, and its
/// eigen-system (`MatrixEigenSystem`: the eigenvalues into `evalues`, the vectors into
/// `m_evectors`, a new matrix when there is none); `NO_ERROR`.
pub fn principle_components(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    m_evectors: Option<&mut Matrix>,
    evalues: &mut [f32],
    threshold: u8,
) -> Result<(i32, [f64; 3], Ret<Matrix>), Fatal> {
    let mut f = |g: &mut Globals, x: i32, y: i32, z: i32| {
        Ok(f64::from(mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?))
    };
    let (weight, means) = weighted_mean(&mut f, g, mri, threshold)?;
    let (mx, my, mz) = (means[0], means[1], means[2]);
    let mut m_x = matrix_alloc(g, out, err, 3, 1, MATRIX_REAL)?.expect("a real matrix");
    let mut m_xt: Option<Matrix> = None;
    let mut m_tmp = matrix_alloc(g, out, err, 3, 3, MATRIX_REAL)?.expect("a real matrix");
    let mut m_cov = matrix_alloc(g, out, err, 3, 3, MATRIX_REAL)?.expect("a real matrix");
    for z in 0..mri.vg.depth {
        for y in 0..mri.vg.height {
            for x in 0..mri.vg.width {
                let val = f64::from(mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?);
                if val > f64::from(threshold) {
                    let o = |p: i32, m: f64| mul64(sub64(f64::from(p as f32), m), val) as f32;
                    m_x.set_relt(1, 1, o(x, mx));
                    m_x.set_relt(2, 1, o(y, my));
                    m_x.set_relt(3, 1, o(z, mz));
                    match m_xt.as_mut() {
                        Some(t) => {
                            matrix_transpose(g, out, err, &m_x, Some(t))?;
                        }
                        None => m_xt = matrix_transpose(g, out, err, &m_x, None)?.into_new(),
                    }
                    matrix_multiply(g, out, err, Some(&m_x), m_xt.as_ref(), Some(&mut m_tmp))?;
                    let c = m_cov.clone();
                    matrix_add(g, out, err, &m_tmp, &c, Some(&mut m_cov))?;
                }
            }
        }
    }
    if weight > 0.0 {
        let c = m_cov.clone();
        matrix_scalar_mul(g, out, err, &c, div64(1.0, weight) as f32, Some(&mut m_cov))?;
    }
    let r = matrix_eigen_system(g, out, err, &m_cov, evalues, m_evectors)?;
    Ok((NO_ERROR, means, r))
}

/// `%f`.
fn c_f(v: f64) -> String {
    larmorx_core::cfmt::f(v, 6)
}

/// `FEQUAL(a, b)` of two `float`s: `|a - b| < FLT_EPSILON`, in `float`.
fn fequal(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

/// `MRIcompareHeaders(mri1, mri2)`: 0 when both exist and their voxel sizes, `ptype`, field of
/// view, thickness, pixel size, location, extents in space (`xstart` ... `zend`), flip angle,
/// TR, TE, TI, direction cosines, centre (each within `FLT_EPSILON`; `ptype` and the location
/// exactly) and `ras_good_flag` agree; 1 otherwise.
pub fn compare_headers(m1: Option<&Mri>, m2: Option<&Mri>) -> i32 {
    let (Some(a), Some(b)) = (m1, m2) else {
        return 1;
    };
    let (u, v) = (&a.vg, &b.vg);
    let floats = [
        (u.xsize, v.xsize),
        (u.ysize, v.ysize),
        (u.zsize, v.zsize),
    ];
    if floats.iter().any(|&(x, y)| !fequal(x, y)) {
        return 1;
    }
    if a.ptype != b.ptype {
        return 1;
    }
    if !fequal(a.fov, b.fov) || !fequal(a.thick, b.thick) || !fequal(a.ps, b.ps) {
        return 1;
    }
    if a.location != b.location {
        return 1;
    }
    let rest = [
        (a.xstart, b.xstart),
        (a.xend, b.xend),
        (a.ystart, b.ystart),
        (a.yend, b.yend),
        (a.zstart, b.zstart),
        (a.zend, b.zend),
    ];
    if rest.iter().any(|&(x, y)| !fequal(x, y)) {
        return 1;
    }
    if !((a.flip_angle - b.flip_angle).abs() < f64::from(f32::EPSILON)) {
        return 1;
    }
    let geo = [
        (a.tr, b.tr),
        (a.te, b.te),
        (a.ti, b.ti),
        (u.x_r, v.x_r),
        (u.x_a, v.x_a),
        (u.x_s, v.x_s),
        (u.y_r, v.y_r),
        (u.y_a, v.y_a),
        (u.y_s, v.y_s),
        (u.z_r, v.z_r),
        (u.z_a, v.z_a),
        (u.z_s, v.z_s),
        (u.c_r, v.c_r),
        (u.c_a, v.c_a),
        (u.c_s, v.c_s),
    ];
    if geo.iter().any(|&(x, y)| !fequal(x, y)) {
        return 1;
    }
    i32::from(u.ras_good_flag != v.ras_good_flag)
}

/// `stuff_four_by_four(m, ...)` of the direction cosines (`MRIvolumeDeterminant`'s matrix):
/// rows `(x_r, y_r, z_r, 0)`, `(x_a, y_a, z_a, 0)`, `(x_s, y_s, z_s, 0)`, `(0, 0, 0, 1)`.
fn dircos4(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    let mut m = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a real matrix");
    let rows = [
        [vg.x_r, vg.y_r, vg.z_r, 0.0],
        [vg.x_a, vg.y_a, vg.z_a, 0.0],
        [vg.x_s, vg.y_s, vg.z_s, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            m.set_relt(r as i32 + 1, c as i32 + 1, v);
        }
    }
    Ok(m)
}

/// `MRIvolumeDeterminant(mri)`: the determinant of the direction cosines' 4 x 4 matrix
/// (`MatrixDeterminant`), as a `float`.
pub fn volume_determinant(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<f32, Fatal> {
    let m = dircos4(g, out, err, vg)?;
    Ok(matrix_determinant(g, err, &m))
}

/// `MRIdump(mri, fp)`: the header on `fp`, one `<name> = <value>` line per field (the names right
/// aligned in 6 columns and cut to 6 characters: `nframe`, `xstart`, ...; floats as `%f`), then
/// `det(xyz_ras)` ([`volume_determinant`]), `ras_good_flag`, `brightness`, `subject_name`,
/// `path_to_t1`, `fname_format`, and `register_mat` (`MatrixPrint`) when there is one;
/// `NO_ERROR`.
pub fn mri_dump(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    fp: &mut dyn Write,
) -> Result<i32, Fatal> {
    let vg = &mri.vg;
    let mut t: Vec<u8> = Vec::new();
    let mut line = |name: &str, v: &[u8]| {
        let n: String = name.chars().take(6).collect();
        t.extend_from_slice(format!("{n:>6} = ").as_bytes());
        t.extend_from_slice(v);
        t.push(b'\n');
    };
    let f = |v: f32| c_f(f64::from(v)).into_bytes();
    let i = |v: i32| v.to_string().into_bytes();
    line("fname", vg.fname.c_bytes());
    line("height", &i(vg.height));
    line("width", &i(vg.width));
    line("depth", &i(vg.depth));
    line("nframes", &i(mri.nframes));
    line("imnr0", &i(mri.imnr0));
    line("imnr1", &i(mri.imnr1));
    line("xnum", &i(vg.width));
    line("ynum", &i(vg.height));
    line("fov", &f(mri.fov));
    line("thick", &f(mri.thick));
    line("xstart", &f(mri.xstart));
    line("xend", &f(mri.xend));
    line("ystart", &f(mri.ystart));
    line("yend", &f(mri.yend));
    line("zstart", &f(mri.zstart));
    line("zend", &f(mri.zend));
    line("type", &i(mri.mtype));
    line("xsize", &f(vg.xsize));
    line("ysize", &f(vg.ysize));
    line("zsize", &f(vg.zsize));
    let three = |a: f32, b: f32, c: f32| {
        format!(
            "{} {} {}",
            c_f(f64::from(a)),
            c_f(f64::from(b)),
            c_f(f64::from(c))
        )
        .into_bytes()
    };
    line("x ras", &three(vg.x_r, vg.x_a, vg.x_s));
    line("y ras", &three(vg.y_r, vg.y_a, vg.y_s));
    line("z ras", &three(vg.z_r, vg.z_a, vg.z_s));
    line("c ras", &three(vg.c_r, vg.c_a, vg.c_s));
    let _ = fp.write_all(&t);
    let det = volume_determinant(g, out, err, vg)?;
    let _ = writeln!(fp, "det(xyz_ras) = {}", c_f(f64::from(det)));
    let _ = writeln!(fp, "ras_good_flag = {}", vg.ras_good_flag);
    let _ = writeln!(fp, "brightness = {}", mri.brightness);
    let _ = writeln!(fp, "subject_name = {}", mri.subject_name);
    let _ = writeln!(fp, "path_to_t1 = {}", mri.path_to_t1);
    let _ = writeln!(fp, "fname_format = {}", mri.fname_format);
    if let Some(m) = &vg.register_mat {
        let _ = fp.write_all(b"register_mat = \n");
        let mut buf = Vec::new();
        matrix_print(g, Some(&mut buf as &mut dyn Write), &mut std::io::sink(), err, Some(m), 5);
        let _ = fp.write_all(&buf);
    }
    Ok(NO_ERROR)
}
