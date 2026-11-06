// SPDX-License-Identifier: Apache-2.0
//! One volume of PRELUDE (`specs/prelude.md` §4.4 to §4.10): the mask, the linear ramps, the
//! phase intervals and regions, the constraints and their merging, the slices' joining.
//!
//! Every value follows the spec's order and precision; the structures, the memory layout and the
//! threads are larmorx's (§10). The defects' rules (D13) are switches of [`Rules`], so that a
//! caller can compute FSL's and the corrected result and name a defect exactly where they differ.

use rayon::prelude::*;

use super::components::{self, NONE};
use super::merge::{self, PairMap, Sum};
use super::num::{
    self, TWO_PI, add64, extrema_finite, extrema_fsl, interval, interval_width, mul64, narrow, rnd,
    sub32, sub64, wide, wrap,
};
use crate::cli::fmt;

/// How a volume's regions are labelled and unwrapped (§4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// 3D mask, 3D labels, 3D unwrapping.
    ThreeD,
    /// 3D mask, labels slice by slice (numbered through the slices), 3D unwrapping.
    Labels2D,
    /// `-s`: the mask, the labels and the unwrapping slice by slice, then the slices joined.
    Slices,
}

/// The defects' rules (§9): `true` is larmorx's correction, `false` FSL's behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// FSL-261: the ramps over every voxel (FSL: without the last column, row and slice).
    pub ramps_everywhere: bool,
    /// FSL-268: NaN values of the mask's source left out of the threshold and the maximum.
    pub nan_source: bool,
    /// FSL-269: NaN phases left out of the mask.
    pub nan_phase: bool,
}

impl Rules {
    /// Every correction (the default) or none (`reproduce`).
    pub fn all(fix: bool) -> Rules {
        Rules {
            ramps_everywhere: fix,
            nan_source: fix,
            nan_phase: fix,
        }
    }
}

/// The mask's threshold (§4.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Threshold {
    /// `-t`'s value, or 0.5 with `-m`.
    Given(f32),
    /// The source's default threshold (2nd and 98th percentiles).
    Default,
}

/// A case FSL's result is undefined for (a sort whose scans read outside its values): refused
/// in both modes (§6.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Undefined {
    /// The defect whose rule leads there (FSL-268 or FSL-269).
    pub defect: &'static str,
    /// What is sorted.
    pub what: String,
}

/// What one volume's processing needs.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// `-n`, the number of phase intervals (at least 2).
    pub n: i32,
    pub mode: Mode,
    pub threshold: Threshold,
    /// `--removeramps`.
    pub ramps: bool,
    pub verbose: bool,
    pub rules: Rules,
    /// Keep the ramp-free phase (FSL-266's `grot`).
    pub keep_ramp_free: bool,
}

/// What one volume gives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VolumeResult {
    /// The mask `--savemask` writes (0 or 1), on its grid (the source's).
    pub mask: Vec<u8>,
    /// The grid of [`VolumeResult::mask`].
    pub mask_shape: [usize; 3],
    /// `-v`'s lines of the ramps' estimate and removal (with `--removeramps`).
    pub ramp_lines: Vec<String>,
    /// The phase without its ramps (FSL-266's `grot`), when asked for.
    pub ramp_free: Option<Vec<f32>>,
    /// The regions' numbers (0 outside).
    pub labels: Vec<i32>,
    /// `-v`'s lines from `Calculating starting matrices` to `Did while loop` (with each slice's
    /// `SLICE NUMBER` in the slices mode).
    pub merge_lines: Vec<String>,
    /// The unwrapped phase (0 outside the regions).
    pub unwrapped: Vec<f32>,
    /// The phase as the volume leaves it (the ramps removed and restored), which `-r` writes.
    pub phase: Vec<f32>,
    /// Whether a NaN phase lay inside the mask before FSL-269's rule (both modes).
    pub nan_in_mask: bool,
    /// Whether a mask voxel lies in the last column, row or slice (FSL-261's domain).
    pub mask_on_edge: bool,
}

