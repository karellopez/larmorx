// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy's `scipy.ndimage` filters, morphology and labelling, ported from SciPy 1.18.1 (the C
//! and Cython sources of these functions are the same in SciPy 1.15.3, the shared
//! environment's): the minimum and maximum filters (with a footprint, a non-flat structuring
//! element, or separable by axis), the rank, median and percentile filters, grey erosion,
//! dilation, opening and closing, binary erosion, dilation, opening, closing, propagation and
//! hole filling, `label`, `sum_labels`, `generate_binary_structure` and `iterate_structure`.
//!
//! They serve the ports of sdcflows' `brain_masker` and niworkflows' `IntensityClip` (grey
//! closing with `ball(1)`, a median filter with `ball(3)`, binary openings, closings, erosions and
//! dilations with balls, the largest connected component), and `lx.ndimage` for any caller.
//!
//! **Exactness.** Results are SciPy's, element for element and bit for bit: the boundary of each
//! mode is SciPy's (`NI_InitFilterOffsets` for the n-dimensional filters, `NI_ExtendLine` for the
//! one-dimensional ones), a footprint's elements are visited in SciPy's order (C order of the
//! footprint), the minimum and maximum keep SciPy's comparisons (so NaNs and the signs of zeros
//! travel as in SciPy), values with a structuring element are added in the array's own type as
//! SciPy's macros add them, the rank filter selects with SciPy's own quickselect (`NI_Select`,
//! so ties, zeros and NaNs come out as SciPy's), and every cast between a double and an element
//! type is C's on x86-64 (`CASE_FILTER_OUT_SAFE` for unsigned outputs). [`label`] numbers the
//! components as SciPy's `_ni_label` does: by the first voxel of each in C order.
//!
//! **Arrays.** Inputs are any strided layout ([`Layout`]: shape and element strides in index
//! order, as numpy's); outputs are C-ordered (SciPy allocates them with `numpy.zeros`). Every
//! output element is computed independently, so the work is split over the threads of the
//! current rayon pool and the result does not depend on their number.
//!
//! One deliberate difference: SciPy's `reflect` boundary of the n-dimensional filters maps a
//! coordinate `2·m·(2n)` before an axis of length `n` (`m >= 2`, a footprint more than eight
//! times the axis) to index `-1`, reading outside the array; larmorx maps it to index 0, the
//! reflection's value (`docs/findings/scipy-ndimage.md`).
//!
//! Copyright (c) 2001-2002 Enthought, Inc. 2003-2024, SciPy Developers.
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of the copyright holder nor the names of its contributors may be used
//!    to endorse or promote products derived from this software without specific prior
//!    written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
//! OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//! COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//! EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//! HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR
//! TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//! SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#![forbid(unsafe_code)]
// The ports keep SciPy's loops and comparisons: index loops and counters as in the C, negated
// comparisons whose NaN behaviour is SciPy's (`!(a > x)` is not `a <= x` for a NaN).
#![allow(
    clippy::needless_range_loop,
    clippy::explicit_counter_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::type_complexity,
    clippy::if_same_then_else
)]

use larmorx_core::numpy::NpFloat;

pub mod filters;
pub mod measurements;
pub mod morphology;
mod neighbourhood;
#[cfg(test)]
mod tests;

pub use filters::{Rank1d, min_or_max_filter, min_or_max_filter1d, rank_filter, rank_filter_1d};
pub use measurements::{LabelValue, Labels, label, sum_labels_bincount};
pub use morphology::{
    BinaryErosion, MaskElement, binary_erosion, center_is_true, dilation_arguments,
    generate_binary_structure, iterate_structure,
};

/// How a filter extends the input beyond its edges (SciPy's `NI_ExtendMode` as these filters
/// use it: `grid-wrap` is `wrap`, `grid-constant` is `constant`, `grid-mirror` is `reflect`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extend {
    /// `nearest` (`a a a | a b c d | d d d`).
    Nearest,
    /// `wrap` and `grid-wrap` (`b c d | a b c d | a b c`).
    Wrap,
    /// `reflect` and `grid-mirror` (`c b a | a b c d | d c b`).
    Reflect,
    /// `mirror` (`d c b | a b c d | c b a`).
    Mirror,
    /// `constant` and `grid-constant` (`cval` beyond the edges).
    Constant,
}

