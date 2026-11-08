// SPDX-License-Identifier: Apache-2.0 AND MIT
//! The affine nibabel reads from a NIfTI header, bit for bit: `Nifti1Header.get_best_affine`
//! of nibabel 5.4.2 (MIT, Copyright the NiBabel developers), whose qform is computed in numpy's
//! `longdouble`.
//!
//! [`NiftiHeader::best_affine`] computes the qform in plain float64 (`R[i][j] · vox[j]`), the
//! same on every platform. nibabel's `get_qform` computes it in `longdouble`, which is x87's
//! extended format (64-bit significand) on x86-64 Linux: `quaternions.fillpositive` (`w =
//! √(1 − ((0 + b²) + c²) + d²)`, 0 within the header's `quaternion_threshold`),
//! `quaternions.quat2mat` and `np.dot(R, diag(pixdim[1:4] · (1, 1, qfac)))` through numpy's
//! `LONGDOUBLE_dot` (from `+0`, each product and sum rounded to 64 bits), then one rounding to
//! float64. The results differ from the plain float64 ones in the last bit of some oblique
//! rotations, and in the sign of zero: the dot product's sum from `+0` makes every zero `+0`,
//! where `0 · (−vox)` is `−0`. Ports of programs that read images through nibabel (tedana's
//! `t2smap`, which writes the affine it read into every output) use
//! [`NiftiHeader::best_affine_nibabel`]; this module emulates the extended arithmetic exactly
//! (integer significands, round to nearest with ties to even), so every platform gives x86-64
//! Linux's bits. The same arithmetic in Python is `larmorx.io.nibabel_header` (`_get_qform`).
//!
//! Ported from nibabel 5.4.2: `nifti1.py` (`get_best_affine`, `get_qform`,
//! `quaternion_threshold`), `nifti2.py` (`quaternion_threshold`), `quaternions.py`
//! (`fillpositive`, `quat2mat`); numpy 2.5.4's `LONGDOUBLE_dot` (`arraytypes.c.src`) and the
//! `longdouble` `matmul` inner loop (BSD-3-Clause, Copyright NumPy Developers).

use std::cmp::Ordering;

use larmorx_core::affine::Affine;
use larmorx_core::linalg::Mat3;

use super::header::{HeaderError, NiftiHeader, NiftiVersion};

/// A finite x87 extended value `(−1)^neg · m · 2^e` with `2^63 <= m < 2^64`, or a signed zero
/// (`m == 0`). The exponent range is not limited: no value of these formulas leaves x87's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct X87 {
    neg: bool,
    m: u64,
    e: i32,
}

impl X87 {
    const fn zero(neg: bool) -> Self {
        Self { neg, m: 0, e: 0 }
    }

    /// A finite double, exactly (subnormals included).
    fn of(x: f64) -> Self {
        debug_assert!(x.is_finite());
        let neg = x.is_sign_negative();
        let bits = x.abs().to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & ((1u64 << 52) - 1);
        let (m, e) = if biased == 0 {
            (frac, -1074)
        } else {
            (frac | (1u64 << 52), biased - 1075)
        };
        if m == 0 {
            return Self::zero(neg);
        }
        let lz = m.leading_zeros();
        Self {
            neg,
            m: m << lz,
            e: e - lz as i32,
        }
    }

    fn is_zero(self) -> bool {
        self.m == 0
    }

    /// `mag · 2^e` (`mag > 0`, plus a nonzero part below its last bit when `sticky`) rounded to
    /// a 64-bit significand, to nearest with ties to even.
    fn round(neg: bool, mag: u128, e: i32, sticky: bool) -> Self {
        debug_assert!(mag != 0);
        let width = 128 - mag.leading_zeros() as i32;
        if width <= 64 {
            debug_assert!(!sticky);
            return Self {
                neg,
                m: (mag << (64 - width)) as u64,
                e: e - (64 - width),
            };
        }
        let cut = width - 64;
        let mut q = mag >> cut;
        let rest = mag & ((1u128 << cut) - 1);
        let half = 1u128 << (cut - 1);
        if rest > half || (rest == half && (sticky || q & 1 == 1)) {
            q += 1;
        }
        let mut e = e + cut;
        if q >> 64 != 0 {
            q >>= 1;
            e += 1;
        }
        Self {
            neg,
            m: q as u64,
            e,
        }
    }

