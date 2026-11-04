// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (newimagefns.cc: box_kernel,
// gaussian_kernel3D, spherical_kernel, fsllog2; newimagefns.h: morphfilter, dilateval,
// ext_edge, dilall, convolve, convolve with a mask, efficient_convolve, generic_convolve,
// bandpass_temporal_filter), with
// Armadillo 14.2.1's `max`, `min` and `sort` of a column vector as fsl-armawrap 0.7.0 calls them.
// Copyright (C) 1999-2018 University of Oxford (Mark Jenkinson, Matthew Webster et al.); Armadillo
// Copyright 2008-2016 Conrad Sanderson and National ICT Australia (Apache-2.0). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL
// Licence (see LICENSE).
//! newimage's kernels, morphological filters and convolutions for every element type fslmaths
//! computes in (`char`, `short`, `int`, `float`, `double`).
//!
//! - [`Kern`]: a kernel (`volume<float>`) with its sizes as FSL sets them: a size of 0 or below
//!   leaves the kernel without values, and every loop over it empty, as in FSL.
//! - [`box_kernel_vox`], [`box_kernel_mm`], [`gaussian_kernel3d_mm`], [`spherical_kernel`]: the
//!   kernels of `-kernel` and `-s`.
//! - [`morphfilter`]: mean, modal and maximum dilation, the two erosions, the median.
//! - [`dilall`]: dilation by the mean of the set neighbours until every voxel is set.
//! - [`generic_convolve`]: separable (along x, then y, then z, renormalised by the kernel's
//!   overlap with the image) or not (the kernel normalised to sum 1, the direct sum or the FFT,
//!   then optionally renormalised by the convolution of a volume of ones).
//! - [`bandpass_temporal_filter`]: fslmaths' `-bptf`, with the checks of its half-widths
//!   ([`bptf_half_widths`], [`bptf_stop`]).
//!
//! Conversions to the element type (`(T) x` of a `double` or a `float`) go through the caller's
//! function, so that fslmaths' D13 handling of undefined conversions (FSL-151) applies.

use std::collections::VecDeque;

use crate::arma;
use crate::cnum;
use crate::glibc_double;
use crate::glibc_float;
use crate::nifti::dt;
use crate::sse::{self, S64};
use crate::stats;
use crate::stdsort;
use crate::volops::{Arith, Op};
use crate::volume::{FslError, Volume};

/// A kernel: newimage's `volume<float>` of sizes `dims` (`new T[x*y*z]` when that product is
/// positive, so a kernel with two negative sizes still holds values no loop reaches).
#[derive(Clone, Debug, PartialEq)]
pub struct Kern {
    pub dims: [i64; 3],
    pub data: Vec<f32>,
}

impl Kern {
    /// `volume<float>(x, y, z)` filled with `v` (`new_kernel = v`).
    pub fn filled(x: i64, y: i64, z: i64, v: f32) -> Kern {
        let n = x.wrapping_mul(y).wrapping_mul(z);
        Kern {
            dims: [x, y, z],
            data: vec![v; n.max(0) as usize],
        }
    }

    /// The kernel read from an image (`read_volume(kernel, name)`).
    pub fn from_volume(v: &Volume<f32>) -> Kern {
        let n = v.nvoxels().max(0) as usize;
        Kern {
            dims: [v.xsize(), v.ysize(), v.zsize()],
            data: v.data[..n].to_vec(),
        }
    }

    /// `nvoxels()`: the product of the sizes.
    pub fn nvoxels(&self) -> i64 {
        self.dims[0]
            .wrapping_mul(self.dims[1])
            .wrapping_mul(self.dims[2])
    }

    /// Whether a size is 0 or below (no loop over the kernel runs).
    pub fn has_no_voxels(&self) -> bool {
        self.dims.iter().any(|&d| d <= 0)
    }

    #[inline]
    fn in_bounds(&self, x: i64, y: i64, z: i64) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < self.dims[0] && y < self.dims[1] && z < self.dims[2]
    }

    /// `kernel(x, y, z)`: the value, 0 outside (zero padding).
    #[inline]
    pub fn at(&self, x: i64, y: i64, z: i64) -> f32 {
        if self.in_bounds(x, y, z) {
            self.data[((z * self.dims[1] + y) * self.dims[0] + x) as usize]
        } else {
            0.0
        }
    }

    /// `sum()`: the values added in FSL's blocks; `true` when there are none (FSL then prints
    /// `ERROR:: Empty mask image`).
    pub fn sum(&self) -> (f64, bool) {
        stats::masked_sum(&self.data, None)
    }

    /// morphfilter's and the masked convolve's test for an off-centre kernel:
    /// `(max - min) % 2 == 1` along an axis (C's `%`).
    pub fn off_centre_by_extent(&self) -> bool {
        self.dims.iter().any(|&d| (d - 1) % 2 == 1)
    }

    /// The plain convolve's test: a size `% 2 == 0` (C's `%`).
    pub fn off_centre_by_size(&self) -> bool {
        self.dims.iter().any(|&d| d % 2 == 0)
    }
}

/// `box_kernel(x, y, z)` (voxel sizes): ones.
pub fn box_kernel_vox(x: i32, y: i32, z: i32) -> Kern {
    Kern::filled(i64::from(x), i64::from(y), i64::from(z), 1.0)
}

/// `(int) x` of a `float` as x86-64 converts it (out of range: `INT_MIN`).
#[inline]
fn f2i(x: f32) -> i32 {
    crate::cnum::f32_to_i32(x)
}

/// `box_kernel(length, xdim, ydim, zdim)` (millimetres): `2 floor(length / dim / 2) + 1` voxels
/// along each axis, in `float`.
pub fn box_kernel_mm(length: f32, xdim: f32, ydim: f32, zdim: f32) -> Kern {
    let n = |d: f32| {
        f2i((length / d / 2.0).floor())
            .wrapping_mul(2)
            .wrapping_add(1)
    };
    box_kernel_vox(n(xdim), n(ydim), n(zdim))
}

/// The kernel's sizes and whether its Gaussian divides by zero (`2 sigma^2 == 0` in `float`:
/// FSL's centre value is then `exp(0/0)`, NaN; FSL-160).
pub struct Gauss {
    pub kernel: Kern,
    pub zero_width: bool,
}

/// `gaussian_kernel3D(sigma, xdim, ydim, zdim, cutoff)`: `2 ceil(sigma cutoff / dim) + 1` voxels
/// along each axis, the values `expf(-(x^2 dx^2 + y^2 dy^2 + z^2 dz^2) / (2 sigma^2))` (glibc's
/// `expf`, every operation in `float`), not normalised. `impulse` (FSL-160's correction) gives
/// the unit impulse where `2 sigma^2` is 0.
pub fn gaussian_kernel3d_mm(
    sigma: f32,
    xdim: f32,
    ydim: f32,
    zdim: f32,
    cutoff: f32,
    impulse: bool,
) -> Gauss {
    let n = |d: f32| {
        f2i((sigma * cutoff / d).ceil())
            .wrapping_mul(2)
            .wrapping_add(1)
    };
    let (sx, sy, sz) = (n(xdim), n(ydim), n(zdim));
    let mut k = Kern::filled(i64::from(sx), i64::from(sy), i64::from(sz), 0.0);
    let (dx2, dy2, dz2) = (xdim * xdim, ydim * ydim, zdim * zdim);
    let den = 2.0 * sigma * sigma;
    let zero_width = den == 0.0;
    for z in -sz / 2..=sz / 2 {
        for y in -sy / 2..=sy / 2 {
            for x in -sx / 2..=sx / 2 {
                let num = (x.wrapping_mul(x)) as f32 * dx2
                    + (y.wrapping_mul(y)) as f32 * dy2
                    + (z.wrapping_mul(z)) as f32 * dz2;
                let v = if zero_width && impulse {
                    if x == 0 && y == 0 && z == 0 { 1.0 } else { 0.0 }
                } else {
                    // the centre's 0 / 0 is SSE's default NaN, on ARM too (its own NaN has the
                    // sign bit clear: the golden case of FSL-160's reproduce failed on ARM)
                    glibc_float::expf(sse::div_f32(-num, den))
                };
                let (ix, iy, iz) = (
                    i64::from(x + sx / 2),
                    i64::from(y + sy / 2),
                    i64::from(z + sz / 2),
                );
                if k.in_bounds(ix, iy, iz) {
                    let i = ((iz * k.dims[1] + iy) * k.dims[0] + ix) as usize;
                    k.data[i] = v;
                }
            }
        }
    }
    Gauss {
        kernel: k,
        zero_width,
    }
}

