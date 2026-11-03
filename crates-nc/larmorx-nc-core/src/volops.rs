// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (newimage.h: samesize,
// no_mask_voxels, maskedIterator, cyclicIterator, smallestPositiveT; newimage.cc: the arithmetic
// operators, binarise, threshold, calculateExtrema, find_thresholds, calc_robustlimits,
// imageHistogram, newimagevox2mm_mat; newimagefns.h: abs). Copyright (C) 1999-2018 University of
// Oxford (Mark Jenkinson, Matthew Webster et al.). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! newimage's arithmetic, thresholds and statistics for every element type it is instantiated
//! with (`char`, `short`, `int`, `float`, `double`), as fslmaths uses them.
//!
//! - [`Arith`]: C's arithmetic of each type as FSL's x86-64 build performs it: `char` and
//!   `short` promoted to `int` and the result's low bits kept, `int` wrapping, `float` and
//!   `double` with SSE's NaNs ([`crate::sse`]); an integer division by zero (or of the most
//!   negative `int` by -1) is the processor's trap (`None`).
//! - The operators `+=`, `-=`, `*=`, `/=` of a volume with a number ([`Volume::op_scalar`]) and
//!   with another volume ([`Volume::op_volume`]: `samesize(*this, source, SUBSET)`, the source
//!   cycled over a volume with more time points).
//! - [`Volume::binarise`] and [`Volume::threshold`], inclusive or exclusive, inverted or not.
//! - [`Volume::extrema_masked`] (`calculateExtrema` with a mask), [`Volume::robust_limits_masked`]
//!   (`calc_robustlimits`: Steve Smith's 2%-98% thresholds for any type, with or without a mask).
//! - [`Volume::newimagevox2mm_mat`], [`samesize_subset`], [`abs_value`].

use crate::arma::Mat;
use crate::cnum;
use crate::nifti;
use crate::sse;
use crate::volume::{Elem, FslError, Volume, round_f64};

/// newimage's `maskDelimiter` (`maskThreshold()`).
pub const MASK_THRESHOLD: f64 = 0.5;

/// C's arithmetic of an element type as FSL's x86-64 build performs it.
pub trait Arith: Elem {
    /// Whether the type is `float` or `double` (`dtype(vol) >= DT_FLOAT`).
    const IS_FLOAT: bool;
    /// `std::numeric_limits<T>::max()`.
    const MAXV: Self;
    /// newimage's `smallestPositiveT`: 1 for the integers, `numeric_limits<T>::min()` for the
    /// floating types.
    const SMALLEST_POSITIVE: Self;
    /// `a += b`.
    fn add(a: Self, b: Self) -> Self;
    /// `a -= b`.
    fn sub(a: Self, b: Self) -> Self;
    /// `a *= b`.
    fn mul(a: Self, b: Self) -> Self;
    /// `a /= b`; `None` where the integer division traps (a division by 0, or `INT_MIN / -1`).
    fn div(a: Self, b: Self) -> Option<Self>;
    /// `(double)(a - b)`: the difference in C's arithmetic (an `int` for `char` and `short`),
    /// converted to `double`.
    fn diff(a: Self, b: Self) -> f64;
    /// `(double)(-a)`.
    fn neg_f64(a: Self) -> f64;
    /// `a + k` with C's promotions, the result converted back to the type (`max() + 1`).
    fn plus_int(a: Self, k: i32) -> Self;
    /// `(T) i` from an `int`.
    fn from_int(i: i32) -> Self;
    /// `(int) a` as x86-64 converts it.
    fn to_int(a: Self) -> i32;
    /// Whether `(int) a` is defined (in range, not NaN); always for the integer types.
    fn int_defined(a: Self) -> bool;
    /// `(int) a` clamped to `int`'s range, NaN as 0 (larmorx's correction of the undefined
    /// conversions).
    fn to_int_clamped(a: Self) -> i32;
    /// `a != 0`.
    fn nonzero(a: Self) -> bool;
    /// Whether the value is NaN.
    fn is_nan(a: Self) -> bool;
}

