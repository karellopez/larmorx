// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `VOL_GEOM` (`include/mri.h`): a volume's extent, voxel sizes, direction cosines, centre and
//! shears, its file name and the cached voxel-to-RAS matrices; and the matrices FreeSurfer computes
//! from it (`utils/mri.cpp`: `MRIxfmCRS2XYZ`, `extract_i_to_r`, `extract_r_to_i`,
//! `MRIreInitCache`, `MRIsetVox2RASFromMatrix`, `MRIp0ToCRAS`, `MRImatrixOf*`).
//!
//! An `MRI` is a `VOL_GEOM` in C++ (a subclass); here [`crate::mri::Mri`] holds one and dereferences
//! to it, so a function on a `VOL_GEOM *` takes `&VolGeom` and is given `&mri.vg` (or `&*mri`).

use std::io::Write;

use larmorx_core::defects::FS_59;
use larmorx_freesurfer_core::affine::{AffineMatrix, set_affine_matrix};
use larmorx_freesurfer_core::diag::DIAG_INFO;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply, matrix_multiply_d,
};
use larmorx_freesurfer_core::matrix_ops::{
    matrix_identity, matrix_inverse, matrix_print, matrix_zero,
};
use larmorx_freesurfer_core::ret::Ret;

/// `VOL_GEOM`. `valid`, `width`, `height` and `depth` have no initial value in C++ (a new
/// `VOL_GEOM`'s are memory contents until `initVolGeom` or a caller sets them): 0 here. The other
/// fields start as C++'s initialisers give them: voxel sizes 1, the coronal direction cosines
/// (`x_r = -1`, `y_s = -1`, `z_a = 1`), the rest 0, no cached matrices.
#[derive(Debug, PartialEq)]
pub struct VolGeom {
    pub valid: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub xsize: f32,
    pub ysize: f32,
    pub zsize: f32,
    pub ras_good_flag: i32,
    pub x_r: f32,
    pub x_a: f32,
    pub x_s: f32,
    pub y_r: f32,
    pub y_a: f32,
    pub y_s: f32,
    pub z_r: f32,
    pub z_a: f32,
    pub z_s: f32,
    pub c_r: f32,
    pub c_a: f32,
    pub c_s: f32,
    pub s_r: f32,
    pub s_a: f32,
    pub s_s: f32,
    /// `fname` (a `STRLEN` buffer in C).
    pub fname: String,
    /// `i_to_r__`: the cached voxel-to-RAS matrix.
    pub i_to_r: Option<AffineMatrix>,
    /// `r_to_i__`: the cached RAS-to-voxel matrix (`NULL` when the inverse failed).
    pub r_to_i: Option<Matrix>,
    pub register_mat: Option<Matrix>,
}

impl Default for VolGeom {
    fn default() -> Self {
        VolGeom {
            valid: 0,
            width: 0,
            height: 0,
            depth: 0,
            xsize: 1.0,
            ysize: 1.0,
            zsize: 1.0,
            ras_good_flag: 0,
            x_r: -1.0,
            x_a: 0.0,
            x_s: 0.0,
            y_r: 0.0,
            y_a: 0.0,
            y_s: -1.0,
            z_r: 0.0,
            z_a: 1.0,
            z_s: 0.0,
            c_r: 0.0,
            c_a: 0.0,
            c_s: 0.0,
            s_r: 0.0,
            s_a: 0.0,
            s_s: 0.0,
            fname: String::new(),
            i_to_r: None,
            r_to_i: None,
            register_mat: None,
        }
    }
}

impl Clone for VolGeom {
    /// C++'s copy constructor: every field and the cached matrices the source has; never
    /// `register_mat` (the copy has none).
    fn clone(&self) -> Self {
        let mut v = VolGeom::default();
        v.copy_fields(self);
        v.i_to_r = self.i_to_r;
        v.r_to_i = self.r_to_i.clone();
        v
    }
}

