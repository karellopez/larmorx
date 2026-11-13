// SPDX-License-Identifier: GPL-3.0-or-later
// Translated from GSL 1.8's randist/flat.c, gauss.c, exponential.c, gamma.c, chisq.c, fdist.c
// and tdist.c (Copyright (C) 1996, 1997, 1998, 1999, 2000 James Theiler, Brian Gough; GNU GPL
// version 2 or later) as FreeSurfer 8.2.0's utils/numerics.cpp transcribes them (Silvester
// Czanner, 2006; that file carries The General Hospital Corporation's notice); translated to Rust
// in 2026 by Karel Lopez Vilaret, under the GNU GPL version 3 or later.
//! FreeSurfer's random distributions (`numerics.cpp`'s `sc_ran_*`, `sc_ugauss`, `sc_gamma_*`), as
//! methods of [`ScRng`], statement by statement: each draws from the stream in the order the
//! official build's code does (GCC 8.4.0 evaluates `sc_gamma_int(r, na) + sc_gamma_frac(r, a -
//! na)` left to right) and computes each expression in binary64 with the operand order of its
//! instructions (`crate::sse`: the NaN that passes on), glibc 2.35's `log` and `exp`
//! (`larmorx_core::math::log_glibc`, `exp_glibc`), the correctly rounded `tan` (the user's
//! decision of 2026-11-04: FreeSurfer's `sc_gamma_large` may differ in its last bits where glibc's
//! `tan` is not correctly rounded, 41,708 of its 2²⁴ arguments, `specs/ranlux.md` §6).
//!
//! D13 (`specs/ranlux.md` §8.1, FS-100): `sc_ran_gamma` converts `floor(a)` to `unsigned int`,
//! which C leaves undefined outside [0, 2³²); the official build keeps the low 32 bits of the
//! 64-bit truncation (`cvttsd2si`), 0 for NaN, ±∞ and beyond 2⁶³. By default the replica uses the
//! whole shape for a finite shape of 2³² or more (rule A) and refuses NaN, ±∞ and negative shapes
//! (rule B); under `reproduce` it gives FreeSurfer's value, except where FreeSurfer's loop does not
//! end (−∞, negative shapes: the same errors). FS-101's draws that never return:
//! [`DrawError::NeverReturns`].

use larmorx_core::defects::FS_100;
use larmorx_core::math::{exp_glibc as exp, log_glibc as log};

use crate::rng::{DrawError, ScRng, shape_text};
use crate::sse::{add, cvttsd2si64, div, mul, neg, sqrt, sub};

/// `M_PI` (`.LC125`, `0x400921fb54442d18`).
const M_PI: f64 = std::f64::consts::PI;
/// `M_E` (`.LC126`, `0x4005bf0a8b145769`).
const M_E: f64 = std::f64::consts::E;
/// 2³²: from there `(unsigned int)floor(a)` is undefined in C (FS-100).
const TWO_32: f64 = 4_294_967_296.0;

impl ScRng {
    /// `sc_ran_flat(r, a, b)`: `a * (1 - u) + b * u` (the build: `u·b + (1 − u)·a`).
    pub fn ran_flat(&mut self, a: f64, b: f64) -> f64 {
        let u = self.uni();
        // subsd: 1 - u; mulsd: u * b; mulsd: (1 - u) * a; addsd: (u * b) + ((1 - u) * a)
        let one_u = 1.0 - u;
        let ub = mul(u, b);
        let ua = mul(one_u, a);
        add(ub, ua)
    }

    /// `sc_ran_gaussian(r, sigma)`: Marsaglia's polar method, `sigma * y * sqrt(-2.0 * log(r2) /
    /// r2)`.
    ///
    /// # Errors
    /// FS-101 under `reproduce`: on the all-zero stream `x = y = −1` is rejected for ever.
    pub fn ran_gaussian(&mut self, sigma: f64) -> Result<f64, DrawError> {
        self.gaussian(sigma).map_err(|e| e.in_draw("Gaussian draw"))
    }

