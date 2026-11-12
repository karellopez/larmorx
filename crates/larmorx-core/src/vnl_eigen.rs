// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's eigensystems, as ITK's `ImageMomentsCalculator` and ANTs' `antsAI` compute them.
//!
//! - [`symmetric_eigensystem`]: `vnl_symmetric_eigensystem<double>`, EISPACK's `rs`
//!   (`tred2`, then `tql2` with `pythag`): the eigenvalues in ascending order and the
//!   eigenvectors as the columns of `V`.
//! - [`real_eigenvalues`]: the eigenvalues of `vnl_real_eigensystem` (EISPACK's `rg` with
//!   eigenvectors: `balanc`, `elmhes`, `eltran`, `hqr2`). The eigenvectors (`eltran`, the
//!   accumulation and back-substitution of `hqr2`, `balbak`) do not change the eigenvalues
//!   and are not computed; every update of the Hessenberg matrix `hqr2` makes is, over the
//!   ranges `hqr2` uses.
//!
//! - [`real_eigensystem`]: `vnl_real_eigensystem` with its eigenvectors (`D`, `V`, `Vreal`), as
//!   FreeSurfer 8.2.0's `OpenNonSymmetricEigenSystem` reads them (ITK 4.13.2: `rg` with
//!   `balanc`, `elmhes`, `eltran`, `hqr2` and `balbak`, written again with the f2c macros `abs`,
//!   `max` in their C semantics; three defects of VNL's, ITK-110 to ITK-112, are documented
//!   there).
//! - [`symmetric_eigensystem_f32`]: `vnl_symmetric_eigensystem<float>`, `rs` in double rounded to
//!   float.
//!
//! The f2c translations are followed statement by statement, in their order and with their
//! 1-based indices, so that every intermediate is rounded as the C code rounds it. Only
//! `sqrt` and the four basic operations are used, which IEEE arithmetic rounds exactly: the
//! result is the same on every platform.
//!
//! Ported from VXL as bundled with ITK v5.4.5 (BSD licence; EISPACK from netlib):
//! `core/vnl/algo/vnl_symmetric_eigensystem.hxx`, `core/vnl/algo/vnl_real_eigensystem.cxx`,
//! `v3p/netlib/eispack/{rs,tred2,tql2,pythag,rg,balanc,elmhes,hqr2}.c`.

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
// EISPACK is netlib software, distributed by VXL as above.

// The loops keep EISPACK's 1-based Fortran indices, so the port can be checked line by line
// against the f2c code.
#![allow(
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    clippy::assign_op_pattern,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::mut_range_bound
)]

/// A Fortran array of `n × n` doubles, column-major, indexed from 1.
struct Fortran {
    n: usize,
    data: Vec<f64>,
}

impl Fortran {
    #[inline]
    fn at(&self, r: usize, c: usize) -> f64 {
        self.data[(c - 1) * self.n + (r - 1)]
    }
    #[inline]
    fn set(&mut self, r: usize, c: usize, v: f64) {
        self.data[(c - 1) * self.n + (r - 1)] = v;
    }
    #[inline]
    fn swap(&mut self, r1: usize, c1: usize, r2: usize, c2: usize) {
        let (a, b) = ((c1 - 1) * self.n + (r1 - 1), (c2 - 1) * self.n + (r2 - 1));
        self.data.swap(a, b);
    }
}

/// f2c's `d_sign(a, b)`: `|a|` with the sign of `b` (`b ≥ 0`, so `−0` counts as positive).
#[inline]
fn d_sign(a: f64, b: f64) -> f64 {
    let x = if a >= 0.0 { a } else { -a };
    if b >= 0.0 { x } else { -x }
}

/// EISPACK's `pythag(a, b)`: `sqrt(a² + b²)` by Moler and Morrison's iteration.
pub fn pythag(a: f64, b: f64) -> f64 {
    let mut p = a.abs().max(b.abs());
    if p == 0.0 {
        return p;
    }
    let d = a.abs().min(b.abs()) / p;
    let mut r = d * d;
    loop {
        let t = r + 4.0;
        if t == 4.0 {
            return p;
        }
        let s = r / t;
        let u = s * 2.0 + 1.0;
        p *= u;
        let q = s / u;
        r = q * q * r;
    }
}

/// `vnl_symmetric_eigensystem<double>(A)`: `D` (ascending) and `V` (eigenvectors as its
/// columns, row-major here), and EISPACK's error flag `ierr` (0, or the index of the
/// eigenvalue `tql2` failed to converge on after 30 iterations; vnl then prints
/// `vnl_symmetric_eigensystem: ierr = …` and leaves `V` unset).
#[derive(Clone, Debug, PartialEq)]
pub struct SymmetricEigen {
    pub values: Vec<f64>,
    /// `V(r, c)` at `vectors[r * n + c]`.
    pub vectors: Vec<f64>,
    pub ierr: usize,
}

/// [`SymmetricEigen`] of the `n × n` matrix `a` (row-major). vnl passes its row-major data
/// to the Fortran routine, which reads it column-major: EISPACK sees `Aᵀ`, and `tred2` reads
/// only its lower triangle, that is the upper triangle of `A` as stored (which matters when
/// `A` is symmetric only up to rounding).
pub fn symmetric_eigensystem(a: &[f64], n: usize) -> SymmetricEigen {
    assert!(n >= 1 && a.len() == n * n, "not an n × n matrix");
    let a = Fortran {
        n,
        data: a.to_vec(),
    };
    let mut z = Fortran {
        n,
        data: vec![0.0; n * n],
    };
    let mut d = vec![0.0f64; n + 1];
    let mut e = vec![0.0f64; n + 1];
    tred2(&a, &mut d, &mut e, &mut z);
    let ierr = tql2(n, &mut d, &mut e, &mut z);
    let mut vectors = vec![0.0f64; n * n];
    for c in 0..n {
        for r in 0..n {
            vectors[r * n + c] = z.at(r + 1, c + 1);
        }
    }
    SymmetricEigen {
        values: d[1..].to_vec(),
        vectors,
        ierr,
    }
}

