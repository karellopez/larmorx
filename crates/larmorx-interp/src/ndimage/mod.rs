// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy's `ndimage` spline interpolation: `map_coordinates`, `spline_filter` and
//! `spline_filter1d`, ported from SciPy 1.15.2 (`scipy/ndimage/_interpolation.py`,
//! `_ni_support.py`, `src/ni_interpolation.c`, `src/ni_splines.c`; BSD-3-Clause, see the
//! notices in [`splines`] and [`geometric`] and in NOTICE).
//!
//! This is the interpolation under fMRIPrep's one-shot resampler and nitransforms. Results are
//! bit-identical to SciPy on x86-64 Linux: the same prepadding (`grid-constant` and `nearest`
//! pad 12 voxels before prefiltering), the same float64 prefilter with SciPy's boundary
//! initialisations, the same boundary rules for coordinates and spline footprints, and the
//! same order of every multiplication and addition.
//!
//! Arrays are in Fortran order (first axis fastest), as everywhere in larmorx; SciPy's results
//! do not depend on memory layout. Axis `d` of the array is coordinate `d`.
//!
//! Deliberate differences (`docs/findings/scipy-ndimage.md`):
//! - The prefilter's `pow(z, n)` is correctly rounded ([`larmorx_core::math::powi`]); glibc's
//!   differs only for powers below 1e-100, which cannot change a coefficient.
//! - Coordinates that are NaN or beyond ±2^63 make SciPy cast them to integers with undefined
//!   behaviour; larmorx reproduces x86-64's result (the integer `i64::MIN`), and where SciPy
//!   would then read outside the array (modes `nearest`, orders ≥ 1) it returns NaN.
#![allow(clippy::needless_range_loop)]

mod geometric;
pub(crate) mod splines;

pub use geometric::Spline;

use larmorx_core::RealElement;
use rayon::prelude::*;

/// How the input is extended beyond its edges (`mode` in `scipy.ndimage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// `reflect` (`d c b a | a b c d | d c b a`).
    Reflect,
    /// `grid-mirror`: SciPy's synonym for `reflect`.
    GridMirror,
    /// `constant`: no interpolation beyond the edges; outside `[0, n-1]` gives `cval`.
    Constant,
    /// `grid-constant`: `cval` beyond the edges, interpolated across them (fMRIPrep's default).
    GridConstant,
    /// `nearest` (`a a a a | a b c d | d d d d`).
    Nearest,
    /// `mirror` (`d c b | a b c d | c b a`).
    Mirror,
    /// `grid-wrap` (`a b c d | a b c d | a b c d`).
    GridWrap,
    /// `wrap`: SciPy's legacy wrap, with period `n - 1` for coordinates.
    Wrap,
}

impl Mode {
    /// All modes, in SciPy's documentation order.
    pub const ALL: [Mode; 8] = [
        Mode::Reflect,
        Mode::GridMirror,
        Mode::Constant,
        Mode::GridConstant,
        Mode::Nearest,
        Mode::Mirror,
        Mode::GridWrap,
        Mode::Wrap,
    ];

    /// The mode for SciPy's name (`"grid-constant"`, `"mirror"`, ...).
    pub fn from_name(name: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.name() == name)
    }

    /// SciPy's name of the mode.
    pub const fn name(self) -> &'static str {
        match self {
            Mode::Reflect => "reflect",
            Mode::GridMirror => "grid-mirror",
            Mode::Constant => "constant",
            Mode::GridConstant => "grid-constant",
            Mode::Nearest => "nearest",
            Mode::Mirror => "mirror",
            Mode::GridWrap => "grid-wrap",
            Mode::Wrap => "wrap",
        }
    }

    /// `_extend_mode_to_code(mode)` (not the filter variant: `spline_filter1d` and
    /// `map_coordinates` both call it without `is_filter`).
    pub(crate) const fn extend(self) -> Extend {
        match self {
            Mode::Nearest => Extend::Nearest,
            Mode::Wrap => Extend::Wrap,
            Mode::Reflect | Mode::GridMirror => Extend::Reflect,
            Mode::Mirror => Extend::Mirror,
            Mode::Constant => Extend::Constant,
            Mode::GridWrap => Extend::GridWrap,
            Mode::GridConstant => Extend::GridConstant,
        }
    }

    /// Voxels padded on every side before prefiltering (`_prepad_for_spline_filter`).
    pub const fn prepad(self) -> usize {
        match self {
            Mode::Nearest | Mode::GridConstant => 12,
            _ => 0,
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for Mode {
    type Err = NdimageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Mode::from_name(s).ok_or_else(|| NdimageError::Mode(s.to_owned()))
    }
}

/// SciPy's internal extension codes (`NI_ExtendMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Extend {
    Nearest,
    Wrap,
    Reflect,
    Mirror,
    Constant,
    GridWrap,
    GridConstant,
}