/// `spherical_kernel(radius, xdim, ydim, zdim)`: `2 round(radius / dim) + 1` voxels along each
/// axis, 1 where `x^2 dx^2 + y^2 dy^2 + z^2 dz^2 <= radius^2` (in `float`), else 0.
pub fn spherical_kernel(radius: f32, xdim: f32, ydim: f32, zdim: f32) -> Kern {
    let n = |d: f32| {
        crate::interp::round_f32(radius / d)
            .wrapping_mul(2)
            .wrapping_add(1)
    };
    let (sx, sy, sz) = (n(xdim), n(ydim), n(zdim));
    let mut k = Kern::filled(i64::from(sx), i64::from(sy), i64::from(sz), 0.0);
    let (dx2, dy2, dz2) = (xdim * xdim, ydim * ydim, zdim * zdim);
    let r2 = radius * radius;
    for z in -sz / 2..=sz / 2 {
        for y in -sy / 2..=sy / 2 {
            for x in -sx / 2..=sx / 2 {
                let d = (x.wrapping_mul(x)) as f32 * dx2
                    + (y.wrapping_mul(y)) as f32 * dy2
                    + (z.wrapping_mul(z)) as f32 * dz2;
                let (ix, iy, iz) = (
                    i64::from(x + sx / 2),
                    i64::from(y + sy / 2),
                    i64::from(z + sz / 2),
                );
                if d <= r2 && k.in_bounds(ix, iy, iz) {
                    let i = ((iz * k.dims[1] + iy) * k.dims[0] + ix) as usize;
                    k.data[i] = 1.0;
                }
            }
        }
    }
    k
}

// ------------------------------------------------------------------------------------------------
// Armadillo's reductions and sort of a column vector

/// `sort(X, "ascend")` / `"descend"` of a column vector: libstdc++'s `std::sort` with `a < b`
/// (or `a > b`). Armadillo refuses a vector holding a NaN (`sort(): detected NaN`, a
/// `std::logic_error`): `false` then, the vector untouched.
pub fn arma_sort(v: &mut [f64], descend: bool) -> bool {
    if v.iter().any(|x| x.is_nan()) {
        return false;
    }
    if descend {
        stdsort::sort_by(v, |a, b| a > b);
    } else {
        stdsort::sort_by(v, |a, b| a < b);
    }
    true
}

/// Armadillo's exception for a NaN in `sort()` (fsl-armawrap's `SortAscending`/`SortDescending`).
pub fn sort_nan_error() -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::logic_error",
        what: "sort(): detected NaN".into(),
    }
}

// ------------------------------------------------------------------------------------------------
// morphfilter

/// morphfilter's filter types (`dilateM`, `dilateD`, `dilate`, `erodeS`, `erode`, `median`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Morph {
    DilateM,
    DilateD,
    Dilate,
    ErodeS,
    Erode,
    Median,
}

/// What [`morphfilter`] prints before filtering, in FSL's order: the empty kernel's sum message,
/// then the off-centre warning.
pub fn morph_messages(k: &Kern) -> Vec<String> {
    let mut m = Vec::new();
    let dummy: Vec<f32> = k
        .data
        .iter()
        .map(|&v| {
            if (0.5..=f32::MAX).contains(&v) {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    if stats::masked_sum(&dummy, None).1 {
        m.push(stats::EMPTY_MASK.to_owned());
    }
    if k.off_centre_by_extent() {
        m.push(
            "WARNING:: Off-centre morphfilter being performed as kernel has even dimensions"
                .to_owned(),
        );
    }
    m
}

/// The number of values morphfilter's vector holds (`nker`: the kernel values in
/// `[0.5, FLT_MAX]`).
fn nker(k: &Kern) -> usize {
    k.data
        .iter()
        .filter(|&&v| (0.5..=f32::MAX).contains(&v))
        .count()
}

/// What [`morphfilter`] met that FSL does not compute: a NaN in a vector it sorts (`-fmedian`,
/// `-dilD`: Armadillo's exception), more values than its vector holds (a `+inf` kernel value).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MorphIssues {
    pub nan_sorted: bool,
    pub overflow: bool,
}

/// `morphfilter(source, kernel, type)` of one volume (`dims` its sizes): for each voxel, the
/// source values under the kernel's values above 0.5 (inside the volume; for the dilations only
/// the non-zero ones), as `double`:
/// - `median`: sorted ascending, the `max(1, (n + 2) / 2)`-th;
/// - `dilate`, `erode`: Armadillo's maximum, minimum (NaN values left out);
/// - `erodeS`: 0 where the voxel is not 0 and the minimum is 0, else the voxel;
/// - `dilateM`: a zero voxel becomes the values' sum (Armadillo's `accu`) over their number;
/// - `dilateD`: a zero voxel becomes the mode of the values sorted descending (the first,
///   largest, of the most frequent).
///
/// A voxel without values keeps its value. A NaN in a sorted vector (Armadillo refuses it) is
/// left out of the median or the mode (FSL-162's correction; NaN when nothing is left) and
/// reported, as a vector that would overflow. `conv` is `(T) x` of a `double`.
pub fn morphfilter<T: Arith>(
    src: &[T],
    dims: [i64; 3],
    k: &Kern,
    kind: Morph,
    conv: &mut impl FnMut(f64) -> T,
    issues: &mut MorphIssues,
) -> Vec<T> {
    let (nx, ny, nz) = (dims[0], dims[1], dims[2]);
    let mut result = vec![T::from_int(0); src.len()];
    let cap = nker(k);
    let (kmaxx, kmaxy, kmaxz) = (k.dims[0] - 1, k.dims[1] - 1, k.dims[2] - 1);
    let (midx, midy, midz) = (kmaxx / 2, kmaxy / 2, kmaxz / 2);
    let at = |x: i64, y: i64, z: i64| src[((z * ny + y) * nx + x) as usize];
    let dilation = matches!(kind, Morph::DilateM | Morph::DilateD);
    let mut vals: Vec<f64> = Vec::with_capacity(cap + 1);
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                vals.clear();
                let s0 = at(x, y, z);
                for mz in 0.max(midz - z)..=kmaxz.min(nz - 1 - z + midz) {
                    for my in 0.max(midy - y)..=kmaxy.min(ny - 1 - y + midy) {
                        for mx in 0.max(midx - x)..=kmaxx.min(nx - 1 - x + midx) {
                            if f64::from(k.at(mx, my, mz)) > 0.5 {
                                let s = at(x + mx - midx, y + my - midy, z + mz - midz);
                                if !dilation || T::nonzero(s) {
                                    vals.push(s.to_f64());
                                }
                            }
                        }
                    }
                }
                if vals.len() > cap {
                    issues.overflow = true;
                }
                let i = ((z * ny + y) * nx + x) as usize;
                if vals.is_empty() {
                    result[i] = s0;
                    continue;
                }
                result[i] = match kind {
                    Morph::Median => {
                        if !arma_sort(&mut vals, false) {
                            issues.nan_sorted = true;
                            vals.retain(|v| !v.is_nan());
                            stdsort::sort_by(&mut vals, |a, b| a < b);
                        }
                        let n = vals.len();
                        if n == 0 {
                            conv(f64::NAN)
                        } else {
                            conv(vals[((n + 2) / 2).max(1) - 1])
                        }
                    }
                    Morph::Dilate => conv(arma::direct_max(&vals)),
                    Morph::Erode => conv(arma::direct_min(&vals)),
                    Morph::ErodeS => {
                        if T::nonzero(s0) && arma::direct_min(&vals) == 0.0 {
                            T::from_int(0)
                        } else {
                            s0
                        }
                    }
                    Morph::DilateM => {
                        if T::nonzero(s0) {
                            s0
                        } else {
                            conv(arma::accu(&vals) / vals.len() as f64)
                        }
                    }
                    Morph::DilateD => {
                        if T::nonzero(s0) {
                            s0
                        } else {
                            if !arma_sort(&mut vals, true) {
                                issues.nan_sorted = true;
                                vals.retain(|v| !v.is_nan());
                                stdsort::sort_by(&mut vals, |a, b| a > b);
                            }
                            conv(mode_of_sorted(&vals))
                        }
                    }
                };
            }
        }
    }
    result
}