macro_rules! int_arith {
    ($t:ty, $wide:ty) => {
        impl Arith for $t {
            const IS_FLOAT: bool = false;
            const MAXV: Self = <$t>::MAX;
            const SMALLEST_POSITIVE: Self = 1;
            #[inline]
            fn add(a: Self, b: Self) -> Self {
                a.wrapping_add(b)
            }
            #[inline]
            fn sub(a: Self, b: Self) -> Self {
                a.wrapping_sub(b)
            }
            #[inline]
            fn mul(a: Self, b: Self) -> Self {
                a.wrapping_mul(b)
            }
            #[inline]
            fn div(a: Self, b: Self) -> Option<Self> {
                // char and short divide as int: only a 0 divisor traps
                let (wa, wb) = (<$wide>::from(a), <$wide>::from(b));
                wa.checked_div(wb).map(|q| q as $t)
            }
            #[inline]
            fn diff(a: Self, b: Self) -> f64 {
                f64::from((<$wide>::from(a)).wrapping_sub(<$wide>::from(b)))
            }
            #[inline]
            fn neg_f64(a: Self) -> f64 {
                f64::from((<$wide>::from(a)).wrapping_neg())
            }
            #[inline]
            fn plus_int(a: Self, k: i32) -> Self {
                (i32::from(a)).wrapping_add(k) as $t
            }
            #[inline]
            fn from_int(i: i32) -> Self {
                i as $t
            }
            #[inline]
            fn to_int(a: Self) -> i32 {
                i32::from(a)
            }
            #[inline]
            fn int_defined(_a: Self) -> bool {
                true
            }
            #[inline]
            fn to_int_clamped(a: Self) -> i32 {
                i32::from(a)
            }
            #[inline]
            fn nonzero(a: Self) -> bool {
                a != 0
            }
            #[inline]
            fn is_nan(_a: Self) -> bool {
                false
            }
        }
    };
}

int_arith!(i8, i32);
int_arith!(i16, i32);
int_arith!(i32, i32);

impl Arith for f32 {
    const IS_FLOAT: bool = true;
    const MAXV: Self = f32::MAX;
    const SMALLEST_POSITIVE: Self = f32::MIN_POSITIVE;
    #[inline]
    fn add(a: Self, b: Self) -> Self {
        sse::add_f32(a, b)
    }
    #[inline]
    fn sub(a: Self, b: Self) -> Self {
        sse::sub_f32(a, b)
    }
    #[inline]
    fn mul(a: Self, b: Self) -> Self {
        sse::mul_f32(a, b)
    }
    #[inline]
    fn div(a: Self, b: Self) -> Option<Self> {
        Some(sse::div_f32(a, b))
    }
    #[inline]
    fn diff(a: Self, b: Self) -> f64 {
        f64::from(sse::sub_f32(a, b))
    }
    #[inline]
    fn neg_f64(a: Self) -> f64 {
        f64::from(-a)
    }
    #[inline]
    fn plus_int(a: Self, k: i32) -> Self {
        sse::add_f32(a, k as f32)
    }
    #[inline]
    fn from_int(i: i32) -> Self {
        i as f32
    }
    #[inline]
    fn to_int(a: Self) -> i32 {
        cnum::f32_to_i32(a)
    }
    #[inline]
    fn int_defined(a: Self) -> bool {
        !a.is_nan() && (-2_147_483_648.0..2_147_483_648.0).contains(&a)
    }
    #[inline]
    fn to_int_clamped(a: Self) -> i32 {
        if a.is_nan() { 0 } else { a as i32 }
    }
    #[inline]
    fn nonzero(a: Self) -> bool {
        a != 0.0
    }
    #[inline]
    fn is_nan(a: Self) -> bool {
        a.is_nan()
    }
}

impl Arith for f64 {
    const IS_FLOAT: bool = true;
    const MAXV: Self = f64::MAX;
    const SMALLEST_POSITIVE: Self = f64::MIN_POSITIVE;
    #[inline]
    fn add(a: Self, b: Self) -> Self {
        sse::add_f64(a, b)
    }
    #[inline]
    fn sub(a: Self, b: Self) -> Self {
        sse::sub_f64(a, b)
    }
    #[inline]
    fn mul(a: Self, b: Self) -> Self {
        sse::mul_f64(a, b)
    }
    #[inline]
    fn div(a: Self, b: Self) -> Option<Self> {
        Some(sse::div_f64(a, b))
    }
    #[inline]
    fn diff(a: Self, b: Self) -> f64 {
        sse::sub_f64(a, b)
    }
    #[inline]
    fn neg_f64(a: Self) -> f64 {
        -a
    }
    #[inline]
    fn plus_int(a: Self, k: i32) -> Self {
        sse::add_f64(a, f64::from(k))
    }
    #[inline]
    fn from_int(i: i32) -> Self {
        f64::from(i)
    }
    #[inline]
    fn to_int(a: Self) -> i32 {
        cnum::f64_to_i32(a)
    }
    #[inline]
    fn int_defined(a: Self) -> bool {
        !a.is_nan() && a > -2_147_483_649.0 && a < 2_147_483_648.0
    }
    #[inline]
    fn to_int_clamped(a: Self) -> i32 {
        if a.is_nan() { 0 } else { a as i32 }
    }
    #[inline]
    fn nonzero(a: Self) -> bool {
        a != 0.0
    }
    #[inline]
    fn is_nan(a: Self) -> bool {
        a.is_nan()
    }
}