    /// The magnitudes compared.
    fn cmp_abs(self, other: Self) -> Ordering {
        match (self.is_zero(), other.is_zero()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => (self.e, self.m).cmp(&(other.e, other.m)),
        }
    }

    fn neg(self) -> Self {
        Self {
            neg: !self.neg,
            ..self
        }
    }

    /// `fadd`: `a + b` rounded to the extended format (IEEE's zeros: `+0` for an exact zero
    /// of two nonzero values, `−0` only for `−0 + −0`).
    fn add(a: Self, b: Self) -> Self {
        if a.is_zero() && b.is_zero() {
            return Self::zero(a.neg && b.neg);
        }
        if a.is_zero() {
            return b;
        }
        if b.is_zero() {
            return a;
        }
        let (big, small) = if a.cmp_abs(b) == Ordering::Less {
            (b, a)
        } else {
            (a, b)
        };
        let d = (big.e - small.e) as u32;
        let top = u128::from(big.m) << 63;
        let (low, sticky) = if d >= 127 {
            (0u128, true)
        } else {
            let s = u128::from(small.m) << 63;
            (s >> d, d > 0 && s & ((1u128 << d) - 1) != 0)
        };
        if big.neg == small.neg {
            return Self::round(big.neg, top + low, big.e - 63, sticky);
        }
        // top − (low + f) with 0 < f < 1 when sticky: (top − low − 1) and a nonzero remainder.
        let mag = top - low - u128::from(sticky);
        if mag == 0 {
            return Self::zero(false);
        }
        Self::round(big.neg, mag, big.e - 63, sticky)
    }

    fn sub(a: Self, b: Self) -> Self {
        Self::add(a, b.neg())
    }

    /// `fmul`: exact product rounded to the extended format.
    fn mul(a: Self, b: Self) -> Self {
        let neg = a.neg != b.neg;
        if a.is_zero() || b.is_zero() {
            return Self::zero(neg);
        }
        Self::round(neg, u128::from(a.m) * u128::from(b.m), a.e + b.e, false)
    }

    /// `fdiv`: `a / b` (`b` nonzero) rounded to the extended format.
    fn div(a: Self, b: Self) -> Self {
        debug_assert!(!b.is_zero());
        let neg = a.neg != b.neg;
        if a.is_zero() {
            return Self::zero(neg);
        }
        let n = u128::from(a.m) << 64;
        let d = u128::from(b.m);
        let mut q = n / d;
        let r = n % d;
        let mut e = a.e - b.e - 64;
        if q >> 64 != 0 {
            // 65 bits: the last one decides with the remainder.
            let low = q & 1;
            q >>= 1;
            e += 1;
            if low == 1 && (r != 0 || q & 1 == 1) {
                q += 1;
            }
        } else {
            match (2 * r).cmp(&d) {
                Ordering::Greater => q += 1,
                Ordering::Equal if q & 1 == 1 => q += 1,
                _ => {}
            }
        }
        if q >> 64 != 0 {
            q >>= 1;
            e += 1;
        }
        Self {
            neg,
            m: q as u64,
            e,
        }
    }

    /// `fsqrt` of a positive value, rounded to the extended format.
    fn sqrt(a: Self) -> Self {
        debug_assert!(!a.neg || a.is_zero());
        if a.is_zero() {
            return a;
        }
        // M = m · 2^k with e − k even and 2^126 <= M < 2^128: its integer root has 64 bits.
        let k = if (a.e - 63).rem_euclid(2) == 0 {
            63
        } else {
            64
        };
        let big = u128::from(a.m) << k;
        let s0 = big.isqrt();
        let rem = big - s0 * s0;
        // √M ≥ s0 + ½ exactly when rem > s0 (√M is never a half-integer: no ties).
        let mut r = s0 + u128::from(rem > s0);
        let mut e = (a.e - k) / 2;
        if r >> 64 != 0 {
            r >>= 1;
            e += 1;
        }
        Self {
            neg: false,
            m: r as u64,
            e,
        }
    }

