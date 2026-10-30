// SPDX-License-Identifier: Apache-2.0
//! What the smoothness weight's `tanh` changes (GLIBC-06, `specs/bet.md` §8.1): BET's surface
//! found with glibc 2.35's bits (`tanh_glibc`, FSL's) and with the correctly rounded `tanh`, on
//! each image given, and how the two differ: the passes (reruns) and their self-intersection
//! totals, the largest vertex distance, and the brain masks' differing voxels and Dice.
//!
//! `cargo run --release -p larmorx-fsl --example tanh_effect -- [--threads N] <image>...`
//! (the first volume of each; bet2's defaults otherwise). The measurement behind BET part 2b's
//! choice of how GLIBC-06's notice is found (rule D: measure first).

use larmorx_core::defects::UpstreamDefects;
use larmorx_fsl::bet::surface::Tanh;
use larmorx_fsl::bet::{self, Deformation};
use larmorx_mri::hmc::files::ImageFiles;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut threads = 0usize;
    if args.first().map(String::as_str) == Some("--threads") {
        threads = args[1].parse().expect("--threads N");
        args.drain(..2);
    }
    println!(
        "| image | passes (glibc) | passes (correctly rounded) | totals (glibc / correctly rounded) | largest vertex distance (mm) | mask voxels differing | Dice |"
    );
    println!("|---|---|---|---|---|---|---|");
    for path in &args {
        let files = ImageFiles::of(path).expect("an image file name");
        let (_, input) = bet::image::read(&files, threads).expect("readable");
        let vol = &input.vol;
        let run = |tanh: Tanh| {
            let opts = Deformation {
                tanh,
                ..Deformation::default()
            };
            let found = bet::find_surface(vol, None, None, &opts, threads);
            let mask = bet::pooled(threads, || {
                bet::brain_mask(
                    &found.mesh,
                    &found.points,
                    vol.shape,
                    vol.voxel,
                    UpstreamDefects::Fix,
                    &mut Vec::new(),
                )
            })
            .expect("a mask");
            (found, mask)
        };
        let (g, mg) = run(Tanh::Glibc);
        let (c, mc) = run(Tanh::CorrectlyRounded);
        let far = g
            .points
            .iter()
            .zip(&c.points)
            .map(|(a, b)| bet::mesh::dist(*a, *b))
            .fold(0.0f64, f64::max);
        let differ = mg.iter().zip(&mc).filter(|(a, b)| a != b).count();
        let (na, nb) = (
            mg.iter().filter(|&&x| x == 1).count(),
            mc.iter().filter(|&&x| x == 1).count(),
        );
        let both = mg
            .iter()
            .zip(&mc)
            .filter(|(a, b)| **a == 1 && **b == 1)
            .count();
        let dice = 2.0 * both as f64 / (na + nb).max(1) as f64;
        let totals = |f: &bet::Found| {
            f.passes
                .iter()
                .map(|p| p.total.map_or("exact".to_owned(), |t| format!("{t:.1}")))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let name = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
        println!(
            "| {name} | {} | {} | {} / {} | {far:.6} | {differ} of {} | {dice:.6} |",
            g.passes.len(),
            c.passes.len(),
            totals(&g),
            totals(&c),
            mg.len()
        );
    }
}