/// EISPACK's `tred2`: Householder reduction of the real symmetric matrix `a` (its lower
/// triangle) to tridiagonal form, the transformations accumulated in `z`.
fn tred2(a: &Fortran, d: &mut [f64], e: &mut [f64], z: &mut Fortran) {
    let n = a.n;
    for i in 1..=n {
        for j in i..=n {
            z.set(j, i, a.at(j, i));
        }
        d[i] = a.at(n, i);
    }
    if n != 1 {
        for ii in 2..=n {
            let i = n + 2 - ii;
            let l = i - 1;
            let mut h = 0.0f64;
            let mut scale = 0.0f64;
            let mut skip = l < 2;
            if !skip {
                for k in 1..=l {
                    scale += d[k].abs();
                }
                skip = scale == 0.0;
            }
            if skip {
                // L130
                e[i] = d[l];
                for j in 1..=l {
                    d[j] = z.at(l, j);
                    z.set(i, j, 0.0);
                    z.set(j, i, 0.0);
                }
            } else {
                // L140
                for k in 1..=l {
                    d[k] /= scale;
                    h += d[k] * d[k];
                }
                let mut f = d[l];
                let mut g = -d_sign(h.sqrt(), f);
                e[i] = scale * g;
                h -= f * g;
                d[l] = f - g;
                for j in 1..=l {
                    e[j] = 0.0;
                }
                for j in 1..=l {
                    f = d[j];
                    z.set(j, i, f);
                    g = e[j] + z.at(j, j) * f;
                    for k in j + 1..=l {
                        g += z.at(k, j) * d[k];
                        e[k] += z.at(k, j) * f;
                    }
                    e[j] = g;
                }
                f = 0.0;
                for j in 1..=l {
                    e[j] /= h;
                    f += e[j] * d[j];
                }
                let hh = f / (h + h);
                for j in 1..=l {
                    e[j] -= hh * d[j];
                }
                for j in 1..=l {
                    f = d[j];
                    g = e[j];
                    for k in j..=l {
                        let v = z.at(k, j) - f * e[k] - g * d[k];
                        z.set(k, j, v);
                    }
                    d[j] = z.at(l, j);
                    z.set(i, j, 0.0);
                }
            }
            // L290
            d[i] = h;
        }
        for i in 2..=n {
            let l = i - 1;
            let v = z.at(l, l);
            z.set(n, l, v);
            z.set(l, l, 1.0);
            let h = d[i];
            if h != 0.0 {
                for k in 1..=l {
                    d[k] = z.at(k, i) / h;
                }
                for j in 1..=l {
                    let mut g = 0.0f64;
                    for k in 1..=l {
                        g += z.at(k, i) * z.at(k, j);
                    }
                    for k in 1..=l {
                        let v = z.at(k, j) - g * d[k];
                        z.set(k, j, v);
                    }
                }
            }
            // L380
            for k in 1..=l {
                z.set(k, i, 0.0);
            }
        }
    }
    // L510
    for i in 1..=n {
        d[i] = z.at(n, i);
        z.set(n, i, 0.0);
    }
    z.set(n, n, 1.0);
    e[1] = 0.0;
}

/// EISPACK's `tql2`: the eigenvalues and eigenvectors of the tridiagonal matrix by the QL
/// method, sorted ascending. Returns `ierr`.
fn tql2(n: usize, d: &mut [f64], e: &mut [f64], z: &mut Fortran) -> usize {
    if n == 1 {
        return 0;
    }
    for i in 2..=n {
        e[i - 1] = e[i];
    }
    let mut f = 0.0f64;
    let mut tst1 = 0.0f64;
    e[n] = 0.0;
    for l in 1..=n {
        let mut j = 0;
        let h = d[l].abs() + e[l].abs();
        if tst1 < h {
            tst1 = h;
        }
        // Look for a small sub-diagonal element (e[n] is 0, so the search ends at n for
        // finite values).
        let mut m = l;
        while m <= n {
            let tst2 = tst1 + e[m].abs();
            if tst2 == tst1 {
                break;
            }
            m += 1;
        }
        if m > n {
            // Only with NaN or infinite values, where the f2c code reads past the arrays.
            return l;
        }
        if m != l {
            loop {
                // L130
                if j == 30 {
                    return l;
                }
                j += 1;
                let l1 = l + 1;
                let l2 = l1 + 1;
                let mut g = d[l];
                let mut p = (d[l1] - g) / (e[l] * 2.0);
                let mut r = pythag(p, 1.0);
                d[l] = e[l] / (p + d_sign(r, p));
                d[l1] = e[l] * (p + d_sign(r, p));
                let dl1 = d[l1];
                let mut h = g - d[l];
                if l2 <= n {
                    for i in l2..=n {
                        d[i] -= h;
                    }
                }
                // L145
                f += h;
                p = d[m];
                let mut c = 1.0f64;
                let mut c2 = c;
                let el1 = e[l1];
                let mut s = 0.0f64;
                let mut c3 = 0.0f64;
                let mut s2 = 0.0f64;
                let mml = m - l;
                for ii in 1..=mml {
                    c3 = c2;
                    c2 = c;
                    s2 = s;
                    let i = m - ii;
                    g = c * e[i];
                    h = c * p;
                    r = pythag(p, e[i]);
                    e[i + 1] = s * r;
                    s = e[i] / r;
                    c = p / r;
                    p = c * d[i] - s * g;
                    d[i + 1] = h + s * (c * g + s * d[i]);
                    for k in 1..=n {
                        h = z.at(k, i + 1);
                        let zi = z.at(k, i);
                        z.set(k, i + 1, s * zi + c * h);
                        z.set(k, i, c * zi - s * h);
                    }
                }
                p = -s * s2 * c3 * el1 * e[l] / dl1;
                e[l] = s * p;
                d[l] = c * p;
                let tst2 = tst1 + e[l].abs();
                if !(tst2 > tst1) {
                    break;
                }
            }
        }
        // L220
        d[l] += f;
    }
    // Order the eigenvalues and eigenvectors.
    for ii in 2..=n {
        let i = ii - 1;
        let mut k = i;
        let mut p = d[i];
        for j in ii..=n {
            if d[j] >= p {
                continue;
            }
            k = j;
            p = d[j];
        }
        if k != i {
            d[k] = d[i];
            d[i] = p;
            for j in 1..=n {
                z.swap(j, i, j, k);
            }
        }
    }
    0
}

/// The eigenvalues of `vnl_real_eigensystem(M)` for the `n × n` matrix `m` (row-major):
/// `(wr, wi, ierr)`, as `rg` with eigenvectors (`matz = 1`) computes them. `ierr` is 0, or
/// the index of the eigenvalue `hqr2` failed on after `30·n` iterations (vnl then prints
/// ` *** vnl_real_eigensystem: Failed on …th eigenvalue` and the matrix).
pub fn real_eigenvalues(m: &[f64], n: usize) -> (Vec<f64>, Vec<f64>, usize) {
    assert!(n >= 1 && m.len() == n * n, "not an n × n matrix");
    // vnl_fortran_copy: the Fortran array holds M itself.
    let mut a = Fortran {
        n,
        data: vec![0.0; n * n],
    };
    for r in 0..n {
        for c in 0..n {
            a.set(r + 1, c + 1, m[r * n + c]);
        }
    }
    let (low, igh) = balanc(&mut a);
    elmhes(&mut a, low, igh);
    let mut wr = vec![0.0f64; n + 1];
    let mut wi = vec![0.0f64; n + 1];
    let ierr = hqr2_values(&mut a, low, igh, &mut wr, &mut wi);
    (wr[1..].to_vec(), wi[1..].to_vec(), ierr)
}

