// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The Powell registration's costs timed on one byte image pair (relay 49 of track C, the costs'
//! speed): `costs_bench <n> <evals>` makes two `n`^3 byte images of seeded noise (integer
//! arithmetic), and times `<evals>` evaluations of the least-squares, normalised-correlation and
//! byte-sampling paths with a small rotation, printing nanoseconds per voxel and a checksum.

use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc};
use larmorx_freesurfer_robust::costs::{least_squares, ncc};
use larmorx_freesurfer_volume::sample::{ByteSampler, sample_volume_frame};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: i32 = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(128);
    let evals: usize = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(5);
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut s = mri_alloc(&mut g, &mut out, &mut err, n, n, n, MRI_UCHAR).expect("s");
    let mut t = mri_alloc(&mut g, &mut out, &mut err, n, n, n, MRI_UCHAR).expect("t");
    let mut x: u64 = 0x1234_5678;
    for b in s.chunk.as_mut().expect("voxels").iter_mut() {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (x >> 56) as u8;
    }
    for b in t.chunk.as_mut().expect("voxels").iter_mut() {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (x >> 56) as u8;
    }
    // a small rotation about z and a shift (no whole coordinates)
    let (c, sn) = (0.999_f64, 0.044_710_177_812_216_3_f64);
    let msi = [
        [c, -sn, 0.0, 1.3],
        [sn, c, 0.0, -0.7],
        [0.0, 0.0, 1.0, 0.25],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mti = [
        [c, sn, 0.0, -1.1],
        [-sn, c, 0.0, 0.6],
        [0.0, 0.0, 1.0, -0.35],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let vox = (n as f64).powi(3);
    let t0 = Instant::now();
    let mut sum = 0.0;
    for _ in 0..evals {
        sum += least_squares(
            &mut g,
            &mut err,
            &mut s,
            &mut t,
            &msi,
            &mti,
            [1, 1, 1],
            1.0,
            1.0,
        );
    }
    let ls = t0.elapsed().as_secs_f64() / evals as f64;
    println!(
        "least squares {:.1} ns/voxel ({:.1} ms/eval) sum {sum}",
        ls * 1e9 / vox,
        ls * 1e3
    );
    let t0 = Instant::now();
    let mut sum = 0.0;
    for _ in 0..evals {
        sum += ncc(
            &mut g,
            &mut out,
            &mut err,
            &mut s,
            &mut t,
            &msi,
            &mti,
            [1, 1, 1],
        )
        .expect("ncc");
    }
    let nc = t0.elapsed().as_secs_f64() / evals as f64;
    println!(
        "ncc {:.1} ns/voxel ({:.1} ms/eval) sum {sum}",
        nc * 1e9 / vox,
        nc * 1e3
    );
    // the sampler alone along the grid
    let b = ByteSampler::new(&s).expect("a byte sampler");
    for (name, fast) in [("byte sampler", true), ("general sampler", false)] {
        let t0 = Instant::now();
        let mut acc = 0.0;
        for _ in 0..evals {
            for z in 0..n {
                for y in 0..n {
                    for xi in 0..n {
                        let (xf, yf, zf) = (f64::from(xi), f64::from(y), f64::from(z));
                        let px = c * xf - sn * yf + 1.3;
                        let py = sn * xf + c * yf - 0.7;
                        let pz = zf + 0.25;
                        let mut v = 0.0;
                        if fast {
                            b.sample(&mut g, &mut err, &s, px, py, pz, 0, &mut v);
                        } else {
                            sample_volume_frame(&mut g, &mut err, &s, px, py, pz, 0, &mut v);
                        }
                        acc += v;
                    }
                }
            }
        }
        let e = t0.elapsed().as_secs_f64() / evals as f64;
        println!("{name} {:.1} ns/sample sum {acc}", e * 1e9 / vox);
    }
}