/// `(T) fabs((double) x)` (newimagefns' `abs`): for `float`, the value through `double` and
/// back (a signalling NaN quieted, as the conversions do), its sign cleared.
pub fn abs_value<T: Arith>(x: T) -> T {
    match T::DTYPE {
        nifti::dt::FLOAT => {
            let f = x.to_f32();
            let f = if f.is_nan() { sse::quiet_f32(f) } else { f };
            T::from_f32(f32::from_bits(f.to_bits() & 0x7fff_ffff))
        }
        nifti::dt::DOUBLE => T::from_f64(f64::from_bits(x.to_f64().to_bits() & !(1u64 << 63))),
        _ => T::from_f64(x.to_f64().abs()),
    }
}

/// `samesize(vol1, vol2, SUBSET)`: the first `min(dimensionality)` sizes equal, and `vol1` of
/// at least `vol2`'s dimensionality.
pub fn samesize_subset<S: Elem, T: Elem>(v1: &Volume<S>, v2: &Volume<T>) -> bool {
    let (d1, d2) = (v1.dimensionality(), v2.dimensionality());
    let n = d1.min(d2);
    let mut same = v1.dims[0] == v2.dims[0] && d1 >= d2;
    for k in 1..7 {
        if n > k as i32 {
            same = same && v1.dims[k] == v2.dims[k];
        }
    }
    same
}

/// Whether the voxel `k` is in `mask` (`maskedIterator`: above 0.5, the mask cycled), or
/// every voxel for an empty mask.
#[inline]
fn in_mask<T: Elem>(mask: &[T], k: usize) -> bool {
    mask.is_empty() || mask[k % mask.len()].to_f64() > MASK_THRESHOLD
}

/// The operator of [`Volume::op_scalar`] and [`Volume::op_volume`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn verb(self) -> &'static str {
        match self {
            Op::Add => "add",
            Op::Sub => "subtract",
            Op::Mul => "multiply",
            Op::Div => "divide",
        }
    }

    #[inline]
    fn apply<T: Arith>(self, a: T, b: T) -> Option<T> {
        match self {
            Op::Add => Some(T::add(a, b)),
            Op::Sub => Some(T::sub(a, b)),
            Op::Mul => Some(T::mul(a, b)),
            Op::Div => T::div(a, b),
        }
    }
}

/// An integer division that traps (x86-64's SIGFPE): the voxel where it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivideTrap {
    pub voxel: usize,
}

impl<T: Arith> Volume<T> {
    /// `*this op= val` over every value (`nonsafe_fast_iterator`). `Err` is the first value
    /// whose integer division traps.
    pub fn op_scalar(&mut self, op: Op, val: T) -> Result<(), DivideTrap> {
        for (k, x) in self.data.iter_mut().enumerate() {
            *x = op.apply(*x, val).ok_or(DivideTrap { voxel: k })?;
        }
        Ok(())
    }

    /// `*this op= source`: FSL's "Attempted to add images of different sizes" (exception 3)
    /// unless `samesize(*this, source, SUBSET)`; the source cycled over this volume's values.
    /// The inner `Err` is a trapping integer division.
    pub fn op_volume(
        &mut self,
        op: Op,
        source: &Volume<T>,
    ) -> Result<Result<(), DivideTrap>, FslError> {
        if self.dimensionality() < source.dimensionality() || !samesize_subset(self, source) {
            return Err(FslError::imthrow(
                format!("Attempted to {} images of different sizes", op.verb()),
                3,
            ));
        }
        let n = source.data.len();
        if n == 0 {
            return Ok(Ok(()));
        }
        for (k, x) in self.data.iter_mut().enumerate() {
            match op.apply(*x, source.data[k % n]) {
                Some(v) => *x = v,
                None => return Ok(Err(DivideTrap { voxel: k })),
            }
        }
        Ok(Ok(()))
    }