/// EISPACK's `balanc`: permutations isolating eigenvalues, then scaling by powers of 16.
/// Returns `(low, igh)`.
fn balanc(a: &mut Fortran) -> (usize, usize) {
    let n = a.n;
    let radix = 16.0f64;
    let b2 = radix * radix;
    let mut k = 1usize;
    let mut l = n;
    // The exchange of row and column j with m (L20). The scale factors are not needed.
    let exchange = |a: &mut Fortran, j: usize, m: usize, k: usize, l: usize| {
        if j == m {
            return;
        }
        for i in 1..=l {
            a.swap(i, j, i, m);
        }
        for i in k..=n {
            a.swap(j, i, m, i);
        }
    };
    // Rows isolating an eigenvalue are pushed down (L100, then L80).
    'rows: loop {
        for jj in 1..=l {
            let j = l + 1 - jj;
            let mut isolated = true;
            for i in 1..=l {
                if i == j {
                    continue;
                }
                if a.at(j, i) != 0.0 {
                    isolated = false;
                    break;
                }
            }
            if isolated {
                exchange(a, j, l, k, l);
                // L80
                if l == 1 {
                    return (k, l);
                }
                l -= 1;
                continue 'rows;
            }
        }
        break;
    }
    // Columns isolating an eigenvalue are pushed left (L140, then L130).
    'cols: loop {
        for j in k..=l {
            let mut isolated = true;
            for i in k..=l {
                if i == j {
                    continue;
                }
                if a.at(i, j) != 0.0 {
                    isolated = false;
                    break;
                }
            }
            if isolated {
                exchange(a, j, k, k, l);
                k += 1;
                continue 'cols;
            }
        }
        break;
    }
    // Balance the submatrix in rows k to l (L190).
    loop {
        let mut noconv = false;
        for i in k..=l {
            let mut c = 0.0f64;
            let mut r = 0.0f64;
            for j in k..=l {
                if j == i {
                    continue;
                }
                c += a.at(j, i).abs();
                r += a.at(i, j).abs();
            }
            if c == 0.0 || r == 0.0 {
                continue;
            }
            let mut g = r / radix;
            let mut f = 1.0f64;
            let s = c + r;
            while c < g {
                f *= radix;
                c *= b2;
            }
            g = r * radix;
            while c >= g {
                f /= radix;
                c /= b2;
            }
            if (c + r) / f >= s * 0.95 {
                continue;
            }
            g = 1.0 / f;
            noconv = true;
            for j in k..=n {
                let v = a.at(i, j) * g;
                a.set(i, j, v);
            }
            for j in 1..=l {
                let v = a.at(j, i) * f;
                a.set(j, i, v);
            }
        }
        if !noconv {
            break;
        }
    }
    (k, l)
}

/// EISPACK's `elmhes`: reduction to upper Hessenberg form by stabilised elementary
/// similarity transformations (rows and columns `low..=igh`). The multipliers are kept
/// below the sub-diagonal, as in EISPACK (`eltran` reads them; `hqr2` sets them to 0 where
/// it needs to).
fn elmhes(a: &mut Fortran, low: usize, igh: usize) {
    let n = a.n;
    let la = igh as isize - 1;
    let kp1 = low + 1;
    if la < kp1 as isize {
        return;
    }
    for m in kp1..=la as usize {
        let mm1 = m - 1;
        let mut x = 0.0f64;
        let mut i = m;
        for j in m..=igh {
            if a.at(j, mm1).abs() <= x.abs() {
                continue;
            }
            x = a.at(j, mm1);
            i = j;
        }
        if i != m {
            for j in mm1..=n {
                a.swap(i, j, m, j);
            }
            for j in 1..=igh {
                a.swap(j, i, j, m);
            }
        }
        if x == 0.0 {
            continue;
        }
        let mp1 = m + 1;
        for i in mp1..=igh {
            let mut y = a.at(i, mm1);
            if y == 0.0 {
                continue;
            }
            y /= x;
            a.set(i, mm1, y);
            for j in m..=n {
                let v = a.at(i, j) - y * a.at(m, j);
                a.set(i, j, v);
            }
            for j in 1..=igh {
                let v = a.at(j, m) + y * a.at(j, i);
                a.set(j, m, v);
            }
        }
    }
}

/// The eigenvalue part of EISPACK's `hqr2` on the Hessenberg matrix `h`: the QR iterations
/// with every update of `h` that `hqr2` makes (rows to `n`, columns from 1), without the
/// transformations it accumulates for the eigenvectors. Returns `ierr`.
fn hqr2_values(h: &mut Fortran, low: usize, igh: usize, wr: &mut [f64], wi: &mut [f64]) -> usize {
    let n = h.n;
    let mut norm = 0.0f64;
    let mut k = 1usize;
    // Roots isolated by balanc, and the matrix norm.
    for i in 1..=n {
        for j in k..=n {
            norm += h.at(i, j).abs();
        }
        k = i;
        if i >= low && i <= igh {
            continue;
        }
        wr[i] = h.at(i, i);
        wi[i] = 0.0;
    }
    let mut en = igh as isize;
    let mut t = 0.0f64;
    let mut itn = n * 30;
    // L60: the next eigenvalues.
    while en >= low as isize {
        let enu = en as usize;
        let mut its = 0usize;
        let na = enu - 1;
        let enm2 = na as isize - 1;
        loop {
            // L70: a single small sub-diagonal element, for l = en step -1 until low.
            let mut l = low;
            for ll in low..=enu {
                l = enu + low - ll;
                if l == low {
                    break;
                }
                let mut s = h.at(l - 1, l - 1).abs() + h.at(l, l).abs();
                if s == 0.0 {
                    s = norm;
                }
                let tst1 = s;
                let tst2 = tst1 + h.at(l, l - 1).abs();
                if tst2 == tst1 {
                    break;
                }
            }
            // L100: form the shift.
            let mut x = h.at(enu, enu);
            if l == enu {
                // L270: one root found.
                h.set(enu, enu, x + t);
                wr[enu] = h.at(enu, enu);
                wi[enu] = 0.0;
                en = na as isize;
                break;
            }
            let mut y = h.at(na, na);
            let mut w = h.at(enu, na) * h.at(na, enu);
            if l == na {
                // L280: two roots found.
                let mut p = (y - x) / 2.0;
                let mut q = p * p + w;
                let mut zz = q.abs().sqrt();
                h.set(enu, enu, x + t);
                x = h.at(enu, enu);
                h.set(na, na, y + t);
                if q < 0.0 {
                    // L320: a complex pair.
                    wr[na] = x + p;
                    wr[enu] = x + p;
                    wi[na] = zz;
                    wi[enu] = -zz;
                } else {
                    // A real pair.
                    zz = p + d_sign(zz, p);
                    wr[na] = x + zz;
                    wr[enu] = wr[na];
                    if zz != 0.0 {
                        wr[enu] = x - w / zz;
                    }
                    wi[na] = 0.0;
                    wi[enu] = 0.0;
                    x = h.at(enu, na);
                    let s = x.abs() + zz.abs();
                    p = x / s;
                    q = zz / s;
                    let r = (p * p + q * q).sqrt();
                    p /= r;
                    q /= r;
                    for j in na..=n {
                        zz = h.at(na, j);
                        let hen = h.at(enu, j);
                        h.set(na, j, q * zz + p * hen);
                        h.set(enu, j, q * hen - p * zz);
                    }
                    for i in 1..=enu {
                        zz = h.at(i, na);
                        let hen = h.at(i, enu);
                        h.set(i, na, q * zz + p * hen);
                        h.set(i, enu, q * hen - p * zz);
                    }
                }
                // L330
                en = enm2;
                break;
            }
            if itn == 0 {
                return enu;
            }
            if its == 10 || its == 20 {
                // An exceptional shift.
                t += x;
                for i in low..=enu {
                    let v = h.at(i, i) - x;
                    h.set(i, i, v);
                }
                let s = h.at(enu, na).abs() + h.at(na, enm2 as usize).abs();
                x = s * 0.75;
                y = x;
                w = s * -0.4375 * s;
            }
            // L130
            its += 1;
            itn -= 1;
            // Two consecutive small sub-diagonal elements, for m = en-2 step -1 until l.
            let enm2u = enm2 as usize;
            let (mut m, mut p, mut q, mut r) = (enm2u, 0.0f64, 0.0f64, 0.0f64);
            for mm in l..=enm2u {
                m = enm2u + l - mm;
                let zz = h.at(m, m);
                r = x - zz;
                let s = y - zz;
                p = (r * s - w) / h.at(m + 1, m) + h.at(m, m + 1);
                q = h.at(m + 1, m + 1) - zz - r - s;
                r = h.at(m + 2, m + 1);
                let s = p.abs() + q.abs() + r.abs();
                p /= s;
                q /= s;
                r /= s;
                if m == l {
                    break;
                }
                let tst1 =
                    p.abs() * (h.at(m - 1, m - 1).abs() + zz.abs() + h.at(m + 1, m + 1).abs());
                let tst2 = tst1 + h.at(m, m - 1).abs() * (q.abs() + r.abs());
                if tst2 == tst1 {
                    break;
                }
            }
            // L150
            let mp2 = m + 2;
            for i in mp2..=enu {
                h.set(i, i - 2, 0.0);
                if i == mp2 {
                    continue;
                }
                h.set(i, i - 3, 0.0);
            }
            // The double QR step on rows l to en and columns m to en.
            for k in m..=na {
                let notlas = k != na;
                if k != m {
                    p = h.at(k, k - 1);
                    q = h.at(k + 1, k - 1);
                    r = 0.0;
                    if notlas {
                        r = h.at(k + 2, k - 1);
                    }
                    x = p.abs() + q.abs() + r.abs();
                    if x == 0.0 {
                        continue;
                    }
                    p /= x;
                    q /= x;
                    r /= x;
                }
                // L170
                let s = d_sign((p * p + q * q + r * r).sqrt(), p);
                if k != m {
                    h.set(k, k - 1, -s * x);
                } else if l != m {
                    let v = -h.at(k, k - 1);
                    h.set(k, k - 1, v);
                }
                // L190
                p += s;
                x = p / s;
                y = q / s;
                let zz = r / s;
                q /= p;
                r /= p;
                let jmax = enu.min(k + 3);
                if !notlas {
                    for j in k..=n {
                        let pp = h.at(k, j) + q * h.at(k + 1, j);
                        let v0 = h.at(k, j) - pp * x;
                        let v1 = h.at(k + 1, j) - pp * y;
                        h.set(k, j, v0);
                        h.set(k + 1, j, v1);
                    }
                    for i in 1..=jmax {
                        let pp = x * h.at(i, k) + y * h.at(i, k + 1);
                        let v0 = h.at(i, k) - pp;
                        let v1 = h.at(i, k + 1) - pp * q;
                        h.set(i, k, v0);
                        h.set(i, k + 1, v1);
                    }
                } else {
                    for j in k..=n {
                        let pp = h.at(k, j) + q * h.at(k + 1, j) + r * h.at(k + 2, j);
                        let v0 = h.at(k, j) - pp * x;
                        let v1 = h.at(k + 1, j) - pp * y;
                        let v2 = h.at(k + 2, j) - pp * zz;
                        h.set(k, j, v0);
                        h.set(k + 1, j, v1);
                        h.set(k + 2, j, v2);
                    }
                    for i in 1..=jmax {
                        let pp = x * h.at(i, k) + y * h.at(i, k + 1) + zz * h.at(i, k + 2);
                        let v0 = h.at(i, k) - pp;
                        let v1 = h.at(i, k + 1) - pp * q;
                        let v2 = h.at(i, k + 2) - pp * r;
                        h.set(i, k, v0);
                        h.set(i, k + 1, v1);
                        h.set(i, k + 2, v2);
                    }
                }
            }
            // goto L70
        }
    }
    0
}

