// SPDX-License-Identifier: Apache-2.0
//! Partial volumes (`specs/fast.md` §4.8): the mixel types (the pure classes and the mixtures of
//! two classes), their likelihoods over 101 fractions, one sweep of iterated conditional modes
//! with the mixel MRF, and each voxel's fractions by least energy.
//!
//! For speed, everything that depends only on a type and a fraction (its mean, variance and the
//! Gaussian's normalisation) is computed once, not per voxel; the per-voxel work (likelihoods,
//! fractions) runs in parallel, the sweep in storage order.

use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::Relaxed;

use larmorx_core::math;
use rayon::prelude::*;

use super::grid::Grid;
use super::stats::{self, Gauss};

/// What the partial-volume estimation found, per mask voxel.
#[derive(Clone, Debug)]
pub struct Pve {
    /// Each class's fraction (`n` per voxel).
    pub frac: Vec<f32>,
    /// The class of largest fraction (1..n; 0 where none is above `-1e-10`).
    pub seg: Vec<u8>,
    /// The mixel type (0..n-1 the pure classes, then the mixtures).
    pub mixel: Vec<u8>,
}

/// The mixel types of `n` classes (§4.8.4): `(a, b)` (0-based classes) of each mixed type after
/// the `n` pure ones; for 3 classes (1,2), (1,3), (2,3), otherwise the neighbours in mean order.
pub fn mixtures(n: usize) -> Vec<(usize, usize)> {
    match n {
        0 | 1 => Vec::new(),
        3 => vec![(0, 1), (0, 2), (1, 2)],
        _ => (0..n - 1).map(|a| (a, a + 1)).collect(),
    }
}

/// The fractions of one class (§8.7): from 0 by `f32(δ + 0.01)` while `δ ≤ 1`.
pub fn fractions() -> Vec<f32> {
    let mut out = Vec::with_capacity(101);
    let mut d = 0f32;
    while d <= 1.0 {
        out.push(d);
        d = (f64::from(d) + 0.01) as f32;
    }
    out
}

/// One mixed type at one fraction: `δ`, `1 - δ`, its Gaussian and `log σ²` (single).
#[derive(Clone, Copy, Debug)]
struct Mix {
    delta: f32,
    rest: f32,
    gauss: Gauss,
    log_var: f32,
}

fn mixes(classes: &[Gauss], a: usize, b: usize, deltas: &[f32]) -> Vec<Mix> {
    let (ga, gb) = (classes[a], classes[b]);
    deltas
        .iter()
        .map(|&d| {
            let rest = 1.0 - d;
            let mean = d * ga.mean + rest * gb.mean;
            let var = d * d * ga.var + rest * rest * gb.var;
            Mix {
                delta: d,
                rest,
                gauss: Gauss::new(mean, var),
                log_var: math::logf(var),
            }
        })
        .collect()
}

/// The partial volumes of the mask voxels of `grid` whose (bias-corrected) intensities are `x`,
/// from the sorted posteriors `post` (`n` per voxel); `mixel_weight` is `-R`.
pub fn estimate(
    grid: &Grid,
    post: &[f32],
    x: &[f32],
    n: usize,
    mixel_weight: f32,
    len: usize,
) -> Pve {
    let nm = grid.len();
    let pairs = mixtures(n);
    let nt = n + pairs.len();
    if pairs.is_empty() {
        // one class: every voxel of type 0, its fraction 1
        return Pve {
            frac: vec![1.0; nm * n],
            seg: vec![if n == 0 { 0 } else { 1 }; nm],
            mixel: vec![0; nm],
        };
    }
    let classes: Vec<Gauss> = stats::class_stats(post, x, n, len)
        .into_iter()
        .map(|(m, v)| Gauss::new(m, v))
        .collect();
    let deltas = fractions();
    let tables: Vec<Vec<Mix>> = pairs
        .iter()
        .map(|&(a, b)| mixes(&classes, a, b, &deltas))
        .collect();
    // the types' likelihoods (§4.8.5)
    let mut like = vec![0f32; nm * nt];
    like.par_chunks_mut(nt)
        .zip(x.par_iter())
        .for_each(|(row, &xi)| {
            for (c, g) in classes.iter().enumerate() {
                row[c] = math::exp(-f64::from(g.neg_log(xi))) as f32;
            }
            for (t, table) in tables.iter().enumerate() {
                let mut s = 0f32;
                for m in table {
                    let term = math::exp(-f64::from(m.gauss.neg_log(xi))) * 0.01;
                    s = (f64::from(s) + term) as f32;
                }
                row[n + t] = s;
            }
        });
    let types = icm(grid, &like, n, &pairs, mixel_weight);
    // the fractions (§4.8.8)
    let mut frac = vec![0f32; nm * n];
    frac.par_chunks_mut(n)
        .zip(types.par_iter())
        .zip(x.par_iter())
        .for_each(|((f, &t), &xi)| {
            let t = t as usize;
            if t < n {
                f[t] = 1.0;
                return;
            }
            let (a, b) = pairs[t - n];
            let (mut best, mut low) = (None, 1e13f32);
            for m in &tables[t - n] {
                let d = xi - m.gauss.mean;
                let e = (d * d / m.gauss.var + m.log_var) / 2.0;
                if e < low {
                    low = e;
                    best = Some(m);
                }
            }
            let (da, db) = best.map_or((0.0, 1.0), |m| (m.delta, m.rest));
            f[a] = da;
            f[b] = db;
        });
    let seg = pveseg(&frac, n);
    Pve {
        frac,
        seg,
        mixel: types,
    }
}

