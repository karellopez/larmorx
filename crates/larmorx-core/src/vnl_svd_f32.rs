// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's singular value decomposition in single precision (`vnl_svd<float>`) and its
//! `solve`.
//!
//! ITK's B-spline filters (`BSplineControlPointImageFilter`,
//! `BSplineScatteredDataPointSetToImageFilter`) compute the coefficients that refine a
//! control-point lattice to twice its resolution with `vnl_svd<float>(R).solve(S)`, so N4's
//! bias field depends on the last bits of this float SVD. It is LINPACK's `ssvdc` as
//! translated by f2c in VXL's `v3p/netlib`, ported operation for operation with the
//! single-precision BLAS it calls. Two f2c details matter: `snrm2` and `srotg` multiply a
//! float by the **double** square root before rounding (`scale * sqrt(ssq)` with `sqrt` of
//! `doublereal`), and `sdot`/`snrm2` return `doublereal` (exact, as the float they hold). The
//! other operations are single precision, as gcc evaluates f2c's `real` arithmetic on x86-64
//! (`FLT_EVAL_METHOD` 0); a double square root of a float rounded back to float is the float
//! square root, as is a double quotient of floats rounded to float.
//!
//! Ported from VXL as bundled with ITK v5.4.5 (BSD licence; LINPACK and BLAS from netlib):
//! `core/vnl/algo/vnl_svd.hxx` (the constructor, `solve(vnl_matrix)`), `core/vnl/vnl_matrix.h`
//! (`operator*`), `v3p/netlib/linpack/ssvdc.c`, `v3p/netlib/blas/{snrm2,sdot,saxpy,sscal,srot,
//! srotg}.c` and `v3p/netlib/libf2c/r_sign.c`. The double-precision version is
//! [`crate::vnl_svd`].

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
// LINPACK (ssvdc) and the reference BLAS are netlib software, distributed by VXL as above.

// The loops keep LINPACK's 1-based Fortran indices, so the port can be checked line by line
// against ssvdc.c.
#![allow(clippy::needless_range_loop)]

// ------------------------------------------------------------------------------------------------
// Single-precision reference BLAS (unit stride), as f2c translated them.

/// `snrm2`: the scaled sum of squares in float; the norm is `scale * sqrt(ssq)` with the
/// square root in double, rounded to float once.
fn snrm2(x: &[f32]) -> f32 {
    match x.len() {
        0 => 0.0,
        1 => x[0].abs(),
        _ => {
            let (mut scale, mut ssq) = (0.0f32, 1.0f32);
            for &xi in x {
                if xi != 0.0 {
                    let absxi = xi.abs();
                    if scale < absxi {
                        let r = scale / absxi;
                        ssq = ssq * (r * r) + 1.0;
                        scale = absxi;
                    } else {
                        let r = absxi / scale;
                        ssq += r * r;
                    }
                }
            }
            (f64::from(scale) * f64::from(ssq).sqrt()) as f32
        }
    }
}

/// `sdot` with its unrolling by five (float accumulation, left to right).
fn sdot(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len();
    let m = n % 5;
    let mut t = 0.0f32;
    for i in 0..m {
        t += x[i] * y[i];
    }
    let mut i = m;
    while i + 5 <= n {
        t = t
            + x[i] * y[i]
            + x[i + 1] * y[i + 1]
            + x[i + 2] * y[i + 2]
            + x[i + 3] * y[i + 3]
            + x[i + 4] * y[i + 4];
        i += 5;
    }
    t
}

/// `y += a·x`; nothing when `a == 0`.
fn saxpy(a: f32, x: &[f32], y: &mut [f32]) {
    if a == 0.0 {
        return;
    }
    for (yi, &xi) in y.iter_mut().zip(x) {
        *yi += a * xi;
    }
}

fn sscal(a: f32, x: &mut [f32]) {
    for xi in x {
        *xi *= a;
    }
}

fn srot(x: &mut [f32], y: &mut [f32], c: f32, s: f32) {
    for (xi, yi) in x.iter_mut().zip(y.iter_mut()) {
        let t = c * *xi + s * *yi;
        *yi = c * *yi - s * *xi;
        *xi = t;
    }
}

