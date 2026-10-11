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
//!
//! One SciPy defect is corrected by default (D13, `docs/api/upstream-defects.md`): the
//! prefilter's reflect boundary (SCI-03). [`UpstreamDefects::Reproduce`] gives SciPy's result.
#![allow(clippy::needless_range_loop)]

mod geometric;
pub(crate) mod splines;

pub use geometric::Spline;

use larmorx_core::RealElement;
use larmorx_core::defects::{self, Notice, UpstreamDefects};
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

    /// Whether C defines the conversion of `cval` to this type (always for floating types;
    /// for integer types, when it is not NaN and its truncation is within the type's range).
    /// Where it is not, numpy's cast gives x86-64's answer, and SCI-02 applies.
    fn pad_defined(_cval: f64) -> bool {
        true
    }

    /// [`Sample::pad_value`] with SCI-02 corrected: an undefined conversion clamps to the
    /// type's range, NaN to 0.
    fn pad_value_clamped(cval: f64) -> f64 {
        Self::pad_value(cval)
    }
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
            fn pad_defined(cval: f64) -> bool {
                cval > <$t>::MIN as f64 - 1.0 && cval < <$t>::MAX as f64 + 1.0
            }
            fn pad_value_clamped(cval: f64) -> f64 {
                cval as $t as f64
            }
        }
    )*};
}
sample_int!(u8, i8, u16, i16, u32, i32, u64, i64);

/// Output element types and SciPy's conversion of the interpolated double (`CASE_INTERP_OUT*`).
pub trait Output: Copy + Send + Sync + Default {
    fn from_interp(t: f64) -> Self;

    /// Whether SciPy's conversion of `t` reaches an undefined cast: NaN into a signed integer
    /// output (SCI-02). x86-64 gives `i32::MIN` there (0 once narrowed to 8 or 16 bits),
    /// `i64::MIN` for int64.
    fn interp_undefined(_t: f64) -> bool {
        false
    }

    /// [`Output::from_interp`] with SCI-02 corrected: NaN is 0.
    fn from_interp_clamped(t: f64) -> Self {
        Self::from_interp(t)
    }
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
    ($cast:ident: $($t:ty),*) => {$(
        impl Output for $t {
            fn from_interp(t: f64) -> Self {
                let mut t = if t > 0.0 { t + 0.5 } else { t - 0.5 };
                if t > <$t>::MAX as f64 { t = <$t>::MAX as f64; }
                if t < <$t>::MIN as f64 { t = <$t>::MIN as f64; }
                // Only NaN reaches the cast out of range.
                $cast(t) as $t
            }
            fn interp_undefined(t: f64) -> bool {
                t.is_nan()
            }
            fn from_interp_clamped(t: f64) -> Self {
                if t.is_nan() { 0 } else { Self::from_interp(t) }
            }
        }
    )*};
}
output_uint!(u8, u16, u32, u64);
// GCC converts to the narrow types through a 32-bit `cvttsd2si`, to int64 through a 64-bit one.
output_int!(c_cast_i32: i8, i16, i32);
output_int!(c_cast_i64: i64);

/// C's `(int)x` as x86-64 executes it: truncation toward zero, and `i32::MIN` for NaN and
/// values outside the range of `i32`.
#[inline(always)]
pub(crate) fn c_cast_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

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
/// does not depend on it. `defects` chooses between the exact reflect boundary and SciPy's
/// (SCI-03); the returned notices say whether that changed the result.
pub fn spline_filter1d(
    data: &mut [f64],
    shape: &[usize],
    axis: usize,
    order: u32,
    mode: Mode,
    parallel: bool,
    defects: UpstreamDefects,
) -> Result<Vec<Notice>, NdimageError> {
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
    let changed = order >= 2 && filter_axis(data, shape, axis, order, mode, parallel, defects);
    Ok(reflect_notice(changed, mode, defects).into_iter().collect())
}

