// SPDX-License-Identifier: Apache-2.0
//! The coarse searches of `-twopass` (`specs/3dvolreg.md` §8.6): the integer shift search and
//! the search over 45 rotations, each with its shift search.
//!
//! The sums are AFNI's: float32, sequential in storage order, one sum per candidate, so the
//! scores (and so the choice, with its strict "better than") are the spec's. Speed comes from
//! keeping that order while doing more at once: the base's terms are computed once per base,
//! eight shift candidates are summed in the same sweep (independent chains), and the 45
//! rotations run in parallel, the best chosen afterwards in the spec's order.

use rayon::prelude::*;

use crate::automask::{self, MaskSettings};

use super::blur::{self, BlurSettings};
use super::grid::M3;
use super::rotate::{Resampler, Transform, Trig, Work, rot3};
use super::shift::Method;

/// The coarse search's settings.
#[derive(Clone, Copy, Debug)]
pub struct CoarseSettings {
    /// `del` (after §8.6's adjustment) and `num`.
    pub del: i32,
    pub num: i32,
    /// The rotations searched (`-coarserot`, the default).
    pub rotations: bool,
    /// The clip fraction of the weight's automask (0.333 with `-maxdisp`, else 0.5; AFNI-92).
    pub clfrac: f32,
    pub zpad: usize,
    pub trig: Trig,
    /// The internal angle axes and their signs (§4.4).
    pub axes: [usize; 3],
    pub signs: [f32; 3],
    pub blur: BlurSettings,
    /// Candidates over an empty region are taken, as AFNI does (AFNI-93 reproduced).
    pub take_empty: bool,
}

/// What a search found: the rotation `(r, p, y)` (degrees, the reported convention), the shift
/// (voxels) and `-verbose`'s marks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub angles: [f32; 3],
    pub shift: [i32; 3],
    /// The text after the colon of `-verbose`'s line, before the numbers.
    pub marks: String,
    /// The search met a region without voxels (AFNI-93).
    pub empty_region: bool,
}

/// The candidates of the shift search, in AFNI's order (dz outer, dx inner).
fn candidates(del: i32, num: i32) -> Vec<[i32; 3]> {
    let d = del * num;
    let mut out = Vec::new();
    if del <= 0 {
        return out;
    }
    let steps: Vec<i32> = (-num..=num).map(|s| s * del).collect();
    for &dz in &steps {
        for &dy in &steps {
            for &dx in &steps {
                if dx * dx + dy * dy + dz * dz <= d * d {
                    out.push([dx, dy, dz]);
                }
            }
        }
    }
    out
}

/// The base's part of a shift search: the region's voxels (with the weight's positive voxels
/// only), their `W·b`, and the base's sums.
struct Region {
    idx: Vec<usize>,
    w: Vec<f32>,
    wb: Vec<f32>,
    sbb: f32,
    /// `Σ B²` over the whole volume.
    total: f32,
    empty: bool,
}

fn region(b: &[f32], w: Option<&[f32]>, n: [usize; 3], e: i64) -> Region {
    let [nx, ny, nz] = n;
    let range = |len: usize| (e.max(0) as usize)..((len as i64 - e).max(0) as usize);
    let (ri, rj, rk) = (range(nx), range(ny), range(nz));
    let empty = ri.is_empty() || rj.is_empty() || rk.is_empty();
    let mut idx = Vec::new();
    let mut ws = Vec::new();
    let mut wb = Vec::new();
    let mut sbb = 0f32;
    for k in rk.clone() {
        for j in rj.clone() {
            for i in ri.clone() {
                let p = i + nx * (j + ny * k);
                let bv = b[p];
                match w {
                    Some(w) => {
                        let wv = w[p];
                        if wv > 0.0 {
                            idx.push(p);
                            ws.push(wv);
                            let t = wv * bv;
                            wb.push(t);
                            sbb += t * bv;
                        }
                    }
                    None => {
                        idx.push(p);
                        ws.push(1.0);
                        wb.push(bv);
                        sbb += bv * bv;
                    }
                }
            }
        }
    }
    let mut total = 0f32;
    for &x in b {
        total += x * x;
    }
    Region {
        idx,
        w: ws,
        wb,
        sbb,
        total,
        empty,
    }
}