/// Errors, with SciPy's messages where it has one.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NdimageError {
    #[error("spline order not supported")]
    Order(u32),
    #[error("boundary mode not supported: {0:?}")]
    Mode(String),
    #[error("the data length {len} does not match the shape {shape:?}")]
    Shape { len: usize, shape: Vec<usize> },
    #[error("axis {axis} is out of bounds for an array of dimension {ndim}")]
    Axis { axis: usize, ndim: usize },
    #[error("invalid shape for coordinate array")]
    Coordinates,
    #[error("output shape not correct")]
    Output,
}

/// Element types the interpolators read. `pad_value` is SciPy's prepadding value: `cval`
/// stored in the input's type (`np.pad(..., constant_values=cval)`), read back as a double.
pub trait Sample: RealElement + Send + Sync {
    fn pad_value(cval: f64) -> f64;
}

impl Sample for f64 {
    fn pad_value(cval: f64) -> f64 {
        cval
    }
}

impl Sample for f32 {
    fn pad_value(cval: f64) -> f64 {
        f64::from(cval as f32)
    }
}

macro_rules! sample_int {
    ($($t:ty),*) => {$(
        impl Sample for $t {
            fn pad_value(cval: f64) -> f64 {
                // numpy's unsafe float-to-integer cast (C truncation; x86-64 gives the
                // "integer indefinite" value for NaN and out-of-range values).
                c_cast_i64(cval) as $t as f64
            }
        }
    )*};
}
sample_int!(u8, i8, u16, i16, u32, i32, u64, i64);

/// Output element types and SciPy's conversion of the interpolated double (`CASE_INTERP_OUT*`).
pub trait Output: Copy + Send + Sync + Default {
    fn from_interp(t: f64) -> Self;
}

impl Output for f64 {
    #[inline(always)]
    fn from_interp(t: f64) -> Self {
        t
    }
}

impl Output for f32 {
    #[inline(always)]
    fn from_interp(t: f64) -> Self {
        t as f32
    }
}

macro_rules! output_uint {
    ($($t:ty),*) => {$(
        impl Output for $t {
            fn from_interp(t: f64) -> Self {
                let mut t = if t > 0.0 { t + 0.5 } else { 0.0 };
                if t > <$t>::MAX as f64 { t = <$t>::MAX as f64; }
                if t < 0.0 { t = 0.0; }
                t as $t
            }
        }
    )*};
}
macro_rules! output_int {
    ($($t:ty),*) => {$(
        impl Output for $t {
            fn from_interp(t: f64) -> Self {
                let mut t = if t > 0.0 { t + 0.5 } else { t - 0.5 };
                if t > <$t>::MAX as f64 { t = <$t>::MAX as f64; }
                if t < <$t>::MIN as f64 { t = <$t>::MIN as f64; }
                c_cast_i64(t) as $t
            }
        }
    )*};
}
output_uint!(u8, u16, u32, u64);
output_int!(i8, i16, i32, i64);

/// C's `(npy_intp)x` as x86-64 executes it (`cvttsd2si`): truncation toward zero, and
/// `i64::MIN` for NaN and values outside the range of `i64` (undefined behaviour in C).
#[inline(always)]
pub(crate) fn c_cast_i64(x: f64) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// `floor(x)`, exactly as C's `floor` for every input, without a call into the C library:
/// baseline x86-64 has no rounding instruction (SSE4.1), so `f64::floor` would be one call per
/// use in the interpolation loops.
#[inline(always)]
pub(crate) fn floor(x: f64) -> f64 {
    const TWO_52: f64 = 4_503_599_627_370_496.0;
    if x.abs() < TWO_52 {
        if x == 0.0 {
            return x; // keeps -0.0
        }
        let t = (x as i64) as f64; // exact truncation toward zero
        if t > x { t - 1.0 } else { t }
    } else {
        x // integral already, infinite or NaN
    }
}

