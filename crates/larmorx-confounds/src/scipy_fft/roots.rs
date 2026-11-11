// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The twiddle factors of SciPy's single-precision FFTs: pocketfft's `sincos_2pibyn<float>`
//! (SciPy 1.15, `scipy/_lib/pocketfft/pocketfft_hdronly.h`) and ducc0's `UnityRoots<float,
//! Cmplx<float>>` (SciPy 1.18.1, `subprojects/duccfft/ducc0/math/unity_roots.h`), which build
//! the table the same way.
//!
//! For a length `n` the table holds `exp(2 pi i k / n)`, `k = 0..n`, as two short tables of
//! double-precision roots: `v1[j] = exp(2 pi i j / n)` for `j <= mask` and `v2[j] = exp(2 pi i j
//! (mask + 1) / n)`, where `(mask + 1)^2 >= (n + 2) / 2`. Each root is computed in its first
//! octant (`calc`: the argument `x pi / (4 n)`, `x = 8 k` reduced into `[0, n]`, its sine and
//! cosine swapped and negated by octant), in double precision: the angle step `ang =
//! double(0.25L pi / n)` (x87's long double; for a power of two `n` exactly `fl(pi) / (4 n)`),
//! the argument `double(x) * ang` (one rounded product), then `sin` and `cos` of it. Entry `k`
//! is the double-precision product `v1[k & mask] * v2[k >> shift]` rounded to `float` (for `2 k
//! > n`, the conjugate of entry `n - k`). Both libraries are compiled for the x86-64 baseline
//! (no fused multiply-add: `objdump` of `pypocketfft` and `pyduccfft` shows no `vfmadd`), and
//! GCC merges each `std::cos(a)`, `std::sin(a)` pair into one call of glibc's `sincos(a)` (the
//! two extensions import `sincos`, never `sin` or `cos`).
//!
//! **The sine and cosine.** glibc 2.35's `sincos` is not correctly rounded (GLIBC-07), but on
//! these arguments it rounds to the same table entries as the correctly rounded functions of
//! `larmorx_core::math`: every table of every power of two up to 2^31 built both ways
//! (`examples/twiddle_survey.rs`: 234 of the 262,172 double-precision roots differ in their last
//! bit, none of the 2^32 - 1 `float` entries), and up to 2^22 in the test
//! `twiddles_agree_with_glibcs_sincos`. So the correctly rounded functions are used, in both
//! modes: no last bit of glibc's reaches an FFT.
//!
//! pocketfft and ducc0: Copyright (C) 2010-2023 Max-Planck-Society, Copyright (C) 2019 Peter
//! Bell; authors Martin Reinecke, Peter Bell. BSD-3-Clause (ducc0's files: "BSD-3-Clause OR
//! GPL-2.0-or-later", used here under BSD-3-Clause): redistribution and use in source and binary
//! forms, with or without modification, are permitted provided that the following conditions
//! are met: redistributions of source code must retain the above copyright notice, this list of
//! conditions and the following disclaimer; redistributions in binary form must reproduce the
//! above copyright notice, this list of conditions and the following disclaimer in the
//! documentation and/or other materials provided with the distribution; neither the name of the
//! copyright holder nor the names of its contributors may be used to endorse or promote products
//! derived from this software without specific prior written permission. THIS SOFTWARE IS
//! PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED
//! WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
//! FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
//! CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//! SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//! THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
//! OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.

use super::C32;
use larmorx_core::math;

/// The sine and cosine a table is built with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SinCos {
    /// `larmorx_core::math::sin` and `cos` (CORE-MATH, correctly rounded): the tables used.
    CorrectlyRounded,
    /// glibc 2.35's `sincos` (`larmorx_core::math::sincos_glibc`, the original's call): for the
    /// test that proves the two give the same tables.
    #[cfg_attr(not(test), allow(dead_code))]
    Glibc,
}

impl SinCos {
    #[inline]
    fn eval(self, a: f64) -> (f64, f64) {
        match self {
            SinCos::CorrectlyRounded => (math::sin(a), math::cos(a)),
            SinCos::Glibc => math::sincos_glibc(a),
        }
    }
}

/// The roots of unity of one length (the module's documentation).
#[derive(Clone, Debug)]
pub(crate) struct UnityRoots {
    n: usize,
    mask: usize,
    shift: u32,
    v1: Vec<(f64, f64)>,
    v2: Vec<(f64, f64)>,
}

/// `exp(2 pi i x / (8 n))` for `x = 8 k` as `calc` computes it: the octant's sine and cosine of
/// `double(x') * ang`, swapped and negated.
fn calc(x: usize, n: usize, ang: f64, sc: SinCos) -> (f64, f64) {
    let mut x = x << 3;
    if x < 4 * n {
        // first half
        if x < 2 * n {
            // first quadrant
            if x < n {
                let (s, c) = sc.eval(x as f64 * ang);
                return (c, s);
            }
            let (s, c) = sc.eval((2 * n - x) as f64 * ang);
            return (s, c);
        }
        // second quadrant
        x -= 2 * n;
        if x < n {
            let (s, c) = sc.eval(x as f64 * ang);
            return (-s, c);
        }
        let (s, c) = sc.eval((2 * n - x) as f64 * ang);
        return (-c, s);
    }
    x = 8 * n - x;
    if x < 2 * n {
        // third quadrant
        if x < n {
            let (s, c) = sc.eval(x as f64 * ang);
            return (c, -s);
        }
        let (s, c) = sc.eval((2 * n - x) as f64 * ang);
        return (s, -c);
    }
    // fourth quadrant
    x -= 2 * n;
    if x < n {
        let (s, c) = sc.eval(x as f64 * ang);
        return (-s, -c);
    }
    let (s, c) = sc.eval((2 * n - x) as f64 * ang);
    (-c, -s)
}

