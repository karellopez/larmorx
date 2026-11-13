// SPDX-License-Identifier: Apache-2.0
//! The distributions of `specs/ranlux.md` §4, as methods of [`Ranlux`]: each consumes the stream
//! in the spec's order and computes the spec's expression operation by operation in binary64
//! (no fused multiply-add), with x86-64's NaNs (§7, the operand order of its table) on every
//! platform. A clean-room implementation of the spec.

use super::{DrawError, Ranlux};
use crate::defects::FS_100;
use crate::math::x86_nan::{DEFAULT_NAN_F64, add_f64, div_f64, mul_f64, sub_f64};
use crate::math::{exp_glibc, log_glibc};

/// π rounded to binary64 (§5: `0x400921fb54442d18`).
const PI_D: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
/// e rounded to binary64 (§5: `0x4005bf0a8b145769`).
const E_D: f64 = f64::from_bits(0x4005_bf0a_8b14_5769);
/// The integer gamma shape from which [`Ranlux::gamma_large`] replaces the product of uniforms.
const LARGE_SHAPE: u32 = 12;
/// 2³², the end of the gamma shapes FreeSurfer converts correctly (FS-100).
const TWO_32: f64 = 4_294_967_296.0;
/// 2⁶³: from there x86-64's conversion to a 64-bit integer gives its "integer indefinite".
const TWO_63: f64 = 9_223_372_036_854_775_808.0;

/// A shape as the notices and errors write it: Rust's shortest form, in exponent form from 10¹⁶.
pub(super) fn shape_text(a: f64) -> String {
    if a.is_finite() && a.abs() >= 1e16 {
        format!("{a:e}")
    } else {
        format!("{a}")
    }
}

/// The square root with SSE's NaNs: a NaN passed on quieted, the default NaN for a negative
/// number (`sqrtsd`).
#[inline]
fn sqrt(x: f64) -> f64 {
    let r = x.sqrt();
    if r.is_nan() {
        if x.is_nan() {
            f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
        } else {
            DEFAULT_NAN_F64
        }
    } else {
        r
    }
}

impl Ranlux {
    /// Flat on [a, b] (§4.1, `sc_ran_flat`): one uniform `U`, the result `a·(1 − U) + b·U`
    /// (computed as `U·b + (1 − U)·a`, the operand order of §7: b's NaN passes before a's).
    #[inline]
    pub fn flat(&mut self, a: f64, b: f64) -> f64 {
        let u = self.uniform();
        add_f64(mul_f64(u, b), mul_f64(1.0 - u, a))
    }

    /// Gaussian with standard deviation `sigma` (§4.2, `sc_ran_gaussian`; Marsaglia's polar
    /// method): pairs of uniforms until `0 < R ≤ 1`, then `(σ·Y)·sqrt(−2·log(R) / R)`; the
    /// second deviate of the pair is discarded.
    ///
    /// # Errors
    /// FS-101 under `reproduce` ([`DrawError::NeverReturns`]): an all-zero stream rejects every
    /// pair.
    pub fn gaussian(&mut self, sigma: f64) -> Result<f64, DrawError> {
        self.gaussian_draw(sigma)
            .map_err(|e| e.in_draw("Gaussian draw"))
    }

    /// The unit Gaussian (§4.3, `sc_ugauss`): [`Ranlux::gaussian`] with σ = 1.
    ///
    /// # Errors
    /// As [`Ranlux::gaussian`].
    pub fn ugaussian(&mut self) -> Result<f64, DrawError> {
        self.unit_gaussian()
            .map_err(|e| e.in_draw("unit Gaussian draw"))
    }

    #[inline]
    fn gaussian_draw(&mut self, sigma: f64) -> Result<f64, DrawError> {
        let (y, f) = self.polar()?;
        Ok(mul_f64(f, mul_f64(y, sigma)))
    }

    #[inline]
    fn unit_gaussian(&mut self) -> Result<f64, DrawError> {
        let (y, f) = self.polar()?;
        Ok(f * y)
    }

