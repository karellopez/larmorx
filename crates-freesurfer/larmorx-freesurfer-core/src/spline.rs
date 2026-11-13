// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret. Translated from FreeSurfer 8.2.0's utils/numerics.cpp, whose spline
// routines are John Burkardt's (Florida State University, about 2006: "Original Author: John
// Burkardt, http://www.csit.fsu.edu/~burkardt/cpp_src/spline/spline.html") as FreeSurfer changed
// them (single precision, constant ends).
//! `fs.spline` (queue item 3.1-54): the cubic spline of FreeSurfer's intensity normalisation
//! (`mri_normalize`'s `MRIsplineNormalize` calls `OpenSpline` and `OpenSplint`), translated
//! statement by statement from FreeSurfer 8.2.0's `utils/numerics.cpp`: `OpenSpline` (1287-1296),
//! `OpenSplint` (1305-1312), `SplineCubicSet` (1417-1535), `SplineCubicValue` (1572-1611) and
//! `d3_np_fs` (1648-1680), John Burkardt's 2006 routines as FreeSurfer converted them to `float`
//! and made the evaluation constant outside the knots; `include/numerics.h`'s `SPLINE_USE_*`.
//!
//! Bit for bit with FreeSurfer's official build (GCC 8.4.0, x86-64 SSE2, no FMA): `float`
//! arithmetic where the source's operands are `float`, `double` where a `double` constant or
//! operand makes the expression `double` (`SplineCubicValue`'s value and first derivative), every
//! conversion as C makes it, and SSE's NaNs on every platform ([`crate::sse`]: of two NaN operands
//! the first source operand's, in the order of the official build's instructions: the source's
//! order, but for `d3_np_fs`'s two products with the super-diagonal, which the build computes as
//! `a[0 + i * 3] * xmult` and `x[i + 1] * a[0 + (i + 1) * 3]`).
//!
//! The fatal errors print FreeSurfer's lines on standard output (`std::cout`; the caller's `out`)
//! and end the program with `exit(1)` ([`Fatal::Exit`]).
//!
//! D13: **FS-103** (`SplineCubicSet` lets a NaN knot pass its check that the knots increase
//! strictly, so the spline's values are NaN): by default the check is "`t[i + 1] > t[i]` holds",
//! so a NaN knot gives the fatal error of a non-increasing pair (rule B); `reproduce` keeps
//! FreeSurfer's check, with a notice. **Not reproduced** (never reached by FreeSurfer's callers,
//! which pass two knots or more): `SplineCubicValue` with fewer than two knots and a NaN point
//! reads before its arrays (`t[-1]`); here it gives the point, quieted, as larmorx's clean room
//! does. `d3_np_fs` of order 0 writes before its result (`x[-1]`); here it returns the empty
//! solution.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::FS_103;

use crate::globals::Globals;
use crate::log::Fatal;
use crate::sse::{add, add64, div, div64, mul, mul64, sub, sub64};

/// `SPLINE_USE_QUADRATIC`: a quadratic over the end interval (equal second derivatives).
pub const SPLINE_USE_QUADRATIC: i32 = 0;
/// `SPLINE_USE_FIRST_DERIVATIVE`: the first derivative at the end is given.
pub const SPLINE_USE_FIRST_DERIVATIVE: i32 = 1;
/// `SPLINE_USE_SECOND_DERIVATIVE`: the second derivative at the end is given.
pub const SPLINE_USE_SECOND_DERIVATIVE: i32 = 2;

/// A `float` widened to `double` (`cvtss2sd`): exact; a NaN quieted, its sign and payload kept.
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

/// A `double` stored in a `float` (`cvtsd2ss`): rounded to nearest; a NaN keeps its sign and the
/// top 22 bits of its payload, quieted.
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

/// `(float)((double)v / d)`: a `float` divided by a `double` constant, stored in a `float` (the
/// source's `(t[1] - t[0]) / 3.0`; the same bits as the `float` division the build makes of it).
#[inline]
fn fdiv_d(v: f32, d: f64) -> f32 {
    narrow(div64(wide(v), d))
}

/// `SplineCubicSet`'s fatal error: its lines on `out` (`std::cout`), then `exit(1)`.
fn fatal(out: &mut dyn Write, lines: &[String]) -> Fatal {
    let _ = out.write_all(b"\n");
    let _ = out.write_all(b"SPLINE_CUBIC_SET - Fatal error!\n");
    for l in lines {
        let _ = out.write_all(l.as_bytes());
    }
    let _ = out.flush();
    Fatal::Exit(1)
}

