// SPDX-License-Identifier: Apache-2.0
//! FLIRT's search over rotations (`specs/flirt.md` §8.1; Jenkinson and Smith 2001, "A global
//! optimisation method for robust affine registration of brain images").
//!
//! Every coarse grid point is optimised from the same reference parameters, every fine grid
//! point's cost and every threshold point's optimisation are independent: each runs in parallel
//! and the results are collected in grid order, so the search gives the same matrices at any
//! thread count.

use larmorx_mri::hmc::rigid::{self, Params};
use rayon::prelude::*;

use super::mat::{self, Mat4};
use super::optimise::{self, Settings, Subset};

/// The search's settings: the rotation ranges (radians, `[min, max]` per axis) and the coarse
/// and fine steps (radians, single precision).
#[derive(Clone, Copy, Debug)]
pub struct Ranges {
    pub ranges: [[f64; 2]; 3],
    pub coarse: f32,
    pub fine: f32,
}

/// The rotation samples of one axis for step `delta` (§8.1, step 1).
pub fn samples(range: [f64; 2], delta: f32) -> Vec<f32> {
    let (lo, hi) = (range[0], range[1]);
    let q = (hi - lo) / f64::from(delta);
    let m = if q.is_finite() {
        (q.abs() + 0.5).trunc().copysign(q) as i64 + 1
    } else {
        1
    };
    let m = m.max(1) as usize;
    let (lof, hif) = (lo as f32, hi as f32);
    if m == 1 {
        return vec![(hif + lof) / 2.0];
    }
    (0..m)
        .map(|k| lof + k as f32 * (hif - lof) / (m as f32 - 1.0))
        .collect()
}

/// The search's result: the optimised matrices (`S`) and their starting points (`P`), each
/// with its cost, sorted by the optimised cost.
pub struct Found {
    pub results: Vec<(f32, Mat4)>,
    pub starts: Vec<(f32, Mat4)>,
}

