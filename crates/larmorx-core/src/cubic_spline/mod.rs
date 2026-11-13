// SPDX-License-Identifier: Apache-2.0
//! The interpolating cubic spline of FreeSurfer 8.2.0's intensity normalisation (`mri_normalize`'s
//! `MRIsplineNormalize`): a **clean-room implementation** of `specs/burkardt-spline.md`
//! (FreeSurfer's foundations, queue item 3.1-06), written from that spec, the numerical-analysis
//! literature it cites (cubic splines with first-derivative ends, tridiagonal elimination) and
//! black-box runs of FreeSurfer's own functions only.
//!
//! - [`second_derivatives`] (FreeSurfer's `SplineCubicSet`): the second derivatives of the spline
//!   through the knots with the given end conditions (§3), checked as FreeSurfer checks them,
//!   its five fatal errors as [`SplineError`]s with FreeSurfer's text (§7).
//! - [`solve_tridiagonal`] (`d3_np_fs`): the tridiagonal solve without pivoting (§4).
//! - [`evaluate`] (`SplineCubicValue`): the value and derivatives at a point, without
//!   extrapolation (§5).
//! - [`open_spline`] and [`open_splint`] (`OpenSpline`, `OpenSplint`): FreeSurfer's wrappers (§6).
//!
//! Every result has FreeSurfer's bits: binary32 operations where FreeSurfer's are binary32, the
//! binary64 steps of §5 where they are binary64, no fused multiply-add, x86-64's NaNs on every
//! platform. FS-103 (a NaN knot passes FreeSurfer's check, §8) is an error by default and
//! FreeSurfer's acceptance under [`UpstreamDefects::Reproduce`].

#[cfg(test)]
mod tests;

use crate::cfmt;
use crate::defects::{FS_103, Notices, UpstreamDefects};
use crate::math::x86_nan::{
    add_f32, add_f64, div_f32, div_f64, mul_f32, mul_f64, sub_f32, sub_f64,
};
use std::fmt;

/// The exit status of a program that meets a [`SplineError`] (FreeSurfer's `exit(1)`).
pub const EXIT_STATUS: u8 = 1;

/// One of `SplineCubicSet`'s fatal errors (§7). [`SplineError::message`] is FreeSurfer's text,
/// which the programs print on standard output before they exit with [`EXIT_STATUS`].
#[derive(Clone, Debug, PartialEq)]
pub enum SplineError {
    /// E1: fewer than two knots.
    TooFewKnots {
        /// The number of knots.
        n: usize,
    },
    /// E2: the knots do not increase strictly at `i` (`t[i + 1] <= t[i]`; by default also a NaN
    /// at `i` or `i + 1`, FS-103).
    NotIncreasing {
        /// The first index where they do not.
        i: usize,
        /// `t[i]`.
        ti: f32,
        /// `t[i + 1]`.
        tnext: f32,
    },
    /// E3: a beginning condition other than 0, 1 or 2.
    BadBegin {
        /// The condition given.
        ibcbeg: i32,
    },
    /// E4: an end condition other than 0, 1 or 2.
    BadEnd {
        /// The condition given.
        ibcend: i32,
    },
    /// E5: the tridiagonal system has a zero diagonal entry.
    NotSolvable,
}

impl SplineError {
    /// FreeSurfer's text, line feeds included (each message starts with an empty line).
    #[must_use]
    pub fn message(&self) -> String {
        let head = "\nSPLINE_CUBIC_SET - Fatal error!\n";
        match self {
            SplineError::TooFewKnots { n } => format!(
                "{head}  The number of data points N must be at least 2.\n  The input value is {n}.\n"
            ),
            SplineError::NotIncreasing { i, ti, tnext } => format!(
                "{head}  The knots must be strictly increasing, but\n  T({i}) = {}\n  T({}) = {}\n",
                cfmt::g(f64::from(*ti), 6),
                i + 1,
                cfmt::g(f64::from(*tnext), 6)
            ),
            SplineError::BadBegin { ibcbeg } => {
                format!("{head}  IBCBEG must be 0, 1 or 2.\n  The input value is {ibcbeg}.\n")
            }
            SplineError::BadEnd { ibcend } => {
                format!("{head}  IBCEND must be 0, 1 or 2.\n  The input value is {ibcend}.\n")
            }
            SplineError::NotSolvable => {
                format!("{head}  The linear system could not be solved.\n")
            }
        }
    }
}

impl fmt::Display for SplineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message().trim())
    }
}

impl std::error::Error for SplineError {}

/// A binary32 value as binary64: exact, a NaN quieted with its sign and payload kept
/// (`cvtss2sd`).
#[inline]
fn wide(x: f32) -> f64 {
    if x.is_nan() {
        let b = u64::from(x.to_bits());
        f64::from_bits(
            ((b & 0x8000_0000) << 32) | 0x7ff8_0000_0000_0000 | ((b & 0x003f_ffff) << 29),
        )
    } else {
        f64::from(x)
    }
}

