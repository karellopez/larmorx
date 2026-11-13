// SPDX-License-Identifier: Apache-2.0
//! The main package's VNL ports on matrices given as text, one per line, every float as the 8
//! hexadecimal digits of its bits (row by row); one line out per line in (README.md):
//!
//! - `eig <n> <a…>`: `vnl_eigen::symmetric_eigensystem_f32_full`: `<never> <ierr> | <values> | <vectors>`
//! - `eigns <n> <a…>`: `vnl_eigen::real_eigensystem` (the matrix widened to double): `1` where
//!   `BalancNeverReturns`, else `0 <ierr> | <real parts> | <Vreal>` (rounded to float)
//! - `svd <rows> <cols> <a…>`: `vnl_svd_f32::vnl_svd_f32_full`: `<never> <info> | <w> | <U> | <V>`
//! - `qr <rows> <cols> <a…>`: `vnl_qr::Qr::<f32>`: `<Q> | <R>`
//! - `inv <n> <a…>`: 1 x 1 `1.0 / a` in double, 2 to 4 rows `vnl_fixed::inverse2..4` after
//!   `det2..4`, more `vnl_qr::Qr::<f32>`'s inverse after its determinant: `<det != 0> | <inverse>`
//! - `det <n> <a…>`: `vnl_fixed::determinant`
//!
//! Matrices are printed row by row.

use larmorx_core::{vnl_eigen, vnl_fixed, vnl_qr, vnl_svd_f32};
use std::io::{BufRead, Write};

fn h(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn hs(v: &[f32]) -> String {
    v.iter().map(|&x| h(x)).collect::<Vec<_>>().join(" ")
}

fn f(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).expect("a float's 8 hexadecimal digits"))
}

fn floats(t: &[&str], n: usize) -> Vec<f32> {
    t[..n].iter().map(|s| f(s)).collect()
}

fn line(t: &[&str]) -> String {
    let num = |i: usize| -> usize { t[i].parse().expect("a size") };
    match t[0] {
        "eig" => {
            let n = num(1);
            let e = vnl_eigen::symmetric_eigensystem_f32_full(&floats(&t[2..], n * n), n);
            format!(
                "{} {} | {} | {}",
                u8::from(e.never_returns()),
                e.ierr,
                hs(&e.values),
                hs(&e.vectors)
            )
        }
        "eigns" => {
            let n = num(1);
            let a: Vec<f64> = floats(&t[2..], n * n)
                .iter()
                .map(|&v| f64::from(v))
                .collect();
            match vnl_eigen::real_eigensystem(&a, n) {
                Err(vnl_eigen::BalancNeverReturns) => "1".to_string(),
                Ok(es) => {
                    let re: Vec<f32> = es.d_re.iter().map(|&v| v as f32).collect();
                    let vr: Vec<f32> = es.vreal.iter().map(|&v| v as f32).collect();
                    format!("0 {} | {} | {}", es.ierr, hs(&re), hs(&vr))
                }
            }
        }
        "svd" => {
            let (r, c) = (num(1), num(2));
            let s = vnl_svd_f32::vnl_svd_f32_full(&floats(&t[3..], r * c), r, c);
            let u: Vec<f32> = (0..r * c).map(|k| s.u[(k % c) * r + k / c]).collect();
            let v: Vec<f32> = (0..c * c).map(|k| s.v[(k % c) * c + k / c]).collect();
            format!(
                "{} {} | {} | {} | {}",
                u8::from(s.never_returns()),
                s.info,
                hs(&s.w),
                hs(&u),
                hs(&v)
            )
        }
        "qr" => {
            let (r, c) = (num(1), num(2));
            let q = vnl_qr::Qr::new(&floats(&t[3..], r * c), r, c);
            format!("{} | {}", hs(&q.q()), hs(&q.r()))
        }
        "inv" => {
            let n = num(1);
            let a = floats(&t[2..], n * n);
            let (ok, inv): (bool, Vec<f32>) = match n {
                1 => (a[0] != 0.0, vec![(1.0f64 / f64::from(a[0])) as f32]),
                2 => {
                    let m = [[a[0], a[1]], [a[2], a[3]]];
                    (vnl_fixed::det2(&m) != 0.0, vnl_fixed::inverse2(&m).concat())
                }
                3 => {
                    let m = [[a[0], a[1], a[2]], [a[3], a[4], a[5]], [a[6], a[7], a[8]]];
                    (vnl_fixed::det3(&m) != 0.0, vnl_fixed::inverse3(&m).concat())
                }
                4 => {
                    let mut m = [[0f32; 4]; 4];
                    for (k, &x) in a.iter().enumerate() {
                        m[k / 4][k % 4] = x;
                    }
                    (vnl_fixed::det4(&m) != 0.0, vnl_fixed::inverse4(&m).concat())
                }
                _ => {
                    let q = vnl_qr::Qr::new(&a, n, n);
                    (q.determinant() != 0.0, q.inverse().0)
                }
            };
            format!("{} | {}", u8::from(ok), hs(&inv))
        }
        "det" => {
            let n = num(1);
            h(vnl_fixed::determinant(&floats(&t[2..], n * n), n))
        }
        other => panic!("unknown command {other}"),
    }
}

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::BufWriter::new(std::io::stdout());
    for l in stdin.lock().lines() {
        let l = l.expect("a line");
        let t: Vec<&str> = l.split_whitespace().collect();
        let o = if t.is_empty() {
            String::new()
        } else {
            line(&t)
        };
        writeln!(out, "{o}").expect("standard output");
    }
}
