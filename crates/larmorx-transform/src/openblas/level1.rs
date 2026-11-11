// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! OpenBLAS's level-1 BLAS on the Haswell kernels at one thread (OpenBLAS 0.3.30 and
//! v0.3.34-106-g446c436e: the same kernel files; the interfaces differ only in their threading
//! and export attributes), any length and any increment, on every platform: the vector
//! operations LAPACK's blocked routines make (`DAXPY`, `DSCAL`, `DCOPY`, `DSWAP`, `IDAMAX`,
//! `DNRM2`) and the kernels the level-2 drivers call (`AXPYU_K`, [`axpy_k`]).
//!
//! A vector is addressed as BLAS does: `n` elements from `off` with increment `inc`; with a
//! negative increment the first element is the last in memory (`x(1)` at `off + (n-1)·|inc|`),
//! as the interfaces' `x -= (n - 1)·incx` makes it.
//!
//! - [`axpy_k`] (`kernel/x86_64/daxpy.c`, `daxpy_microk_haswell-2.c`): unit increments: the
//!   first `16·⌊n/16⌋` elements `y = fma(a, x, y)` (the microkernel), the rest
//!   `y + round(a·x)`; any other increments: `y + round(a·x)` in order (the C loop, compiled
//!   without fused multiply-adds).
//! - [`daxpy`] (`interface/axpy.c`): nothing for `n ≤ 0` or `a = 0`; both increments 0:
//!   `y += (n·a)·x` once; else [`axpy_k`].
//! - [`dscal`] (`interface/scal.c`, `kernel/x86_64/dscal.c`, `dscal_microk_haswell-2.c`):
//!   nothing for `n ≤ 0`, `inc ≤ 0` or `a = 1`; otherwise `x·a` for every element, `a = 0`
//!   included (the interface's call multiplies, so `0·NaN` and `0·∞` are NaN and `0·(-x)` is
//!   -0; OpenBLAS's internal calls store +0 instead, which larmorx's drivers do themselves).
//! - [`dcopy`], [`dswap`] (`interface/copy.c`, `swap.c`): no arithmetic; elements in order.
//! - [`idamax`] (`interface/imax.c`, `kernel/x86_64/iamax_sse2.S`): the kernel's search as
//!   written: the largest magnitude by `maxpd` into four two-lane accumulators started from
//!   `|x(1)|` (the lane order below; x86's `MAX(d, s)` gives `s` when either is NaN), reduced
//!   pairwise, then the first element equal to it, found by `cmpeqpd` on blocks of eight
//!   (ordered: NaN never matches) and `comisd` on single elements (unordered: NaN always
//!   matches), the eighth of a matching block and the last of the whole taken unseen. For
//!   finite input this is the first index of the largest magnitude; with NaN the answer also
//!   depends on whether `x(1)` lies on a 16-byte boundary (`aligned`), as in OpenBLAS.
//! - [`dnrm2`] (`interface/nrm2.c`, `kernel/x86_64/nrm2.S`): `|x(1)|` for one element,
//!   `√n·|x(1)|` for increment 0, else the x87 kernel (`numpy_linalg`'s port).
//!
//! Upstream files (BSD-3-Clause; the notice is in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `interface/axpy.c`, `scal.c`, `copy.c`, `swap.c`, `imax.c`,
//! `nrm2.c`, `kernel/x86_64/daxpy.c`, `daxpy_microk_haswell-2.c`, `dscal.c`,
//! `dscal_microk_haswell-2.c`, `iamax_sse2.S`, `nrm2.S`, `KERNEL.HASWELL`, `KERNEL`. The logic
//! is re-expressed in Rust; no code was copied.

/// The memory position of element `t` of a BLAS vector of `n` elements from `off` with
/// increment `inc`.
#[inline(always)]
pub fn at(n: usize, off: usize, inc: isize, t: usize) -> usize {
    if inc >= 0 {
        off + t * inc as usize
    } else {
        off + (n - 1 - t) * inc.unsigned_abs()
    }
}

/// `AXPYU_K(n, a, x, incx, y, incy)`: `y ← a·x + y`, the kernel (no test of `a`; module
/// documentation).
#[allow(clippy::too_many_arguments)]
pub fn axpy_k(
    n: usize,
    a: f64,
    x: &[f64],
    xoff: usize,
    incx: isize,
    y: &mut [f64],
    yoff: usize,
    incy: isize,
) {
    if n == 0 {
        return;
    }
    if incx == 1 && incy == 1 {
        let n1 = n & !15;
        let (xs, ys) = (&x[xoff..xoff + n], &mut y[yoff..yoff + n]);
        for (yi, &xi) in ys[..n1].iter_mut().zip(&xs[..n1]) {
            *yi = a.mul_add(xi, *yi);
        }
        for (yi, &xi) in ys[n1..].iter_mut().zip(&xs[n1..]) {
            let p = a * xi;
            *yi += p;
        }
        return;
    }
    for t in 0..n {
        let p = a * x[at(n, xoff, incx, t)];
        y[at(n, yoff, incy, t)] += p;
    }
}