/// The product of `vnl_real_eigensystem(M)`'s eigenvalues as `ImageMomentsCalculator`
/// forms it: `std::complex<double> det(1, 0)`, multiplied by each eigenvalue in order
/// (`(a + bi)(c + di) = (ac − bd) + (ad + bc)i`); a complex pair is `(wr[c], ±wi[c])`, as vnl
/// copies it. Returns the product, or `None` if `hqr2` failed.
pub fn real_eigenvalue_product(m: &[f64], n: usize) -> Option<(f64, f64)> {
    let (wr, wi, ierr) = real_eigenvalues(m, n);
    if ierr != 0 {
        return None;
    }
    let mut d: Vec<(f64, f64)> = vec![(0.0, 0.0); n];
    let mut c = 0;
    while c < n {
        d[c] = (wr[c], wi[c]);
        if wi[c] != 0.0 && c + 1 < n {
            d[c + 1] = (wr[c], -wi[c]);
            c += 1;
        }
        c += 1;
    }
    let (mut re, mut im) = (1.0f64, 0.0f64);
    for &(c, dd) in &d {
        let (a, b) = (re, im);
        re = a * c - b * dd;
        im = a * dd + b * c;
    }
    Some((re, im))
}

// ------------------------------------------------------------------------------------------------
// vnl_real_eigensystem with its eigenvectors (FreeSurfer's OpenNonSymmetricEigenSystem), ITK
// 4.13.2: rg with matz = 1 (balanc, elmhes, eltran, hqr2, balbak), statement by statement, the f2c
// macros abs, min and max with their C semantics.

/// f2c's `abs(x)`: `x >= 0 ? x : -x` (keeps `-0`; flips a NaN's sign).
#[inline]
fn abs_m(x: f64) -> f64 {
    if x >= 0.0 { x } else { -x }
}

