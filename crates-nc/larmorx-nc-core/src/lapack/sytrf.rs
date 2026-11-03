// SPDX-License-Identifier: LicenseRef-FSL AND BSD-3-Clause
// Translated from the reference LAPACK 3.12.0 bundled with OpenBLAS 0.3.33 (lapack-netlib;
// modified BSD licence, Copyright (c) 1992-2023 The University of Tennessee and The University
// of Tennessee Research Foundation, 2000-2023 The University of California Berkeley, 2006-2023
// The University of Colorado Denver) and OpenBLAS 0.3.33 (BSD-3-Clause; Copyright (c) 2011-2014,
// The OpenBLAS Project); both notices are in ../lapack.rs and ../../NOTICE. Part of larmorx-nc
// (FSL Licence terms; see LICENSE).
//! LAPACK's symmetric indefinite factorisation and solve with the lower triangle, `dsytrf('L')`
//! and `dsytrs('L')`, as FSL 6.0.7.23's OpenBLAS 0.3.33 computes them: the call sequence of
//! Armadillo 14.2.1's `auxlib::solve_sym_fast` (`inv(A) * B` for a symmetric `A` of at least 100
//! rows: [`crate::arma::solve`]).
//!
//! OpenBLAS has no `dsytrf` of its own: the routines are reference LAPACK 3.12.0's Fortran
//! (`SRC/dsytrf.f`, `dlasyf.f`, `dsytf2.f`, `dsytrs.f`, `ilaenv.f`), compiled by gfortran into
//! plain scalar SSE2 (no fused multiply-adds, no vectorised arithmetic: FSL's
//! `libopenblasp-r0.3.33.so`, disassembled), so their own arithmetic rounds each operation in
//! the order the Fortran writes it. They call OpenBLAS's BLAS on its Haswell kernels ([`super::blas`]):
//! `dgemv('N')` with a strided `x` (the workspace's rows), `dgemv('T')` with a strided `y` (a
//! row of `B`), `dgemm('N', 'T')`, `dger` and `dsyr` (each column by `daxpy`'s kernel),
//! `dscal`, `idamax`; `dcopy` and `dswap` move values unchanged.
//!
//! Bunch-Kaufman pivoting (`α = (1 + √17)/8`): 1 × 1 and 2 × 2 pivots, `ipiv` 1-based (a 2 × 2
//! pivot's two entries both `−kp`). `dsytrf` takes its block size from `ILAENV(1, 'DSYTRF')`
//! (64; the minimum `ILAENV(2, ...)` is 8): with the workspace Armadillo gives (`n · 64`
//! elements after the workspace query, at least 16) a matrix of more than 64 rows is factorised
//! in panels of 63 or 64 columns by `dlasyf` (the panel's columns into the workspace `W`, the
//! trailing lower triangle updated by `dgemv` on the diagonal blocks and `dgemm` below them),
//! and the last 64 or fewer columns (or the whole of a smaller matrix) by the unblocked
//! `dsytf2` (rank-one updates by `dsyr`, rank-two by its own loop). `dsytrs` solves `L·D·Lᵀ X =
//! B` column-oriented: the interchanges and `L⁻¹` by `dger`, `D⁻¹` by `dscal` or the 2 × 2
//! formula, then `L⁻ᵀ` by `dgemv('T')` and the interchanges backwards.
//!
//! The pivots follow the comparisons of finite values; NaN inputs (which the symmetric path of
//! FSL's callers never meets: their matrices are spline normal equations) are not modelled
//! (`idamax`'s NaN rules, Fortran's `MAX`). The logic is re-expressed in Rust; no code was
//! copied.

use super::blas::{MatRef, dgemm, dgemv_n, dgemv_t_inc, dger, dscal, dsyr_l};
use super::blocked::iamax_any;

/// `ILAENV(1, 'DSYTRF', ...)`: the block size.
pub const SYTRF_NB: usize = 64;
/// `ILAENV(2, 'DSYTRF', ...)`: the smallest block size worth blocking with a short workspace.
const SYTRF_NBMIN: usize = 8;

/// `dsytrf`'s workspace query (`lwork = −1`): `max(1, n · NB)`.
pub fn sytrf_query(n: usize) -> usize {
    (n * SYTRF_NB).max(1)
}