/// The default threshold of a source volume (§4.4), with FSL-268's rule: by default its NaN
/// values are left out (none left: 0); FSL's sorts them in (libstdc++'s order, refused where
/// its scans read outside the values).
pub fn default_threshold(src: &[f32], nan_fix: bool) -> Result<f32, Undefined> {
    let has_nan = src.par_iter().any(|x| x.is_nan());
    let mut v: Vec<f32> = if nan_fix && has_nan {
        src.par_iter().copied().filter(|x| !x.is_nan()).collect()
    } else {
        src.to_vec()
    };
    let n = v.len();
    if n == 0 {
        return Ok(0.0);
    }
    let at = |q: f32| -> usize { ((n as f32 * q).floor() as usize).min(n - 1) };
    let (i2, i98) = (at(0.02), at(0.98));
    let (t2, t98) = if has_nan && !nan_fix {
        if !crate::flirt::schedule::std_sort_checked(&mut v, &|a: &f32, b: &f32| a < b) {
            return Err(Undefined {
                defect: "FSL-268",
                what: "the default threshold's sort of a magnitude holding NaN values".into(),
            });
        }
        (v[i2], v[i98])
    } else {
        let cmp = |a: &f32, b: &f32| a.total_cmp(b);
        v.select_nth_unstable_by(i98, cmp);
        let t98 = v[i98];
        let t2 = if i2 < i98 {
            *v[..i98].select_nth_unstable_by(i2, cmp).1
        } else {
            t98
        };
        (t2, t98)
    };
    let span = sub32(t98, t2);
    Ok(narrow(add64(mul64(0.1, wide(span)), wide(t2))))
}

/// The largest value of `v` by §4.2's rule (FSL's), or of its values that are not NaN (0 when
/// none: FSL-268's correction).
fn top(v: &[f32], nan_fix: bool) -> f32 {
    if nan_fix {
        extrema_finite(v).map_or(0.0, |e| e.0)
    } else {
        extrema_fsl(v).map_or(0.0, |e| e.0)
    }
}

/// The mask of §4.4 on the source's grid: the voxels `theta <= x <= top`, their largest
/// 6-connected component (in the slices mode each slice's largest 4-connected one).
pub fn make_mask(
    src: &[f32],
    shape: [usize; 3],
    threshold: Threshold,
    slices: bool,
    nan_fix: bool,
) -> Result<Vec<u8>, Undefined> {
    let theta = match threshold {
        Threshold::Given(t) => t,
        Threshold::Default => default_threshold(src, nan_fix)?,
    };
    let plane = shape[0] * shape[1];
    let n = plane * shape[2];
    let mut parent = vec![NONE; n];
    let mut counts = vec![0u32; n];
    let mut mask = vec![0u8; n];
    if slices {
        let tops: Vec<f32> = if plane == 0 {
            Vec::new()
        } else {
            src.par_chunks(plane).map(|s| top(s, nan_fix)).collect()
        };
        let candidate = |i: usize| {
            let x = src[i];
            theta <= x && x <= tops[i / plane]
        };
        components::label(shape, candidate, |_, _| true, false, &mut parent);
        for z in 0..shape[2] {
            if let Some(r) = components::largest(&parent, z * plane..(z + 1) * plane, &mut counts) {
                for i in z * plane..(z + 1) * plane {
                    if parent[i] == r {
                        mask[i] = 1;
                    }
                }
            }
        }
    } else {
        let t = top(src, nan_fix);
        let candidate = |i: usize| {
            let x = src[i];
            theta <= x && x <= t
        };
        components::label(shape, candidate, |_, _| true, true, &mut parent);
        if let Some(r) = components::largest(&parent, 0..n, &mut counts) {
            mask.par_iter_mut()
                .zip(parent.par_iter())
                .for_each(|(m, &p)| *m = u8::from(p == r));
        }
    }
    Ok(mask)
}

