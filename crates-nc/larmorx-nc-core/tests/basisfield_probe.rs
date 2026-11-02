// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! The replica's side of `validation/probes/fsl-basisfield/`: writes, into `replica.txt` of every
//! case directory under `BF_PROBE_ROOT`, the lines `basisfield_probe.cc` prints from FSL's own
//! library, from `larmorx_nc_core::basisfield`. `BF_PROBE_BLAS_THREADS` is OpenBLAS's thread
//! count for the energies' dot products (default 1). Run with `--ignored`.

use larmorx_nc_core::basisfield::{Csc, FieldIndex, SplineField};
use larmorx_nc_core::volume::{FslError, Volume};
use std::fmt::Write as _;
use std::path::Path;

fn pd(out: &mut String, tag: &str, v: f64) {
    writeln!(out, "{tag} {:016x}", v.to_bits()).unwrap();
}

fn pvec(out: &mut String, tag: &str, v: &[f64]) {
    writeln!(out, "{tag} n={}", v.len()).unwrap();
    for &x in v {
        pd(out, tag, x);
    }
}

fn pcsc(out: &mut String, tag: &str, m: &Csc) {
    writeln!(out, "{tag} m={} n={} nz={}", m.m, m.n, m.jcp[m.n as usize]).unwrap();
    for c in 0..m.n as usize {
        for e in m.jcp[c] as usize..m.jcp[c + 1] as usize {
            writeln!(out, "{tag} {} {} {:016x}", c, m.irp[e], m.val[e].to_bits()).unwrap();
        }
    }
}