/// `srotg`: the Givens rotation zeroing `b`; returns `(r, z, c, s)` (`sa` and `sb` on exit,
/// and `c`, `s`).
fn srotg(sa: f32, sb: f32) -> (f32, f32, f32, f32) {
    let roe = if sa.abs() > sb.abs() { sa } else { sb };
    let scale = sa.abs() + sb.abs();
    if scale == 0.0 {
        return (0.0, 0.0, 1.0, 0.0);
    }
    let r1 = sa / scale;
    let r2 = sb / scale;
    let mut r = (f64::from(scale) * f64::from(r1 * r1 + r2 * r2).sqrt()) as f32;
    // `r_sign(1, roe) * r`: exact.
    if roe < 0.0 {
        r = -r;
    }
    let c = sa / r;
    let s = sb / r;
    let mut z = 1.0f32;
    if sa.abs() > sb.abs() {
        z = s;
    }
    if sb.abs() >= sa.abs() && c != 0.0 {
        z = 1.0 / c;
    }
    (r, z, c, s)
}

/// f2c's `r_sign(a, b)`: |a| with the sign of b (b ≥ 0 counts as positive).
fn r_sign(a: f32, b: f32) -> f32 {
    let x = if a >= 0.0 { a } else { -a };
    if b >= 0.0 { x } else { -x }
}