/// The scores of `cands` for the volume `v` over `reg` (eight candidates per sweep).
fn scores(v: &[f32], reg: &Region, cands: &[[i32; 3]], n: [usize; 3], weighted: bool) -> Vec<f32> {
    let [nx, ny, _] = n;
    let off: Vec<i64> = cands
        .iter()
        .map(|c| i64::from(c[0]) + nx as i64 * (i64::from(c[1]) + ny as i64 * i64::from(c[2])))
        .collect();
    let mut out = Vec::with_capacity(cands.len());
    for chunk in off.chunks(8) {
        let m = chunk.len();
        let mut svv = [0f32; 8];
        let mut sbv = [0f32; 8];
        for (t, &p) in reg.idx.iter().enumerate() {
            let (wv, wbv) = (reg.w[t], reg.wb[t]);
            for c in 0..m {
                let x = v[(p as i64 - chunk[c]) as usize];
                if weighted {
                    svv[c] += (wv * x) * x;
                } else {
                    svv[c] += x * x;
                }
                sbv[c] += wbv * x;
            }
        }
        for c in 0..m {
            out.push(if svv[c] > 0.0 {
                reg.sbb - sbv[c] * sbv[c] / svv[c]
            } else {
                reg.sbb
            });
        }
    }
    out
}

/// The best shift of `v` against the base of `reg`: the first candidate strictly better than
/// the best so far, from no shift with the score `Σ B²`.
fn best_shift(
    v: &[f32],
    reg: &Region,
    n: [usize; 3],
    del: i32,
    num: i32,
    weighted: bool,
    take_empty: bool,
) -> ([i32; 3], f32) {
    let mut best = ([0i32; 3], reg.total);
    if reg.empty && !take_empty {
        return best;
    }
    let cands = candidates(del, num);
    let s = if reg.empty {
        vec![0f32; cands.len()]
    } else {
        scores(v, reg, &cands, n, weighted)
    };
    for (c, sc) in cands.iter().zip(s) {
        if sc < best.1 {
            best = (*c, sc);
        }
    }
    best
}

/// The shift-only search (`-nocoarserot`): `v` against `b` without a weight.
pub fn shift_only(v: &[f32], b: &[f32], n: [usize; 3], s: &CoarseSettings) -> Found {
    let e = i64::from(s.del) * i64::from(s.num + 1);
    let reg = region(b, None, n, e);
    let (shift, _) = best_shift(v, &reg, n, s.del, s.num, false, s.take_empty);
    Found {
        angles: [0.0; 3],
        shift,
        marks: String::new(),
        empty_region: reg.empty,
    }
}

/// The volume subsampled by 2 (`B'[i,j,k] = B[2i,2j,2k]`) after the coarse blur.
fn subsample(v: &[f32], n: [usize; 3], s: &BlurSettings) -> (Vec<f32>, [usize; 3]) {
    let mut b = v.to_vec();
    blur::fir(&mut b, n, [1.0; 3], [1.0; 3], s.coarse_factor(), true, s);
    let m = n.map(|d| d.div_ceil(2));
    let mut out = Vec::with_capacity(m[0] * m[1] * m[2]);
    for k in 0..m[2] {
        for j in 0..m[1] {
            for i in 0..m[0] {
                out.push(b[2 * i + n[0] * (2 * j + n[1] * 2 * k)]);
            }
        }
    }
    (out, m)
}

/// The weight of the rotation search (§8.6 step 2) and the percentage of its voxels kept.
fn rotation_weight(b: &[f32], n: [usize; 3], s: &CoarseSettings) -> (Vec<f32>, f32) {
    let mut w: Vec<f32> = b.iter().map(|x| x.abs()).collect();
    blur::fir(
        &mut w,
        n,
        [1.0; 3],
        [1.0; 3],
        s.blur.coarse_factor(),
        true,
        &s.blur,
    );
    let max = w.iter().fold(f32::NEG_INFINITY, |m, &x| m.max(x));
    let r = 1.0f32 / max;
    for x in w.iter_mut() {
        *x *= r;
        if *x < 0.05 {
            *x = 0.0;
        }
    }
    let settings = MaskSettings {
        clfrac: s.clfrac,
        ..MaskSettings::default()
    };
    let mut log = Vec::new();
    match automask::form_mask(&w, n, &settings, &mut log) {
        Ok(mask) => {
            for (x, &m) in w.iter_mut().zip(&mask) {
                if m == 0 {
                    *x = 0.0;
                }
            }
        }
        Err(_) => w.iter_mut().for_each(|x| *x = 0.0),
    }
    let kept = w.iter().filter(|&&x| x > 0.0).count();
    let pct = 100.0f32 * kept as f32 / w.len() as f32;
    (w, pct)
}

