// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/thd_shift2.c.
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! Shifting time series by a fraction of a sample: AFNI's `thd_shift2.c`, bit for bit.
//!
//! [`shift_two_rows`] is `SHIFT_two_rows`: it shifts row `f` by `af` samples and row `g` (if
//! any) by `ag` samples, in place, with the method that `SHIFT_set_method` selected. Every
//! method keeps AFNI's arithmetic, including where C mixes precisions:
//!
//! - The rows are `float`.
//! - The Lagrange weights are evaluated in double (their constants are double literals), but
//!   `x*x` is a float product, as the macros write it; the weights are stored as float.
//! - Inside the series the weighted sums are float. Near the ends AFNI uses `FINS(i)`, which is
//!   `(i out of range) ? 0.0 : f[i]`: its type is double, so those sums are double, rounded to
//!   float at the end. The interior range is AFNI's (`ibot`, `itop`), which for `-wsinc5` is
//!   one sample narrower at the start than the kernel needs.
//! - The integer part of the shift is `(int)(-af)`, minus one when `-af < 0`, so a negative
//!   whole shift gives the fraction 1, not 0.
//! - Weighted sinc calls the C library's float `sinf`/`cosf`; this crate uses its port of
//!   glibc's ([`crate::glibc_sincosf`]), the library AFNI runs with on Linux.
//! - The Fourier method shifts two rows at once through one complex FFT (AFNI's own
//!   `csfft_cox`, [`crate::csfft`]), with the phase factors built by recurrence (`RECUR`) from
//!   double `cos`/`sin` stored as float, and the rows padded with zeros (`ZFILL`).

use crate::csfft::{Complex32, CsfftPlan};
use crate::glibc_sincosf::{cosf, sinf};

/// The interpolation methods of `SHIFT_set_method` (`MRI_FOURIER`, `MRI_LINEAR`, ...).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Method {
    /// FFT phase shift (`fft_shift2`, AFNI's default).
    #[default]
    Fourier,
    /// Linear interpolation (`lin_shift`).
    Linear,
    /// Cubic Lagrange polynomials (`cub_shift`).
    Cubic,
    /// Quintic Lagrange polynomials (`quint_shift`).
    Quintic,
    /// Heptic Lagrange polynomials (`hept_shift`).
    Heptic,
    /// Weighted sinc over ±5 points (`wsinc5_shift`).
    Wsinc5,
    /// Weighted sinc over ±9 points (`wsinc9_shift`).
    Wsinc9,
    /// Nearest neighbour (`nn_shift`; "experimental, do not use" in AFNI; no 3dTshift option
    /// selects it).
    Nn,
    /// Two-step interpolation (`ts_shift`; experimental; no 3dTshift option selects it).
    Ts,
}

/// Work space for [`shift_two_rows`]: AFNI keeps it in static variables (`row`, `cf`, `cg`,
/// `lcbuf`); here each thread owns one. The FFT plan is kept for the last length used.
#[derive(Default)]
pub struct ShiftWork {
    plan: Option<CsfftPlan>,
    scratch: Vec<Complex32>,
    row: Vec<Complex32>,
    cf: Vec<Complex32>,
    cg: Vec<Complex32>,
    lcbuf: Vec<f32>,
}

impl std::fmt::Debug for ShiftWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftWork").finish_non_exhaustive()
    }
}

impl ShiftWork {
    fn plan(&mut self, nup: usize) -> &CsfftPlan {
        if self.plan.as_ref().is_none_or(|p| p.size() != nup) {
            let plan = CsfftPlan::new(nup).unwrap_or_else(|e| panic!("{e}"));
            self.scratch = vec![Complex32::default(); plan.scratch_len()];
            self.plan = Some(plan);
        }
        self.plan.as_ref().expect("the plan was just made")
    }
}

/// `SHIFT_two_rows(n, nup, af, f, ag, g)`: shifts `f` by `af` and `g` (if any) by `ag`
/// samples, in place (`n` is the rows' length). `nup` is the FFT length, used only by
/// [`Method::Fourier`]; it must be even, at least `n`, and a length `csfft_cox` computes
/// itself (3dTshift uses `csfft_nextup_one35(ntt + 4)`).
///
/// # Panics
///
/// If `g` is not as long as `f`, or `nup` is not usable for the Fourier method.
pub fn shift_two_rows(
    method: Method,
    nup: usize,
    af: f32,
    f: &mut [f32],
    ag: f32,
    g: Option<&mut [f32]>,
    work: &mut ShiftWork,
) {
    if let Some(g) = g.as_deref() {
        assert_eq!(
            g.len(),
            f.len(),
            "SHIFT_two_rows: rows of different lengths"
        );
    }
    match method {
        Method::Fourier => fft_shift2(nup, af, f, ag, g, work),
        _ => {
            shift_one(method, af, f, &mut work.lcbuf);
            if let Some(g) = g {
                shift_one(method, ag, g, &mut work.lcbuf);
            }
        }
    }
}