/// `map_coordinate`: maps a coordinate outside `[0, len - 1]` back inside, according to the
/// boundary mode. For `Constant` an outside coordinate becomes -1; `Nearest`'s and
/// `GridConstant`'s coordinates are not mapped by the caller.
#[inline]
pub(crate) fn map_coordinate(input: f64, len: i64, mode: Extend) -> f64 {
    let mut x = input;
    if x < 0.0 {
        match mode {
            Extend::Mirror => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    let sz2 = 2 * len - 2;
                    x = sz2.wrapping_mul(c_cast_i64(-x / sz2 as f64)) as f64 + x;
                    x = if x <= (1 - len) as f64 {
                        x + sz2 as f64
                    } else {
                        -x
                    };
                }
            }
            Extend::Reflect => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    let sz2 = 2 * len;
                    if x < -(sz2 as f64) {
                        x = sz2.wrapping_mul(c_cast_i64(-x / sz2 as f64)) as f64 + x;
                    }
                    // -1e-15 check to avoid possibility that: (-in - 1) == -1
                    x = if x < -(len as f64) {
                        x + sz2 as f64
                    } else {
                        (if x > -1e-15 { 1e-15 } else { -x }) - 1.0
                    };
                }
            }
            Extend::Wrap => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    let sz = len - 1;
                    x += sz.wrapping_mul(c_cast_i64(-x / sz as f64).wrapping_add(1)) as f64;
                }
            }
            Extend::GridWrap => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    x += len.wrapping_mul(c_cast_i64((-1.0 - x) / len as f64).wrapping_add(1))
                        as f64;
                }
            }
            Extend::Nearest => x = 0.0,
            Extend::Constant => x = -1.0,
            Extend::GridConstant => {}
        }
    } else if x > (len - 1) as f64 {
        match mode {
            Extend::Mirror => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    let sz2 = 2 * len - 2;
                    x -= sz2.wrapping_mul(c_cast_i64(x / sz2 as f64)) as f64;
                    if x >= len as f64 {
                        x = sz2 as f64 - x;
                    }
                }
            }
            Extend::Reflect => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    let sz2 = 2 * len;
                    x -= sz2.wrapping_mul(c_cast_i64(x / sz2 as f64)) as f64;
                    if x >= len as f64 {
                        x = sz2 as f64 - x - 1.0;
                    }
                }
            }
            Extend::Wrap => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    let sz = len - 1;
                    x -= sz.wrapping_mul(c_cast_i64(x / sz as f64)) as f64;
                }
            }
            Extend::GridWrap => {
                if len <= 1 {
                    x = 0.0;
                } else {
                    x -= len.wrapping_mul(c_cast_i64(x / len as f64)) as f64;
                }
            }
            Extend::Nearest => x = (len - 1) as f64,
            Extend::Constant => x = -1.0,
            Extend::GridConstant => {}
        }
    }
    x
}

fn check_shape(len: usize, shape: &[usize]) -> Result<(), NdimageError> {
    if shape.iter().product::<usize>() != len {
        return Err(NdimageError::Shape {
            len,
            shape: shape.to_vec(),
        });
    }
    Ok(())
}

/// `scipy.ndimage.spline_filter1d(input, order, axis, output=np.float64, mode)` on a
/// Fortran-ordered float64 array, in place. Orders 0 and 1 leave the data unchanged.
///
/// Lines are filtered in parallel on the current rayon pool when `parallel` is set; the result
/// does not depend on it.
pub fn spline_filter1d(
    data: &mut [f64],
    shape: &[usize],
    axis: usize,
    order: u32,
    mode: Mode,
    parallel: bool,
) -> Result<(), NdimageError> {
    if order > 5 {
        return Err(NdimageError::Order(order));
    }
    check_shape(data.len(), shape)?;
    if axis >= shape.len() {
        return Err(NdimageError::Axis {
            axis,
            ndim: shape.len(),
        });
    }
    if order >= 2 {
        filter_axis(data, shape, axis, order, mode.extend(), parallel);
    }
    Ok(())
}

/// `scipy.ndimage.spline_filter(input, order, output=np.float64, mode)`: the 1D filter along
/// every axis in turn, in place on a Fortran-ordered float64 array. `order` must be 2 to 5.
pub fn spline_filter(
    data: &mut [f64],
    shape: &[usize],
    order: u32,
    mode: Mode,
    parallel: bool,
) -> Result<(), NdimageError> {
    if !(2..=5).contains(&order) {
        return Err(NdimageError::Order(order));
    }
    check_shape(data.len(), shape)?;
    for axis in 0..shape.len() {
        filter_axis(data, shape, axis, order, mode.extend(), parallel);
    }
    Ok(())
}