/// The workspace Armadillo 14.2.1's `solve_sym_fast` passes: `max(16, n)`, raised to the
/// query's answer when `n` exceeds 16.
pub fn armadillo_lwork(n: usize) -> usize {
    let base = n.max(16);
    if n > 16 {
        sytrf_query(n).max(base)
    } else {
        base
    }
}

/// `IDAMAX` (1-based) of a gathered vector.
#[inline]
fn idamax(x: &[f64]) -> usize {
    iamax_any(x)
}

/// `dsytrf('L')` of the column-major `n × n` array `a` (leading dimension `lda`; only the lower
/// triangle is read and written) with a workspace of `lwork` elements: `L` and `D` in place,
/// the pivots in `ipiv` (1-based). Returns `info`: 0, or the first `k` whose pivot `D(k, k)` is
/// exactly zero (the factorisation is completed, `D` singular).
pub fn sytrf_l(a: &mut [f64], n: usize, lda: usize, ipiv: &mut [i32], lwork: usize) -> i32 {
    assert!(lda >= n.max(1), "dsytrf: LDA must be at least max(1, N)");
    assert!(lwork >= 1, "dsytrf: LWORK must be at least 1");
    assert!(ipiv.len() >= n);
    let ldwork = n;
    let mut nb = SYTRF_NB;
    let mut nbmin = 2;
    if nb > 1 && nb < n {
        let iws = ldwork * nb;
        if lwork < iws {
            nb = (lwork / ldwork).max(1);
            nbmin = nbmin.max(SYTRF_NBMIN);
        }
    }
    if nb < nbmin {
        nb = n;
    }
    let mut work = if nb < n {
        vec![0.0f64; ldwork * nb]
    } else {
        Vec::new()
    };
    let mut info = 0i32;
    let mut k = 1usize;
    while k <= n {
        let off = (k - 1) * lda + (k - 1);
        let (kb, iinfo) = if k + nb <= n {
            dlasyf_l(
                a,
                lda,
                off,
                n - k + 1,
                nb,
                &mut ipiv[k - 1..],
                &mut work,
                ldwork,
            )
        } else {
            let iinfo = dsytf2_l(a, lda, off, n - k + 1, &mut ipiv[k - 1..]);
            (n - k + 1, iinfo)
        };
        if info == 0 && iinfo > 0 {
            info = iinfo + k as i32 - 1;
        }
        let shift = (k - 1) as i32;
        for p in &mut ipiv[k - 1..k - 1 + kb] {
            if *p > 0 {
                *p += shift;
            } else {
                *p -= shift;
            }
        }
        k += kb;
    }
    info
}

