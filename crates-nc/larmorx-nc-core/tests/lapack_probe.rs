// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! The replica's side of `validation/probes/fsl-miscmaths/lapack_probe.c`: for every case file
//! `*.case` under `LP_PROBE_ROOT`, writes `<case>.replica` with the lines the probe prints from
//! FSL's OpenBLAS (`dgesv`, `dsyrk('U', 'T')`, `dgemm`, Armadillo's `dsytrf`/`dsytrs` sequence
//! and the BLAS routines it calls), from `larmorx_nc_core::lapack::blocked`, `arma::syrk_t`,
//! `lapack::blas` and `lapack::sytrf`. Run with `--ignored`.

use larmorx_nc_core::arma::{Mat, syrk_t};
use larmorx_nc_core::lapack::blas::{MatRef, axpy_k, dgemm, dgemv_n, dgemv_t_inc, dger, dsyr_l};
use larmorx_nc_core::lapack::blocked::{gesv_any, gesv_one, getrf_any, getri_any};
use larmorx_nc_core::lapack::sytrf::{armadillo_lwork, inv_sym, sytrf_l, sytri_l, sytrs_l};
use std::fmt::Write as _;

fn f64s(b: &[u8], off: usize, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| f64::from_le_bytes(b[off + 8 * i..off + 8 * i + 8].try_into().unwrap()))
        .collect()
}

fn pd(out: &mut String, tag: &str, v: f64) {
    writeln!(out, "{tag} {:016x}", v.to_bits()).unwrap();
}

