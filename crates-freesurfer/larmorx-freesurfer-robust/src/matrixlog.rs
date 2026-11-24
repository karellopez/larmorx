// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The matrix logarithm and exponential of Martin Reuter's `MyMatrix` (queue 3.2-15: the affine
//! mean space of `mri_robust_template`, `--affine` without `--fixtp`), translated from FreeSurfer
//! 8.2.0's `mri_robust_register/MyMatrix.cpp`:
//!
//! - [`geometric_mean3`] (`GeometricMean` of `vnl_matrix_fixed<double, 3, 3>`s): `exp(sum_i
//!   log(M_i) / n)`;
//! - [`matrix_exp`] (`MatrixExp`, after MATLAB's `expm`: Higham's scaling and squaring with a
//!   Pade approximant, `PadeApproximantOfDegree`, `getPadeCoefficients`, `expmchk`; the
//!   approximant's quotient by `vnl_svd<double>`'s `solve`);
//! - [`matrix_log`] (`MatrixLog`, after MATLAB's `logm`/`funm`: the complex Schur form
//!   (`SchurComplex`), its blocking (`blocking`, `swapping`, `cumsum0`) and reordering
//!   (`OrdSchurComplex`, `OrdSchurComplexLogical` on LAPACK's `ZTRSEN`), each diagonal block's
//!   logarithm by inverse scaling and squaring (`MatrixLog_isst`: square roots, then the [8/8]
//!   Pade approximant by partial fractions `MatrixLog_pf` on Gauss-Legendre nodes
//!   `gauss_legendre`, each term a `vnl_svd<std::complex<double>>` solve), the block Parlett
//!   recurrence above the diagonal (scalar formula, or `sylv_tri`'s triangular Sylvester
//!   equation), `U F U^H`, and its imaginary-part check).
//!
//! The arithmetic is VNL's and C++'s as the official build compiles it: `vnl_matrix`'s products
//! and sums (`larmorx_core::vnl_matrix`), complex products and quotients with `__muldc3` and
//! `__divdc3` (`larmorx_core::cplx`), `std::abs`, `std::sqrt` and `std::log` of a complex number
//! the decided `cabs`, `csqrt` and `clog`, `std::norm` (the complex Frobenius norm's squares)
//! `cabs(z)^2` (libstdc++'s floating-point form), and every real operation that can meet a NaN
//! with x86-64's SSE rules.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::cplx::{C64, cabs, clog, csqrt, cxx_div, cxx_mul};
use larmorx_core::defects::{FS_1099, FS_1100, FS_1311, FS_1312};
use larmorx_core::lapack_ztrsen::ztrsen_nv;
use larmorx_core::math;
use larmorx_core::vnl_eigen::symmetric_eigensystem;
use larmorx_core::vnl_matrix::{VnlCMatrix, VnlMatrix};
use larmorx_core::vnl_svd_complex::{mat_vec, suspicious_zsvdc, vnl_svd_complex};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::numerics::vnl_svd_hxx;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64};

use crate::mymatrix::{Mat3, schur_complex, svd_solve};

/// `vnl_matrix<double>` `a + b`.
fn madd(a: &VnlMatrix, b: &VnlMatrix) -> VnlMatrix {
    assert_eq!((a.rows, a.cols), (b.rows, b.cols), "operator+: sizes");
    VnlMatrix {
        rows: a.rows,
        cols: a.cols,
        data: a
            .data
            .iter()
            .zip(&b.data)
            .map(|(&x, &y)| add64(x, y))
            .collect(),
    }
}

/// `s * m` (`vnl_tag_mul`: each element `m(i, j) * s`).
fn mscale(m: &VnlMatrix, s: f64) -> VnlMatrix {
    VnlMatrix {
        rows: m.rows,
        cols: m.cols,
        data: m.data.iter().map(|&x| mul64(x, s)).collect(),
    }
}

/// `-m`.
fn mneg(m: &VnlMatrix) -> VnlMatrix {
    VnlMatrix {
        rows: m.rows,
        cols: m.cols,
        data: m.data.iter().map(|&x| -x).collect(),
    }
}

/// Complex `a + b`, `a - b`, `s * m` (each element `m(i, j) * s`), `-m`.
fn cadd(a: &VnlCMatrix, b: &VnlCMatrix) -> VnlCMatrix {
    assert_eq!((a.rows, a.cols), (b.rows, b.cols), "operator+: sizes");
    VnlCMatrix {
        rows: a.rows,
        cols: a.cols,
        data: a.data.iter().zip(&b.data).map(|(x, y)| x.add(*y)).collect(),
    }
}

fn csub(a: &VnlCMatrix, b: &VnlCMatrix) -> VnlCMatrix {
    assert_eq!((a.rows, a.cols), (b.rows, b.cols), "operator-: sizes");
    VnlCMatrix {
        rows: a.rows,
        cols: a.cols,
        data: a.data.iter().zip(&b.data).map(|(x, y)| x.sub(*y)).collect(),
    }
}