/// The mask of the source's grid read at the phase's voxel indices (FSL-271 with `reproduce`):
/// a coordinate one voxel beyond the source's grid moves onto its edge; a point then inside reads
/// that voxel, any other reads 0.
pub fn mask_on_grid(mask: &[u8], from: [usize; 3], to: [usize; 3]) -> Vec<u8> {
    if from == to {
        return mask.to_vec();
    }
    let edge = |c: usize, n: usize| if c == n && n > 0 { n - 1 } else { c };
    let mut out = vec![0u8; to[0] * to[1] * to[2]];
    for z in 0..to[2] {
        for y in 0..to[1] {
            for x in 0..to[0] {
                let (a, b, c) = (edge(x, from[0]), edge(y, from[1]), edge(z, from[2]));
                if a < from[0] && b < from[1] && c < from[2] {
                    out[(z * to[1] + y) * to[0] + x] = mask[(c * from[1] + b) * from[0] + a];
                }
            }
        }
    }
    out
}

/// The slopes of §4.5 and the voxels they apply to.
#[derive(Clone, Copy, Debug)]
struct Ramps {
    r: [f64; 3],
    c: [f64; 3],
    domain: [usize; 3],
}

impl Ramps {
    /// `((r_x d_x + r_y d_y) + r_z d_z)` at voxel `(x, y, z)`.
    #[inline]
    fn at(&self, x: usize, y: usize, z: usize) -> [f64; 3] {
        [
            mul64(self.r[0], x as f64 - self.c[0]),
            mul64(self.r[1], y as f64 - self.c[1]),
            mul64(self.r[2], z as f64 - self.c[2]),
        ]
    }
}

/// §4.5's estimate over the domain: the mean wrapped forward difference along each axis.
fn estimate_ramps(phase: &[f32], mask: &[u8], shape: [usize; 3], everywhere: bool) -> Ramps {
    let [nx, ny, nz] = shape;
    let domain = if everywhere {
        shape
    } else {
        [
            nx.saturating_sub(1),
            ny.saturating_sub(1),
            nz.saturating_sub(1),
        ]
    };
    let mut s = [0f64; 3];
    let mut n = [0u32; 3];
    let steps = [1, nx, nx * ny];
    for z in 0..domain[2] {
        for y in 0..domain[1] {
            for x in 0..domain[0] {
                let i = (z * ny + y) * nx + x;
                if mask[i] == 0 {
                    continue;
                }
                let inside = [x + 1 < nx, y + 1 < ny, z + 1 < nz];
                for a in 0..3 {
                    if !inside[a] {
                        continue;
                    }
                    let q = i + steps[a];
                    if mask[q] == 0 {
                        continue;
                    }
                    s[a] = add64(s[a], wide(wrap(sub32(phase[q], phase[i]))));
                    n[a] += 1;
                }
            }
        }
    }
    let r = [0, 1, 2].map(|a| {
        if n[a] > 0 {
            num::div64(s[a], f64::from(n[a] as f32))
        } else {
            0.0
        }
    });
    Ramps {
        r,
        c: shape.map(|k| (k as f64 - 1.0) / 2.0),
        domain,
    }
}

/// Runs `f(i, x, y, z)` over the ramps' domain inside the mask, rows in parallel.
fn over_domain(
    values: &mut [f32],
    mask: &[u8],
    shape: [usize; 3],
    ramps: &Ramps,
    f: impl Fn(f32, [f64; 3]) -> f32 + Sync,
) {
    let [nx, ny, _] = shape;
    if nx == 0 {
        return;
    }
    values
        .par_chunks_mut(nx)
        .zip(mask.par_chunks(nx))
        .enumerate()
        .for_each(|(row, (v, m))| {
            let (y, z) = (row % ny, row / ny);
            if y >= ramps.domain[1] || z >= ramps.domain[2] {
                return;
            }
            for x in 0..ramps.domain[0] {
                if m[x] != 0 {
                    v[x] = f(v[x], ramps.at(x, y, z));
                }
            }
        });
}