    /// `*this *= *this` (`-sqr`).
    pub fn square(&mut self) {
        for x in &mut self.data {
            *x = T::mul(*x, *x);
        }
    }

    /// `binarise(lower, upper, tt, invert)`: 1 inside the range (`>= lower && <= upper`
    /// inclusive, `> lower && < upper` exclusive), 0 elsewhere; the other way round inverted.
    pub fn binarise(&mut self, lower: T, upper: T, inclusive: bool, invert: bool) {
        let (one, zero) = (T::from_int(1), T::from_int(0));
        for x in &mut self.data {
            let inside = if inclusive {
                *x >= lower && *x <= upper
            } else {
                *x > lower && *x < upper
            };
            *x = if inside != invert { one } else { zero };
        }
    }

    /// `threshold(lower, upper, tt, invert)`: the values outside the range (inside, inverted)
    /// set to 0.
    pub fn threshold(&mut self, lower: T, upper: T, inclusive: bool, invert: bool) {
        let zero = T::from_int(0);
        for x in &mut self.data {
            let inside = if inclusive {
                *x >= lower && *x <= upper
            } else {
                *x > lower && *x < upper
            };
            if invert == inside {
                *x = zero;
            }
        }
    }

    /// `calculateExtrema(*this, coords, mask)`: `(max, min)` of the masked values (every value
    /// for an empty mask) from the first, each larger one a new maximum, else each smaller a
    /// new minimum; `(0, 0)` with FSL's "calculateExtrema: mask is empty" (returned) when no
    /// value is in a non-empty volume's mask. `Err` is the mask of another size.
    pub fn extrema_masked(
        &self,
        mask: Option<&Volume<T>>,
    ) -> Result<((T, T), Option<&'static str>), FslError> {
        let m: &[T] = mask.map_or(&[], |m| m.data.as_slice());
        if let Some(mv) = mask
            && !mv.data.is_empty()
            && !samesize_subset(self, mv)
        {
            return Err(mask_size_error("calculateExtrema: "));
        }
        let mut it = self
            .data
            .iter()
            .enumerate()
            .filter(|(k, _)| in_mask(m, *k))
            .map(|(_, &v)| v);
        let Some(first) = it.next() else {
            let msg = (!self.data.is_empty()).then_some("calculateExtrema: mask is empty");
            return Ok(((T::default(), T::default()), msg));
        };
        let (mut mx, mut mn) = (first, first);
        for v in it {
            if v > mx {
                mx = v;
            } else if v < mn {
                mn = v;
            }
        }
        Ok(((mx, mn), None))
    }

    /// The extremes with NaN values left out (larmorx's correction of FSL-153: newimage's
    /// extremes start from the first value, so a NaN first value makes both NaN); `None` when
    /// every value is NaN.
    pub fn extrema_without_nan(&self) -> Option<(T, T)> {
        let mut it = self.data.iter().copied().filter(|&v| !T::is_nan(v));
        let first = it.next()?;
        let (mut mx, mut mn) = (first, first);
        for v in it {
            if v > mx {
                mx = v;
            } else if v < mn {
                mn = v;
            }
        }
        Some((mx, mn))
    }

    /// `no_mask_voxels(mask)`: the values above 0.5.
    pub fn no_mask_voxels(&self) -> usize {
        self.data
            .iter()
            .filter(|v| v.to_f64() > MASK_THRESHOLD)
            .count()
    }

    /// `calc_robustlimits(vol)` or, with a mask, `calc_robustlimits(vol, mask)`:
    /// `[robustmin, robustmax]` as FSL computes them and corrected ([`RobustRange`], FSL-187),
    /// and the warning printed on standard error ("WARNING:: Empty
    /// mask image in calc_robustlimits": the limits are then 0). `Err` is a mask of another
    /// size.
    pub fn robust_limits_masked(
        &self,
        mask: Option<&Volume<T>>,
    ) -> Result<(RobustRange<T>, Option<&'static str>), FslError> {
        if let Some(m) = mask
            && m.nvoxels() > 0
            && m.no_mask_voxels() == 0
        {
            let zero = T::from_int(0);
            let r = RobustRange {
                fsl: (zero, zero),
                fixed: (zero, zero),
                overflow: false,
            };
            return Ok((r, Some("WARNING:: Empty mask image in calc_robustlimits")));
        }
        robust_range(self, mask).map(|r| (r, None))
    }

