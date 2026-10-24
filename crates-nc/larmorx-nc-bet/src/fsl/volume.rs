// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! FSL's `newimage` volumes (fsl-newimage 2203.12: `newimage.h`, `newimage.cc`, `generalio.cc`,
//! `newimageio.h`), as BET uses them: an image's data and properties, read from a file with
//! FSL's conversions, swapped to FSL's "radiological" storage, and written back.
//!
//! What a program sees:
//! - **Reading** (`read_volume`): the file's values converted to the volume's type with the
//!   header's slope and intercept in `float` (FSL's `convertbuffer`), a 4D input truncated to its
//!   first volume with a warning, the voxel sizes made positive, and the data mirrored along x
//!   when the file's matrices are right-handed ("neurological"): FSL computes in left-handed
//!   ("radiological") storage, and every coordinate in voxels or millimetres refers to it.
//! - **Writing** (`save_volume`): the data mirrored back, a fresh header (FSL's `set_fsl_hdr`:
//!   dimensions, voxel sizes, both matrices and codes, intent, display range, `aux_file`,
//!   units of mm and s, `descrip` the newimage build) in the format `FSLOUTPUTTYPE` names.
//!
//! The matrices of a mirrored volume are rewritten with Armadillo's 4x4 arithmetic
//! ([`arma`](crate::fsl::arma)), as newimage does, so a header written back can differ from the
//! one read in its last bits.

use crate::fsl::arma::{self, Mat4};
use crate::fsl::filename;
use crate::fsl::glibc_pow;
use crate::fsl::nifti::{self, Extension, NiftiHeader, dt};

/// The build string newimage 2203.12 writes into `descrip` (FSL 6.0.7.17's conda build).
pub const BUILDSTRING: &str = "2203.12-dirty 2024-02-01T16:17:47+00:00";

pub const FSL_RADIOLOGICAL: i32 = -1;
pub const FSL_NEUROLOGICAL: i32 = 1;
pub const FSL_INCONSISTENT: i32 = 0;
pub const FSL_ZERODET: i32 = -101;

/// The image types newimage templates are instantiated for (BET uses these four).
pub trait Elem: Copy + Default + PartialOrd + std::fmt::Debug + Send + Sync + 'static {
    const DTYPE: i16;
    const SIZE: usize;
    fn to_f64(self) -> f64;
    fn to_f32(self) -> f32;
    /// C's conversion `(T) x` from `float`, as x86-64 performs it.
    fn from_f32(x: f32) -> Self;
    /// C's conversion `(T) x` from `double`, as x86-64 performs it.
    fn from_f64(x: f64) -> Self;
    fn put_le(self, out: &mut Vec<u8>);
}

/// x86-64's truncating conversion to `int32_t` (`cvttss2si`/`cvttsd2si`): `INT_MIN` for NaN and
/// values out of range.
pub fn c_to_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