/// Labels the regions of §4.6 into `labels` (which holds each voxel's interval on entry; 0
/// outside the mask). Returns the largest number in each slice (one entry for the 3D labels).
fn number_regions(
    labels: &mut [i32],
    shape: [usize; 3],
    mode: Mode,
    parent: &mut [u32],
) -> Vec<i32> {
    let [nx, ny, nz] = shape;
    let plane = nx * ny;
    {
        let iv: &[i32] = labels;
        components::label(
            shape,
            |i| iv[i] != 0,
            |p, q| iv[p] == iv[q],
            mode == Mode::ThreeD,
            parent,
        );
    }
    match mode {
        Mode::ThreeD => {
            let mut next = 0i32;
            for i in 0..labels.len() {
                let p = parent[i];
                labels[i] = if p == NONE {
                    0
                } else if p as usize == i {
                    next += 1;
                    next
                } else {
                    labels[p as usize]
                };
            }
            vec![next]
        }
        Mode::Labels2D | Mode::Slices => {
            // each slice numbered on from the previous slice's largest number (FSL takes it
            // from that slice itself, 0 for a slice without regions); the slices mode from 0
            let mut maxima = vec![0i32; nz];
            for z in 0..nz {
                let base = if mode == Mode::Labels2D && z > 0 {
                    maxima[z - 1]
                } else {
                    0
                };
                let mut next = base;
                let mut largest = 0i32;
                for i in z * plane..(z + 1) * plane {
                    let p = parent[i];
                    labels[i] = if p == NONE {
                        0
                    } else if p as usize == i {
                        next += 1;
                        next
                    } else {
                        labels[p as usize]
                    };
                    largest = largest.max(labels[i]);
                }
                maxima[z] = largest;
            }
            maxima
        }
    }
}

/// The sums of §4.7 over the voxels `range` of a volume (backward neighbours along x and y, and
/// z with `three_d`), in the visiting order.
fn collect_sums(
    labels: &[i32],
    phase: &[f32],
    shape: [usize; 3],
    zs: std::ops::Range<usize>,
    three_d: bool,
) -> PairMap<Sum> {
    let [nx, ny, _] = shape;
    let plane = nx * ny;
    let mut sums: PairMap<Sum> = PairMap::default();
    for z in zs {
        for y in 0..ny {
            for x in 0..nx {
                let i = z * plane + y * nx + x;
                let l1 = labels[i];
                if l1 == 0 {
                    continue;
                }
                let mut visit = |q: usize| {
                    let l2 = labels[q];
                    if l2 != 0 && l2 != l1 {
                        merge::contribute(&mut sums, l1 as u32, phase[i], l2 as u32, phase[q]);
                    }
                };
                if x > 0 {
                    visit(i - 1);
                }
                if y > 0 {
                    visit(i - nx);
                }
                if three_d && z > 0 {
                    visit(i - plane);
                }
            }
        }
    }
    sums
}

/// `u = fl(phi + 2 pi o_l)` for the voxels with a region, 0 elsewhere.
fn unwrap_values(out: &mut [f32], labels: &[i32], phase: &[f32], offsets: &[f32]) {
    out.par_iter_mut()
        .zip(labels.par_iter().zip(phase.par_iter()))
        .with_min_len(1 << 14)
        .for_each(|(u, (&l, &p))| {
            *u = if l == 0 {
                0.0
            } else {
                narrow(add64(wide(p), TWO_PI * f64::from(offsets[l as usize])))
            };
        });
}

/// The slices' join (§4.9): each slice moved by the whole turns of the upper median difference
/// with the slice below (as already joined).
fn join_slices(u: &mut [f32], labels: &[i32], shape: [usize; 3]) -> Result<(), Undefined> {
    let plane = shape[0] * shape[1];
    for z in 0..shape[2].saturating_sub(1) {
        let (lo, hi) = u.split_at_mut((z + 1) * plane);
        let below = &lo[z * plane..];
        let above = &mut hi[..plane];
        let lb = &labels[z * plane..(z + 1) * plane];
        let la = &labels[(z + 1) * plane..(z + 2) * plane];
        let mut d: Vec<f32> = (0..plane)
            .filter(|&j| lb[j] != 0 && la[j] != 0)
            .map(|j| sub32(above[j], below[j]))
            .collect();
        if d.is_empty() {
            continue;
        }
        let at = d.len() / 2;
        let median = if d.iter().any(|x| x.is_nan()) {
            if !crate::flirt::schedule::std_sort_checked(&mut d, &|a: &f32, b: &f32| a < b) {
                return Err(Undefined {
                    defect: "FSL-269",
                    what: format!(
                        "the sort of the differences between slices {z} and {} holding NaN values",
                        z + 1
                    ),
                });
            }
            d[at]
        } else {
            *d.select_nth_unstable_by(at, |a, b| a.total_cmp(b)).1
        };
        let m = f64::from(rnd(num::div64(wide(median), TWO_PI)));
        let shift = m * TWO_PI;
        for j in 0..plane {
            if la[j] != 0 {
                above[j] = narrow(sub64(wide(above[j]), shift));
            }
        }
    }
    Ok(())
}