    /// `newimagevox2mm_mat()`: the sform, else the qform, else the voxel sizes.
    pub fn newimagevox2mm_mat(&self) -> Mat {
        if self.sform_code != nifti::XFORM_UNKNOWN {
            self.sform.clone()
        } else if self.qform_code != nifti::XFORM_UNKNOWN {
            self.qform.clone()
        } else {
            let mut m = Mat::eye(4);
            m.set(0, 0, f64::from(self.xdim()));
            m.set(1, 1, f64::from(self.ydim()));
            m.set(2, 2, f64::from(self.zdim()));
            m
        }
    }
}

fn mask_size_error(source: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: format!("{source}maskedIterator: mask and volume must be the same size"),
    }
}

/// `imageHistogram(vol, bins, min, max, hist, mask)`: every masked value counted in
/// `hist[max(0, min((int)(fA*v + fB), bins-1))]`; the number of values, or -1 when
/// `max <= min`. A value more than about 2^31 bins above the range converts to `INT_MIN` on
/// x86-64 (C leaves the conversion undefined) and so counts in the first bin (FSL-187); with
/// `clamp` it counts in the last. `overflow` is set when any value's bin did not fit.
fn image_histogram<T: Arith>(
    vol: &Volume<T>,
    bins: i32,
    (min, max): (T, T),
    mask: &[T],
    hist: &mut [f64],
    clamp: bool,
    overflow: &mut bool,
) -> i64 {
    if max <= min {
        return -1;
    }
    hist.iter_mut().for_each(|h| *h = 0.0);
    let range = T::diff(max, min);
    let fa = f64::from(bins) / range;
    let fb = (f64::from(bins) * T::neg_f64(min)) / range;
    let mut n = 0i64;
    for (k, &v) in vol.data.iter().enumerate() {
        if !in_mask(mask, k) {
            continue;
        }
        let x = fa * v.to_f64() + fb;
        let b = if x >= 2_147_483_648.0 {
            *overflow = true;
            if clamp { bins - 1 } else { 0 }
        } else {
            cnum::f64_to_i32(x).min(bins - 1).max(0)
        };
        hist[b as usize] += 1.0;
        n += 1;
    }
    n
}

/// FSL's robust range and the corrected one (FSL-187): the same unless a value lies more than
/// about 2^31 bins above a pass's histogram range, which FSL counts in the first bin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobustRange<T> {
    /// `find_thresholds` as FSL computes it.
    pub fsl: (T, T),
    /// With such values counted in the last bin.
    pub fixed: (T, T),
    /// Whether any value's bin left `int`'s range in some pass (the ranges may still agree).
    pub overflow: bool,
}

impl<T: Arith> RobustRange<T> {
    /// The corrected range, or FSL's with `reproduce`.
    pub fn pick(&self, reproduce: bool) -> (T, T) {
        if reproduce { self.fsl } else { self.fixed }
    }

    /// Whether the two ranges differ (bit for bit).
    pub fn differs(&self) -> bool {
        let bits = |(a, b): (T, T)| (a.to_f64().to_bits(), b.to_f64().to_bits());
        bits(self.fsl) != bits(self.fixed)
    }

    /// FSL-187's notice text for `what` (the caller's name of the range), when the ranges
    /// differ.
    pub fn notice(&self, what: &str) -> Option<String> {
        self.differs().then(|| {
            format!(
                "{what}: values far above a pass's histogram range counted in its last bin, not \
                 its first: robust range {} {} (FSL's {} {})",
                self.fixed.0.to_f64(),
                self.fixed.1.to_f64(),
                self.fsl.0.to_f64(),
                self.fsl.1.to_f64()
            )
        })
    }
}

/// [`find_thresholds`] as FSL computes it and corrected ([`RobustRange`]); the corrected pass
/// runs only when a value's bin did not fit in FSL's.
pub fn robust_range<T: Arith>(
    vol: &Volume<T>,
    mask: Option<&Volume<T>>,
) -> Result<RobustRange<T>, FslError> {
    let (fsl, overflow) = thresholds(vol, mask, false)?;
    let fixed = if overflow {
        thresholds(vol, mask, true)?.0
    } else {
        fsl
    };
    Ok(RobustRange {
        fsl,
        fixed,
        overflow,
    })
}

/// `find_thresholds(vol, minval, maxval, mask, use_mask)` for any element type: from a
/// 1000-bin histogram between the (masked) extremes, the bins holding 2% and 98% of the
/// values, refined over at most 10 passes while the range they span is under a tenth of the
/// histogram's, in the type's own arithmetic where FSL's code computes in it.
pub fn find_thresholds<T: Arith>(
    vol: &Volume<T>,
    mask: Option<&Volume<T>>,
) -> Result<(T, T), FslError> {
    thresholds(vol, mask, false).map(|r| r.0)
}