/// The search (§8.1) with `dof` degrees of freedom. `eval` is the search cost of a trial
/// matrix; `centre` the parameters' centre; `ref_params` the reference parameters (the
/// centres of mass aligned); `min_voxel` the reference level's smallest voxel size.
#[allow(clippy::too_many_arguments)]
pub fn search(
    eval: &(dyn Fn(&Mat4) -> f32 + Sync),
    centre: [f64; 3],
    ref_params: Params,
    dof: usize,
    r: &Ranges,
    settings: &Settings,
    min_voxel: f32,
    n_threads_parallel: bool,
) -> Result<Found, String> {
    let coarse: Vec<Vec<f32>> = (0..3).map(|k| samples(r.ranges[k], r.coarse)).collect();
    let fine: Vec<Vec<f32>> = (0..3).map(|k| samples(r.ranges[k], r.fine)).collect();
    // the subset: a common scale and three translations (dof > 6), else the translations
    let mut cols: Vec<[f64; 12]> = Vec::new();
    if dof > 6 {
        let mut s = [0.0; 12];
        s[6] = 1.0;
        s[7] = 1.0;
        s[8] = 1.0;
        cols.push(s);
    }
    for k in 3..6 {
        let mut c = [0.0; 12];
        c[k] = 1.0;
        cols.push(c);
    }
    let subset = Subset::new(&cols).expect("finite mask");
    // the search's parameters are Euler angles whatever -anglerep says (§3)
    let euler = Settings {
        rep: super::param::AngleRep::Euler,
        ..settings.clone()
    };
    let settings = &euler;
    let at = |angles: [f32; 3], t: [f64; 3], sc: f64| -> Params {
        let mut p = ref_params;
        p[0] = f64::from(angles[0]);
        p[1] = f64::from(angles[1]);
        p[2] = f64::from(angles[2]);
        p[3..6].copy_from_slice(&t);
        p[6] = sc;
        p[7] = sc;
        p[8] = sc;
        p
    };
    let run = |p: &Params| -> Result<(Params, f32), String> {
        let m = rigid::compose(p, 12, centre);
        let (out, c) = optimise::optimise(eval, &m, centre, 12, 4, settings, Some(&subset))?;
        Ok((rigid::decompose(&out, centre), c))
    };
    let par_map = |n: usize, f: &(dyn Fn(usize) -> Result<(Params, f32), String> + Sync)| {
        if n_threads_parallel {
            (0..n).into_par_iter().map(f).collect::<Result<Vec<_>, _>>()
        } else {
            (0..n).map(f).collect::<Result<Vec<_>, _>>()
        }
    };
    // 4. the coarse grid
    let [cx, cy, cz] = [coarse[0].len(), coarse[1].len(), coarse[2].len()];
    let rp = ref_params;
    let coarse_out = par_map(cx * cy * cz, &|i| {
        let (a, b, c) = (i / (cy * cz), (i / cz) % cy, i % cz);
        run(&at(
            [coarse[0][a], coarse[1][b], coarse[2][c]],
            [rp[3], rp[4], rp[5]],
            rp[6],
        ))
    })?;
    let mut trans: Vec<[f64; 3]> = coarse_out.iter().map(|(p, _)| [p[3], p[4], p[5]]).collect();
    let mut scales: Vec<f64> = coarse_out.iter().map(|(p, _)| p[6]).collect();
    // 5. the scales replaced by their median
    let mut sorted = scales.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[(0.5 * sorted.len() as f64) as usize];
    scales.fill(median);
    let _ = &mut trans;
    // 6. the fine grid: translations and scale interpolated on the coarse grid
    let [fx, fy, fz] = [fine[0].len(), fine[1].len(), fine[2].len()];
    let f = [
        (cx as f32 - 1.0) / ((fx as f32 - 1.0).max(1.0)),
        (cy as f32 - 1.0) / ((fy as f32 - 1.0).max(1.0)),
        (cz as f32 - 1.0) / ((fz as f32 - 1.0).max(1.0)),
    ];
    let coarse_at = |a: i64, b: i64, c: i64, k: usize| -> f64 {
        if a < 0 || b < 0 || c < 0 || a >= cx as i64 || b >= cy as i64 || c >= cz as i64 {
            return 0.0;
        }
        let i = a as usize * cy * cz + b as usize * cz + c as usize;
        if k < 3 { trans[i][k] } else { scales[i] }
    };
    let interp = |pos: [f32; 3], k: usize| -> f64 {
        let fl = pos.map(|v| v.floor());
        let (ia, ib, ic) = (fl[0] as i64, fl[1] as i64, fl[2] as i64);
        let (da, db, dc) = (
            f64::from(pos[0] - fl[0]),
            f64::from(pos[1] - fl[1]),
            f64::from(pos[2] - fl[2]),
        );
        let mut s = 0.0;
        for (oa, wa) in [(0, 1.0 - da), (1, da)] {
            for (ob, wb) in [(0, 1.0 - db), (1, db)] {
                for (oc, wc) in [(0, 1.0 - dc), (1, dc)] {
                    let w = wa * wb * wc;
                    if w != 0.0 {
                        s += w * coarse_at(ia + oa, ib + ob, ic + oc, k);
                    }
                }
            }
        }
        s
    };
    let nfine = fx * fy * fz;
    let fine_params: Vec<Params> = (0..nfine)
        .map(|i| {
            let (a, b, c) = (i / (fy * fz), (i / fz) % fy, i % fz);
            let pos = [a as f32 * f[0], b as f32 * f[1], c as f32 * f[2]];
            let t = [interp(pos, 0), interp(pos, 1), interp(pos, 2)];
            let mut sc = interp(pos, 3);
            if !(0.5..=2.0).contains(&sc) || dof <= 6 {
                sc = 1.0;
            }
            let mut p = at([fine[0][a], fine[1][b], fine[2][c]], t, sc);
            p[9] = 0.0;
            p[10] = 0.0;
            p[11] = 0.0;
            p
        })
        .collect();
    let mut costs: Vec<f32> = if n_threads_parallel {
        fine_params
            .par_iter()
            .map(|p| eval(&rigid::compose(p, 12, centre)))
            .collect()
    } else {
        fine_params
            .iter()
            .map(|p| eval(&rigid::compose(p, 12, centre)))
            .collect()
    };
    // 7. the threshold, and the points below it optimised (their costs replace the grid's)
    let (cmin, cmax) = costs
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &c| {
            (l.min(c), h.max(c))
        });
    let mut sc: Vec<f32> = costs.clone();
    sc.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = sc[((0.2 * sc.len() as f64) as usize).min(sc.len() - 1)];
    let mut t = (cmin + 0.2 * (cmax - cmin)).min(pct);
    if t <= cmin {
        t = (1.0001 * cmin).max(0.9999 * cmin);
    }
    let below: Vec<usize> = (0..nfine).filter(|&i| costs[i] < t).collect();
    let fp = &fine_params;
    let opt = par_map(below.len(), &|j| run(&fp[below[j]]))?;
    for (j, &i) in below.iter().enumerate() {
        costs[i] = opt[j].1;
    }
    // 8. the minima (the last index on each axis excluded)
    let idx = |a: usize, b: usize, c: usize| a * fy * fz + b * fz + c;
    let mut minima = Vec::new();
    for a in 0..fx.saturating_sub(1) {
        for b in 0..fy.saturating_sub(1) {
            for c in 0..fz.saturating_sub(1) {
                let v = costs[idx(a, b, c)];
                let mut is_min = true;
                'n: for da in -1i64..=1 {
                    for db in -1i64..=1 {
                        for dc in -1i64..=1 {
                            if da == 0 && db == 0 && dc == 0 {
                                continue;
                            }
                            let (na, nb, nc) = (a as i64 + da, b as i64 + db, c as i64 + dc);
                            if na < 0
                                || nb < 0
                                || nc < 0
                                || na >= fx as i64
                                || nb >= fy as i64
                                || nc >= fz as i64
                            {
                                continue;
                            }
                            if costs[idx(na as usize, nb as usize, nc as usize)] < v {
                                is_min = false;
                                break 'n;
                            }
                        }
                    }
                }
                if is_min {
                    minima.push(idx(a, b, c));
                }
            }
        }
    }
    if minima.is_empty() {
        let mut best = 0usize;
        for c in 0..fz {
            for b in 0..fy {
                for a in 0..fx {
                    if costs[idx(a, b, c)] < costs[best] {
                        best = idx(a, b, c);
                    }
                }
            }
        }
        minima.push(best);
    }
    // 9. each minimum optimised freely; near-duplicates dropped
    let free = dof.min(7);
    let starts: Vec<Mat4> = minima
        .iter()
        .map(|&i| rigid::compose(&fine_params[i], free, centre))
        .collect();
    let run_free = |k: usize| -> Result<(Mat4, f32), String> {
        optimise::optimise(eval, &starts[k], centre, free, 4, settings, None)
    };
    let freed: Vec<(Mat4, f32)> = if n_threads_parallel {
        (0..starts.len())
            .into_par_iter()
            .map(run_free)
            .collect::<Result<_, _>>()?
    } else {
        (0..starts.len()).map(run_free).collect::<Result<_, _>>()?
    };
    let mut list: Vec<(f32, Mat4, Mat4)> = Vec::new();
    for (k, (m, c)) in freed.into_iter().enumerate() {
        let near = list
            .iter()
            .position(|(_, e, _)| mat::rms_deviation(&m, e, 80.0) < f64::from(min_voxel));
        match near {
            Some(j) => {
                if c < list[j].0 {
                    list[j] = (c, m, starts[k]);
                }
            }
            None => list.push((c, m, starts[k])),
        }
    }
    list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Found {
        results: list.iter().map(|(c, m, _)| (*c, *m)).collect(),
        starts: list.iter().map(|(c, _, s)| (*c, *s)).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_samples() {
        let r = [(-90.0f64).to_radians(), 90.0f64.to_radians()];
        let c = samples(r, 60.0f64.to_radians() as f32);
        assert_eq!(c.len(), 4);
        assert!((f64::from(c[1]) - (-30.0f64).to_radians()).abs() < 1e-6);
        assert_eq!(samples(r, 18.0f64.to_radians() as f32).len(), 11);
        assert_eq!(samples([0.0, 0.0], 0.3), vec![0.0]);
    }
}