impl Extend {
    /// The extension for SciPy's mode name; `None` where SciPy raises `RuntimeError("boundary
    /// mode not supported")`.
    pub fn from_name(name: &str) -> Option<Extend> {
        Some(match name {
            "nearest" => Extend::Nearest,
            "wrap" | "grid-wrap" => Extend::Wrap,
            "reflect" | "grid-mirror" => Extend::Reflect,
            "mirror" => Extend::Mirror,
            "constant" | "grid-constant" => Extend::Constant,
            _ => return None,
        })
    }
}

/// Errors, with SciPy's messages where it has one.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NdimageError {
    /// The shapes or lengths of the arguments disagree.
    #[error("{0}")]
    Shape(String),
    /// `ValueError('invalid origin')`.
    #[error("invalid origin")]
    Origin,
    /// `RuntimeError('rank not within filter footprint size')`.
    #[error("rank not within filter footprint size")]
    Rank,
    /// `ValueError("All-zero footprint is not supported.")`.
    #[error("All-zero footprint is not supported.")]
    EmptyFootprint,
    /// `RuntimeError('structure must not be empty')`.
    #[error("structure must not be empty")]
    EmptyStructure,
    /// `ValueError('structure dimensions must be equal to 3')`.
    #[error("structure dimensions must be equal to 3")]
    LabelStructure,
    /// `AssertionError('Structuring element is not symmetric')` (`_ni_label`).
    #[error("Structuring element is not symmetric")]
    LabelSymmetry,
    /// numpy's `IndexError` of `_center_is_true` (a binary operation's origin beyond its
    /// structure).
    #[error("{0}")]
    CentreIndex(String),
    /// `AssertionError("Cannot label scalars or empty arrays")` (`_ni_label`).
    #[error("Cannot label scalars or empty arrays")]
    LabelEmpty,
    /// A binary erosion or dilation iterated until nothing changes that alternates between two
    /// states for ever (a structuring element without its centre); SciPy loops without end.
    #[error(
        "the binary erosion or dilation alternates between two states for ever (a structuring \
         element without its centre, iterations < 1): SciPy would not return"
    )]
    Oscillation,
}

/// An array's shape and element strides, in index order (numpy's, with strides counted in
/// elements, every one positive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The length of every axis.
    pub shape: Vec<usize>,
    /// The distance in elements between neighbours along every axis.
    pub strides: Vec<usize>,
}

impl Layout {
    /// A C-ordered array (the last axis contiguous).
    pub fn c(shape: &[usize]) -> Layout {
        Layout {
            shape: shape.to_vec(),
            strides: c_strides(shape),
        }
    }

    /// A Fortran-ordered array (the first axis contiguous).
    pub fn f(shape: &[usize]) -> Layout {
        let mut strides = vec![0; shape.len()];
        let mut s = 1;
        for (d, &n) in shape.iter().enumerate() {
            strides[d] = s;
            s *= n.max(1);
        }
        Layout {
            shape: shape.to_vec(),
            strides,
        }
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }

    /// Whether the array has no element.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of axes.
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// The offset of a multi-index.
    #[inline]
    pub(crate) fn offset(&self, idx: &[usize]) -> usize {
        idx.iter().zip(&self.strides).map(|(i, s)| i * s).sum()
    }

    /// The smallest buffer length that holds every element.
    pub fn span(&self) -> usize {
        if self.is_empty() {
            return 0;
        }
        1 + self
            .shape
            .iter()
            .zip(&self.strides)
            .map(|(n, s)| (n - 1) * s)
            .sum::<usize>()
    }
}

/// The element strides of a C-ordered array of `shape`.
pub fn c_strides(shape: &[usize]) -> Vec<usize> {
    let mut strides = vec![0; shape.len()];
    let mut s = 1;
    for d in (0..shape.len()).rev() {
        strides[d] = s;
        s *= shape[d].max(1);
    }
    strides
}

/// The multi-index of the C-order position `lin` in `shape`.
#[inline]
pub(crate) fn unravel(mut lin: usize, shape: &[usize], idx: &mut [usize]) {
    for d in (0..shape.len()).rev() {
        let n = shape[d];
        idx[d] = lin % n;
        lin /= n;
    }
}