/// [`find_thresholds`], with FSL-187's bins clamped when `clamp`; and whether any value's bin
/// did not fit in an `int`.
fn thresholds<T: Arith>(
    vol: &Volume<T>,
    mask: Option<&Volume<T>>,
    clamp: bool,
) -> Result<((T, T), bool), FslError> {
    let mut overflow = false;
    const BINS: i32 = 1000;
    const MAX_PASSES: i32 = 10;
    let m: &[T] = mask.map_or(&[], |m| m.data.as_slice());
    let extremes = || -> Result<(T, T), FslError> {
        Ok(vol.extrema_masked(mask.filter(|m| !m.data.is_empty()))?.0)
    };
    let mut hist = vec![0f64; BINS as usize];
    let (mut top_bin, mut bottom_bin) = (0i32, 0i32);
    let mut pass = 1;
    let mut lowest_bin = 0i32;
    let mut highest_bin = BINS - 1;
    let zero = T::from_int(0);
    let (mut thresh98, mut thresh2) = (zero, zero);
    let (vmax, vmin) = extremes()?;
    let (mut min, mut max) = (vmin, vmax);
    while pass == 1 || T::diff(thresh98, thresh2) < T::diff(max, min) / 10.0 {
        if pass > 1 {
            bottom_bin = (bottom_bin - 1).max(0);
            top_bin = (top_bin + 1).min(BINS - 1);
            let range = T::diff(max, min);
            let tmpmin = T::from_f64(
                min.to_f64() + sse::mul_f64(f64::from(bottom_bin) / f64::from(BINS), range),
            );
            max = T::from_f64(
                min.to_f64() + sse::mul_f64(f64::from(top_bin + 1) / f64::from(BINS), range),
            );
            min = tmpmin;
        }
        if pass == MAX_PASSES || min == max {
            let (a, b) = extremes()?;
            min = b;
            max = a;
        }
        let mut validsize =
            image_histogram(vol, BINS, (min, max), m, &mut hist, clamp, &mut overflow);
        if validsize < 1 {
            return Ok(((min, max), overflow));
        }
        if pass == MAX_PASSES {
            validsize -= i64::from(round_f64(hist[lowest_bin as usize]))
                + i64::from(round_f64(hist[highest_bin as usize]));
            lowest_bin += 1;
            highest_bin -= 1;
        }
        if validsize < 0 {
            thresh2 = min;
            thresh98 = min;
            break;
        }
        let fa = T::diff(max, min) / f64::from(BINS);
        let mut count: i32 = 0;
        bottom_bin = lowest_bin;
        while i64::from(count) < validsize / 50 {
            count += round_f64(hist[bottom_bin as usize]);
            bottom_bin += 1;
        }
        bottom_bin -= 1;
        thresh2 = T::add(min, T::from_f64(sse::mul_f64(f64::from(bottom_bin), fa)));
        count = 0;
        top_bin = highest_bin;
        while i64::from(count) < validsize / 50 {
            count += round_f64(hist[top_bin as usize]);
            top_bin -= 1;
        }
        top_bin += 1;
        thresh98 = T::add(min, T::from_f64(sse::mul_f64(f64::from(top_bin + 1), fa)));
        if pass == MAX_PASSES {
            break;
        }
        pass += 1;
    }
    Ok(((thresh2, thresh98), overflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_arithmetic_wraps_as_c() {
        assert_eq!(i8::add(100, 100), -56);
        assert_eq!(i8::div(-128, -1), Some(-128));
        assert_eq!(i32::div(i32::MIN, -1), None);
        assert_eq!(i16::div(5, 0), None);
        assert_eq!(i8::plus_int(127, 1), -128);
        assert_eq!(f32::plus_int(16_777_216.0, 1), 16_777_216.0);
    }

    #[test]
    fn subset_sizes() {
        let a = Volume::<f32>::new(4, 3, 2, 5);
        let b = Volume::<f32>::new(4, 3, 2, 1);
        let c = Volume::<f32>::new(4, 1, 1, 1);
        let d = Volume::<f32>::new(4, 3, 2, 3);
        assert!(samesize_subset(&a, &b));
        assert!(samesize_subset(&a, &c));
        assert!(!samesize_subset(&a, &d));
        assert!(!samesize_subset(&b, &a));
    }
}