/// A binary64 value rounded to binary32 (to nearest, ties to even); a NaN keeps its sign and the
/// top 22 bits of its payload, quieted (`cvtsd2ss`).
#[inline]
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `d3_np_fs` (§4): solves the tridiagonal system with sub-diagonal `l` (`l[0]` unused),
/// diagonal `d`, super-diagonal `u` (`u[n - 1]` unused) and right-hand side `b`, by elimination
/// without pivoting, in binary32. `None` when a diagonal entry is 0 before elimination (+0 or
/// −0); a diagonal that becomes 0 during the elimination divides by zero (±∞ or NaN results).
///
/// # Panics
/// When the four slices do not have the same length.
#[must_use]
pub fn solve_tridiagonal(l: &[f32], d: &[f32], u: &[f32], b: &[f32]) -> Option<Vec<f32>> {
    let n = d.len();
    assert!(
        l.len() == n && u.len() == n && b.len() == n,
        "the system's arrays must have one length"
    );
    // +0 and -0 compare equal to 0; a NaN does not
    if d.contains(&0.0) {
        return None;
    }
    let mut x = b.to_vec();
    let mut dd = d.to_vec();
    for i in 1..n {
        let m = div_f32(l[i], dd[i - 1]);
        dd[i] = sub_f32(d[i], mul_f32(u[i - 1], m));
        x[i] = sub_f32(x[i], mul_f32(m, x[i - 1]));
    }
    if n > 0 {
        x[n - 1] = div_f32(x[n - 1], dd[n - 1]);
    }
    for i in (0..n.saturating_sub(1)).rev() {
        x[i] = div_f32(sub_f32(x[i], mul_f32(u[i], x[i + 1])), dd[i]);
    }
    Some(x)
}

/// `SplineCubicSet` (§3): the second derivatives of the cubic spline through `(t[i], y[i])`.
///
/// `ibcbeg` and `ibcend` are the end conditions (0: a quadratic over the end interval; 1: the
/// first derivative there is `ybcbeg` / `ybcend`; 2: the second derivative is). The checks come
/// in FreeSurfer's order: the number of knots, the knots' order, `ibcbeg`, `ibcend`.
///
/// FS-103 (§8): by default a NaN knot fails the order check (E2); with `notices` in
/// [`UpstreamDefects::Reproduce`] mode it passes, as in FreeSurfer, and the results are NaN (a
/// notice says so).
///
/// # Errors
/// FreeSurfer's fatal errors E1 to E5 ([`SplineError`]).
///
/// # Panics
/// When `t` and `y` differ in length.
#[allow(clippy::too_many_arguments)]
pub fn second_derivatives(
    t: &[f32],
    y: &[f32],
    ibcbeg: i32,
    ybcbeg: f32,
    ibcend: i32,
    ybcend: f32,
    notices: &mut Notices,
) -> Result<Vec<f32>, SplineError> {
    assert_eq!(t.len(), y.len(), "one value per knot");
    let n = t.len();
    if n <= 1 {
        return Err(SplineError::TooFewKnots { n });
    }
    let reproduce = notices.mode() == UpstreamDefects::Reproduce;
    let mut nan_knot = false;
    for i in 0..n - 1 {
        let increasing = t[i + 1] > t[i];
        let fails = if reproduce {
            t[i + 1] <= t[i]
        } else {
            !increasing
        };
        if fails {
            return Err(SplineError::NotIncreasing {
                i,
                ti: t[i],
                tnext: t[i + 1],
            });
        }
        nan_knot |= t[i].is_nan() || t[i + 1].is_nan();
    }
    if !(0..=2).contains(&ibcbeg) {
        return Err(SplineError::BadBegin { ibcbeg });
    }
    let mut l = vec![0.0f32; n];
    let mut d = vec![0.0f32; n];
    let mut u = vec![0.0f32; n];
    let mut b = vec![0.0f32; n];
    let dt = |i: usize| sub_f32(t[i + 1], t[i]);
    let slope = |i: usize| div_f32(sub_f32(y[i + 1], y[i]), dt(i));
    match ibcbeg {
        0 => {
            d[0] = 1.0;
            u[0] = -1.0;
            b[0] = 0.0;
        }
        1 => {
            d[0] = div_f32(dt(0), 3.0);
            u[0] = div_f32(dt(0), 6.0);
            b[0] = sub_f32(slope(0), ybcbeg);
        }
        _ => {
            d[0] = 1.0;
            u[0] = 0.0;
            b[0] = ybcbeg;
        }
    }
    for i in 1..n - 1 {
        l[i] = div_f32(dt(i - 1), 6.0);
        d[i] = div_f32(sub_f32(t[i + 1], t[i - 1]), 3.0);
        u[i] = div_f32(dt(i), 6.0);
        b[i] = sub_f32(slope(i), slope(i - 1));
    }
    match ibcend {
        0 => {
            l[n - 1] = -1.0;
            d[n - 1] = 1.0;
            b[n - 1] = 0.0;
        }
        1 => {
            l[n - 1] = div_f32(dt(n - 2), 6.0);
            d[n - 1] = div_f32(dt(n - 2), 3.0);
            b[n - 1] = sub_f32(ybcend, slope(n - 2));
        }
        2 => {
            l[n - 1] = 0.0;
            d[n - 1] = 1.0;
            b[n - 1] = ybcend;
        }
        _ => return Err(SplineError::BadEnd { ibcend }),
    }
    if nan_knot {
        notices.add(
            &FS_103,
            "a NaN knot passes FreeSurfer's check that the knots increase strictly, and the \
             spline's values are NaN",
        );
    }
    if n == 2 && ibcbeg == 0 && ibcend == 0 {
        return Ok(vec![0.0, 0.0]);
    }
    solve_tridiagonal(&l, &d, &u, &b).ok_or(SplineError::NotSolvable)
}

