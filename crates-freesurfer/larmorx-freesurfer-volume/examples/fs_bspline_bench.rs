// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.bspline's speed and memory, the port's side of `validation/probes/fs-bspline-bench/`
//! (`bench.cpp` runs FreeSurfer's own code on the same volume):
//!
//!     fs_bspline_bench <w> <h> <d> <type: 0 uchar, 3 float> <transform: 0 none, 1 linear>
//!
//! One line per step: the step, its seconds, the peak resident memory after it (kB, the
//! process's `VmHWM` on Linux, 0 elsewhere) and the FNV-1a (64 bits) of the voxels it made.

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::bspline::{mri_linear_transform_bspline, mri_to_bspline};

fn peak_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn fnv(m: &Mri) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in m.chunk.as_deref().unwrap_or(&[]) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn main() {
    let a: Vec<i32> = std::env::args()
        .skip(1)
        .map(|s| s.parse().expect("an integer"))
        .collect();
    let [w, h, d, ty, xf] = a[..] else {
        eprintln!("usage: fs_bspline_bench <w> <h> <d> <type> <transform>");
        std::process::exit(2);
    };
    let mode = match std::env::var("LARMORX_UPSTREAM_DEFECTS").as_deref() {
        Ok("reproduce") => UpstreamDefects::Reproduce,
        _ => UpstreamDefects::Fix,
    };
    let mut g = Globals::new(mode);
    let (mut o, mut e) = (std::io::stdout(), std::io::stderr());
    let mut src = mri_alloc_sequence(&mut g, &mut o, &mut e, w, h, d, ty, 1).expect("a volume");
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let (dx, dy, dz) = (x - w / 2, y - h / 2, z - d / 2);
                let (r2, rr) = (dx * dx + dy * dy + dz * dz, w / 3);
                let hsh = (x as u32).wrapping_mul(73856093)
                    ^ (y as u32).wrapping_mul(19349663)
                    ^ (z as u32).wrapping_mul(83492791);
                let v = if r2 < rr * rr {
                    80 + ((hsh >> 7) % 40) as i32 + if r2 < rr * rr / 4 { 60 } else { 0 }
                } else {
                    ((hsh >> 9) % 8) as i32
                };
                mri_set_vox_val(&mut g, &mut e, &mut src, x, y, z, 0, v as f32, 0).expect("set");
            }
        }
    }
    println!("fnv-src 0 {} {:016x}", peak_kb(), fnv(&src));
    let t0 = Instant::now();
    // MRItoBSpline's own line goes to standard output, as FreeSurfer's does
    let b = mri_to_bspline(&mut g, &mut o, &mut e, &src, None, 3).expect("coefficients");
    let _ = o.flush();
    println!(
        "coeff {:.3} {} {:016x}",
        t0.elapsed().as_secs_f64(),
        peak_kb(),
        fnv(b.coeff.as_ref().expect("coefficients"))
    );
    if xf != 0 {
        let mut m = matrix_alloc(&mut g, &mut o, &mut e, 4, 4, MATRIX_REAL)
            .expect("a matrix")
            .expect("a matrix");
        for i in 1..=4 {
            m.set_relt(i, i, 1.0);
        }
        let (c, s, c2, s2) = (
            0.998_629_534_75f64,
            0.052_335_956_24f64,
            0.999_390_827_02f64,
            0.034_899_496_70f64,
        );
        let r = [[c, -s * c2, s * s2], [s, c * c2, -c * s2], [0.0, s2, c2]];
        for (i, row) in r.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                m.set_relt(i as i32 + 1, j as i32 + 1, *v as f32);
            }
        }
        let ctr = [f64::from(w) / 2.0, f64::from(h) / 2.0, f64::from(d) / 2.0];
        for (i, row) in r.iter().enumerate() {
            let t = ctr[i] - (row[0] * ctr[0] + row[1] * ctr[1] + row[2] * ctr[2]);
            m.set_relt(i as i32 + 1, 4, (t + 0.375 * (i + 1) as f64) as f32);
        }
        let t2 = Instant::now();
        let out = mri_linear_transform_bspline(&mut g, &mut o, &mut e, &b, None, Some(&m))
            .expect("the transform")
            .into_new()
            .expect("a new volume");
        println!(
            "transform {:.3} {} {:016x}",
            t2.elapsed().as_secs_f64(),
            peak_kb(),
            fnv(&out)
        );
    }
}