impl UnityRoots {
    /// The table of a power of two `n` with the correctly rounded sine and cosine.
    pub(crate) fn new(n: usize) -> UnityRoots {
        UnityRoots::with(n, SinCos::CorrectlyRounded)
    }

    pub(crate) fn with(n: usize, sc: SinCos) -> UnityRoots {
        assert!(
            n.is_power_of_two(),
            "the tables are built for powers of two"
        );
        // double(0.25L * pi / n): x87's pi (64 bits) over a power of two is exact, and rounds to
        // double as fl(pi) does (its 11 dropped bits are 0x235, below the half).
        let ang = std::f64::consts::PI * 0.25 / n as f64;
        let nval = (n + 2) / 2;
        let mut shift = 1u32;
        while (1usize << shift) * (1usize << shift) < nval {
            shift += 1;
        }
        let mask = (1usize << shift) - 1;
        let mut v1 = vec![(1.0, 0.0); mask + 1];
        for (i, v) in v1.iter_mut().enumerate().skip(1) {
            *v = calc(i, n, ang, sc);
        }
        let mut v2 = vec![(1.0, 0.0); (nval + mask) / (mask + 1)];
        for (i, v) in v2.iter_mut().enumerate().skip(1) {
            *v = calc(i * (mask + 1), n, ang, sc);
        }
        UnityRoots {
            n,
            mask,
            shift,
            v1,
            v2,
        }
    }

    /// The length the table was built for.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.n
    }

    /// Entry `idx` (`0 <= idx < n`): `operator[]`.
    #[inline]
    pub(crate) fn get(&self, idx: usize) -> C32 {
        if 2 * idx <= self.n {
            let (a, b) = (self.v1[idx & self.mask], self.v2[idx >> self.shift]);
            C32::new(
                (a.0 * b.0 - a.1 * b.1) as f32,
                (a.0 * b.1 + a.1 * b.0) as f32,
            )
        } else {
            let idx = self.n - idx;
            let (a, b) = (self.v1[idx & self.mask], self.v2[idx >> self.shift]);
            C32::new(
                (a.0 * b.0 - a.1 * b.1) as f32,
                -((a.0 * b.1 + a.1 * b.0) as f32),
            )
        }
    }
}

/// For the powers of two `2^0 ..= 2^max_log2`: how many double-precision roots (`v1`, `v2`) and
/// how many `float` entries differ between glibc's `sincos` and the correctly rounded functions
/// (`(lengths, roots compared, roots differing, entries compared, entries differing)`).
pub fn twiddle_survey(max_log2: u32) -> (usize, usize, usize, usize, usize) {
    let (mut roots, mut roots_diff, mut entries, mut entries_diff) = (0, 0, 0, 0);
    for k in 0..=max_log2 {
        let n = 1usize << k;
        let cr = UnityRoots::with(n, SinCos::CorrectlyRounded);
        let gl = UnityRoots::with(n, SinCos::Glibc);
        let bits = |v: &(f64, f64)| (v.0.to_bits(), v.1.to_bits());
        for (a, b) in cr.v1.iter().zip(&gl.v1).chain(cr.v2.iter().zip(&gl.v2)) {
            roots += 1;
            roots_diff += usize::from(bits(a) != bits(b));
        }
        if cr.v1.iter().zip(&gl.v1).any(|(a, b)| bits(a) != bits(b))
            || cr.v2.iter().zip(&gl.v2).any(|(a, b)| bits(a) != bits(b))
        {
            for idx in 0..n {
                let (a, b) = (cr.get(idx), gl.get(idx));
                entries_diff += usize::from(
                    a.re.to_bits() != b.re.to_bits() || a.im.to_bits() != b.im.to_bits(),
                );
            }
        }
        entries += n;
    }
    (
        max_log2 as usize + 1,
        roots,
        roots_diff,
        entries,
        entries_diff,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twiddles_agree_with_glibcs_sincos() {
        let (lengths, roots, _roots_diff, entries, entries_diff) = twiddle_survey(22);
        assert_eq!(lengths, 23);
        assert!(roots > 0 && entries > 0);
        assert_eq!(
            entries_diff, 0,
            "a float twiddle differs between glibc and CORE-MATH"
        );
    }

    #[test]
    fn small_tables() {
        let r = UnityRoots::new(8);
        assert_eq!(r.get(0), C32::new(1.0, 0.0));
        // exp(2 pi i / 4) = i: calc(2, 8) is x = 16 = 2 n, the second quadrant's -sin(0), cos(0).
        let q = r.get(2);
        assert_eq!((q.re.to_bits(), q.im), ((-0.0f32).to_bits(), 1.0));
        let h = r.get(4);
        assert_eq!((h.re, h.im), (-1.0, 0.0));
        let c = r.get(6);
        assert_eq!(c.im, -1.0);
    }
}