/// C's conversion of a double to a 32-bit integer on x86-64 (`cvttsd2si`): truncation toward
/// zero, `i32::MIN` for a NaN or a value out of range.
#[inline]
pub(crate) fn cvtt_i32(v: f64) -> i32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v <= -2_147_483_649.0 {
        i32::MIN
    } else {
        v as i32
    }
}

/// C's conversion of a double to a 64-bit integer on x86-64 (`cvttsd2si`, 64-bit).
#[inline]
pub(crate) fn cvtt_i64(v: f64) -> i64 {
    if v.is_nan() || v >= 9_223_372_036_854_775_808.0 || v < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        v as i64
    }
}

/// C's conversion of a double to an unsigned 64-bit integer as GCC compiles it on x86-64:
/// `cvttsd2si` below 2^63, else of `v - 2^63` with the top bit flipped.
#[inline]
pub(crate) fn cvtt_u64(v: f64) -> u64 {
    const TWO63: f64 = 9_223_372_036_854_775_808.0;
    if v < TWO63 || v.is_nan() {
        cvtt_i64(v) as u64
    } else {
        (cvtt_i64(v - TWO63) as u64) ^ (1u64 << 63)
    }
}

/// An element type of SciPy's filters (numpy's `bool` is `u8`, as C's `npy_bool`).
pub trait NdElement: Copy + Send + Sync + PartialOrd + Default + 'static {
    /// Whether the value is not zero (C's truth of the value).
    fn nonzero(self) -> bool;
    /// `(double)value`.
    fn to_f64(self) -> f64;
    /// `(type)double`: C's cast on x86-64 (`CASE_FILTER_OUT`, `(_type)_cval`).
    fn from_f64(v: f64) -> Self;
    /// `CASE_FILTER_OUT_SAFE`'s cast (unsigned types: `v > -1 ? (type)v : -(type)(-v)`; the
    /// others as [`NdElement::from_f64`]).
    fn from_f64_safe(v: f64) -> Self {
        Self::from_f64(v)
    }
    /// `x += s` for a double `s` (`_res += _ss[0]` of `CASE_MIN_OR_MAX_POINT`): `(type)((double)x
    /// + s)`.
    fn add_double(self, s: f64) -> Self {
        Self::from_f64(self.to_f64() + s)
    }
    /// `x += (type)s` (`_tmp += (_type)_ss[_ii]`): `s` cast to the type, then C's addition in
    /// the type (integer promotion and truncation for the narrow integers, wrapping for the
    /// wide ones).
    fn add_cast(self, s: f64) -> Self;
}

macro_rules! narrow_int {
    ($t:ty, $unsigned:expr) => {
        impl NdElement for $t {
            #[inline]
            fn nonzero(self) -> bool {
                self != 0
            }
            #[inline]
            fn to_f64(self) -> f64 {
                self as f64
            }
            #[inline]
            fn from_f64(v: f64) -> Self {
                cvtt_i32(v) as $t
            }
            #[inline]
            fn from_f64_safe(v: f64) -> Self {
                if $unsigned && !(v > -1.0) {
                    (cvtt_i32(-v) as $t).wrapping_neg()
                } else {
                    cvtt_i32(v) as $t
                }
            }
            #[inline]
            fn add_cast(self, s: f64) -> Self {
                (self as i32).wrapping_add(Self::from_f64(s) as i32) as $t
            }
        }
    };
}

narrow_int!(u8, true);
narrow_int!(i8, false);
narrow_int!(u16, true);
narrow_int!(i16, false);

impl NdElement for i32 {
    #[inline]
    fn nonzero(self) -> bool {
        self != 0
    }
    #[inline]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        cvtt_i32(v)
    }
    #[inline]
    fn add_cast(self, s: f64) -> Self {
        self.wrapping_add(cvtt_i32(s))
    }
}