fn cscale(m: &VnlCMatrix, s: C64) -> VnlCMatrix {
    VnlCMatrix {
        rows: m.rows,
        cols: m.cols,
        data: m.data.iter().map(|&x| cxx_mul(x, s)).collect(),
    }
}

fn cneg(m: &VnlCMatrix) -> VnlCMatrix {
    VnlCMatrix {
        rows: m.rows,
        cols: m.cols,
        data: m.data.iter().map(|x| x.neg()).collect(),
    }
}

/// `m + d I` for a complex diagonal (`vnl_matrix + vnl_diag_matrix`: `m(j, j) += d`).
fn cadd_diag(m: &VnlCMatrix, d: C64) -> VnlCMatrix {
    let mut r = m.clone();
    for j in 0..m.rows.min(m.cols) {
        *r.at_mut(j, j) = r.at(j, j).add(d);
    }
    r
}

/// `m - d I` (`vnl_matrix - vnl_diag_matrix`: `m(j, j) -= d`).
fn csub_diag(m: &VnlCMatrix, d: C64) -> VnlCMatrix {
    let mut r = m.clone();
    for j in 0..m.rows.min(m.cols) {
        *r.at_mut(j, j) = r.at(j, j).sub(d);
    }
    r
}

/// The complex identity (`set_identity`).
fn cidentity(n: usize) -> VnlCMatrix {
    let mut m = VnlCMatrix::zeros(n, n);
    for i in 0..n {
        *m.at_mut(i, i) = C64::ONE;
    }
    m
}

/// `frobenius_norm()` of a complex matrix: `sqrt` of `std::norm` summed in storage order from
/// `+0`, `std::norm(z)` libstdc++'s `abs(z) * abs(z)` (`cabs`).
fn cfro(m: &VnlCMatrix) -> f64 {
    let mut val = 0.0f64;
    for &z in &m.data {
        let r = cabs(z);
        val = add64(val, mul64(r, r));
    }
    sqrt64(val)
}

/// `getSubMatrix(A, rows, cols)`.
fn get_sub(a: &VnlCMatrix, rows: &[usize], cols: &[usize]) -> VnlCMatrix {
    let mut m = VnlCMatrix::zeros(rows.len(), cols.len());
    for (r, &ri) in rows.iter().enumerate() {
        for (c, &ci) in cols.iter().enumerate() {
            *m.at_mut(r, c) = a.at(ri, ci);
        }
    }
    m
}

/// `setSubMatrix(A, rows, cols, B)`.
fn set_sub(a: &mut VnlCMatrix, rows: &[usize], cols: &[usize], b: &VnlCMatrix) {
    for (r, &ri) in rows.iter().enumerate() {
        for (c, &ci) in cols.iter().enumerate() {
            *a.at_mut(ri, ci) = b.at(r, c);
        }
    }
}