/// `SplineCubicSet(n, t, y, ibcbeg, ybcbeg, ibcend, ybcend)`: the `n` second derivatives of the
/// cubic spline through `(t[i], y[i])` with the end conditions `ibcbeg` and `ibcend`
/// (`SPLINE_USE_*`) and their values `ybcbeg`, `ybcend`.
///
/// # Errors
/// FreeSurfer's fatal errors (its lines written on `out`, then [`Fatal::Exit`]`(1)`): `n <= 1`; a
/// pair of knots that does not increase (FS-103 decides a NaN knot, above); `ibcbeg` or `ibcend`
/// not 0, 1 or 2; a zero diagonal entry in the system.
///
/// # Panics
/// When `t` or `y` holds fewer than `n` values.
#[allow(clippy::too_many_arguments)]
pub fn spline_cubic_set(
    g: &mut Globals,
    out: &mut dyn Write,
    n: i32,
    t: &[f32],
    y: &[f32],
    ibcbeg: i32,
    ybcbeg: f32,
    ibcend: i32,
    ybcend: f32,
) -> Result<Vec<f32>, Fatal> {
    //
    //  Check.
    //
    if n <= 1 {
        return Err(fatal(
            out,
            &[
                "  The number of data points N must be at least 2.\n".to_owned(),
                format!("  The input value is {n}.\n"),
            ],
        ));
    }
    let n = n as usize;
    assert!(
        t.len() >= n && y.len() >= n,
        "SplineCubicSet: n values of t and y"
    );

    let reproduce = g.notices.reproduce();
    let mut nan_knot = false;
    for i in 0..n - 1 {
        // FreeSurfer: `t[i + 1] <= t[i]`, false for a NaN; FS-103's correction: "not increasing"
        let fails = if reproduce {
            t[i + 1] <= t[i]
        } else {
            !(t[i + 1] > t[i])
        };
        if fails {
            return Err(fatal(
                out,
                &[
                    "  The knots must be strictly increasing, but\n".to_owned(),
                    format!("  T({i}) = {}\n", cfmt::ostream(wide(t[i]))),
                    format!("  T({}) = {}\n", i + 1, cfmt::ostream(wide(t[i + 1]))),
                ],
            ));
        }
        nan_knot |= t[i].is_nan() || t[i + 1].is_nan();
    }

    let mut a = vec![0.0f32; 3 * n];
    let mut b = vec![0.0f32; n];
    //
    //  Set up the first equation.
    //
    if ibcbeg == SPLINE_USE_QUADRATIC {
        b[0] = 0.0;
        a[1] = 1.0;
        a[3] = -1.0;
    } else if ibcbeg == SPLINE_USE_FIRST_DERIVATIVE {
        b[0] = sub(div(sub(y[1], y[0]), sub(t[1], t[0])), ybcbeg);
        a[1] = fdiv_d(sub(t[1], t[0]), 3.0);
        a[3] = fdiv_d(sub(t[1], t[0]), 6.0);
    } else if ibcbeg == SPLINE_USE_SECOND_DERIVATIVE {
        b[0] = ybcbeg;
        a[1] = 1.0;
        a[3] = 0.0;
    } else {
        return Err(fatal(
            out,
            &[
                "  IBCBEG must be 0, 1 or 2.\n".to_owned(),
                format!("  The input value is {ibcbeg}.\n"),
            ],
        ));
    }
    //
    //  Set up the intermediate equations.
    //
    for i in 1..n - 1 {
        b[i] = sub(
            div(sub(y[i + 1], y[i]), sub(t[i + 1], t[i])),
            div(sub(y[i], y[i - 1]), sub(t[i], t[i - 1])),
        );
        a[2 + (i - 1) * 3] = fdiv_d(sub(t[i], t[i - 1]), 6.0);
        a[1 + i * 3] = fdiv_d(sub(t[i + 1], t[i - 1]), 3.0);
        a[(i + 1) * 3] = fdiv_d(sub(t[i + 1], t[i]), 6.0);
    }
    //
    //  Set up the last equation.
    //
    if ibcend == SPLINE_USE_QUADRATIC {
        b[n - 1] = 0.0;
        a[2 + (n - 2) * 3] = -1.0;
        a[1 + (n - 1) * 3] = 1.0;
    } else if ibcend == SPLINE_USE_FIRST_DERIVATIVE {
        b[n - 1] = sub(
            ybcend,
            div(sub(y[n - 1], y[n - 2]), sub(t[n - 1], t[n - 2])),
        );
        a[2 + (n - 2) * 3] = fdiv_d(sub(t[n - 1], t[n - 2]), 6.0);
        a[1 + (n - 1) * 3] = fdiv_d(sub(t[n - 1], t[n - 2]), 3.0);
    } else if ibcend == SPLINE_USE_SECOND_DERIVATIVE {
        b[n - 1] = ybcend;
        a[2 + (n - 2) * 3] = 0.0;
        a[1 + (n - 1) * 3] = 1.0;
    } else {
        return Err(fatal(
            out,
            &[
                "  IBCEND must be 0, 1 or 2.\n".to_owned(),
                format!("  The input value is {ibcend}.\n"),
            ],
        ));
    }
    if nan_knot {
        g.notices.add(
            &FS_103,
            "a NaN knot passes FreeSurfer's check that the knots increase strictly, and the \
             spline's values are NaN",
        );
    }
    //
    //  Solve the linear system.
    //
    if n == 2 && ibcbeg == 0 && ibcend == 0 {
        Ok(vec![0.0, 0.0])
    } else {
        d3_np_fs(&mut a, &b).ok_or_else(|| {
            fatal(
                out,
                &["  The linear system could not be solved.\n".to_owned()],
            )
        })
    }
}

