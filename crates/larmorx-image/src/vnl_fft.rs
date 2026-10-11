// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's one-dimensional FFT (`vnl_fft_1d<double>`) for lengths that are powers of 2, bit
//! for bit.
//!
//! ITK's N4 sharpens its intensity histogram with `vnl_fft_1d<double>`
//! (`itkN4BiasFieldCorrectionImageFilter.hxx`). ITK 5.4.5 builds vnl without FFTW, so this is
//! VXL's own FFT: Clive Temperton's self-sorting, in-place generalized prime factor FFT
//! (GPFA), Fortran translated by f2c in VXL's `v3p/netlib/temperton`. N4 pads its histogram
//! to a power of 2, so only the radix-2 part (`dgpfa2f`: type I radix-4 passes, a central
//! radix-2 or radix-8 pass, type II radix-4 passes) is ported; other lengths are refused.
//!
//! - `fwd_transform` is the transform with `isign = +1` (the sum with `exp(+2πi·jk/n)`),
//!   `bwd_transform` with `isign = −1`; neither scales the result.
//! - The twiddle factors are `setdgpfa`'s: `cos(k·δ)`, `sin(k·δ)` with `δ = 2π/n` and
//!   `2π = 4·asin(1)`, from [`larmorx_core::math`], correctly rounded. glibc's `sincos` (what
//!   ITK's build calls) gives the same values for every length up to 512, N4's default
//!   (`docs/findings/ants-n4.md`).
//! - Every operation is in double precision, in the Fortran order; there is no fused
//!   multiply-add (gcc compiled the C without FMA for baseline x86-64).
//!
//! Ported from VXL as bundled with ITK v5.4.5 (BSD licence): `core/vnl/algo/vnl_fft_1d.h`,
//! `vnl_fft_base.hxx`, `vnl_fft_prime_factors.hxx`, `vnl_fft.cxx`, and from Temperton's GPFA as
//! distributed in VXL's `v3p/netlib/temperton/{setdgpfa,dgpfa,dgpfa2f}.{f,c}`.

// Original copyright and licence (BSD-3-Clause) notice of VXL
// (Modules/ThirdParty/VNL/src/vxl/core/vxl_copyright.h in ITK v5.4.5):
//
//   Copyright 2000-2013 VXL Contributors
//   All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions
//   are met:
//
//   * Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   * Neither the names of the copyright holders nor the names of their
//     contributors may be used to endorse or promote products derived
//     from this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
//   FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//   COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//   INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//   (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//   HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//   STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//   OF THE POSSIBILITY OF SUCH DAMAGE.
//
// The GPFA routines are C. Temperton's (A Generalized Prime Factor FFT Algorithm for any
// N = 2^p 3^q 5^r, SIAM J. Sci. Stat. Comput. 13 (1992) 676-686), distributed by VXL as above.

// The port keeps the Fortran's 1-based indices and variable names, so it can be checked line
// by line against dgpfa2f.f.
#![allow(clippy::too_many_arguments, clippy::many_single_char_names)]

use larmorx_core::math;

/// `vnl_fft_1d<double>` of a power-of-2 length.
#[derive(Clone, Debug)]
pub struct VnlFft1d {
    n: usize,
    /// `setdgpfa`'s twiddle factors, `2·n` values (cos, sin interleaved).
    trigs: Vec<f64>,
    /// The power of 2 (`npqr[0]`).
    ip: usize,
}