/// f2c's `max(a, b)`: `a >= b ? a : b`.
#[inline]
fn max_m(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// `vnl_real_eigensystem(M)`: `D` (complex eigenvalues, as `(re, im)`), `V` (complex
/// eigenvectors as columns, row-major) and `Vreal` (the real eigenvectors' columns, row-major).
#[derive(Clone, Debug, PartialEq)]
pub struct RealEigensystem {
    pub d_re: Vec<f64>,
    pub d_im: Vec<f64>,
    /// `V(r, c)` at `[r * n + c]`, real and imaginary parts.
    pub v_re: Vec<f64>,
    pub v_im: Vec<f64>,
    /// `Vreal(r, c)` at `[r * n + c]`; the columns of a complex pair are 0 (VNL never writes them:
    /// memory contents, ITK-110), and [`RealEigensystem::vreal_written`] says which are written.
    pub vreal: Vec<f64>,
    pub vreal_written: Vec<bool>,
    /// EISPACK's `ierr`: 0, or the index of the eigenvalue `hqr2` failed on (then VNL prints
    /// [`real_eigensystem_failure_message`]; the eigenvalues it had not found are memory
    /// contents in VNL, 0 here, ITK-111).
    pub ierr: usize,
    pub notices: Vec<crate::defects::Notice>,
}

/// The message VNL writes on standard error when `hqr2` fails: the index, then the matrix by
/// `vnl_matrix`'s `operator<<` and `std::endl`.
pub fn real_eigensystem_failure_message(m: &[f64], n: usize, ierr: usize) -> String {
    format!(
        " *** vnl_real_eigensystem: Failed on {ierr}th eigenvalue\n{}\n",
        crate::vnl_cholesky::matrix_text(m, n, n)
    )
}

/// EISPACK's `balanc` would never return (ITK-112): an infinite or NaN entry, or entries so large
/// that a row's or column's scaled sum overflows, in the part it balances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BalancNeverReturns;

/// [`RealEigensystem`] of the `n × n` matrix `m` (row-major), or [`BalancNeverReturns`] where
/// VNL's `balanc` loops for ever.
pub fn real_eigensystem(m: &[f64], n: usize) -> Result<RealEigensystem, BalancNeverReturns> {
    use crate::defects::{ITK_110, ITK_111, Notice, UpstreamDefects, push_once};
    assert!(n >= 1 && m.len() == n * n, "not an n × n matrix");
    let mut a = Fortran {
        n,
        data: vec![0.0; n * n],
    };
    for r in 0..n {
        for c in 0..n {
            a.set(r + 1, c + 1, m[r * n + c]);
        }
    }
    let mut scale = vec![0.0f64; n + 1];
    let (low, igh) = balanc_full(&mut a, &mut scale)?;
    let mut int = vec![0usize; n + 1];
    elmhes_full(&mut a, low, igh, &mut int);
    let mut z = Fortran {
        n,
        data: vec![0.0; n * n],
    };
    eltran(&a, low, igh, &int, &mut z);
    // wr, wi: vnl_vector(n), uninitialised; hqr2 writes every entry unless it fails.
    let mut wr = vec![0.0f64; n + 1];
    let mut wi = vec![0.0f64; n + 1];
    let mut found = vec![false; n + 1];
    let ierr = hqr2_full(&mut a, low, igh, &mut wr, &mut wi, &mut z, &mut found);
    let mut notices = Vec::new();
    if ierr == 0 {
        balbak(n, low, igh, &scale, n, &mut z);
    } else if found[1..].iter().any(|f| !f) {
        push_once(
            &mut notices,
            Notice::new(
                &ITK_111,
                UpstreamDefects::Fix,
                "hqr2 failed: the eigenvalues it had not found are memory contents in VNL, 0 here",
            ),
        );
    }
    let mut d_re = vec![0.0f64; n];
    let mut d_im = vec![0.0f64; n];
    let mut v_re = vec![0.0f64; n * n];
    let mut v_im = vec![0.0f64; n * n];
    let mut vreal = vec![0.0f64; n * n];
    let mut vreal_written = vec![false; n];
    // devout(c, r) is z(r + 1, c + 1).
    let mut c = 0usize;
    while c < n {
        d_re[c] = wr[c + 1];
        d_im[c] = wi[c + 1];
        if wi[c + 1] != 0.0 && c + 1 < n {
            d_re[c + 1] = wr[c + 1];
            d_im[c + 1] = -wi[c + 1];
            for r in 0..n {
                v_re[r * n + c] = z.at(r + 1, c + 1);
                v_im[r * n + c] = z.at(r + 1, c + 2);
                v_re[r * n + c + 1] = z.at(r + 1, c + 1);
                v_im[r * n + c + 1] = -z.at(r + 1, c + 2);
            }
            push_once(
                &mut notices,
                Notice::new(
                    &ITK_110,
                    UpstreamDefects::Fix,
                    "Vreal's columns of a complex pair (never written by VNL: memory contents) are 0",
                ),
            );
            c += 1;
        } else {
            for r in 0..n {
                v_re[r * n + c] = z.at(r + 1, c + 1);
                v_im[r * n + c] = 0.0;
                vreal[r * n + c] = z.at(r + 1, c + 1);
            }
            vreal_written[c] = true;
        }
        c += 1;
    }
    Ok(RealEigensystem {
        d_re,
        d_im,
        v_re,
        v_im,
        vreal,
        vreal_written,
        ierr,
        notices,
    })
}

/// `balanc` with its scale factors; `Err` where it never returns.
fn balanc_full(a: &mut Fortran, scale: &mut [f64]) -> Result<(usize, usize), BalancNeverReturns> {
    let n = a.n;
    let radix = 16.0f64;
    let b2 = radix * radix;
    let mut k = 1usize;
    let mut l = n;
    // L20: exchange row and column j with m; iexc says where to go back.
    let exchange = |a: &mut Fortran, scale: &mut [f64], j: usize, m: usize, k: usize, l: usize| {
        scale[m] = j as f64;
        if j != m {
            for i in 1..=l {
                let f = a.at(i, j);
                a.set(i, j, a.at(i, m));
                a.set(i, m, f);
            }
            for i in k..=n {
                let f = a.at(j, i);
                a.set(j, i, a.at(m, i));
                a.set(m, i, f);
            }
        }
    };
    // L100: rows isolating an eigenvalue, pushed down.
    'l100: loop {
        let mut jj = 1;
        while jj <= l {
            let j = l + 1 - jj;
            let mut isolated = true;
            for i in 1..=l {
                if i == j {
                    continue;
                }
                if a.at(j, i) != 0.0 {
                    isolated = false;
                    break;
                }
            }
            if isolated {
                exchange(a, scale, j, l, k, l);
                // L80.
                if l == 1 {
                    return Ok((k, l));
                }
                l -= 1;
                continue 'l100;
            }
            jj += 1;
        }
        break;
    }
    // L140: columns isolating an eigenvalue, pushed left.
    'l140: loop {
        for j in k..=l {
            let mut isolated = true;
            for i in k..=l {
                if i == j {
                    continue;
                }
                if a.at(i, j) != 0.0 {
                    isolated = false;
                    break;
                }
            }
            if isolated {
                exchange(a, scale, j, k, k, l);
                // L130.
                k += 1;
                continue 'l140;
            }
        }
        break;
    }
    for s in scale.iter_mut().take(l + 1).skip(k) {
        *s = 1.0;
    }
    // L190: iterative balancing of rows and columns k to l.
    loop {
        let mut noconv = false;
        for i in k..=l {
            let mut c = 0.0f64;
            let mut r = 0.0f64;
            for j in k..=l {
                if j == i {
                    continue;
                }
                c += abs_m(a.at(j, i));
                r += abs_m(a.at(i, j));
            }
            if c == 0.0 || r == 0.0 {
                continue;
            }
            let mut g = r / radix;
            let mut f = 1.0f64;
            let s = c + r;
            // L210: never ends on a NaN c or g.
            if c.is_nan() || g.is_nan() {
                return Err(BalancNeverReturns);
            }
            while !(c >= g) {
                f *= radix;
                c *= b2;
            }
            g = r * radix;
            // L230: never ends on an infinite or NaN c, or a NaN g.
            if !c.is_finite() || g.is_nan() {
                return Err(BalancNeverReturns);
            }
            while !(c < g) {
                f /= radix;
                c /= b2;
            }
            if (c + r) / f >= s * 0.95 {
                continue;
            }
            g = 1.0 / f;
            scale[i] *= f;
            noconv = true;
            for j in k..=n {
                let v = a.at(i, j) * g;
                a.set(i, j, v);
            }
            for j in 1..=l {
                let v = a.at(j, i) * f;
                a.set(j, i, v);
            }
        }
        if !noconv {
            break;
        }
    }
    Ok((k, l))
}

/// `elmhes` with its interchanges `int`.
fn elmhes_full(a: &mut Fortran, low: usize, igh: usize, int: &mut [usize]) {
    let n = a.n;
    let la = igh as isize - 1;
    let kp1 = low + 1;
    if la < kp1 as isize {
        return;
    }
    for m in kp1..=la as usize {
        let mm1 = m - 1;
        let mut x = 0.0f64;
        let mut i = m;
        for j in m..=igh {
            if abs_m(a.at(j, mm1)) <= abs_m(x) {
                continue;
            }
            x = a.at(j, mm1);
            i = j;
        }
        int[m] = i;
        if i != m {
            for j in mm1..=n {
                let y = a.at(i, j);
                a.set(i, j, a.at(m, j));
                a.set(m, j, y);
            }
            for j in 1..=igh {
                let y = a.at(j, i);
                a.set(j, i, a.at(j, m));
                a.set(j, m, y);
            }
        }
        if x == 0.0 {
            continue;
        }
        for i in m + 1..=igh {
            let mut y = a.at(i, mm1);
            if y == 0.0 {
                continue;
            }
            y /= x;
            a.set(i, mm1, y);
            for j in m..=n {
                let v = a.at(i, j) - y * a.at(m, j);
                a.set(i, j, v);
            }
            for j in 1..=igh {
                let v = a.at(j, m) + y * a.at(j, i);
                a.set(j, m, v);
            }
        }
    }
}