#[test]
#[ignore]
fn lapack_probe() {
    let root = std::env::var("LP_PROBE_ROOT").expect("LP_PROBE_ROOT");
    for e in std::fs::read_dir(&root).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("case") {
            continue;
        }
        let b = std::fs::read(&p).unwrap();
        let h: Vec<u32> = (0..3)
            .map(|i| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap()))
            .collect();
        let mut out = String::new();
        if h[0] == 0 {
            let (n, nrhs) = (h[1] as usize, h[2] as usize);
            let mut a = f64s(&b, 12, n * n);
            let mut x = f64s(&b, 12 + 8 * n * n, n * nrhs);
            let mut ipiv = vec![0i32; n];
            let info = if nrhs == 1 {
                gesv_one(&mut a, n, &mut ipiv, &mut x)
            } else {
                gesv_any(&mut a, n, &mut ipiv, &mut x, nrhs)
            };
            writeln!(out, "info {info}").unwrap();
            for v in &ipiv {
                writeln!(out, "ipiv {v}").unwrap();
            }
            for &v in &a {
                pd(&mut out, "lu", v);
            }
            if info == 0 {
                for &v in &x {
                    pd(&mut out, "x", v);
                }
            } else {
                // dgesv leaves B as it was when the factorisation failed.
                for &v in &f64s(&b, 12 + 8 * n * n, n * nrhs) {
                    pd(&mut out, "x", v);
                }
            }
        } else if h[0] == 5 {
            let n = h[1] as usize;
            let mut a = f64s(&b, 12, n * n);
            let mut ipiv = vec![0i32; n];
            let info = getrf_any(&mut a, n, n, n, &mut ipiv);
            writeln!(out, "info {info}").unwrap();
            if info == 0 {
                if n > 16 {
                    // dgetri's workspace query: N * NB, NB = 64
                    writeln!(out, "lwork {}", (n * 64).max(n)).unwrap();
                }
                let info2 = getri_any(&mut a, n, &ipiv);
                writeln!(out, "info2 {info2}").unwrap();
                for &v in &a {
                    pd(&mut out, "inv", v);
                }
            }
        } else if h[0] == 6 {
            let n = h[1] as usize;
            let a0 = f64s(&b, 12, n * n);
            let mut f = a0.clone();
            let mut ipiv = vec![0i32; n];
            let info = sytrf_l(&mut f, n, n, &mut ipiv, armadillo_lwork(n));
            writeln!(out, "info {info}").unwrap();
            if info == 0 {
                let info2 = sytri_l(&mut f, n, n, &ipiv);
                writeln!(out, "info2 {info2}").unwrap();
                if info2 == 0 {
                    let inv = inv_sym(&a0, n).expect("inverse");
                    for &v in &inv {
                        pd(&mut out, "inv", v);
                    }
                }
            }
        } else if h[0] == 3 {
            let (n, nrhs) = (h[1] as usize, h[2] as usize);
            let mut a = f64s(&b, 12, n * n);
            let mut x = f64s(&b, 12 + 8 * n * n, n * nrhs);
            let lwork = armadillo_lwork(n);
            if n > 16 {
                writeln!(out, "lwork {lwork}").unwrap();
            }
            let mut ipiv = vec![0i32; n];
            let info = sytrf_l(&mut a, n, n, &mut ipiv, lwork);
            writeln!(out, "info {info}").unwrap();
            for v in &ipiv {
                writeln!(out, "ipiv {v}").unwrap();
            }
            for j in 0..n {
                for i in j..n {
                    pd(&mut out, "f", a[j * n + i]);
                }
            }
            if info == 0 {
                sytrs_l(&a, n, n, &ipiv, &mut x, n, nrhs);
                writeln!(out, "info2 0").unwrap();
                for &v in &x {
                    pd(&mut out, "x", v);
                }
            }
        } else if h[0] == 4 {
            let (op, m) = (h[1], h[2] as usize);
            let n = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
            let mut off = 16;
            let mut take = |k: usize| {
                let v = f64s(&b, off, k);
                off += 8 * k;
                v
            };
            match op {
                0 => {
                    let alpha = take(1)[0];
                    let x = take(m);
                    let mut y = take(m);
                    if alpha != 0.0 {
                        axpy_k(alpha, &x, 1, &mut y, 1);
                    }
                    for &v in &y {
                        pd(&mut out, "y", v);
                    }
                }
                1 => {
                    let ld = m + 1;
                    let x = take(m);
                    let mut bm = take(ld * n);
                    let y: Vec<f64> = (0..n).map(|j| bm[j * ld]).collect();
                    dger(-1.0, &x, &y, &mut bm, MatRef { off: 1, ld });
                    for &v in &bm {
                        pd(&mut out, "b", v);
                    }
                }
                2 => {
                    let ld = m + 3;
                    let alpha = take(1)[0];
                    let x = take(m);
                    let mut a = take(ld * m);
                    dsyr_l(alpha, &x, &mut a, MatRef { off: 0, ld });
                    for &v in &a {
                        pd(&mut out, "a", v);
                    }
                }
                3 => {
                    let ld = m + 1;
                    let x = take(m);
                    let bm = take(ld * n);
                    let mut y: Vec<f64> = (0..n).map(|j| bm[j * ld]).collect();
                    dgemv_t_inc(
                        m,
                        n,
                        -1.0,
                        &bm,
                        MatRef { off: 1, ld },
                        &x,
                        1,
                        1.0,
                        &mut y,
                        ld,
                    );
                    for &v in &y {
                        pd(&mut out, "y", v);
                    }
                }
                4 => {
                    let a = take(m * n);
                    let xs = take(n * 5);
                    let x: Vec<f64> = (0..n).map(|j| xs[5 * j]).collect();
                    let mut y = take(m);
                    dgemv_n(m, n, -1.0, &a, MatRef { off: 0, ld: m }, &x, 5, 1.0, &mut y);
                    for &v in &y {
                        pd(&mut out, "y", v);
                    }
                }
                _ => {
                    let k = u32::from_le_bytes(b[16..20].try_into().unwrap()) as usize;
                    off = 20;
                    let ldw = n + 2;
                    let mut take = |k: usize| {
                        let v = f64s(&b, off, k);
                        off += 8 * k;
                        v
                    };
                    let a = take(m * k);
                    let w = take(ldw * k);
                    let mut c = take(m * n);
                    dgemm(
                        false,
                        true,
                        m,
                        n,
                        k,
                        -1.0,
                        &a,
                        MatRef { off: 0, ld: m },
                        &w,
                        MatRef { off: 0, ld: ldw },
                        1.0,
                        &mut c,
                        MatRef { off: 0, ld: m },
                    );
                    for &v in &c {
                        pd(&mut out, "c", v);
                    }
                }
            }
        } else if h[0] == 2 {
            let (m, n) = (h[1] as usize, h[2] as usize);
            let k = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
            let a = f64s(&b, 16, m * k);
            let bb = f64s(&b, 16 + 8 * m * k, k * n);
            let mut c = vec![0.0; m * n];
            dgemm(
                false,
                false,
                m,
                n,
                k,
                1.0,
                &a,
                MatRef { off: 0, ld: m },
                &bb,
                MatRef { off: 0, ld: k },
                0.0,
                &mut c,
                MatRef { off: 0, ld: m },
            );
            for &v in &c {
                pd(&mut out, "c", v);
            }
        } else {
            let (k, n) = (h[1] as usize, h[2] as usize);
            let a = Mat::from_col_major(k, n, &f64s(&b, 12, k * n));
            let c = syrk_t(&a);
            for j in 0..n {
                for i in 0..=j {
                    pd(&mut out, "c", c.at(i, j));
                }
            }
        }
        std::fs::write(p.with_extension("replica"), out).unwrap();
    }
}