/// dilateD's mode of values sorted descending: the first value whose run is longer than every
/// earlier run (NaN for no values).
fn mode_of_sorted(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut mode = v[0];
    let (mut maxn, mut currentn) = (1, 1);
    for i in 1..v.len() {
        if v[i] == v[i - 1] {
            currentn += 1;
        } else {
            currentn = 1;
        }
        if currentn > maxn {
            mode = v[i];
            maxn = currentn;
        }
    }
    mode
}

// ------------------------------------------------------------------------------------------------
// dilall

/// `dilall(input, mask)` of one volume, the mask `binarise(0, 0, inclusive, true)` of the input
/// (its non-zero voxels, NaN included): the voxels next to the mask (26-connected) queued in
/// z, y, x order, then each unset one taken from the queue set to the mean (in `double`, in
/// z, y, x order) of its set neighbours and its own unset neighbours next to the mask queued.
/// `conv` is `(T) x` of a `double`.
pub fn dilall<T: Arith>(input: &[T], dims: [i64; 3], conv: &mut impl FnMut(f64) -> T) -> Vec<T> {
    let (nx, ny, nz) = (dims[0], dims[1], dims[2]);
    let idx = |x: i64, y: i64, z: i64| ((z * ny + y) * nx + x) as usize;
    let mut im = input.to_vec();
    // (T) 0.5 is 0 for the integer types: `mask > (T) 0.5` is `mask > 0` there; the mask is 0 or 1
    let mut mask: Vec<bool> = input.iter().map(|&v| T::nonzero(v)).collect();
    let ext_edge = |mask: &[bool], x: i64, y: i64, z: i64| -> bool {
        if mask[idx(x, y, z)] {
            return false;
        }
        for zz in (z - 1).max(0)..=(z + 1).min(nz - 1) {
            for yy in (y - 1).max(0)..=(y + 1).min(ny - 1) {
                for xx in (x - 1).max(0)..=(x + 1).min(nx - 1) {
                    if mask[idx(xx, yy, zz)] && !(xx == x && yy == y && zz == z) {
                        return true;
                    }
                }
            }
        }
        false
    };
    let mut queue: VecDeque<(i64, i64, i64)> = VecDeque::new();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if ext_edge(&mask, x, y, z) {
                    queue.push_front((x, y, z));
                }
            }
        }
    }
    while let Some((x, y, z)) = queue.pop_back() {
        if mask[idx(x, y, z)] {
            continue;
        }
        let mut sum = 0f64;
        let mut n = 0i32;
        for zz in (z - 1).max(0)..=(z + 1).min(nz - 1) {
            for yy in (y - 1).max(0)..=(y + 1).min(ny - 1) {
                for xx in (x - 1).max(0)..=(x + 1).min(nx - 1) {
                    if mask[idx(xx, yy, zz)] && !(xx == x && yy == y && zz == z) {
                        // SSE's NaNs, the neighbour first: FSL's build converts the neighbour into
                        // the register it adds the sum to (`addsd`), so of a NaN neighbour and the
                        // default NaN of an earlier `inf + -inf` it passes on the neighbour's
                        // (random testing's case `dilall-nan-order`, 2026-11-04)
                        sum = sse::add_f64(im[idx(xx, yy, zz)].to_f64(), sum);
                        n += 1;
                    }
                }
            }
        }
        im[idx(x, y, z)] = conv(sse::div_f64(sum, f64::from(n.max(1))));
        mask[idx(x, y, z)] = true;
        for zz in (z - 1).max(0)..=(z + 1).min(nz - 1) {
            for yy in (y - 1).max(0)..=(y + 1).min(ny - 1) {
                for xx in (x - 1).max(0)..=(x + 1).min(nx - 1) {
                    if ext_edge(&mask, xx, yy, zz) {
                        queue.push_front((xx, yy, zz));
                    }
                }
            }
        }
    }
    im
}

// ------------------------------------------------------------------------------------------------
// Convolutions

/// `val += s * k` of the plain convolve: `float` for every type but `double` (`(double) s * k`
/// added to the `float` sum in `double`).
#[inline]
fn madd_plain<T: Arith>(val: f32, s: T, k: f32) -> f32 {
    if T::DTYPE == dt::DOUBLE {
        (f64::from(val) + s.to_f64() * f64::from(k)) as f32
    } else {
        val + s.to_f32() * k
    }
}

/// [`madd_plain`] with SSE's NaNs (the running sum the first operand).
#[inline]
fn madd_plain_sse<T: Arith>(val: f32, s: T, k: f32) -> f32 {
    if T::DTYPE == dt::DOUBLE {
        sse::add_f64(f64::from(val), sse::mul_f64(s.to_f64(), f64::from(k))) as f32
    } else {
        sse::add_f32(val, sse::mul_f32(s.to_f32(), k))
    }
}

/// `convolve(source, kernel)` of one volume (no mask, not normalised): each voxel the `float`
/// sum over the whole kernel (x fastest) of the source, extrapolated outside the volume, times
/// the kernel, centred on `(size - 1) / 2`; `conv` is `(T)` of that `float`.
pub fn convolve_plain<T: Arith>(
    src: &Volume<T>,
    k: &Kern,
    conv: &mut impl FnMut(f64) -> T,
) -> Volume<T> {
    let mut result = src.clone();
    let [kx, ky, kz] = k.dims;
    let (midx, midy, midz) = ((kx - 1) / 2, (ky - 1) / 2, (kz - 1) / 2);
    let (nx, ny, nz) = (src.xsize(), src.ysize(), src.zsize());
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let mut val = 0f32;
                for mz in 0..kz {
                    for my in 0..ky {
                        for mx in 0..kx {
                            let s = src.get(x + mx - midx, y + my - midy, z + mz - midz);
                            val = madd_plain(val, s, k.at(mx, my, mz));
                        }
                    }
                }
                if val.is_nan() {
                    val = 0.0;
                    for mz in 0..kz {
                        for my in 0..ky {
                            for mx in 0..kx {
                                let s = src.get(x + mx - midx, y + my - midy, z + mz - midz);
                                val = madd_plain_sse(val, s, k.at(mx, my, mz));
                            }
                        }
                    }
                }
                let i = src.idx(x, y, z);
                result.data[i] = conv(f64::from(val));
            }
        }
    }
    result
}

/// The plain convolve's warning (printed by each call when a kernel size is even).
pub const OFF_CENTRE_CONVOLUTION: &str =
    "WARNING:: Off-centre convolution being performed as kernel has even dimensions";

/// `fsllog2(x)`: `log(x) / log(2)` with `log(float)` (glibc's `logf`), in `double`, as `float`.
pub fn fsllog2(x: f32) -> f32 {
    (f64::from(glibc_float::logf(x)) / std::f64::consts::LN_2) as f32
}

/// efficient_convolve's choice: the FFT when `nvox nker > 44 * 2 nvox log2(2 nvox)` (in
/// `float`).
pub fn uses_fft(nvox: i64, nker: i64) -> bool {
    let two = 2i64.wrapping_mul(nvox) as f32;
    let offt = two * fsllog2(two);
    let osum = nvox as f32 * nker as f32;
    osum > offt * 44.0
}

/// Why [`generic_convolve`] stopped: an integer division by zero in the renormalisation (FSL's
/// trap), or an exception.
#[derive(Debug)]
pub enum ConvStop {
    Trap,
    Exception(FslError),
}