/// `dlasyf('L')` on the `n × n` trailing matrix at `off` of `a` (leading dimension `lda`): up to
/// `nb − 1` columns factorised (one more when a 2 × 2 pivot closes the panel) with the workspace
/// `w` (leading dimension `ldw`), the rest of the lower triangle updated. Returns `(kb, info)`.
#[allow(clippy::too_many_arguments)]
fn dlasyf_l(
    a: &mut [f64],
    lda: usize,
    off: usize,
    n: usize,
    nb: usize,
    ipiv: &mut [i32],
    w: &mut [f64],
    ldw: usize,
) -> (usize, i32) {
    let ai = move |i: usize, j: usize| off + (j - 1) * lda + (i - 1);
    let wi = move |i: usize, j: usize| (j - 1) * ldw + (i - 1);
    let alpha = (1.0 + 17f64.sqrt()) / 8.0;
    let mut info = 0i32;
    // `y(k..=n, col) −= A(k..=n, 1..k)·W(row, 1..k)ᵀ` with `y` column `col` of `W`: DGEMV('N')
    // with the strided row `row` of `W` as `x`.
    let gemv_w = |a: &[f64], w: &mut [f64], k: usize, row: usize, col: usize| {
        if k <= 1 {
            return;
        }
        let x: Vec<f64> = (1..k).map(|j| w[wi(row, j)]).collect();
        let s = wi(k, col);
        dgemv_n(
            n - k + 1,
            k - 1,
            -1.0,
            a,
            MatRef {
                off: ai(k, 1),
                ld: lda,
            },
            &x,
            ldw,
            1.0,
            &mut w[s..s + n - k + 1],
        );
    };
    let mut k = 1usize;
    loop {
        if (k >= nb && nb < n) || k > n {
            break;
        }
        // Column k of the trailing matrix into column k of W, updated.
        for i in k..=n {
            w[wi(i, k)] = a[ai(i, k)];
        }
        gemv_w(a, w, k, k, k);
        let mut kstep = 1usize;
        let absakk = w[wi(k, k)].abs();
        let (imax, colmax) = if k < n {
            let im = k + idamax(&w[wi(k + 1, k)..=wi(n, k)]);
            (im, w[wi(im, k)].abs())
        } else {
            (0, 0.0)
        };
        let kp;
        if absakk.max(colmax) == 0.0 {
            // Column k is zero or underflow: info set, A's column left as it was.
            if info == 0 {
                info = k as i32;
            }
            kp = k;
        } else {
            if absakk >= alpha * colmax {
                kp = k;
            } else {
                // Column imax into column k + 1 of W, updated.
                for t in 0..imax - k {
                    w[wi(k + t, k + 1)] = a[ai(imax, k + t)];
                }
                for i in imax..=n {
                    w[wi(i, k + 1)] = a[ai(i, imax)];
                }
                gemv_w(a, w, k, imax, k + 1);
                let mut jmax = k - 1 + idamax(&w[wi(k, k + 1)..wi(imax, k + 1)]);
                let mut rowmax = w[wi(jmax, k + 1)].abs();
                if imax < n {
                    jmax = imax + idamax(&w[wi(imax + 1, k + 1)..=wi(n, k + 1)]);
                    rowmax = rowmax.max(w[wi(jmax, k + 1)].abs());
                }
                if absakk >= alpha * colmax * (colmax / rowmax) {
                    kp = k;
                } else if w[wi(imax, k + 1)].abs() >= alpha * rowmax {
                    kp = imax;
                    for i in k..=n {
                        w[wi(i, k)] = w[wi(i, k + 1)];
                    }
                } else {
                    kp = imax;
                    kstep = 2;
                }
            }
            let kk = k + kstep - 1;
            if kp != kk {
                a[ai(kp, kp)] = a[ai(kk, kk)];
                for t in 0..kp - kk - 1 {
                    a[ai(kp, kk + 1 + t)] = a[ai(kk + 1 + t, kk)];
                }
                if kp < n {
                    for i in kp + 1..=n {
                        a[ai(i, kp)] = a[ai(i, kk)];
                    }
                }
                for j in 1..k {
                    a.swap(ai(kk, j), ai(kp, j));
                }
                for j in 1..=kk {
                    w.swap(wi(kk, j), wi(kp, j));
                }
            }
            if kstep == 1 {
                for i in k..=n {
                    a[ai(i, k)] = w[wi(i, k)];
                }
                if k < n {
                    let r1 = 1.0 / a[ai(k, k)];
                    let s = ai(k + 1, k);
                    dscal(r1, &mut a[s..s + n - k]);
                }
            } else {
                if k + 1 < n {
                    let mut d21 = w[wi(k + 1, k)];
                    let d11 = w[wi(k + 1, k + 1)] / d21;
                    let d22 = w[wi(k, k)] / d21;
                    let t = 1.0 / (d11 * d22 - 1.0);
                    d21 = t / d21;
                    for j in k + 2..=n {
                        a[ai(j, k)] = d21 * (d11 * w[wi(j, k)] - w[wi(j, k + 1)]);
                        a[ai(j, k + 1)] = d21 * (d22 * w[wi(j, k + 1)] - w[wi(j, k)]);
                    }
                }
                a[ai(k, k)] = w[wi(k, k)];
                a[ai(k + 1, k)] = w[wi(k + 1, k)];
                a[ai(k + 1, k + 1)] = w[wi(k + 1, k + 1)];
            }
        }
        if kstep == 1 {
            ipiv[k - 1] = kp as i32;
        } else {
            ipiv[k - 1] = -(kp as i32);
            ipiv[k] = -(kp as i32);
        }
        k += kstep;
    }
    // The trailing lower triangle: A22 −= L21·D·L21ᵀ, the diagonal blocks of nb columns by
    // DGEMV column by column, the blocks below them by DGEMM('N', 'T').
    let mut j = k;
    while j <= n {
        let jb = nb.min(n - j + 1);
        for jj in j..j + jb {
            let rows = j + jb - jj;
            if k > 1 {
                let x: Vec<f64> = (1..k).map(|c| w[wi(jj, c)]).collect();
                let s = ai(jj, jj);
                let mut y: Vec<f64> = a[s..s + rows].to_vec();
                dgemv_n(
                    rows,
                    k - 1,
                    -1.0,
                    a,
                    MatRef {
                        off: ai(jj, 1),
                        ld: lda,
                    },
                    &x,
                    ldw,
                    1.0,
                    &mut y,
                );
                a[s..s + rows].copy_from_slice(&y);
            }
        }
        if j + jb <= n {
            let m = n - j - jb + 1;
            let kk = k - 1;
            // A(j+jb.., 1..k) as its own array (the product reads columns 1..k−1 and writes
            // columns j.., which never overlap).
            let mut l21 = vec![0.0f64; m * kk];
            for c in 0..kk {
                for r in 0..m {
                    l21[c * m + r] = a[ai(j + jb + r, 1 + c)];
                }
            }
            let cs = ai(j + jb, j);
            dgemm(
                false,
                true,
                m,
                jb,
                kk,
                -1.0,
                &l21,
                MatRef { off: 0, ld: m },
                w,
                MatRef {
                    off: wi(j, 1),
                    ld: ldw,
                },
                1.0,
                a,
                MatRef { off: cs, ld: lda },
            );
        }
        j += nb;
    }
    // The interchanges of the panel applied to the columns before it.
    let mut j = k as i64 - 1;
    loop {
        let jj = j;
        let mut jp = ipiv[(j - 1) as usize] as i64;
        if jp < 0 {
            jp = -jp;
            j -= 1;
        }
        j -= 1;
        if jp != jj && j >= 1 {
            for c in 1..=j as usize {
                a.swap(ai(jp as usize, c), ai(jj as usize, c));
            }
        }
        if j <= 1 {
            break;
        }
    }
    (k - 1, info)
}