/// The rotation-and-shift search (the default) of `v` against `b` on a grid of `n` voxels of
/// sizes `sizes`.
pub fn rotation_and_shift(
    v: &[f32],
    b: &[f32],
    n: [usize; 3],
    sizes: [f32; 3],
    s: &CoarseSettings,
) -> Found {
    let mut marks = String::new();
    let big = n.iter().all(|&d| d >= 120);
    let (vb, bb, m, del, sizes) = if big {
        marks.push('x');
        let (vs, m) = subsample(v, n, &s.blur);
        let (bs, _) = subsample(b, n, &s.blur);
        (vs, bs, m, s.del / 2, sizes.map(|d| 2.0 * d))
    } else {
        (v.to_vec(), b.to_vec(), n, s.del, sizes)
    };
    marks.push('w');
    let (w, pct) = rotation_weight(&bb, m, s);
    marks.push_str(&format!("[{}%]", crate::cfmt::f1(f64::from(pct))));
    let e = i64::from(del) * i64::from(s.num + 1);
    let reg = region(&bb, Some(&w), m, e);
    if reg.empty && !s.take_empty {
        marks.push_str(&".".repeat(45));
        return Found {
            angles: [0.0; 3],
            shift: [0; 3],
            marks,
            empty_region: true,
        };
    }
    let mut rotations = Vec::with_capacity(45);
    for a in -1i32..=1 {
        for bi in -2i32..=2 {
            for c in -1i32..=1 {
                rotations.push([9 * a, 9 * bi, 9 * c]);
            }
        }
    }
    let resampler = Resampler {
        method: Method::NN,
        zpad: s.zpad,
        trig: s.trig,
    };
    let deg = std::f64::consts::PI / 180.0;
    let results: Vec<([i32; 3], f32)> = rotations
        .par_iter()
        .map_init(Work::default, |work, rot| {
            if *rot == [0, 0, 0] {
                return best_shift(&vb, &reg, m, del, s.num, true, s.take_empty);
            }
            let theta = [0, 1, 2].map(|q| -deg * f64::from(s.signs[q]) * f64::from(rot[q]));
            let mat: M3 = rot3(s.axes, theta, s.trig);
            let mut out = vec![0f32; vb.len()];
            let tf = Transform::Matrix {
                m: mat,
                shift: [0.0; 3],
            };
            resampler.apply(&vb, m, sizes, &tf, &mut out, work);
            best_shift(&out, &reg, m, del, s.num, true, s.take_empty)
        })
        .collect();
    let mut best = 1.0e38f32;
    let mut found = (0usize, [0i32; 3]);
    for (q, &(shift, score)) in results.iter().enumerate() {
        if score < best {
            best = score;
            found = (q, shift);
            marks.push('*');
        } else {
            marks.push('.');
        }
    }
    let mut shift = found.1;
    if big {
        shift = shift.map(|x| 2 * x);
    }
    Found {
        angles: rotations[found.0].map(|x| x as f32),
        shift,
        marks,
        empty_region: reg.empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_in_order() {
        let c = candidates(2, 2);
        assert_eq!(c[0], [0, 0, -4]);
        assert_eq!(c.len(), 33);
        assert!(c.contains(&[0, 0, 0]));
        assert!(candidates(0, 2).is_empty());
    }

    #[test]
    fn a_shifted_volume_is_found() {
        let n = [20, 20, 20];
        let mut b = vec![0f32; 8000];
        let mut v = vec![0f32; 8000];
        for k in 0..20 {
            for j in 0..20 {
                for i in 0..20 {
                    let r = |a: f32, c: f32| (a - c) * (a - c);
                    let f = |x: f32, y: f32, z: f32| {
                        100.0 / (1.0 + 0.1 * (r(x, 10.0) + r(y, 9.0) + r(z, 10.0)))
                    };
                    b[i + 20 * (j + 20 * k)] = f(i as f32, j as f32, k as f32);
                    v[i + 20 * (j + 20 * k)] = f(i as f32 + 2.0, j as f32, k as f32);
                }
            }
        }
        let reg = region(&b, None, n, 4);
        let (shift, _) = best_shift(&v, &reg, n, 2, 1, false, false);
        assert_eq!(shift, [2, 0, 0]);
    }
}
