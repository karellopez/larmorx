// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The eigen- and singular-value operations of
//! `validation/investigations/fsl-flirt/lapack_eig_svd_probe.py` (its record layouts):
//!
//! - `syevd<n>`: an `n × n` symmetric matrix; out the eigenvalues, the vectors, `info`;
//! - `gesdd<m>x<n>`: an `m × n` matrix; out `s`, `U` (`m × k`), `Vᵀ` (`k × n`), `info`;
//! - `gemvn<m>x<n>`, `gemvt<m>x<n>`: `A`, `x`, `y`, `alpha`, `beta`; out `y`;
//! - `nrm2_<n>`: a vector; out its norm;
//! - `gemm<ta><tb><m>x<n>x<k>` (`ta`, `tb` each `n` or `t`): `A`, `B`, `C`, `alpha`, `beta`; out
//!   `C`;
//! - `armaeig<n>`, `armasvd<m>x<n>`, `armapinv<m>x<n>`: Armadillo 14.2.1's `eig_sym`,
//!   `svd_econ` and `pinv` (out the results, then 1, or zeros and 0 when they fail).

use larmorx_nc_core::lapack::blas::{self, MatRef};
use larmorx_nc_core::lapack::reference;

fn dims(s: &str) -> Vec<usize> {
    s.split('x').map(|t| t.parse().expect("size")).collect()
}

pub fn run(op: &str, count: usize, words: &[f64]) -> Option<Vec<f64>> {
    let mut out = Vec::new();
    if let Some(rest) = op.strip_prefix("syevd") {
        let n: usize = rest.parse().ok()?;
        for r in words.chunks_exact(n * n).take(count) {
            let mut a = r.to_vec();
            let mut w = vec![0.0; n];
            let info = reference::dsyevd_vu(n, &mut a, &mut w);
            out.extend(&w);
            out.extend(&a);
            out.push(info as f64);
        }
        return Some(out);
    }
    if let Some(rest) = op.strip_prefix("gesdd") {
        let d = dims(rest);
        let (m, n) = (d[0], d[1]);
        let k = m.min(n);
        for r in words.chunks_exact(m * n).take(count) {
            let mut a = r.to_vec();
            let mut s = vec![0.0; k];
            let mut u = vec![0.0; m * k];
            let mut vt = vec![0.0; k * n];
            let info = reference::dgesdd_s(m, n, &mut a, &mut s, &mut u, &mut vt);
            out.extend(&s);
            out.extend(&u);
            out.extend(&vt);
            out.push(info as f64);
        }
        return Some(out);
    }
    if let Some(rest) = op.strip_prefix("gemvn").or_else(|| op.strip_prefix("gemvt")) {
        let trans = op.starts_with("gemvt");
        let d = dims(rest);
        let (m, n) = (d[0], d[1]);
        let (lx, ly) = if trans { (m, n) } else { (n, m) };
        let rec = m * n + lx + ly + 2;
        for r in words.chunks_exact(rec).take(count) {
            let a = &r[..m * n];
            let x = &r[m * n..m * n + lx];
            let mut y = r[m * n + lx..m * n + lx + ly].to_vec();
            let alpha = r[rec - 2];
            let beta = r[rec - 1];
            let am = MatRef { off: 0, ld: m };
            if trans {
                blas::dgemv_t(m, n, alpha, a, am, x, 1, beta, &mut y);
            } else {
                blas::dgemv_n(m, n, alpha, a, am, x, 1, beta, &mut y);
            }
            out.extend(&y);
        }
        return Some(out);
    }
    if let Some(rest) = op.strip_prefix("rot").and_then(|r| r.split_once('_')) {
        // rot<inc>_<n>: x (n), y (n), c, s; out x, y.
        let inc: usize = rest.0.parse().ok()?;
        let n: usize = rest.1.parse().ok()?;
        for r in words.chunks_exact(2 * n + 2).take(count) {
            let mut x = r[..n].to_vec();
            let mut y = r[n..2 * n].to_vec();
            blas::drot(&mut x, inc, &mut y, inc, r[2 * n], r[2 * n + 1]);
            out.extend(&x);
            out.extend(&y);
        }
        return Some(out);
    }
    if let Some(rest) = op.strip_prefix("nrm2_") {
        let n: usize = rest.parse().ok()?;
        for r in words.chunks_exact(n).take(count) {
            out.push(blas::dnrm2(r, 1));
        }
        return Some(out);
    }
    if let Some(rest) = op.strip_prefix("gemm") {
        let ta = rest.as_bytes()[0] == b't';
        let tb = rest.as_bytes()[1] == b't';
        let d = dims(&rest[2..]);
        let (m, n, k) = (d[0], d[1], d[2]);
        let rec = m * k + k * n + m * n + 2;
        for r in words.chunks_exact(rec).take(count) {
            let a = &r[..m * k];
            let b = &r[m * k..m * k + k * n];
            let mut c = r[m * k + k * n..m * k + k * n + m * n].to_vec();
            let alpha = r[rec - 2];
            let beta = r[rec - 1];
            let am = MatRef {
                off: 0,
                ld: if ta { k } else { m },
            };
            let bm = MatRef {
                off: 0,
                ld: if tb { n } else { k },
            };
            let cm = MatRef { off: 0, ld: m };
            blas::dgemm(ta, tb, m, n, k, alpha, a, am, b, bm, beta, &mut c, cm);
            out.extend(&c);
        }
        return Some(out);
    }
    None
}