    fn gaussian(&mut self, sigma: f64) -> Result<f64, DrawError> {
        if self.zero_forever {
            return Err(self.never("Gaussian draw"));
        }
        let (mut y, mut r2);
        loop {
            // addsd x, x; subsd 1.0 (`-1 + 2 * u`, exact)
            let x = 2.0 * self.uni() - 1.0;
            y = 2.0 * self.uni() - 1.0;
            r2 = x * x + y * y;
            if !(r2 > 1.0 || r2 == 0.0) {
                break;
            }
        }
        // mulsd: y * sigma; log(r2) * -2.0; divsd r2; sqrt; sqrt(...) * (y * sigma)
        let ys = mul(y, sigma);
        let f = sqrt(div(mul(log(r2), -2.0), r2));
        Ok(mul(f, ys))
    }

    /// `sc_ugauss(r)`: `sc_ran_gaussian(r, 1.0)`.
    ///
    /// # Errors
    /// As [`ScRng::ran_gaussian`].
    pub fn ugauss(&mut self) -> Result<f64, DrawError> {
        self.gaussian(1.0)
            .map_err(|e| e.in_draw("unit Gaussian draw"))
    }

    /// `sc_gamma_int(r, a)`: below 12 `-log` of the product of `a` positive uniforms, from 12
    /// `sc_gamma_large(r, (double)a)`.
    ///
    /// # Errors
    /// FS-101 under `reproduce`.
    pub fn gamma_int(&mut self, a: u32) -> Result<f64, DrawError> {
        self.g_int(a).map_err(|e| e.in_draw("gamma draw"))
    }

    fn g_int(&mut self, a: u32) -> Result<f64, DrawError> {
        if a < 12 {
            let mut prod = 1.0;
            for _ in 0..a {
                // mulsd: prod * P
                prod = mul(prod, self.uni_pos()?);
            }
            // xorpd: -log(prod)
            Ok(neg(log(prod)))
        } else {
            self.g_large(f64::from(a))
        }
    }

    /// `sc_gamma_large(r, a)`: Knuth's rejection method with a Cauchy proposal (meant for
    /// `a ≥ 12`).
    ///
    /// # Errors
    /// FS-101 under `reproduce`, where the all-zero stream never gives `ret > 0` (`a ≤ 1`).
    pub fn gamma_large(&mut self, a: f64) -> Result<f64, DrawError> {
        self.g_large(a).map_err(|e| e.in_draw("gamma draw"))
    }

    fn g_large(&mut self, a: f64) -> Result<f64, DrawError> {
        // addsd a, a; subsd 1.0; sqrt
        let sqa = sqrt(sub(add(a, a), 1.0));
        loop {
            let (y, sy, ret);
            loop {
                // mulsd: u * M_PI; tan; mulsd: sqa * y; addsd: a + (sqa * y); subsd 1.0
                let yy = (self.tan)(mul(self.uni(), M_PI));
                let s = mul(sqa, yy);
                let t = sub(add(a, s), 1.0);
                if t <= 0.0 {
                    if self.zero_forever {
                        return Err(self.never("gamma draw"));
                    }
                    continue;
                }
                (y, sy, ret) = (yy, s, t);
                break;
            }
            let v = self.uni();
            // subsd: a - 1; divsd: ret / (a - 1); log; mulsd: log(...) * (a - 1); subsd: - sqa * y;
            // exp; mulsd: y * y; addsd: + 1; mulsd: (y * y + 1) * exp(...)
            let a1 = sub(a, 1.0);
            let e = exp(sub(mul(log(div(ret, a1)), a1), sy));
            let bound = mul(add(mul(y, y), 1.0), e);
            if v > bound {
                continue;
            }
            return Ok(ret);
        }
    }