impl VolGeom {
    /// The fields C++'s copy constructor and assignment copy (all but the matrices).
    fn copy_fields(&mut self, o: &VolGeom) {
        self.valid = o.valid;
        self.width = o.width;
        self.height = o.height;
        self.depth = o.depth;
        self.xsize = o.xsize;
        self.ysize = o.ysize;
        self.zsize = o.zsize;
        self.ras_good_flag = o.ras_good_flag;
        self.x_r = o.x_r;
        self.x_a = o.x_a;
        self.x_s = o.x_s;
        self.y_r = o.y_r;
        self.y_a = o.y_a;
        self.y_s = o.y_s;
        self.z_r = o.z_r;
        self.z_a = o.z_a;
        self.z_s = o.z_s;
        self.c_r = o.c_r;
        self.c_a = o.c_a;
        self.c_s = o.c_s;
        self.s_r = o.s_r;
        self.s_a = o.s_a;
        self.s_s = o.s_s;
        self.fname = o.fname.clone();
    }

    /// C++'s copy assignment (`*this = other`): every field; the cached matrices the source has,
    /// copied into this one's; `register_mat` untouched. Where the source has no cached matrix,
    /// FreeSurfer keeps this one's, computed from the old geometry (FS-59): by default this one's
    /// are dropped as well (the copy is like its source), with `reproduce` kept.
    pub fn assign(&mut self, g: &mut Globals, other: &VolGeom) {
        self.copy_fields(other);
        match other.i_to_r {
            Some(m) => self.i_to_r = Some(m),
            None => {
                if self.i_to_r.is_some() {
                    g.notices.add(
                        &FS_59,
                        "a geometry assigned from one without a cached voxel-to-RAS matrix",
                    );
                    if !g.notices.reproduce() {
                        self.i_to_r = None;
                    }
                }
            }
        }
        match &other.r_to_i {
            Some(m) => match &mut self.r_to_i {
                // MatrixCopy into the existing 4 x 4 matrix: the same numbers
                Some(dst) if dst.rows >= m.rows && dst.cols >= m.cols => {
                    *dst = m.clone();
                }
                _ => self.r_to_i = Some(m.clone()),
            },
            None => {
                if self.r_to_i.is_some() {
                    g.notices.add(
                        &FS_59,
                        "a geometry assigned from one without a cached RAS-to-voxel matrix",
                    );
                    if !g.notices.reproduce() {
                        self.r_to_i = None;
                    }
                }
            }
        }
    }
}

/// `MatrixPrint(stdout, m)` (precision 5): the matrix on standard output.
fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: &Matrix) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, Some(m), 5);
    let _ = out.write_all(&buf);
}

/// A 4 x 4 real matrix of zeros (FreeSurfer's `MatrixAlloc(4, 4, MATRIX_REAL)`, which cannot fail).
fn m44(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix"))
}

/// The matrix a FreeSurfer function returned when it allocates its result (`Ret::New`).
fn new_of(r: Ret<Matrix>) -> Option<Matrix> {
    r.into_new()
}

