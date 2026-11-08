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
use super::rotate::{self, Transform, Trig, rot3};
use super::shift;

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

/// The scores of `cands` for the volume `v` over `reg`, eight candidates per sweep (sixteen
/// independent float32 chains, each summed in AFNI's order).
fn scores(v: &[f32], reg: &Region, cands: &[[i32; 3]], n: [usize; 3], weighted: bool) -> Vec<f32> {
    const B: usize = 8;
    let [nx, ny, _] = n;
    let off: Vec<isize> = cands
        .iter()
        .map(|c| c[0] as isize + nx as isize * (c[1] as isize + ny as isize * c[2] as isize))
        .collect();
    let mut out = Vec::with_capacity(cands.len());
    for chunk in off.chunks(B) {
        let m = chunk.len();
        let mut o = [0isize; B];
        o[..m].copy_from_slice(chunk);
        let mut svv = [0f32; B];
        let mut sbv = [0f32; B];
        for ((&p, &wv), &wbv) in reg.idx.iter().zip(&reg.w).zip(&reg.wb) {
            let p = p as isize;
            let x: [f32; B] = std::array::from_fn(|c| v[(p - o[c]) as usize]);
            if weighted {
                for c in 0..B {
                    svv[c] += (wv * x[c]) * x[c];
                    sbv[c] += wbv * x[c];
                }
            } else {
                for c in 0..B {
                    svv[c] += x[c] * x[c];
                    sbv[c] += wbv * x[c];
                }
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
    cands: &[[i32; 3]],
    weighted: bool,
    take_empty: bool,
) -> ([i32; 3], f32) {
    let mut best = ([0i32; 3], reg.total);
    if reg.empty && !take_empty {
        return best;
    }
    let s = if reg.empty {
        vec![0f32; cands.len()]
    } else {
        scores(v, reg, cands, n, weighted)
    };
    for (c, sc) in cands.iter().zip(s) {
        if sc < best.1 {
            best = (*c, sc);
        }
    }
    best
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

/// A row of a nearest-neighbour shear whose shift moves it out of the row: all zeros.
const ZERO_ROW: i32 = i32::MIN;

/// One shear of a nearest-neighbour rotation: its axis and the integer offset `ia` of every row
/// (§5.6: the sample `i + ia`), indexed by the other two axes in index order, with the pair rule
/// of §5.5.
struct NnShear {
    axis: usize,
    ia: Vec<i32>,
}

/// A nearest-neighbour rotation as an index map: the flip and the shears of its plan, on the
/// padded grid `dims` (padding `z`).
struct NnPlan {
    dims: [usize; 3],
    z: usize,
    flip: Option<(usize, usize)>,
    shears: Vec<NnShear>,
}

impl NnPlan {
    /// The plan of `p` (from [`rotate::plan`]) on a grid of `n` voxels padded by `z`; `None`
    /// when the plan has no shears (the volume stays as it was).
    fn new(p: &rotate::Plan, n: [usize; 3], z: usize) -> Option<NnPlan> {
        let shears = p.shears.as_ref()?;
        let dims = n.map(|d| d + 2 * z);
        let mut out = Vec::new();
        for sh in shears {
            let Some((a, b, s, _)) = rotate::coefficients(sh, dims) else {
                continue;
            };
            let others: Vec<usize> = (0..3).filter(|&x| x != sh.axis).collect();
            let (u, w) = (others[0], others[1]);
            let cu = 0.5f32 * (dims[u] as f32 - 1.0);
            let cw = 0.5f32 * (dims[w] as f32 - 1.0);
            let (nu, nw, len) = (dims[u], dims[w], dims[sh.axis]);
            let mut ia = vec![0i32; nu * nw];
            let cell = |alpha: f32| match shift::split(alpha, len) {
                Some((i, _)) => i as i32,
                None => ZERO_ROW,
            };
            for wi in 0..nw {
                let mut ui = 0;
                while ui < nu {
                    let a0 = a * (ui as f32 - cu) + b * (wi as f32 - cw) + s;
                    ia[ui + nu * wi] = cell(a0);
                    if ui + 1 < nu {
                        ia[ui + 1 + nu * wi] = cell(a0 + a);
                    }
                    ui += 2;
                }
            }
            out.push(NnShear { axis: sh.axis, ia });
        }
        Some(NnPlan {
            dims,
            z,
            flip: p.flip,
            shears: out,
        })
    }

    /// `v` (a volume of `n` voxels) rotated into `out` by the plan's integer row shifts, one
    /// pass per shear over the padded volume (`bufs`: two padded work volumes).
    fn apply(&self, v: &[f32], n: [usize; 3], out: &mut [f32], bufs: &mut [Vec<f32>; 2]) {
        let [px, py, pz] = self.dims;
        let z = self.z;
        let total = px * py * pz;
        let [a, b] = bufs;
        a.clear();
        a.resize(total, 0.0);
        b.clear();
        b.resize(total, 0.0);
        for k in 0..n[2] {
            for j in 0..n[1] {
                let at = z + px * (j + z + py * (k + z));
                a[at..at + n[0]].copy_from_slice(&v[n[0] * (j + n[1] * k)..][..n[0]]);
            }
        }
        if let Some((f, g)) = self.flip {
            let rev = |axis: usize, i: usize| {
                if axis == f || axis == g {
                    self.dims[axis] - 1 - i
                } else {
                    i
                }
            };
            for k in 0..pz {
                for j in 0..py {
                    for i in 0..px {
                        b[i + px * (j + py * k)] = a[rev(0, i) + px * (rev(1, j) + py * rev(2, k))];
                    }
                }
            }
            std::mem::swap(a, b);
        }
        let plane = px * py;
        for sh in &self.shears {
            let (src, dst): (&[f32], &mut [f32]) = (a, b);
            match sh.axis {
                0 => {
                    // Rows along x; the table indexed by (y, z).
                    for (r, row) in dst.chunks_mut(px).enumerate() {
                        let ia = sh.ia[r];
                        let srow = &src[px * r..px * r + px];
                        row.fill(0.0);
                        if ia == ZERO_ROW {
                            continue;
                        }
                        let ia = ia as isize;
                        let lo = (-ia).max(0) as usize;
                        let hi = (px as isize - ia).min(px as isize).max(0) as usize;
                        if lo < hi {
                            let s0 = (lo as isize + ia) as usize;
                            row[lo..hi].copy_from_slice(&srow[s0..s0 + (hi - lo)]);
                        }
                    }
                }
                1 => {
                    // Rows along y; the table indexed by (x, z).
                    for (k, dpl) in dst.chunks_mut(plane).enumerate() {
                        let spl = &src[plane * k..plane * (k + 1)];
                        let tab = &sh.ia[px * k..px * (k + 1)];
                        for j in 0..py {
                            let drow = &mut dpl[px * j..px * (j + 1)];
                            for (i, (d, &ia)) in drow.iter_mut().zip(tab).enumerate() {
                                let jj = j as i64 + i64::from(ia);
                                *d = if ia != ZERO_ROW && jj >= 0 && jj < py as i64 {
                                    spl[i + px * jj as usize]
                                } else {
                                    0.0
                                };
                            }
                        }
                    }
                }
                _ => {
                    // Rows along z; the table indexed by (x, y).
                    for (k, dpl) in dst.chunks_mut(plane).enumerate() {
                        for (q, (d, &ia)) in dpl.iter_mut().zip(&sh.ia).enumerate() {
                            let kk = k as i64 + i64::from(ia);
                            *d = if ia != ZERO_ROW && kk >= 0 && kk < pz as i64 {
                                src[q + plane * kk as usize]
                            } else {
                                0.0
                            };
                        }
                    }
                }
            }
            std::mem::swap(a, b);
        }
        for k in 0..n[2] {
            for j in 0..n[1] {
                let at = z + px * (j + z + py * (k + z));
                out[n[0] * (j + n[1] * k)..][..n[0]].copy_from_slice(&a[at..at + n[0]]);
            }
        }
    }
}

/// A coarse search against one base, prepared once per run (the base's region, weight and sums,
/// and the 44 rotations' index maps), then run on each volume.
pub struct Searcher {
    settings: CoarseSettings,
    n: [usize; 3],
    cands: Vec<[i32; 3]>,
    reg: Region,
    /// The rotation search: the grid searched (subsampled when every axis has 120 voxels or
    /// more), the weight's percentage, and the 45 rotations with their maps (`None`: identity,
    /// or a rotation that leaves the volume unchanged).
    rotations: Option<Rotations>,
}

struct Rotations {
    big: bool,
    m: [usize; 3],
    pct: f32,
    angles: Vec<[i32; 3]>,
    plans: Vec<Option<NnPlan>>,
}

impl Searcher {
    /// The search of §8.6 against the base `b` (a volume of `n` voxels of sizes `sizes`).
    pub fn new(b: &[f32], n: [usize; 3], sizes: [f32; 3], s: &CoarseSettings) -> Searcher {
        if !s.rotations {
            let e = i64::from(s.del) * i64::from(s.num + 1);
            return Searcher {
                settings: *s,
                n,
                cands: candidates(s.del, s.num),
                reg: region(b, None, n, e),
                rotations: None,
            };
        }
        let big = n.iter().all(|&d| d >= 120);
        let (bb, m, del, sizes) = if big {
            let (bs, m) = subsample(b, n, &s.blur);
            (bs, m, s.del / 2, sizes.map(|d| 2.0 * d))
        } else {
            (b.to_vec(), n, s.del, sizes)
        };
        let (w, pct) = rotation_weight(&bb, m, s);
        let e = i64::from(del) * i64::from(s.num + 1);
        let reg = region(&bb, Some(&w), m, e);
        let mut angles = Vec::with_capacity(45);
        for a in -1i32..=1 {
            for bi in -2i32..=2 {
                for c in -1i32..=1 {
                    angles.push([9 * a, 9 * bi, 9 * c]);
                }
            }
        }
        let deg = std::f64::consts::PI / 180.0;
        let small = m.iter().any(|&d| d < 2);
        let plans = angles
            .par_iter()
            .map(|rot| {
                if *rot == [0, 0, 0] || small {
                    return None;
                }
                let theta = [0, 1, 2].map(|q| -deg * f64::from(s.signs[q]) * f64::from(rot[q]));
                let mat: M3 = rot3(s.axes, theta, s.trig);
                let tf = Transform::Matrix {
                    m: mat,
                    shift: [0.0; 3],
                };
                let p = rotate::plan(&tf, sizes, s.trig);
                if p.shears.is_none() {
                    rotate::NO_SHEARS.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                NnPlan::new(&p, m, s.zpad)
            })
            .collect();
        Searcher {
            settings: *s,
            n,
            cands: candidates(del, s.num),
            reg,
            rotations: Some(Rotations {
                big,
                m,
                pct,
                angles,
                plans,
            }),
        }
    }

    /// The search for the volume `v` (on the base's grid).
    pub fn search(&self, v: &[f32]) -> Found {
        let s = &self.settings;
        let Some(r) = &self.rotations else {
            let (shift, _) = best_shift(v, &self.reg, self.n, &self.cands, false, s.take_empty);
            return Found {
                angles: [0.0; 3],
                shift,
                marks: String::new(),
                empty_region: self.reg.empty,
            };
        };
        let mut marks = String::new();
        let sub;
        let vb: &[f32] = if r.big {
            marks.push('x');
            sub = subsample(v, self.n, &s.blur).0;
            &sub
        } else {
            v
        };
        marks.push('w');
        marks.push_str(&format!("[{}%]", crate::cfmt::f1(f64::from(r.pct))));
        if self.reg.empty && !s.take_empty {
            marks.push_str(&".".repeat(45));
            return Found {
                angles: [0.0; 3],
                shift: [0; 3],
                marks,
                empty_region: true,
            };
        }
        let results: Vec<([i32; 3], f32)> = r
            .plans
            .par_iter()
            .map_init(
                || ([Vec::new(), Vec::new()], Vec::new()),
                |(bufs, out), plan| match plan {
                    None => best_shift(vb, &self.reg, r.m, &self.cands, true, s.take_empty),
                    Some(p) => {
                        out.resize(vb.len(), 0.0);
                        p.apply(vb, r.m, out, bufs);
                        best_shift(out, &self.reg, r.m, &self.cands, true, s.take_empty)
                    }
                },
            )
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
        if r.big {
            shift = shift.map(|x| 2 * x);
        }
        Found {
            angles: r.angles[found.0].map(|x| x as f32),
            shift,
            marks,
            empty_region: self.reg.empty,
        }
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
        let (shift, _) = best_shift(&v, &reg, n, &candidates(2, 1), false, false);
        assert_eq!(shift, [2, 0, 0]);
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    /// `cargo test --release -p larmorx-afni coarse_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn coarse_timing() {
        larmorx_core::parallel::with_threads(1, timed).unwrap();
    }

    fn timed() {
        let n = [64usize, 64, 34];
        let total = n[0] * n[1] * n[2];
        let b: Vec<f32> = (0..total).map(|p| ((p * 7919) % 1000) as f32).collect();
        let w: Vec<f32> = (0..total)
            .map(|p| if p % 3 == 0 { 0.0 } else { 0.5 })
            .collect();
        let reg = region(&b, Some(&w), n, 6);
        let cands = candidates(2, 2);
        let t = std::time::Instant::now();
        let mut acc = 0.0f32;
        for _ in 0..45 {
            acc += scores(&b, &reg, &cands, n, true)[0];
        }
        eprintln!(
            "45 x 33 scores over {} voxels: {:?} ({acc})",
            reg.idx.len(),
            t.elapsed()
        );
        let deg = std::f64::consts::PI / 180.0;
        let mat = rot3(
            [2, 0, 1],
            [-9.0 * deg, 18.0 * deg, -9.0 * deg],
            Trig::Correct,
        );
        let tf = Transform::Matrix {
            m: mat,
            shift: [0.0; 3],
        };
        let p = rotate::plan(&tf, [3.0, 3.0, 4.0], Trig::Correct);
        let plan = NnPlan::new(&p, n, 4).unwrap();
        let mut out = vec![0f32; total];
        let t = std::time::Instant::now();
        let mut bufs = [Vec::new(), Vec::new()];
        for _ in 0..44 {
            plan.apply(&b, n, &mut out, &mut bufs);
        }
        eprintln!("44 NN rotations: {:?}", t.elapsed());
        let r = rotate::Resampler {
            method: shift::Method::NN,
            zpad: 4,
            trig: Trig::Correct,
        };
        let mut work = rotate::Work::default();
        let t = std::time::Instant::now();
        for _ in 0..44 {
            r.apply(&b, n, [3.0, 3.0, 4.0], &tf, &mut out, &mut work);
        }
        eprintln!("44 NN rotations by shears: {:?}", t.elapsed());
        for m in [
            shift::Method::Linear,
            shift::Method::Heptic,
            shift::Method::Fourier,
        ] {
            let r = rotate::Resampler {
                method: m,
                zpad: 4,
                trig: Trig::Correct,
            };
            let t = std::time::Instant::now();
            for _ in 0..10 {
                r.apply(&b, n, [3.0, 3.0, 4.0], &tf, &mut out, &mut work);
            }
            eprintln!("10 {m:?} rotations: {:?}", t.elapsed());
        }
        let bs = BlurSettings {
            firfac: None,
            old: false,
            trig: Trig::Correct,
        };
        let mut v = b.clone();
        let t = std::time::Instant::now();
        for _ in 0..10 {
            blur::fir(&mut v, n, [2.0; 3], [1.0; 3], 2.5, false, &bs);
        }
        eprintln!("10 blurs (sigma 2): {:?}", t.elapsed());
    }
}
