// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of `validation/probes/fs-robust/bench.cpp`: the same `n^3` float volume, the
//! same operations, their seconds and the FNV-1a of their outputs' voxels.
//!
//!     fs_robust_bench <op> <n>    op: partials | subsample | valscale | blur | centroid | sqrtm |
//!                                 rstep | rstepa | all (sqrtm: n square roots of 4 x 4
//!                                 transforms; rstep, rstepa: one robust registration step,
//!                                 rigid, then affine with the intensity scale)

use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::vnl_matrix::VnlMatrix;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_robust::regstep::{Cost, RegistrationStep, StepParams};
use larmorx_freesurfer_robust::transformation::{Kind, Transformation};
use larmorx_freesurfer_robust::{costs, mymatrix, mymri};
use larmorx_freesurfer_volume::arith::Pair;

fn fnv(m: &Mri, mut h: u64) -> u64 {
    for &b in m.chunk.as_ref().expect("voxels") {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

const BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// The pattern: a `side^3` float volume (`n` its size parameter), moved by `sx` voxels along x.
fn pattern(g: &mut Globals, n: i32, side: i32, sx: i32) -> Mri {
    let (mut o, mut e) = (std::io::sink(), std::io::sink());
    let mut src = mri_alloc_sequence(g, &mut o, &mut e, side, side, side, MRI_FLOAT, 1).unwrap();
    for z in 0..side {
        for y in 0..side {
            for x0 in 0..side {
                let x = x0 - sx;
                let r = (x - n / 2) * (x - n / 2)
                    + (y - n / 2) * (y - n / 2)
                    + (z - n / 3) * (z - n / 3);
                let v = if r < n * n / 6 {
                    ((x * 7 + y * 13 + z * 29) % 97 + 100) as f32
                } else {
                    0.0
                };
                mri_set_vox_val(g, &mut e, &mut src, x0, y, z, 0, v, 0).unwrap();
            }
        }
    }
    src.vg.xsize = 1.0;
    src.vg.ysize = 1.0;
    src.vg.zsize = 1.0;
    reinit_cache(g, &mut o, &mut e, &mut src.vg).unwrap();
    src
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() == 3, "usage: fs_robust_bench <op> <n>");
    let op = args[1].as_str();
    let n: i32 = args[2].parse().expect("n");
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    // sqrtm: n is a count, no volume
    let side = if op == "sqrtm" { 1 } else { n };
    let mut src = pattern(&mut g, n, side, 0);
    let mut run = |name: &str, f: &mut dyn FnMut(&mut Globals, &mut Mri) -> u64| {
        if op != "all" && op != name {
            return;
        }
        let t0 = Instant::now();
        let h = f(&mut g, &mut src);
        println!("{name} {:.3} {h:016x}", t0.elapsed().as_secs_f64());
    };
    let (mut o2, mut e2) = (std::io::sink(), std::io::sink());
    run("partials", &mut |g, src| {
        let p = mymri::get_partials(g, &mut o2, &mut e2, src).unwrap();
        let h = fnv(&p.fx, BASIS);
        let h = fnv(&p.fy, h);
        let h = fnv(p.fz.as_ref().unwrap(), h);
        fnv(&p.blur, h)
    });
    run("subsample", &mut |g, src| {
        let s = mymri::sub_sample(g, &mut o2, &mut e2, src, None, true, 0)
            .unwrap()
            .unwrap_new();
        fnv(&s, BASIS)
    });
    run("valscale", &mut |g, src| {
        let s = mymri::mri_valscale(g, &mut o2, &mut e2, Pair::New(src), 1.1, 3.0)
            .unwrap()
            .unwrap_new();
        fnv(&s, BASIS)
    });
    run("blur", &mut |g, src| {
        let s = mymri::get_blur(g, &mut o2, &mut e2, Pair::New(src))
            .unwrap()
            .unwrap_new();
        fnv(&s, BASIS)
    });
    run("centroid", &mut |g, src| {
        let c = costs::centroid(g, &mut e2, src).unwrap();
        c.iter()
            .fold(0u64, |h, v| h.wrapping_mul(31).wrapping_add(v.to_bits()))
    });
    run("sqrtm", &mut |g, _| {
        let mut h = 0u64;
        for k in 0..n {
            let mut m = VnlMatrix::zeros(4, 4);
            for (i, j, v) in [
                (0, 0, 0.6),
                (0, 1, -0.8),
                (1, 0, 0.8),
                (1, 1, 0.6),
                (2, 2, 1.02),
            ] {
                *m.at_mut(i, j) = v;
            }
            *m.at_mut(3, 3) = 1.0;
            *m.at_mut(0, 3) = f64::from(k) * 0.001;
            *m.at_mut(1, 3) = -f64::from(k) * 0.002;
            *m.at_mut(2, 3) = 0.5;
            let r = mymatrix::matrix_sqrt(g, &mut o2, &mut e2, &m).unwrap();
            for v in &r.data {
                h = h.wrapping_mul(31).wrapping_add(v.to_bits());
            }
        }
        h
    });
    let mut step = |g: &mut Globals, src: &mut Mri, kind: Kind, iscale: bool| {
        let mut t = pattern(g, n, n, 1);
        let params = StepParams {
            sat: -1.0,
            iscale,
            transonly: false,
            rigid: kind == Kind::Rigid3d,
            isoscale: false,
            costfun: Cost::Rob,
            subsamplesize: -1,
            debug: 0,
            verbose: 0,
            iscalefinal: 1.0,
        };
        let mut trans = Transformation::new(kind);
        let mut s = RegistrationStep::<f32>::new(params);
        s.set_float_svd(true);
        let (m, second) = s
            .compute_registration_step(g, &mut o2, &mut e2, Some(&mut trans), src, &mut t)
            .unwrap();
        let mut h = fnv(s.mri_weights.as_ref().unwrap(), BASIS);
        for row in m {
            for v in row {
                h = h.wrapping_mul(31).wrapping_add(v.to_bits());
            }
        }
        h.wrapping_mul(31).wrapping_add(second.to_bits())
    };
    run("rstep", &mut |g, src| step(g, src, Kind::Rigid3d, false));
    run("rstepa", &mut |g, src| step(g, src, Kind::Affine3d, true));
}