/// `DAXPY(N, A, X, INCX, Y, INCY)` (module documentation).
#[allow(clippy::too_many_arguments)]
pub fn daxpy(
    n: usize,
    a: f64,
    x: &[f64],
    xoff: usize,
    incx: isize,
    y: &mut [f64],
    yoff: usize,
    incy: isize,
) {
    if n == 0 || a == 0.0 {
        return;
    }
    if incx == 0 && incy == 0 {
        let s = (n as f64) * a;
        let p = s * x[xoff];
        y[yoff] += p;
        return;
    }
    axpy_k(n, a, x, xoff, incx, y, yoff, incy);
}

/// `DSCAL(N, A, X, INCX)` (module documentation).
pub fn dscal(n: usize, a: f64, x: &mut [f64], off: usize, inc: isize) {
    if n == 0 || inc <= 0 || a == 1.0 {
        return;
    }
    for t in 0..n {
        x[off + t * inc as usize] *= a;
    }
}

/// `DCOPY(N, X, INCX, Y, INCY)`.
#[allow(clippy::too_many_arguments)]
pub fn dcopy(
    n: usize,
    x: &[f64],
    xoff: usize,
    incx: isize,
    y: &mut [f64],
    yoff: usize,
    incy: isize,
) {
    for t in 0..n {
        y[at(n, yoff, incy, t)] = x[at(n, xoff, incx, t)];
    }
}

/// `DSWAP(N, X, INCX, Y, INCY)` of two separate vectors, element by element in order.
#[allow(clippy::too_many_arguments)]
pub fn dswap(
    n: usize,
    x: &mut [f64],
    xoff: usize,
    incx: isize,
    y: &mut [f64],
    yoff: usize,
    incy: isize,
) {
    for t in 0..n {
        std::mem::swap(&mut x[at(n, xoff, incx, t)], &mut y[at(n, yoff, incy, t)]);
    }
}

/// `DNRM2(N, X, INCX)` (module documentation).
pub fn dnrm2(n: usize, x: &[f64], off: usize, inc: isize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return x[off].abs();
    }
    if inc == 0 {
        return (n as f64).sqrt() * x[off].abs();
    }
    if inc > 0 {
        return crate::numpy_linalg::dnrm2(n, x, off, inc as usize);
    }
    let v: Vec<f64> = (0..n).map(|t| x[at(n, off, inc, t)]).collect();
    crate::numpy_linalg::dnrm2(n, &v, 0, 1)
}

/// x86's `MAXPD`/`MAXSD` lane: `d` if `d > s`, else `s` (so a NaN in either gives `s`).
#[inline(always)]
fn mx(d: f64, s: f64) -> f64 {
    if d > s { d } else { s }
}

/// `comisd` + `je`: equal, or unordered.
#[inline(always)]
fn comisd_eq(v: f64, m: f64) -> bool {
    v == m || v.is_nan() || m.is_nan()
}

/// The four two-lane accumulators of `iamax_sse2.S`.
type Acc = [[f64; 2]; 4];

/// The kernel's reduction (`.L20`, `.L60`, `.L90`): `xmm0 = max(xmm0, xmm1)`, `xmm2 =
/// max(xmm2, xmm3)`, `xmm0 = max(xmm0, xmm2)`, `xmm1 = xmm0`, the high lane against the low
/// one. Returns the new registers (`xmm0` with the result in its low lane).
fn reduce(acc: &mut Acc) {
    let lanes = |d: [f64; 2], s: [f64; 2]| [mx(d[0], s[0]), mx(d[1], s[1])];
    acc[0] = lanes(acc[0], acc[1]);
    acc[2] = lanes(acc[2], acc[3]);
    acc[0] = lanes(acc[0], acc[2]);
    acc[1] = acc[0];
    // unpckhpd xmm0, xmm0; maxsd xmm1, xmm0.
    let hi = acc[0][1];
    acc[0] = [mx(hi, acc[1][0]), hi];
}