    /// `sc_gamma_frac(r, a)`: Ahrens and Dieter's method for a shape in (0, 1) (Knuth's exercise
    /// 3.4.1-16).
    ///
    /// # Errors
    /// FS-101 under `reproduce`.
    pub fn gamma_frac(&mut self, a: f64) -> Result<f64, DrawError> {
        self.g_frac(a).map_err(|e| e.in_draw("gamma draw"))
    }

    fn g_frac(&mut self, a: f64) -> Result<f64, DrawError> {
        // addsd: a + M_E; divsd: M_E / (a + M_E)
        let p = div(M_E, add(a, M_E));
        loop {
            let u = self.uni();
            let v = self.uni_pos()?;
            let (ret, q);
            if u < p {
                // divsd: 1.0 / a; mulsd: log(v) * (1 / a); exp; xorpd; exp
                ret = exp(mul(log(v), div(1.0, a)));
                q = exp(neg(ret));
            } else {
                // subsd: 1.0 - log(v); log; subsd: a - 1; mulsd: log(ret) * (a - 1); exp
                ret = sub(1.0, log(v));
                q = exp(mul(log(ret), sub(a, 1.0)));
            }
            if !(self.uni() >= q) {
                return Ok(ret);
            }
        }
    }

    /// `sc_ran_gamma(r, a, b)`: `b * sc_gamma_int(r, na)` for an integer shape, `b *
    /// sc_gamma_frac(r, a)` below 1, else `b * (sc_gamma_int(r, na) + sc_gamma_frac(r, a - na))`
    /// with `na = (unsigned int)floor(a)` (FS-100, above).
    ///
    /// # Errors
    /// FS-100's ([`DrawError::ShapeNotFinite`], [`DrawError::ShapeNegative`]) and FS-101's.
    pub fn ran_gamma(&mut self, a: f64, b: f64) -> Result<f64, DrawError> {
        self.gamma(a, b).map_err(|e| e.in_draw("gamma draw"))
    }

    fn gamma(&mut self, a: f64, b: f64) -> Result<f64, DrawError> {
        // FS-100 (larmorx): the shapes FreeSurfer's conversion does not define.
        if a.is_nan() || a.is_infinite() {
            if self.notices.fix() || a == f64::NEG_INFINITY {
                return Err(DrawError::ShapeNotFinite(a));
            }
            self.notices.add(
                &FS_100,
                format!(
                    "FreeSurfer's gamma draw takes the shape {}, which is not a finite number, \
                     for a shape below 1",
                    shape_text(a)
                ),
            );
        } else if a < 0.0 {
            return Err(DrawError::ShapeNegative(a));
        }
        let fl = a.floor();
        // `(unsigned int)floor(a)`: cvttsd2si to 64 bits, the low 32 kept
        let na = cvttsd2si64(fl) as u32;
        if fl.is_finite() && fl >= TWO_32 {
            if self.notices.fix() {
                self.notices.add(
                    &FS_100,
                    format!(
                        "FreeSurfer's gamma draw truncates the shape {} to 32 bits ({na}) and \
                         gives a value of the wrong size; larmorx uses the whole shape",
                        shape_text(a)
                    ),
                );
                // the original's arithmetic with the whole integer part (`sc_gamma_int` of an
                // integer from 12 is `sc_gamma_large` of it)
                if a == fl {
                    return Ok(mul(self.g_large(fl)?, b));
                }
                let gi = self.g_large(fl)?;
                let gf = self.g_frac(sub(a, fl))?;
                return Ok(mul(add(gf, gi), b));
            }
            self.notices.add(
                &FS_100,
                format!(
                    "FreeSurfer's gamma draw truncates the shape {} to 32 bits ({na}) and gives \
                     a value of the wrong size",
                    shape_text(a)
                ),
            );
        }

        // ucomisd: a == (double)na
        let nad = f64::from(na);
        if a == nad {
            Ok(mul(self.g_int(na)?, b))
        } else if na == 0 {
            Ok(mul(self.g_frac(a)?, b))
        } else {
            // gamma_int first, then gamma_frac; addsd: gamma_frac's result is the destination
            let gi = self.g_int(na)?;
            let gf = self.g_frac(sub(a, nad))?;
            Ok(mul(add(gf, gi), b))
        }
    }