/// The mixel types (§4.8.6, §4.8.7): each mask voxel's type of largest likelihood (`like`, `nt =
/// n + pairs.len()` per voxel; the first on a tie, from -1), then one sweep of iterated
/// conditional modes in storage order, in place, with the mixel MRF of weight `r_weight` (`-R`).
pub fn icm(
    grid: &Grid,
    like: &[f32],
    n: usize,
    pairs: &[(usize, usize)],
    r_weight: f32,
) -> Vec<u8> {
    let nm = grid.len();
    let nt = n + pairs.len();
    const NONE: u8 = u8::MAX;
    let mut types = vec![NONE; nm + 1];
    types[..nm]
        .par_iter_mut()
        .zip(like.par_chunks(nt))
        .for_each(|(t, row)| {
            *t = argmax(row.iter().copied(), -1.0) as u8;
        });
    // the mixel MRF's factors: +2 the same type, +1 a pure class and a mixture that holds it,
    // -1 otherwise, 0 a neighbour outside the mask
    let holds = |pure: usize, mixed: usize| {
        let (a, b) = pairs[mixed - n];
        pure == a || pure == b
    };
    let mut factor = vec![[0f32; 256]; nt];
    for (t, f) in factor.iter_mut().enumerate() {
        for u in 0..nt {
            f[u] = if t == u {
                2.0
            } else if (t < n && u >= n && holds(t, u)) || (u < n && t >= n && holds(u, t)) {
                1.0
            } else {
                -1.0
            };
        }
    }
    // the sweep in storage order, in place, on the pipeline of [`Grid::gauss_seidel`]
    let cells: Vec<AtomicU8> = types.into_iter().map(AtomicU8::new).collect();
    grid.gauss_seidel(|r| {
        let mut k = vec![0f32; nt];
        for i in r {
            let nb = grid.neighbours(i);
            k.fill(0.0);
            for (q, &j) in nb.iter().enumerate() {
                let w = grid.weights[q];
                let u = cells[j].load(Relaxed) as usize;
                for (t, kt) in k.iter_mut().enumerate() {
                    *kt += w * factor[t][u];
                }
            }
            let row = &like[i * nt..(i + 1) * nt];
            let (mut high, mut best) = (-1f32, cells[i].load(Relaxed));
            for (t, (&l, &kt)) in row.iter().zip(&k).enumerate() {
                let v = l * math::expf(r_weight * kt);
                if v > high {
                    high = v;
                    best = t as u8;
                }
            }
            cells[i].store(best, Relaxed);
        }
    });
    let mut types: Vec<u8> = cells.into_iter().map(AtomicU8::into_inner).collect();
    types.truncate(nm);
    types
}

/// `_pveseg` (§4.8.9): each voxel's class of largest partial volume (1..n, the first on a tie,
/// from `-1e-10`), 0 where none is above it.
pub fn pveseg(frac: &[f32], n: usize) -> Vec<u8> {
    if n == 0 {
        return Vec::new();
    }
    frac.par_chunks(n)
        .map(|row| {
            let k = argmax(row.iter().copied(), -1e-10);
            if row.iter().any(|&v| v > -1e-10) {
                (k + 1) as u8
            } else {
                0
            }
        })
        .collect()
}

/// The index of the first value strictly above every earlier one and `start` (0 when none).
#[inline]
pub fn argmax(values: impl Iterator<Item = f32>, start: f32) -> usize {
    let (mut best, mut high) = (0usize, start);
    for (k, v) in values.enumerate() {
        if v > high {
            high = v;
            best = k;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_fractions() {
        assert_eq!(mixtures(3), vec![(0, 1), (0, 2), (1, 2)]);
        assert_eq!(mixtures(2), vec![(0, 1)]);
        assert_eq!(mixtures(4), vec![(0, 1), (1, 2), (2, 3)]);
        let d = fractions();
        assert_eq!(d.len(), 101);
        assert_eq!(d[0], 0.0);
        assert!((d[100] - 1.0).abs() < 1e-5);
        assert_eq!(argmax([0.5f32, 0.7, 0.7].into_iter(), -1.0), 1);
        assert_eq!(argmax([f32::NAN, f32::NAN].into_iter(), -1.0), 0);
    }
}
