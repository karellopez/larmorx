// SPDX-License-Identifier: Apache-2.0
//! fslmaths' threshold-free cluster enhancement (`specs/fslmaths.md` §14): `-tfce H E C`,
//! `-tfceS H E C X Y Z T`, with `-tfce_minT` (the lowest threshold) and `-tfce_delta` (the
//! step) set earlier in the chain.
//!
//! Each volume on its own; at each threshold the clusters (connected components) are labelled
//! by a breadth-first walk over the voxels above it. The enhancement of a voxel adds its terms
//! threshold by threshold in increasing order, so the result does not depend on the order the
//! clusters are found in. Black-box runs give FSL's values bit for bit with `E` and `H` held in
//! single precision (findings 17). The defects: FSL-153 (a NaN first voxel), FSL-174 (a step
//! that cannot advance), FSL-175 (no positive value with the automatic step), FSL-176 (other
//! connectivities), FSL-177 (`-tfceS`'s arrays), FSL-178 (`-tfceS` at a voxel outside).

use std::collections::VecDeque;
use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::math;

use super::filters::abort_lines;
use super::image::Img;
use super::num::{self, Num};
use super::ops::Stop;

/// `-tfce_minT` and `-tfce_delta` (single precision; 0 at the start).
#[derive(Clone, Copy, Debug, Default)]
pub struct Settings {
    pub min_t: f32,
    pub delta: f32,
}

const FAILED: &str = "ERROR: TFCE failed, please check your file for valid sizes and voxel values.";
const NEEDS_DELTA: &str = "Error: tfce requires a positive deltaT input.";
const NEEDS_SIZE: &str =
    "Error: tfce currently requires an input with at least 3 voxels extent into each dimension.";
const MANY_STEPS: &str = "Warning: tfce has detected a large number of integral steps. This operation may require a great deal of time to complete.";
const NOT_REACHED: &str = "it doesn't reach to specified threshold";

/// FSL's TFCE error: its lines, then status 1.
fn failure(what: &str) -> Stop {
    Stop::Exception(format!("{FAILED}{what}\n\nExiting\n"))
}

/// The neighbour offsets of a connectivity: faces from 6, edges from 18, corners from 26 (none
/// below 6), as `(dx, dy, dz)`.
fn offsets(c: i32) -> Vec<[i64; 3]> {
    let mut v = Vec::new();
    for dz in -1i64..=1 {
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let n = dx.abs() + dy.abs() + dz.abs();
                let ok = match n {
                    1 => c >= 6,
                    2 => c >= 18,
                    3 => c >= 26,
                    _ => false,
                };
                if ok {
                    v.push([dx, dy, dz]);
                }
            }
        }
    }
    v
}

/// FSL-176 with the default: a connectivity other than 6, 18, 26 is an error (rule B).
fn check_connectivity(notices: &mut Notices, op: &str, c: i32) -> Result<(), Stop> {
    if notices.fix() && !matches!(c, 6 | 18 | 26) {
        notices.add(
            &defects::FSL_176,
            format!("{op} with the connectivity {c} is refused (FSL reads it as another one)"),
        );
        return Err(Stop::Refuse(format!(
            "{op}: the connectivity must be 6, 18 or 26, not {c} (FSL-176)"
        )));
    }
    Ok(())
}

/// The volume's values in single precision.
fn single<T: Num>(vol: &[T]) -> Vec<f32> {
    vol.iter().map(|v| num::to_f32(v.to_f64())).collect()
}

/// The maximum from the first value (§11.1; FSL-153 when `skip_nan`: NaN left out).
fn maximum<T: Num>(vol: &[T], skip_nan: bool) -> T {
    super::ops::extremes(vol, skip_nan).map_or(T::zero(), |e| e.1)
}