/// `SplineCubicValue(n, t, tval, y, ypp, ypval, yppval)`: the spline's value at `tval`, with
/// `ypp` from [`spline_cubic_set`]. FreeSurfer's change: at or below `t[0]` the value is `y[0]`
/// and neither derivative is written; at or above `t[n - 1]` it is `y[n - 1]`, `*ypval` is set to
/// 0 (twice; `*yppval` is not written). Between the knots (and for a NaN `tval`) Burkardt's
/// cubic, the value and first derivative in `double`, the second derivative in `float`.
///
/// # Panics
/// When `n < 1` or `t`, `y` or `ypp` holds fewer than `n` values.
pub fn spline_cubic_value(
    n: i32,
    t: &[f32],
    tval: f32,
    y: &[f32],
    ypp: &[f32],
    ypval: &mut f32,
    yppval: &mut f32,
) -> f32 {
    assert!(n >= 1, "SplineCubicValue: at least one knot");
    let nn = n as usize;
    assert!(
        t.len() >= nn && y.len() >= nn && ypp.len() >= nn,
        "SplineCubicValue: n values"
    );
    let yval;
    if tval <= t[0] {
        // enforce constant function outside of domain
        yval = y[0];
    } else if tval >= t[nn - 1] {
        // enforce constant function outside of domain
        yval = y[nn - 1];
        *ypval = 0.0;
        *ypval = 0.0;
    } else if nn < 2 {
        // a NaN `tval` with one knot: FreeSurfer reads `t[-1]` (not reproduced)
        yval = f32::from_bits(tval.to_bits() | 0x0040_0000);
    } else {
        // we're not outside of the range, so interpolate
        let mut ival = nn - 2;
        for i in 0..nn - 1 {
            if tval < t[i + 1] {
                ival = i;
                break;
            }
        }
        let dt = sub(tval, t[ival]);
        let h = sub(t[ival + 1], t[ival]);
        // `(y[ival + 1] - y[ival]) / h - (ypp[ival + 1] / 6.0 + ypp[ival] / 3.0) * h`, in double
        let slope = wide(div(sub(y[ival + 1], y[ival]), h));
        let (m0, m1, dtd, hd) = (wide(ypp[ival]), wide(ypp[ival + 1]), wide(dt), wide(h));
        let gg = sub64(slope, mul64(add64(div64(m1, 6.0), div64(m0, 3.0)), hd));
        let diff = wide(sub(ypp[ival + 1], ypp[ival]));

        // yval = y[ival] + dt * (G + dt * (0.5 * ypp[ival] + dt * ((ypp[ival + 1] - ypp[ival]) /
        // (6.0 * h)))), the build's operands nested part first
        let k = div64(diff, mul64(6.0, hd));
        let inner = add64(mul64(k, dtd), mul64(m0, 0.5));
        yval = narrow(add64(
            mul64(add64(mul64(inner, dtd), gg), dtd),
            wide(y[ival]),
        ));

        // *ypval = G + dt * (ypp[ival] + dt * (0.5 * (ypp[ival + 1] - ypp[ival]) / h))
        let q = div64(mul64(diff, 0.5), hd);
        *ypval = narrow(add64(mul64(add64(mul64(q, dtd), m0), dtd), gg));

        // *yppval = ypp[ival] + dt * (ypp[ival + 1] - ypp[ival]) / h, in float
        *yppval = add(div(mul(dt, sub(ypp[ival + 1], ypp[ival])), h), ypp[ival]);
    }
    yval
}