    /// The polar method's accepted `Y` and its factor `sqrt(−2·log(R) / R)` (never NaN).
    #[inline]
    fn polar(&mut self) -> Result<(f64, f64), DrawError> {
        if self.zero_forever {
            // X = Y = −1: R = 2, rejected for ever.
            return Err(self.never("Gaussian draw"));
        }
        loop {
            let x = -1.0 + 2.0 * self.uniform();
            let y = -1.0 + 2.0 * self.uniform();
            let r = x * x + y * y;
            if r > 1.0 || r == 0.0 {
                continue;
            }
            return Ok((y, (-2.0 * log_glibc(r) / r).sqrt()));
        }
    }

    /// Exponential with mean `mu` (§4.4, `sc_ran_exponential`): one positive uniform `P`, the
    /// result `log(P)·(−μ)`.
    ///
    /// # Errors
    /// FS-101 under `reproduce` ([`DrawError::NeverReturns`]).
    pub fn exponential(&mut self, mu: f64) -> Result<f64, DrawError> {
        self.exponential_draw(mu)
            .map_err(|e| e.in_draw("exponential draw"))
    }

    #[inline]
    fn exponential_draw(&mut self, mu: f64) -> Result<f64, DrawError> {
        let p = self.uniform_pos()?;
        Ok(mul_f64(log_glibc(p), -mu))
    }

    /// Gamma with shape `a` and scale `b` (§4.5, `sc_ran_gamma`): `b·G_int(n)` for an integer
    /// shape `n`, `b·G_frac(a)` below 1, else `b·(G_int(n) + G_frac(a − n))` with `n = floor(a)`
    /// (the integer part's draws first).
    ///
    /// Shapes outside [0, 2³²) are FS-100 (§8.1): by default a finite shape of 2³² or more uses
    /// its whole integer part (FreeSurfer truncates it to 32 bits), with a notice; NaN and ±∞
    /// are [`DrawError::ShapeNotFinite`]. Under `reproduce` such shapes give FreeSurfer's value
    /// bit for bit, but for −∞ ([`DrawError::ShapeNotFinite`]: FreeSurfer's draw never
    /// returns). A negative shape is [`DrawError::ShapeNegative`] in both modes.
    ///
    /// # Errors
    /// FS-100's, and FS-101's under `reproduce` ([`DrawError::NeverReturns`]).
    pub fn gamma(&mut self, a: f64, b: f64) -> Result<f64, DrawError> {
        self.gamma_draw(a, b).map_err(|e| e.in_draw("gamma draw"))
    }

    fn gamma_draw(&mut self, a: f64, b: f64) -> Result<f64, DrawError> {
        if a.is_nan() || a.is_infinite() {
            if self.notices.fix() || a == f64::NEG_INFINITY {
                return Err(DrawError::ShapeNotFinite(a));
            }
            // FreeSurfer's integer part of NaN and +∞ is 0.
            self.notices.add(
                &FS_100,
                format!(
                    "FreeSurfer's gamma draw takes the shape {}, which is not a finite number, \
                     for a shape below 1",
                    shape_text(a)
                ),
            );
            return Ok(mul_f64(self.g_frac(a)?, b));
        }
        if a < 0.0 {
            return Err(DrawError::ShapeNegative(a));
        }
        let fl = a.floor();
        if fl < TWO_32 {
            // fl is an integer in [0, 2^32): exact.
            let n = fl as u32;
            return if a == fl {
                Ok(mul_f64(self.g_int(n)?, b))
            } else if n == 0 {
                Ok(mul_f64(self.g_frac(a)?, b))
            } else {
                let gi = self.g_int(n)?;
                let gf = self.g_frac(a - fl)?;
                Ok(mul_f64(add_f64(gi, gf), b))
            };
        }
        let truncated = if fl >= TWO_63 {
            0
        } else {
            // The low 32 bits of the 64-bit truncation.
            (fl as i64) as u32
        };
        if self.notices.fix() {
            self.notices.add(
                &FS_100,
                format!(
                    "FreeSurfer's gamma draw truncates the shape {} to 32 bits ({truncated}) \
                     and gives a value of the wrong size; larmorx uses the whole shape",
                    shape_text(a)
                ),
            );
            let gl = self.g_large(fl)?;
            return if a == fl {
                Ok(mul_f64(gl, b))
            } else {
                let gf = self.g_frac(a - fl)?;
                Ok(mul_f64(add_f64(gl, gf), b))
            };
        }
        self.notices.add(
            &FS_100,
            format!(
                "FreeSurfer's gamma draw truncates the shape {} to 32 bits ({truncated}) and \
                 gives a value of the wrong size",
                shape_text(a)
            ),
        );
        if truncated == 0 {
            Ok(mul_f64(self.g_frac(a)?, b))
        } else {
            let gi = self.g_int(truncated)?;
            let gf = self.g_frac(a - f64::from(truncated))?;
            Ok(mul_f64(add_f64(gi, gf), b))
        }
    }