    /// `float64(longdouble)`: one rounding to 53 bits, to nearest with ties to even.
    fn to_f64(self) -> f64 {
        if self.is_zero() {
            return if self.neg { -0.0 } else { 0.0 };
        }
        let low = self.m & 0x7ff;
        let mut q = self.m >> 11;
        if low > 0x400 || (low == 0x400 && q & 1 == 1) {
            q += 1;
        }
        let mut e = self.e + 11;
        if q >> 53 != 0 {
            q >>= 1;
            e += 1;
        }
        let biased = e + 52 + 1023;
        let mag = if (1..2047).contains(&biased) {
            f64::from_bits(((biased as u64) << 52) | (q & ((1u64 << 52) - 1)))
        } else {
            // Outside the normal range (no header reaches it): the plain conversion.
            q as f64 * 2f64.powi(e)
        };
        if self.neg { -mag } else { mag }
    }
}

impl NiftiHeader {
    /// The qform affine nibabel 5.4.2's `get_qform` computes, bit for bit (the module's
    /// documentation), whatever `qform_code` says. `pixdim[0]` is taken as `qfac` when it is
    /// −1, else as 1, and the voxel sizes as they are: on a header after nibabel's load-time
    /// fixes ([`NiftiHeader::apply_load_fixes`]) these are nibabel's values.
    pub fn qform_nibabel(&self) -> Result<Affine, HeaderError> {
        let threshold = match self.version {
            NiftiVersion::V1 => 3.0 * f64::from(f32::EPSILON),
            NiftiVersion::V2 => 3.0 * f64::EPSILON,
        };
        let (x, y, z) = (
            X87::of(self.quatern_b),
            X87::of(self.quatern_c),
            X87::of(self.quatern_d),
        );
        // fillpositive: w2 = 1.0 - xyz @ xyz (matmul's longdouble loop from 0).
        let mut dot = X87::zero(false);
        for v in [x, y, z] {
            dot = X87::add(dot, X87::mul(v, v));
        }
        let w2 = X87::sub(X87::of(1.0), dot);
        let w = if w2.cmp_abs(X87::of(threshold)) == Ordering::Less {
            X87::zero(false)
        } else if w2.neg {
            return Err(HeaderError::NonUnitQuaternion(w2.to_f64()));
        } else {
            X87::sqrt(w2)
        };
        // quat2mat: Nq = w*w + x*x + y*y + z*z, left to right.
        let sq = |v: X87| X87::mul(v, v);
        let nq = X87::add(X87::add(X87::add(sq(w), sq(x)), sq(y)), sq(z));
        let one = X87::of(1.0);
        let r: [[X87; 3]; 3] = if nq.cmp_abs(X87::of(f64::EPSILON)) == Ordering::Less {
            let o = X87::zero(false);
            [[one, o, o], [o, one, o], [o, o, one]]
        } else {
            let s = X87::div(X87::of(2.0), nq);
            let (xs, ys, zs) = (X87::mul(x, s), X87::mul(y, s), X87::mul(z, s));
            let (wx, wy, wz) = (X87::mul(w, xs), X87::mul(w, ys), X87::mul(w, zs));
            let (xx, xy, xz) = (X87::mul(x, xs), X87::mul(x, ys), X87::mul(x, zs));
            let (yy, yz, zz) = (X87::mul(y, ys), X87::mul(y, zs), X87::mul(z, zs));
            [
                [
                    X87::sub(one, X87::add(yy, zz)),
                    X87::sub(xy, wz),
                    X87::add(xz, wy),
                ],
                [
                    X87::add(xy, wz),
                    X87::sub(one, X87::add(xx, zz)),
                    X87::sub(yz, wx),
                ],
                [
                    X87::sub(xz, wy),
                    X87::add(yz, wx),
                    X87::sub(one, X87::add(xx, yy)),
                ],
            ]
        };
        let qfac = if self.pixdim[0] == -1.0 { -1.0 } else { 1.0 };
        // vox[2] * qfac: a double times ±1, exact (the sign of zero included).
        let vox = [self.pixdim[1], self.pixdim[2], self.pixdim[3] * qfac].map(X87::of);
        // np.dot(R, diag(vox)) through LONGDOUBLE_dot: tmp = +0; tmp += R[i][k] · S[k][j].
        let mut linear: Mat3 = [[0.0; 3]; 3];
        for (i, row) in r.iter().enumerate() {
            for j in 0..3 {
                let mut tmp = X87::zero(false);
                for (k, &rik) in row.iter().enumerate() {
                    let skj = if k == j { vox[j] } else { X87::zero(false) };
                    tmp = X87::add(tmp, X87::mul(rik, skj));
                }
                linear[i][j] = tmp.to_f64();
            }
        }
        Ok(Affine::from_linear(
            linear,
            [self.qoffset_x, self.qoffset_y, self.qoffset_z],
        ))
    }