/// `MRIxfmCRS2XYZ(mri, base, useshear)`: the voxel-to-scanner-RAS matrix of the geometry,
/// `[Mdc D, P0; 0 0 0 1]`: each column of `Mdc D` is a direction cosine times its voxel size (in
/// `double`, stored as `float`); with `useshear` and a shear above 1e-5, `Mdc D` times the shear
/// matrix (`MatrixMultiplyD`; `Gdiag & DIAG_INFO` prints it first); `P0 = c - Mdc D (n/2 + base)`,
/// the centre minus the product with the centre's voxel (`width / 2.0 + base`, ...), in `double`.
pub fn xfm_crs2xyz(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    base: i32,
    useshear: bool,
) -> Result<Matrix, Fatal> {
    let mut m = m44(g, out, err)?;
    let p = |a: f32, b: f32| (f64::from(a) * f64::from(b)) as f32;
    m.set_relt(1, 1, p(vg.x_r, vg.xsize));
    m.set_relt(2, 1, p(vg.x_a, vg.xsize));
    m.set_relt(3, 1, p(vg.x_s, vg.xsize));
    m.set_relt(1, 2, p(vg.y_r, vg.ysize));
    m.set_relt(2, 2, p(vg.y_a, vg.ysize));
    m.set_relt(3, 2, p(vg.y_s, vg.ysize));
    m.set_relt(1, 3, p(vg.z_r, vg.zsize));
    m.set_relt(2, 3, p(vg.z_a, vg.zsize));
    m.set_relt(3, 3, p(vg.z_s, vg.zsize));
    for r in 1..=3 {
        m.set_relt(r, 4, 0.0);
    }
    for c in 1..=3 {
        m.set_relt(4, c, 0.0);
    }
    m.set_relt(4, 4, 1.0);
    if useshear
        && (f64::from(vg.s_r) > 1e-5 || f64::from(vg.s_a) > 1e-5 || f64::from(vg.s_s) > 1e-5)
    {
        let mut shear = new_of(matrix_identity(g, out, err, 4, None)?).expect("an identity");
        shear.set_relt(1, 2, vg.s_r);
        shear.set_relt(1, 3, vg.s_a);
        shear.set_relt(2, 3, vg.s_s);
        if g.gdiag & DIAG_INFO != 0 {
            let _ = out.write_all(b"[DEBUG] MRIxfmCRS2XYZ() matshear:\n");
            print_matrix(g, out, err, &shear);
        }
        m = new_of(matrix_multiply_d(
            g,
            out,
            err,
            Some(&m),
            Some(&shear),
            None,
        )?)
        .expect("a 4 x 4 product");
    }
    let mut pcrs = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    let half = |n: i32| (f64::from(n) / 2.0 + f64::from(base)) as f32;
    pcrs.set_relt(1, 1, half(vg.width));
    pcrs.set_relt(2, 1, half(vg.height));
    pcrs.set_relt(3, 1, half(vg.depth));
    pcrs.set_relt(4, 1, 1.0);
    let off = new_of(matrix_multiply_d(g, out, err, Some(&m), Some(&pcrs), None)?)
        .expect("a 4 x 1 product");
    let d = |c: f32, o: f32| (f64::from(c) - f64::from(o)) as f32;
    m.set_relt(1, 4, d(vg.c_r, off.relt(1, 1)));
    m.set_relt(2, 4, d(vg.c_a, off.relt(2, 1)));
    m.set_relt(3, 4, d(vg.c_s, off.relt(3, 1)));
    Ok(m)
}

/// `extract_i_to_r(mri)`: [`xfm_crs2xyz`] with base 0, no shear.
pub fn extract_i_to_r(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    xfm_crs2xyz(g, out, err, vg, 0, false)
}

/// `extract_r_to_i(mri)`: `MatrixInverse` of [`extract_i_to_r`]; `None` (`NULL`) when it fails.
pub fn extract_r_to_i(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    let m = extract_i_to_r(g, out, err, vg)?;
    Ok(new_of(matrix_inverse(g, out, err, Some(&m), None)?))
}

/// The cache `MRIreInitCache` and the constructors fill: `i_to_r__` from [`extract_i_to_r`] (as
/// an `AffineMatrix`), `r_to_i__` from [`extract_r_to_i`].
pub fn fill_cache(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
) -> Result<(), Fatal> {
    let tmp = extract_i_to_r(g, out, err, vg)?;
    let mut am = AffineMatrix::default();
    set_affine_matrix(err, &mut am, &tmp)?;
    vg.i_to_r = Some(am);
    vg.r_to_i = extract_r_to_i(g, out, err, vg)?;
    Ok(())
}