/// `dsytf2('L')` (unblocked) on the `n × n` trailing matrix at `off` of `a` (leading dimension
/// `lda`). Returns `info`.
fn dsytf2_l(a: &mut [f64], lda: usize, off: usize, n: usize, ipiv: &mut [i32]) -> i32 {
    let ai = move |i: usize, j: usize| off + (j - 1) * lda + (i - 1);
    let alpha = (1.0 + 17f64.sqrt()) / 8.0;
    let mut info = 0i32;
    let mut k = 1usize;
    while k <= n {
        let mut kstep = 1usize;
        let absakk = a[ai(k, k)].abs();
        let (imax, colmax) = if k < n {
            let im = k + idamax(&a[ai(k + 1, k)..=ai(n, k)]);
            (im, a[ai(im, k)].abs())
        } else {
            (0, 0.0)
        };
        let kp;
        if absakk.max(colmax) == 0.0 || absakk.is_nan() {
            if info == 0 {
                info = k as i32;
            }
            kp = k;
        } else {
            if absakk >= alpha * colmax {
                kp = k;
            } else {
                // The largest off-diagonal element of row imax: A(imax, k..imax) (strided) and
                // A(imax+1.., imax).
                let row: Vec<f64> = (k..imax).map(|c| a[ai(imax, c)]).collect();
                let mut jmax = k - 1 + idamax(&row);
                let mut rowmax = a[ai(imax, jmax)].abs();
                if imax < n {
                    jmax = imax + idamax(&a[ai(imax + 1, imax)..=ai(n, imax)]);
                    rowmax = rowmax.max(a[ai(jmax, imax)].abs());
                }
                if absakk >= alpha * colmax * (colmax / rowmax) {
                    kp = k;
                } else if a[ai(imax, imax)].abs() >= alpha * rowmax {
                    kp = imax;
                } else {
                    kp = imax;
                    kstep = 2;
                }
            }
            let kk = k + kstep - 1;
            if kp != kk {
                if kp < n {
                    for i in kp + 1..=n {
                        a.swap(ai(i, kk), ai(i, kp));
                    }
                }
                for t in 0..kp - kk - 1 {
                    a.swap(ai(kk + 1 + t, kk), ai(kp, kk + 1 + t));
                }
                a.swap(ai(kk, kk), ai(kp, kp));
                if kstep == 2 {
                    a.swap(ai(k + 1, k), ai(kp, k));
                }
            }
            if kstep == 1 {
                if k < n {
                    let d11 = 1.0 / a[ai(k, k)];
                    let x: Vec<f64> = a[ai(k + 1, k)..=ai(n, k)].to_vec();
                    dsyr_l(
                        -d11,
                        &x,
                        a,
                        MatRef {
                            off: ai(k + 1, k + 1),
                            ld: lda,
                        },
                    );
                    let s = ai(k + 1, k);
                    dscal(d11, &mut a[s..s + n - k]);
                }
            } else if k + 1 < n {
                let mut d21 = a[ai(k + 1, k)];
                let d11 = a[ai(k + 1, k + 1)] / d21;
                let d22 = a[ai(k, k)] / d21;
                let t = 1.0 / (d11 * d22 - 1.0);
                d21 = t / d21;
                for j in k + 2..=n {
                    let wk = d21 * (d11 * a[ai(j, k)] - a[ai(j, k + 1)]);
                    let wkp1 = d21 * (d22 * a[ai(j, k + 1)] - a[ai(j, k)]);
                    for i in j..=n {
                        a[ai(i, j)] = a[ai(i, j)] - a[ai(i, k)] * wk - a[ai(i, k + 1)] * wkp1;
                    }
                    a[ai(j, k)] = wk;
                    a[ai(j, k + 1)] = wkp1;
                }
            }
        }
        if kstep == 1 {
            ipiv[k - 1] = kp as i32;
        } else {
            ipiv[k - 1] = -(kp as i32);
            ipiv[k] = -(kp as i32);
        }
        k += kstep;
    }
    info
}