/// `eltran`: the transformations of `elmhes` accumulated into `z`.
fn eltran(a: &Fortran, low: usize, igh: usize, int: &[usize], z: &mut Fortran) {
    let n = a.n;
    for j in 1..=n {
        for i in 1..=n {
            z.set(i, j, 0.0);
        }
        z.set(j, j, 1.0);
    }
    let kl = igh as isize - low as isize - 1;
    if kl < 1 {
        return;
    }
    for mm in 1..=kl as usize {
        let mp = igh - mm;
        for i in mp + 1..=igh {
            z.set(i, mp, a.at(i, mp - 1));
        }
        let i = int[mp];
        if i == mp {
            continue;
        }
        for j in mp..=igh {
            z.set(mp, j, z.at(i, j));
            z.set(i, j, 0.0);
        }
        z.set(i, mp, 1.0);
    }
}

/// `cdiv`: the complex quotient `(ar + i·ai) / (br + i·bi)`.
fn cdiv(ar: f64, ai: f64, br: f64, bi: f64) -> (f64, f64) {
    let s = abs_m(br) + abs_m(bi);
    let ars = ar / s;
    let ais = ai / s;
    let brs = br / s;
    let bis = bi / s;
    let s = brs * brs + bis * bis;
    ((ars * brs + ais * bis) / s, (ais * brs - ars * bis) / s)
}

/// f2c's `d_sign(a, b)`: `|a|` with the sign of `b`.
#[inline]
fn d_sign_f(a: f64, b: f64) -> f64 {
    let x = if a >= 0.0 { a } else { -a };
    if b >= 0.0 { x } else { -x }
}

