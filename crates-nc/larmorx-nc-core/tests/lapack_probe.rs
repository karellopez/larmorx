// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! The replica's side of `validation/probes/fsl-miscmaths/lapack_probe.c`: for every case file
//! `*.case` under `LP_PROBE_ROOT`, writes `<case>.replica` with the lines the probe prints from
//! FSL's OpenBLAS (`dgesv`, `dsyrk('U', 'T')`), from `larmorx_nc_core::lapack::blocked` and
//! `arma::syrk_t`. Run with `--ignored`.

use larmorx_nc_core::arma::{Mat, syrk_t};
use larmorx_nc_core::lapack::blocked::gesv_any;
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
            let info = gesv_any(&mut a, n, &mut ipiv, &mut x, nrhs);
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