impl NdElement for u32 {
    #[inline]
    fn nonzero(self) -> bool {
        self != 0
    }
    #[inline]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        cvtt_i64(v) as u32
    }
    #[inline]
    fn from_f64_safe(v: f64) -> Self {
        if v > -1.0 {
            cvtt_i64(v) as u32
        } else {
            (cvtt_i64(-v) as u32).wrapping_neg()
        }
    }
    #[inline]
    fn add_cast(self, s: f64) -> Self {
        self.wrapping_add(Self::from_f64(s))
    }
}

impl NdElement for i64 {
    #[inline]
    fn nonzero(self) -> bool {
        self != 0
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        cvtt_i64(v)
    }
    #[inline]
    fn add_cast(self, s: f64) -> Self {
        self.wrapping_add(cvtt_i64(s))
    }
}

impl NdElement for u64 {
    #[inline]
    fn nonzero(self) -> bool {
        self != 0
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        cvtt_u64(v)
    }
    #[inline]
    fn from_f64_safe(v: f64) -> Self {
        if v > -1.0 {
            cvtt_u64(v)
        } else {
            cvtt_u64(-v).wrapping_neg()
        }
    }
    #[inline]
    fn add_cast(self, s: f64) -> Self {
        self.wrapping_add(cvtt_u64(s))
    }
}

// The floating-point conversions and additions are SSE's on every platform, NaNs included
// (`larmorx_core::numpy::NpFloat`).
impl NdElement for f32 {
    #[inline]
    fn nonzero(self) -> bool {
        self != 0.0
    }
    #[inline]
    fn to_f64(self) -> f64 {
        NpFloat::to_f64(self)
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        <f32 as NpFloat>::from_f64(v)
    }
    #[inline]
    fn add_double(self, s: f64) -> Self {
        <f32 as NpFloat>::from_f64(NpFloat::add(NpFloat::to_f64(self), s))
    }
    #[inline]
    fn add_cast(self, s: f64) -> Self {
        NpFloat::add(self, <f32 as NpFloat>::from_f64(s))
    }
}

impl NdElement for f64 {
    #[inline]
    fn nonzero(self) -> bool {
        self != 0.0
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        v
    }
    #[inline]
    fn add_double(self, s: f64) -> Self {
        NpFloat::add(self, s)
    }
    #[inline]
    fn add_cast(self, s: f64) -> Self {
        NpFloat::add(self, s)
    }
}

/// A footprint or structuring element's shape and truth values, C-ordered (numpy's
/// `footprint.flags.contiguous`, as SciPy reads it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footprint {
    /// The length of every axis.
    pub shape: Vec<usize>,
    /// The values, C-ordered.
    pub mask: Vec<bool>,
}

impl Footprint {
    /// A footprint of `shape` from its C-ordered values.
    pub fn new(shape: &[usize], mask: Vec<bool>) -> Result<Footprint, NdimageError> {
        if shape.iter().product::<usize>() != mask.len() {
            return Err(NdimageError::Shape(
                "footprint shape and values disagree".into(),
            ));
        }
        Ok(Footprint {
            shape: shape.to_vec(),
            mask,
        })
    }

    /// A footprint of `shape` that is true everywhere.
    pub fn full(shape: &[usize]) -> Footprint {
        Footprint {
            shape: shape.to_vec(),
            mask: vec![true; shape.iter().product()],
        }
    }

    /// The number of true elements.
    pub fn count(&self) -> usize {
        self.mask.iter().filter(|&&b| b).count()
    }

    /// The footprint reversed along every axis (`footprint[::-1, ::-1, ...]`).
    pub fn reversed(&self) -> Footprint {
        let mut mask = self.mask.clone();
        mask.reverse();
        Footprint {
            shape: self.shape.clone(),
            mask,
        }
    }
}

/// SciPy's origin check (`lenf // 2 + origin` within the footprint) for every axis.
pub(crate) fn check_origins(fshape: &[usize], origins: &[isize]) -> Result<(), NdimageError> {
    if origins.len() != fshape.len() {
        return Err(NdimageError::Shape(
            "sequence argument must have length equal to input rank".into(),
        ));
    }
    for (&lenf, &o) in fshape.iter().zip(origins) {
        let c = (lenf / 2) as isize + o;
        if c < 0 || c >= lenf as isize {
            return Err(NdimageError::Origin);
        }
    }
    Ok(())
}