/// f2c's `max`/`dmax` macro: `a >= b ? a : b`.
fn fmax(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// Column-major matrix with LINPACK's 1-based indexing.
struct Fortran {
    data: Vec<f32>,
    ld: usize,
}

impl Fortran {
    fn new(rows: usize, cols: usize) -> Self {
        Fortran {
            data: vec![0.0; rows * cols],
            ld: rows,
        }
    }

    #[inline]
    fn idx(&self, i: usize, j: usize) -> usize {
        (i - 1) + (j - 1) * self.ld
    }

    #[inline]
    fn at(&self, i: usize, j: usize) -> f32 {
        self.data[self.idx(i, j)]
    }

    #[inline]
    fn set(&mut self, i: usize, j: usize, v: f32) {
        let k = self.idx(i, j);
        self.data[k] = v;
    }

    fn col(&self, i: usize, j: usize, len: usize) -> &[f32] {
        let k = self.idx(i, j);
        &self.data[k..k + len]
    }

    fn col_mut(&mut self, i: usize, j: usize, len: usize) -> &mut [f32] {
        let k = self.idx(i, j);
        &mut self.data[k..k + len]
    }

    /// Two disjoint column segments: (rows i.. of column ja, rows i.. of column jb), ja ≠ jb.
    fn two_cols(&mut self, i: usize, ja: usize, jb: usize, len: usize) -> (&mut [f32], &mut [f32]) {
        let (ka, kb) = (self.idx(i, ja), self.idx(i, jb));
        if ka < kb {
            let (lo, hi) = self.data.split_at_mut(kb);
            (&mut lo[ka..ka + len], &mut hi[..len])
        } else {
            let (lo, hi) = self.data.split_at_mut(ka);
            (&mut hi[..len], &mut lo[kb..kb + len])
        }
    }
}

/// The SVD of a `rows × cols` float matrix (row-major input), as `vnl_svd<float>` computes
/// it: `(U (rows × cols, column-major), w (singular values, ≥ 0), V (cols × cols,
/// column-major))`, or `None` if LINPACK fails to converge. Requires `rows ≥ cols` (economy
/// size, `job = 21`).
#[allow(clippy::many_single_char_names)]
pub fn vnl_svd_f32(m: &[f32], rows: usize, cols: usize) -> Option<(Vec<f32>, Vec<f32>, Vec<f32>)> {
    let (n, p) = (rows, cols);
    assert!(n >= p && p > 0 && m.len() == n * p);
    let mut x = Fortran::new(n, p);
    for i in 0..n {
        for j in 0..p {
            x.set(i + 1, j + 1, m[i * p + j]);
        }
    }
    let mm_len = (n + 1).min(p);
    let mut s = vec![0.0f32; mm_len + 1]; // 1-based
    let mut e = vec![0.0f32; p + 1];
    let mut u = Fortran::new(n, p);
    let mut v = Fortran::new(p, p);
    let mut work = vec![0.0f32; n + 1];
    let ncu = n.min(p); // job = 21: jobu = 2
    let (wantu, wantv) = (true, true);

    // Householder reduction to bidiagonal form.
    let nct = (n - 1).min(p);
    let nrt = 0.max((p as isize - 2).min(n as isize)) as usize;
    let lu = nct.max(nrt);
    for l in 1..=lu {
        let lp1 = l + 1;
        if l <= nct {
            s[l] = snrm2(x.col(l, l, n - l + 1));
            if s[l] != 0.0 {
                if x.at(l, l) != 0.0 {
                    s[l] = r_sign(s[l], x.at(l, l));
                }
                let scale = 1.0 / s[l];
                sscal(scale, x.col_mut(l, l, n - l + 1));
                x.set(l, l, x.at(l, l) + 1.0);
            }
            s[l] = -s[l];
        }
        if p >= lp1 {
            for j in lp1..=p {
                if l <= nct && s[l] != 0.0 {
                    let t = -sdot(x.col(l, l, n - l + 1), x.col(l, j, n - l + 1)) / x.at(l, l);
                    let (xl, xj) = x.two_cols(l, l, j, n - l + 1);
                    saxpy(t, xl, xj);
                }
                e[j] = x.at(l, j);
            }
        }
        if wantu && l <= nct {
            for i in l..=n {
                u.set(i, l, x.at(i, l));
            }
        }
        if l <= nrt {
            e[l] = snrm2(&e[lp1..=p]);
            if e[l] != 0.0 {
                if e[lp1] != 0.0 {
                    e[l] = r_sign(e[l], e[lp1]);
                }
                let scale = 1.0 / e[l];
                sscal(scale, &mut e[lp1..=p]);
                e[lp1] += 1.0;
            }
            e[l] = -e[l];
            if lp1 <= n && e[l] != 0.0 {
                for w in work.iter_mut().take(n + 1).skip(lp1) {
                    *w = 0.0;
                }
                for j in lp1..=p {
                    saxpy(e[j], x.col(lp1, j, n - l), &mut work[lp1..=n]);
                }
                for j in lp1..=p {
                    let a = -e[j] / e[lp1];
                    saxpy(a, &work[lp1..=n], x.col_mut(lp1, j, n - l));
                }
            }
            if wantv {
                for i in lp1..=p {
                    v.set(i, l, e[i]);
                }
            }
        }
    }

    // The final bidiagonal matrix of order m.
    let mut m_ = p.min(n + 1);
    let nctp1 = nct + 1;
    let nrtp1 = nrt + 1;
    if nct < p {
        s[nctp1] = x.at(nctp1, nctp1);
    }
    if n < m_ {
        s[m_] = 0.0;
    }
    if nrtp1 < m_ {
        e[nrtp1] = x.at(nrtp1, m_);
    }
    e[m_] = 0.0;

    // Generate U.
    if wantu {
        if ncu >= nctp1 {
            for j in nctp1..=ncu {
                for i in 1..=n {
                    u.set(i, j, 0.0);
                }
                u.set(j, j, 1.0);
            }
        }
        for ll in 1..=nct {
            let l = nct - ll + 1;
            if s[l] != 0.0 {
                let lp1 = l + 1;
                if ncu >= lp1 {
                    for j in lp1..=ncu {
                        let t = -sdot(u.col(l, l, n - l + 1), u.col(l, j, n - l + 1)) / u.at(l, l);
                        let (ul, uj) = u.two_cols(l, l, j, n - l + 1);
                        saxpy(t, ul, uj);
                    }
                }
                sscal(-1.0, u.col_mut(l, l, n - l + 1));
                u.set(l, l, u.at(l, l) + 1.0);
                for i in 1..l {
                    u.set(i, l, 0.0);
                }
            } else {
                for i in 1..=n {
                    u.set(i, l, 0.0);
                }
                u.set(l, l, 1.0);
            }
        }
    }

    // Generate V.
    if wantv {
        for ll in 1..=p {
            let l = p - ll + 1;
            let lp1 = l + 1;
            if l <= nrt && e[l] != 0.0 {
                for j in lp1..=p {
                    let t = -sdot(v.col(lp1, l, p - l), v.col(lp1, j, p - l)) / v.at(lp1, l);
                    let (vl, vj) = v.two_cols(lp1, l, j, p - l);
                    saxpy(t, vl, vj);
                }
            }
            for i in 1..=p {
                v.set(i, l, 0.0);
            }
            v.set(l, l, 1.0);
        }
    }

    // Main iteration loop for the singular values.
    let mm = m_;
    let mut iter = 0;
    let maxit = 1000;
    while m_ != 0 {
        if iter >= maxit {
            return None;
        }
        let mut l = 0usize;
        for ll in 1..=m_ {
            l = m_ - ll;
            if l == 0 {
                break;
            }
            let test = s[l].abs() + s[l + 1].abs();
            let ztest = test + e[l].abs();
            if ztest == test {
                e[l] = 0.0;
                break;
            }
        }
        let kase;
        if l == m_ - 1 {
            kase = 4;
        } else {
            let lp1 = l + 1;
            let mp1 = m_ + 1;
            let mut ls = 0usize;
            for lls in lp1..=mp1 {
                ls = m_ + lp1 - lls;
                if ls == l {
                    break;
                }
                let mut test = 0.0f32;
                if ls != m_ {
                    test += e[ls].abs();
                }
                if ls != l + 1 {
                    test += e[ls - 1].abs();
                }
                let ztest = test + s[ls].abs();
                if ztest == test {
                    s[ls] = 0.0;
                    break;
                }
            }
            if ls == l {
                kase = 3;
            } else if ls == m_ {
                kase = 1;
            } else {
                kase = 2;
                l = ls;
            }
        }
        l += 1;
        match kase {
            1 => {
                // Deflate negligible s[m].
                let mm1 = m_ - 1;
                let mut f = e[m_ - 1];
                e[m_ - 1] = 0.0;
                for kk in l..=mm1 {
                    let k = mm1 - kk + l;
                    let (r, _, cs, sn) = srotg(s[k], f);
                    s[k] = r;
                    if k != l {
                        f = -sn * e[k - 1];
                        e[k - 1] *= cs;
                    }
                    if wantv {
                        let (vk, vm) = v.two_cols(1, k, m_, p);
                        srot(vk, vm, cs, sn);
                    }
                }
            }
            2 => {
                // Split at negligible s[l].
                let mut f = e[l - 1];
                e[l - 1] = 0.0;
                for k in l..=m_ {
                    let (r, _, cs, sn) = srotg(s[k], f);
                    s[k] = r;
                    f = -sn * e[k];
                    e[k] *= cs;
                    if wantu {
                        let (uk, ul) = u.two_cols(1, k, l - 1, n);
                        srot(uk, ul, cs, sn);
                    }
                }
            }
            3 => {
                // One QR step: the shift.
                let scale = fmax(
                    fmax(
                        fmax(fmax(s[m_].abs(), s[m_ - 1].abs()), e[m_ - 1].abs()),
                        s[l].abs(),
                    ),
                    e[l].abs(),
                );
                let sm = s[m_] / scale;
                let smm1 = s[m_ - 1] / scale;
                let emm1 = e[m_ - 1] / scale;
                let sl = s[l] / scale;
                let el = e[l] / scale;
                let b = ((smm1 + sm) * (smm1 - sm) + emm1 * emm1) / 2.0;
                let c = (sm * emm1) * (sm * emm1);
                let mut shift = 0.0f32;
                if !(b == 0.0 && c == 0.0) {
                    shift = f64::from(b * b + c).sqrt() as f32;
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c / (b + shift);
                }
                let mut f = (sl + sm) * (sl - sm) + shift;
                let mut g = sl * el;
                // Chase zeros.
                let mm1 = m_ - 1;
                for k in l..=mm1 {
                    let (r, _, cs, sn) = srotg(f, g);
                    f = r;
                    if k != l {
                        e[k - 1] = f;
                    }
                    f = cs * s[k] + sn * e[k];
                    e[k] = cs * e[k] - sn * s[k];
                    g = sn * s[k + 1];
                    s[k + 1] *= cs;
                    if wantv {
                        let (vk, vk1) = v.two_cols(1, k, k + 1, p);
                        srot(vk, vk1, cs, sn);
                    }
                    let (r, _, cs, sn) = srotg(f, g);
                    f = r;
                    s[k] = f;
                    f = cs * e[k] + sn * s[k + 1];
                    s[k + 1] = -sn * e[k] + cs * s[k + 1];
                    g = sn * e[k + 1];
                    e[k + 1] *= cs;
                    if wantu && k < n {
                        let (uk, uk1) = u.two_cols(1, k, k + 1, n);
                        srot(uk, uk1, cs, sn);
                    }
                }
                e[m_ - 1] = f;
                iter += 1;
            }
            _ => {
                // Convergence: make the singular value positive, then order them.
                if s[l] < 0.0 {
                    s[l] = -s[l];
                    if wantv {
                        sscal(-1.0, v.col_mut(1, l, p));
                    }
                }
                while l != mm && s[l] < s[l + 1] {
                    s.swap(l, l + 1);
                    if wantv && l < p {
                        let (a, b) = v.two_cols(1, l, l + 1, p);
                        a.swap_with_slice(b);
                    }
                    if wantu && l < n {
                        let (a, b) = u.two_cols(1, l, l + 1, n);
                        a.swap_with_slice(b);
                    }
                    l += 1;
                }
                iter = 0;
                m_ -= 1;
            }
        }
    }
    let w = (0..p)
        .map(|j| if j < mm_len { s[j + 1].abs() } else { 0.0 })
        .collect();
    Some((u.data, w, v.data))
}

/// `vnl_matrix<float>::operator*`: `a (l × m) · b (m × n)`, row-major, each entry summed from
/// 0 in index order in float.
pub fn matmul_f32(a: &[f32], l: usize, m: usize, b: &[f32], n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; l * n];
    for i in 0..l {
        for k in 0..n {
            let mut sum = 0.0f32;
            for j in 0..m {
                sum += a[i * m + j] * b[j * n + k];
            }
            out[i * n + k] = sum;
        }
    }
    out
}