/// Filters every line along `axis`. A line `(inner, outer)` has elements
/// `inner + stride·(t + len·outer)` for `t < len`, with `stride` the product of the earlier
/// axes' lengths.
pub(crate) fn filter_axis(
    data: &mut [f64],
    shape: &[usize],
    axis: usize,
    order: u32,
    mode: Extend,
    parallel: bool,
) {
    let len = shape[axis];
    if len <= 1 || data.is_empty() {
        return;
    }
    let filter = splines::LineFilter::new(order, mode, len);
    let stride: usize = shape[..axis].iter().product();
    let block = stride * len;
    if stride == 1 {
        // Lines are contiguous.
        if parallel {
            data.par_chunks_mut(len).for_each(|line| filter.apply(line));
        } else {
            data.chunks_mut(len).for_each(|line| filter.apply(line));
        }
        return;
    }
    let outer = data.len() / block;
    // A block holds `len` rows of `stride` values; its lines are its columns, filtered in
    // tiles of columns so that a tile's rows stay in cache.
    const TILE: usize = 512;
    let process_block = |chunk: &mut [f64], acc: &mut Vec<f64>| {
        for c0 in (0..stride).step_by(TILE) {
            filter.apply_rows(chunk, stride, c0..(c0 + TILE).min(stride), acc);
        }
    };
    if !parallel {
        let mut acc = Vec::new();
        data.chunks_mut(block)
            .for_each(|chunk| process_block(chunk, &mut acc));
    } else if outer >= rayon::current_num_threads() * 2 {
        data.par_chunks_mut(block)
            .for_each_init(Vec::new, |acc, chunk| process_block(chunk, acc));
    } else {
        // Few blocks (the last axis): copy tiles of columns out, filter them in parallel, and
        // write them back row by row.
        let tiles: Vec<(usize, usize)> = (0..outer)
            .flat_map(|o| (0..stride).step_by(TILE).map(move |c0| (o, c0)))
            .collect();
        let src = &*data;
        let filtered: Vec<Vec<f64>> = tiles
            .par_iter()
            .map_init(Vec::new, |acc, &(o, c0)| {
                let w = TILE.min(stride - c0);
                let mut buf = vec![0.0f64; w * len];
                for t in 0..len {
                    let from = o * block + t * stride + c0;
                    buf[t * w..(t + 1) * w].copy_from_slice(&src[from..from + w]);
                }
                filter.apply_rows(&mut buf, w, 0..w, acc);
                buf
            })
            .collect();
        let per_block = stride.div_ceil(TILE);
        data.par_chunks_mut(stride)
            .enumerate()
            .for_each(|(r, row)| {
                let (o, t) = (r / len, r % len);
                for (k, buf) in filtered[o * per_block..(o + 1) * per_block]
                    .iter()
                    .enumerate()
                {
                    let c0 = k * TILE;
                    let w = TILE.min(stride - c0);
                    row[c0..c0 + w].copy_from_slice(&buf[t * w..(t + 1) * w]);
                }
            });
    }
}

/// `map_coordinates(input, coordinates, output, order, mode, cval, prefilter)` for an array of
/// `D` dimensions in Fortran order. `coordinates[n]` is the point of output element `n`.
///
/// The input is prefiltered once ([`Spline::new`]); points are evaluated in parallel on the
/// current rayon pool.
#[allow(clippy::too_many_arguments)]
pub fn map_coordinates<T: Sample, O: Output, const D: usize>(
    input: &[T],
    shape: [usize; D],
    coordinates: &[[f64; D]],
    output: &mut [O],
    order: u32,
    mode: Mode,
    cval: f64,
    prefilter: bool,
) -> Result<(), NdimageError> {
    if output.len() != coordinates.len() {
        return Err(NdimageError::Output);
    }
    let spline = Spline::new(input, shape, order, mode, cval, prefilter, true)?;
    const CHUNK: usize = 4096;
    // 3D inputs take the batched path (the same values, faster).
    let spline3 = (&spline as &dyn std::any::Any).downcast_ref::<Spline<3>>();
    output
        .par_chunks_mut(CHUNK)
        .zip(coordinates.par_chunks(CHUNK))
        .for_each(|(out, pts)| match spline3 {
            Some(s3) => {
                let pts3: Vec<[f64; 3]> =
                    pts.iter().map(|p| std::array::from_fn(|d| p[d])).collect();
                let mut t = vec![0.0f64; pts3.len()];
                s3.sample_batch(&pts3, &mut t);
                for (o, t) in out.iter_mut().zip(t) {
                    *o = O::from_interp(t);
                }
            }
            None => {
                for (o, p) in out.iter_mut().zip(pts) {
                    *o = O::from_interp(spline.sample(*p));
                }
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests;
