// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The port's side of `validation/investigations/version-numerics/lapack_probe.py`: reads
//! `count` records of little-endian doubles (an `n × n` column-major matrix, then for the solves
//! an `n × nrhs` right-hand side), runs one operation on each and writes the output words in the
//! layout the probe writes for FSL's own LAPACK:
//!
//! - `getrf<n>`: the factors (`n²`), the pivots (`n`), `info`;
//! - `getri<n>`: the result (`n²`), `dgetrf`'s `info`, `dgetri`'s (`-99` when not called);
//! - `trtri_u<n>`, `trtri_l<n>`: the result (`n²`), `info`;
//! - `gesv<n>x<nrhs>`: the solution, the factors, the pivots, `info`;
//! - `arma_inv<n>`, `arma_solve<n>x<nrhs>`: Armadillo 14.2.1's `inv()` and `inv(A) * B`
//!   (`arma::inv`, `arma::solve`): the result (zeros on failure), then 1 or 0;
//! - `arma_det<n>`: `arma::det`.
//!
//! Usage: `lapack-probe <op> <count> <in.f64> <out.f64>`.

use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::lapack;
use std::io::Write;

fn parse(op: &str) -> (usize, usize) {
    let digits: String = op
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .to_string();
    let mut it = digits.split('x');
    let n = it.next().unwrap().parse().unwrap();
    let nrhs = it.next().map_or(0, |s| s.parse().unwrap());
    (n, nrhs)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 5, "usage: lapack-probe <op> <count> <in> <out>");
    let op = args[1].as_str();
    let count: usize = args[2].parse().expect("count");
    let raw = std::fs::read(&args[3]).expect("input");
    let words: Vec<f64> = raw
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| f64::from_le_bytes(*c))
        .collect();
    let (n, nrhs) = parse(op);
    let nn = n * n;
    let rec = nn + n * nrhs;
    assert_eq!(words.len(), count * rec, "input size");
    let mut out: Vec<f64> = Vec::new();
    for r in words.chunks_exact(rec) {
        let mut a = r[..nn].to_vec();
        let mut ipiv = [0i32; 4];
        if op.starts_with("getrf") {
            let info = lapack::getrf(&mut a, n, &mut ipiv);
            out.extend(&a);
            out.extend(ipiv[..n].iter().map(|&p| f64::from(p)));
            out.push(f64::from(info));
        } else if op.starts_with("getri") {
            let info1 = lapack::getrf(&mut a, n, &mut ipiv);
            let info2 = if info1 == 0 {
                lapack::getri(&mut a, n, &ipiv)
            } else {
                -99
            };
            out.extend(&a);
            out.push(f64::from(info1));
            out.push(f64::from(info2));
        } else if op.starts_with("trtri") {
            let info = lapack::trtri(&mut a, n, op.contains("_u"));
            out.extend(&a);
            out.push(f64::from(info));
        } else if op.starts_with("gesv") {
            let mut b = r[nn..].to_vec();
            let info = lapack::gesv(&mut a, n, &mut ipiv, &mut b, nrhs);
            out.extend(&b);
            out.extend(&a);
            out.extend(ipiv[..n].iter().map(|&p| f64::from(p)));
            out.push(f64::from(info));
        } else {
            let m = Mat::from_col_major(n, n, &a);
            if op.starts_with("arma_inv") {
                match arma::inv(&m) {
                    Ok(x) => {
                        out.extend(x.as_slice());
                        out.push(1.0);
                    }
                    Err(_) => {
                        out.extend(std::iter::repeat_n(0.0, nn));
                        out.push(0.0);
                    }
                }
            } else if op.starts_with("arma_solve") {
                let b = Mat::from_col_major(n, nrhs, &r[nn..]);
                match arma::solve(&m, &b) {
                    Ok(x) => {
                        out.extend(x.as_slice());
                        out.push(1.0);
                    }
                    Err(_) => {
                        out.extend(std::iter::repeat_n(0.0, n * nrhs));
                        out.push(0.0);
                    }
                }
            } else if op.starts_with("arma_det") {
                out.push(arma::det(&m));
            } else {
                panic!("unknown operation {op}");
            }
        }
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(&args[4]).expect("output"));
    for v in out {
        f.write_all(&v.to_le_bytes()).expect("write");
    }
}