/// `vnl_svd<float>(M).solve(B)` for a square `n × n` matrix `M` and an `n × k` right-hand
/// side (both row-major): `V · diag(1/w) · (Uᵀ · B)`, zero singular values skipped
/// (`zero_out_absolute(0)`). `None` if the SVD fails to converge.
pub fn vnl_svd_solve_f32(m: &[f32], n: usize, b: &[f32], k: usize) -> Option<Vec<f32>> {
    let (u, w, v) = vnl_svd_f32(m, n, n)?;
    // Uᵀ and V as row-major matrices (both are column-major n × n).
    let mut ut = vec![0.0f32; n * n];
    let mut vr = vec![0.0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            ut[i * n + j] = u[i * n + j]; // Uᵀ(i, j) = U(j, i) = u[j + i·n]
            vr[i * n + j] = v[i + j * n];
        }
    }
    let mut x = matmul_f32(&ut, n, n, b, k);
    for (i, row) in x.chunks_mut(k).enumerate() {
        let mut weight = w[i];
        if weight != 0.0 {
            weight = 1.0 / weight;
        }
        for xij in row {
            *xij *= weight;
        }
    }
    Some(matmul_f32(&vr, n, n, &x, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_well_conditioned_systems() {
        // M · X = B with a known X.
        let m = [4.0f32, 1.0, 0.5, 2.0, 5.0, 1.0, 0.5, 1.0, 3.0];
        let x = [1.0f32, -2.0, 0.5, 3.0, 2.0, -1.0];
        let b = matmul_f32(&m, 3, 3, &x, 2);
        let got = vnl_svd_solve_f32(&m, 3, &b, 2).unwrap();
        for (g, e) in got.iter().zip(x) {
            assert!((g - e).abs() < 1e-5, "{g} vs {e}");
        }
    }

    #[test]
    fn singular_values_are_ordered_and_positive() {
        let m = [0.0f32, 2.0, 0.0, -3.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let (_, w, _) = vnl_svd_f32(&m, 3, 3).unwrap();
        assert_eq!(w, [3.0, 2.0, 1.0]);
    }
}