/// The one-row shifters (`lin_shift2` and the others call them on `f`, then on `g`).
fn shift_one(method: Method, af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    match method {
        Method::Linear => lin_shift(af, f, lcbuf),
        Method::Cubic => cub_shift(af, f, lcbuf),
        Method::Quintic => quint_shift(af, f, lcbuf),
        Method::Heptic => hept_shift(af, f, lcbuf),
        Method::Wsinc5 => wsinc5_shift(af, f, lcbuf),
        Method::Wsinc9 => wsinc9_shift(af, f, lcbuf),
        Method::Nn => nn_shift(af, f),
        Method::Ts => ts_shift(af, f, lcbuf),
        Method::Fourier => unreachable!("the Fourier method shifts two rows at once"),
    }
}

// ------------------------------------------------------------------------------------------------
// Fourier

/// `mrilib.h`'s `PI`, `3.1415926535897932`: the double nearest π.
const PI: f64 = std::f64::consts::PI;

/// `CMULT(u, v)`: the complex product in float.
#[inline]
fn cmult(u: Complex32, v: Complex32) -> Complex32 {
    Complex32::new(u.r * v.r - u.i * v.i, u.r * v.i + u.i * v.r)
}

/// `CEXPIT(t)`: `cos` and `sin` of the float `t` in double, stored as float.
#[inline]
fn cexpit(t: f32) -> Complex32 {
    let t = f64::from(t);
    Complex32::new(
        larmorx_core::math::cos(t) as f32,
        larmorx_core::math::sin(t) as f32,
    )
}