/// `-tfce H E C` (§14.1).
#[allow(clippy::too_many_arguments)]
pub fn tfce<T: Num>(
    img: &mut Img<T>,
    h_exp: f32,
    e_exp: f32,
    c: i32,
    set: Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
) -> Result<(), Stop> {
    let fix = notices.fix();
    check_connectivity(notices, "-tfce", c)?;
    let [nx, ny, nz, nt] = img.dims;
    // FSL tests each volume's step before the sizes (random testing, findings 26)
    let small = nx < 3 || ny < 3 || nz < 3;
    let per = nx * ny * nz;
    let offs = offsets(c);
    // FSL's TFCE in single precision for a double image too: its maximum, steps and
    // thresholds are the float image's (random testing: a float64 image's -tfce is FSL's float
    // result bit for bit; findings 26)
    let mut result = Vec::with_capacity(img.data.len());
    let (mut n153, mut n175) = (false, false);
    for t in 0..nt {
        let vol = &img.data[t * per..(t + 1) * per];
        let rep_m = maximum(vol, false);
        let ok_m = maximum(vol, true);
        let m = if fix { ok_m } else { rep_m };
        n153 |= !rep_m.same(ok_m);
        // the maximum as the float image's (a double image's rounded to single precision)
        let m64 = num::to_f64(num::to_f32(m.to_f64()));
        let auto = set.delta == 0.0;
        let delta = if auto {
            num::to_f32(m64 / 100.0)
        } else {
            set.delta
        };
        if fix && auto && !(m64 > 0.0) {
            // FSL-175: no value above the lowest threshold: the enhancement 0
            n175 |= m64 <= 0.0;
            result.extend(std::iter::repeat_n(T::zero(), per));
            continue;
        }
        if delta <= 0.0 {
            return Err(failure(NEEDS_DELTA));
        }
        if small {
            return Err(failure(NEEDS_SIZE));
        }
        if delta.is_nan() && fix {
            notices.add(
                &defects::FSL_174,
                "-tfce with a NaN step is refused (FSL gives zeros)",
            );
            return Err(Stop::Refuse(
                "-tfce: the threshold step is NaN (FSL-174)".to_owned(),
            ));
        }
        let steps = f64::from(num::to_f32(m64) / delta);
        if steps > 10000.0 {
            let _ = writeln!(out, "{MANY_STEPS}");
        }
        // the thresholds h0, h0 + δ, ... while h < M + δ (single-precision sums)
        let below = |h: f32| -> bool { h < num::to_f32(m64) + delta };
        let mut hs = Vec::new();
        let mut h = set.min_t;
        while below(h) {
            hs.push(h);
            let next = h + delta;
            if next == h {
                // FSL-174: the thresholds never advance (FSL never ends)
                if fix {
                    notices.add(
                        &defects::FSL_174,
                        format!(
                            "-tfce with thresholds that cannot advance (from {} by {delta}) is \
                             refused (FSL never ends)",
                            set.min_t
                        ),
                    );
                    return Err(Stop::Refuse(format!(
                        "-tfce: the threshold {h} plus the step {delta} stays {h} (FSL-174)"
                    )));
                }
                return Err(Stop::Refuse(format!(
                    "-tfce: the threshold {h} plus the step {delta} stays {h} (FSL-174: FSL never \
                     ends here)"
                )));
            }
            h = next;
        }
        let v = single(vol);
        let enh = enhance(&v, [nx, ny, nz], &hs, h_exp, e_exp, &offs);
        let mut changed = false;
        for e in enh {
            let (x, ch) = T::convert(num::to_f64(e), fix);
            changed |= ch;
            result.push(x);
        }
        super::ops::note151(notices, changed, "-tfce's values");
    }
    let mode = notices.mode();
    if n153 {
        notices.add(
            &defects::FSL_153,
            match mode {
                UpstreamDefects::Fix => {
                    "-tfce: the maximum of the values that are not NaN (FSL's is NaN: the first \
                     voxel is NaN, and its enhancement 0)"
                }
                UpstreamDefects::Reproduce => {
                    "-tfce: a NaN first voxel makes the maximum NaN (the enhancement 0), as FSL \
                     takes it"
                }
            },
        );
    }
    if n175 {
        notices.add(
            &defects::FSL_175,
            "-tfce of a volume without positive values: the enhancement 0 (FSL stops: tfce \
             requires a positive deltaT input)",
        );
    }
    img.data = result;
    Ok(())
}