/// `dsytrs('L')`: `X = A⁻¹B` from [`sytrf_l`]'s factors `a` (leading dimension `lda`) and
/// pivots, for the `n × nrhs` right-hand sides `b` (leading dimension `ldb`, overwritten by the
/// solution).
pub fn sytrs_l(
    a: &[f64],
    n: usize,
    lda: usize,
    ipiv: &[i32],
    b: &mut [f64],
    ldb: usize,
    nrhs: usize,
) {
    assert!(lda >= n.max(1) && ldb >= n.max(1));
    if n == 0 || nrhs == 0 {
        return;
    }
    let ai = move |i: usize, j: usize| (j - 1) * lda + (i - 1);
    let bi = move |i: usize, j: usize| (j - 1) * ldb + (i - 1);
    let swap_rows = |b: &mut [f64], r1: usize, r2: usize| {
        for j in 1..=nrhs {
            b.swap(bi(r1, j), bi(r2, j));
        }
    };
    let row = |b: &[f64], r: usize| -> Vec<f64> { (1..=nrhs).map(|j| b[bi(r, j)]).collect() };
    // B(r0.., :) −= A(r0.., col)·B(src, :) by DGER.
    let ger = |b: &mut [f64], col: usize, src: usize, r0: usize| {
        let y = row(b, src);
        dger(
            -1.0,
            &a[ai(r0, col)..=ai(n, col)],
            &y,
            b,
            MatRef {
                off: bi(r0, 1),
                ld: ldb,
            },
        );
    };
    // B(dst, :) −= A(k+1.., col)ᵀ·B(k+1.., :) by DGEMV('T') with the strided row as y.
    let gemv_t = |b: &mut [f64], k: usize, col: usize, dst: usize| {
        let mut y = row(b, dst);
        dgemv_t_inc(
            n - k,
            nrhs,
            -1.0,
            b,
            MatRef {
                off: bi(k + 1, 1),
                ld: ldb,
            },
            &a[ai(k + 1, col)..=ai(n, col)],
            1,
            1.0,
            &mut y,
            ldb,
        );
        for (j, v) in y.into_iter().enumerate() {
            b[bi(dst, j + 1)] = v;
        }
    };
    // L·D·X = B, overwriting B with X.
    let mut k = 1usize;
    while k <= n {
        if ipiv[k - 1] > 0 {
            let kp = ipiv[k - 1] as usize;
            if kp != k {
                swap_rows(b, k, kp);
            }
            if k < n {
                ger(b, k, k, k + 1);
            }
            let mut r = row(b, k);
            dscal(1.0 / a[ai(k, k)], &mut r);
            for (j, v) in r.into_iter().enumerate() {
                b[bi(k, j + 1)] = v;
            }
            k += 1;
        } else {
            let kp = (-ipiv[k - 1]) as usize;
            if kp != k + 1 {
                swap_rows(b, k + 1, kp);
            }
            if k + 1 < n {
                ger(b, k, k, k + 2);
                ger(b, k + 1, k + 1, k + 2);
            }
            let akm1k = a[ai(k + 1, k)];
            let akm1 = a[ai(k, k)] / akm1k;
            let ak = a[ai(k + 1, k + 1)] / akm1k;
            let denom = akm1 * ak - 1.0;
            for j in 1..=nrhs {
                let bkm1 = b[bi(k, j)] / akm1k;
                let bk = b[bi(k + 1, j)] / akm1k;
                b[bi(k, j)] = (ak * bkm1 - bk) / denom;
                b[bi(k + 1, j)] = (akm1 * bk - bkm1) / denom;
            }
            k += 2;
        }
    }
    // Lᵀ·X = B, overwriting B with X.
    let mut k = n;
    while k >= 1 {
        if ipiv[k - 1] > 0 {
            if k < n {
                gemv_t(b, k, k, k);
            }
            let kp = ipiv[k - 1] as usize;
            if kp != k {
                swap_rows(b, k, kp);
            }
            k -= 1;
        } else {
            if k < n {
                gemv_t(b, k, k, k);
                gemv_t(b, k, k - 1, k - 1);
            }
            let kp = (-ipiv[k - 1]) as usize;
            if kp != k {
                swap_rows(b, k, kp);
            }
            if k < 2 {
                break;
            }
            k -= 2;
        }
    }
}