/// `hqr2`: the eigenvalues of the Hessenberg matrix `h` and the eigenvectors (back-substituted
/// and transformed by `z`). `found[i]` says whether `wr[i]`, `wi[i]` were written. Returns `ierr`.
#[allow(clippy::many_single_char_names, unused_assignments)]
fn hqr2_full(
    h: &mut Fortran,
    low: usize,
    igh: usize,
    wr: &mut [f64],
    wi: &mut [f64],
    z: &mut Fortran,
    found: &mut [bool],
) -> usize {
    let n = h.n;
    let mut norm = 0.0f64;
    let mut k = 1usize;
    for i in 1..=n {
        for j in k..=n {
            norm += abs_m(h.at(i, j));
        }
        k = i;
        if i >= low && i <= igh {
            continue;
        }
        wr[i] = h.at(i, i);
        wi[i] = 0.0;
        found[i] = true;
    }
    let mut en = igh as isize;
    let mut t = 0.0f64;
    let mut itn = (n * 30) as isize;
    // Values that persist across the loops, as the C locals do.
    let (mut p, mut q, mut r, mut s, mut w, mut x, mut y, mut zz) = (
        0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64,
    );
    let mut m: usize;
    let mut l: usize;
    // L60.
    while en >= low as isize {
        let enu = en as usize;
        let mut its = 0;
        let na = enu - 1;
        let enm2 = na as isize - 1;
        loop {
            // L70.
            l = low;
            let mut ll = low;
            while ll <= enu {
                l = enu + low - ll;
                if l == low {
                    break;
                }
                s = abs_m(h.at(l - 1, l - 1)) + abs_m(h.at(l, l));
                if s == 0.0 {
                    s = norm;
                }
                let tst1 = s;
                let tst2 = tst1 + abs_m(h.at(l, l - 1));
                if tst2 == tst1 {
                    break;
                }
                ll += 1;
            }
            // L100.
            x = h.at(enu, enu);
            if l == enu {
                // L270.
                h.set(enu, enu, x + t);
                wr[enu] = h.at(enu, enu);
                wi[enu] = 0.0;
                found[enu] = true;
                en = na as isize;
                break;
            }
            y = h.at(na, na);
            w = h.at(enu, na) * h.at(na, enu);
            if l == na {
                // L280.
                p = (y - x) / 2.0;
                q = p * p + w;
                zz = abs_m(q).sqrt();
                h.set(enu, enu, x + t);
                x = h.at(enu, enu);
                h.set(na, na, y + t);
                if q < 0.0 {
                    // L320.
                    wr[na] = x + p;
                    wr[enu] = x + p;
                    wi[na] = zz;
                    wi[enu] = -zz;
                } else {
                    zz = p + d_sign_f(zz, p);
                    wr[na] = x + zz;
                    wr[enu] = wr[na];
                    if zz != 0.0 {
                        wr[enu] = x - w / zz;
                    }
                    wi[na] = 0.0;
                    wi[enu] = 0.0;
                    x = h.at(enu, na);
                    s = abs_m(x) + abs_m(zz);
                    p = x / s;
                    q = zz / s;
                    r = (p * p + q * q).sqrt();
                    p /= r;
                    q /= r;
                    for j in na..=n {
                        zz = h.at(na, j);
                        h.set(na, j, q * zz + p * h.at(enu, j));
                        h.set(enu, j, q * h.at(enu, j) - p * zz);
                    }
                    for i in 1..=enu {
                        zz = h.at(i, na);
                        h.set(i, na, q * zz + p * h.at(i, enu));
                        h.set(i, enu, q * h.at(i, enu) - p * zz);
                    }
                    for i in low..=igh {
                        zz = z.at(i, na);
                        z.set(i, na, q * zz + p * z.at(i, enu));
                        z.set(i, enu, q * z.at(i, enu) - p * zz);
                    }
                }
                found[na] = true;
                found[enu] = true;
                // L330.
                en = enm2;
                break;
            }
            if itn == 0 {
                // L1000.
                return enu;
            }
            if its == 10 || its == 20 {
                t += x;
                for i in low..=enu {
                    let v = h.at(i, i) - x;
                    h.set(i, i, v);
                }
                s = abs_m(h.at(enu, na)) + abs_m(h.at(na, enm2 as usize));
                x = s * 0.75;
                y = x;
                w = s * -0.4375 * s;
            }
            // L130.
            its += 1;
            itn -= 1;
            let enm2u = enm2 as usize;
            m = enm2u;
            let mut mm = l;
            while mm <= enm2u {
                m = enm2u + l - mm;
                zz = h.at(m, m);
                r = x - zz;
                s = y - zz;
                p = (r * s - w) / h.at(m + 1, m) + h.at(m, m + 1);
                q = h.at(m + 1, m + 1) - zz - r - s;
                r = h.at(m + 2, m + 1);
                s = abs_m(p) + abs_m(q) + abs_m(r);
                p /= s;
                q /= s;
                r /= s;
                if m == l {
                    break;
                }
                let tst1 =
                    abs_m(p) * (abs_m(h.at(m - 1, m - 1)) + abs_m(zz) + abs_m(h.at(m + 1, m + 1)));
                let tst2 = tst1 + abs_m(h.at(m, m - 1)) * (abs_m(q) + abs_m(r));
                if tst2 == tst1 {
                    break;
                }
                mm += 1;
            }
            // L150.
            let mp2 = m + 2;
            for i in mp2..=enu {
                h.set(i, i - 2, 0.0);
                if i == mp2 {
                    continue;
                }
                h.set(i, i - 3, 0.0);
            }
            for k in m..=na {
                let notlas = k != na;
                if k != m {
                    p = h.at(k, k - 1);
                    q = h.at(k + 1, k - 1);
                    r = 0.0;
                    if notlas {
                        r = h.at(k + 2, k - 1);
                    }
                    x = abs_m(p) + abs_m(q) + abs_m(r);
                    if x == 0.0 {
                        continue;
                    }
                    p /= x;
                    q /= x;
                    r /= x;
                }
                // L170.
                s = d_sign_f((p * p + q * q + r * r).sqrt(), p);
                if k != m {
                    h.set(k, k - 1, -s * x);
                } else if l != m {
                    let v = -h.at(k, k - 1);
                    h.set(k, k - 1, v);
                }
                // L190.
                p += s;
                x = p / s;
                y = q / s;
                zz = r / s;
                q /= p;
                r /= p;
                let jmax = enu.min(k + 3);
                if !notlas {
                    for j in k..=n {
                        p = h.at(k, j) + q * h.at(k + 1, j);
                        h.set(k, j, h.at(k, j) - p * x);
                        h.set(k + 1, j, h.at(k + 1, j) - p * y);
                    }
                    for i in 1..=jmax {
                        p = x * h.at(i, k) + y * h.at(i, k + 1);
                        h.set(i, k, h.at(i, k) - p);
                        h.set(i, k + 1, h.at(i, k + 1) - p * q);
                    }
                    for i in low..=igh {
                        p = x * z.at(i, k) + y * z.at(i, k + 1);
                        z.set(i, k, z.at(i, k) - p);
                        z.set(i, k + 1, z.at(i, k + 1) - p * q);
                    }
                } else {
                    for j in k..=n {
                        p = h.at(k, j) + q * h.at(k + 1, j) + r * h.at(k + 2, j);
                        h.set(k, j, h.at(k, j) - p * x);
                        h.set(k + 1, j, h.at(k + 1, j) - p * y);
                        h.set(k + 2, j, h.at(k + 2, j) - p * zz);
                    }
                    for i in 1..=jmax {
                        p = x * h.at(i, k) + y * h.at(i, k + 1) + zz * h.at(i, k + 2);
                        h.set(i, k, h.at(i, k) - p);
                        h.set(i, k + 1, h.at(i, k + 1) - p * q);
                        h.set(i, k + 2, h.at(i, k + 2) - p * r);
                    }
                    for i in low..=igh {
                        p = x * z.at(i, k) + y * z.at(i, k + 1) + zz * z.at(i, k + 2);
                        z.set(i, k, z.at(i, k) - p);
                        z.set(i, k + 1, z.at(i, k + 1) - p * q);
                        z.set(i, k + 2, z.at(i, k + 2) - p * r);
                    }
                }
            }
            // goto L70.
        }
    }
    // L340: back-substitution.
    if norm == 0.0 {
        return 0;
    }
    for nn in 1..=n {
        let en = n + 1 - nn;
        p = wr[en];
        q = wi[en];
        let na = en as isize - 1;
        if q < 0.0 {
            // L710: the complex vector.
            let nau = na as usize;
            m = nau;
            if abs_m(h.at(en, nau)) <= abs_m(h.at(nau, en)) {
                // L720.
                let (cr, ci) = cdiv(0.0, -h.at(nau, en), h.at(nau, nau) - p, q);
                h.set(nau, nau, cr);
                h.set(nau, en, ci);
            } else {
                h.set(nau, nau, q / h.at(en, nau));
                h.set(nau, en, -(h.at(en, en) - p) / h.at(en, nau));
            }
            // L730.
            h.set(en, nau, 0.0);
            h.set(en, en, 1.0);
            let enm2 = na - 1;
            if enm2 == 0 {
                continue;
            }
            for ii in 1..=enm2 as usize {
                let i = nau - ii;
                w = h.at(i, i) - p;
                let mut ra = 0.0f64;
                let mut sa = 0.0f64;
                for j in m..=en {
                    ra += h.at(i, j) * h.at(j, nau);
                    sa += h.at(i, j) * h.at(j, en);
                }
                if wi[i] < 0.0 {
                    zz = w;
                    r = ra;
                    s = sa;
                    continue;
                }
                // L770.
                m = i;
                if wi[i] == 0.0 {
                    let (cr, ci) = cdiv(-ra, -sa, w, q);
                    h.set(i, nau, cr);
                    h.set(i, en, ci);
                } else {
                    // L780.
                    x = h.at(i, i + 1);
                    y = h.at(i + 1, i);
                    let mut vr = (wr[i] - p) * (wr[i] - p) + wi[i] * wi[i] - q * q;
                    let vi = (wr[i] - p) * 2.0 * q;
                    if !(vr != 0.0 || vi != 0.0) {
                        let tst1 = norm * (abs_m(w) + abs_m(q) + abs_m(x) + abs_m(y) + abs_m(zz));
                        vr = tst1;
                        loop {
                            vr *= 0.01;
                            let tst2 = tst1 + vr;
                            if !(tst2 > tst1) {
                                break;
                            }
                        }
                    }
                    // L784.
                    let d1 = x * r - zz * ra + q * sa;
                    let d2 = x * s - zz * sa - q * ra;
                    let (cr, ci) = cdiv(d1, d2, vr, vi);
                    h.set(i, nau, cr);
                    h.set(i, en, ci);
                    if abs_m(x) > abs_m(zz) + abs_m(q) {
                        h.set(i + 1, nau, (-ra - w * h.at(i, nau) + q * h.at(i, en)) / x);
                        h.set(i + 1, en, (-sa - w * h.at(i, en) - q * h.at(i, nau)) / x);
                    } else {
                        // L785.
                        let d1 = -r - y * h.at(i, nau);
                        let d2 = -s - y * h.at(i, en);
                        let (cr, ci) = cdiv(d1, d2, zz, q);
                        h.set(i + 1, nau, cr);
                        h.set(i + 1, en, ci);
                    }
                }
                // L790: overflow control.
                t = max_m(abs_m(h.at(i, nau)), abs_m(h.at(i, en)));
                if t == 0.0 {
                    continue;
                }
                let tst1 = t;
                let tst2 = tst1 + 1.0 / tst1;
                if tst2 > tst1 {
                    continue;
                }
                for j in i..=en {
                    h.set(j, nau, h.at(j, nau) / t);
                    h.set(j, en, h.at(j, en) / t);
                }
            }
        } else if q == 0.0 {
            // L600: the real vector.
            m = en;
            h.set(en, en, 1.0);
            if na == 0 {
                continue;
            }
            for ii in 1..=na as usize {
                let i = en - ii;
                w = h.at(i, i) - p;
                r = 0.0;
                for j in m..=en {
                    r += h.at(i, j) * h.at(j, en);
                }
                if wi[i] < 0.0 {
                    zz = w;
                    s = r;
                    continue;
                }
                // L630.
                m = i;
                if wi[i] == 0.0 {
                    t = w;
                    if t == 0.0 {
                        let tst1 = norm;
                        t = tst1;
                        loop {
                            t *= 0.01;
                            let tst2 = norm + t;
                            if !(tst2 > tst1) {
                                break;
                            }
                        }
                    }
                    // L635.
                    h.set(i, en, -r / t);
                } else {
                    // L640.
                    x = h.at(i, i + 1);
                    y = h.at(i + 1, i);
                    q = (wr[i] - p) * (wr[i] - p) + wi[i] * wi[i];
                    t = (x * s - zz * r) / q;
                    h.set(i, en, t);
                    if abs_m(x) <= abs_m(zz) {
                        h.set(i + 1, en, (-s - y * t) / zz);
                    } else {
                        h.set(i + 1, en, (-r - w * t) / x);
                    }
                }
                // L680: overflow control.
                t = abs_m(h.at(i, en));
                if t == 0.0 {
                    continue;
                }
                let tst1 = t;
                let tst2 = tst1 + 1.0 / tst1;
                if tst2 > tst1 {
                    continue;
                }
                for j in i..=en {
                    h.set(j, en, h.at(j, en) / t);
                }
            }
        }
        // q > 0 or NaN: L800.
    }
    // The vectors of isolated roots.
    for i in 1..=n {
        if i >= low && i <= igh {
            continue;
        }
        for j in i..=n {
            z.set(i, j, h.at(i, j));
        }
    }
    // Multiply by the transformation matrix.
    for jj in low..=n {
        let j = n + low - jj;
        let mm = j.min(igh);
        for i in low..=igh {
            zz = 0.0;
            for k in low..=mm {
                zz += z.at(i, k) * h.at(k, j);
            }
            z.set(i, j, zz);
        }
    }
    0
}