    /// `sc_ran_fdist(r, nu1, nu2)`: `(Y1 * nu2) / (Y2 * nu1)` with `Y1 = gamma(nu1 / 2, 2)` drawn
    /// first (the build: `(Y1·ν₂) / (ν₁·Y2)`).
    ///
    /// # Errors
    /// As [`ScRng::ran_gamma`].
    pub fn ran_fdist(&mut self, nu1: f64, nu2: f64) -> Result<f64, DrawError> {
        self.fdist(nu1, nu2).map_err(|e| e.in_draw("F draw"))
    }

    fn fdist(&mut self, nu1: f64, nu2: f64) -> Result<f64, DrawError> {
        // `nu / 2` is `0.5 * nu` in the build (exact)
        let y1 = self.gamma(mul(0.5, nu1), 2.0)?;
        let y2 = self.gamma(mul(0.5, nu2), 2.0)?;
        Ok(div(mul(y1, nu2), mul(nu1, y2)))
    }

    /// `sc_ran_chisq(r, nu)`: `2 * sc_ran_gamma(r, nu / 2, 1.0)`.
    ///
    /// # Errors
    /// As [`ScRng::ran_gamma`].
    pub fn ran_chisq(&mut self, nu: f64) -> Result<f64, DrawError> {
        self.chisq(nu).map_err(|e| e.in_draw("chi-squared draw"))
    }

    fn chisq(&mut self, nu: f64) -> Result<f64, DrawError> {
        // mulsd: nu * 0.5; addsd g, g
        let g = self.gamma(mul(nu, 0.5), 1.0)?;
        Ok(add(g, g))
    }

    /// `sc_ran_tdist(r, nu)`: for `nu <= 2` a unit Gaussian over `sqrt(chisq(nu) / nu)`, else
    /// Marsaglia's 1980 rejection method.
    ///
    /// # Errors
    /// As [`ScRng::ran_gamma`] for `nu <= 2`; FS-101 under `reproduce`.
    pub fn ran_tdist(&mut self, nu: f64) -> Result<f64, DrawError> {
        self.tdist(nu).map_err(|e| e.in_draw("t draw"))
    }

    fn tdist(&mut self, nu: f64) -> Result<f64, DrawError> {
        if nu <= 2.0 {
            let y1 = self.gaussian(1.0)?;
            let y2 = self.chisq(nu)?;
            return Ok(div(y1, sqrt(div(y2, nu))));
        }
        loop {
            let y1 = self.gaussian(1.0)?;
            // mulsd: 0.5 * nu; subsd 1.0; divsd: 1.0 / (...)
            let y2 = self.exponential(div(1.0, sub(mul(0.5, nu), 1.0)))?;
            // subsd: nu - 2.0; mulsd: y1 * y1; divsd
            let z = div(mul(y1, y1), sub(nu, 2.0));
            let one_z = sub(1.0, z);
            if one_z < 0.0 || exp(sub(neg(y2), z)) > one_z {
                continue;
            }
            // divsd: 2.0 / nu; subsd: 1.0 - (2 / nu); mulsd: (1 - 2 / nu) * (1 - z); sqrt; divsd
            return Ok(div(y1, sqrt(mul(sub(1.0, div(2.0, nu)), one_z))));
        }
    }

    /// `sc_ran_exponential(r, mu)`: `-mu * log(u)` with a positive uniform `u` (the build:
    /// `log(u)·(−μ)`).
    ///
    /// # Errors
    /// FS-101 under `reproduce`.
    pub fn ran_exponential(&mut self, mu: f64) -> Result<f64, DrawError> {
        self.exponential(mu)
            .map_err(|e| e.in_draw("exponential draw"))
    }

    fn exponential(&mut self, mu: f64) -> Result<f64, DrawError> {
        let u = self.uni_pos()?;
        Ok(mul(log(u), neg(mu)))
    }
}