impl VnlFft1d {
    /// The transform of length `n`; `None` unless `n` is a power of 2 (at least 2).
    pub fn new(n: usize) -> Option<Self> {
        if n < 2 || !n.is_power_of_two() {
            return None;
        }
        let ip = n.trailing_zeros() as usize;
        Some(VnlFft1d {
            n,
            trigs: setdgpfa(n),
            ip,
        })
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `fwd_transform`: in place, `signal` as `n` complex values (real, imaginary
    /// interleaved), `isign = +1`.
    pub fn fwd_transform(&self, signal: &mut [f64]) {
        self.transform(signal, 1);
    }

    /// `bwd_transform`: `isign = −1`, not scaled.
    pub fn bwd_transform(&self, signal: &mut [f64]) {
        self.transform(signal, -1);
    }

    fn transform(&self, signal: &mut [f64], isign: i64) {
        assert_eq!(signal.len(), 2 * self.n, "signal length");
        // vnl_fft_base::transform for one dimension: A = data, B = data + 1, INC = 2,
        // JUMP = 0, LOT = 1.
        dgpfa2f(
            signal,
            &self.trigs,
            2,
            0,
            self.n as i64,
            self.ip as i64,
            1,
            isign,
        );
    }
}

/// `setdgpfa` for `n = 2^ip`: the rotated twiddle factors of the radix-2 part.
fn setdgpfa(n: usize) -> Vec<f64> {
    // TWOPI = 4.0 * ASIN(1.0): asin(1) is the double nearest pi/2, times 4 exactly.
    let twopi = std::f64::consts::FRAC_PI_2 * 4.0;
    let ni = n; // NJ(1) = 2**IP
    let del = twopi / ni as f64;
    let irot = n / ni;
    let kink = irot % ni;
    let mut trigs = Vec::with_capacity(2 * n);
    let mut kk = 0usize;
    for _ in 0..ni {
        let angle = kk as f64 * del;
        trigs.push(math::cos(angle));
        trigs.push(math::sin(angle));
        kk += kink;
        if kk > ni {
            kk -= ni;
        }
    }
    trigs
}

/// Fortran `DO var = start, end, step` with a positive step: the values `start`,
/// `start + step`, … up to `end`.
fn fortran_range(start: i64, end: i64, step: i64) -> impl Iterator<Item = i64> {
    debug_assert!(step > 0);
    let count = if end >= start {
        (end - start) / step + 1
    } else {
        0
    };
    (0..count).map(move |k| start + k * step)
}

/// `dgpfa2f`: the radix-2 section of the GPFA. `d` holds `a` and `b` interleaved as vnl
/// passes them (`b = a + 1`): `a(k)` is `d[k − 1]`, `b(k)` is `d[k]`. `trigs(k)` is
/// `trigs[k − 1]`.
fn dgpfa2f(
    d: &mut [f64],
    trigs: &[f64],
    inc: i64,
    jump: i64,
    n: i64,
    mm: i64,
    lot: i64,
    isign: i64,
) {
    macro_rules! a {
        ($k:expr) => {
            d[($k - 1) as usize]
        };
    }
    macro_rules! b {
        ($k:expr) => {
            d[($k) as usize]
        };
    }
    macro_rules! tr {
        ($k:expr) => {
            trigs[($k - 1) as usize]
        };
    }
    let lvr: i64 = 1024;
    let n2 = 1i64 << mm;
    let inq = n / n2;
    let jstepx = (n2 - n) * inc;
    let ninc = n * inc;
    let ink = inc * inq;

    let mut m2 = 0;
    let mut m8 = 0;
    let m;
    if mm % 2 == 0 {
        m = mm / 2;
    } else if mm % 4 == 1 {
        m = (mm - 1) / 2;
        m2 = 1;
    } else {
        // mm % 4 == 3
        m = (mm - 3) / 2;
        m8 = 1;
    }
    let mh = (m + 1) / 2;

    let nblox = 1 + (lot - 1) / lvr;
    let mut left = lot;
    let s = isign as f64;
    let mut istart: i64 = 1;

    for _nb in 1..=nblox {
        let mut nvex;
        if left <= lvr {
            nvex = left;
        } else if left < 2 * lvr {
            nvex = left / 2;
            nvex += nvex % 2;
        } else {
            nvex = lvr;
        }
        left -= nvex;

        let mut la: i64 = 1;

        // Type I radix-4 passes.
        let mut mu = inq % 4;
        if isign == -1 {
            mu = 4 - mu;
        }
        let mut ss = 1.0f64;
        if mu == 3 {
            ss = -1.0;
        }

        'radix2: {
            // `if (mh.eq.0) go to 200`: no type I pass (the loop below runs zero times).
            for _ipass in 1..=mh {
                let jstep = (n * inc) / (4 * la);
                let jstepl = jstep - ninc;

                // k = 0 loop (no twiddle factors).
                for jjj in fortran_range(0, (n - 1) * inc, 4 * jstep) {
                    let mut ja = istart + jjj;
                    for _nu in 1..=inq {
                        let mut jb = ja + jstepl;
                        if jb < istart {
                            jb += ninc;
                        }
                        let mut jc = jb + jstepl;
                        if jc < istart {
                            jc += ninc;
                        }
                        let mut jd = jc + jstepl;
                        if jd < istart {
                            jd += ninc;
                        }
                        let mut j = 0;
                        for _l in 1..=nvex {
                            let aja = a!(ja + j);
                            let ajc = a!(jc + j);
                            let t0 = aja + ajc;
                            let t2 = aja - ajc;
                            let ajb = a!(jb + j);
                            let ajd = a!(jd + j);
                            let t1 = ajb + ajd;
                            let t3 = ss * (ajb - ajd);
                            let bja = b!(ja + j);
                            let bjc = b!(jc + j);
                            let u0 = bja + bjc;
                            let u2 = bja - bjc;
                            let bjb = b!(jb + j);
                            let bjd = b!(jd + j);
                            let u1 = bjb + bjd;
                            let u3 = ss * (bjb - bjd);
                            a!(ja + j) = t0 + t1;
                            a!(jc + j) = t0 - t1;
                            b!(ja + j) = u0 + u1;
                            b!(jc + j) = u0 - u1;
                            a!(jb + j) = t2 - u3;
                            a!(jd + j) = t2 + u3;
                            b!(jb + j) = u2 + t3;
                            b!(jd + j) = u2 - t3;
                            j += jump;
                        }
                        ja += jstepx;
                        if ja < istart {
                            ja += ninc;
                        }
                    }
                }

                // Finished if n2 = 4.
                if n2 == 4 {
                    break 'radix2;
                }
                let mut kk = 2 * la;

                // Nonzero k.
                for k in fortran_range(ink, jstep - ink, ink) {
                    let co1 = tr!(kk + 1);
                    let si1 = s * tr!(kk + 2);
                    let co2 = tr!(2 * kk + 1);
                    let si2 = s * tr!(2 * kk + 2);
                    let co3 = tr!(3 * kk + 1);
                    let si3 = s * tr!(3 * kk + 2);
                    for jjj in fortran_range(k, (n - 1) * inc, 4 * jstep) {
                        let mut ja = istart + jjj;
                        for _nu in 1..=inq {
                            let mut jb = ja + jstepl;
                            if jb < istart {
                                jb += ninc;
                            }
                            let mut jc = jb + jstepl;
                            if jc < istart {
                                jc += ninc;
                            }
                            let mut jd = jc + jstepl;
                            if jd < istart {
                                jd += ninc;
                            }
                            let mut j = 0;
                            for _l in 1..=nvex {
                                let aja = a!(ja + j);
                                let ajc = a!(jc + j);
                                let t0 = aja + ajc;
                                let t2 = aja - ajc;
                                let ajb = a!(jb + j);
                                let ajd = a!(jd + j);
                                let t1 = ajb + ajd;
                                let t3 = ss * (ajb - ajd);
                                let bja = b!(ja + j);
                                let bjc = b!(jc + j);
                                let u0 = bja + bjc;
                                let u2 = bja - bjc;
                                let bjb = b!(jb + j);
                                let bjd = b!(jd + j);
                                let u1 = bjb + bjd;
                                let u3 = ss * (bjb - bjd);
                                a!(ja + j) = t0 + t1;
                                b!(ja + j) = u0 + u1;
                                a!(jb + j) = co1 * (t2 - u3) - si1 * (u2 + t3);
                                b!(jb + j) = si1 * (t2 - u3) + co1 * (u2 + t3);
                                a!(jc + j) = co2 * (t0 - t1) - si2 * (u0 - u1);
                                b!(jc + j) = si2 * (t0 - t1) + co2 * (u0 - u1);
                                a!(jd + j) = co3 * (t2 + u3) - si3 * (u2 - t3);
                                b!(jd + j) = si3 * (t2 + u3) + co3 * (u2 - t3);
                                j += jump;
                            }
                            ja += jstepx;
                            if ja < istart {
                                ja += ninc;
                            }
                        }
                    }
                    kk += 2 * la;
                }
                la *= 4;
            }

            // Central radix-2 pass (label 200).
            if m2 != 0 {
                let jstep = (n * inc) / (2 * la);
                let jstepl = jstep - ninc;
                for jjj in fortran_range(0, (n - 1) * inc, 2 * jstep) {
                    let mut ja = istart + jjj;
                    for _nu in 1..=inq {
                        let mut jb = ja + jstepl;
                        if jb < istart {
                            jb += ninc;
                        }
                        let mut j = 0;
                        for _l in 1..=nvex {
                            let aja = a!(ja + j);
                            let ajb = a!(jb + j);
                            let t0 = aja - ajb;
                            a!(ja + j) = aja + ajb;
                            a!(jb + j) = t0;
                            let bja = b!(ja + j);
                            let bjb = b!(jb + j);
                            let u0 = bja - bjb;
                            b!(ja + j) = bja + bjb;
                            b!(jb + j) = u0;
                            j += jump;
                        }
                        ja += jstepx;
                        if ja < istart {
                            ja += ninc;
                        }
                    }
                }
                // Finished if n2 = 2.
                if n2 == 2 {
                    break 'radix2;
                }
                let mut kk = 2 * la;
                for k in fortran_range(ink, jstep - ink, ink) {
                    let co1 = tr!(kk + 1);
                    let si1 = s * tr!(kk + 2);
                    for jjj in fortran_range(k, (n - 1) * inc, 2 * jstep) {
                        let mut ja = istart + jjj;
                        for _nu in 1..=inq {
                            let mut jb = ja + jstepl;
                            if jb < istart {
                                jb += ninc;
                            }
                            let mut j = 0;
                            if kk == n2 / 2 {
                                for _l in 1..=nvex {
                                    let aja = a!(ja + j);
                                    let ajb = a!(jb + j);
                                    let t0 = ss * (aja - ajb);
                                    a!(ja + j) = aja + ajb;
                                    let bjb = b!(jb + j);
                                    let bja = b!(ja + j);
                                    a!(jb + j) = ss * (bjb - bja);
                                    b!(ja + j) = bja + bjb;
                                    b!(jb + j) = t0;
                                    j += jump;
                                }
                            } else {
                                for _l in 1..=nvex {
                                    let aja = a!(ja + j);
                                    let ajb = a!(jb + j);
                                    let t0 = aja - ajb;
                                    a!(ja + j) = aja + ajb;
                                    let bja = b!(ja + j);
                                    let bjb = b!(jb + j);
                                    let u0 = bja - bjb;
                                    b!(ja + j) = bja + bjb;
                                    a!(jb + j) = co1 * t0 - si1 * u0;
                                    b!(jb + j) = si1 * t0 + co1 * u0;
                                    j += jump;
                                }
                            }
                            ja += jstepx;
                            if ja < istart {
                                ja += ninc;
                            }
                        }
                    }
                    kk += 2 * la;
                }
                la *= 2;
            } else if m8 != 0 {
                // Central radix-8 pass (label 300).
                let jstep = (n * inc) / (8 * la);
                let jstepl = jstep - ninc;
                let mut mu = inq % 8;
                if isign == -1 {
                    mu = 8 - mu;
                }
                let mut c1 = 1.0f64;
                if mu == 3 || mu == 7 {
                    c1 = -1.0;
                }
                // c2 = sqrt((float).5): the double square root of 0.5.
                let mut c2 = 0.5f64.sqrt();
                if mu == 3 || mu == 5 {
                    c2 = -c2;
                }
                let c3 = c1 * c2;

                // Stage 1.
                for k in fortran_range(0, jstep - ink, ink) {
                    for jjj in fortran_range(k, (n - 1) * inc, 8 * jstep) {
                        let mut ja = istart + jjj;
                        for _nu in 1..=inq {
                            let [jb, jc, jd, je, jf, jg, jh] = chain7(ja, jstepl, istart, ninc);
                            let mut j = 0;
                            for _l in 1..=nvex {
                                let aja = a!(ja + j);
                                let aje = a!(je + j);
                                let t0 = aja - aje;
                                a!(ja + j) = aja + aje;
                                let ajc = a!(jc + j);
                                let ajg = a!(jg + j);
                                let t1 = c1 * (ajc - ajg);
                                a!(je + j) = ajc + ajg;
                                let ajb = a!(jb + j);
                                let ajf = a!(jf + j);
                                let t2 = ajb - ajf;
                                a!(jc + j) = ajb + ajf;
                                let ajd = a!(jd + j);
                                let ajh = a!(jh + j);
                                let t3 = ajd - ajh;
                                a!(jg + j) = ajd + ajh;
                                a!(jb + j) = t0;
                                a!(jf + j) = t1;
                                a!(jd + j) = c2 * (t2 - t3);
                                a!(jh + j) = c3 * (t2 + t3);
                                let bja = b!(ja + j);
                                let bje = b!(je + j);
                                let u0 = bja - bje;
                                b!(ja + j) = bja + bje;
                                let bjc = b!(jc + j);
                                let bjg = b!(jg + j);
                                let u1 = c1 * (bjc - bjg);
                                b!(je + j) = bjc + bjg;
                                let bjb = b!(jb + j);
                                let bjf = b!(jf + j);
                                let u2 = bjb - bjf;
                                b!(jc + j) = bjb + bjf;
                                let bjd = b!(jd + j);
                                let bjh = b!(jh + j);
                                let u3 = bjd - bjh;
                                b!(jg + j) = bjd + bjh;
                                b!(jb + j) = u0;
                                b!(jf + j) = u1;
                                b!(jd + j) = c2 * (u2 - u3);
                                b!(jh + j) = c3 * (u2 + u3);
                                j += jump;
                            }
                            ja += jstepx;
                            if ja < istart {
                                ja += ninc;
                            }
                        }
                    }
                }

                // Stage 2, k = 0 (no twiddle factors).
                for jjj in fortran_range(0, (n - 1) * inc, 8 * jstep) {
                    let mut ja = istart + jjj;
                    for _nu in 1..=inq {
                        let [jb, jc, jd, je, jf, jg, jh] = chain7(ja, jstepl, istart, ninc);
                        let mut j = 0;
                        for _l in 1..=nvex {
                            let aja = a!(ja + j);
                            let aje = a!(je + j);
                            let t0 = aja + aje;
                            let t2 = aja - aje;
                            let ajc = a!(jc + j);
                            let ajg = a!(jg + j);
                            let t1 = ajc + ajg;
                            let t3 = c1 * (ajc - ajg);
                            let bja = b!(ja + j);
                            let bje = b!(je + j);
                            let u0 = bja + bje;
                            let u2 = bja - bje;
                            let bjc = b!(jc + j);
                            let bjg = b!(jg + j);
                            let u1 = bjc + bjg;
                            let u3 = c1 * (bjc - bjg);
                            a!(ja + j) = t0 + t1;
                            a!(je + j) = t0 - t1;
                            b!(ja + j) = u0 + u1;
                            b!(je + j) = u0 - u1;
                            a!(jc + j) = t2 - u3;
                            a!(jg + j) = t2 + u3;
                            b!(jc + j) = u2 + t3;
                            b!(jg + j) = u2 - t3;
                            let ajb = a!(jb + j);
                            let ajd = a!(jd + j);
                            let t0 = ajb + ajd;
                            let t2 = ajb - ajd;
                            let ajf = a!(jf + j);
                            let ajh = a!(jh + j);
                            let t1 = ajf - ajh;
                            let t3 = ajf + ajh;
                            let bjb = b!(jb + j);
                            let bjd = b!(jd + j);
                            let u0 = bjb + bjd;
                            let u2 = bjb - bjd;
                            let bjf = b!(jf + j);
                            let bjh = b!(jh + j);
                            let u1 = bjf - bjh;
                            let u3 = bjf + bjh;
                            a!(jb + j) = t0 - u3;
                            a!(jh + j) = t0 + u3;
                            b!(jb + j) = u0 + t3;
                            b!(jh + j) = u0 - t3;
                            a!(jd + j) = t2 + u1;
                            a!(jf + j) = t2 - u1;
                            b!(jd + j) = u2 - t1;
                            b!(jf + j) = u2 + t1;
                            j += jump;
                        }
                        ja += jstepx;
                        if ja < istart {
                            ja += ninc;
                        }
                    }
                }

                if n2 == 8 {
                    break 'radix2;
                }

                // Nonzero k.
                let mut kk = 2 * la;
                for k in fortran_range(ink, jstep - ink, ink) {
                    let co1 = tr!(kk + 1);
                    let si1 = s * tr!(kk + 2);
                    let co2 = tr!(2 * kk + 1);
                    let si2 = s * tr!(2 * kk + 2);
                    let co3 = tr!(3 * kk + 1);
                    let si3 = s * tr!(3 * kk + 2);
                    let co4 = tr!(4 * kk + 1);
                    let si4 = s * tr!(4 * kk + 2);
                    let co5 = tr!(5 * kk + 1);
                    let si5 = s * tr!(5 * kk + 2);
                    let co6 = tr!(6 * kk + 1);
                    let si6 = s * tr!(6 * kk + 2);
                    let co7 = tr!(7 * kk + 1);
                    let si7 = s * tr!(7 * kk + 2);
                    for jjj in fortran_range(k, (n - 1) * inc, 8 * jstep) {
                        let mut ja = istart + jjj;
                        for _nu in 1..=inq {
                            let [jb, jc, jd, je, jf, jg, jh] = chain7(ja, jstepl, istart, ninc);
                            let mut j = 0;
                            for _l in 1..=nvex {
                                let aja = a!(ja + j);
                                let aje = a!(je + j);
                                let t0 = aja + aje;
                                let t2 = aja - aje;
                                let ajc = a!(jc + j);
                                let ajg = a!(jg + j);
                                let t1 = ajc + ajg;
                                let t3 = c1 * (ajc - ajg);
                                let bja = b!(ja + j);
                                let bje = b!(je + j);
                                let u0 = bja + bje;
                                let u2 = bja - bje;
                                let bjc = b!(jc + j);
                                let bjg = b!(jg + j);
                                let u1 = bjc + bjg;
                                let u3 = c1 * (bjc - bjg);
                                a!(ja + j) = t0 + t1;
                                b!(ja + j) = u0 + u1;
                                a!(je + j) = co4 * (t0 - t1) - si4 * (u0 - u1);
                                b!(je + j) = si4 * (t0 - t1) + co4 * (u0 - u1);
                                a!(jc + j) = co2 * (t2 - u3) - si2 * (u2 + t3);
                                b!(jc + j) = si2 * (t2 - u3) + co2 * (u2 + t3);
                                a!(jg + j) = co6 * (t2 + u3) - si6 * (u2 - t3);
                                b!(jg + j) = si6 * (t2 + u3) + co6 * (u2 - t3);
                                let ajb = a!(jb + j);
                                let ajd = a!(jd + j);
                                let t0 = ajb + ajd;
                                let t2 = ajb - ajd;
                                let ajf = a!(jf + j);
                                let ajh = a!(jh + j);
                                let t1 = ajf - ajh;
                                let t3 = ajf + ajh;
                                let bjb = b!(jb + j);
                                let bjd = b!(jd + j);
                                let u0 = bjb + bjd;
                                let u2 = bjb - bjd;
                                let bjf = b!(jf + j);
                                let bjh = b!(jh + j);
                                let u1 = bjf - bjh;
                                let u3 = bjf + bjh;
                                a!(jb + j) = co1 * (t0 - u3) - si1 * (u0 + t3);
                                b!(jb + j) = si1 * (t0 - u3) + co1 * (u0 + t3);
                                a!(jh + j) = co7 * (t0 + u3) - si7 * (u0 - t3);
                                b!(jh + j) = si7 * (t0 + u3) + co7 * (u0 - t3);
                                a!(jd + j) = co3 * (t2 + u1) - si3 * (u2 - t1);
                                b!(jd + j) = si3 * (t2 + u1) + co3 * (u2 - t1);
                                a!(jf + j) = co5 * (t2 - u1) - si5 * (u2 + t1);
                                b!(jf + j) = si5 * (t2 - u1) + co5 * (u2 + t1);
                                j += jump;
                            }
                            ja += jstepx;
                            if ja < istart {
                                ja += ninc;
                            }
                        }
                    }
                    kk += 2 * la;
                }
                la *= 8;
            }

            // Type II radix-4 passes (label 400).
            let mut mu = inq % 4;
            if isign == -1 {
                mu = 4 - mu;
            }
            let mut ss = 1.0f64;
            if mu == 3 {
                ss = -1.0;
            }
            for ipass in (mh + 1)..=m {
                let jstep = (n * inc) / (4 * la);
                let jstepl = jstep - ninc;
                let laincl = la * ink - ninc;

                // k = 0 loop (no twiddle factors).
                for ll in fortran_range(0, (la - 1) * ink, 4 * jstep) {
                    for jjj in fortran_range(ll, (n - 1) * inc, 4 * la * ink) {
                        let mut ja = istart + jjj;
                        for _nu in 1..=inq {
                            let x = chain16(ja, jstepl, laincl, istart, ninc);
                            let [jb, jc, jd, je, jf, jg, jh, ji, jj, jk, jl, jm, jn, jo, jp] = x;
                            let mut j = 0;
                            for _l in 1..=nvex {
                                let aja = a!(ja + j);
                                let mut ajc = a!(jc + j);
                                let mut t0 = aja + ajc;
                                let mut t2 = aja - ajc;
                                let mut ajb = a!(jb + j);
                                let mut ajd = a!(jd + j);
                                let mut t1 = ajb + ajd;
                                let mut t3 = ss * (ajb - ajd);
                                let aji = a!(ji + j);
                                ajc = aji;
                                let bja = b!(ja + j);
                                let mut bjc = b!(jc + j);
                                let mut u0 = bja + bjc;
                                let mut u2 = bja - bjc;
                                let mut bjb = b!(jb + j);
                                let mut bjd = b!(jd + j);
                                let mut u1 = bjb + bjd;
                                let mut u3 = ss * (bjb - bjd);
                                let aje = a!(je + j);
                                ajb = aje;
                                a!(ja + j) = t0 + t1;
                                a!(ji + j) = t0 - t1;
                                b!(ja + j) = u0 + u1;
                                bjc = u0 - u1;
                                let bjm = b!(jm + j);
                                bjd = bjm;
                                a!(je + j) = t2 - u3;
                                ajd = t2 + u3;
                                bjb = u2 + t3;
                                b!(jm + j) = u2 - t3;
                                // ----------------------
                                let mut ajg = a!(jg + j);
                                t0 = ajb + ajg;
                                t2 = ajb - ajg;
                                let ajf = a!(jf + j);
                                let mut ajh = a!(jh + j);
                                t1 = ajf + ajh;
                                t3 = ss * (ajf - ajh);
                                let ajj = a!(jj + j);
                                ajg = ajj;
                                let bje = b!(je + j);
                                let mut bjg = b!(jg + j);
                                u0 = bje + bjg;
                                u2 = bje - bjg;
                                let bjf = b!(jf + j);
                                let mut bjh = b!(jh + j);
                                u1 = bjf + bjh;
                                u3 = ss * (bjf - bjh);
                                b!(je + j) = bjb;
                                a!(jb + j) = t0 + t1;
                                a!(jj + j) = t0 - t1;
                                let bjj = b!(jj + j);
                                bjg = bjj;
                                b!(jb + j) = u0 + u1;
                                b!(jj + j) = u0 - u1;
                                a!(jf + j) = t2 - u3;
                                ajh = t2 + u3;
                                b!(jf + j) = u2 + t3;
                                bjh = u2 - t3;
                                // ----------------------
                                let ajk = a!(jk + j);
                                t0 = ajc + ajk;
                                t2 = ajc - ajk;
                                let mut ajl = a!(jl + j);
                                t1 = ajg + ajl;
                                t3 = ss * (ajg - ajl);
                                let bji = b!(ji + j);
                                let bjk = b!(jk + j);
                                u0 = bji + bjk;
                                u2 = bji - bjk;
                                let ajo = a!(jo + j);
                                ajl = ajo;
                                let mut bjl = b!(jl + j);
                                u1 = bjg + bjl;
                                u3 = ss * (bjg - bjl);
                                b!(ji + j) = bjc;
                                a!(jc + j) = t0 + t1;
                                a!(jk + j) = t0 - t1;
                                let bjo = b!(jo + j);
                                bjl = bjo;
                                b!(jc + j) = u0 + u1;
                                b!(jk + j) = u0 - u1;
                                a!(jg + j) = t2 - u3;
                                a!(jo + j) = t2 + u3;
                                b!(jg + j) = u2 + t3;
                                b!(jo + j) = u2 - t3;
                                // ----------------------
                                let ajm = a!(jm + j);
                                t0 = ajm + ajl;
                                t2 = ajm - ajl;
                                let ajn = a!(jn + j);
                                let ajp = a!(jp + j);
                                t1 = ajn + ajp;
                                t3 = ss * (ajn - ajp);
                                a!(jm + j) = ajd;
                                u0 = bjd + bjl;
                                u2 = bjd - bjl;
                                let bjn = b!(jn + j);
                                let bjp = b!(jp + j);
                                u1 = bjn + bjp;
                                u3 = ss * (bjn - bjp);
                                a!(jn + j) = ajh;
                                a!(jd + j) = t0 + t1;
                                a!(jl + j) = t0 - t1;
                                b!(jd + j) = u0 + u1;
                                b!(jl + j) = u0 - u1;
                                b!(jn + j) = bjh;
                                a!(jh + j) = t2 - u3;
                                a!(jp + j) = t2 + u3;
                                b!(jh + j) = u2 + t3;
                                b!(jp + j) = u2 - t3;
                                j += jump;
                            }
                            ja += jstepx;
                            if ja < istart {
                                ja += ninc;
                            }
                        }
                    }
                }

                // Finished if last pass.
                if ipass == m {
                    break 'radix2;
                }
                let mut kk = 2 * la;

                // Nonzero k.
                for k in fortran_range(ink, jstep - ink, ink) {
                    let co1 = tr!(kk + 1);
                    let si1 = s * tr!(kk + 2);
                    let co2 = tr!(2 * kk + 1);
                    let si2 = s * tr!(2 * kk + 2);
                    let co3 = tr!(3 * kk + 1);
                    let si3 = s * tr!(3 * kk + 2);
                    for ll in fortran_range(k, (la - 1) * ink, 4 * jstep) {
                        for jjj in fortran_range(ll, (n - 1) * inc, 4 * la * ink) {
                            let mut ja = istart + jjj;
                            for _nu in 1..=inq {
                                let x = chain16(ja, jstepl, laincl, istart, ninc);
                                let [jb, jc, jd, je, jf, jg, jh, ji, jj, jk, jl, jm, jn, jo, jp] =
                                    x;
                                let mut j = 0;
                                for _l in 1..=nvex {
                                    let aja = a!(ja + j);
                                    let mut ajc = a!(jc + j);
                                    let mut t0 = aja + ajc;
                                    let mut t2 = aja - ajc;
                                    let mut ajb = a!(jb + j);
                                    let mut ajd = a!(jd + j);
                                    let mut t1 = ajb + ajd;
                                    let mut t3 = ss * (ajb - ajd);
                                    let aji = a!(ji + j);
                                    ajc = aji;
                                    let bja = b!(ja + j);
                                    let mut bjc = b!(jc + j);
                                    let mut u0 = bja + bjc;
                                    let mut u2 = bja - bjc;
                                    let mut bjb = b!(jb + j);
                                    let mut bjd = b!(jd + j);
                                    let mut u1 = bjb + bjd;
                                    let mut u3 = ss * (bjb - bjd);
                                    let aje = a!(je + j);
                                    ajb = aje;
                                    a!(ja + j) = t0 + t1;
                                    b!(ja + j) = u0 + u1;
                                    a!(je + j) = co1 * (t2 - u3) - si1 * (u2 + t3);
                                    bjb = si1 * (t2 - u3) + co1 * (u2 + t3);
                                    let bjm = b!(jm + j);
                                    bjd = bjm;
                                    a!(ji + j) = co2 * (t0 - t1) - si2 * (u0 - u1);
                                    bjc = si2 * (t0 - t1) + co2 * (u0 - u1);
                                    ajd = co3 * (t2 + u3) - si3 * (u2 - t3);
                                    b!(jm + j) = si3 * (t2 + u3) + co3 * (u2 - t3);
                                    // ----------------------------------------
                                    let mut ajg = a!(jg + j);
                                    t0 = ajb + ajg;
                                    t2 = ajb - ajg;
                                    let ajf = a!(jf + j);
                                    let mut ajh = a!(jh + j);
                                    t1 = ajf + ajh;
                                    t3 = ss * (ajf - ajh);
                                    let ajj = a!(jj + j);
                                    ajg = ajj;
                                    let bje = b!(je + j);
                                    let mut bjg = b!(jg + j);
                                    u0 = bje + bjg;
                                    u2 = bje - bjg;
                                    let bjf = b!(jf + j);
                                    let mut bjh = b!(jh + j);
                                    u1 = bjf + bjh;
                                    u3 = ss * (bjf - bjh);
                                    b!(je + j) = bjb;
                                    a!(jb + j) = t0 + t1;
                                    b!(jb + j) = u0 + u1;
                                    let bjj = b!(jj + j);
                                    bjg = bjj;
                                    a!(jf + j) = co1 * (t2 - u3) - si1 * (u2 + t3);
                                    b!(jf + j) = si1 * (t2 - u3) + co1 * (u2 + t3);
                                    a!(jj + j) = co2 * (t0 - t1) - si2 * (u0 - u1);
                                    b!(jj + j) = si2 * (t0 - t1) + co2 * (u0 - u1);
                                    ajh = co3 * (t2 + u3) - si3 * (u2 - t3);
                                    bjh = si3 * (t2 + u3) + co3 * (u2 - t3);
                                    // ----------------------------------------
                                    let ajk = a!(jk + j);
                                    t0 = ajc + ajk;
                                    t2 = ajc - ajk;
                                    let mut ajl = a!(jl + j);
                                    t1 = ajg + ajl;
                                    t3 = ss * (ajg - ajl);
                                    let bji = b!(ji + j);
                                    let bjk = b!(jk + j);
                                    u0 = bji + bjk;
                                    u2 = bji - bjk;
                                    let ajo = a!(jo + j);
                                    ajl = ajo;
                                    let mut bjl = b!(jl + j);
                                    u1 = bjg + bjl;
                                    u3 = ss * (bjg - bjl);
                                    b!(ji + j) = bjc;
                                    a!(jc + j) = t0 + t1;
                                    b!(jc + j) = u0 + u1;
                                    let bjo = b!(jo + j);
                                    bjl = bjo;
                                    a!(jg + j) = co1 * (t2 - u3) - si1 * (u2 + t3);
                                    b!(jg + j) = si1 * (t2 - u3) + co1 * (u2 + t3);
                                    a!(jk + j) = co2 * (t0 - t1) - si2 * (u0 - u1);
                                    b!(jk + j) = si2 * (t0 - t1) + co2 * (u0 - u1);
                                    a!(jo + j) = co3 * (t2 + u3) - si3 * (u2 - t3);
                                    b!(jo + j) = si3 * (t2 + u3) + co3 * (u2 - t3);
                                    // ----------------------------------------
                                    let ajm = a!(jm + j);
                                    t0 = ajm + ajl;
                                    t2 = ajm - ajl;
                                    let ajn = a!(jn + j);
                                    let ajp = a!(jp + j);
                                    t1 = ajn + ajp;
                                    t3 = ss * (ajn - ajp);
                                    a!(jm + j) = ajd;
                                    u0 = bjd + bjl;
                                    u2 = bjd - bjl;
                                    a!(jn + j) = ajh;
                                    let bjn = b!(jn + j);
                                    let bjp = b!(jp + j);
                                    u1 = bjn + bjp;
                                    u3 = ss * (bjn - bjp);
                                    b!(jn + j) = bjh;
                                    a!(jd + j) = t0 + t1;
                                    b!(jd + j) = u0 + u1;
                                    a!(jh + j) = co1 * (t2 - u3) - si1 * (u2 + t3);
                                    b!(jh + j) = si1 * (t2 - u3) + co1 * (u2 + t3);
                                    a!(jl + j) = co2 * (t0 - t1) - si2 * (u0 - u1);
                                    b!(jl + j) = si2 * (t0 - t1) + co2 * (u0 - u1);
                                    a!(jp + j) = co3 * (t2 + u3) - si3 * (u2 - t3);
                                    b!(jp + j) = si3 * (t2 + u3) + co3 * (u2 - t3);
                                    j += jump;
                                }
                                ja += jstepx;
                                if ja < istart {
                                    ja += ninc;
                                }
                            }
                        }
                    }
                    kk += 2 * la;
                }
                la *= 4;
            }
        }
        istart += nvex * jump;
    }
}

/// `jb, jc, …, jh`: each the previous plus `jstepl`, wrapped by `ninc` below `istart`.
fn chain7(ja: i64, jstepl: i64, istart: i64, ninc: i64) -> [i64; 7] {
    let mut out = [0i64; 7];
    let mut prev = ja;
    for o in &mut out {
        let mut v = prev + jstepl;
        if v < istart {
            v += ninc;
        }
        *o = v;
        prev = v;
    }
    out
}

/// The 15 indices of a type II radix-4 butterfly group: `jb, jc, jd` from `ja`, `je` from `ja`
/// by `laincl` and `jf, jg, jh` from it, `ji` from `je`, …, as in labels 415 and 445.
fn chain16(ja: i64, jstepl: i64, laincl: i64, istart: i64, ninc: i64) -> [i64; 15] {
    let wrap = |v: i64| if v < istart { v + ninc } else { v };
    let [jb, jc, jd] = {
        let jb = wrap(ja + jstepl);
        let jc = wrap(jb + jstepl);
        let jd = wrap(jc + jstepl);
        [jb, jc, jd]
    };
    let je = wrap(ja + laincl);
    let jf = wrap(je + jstepl);
    let jg = wrap(jf + jstepl);
    let jh = wrap(jg + jstepl);
    let ji = wrap(je + laincl);
    let jj = wrap(ji + jstepl);
    let jk = wrap(jj + jstepl);
    let jl = wrap(jk + jstepl);
    let jm = wrap(ji + laincl);
    let jn = wrap(jm + jstepl);
    let jo = wrap(jn + jstepl);
    let jp = wrap(jo + jstepl);
    [jb, jc, jd, je, jf, jg, jh, ji, jj, jk, jl, jm, jn, jo, jp]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plain DFT with the same sign convention (`isign = +1`: `exp(+2πi·jk/n)`).
    fn dft(x: &[f64], isign: f64) -> Vec<f64> {
        let n = x.len() / 2;
        let mut out = vec![0.0; 2 * n];
        for k in 0..n {
            let (mut re, mut im) = (0.0, 0.0);
            for j in 0..n {
                let angle = isign * 2.0 * std::f64::consts::PI * (j * k % n) as f64 / n as f64;
                let (s, c) = angle.sin_cos();
                re += x[2 * j] * c - x[2 * j + 1] * s;
                im += x[2 * j] * s + x[2 * j + 1] * c;
            }
            out[2 * k] = re;
            out[2 * k + 1] = im;
        }
        out
    }

    #[test]
    fn matches_the_dft_for_every_pass_type() {
        // 2^1 … 2^11: central radix-2 (odd mm, mm % 4 == 1), radix-8 (mm % 4 == 3), and
        // radix-4 only (even mm), with type I and type II passes.
        for mm in 1..=11 {
            let n = 1usize << mm;
            let fft = VnlFft1d::new(n).unwrap();
            let x: Vec<f64> = (0..2 * n)
                .map(|i| ((i * 7919) % 101) as f64 / 17.0 - 2.5)
                .collect();
            for isign in [1.0, -1.0] {
                let mut y = x.clone();
                if isign > 0.0 {
                    fft.fwd_transform(&mut y);
                } else {
                    fft.bwd_transform(&mut y);
                }
                let expected = dft(&x, isign);
                let scale = expected.iter().fold(1.0f64, |m, v| m.max(v.abs()));
                for (a, b) in y.iter().zip(&expected) {
                    assert!(
                        (a - b).abs() < 1e-9 * scale,
                        "n = {n}, isign {isign}: {a} vs {b}"
                    );
                }
            }
        }
        assert!(VnlFft1d::new(12).is_none());
    }
}