fn run(dir: &Path, blas_threads: usize, out: &mut String) -> Result<(), FslError> {
    let params = std::fs::read_to_string(dir.join("params.txt")).unwrap();
    let p: Vec<&str> = params.split_whitespace().collect();
    let u = |i: usize| p[i].parse::<u32>().unwrap();
    let d = |i: usize| p[i].parse::<f64>().unwrap();
    let (nx, ny, nz) = (u(0), u(1), u(2));
    let (vx, vy, vz) = (d(3), d(4), d(5));
    let ksp = [u(6), u(7), u(8)];
    let order: i32 = p[9].parse().unwrap();
    let nthr: u32 = std::env::var("BF_PROBE_NTHR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut f = SplineField::new(&[nx, ny, nz], &[vx, vy, vz], &ksp, order, nthr)?;
    let csz = f.coef_size();
    writeln!(out, "csz {} {} {}", csz[0], csz[1], csz[2]).unwrap();
    let cb = std::fs::read(dir.join("coef.bin")).unwrap();
    let coef: Vec<f64> = cb
        .as_chunks::<8>()
        .0
        .iter()
        .take(f.coef_sz() as usize)
        .map(|c| f64::from_le_bytes(*c))
        .collect();
    f.set_coef(&coef)?;
    if std::env::var("BF_PROBE_ENERGY").is_ok() {
        pd(out, "memen", f.mem_energy(blas_threads)?);
        pd(out, "bendeng", f.bend_energy(blas_threads)?);
        pvec(out, "memen_grad", &f.mem_energy_grad()?);
        pvec(out, "bend_grad", &f.bend_energy_grad()?);
        return Ok(());
    }
    let n = (nx * ny * nz) as usize;
    let rf = |name: &str| -> Vec<f32> {
        std::fs::read(dir.join(name))
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .take(n)
            .map(|c| f32::from_le_bytes(*c))
            .collect()
    };
    let mk = |data: Vec<f32>| {
        let mut v: Volume<f32> = Volume::new(i64::from(nx), i64::from(ny), i64::from(nz), 1);
        v.setdims(vx as f32, vy as f32, vz as f32);
        v.data = data;
        v
    };
    let ima1 = mk(rf("ima1.bin"));
    let ima2 = mk(rf("ima2.bin"));
    let ones = mk(vec![1.0; n]);
    let mut mask: Volume<i8> = Volume::new(i64::from(nx), i64::from(ny), i64::from(nz), 1);
    mask.setdims(vx as f32, vy as f32, vz as f32);
    mask.data = std::fs::read(dir.join("mask.bin"))
        .unwrap()
        .iter()
        .take(n)
        .map(|&b| b as i8)
        .collect();
    for fi in 0..4 {
        let fi_ = FieldIndex::from_index(fi);
        let g = f.get(fi_)?.to_vec();
        pvec(out, &format!("field{fi}"), &g);
        let mut v: Volume<f32> = mk(vec![0.0; n]);
        f.as_volume(&mut v, fi_)?;
        for x in &v.data {
            writeln!(out, "asvol{fi} {:08x}", x.to_bits()).unwrap();
        }
    }
    let pts = std::fs::read_to_string(dir.join("points.txt")).unwrap();
    for l in pts.lines().filter(|l| !l.trim().is_empty()) {
        let c: Vec<f64> = l.split_whitespace().map(|s| s.parse().unwrap()).collect();
        for fi in 0..4 {
            pd(
                out,
                "peek",
                f.peek(c[0], c[1], c[2], FieldIndex::from_index(fi))?,
            );
        }
    }
    let wide = std::fs::read_to_string(dir.join("wide.txt")).unwrap();
    for l in wide.lines().filter(|l| !l.trim().is_empty()) {
        let c: Vec<i32> = l.split_whitespace().map(|s| s.parse().unwrap()).collect();
        for fi in 0..4 {
            pd(
                out,
                "wide",
                f.peek_wide(c[0], c[1], c[2], FieldIndex::from_index(fi))?,
            );
        }
    }
    let (d0, dx, dy, dz) = ([0u32, 0, 0], [1u32, 0, 0], [0u32, 1, 0], [0u32, 0, 1]);
    pvec(out, "jte_ima_mask", &f.jte_field(&ima1, Some(&mask))?);
    pvec(out, "jte_ima", &f.jte_field(&ima1, None)?);
    pvec(out, "jte_dx_ima_mask", &f.jte(&dx, &ima1, Some(&mask))?);
    pvec(
        out,
        "jte_prod_mask",
        &f.jte2_field(&ima1, &ima2, Some(&mask))?,
    );
    pvec(out, "jte_dy_prod", &f.jte2(&dy, &ima1, &ima2, None)?);
    pvec(
        out,
        "jte_dz_prod_mask",
        &f.jte2(&dz, &ima1, &ima2, Some(&mask))?,
    );
    pcsc(out, "jtj_sym_mask", &f.jtj(&d0, &ima1, &ima2, Some(&mask))?);
    pcsc(out, "jtj_sym_ones", &f.jtj_field(&ones, None)?);
    pcsc(out, "jtj_dx_sym", &f.jtj(&dx, &ima1, &ones, Some(&mask))?);
    pcsc(
        out,
        "jtj_dx_dy",
        &f.jtj_cross(&dx, &ima1, &dy, &ima2, Some(&mask))?,
    );
    pcsc(out, "jtj_0_dz", &f.jtj_cross(&d0, &ima1, &dz, &ones, None)?);
    pd(out, "memen", f.mem_energy(blas_threads)?);
    pd(out, "bendeng", f.bend_energy(blas_threads)?);
    pvec(out, "memen_grad", &f.mem_energy_grad()?);
    pvec(out, "bend_grad", &f.bend_energy_grad()?);
    pcsc(out, "memen_hess", &f.mem_energy_hess()?);
    pcsc(out, "bend_hess", &f.bend_energy_hess()?);
    f.scale_field(-1.5);
    let g = f.get(FieldIndex::Field)?.to_vec();
    pvec(out, "scaled_field", &g);
    Ok(())
}

#[test]
#[ignore = "a probe: run by validation/probes/fsl-basisfield/check.py"]
fn basisfield_probe() {
    let root = std::env::var("BF_PROBE_ROOT").expect("BF_PROBE_ROOT");
    let blas_threads = std::env::var("BF_PROBE_BLAS_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("params.txt").exists())
        .filter(|p| match std::env::var("BF_PROBE_ONLY") {
            Ok(only) => p.file_name().is_some_and(|n| n.to_string_lossy() == only),
            Err(_) => true,
        })
        .collect();
    dirs.sort();
    for dir in dirs {
        let mut out = String::new();
        if let Err(e) = run(&dir, blas_threads, &mut out) {
            writeln!(out, "exception {}", e.what).unwrap();
        }
        let name = std::env::var("BF_PROBE_OUTNAME").unwrap_or_else(|_| "replica.txt".into());
        std::fs::write(dir.join(name), out).unwrap();
    }
}
