// SPDX-License-Identifier: Apache-2.0
//! numpy's elementwise `exp` and `log` (and `clip`) with the bits of its x86-64 Linux wheels (numpy 2.3.5 and
//! 2.5.4, measured on this project's machines: `validation/investigations/t2smap/numerics/`).
//!
//! - `float32` `exp`: numpy's own kernel on CPUs with AVX2 and FMA3 or AVX-512F
//!   ([`crate::math::expf_numpy`]; numpy 2.3.5 and 2.5.4 have the same kernel, and both widths
//!   give the same bits); on older x86-64 CPUs the C library's `expf`.
//! - `float32` `log`: numpy's own kernel on CPUs with AVX2 and FMA3 or AVX-512F
//!   ([`crate::math::logf_numpy`]); on older x86-64 CPUs numpy calls the C library's `logf`
//!   (`logf@GLIBC_2.27`, [`crate::math::logf_glibc`]).
//! - `float64` `exp` and `log`: on CPUs without AVX-512 numpy calls the C library's (glibc 2.35
//!   on this project's machines), through the old symbols its manylinux wheels bind
//!   (`exp@GLIBC_2.2.5`, `log@GLIBC_2.2.5`): the same values as the current ones
//!   ([`crate::math::exp_glibc`], [`crate::math::log_glibc`]) except that the logarithm of a
//!   negative number (or `-inf`) is C's positive `NAN`, not SSE's default NaN. On CPUs with
//!   AVX-512 (AVX512_SKX) numpy uses Intel's SVML kernels instead, which differ from glibc's
//!   and from the correctly rounded results in a few per cent of arguments (`exp`); larmorx
//!   follows the first path (as `OPENBLAS_CORETYPE=Haswell` for BLAS).
//!
//! These serve `reproduce`; larmorx's default is the correctly rounded functions
//! ([`crate::math::exp`], [`crate::math::log`], [`crate::math::logf`]).

/// numpy's `float32` `log` (x86-64, AVX2/FMA3 or AVX-512F path).
#[inline]
#[must_use]
pub fn log_f32(x: f32) -> f32 {
    crate::math::logf_numpy(x)
}

/// numpy's `float32` `exp` (x86-64, AVX2/FMA3 or AVX-512F path: numpy's own kernel,
/// [`crate::math::expf_numpy`]; on older x86-64 CPUs numpy calls the C library's `expf`,
/// [`crate::math::expf_glibc`]).
#[inline]
#[must_use]
pub fn exp_f32(x: f32) -> f32 {
    crate::math::expf_numpy(x)
}

/// `numpy.clip(a, a_min, a_max)` with scalar bounds of the array's type (`_npy_clip`'s loop for
/// constant bounds): when neither bound is a NaN, `x < a_min` gives `a_min`, then `x > a_max`
/// gives `a_max` (so `a_max` wins where `a_min > a_max`), and a NaN `x` is kept; when a bound is
/// a NaN, every element becomes that bound (`a_min`'s NaN first).
pub fn clip<T: super::NpFloat>(a: &mut [T], a_min: T, a_max: T) {
    if a_min.is_nan() || a_max.is_nan() {
        let fill = if a_min.is_nan() { a_min } else { a_max };
        a.iter_mut().for_each(|x| *x = fill);
        return;
    }
    for x in a.iter_mut() {
        let mut v = *x;
        if v < a_min {
            v = a_min;
        }
        if v > a_max {
            v = a_max;
        }
        *x = v;
    }
}

/// numpy's `float64` `exp` (x86-64 Linux without AVX-512: glibc 2.35's `exp@GLIBC_2.2.5`).
#[inline]
#[must_use]
pub fn exp_f64(x: f64) -> f64 {
    crate::math::exp_glibc(x)
}

/// numpy's `float64` `log` (x86-64 Linux without AVX-512: glibc 2.35's `log@GLIBC_2.2.5`; a
/// negative argument gives C's `NAN`, `0x7ff8000000000000`).
#[inline]
#[must_use]
pub fn log_f64(x: f64) -> f64 {
    if x < 0.0 {
        f64::from_bits(0x7ff8_0000_0000_0000)
    } else {
        crate::math::log_glibc(x)
    }
}