/// Processes one volume (§4.4 to §4.10): `phase` is the volume's phase after §4.2, `src` the
/// mask's source volume on its grid `src_shape`. `Err` where FSL's result is undefined.
pub fn process(
    mut phase: Vec<f32>,
    shape: [usize; 3],
    src: &[f32],
    src_shape: [usize; 3],
    p: &Params,
) -> Result<VolumeResult, Undefined> {
    let [nx, ny, nz] = shape;
    let plane = nx * ny;
    let nvox = plane * nz;
    // §4.4: the mask on the source's grid, read at the phase's voxels
    let src_mask = make_mask(
        src,
        src_shape,
        p.threshold,
        p.mode == Mode::Slices,
        p.rules.nan_source,
    )?;
    let mut mask = mask_on_grid(&src_mask, src_shape, shape);
    let nan_in_mask = mask
        .par_iter()
        .zip(phase.par_iter())
        .any(|(&m, x)| m != 0 && x.is_nan());
    let mut nan_voxels: Vec<usize> = Vec::new();
    if nan_in_mask && p.rules.nan_phase {
        for i in 0..nvox {
            if mask[i] != 0 && phase[i].is_nan() {
                mask[i] = 0;
                nan_voxels.push(i);
            }
        }
    }
    let saved_mask = if src_shape == shape {
        mask.clone()
    } else {
        src_mask
    };
    let mask_on_edge = (0..nvox).into_par_iter().any(|i| {
        mask[i] != 0 && {
            let (x, y, z) = (i % nx, (i / nx) % ny, i / plane);
            x + 1 == nx || y + 1 == ny || z + 1 == nz
        }
    });
    // §4.5: the ramps
    let mut ramp_lines = Vec::new();
    let mut ramps = None;
    let mut ramp_free = None;
    if p.ramps {
        if p.verbose {
            ramp_lines.push("Removing linear ramps".to_owned());
        }
        let r = estimate_ramps(&phase, &mask, shape, p.rules.ramps_everywhere);
        if p.verbose {
            ramp_lines.push("Estimated linear ramps".to_owned());
        }
        over_domain(&mut phase, &mask, shape, &r, |v, [a, b, c]| {
            wrap(narrow(sub64(sub64(sub64(wide(v), a), b), c)))
        });
        if p.verbose {
            ramp_lines.push("Removed linear ramps".to_owned());
            ramp_lines.push(format!("Ramp values: {} ", fmt::f(r.r[0])));
            ramp_lines.push(format!("{} ", fmt::f(r.r[1])));
            ramp_lines.push(format!("{} ", fmt::f(r.r[2])));
            ramp_lines.push(String::new());
        }
        if p.keep_ramp_free {
            ramp_free = Some(phase.clone());
        }
        ramps = Some(r);
    }
    // §4.6: the intervals, then the regions
    let width = interval_width(p.n);
    let mut labels: Vec<i32> = phase
        .par_iter()
        .zip(mask.par_iter())
        .with_min_len(1 << 14)
        .map(|(&v, &m)| if m != 0 { interval(v, width, p.n) } else { 0 })
        .collect();
    let mut parent = vec![NONE; nvox];
    let maxima = number_regions(&mut labels, shape, p.mode, &mut parent);
    drop(parent);
    // §4.7 to §4.9
    let mut unwrapped = vec![0f32; nvox];
    let mut merge_lines = Vec::new();
    if p.mode == Mode::Slices {
        let per: Vec<(Vec<String>, Vec<f32>)> = (0..nz)
            .into_par_iter()
            .map(|z| {
                let mut lines = Vec::new();
                let l = maxima[z];
                if p.verbose {
                    lines.push(format!("SLICE NUMBER {z}"));
                    lines.push(format!("Calculating starting matrices ({l} by {l})"));
                }
                let sums = collect_sums(&labels, &phase, shape, z..z + 1, false);
                if p.verbose {
                    lines.push("Finished connection_matrices".to_owned());
                }
                let m = merge::merge(l as u32, sums, p.verbose, &mut lines);
                let mut u = vec![0f32; plane];
                let r = z * plane..(z + 1) * plane;
                unwrap_values(&mut u, &labels[r.clone()], &phase[r], &m.offsets);
                (lines, u)
            })
            .collect();
        for (z, (lines, u)) in per.into_iter().enumerate() {
            merge_lines.extend(lines);
            unwrapped[z * plane..(z + 1) * plane].copy_from_slice(&u);
        }
        join_slices(&mut unwrapped, &labels, shape)?;
    } else {
        let l = maxima.iter().copied().max().unwrap_or(0);
        if p.verbose {
            merge_lines.push(format!("Calculating starting matrices ({l} by {l})"));
        }
        let sums = collect_sums(&labels, &phase, shape, 0..nz, true);
        if p.verbose {
            merge_lines.push("Finished connection_matrices".to_owned());
        }
        let m = merge::merge(l as u32, sums, p.verbose, &mut merge_lines);
        unwrap_values(&mut unwrapped, &labels, &phase, &m.offsets);
    }
    // FSL-269's correction: the unwrapped phase NaN where the phase is
    for &i in &nan_voxels {
        unwrapped[i] = phase[i];
    }
    // §4.5: the ramps restored on the unwrapped and the kept phase
    if let Some(r) = ramps {
        let add = |v: f32, [a, b, c]: [f64; 3]| narrow(add64(wide(v), add64(add64(a, b), c)));
        over_domain(&mut unwrapped, &mask, shape, &r, add);
        over_domain(&mut phase, &mask, shape, &r, add);
    }
    Ok(VolumeResult {
        mask: saved_mask,
        mask_shape: src_shape,
        ramp_lines,
        ramp_free,
        labels,
        merge_lines,
        unwrapped,
        phase,
        nan_in_mask,
        mask_on_edge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(mode: Mode) -> Params {
        Params {
            n: 8,
            mode,
            threshold: Threshold::Given(0.5),
            ramps: false,
            verbose: true,
            rules: Rules::all(true),
            keep_ramp_free: false,
        }
    }

    #[test]
    fn a_wrapped_ramp_is_unwrapped() {
        // a phase rising 0.5 rad per voxel along x, wrapped
        let shape = [16, 4, 3];
        let n = 16 * 4 * 3;
        let truth: Vec<f32> = (0..n).map(|i| (i % 16) as f32 * 0.5 - 3.0).collect();
        let phase: Vec<f32> = truth.iter().map(|&v| wrap(v)).collect();
        let mag = vec![1f32; n];
        for mode in [Mode::ThreeD, Mode::Labels2D, Mode::Slices] {
            let r = process(phase.clone(), shape, &mag, shape, &params(mode)).unwrap();
            let base = r.unwrapped[0] - truth[0];
            for i in 0..n {
                assert!(
                    (r.unwrapped[i] - truth[i] - base).abs() < 1e-4,
                    "{mode:?} voxel {i}: {} vs {}",
                    r.unwrapped[i],
                    truth[i]
                );
            }
            assert!(r.merge_lines.last().unwrap().starts_with("Did while loop"));
        }
    }

    #[test]
    fn thresholds() {
        let v: Vec<f32> = (0..100).map(|i| i as f32).collect();
        // i2 = 2, i98 = 98: 0.1 * 96 + 2
        assert_eq!(default_threshold(&v, true).unwrap(), 11.6);
        let mut w = v.clone();
        w[5] = f32::NAN;
        assert_eq!(default_threshold(&[f32::NAN], true).unwrap(), 0.0);
        assert!(default_threshold(&w, true).is_ok());
    }

    #[test]
    fn grids() {
        let m = vec![1u8; 4];
        assert_eq!(
            mask_on_grid(&m, [2, 2, 1], [3, 2, 1]),
            vec![1, 1, 1, 1, 1, 1]
        );
        assert_eq!(mask_on_grid(&m, [2, 2, 1], [4, 2, 1])[3], 0);
    }
}