/// One pass of the separable path: `convolve(source, ker, mask, ignoremask=true, renormalise)`
/// with a kernel of `ker.len()` values along `axis`, centred on `(n - 1) / 2`: each voxel the
/// `float` sums of `(double) s * k` and of `k` over the kernel's voxels inside the volume, then
/// `(T)(val / norm)` when renormalising and `|norm| > 1e-12`, else `(T) val`.
fn convolve_masked_axis<T: Arith>(
    src: &[T],
    dims: [i64; 3],
    ker: &[f64],
    axis: usize,
    renormalise: bool,
    conv: &mut impl FnMut(f64) -> T,
) -> Vec<T> {
    let (nx, ny, nz) = (dims[0], dims[1], dims[2]);
    let n = ker.len() as i64;
    let mid = (n - 1) / 2;
    let mut out = src.to_vec();
    let at = |x: i64, y: i64, z: i64| src[((z * ny + y) * nx + x) as usize];
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let (mut val, mut norm) = (0f32, 0f32);
                let mut nan = false;
                for m in 0..n {
                    let d = m - mid;
                    let (x2, y2, z2) = match axis {
                        0 => (x + d, y, z),
                        1 => (x, y + d, z),
                        _ => (x, y, z + d),
                    };
                    if x2 >= 0 && x2 < nx && y2 >= 0 && y2 < ny && z2 >= 0 && z2 < nz {
                        let k = ker[m as usize];
                        val = (f64::from(val) + at(x2, y2, z2).to_f64() * k) as f32;
                        norm = (f64::from(norm) + k) as f32;
                        nan |= val.is_nan() || norm.is_nan();
                    }
                }
                if nan {
                    (val, norm) = (0.0, 0.0);
                    for m in 0..n {
                        let d = m - mid;
                        let (x2, y2, z2) = match axis {
                            0 => (x + d, y, z),
                            1 => (x, y + d, z),
                            _ => (x, y, z + d),
                        };
                        if x2 >= 0 && x2 < nx && y2 >= 0 && y2 < ny && z2 >= 0 && z2 < nz {
                            let k = ker[m as usize];
                            let p = sse::mul_f64(at(x2, y2, z2).to_f64(), k);
                            val = sse::add_f64(f64::from(val), p) as f32;
                            norm = sse::add_f64(f64::from(norm), k) as f32;
                        }
                    }
                }
                let i = ((z * ny + y) * nx + x) as usize;
                out[i] = if renormalise && f64::from(norm.abs()) > 1e-12 {
                    conv(f64::from(sse::div_f32(val, norm)))
                } else {
                    conv(f64::from(val))
                };
            }
        }
    }
    out
}

/// `generic_convolve(source, kernel, separable, renormalise)` of every volume of `src`.
///
/// - Separable: the kernel's lines through its centre (`kernel(n, ysize/2, zsize/2)`, ...) as
///   `double` kernels, every volume convolved along x, then every volume along y, then z
///   ([`convolve_masked_axis`]).
/// - Not separable: the kernel divided (in `float`) by its sum when that is not 0, each volume
///   convolved by efficient_convolve (the direct sum, [`convolve_plain`], or, for a large kernel,
///   the FFT, [`crate::kissfft::fft_convolve`], which prints no warning, its twiddle factors
///   from `libm`), then, when renormalising, divided by the same convolution of a volume of
///   ones (an integer division by zero is FSL's trap).
///
/// `msgs` receives what FSL prints, in its order (the empty kernel's sum message, the
/// off-centre warnings of each call).
pub fn generic_convolve<T: Arith>(
    src: &Volume<T>,
    k: &Kern,
    separable: bool,
    renormalise: bool,
    libm: &larmorx_core::math::Libm,
    msgs: &mut Vec<String>,
    conv: &mut impl FnMut(f64) -> T,
) -> Result<Volume<T>, ConvStop> {
    let mut result = src.clone();
    let nvox = src.nvoxels().max(0) as usize;
    let dims = [src.xsize(), src.ysize(), src.zsize()];
    let nt = src.tsize().max(0) as usize;
    if separable {
        let [kx, ky, kz] = k.dims;
        let line = |axis: usize| -> Vec<f64> {
            let n = k.dims[axis].max(0);
            (0..n)
                .map(|m| {
                    f64::from(match axis {
                        0 => k.at(m, ky / 2, kz / 2),
                        1 => k.at(kx / 2, m, kz / 2),
                        _ => k.at(kx / 2, ky / 2, m),
                    })
                })
                .collect()
        };
        for axis in 0..3 {
            let ker = line(axis);
            let warn = (k.dims[axis] - 1) % 2 == 1;
            for t in 0..nt {
                if warn {
                    msgs.push(OFF_CENTRE_CONVOLUTION.to_owned());
                }
                let r = &mut result.data[t * nvox..(t + 1) * nvox];
                let out = convolve_masked_axis(r, dims, &ker, axis, renormalise, conv);
                r.copy_from_slice(&out);
            }
        }
        return Ok(result);
    }
    let mut norm_kernel = k.clone();
    let (s, empty) = k.sum();
    if empty {
        msgs.push(stats::EMPTY_MASK.to_owned());
    }
    if s != 0.0 {
        let d = s as f32;
        for v in &mut norm_kernel.data {
            *v = sse::div_f32(*v, d);
        }
    }
    let fft = uses_fft(nvox as i64, norm_kernel.nvoxels());
    let warn = !fft && norm_kernel.off_centre_by_size();
    let efficient = |v: &Volume<T>, conv: &mut dyn FnMut(f64) -> T| -> Volume<T> {
        if fft {
            crate::kissfft::fft_convolve(v, norm_kernel.dims, &norm_kernel.data, libm, &mut |x| {
                conv(x)
            })
        } else {
            convolve_plain(v, &norm_kernel, &mut |x| conv(x))
        }
    };
    for t in 0..nt {
        if warn {
            msgs.push(OFF_CENTRE_CONVOLUTION.to_owned());
        }
        let v = src.sub_volume(t as i64).map_err(ConvStop::Exception)?;
        let r = efficient(&v, conv);
        result.data[t * nvox..(t + 1) * nvox].copy_from_slice(&r.data);
    }
    if renormalise {
        let mut unitary = src.clone();
        unitary.data.fill(T::from_int(1));
        for t in 0..nt {
            if warn {
                msgs.push(OFF_CENTRE_CONVOLUTION.to_owned());
            }
            let v = unitary.sub_volume(t as i64).map_err(ConvStop::Exception)?;
            let r = efficient(&v, conv);
            unitary.data[t * nvox..(t + 1) * nvox].copy_from_slice(&r.data);
        }
        match result.op_volume(Op::Div, &unitary) {
            Err(e) => return Err(ConvStop::Exception(e)),
            Ok(Err(_)) => return Err(ConvStop::Trap),
            Ok(Ok(())) => {}
        }
    }
    Ok(result)
}

// ------------------------------------------------------------------------------------------------
// subsample_by_2, edge_strengthen, log_edge_detect

/// The sum of values in C's arithmetic of `T` (`char` and `short` promoted to `int`, `int`
/// wrapping, `float` in `float`, `double` in `double`), as `double`; `sse` adds with SSE's NaNs.
fn csum<T: Arith>(v: &[T], use_sse: bool) -> f64 {
    match T::DTYPE {
        dt::FLOAT => {
            let mut s = v[0].to_f32();
            for x in &v[1..] {
                s = if use_sse {
                    sse::add_f32(s, x.to_f32())
                } else {
                    s + x.to_f32()
                };
            }
            f64::from(s)
        }
        dt::DOUBLE => {
            let mut s = v[0].to_f64();
            for x in &v[1..] {
                s = if use_sse {
                    sse::add_f64(s, x.to_f64())
                } else {
                    s + x.to_f64()
                };
            }
            s
        }
        _ => {
            let mut s = T::to_int(v[0]);
            for x in &v[1..] {
                s = s.wrapping_add(T::to_int(*x));
            }
            f64::from(s)
        }
    }
}

/// Whether `subsample_by_2` of a volume of these sizes reads outside it (always for the centred
/// smoothing; for the other only along an odd size).
pub fn subsample_reads_outside(dims: [i64; 3], centred: bool) -> bool {
    centred || dims.iter().any(|&d| d % 2 == 1)
}

