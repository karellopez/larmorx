// SPDX-License-Identifier: Apache-2.0
//! FLIRT's search over rotations (`specs/flirt.md` §8.1; Jenkinson and Smith 2001, "A global
//! optimisation method for robust affine registration of brain images").
//!
//! Every coarse grid point is optimised from the same reference parameters, every fine grid
//! point's cost and every threshold point's optimisation are independent: each runs in parallel
//! and the results are collected in grid order, so the search gives the same matrices at any
//! thread count.

use larmorx_mri::hmc::interp::trilinear_zero;
use larmorx_mri::hmc::rigid::Params;
use larmorx_mri::hmc::volume::Volume;
use rayon::prelude::*;

use super::mat::{self, IDENTITY, Mat4};
use super::optimise::{self, Kind, Settings, Subset};
use super::trace::Trace;

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
    /// The grids `-debugsave` writes (§2.3): the coarse grid's shape, translations and scales
    /// (the median), the fine grid's shape and costs (the threshold's optimised costs in).
    pub coarse_shape: [usize; 3],
    pub translations: Vec<[f64; 3]>,
    pub scales: Vec<f64>,
    pub fine_shape: [usize; 3],
    pub costs: Vec<f32>,
    /// The reference parameters the search leaves in force (the last subset optimisation's
    /// `x₀`: the last threshold point's interpolated parameters).
    pub last_x0: Params,
    /// With `-anglerep quaternion`, a starting point had a rotation (FSL-90 applies).
    pub quaternion_starts: bool,
}

/// The rotation samplings' lines (`Coarse rotation samplings are:` and the fine ones), which
/// `measurecost`, `gridmeasurecost`, `optimise` and the search print at a level of 4 or more.
pub fn samplings_text(t: &Trace, r: &Ranges) -> String {
    let mut s = String::from("Coarse rotation samplings are:\n");
    for k in 0..3 {
        s.push_str(&t.nums(samples(r.ranges[k], r.coarse).into_iter().map(f64::from)));
        s.push('\n');
    }
    s.push_str(" and fine rotation samplings are:\n");
    for k in 0..3 {
        s.push_str(&t.nums(samples(r.ranges[k], r.fine).into_iter().map(f64::from)));
        s.push('\n');
    }
    s.push('\n');
    s
}

