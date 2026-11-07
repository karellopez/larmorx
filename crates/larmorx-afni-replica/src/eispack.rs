// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: src/eispack/eis_svd.c (EISPACK's svd, August 1983, by
// Burton S. Garbow, Argonne National Laboratory, a translation of Golub and Reinsch's Algol
// procedure; as f2c translated it), src/eispack/eis_pythag.c (pythag), src/f2c/d_sign.c, and
// the 3 x 3 path of svd_double in src/cs_symeig.c with its CHECK_SVD test. EISPACK and f2c are
// freely available (AFNI's LICENSE.txt: "License: Free [https://www.netlib.org/misc/faq.html]");
// libf2c (d_sign): Copyright 1990 - 1995 by AT&T Bell Laboratories and Bellcore, under its
// permission notice (src/f2c/Notice; reproduced in NOTICE).
// cs_symeig.c: Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert
// W. Cox et al.), released by AFNI 26.3.05 under the Creative Commons Attribution 4.0
// International licence (CC BY 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! The singular value decomposition AFNI computes for a 3 x 3 matrix (`svd_double` with
//! EISPACK's `svd`), which 3dvolreg reaches through `DMAT_svdrot_newer` whenever it rotates a
//! volume by a matrix (the coarse rotation search of `-twopass`, `-rotparent`).
//!
//! The translation keeps f2c's 1-based indices and every operation in the Fortran's order.
//! `svd_double` checks the decomposition (`CHECK_SVD`, defined in AFNI's build): if the average
//! error of `U·diag(w)·Vᵀ` against the input is at least `1e-5` of the average entry, AFNI
//! perturbs the matrix with `drand48` and decomposes it again, then tries another method. A
//! matrix 3dvolreg decomposes is a rotation, whose decomposition never fails that test; larmorx
//! reports the failure ([`SvdError`]) instead of perturbing.

/// f2c's `abs`: `x >= 0 ? x : -x` (so `-0.0` stays `-0.0`).
#[inline]
fn fabs(x: f64) -> f64 {
    if x >= 0.0 { x } else { -x }
}

/// f2c's `d_sign(a, b)`: `|a|` with the sign of `b` (`b >= 0` gives `+|a|`, so a `-0.0` `b`
/// does too).
#[inline]
fn d_sign(a: f64, b: f64) -> f64 {
    let x = if a >= 0.0 { a } else { -a };
    if b >= 0.0 { x } else { -x }
}

/// EISPACK's `pythag(a, b)`: `sqrt(a² + b²)` without overflow or destructive underflow, by its
/// iteration.
pub fn pythag(a: f64, b: f64) -> f64 {
    let (d1, d2) = (fabs(a), fabs(b));
    let mut p = if d1 >= d2 { d1 } else { d2 };
    if p == 0.0 {
        return p;
    }
    let (d2, d3) = (fabs(a), fabs(b));
    let d1 = (if d2 <= d3 { d2 } else { d3 }) / p;
    let mut r = d1 * d1;
    loop {
        let t = r + 4.0;
        if t == 4.0 {
            return p;
        }
        let s = r / t;
        let u = s * 2.0 + 1.0;
        p *= u;
        let d1 = s / u;
        r *= d1 * d1;
    }
}