/// `subsample_by_2(refvol, centred)` of volume `t` of `refvol`, its extrapolation set to
/// extraslice (fslmaths' `-subsamp2`, `-subsamp2offc`): half the sizes (rounded up), twice the
/// voxel sizes, the sform and qform composed with the 2x scaling (and a half-voxel shift when not
/// centred); each voxel the hand-coded 3x3x3 smoothing (centred: `0.125` the centre, `0.0625` the
/// six faces, `0.0312` the twelve edges, `0.0156` the eight corners, each group summed in `T`'s
/// arithmetic) or the mean of the 2x2x2 block (`getneighbours`' order inside the volume, the
/// other order at its far edges), `/ 8.0`; `conv` is `(T)` of the `double`.
pub fn subsample_by_2<T: Arith>(
    refvol: &Volume<T>,
    t: usize,
    centred: bool,
    conv: &mut impl FnMut(f64) -> T,
) -> Volume<T> {
    use crate::arma::{self as am, Mat};
    use crate::nifti::XFORM_UNKNOWN;
    use crate::volume::Extrapolation;
    let (sx, sy, sz) = (refvol.xsize(), refvol.ysize(), refvol.zsize());
    let nvox = refvol.nvoxels().max(0) as usize;
    let mut src = Volume::<T>::new(sx, sy, sz, 1);
    src.copy_properties(refvol);
    src.data
        .copy_from_slice(&refvol.data[t * nvox..(t + 1) * nvox]);
    src.extrap = Extrapolation::Extraslice;
    let mut half = Volume::<T>::new((sx + 1) / 2, (sy + 1) / 2, (sz + 1) / 2, 1);
    half.copy_properties(refvol);
    half.setdims(
        (f64::from(refvol.xdim()) * 2.0) as f32,
        (f64::from(refvol.ydim()) * 2.0) as f32,
        (f64::from(refvol.zdim()) * 2.0) as f32,
    );
    let mut sub2 = Mat::eye(4);
    for k in 0..3 {
        sub2.set(k, k, 2.0);
        if !centred {
            sub2.set(k, 3, 0.5);
        }
    }
    if refvol.sform_code != XFORM_UNKNOWN {
        half.sform = am::mul_tinysq(&refvol.sform, &sub2);
        half.sform_code = refvol.sform_code;
    }
    if refvol.qform_code != XFORM_UNKNOWN {
        half.qform = am::mul_tinysq(&refvol.qform, &sub2);
        half.qform_code = refvol.qform_code;
    }
    const FACES: [(i64, i64, i64); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];
    const EDGES: [(i64, i64, i64); 12] = [
        (1, 1, 0),
        (1, -1, 0),
        (-1, 1, 0),
        (-1, -1, 0),
        (1, 0, 1),
        (1, 0, -1),
        (-1, 0, 1),
        (-1, 0, -1),
        (0, 1, 1),
        (0, 1, -1),
        (0, -1, 1),
        (0, -1, -1),
    ];
    const CORNERS: [(i64, i64, i64); 8] = [
        (1, 1, 1),
        (1, 1, -1),
        (1, -1, 1),
        (1, -1, -1),
        (-1, 1, 1),
        (-1, 1, -1),
        (-1, -1, 1),
        (-1, -1, -1),
    ];
    // getneighbours: v000 v001 v010 v011 v100 v101 v110 v111
    const NEIGH: [(i64, i64, i64); 8] = [
        (0, 0, 0),
        (0, 0, 1),
        (0, 1, 0),
        (0, 1, 1),
        (1, 0, 0),
        (1, 0, 1),
        (1, 1, 0),
        (1, 1, 1),
    ];
    const EDGE_ORDER: [(i64, i64, i64); 8] = [
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (1, 1, 0),
        (1, 0, 1),
        (0, 1, 1),
        (1, 1, 1),
    ];
    for z in 0..half.zsize() {
        let bz = 2 * z;
        for y in 0..half.ysize() {
            let by = 2 * y;
            for x in 0..half.xsize() {
                let bx = 2 * x;
                let vals = |offs: &[(i64, i64, i64)]| -> Vec<T> {
                    offs.iter()
                        .map(|&(dx, dy, dz)| src.get(bx + dx, by + dy, bz + dz))
                        .collect()
                };
                let v = if centred {
                    let c = src.get(bx, by, bz).to_f64();
                    let (f, e, k) = (vals(&FACES), vals(&EDGES), vals(&CORNERS));
                    let r = 0.125 * c
                        + 0.0625 * csum(&f, false)
                        + 0.0312 * csum(&e, false)
                        + 0.0156 * csum(&k, false);
                    if r.is_nan() {
                        let t0 = sse::mul_f64(0.125, c);
                        let t1 = sse::mul_f64(0.0625, csum(&f, true));
                        let t2 = sse::mul_f64(0.0312, csum(&e, true));
                        let t3 = sse::mul_f64(0.0156, csum(&k, true));
                        sse::add_f64(sse::add_f64(sse::add_f64(t0, t1), t2), t3)
                    } else {
                        r
                    }
                } else {
                    let order: &[(i64, i64, i64)] = if src.in_bounds(bx + 1, by + 1, bz + 1) {
                        &NEIGH
                    } else {
                        &EDGE_ORDER
                    };
                    let b = vals(order);
                    let r = csum(&b, false) / 8.0;
                    if r.is_nan() {
                        sse::div_f64(csum(&b, true), 8.0)
                    } else {
                        r
                    }
                };
                half.set(x, y, z, conv(v));
            }
        }
    }
    half
}

/// `edge_strengthen(source)` of every volume: the gradient magnitude `sqrt(sum (d_i / dim_i)^2) /
/// (2 sqrt(sum 1 / dim_i^2))` from central differences (in `T`'s arithmetic, then `double`;
/// `pow(x, 2.0)` as `x * x`, FSL's build), at the voxels inside a one-voxel border (in-plane only when the image
/// has at most 2 slices); the border keeps its values. `conv` is `(T)` of the `double`.
pub fn edge_strengthen<T: Arith>(src: &Volume<T>, conv: &mut impl FnMut(f64) -> T) -> Volume<T> {
    // `pow(x, 2.0)` is `x * x` in FSL's build (GCC folds it into a multiplication, the correctly
    // rounded square): glibc's `pow` differs from it in the last place for some doubles (found by
    // random testing, 2026-11-03: `validation/investigations/fsl-maths/edge_pow_probe.py`).
    let pow = |x: f64, _two: f64| sse::mul_f64(x, x);
    let (xd, yd, zd) = (
        f64::from(src.xdim()),
        f64::from(src.ydim()),
        f64::from(src.zdim()),
    );
    let tmpf = (2.0 * (1.0 / pow(xd, 2.0) + 1.0 / pow(yd, 2.0) + 1.0 / pow(zd, 2.0)).sqrt()) as f32;
    let (px, py, pz) = (pow(xd, 2.0), pow(yd, 2.0), pow(zd, 2.0));
    let mut result = src.clone();
    let (nx, ny, nz, nt) = (src.xsize(), src.ysize(), src.zsize(), src.tsize());
    let nvox = src.nvoxels().max(0) as usize;
    let at = |x: i64, y: i64, z: i64, t: i64| src.data[t as usize * nvox + src.idx(x, y, z)];
    let sq = |a: T, b: T, p: f64| sse::div_f64(pow(T::diff(a, b), 2.0), p);
    let three_d = nz > 2;
    let (z0, z1) = if three_d { (1, nz - 1) } else { (0, nz) };
    for t in 0..nt {
        for z in z0..z1 {
            for y in 1..ny - 1 {
                for x in 1..nx - 1 {
                    let ty = sq(at(x, y + 1, z, t), at(x, y - 1, z, t), py);
                    let tx = sq(at(x + 1, y, z, t), at(x - 1, y, z, t), px);
                    let s = if three_d {
                        let tz = sq(at(x, y, z + 1, t), at(x, y, z - 1, t), pz);
                        sse::add_f64(sse::add_f64(tz, ty), tx)
                    } else {
                        sse::add_f64(ty, tx)
                    };
                    let v = sse::div_f64(sse::sqrt_f64(s), f64::from(tmpf));
                    result.data[t as usize * nvox + src.idx(x, y, z)] = conv(v);
                }
            }
        }
    }
    result
}