/// The enhancement of one volume (single precision) over the thresholds `hs`: at each, the
/// clusters of interior voxels above it; each voxel of a cluster of `s` adds
/// `pow(s, E) · powf(h, H)` (the product in double).
fn enhance(
    v: &[f32],
    dims: [usize; 3],
    hs: &[f32],
    h_exp: f32,
    e_exp: f32,
    offs: &[[i64; 3]],
) -> Vec<f32> {
    let [nx, ny, nz] = dims;
    let n = v.len();
    let mut enh = vec![0.0f32; n];
    let idx = |x: usize, y: usize, z: usize| x + nx * (y + ny * z);
    let deltas: Vec<i64> = offs
        .iter()
        .map(|[dx, dy, dz]| dx + nx as i64 * (dy + ny as i64 * dz))
        .collect();
    // the interior voxels, then those above each threshold in turn
    let mut active: Vec<usize> = Vec::new();
    for z in 1..nz - 1 {
        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                active.push(idx(x, y, z));
            }
        }
    }
    let mut mark = vec![u32::MAX; n];
    let mut seen = vec![u32::MAX; n];
    let mut queue = VecDeque::new();
    let mut members = Vec::new();
    for (step, &h) in hs.iter().enumerate() {
        let step = step as u32;
        active.retain(|&k| v[k] > h);
        for &k in &active {
            mark[k] = step;
        }
        let hp = math::powf(h, h_exp);
        let hp = if hp.is_nan() && !h.is_nan() && !h_exp.is_nan() {
            f32::from_bits(num::DEFAULT_NAN_32)
        } else {
            hp
        };
        for &k in &active {
            if seen[k] == step {
                continue;
            }
            seen[k] = step;
            queue.clear();
            members.clear();
            queue.push_back(k);
            while let Some(p) = queue.pop_front() {
                members.push(p);
                for &d in &deltas {
                    let q = (p as i64 + d) as usize;
                    if mark[q] == step && seen[q] != step {
                        seen[q] = step;
                        queue.push_back(q);
                    }
                }
            }
            let s = members.len() as f64;
            let term = num::nan64(
                s,
                f64::from(hp),
                math::pow(s, f64::from(e_exp)) * f64::from(hp),
            );
            for &p in &members {
                enh[p] = num::acc32(enh[p], term);
            }
        }
    }
    enh
}

/// The cluster of voxel `at` among the voxels whose value is in `[h, FLT_MAX]` (faces, edges,
/// corners by the connectivity `c`: exactly 6, 18 or 26; any other: the voxel alone).
fn cluster(v: &[f32], dims: [usize; 3], at: usize, h: f32, c: i32) -> Vec<usize> {
    let [nx, ny, nz] = dims;
    let inside = |x: f32| x >= h && x <= f32::MAX;
    if !inside(v[at]) {
        return Vec::new();
    }
    let offs = if matches!(c, 6 | 18 | 26) {
        offsets(c)
    } else {
        Vec::new()
    };
    let mut seen = vec![false; v.len()];
    let mut queue = VecDeque::from([at]);
    seen[at] = true;
    let mut out = Vec::new();
    while let Some(p) = queue.pop_front() {
        out.push(p);
        let (x, y, z) = (p % nx, (p / nx) % ny, p / (nx * ny));
        for [dx, dy, dz] in &offs {
            let (qx, qy, qz) = (x as i64 + dx, y as i64 + dy, z as i64 + dz);
            if qx < 0 || qy < 0 || qz < 0 || qx >= nx as i64 || qy >= ny as i64 || qz >= nz as i64 {
                continue;
            }
            let q = qx as usize + nx * (qy as usize + ny * qz as usize);
            if !seen[q] && inside(v[q]) {
                seen[q] = true;
                queue.push_back(q);
            }
        }
    }
    out
}