impl Elem for i16 {
    const DTYPE: i16 = dt::SIGNED_SHORT;
    const SIZE: usize = 2;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
    fn from_f32(x: f32) -> Self {
        // GCC converts float to short through a 32-bit integer and keeps its low half.
        c_to_i32(f64::from(x)) as i16
    }
    fn from_f64(x: f64) -> Self {
        c_to_i32(x) as i16
    }
    fn put_le(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

impl Elem for i32 {
    const DTYPE: i16 = dt::SIGNED_INT;
    const SIZE: usize = 4;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn to_f32(self) -> f32 {
        self as f32
    }
    fn from_f32(x: f32) -> Self {
        c_to_i32(f64::from(x))
    }
    fn from_f64(x: f64) -> Self {
        c_to_i32(x)
    }
    fn put_le(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

impl Elem for f32 {
    const DTYPE: i16 = dt::FLOAT;
    const SIZE: usize = 4;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn to_f32(self) -> f32 {
        self
    }
    fn from_f32(x: f32) -> Self {
        x
    }
    fn from_f64(x: f64) -> Self {
        x as f32
    }
    fn put_le(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

/// FSL's `volume<char>`: C's `char` (signed on x86-64), written as unsigned bytes
/// (`DT_UNSIGNED_CHAR`); the bytes go through unchanged.
impl Elem for i8 {
    const DTYPE: i16 = dt::UNSIGNED_CHAR;
    const SIZE: usize = 1;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
    fn from_f32(x: f32) -> Self {
        c_to_i32(f64::from(x)) as i8
    }
    fn from_f64(x: f64) -> Self {
        c_to_i32(x) as i8
    }
    fn put_le(self, out: &mut Vec<u8>) {
        out.push(self as u8);
    }
}

impl Elem for f64 {
    const DTYPE: i16 = dt::DOUBLE;
    const SIZE: usize = 8;
    fn to_f64(self) -> f64 {
        self
    }
    fn to_f32(self) -> f32 {
        self as f32
    }
    fn from_f32(x: f32) -> Self {
        f64::from(x)
    }
    fn from_f64(x: f64) -> Self {
        x
    }
    fn put_le(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

/// The settings of FSL's environment that the image layer consults (read once, by the command
/// line, from the process environment; given explicitly by the Python API).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FslSettings {
    /// `FSLOUTPUTTYPE` (None when unset).
    pub output_type: Option<String>,
    /// `FSL_LOAD_NIFTI_EXTENSIONS`: extensions are kept when it is set and `atoi` of it is not 0.
    pub load_extensions: Option<String>,
}

impl FslSettings {
    pub fn from_env() -> FslSettings {
        let get = |n: &str| std::env::var_os(n).map(|v| v.to_string_lossy().into_owned());
        FslSettings {
            output_type: get("FSLOUTPUTTYPE"),
            load_extensions: get("FSL_LOAD_NIFTI_EXTENSIONS"),
        }
    }

    fn keep_extensions(&self) -> bool {
        self.load_extensions
            .as_deref()
            .is_some_and(|v| crate::fsl::cstr::atoi(v) != 0)
    }
}

/// An error of the image layer: the `Image Exception` lines newimage printed on standard error,
/// and the message of the exception it threw (`what()`). Uncaught, FSL's programs abort with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError {
    pub printed: String,
    pub what: String,
    /// The exception's C++ type, as `terminate` names it.
    pub type_name: &'static str,
    /// Raised by `exit(EXIT_FAILURE)` rather than an exception (FSLOUTPUTTYPE errors): the
    /// program stops with this status after `printed`.
    pub exit: Option<u8>,
}

/// newimage's `imthrow`: prints `Image Exception : #<n> :: <msg>` and throws.
fn imthrow(printed: &mut String, msg: &str, n: i32) -> ImageError {
    printed.push_str(&format!("Image Exception : #{n} :: {msg}\n"));
    ImageError {
        printed: printed.clone(),
        what: msg.to_owned(),
        type_name: "std::runtime_error",
        exit: None,
    }
}

/// An FSL volume: up to 7 dimensions, x fastest.
#[derive(Clone, Debug)]
pub struct Volume<T: Elem> {
    pub data: Vec<T>,
    pub dims: [i64; 7],
    pub xdim: f32,
    pub ydim: f32,
    pub zdim: f32,
    pub tr: f32,
    pub pxdim5: f32,
    pub pxdim6: f32,
    pub pxdim7: f32,
    pub sform: Mat4,
    pub qform: Mat4,
    pub sform_code: i32,
    pub qform_code: i32,
    pub radiological_file: bool,
    pub intent_code: i32,
    pub intent_params: [f32; 3],
    pub display_max: f32,
    pub display_min: f32,
    pub aux_file: [u8; 24],
    pub extensions: Vec<Extension>,
}

impl<T: Elem> Volume<T> {
    /// `volume<T>(x, y, z)`: zeros, newimage's default properties.
    pub fn new(nx: i64, ny: i64, nz: i64) -> Volume<T> {
        Volume::new_4d(nx, ny, nz, 1)
    }

    pub fn new_4d(nx: i64, ny: i64, nz: i64, nt: i64) -> Volume<T> {
        let n = (nx * ny * nz * nt).max(0) as usize;
        Volume {
            data: vec![T::default(); n],
            dims: [nx, ny, nz, nt, 1, 1, 1],
            xdim: 1.0,
            ydim: 1.0,
            zdim: 1.0,
            tr: 1.0,
            pxdim5: 1.0,
            pxdim6: 1.0,
            pxdim7: 1.0,
            sform: arma::IDENTITY,
            qform: arma::IDENTITY,
            sform_code: nifti::XFORM_UNKNOWN,
            qform_code: nifti::XFORM_UNKNOWN,
            radiological_file: true,
            intent_code: 0,
            intent_params: [0.0; 3],
            display_max: 0.0,
            display_min: 0.0,
            aux_file: [0; 24],
            extensions: Vec::new(),
        }
    }

    pub fn xsize(&self) -> i64 {
        self.dims[0]
    }
    pub fn ysize(&self) -> i64 {
        self.dims[1]
    }
    pub fn zsize(&self) -> i64 {
        self.dims[2]
    }
    pub fn tsize(&self) -> i64 {
        self.dims[3]
    }
    pub fn nvoxels(&self) -> usize {
        (self.dims[0] * self.dims[1] * self.dims[2]) as usize
    }

    /// `dimensionality()`.
    pub fn dimensionality(&self) -> i32 {
        let mut d = 0;
        if self.dims[0] >= 1 {
            d = 1;
        }
        for (k, &n) in self.dims.iter().enumerate().skip(1) {
            if n > 1 {
                d = k as i32 + 1;
            }
        }
        d
    }

    #[inline]
    pub fn index(&self, x: i64, y: i64, z: i64) -> usize {
        ((z * self.dims[1] + y) * self.dims[0] + x) as usize
    }

    /// `value(x, y, z)` (no bounds check in FSL; here an index into the first volume).
    #[inline]
    pub fn value(&self, x: i64, y: i64, z: i64) -> T {
        self.data[self.index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: i64, y: i64, z: i64, v: T) {
        let i = self.index(x, y, z);
        self.data[i] = v;
    }

    #[inline]
    pub fn in_bounds(&self, x: i64, y: i64, z: i64) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < self.dims[0] && y < self.dims[1] && z < self.dims[2]
    }

    /// `operator()(x, y, z)` with newimage's default extrapolation (zero padding).
    #[inline]
    pub fn get(&self, x: i64, y: i64, z: i64) -> T {
        if self.in_bounds(x, y, z) {
            self.value(x, y, z)
        } else {
            T::default()
        }
    }

    /// `copyproperties` / `copybasicproperties` from another volume.
    pub fn copy_properties_from<S: Elem>(&mut self, s: &Volume<S>) {
        self.xdim = s.xdim;
        self.ydim = s.ydim;
        self.zdim = s.zdim;
        self.tr = s.tr;
        self.pxdim5 = s.pxdim5;
        self.pxdim6 = s.pxdim6;
        self.pxdim7 = s.pxdim7;
        self.sform = s.sform;
        self.qform = s.qform;
        self.sform_code = s.sform_code;
        self.qform_code = s.qform_code;
        self.radiological_file = s.radiological_file;
        self.intent_code = s.intent_code;
        self.intent_params = s.intent_params;
        self.display_max = s.display_max;
        self.display_min = s.display_min;
        self.aux_file = s.aux_file;
        self.extensions = s.extensions.clone();
    }

    /// `copyconvert(source, dest)`: the source's properties and values converted with C's
    /// conversion from `float` (`convertbuffer` without scaling: `(D) value`).
    pub fn copyconvert<S: Elem>(s: &Volume<S>) -> Volume<T> {
        let mut v = Volume::<T>::new_4d(s.dims[0], s.dims[1], s.dims[2], s.dims[3]);
        v.dims = s.dims;
        v.copy_properties_from(s);
        v.data = s.data.iter().map(|&x| convert::<S, T>(x)).collect();
        v
    }

    /// `sampling_mat()`.
    pub fn sampling_mat(&self) -> Mat4 {
        let mut m = arma::IDENTITY;
        m[0][0] = f64::from(self.xdim);
        m[1][1] = f64::from(self.ydim);
        m[2][2] = f64::from(self.zdim);
        m
    }

    /// `swapmat(dim1, dim2, dim3)` for (-1, 2, 3): x to `(nx - 1) * xdim - x` in mm (the fov is
    /// a `float`).
    pub fn swapmat_lr(&self) -> Mat4 {
        let fov = ((self.dims[0] - 1) as f32) * self.xdim;
        arma::from_rows([
            [-1.0, 0.0, 0.0, f64::from(fov)],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// `left_right_order()`.
    pub fn left_right_order(&self) -> i32 {
        let mut order;
        let (mut dets, mut detq, mut det) = (-1.0f32, -1.0f32, -1.0f32);
        if self.qform_code != nifti::XFORM_UNKNOWN {
            detq = arma::det(&self.qform) as f32;
            det = detq;
        }
        if self.sform_code != nifti::XFORM_UNKNOWN {
            dets = arma::det(&self.sform) as f32;
            det = dets;
        }
        order = if det < 0.0 {
            FSL_RADIOLOGICAL
        } else {
            FSL_NEUROLOGICAL
        };
        if self.sform_code != nifti::XFORM_UNKNOWN && self.qform_code != nifti::XFORM_UNKNOWN {
            if dets * detq < 0.0 {
                order = FSL_INCONSISTENT;
            }
            if (dets * detq).abs() < 1e-12 {
                order = FSL_ZERODET;
            }
        }
        if det.abs() < 1e-12 {
            order = FSL_ZERODET;
        }
        order
    }

    /// `basic_swapdimensions(-1, 2, 3, false, header_only)`: mirrors x (data and matrices).
    pub fn swap_lr(&mut self, header_only: bool) {
        let (nx, ny, nz) = (self.dims[0], self.dims[1], self.dims[2]);
        let nvox = (nx * ny * nz) as usize;
        if !header_only && nvox > 1 {
            let nvol = self.data.len() / nvox.max(1);
            for t in 0..nvol {
                let base = t * nvox;
                for z in 0..nz {
                    for y in 0..ny {
                        let row = base + ((z * ny + y) * nx) as usize;
                        self.data[row..row + nx as usize].reverse();
                    }
                }
            }
        }
        // Matrices: S * inv(sampling) * inv(swap) * swapped sampling, left to right.
        let samp_inv = arma::inv(&self.sampling_mat()).unwrap_or(arma::IDENTITY);
        let swap_inv = arma::inv(&self.swapmat_lr()).unwrap_or(arma::IDENTITY);
        let mut sw = arma::IDENTITY;
        sw[0][0] = f64::from((-self.xdim).abs());
        sw[1][1] = f64::from(self.ydim.abs());
        sw[2][2] = f64::from(self.zdim.abs());
        let chain = |m: &Mat4| arma::mul(&arma::mul(&arma::mul(m, &samp_inv), &swap_inv), &sw);
        let new_s = chain(&self.sform);
        let new_q = chain(&self.qform);
        // setdims(-xdim, ydim, zdim): absolute values.
        self.xdim = (-self.xdim).abs();
        self.sform = new_s;
        self.qform = new_q;
    }

    /// `makeradiological(header_only)`.
    pub fn make_radiological(&mut self, header_only: bool) {
        if self.left_right_order() == FSL_NEUROLOGICAL {
            self.swap_lr(header_only);
        }
    }

    /// `makeneurological()`.
    pub fn make_neurological(&mut self) {
        if self.left_right_order() == FSL_RADIOLOGICAL {
            self.swap_lr(false);
        }
    }

    /// `niftivox2newimagevox_mat()`.
    pub fn niftivox2newimagevox_mat(&self) -> Mat4 {
        if !self.radiological_file && self.left_right_order() == FSL_RADIOLOGICAL {
            // inv(sampling) * swap * sampling: inv(A) * B * C is a solve of A against B * C.
            let s = self.sampling_mat();
            let bc = arma::mul(&self.swapmat_lr(), &s);
            return arma::solve(&s, &bc).unwrap_or(arma::IDENTITY);
        }
        arma::IDENTITY
    }

    /// `calculateExtrema`: (max, min) over every value, starting from the first, with the
    /// original's `if (v > max) ... else if (v < min)`.
    pub fn extrema(&self) -> (T, T) {
        let Some(&first) = self.data.first() else {
            return (T::default(), T::default());
        };
        let (mut mx, mut mn) = (first, first);
        for &v in &self.data[1..] {
            if v > mx {
                mx = v;
            } else if v < mn {
                mn = v;
            }
        }
        (mx, mn)
    }

    pub fn max(&self) -> T {
        self.extrema().0
    }

    pub fn min(&self) -> T {
        self.extrema().1
    }
}

impl Volume<f32> {
    /// `robustmin()`, `robustmax()`: Steve Smith's 2%-98% thresholds (`find_thresholds`, no mask).
    pub fn robust_limits(&self) -> (f32, f32) {
        find_thresholds(self)
    }
}

impl<T: Elem> Volume<T> {
    #[inline]
    fn value_f32(&self, x: i64, y: i64, z: i64) -> f32 {
        self.value(x, y, z).to_f32()
    }

    #[inline]
    fn get_f32(&self, x: i64, y: i64, z: i64) -> f32 {
        self.get(x, y, z).to_f32()
    }

    /// `interpolate(x, y, z)`, trilinear with zero padding (newimage's defaults).
    pub fn interpolate(&self, x: f32, y: f32, z: f32) -> f32 {
        let ix = x.floor() as i64;
        let iy = y.floor() as i64;
        let iz = z.floor() as i64;
        let dx = x - ix as f32;
        let dy = y - iy as f32;
        let dz = z - iz as f32;
        let (v000, v001, v010, v011, v100, v101, v110, v111);
        if ix >= 0
            && iy >= 0
            && iz >= 0
            && ix < self.dims[0] - 1
            && iy < self.dims[1] - 1
            && iz < self.dims[2] - 1
        {
            v000 = self.value_f32(ix, iy, iz);
            v100 = self.value_f32(ix + 1, iy, iz);
            v110 = self.value_f32(ix + 1, iy + 1, iz);
            v010 = self.value_f32(ix, iy + 1, iz);
            v011 = self.value_f32(ix, iy + 1, iz + 1);
            v111 = self.value_f32(ix + 1, iy + 1, iz + 1);
            v101 = self.value_f32(ix + 1, iy, iz + 1);
            v001 = self.value_f32(ix, iy, iz + 1);
        } else {
            v000 = self.get_f32(ix, iy, iz);
            v001 = self.get_f32(ix, iy, iz + 1);
            v010 = self.get_f32(ix, iy + 1, iz);
            v011 = self.get_f32(ix, iy + 1, iz + 1);
            v100 = self.get_f32(ix + 1, iy, iz);
            v101 = self.get_f32(ix + 1, iy, iz + 1);
            v110 = self.get_f32(ix + 1, iy + 1, iz);
            v111 = self.get_f32(ix + 1, iy + 1, iz + 1);
        }
        q_tri_interpolation(v000, v001, v010, v011, v100, v101, v110, v111, dx, dy, dz)
    }
}

/// newimage's `q_tri_interpolation`, in `float`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn q_tri_interpolation(
    v000: f32,
    v001: f32,
    v010: f32,
    v011: f32,
    v100: f32,
    v101: f32,
    v110: f32,
    v111: f32,
    dx: f32,
    dy: f32,
    dz: f32,
) -> f32 {
    let temp1 = (v100 - v000) * dx + v000;
    let temp2 = (v101 - v001) * dx + v001;
    let temp3 = (v110 - v010) * dx + v010;
    let temp4 = (v111 - v011) * dx + v011;
    let temp5 = (temp3 - temp1) * dy + temp1;
    let temp6 = (temp4 - temp2) * dy + temp2;
    (temp6 - temp5) * dz + temp5
}

/// `gaussian_kernel1D(sigma, radius)` (newimagefns.cc): `2 radius + 1` values
/// `exp(-j^2 / (2 sigma^2))` rounded to `float` and summed in `float`, then scaled in `double` by
/// the inverse of the sum.
pub fn gaussian_kernel_1d(sigma: f32, radius: i32) -> Vec<f64> {
    let mut kern = Vec::with_capacity((2 * i64::from(radius) + 1).max(0) as usize);
    let mut sum = 0f32;
    for j in -radius..=radius {
        let val: f32 = if f64::from(sigma) > 1e-6 {
            let s = f64::from(sigma);
            glibc_pow::exp(f64::from(-(j.wrapping_mul(j))) / (2.0 * s * s)) as f32
        } else if j == 0 {
            1.0
        } else {
            0.0
        };
        kern.push(f64::from(val));
        sum += val;
    }
    let f = 1.0 / f64::from(sum);
    for k in &mut kern {
        *k *= f;
    }
    kern
}

/// One pass of `convolve_separable`: `convolve(source, kernel)` with a kernel along `axis`
/// (centred on `(n - 1) / 2`), the image zero outside; each output voxel the `float` sum, in
/// kernel order, of the `double` products (`val += source(...) * kernel(...)`). The z slices are
/// shared between `threads` threads; the result does not depend on their number.
fn convolve_axis(src: &Volume<f32>, ker: &[f64], axis: usize, threads: usize) -> Volume<f32> {
    let mut result = src.clone();
    let (nx, ny, nz) = (src.dims[0], src.dims[1], src.dims[2]);
    let mid = (ker.len() as i64 - 1) / 2;
    let plane = (nx * ny) as usize;
    let n = (nx * ny * nz) as usize;
    if n == 0 {
        return result;
    }
    let data = &src.data[..n];
    let slice = |z: i64, out: &mut [f32]| {
        for y in 0..ny {
            for x in 0..nx {
                let mut val = 0f32;
                for (m, &k) in ker.iter().enumerate() {
                    let d = m as i64 - mid;
                    let (cx, cy, cz) = match axis {
                        0 => (x + d, y, z),
                        1 => (x, y + d, z),
                        _ => (x, y, z + d),
                    };
                    if cx < 0 || cy < 0 || cz < 0 || cx >= nx || cy >= ny || cz >= nz {
                        continue; // + 0 * k: no change
                    }
                    let v = data[((cz * ny + cy) * nx + cx) as usize];
                    val = (f64::from(val) + f64::from(v) * k) as f32;
                }
                out[(y * nx + x) as usize] = val;
            }
        }
    };
    let out = &mut result.data[..n];
    let threads = threads.max(1).min(nz as usize);
    if threads == 1 {
        for (z, chunk) in out.chunks_mut(plane).enumerate() {
            slice(z as i64, chunk);
        }
    } else {
        let per = (nz as usize).div_ceil(threads);
        std::thread::scope(|scope| {
            for (t, block) in out.chunks_mut(plane * per).enumerate() {
                let slice = &slice;
                scope.spawn(move || {
                    for (k, chunk) in block.chunks_mut(plane).enumerate() {
                        slice((t * per + k) as i64, chunk);
                    }
                });
            }
        });
    }
    result
}

/// `smooth(source, sigma_mm)` (newimagefns.h): a separable Gaussian of `sigma_mm` millimetres,
/// along x, then y, then z, each kernel of radius `2 (int)(sigma - 0.001) + 3` voxels.
pub fn smooth(src: &Volume<f32>, sigma_mm: f32, threads: usize) -> Volume<f32> {
    let sigma = [
        sigma_mm / src.xdim,
        sigma_mm / src.ydim,
        sigma_mm / src.zdim,
    ];
    let mut result = src.clone();
    for (axis, &s) in sigma.iter().enumerate() {
        let radius = c_to_i32(f64::from(s) - 0.001)
            .wrapping_mul(2)
            .wrapping_add(3);
        let ker = gaussian_kernel_1d(s, radius);
        result = convolve_axis(&result, &ker, axis, threads);
    }
    result
}

/// `(D) x` for the element types (C's conversions).
pub fn convert<S: Elem, D: Elem>(x: S) -> D {
    match S::DTYPE {
        dt::FLOAT => D::from_f32(x.to_f32()),
        _ => D::from_f64(x.to_f64()),
    }
}

/// `find_thresholds` without a mask, for `float` volumes.
fn find_thresholds(vol: &Volume<f32>) -> (f32, f32) {
    const BINS: i32 = 1000;
    const MAX_PASSES: i32 = 10;
    let mut hist = vec![0f64; BINS as usize];
    let (mut top_bin, mut bottom_bin) = (0i32, 0i32);
    let mut pass = 1;
    let mut lowest_bin = 0i32;
    let mut highest_bin = BINS - 1;
    let (mut thresh98, mut thresh2) = (0f32, 0f32);
    let (vmax, vmin) = vol.extrema();
    let (mut min, mut max) = (vmin, vmax);
    while pass == 1 || f64::from(thresh98 - thresh2) < f64::from(max - min) / 10.0 {
        if pass > 1 {
            bottom_bin = (bottom_bin - 1).max(0);
            top_bin = (top_bin + 1).min(BINS - 1);
            let tmpmin = (f64::from(min)
                + (f64::from(bottom_bin) / f64::from(BINS)) * f64::from(max - min))
                as f32;
            max = (f64::from(min)
                + (f64::from(top_bin + 1) / f64::from(BINS)) * f64::from(max - min))
                as f32;
            min = tmpmin;
        }
        if pass == MAX_PASSES || min == max {
            min = vmin;
            max = vmax;
        }
        let mut validsize = image_histogram(vol, BINS, min, max, &mut hist);
        if validsize < 1 {
            return (min, max);
        }
        if pass == MAX_PASSES {
            validsize -=
                round_i64(hist[lowest_bin as usize]) + round_i64(hist[highest_bin as usize]);
            lowest_bin += 1;
            highest_bin -= 1;
        }
        if validsize < 0 {
            thresh2 = min;
            thresh98 = min;
            break;
        }
        let fa = f64::from(max - min) / f64::from(BINS);
        let mut count: i32 = 0;
        bottom_bin = lowest_bin;
        while i64::from(count) < validsize / 50 {
            count += round_i32(hist[bottom_bin as usize]);
            bottom_bin += 1;
        }
        bottom_bin -= 1;
        thresh2 = min + (f64::from(bottom_bin) * fa) as f32;
        count = 0;
        top_bin = highest_bin;
        while i64::from(count) < validsize / 50 {
            count += round_i32(hist[top_bin as usize]);
            top_bin -= 1;
        }
        top_bin += 1;
        thresh98 = min + (f64::from(top_bin + 1) * fa) as f32;
        if pass == MAX_PASSES {
            break;
        }
        pass += 1;
    }
    (thresh2, thresh98)
}

/// `MISCMATHS::round(double)`: half away from zero, by truncation.
pub fn misc_round(x: f64) -> i32 {
    if x > 0.0 {
        c_to_i32(x + 0.5)
    } else {
        c_to_i32(x - 0.5)
    }
}

fn round_i32(x: f64) -> i32 {
    misc_round(x)
}

fn round_i64(x: f64) -> i64 {
    i64::from(round_i32(x))
}

/// `imageHistogram` without a mask: every value, `hist[max(0, min((int)(fA*v + fB), bins-1))]`.
fn image_histogram(vol: &Volume<f32>, bins: i32, min: f32, max: f32, hist: &mut [f64]) -> i64 {
    if max <= min {
        return -1;
    }
    for h in hist.iter_mut() {
        *h = 0.0;
    }
    let fa = f64::from(bins) / f64::from(max - min);
    let fb = (f64::from(bins) * f64::from(-min)) / f64::from(max - min);
    let mut n = 0i64;
    for &v in &vol.data {
        let b = c_to_i32(fa * f64::from(v) + fb).min(bins - 1).max(0);
        hist[b as usize] += 1.0;
        n += 1;
    }
    n
}

// ------------------------------------------------------------------------------------------------
// Reading

/// `dtype(filename)` is not needed by BET; `read_volume` reads into the volume's type.
///
/// `read_volume(target, filename)` with newimage's legacy behaviour (a 4D image is truncated to
/// its first volume with a warning, printed to `warn`).
pub fn read_volume<T: Elem>(
    name: &str,
    settings: &FslSettings,
    warn: &mut String,
) -> Result<Volume<T>, ImageError> {
    let mut v = read_general::<T>(name, settings)?;
    if v.tsize() > 1 {
        warn.push_str(
            "Warning: An input intended to be a single 3D volume has multiple timepoints. \
             Input will be truncated to first volume, but this functionality is deprecated \
             and will be removed in a future release.\n",
        );
        let n = v.nvoxels();
        v.data.truncate(n);
        v.dims = [v.dims[0], v.dims[1], v.dims[2], 1, 1, 1, 1];
    }
    Ok(v)
}

/// `read_volume4D`: every volume.
pub fn read_volume4d<T: Elem>(name: &str, settings: &FslSettings) -> Result<Volume<T>, ImageError> {
    read_general::<T>(name, settings)
}

/// `readGeneralVolume(target, filename, dtype, swap2radiological=true)` for the whole image.
fn read_general<T: Elem>(name: &str, settings: &FslSettings) -> Result<Volume<T>, ImageError> {
    let mut printed = String::new();
    let loaded = filename::return_validimagefilename(name, &mut printed)
        .and_then(|path| nifti::load_image(&path).map_err(|e| e.0));
    let loaded = match loaded {
        Ok(l) => l,
        Err(what) => {
            return Err(imthrow(
                &mut printed,
                &format!("Failed to read volume {name}\nError : {what}"),
                22,
            ));
        }
    };
    let mut header = loaded.header;
    let extensions = if settings.keep_extensions() {
        loaded.extensions
    } else {
        Vec::new()
    };
    if header.is_analyze() {
        let o = header.legacy.origin();
        header.sx[0] = header.pixdim[1];
        header.sy[1] = header.pixdim[2];
        header.sz[2] = header.pixdim[3];
        header.sx[3] = -(f64::from(o[0]) - 1.0) * header.pixdim[1];
        header.sy[3] = -(f64::from(o[1]) - 1.0) * header.pixdim[2];
        header.sz[3] = -(f64::from(o[2]) - 1.0) * header.pixdim[3];
        let s = header.sform();
        header.set_qform(&s);
        header.qform_code = nifti::XFORM_ALIGNED_ANAT;
        header.sform_code = nifti::XFORM_ALIGNED_ANAT;
    }
    for i in 1..=(header.dim[0].clamp(0, 7) as usize) {
        header.pixdim[i] = if header.pixdim[i] == 0.0 {
            1.0
        } else {
            header.pixdim[i].abs()
        };
    }
    let data = convert_and_scale::<T>(&loaded.data, &header).map_err(|what| {
        let mut p = printed.clone();
        imthrow(&mut p, &what, 8)
    })?;
    let mut v = Volume::<T>::new(0, 0, 0);
    v.data = data;
    v.dims = [
        header.dim[1],
        header.dim[2],
        header.dim[3],
        header.dim[4],
        header.dim[5],
        header.dim[6],
        header.dim[7],
    ];
    set_volume_properties(&header, &mut v);
    v.extensions = extensions;
    if !v.radiological_file {
        v.make_radiological(false);
    }
    Ok(v)
}

/// `set_volume_properties`.
fn set_volume_properties<T: Elem>(h: &NiftiHeader, v: &mut Volume<T>) {
    v.xdim = (h.pixdim[1] as f32).abs();
    v.ydim = (h.pixdim[2] as f32).abs();
    v.zdim = (h.pixdim[3] as f32).abs();
    v.tr = (h.pixdim[4] as f32).abs();
    v.pxdim5 = h.pixdim[5] as f32;
    v.pxdim6 = h.pixdim[6] as f32;
    v.pxdim7 = h.pixdim[7] as f32;
    let q = h.qform();
    let s = h.sform();
    let to_mat = |m: &[[f32; 4]; 4]| {
        let mut rows = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                rows[i][j] = f64::from(m[i][j]);
            }
        }
        arma::from_rows(rows)
    };
    v.sform_code = h.sform_code;
    v.sform = to_mat(&s);
    v.qform_code = h.qform_code;
    v.qform = to_mat(&q);
    v.radiological_file = nifti_left_right_order(h) == FSL_RADIOLOGICAL;
    v.intent_code = h.intent_code;
    v.intent_params = [h.intent_p1 as f32, h.intent_p2 as f32, h.intent_p3 as f32];
    v.display_min = h.cal_min as f32;
    v.display_max = h.cal_max as f32;
    let mut aux = [0u8; 24];
    let n = h.aux_file.len().min(24);
    aux[..n].copy_from_slice(&h.aux_file[..n]);
    v.aux_file = aux;
}

/// `NiftiGetLeftRightOrder` (determinants of the header's float matrices, in float).
fn nifti_left_right_order(h: &NiftiHeader) -> i32 {
    use nifti::legacy;
    let (mut dets, mut detq, mut det) = (-1.0f32, -1.0f32, -1.0f32);
    if h.qform_code != nifti::XFORM_UNKNOWN {
        detq = legacy::mat33_determ(&legacy::mat44_to_mat33(&h.qform()));
        det = detq;
    }
    if h.sform_code != nifti::XFORM_UNKNOWN {
        dets = legacy::mat33_determ(&legacy::mat44_to_mat33(&h.sform()));
        det = dets;
    }
    let mut order = if det < 0.0 {
        FSL_RADIOLOGICAL
    } else {
        FSL_NEUROLOGICAL
    };
    if h.sform_code != nifti::XFORM_UNKNOWN && h.qform_code != nifti::XFORM_UNKNOWN {
        if dets * detq < 0.0 {
            order = FSL_INCONSISTENT;
        }
        if f64::from((dets * detq).abs()) < 1e-12 {
            order = FSL_ZERODET;
        }
    }
    if f64::from(det.abs()) < 1e-12 {
        order = FSL_ZERODET;
    }
    order
}

/// `ConvertAndScaleNewNiftiBuffer`: the stored values to `T`, with the slope and intercept in
/// `float`.
fn convert_and_scale<T: Elem>(raw: &[u8], h: &NiftiHeader) -> Result<Vec<T>, String> {
    let mut slope = h.scl_slope as f32;
    let mut intercept = h.scl_inter as f32;
    if f64::from(slope.abs()) < 1e-30 {
        slope = 1.0;
        intercept = 0.0;
    }
    let doscaling = (f64::from(slope) - 1.0).abs() > 1e-30 || f64::from(intercept.abs()) > 1e-30;
    let n = h.n_elements();
    let same = T::DTYPE == h.datatype;
    // `convertbuffer`: `(D) v` when the slope is 1 and the intercept 0, else
    // `(D) (v * slope + intercept)` in the arithmetic of the stored type and float.
    let scale = (same && doscaling) || !same;
    let simple = slope == 1.0 && intercept == 0.0;
    macro_rules! conv_int {
        ($t:ty, $w:expr) => {{
            let mut out = Vec::with_capacity(n);
            for c in raw.chunks_exact($w).take(n) {
                let v = <$t>::from_le_bytes(c.try_into().unwrap());
                out.push(if !scale || simple {
                    T::from_f64(v as f64)
                } else {
                    // integer * float: the integer converted to float, then float arithmetic
                    T::from_f32((v as f32) * slope + intercept)
                });
            }
            out
        }};
    }
    macro_rules! conv_small_int {
        ($t:ty, $w:expr) => {{
            let mut out = Vec::with_capacity(n);
            for c in raw.chunks_exact($w).take(n) {
                let v = <$t>::from_le_bytes(c.try_into().unwrap());
                out.push(if !scale || simple {
                    T::from_f32(v as f32)
                } else {
                    T::from_f32((v as f32) * slope + intercept)
                });
            }
            out
        }};
    }
    let out: Vec<T> = match h.datatype {
        dt::SIGNED_SHORT => conv_small_int!(i16, 2),
        dt::UNSIGNED_CHAR => conv_small_int!(u8, 1),
        dt::INT8 => conv_small_int!(i8, 1),
        dt::UINT16 => conv_small_int!(u16, 2),
        dt::SIGNED_INT => {
            // (D) int: exact in double (for T = float, int to float rounds: from_f64 of the
            // exact value rounds once, as (float) int does).
            conv_int!(i32, 4)
        }
        dt::UINT32 => conv_int!(u32, 4),
        dt::INT64 => {
            let mut out = Vec::with_capacity(n);
            for c in raw.as_chunks::<8>().0.iter().take(n) {
                let v = i64::from_le_bytes(*c);
                out.push(if !scale || simple {
                    int64_to::<T>(v as i128)
                } else {
                    T::from_f32((v as f32) * slope + intercept)
                });
            }
            out
        }
        dt::UINT64 => {
            let mut out = Vec::with_capacity(n);
            for c in raw.as_chunks::<8>().0.iter().take(n) {
                let v = u64::from_le_bytes(*c);
                out.push(if !scale || simple {
                    int64_to::<T>(v as i128)
                } else {
                    T::from_f32((v as f32) * slope + intercept)
                });
            }
            out
        }
        dt::FLOAT => {
            let mut out = Vec::with_capacity(n);
            for c in raw.as_chunks::<4>().0.iter().take(n) {
                let v = f32::from_le_bytes(*c);
                out.push(if !scale || simple {
                    T::from_f32(v)
                } else {
                    T::from_f32(v * slope + intercept)
                });
            }
            out
        }
        dt::DOUBLE => {
            let mut out = Vec::with_capacity(n);
            for c in raw.as_chunks::<8>().0.iter().take(n) {
                let v = f64::from_le_bytes(*c);
                out.push(if !scale || simple {
                    T::from_f64(v)
                } else {
                    T::from_f64(v * f64::from(slope) + f64::from(intercept))
                });
            }
            out
        }
        other => return Err(format!("Fslread: DT {other} not supported")),
    };
    Ok(out)
}

/// `(T) v` for a 64-bit integer: to `float` with one rounding (x86-64's conversion), to
/// `double` likewise, to `short` and `int` by truncation (the value is integral).
fn int64_to<T: Elem>(v: i128) -> T {
    match T::DTYPE {
        dt::FLOAT => T::from_f32(v as f32),
        dt::DOUBLE => T::from_f64(v as f64),
        _ => T::from_f64(v as f64),
    }
}

// ------------------------------------------------------------------------------------------------
// Saving

/// The output types `FSLOUTPUTTYPE` accepts (`FslGetEnvOutputType`).
pub fn output_type(settings: &FslSettings) -> Result<i32, ImageError> {
    let Some(v) = settings.output_type.as_deref() else {
        return Err(ImageError {
            printed: "ERROR:: Environment variable FSLOUTPUTTYPE is not set!\n\
                      Please make sure that the appropriate configuration file is sourced by your shell (e.g. by putting it in .profile).\n\
                      e.g. bash or sh users add the line \". ${FSLDIR}/etc/fslconf/fsl.sh\"\n\
                      e.g. tcsh or csh users add the line \"source ${FSLDIR}/etc/fslconf/fsl.csh\"\n"
                .to_owned(),
            what: String::new(),
            type_name: "",
            exit: Some(1),
        });
    };
    Ok(match v {
        "NIFTI" => 1,
        "NIFTI2" => 2,
        "NIFTI_GZ" => 101,
        "NIFTI2_GZ" => 102,
        "NIFTI_PAIR" => 11,
        "NIFTI2_PAIR" => 12,
        "NIFTI_PAIR_GZ" => 111,
        "NIFTI2_PAIR_GZ" => 112,
        other => {
            return Err(ImageError {
                printed: format!(
                    "ERROR:: Unrecognised value ({other}) of environment variable FSLOUTPUTTYPE\n\
                     Legal values are: NIFTI, NIFTI_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ\n"
                ),
                what: String::new(),
                type_name: "",
                exit: Some(1),
            });
        }
    })
}

/// `outputExtension(filetype)`.
pub fn output_extension(filetype: i32) -> &'static str {
    match filetype {
        1 | 2 => ".nii",
        101 | 102 => ".nii.gz",
        10..=12 => ".hdr",
        111 | 112 | 100 => ".hdr.gz",
        _ => "",
    }
}

/// How `save_volume` went: `Ok(None)` when written, `Ok(Some(-1))` when newimage returns -1
/// without writing (an image with no volume).
pub type SaveResult = Result<Option<i32>, ImageError>;

/// Whether NewNifti's `vox_offset` (FSL-56) would differ from the correct one when `v` is saved
/// with `settings`: a single-file output carrying two or more extensions of different sizes.
pub fn offset_defect_applies<T: Elem>(v: &Volume<T>, settings: &FslSettings) -> bool {
    let single = output_type(settings).is_ok_and(|t| output_extension(t).starts_with(".nii"));
    single
        && v.extensions.len() > 1
        && v.extensions
            .iter()
            .any(|e| e.size() != v.extensions[0].size())
}

/// `save_volume(source, filename)`: mirrored back to the file's left-right order if it was read
/// mirrored, a fresh FSL header, the type of `FSLOUTPUTTYPE`. `reproduce_offset` keeps NewNifti's
/// `vox_offset` of several extensions (FSL-56).
pub fn save_volume<T: Elem>(
    source: &Volume<T>,
    name: &str,
    settings: &FslSettings,
    reproduce_offset: bool,
) -> SaveResult {
    if source.tsize() < 1 {
        return Ok(Some(-1));
    }
    let mut v = source.clone();
    let currently_rad = v.left_right_order() == FSL_RADIOLOGICAL;
    if !v.radiological_file && currently_rad {
        v.make_neurological();
    }
    let filetype = output_type(settings)?;
    let mut h = set_fsl_hdr(&v);
    h.description = BUILDSTRING.as_bytes().to_vec();
    let version = filetype % 100 % 10 % 3;
    let single = filetype % 100 < 10;
    h.set_nifti_version(version, single);
    h.bits_per_voxel = (T::SIZE * 8) as i32;
    let path = format!(
        "{}{}",
        filename::make_basename(name),
        output_extension(filetype)
    );
    let mut bytes = Vec::with_capacity(v.data.len() * T::SIZE);
    for &x in &v.data {
        x.put_le(&mut bytes);
    }
    nifti::save_image(
        &path,
        &bytes,
        &v.extensions,
        &h,
        filetype >= 100,
        reproduce_offset,
    )
    .map_err(|e| {
        let mut p = String::new();
        imthrow(&mut p, &e.0, 22)
    })?;
    Ok(None)
}

/// `set_fsl_hdr`.
pub fn set_fsl_hdr<T: Elem>(v: &Volume<T>) -> NiftiHeader {
    let mut h = NiftiHeader::default();
    h.dim[0] = i64::from(v.dimensionality());
    for k in 0..7 {
        h.dim[k + 1] = v.dims[k];
    }
    h.datatype = T::DTYPE;
    h.pixdim[1] = f64::from(v.xdim);
    h.pixdim[2] = f64::from(v.ydim);
    h.pixdim[3] = f64::from(v.zdim);
    h.pixdim[4] = f64::from(v.tr);
    h.pixdim[5] = f64::from(v.pxdim5);
    h.pixdim[6] = f64::from(v.pxdim6);
    h.pixdim[7] = f64::from(v.pxdim7);
    h.sform_code = v.sform_code;
    h.qform_code = v.qform_code;
    let to44 = |m: &Mat4| {
        let rows = arma::to_rows(m);
        let mut r = [[0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                r[i][j] = rows[i][j] as f32;
            }
        }
        r
    };
    h.set_sform(&to44(&v.sform));
    h.set_qform(&to44(&v.qform));
    h.intent_code = v.intent_code;
    h.intent_p1 = f64::from(v.intent_params[0]);
    h.intent_p2 = f64::from(v.intent_params[1]);
    h.intent_p3 = f64::from(v.intent_params[2]);
    h.scl_slope = 1.0;
    h.scl_inter = 0.0;
    h.cal_min = f64::from(v.display_min);
    h.cal_max = f64::from(v.display_max);
    let end = v.aux_file.iter().position(|&c| c == 0).unwrap_or(24);
    h.aux_file = v.aux_file[..end].to_vec();
    h.units = nifti::UNITS_SEC | nifti::UNITS_MM;
    h.slice_ordering = 0;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirroring_twice_restores_the_matrices_of_a_simple_grid() {
        let mut v = Volume::<f32>::new(4, 3, 2);
        v.xdim = 2.0;
        v.ydim = 2.0;
        v.zdim = 3.0;
        v.sform = arma::from_rows([
            [2.0, 0.0, 0.0, -3.0],
            [0.0, 2.0, 0.0, -2.0],
            [0.0, 0.0, 3.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        v.qform = v.sform;
        v.sform_code = 1;
        v.qform_code = 1;
        for (i, d) in v.data.iter_mut().enumerate() {
            *d = i as f32;
        }
        let before = v.clone();
        assert_eq!(v.left_right_order(), FSL_NEUROLOGICAL);
        v.swap_lr(false);
        assert_eq!(v.left_right_order(), FSL_RADIOLOGICAL);
        assert_eq!(v.value(0, 0, 0), 3.0);
        v.swap_lr(false);
        assert_eq!(v.data, before.data);
        assert_eq!(v.sform, before.sform);
    }

    #[test]
    fn robust_limits_of_a_ramp() {
        let mut v = Volume::<f32>::new(10, 10, 10);
        for (i, d) in v.data.iter_mut().enumerate() {
            *d = i as f32;
        }
        let (lo, hi) = v.robust_limits();
        assert!(lo > 0.0 && lo < 40.0, "{lo}");
        assert!(hi > 960.0 && hi <= 1000.0, "{hi}");
    }
}