/// `gaussian_kernel2D(sigma, radius)`: `(2 radius + 1)^2` values `exp(-(i^2 + j^2) / (2 sigma^2))`
/// (glibc's `exp` in `double`, rounded to `float`; the unit impulse for a sigma of at most 1e-6),
/// summed in `float`, then multiplied by `(float)(1.0 / sum)`.
pub fn gaussian_kernel2d(sigma: f32, radius: i32) -> Kern {
    let n = radius.wrapping_mul(2).wrapping_add(1);
    let mut k = Kern::filled(i64::from(n), i64::from(n), 1, 0.0);
    let mut sum = 0f32;
    for i in -radius..=radius {
        for j in -radius..=radius {
            let val: f32 = if f64::from(sigma) > 1e-6 {
                let s = f64::from(sigma);
                let num = f64::from(-(i.wrapping_mul(i).wrapping_add(j.wrapping_mul(j))));
                crate::glibc_double::exp(num / (2.0 * s * s)) as f32
            } else if i == 0 && j == 0 {
                1.0
            } else {
                0.0
            };
            let (ix, iy) = (i64::from(j + radius), i64::from(i + radius));
            if k.in_bounds(ix, iy, 0) {
                let idx = (iy * k.dims[0] + ix) as usize;
                k.data[idx] = val;
            }
            sum += val;
        }
    }
    let f = (1.0 / f64::from(sum)) as f32;
    for v in &mut k.data {
        *v *= f;
    }
    k
}

/// `T <= f` (or `>`) as C compares a value of `T` with a `float`: `double` in `double`, the others
/// in `float`.
#[inline]
fn cmp_f<T: Arith>(v: T, f: f32) -> std::cmp::Ordering {
    let o = if T::DTYPE == dt::DOUBLE {
        v.to_f64().partial_cmp(&f64::from(f))
    } else {
        v.to_f32().partial_cmp(&f)
    };
    o.unwrap_or(std::cmp::Ordering::Equal)
}

/// `log_edge_detect(source, sigma1, sigma2)` (mode 0): the difference of two 2D Gaussians of
/// radius `(int)(4 sigma2)` (`gaussian_kernel2D(sigma2) - gaussian_kernel2D(sigma1)`), convolved
/// with volume 0 only (`convolve(source, kernel)`: the other volumes keep their values unless
/// `every_volume`, FSL-164's correction), then 1 at the voxels inside a one-voxel border where the
/// value and a 6-neighbour cross zero (`v > 0` and a neighbour `<= -v`, or `v < 0` and a
/// neighbour `> -v`), 0 elsewhere.
pub fn log_edge_detect<T: Arith>(
    src: &Volume<T>,
    sigma1: f32,
    sigma2: f32,
    every_volume: bool,
    conv: &mut impl FnMut(f64) -> T,
) -> Volume<T> {
    let radius = f2i(4.0 * sigma2);
    let mut k = gaussian_kernel2d(sigma2, radius);
    let k1 = gaussian_kernel2d(sigma1, radius);
    for (a, b) in k.data.iter_mut().zip(&k1.data) {
        *a -= *b;
    }
    let nvox = src.nvoxels().max(0) as usize;
    let mut lr = src.clone();
    let nconv = if every_volume {
        src.tsize().max(0) as usize
    } else {
        1.min(src.tsize().max(0) as usize)
    };
    for t in 0..nconv {
        let mut v = Volume::<T>::new(src.xsize(), src.ysize(), src.zsize(), 1);
        v.copy_properties(src);
        v.data.copy_from_slice(&src.data[t * nvox..(t + 1) * nvox]);
        let r = convolve_plain(&v, &k, conv);
        lr.data[t * nvox..(t + 1) * nvox].copy_from_slice(&r.data);
    }
    let mut zc = src.clone();
    zc.data.fill(T::from_int(0));
    let (nx, ny, nz) = (src.xsize(), src.ysize(), src.zsize());
    let one = T::from_int(1);
    use std::cmp::Ordering::Greater;
    for t in 0..src.tsize().max(0) as usize {
        let at = |x: i64, y: i64, z: i64| lr.data[t * nvox + lr.idx(x, y, z)];
        for z in 1..nz - 1 {
            for y in 1..ny - 1 {
                for x in 1..nx - 1 {
                    let val = at(x, y, z).to_f32();
                    let ival = -val;
                    let n = [
                        at(x - 1, y, z),
                        at(x + 1, y, z),
                        at(x, y - 1, z),
                        at(x, y + 1, z),
                        at(x, y, z - 1),
                        at(x, y, z + 1),
                    ];
                    let nan = |v: T| v.to_f64().is_nan() || ival.is_nan();
                    let le = |v: T| !nan(v) && cmp_f(v, ival) != Greater;
                    let gt = |v: T| !nan(v) && cmp_f(v, ival) == Greater;
                    let cross = (val > 0.0 && n.iter().any(|&v| le(v)))
                        || (val < 0.0 && n.iter().any(|&v| gt(v)));
                    if cross {
                        let i = t * nvox + zc.idx(x, y, z);
                        zc.data[i] = one;
                    }
                }
            }
        }
    }
    zc
}

// ------------------------------------------------------------------------------------------------
// Connected components and fill_holes

/// `backConnectivity(n)`: the neighbours already visited in z, y, x order (3, 9 or 13 offsets).
pub fn back_connectivity(n: i32) -> Vec<(i64, i64, i64)> {
    let mut v = Vec::new();
    if n == 6 || n == 18 || n == 26 {
        v.extend([(-1, 0, 0), (0, -1, 0), (0, 0, -1)]);
    }
    if n == 18 || n == 26 {
        v.extend([
            (-1, -1, 0),
            (-1, 0, -1),
            (0, 1, -1),
            (0, -1, -1),
            (1, -1, 0),
            (1, 0, -1),
        ]);
    }
    if n == 26 {
        v.extend([(-1, 1, -1), (-1, -1, -1), (1, 1, -1), (1, -1, -1)]);
    }
    v
}

/// `MISCMATHS::round` of a value (an `int` unchanged; `float` and `double` half away from zero,
/// in `double`, truncated).
#[inline]
fn round_t<T: Arith>(v: T) -> i32 {
    if T::IS_FLOAT {
        let x = v.to_f64();
        if x > 0.0 {
            crate::cnum::f64_to_i32(x + 0.5)
        } else {
            crate::cnum::f64_to_i32(x - 0.5)
        }
    } else {
        T::to_int(v)
    }
}

/// `connected_components(vol, clustersize, numconnected)` of one volume: the voxels above 0.5,
/// joined to a back neighbour (inside the volume's lower bounds; beyond its upper bounds the
/// value is 0) whose rounded value equals the voxel's; the provisional labels' equivalences
/// resolved through the parent map (labels read back through `float`), then numbered 1, 2, ...
/// in order of first label. Returns the labels and the cluster sizes.
pub fn connected_components<T: Arith>(
    vol: &[T],
    dims: [i64; 3],
    numconnected: i32,
) -> (Vec<i32>, Vec<i64>) {
    let (nx, ny, nz) = (dims[0], dims[1], dims[2]);
    let idx = |x: i64, y: i64, z: i64| ((z * ny + y) * nx + x) as usize;
    let mut label = vec![0i32; vol.len()];
    let (mut ea, mut eb) = (Vec::new(), Vec::new());
    let neigh = back_connectivity(numconnected);
    let mut labelnum = 0i32;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let val = vol[idx(x, y, z)];
                if val.to_f64() <= 0.5 || val.to_f64().is_nan() {
                    continue;
                }
                let mut lval = label[idx(x, y, z)];
                for &(dx, dy, dz) in &neigh {
                    let (xn, yn, zn) = (x + dx, y + dy, z + dz);
                    if xn < 0 || yn < 0 || zn < 0 {
                        continue;
                    }
                    let inside = xn < nx && yn < ny && zn < nz;
                    let nv = if inside {
                        vol[idx(xn, yn, zn)]
                    } else {
                        T::from_int(0)
                    };
                    let r = round_t(nv);
                    let rel = if T::DTYPE == dt::FLOAT {
                        r as f32 == val.to_f32()
                    } else {
                        f64::from(r) == val.to_f64()
                    };
                    if rel {
                        let lval2 = if inside { label[idx(xn, yn, zn)] } else { 0 };
                        if lval != lval2 {
                            if lval != 0 {
                                ea.push(lval2);
                                eb.push(lval);
                            }
                            label[idx(x, y, z)] = lval2;
                            lval = lval2;
                        }
                    }
                }
                if lval == 0 {
                    labelnum += 1;
                    label[idx(x, y, z)] = labelnum;
                }
            }
        }
    }
    // relabel_components_uniquely
    let maxl = label.iter().copied().max().unwrap_or(0).max(0) as usize;
    let mut emap = vec![-0.2f64; maxl + 1];
    let parent = |mut i: i32, emap: &[f64]| -> i32 {
        while emap[i as usize] > 0.0 {
            i = crate::interp::round_f32(emap[i as usize] as f32);
        }
        i
    };
    for n in 0..ea.len() {
        let n1 = parent(ea[n], &emap);
        let n2 = parent(eb[n], &emap);
        if n1 != n2 {
            emap[n1.max(n2) as usize] = f64::from(n1.min(n2));
        }
    }
    let mut newlabel = 1i32;
    for n in 1..=maxl as i32 {
        let n1 = parent(n, &emap);
        if n1 < n {
            emap[n as usize] = emap[n1 as usize];
        } else {
            emap[n as usize] = f64::from(-newlabel);
            newlabel += 1;
        }
    }
    let mut sizes = vec![0i64; (newlabel - 1).max(0) as usize];
    for l in label.iter_mut() {
        if *l > 0 {
            let tmp = crate::interp::round_f32(-(emap[*l as usize] as f32));
            *l = tmp;
            if tmp >= 1 {
                sizes[(tmp - 1) as usize] += 1;
            }
        }
    }
    (label, sizes)
}