/// `balbak`: undoes `balanc`'s scaling and permutations on the `m` columns of `z`.
fn balbak(n: usize, low: usize, igh: usize, scale: &[f64], m: usize, z: &mut Fortran) {
    if m == 0 {
        return;
    }
    if igh != low {
        for i in low..=igh {
            let s = scale[i];
            for j in 1..=m {
                let v = z.at(i, j) * s;
                z.set(i, j, v);
            }
        }
    }
    for ii in 1..=n {
        let mut i = ii;
        if i >= low && i <= igh {
            continue;
        }
        if i < low {
            i = low - ii;
        }
        let k = scale[i] as usize;
        if k == i {
            continue;
        }
        for j in 1..=m {
            let s = z.at(i, j);
            z.set(i, j, z.at(k, j));
            z.set(k, j, s);
        }
    }
}

/// `vnl_symmetric_eigensystem<float>(A)` as FreeSurfer's `OpenEigenSystem` reads it: EISPACK's
/// `rs` in double on the matrix converted to double ([`symmetric_eigensystem`]), the eigenvalues
/// and eigenvectors rounded to float. `ierr` as in [`SymmetricEigen`] (VNL then prints
/// `vnl_symmetric_eigensystem: ierr = …` and leaves `V` unset: memory contents, here the double
/// vectors as computed).
pub fn symmetric_eigensystem_f32(a: &[f32], n: usize) -> (Vec<f32>, Vec<f32>, usize) {
    let ad: Vec<f64> = a.iter().map(|&v| f64::from(v)).collect();
    let e = symmetric_eigensystem(&ad, n);
    let values = e.values.iter().map(|&v| v as f32).collect();
    let vectors = e.vectors.iter().map(|&v| v as f32).collect();
    (values, vectors, e.ierr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pythag_is_the_hypotenuse() {
        assert!((pythag(3.0, 4.0) - 5.0).abs() < 1e-15);
        assert_eq!(pythag(0.0, 0.0), 0.0);
        assert!((pythag(1e-3, 1.0) - (1.0f64 + 1e-6).sqrt()).abs() < 1e-15);
    }

    #[test]
    fn symmetric_eigensystem_decomposes() {
        let a = [4.0, 1.0, 0.5, 1.0, 3.0, -0.25, 0.5, -0.25, 2.0];
        let e = symmetric_eigensystem(&a, 3);
        assert_eq!(e.ierr, 0);
        assert!(e.values[0] <= e.values[1] && e.values[1] <= e.values[2]);
        for c in 0..3 {
            for r in 0..3 {
                let av: f64 = (0..3).map(|k| a[r * 3 + k] * e.vectors[k * 3 + c]).sum();
                assert!((av - e.values[c] * e.vectors[r * 3 + c]).abs() < 1e-13);
            }
        }
        // A diagonal matrix: the unit vectors, sorted.
        let e = symmetric_eigensystem(&[3.0, 0.0, 0.0, 1.0], 2);
        assert_eq!(e.values, [1.0, 3.0]);
        assert_eq!(e.vectors, [0.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn real_eigenvalues_of_a_rotation() {
        let (c, s) = (0.6f64, 0.8f64);
        let m = [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0];
        let (wr, wi, ierr) = real_eigenvalues(&m, 3);
        assert_eq!(ierr, 0);
        let mut pairs: Vec<(f64, f64)> = wr.iter().copied().zip(wi.iter().copied()).collect();
        pairs.sort_by(|a, b| a.1.total_cmp(&b.1));
        assert!((pairs[0].0 - 0.6).abs() < 1e-15 && (pairs[0].1 + 0.8).abs() < 1e-15);
        assert!((pairs[1].0 - 1.0).abs() < 1e-15 && pairs[1].1 == 0.0);
        let (re, im) = real_eigenvalue_product(&m, 3).unwrap();
        assert!((re - 1.0).abs() < 1e-15 && im.abs() < 1e-15);
        // A reflection.
        let (re, _) = real_eigenvalue_product(&[0.0, 1.0, 1.0, 0.0], 2).unwrap();
        assert!((re + 1.0).abs() < 1e-15);
    }

    /// The full real eigensystem has the eigenvalues of [`real_eigenvalues`] (where both use
    /// the same `abs`), and its real eigenvectors satisfy `A v = λ v`.
    #[test]
    fn the_real_eigensystem_has_the_eigenvalues_and_vectors() {
        let mut rng = crate::x87::cases::SplitMix64::new(11);
        for _ in 0..300 {
            let n = 1 + rng.below(7) as usize;
            let m: Vec<f64> = (0..n * n)
                .map(|_| rng.range(-1000, 1000) as f64 / 97.0)
                .collect();
            let es = real_eigensystem(&m, n).unwrap();
            let (wr, wi, ierr) = real_eigenvalues(&m, n);
            assert_eq!(ierr, es.ierr);
            let mut c = 0;
            while c < n {
                assert_eq!(wr[c].to_bits(), es.d_re[c].to_bits());
                assert_eq!(wi[c].to_bits(), es.d_im[c].to_bits());
                if wi[c] != 0.0 {
                    c += 2;
                    continue;
                }
                assert!(es.vreal_written[c]);
                let norm: f64 = (0..n)
                    .map(|r| es.vreal[r * n + c].abs())
                    .fold(0.0, f64::max);
                for r in 0..n {
                    let av: f64 = (0..n).map(|k| m[r * n + k] * es.vreal[k * n + c]).sum();
                    assert!(
                        (av - wr[c] * es.vreal[r * n + c]).abs()
                            <= 1e-9 * (1.0 + norm) * (1.0 + wr[c].abs())
                    );
                }
                c += 1;
            }
        }
        assert_eq!(
            real_eigensystem(&[0.0, f64::NAN, 1.0, 0.0], 2),
            Err(BalancNeverReturns)
        );
        assert_eq!(
            real_eigensystem(&[0.0, f64::INFINITY, 1.0, 0.0], 2),
            Err(BalancNeverReturns)
        );
    }
}
