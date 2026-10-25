// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! Replays mcflirt's own `MISCMATHS::decompose_aff` calls through the replica's
//! `larmorx_nc_core::miscmaths::decompose_aff`, input by input, and reports every parameter
//! whose bits differ.
//!
//! The calls are the `DEC` lines that the LD_PRELOAD interposer of
//! `validation/probes/fsl-mcflirt-affine-trace` prints while FSL's mcflirt runs: the affine (16
//! doubles, column-major), the centre (3) and FSL's 12 parameters, all as the bits of their
//! doubles. Each differing call is explained down to `rotmat2euler`'s `atan2f` arguments, with
//! the port's and the platform's `atan2f` side by side.
//!
//! Usage: `affine-trace-replay <trace.txt>`. Exits 0 when every call agrees.

use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::fdlibm::atan2f;
use larmorx_nc_core::miscmaths::{decompose_aff, dot_f, norm2};
use std::io::BufRead;

fn hex(s: &str) -> Vec<f64> {
    s.split(',')
        .map(|h| f64::from_bits(u64::from_str_radix(h, 16).expect("hex double")))
        .collect()
}

/// The rotation matrix `decompose_aff` hands to `rotmat2euler`, and the `atan2f` arguments of
/// its y rotation (the same steps as the port).
fn explain(affmat: &Mat) {
    let aff3 = affmat.sub(0, 3, 0, 3);
    let x = affmat.sub(0, 3, 0, 1);
    let y = affmat.sub(0, 3, 1, 2);
    let z = affmat.sub(0, 3, 2, 3);
    let sx = norm2(&x) as f32;
    let dxy = dot_f(&x, &y);
    let sy = (dot_f(&y, &y) - ((dxy * dxy) / (sx * sx))).sqrt();
    let a = dxy / (sx * sy);
    let x0 = x.div_scalar(f64::from(sx));
    let y0 = y.div_scalar(f64::from(sy)).minus(&x0.scale(f64::from(a)));
    let d0 = dot_f(&x0, &z);
    let d1 = dot_f(&y0, &z);
    let sz = (dot_f(&z, &z) - d0 * d0 - d1 * d1).sqrt();
    let b = d0 / sz;
    let c = d1 / sz;
    let mut scales = Mat::zeros(3, 3);
    scales.set(0, 0, f64::from(sx));
    scales.set(1, 1, f64::from(sy));
    scales.set(2, 2, f64::from(sz));
    let skew = Mat::from_rows(
        3,
        3,
        &[
            1.0,
            f64::from(a),
            f64::from(b),
            0.0,
            1.0,
            f64::from(c),
            0.0,
            0.0,
            1.0,
        ],
    );
    let Ok(skew_inv) = arma::inv(&skew) else {
        return;
    };
    let Ok(solved) = arma::solve(&scales, &skew_inv) else {
        return;
    };
    let rot = arma::mul_tinysq(&aff3, &solved);
    let r = |i: usize, j: usize| rot.at(i - 1, j - 1);
    let cy = (r(1, 1) * r(1, 1) + r(1, 2) * r(1, 2)).sqrt() as f32;
    let sy_ = (-r(1, 3)) as f32;
    let port = atan2f(sy_, cy);
    let platform = sy_.atan2(cy);
    println!(
        "   atan2f(sy, cy) = atan2f({:#010x}, {:#010x}): port {:#010x}, this platform's C library \
         {:#010x}",
        sy_.to_bits(),
        cy.to_bits(),
        port.to_bits(),
        platform.to_bits()
    );
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: affine-trace-replay <trace.txt>");
    let f = std::io::BufReader::new(std::fs::File::open(&path).expect("trace file"));
    let (mut n, mut bad) = (0usize, 0usize);
    for line in f.lines() {
        let line = line.expect("trace line");
        let Some(pos) = line.find("DEC ") else {
            continue;
        };
        let p: Vec<&str> = line[pos..].split_whitespace().collect();
        if p.len() < 5 {
            continue;
        }
        let (a, c, want) = (hex(p[1]), hex(p[2]), hex(p[4]));
        let mut rows = [0.0f64; 16];
        for r in 0..4 {
            for col in 0..4 {
                rows[r * 4 + col] = a[col * 4 + r];
            }
        }
        let m = Mat::from_rows(4, 4, &rows);
        let got = decompose_aff(&m, &Mat::col(&c));
        let diff: Vec<usize> = (0..12)
            .filter(|&i| got[i].to_bits() != want[i].to_bits())
            .collect();
        if !diff.is_empty() {
            bad += 1;
            if bad <= 20 {
                println!("call {n} (0-based): parameters {diff:?} differ");
                for &i in &diff {
                    println!(
                        "   p{}: FSL {:.17e}, replica {:.17e}",
                        i + 1,
                        want[i],
                        got[i]
                    );
                }
                explain(&m);
            }
        }
        n += 1;
    }
    println!("{n} decompose_aff calls, {bad} differ");
    std::process::exit(i32::from(bad > 0));
}