/// `IDAMAX(N, X, INCX)`: the 1-based index of the kernel's largest magnitude (module
/// documentation), 0 for `n = 0` or `inc ≤ 0`. `aligned` says whether `x(1)` lies on a
/// 16-byte boundary (only NaN input can tell).
pub fn idamax(n: usize, x: &[f64], off: usize, inc: isize, aligned: bool) -> usize {
    if n == 0 || inc <= 0 {
        return 0;
    }
    let inc = inc as usize;
    let ab = |t: usize| x[off + t * inc].abs();
    let first = ab(0);
    let mut acc: Acc = [[first; 2]; 4];
    let mut p = 1;
    let mut m = n - 1;
    let unit = inc == 1;
    // The aligned mode of the unit-stride path: more than 7 elements after the first.
    let aligned_mode = unit && m > 7;
    if aligned_mode && aligned {
        // X (element 2) is not on a 16-byte boundary: one element into xmm3 first.
        let v = ab(p);
        acc[3] = [mx(acc[3][0], v), mx(acc[3][1], v)];
        p += 1;
        m -= 1;
    }
    let fold = |acc: &mut Acc, reg: usize, p: usize| {
        acc[reg][0] = mx(acc[reg][0], ab(p));
        acc[reg][1] = mx(acc[reg][1], ab(p + 1));
    };
    for _ in 0..m >> 4 {
        for blk in 0..8 {
            fold(&mut acc, blk % 4, p + 2 * blk);
        }
        p += 16;
    }
    let r = m & 15;
    if r & 8 != 0 {
        for blk in 0..4 {
            fold(&mut acc, blk, p + 2 * blk);
        }
        p += 8;
    }
    if r & 4 != 0 {
        fold(&mut acc, 0, p);
        fold(&mut acc, 1, p + 2);
        p += 4;
    }
    if r & 2 != 0 {
        fold(&mut acc, 2, p);
        p += 2;
    }
    if r & 1 != 0 {
        let v = ab(p);
        acc[3] = [mx(acc[3][0], v), mx(acc[3][1], v)];
        if !unit {
            // .L88: the strided path reduces here once more before .L90.
            reduce(&mut acc);
        }
    }
    reduce(&mut acc);
    let max = acc[0][0];
    // The search, from element 1 again.
    let mut ret = 0usize;
    let mut q = 0usize;
    let mut left = n;
    if aligned_mode && !aligned {
        ret += 1;
        if comisd_eq(ab(0), max) {
            return ret.min(n);
        }
        q = 1;
        left -= 1;
    }
    let scan = |q: usize, count: usize, ret: &mut usize| -> bool {
        for t in 0..count {
            *ret += 1;
            if comisd_eq(ab(q + t), max) {
                return true;
            }
        }
        false
    };
    for _ in 0..left >> 3 {
        if (0..8).any(|t| ab(q + t) == max) {
            if !scan(q, 7, &mut ret) {
                ret += 1;
            }
            return ret.min(n);
        }
        q += 8;
        ret += 8;
    }
    if left & 4 != 0 {
        if scan(q, 4, &mut ret) {
            return ret.min(n);
        }
        q += 4;
    }
    if left & 2 != 0 && scan(q, 2, &mut ret) {
        return ret.min(n);
    }
    (ret + 1).min(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idamax_finds_the_first_largest_magnitude() {
        for n in [1usize, 2, 3, 7, 8, 9, 16, 17, 40, 1001] {
            let x: Vec<f64> = (0..n).map(|i| ((i * 37 % 101) as f64) - 50.0).collect();
            let want = 1 + (0..n).fold(0, |b, i| if x[i].abs() > x[b].abs() { i } else { b });
            for aligned in [true, false] {
                assert_eq!(idamax(n, &x, 0, 1, aligned), want, "n = {n}");
            }
            let y: Vec<f64> = x.iter().flat_map(|&v| [v, 9e9]).collect();
            assert_eq!(idamax(n, &y, 0, 2, true), want, "n = {n}, stride 2");
        }
        assert_eq!(idamax(0, &[], 0, 1, true), 0);
        assert_eq!(idamax(3, &[1.0, 2.0, 3.0], 0, -1, true), 0);
    }

    #[test]
    fn axpy_and_scal_follow_the_kernels() {
        let x: Vec<f64> = (0..37).map(|i| i as f64 * 0.1 + 0.3).collect();
        let mut y = vec![1.0f64; 37];
        daxpy(37, 0.7, &x, 0, 1, &mut y, 0, 1);
        for i in 0..37 {
            let want = if i < 32 {
                0.7f64.mul_add(x[i], 1.0)
            } else {
                1.0 + 0.7 * x[i]
            };
            assert_eq!(y[i].to_bits(), want.to_bits());
        }
        let mut z = vec![-1.0, f64::NAN, 2.0];
        dscal(3, 0.0, &mut z, 0, 1);
        assert_eq!(z[0].to_bits(), (-0.0f64).to_bits());
        assert!(z[1].is_nan());
        let mut w = vec![1.0, 2.0, 3.0, 4.0];
        let mut v = vec![5.0, 6.0];
        dswap(2, &mut w, 0, -2, &mut v, 0, 1);
        assert_eq!((w, v), (vec![6.0, 2.0, 5.0, 4.0], vec![3.0, 1.0]));
    }
}