/// The spline's value at a point and the derivatives FreeSurfer writes (§5): `None` where it
/// writes nothing (both below the first knot; the second above the last).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evaluation {
    /// The value.
    pub value: f32,
    /// The first derivative, where FreeSurfer writes it (`+0` from the last knot on).
    pub first: Option<f32>,
    /// The second derivative, where FreeSurfer writes it (between the knots only).
    pub second: Option<f32>,
}

/// `SplineCubicValue` (§5): the spline through `(t[i], y[i])` with second derivatives `ypp`
/// (from [`second_derivatives`]) at `x`. FreeSurfer's evaluation does not extrapolate: at or
/// below the first knot it gives the first value, at or above the last the last value. Between
/// knots (and for a NaN `x`) the interval's cubic in nested form, with `δ`, `h`, the slope and
/// the second derivatives' difference rounded to binary32 and the rest in binary64, rounded to
/// binary32 once.
///
/// With fewer than two knots there is no interval: a NaN `x` gives a NaN value (FreeSurfer would
/// read outside its arrays; its callers never ask).
///
/// # Panics
/// When `t`, `y` and `ypp` differ in length, or are empty.
#[must_use]
pub fn evaluate(t: &[f32], y: &[f32], ypp: &[f32], x: f32) -> Evaluation {
    let n = t.len();
    assert!(
        n > 0 && y.len() == n && ypp.len() == n,
        "one value and one second derivative per knot"
    );
    if x <= t[0] {
        return Evaluation {
            value: y[0],
            first: None,
            second: None,
        };
    }
    if x >= t[n - 1] {
        return Evaluation {
            value: y[n - 1],
            first: Some(0.0),
            second: None,
        };
    }
    if n < 2 {
        return Evaluation {
            value: f32::from_bits(x.to_bits() | 0x0040_0000),
            first: None,
            second: None,
        };
    }
    let j = (0..n - 1).find(|&i| x < t[i + 1]).unwrap_or(n - 2);
    let delta = sub_f32(x, t[j]);
    let h = sub_f32(t[j + 1], t[j]);
    let s = div_f32(sub_f32(y[j + 1], y[j]), h);
    let i = sub_f32(ypp[j + 1], ypp[j]);
    let (mj, mj1) = (wide(ypp[j]), wide(ypp[j + 1]));
    let (delta64, h64, i64_) = (wide(delta), wide(h), wide(i));
    let g = sub_f64(
        wide(s),
        mul_f64(add_f64(div_f64(mj1, 6.0), div_f64(mj, 3.0)), h64),
    );
    let k = div_f64(i64_, mul_f64(6.0, h64));
    let half_mj = mul_f64(0.5, mj);
    let n_ = mul_f64(add_f64(mul_f64(k, delta64), half_mj), delta64);
    let value = add_f64(mul_f64(add_f64(n_, g), delta64), wide(y[j]));
    let first = add_f64(
        mul_f64(
            add_f64(mul_f64(div_f64(mul_f64(0.5, i64_), h64), delta64), mj),
            delta64,
        ),
        g,
    );
    let second = add_f32(div_f32(mul_f32(delta, i), h), ypp[j]);
    Evaluation {
        value: narrow(value),
        first: Some(narrow(first)),
        second: Some(second),
    }
}

/// `OpenSpline` (§6): [`second_derivatives`] with the first derivatives `d0` and `d1` at the two
/// ends (conditions 1 and 1). FreeSurfer's callers pass `d0 = d1 = 0` and at most 80 knots.
///
/// # Errors
/// As [`second_derivatives`].
pub fn open_spline(
    t: &[f32],
    y: &[f32],
    d0: f32,
    d1: f32,
    notices: &mut Notices,
) -> Result<Vec<f32>, SplineError> {
    second_derivatives(t, y, 1, d0, 1, d1, notices)
}

/// `OpenSplint` (§6): the value of [`evaluate`] at `x`; the derivatives are discarded.
#[must_use]
pub fn open_splint(t: &[f32], y: &[f32], ypp: &[f32], x: f32) -> f32 {
    evaluate(t, y, ypp, x).value
}