/// Whether `fill_holes`' edge test divides by zero (`x % maxx`, then `y % maxy`, then `z % maxz`,
/// each taken only when the previous is not 0): a size of 1 along an axis reached by the test.
pub fn fill_holes_traps(dims: [i64; 3]) -> bool {
    let (mx, my, mz) = (dims[0] - 1, dims[1] - 1, dims[2] - 1);
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                if mx == 0 {
                    return true;
                }
                if x % mx == 0 {
                    continue;
                }
                if my == 0 {
                    return true;
                }
                if y % my == 0 {
                    continue;
                }
                if mz == 0 {
                    return true;
                }
                let _ = z % mz;
            }
        }
    }
    false
}

/// `fill_holes(im, connectivity)` of one volume: the components (6- or 26-connected) of the zero
/// voxels that touch no edge of the volume set to 1, the other voxels keep their values. A voxel
/// is at an edge when one of its coordinates is 0 or the last; with `planar` (FSL-165's
/// correction) an axis of size 1 does not count, so a 2D image's holes are filled in its plane
/// (FSL's test divides by zero there: [`fill_holes_traps`]).
pub fn fill_holes<T: Arith>(im: &[T], dims: [i64; 3], connectivity: i32, planar: bool) -> Vec<T> {
    let work: Vec<T> = im
        .iter()
        .map(|&v| T::from_int(i32::from(v.to_f64() == 0.0)))
        .collect();
    let (labels, _) = connected_components(&work, dims, connectivity);
    let (nx, ny, nz) = (dims[0], dims[1], dims[2]);
    let mut edge = std::collections::BTreeSet::new();
    edge.insert(0i32);
    let at_edge = |c: i64, n: i64| c == 0 || c == n - 1;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let e = if planar {
                    (nx > 1 && at_edge(x, nx))
                        || (ny > 1 && at_edge(y, ny))
                        || (nz > 1 && at_edge(z, nz))
                } else {
                    at_edge(x, nx) || at_edge(y, ny) || at_edge(z, nz)
                };
                if e {
                    edge.insert(labels[((z * ny + y) * nx + x) as usize]);
                }
            }
        }
    }
    im.iter()
        .zip(&labels)
        .map(|(&v, l)| if edge.contains(l) { v } else { T::from_int(1) })
        .collect()
}

// ------------------------------------------------------------------------------------------------
// Temporal filtering (fslmaths' -bptf)

/// What `bandpass_temporal_filter` meets before filtering, from the half-widths of its two
/// filters (`(int)(hp_sigma * 3)` and `(int)(lp_sigma * 20) + 2`, x86-64's conversions and
/// wrapping `int` arithmetic).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BptfStop {
    /// The weights' array has a negative length (`2m + 1` wraps around): `new double[]` throws
    /// `std::bad_array_new_length`.
    BadLength,
    /// A half-width below 0 (`(int)` of an infinite sigma, or of one beyond int's range, is the
    /// most negative int): the weights' pointer is moved outside its array and FSL crashes (a
    /// segmentation fault, measured for the low-pass; the high-pass's weight loop then writes
    /// outside its array).
    Crash,
}

/// The half-widths `(hp, lp)` of `bandpass_temporal_filter`'s masks (0 for a sigma at or below
/// 0, and for NaN, which neither filter then uses).
pub fn bptf_half_widths(hp_sigma: f64, lp_sigma: f64) -> (i32, i32) {
    let hp = if hp_sigma <= 0.0 || hp_sigma.is_nan() {
        0
    } else {
        cnum::f64_to_i32(hp_sigma * 3.0)
    };
    let lp = if lp_sigma <= 0.0 || lp_sigma.is_nan() {
        0
    } else {
        cnum::f64_to_i32(lp_sigma * 20.0).wrapping_add(2)
    };
    (hp, lp)
}

/// Whether FSL stops before filtering with these half-widths (a filter that runs: its sigma
/// above 0).
pub fn bptf_stop(hp_sigma: f64, lp_sigma: f64) -> Option<BptfStop> {
    let (hm, lm) = bptf_half_widths(hp_sigma, lp_sigma);
    for (on, m) in [(hp_sigma > 0.0, hm), (lp_sigma > 0.0, lm)] {
        if !on {
            continue;
        }
        if m.wrapping_add(m).wrapping_add(1) < 0 {
            return Some(BptfStop::BadLength);
        }
        if m < 0 {
            return Some(BptfStop::Crash);
        }
    }
    None
}

/// `exp(-0.5 * (double)(t * t) / (sigma * sigma))` with `t * t` in wrapping int arithmetic.
fn bptf_weight(t: i32, sigma: f64) -> f64 {
    glibc_double::exp(-0.5 * f64::from(t.wrapping_mul(t)) / (sigma * sigma))
}

