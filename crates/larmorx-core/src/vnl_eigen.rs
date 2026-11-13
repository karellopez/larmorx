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
//! - [`real_eigensystem`]: `vnl_real_eigensystem` with its eigenvectors (`D`, `V`, `Vreal`; ITK
//!   4.13.2: `rg` with
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

use crate::itk_real::ItkReal;

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

/// EISPACK's `pythag(a, b)`: `sqrt(a² + b²)` by Moler and Morrison's iteration, with f2c's
/// `abs`, `max` and `min` macros as VXL's C has them (`x >= 0 ? x : -x`, `a >= b ? a : b`,
/// `a <= b ? a : b`): of a NaN and a number, `max` and `min` give the number when the NaN is
/// `a` and the NaN when it is `b`, so `pythag(NaN, x)` is `pythag(x, x)` and `pythag(NaN, 0)` is 0;
/// `abs` keeps `-0`, so `pythag(-0, ±0)` is `-0`.
///
/// `None` where VXL's loop never ends (ITK-115): the smaller magnitude over the larger is NaN (a
/// NaN `b`, or two infinities), and `4 + r` never equals 4 again.
pub fn pythag(a: f64, b: f64) -> Option<f64> {
    let (aa, ab) = (abs_m(a), abs_m(b));
    let mut p = max_m(aa, ab);
    if p == 0.0 {
        return Some(p);
    }
    let d = min_m(aa, ab) / p;
    let mut r = d * d;
    if r.is_nan() {
        return None;
    }
    loop {
        let t = r + 4.0;
        if t == 4.0 {
            return Some(p);
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
    /// larmorx's own: ITK-115 when VNL would never return (`pythag` looping for ever inside
    /// `tql2`, on a matrix with an infinite or NaN entry). `ierr` is then the eigenvalue `tql2`
    /// was working on, and the values and vectors are those reached there.
    pub notices: Vec<crate::defects::Notice>,
}

impl SymmetricEigen {
    /// Whether VNL's eigensystem would never return on this matrix (ITK-115).
    pub fn never_returns(&self) -> bool {
        self.notices.iter().any(|n| n.defect.id == "ITK-115")
    }
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
    let (ierr, never_returns) = tql2(n, &mut d, &mut e, &mut z);
    let mut notices = Vec::new();
    if never_returns {
        notices.push(crate::defects::Notice::new(
            &crate::defects::ITK_115,
            crate::defects::UpstreamDefects::Fix,
            format!(
                "EISPACK's pythag loops for ever on eigenvalue {ierr} (a NaN or two infinities): VNL never returns; stopped there"
            ),
        ));
    }
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
        notices,
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
                    scale += abs_m(d[k]);
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
/// method, sorted ascending. Returns `ierr` and whether VNL would never return (ITK-115: `pythag`
/// loops for ever; `ierr` is then the eigenvalue it was working on, nothing sorted).
fn tql2(n: usize, d: &mut [f64], e: &mut [f64], z: &mut Fortran) -> (usize, bool) {
    if n == 1 {
        return (0, false);
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
            // e[n] is 0 at the start, but an earlier QL step leaves `0 · inf` there, a NaN,
            // where pythag returned an infinity: the search then runs past n (tst1 is never
            // NaN). The f2c code goes on with m = n + 1: it reads (and writes) d[n + 1], past
            // the array, and then calls pythag(p, e[n]) with that NaN as its second argument,
            // which never returns (ITK-115).
            return (l, true);
        }
        if m != l {
            loop {
                // L130
                if j == 30 {
                    return (l, false);
                }
                j += 1;
                let l1 = l + 1;
                let l2 = l1 + 1;
                let mut g = d[l];
                let mut p = (d[l1] - g) / (e[l] * 2.0);
                let Some(mut r) = pythag(p, 1.0) else {
                    return (l, true);
                };
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
                    let Some(ri) = pythag(p, e[i]) else {
                        return (l, true);
                    };
                    r = ri;
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
    (0, false)
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
                let v = a.at(i, j).sse_mul(g);
                a.set(i, j, v);
            }
            for j in 1..=l {
                let v = a.at(j, i).sse_mul(f);
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
            y = y.sse_div(x);
            a.set(i, mm1, y);
            for j in m..=n {
                let v = a.at(i, j).sse_sub(mul(124, y, a.at(m, j)));
                a.set(i, j, v);
            }
            for j in 1..=igh {
                let v = vpos(j - 1, igh);
                let w = add(125 + v, a.at(j, m), mul(128 + v, y, a.at(j, i)));
                a.set(j, m, w);
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
// vnl_real_eigensystem with its eigenvectors (D, V, Vreal), ITK
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

/// f2c's `min(a, b)`: `a <= b ? a : b`.
#[inline]
fn min_m(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

// The NaNs of the real eigensystem. `balanc` balances only a part without infinite or NaN entries
// (it never returns otherwise, ITK-112), but the rows above that part and the columns right of it
// (where EISPACK's isolated eigenvalues live) can hold any value, and `elmhes`, `hqr2`'s sweeps
// and two-root steps, its back-substitution and `cdiv` combine them. Every such operation is an
// SSE one (`ItkReal::sse_*`: of two NaN operands the first source's is passed on, a NaN made from
// numbers is the default NaN with the sign bit set, on every platform), in the operand order of
// the official build (ITK 4.13.2's netlib compiled by GCC 8.4.0 at `-O3`); where GCC chose it per
// statement, the order is a numbered site in [`ORDER`]. Loops GCC vectorized (two doubles at a
// time) can order the elements of the vector body and the one left over differently: their sites
// come in three ([`vpos`]). The operations of the balanced part alone stay plain: its values are
// finite.

/// The number of operand-order sites.
const SITES: usize = 206;

/// The official build's choice at each site: for `add` and `mul`, 0 the source's left operand
/// first, 1 the right one; for `neg_div`, 0 `(-a) / b`, 1 `-(a / b)`, 2 `a / (-b)`. Read from the
/// object code's behaviour on NaNs with distinct payloads (validation/probes/fs-vnl, family
/// `eignan`); a site never seen deciding a NaN keeps the source's order.
static ORDER: [u8; SITES] = {
    let mut o = [0u8; SITES];
    // cdiv: `bis * ars`
    o[9] = 1;
    // the two-root step: `d_sign(zz, p) + p`; its row loop (one by one, two at a time, the one
    // left over) and column loop (every element alike)
    o[13] = 1;
    let row: [(usize, [u8; 5]); 3] = [
        (19, [1, 0, 0, 1, 0]),
        (196, [1, 0, 1, 1, 1]),
        (201, [1, 0, 1, 1, 0]),
    ];
    let mut r = 0;
    while r < 3 {
        let mut k = 0;
        while k < 5 {
            o[row[r].0 + k] = row[r].1[k];
            k += 1;
        }
        r += 1;
    }
    // the column loop and the transformations' loop of the two-root step, every element alike
    let mut v = 0;
    while v < 3 {
        o[24 + v] = 1;
        o[30 + v] = 1;
        o[33 + v] = 1;
        o[148 + v] = 1;
        o[154 + v] = 1;
        o[157 + v] = 1;
        v += 1;
    }
    // `w + p * p`
    o[145] = 1;
    // the complex back-substitution: `h(j, na) * h(i, j)`, `(...) * norm`, `r * x`, `ra * zz`,
    // `s * x`, `h(i, en) * q`
    o[85] = 1;
    o[93] = 1;
    o[99] = 1;
    o[100] = 1;
    o[102] = 1;
    o[107] = 1;
    // the real one: `r * zz`
    o[119] = 1;
    // elmhes: `a(m, j) * y`; `a(j, i) * y + a(j, m)` (vectorized and alone), `y * a(j, i) + a(j, m)`
    // for the one left over
    o[124] = 1;
    o[125] = 1;
    o[126] = 1;
    o[127] = 1;
    o[128] = 1;
    o[130] = 1;
    // the sweep's column loops over two and three columns (the vector body; the one left over),
    // and the same loops over the transformations
    let two: [[u8; 4]; 2] = [[1, 1, 1, 1], [0, 0, 1, 1]];
    let three: [[u8; 7]; 2] = [[0, 1, 1, 1, 1, 0, 0], [0, 1, 0, 1, 1, 1, 1]];
    let mut v = 0;
    while v < 2 {
        let mut k = 0;
        while k < 4 {
            o[43 + 3 * k + v] = two[v][k];
            o[163 + 3 * k + v] = two[v][k];
            k += 1;
        }
        let mut k = 0;
        while k < 7 {
            o[62 + 3 * k + v] = three[v][k];
            o[175 + 3 * k + v] = three[v][k];
            k += 1;
        }
        v += 1;
    }
    // the transformations' loops order their one left over otherwise
    o[164] = 1;
    o[167] = 0;
    o[170] = 0;
    o[173] = 1;
    o[176] = 0;
    o[179] = 1;
    o[182] = 0;
    o[185] = 0;
    o[188] = 0;
    o[191] = 1;
    o[194] = 1;
    // the loops' one-by-one versions (a single element): as the vector body over two columns, as the
    // one left over over three
    let mut k = 0;
    while k < 4 {
        o[43 + 3 * k + 2] = two[0][k];
        o[163 + 3 * k + 2] = two[0][k];
        k += 1;
    }
    let mut k = 0;
    while k < 7 {
        o[62 + 3 * k + 2] = three[1][k];
        o[175 + 3 * k + 2] = three[1][k];
        k += 1;
    }
    // the sweep's row loops: `q * h(k + 1, j) + h(k, j)`; `(q * h(k + 1, j) + h(k, j)) +
    // h(k + 2, j) * r`, `zz * p`
    o[39] = 1;
    o[56] = 1;
    o[58] = 1;
    o[61] = 1;
    o
};

#[inline(always)]
fn order(site: usize) -> u8 {
    ORDER[site]
}

/// `a + b` at a site.
#[inline(always)]
fn add(site: usize, a: f64, b: f64) -> f64 {
    if order(site) == 0 {
        a.sse_add(b)
    } else {
        b.sse_add(a)
    }
}

/// `a * b` at a site.
#[inline(always)]
fn mul(site: usize, a: f64, b: f64) -> f64 {
    if order(site) == 0 {
        a.sse_mul(b)
    } else {
        b.sse_mul(a)
    }
}

/// `-a * b` at a site (where GCC puts the negation, and which operand comes first): 0 `(-a) * b`,
/// 1 `b * (-a)`, 2 `-(a * b)`, 3 `-(b * a)`, 4 `a * (-b)`, 5 `(-b) * a`.
#[inline(always)]
fn neg_mul(site: usize, a: f64, b: f64) -> f64 {
    match order(site) {
        0 => (-a).sse_mul(b),
        1 => b.sse_mul(-a),
        2 => -(a.sse_mul(b)),
        3 => -(b.sse_mul(a)),
        4 => a.sse_mul(-b),
        _ => (-b).sse_mul(a),
    }
}

/// `-a / b` at a site (where GCC puts the negation).
#[inline(always)]
fn neg_div(site: usize, a: f64, b: f64) -> f64 {
    match order(site) {
        0 => (-a).sse_div(b),
        1 => -(a.sse_div(b)),
        _ => a.sse_div(-b),
    }
}

/// The class of element `index` of a loop of `count` elements GCC vectorized two at a time: 0 in
/// the vector body, 1 the one left over, 2 a loop too short for the vector code.
#[inline(always)]
fn vpos(index: usize, count: usize) -> usize {
    if count < 2 {
        2
    } else if index < count - count % 2 {
        0
    } else {
        1
    }
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
    use crate::defects::{ITK_110, ITK_111, ITK_117, Notice, UpstreamDefects, push_once};
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
        } else if wi[c + 1] != 0.0 {
            // The last eigenvalue with an imaginary part that is not 0 (only a NaN): VNL reads V's
            // imaginary parts past its eigenvector matrix and writes the conjugate past D (ITK-117);
            // here they are 0, and Vreal's column stays unwritten, as VNL leaves it.
            for r in 0..n {
                v_re[r * n + c] = z.at(r + 1, c + 1);
                v_im[r * n + c] = 0.0;
            }
            push_once(
                &mut notices,
                Notice::new(
                    &ITK_117,
                    UpstreamDefects::Fix,
                    "the last eigenvalue's imaginary part is NaN: V's imaginary parts in its column (read past VNL's arrays: memory contents) are 0",
                ),
            );
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
                let v = a.at(i, j).sse_mul(g);
                a.set(i, j, v);
            }
            for j in 1..=l {
                let v = a.at(j, i).sse_mul(f);
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
            y = y.sse_div(x);
            a.set(i, mm1, y);
            for j in m..=n {
                let v = a.at(i, j).sse_sub(mul(124, y, a.at(m, j)));
                a.set(i, j, v);
            }
            for j in 1..=igh {
                let v = vpos(j - 1, igh);
                let w = add(125 + v, a.at(j, m), mul(128 + v, y, a.at(j, i)));
                a.set(j, m, w);
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
    let s = add(1, abs_m(br), abs_m(bi));
    let ars = ar.sse_div(s);
    let ais = ai.sse_div(s);
    let brs = br.sse_div(s);
    let bis = bi.sse_div(s);
    let s = add(2, mul(3, brs, brs), mul(4, bis, bis));
    (
        add(5, mul(6, ars, brs), mul(7, ais, bis)).sse_div(s),
        mul(8, ais, brs).sse_sub(mul(9, ars, bis)).sse_div(s),
    )
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
            norm = add(0, norm, abs_m(h.at(i, j)));
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
                h.set(enu, enu, add(144, x, t));
                wr[enu] = h.at(enu, enu);
                wi[enu] = 0.0;
                found[enu] = true;
                en = na as isize;
                break;
            }
            y = h.at(na, na);
            w = mul(131, h.at(enu, na), h.at(na, enu));
            if l == na {
                // L280.
                p = y.sse_sub(x).sse_mul(0.5);
                q = add(145, mul(10, p, p), w);
                zz = abs_m(q).sqrt();
                h.set(enu, enu, add(146, x, t));
                x = h.at(enu, enu);
                h.set(na, na, add(147, y, t));
                if q < 0.0 {
                    // L320.
                    wr[na] = add(11, x, p);
                    wr[enu] = add(12, x, p);
                    wi[na] = zz;
                    wi[enu] = -zz;
                } else {
                    zz = add(13, p, d_sign_f(zz, p));
                    wr[na] = add(14, x, zz);
                    wr[enu] = wr[na];
                    if zz != 0.0 {
                        wr[enu] = x.sse_sub(w.sse_div(zz));
                    }
                    wi[na] = 0.0;
                    wi[enu] = 0.0;
                    x = h.at(enu, na);
                    s = add(15, abs_m(x), abs_m(zz));
                    p = x.sse_div(s);
                    q = zz.sse_div(s);
                    r = add(16, mul(17, p, p), mul(18, q, q)).sqrt();
                    p = p.sse_div(r);
                    q = q.sse_div(r);
                    // more than 16 columns: two at a time (then the one left over), else one by one
                    let count = n + 1 - na;
                    for j in na..=n {
                        zz = h.at(na, j);
                        let b = if count <= 16 {
                            19
                        } else if j - na < count - count % 2 {
                            196
                        } else {
                            201
                        };
                        h.set(
                            na,
                            j,
                            add(b, mul(b + 1, q, zz), mul(b + 2, p, h.at(enu, j))),
                        );
                        h.set(
                            enu,
                            j,
                            mul(b + 3, q, h.at(enu, j)).sse_sub(mul(b + 4, p, zz)),
                        );
                    }
                    for i in 1..=enu {
                        zz = h.at(i, na);
                        let v = vpos(i - 1, enu);
                        h.set(
                            i,
                            na,
                            add(24 + v, mul(27 + v, q, zz), mul(30 + v, p, h.at(i, enu))),
                        );
                        h.set(
                            i,
                            enu,
                            mul(33 + v, q, h.at(i, enu)).sse_sub(mul(36 + v, p, zz)),
                        );
                    }
                    for i in low..=igh {
                        zz = z.at(i, na);
                        let v = vpos(i - low, igh + 1 - low);
                        z.set(
                            i,
                            na,
                            add(148 + v, mul(151 + v, q, zz), mul(154 + v, p, z.at(i, enu))),
                        );
                        z.set(
                            i,
                            enu,
                            mul(157 + v, q, z.at(i, enu)).sse_sub(mul(160 + v, p, zz)),
                        );
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
                t = add(132, t, x);
                for i in low..=enu {
                    let v = h.at(i, i).sse_sub(x);
                    h.set(i, i, v);
                }
                s = add(133, abs_m(h.at(enu, na)), abs_m(h.at(na, enm2 as usize)));
                x = s.sse_mul(0.75);
                y = x;
                w = s.sse_mul(-0.4375).sse_mul(s);
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
                r = x.sse_sub(zz);
                s = y.sse_sub(zz);
                p = add(
                    134,
                    mul(135, r, s).sse_sub(w).sse_div(h.at(m + 1, m)),
                    h.at(m, m + 1),
                );
                q = h.at(m + 1, m + 1).sse_sub(zz).sse_sub(r).sse_sub(s);
                r = h.at(m + 2, m + 1);
                s = add(136, add(137, abs_m(p), abs_m(q)), abs_m(r));
                p = p.sse_div(s);
                q = q.sse_div(s);
                r = r.sse_div(s);
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
                    x = add(138, add(139, abs_m(p), abs_m(q)), abs_m(r));
                    if x == 0.0 {
                        continue;
                    }
                    p = p.sse_div(x);
                    q = q.sse_div(x);
                    r = r.sse_div(x);
                }
                // L170.
                s = d_sign_f(
                    add(140, add(141, p.sse_mul(p), q.sse_mul(q)), r.sse_mul(r)).sqrt(),
                    p,
                );
                if k != m {
                    h.set(k, k - 1, neg_mul(142, s, x));
                } else if l != m {
                    let v = -h.at(k, k - 1);
                    h.set(k, k - 1, v);
                }
                // L190.
                p = add(143, p, s);
                x = p.sse_div(s);
                y = q.sse_div(s);
                zz = r.sse_div(s);
                q = q.sse_div(p);
                r = r.sse_div(p);
                let jmax = enu.min(k + 3);
                if !notlas {
                    for j in k..=n {
                        p = add(39, h.at(k, j), mul(40, q, h.at(k + 1, j)));
                        h.set(k, j, h.at(k, j).sse_sub(mul(41, p, x)));
                        h.set(k + 1, j, h.at(k + 1, j).sse_sub(mul(42, p, y)));
                    }
                    for i in 1..=jmax {
                        let v = vpos(i - 1, jmax);
                        p = add(
                            43 + v,
                            mul(46 + v, x, h.at(i, k)),
                            mul(49 + v, y, h.at(i, k + 1)),
                        );
                        h.set(i, k, h.at(i, k).sse_sub(p));
                        h.set(i, k + 1, h.at(i, k + 1).sse_sub(mul(52 + v, p, q)));
                    }
                    for i in low..=igh {
                        let v = vpos(i - low, igh + 1 - low);
                        p = add(
                            163 + v,
                            mul(166 + v, x, z.at(i, k)),
                            mul(169 + v, y, z.at(i, k + 1)),
                        );
                        z.set(i, k, z.at(i, k).sse_sub(p));
                        z.set(i, k + 1, z.at(i, k + 1).sse_sub(mul(172 + v, p, q)));
                    }
                } else {
                    for j in k..=n {
                        p = add(
                            55,
                            add(56, h.at(k, j), mul(57, q, h.at(k + 1, j))),
                            mul(58, r, h.at(k + 2, j)),
                        );
                        h.set(k, j, h.at(k, j).sse_sub(mul(59, p, x)));
                        h.set(k + 1, j, h.at(k + 1, j).sse_sub(mul(60, p, y)));
                        h.set(k + 2, j, h.at(k + 2, j).sse_sub(mul(61, p, zz)));
                    }
                    for i in 1..=jmax {
                        let v = vpos(i - 1, jmax);
                        p = add(
                            62 + v,
                            add(
                                65 + v,
                                mul(68 + v, x, h.at(i, k)),
                                mul(71 + v, y, h.at(i, k + 1)),
                            ),
                            mul(74 + v, zz, h.at(i, k + 2)),
                        );
                        h.set(i, k, h.at(i, k).sse_sub(p));
                        h.set(i, k + 1, h.at(i, k + 1).sse_sub(mul(77 + v, p, q)));
                        h.set(i, k + 2, h.at(i, k + 2).sse_sub(mul(80 + v, p, r)));
                    }
                    for i in low..=igh {
                        let v = vpos(i - low, igh + 1 - low);
                        p = add(
                            175 + v,
                            add(
                                178 + v,
                                mul(181 + v, x, z.at(i, k)),
                                mul(184 + v, y, z.at(i, k + 1)),
                            ),
                            mul(187 + v, zz, z.at(i, k + 2)),
                        );
                        z.set(i, k, z.at(i, k).sse_sub(p));
                        z.set(i, k + 1, z.at(i, k + 1).sse_sub(mul(190 + v, p, q)));
                        z.set(i, k + 2, z.at(i, k + 2).sse_sub(mul(193 + v, p, r)));
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
                let (cr, ci) = cdiv(0.0, -h.at(nau, en), h.at(nau, nau).sse_sub(p), q);
                h.set(nau, nau, cr);
                h.set(nau, en, ci);
            } else {
                h.set(nau, nau, q.sse_div(h.at(en, nau)));
                h.set(nau, en, neg_div(83, h.at(en, en).sse_sub(p), h.at(en, nau)));
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
                w = h.at(i, i).sse_sub(p);
                let mut ra = 0.0f64;
                let mut sa = 0.0f64;
                for j in m..=en {
                    ra = add(84, ra, mul(85, h.at(i, j), h.at(j, nau)));
                    sa = add(86, sa, mul(87, h.at(i, j), h.at(j, en)));
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
                    let wp = wr[i].sse_sub(p);
                    let mut vr = add(88, mul(89, wp, wr[i].sse_sub(p)), mul(90, wi[i], wi[i]))
                        .sse_sub(mul(91, q, q));
                    let vi = mul(92, wr[i].sse_sub(p).sse_mul(2.0), q);
                    if !(vr != 0.0 || vi != 0.0) {
                        let tst1 = mul(
                            93,
                            norm,
                            add(
                                94,
                                add(95, add(96, add(97, abs_m(w), abs_m(q)), abs_m(x)), abs_m(y)),
                                abs_m(zz),
                            ),
                        );
                        vr = tst1;
                        loop {
                            vr = vr.sse_mul(0.01);
                            let tst2 = tst1 + vr;
                            if !(tst2 > tst1) {
                                break;
                            }
                        }
                    }
                    // L784.
                    let d1 = add(98, mul(99, x, r).sse_sub(mul(100, zz, ra)), mul(101, q, sa));
                    let d2 = mul(102, x, s)
                        .sse_sub(mul(103, zz, sa))
                        .sse_sub(mul(104, q, ra));
                    let (cr, ci) = cdiv(d1, d2, vr, vi);
                    h.set(i, nau, cr);
                    h.set(i, en, ci);
                    if abs_m(x) > abs_m(zz) + abs_m(q) {
                        h.set(
                            i + 1,
                            nau,
                            add(
                                105,
                                (-ra).sse_sub(mul(106, w, h.at(i, nau))),
                                mul(107, q, h.at(i, en)),
                            )
                            .sse_div(x),
                        );
                        h.set(
                            i + 1,
                            en,
                            (-sa)
                                .sse_sub(mul(108, w, h.at(i, en)))
                                .sse_sub(mul(109, q, h.at(i, nau)))
                                .sse_div(x),
                        );
                    } else {
                        // L785.
                        let d1 = (-r).sse_sub(mul(110, y, h.at(i, nau)));
                        let d2 = (-s).sse_sub(mul(111, y, h.at(i, en)));
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
                    h.set(j, nau, h.at(j, nau).sse_div(t));
                    h.set(j, en, h.at(j, en).sse_div(t));
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
                w = h.at(i, i).sse_sub(p);
                r = 0.0;
                for j in m..=en {
                    r = add(112, r, mul(113, h.at(i, j), h.at(j, en)));
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
                            t = t.sse_mul(0.01);
                            let tst2 = norm + t;
                            if !(tst2 > tst1) {
                                break;
                            }
                        }
                    }
                    // L635.
                    h.set(i, en, neg_div(114, r, t));
                } else {
                    // L640.
                    x = h.at(i, i + 1);
                    y = h.at(i + 1, i);
                    let wp = wr[i].sse_sub(p);
                    q = add(115, mul(116, wp, wr[i].sse_sub(p)), mul(117, wi[i], wi[i]));
                    t = mul(118, x, s).sse_sub(mul(119, zz, r)).sse_div(q);
                    h.set(i, en, t);
                    if abs_m(x) <= abs_m(zz) {
                        h.set(i + 1, en, (-s).sse_sub(mul(120, y, t)).sse_div(zz));
                    } else {
                        h.set(i + 1, en, (-r).sse_sub(mul(121, w, t)).sse_div(x));
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
                    h.set(j, en, h.at(j, en).sse_div(t));
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
                zz = add(122, zz, mul(123, z.at(i, k), h.at(k, j)));
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
                let v = z.at(i, j).sse_mul(s);
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

/// `vnl_symmetric_eigensystem<float>(A)`, its eigenvalues and eigenvectors: EISPACK's
/// `rs` in double on the matrix converted to double ([`symmetric_eigensystem`]), the eigenvalues
/// and eigenvectors rounded to float. `ierr` as in [`SymmetricEigen`] (VNL then prints
/// `vnl_symmetric_eigensystem: ierr = …` and leaves `V` unset: memory contents, here the double
/// vectors as computed).
pub fn symmetric_eigensystem_f32(a: &[f32], n: usize) -> (Vec<f32>, Vec<f32>, usize) {
    let e = symmetric_eigensystem_f32_full(a, n);
    (e.values, e.vectors, e.ierr)
}

/// [`symmetric_eigensystem_f32`] with the notices (ITK-115: where VNL would never return).
#[derive(Clone, Debug, PartialEq)]
pub struct SymmetricEigenF32 {
    pub values: Vec<f32>,
    /// `V(r, c)` at `vectors[r * n + c]`.
    pub vectors: Vec<f32>,
    pub ierr: usize,
    pub notices: Vec<crate::defects::Notice>,
}

impl SymmetricEigenF32 {
    /// Whether VNL's eigensystem would never return on this matrix (ITK-115).
    pub fn never_returns(&self) -> bool {
        self.notices.iter().any(|n| n.defect.id == "ITK-115")
    }
}

/// [`symmetric_eigensystem_f32`] with its notices.
pub fn symmetric_eigensystem_f32_full(a: &[f32], n: usize) -> SymmetricEigenF32 {
    let ad: Vec<f64> = a.iter().map(|&v| f64::from(v)).collect();
    let e = symmetric_eigensystem(&ad, n);
    SymmetricEigenF32 {
        values: e.values.iter().map(|&v| v as f32).collect(),
        vectors: e.vectors.iter().map(|&v| v as f32).collect(),
        ierr: e.ierr,
        notices: e.notices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The NaNs' bits (signs and payloads) of `real_eigensystem` on a matrix with a NaN on its
    /// diagonal (fs.maths' clean room's reproducer: before, larmorx's debug and release builds
    /// gave different signs) and on seeded matrices whose balanced part is finite off its diagonal
    /// (NaNs of both signs with payloads, infinities, extreme numbers elsewhere), recorded on Linux
    /// x86-64. Every operation that can meet a NaN is explicit, so debug, release and AArch64
    /// agree; the official build's bits are matched where the probe (validation/probes/fs-vnl,
    /// family `eignan`) says so (docs/findings/vnl-netlib.md, F-VNLN-5: not yet everywhere).
    #[test]
    fn real_eigensystem_nan_bits_are_x86_64s() {
        use crate::vnl_fixed::cases;
        use crate::x87::cases::SplitMix64;
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut word = |w: u64| {
            for b in w.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        let run = |m: &[f64], n: usize, word: &mut dyn FnMut(u64)| {
            let Ok(es) = real_eigensystem(m, n) else {
                word(u64::MAX);
                return;
            };
            word(es.ierr as u64);
            for v in es
                .d_re
                .iter()
                .chain(&es.d_im)
                .chain(&es.v_re)
                .chain(&es.v_im)
            {
                word(v.to_bits());
            }
            for c in 0..n {
                if es.vreal_written[c] {
                    (0..n).for_each(|r| word(es.vreal[r * n + c].to_bits()));
                }
            }
        };
        let f = |b: u32| f64::from(f32::from_bits(b));
        let rep: Vec<f64> = [
            0x4170_0000,
            0x3f80_0000,
            0x4106_db6e,
            0x4275_2492,
            0x40a0_0000,
            0x40e0_0000,
            0xc0dc_0000,
            0x42b8_db6e,
            0x4120_0000,
            0x4120_0000,
            0x7fc0_0000,
            0x4040_0000,
            0x4198_0000,
            0xc274_0000,
            0x41a0_0000,
            0x3f80_0000,
        ]
        .into_iter()
        .map(f)
        .collect();
        run(&rep, 4, &mut word);
        let mut rng = SplitMix64::new(0xe16e_7e57);
        for _ in 0..1500 {
            let n = 1 + rng.below(6) as usize;
            let low = 1 + rng.below(n as u64) as usize;
            let igh = low + rng.below((n - low + 1) as u64) as usize;
            let mut a = vec![0.0f64; n * n];
            for i in 1..=n {
                for j in 1..=n {
                    let in_block = |k: usize| k >= low && k <= igh;
                    let ordinary = |rng: &mut SplitMix64| rng.range(-4096, 4096) as f64 / 256.0;
                    a[(i - 1) * n + j - 1] = if in_block(i) && in_block(j) {
                        if i == j && rng.below(3) == 0 {
                            cases::special_f64(&mut rng)
                        } else {
                            ordinary(&mut rng)
                        }
                    } else if ((i < low || i > igh) && j >= i) || (in_block(i) && j > igh) {
                        if rng.below(2) == 0 {
                            cases::special_f64(&mut rng)
                        } else {
                            ordinary(&mut rng)
                        }
                    } else {
                        0.0
                    };
                }
            }
            run(&a, n, &mut word);
        }
        if std::env::var_os("LARMORX_PRINT_DIGESTS").is_some() {
            println!("REAL_EIGEN_NAN_BITS = {h:#018x}");
        }
        assert_eq!(h, REAL_EIGEN_NAN_BITS);
    }

    const REAL_EIGEN_NAN_BITS: u64 = 0x49f7_e86e_5e22_5ffe;

    #[test]
    fn pythag_is_the_hypotenuse() {
        assert!((pythag(3.0, 4.0).unwrap() - 5.0).abs() < 1e-15);
        assert_eq!(pythag(0.0, 0.0), Some(0.0));
        assert!((pythag(1e-3, 1.0).unwrap() - (1.0f64 + 1e-6).sqrt()).abs() < 1e-15);
    }

    /// VXL's C on NaNs, infinities and signed zeros (f2c's `abs`, `max`, `min` macros; ITK-115),
    /// the values ITK 4.13.2's `pythag_` gives (`validation/probes/fs-vnl`, family `nonfinite`).
    #[test]
    fn pythag_keeps_vxls_nans() {
        let (nan, inf) = (f64::NAN, f64::INFINITY);
        // A NaN first gives the second's magnitude twice: pythag(|x|, |x|).
        assert_eq!(pythag(nan, 3.0), pythag(3.0, 3.0));
        assert_eq!(pythag(-nan, -2.0), pythag(2.0, 2.0));
        assert_eq!(pythag(nan, 0.0), Some(0.0));
        // A NaN second, two NaNs, two infinities: VXL loops for ever.
        assert_eq!(pythag(3.0, nan), None);
        assert_eq!(pythag(0.0, nan), None);
        assert_eq!(pythag(nan, nan), None);
        assert_eq!(pythag(inf, -inf), None);
        assert_eq!(pythag(nan, inf), None);
        // One infinity: the infinity.
        assert_eq!(pythag(inf, 2.0), Some(inf));
        assert_eq!(pythag(-7.0, -inf), Some(inf));
        assert_eq!(pythag(inf, nan), None);
        // f2c's abs keeps -0.
        assert_eq!(
            pythag(-0.0, -0.0).map(f64::to_bits),
            Some((-0.0f64).to_bits())
        );
        assert_eq!(
            pythag(-0.0, 0.0).map(f64::to_bits),
            Some((-0.0f64).to_bits())
        );
        assert_eq!(pythag(0.0, -0.0).map(f64::to_bits), Some(0.0f64.to_bits()));
    }

    /// ITK-115: a NaN in the tridiagonal form reaches pythag as its second argument.
    #[test]
    fn symmetric_eigensystems_where_vnl_never_returns() {
        let nan = f64::NAN;
        let e = symmetric_eigensystem(&[1.0, nan, nan, 2.0], 2);
        assert!(e.never_returns(), "{e:?}");
        assert_eq!(e.ierr, 1);
        // An infinite diagonal entry of an otherwise zero row returns.
        let e = symmetric_eigensystem(&[f64::INFINITY, 0.0, 0.0, 2.0], 2);
        assert!(!e.never_returns() && e.ierr == 0, "{e:?}");
        let f = symmetric_eigensystem_f32_full(&[1.0, f32::NAN, f32::NAN, 2.0], 2);
        assert!(f.never_returns());
        assert!(
            symmetric_eigensystem(&[4.0, 1.0, 1.0, 3.0], 2)
                .notices
                .is_empty()
        );
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
