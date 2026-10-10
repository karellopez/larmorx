// SPDX-License-Identifier: Apache-2.0
//! DEV ONLY (not committed): which global B-entry perturbations reproduce the oracle's first
//! vertices (runs with -fudge -stages 1: every volume's first line search starts at identity).
use larmorx_mri::hmc::cost::{TestVolume, normcorr, voxel_map};
use larmorx_mri::hmc::grid::subsample;
use larmorx_mri::hmc::image::{read_reference, read_series};
use larmorx_mri::hmc::report::mat_text;
use larmorx_mri::hmc::rigid;
use larmorx_mri::hmc::search::line_search;
use std::path::Path;

fn oracle_fourths(path: &str) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap();
    let mats: Vec<String> = text
        .split("Cost::affmat = \n")
        .skip(1)
        .map(|b| b.lines().take(4).map(|l| format!("{l}\n")).collect())
        .collect();
    let ident = mat_text(&rigid::IDENTITY);
    let mut out = Vec::new();
    let mut k = usize::MAX;
    for m in &mats {
        if *m == ident {
            k = 0;
        } else {
            k = k.saturating_add(1);
        }
        if k == 3 {
            out.push(m.clone());
        }
    }
    out
}

fn bump(x: f32, d: i32) -> f32 {
    if d == 0 || x == 0.0 {
        x
    } else {
        f32::from_bits((x.to_bits() as i32 + d) as u32)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let series = read_series(Path::new(&args[1]), 4).unwrap();
    let (r, _, _) = read_reference(Path::new(&args[2]), 4).unwrap();
    let fourths = oracle_fourths(&args[3]);
    let grid = subsample(&r, 8.0);
    let u = 0.004f32;
    let nvol = series.volumes.len().min(fourths.len());
    // Precompute per volume: centre, test.
    let combos: Vec<[i32; 4]> = {
        let mut v = Vec::new();
        for a in -1..=1 {
            for b in -1..=1 {
                for c in -1..=1 {
                    for d in -1..=1 {
                        v.push([a, b, c, d]);
                    }
                }
            }
        }
        v
    };
    let which: usize = args.get(4).map_or(0, |s| s.parse().unwrap()); // 0: perturb a1 evals, 1: a2
    if which == 2 {
        per_volume(&series, &grid, &fourths, nvol);
        return;
    }
    let mut results = Vec::new();
    for combo in &combos {
        let mut matches = 0;
        for vol in 0..nvol {
            let v = &series.volumes[vol];
            let c = v.centre_of_mass();
            let tv = TestVolume::new(v, 1.0);
            let mut calls = 0;
            let mut fourth = None;
            let mut phi = |a: f32| -> f32 {
                calls += 1;
                if calls == 3 {
                    fourth = Some(a);
                    return 1.0;
                }
                let mut p = rigid::IDENTITY_PARAMS;
                p[0] = f64::from(a);
                let m = rigid::compose(&p, 6, c);
                let mut b = voxel_map(&m, v.voxel_size, grid.voxel_size);
                let target = if which == 0 { a > 0.0 } else { a < 0.0 };
                if target {
                    b[1][1] = bump(b[1][1], combo[0]);
                    b[1][2] = bump(b[1][2], combo[1]);
                    b[2][1] = bump(b[2][1], combo[2]);
                    b[2][2] = bump(b[2][2], combo[3]);
                }
                normcorr(&grid, &tv, &b)
            };
            // y0 at identity:
            let m0 = rigid::compose(&rigid::IDENTITY_PARAMS, 6, c);
            let y0 = normcorr(&grid, &tv, &voxel_map(&m0, v.voxel_size, grid.voxel_size));
            let _ = line_search(&mut phi, y0, u);
            if let Some(a) = fourth {
                let mut p = rigid::IDENTITY_PARAMS;
                p[0] = f64::from(a);
                let m = rigid::compose(&p, 6, c);
                if mat_text(&m) == fourths[vol] {
                    matches += 1;
                }
            }
        }
        results.push((matches, *combo));
        eprint!(".");
    }
    results.sort();
    for r in results.iter().rev().take(10) {
        println!("{r:?}");
    }
}

fn per_volume(
    series: &larmorx_mri::hmc::image::Series,
    grid: &larmorx_mri::hmc::Volume,
    fourths: &[String],
    nvol: usize,
) {
    use std::collections::BTreeMap;
    let mut fixes: BTreeMap<[i32; 4], usize> = BTreeMap::new();
    let (mut base_ok, mut fixable, mut unfixable) = (0, 0, 0);
    for vol in 0..nvol {
        let v = &series.volumes[vol];
        let c = v.centre_of_mass();
        let tv = TestVolume::new(v, 1.0);
        let m0 = rigid::compose(&rigid::IDENTITY_PARAMS, 6, c);
        let y0 = normcorr(grid, &tv, &voxel_map(&m0, v.voxel_size, grid.voxel_size));
        let try_combo = |combo: [i32; 4]| -> bool {
            let mut calls = 0;
            let mut fourth = None;
            let mut phi = |a: f32| -> f32 {
                calls += 1;
                if calls == 3 {
                    fourth = Some(a);
                    return 1.0;
                }
                let mut p = rigid::IDENTITY_PARAMS;
                p[0] = f64::from(a);
                let m = rigid::compose(&p, 6, c);
                let mut b = voxel_map(&m, v.voxel_size, grid.voxel_size);
                let (i, j) = if a > 0.0 { (0, 1) } else { (2, 3) };
                b[1][3] = bump(b[1][3], combo[i]);
                b[2][3] = bump(b[2][3], combo[j]);
                normcorr(grid, &tv, &b)
            };
            let _ = line_search(&mut phi, y0, 0.004);
            fourth.is_some_and(|a| {
                let mut p = rigid::IDENTITY_PARAMS;
                p[0] = f64::from(a);
                mat_text(&rigid::compose(&p, 6, c)) == fourths[vol]
            })
        };
        if try_combo([0; 4]) {
            base_ok += 1;
            continue;
        }
        let mut found = None;
        for d in 1..=2 {
            for a in -d..=d {
                for b in -d..=d {
                    for cc in -d..=d {
                        for e in -d..=d {
                            let combo = [a, b, cc, e];
                            if found.is_none()
                                && combo.iter().map(|x: &i32| x.abs()).max() == Some(d)
                                && try_combo(combo)
                            {
                                found = Some(combo);
                            }
                        }
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        match found {
            Some(c) => {
                fixable += 1;
                *fixes.entry(c).or_default() += 1;
            }
            None => unfixable += 1,
        }
    }
    println!("base ok {base_ok}, fixable {fixable}, unfixable {unfixable}");
    for (k, v) in fixes {
        println!("{k:?} {v}");
    }
}