/// newimagefns' `bandpass_temporal_filter(source, hp_sigma, lp_sigma)` (sigmas in volumes):
/// for each voxel's time series in double, the high-pass (a non-linear filter: at each time
/// point the line fitted by Gaussian-weighted least squares over `±(int)(3 hp_sigma)` points is
/// subtracted, its first value added back, then the series demeaned) and the Gaussian low-pass
/// over `±((int)(20 lp_sigma) + 2)` points (weights normalised by their total, then by the sum
/// of the weights inside the series); each value written back with `conv` (`(T) array[t]`).
/// A sigma at or below 0 (or NaN) skips its filter. The caller checks [`bptf_stop`] first. The
/// weights are computed for the offsets the series reach only (FSL fills arrays of `2m + 1`),
/// and the low-pass total over all `2m + 1` in FSL's order.
pub fn bandpass_temporal_filter<T: Arith>(
    source: &Volume<T>,
    hp_sigma: f64,
    lp_sigma: f64,
    conv: &mut impl FnMut(f64) -> T,
) -> Volume<T> {
    let (hm, lm) = bptf_half_widths(hp_sigma, lp_sigma);
    let nt = source.tsize().max(0) as i32;
    let nvox = source.nvoxels().max(0) as usize;
    let mut result = source.clone();
    // the weights at the offsets a series reaches (|dt| <= min(m, nt - 1))
    let reach = |m: i32| if m < 0 { 0 } else { m.min((nt - 1).max(0)) };
    let hp_w: Vec<f64> = if hp_sigma > 0.0 {
        let r = reach(hm);
        (-r..=r).map(|t| bptf_weight(t, hp_sigma)).collect()
    } else {
        Vec::new()
    };
    let lp_w: Vec<f64> = if lp_sigma > 0.0 {
        let mut total = S64(0.0);
        if lm >= 0 {
            for t in -lm..=lm {
                total += bptf_weight(t, lp_sigma);
            }
        }
        let r = reach(lm);
        (-r..=r)
            .map(|t| sse::div_f64(bptf_weight(t, lp_sigma), total.0))
            .collect()
    } else {
        Vec::new()
    };
    let hr = reach(hm);
    let lr = reach(lm);
    let mut array = vec![0f64; nt as usize];
    let mut array2 = vec![0f64; nt as usize];
    for v in 0..nvox {
        for t in 0..nt as usize {
            array[t] = source.data[t * nvox + v].to_f64();
        }
        if hp_sigma > 0.0 {
            let mut done_c0 = false;
            let mut c0 = 0.0;
            let mut mean = S64(0.0);
            for t in 0..nt {
                let (mut a, mut b, mut c, mut d, mut n) =
                    (S64(0.0), S64(0.0), S64(0.0), S64(0.0), S64(0.0));
                let lo = t.wrapping_sub(hm).max(0);
                let hi = t.wrapping_add(hm).min(nt - 1);
                let mut tt = lo;
                while tt <= hi {
                    let dt = tt - t;
                    let w = S64(hp_w[(dt + hr) as usize]);
                    let x = array[tt as usize];
                    a += w * f64::from(dt);
                    b += w * x;
                    c += w * f64::from(dt) * f64::from(dt);
                    d += w * f64::from(dt) * x;
                    n += w;
                    tt += 1;
                }
                let tmpdenom = c * n - a * a;
                array2[t as usize] = if tmpdenom.0 != 0.0 {
                    let cc = (b * c - a * d) / tmpdenom;
                    if !done_c0 {
                        c0 = cc.0;
                        done_c0 = true;
                    }
                    (S64(c0) + array[t as usize] - cc).0
                } else {
                    array[t as usize]
                };
                mean += array2[t as usize];
            }
            let mean = mean / f64::from(nt);
            for t in 0..nt as usize {
                array2[t] = sse::sub_f64(array2[t], mean.0);
            }
            array.copy_from_slice(&array2);
        }
        if lp_sigma > 0.0 {
            for t in 0..nt {
                let mut total = S64(0.0);
                let mut sum = S64(0.0);
                let lo = t.wrapping_sub(lm).max(0);
                let hi = t.wrapping_add(lm).min(nt - 1);
                let mut tt = lo;
                while tt <= hi {
                    let w = lp_w[(tt - t + lr) as usize];
                    total += S64(array[tt as usize]) * w;
                    sum += w;
                    tt += 1;
                }
                array2[t as usize] = if sum.0 > 0.0 {
                    (total / sum).0
                } else {
                    total.0
                };
            }
            array.copy_from_slice(&array2);
        }
        for t in 0..nt as usize {
            result.data[t * nvox + v] = conv(array[t]);
        }
    }
    result
}

// ------------------------------------------------------------------------------------------------
// Masked means (fslmaths' -inm, -ing)

/// `mean(mask)` of `values` (`mask` cycled over them, a voxel counted when its value is above
/// 0.5): `calculateSums`' sum in FSL's blocks over the count (at least 1); `true` when no value
/// is in the mask (FSL prints `ERROR:: Empty mask image`).
pub fn masked_mean<T: Arith>(values: &[T], mask: &[T]) -> (f64, bool) {
    let n = values.len();
    let nlim = ((n as f64).sqrt() as i64).max(10000) as usize;
    let (mut total, mut sum) = (0f64, 0f64);
    let (mut cnt, mut nn, mut count) = (0usize, 0usize, 0usize);
    for k in 0..n {
        if mask[k % mask.len()].to_f64() > stats::MASK_THRESHOLD {
            sum = sse::add_f64(sum, values[k].to_f64());
            cnt += 1;
            count += 1;
            if cnt > nlim {
                nn += 1;
                total = sse::add_f64(total, sum);
                sum = 0.0;
                cnt = 0;
            }
        }
    }
    let empty = cnt + nn == 0;
    (sse::add_f64(total, sum) / (count as f64).max(1.0), empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `-dilall`'s mean as FSL's SSE build adds it, the neighbour first: neighbours `inf`, `-inf`
    /// and then a positive NaN give the neighbour's NaN (`0x7fc00000`), not the default NaN of
    /// `inf + -inf` that the running sum holds (random testing's case `dilall-nan-order`); `-inf`
    /// and `inf` alone give the default NaN (`0xffc00000` stored as float).
    #[test]
    fn dilall_passes_on_the_neighbours_nan_first() {
        let nan = f32::from_bits(0x7fc0_0000);
        let input = [
            f32::INFINITY,
            -f32::INFINITY,
            nan,
            1.0,
            0.0,
            1.0,
            1.0,
            1.0,
            1.0,
        ];
        let out = dilall(&input, [3, 3, 1], &mut |x| x as f32);
        assert_eq!(out[4].to_bits(), nan.to_bits());
        let input = [-f32::INFINITY, 0.0, f32::INFINITY];
        let out = dilall(&input, [3, 1, 1], &mut |x| x as f32);
        assert_eq!(out[1].to_bits(), 0xffc0_0000);
    }

    #[test]
    fn kernels_have_fsl_sizes() {
        assert_eq!(box_kernel_mm(5.0, 2.0, 2.5, 3.0).dims, [3, 3, 1]);
        let g = gaussian_kernel3d_mm(1.0, 2.0, 2.0, 2.0, 4.0, false);
        assert_eq!(g.kernel.dims, [5, 5, 5]);
        assert_eq!(g.kernel.at(2, 2, 2), 1.0);
        assert!(!g.zero_width);
        let z = gaussian_kernel3d_mm(0.0, 2.0, 2.0, 2.0, 4.0, false);
        // FSL's centre: SSE's default NaN (sign bit set), on every platform
        assert!(z.zero_width);
        assert_eq!(z.kernel.at(0, 0, 0).to_bits(), 0xffc0_0000);
        let zi = gaussian_kernel3d_mm(0.0, 2.0, 2.0, 2.0, 4.0, true);
        assert_eq!(zi.kernel.data, vec![1.0]);
        let s = spherical_kernel(2.0, 2.0, 2.0, 2.0);
        assert_eq!(s.dims, [3, 3, 3]);
        assert_eq!(s.data.iter().filter(|&&v| v == 1.0).count(), 7);
        assert!(box_kernel_vox(0, 0, 0).has_no_voxels());
        assert_eq!(Kern::filled(-1, -1, 1, 1.0).data.len(), 1);
    }

    #[test]
    fn morph_box3_matches_a_direct_maximum() {
        let dims = [4, 3, 2];
        let src: Vec<f32> = (0..24).map(|i| ((i * 7) % 11) as f32 - 3.0).collect();
        let mut issues = MorphIssues::default();
        let k = box_kernel_vox(3, 3, 3);
        let r = morphfilter(
            &src,
            dims,
            &k,
            Morph::Dilate,
            &mut |x| x as f32,
            &mut issues,
        );
        for z in 0..2i64 {
            for y in 0..3i64 {
                for x in 0..4i64 {
                    let mut m = f32::NEG_INFINITY;
                    for zz in (z - 1).max(0)..=(z + 1).min(1) {
                        for yy in (y - 1).max(0)..=(y + 1).min(2) {
                            for xx in (x - 1).max(0)..=(x + 1).min(3) {
                                m = m.max(src[((zz * 3 + yy) * 4 + xx) as usize]);
                            }
                        }
                    }
                    assert_eq!(r[((z * 3 + y) * 4 + x) as usize], m);
                }
            }
        }
        assert_eq!(issues, MorphIssues::default());
    }

    #[test]
    fn modal_dilation_takes_the_largest_most_frequent() {
        assert_eq!(mode_of_sorted(&[5.0, 3.0, 3.0, 2.0, 2.0]), 3.0);
        assert_eq!(mode_of_sorted(&[5.0, 4.0]), 5.0);
    }

    #[test]
    fn fft_choice_in_float() {
        assert!(!uses_fft(210, 27));
        assert!(uses_fft(210, 1287));
        assert!(!uses_fft(210, 0));
    }
}