    /// The gamma draw of an integer shape `n` (§4.5's `G_int`): below 12 the negated logarithm
    /// of the product of `n` positive uniforms (`−0` for `n = 0`, which draws nothing), from 12
    /// [`Ranlux::gamma_large`].
    ///
    /// # Errors
    /// FS-101 under `reproduce` ([`DrawError::NeverReturns`]), for `1 ≤ n < 12`.
    pub fn gamma_int(&mut self, n: u32) -> Result<f64, DrawError> {
        self.g_int(n).map_err(|e| e.in_draw("gamma draw"))
    }

    #[inline]
    fn g_int(&mut self, n: u32) -> Result<f64, DrawError> {
        if n >= LARGE_SHAPE {
            return self.g_large(f64::from(n));
        }
        let mut prod = 1.0;
        for _ in 0..n {
            prod *= self.uniform_pos()?;
        }
        Ok(-log_glibc(prod))
    }

    /// The gamma draw of a large shape `A` (§4.5's `G_large`, meant for `A ≥ 12`; Knuth's
    /// rejection method with a Cauchy proposal): `s = sqrt(2A − 1)`, then `U`,
    /// `y = tan(π·U)`, `T = s·y + A − 1` until `T > 0`, accepted when a further uniform `V`
    /// is at most `(1 + y²)·exp((A − 1)·log(T / (A − 1)) − s·y)`.
    ///
    /// `tan` is the correctly rounded one ([`crate::math::tan`], §6).
    ///
    /// # Errors
    /// FS-101 under `reproduce` ([`DrawError::NeverReturns`]), only where an all-zero stream
    /// would never give `T > 0` (`A` below 1).
    pub fn gamma_large(&mut self, a: f64) -> Result<f64, DrawError> {
        self.g_large(a).map_err(|e| e.in_draw("gamma draw"))
    }

    fn g_large(&mut self, a: f64) -> Result<f64, DrawError> {
        let s = sqrt(2.0 * a - 1.0);
        let a1 = a - 1.0;
        loop {
            let (y, sy, t) = loop {
                let u = self.uniform();
                let y = (self.tan)(PI_D * u);
                let sy = mul_f64(s, y);
                let t = sub_f64(add_f64(sy, a), 1.0);
                if t <= 0.0 {
                    if self.zero_forever {
                        return Err(self.never("gamma draw"));
                    }
                    continue;
                }
                break (y, sy, t);
            };
            let v = self.uniform();
            let bound = mul_f64(
                1.0 + y * y,
                exp_glibc(sub_f64(mul_f64(a1, log_glibc(div_f64(t, a1))), sy)),
            );
            if v > bound {
                continue;
            }
            return Ok(t);
        }
    }

    /// The gamma draw of a shape `f` in (0, 1) (§4.5's `G_frac`; Ahrens and Dieter's method,
    /// Knuth's exercise 3.4.1-16), with `p = e / (f + e)`: a uniform `U₁` and a positive uniform
    /// `V`, then `X` and `q` by `U₁ < p`, accepted when a further uniform `W` is below `q`.
    ///
    /// # Errors
    /// FS-101 under `reproduce` ([`DrawError::NeverReturns`]).
    pub fn gamma_frac(&mut self, f: f64) -> Result<f64, DrawError> {
        self.g_frac(f).map_err(|e| e.in_draw("gamma draw"))
    }