    /// The affine nibabel 5.4.2 gives an image of this header (`img.affine` after `nb.load`:
    /// `get_best_affine`), bit for bit: the sform if `sform_code != 0`, else the qform if
    /// `qform_code != 0` ([`NiftiHeader::qform_nibabel`]), else
    /// [`NiftiHeader::base_affine`].
    pub fn best_affine_nibabel(&self) -> Result<Affine, HeaderError> {
        if self.sform_code != 0 {
            Ok(self.sform())
        } else if self.qform_code != 0 {
            self.qform_nibabel()
        } else {
            self.base_affine()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value in exact rational form `num / 2^k` (test helper).
    fn exact(x: X87) -> (bool, u64, i32) {
        (x.neg, x.m, x.e)
    }

    #[test]
    fn arithmetic_rounds_to_64_bits() {
        // 1 + 2^-64: exactly half an ulp of 1 (2^-63): ties to even keeps 1.
        let one = X87::of(1.0);
        let tiny = X87::of(2f64.powi(-64));
        assert_eq!(exact(X87::add(one, tiny)), exact(one));
        // 1 + 3·2^-65 is above half an ulp: rounds up to 1 + 2^-63.
        let t = X87::of(3.0 * 2f64.powi(-65));
        let s = X87::add(one, t);
        assert_eq!(s.m, (1u64 << 63) + 1);
        // 1 − 2^-80: just below 1, rounds to 1 (the gap below 1 is 2^-64).
        let below = X87::sub(one, X87::of(2f64.powi(-80)));
        assert_eq!(exact(below), exact(one));
        // x − x is +0; −0 + −0 is −0.
        assert_eq!(exact(X87::sub(s, s)), (false, 0, 0));
        assert_eq!(exact(X87::add(X87::of(-0.0), X87::of(-0.0))), (true, 0, 0));
        // 1/3 in 64 bits, then to a double: the plain double division's value here.
        let third = X87::div(one, X87::of(3.0));
        assert_eq!(third.m, 0xAAAA_AAAA_AAAA_AAAB);
        assert_eq!(third.to_f64(), 1.0 / 3.0);
        // √2 rounded to 64 bits: 0xB504F333F9DE6484, then 0x597D...: rounded down.
        let r2 = X87::sqrt(X87::of(2.0));
        assert_eq!(r2.m, 0xB504_F333_F9DE_6484);
        assert_eq!(r2.to_f64(), std::f64::consts::SQRT_2);
        // Products: exact then rounded.
        let p = X87::mul(X87::of(1.0 + f64::EPSILON), X87::of(1.0 + f64::EPSILON));
        assert_eq!(p.to_f64(), 1.0 + 2.0 * f64::EPSILON);
    }

    #[test]
    fn zeros_of_the_qform_are_positive() {
        // An identity quaternion with qfac −1: plain float64 gives R[0][2]·(−vox) = −0;
        // nibabel's dot product from +0 gives +0.
        let mut h = NiftiHeader::new(
            NiftiVersion::V1,
            &[4, 4, 4],
            larmorx_core::element::DataType::F32,
        )
        .unwrap();
        h.pixdim[0] = -1.0;
        h.pixdim[1] = 3.0;
        h.pixdim[2] = 1.2000000476837158;
        h.pixdim[3] = 1.5;
        h.qform_code = 1;
        let a = *h.qform_nibabel().unwrap().rows();
        assert_eq!(a[0][2].to_bits(), 0.0f64.to_bits());
        assert_eq!(a[1][2].to_bits(), 0.0f64.to_bits());
        assert_eq!(a[2][2], -1.5);
        let plain = *h.qform().unwrap().rows();
        assert_eq!(plain[0][2].to_bits(), (-0.0f64).to_bits());
    }
}