/// `d3_np_fs(n, a, b)`: solves the tridiagonal system of order `n = b.len()` stored in `a` (D3
/// format, `3 * n` values: column `j` holds the super-diagonal entry `a[3 * j]` above the diagonal
/// `a[1 + 3 * j]` and the sub-diagonal `a[2 + 3 * j]` below it) by elimination without pivoting;
/// `a`'s diagonal is overwritten by the factorization. `None` when a diagonal entry is 0 before
/// the elimination (`NULL`).
///
/// # Panics
/// When `a` holds fewer than `3 * n` values.
#[must_use]
pub fn d3_np_fs(a: &mut [f32], b: &[f32]) -> Option<Vec<f32>> {
    let n = b.len();
    assert!(a.len() >= 3 * n, "d3_np_fs: 3 n values of a");
    //
    //  Check.
    //
    for i in 0..n {
        if a[1 + i * 3] == 0.0 {
            return None;
        }
    }
    if n == 0 {
        // FreeSurfer divides x[-1] (not reproduced)
        return Some(Vec::new());
    }

    let mut x = b[..n].to_vec();

    for i in 1..n {
        let xmult = div(a[2 + (i - 1) * 3], a[1 + (i - 1) * 3]);
        // the build: `a[0 + i * 3] * xmult`
        a[1 + i * 3] = sub(a[1 + i * 3], mul(a[i * 3], xmult));
        x[i] = sub(x[i], mul(xmult, x[i - 1]));
    }

    x[n - 1] = div(x[n - 1], a[1 + (n - 1) * 3]);
    for i in (0..n - 1).rev() {
        // the build: `x[i + 1] * a[0 + (i + 1) * 3]`
        x[i] = div(sub(x[i], mul(x[i + 1], a[(i + 1) * 3])), a[1 + i * 3]);
    }

    Some(x)
}

/// `OpenSpline(iaX, iaY, icXY, iYStartDerivative, iYEndDerivative, oaYSecondDerivatives)`: the
/// second derivatives ([`spline_cubic_set`] with first-derivative ends) into
/// `oa_y_second_derivatives`. (FreeSurfer never frees the array `SplineCubicSet` returns.)
///
/// # Errors
/// As [`spline_cubic_set`].
#[allow(clippy::too_many_arguments)]
pub fn open_spline(
    g: &mut Globals,
    out: &mut dyn Write,
    ia_x: &[f32],
    ia_y: &[f32],
    ic_xy: i32,
    i_y_start_derivative: f32,
    i_y_end_derivative: f32,
    oa_y_second_derivatives: &mut [f32],
) -> Result<(), Fatal> {
    let second = spline_cubic_set(
        g,
        out,
        ic_xy,
        ia_x,
        ia_y,
        SPLINE_USE_FIRST_DERIVATIVE,
        i_y_start_derivative,
        SPLINE_USE_FIRST_DERIVATIVE,
        i_y_end_derivative,
    )?;
    let n = ic_xy as usize;
    oa_y_second_derivatives[..n].copy_from_slice(&second[..n]);
    Ok(())
}

/// `OpenSplint(iaX, iaY, iaYSecondDerivatives, icYSecondDerivatives, iX, oY)`: the spline's value
/// at `i_x` ([`spline_cubic_value`]; the derivatives are discarded).
pub fn open_splint(
    ia_x: &[f32],
    ia_y: &[f32],
    ia_y_second_derivatives: &[f32],
    ic_y_second_derivatives: i32,
    i_x: f32,
    o_y: &mut f32,
) {
    let mut first_derivative = 0.0f32;
    let mut second_derivative = 0.0f32;
    *o_y = spline_cubic_value(
        ic_y_second_derivatives,
        ia_x,
        i_x,
        ia_y,
        ia_y_second_derivatives,
        &mut first_derivative,
        &mut second_derivative,
    );
}

#[cfg(test)]
mod tests;