/// The voxels of the volume outside every cluster at `h` (the label 0 of FSL's labelling).
fn background(v: &[f32], h: f32) -> Vec<usize> {
    (0..v.len())
        .filter(|&k| !(v[k] >= h && v[k] <= f32::MAX))
        .collect()
}

/// `-tfceS H E C X Y Z T` (§14.2): the support of TFCE at a voxel.
#[allow(clippy::too_many_arguments)]
pub fn tfce_support<T: Num>(
    img: &mut Img<T>,
    h_exp: f32,
    e_exp: f32,
    c: i32,
    at: [i32; 3],
    target: f32,
    set: Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
) -> Result<(), Stop> {
    let fix = notices.fix();
    check_connectivity(notices, "-tfceS", c)?;
    let [nx, ny, nz, nt] = img.dims;
    let per = nx * ny * nz;
    let dims = [nx, ny, nz];
    let x = if img.flipped {
        nx as i64 - 1 - i64::from(at[0])
    } else {
        i64::from(at[0])
    };
    let (y, z) = (i64::from(at[1]), i64::from(at[2]));
    let outside = x < 0 || y < 0 || z < 0 || x >= nx as i64 || y >= ny as i64 || z >= nz as i64;
    let offset = x + nx as i64 * (y + ny as i64 * z);
    if outside {
        // FSL-178: FSL reads the voxel at its offset from the volume's start
        if fix {
            notices.add(
                &defects::FSL_178,
                format!(
                    "-tfceS at the voxel {} {} {}, outside the image, is refused (FSL reads another \
                     voxel)",
                    at[0], at[1], at[2]
                ),
            );
            return Err(Stop::Refuse(format!(
                "-tfceS: the voxel {} {} {} is outside the image (FSL-178)",
                at[0], at[1], at[2]
            )));
        }
        if offset < 0 || offset >= per as i64 {
            return Err(Stop::Refuse(format!(
                "-tfceS: the voxel {} {} {} lies outside the image's memory (FSL-178: FSL reads \
                 outside the image here)",
                at[0], at[1], at[2]
            )));
        }
        notices.add(
            &defects::FSL_178,
            "-tfceS at a voxel outside the image: the value at its offset, in no cluster, as FSL \
             takes it",
        );
    }
    let at = offset as usize;
    let mut result = Vec::with_capacity(img.data.len());
    let mut n177 = false;
    for t in 0..nt {
        let vol = &img.data[t * per..(t + 1) * per];
        let v = single(vol);
        let m = v[at];
        let auto = set.delta == 0.0;
        let delta = if auto {
            num::to_f32(f64::from(m) / 100.0)
        } else {
            set.delta
        };
        let slots: i64 = if auto {
            101
        } else {
            i64::from(num::cvtt(f64::from(m / delta))) + 1
        };
        let bad_step = !(delta > 0.0) && m > set.min_t;
        if bad_step {
            // FSL-174: a NaN or non-positive step (FSL crashes or fills its arrays)
            if fix {
                notices.add(
                    &defects::FSL_174,
                    format!("-tfceS with the threshold step {delta} is refused (FSL never ends or crashes)"),
                );
                return Err(Stop::Refuse(format!(
                    "-tfceS: the threshold step {delta} cannot advance (FSL-174)"
                )));
            }
            if slots <= 0 || delta.is_nan() {
                return Err(Stop::Refuse(format!(
                    "-tfceS: the threshold step {delta} (FSL-174: FSL crashes here)"
                )));
            }
        }
        // the thresholds from h0 while h < M, each with the voxel's cluster size
        let mut hs: Vec<f32> = Vec::new();
        let mut h = set.min_t;
        while h < m {
            if hs.len() as i64 >= slots {
                n177 = true;
                if !fix {
                    // FSL-174, FSL-177: FSL's arrays are full
                    return Err(Stop::Exception(abort_lines(
                        "std::out_of_range",
                        "Mat::operator(): index out of bounds",
                    )));
                }
            }
            let next = h + delta;
            if next == h {
                if fix {
                    notices.add(
                        &defects::FSL_174,
                        format!("-tfceS with thresholds that cannot advance (from {} by {delta}) is refused (FSL fills its arrays and stops)", set.min_t),
                    );
                    return Err(Stop::Refuse(format!(
                        "-tfceS: the threshold {h} plus the step {delta} stays {h} (FSL-174)"
                    )));
                }
                // reproduce: the arrays fill up, then FSL's abort
                return Err(Stop::Exception(abort_lines(
                    "std::out_of_range",
                    "Mat::operator(): index out of bounds",
                )));
            }
            hs.push(h);
            h = next;
        }
        if !hs.is_empty() && slots <= 0 && !fix {
            return Err(Stop::Refuse(
                "-tfceS: FSL's arrays have no element (FSL-177: FSL divides by zero here)"
                    .to_owned(),
            ));
        }
        if !outside && !hs.is_empty() && !(m <= f32::MAX) {
            // an infinite voxel belongs to no cluster: Armadillo's index error (both modes)
            return Err(Stop::Exception(abort_lines(
                "std::out_of_range",
                "Mat::operator(): index out of bounds",
            )));
        }
        let sizes: Vec<usize> = hs
            .iter()
            .map(|&h| {
                if outside {
                    0
                } else {
                    cluster(&v, dims, at, h, c).len()
                }
            })
            .collect();
        // from the highest threshold down until the sum reaches T
        let mut acc = 0.0f32;
        let mut reached = None;
        for k in (0..hs.len()).rev() {
            let s = math::powf(sizes[k] as f32, e_exp);
            let p = math::powf(hs[k], h_exp);
            acc += num::nan32(s, p, s * p);
            if acc >= target {
                reached = Some(k);
                break;
            }
        }
        if reached.is_none() {
            let _ = writeln!(out, "{NOT_REACHED}");
        }
        let hk = reached.map_or(set.min_t, |k| hs[k]);
        let keep = if outside || !(m >= hk && m <= f32::MAX) {
            background(&v, hk)
        } else {
            cluster(&v, dims, at, hk, c)
        };
        let mut o = vec![T::zero(); per];
        for k in keep {
            o[k] = T::from_f64_c(num::to_f64(v[k]));
        }
        result.extend(o);
    }
    if n177 {
        notices.add(
            &defects::FSL_177,
            "-tfceS: arrays as long as the thresholds taken (FSL's are shorter: it stops)",
        );
    }
    img.data = result;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connectivities() {
        assert_eq!(offsets(6).len(), 6);
        assert_eq!(offsets(18).len(), 18);
        assert_eq!(offsets(26).len(), 26);
        assert_eq!(offsets(7).len(), 6);
        assert!(offsets(5).is_empty());
    }

    #[test]
    fn one_cluster_one_threshold() {
        // a 3 x 3 x 3 interior voxel: one cluster of 1 at h = 0: pow(1, E) * powf(0, H) = 0;
        // at h = 1: the voxel of 2 above it adds 1 * 1
        let mut v = vec![0.0f32; 125];
        v[62] = 2.0;
        let e = enhance(&v, [5, 5, 5], &[0.0, 1.0], 2.0, 0.5, &offsets(6));
        assert_eq!(e[62], 1.0);
        assert_eq!(e.iter().filter(|&&x| x != 0.0).count(), 1);
    }
}