/// EISPACK's `svd` for an `m` x `n` matrix, column-major (`a[i + j*m]`), with `u` and `v`
/// computed (`matu` and `matv` true): returns `(w, u, v, ierr)`.
fn svd(m: usize, n: usize, a_in: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>, usize) {
    // 1-based arrays, as f2c's parameter adjustments make them.
    let mut u = vec![0.0f64; (m + 1) * (n + 1) + 1];
    let mut v = vec![0.0f64; (n + 1) * (n + 1) + 1];
    let mut w = vec![0.0f64; n + 1];
    let mut rv1 = vec![0.0f64; n + 1];
    let ud = m; // u_dim1 = ldu = m
    let vd = n; // v_dim1 = ldv = n
    let ui = |i: usize, j: usize| i + j * ud;
    let vi = |i: usize, j: usize| i + j * vd;
    for i in 1..=m {
        for j in 1..=n {
            u[ui(i, j)] = a_in[(i - 1) + (j - 1) * m];
        }
    }
    // Householder reduction to bidiagonal form.
    let mut g = 0.0f64;
    let mut scale = 0.0f64;
    let mut x = 0.0f64;
    let mut l: usize = 0;
    let mut s;
    let mut f;
    let mut h;
    for i in 1..=n {
        l = i + 1;
        rv1[i] = scale * g;
        g = 0.0;
        s = 0.0;
        scale = 0.0;
        'l210: {
            if i > m {
                break 'l210;
            }
            for k in i..=m {
                scale += fabs(u[ui(k, i)]);
            }
            if scale == 0.0 {
                break 'l210;
            }
            for k in i..=m {
                u[ui(k, i)] /= scale;
                let d = u[ui(k, i)];
                s += d * d;
            }
            f = u[ui(i, i)];
            g = -d_sign(s.sqrt(), f);
            h = f * g - s;
            u[ui(i, i)] = f - g;
            if i != n {
                for j in l..=n {
                    s = 0.0;
                    for k in i..=m {
                        s += u[ui(k, i)] * u[ui(k, j)];
                    }
                    f = s / h;
                    for k in i..=m {
                        u[ui(k, j)] += f * u[ui(k, i)];
                    }
                }
            }
            for k in i..=m {
                u[ui(k, i)] = scale * u[ui(k, i)];
            }
        }
        // L210
        w[i] = scale * g;
        g = 0.0;
        s = 0.0;
        scale = 0.0;
        'l290: {
            if i > m || i == n {
                break 'l290;
            }
            for k in l..=n {
                scale += fabs(u[ui(i, k)]);
            }
            if scale == 0.0 {
                break 'l290;
            }
            for k in l..=n {
                u[ui(i, k)] /= scale;
                let d = u[ui(i, k)];
                s += d * d;
            }
            f = u[ui(i, l)];
            g = -d_sign(s.sqrt(), f);
            h = f * g - s;
            u[ui(i, l)] = f - g;
            for k in l..=n {
                rv1[k] = u[ui(i, k)] / h;
            }
            if i != m {
                for j in l..=m {
                    s = 0.0;
                    for k in l..=n {
                        s += u[ui(j, k)] * u[ui(i, k)];
                    }
                    for k in l..=n {
                        u[ui(j, k)] += s * rv1[k];
                    }
                }
            }
            for k in l..=n {
                u[ui(i, k)] = scale * u[ui(i, k)];
            }
        }
        // L290: x = max(x, |w[i]| + |rv1[i]|), f2c's max: (a >= b) ? a : b.
        let d4 = fabs(w[i]) + fabs(rv1[i]);
        x = if x >= d4 { x } else { d4 };
    }
    // Accumulation of right-hand transformations.
    for ii in 1..=n {
        let i = n + 1 - ii;
        if i != n {
            if g != 0.0 {
                for j in l..=n {
                    // Double division avoids possible underflow.
                    v[vi(j, i)] = u[ui(i, j)] / u[ui(i, l)] / g;
                }
                for j in l..=n {
                    s = 0.0;
                    for k in l..=n {
                        s += u[ui(i, k)] * v[vi(k, j)];
                    }
                    for k in l..=n {
                        v[vi(k, j)] += s * v[vi(k, i)];
                    }
                }
            }
            // L360
            for j in l..=n {
                v[vi(i, j)] = 0.0;
                v[vi(j, i)] = 0.0;
            }
        }
        // L390
        v[vi(i, i)] = 1.0;
        g = rv1[i];
        l = i;
    }
    // Accumulation of left-hand transformations.
    let mn = if m < n { m } else { n };
    for ii in 1..=mn {
        let i = mn + 1 - ii;
        l = i + 1;
        g = w[i];
        if i != n {
            for j in l..=n {
                u[ui(i, j)] = 0.0;
            }
        }
        // L430
        if g == 0.0 {
            // L475
            for j in i..=m {
                u[ui(j, i)] = 0.0;
            }
        } else {
            if i != mn {
                for j in l..=n {
                    s = 0.0;
                    for k in l..=m {
                        s += u[ui(k, i)] * u[ui(k, j)];
                    }
                    f = s / u[ui(i, i)] / g;
                    for k in i..=m {
                        u[ui(k, j)] += f * u[ui(k, i)];
                    }
                }
            }
            // L460
            for j in i..=m {
                u[ui(j, i)] /= g;
            }
        }
        // L490
        u[ui(i, i)] += 1.0;
    }
    // Diagonalization of the bidiagonal form.
    let tst1 = x;
    let mut ierr = 0;
    'outer: for kk in 1..=n {
        let k1 = n - kk;
        let k = k1 + 1;
        let mut its = 0;
        loop {
            // L520: test for splitting.
            let mut l1 = 0usize;
            let mut cancel = true;
            for ll in 1..=k {
                l1 = k - ll;
                l = l1 + 1;
                let tst2 = tst1 + fabs(rv1[l]);
                if tst2 == tst1 {
                    cancel = false;
                    break;
                }
                // rv1(1) is always zero, so there is no exit through the bottom of the loop.
                let tst2 = tst1 + fabs(w[l1]);
                if tst2 == tst1 {
                    break;
                }
            }
            if cancel {
                // L540: cancellation of rv1(l) if l greater than 1.
                let mut c = 0.0f64;
                s = 1.0;
                for i in l..=k {
                    f = s * rv1[i];
                    rv1[i] *= c;
                    let tst2 = tst1 + fabs(f);
                    if tst2 == tst1 {
                        break;
                    }
                    g = w[i];
                    h = pythag(f, g);
                    w[i] = h;
                    c = g / h;
                    s = -f / h;
                    for j in 1..=m {
                        let y = u[ui(j, l1)];
                        let z = u[ui(j, i)];
                        u[ui(j, l1)] = y * c + z * s;
                        u[ui(j, i)] = -y * s + z * c;
                    }
                }
            }
            // L565: test for convergence.
            let z = w[k];
            if l == k {
                // L650: convergence.
                if z < 0.0 {
                    // w(k) is made non-negative.
                    w[k] = -z;
                    for j in 1..=n {
                        v[vi(j, k)] = -v[vi(j, k)];
                    }
                }
                continue 'outer;
            }
            // Shift from bottom 2 by 2 minor.
            if its == 30 {
                ierr = k;
                break 'outer;
            }
            its += 1;
            let mut x = w[l];
            let mut y = w[k1];
            g = rv1[k1];
            h = rv1[k];
            f = ((g + z) / h * ((g - z) / y) + y / h - h / y) * 0.5;
            g = pythag(f, 1.0);
            f = x - z / x * z + h / x * (y / (f + d_sign(g, f)) - h);
            // Next QR transformation.
            let mut c = 1.0f64;
            s = 1.0;
            for i1 in l..=k1 {
                let i = i1 + 1;
                g = rv1[i];
                y = w[i];
                h = s * g;
                g *= c;
                let mut z = pythag(f, h);
                rv1[i1] = z;
                c = f / z;
                s = h / z;
                f = x * c + g * s;
                g = -x * s + g * c;
                h = y * s;
                y *= c;
                for j in 1..=n {
                    x = v[vi(j, i1)];
                    z = v[vi(j, i)];
                    v[vi(j, i1)] = x * c + z * s;
                    v[vi(j, i)] = -x * s + z * c;
                }
                // L575
                z = pythag(f, h);
                w[i1] = z;
                // Rotation can be arbitrary if z is zero.
                if z != 0.0 {
                    c = f / z;
                    s = h / z;
                }
                // L580
                f = c * g + s * y;
                x = -s * g + c * y;
                for j in 1..=m {
                    let yy = u[ui(j, i1)];
                    let zz = u[ui(j, i)];
                    u[ui(j, i1)] = yy * c + zz * s;
                    u[ui(j, i)] = -yy * s + zz * c;
                }
            }
            rv1[l] = 0.0;
            rv1[k] = f;
            w[k] = x;
        }
    }
    // Back to 0-based, column-major outputs.
    let wo = w[1..=n].to_vec();
    let mut uo = vec![0.0; m * n];
    let mut vo = vec![0.0; n * n];
    for j in 1..=n {
        for i in 1..=m {
            uo[(i - 1) + (j - 1) * m] = u[ui(i, j)];
        }
        for i in 1..=n {
            vo[(i - 1) + (j - 1) * n] = v[vi(i, j)];
        }
    }
    (wo, uo, vo, ierr)
}