/// `scipy.ndimage.spline_filter(input, order, output=np.float64, mode)`: the 1D filter along
/// every axis in turn, in place on a Fortran-ordered float64 array. `order` must be 2 to 5.
/// `defects` as for [`spline_filter1d`].
pub fn spline_filter(
    data: &mut [f64],
    shape: &[usize],
    order: u32,
    mode: Mode,
    parallel: bool,
    defects: UpstreamDefects,
) -> Result<Vec<Notice>, NdimageError> {
    if !(2..=5).contains(&order) {
        return Err(NdimageError::Order(order));
    }
    check_shape(data.len(), shape)?;
    let mut changed = false;
    for axis in 0..shape.len() {
        changed |= filter_axis(data, shape, axis, order, mode, parallel, defects);
    }
    Ok(reflect_notice(changed, mode, defects).into_iter().collect())
}

/// The SCI-03 notice when the reflect boundary's initialisation changed (fixed) or would have
/// changed (reproduced) the prefiltered coefficients.
pub fn reflect_notice(changed: bool, mode: Mode, defects: UpstreamDefects) -> Option<Notice> {
    changed.then(|| {
        let detail = match defects {
            UpstreamDefects::Fix => format!(
                "the spline prefilter's '{mode}' boundary uses the exact initialisation, so the \
                 coefficients differ from SciPy's on short axes"
            ),
            UpstreamDefects::Reproduce => format!(
                "the spline prefilter's '{mode}' boundary is initialised as SciPy does, so the \
                 splines do not pass through the samples on short axes"
            ),
        };
        Notice::new(&defects::SCI_03, defects, detail)
    })
}

/// Filters every line along `axis`. A line `(inner, outer)` has elements
/// `inner + stride·(t + len·outer)` for `t < len`, with `stride` the product of the earlier
/// axes' lengths. Returns whether SCI-03 changed (or would have changed) a coefficient.
pub(crate) fn filter_axis(
    data: &mut [f64],
    shape: &[usize],
    axis: usize,
    order: u32,
    mode: Mode,
    parallel: bool,
    defects: UpstreamDefects,
) -> bool {
    let len = shape[axis];
    if len <= 1 || data.is_empty() {
        return false;
    }
    let filter = splines::LineFilter::new(order, mode.extend(), len, defects);
    filter_lines(data, shape, axis, &filter, parallel);
    filter.changed()
}

fn filter_lines(
    data: &mut [f64],
    shape: &[usize],
    axis: usize,
    filter: &splines::LineFilter,
    parallel: bool,
) {
    let len = shape[axis];
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
/// current rayon pool. `defects` as for [`spline_filter1d`].
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
    defects: UpstreamDefects,
) -> Result<Vec<Notice>, NdimageError> {
    if output.len() != coordinates.len() {
        return Err(NdimageError::Output);
    }
    let spline = Spline::new(input, shape, order, mode, cval, prefilter, true, defects)?;
    const CHUNK: usize = 4096;
    // 3D inputs take the batched path (the same values, faster).
    let spline3 = (&spline as &dyn std::any::Any).downcast_ref::<Spline<3>>();
    // SCI-02: NaN into a signed integer output is an undefined cast in SciPy.
    let undefined = std::sync::atomic::AtomicUsize::new(0);
    let convert = |t: f64| -> O {
        if O::interp_undefined(t) {
            undefined.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if defects.fix() {
                return O::from_interp_clamped(t);
            }
        }
        O::from_interp(t)
    };
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
                    *o = convert(t);
                }
            }
            None => {
                for (o, p) in out.iter_mut().zip(pts) {
                    *o = convert(spline.sample(*p));
                }
            }
        });
    let mut notices = spline.notices();
    let n = undefined.into_inner();
    if n > 0 {
        let detail = match defects {
            UpstreamDefects::Fix => format!(
                "{n} NaN value(s) written to the integer output are 0; SciPy's cast is undefined \
                 (x86-64 gives the integer type's minimum, or 0 once narrowed)"
            ),
            UpstreamDefects::Reproduce => format!(
                "{n} NaN value(s) written to the integer output are converted as SciPy on x86-64 \
                 converts them"
            ),
        };
        notices.push(Notice::new(&defects::SCI_02, defects, detail));
    }
    Ok(notices)
}

#[cfg(test)]
mod tests;