    fn g_frac(&mut self, f: f64) -> Result<f64, DrawError> {
        let p = div_f64(E_D, add_f64(f, E_D));
        loop {
            let u1 = self.uniform();
            let v = self.uniform_pos()?;
            let (x, q) = if u1 < p {
                let x = exp_glibc(mul_f64(div_f64(1.0, f), log_glibc(v)));
                (x, exp_glibc(-x))
            } else {
                let x = 1.0 - log_glibc(v);
                (x, exp_glibc(mul_f64(sub_f64(f, 1.0), log_glibc(x))))
            };
            let w = self.uniform();
            if w >= q {
                continue;
            }
            return Ok(x);
        }
    }

    /// Chi-squared with `nu` degrees of freedom (§4.6, `sc_ran_chisq`): `2·gamma(ν/2, 1)`.
    ///
    /// # Errors
    /// As [`Ranlux::gamma`] (FS-100 for `ν < 0`, `ν` NaN or infinite, and FS-101).
    pub fn chisq(&mut self, nu: f64) -> Result<f64, DrawError> {
        self.chisq_draw(nu)
            .map_err(|e| e.in_draw("chi-squared draw"))
    }

    #[inline]
    fn chisq_draw(&mut self, nu: f64) -> Result<f64, DrawError> {
        let g = self.gamma_draw(nu / 2.0, 1.0)?;
        Ok(mul_f64(g, 2.0))
    }

    /// F with `nu1` and `nu2` degrees of freedom (§4.7, `sc_ran_fdist`):
    /// `Y₁ = gamma(ν₁/2, 2)`, then `Y₂ = gamma(ν₂/2, 2)`, the result `(Y₁·ν₂) / (ν₁·Y₂)`.
    ///
    /// # Errors
    /// As [`Ranlux::gamma`].
    pub fn fdist(&mut self, nu1: f64, nu2: f64) -> Result<f64, DrawError> {
        self.fdist_draw(nu1, nu2).map_err(|e| e.in_draw("F draw"))
    }

    fn fdist_draw(&mut self, nu1: f64, nu2: f64) -> Result<f64, DrawError> {
        let y1 = self.gamma_draw(nu1 / 2.0, 2.0)?;
        let y2 = self.gamma_draw(nu2 / 2.0, 2.0)?;
        Ok(div_f64(mul_f64(y1, nu2), mul_f64(nu1, y2)))
    }

    /// Student's t with `nu` degrees of freedom (§4.8, `sc_ran_tdist`): for `ν ≤ 2` a unit
    /// Gaussian over `sqrt(chisq(ν) / ν)`; otherwise (a NaN `ν` too) Marsaglia's 1980 rejection
    /// method, a unit Gaussian and an exponential of mean `1 / (ν/2 − 1)` per attempt.
    ///
    /// # Errors
    /// As [`Ranlux::gamma`] for `ν ≤ 2`; FS-101 under `reproduce` ([`DrawError::NeverReturns`]).
    pub fn tdist(&mut self, nu: f64) -> Result<f64, DrawError> {
        self.tdist_draw(nu).map_err(|e| e.in_draw("t draw"))
    }

    fn tdist_draw(&mut self, nu: f64) -> Result<f64, DrawError> {
        if nu <= 2.0 {
            let y1 = self.unit_gaussian()?;
            let y2 = self.chisq_draw(nu)?;
            return Ok(div_f64(y1, sqrt(div_f64(y2, nu))));
        }
        let mean = div_f64(1.0, sub_f64(nu / 2.0, 1.0));
        let nu2 = sub_f64(nu, 2.0);
        loop {
            let y1 = self.unit_gaussian()?;
            let y2 = self.exponential_draw(mean)?;
            let z = div_f64(y1 * y1, nu2);
            let one_z = sub_f64(1.0, z);
            if one_z < 0.0 {
                continue;
            }
            if exp_glibc(sub_f64(-y2, z)) > one_z {
                continue;
            }
            return Ok(div_f64(
                y1,
                sqrt(mul_f64(sub_f64(1.0, div_f64(2.0, nu)), one_z)),
            ));
        }
    }
}