/// `std::ostream << vnl_matrix<double>` at precision 16: each element then a blank, each row
/// then a newline.
fn print_real(m: &VnlMatrix) -> String {
    let mut s = String::new();
    for i in 0..m.rows {
        for j in 0..m.cols {
            s.push_str(&cfmt::g(m.at(i, j), 16));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

/// `std::ostream << vnl_matrix<std::complex<double>>` at precision 16 (`(re,im)` each).
fn print_complex(m: &VnlCMatrix) -> String {
    let mut s = String::new();
    for i in 0..m.rows {
        for j in 0..m.cols {
            let z = m.at(i, j);
            s.push_str(&format!("({},{}) ", cfmt::g(z.re, 16), cfmt::g(z.im, 16)));
        }
        s.push('\n');
    }
    s
}

/// `vnl_svd<std::complex<double>>(a)`: its warning on standard error when LINPACK does not
/// converge (VNL goes on with the decomposition reached), or the end of the program where
/// libf2c's division by zero aborts (`complex division by zero`, status 134) and where VNL never
/// returns (ITK-116: an error, status 1).
fn csvd(
    err: &mut dyn Write,
    a: &VnlCMatrix,
) -> Result<larmorx_core::vnl_svd_complex::VnlCSvd, Fatal> {
    match vnl_svd_complex(a) {
        Err(_) => {
            let _ = writeln!(err, "complex division by zero");
            Err(Fatal::Exit(134))
        }
        Ok(d) if d.never_returns => {
            let _ = writeln!(
                err,
                "MyMatrix: VNL's complex singular value decomposition never returns on this matrix (an infinite or NaN entry)"
            );
            Err(Fatal::Exit(1))
        }
        Ok(d) => {
            if d.info != 0 {
                let _ = err.write_all(suspicious_zsvdc(&vnl_svd_hxx(), a, d.info).as_bytes());
            }
            Ok(d)
        }
    }
}

// ======================================== MATRIX EXP ========================================

/// `getPadeCoefficients(m)`: the numerator's coefficients of the `[m/m]` Pade approximant of
/// `exp` (`m` 3, 5, 7, 9 or 13).
fn pade_coefficients(m: usize) -> &'static [f64] {
    match m {
        3 => &[120.0, 60.0, 12.0, 1.0],
        5 => &[30240.0, 15120.0, 3360.0, 420.0, 30.0, 1.0],
        7 => &[
            17297280.0, 8648640.0, 1995840.0, 277200.0, 25200.0, 1512.0, 56.0, 1.0,
        ],
        9 => &[
            17643225600.0,
            8821612800.0,
            2075673600.0,
            302702400.0,
            30270240.0,
            2162160.0,
            110880.0,
            3960.0,
            90.0,
            1.0,
        ],
        _ => &[
            64764752532480000.0,
            32382376266240000.0,
            7771770303897600.0,
            1187353796428800.0,
            129060195264000.0,
            10559470521600.0,
            670442572800.0,
            33522128640.0,
            1323241920.0,
            40840800.0,
            960960.0,
            16380.0,
            182.0,
            1.0,
        ],
    }
}

/// `PadeApproximantOfDegree(A, m)`: for `m` 3 to 9 the even powers of `A` (`A^2`, then each
/// next one the previous times `A^2`), `U = A sum c(j) A^(j-1)` over odd `j` from `m` down and
/// `V = sum c(j) A^j` over even `j` from `m - 1` down (each term added to the running sum from
/// zeros); for 13 Higham's grouping by `A^2`, `A^4`, `A^6`; then `F = vnl_svd(-U + V).solve(U +
/// V)`.
///
/// FS-1311 (memory contents): FreeSurfer's `getPadeCoefficients` returns its coefficients from
/// local arrays whose lifetime has ended, and the official build reads uninitialized stack
/// memory there (its stores are removed as dead); both modes take the coefficients Higham's
/// algorithm defines (the arrays' values), with the notice.
fn pade(g: &mut Globals, err: &mut dyn Write, a: &VnlMatrix, m: usize) -> Result<VnlMatrix, Fatal> {
    let n = a.cols;
    g.notices.add(
        &FS_1311,
        "the matrix exponential's Pade coefficients: FreeSurfer reads them from memory it no longer owns",
    );
    let c = pade_coefficients(m);
    let (u, v) = if m == 13 {
        let a2 = a.mul(a);
        let a4 = a2.mul(&a2);
        let a6 = a2.mul(&a4);
        let id = VnlMatrix::identity(n);
        let inner = madd(
            &madd(&mscale(&a6, c[13]), &mscale(&a4, c[11])),
            &mscale(&a2, c[9]),
        );
        let u = a.mul(&madd(
            &madd(
                &madd(
                    &madd(&a6.mul(&inner), &mscale(&a6, c[7])),
                    &mscale(&a4, c[5]),
                ),
                &mscale(&a2, c[3]),
            ),
            &mscale(&id, c[1]),
        ));
        let inner = madd(
            &madd(&mscale(&a6, c[12]), &mscale(&a4, c[10])),
            &mscale(&a2, c[8]),
        );
        let v = madd(
            &madd(
                &madd(
                    &madd(&a6.mul(&inner), &mscale(&a6, c[6])),
                    &mscale(&a4, c[4]),
                ),
                &mscale(&a2, c[2]),
            ),
            &mscale(&id, c[0]),
        );
        (u, v)
    } else {
        // s = ceil((m + 1) / 2) powers: I, A^2, A^4, ...
        let s = (m + 1).div_ceil(2);
        let mut ap: Vec<VnlMatrix> = Vec::with_capacity(s);
        ap.push(VnlMatrix::identity(n));
        ap.push(a.mul(a));
        for j in 2..s {
            let next = ap[j - 1].mul(&ap[1]);
            ap.push(next);
        }
        let mut u = VnlMatrix::zeros(n, n);
        let mut v = VnlMatrix::zeros(n, n);
        let mut j = m as i64;
        while j >= 1 {
            u = madd(&u, &mscale(&ap[(j / 2) as usize], c[j as usize]));
            j -= 2;
        }
        u = a.mul(&u);
        let mut j = m as i64 - 1;
        while j >= 0 {
            v = madd(&v, &mscale(&ap[((j + 1) / 2) as usize], c[j as usize]));
            j -= 2;
        }
        (u, v)
    };
    svd_solve(g, err, &madd(&mneg(&u), &v), &madd(&u, &v))
}

/// `expmchk`'s `theta` for `m_vals` 3, 5, 7, 9, 13.
const THETA: [f64; 5] = [
    1.495585217958292e-002,
    2.539398330063230e-001,
    9.504178996162932e-001,
    2.097847961257068e+000,
    5.371920351148152e+000,
];
const M_VALS: [usize; 5] = [3, 5, 7, 9, 13];

/// `frexp(x)`'s exponent and fraction (glibc's: 0 for a zero, NaN or infinity, the exponent
/// then 0).
fn frexp(x: f64) -> (f64, i32) {
    if x == 0.0 || !x.is_finite() {
        return (x, 0);
    }
    let b = x.to_bits();
    let e = ((b >> 52) & 0x7ff) as i32;
    if e == 0 {
        // subnormal: scale by 2^54 first
        let (f, k) = frexp(x * f64::from_bits(((1023 + 54) as u64) << 52));
        return (f, k - 54);
    }
    let f = f64::from_bits((b & !(0x7ff << 52)) | (1022u64 << 52));
    (f, e - 1022)
}

/// `2^k` exactly (`pow(2.0, k)` for an integer `k` whose power is a normal number).
fn pow2(k: i32) -> f64 {
    if (-1022..=1023).contains(&k) {
        f64::from_bits(((1023 + k) as u64) << 52)
    } else {
        math::pow(2.0, f64::from(k))
    }
}

/// `MyMatrix::MatrixExp(m)` of a square matrix: `normA` its one-norm; at most `theta(13)` the
/// Pade approximant of the first degree whose `theta` it does not pass; otherwise `s` from
/// `frexp(normA / theta(13))` (one less when the fraction is exactly 1/2), the degree-13
/// approximant of `m / 2^s` squared `s` times.
pub fn matrix_exp(g: &mut Globals, err: &mut dyn Write, m: &VnlMatrix) -> Result<VnlMatrix, Fatal> {
    assert_eq!(m.rows, m.cols, "MyMatrix::MatrixExp: a square matrix");
    let n = m.cols;
    let norm_a = m.operator_one_norm();
    let mut f = VnlMatrix::zeros(n, n);
    if norm_a <= THETA[4] {
        for (i, &t) in THETA.iter().enumerate() {
            if norm_a <= t {
                f = pade(g, err, m, M_VALS[i])?;
                break;
            }
        }
    } else {
        let (t, mut s) = frexp(div64(norm_a, THETA[4]));
        s -= i32::from(t == 0.5);
        let ss = div64(1.0, pow2(s));
        let a = mscale(m, ss);
        f = pade(g, err, &a, 13)?;
        for _ in 0..s.max(0) {
            f = f.mul(&f);
        }
    }
    Ok(f)
}

// ======================================== MATRIX LOG ========================================

/// `gauss_legendre(n)`: the nodes (ascending) and weights of `n`-point Gauss-Legendre quadrature
/// by Golub and Welsch: the eigenvalues of the symmetric tridiagonal matrix with `i / sqrt(4 i^2
/// - 1)` beside its diagonal (`vnl_symmetric_eigensystem`), and `2 v(0)^2` of each eigenvector.
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut m = vec![0.0f64; n * n];
    for i in 1..n {
        let fi = i as f64;
        let d = div64(fi, sqrt64(mul64(mul64(4.0, fi), fi) - 1.0));
        m[(i - 1) * n + i] = d;
        m[i * n + (i - 1)] = d;
    }
    let e = symmetric_eigensystem(&m, n);
    let x = e.values.clone();
    let w = (0..n)
        .map(|c| {
            let v = e.vectors[c];
            mul64(mul64(2.0, v), v)
        })
        .collect();
    (x, w)
}

/// `blocking(A, delta)`: the block of each diagonal element of the upper triangular `A`: a new
/// block for an element not yet in one, and two blocks merged (the larger index into the smaller,
/// the indices above it lowered by one) whenever two elements of different blocks are within
/// `delta` (`cabs` of their difference).
fn blocking(a: &VnlCMatrix, delta: f64) -> Vec<i32> {
    let n = a.cols;
    let d: Vec<C64> = (0..n).map(|i| a.at(i, i)).collect();
    let mut m = vec![-1i32; n];
    let mut max_m = 0i32;
    for i in 0..n {
        if m[i] == -1 {
            m[i] = max_m;
            max_m += 1;
        }
        for j in i + 1..n {
            if m[i] != m[j] && cabs(d[i].sub(d[j])) <= delta {
                if m[j] == -1 {
                    m[j] = m[i];
                } else {
                    let p = m[i].max(m[j]);
                    let q = m[i].min(m[j]);
                    for k in 0..n {
                        if m[k] == p {
                            m[k] = q;
                        } else if m[k] > p {
                            m[k] -= 1;
                        }
                    }
                    max_m -= 1;
                }
            }
        }
    }
    m
}

/// `swapping(m)`: the blocks ordered by their elements' average index (`std::sort` of `(average,
/// block)` pairs), `mm` each element's place in that order and `ind` the indices of each block in
/// the reordered form (consecutive, `cumsum0` of the sizes).
fn swapping(m: &[i32]) -> (Vec<i32>, Vec<Vec<usize>>) {
    let mmax = (*m.iter().max().expect("a block") + 1) as usize;
    let mut mm = vec![0i32; m.len()];
    let mut gpairs: Vec<(f64, usize)> = Vec::with_capacity(mmax);
    let mut h = vec![0i32; mmax];
    for (i, hi) in h.iter_mut().enumerate() {
        let (mut lengthp, mut sump) = (0i32, 0i32);
        for (j, &mj) in m.iter().enumerate() {
            if mj == i as i32 {
                lengthp += 1;
                sump += j as i32;
            }
        }
        *hi = lengthp;
        gpairs.push((div64(f64::from(sump), f64::from(lengthp)), i));
    }
    gpairs.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    let mut cs = vec![0i32; mmax + 1];
    let mut sum = 0i32;
    for i in 0..mmax {
        sum += h[gpairs[i].1];
        cs[i + 1] = sum;
    }
    let mut ind = Vec::with_capacity(mmax);
    for (i, pair) in gpairs.iter().enumerate() {
        for (j, &mj) in m.iter().enumerate() {
            if mj == pair.1 as i32 {
                mm[j] = i as i32;
            }
        }
        ind.push((cs[i] as usize..cs[i + 1] as usize).collect());
    }
    (mm, ind)
}

/// `OrdSchurComplexLogical(U, T, select)`: LAPACK's `ZTRSEN('N', 'V', ...)` on the transposes
/// (Fortran's column order), the selected eigenvalues moved to the front.
fn ord_schur_logical(u: &mut VnlCMatrix, t: &mut VnlCMatrix, select: &[i32]) {
    let n = t.rows;
    // the row-major transpose is the column-major original
    let mut tc = t.transpose().data;
    let mut qc = u.transpose().data;
    let _ = ztrsen_nv(n, select, &mut tc, &mut qc);
    let back = |d: Vec<C64>| {
        VnlCMatrix {
            rows: n,
            cols: n,
            data: d,
        }
        .transpose()
    };
    *t = back(tc);
    *u = back(qc);
}

/// `OrdSchurComplex(U, T, clusters)`: for each cluster index from 0 up, its eigenvalues moved to
/// the front (unless already there) and the cluster list reordered alike.
fn ord_schur_complex(u: &VnlCMatrix, t: &VnlCMatrix, clusters: &[i32]) -> (VnlCMatrix, VnlCMatrix) {
    let n = t.cols;
    let (mut us, mut ts) = (u.clone(), t.clone());
    let maxi = *clusters.iter().max().expect("a cluster");
    let mut my = clusters.to_vec();
    for i in 0..=maxi {
        let select: Vec<i32> = my.iter().map(|&c| i32::from(c == i)).collect();
        let foundi = select.iter().filter(|&&s| s != 0).count();
        if foundi == 0 {
            continue;
        }
        let mut startvals = 0;
        while startvals < n && select[startvals] != 0 {
            startvals += 1;
        }
        if foundi == startvals {
            continue;
        }
        ord_schur_logical(&mut us, &mut ts, &select);
        let my2 = my.clone();
        let mut count = 0;
        for j in 0..n {
            if j < foundi {
                my[j] = i;
            } else {
                while count < n && my2[count] == i {
                    count += 1;
                }
                my[j] = my2[count];
                count += 1;
            }
        }
    }
    (us, ts)
}

/// `MatrixLog_pf(A, m)`: `log(I + A)` by the `[m/m]` Pade approximant in partial fractions:
/// `sum_j w_j A (I + x_j A)^-1` on the `m` Gauss-Legendre nodes and weights mapped to `[0, 1]`,
/// each term solved by `vnl_svd<std::complex<double>>(I + x_j A^T).solve(A^T)` and the sum
/// transposed back.
fn matrix_log_pf(err: &mut dyn Write, a: &VnlCMatrix, m: usize) -> Result<VnlCMatrix, Fatal> {
    let n = a.cols;
    let (nodes, wts) = gauss_legendre(m);
    let nodes: Vec<f64> = nodes.iter().map(|&x| div64(add64(x, 1.0), 2.0)).collect();
    let wts: Vec<f64> = wts.iter().map(|&x| div64(x, 2.0)).collect();
    let mut s = VnlCMatrix::zeros(n, n);
    let id = cidentity(n);
    let at = a.transpose();
    for j in 0..m {
        let x = cadd(&id, &cscale(&at, C64::real(nodes[j])));
        let d = csvd(err, &x)?;
        s = cadd(&s, &cscale(&d.solve_matrix(&at), C64::real(wts[j])));
    }
    Ok(s.transpose())
}

/// `MatrixLog_isst(A, maxlogiter)`: the logarithm of an upper triangular complex matrix by
/// inverse scaling and squaring: a `1 x 1` matrix's `std::log` (the decided `clog`); otherwise
/// upper triangular square roots (column by column, `R(j, j) = sqrt(T(j, j))`, then for `i` from
/// `j - 1` down `R(i, j) = (T(i, j) - sum_{i<k<j} R(i, k) R(k, j)) / (R(i, i) + R(j, j))`) until
/// `||T - I||_F <= 1/4`, then `2^iter MatrixLog_pf(T - I, 8)`. Reaching `maxlogiter - 1` roots
/// ends the program (`MatrixLog_isst:tooManyIterations Too many square roots.`, status 1).
fn matrix_log_isst(err: &mut dyn Write, a: &VnlCMatrix, maxlogiter: i32) -> Result<VnlCMatrix, Fatal> {
    let n = a.cols;
    if n == 1 {
        let mut m = VnlCMatrix::zeros(1, 1);
        *m.at_mut(0, 0) = clog(a.at(0, 0));
        return Ok(m);
    }
    let mut t = a.clone();
    let mut r = a.clone();
    let mut iter = 0;
    while iter < maxlogiter {
        let phi = cfro(&csub_diag(&t, C64::ONE));
        if phi <= 0.25 {
            break;
        }
        if iter == maxlogiter - 1 {
            let _ = writeln!(err, "MatrixLog_isst:tooManyIterations Too many square roots.");
            return Err(Fatal::Exit(1));
        }
        for j in 0..n {
            *r.at_mut(j, j) = csqrt(t.at(j, j));
            for i in (0..j).rev() {
                let mut s = C64::ZERO;
                for k in i + 1..j {
                    s = s.add(cxx_mul(r.at(i, k), r.at(k, j)));
                }
                *r.at_mut(i, j) = cxx_div(t.at(i, j).sub(s), r.at(i, i).add(r.at(j, j)));
            }
        }
        t = r.clone();
        iter += 1;
    }
    let pf = matrix_log_pf(err, &csub_diag(&t, C64::ONE), 8)?;
    Ok(cscale(&pf, C64::real(pow2(iter))))
}

/// `sylv_tri(T, U, B)`: `X` with `T X + X U = B` for upper triangular `T` (`m x m`) and `U` (`n x
/// n`), column by column: `X(:, i) = (T + U(i, i) I) \ (B(:, i) - X(:, 0:i) U(0:i, i))`, each
/// system by `vnl_svd<std::complex<double>>`'s `solve`.
///
/// FS-1100 (rule A): FreeSurfer subtracts `X(:, 0:i-1) U(0:i-1, i)` only (`get_n_columns(0, i -
/// 1)`: the last solved column left out), so every column after the first misses a term when `U`
/// has two or more columns; by default the full sum, `reproduce` FreeSurfer's.
fn sylv_tri(
    g: &mut Globals,
    err: &mut dyn Write,
    t: &VnlCMatrix,
    u: &VnlCMatrix,
    b: &VnlCMatrix,
) -> Result<VnlCMatrix, Fatal> {
    let m = t.cols;
    let n = u.cols;
    if n >= 2 {
        g.notices.add(
            &FS_1100,
            "the matrix logarithm's triangular Sylvester equation of a block beside a block of two or more eigenvalues: FreeSurfer leaves the last solved column's term out",
        );
    }
    let keep = if g.notices.reproduce() { 1 } else { 0 };
    let mut x = VnlCMatrix::zeros(m, n);
    for i in 0..n {
        let a = cadd_diag(t, u.at(i, i));
        let mut y: Vec<C64> = (0..m).map(|r| b.at(r, i)).collect();
        if i > 0 {
            // X(:, 0:k) U(0:k, i): k = i - 1 columns as FreeSurfer, i by default
            let k = i - keep;
            let rows: Vec<usize> = (0..m).collect();
            let cols: Vec<usize> = (0..k).collect();
            let xk = get_sub(&x, &rows, &cols);
            let uk: Vec<C64> = (0..k).map(|r| u.at(r, i)).collect();
            let p = mat_vec(&xk, &uk);
            for (yr, pr) in y.iter_mut().zip(p) {
                *yr = yr.sub(pr);
            }
        }
        let d = csvd(err, &a)?;
        let col = d.solve_vector(&y);
        for (r, z) in col.into_iter().enumerate() {
            *x.at_mut(r, i) = z;
        }
    }
    Ok(x)
}

/// `MyMatrix::MatrixLog(A, maxlogiter)` of a square real matrix (`maxlogiter` 100 by default):
/// a `1 x 1` matrix's `log`; otherwise the complex Schur form `A = U T U^H` ([`schur_complex`]),
/// FreeSurfer's message for each eigenvalue with a real part at most 0 and an imaginary part below
/// `1e-9` in magnitude, the blocks of eigenvalues within `0.1` of each other ([`blocking`],
/// [`swapping`]) brought together by reordering the Schur form, each diagonal block's logarithm
/// ([`matrix_log_isst`]), the blocks above the diagonal by the block Parlett recurrence (the
/// scalar formula `F(i, j) = (T(i, j) (F(i, i) - F(j, j)) + sum F(i, k) T(k, j) - sum T(i, k) F(k,
/// j)) / (T(i, i) - T(j, j))` between two single eigenvalues, [`sylv_tri`] otherwise), then `F =
/// U F U^H`; an imaginary part whose one-norm passes `10 n 2^-52` times the result's ends the
/// program (`Result too imaginary to ignore!` and FreeSurfer's diagnostics, status 1; FS-1312:
/// by default only above `sqrt(2^-52)` times the result's). Returns the real part.
///
/// FS-1099 (rule B): FreeSurfer prints `Principal matrix logarithm is not defined for A with
/// nonpositive real eigenvalues.` and goes on (its assertion compiled out) to a logarithm that is
/// not the principal one, nearly always ending in the imaginary-part check; by default the
/// message and the end of the program (status 1), `reproduce` goes on as FreeSurfer does.
pub fn matrix_log(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    a: &VnlMatrix,
    maxlogiter: i32,
) -> Result<VnlMatrix, Fatal> {
    assert_eq!(a.rows, a.cols, "MyMatrix::MatrixLog: a square matrix");
    let n = a.cols;
    if n == 1 {
        let x = a.at(0, 0);
        let l = if g.notices.reproduce() {
            math::log_glibc(x)
        } else {
            math::log(x)
        };
        return Ok(VnlMatrix {
            rows: 1,
            cols: 1,
            data: vec![l],
        });
    }
    let (u, t) = schur_complex(g, out, err, a)?;
    let eps = 0.000_000_001;
    for i in 0..n {
        let z = t.at(i, i);
        if z.re <= 0.0 && z.im.abs() < eps {
            let _ = write!(
                err,
                "MatrixLog Error: \n Principal matrix logarithm is not defined for A with \n          nonpositive real eigenvalues. \n"
            );
            g.notices.add(
                &FS_1099,
                "the matrix logarithm of a matrix with an eigenvalue on the nonpositive real axis: FreeSurfer goes on to a logarithm that is not the principal one",
            );
            if !g.notices.reproduce() {
                return Err(Fatal::Exit(1));
            }
        }
    }
    let ord1 = blocking(&t, 0.1);
    let (mut ord, ind) = swapping(&ord1);
    let mord = *ord.iter().max().expect("an eigenvalue");
    for o in &mut ord {
        *o = mord - *o;
    }
    let (u, t) = ord_schur_complex(&u, &t, &ord);
    let mut f = VnlCMatrix::zeros(n, n);
    for col in 0..ind.len() {
        let jj = &ind[col];
        let blk = matrix_log_isst(err, &get_sub(&t, jj, jj), maxlogiter)?;
        set_sub(&mut f, jj, jj, &blk);
        for row in (0..col).rev() {
            let ii = &ind[row];
            if ii.len() == 1 && jj.len() == 1 {
                let (i, j) = (ii[0], jj[0]);
                let mut temp1 = C64::ZERO;
                let mut temp2 = C64::ZERO;
                for k in i + 1..j {
                    temp1 = temp1.add(cxx_mul(f.at(i, k), t.at(k, j)));
                    temp2 = temp2.add(cxx_mul(t.at(i, k), f.at(k, j)));
                }
                let num = cxx_mul(t.at(i, j), f.at(i, i).sub(f.at(j, j)))
                    .add(temp1)
                    .sub(temp2);
                *f.at_mut(i, j) = cxx_div(num, t.at(i, i).sub(t.at(j, j)));
            } else {
                let kk: Vec<usize> = ind[row + 1..col].iter().flatten().copied().collect();
                let mut rhs = get_sub(&f, ii, ii).mul(&get_sub(&t, ii, jj));
                rhs = csub(&rhs, &get_sub(&t, ii, jj).mul(&get_sub(&f, jj, jj)));
                if !kk.is_empty() {
                    rhs = cadd(&rhs, &get_sub(&f, ii, &kk).mul(&get_sub(&t, &kk, jj)));
                    rhs = csub(&rhs, &get_sub(&t, ii, &kk).mul(&get_sub(&f, &kk, jj)));
                }
                let x = sylv_tri(
                    g,
                    err,
                    &get_sub(&t, ii, ii),
                    &cneg(&get_sub(&t, jj, jj)),
                    &rhs,
                )?;
                set_sub(&mut f, ii, jj, &x);
            }
        }
    }
    let f = u.mul(&f).mul(&u.conjugate_transpose());
    let eps_matlab = f64::EPSILON;
    let imagone = f.imag().operator_one_norm();
    let fnorm = f.operator_one_norm();
    let bound = mul64(mul64((10 * n as i32) as f64, eps_matlab), fnorm);
    let mut refuse = imagone > bound;
    if refuse {
        // FS-1312 (rule A): FreeSurfer's bound is below its own complex arithmetic's rounding
        g.notices.add(
            &FS_1312,
            "the matrix logarithm's imaginary part above FreeSurfer's bound: FreeSurfer ends the program",
        );
        if !g.notices.reproduce() {
            refuse = imagone > mul64(f64::EPSILON.sqrt(), fnorm);
        }
    }
    if refuse {
        let mut s = String::new();
        s.push_str(" MatrixLog Error: \n");
        s.push_str(&format!(
            "  Result too imaginary to ignore! ( {} )\n",
            cfmt::ostream(imagone)
        ));
        s.push_str(&format!(" A = {}\n", print_real(a)));
        s.push_str(&format!(" T = {}\n", print_complex(&t)));
        s.push_str(&format!(" U = {}\n", print_complex(&u)));
        let back = u.mul(&t).mul(&u.conjugate_transpose()).real().sub(a).fro_norm();
        s.push_str(&format!(
            " fro( real(U * T * U^*) - A) = {}\n\n",
            cfmt::g(back, 16)
        ));
        s.push_str(&format!(" log(A) = {}\n", print_complex(&f)));
        let _ = err.write_all(s.as_bytes());
        let ex = matrix_exp(g, err, &f.real())?;
        let _ = write!(
            err,
            " fro( exp(real(log(A))) - A)  = {}\n\n",
            cfmt::g(ex.sub(a).fro_norm(), 16)
        );
        return Err(Fatal::Exit(1));
    }
    Ok(f.real())
}

/// `MyMatrix::GeometricMean(vm)` of `vnl_matrix_fixed<double, 3, 3>`s (`n` their count):
/// `MatrixExp((1 / n) sum_i MatrixLog(vm[i]))`, the logarithms summed in order.
pub fn geometric_mean3(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vm: &[Mat3],
) -> Result<Mat3, Fatal> {
    assert!(!vm.is_empty(), "MyMatrix::GeometricMean: a matrix");
    let n = vm.len() as i32;
    let dynm = |m: &Mat3| VnlMatrix {
        rows: 3,
        cols: 3,
        data: m.iter().flatten().copied().collect(),
    };
    let mut geo = matrix_log(g, out, err, &dynm(&vm[0]), 100)?;
    for m in &vm[1..] {
        geo = madd(&geo, &matrix_log(g, out, err, &dynm(m), 100)?);
    }
    let e = matrix_exp(g, err, &mscale(&geo, div64(1.0, f64::from(n))))?;
    Ok(std::array::from_fn(|i| std::array::from_fn(|j| e.at(i, j))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn m3(v: [f64; 9]) -> VnlMatrix {
        VnlMatrix {
            rows: 3,
            cols: 3,
            data: v.to_vec(),
        }
    }

    /// `exp(log(A))` gives `A` back, on one block and on separate blocks of eigenvalues.
    #[test]
    fn log_and_exp_invert_each_other() {
        for a in [
            m3([1.02, 0.03, -0.01, -0.02, 0.99, 0.04, 0.01, -0.03, 1.01]),
            // a rotation by about 0.5 radians about z, scaled: three separate blocks
            m3([
                0.877_582_56,
                -0.479_425_54,
                0.0,
                0.479_425_54,
                0.877_582_56,
                0.0,
                0.0,
                0.0,
                1.3,
            ]),
            // a pair of close eigenvalues beside a separate one: sylv_tri
            m3([1.0, 0.02, 0.3, -0.02, 1.0, 0.1, 0.0, 0.0, 1.5]),
        ] {
            for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
                let mut g = Globals::new(mode);
                let (mut o, mut e) = (Vec::new(), Vec::new());
                let l = matrix_log(&mut g, &mut o, &mut e, &a, 100).expect("a logarithm");
                let back = matrix_exp(&mut g, &mut e, &l).expect("an exponential");
                let d = back.sub(&a).fro_norm();
                if mode == UpstreamDefects::Fix {
                    assert!(d < 1e-13, "exp(log(A)) - A: {d}");
                }
                assert!(e.is_empty(), "{}", String::from_utf8_lossy(&e));
            }
        }
    }

    /// The Gauss-Legendre rule integrates polynomials of degree up to 15 exactly.
    #[test]
    fn gauss_legendre_integrates() {
        let (x, w) = gauss_legendre(8);
        let s: f64 = x.iter().zip(&w).map(|(&xi, &wi)| wi * xi.powi(14)).sum();
        assert!((s - 2.0 / 15.0).abs() < 1e-14);
        assert!(x.windows(2).all(|p| p[0] < p[1]));
    }

    /// A reflection: FS-1099 by default.
    #[test]
    fn negative_eigenvalue_is_refused() {
        let a = m3([-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        assert!(matrix_log(&mut g, &mut o, &mut e, &a, 100).is_err());
        assert!(String::from_utf8_lossy(&e).contains("nonpositive real eigenvalues"));
    }
}