/// `MRIreInitCache(mri)`: the cached matrices recomputed from the geometry; `NO_ERROR`.
pub fn reinit_cache(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
) -> Result<i32, Fatal> {
    fill_cache(g, out, err, vg)?;
    Ok(NO_ERROR)
}

/// The given matrix, or a new 4 x 4 of zeros (`MatrixZero(4, 4, NULL)`).
fn given_or_zero<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&'a mut Matrix>,
    fresh: &'a mut Option<Matrix>,
) -> Result<&'a mut Matrix, Fatal> {
    Ok(match m {
        Some(m) => m,
        None => fresh.insert(new_of(matrix_zero(g, out, err, 4, 4, None)?).expect("zeros")),
    })
}

fn ret_of(fresh: Option<Matrix>) -> Ret<Matrix> {
    match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    }
}

/// `MRImatrixOfDirectionCosines(mri, Mdc)`: `Mdc` (a new 4 x 4 of zeros when `NULL`) with the
/// direction cosines in its first three columns and 1 at (4, 4); the rest of a given matrix is
/// left as it was.
pub fn matrix_of_direction_cosines(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    mdc: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let m = given_or_zero(g, out, err, mdc, &mut fresh)?;
    m.set_relt(1, 1, vg.x_r);
    m.set_relt(2, 1, vg.x_a);
    m.set_relt(3, 1, vg.x_s);
    m.set_relt(1, 2, vg.y_r);
    m.set_relt(2, 2, vg.y_a);
    m.set_relt(3, 2, vg.y_s);
    m.set_relt(1, 3, vg.z_r);
    m.set_relt(2, 3, vg.z_a);
    m.set_relt(3, 3, vg.z_s);
    m.set_relt(4, 4, 1.0);
    Ok(ret_of(fresh))
}

/// `MRImatrixOfVoxelSizes(mri, D)`: `D` (new zeros when `NULL`) with the voxel sizes and 1 on its
/// diagonal.
pub fn matrix_of_voxel_sizes(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    d: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let m = given_or_zero(g, out, err, d, &mut fresh)?;
    m.set_relt(1, 1, vg.xsize);
    m.set_relt(2, 2, vg.ysize);
    m.set_relt(3, 3, vg.zsize);
    m.set_relt(4, 4, 1.0);
    Ok(ret_of(fresh))
}

/// `MRImatrixOfTranslations(mri, P0)`: `P0` (new zeros when `NULL`) with [`xfm_crs2xyz`]'s last
/// column and 1 on its diagonal.
pub fn matrix_of_translations(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    p0: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let m = given_or_zero(g, out, err, p0, &mut fresh)?;
    let v = xfm_crs2xyz(g, out, err, vg, 0, false)?;
    for k in 1..=4 {
        m.set_relt(k, 4, v.relt(k, 4));
        m.set_relt(k, k, 1.0);
    }
    Ok(ret_of(fresh))
}

