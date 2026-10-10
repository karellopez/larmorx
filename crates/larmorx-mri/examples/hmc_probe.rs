// SPDX-License-Identifier: Apache-2.0
//! DEV ONLY (not committed): probe the cost's sensitivity to B entries.
use larmorx_mri::hmc::cost::{TestVolume, normcorr, voxel_map};
use larmorx_mri::hmc::grid::subsample;
use larmorx_mri::hmc::image::{read_reference, read_series};
use larmorx_mri::hmc::rigid;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let series = read_series(Path::new(&args[1]), 4).unwrap();
    let (r, _, _) = read_reference(Path::new(&args[2]), 4).unwrap();
    let a: f64 = args[3].parse().unwrap();
    let grid = subsample(&r, 8.0);
    for vol in 0..args[4].parse::<usize>().unwrap() {
        let v = &series.volumes[vol];
        let c = v.centre_of_mass();
        let tv = TestVolume::new(v, 1.0);
        let mut p = rigid::IDENTITY_PARAMS;
        p[0] = a;
        let m = rigid::compose(&p, 6, c);
        let b = voxel_map(&m, v.voxel_size, grid.voxel_size);
        let base = normcorr(&grid, &tv, &b);
        let tv2 = TestVolume::new(v, 1.05);
        let alt = normcorr(&grid, &tv2, &b);
        print!("vol {vol} base {base:e} smooth1.05 {alt:e} | ");
        for i in 0..3 {
            for j in 0..4 {
                for d in [-1i32, 1] {
                    let mut bb = b;
                    let x = bb[i][j];
                    if x == 0.0 {
                        continue;
                    }
                    bb[i][j] = f32::from_bits((x.to_bits() as i32 + d) as u32);
                    let cst = normcorr(&grid, &tv, &bb);
                    let q = ((cst - base) as f64 / 2f64.powi(-24)).round();
                    print!("b{i}{j}{d:+}:{q} ");
                }
            }
        }
        println!();
    }
}