/// `fft_shift2` with `ZFILL` and `RECUR` defined, as AFNI builds it.
fn fft_shift2(
    nup: usize,
    af: f32,
    f: &mut [f32],
    ag: f32,
    mut g: Option<&mut [f32]>,
    work: &mut ShiftWork,
) {
    let n = f.len();
    let nby2 = nup / 2;
    assert!(
        nup >= n && nup.is_multiple_of(2) && nup >= 2,
        "fft_shift2: nup = {nup} must be even and at least n = {n}"
    );

    // 15 Mar 2001: a shift too big on both rows gives all zeros. (AFNI writes `g[ii] = 0`
    // even when `g` is NULL, which crashes; here only the rows that exist are cleared.)
    let nf = n as f32;
    if (af < -nf || af > nf) && (ag < -nf || ag > nf) {
        f.fill(0.0);
        if let Some(g) = g.as_deref_mut() {
            g.fill(0.0);
        }
        return;
    }

    work.plan(nup);
    let ShiftWork {
        plan,
        scratch,
        row,
        cf,
        cg,
        ..
    } = work;
    let plan = plan.as_ref().expect("made above");
    row.clear();
    row.resize(nup, Complex32::default());
    cf.clear();
    cf.resize(nby2 + 1, Complex32::default());
    cg.clear();
    cg.resize(nby2 + 1, Complex32::default());

    // FFT the pair of rows (the tail stays zero: ZFILL).
    match g.as_deref() {
        Some(g) => {
            for ii in 0..n {
                row[ii] = Complex32::new(f[ii], g[ii]);
            }
        }
        None => {
            for ii in 0..n {
                row[ii] = Complex32::new(f[ii], 0.0);
            }
        }
    }
    plan.execute_with_scratch(-1, row, scratch);

    // Untangle the FFT coefficients of the two rows ("twice too big"). `2.0 * x` is a double
    // product, exact, stored as float.
    cf[0] = Complex32::new((2.0 * f64::from(row[0].r)) as f32, 0.0);
    cg[0] = Complex32::new((2.0 * f64::from(row[0].i)) as f32, 0.0);
    for ii in 1..nby2 {
        let (a, b) = (row[ii], row[nup - ii]);
        cf[ii] = Complex32::new(a.r + b.r, a.i - b.i);
        cg[ii] = Complex32::new(a.i + b.i, -a.r + b.r);
    }
    cf[nby2] = Complex32::new((2.0 * f64::from(row[nby2].r)) as f32, 0.0);
    cg[nby2] = Complex32::new((2.0 * f64::from(row[nby2].i)) as f32, 0.0);

    // Phase shift both rows. `dk` is computed in double and stored as float; `sf`, `sg` are
    // float products.
    let dk = ((2.0 * PI) / nup as f64) as f32;
    let sf = -af * dk;
    let sg = -ag * dk;
    let csf = cexpit(sf);
    let csg = cexpit(sg);
    let mut fac = Complex32::new(1.0, 0.0);
    let mut gac = Complex32::new(1.0, 0.0);
    for ii in 1..=nby2 {
        fac = cmult(csf, fac);
        cf[ii] = cmult(fac, cf[ii]);
        gac = cmult(csg, gac);
        cg[ii] = cmult(gac, cg[ii]);
    }
    cf[nby2].i = 0.0;
    cg[nby2].i = 0.0;

    // Re-tangle the coefficients of the two rows.
    row[0] = Complex32::new(cf[0].r, cg[0].r);
    for ii in 1..nby2 {
        let (a, b) = (cf[ii], cg[ii]);
        row[ii] = Complex32::new(a.r - b.i, a.i + b.r);
        row[nup - ii] = Complex32::new(a.r + b.i, -a.i + b.r);
    }
    row[nby2] = Complex32::new(cf[nby2].r, cg[nby2].r);

    // Inverse FFT and store back; `0.5 / nup` (double, stored as float) undoes "twice too big"
    // and the unnormalised inverse.
    plan.execute_with_scratch(1, row, scratch);
    let scale = (0.5 / nup as f64) as f32;
    match g {
        Some(g) => {
            for ii in 0..n {
                f[ii] = scale * row[ii].r;
                g[ii] = scale * row[ii].i;
            }
        }
        None => {
            for ii in 0..n {
                f[ii] = scale * row[ii].r;
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Polynomial and sinc kernels

/// `af = -af ; ia = (int)af ; if( af < 0 ) ia-- ;` then `aa = af - ia` (float). `None` when
/// `ia <= -n || ia >= n`: the row becomes zeros.
#[inline]
fn split(af: f32, n: usize) -> Option<(i64, f32)> {
    let af = -af;
    // C's (int) truncates toward zero; out of the int range C is undefined (x86 gives
    // INT_MIN) and Rust saturates: either way the shift is beyond the row.
    let mut ia = i64::from(af as i32);
    if af < 0.0 {
        ia -= 1;
    }
    let n = n as i64;
    if ia <= -n || ia >= n {
        return None;
    }
    Some((ia, af - ia as f32))
}

/// `FINS(i)` with `ZFILL`: the sample, or 0 outside the row, as a double (the type of C's
/// `?:` with `0.0`).
#[inline]
fn fins(f: &[f32], i: i64) -> f64 {
    if i < 0 || i >= f.len() as i64 {
        0.0
    } else {
        f64::from(f[i as usize])
    }
}

/// The weights of one AFNI shifter and the order and bounds of its sums.
struct Kernel<'a> {
    /// The offset of the first weight from `ix = ii + ia`.
    lo: i64,
    /// The weights, for the offsets `lo..lo + w.len()`.
    w: &'a [f32],
    /// The weights in the order AFNI's expression adds them.
    order: &'a [usize],
    /// AFNI's bounds of the interior: `ibot = ibot0 - ia`, `itop = n + itop0 - ia`.
    ibot0: i64,
    itop0: i64,
}

/// The weighted sum of one AFNI shifter. Inside `[ibot, itop]` the sum is float; outside, it
/// uses `FINS` and is double.
fn apply(f: &mut [f32], lcbuf: &mut Vec<f32>, ia: i64, k: &Kernel<'_>) {
    let Kernel {
        lo,
        w,
        order,
        ibot0,
        itop0,
    } = *k;
    let n = f.len() as i64;
    lcbuf.clear();
    lcbuf.resize(f.len(), 0.0);
    let mut ibot = (ibot0 - ia).max(0);
    let mut itop = (n + itop0 - ia).min(n - 1);
    for ii in ibot..=itop {
        let ix = ii + ia;
        let mut s = 0.0f32;
        for (k, &j) in order.iter().enumerate() {
            let term = w[j] * f[(ix + lo + j as i64) as usize];
            s = if k == 0 { term } else { s + term };
        }
        lcbuf[ii as usize] = s;
    }
    let edge = |ii: i64| -> f32 {
        let ix = ii + ia;
        let mut s = 0.0f64;
        for (k, &j) in order.iter().enumerate() {
            let term = f64::from(w[j]) * fins(f, ix + lo + j as i64);
            s = if k == 0 { term } else { s + term };
        }
        s as f32
    };
    if ibot > n {
        ibot = n;
    }
    for ii in 0..ibot {
        lcbuf[ii as usize] = edge(ii);
    }
    if itop < 0 {
        itop = -1;
    }
    for ii in itop + 1..n {
        lcbuf[ii as usize] = edge(ii);
    }
    f.copy_from_slice(lcbuf);
}

/// `lin_shift`.
fn lin_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    let Some((ia, aa)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    // wt_00 = 1.0 - aa (double, stored as float) ; wt_p1 = aa
    let w = [(1.0 - f64::from(aa)) as f32, aa];
    // wt_00 * f[ix] + wt_p1 * f[ix+1] ; ibot = -ia, itop = n-2-ia
    apply(
        f,
        lcbuf,
        ia,
        &Kernel {
            lo: 0,
            w: &w,
            order: &[0, 1],
            ibot0: 0,
            itop0: -2,
        },
    );
}

/// `cub_shift`, with the cubic Lagrange weights `P_M1` ... `P_P2`.
fn cub_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    let Some((ia, aa)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    let x = f64::from(aa);
    let w = [
        (x * (1.0 - x) * (x - 2.0) * 0.1666667) as f32, // P_M1
        ((x + 1.0) * (x - 1.0) * (x - 2.0) * 0.5) as f32, // P_00
        (x * (x + 1.0) * (2.0 - x) * 0.5) as f32,       // P_P1
        (x * (x + 1.0) * (x - 1.0) * 0.1666667) as f32, // P_P2
    ];
    // wt_m1*f[ix-1] + wt_00*f[ix] + wt_p1*f[ix+1] + wt_p2*f[ix+2] ; ibot = 1-ia, itop = n-3-ia
    apply(
        f,
        lcbuf,
        ia,
        &Kernel {
            lo: -1,
            w: &w,
            order: &[0, 1, 2, 3],
            ibot0: 1,
            itop0: -3,
        },
    );
}

/// `x*x` as the macros compute it: a float product, then widened (`x*x-1.0` is double).
#[inline]
fn sq(x: f32) -> f64 {
    f64::from(x * x)
}

/// `quint_shift`, with the quintic Lagrange weights `Q_M2` ... `Q_P3`.
fn quint_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    let Some((ia, aa)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    let (x, xx) = (f64::from(aa), sq(aa));
    let w = [
        (x * (xx - 1.0) * (2.0 - x) * (x - 3.0) * 0.008333333) as f32, // Q_M2
        (x * (xx - 4.0) * (x - 1.0) * (x - 3.0) * 0.041666667) as f32, // Q_M1
        ((xx - 4.0) * (xx - 1.0) * (3.0 - x) * 0.083333333) as f32,    // Q_00
        (x * (xx - 4.0) * (x + 1.0) * (x - 3.0) * 0.083333333) as f32, // Q_P1
        (x * (xx - 1.0) * (x + 2.0) * (3.0 - x) * 0.041666667) as f32, // Q_P2
        (x * (xx - 1.0) * (xx - 4.0) * 0.008333333) as f32,            // Q_P3
    ];
    // m2, m1, 00, p1, p2, p3 at offsets -2..=3 ; ibot = 2-ia, itop = n-4-ia
    apply(
        f,
        lcbuf,
        ia,
        &Kernel {
            lo: -2,
            w: &w,
            order: &[0, 1, 2, 3, 4, 5],
            ibot0: 2,
            itop0: -4,
        },
    );
}

/// `hept_shift`, with the heptic Lagrange weights `S_M3` ... `S_P4`.
fn hept_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    let Some((ia, aa)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    let (x, xx) = (f64::from(aa), sq(aa));
    // Offsets -3..=4: weights m3, m2, m1, 00, p1, p2, p3, p4.
    let w = [
        (x * (xx - 1.0) * (xx - 4.0) * (x - 3.0) * (4.0 - x) * 0.0001984126984) as f32, // S_M3
        (x * (xx - 1.0) * (x - 2.0) * (xx - 9.0) * (x - 4.0) * 0.001388888889) as f32,  // S_M2
        (x * (x - 1.0) * (xx - 4.0) * (xx - 9.0) * (4.0 - x) * 0.004166666667) as f32,  // S_M1
        ((xx - 1.0) * (xx - 4.0) * (xx - 9.0) * (x - 4.0) * 0.006944444444) as f32,     // S_00
        (x * (x + 1.0) * (xx - 4.0) * (xx - 9.0) * (4.0 - x) * 0.006944444444) as f32,  // S_P1
        (x * (xx - 1.0) * (x + 2.0) * (xx - 9.0) * (x - 4.0) * 0.004166666667) as f32,  // S_P2
        (x * (xx - 1.0) * (xx - 4.0) * (x + 3.0) * (4.0 - x) * 0.001388888889) as f32,  // S_P3
        (x * (xx - 1.0) * (xx - 4.0) * (xx - 9.0) * 0.0001984126984) as f32,            // S_P4
    ];
    // AFNI adds m2, m1, 00, p1, p2, p3, then m3, then p4 ; ibot = 3-ia, itop = n-5-ia
    apply(
        f,
        lcbuf,
        ia,
        &Kernel {
            lo: -3,
            w: &w,
            order: &[1, 2, 3, 4, 5, 6, 0, 7],
            ibot0: 3,
            itop0: -5,
        },
    );
}

/// `PIF`, `3.1415927f`: the float nearest π.
const PIF: f32 = std::f32::consts::PI;

/// `sinc(x)` (float; `x >= 0`): `sinf(PIF*x)/(PIF*x)`, or `1 - 1.6449341*x*x` up to 0.01.
#[inline]
#[allow(clippy::excessive_precision)] // AFNI's literal, rounded to float as C rounds it
fn sinc(x: f32) -> f32 {
    if x > 0.01 {
        sinf(PIF * x) / (PIF * x)
    } else {
        1.0 - 1.644_934_1 * x * x
    }
}

/// `M3(x)`: the minimum-sidelobe 3-term window (float).
#[inline]
fn m3(x: f32) -> f32 {
    0.424_380_1 + 0.497_340_6 * cosf(PIF * x) + 0.078_279_3 * cosf(PIF * x * 2.0)
}

/// The weights of `wsinc5_shift` (`R = 5`, `wwsinc5`: window argument `0.19999*x`) or
/// `wsinc9_shift` (`R = 9`, `wwsinc9`: `0.11111*x`), for the offsets `-(R-1)..=R`: the
/// distance to the data point is `aa + k` for offset `-k` and `k - aa` for offset `k`.
fn wsinc_weights(r: i64, aa: f32) -> Vec<f32> {
    let (radius, scale) = if r == 5 {
        (5.0f32, 0.199_99f32)
    } else {
        (9.0f32, 0.111_11f32)
    };
    let wwsinc = |x: f32| {
        if x <= radius {
            sinc(x) * m3(scale * x)
        } else {
            0.0
        }
    };
    (-(r - 1)..=r)
        .map(|off| {
            let xx = if off <= 0 {
                aa + (-off) as f32
            } else {
                off as f32 - aa
            };
            wwsinc(xx)
        })
        .collect()
}

/// `wsinc5_shift` and `wsinc9_shift`.
fn wsinc_shift(r: i64, af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    // `fabsf(af) < 0.0001f`: little or nothing to do.
    if af.abs() < 0.0001 {
        return;
    }
    let Some((ia, aa)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    let w = wsinc_weights(r, aa);
    let order: Vec<usize> = (0..w.len()).collect();
    // wsinc5: ibot = 5-ia, itop = n-6-ia ; wsinc9: ibot = 9-ia, itop = n-10-ia
    apply(
        f,
        lcbuf,
        ia,
        &Kernel {
            lo: -(r - 1),
            w: &w,
            order: &order,
            ibot0: r,
            itop0: -(r + 1),
        },
    );
}

fn wsinc5_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    wsinc_shift(5, af, f, lcbuf);
}

fn wsinc9_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    wsinc_shift(9, af, f, lcbuf);
}

/// `nn_shift`: `lcbuf[ii] = FINS(ii + ia)`.
fn nn_shift(af: f32, f: &mut [f32]) {
    let Some((ia, _)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    let src = f.to_vec();
    for (ii, out) in f.iter_mut().enumerate() {
        *out = fins(&src, ii as i64 + ia) as f32;
    }
}

/// `ts_shift`: the nearer sample below a fraction of 0.3 or above 0.7, else the mean of the
/// two (`0.5*(f[ix] + f[ix+1])`: a float sum times a double, stored as float).
fn ts_shift(af: f32, f: &mut [f32], lcbuf: &mut Vec<f32>) {
    let n = f.len() as i64;
    let Some((ia, aa)) = split(af, f.len()) else {
        f.fill(0.0);
        return;
    };
    lcbuf.clear();
    lcbuf.resize(f.len(), 0.0);
    let mut ibot = (-ia).max(0);
    let mut itop = (n - 2 - ia).min(n - 1);
    let aa = f64::from(aa);
    if aa < 0.30 || aa > 0.70 {
        let d = i64::from(aa > 0.70);
        for ii in ibot..=itop {
            lcbuf[ii as usize] = f[(ii + ia + d) as usize];
        }
        for ii in (0..ibot).chain(itop + 1..n) {
            lcbuf[ii as usize] = fins(f, ii + ia + d) as f32;
        }
    } else {
        for ii in ibot..=itop {
            let ix = (ii + ia) as usize;
            lcbuf[ii as usize] = (0.5 * f64::from(f[ix] + f[ix + 1])) as f32;
        }
        if ibot > n {
            ibot = n;
        }
        for ii in 0..ibot {
            lcbuf[ii as usize] = (0.5 * (fins(f, ii + ia) + fins(f, ii + ia + 1))) as f32;
        }
        if itop < 0 {
            itop = -1;
        }
        for ii in itop + 1..n {
            lcbuf[ii as usize] = (0.5 * (fins(f, ii + ia) + fins(f, ii + ia + 1))) as f32;
        }
    }
    f.copy_from_slice(lcbuf);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 * 0.37).sin() * 100.0 + 50.0)
            .collect()
    }

    #[test]
    fn integer_shifts_move_samples() {
        // A shift of +2 is `-af = -2`, so `ia = -3` and the fraction is 1 (AFNI's convention for
        // negative whole numbers): the weight of the next sample is 1, the others 0, and the
        // result is the sample two steps back, exactly, for every Lagrange method.
        for method in [
            Method::Linear,
            Method::Cubic,
            Method::Quintic,
            Method::Heptic,
        ] {
            let x = ramp(40);
            let mut f = x.clone();
            let mut work = ShiftWork::default();
            shift_two_rows(method, 0, 2.0, &mut f, 0.0, None, &mut work);
            for ii in 10..30 {
                assert_eq!(f[ii], x[ii - 2], "{method:?} {ii}");
            }
        }
    }

    #[test]
    fn too_large_shifts_give_zeros() {
        let mut work = ShiftWork::default();
        for method in [Method::Fourier, Method::Cubic, Method::Wsinc9, Method::Nn] {
            let mut f = ramp(20);
            let mut g = ramp(20);
            shift_two_rows(method, 24, 25.0, &mut f, 25.0, Some(&mut g), &mut work);
            assert!(f.iter().chain(&g).all(|&v| v == 0.0), "{method:?}");
        }
    }

    #[test]
    fn fourier_shift_of_a_pair_equals_each_row_alone_closely() {
        let mut work = ShiftWork::default();
        let x = ramp(36);
        let y: Vec<f32> = x.iter().map(|v| 200.0 - v).collect();
        let (mut f, mut g) = (x.clone(), y.clone());
        shift_two_rows(
            Method::Fourier,
            40,
            0.3,
            &mut f,
            0.3,
            Some(&mut g),
            &mut work,
        );
        let mut f1 = x.clone();
        shift_two_rows(Method::Fourier, 40, 0.3, &mut f1, 0.3, None, &mut work);
        for (a, b) in f.iter().zip(&f1) {
            assert!((a - b).abs() < 1e-3, "{a} {b}");
        }
        // Zero shift: the row comes back to float precision.
        let mut h = x.clone();
        shift_two_rows(Method::Fourier, 40, 0.0, &mut h, 0.0, None, &mut work);
        for (a, b) in h.iter().zip(&x) {
            assert!((a - b).abs() < 1e-3, "{a} {b}");
        }
    }

    #[test]
    fn weights_sum_to_one() {
        for aa in [0.1f32, 0.25, 0.5, 0.77] {
            let s: f32 = wsinc_weights(9, aa).iter().sum();
            assert!((s - 1.0).abs() < 0.02, "{aa}: {s}");
        }
    }

    #[test]
    fn wsinc_skips_tiny_shifts() {
        let x = ramp(30);
        let mut f = x.clone();
        let mut work = ShiftWork::default();
        shift_two_rows(Method::Wsinc5, 0, 0.00005, &mut f, 0.0, None, &mut work);
        assert_eq!(f, x);
    }
}