/// `inv(A) * B` by Armadillo 14.2.1's `solve_sym_fast`: [`sytrf_l`] with Armadillo's workspace
/// ([`armadillo_lwork`]) on a copy of `a` (`n × n`, column-major), then [`sytrs_l`] on a copy of
/// `b` (`n × nrhs`). `None` when `dsytrf` reports a singular `D`.
pub fn solve_sym(a: &[f64], n: usize, b: &[f64], nrhs: usize) -> Option<Vec<f64>> {
    let mut f = a.to_vec();
    let mut x = b.to_vec();
    if n == 0 || nrhs == 0 {
        return Some(vec![0.0; n * nrhs]);
    }
    let mut ipiv = vec![0i32; n];
    if sytrf_l(&mut f, n, n, &mut ipiv, armadillo_lwork(n)) != 0 {
        return None;
    }
    sytrs_l(&f, n, n, &ipiv, &mut x, n, nrhs);
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A symmetric indefinite matrix (zero diagonal, so 2 × 2 pivots occur) of `n` rows.
    fn indefinite(n: usize) -> Vec<f64> {
        let mut a = vec![0.0; n * n];
        for c in 0..n {
            for r in 0..n {
                let v = if r == c {
                    0.0
                } else {
                    (((r + 1) * (c + 1) * 7) % 23) as f64 / 23.0 - 0.4
                };
                a[c * n + r] = v;
            }
        }
        a
    }

    #[test]
    fn solves_symmetric_systems() {
        for n in [5usize, 40, 64, 65, 130, 200] {
            let a = indefinite(n);
            let nrhs = 3;
            let b: Vec<f64> = (0..n * nrhs).map(|i| (i % 7) as f64 - 3.0).collect();
            let x = solve_sym(&a, n, &b, nrhs).expect("non-singular");
            for j in 0..nrhs {
                for r in 0..n {
                    let s: f64 = (0..n).map(|c| a[c * n + r] * x[j * n + c]).sum();
                    assert!((s - b[j * n + r]).abs() < 1e-8, "n {n}, row {r}: {s}");
                }
            }
        }
    }

    #[test]
    fn workspace_and_singular() {
        assert_eq!(armadillo_lwork(3), 16);
        assert_eq!(armadillo_lwork(16), 16);
        assert_eq!(armadillo_lwork(17), 17 * 64);
        // A zero column: info points at it, Armadillo refuses.
        let n = 120;
        let mut a = indefinite(n);
        for i in 0..n {
            a[7 * n + i] = 0.0;
            a[i * n + 7] = 0.0;
        }
        let mut f = a.clone();
        let mut ipiv = vec![0; n];
        assert!(sytrf_l(&mut f, n, n, &mut ipiv, armadillo_lwork(n)) > 0);
        assert!(solve_sym(&a, n, &[1.0; 120], 1).is_none());
    }
}
