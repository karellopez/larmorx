// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Reference LAPACK 3.10.0's reordering of a complex Schur factorization, `ZTRSEN` with `JOB =
//! 'N'` (no condition numbers) and `COMPQ = 'V'` (the Schur vectors updated), translated from
//! the release's Fortran (`<workspace>/tags/lapack-3.10.0`, BSD-3-Clause: the University of
//! Tennessee, the University of California Berkeley, the University of Colorado Denver and NAG
//! Ltd.) as Ubuntu 22.04's `liblapack.a` and `libblas.a` (3.10.0-2ubuntu1, gfortran 11) compute
//! it: `ZTRSEN` moves each selected eigenvalue up to its place by `ZTREXC`, one swap of
//! neighbouring diagonal elements at a time (`ZLARTG`, the 3.10 Fortran 90 version with its safe
//! scaling, and `ZROT`), for the replicas that reorder a complex Schur form.
//!
//! The complex arithmetic is gfortran's (`crate::cplx`: the plain product, Smith's division, a
//! real promoted to `(x, +0)`); `MAX` and `MIN` are `MAXSD`'s and `MINSD`'s (`a > b ? a : b`,
//! `a < b ? a : b`).

use crate::cplx::C64;

/// `LA_CONSTANTS`' `dsafmin` (`2^-1022`), `dsafmax` (`2^1022`), `drtmin` (`sqrt(dsafmin /
/// dulp)` = `2^-485`) and `drtmax` (`sqrt(dsafmax dulp)` = `2^485`).
const SAFMIN: f64 = f64::MIN_POSITIVE;
const SAFMAX: f64 = 1.0 / f64::MIN_POSITIVE;
const RTMIN: f64 = f64::from_bits(((1023 - 485) as u64) << 52);
const RTMAX: f64 = f64::from_bits(((1023 + 485) as u64) << 52);

#[inline(always)]
fn max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

#[inline(always)]
fn min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// Element `(i, j)` (1-based) of a column-major array with leading dimension `ld`.
#[inline(always)]
fn ix(i: usize, j: usize, ld: usize) -> usize {
    (i - 1) + (j - 1) * ld
}

/// `ABSSQ(t) = real(t)**2 + aimag(t)**2`.
#[inline(always)]
fn abssq(t: C64) -> f64 {
    t.re * t.re + t.im * t.im
}

/// `ZLARTG(F, G, C, S, R)` (LAPACK 3.10.0's `zlartg.f90`): the plane rotation `[C S; -conj(S)
/// C]` with `C` real that zeroes `G`; returns `(C, S, R)`.
pub fn zlartg(f: C64, g: C64) -> (f64, C64, C64) {
    let czero = C64::ZERO;
    if g == czero {
        return (1.0, czero, f);
    }
    if f == czero {
        let g1 = max(g.re.abs(), g.im.abs());
        if g1 > RTMIN && g1 < RTMAX {
            let g2 = abssq(g);
            let d = g2.sqrt();
            let s = g.conj().fdiv_real(d);
            return (0.0, s, C64::real(d));
        }
        let u = min(SAFMAX, max(SAFMIN, g1));
        let uu = 1.0 / u;
        let gs = g.mul(C64::real(uu));
        let g2 = abssq(gs);
        let d = g2.sqrt();
        let s = gs.conj().fdiv_real(d);
        return (0.0, s, C64::real(d * u));
    }
    let f1 = max(f.re.abs(), f.im.abs());
    let g1 = max(g.re.abs(), g.im.abs());
    if f1 > RTMIN && f1 < RTMAX && g1 > RTMIN && g1 < RTMAX {
        let f2 = abssq(f);
        let g2 = abssq(g);
        let h2 = f2 + g2;
        let d = if f2 > RTMIN && h2 < RTMAX {
            (f2 * h2).sqrt()
        } else {
            f2.sqrt() * h2.sqrt()
        };
        let p = 1.0 / d;
        let c = f2 * p;
        let s = g.conj().mul(f.mul(C64::real(p)));
        let r = f.mul(C64::real(h2 * p));
        return (c, s, r);
    }
    let u = min(SAFMAX, max(max(SAFMIN, f1), g1));
    let uu = 1.0 / u;
    let gs = g.mul(C64::real(uu));
    let g2 = abssq(gs);
    let (w, fs, f2, h2);
    if f1 * uu < RTMIN {
        let v = min(SAFMAX, max(SAFMIN, f1));
        let vv = 1.0 / v;
        w = v * uu;
        fs = f.mul(C64::real(vv));
        f2 = abssq(fs);
        h2 = f2 * (w * w) + g2;
    } else {
        w = 1.0;
        fs = f.mul(C64::real(uu));
        f2 = abssq(fs);
        h2 = f2 + g2;
    }
    let d = if f2 > RTMIN && h2 < RTMAX {
        (f2 * h2).sqrt()
    } else {
        f2.sqrt() * h2.sqrt()
    };
    let p = 1.0 / d;
    let c = (f2 * p) * w;
    let s = gs.conj().mul(fs.mul(C64::real(p)));
    let r = fs.mul(C64::real(h2 * p)).mul(C64::real(u));
    (c, s, r)
}