/// The decomposition failed `svd_double`'s check, where AFNI would perturb the matrix and try
/// again (not ported: never met for the rotation matrices 3dvolreg decomposes).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error(
    "SVD avg err={err:e}: AFNI recomputes it after perturbing the matrix with drand48 (not \
     supported by larmorx)"
)]
pub struct SvdError {
    pub err: f64,
}

/// The singular values and the `u` and `v` matrices (column-major) of [`svd_double3`].
pub type Svd3 = ([f64; 3], [f64; 9], [f64; 9]);

/// `svd_double(3, 3, a, s, u, v)` without sorting (`svd_sort` is 0): `a`, `u` and `v`
/// column-major (`a[i + 3j]` is row `i`, column `j`). Returns `(s, u, v)`.
pub fn svd_double3(a: &[f64; 9]) -> Result<Svd3, SvdError> {
    let (w, u, v, _ierr) = svd(3, 3, a);
    // CHECK_SVD: the average absolute error against the average absolute entry.
    let (m, n) = (3usize, 3usize);
    let mut err = 0.0f64;
    let mut amag = 1.0e-12f64;
    for j in 0..n {
        for i in 0..m {
            let mut aij = a[i + j * m];
            amag += aij.abs();
            for k in 0..n {
                aij -= u[i + k * m] * v[j + k * n] * w[k];
            }
            err += aij.abs();
        }
    }
    amag /= (m * n) as f64;
    err /= (m * n) as f64;
    if err >= 1.0e-5 * amag || !err.is_finite() {
        return Err(SvdError { err });
    }
    let mut s = [0.0; 3];
    let mut uo = [0.0; 9];
    let mut vo = [0.0; 9];
    s.copy_from_slice(&w);
    uo.copy_from_slice(&u);
    vo.copy_from_slice(&v);
    Ok((s, uo, vo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pythag_matches_hypot() {
        for (a, b) in [
            (3.0, 4.0),
            (1e-300, 1e-300),
            (0.0, 0.0),
            (-5.0, 12.0),
            (1e300, 1e300),
        ] {
            let p = pythag(a, b);
            let h = f64::hypot(a, b);
            assert!((p - h).abs() <= 4.0 * f64::EPSILON * h, "{a} {b}: {p} {h}");
        }
    }

    #[test]
    fn svd_of_a_matrix_reconstructs_it() {
        let a = [2.0, 0.5, -1.0, 0.25, 3.0, 0.75, -0.5, 1.5, 4.0];
        let (s, u, v) = svd_double3(&a).unwrap();
        for j in 0..3 {
            for i in 0..3 {
                let mut x = 0.0;
                for k in 0..3 {
                    x += u[i + 3 * k] * s[k] * v[j + 3 * k];
                }
                assert!((x - a[i + 3 * j]).abs() < 1e-12);
            }
        }
        assert!(s.iter().all(|&x| x >= 0.0));
    }
}