/// `numpy.nan_to_num(a)` of a float array (`copy=True`, `nan=0.0`, `posinf`/`neginf` the type's
/// largest and smallest finite values): NaN becomes `+0.0`, `+inf` `T::MAX`, `-inf` `T::MIN`;
/// every other value (a `-0.0` too) is kept.
pub fn nan_to_num<T: super::NpFloat>(a: &mut [T]) {
    let (max, min) = if std::mem::size_of::<T>() == 4 {
        (
            T::from_f64(f64::from(f32::MAX)),
            T::from_f64(f64::from(f32::MIN)),
        )
    } else {
        (T::from_f64(f64::MAX), T::from_f64(f64::MIN))
    };
    for x in a.iter_mut() {
        let v = x.to_f64();
        if v.is_nan() {
            *x = T::ZERO;
        } else if v == f64::INFINITY {
            *x = max;
        } else if v == f64::NEG_INFINITY {
            *x = min;
        }
    }
}

/// `numpy.diff(a, n, axis)` over the middle axis of a C-ordered `(outer, len, inner)` array:
/// `a[1:] - a[:-1]` along the axis, `n` times, in `T` (each difference one subtraction, so the
/// memory order does not matter); the result is C-ordered `(outer, len - n, inner)` (empty when
/// `n >= len`; `n == 0` returns `a` itself).
pub fn diff_axis<T: super::NpFloat>(a: &[T], shape: [usize; 3], n: usize) -> Vec<T> {
    let [outer, len, inner] = shape;
    assert_eq!(
        a.len(),
        outer * len * inner,
        "array length and shape disagree"
    );
    let mut cur = a.to_vec();
    let mut l = len;
    for _ in 0..n {
        if l == 0 {
            break;
        }
        let nl = l - 1;
        let mut next = Vec::with_capacity(outer * nl * inner);
        for o in 0..outer {
            for e in 0..nl {
                for j in 0..inner {
                    let hi = cur[(o * l + e + 1) * inner + j];
                    let lo = cur[(o * l + e) * inner + j];
                    next.push(hi.sub(lo));
                }
            }
        }
        cur = next;
        l = nl;
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::{diff_axis, nan_to_num};

    #[test]
    fn nan_to_num_and_diff() {
        let mut a = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.0, 2.5];
        nan_to_num(&mut a);
        assert_eq!(a[0].to_bits(), 0);
        assert_eq!((a[1], a[2]), (f32::MAX, f32::MIN));
        assert_eq!(a[3].to_bits(), (-0.0f32).to_bits());
        let mut b = [f64::INFINITY];
        nan_to_num(&mut b);
        assert_eq!(b[0], f64::MAX);
        // (1, 4, 2): columns [1, 4, 9, 16] and [0, 1, 0, 1]
        let x = [1.0f64, 0.0, 4.0, 1.0, 9.0, 0.0, 16.0, 1.0];
        assert_eq!(
            diff_axis(&x, [1, 4, 2], 1),
            vec![3.0, 1.0, 5.0, -1.0, 7.0, 1.0]
        );
        assert_eq!(diff_axis(&x, [1, 4, 2], 2), vec![2.0, -2.0, 2.0, 2.0]);
        assert!(diff_axis(&x, [1, 4, 2], 5).is_empty());
        assert_eq!(diff_axis(&x, [1, 4, 2], 0), x.to_vec());
    }

    #[test]
    fn negative_logarithms_are_cs_nan() {
        assert_eq!(super::log_f64(-1.0).to_bits(), 0x7ff8_0000_0000_0000);
        assert_eq!(
            super::log_f64(f64::NEG_INFINITY).to_bits(),
            0x7ff8_0000_0000_0000
        );
        assert_eq!(super::log_f64(-0.0), f64::NEG_INFINITY);
        assert_eq!(super::log_f64(1.0), 0.0);
        assert_eq!(super::exp_f64(0.0), 1.0);
        assert_eq!(super::exp_f32(0.0), 1.0);
    }

    #[test]
    fn clip_follows_numpys_constant_bounds_loop() {
        let mut a = [-1.0f32, 0.5, 2.0, f32::NAN];
        super::clip(&mut a, 0.0, 1.0);
        assert_eq!(&a[..3], &[0.0, 0.5, 1.0]);
        assert!(a[3].is_nan());
        let mut b = [-1.0f32, 0.5, 2.0];
        super::clip(&mut b, 1.0, 0.0); // a_min > a_max: a_max everywhere
        assert_eq!(b, [0.0, 0.0, 0.0]);
        let mut c = [1.0f64, 2.0];
        super::clip(&mut c, f64::NAN, 3.0);
        assert!(c.iter().all(|x| x.is_nan()));
    }
}