/// `ZROT(N, CX, INCX, CY, INCY, C, S)`: `cx = c cx + s cy`, `cy = c cy - conj(s) cx` (`c` real,
/// promoted), element `k` of each at `x_off + k incx`, `y_off + k incy`.
#[allow(clippy::too_many_arguments)]
fn zrot(
    n: usize,
    a: &mut [C64],
    x_off: usize,
    incx: usize,
    y_off: usize,
    incy: usize,
    c: f64,
    s: C64,
) {
    let cc = C64::real(c);
    for k in 0..n {
        let (xi, yi) = (x_off + k * incx, y_off + k * incy);
        let stemp = cc.mul(a[xi]).add(s.mul(a[yi]));
        a[yi] = cc.mul(a[yi]).sub(s.conj().mul(a[xi]));
        a[xi] = stemp;
    }
}

/// `ZTREXC('V', N, T, N, Q, N, IFST, ILST, INFO)` on the column-major `n x n` upper triangular
/// `t` and `q`: the diagonal element at `ifst` moved to `ilst` (1-based) by swaps of neighbours.
pub fn ztrexc(n: usize, t: &mut [C64], q: &mut [C64], ifst: usize, ilst: usize) {
    if n <= 1 || ifst == ilst {
        return;
    }
    let ks: Vec<usize> = if ifst < ilst {
        (ifst..ilst).collect()
    } else {
        (ilst..ifst).rev().collect()
    };
    for k in ks {
        let t11 = t[ix(k, k, n)];
        let t22 = t[ix(k + 1, k + 1, n)];
        let (cs, sn, _temp) = zlartg(t[ix(k, k + 1, n)], t22.sub(t11));
        if k + 2 <= n {
            zrot(
                n - k - 1,
                t,
                ix(k, k + 2, n),
                n,
                ix(k + 1, k + 2, n),
                n,
                cs,
                sn,
            );
        }
        zrot(k - 1, t, ix(1, k, n), 1, ix(1, k + 1, n), 1, cs, sn.conj());
        t[ix(k, k, n)] = t22;
        t[ix(k + 1, k + 1, n)] = t11;
        zrot(n, q, ix(1, k, n), 1, ix(1, k + 1, n), 1, cs, sn.conj());
    }
}

/// `ZTRSEN('N', 'V', SELECT, N, T, N, Q, N, W, M, S, SEP, WORK, LWORK, INFO)` on the
/// column-major `n x n` `t` (upper triangular) and `q`: the selected eigenvalues (`select[k]`
/// nonzero) moved to the leading positions in their order; returns `W` (`T`'s diagonal after) and
/// `M` (the number selected). With `JOB = 'N'` and a valid call (`LWORK >= 1`) `INFO` is 0.
pub fn ztrsen_nv(n: usize, select: &[i32], t: &mut [C64], q: &mut [C64]) -> (Vec<C64>, usize) {
    let m = select[..n].iter().filter(|&&s| s != 0).count();
    if !(m == n || m == 0) {
        let mut ks = 0;
        for k in 1..=n {
            if select[k - 1] != 0 {
                ks += 1;
                if k != ks {
                    ztrexc(n, t, q, k, ks);
                }
            }
        }
    }
    let w = (1..=n).map(|k| t[ix(k, k, n)]).collect();
    (w, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        assert_eq!(RTMIN, f64::from_bits(((1023 - 485) as u64) << 52));
        assert_eq!(RTMAX, f64::from_bits(((1023 + 485) as u64) << 52));
        assert_eq!(SAFMAX, f64::from_bits(((1023 + 1022) as u64) << 52));
    }

    /// Reordering keeps `Q T Q^H` and moves the selected eigenvalue first.
    #[test]
    fn reorders() {
        let n = 3;
        // column-major upper triangular T, Q the identity
        let mut t = vec![C64::ZERO; 9];
        let set = |t: &mut Vec<C64>, i: usize, j: usize, re: f64, im: f64| {
            t[ix(i, j, n)] = C64::new(re, im)
        };
        set(&mut t, 1, 1, 1.0, 0.5);
        set(&mut t, 2, 2, 2.0, -0.25);
        set(&mut t, 3, 3, 0.5, 0.0);
        set(&mut t, 1, 2, 0.3, 0.1);
        set(&mut t, 1, 3, -0.2, 0.4);
        set(&mut t, 2, 3, 0.7, -0.6);
        let mut q = vec![C64::ZERO; 9];
        for i in 1..=n {
            q[ix(i, i, n)] = C64::ONE;
        }
        let (w, m) = ztrsen_nv(n, &[0, 0, 1], &mut t, &mut q);
        assert_eq!(m, 1);
        assert_eq!(w[0], C64::new(0.5, 0.0));
        assert!(t[ix(2, 1, n)] == C64::ZERO && t[ix(3, 1, n)] == C64::ZERO);
    }
}
