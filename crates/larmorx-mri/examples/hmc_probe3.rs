// SPDX-License-Identifier: Apache-2.0
//! DEV ONLY (not committed): dump one cost evaluation.
use larmorx_mri::hmc::cost::{TestVolume, normcorr, voxel_map};
use larmorx_mri::hmc::grid::subsample;
use larmorx_mri::hmc::image::{read_reference, read_series};
use larmorx_mri::hmc::rigid;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let series = read_series(Path::new(&args[1]), 1).unwrap();
    let (r, _, _) = read_reference(Path::new(&args[2]), 1).unwrap();
    let vol: usize = args[3].parse().unwrap();
    let a: f32 = args[4].parse().unwrap();
    let grid = subsample(&r, 8.0);
    let v = &series.volumes[vol];
    let c = v.centre_of_mass();
    let tv = TestVolume::new(v, 1.0);
    let mut p = rigid::IDENTITY_PARAMS;
    p[0] = f64::from(a);
    let m = rigid::compose(&p, 6, c);
    let b = voxel_map(&m, v.voxel_size, grid.voxel_size);
    println!("centre {:?}", c);
    println!("B {:?}", b);
    let cost = normcorr(&grid, &tv, &b);
    println!("cost {:e}", cost);
}