/// `MRIsetVox2RASFromMatrix(mri, m_vox2ras)`: the voxel sizes (the columns' lengths, in `double`)
/// compared with the geometry's (a difference above 0.001 prints the shear warning and the matrix),
/// the direction cosines the columns divided by them, then [`p0_to_cras`] with the last column;
/// `NO_ERROR`. The voxel sizes themselves are not changed.
pub fn set_vox2ras_from_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
    m: &Matrix,
) -> Result<i32, Fatal> {
    let e = |r, c| f64::from(m.relt(r, c));
    let (rx, ry, rz) = (e(1, 1), e(1, 2), e(1, 3));
    let (ax, ay, az) = (e(2, 1), e(2, 2), e(2, 3));
    let (sx, sy, sz) = (e(3, 1), e(3, 2), e(3, 3));
    let (p0r, p0a, p0s) = (e(1, 4), e(2, 4), e(3, 4));
    let xsize = (rx * rx + ax * ax + sx * sx).sqrt();
    let ysize = (ry * ry + ay * ay + sy * sy).sqrt();
    let zsize = (rz * rz + az * az + sz * sz).sqrt();
    let (mx, my, mz) = (
        f64::from(vg.xsize),
        f64::from(vg.ysize),
        f64::from(vg.zsize),
    );
    if (xsize - mx).abs() > 0.001 || (ysize - my).abs() > 0.001 || (zsize - mz).abs() > 0.001 {
        use larmorx_core::cfmt::g as cg;
        let _ =
            out.write_all(b"WARNING: MRIsetRas2VoxFromMatrix(): voxels sizes are inconsistent\n");
        let _ = writeln!(
            out,
            "   ({},{}) ({},{}) ({},{}) ",
            cg(mx, 6),
            cg(xsize, 6),
            cg(my, 6),
            cg(ysize, 6),
            cg(mz, 6),
            cg(zsize, 6)
        );
        let _ = out.write_all(b"This is probably due to shear in the vox2ras matrix\n");
        let _ = out.write_all(b"Input Vox2RAS ------\n");
        print_matrix(g, out, err, m);
    }
    vg.x_r = (rx / xsize) as f32;
    vg.x_a = (ax / xsize) as f32;
    vg.x_s = (sx / xsize) as f32;
    vg.y_r = (ry / ysize) as f32;
    vg.y_a = (ay / ysize) as f32;
    vg.y_s = (sy / ysize) as f32;
    vg.z_r = (rz / zsize) as f32;
    vg.z_a = (az / zsize) as f32;
    vg.z_s = (sz / zsize) as f32;
    p0_to_cras(g, out, err, vg, p0r, p0a, p0s)?;
    Ok(NO_ERROR)
}

/// `MRIp0ToCRAS(mri, r0, a0, s0)`: the centre `c_ras` that puts the first voxel at `(r0, a0, s0)`:
/// [`xfm_crs2xyz`] with its last column replaced by the point (stored as `float`), times the
/// voxel `(width / 2.0, height / 2.0, depth / 2.0, 1)` (`MatrixMultiply`, in `float`); then the
/// cached matrices recomputed; 0.
pub fn p0_to_cras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &mut VolGeom,
    r0: f64,
    a0: f64,
    s0: f64,
) -> Result<i32, Fatal> {
    let mut v2r = xfm_crs2xyz(g, out, err, vg, 0, false)?;
    v2r.set_relt(1, 4, r0 as f32);
    v2r.set_relt(2, 4, a0 as f32);
    v2r.set_relt(3, 4, s0 as f32);
    let mut crs = new_of(matrix_zero(g, out, err, 4, 1, None)?).expect("zeros");
    crs.set_relt(1, 1, (f64::from(vg.width) / 2.0) as f32);
    crs.set_relt(2, 1, (f64::from(vg.height) / 2.0) as f32);
    crs.set_relt(3, 1, (f64::from(vg.depth) / 2.0) as f32);
    crs.set_relt(4, 1, 1.0);
    let ras = new_of(matrix_multiply(g, out, err, Some(&v2r), Some(&crs), None)?)
        .expect("a 4 x 1 product");
    vg.c_r = ras.relt(1, 1);
    vg.c_a = ras.relt(2, 1);
    vg.c_s = ras.relt(3, 1);
    fill_cache(g, out, err, vg)?;
    Ok(0)
}

/// `MatrixCopy(src, dst)` of an optional matrix into an optional one, as the header copies call
/// it: the copy (into `dst` when it has the room), or `None` for a `NULL` source.
pub fn copy_opt(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: Option<&Matrix>,
    dst: Option<Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let Some(src) = src else {
        return Ok(None);
    };
    let mut dst = dst;
    let r = matrix_copy(g, out, err, Some(src), dst.as_mut())?;
    Ok(match r {
        Ret::New(m) => Some(m),
        Ret::Given => dst,
        Ret::Null => None,
    })
}