/// The search (§8.1): `dof` the `search` command's degrees of freedom, which choose the
/// grids' parameter subset and scale rule whatever `-dof` says (observed: with `-dof 6` FSL's
/// coarse grid optimises the common scale and its fine grid keeps the median scale), `free_dof`
/// those of the minima's free optimisations (`-dof` applies there). `eval` is the search cost of a trial
/// matrix; `centre` the parameters' centre; `ref_params` the reference parameters (the
/// centres of mass aligned); `tol_x0` the reference parameters in force when the search starts
/// (its subsets' tolerances, step 2); with `fsl90` (FSL-90 reproduced) and `-anglerep
/// quaternion` the starting points' Euler angles are composed as a quaternion's vector part;
/// `min_voxel` the reference level's smallest voxel size. Its verbose
/// lines go to `settings.trace` at `settings.level` (the run's level lowered by 2); where they
/// interleave with the optimisations' lines (a level of 4 or more) the grids run in order.
#[allow(clippy::too_many_arguments)]
pub fn search(
    eval_logged: &(dyn Fn(&Mat4, &super::cost::Log) -> f32 + Sync),
    centre: [f64; 3],
    ref_params: Params,
    dof: usize,
    free_dof: usize,
    r: &Ranges,
    settings: &Settings,
    tol_x0: Params,
    fsl90: bool,
    min_voxel: f32,
    n_threads_parallel: bool,
) -> Result<Found, String> {
    let user = settings;
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
        tol_x0: Some(tol_x0),
        ..settings.clone()
    };
    let settings = &euler;
    let at = |l: i32| settings.at(l);
    // the run's C library functions (GLIBC-07), through its log; the grids' evaluations write
    // to the run's log (their lines alike), the free optimisations each to their own (below)
    let quiet_grid = super::cost::Log::default();
    let run_log = settings.log.as_deref().unwrap_or(&quiet_grid);
    let libm = run_log.libm();
    let eval = &|m: &Mat4| eval_logged(m, run_log);
    // the lines of each grid point interleave with the evaluations': one point after the other
    let n_threads_parallel = n_threads_parallel && at(4).is_none();
    if let Some(t) = at(4) {
        t.write(&samplings_text(t, r));
    }
    let stars = |n: usize, per: usize| {
        if let Some(t) = at(2) {
            t.write(&"*".repeat(n / per.max(1)));
        }
    };
    let point = |angles: [f32; 3], t: [f64; 3], sc: f64| -> Params {
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
    // from the sampled parameters themselves (observed: FSL's evaluations start at the angles
    // it sampled, and the grid keeps the optimised translations and scale as they come); the
    // matrix their `Starting with :` shows is the identity's parameters composed about the
    // centre (the identity, all NaN about a NaN centre: observed, 2026-11-05)
    let shown = if at(5).is_some() {
        super::param::euler_compose(&larmorx_mri::hmc::rigid::IDENTITY_PARAMS, 12, centre, libm)
    } else {
        IDENTITY
    };
    let run = |p: &Params, kind: Kind| -> Result<(Params, f32), String> {
        optimise::optimise_params(
            eval,
            *p,
            centre,
            12,
            4,
            settings,
            Some(&subset),
            kind,
            &shown,
        )
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
        let r = run(
            &point(
                [coarse[0][a], coarse[1][b], coarse[2][c]],
                [rp[3], rp[4], rp[5]],
                rp[6],
            ),
            Kind::Coarse,
        );
        if !n_threads_parallel && c + 1 == cz {
            stars(1, 1);
        }
        r
    })?;
    if n_threads_parallel {
        stars(cx * cy * cz, cz);
    }
    if let Some(t) = at(2) {
        t.write("\n");
    }
    // the grid holds the results in single precision (step 4)
    let single = |v: f64| f64::from(v as f32);
    let mut trans: Vec<[f64; 3]> = coarse_out
        .iter()
        .map(|(p, _)| [single(p[3]), single(p[4]), single(p[5])])
        .collect();
    let mut scales: Vec<f64> = coarse_out.iter().map(|(p, _)| single(p[6])).collect();
    // 5. the scales replaced by their median
    let mut sorted = scales.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[(sorted.len() as f32 * 0.5f32) as usize];
    scales.fill(median);
    let _ = &mut trans;
    if let Some(t) = at(2) {
        t.line(&format!("Median scale = {}", t.num(median)));
    }
    // 6. the fine grid: translations and scale interpolated on the coarse grid
    let [fx, fy, fz] = [fine[0].len(), fine[1].len(), fine[2].len()];
    let f = [
        (cx as f32 - 1.0) / ((fx as f32 - 1.0).max(1.0)),
        (cy as f32 - 1.0) / ((fy as f32 - 1.0).max(1.0)),
        (cz as f32 - 1.0) / ((fz as f32 - 1.0).max(1.0)),
    ];
    // the coarse grid's four volumes (x the first angle), single precision, interpolated as
    // §6.2's trilinear rule with zeros outside the grid
    let grid = |k: usize| -> Volume {
        let mut data = vec![0.0f32; cx * cy * cz];
        for a in 0..cx {
            for b in 0..cy {
                for c in 0..cz {
                    let i = a * cy * cz + b * cz + c;
                    data[a + cx * (b + cy * c)] =
                        if k < 3 { trans[i][k] } else { scales[i] } as f32;
                }
            }
        }
        Volume::new([cx, cy, cz], [1.0; 3], data)
    };
    let grids: Vec<Volume> = (0..4).map(grid).collect();
    let interp = |pos: [f32; 3], k: usize| -> f64 {
        f64::from(trilinear_zero(&grids[k], pos[0], pos[1], pos[2]))
    };
    let nfine = fx * fy * fz;
    let fine_params: Vec<Params> = (0..nfine)
        .map(|i| {
            let (a, b, c) = (i / (fy * fz), (i / fz) % fy, i % fz);
            let pos = [a as f32 * f[0], b as f32 * f[1], c as f32 * f[2]];
            let t = [interp(pos, 0), interp(pos, 1), interp(pos, 2)];
            let mut sc = interp(pos, 3);
            // a NaN scale is not outside the range: kept (observed, 2026-11-05)
            if sc < 0.5 || sc > 2.0 || dof <= 6 {
                sc = 1.0;
            }
            let mut p = point([fine[0][a], fine[1][b], fine[2][c]], t, sc);
            p[9] = 0.0;
            p[10] = 0.0;
            p[11] = 0.0;
            p
        })
        .collect();
    let mut costs: Vec<f32> = if n_threads_parallel {
        let c: Vec<f32> = fine_params
            .par_iter()
            .map(|p| eval(&super::param::euler_compose(p, 12, centre, libm)))
            .collect();
        stars(nfine, fz);
        c
    } else {
        fine_params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c = eval(&super::param::euler_compose(p, 12, centre, libm));
                if let Some(t) = at(5) {
                    t.evaluation(c, p);
                }
                if (i + 1) % fz == 0 {
                    stars(1, 1);
                }
                c
            })
            .collect()
    };
    if let Some(t) = at(2) {
        t.write("\n");
    }
    // 7. the threshold, and the points below it optimised (their costs replace the grid's)
    let (cmin, cmax) = costs
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &c| {
            (l.min(c), h.max(c))
        });
    let mut sc: Vec<f32> = costs.clone();
    sc.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // the element at trunc(n·p), the product in single precision (step 7)
    let pct = sc[((sc.len() as f32 * 0.2f32) as usize).min(sc.len() - 1)];
    let mut t = (cmin + 0.2 * (cmax - cmin)).min(pct);
    if t <= cmin {
        t = (1.0001 * cmin).max(0.9999 * cmin);
    }
    if let Some(tr) = at(4) {
        tr.line(&format!(
            "Cost threshold = {} and minimum is {}",
            tr.num(f64::from(t)),
            tr.num(f64::from(cmin))
        ));
    }
    let below: Vec<usize> = (0..nfine).filter(|&i| costs[i] < t).collect();
    let fp = &fine_params;
    let at_index = |i: usize| (i / (fy * fz), (i / fz) % fy, i % fz);
    let reached = |tr: &Trace, i: usize, (x, c): &(Params, f32)| {
        let (a, b, cc) = at_index(i);
        tr.line(&format!(
            "({a},{b},{cc}) => {} with {}",
            tr.num(f64::from(*c)),
            tr.nums(*x)
        ));
    };
    let opt = par_map(below.len(), &|j| {
        let r = run(&fp[below[j]], Kind::Threshold)?;
        if !n_threads_parallel && let Some(tr) = at(3) {
            reached(tr, below[j], &r);
        }
        Ok(r)
    })?;
    if let Some(tr) = at(3) {
        if n_threads_parallel {
            for (j, &i) in below.iter().enumerate() {
                reached(tr, i, &opt[j]);
            }
        }
        tr.line("Costs (1st column) are:");
        for (x, c) in &opt {
            tr.line(&format!("{} {}", tr.num(f64::from(*c)), tr.nums(*x)));
        }
        tr.line("");
    }
    for (j, &i) in below.iter().enumerate() {
        costs[i] = opt[j].1;
    }
    // the reference parameters in force after the search: the last subset optimisation's x₀
    let last_x0 = match below.last() {
        Some(&i) => fine_params[i],
        None => point(
            [coarse[0][cx - 1], coarse[1][cy - 1], coarse[2][cz - 1]],
            [rp[3], rp[4], rp[5]],
            rp[6],
        ),
    };
    // 8. the minima (the last index on each axis excluded)
    let idx = |a: usize, b: usize, c: usize| a * fy * fz + b * fz + c;
    // scanned z outer, then y, then x (observed: FSL lists its minima in this order)
    let mut minima = Vec::new();
    for c in 0..fz.saturating_sub(1) {
        for b in 0..fy.saturating_sub(1) {
            for a in 0..fx.saturating_sub(1) {
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
    // no local minimum (a grid of one point per axis, or every point beside a smaller one):
    // the overall minimum, which FSL lists only on its `Cost minima` line (observed: `-nosearch
    // -verbose 7`, random testing's 20261103-07259)
    let fallback = minima.is_empty();
    if fallback {
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
    if let Some(tr) = at(3) {
        for word in ["COST", "Cost"] {
            if fallback && word == "COST" {
                continue;
            }
            for &i in &minima {
                let (a, b, c) = at_index(i);
                tr.line(&format!("{word} minima at : {a},{b},{c}"));
            }
        }
        tr.line("Chosen parameters:");
        for &i in &minima {
            tr.line(&tr.nums(fine_params[i]));
        }
        tr.line("");
    }
    // 9. each minimum optimised freely; near-duplicates dropped. The start is the matrix of
    // its first min(dof, 7) parameters (with FSL-90 reproduced and quaternions, its angles read
    // as a quaternion's vector part), the optimisation from that matrix's decomposition in
    // -anglerep's representation.
    let free = free_dof.min(7);
    let rep = user.rep;
    let quaternion = rep == super::param::AngleRep::Quaternion;
    let quaternion_starts = quaternion
        && minima
            .iter()
            .any(|&i| fine_params[i][..3].iter().any(|&a| a != 0.0));
    let log = run_log;
    // each start in turn, as FLIRT runs them (observed, 2026-11-05: with -dof 3 and -anglerep
    // quaternion the messages of a start's composition, its dof message and its optimisation's
    // evaluations follow one another start by start): the start composed (FSL-90's message for
    // a vector part outside the unit ball; this matrix, composed once, is the one `P` keeps,
    // §8.1 step 9), its decomposition the optimisation's start (its first evaluation and its
    // `Starting with` the decomposition composed again: under gimbal lock other angles), a
    // `-dof` below 6's message after the start's own messages (2026-11-02), then its
    // optimisation; each start's lines go to its own log, passed on in the starts' order
    // whatever runs in parallel
    let dof_message = super::optimise::clamp_dof(free_dof as i64).1;
    let free_settings = Settings {
        tol_x0: None,
        ..user.clone()
    };
    let free_settings = Settings {
        level: settings.level,
        ..free_settings
    };
    let freed_line = |tr: &Trace, c: f32, m: &Mat4| {
        tr.line(&format!(
            "{} ::: {}",
            tr.num(f64::from(c)),
            tr.nums(super::param::decompose(rep, m, centre, log.libm()))
        ));
    };
    type Freed = (Mat4, Mat4, f32, f32, std::sync::Arc<super::cost::Log>);
    let run_free = |k: usize| -> Result<Freed, String> {
        let own = std::sync::Arc::new(log.child());
        let i = minima[k];
        let m = if fsl90 && quaternion {
            // composed once (§8.1 step 9; spec writer, 2026-11-03: FLIRT's further messages
            // with -dof 6 are its evaluations' compositions outside the unit ball)
            super::param::compose(rep, &fine_params[i], free, centre, &own)
        } else {
            super::param::euler_compose(&fine_params[i], free, centre, own.libm())
        };
        let x = super::param::decompose(rep, &m, centre, own.libm());
        let start = super::param::compose(rep, &optimise::finite_start(&x), free, centre, &own);
        if let Some(msg) = &dof_message {
            own.push(msg.as_str());
        }
        let s_k = Settings {
            log: Some(std::sync::Arc::clone(&own)),
            ..free_settings.clone()
        };
        let eval_k = |m: &Mat4| eval_logged(m, &own);
        let (x, c) =
            optimise::optimise_params(&eval_k, x, centre, free, 4, &s_k, None, Kind::Free, &start)?;
        let opt = super::param::compose(rep, &x, free, centre, &own);
        // the start's own cost goes with it into `P`: the composed start's (composed once),
        // evaluated after the optimisation (counted, not printed)
        let c0 = eval_k(&m);
        if !n_threads_parallel && let Some(tr) = at(1) {
            freed_line(tr, c, &opt);
        }
        Ok((opt, m, c, c0, own))
    };
    if let Some(tr) = at(1) {
        tr.line("After free optimisation, parameters are:");
    }
    let freed: Vec<Freed> = if n_threads_parallel {
        let f: Vec<Freed> = (0..minima.len())
            .into_par_iter()
            .map(run_free)
            .collect::<Result<_, _>>()?;
        if let Some(tr) = at(1) {
            for (m, _, c, _, _) in &f {
                freed_line(tr, *c, m);
            }
        }
        f
    } else {
        (0..minima.len()).map(run_free).collect::<Result<_, _>>()?
    };
    let mut starts = Vec::with_capacity(freed.len());
    let freed: Vec<(Mat4, f32, f32)> = freed
        .into_iter()
        .map(|(m, start, c, c0, own)| {
            log.absorb(&own);
            starts.push(start);
            (m, c, c0)
        })
        .collect();
    // the list, sorted by the optimised cost: a new entry is compared with the entries in their
    // order and the first within the distance decides (a strictly better new entry replaces it,
    // otherwise the new one is dropped); a new entry is inserted after every entry of lower or
    // equal cost
    let mut list: Vec<(f32, Mat4, Mat4, f32)> = Vec::new();
    let insert = |list: &mut Vec<(f32, Mat4, Mat4, f32)>, e: (f32, Mat4, Mat4, f32)| {
        let at = list.iter().take_while(|x| x.0 <= e.0).count();
        list.insert(at, e);
    };
    for (k, (m, c, c0)) in freed.into_iter().enumerate() {
        let near = list
            .iter()
            .position(|(_, e, _, _)| mat::rms_deviation(&m, e, 80.0) < f64::from(min_voxel));
        match near {
            Some(j) => {
                if c < list[j].0 {
                    list.remove(j);
                    insert(&mut list, (c, m, starts[k], c0));
                }
            }
            None => insert(&mut list, (c, m, starts[k], c0)),
        }
    }
    if let Some(tr) = at(1) {
        let cols = |m: &Mat4| tr.nums((0..16).map(|k| m[k % 4][k / 4]));
        tr.line("Parameters (1st column costs) are:");
        for (c, m, s, c0) in &list {
            tr.line(&format!(
                "{} {}{} {}",
                tr.num(f64::from(*c)),
                cols(m),
                tr.num(f64::from(*c0)),
                cols(s)
            ));
        }
        tr.line("");
    }
    Ok(Found {
        results: list.iter().map(|(c, m, _, _)| (*c, *m)).collect(),
        starts: list.iter().map(|(_, _, s, c0)| (*c0, *s)).collect(),
        coarse_shape: [cx, cy, cz],
        translations: trans,
        scales,
        fine_shape: [fx, fy, fz],
        costs,
        last_x0,
        quaternion_starts,
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
