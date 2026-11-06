// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-fugue 2603.0 (unwarpfns.{h,cc}: maximum_position,
// basic_mask_threshold, make_basic_head_mask, largest_connected_component, make_head_mask,
// make_head_mask2D, wrap, estimate_linear_ramps, remove_linear_ramps, restore_linear_ramps,
// the constraint map (getP, find_maxC, calc_constraint, make_constraints), apply_unwrapping,
// find_phase_labels, find_phase_labels2D, unwrap, unwrap2D). PRELUDE: Mark Jenkinson, FMRIB
// Image Analysis Group; Copyright (C) 2000-2001 University of Oxford. Rust translation Copyright
// 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL Licence (see
// LICENSE).
//! The unwrapping routines of FSL's `unwarpfns` that `prelude` calls, on 3D volumes.
//!
//! Each routine follows its C++ statement by statement, with C's types: `float` values stay
//! `f32`, an expression with a `double` literal or a `double` operand (`M_PI`, `0.5`, a NEWMAT
//! element) is computed in `f64` and rounded when it is stored; NaNs are SSE's ([`S32`],
//! [`S64`]); integer conversions are x86-64's ([`cnum`]).
//!
//! [`unwrap`] is FSL's constraint-merging algorithm with the same arithmetic in the same order,
//! organised for speed: FSL scans every label for every merge (O(L²) for L labels); here each
//! label keeps its neighbours and members, and the largest constraint is kept in an ordered set,
//! which visits the same constraints in the same order (the test `unwrap_matches_fsl_loops`
//! compares it with a statement-by-statement translation, [`unwrap_reference`]).

use std::collections::{BTreeMap, BTreeSet};

use larmorx_nc_core::cnum;
use larmorx_nc_core::filters::connected_components;
use larmorx_nc_core::newimagefns;
use larmorx_nc_core::sse::{S32, S64};
use larmorx_nc_core::stdsort::{self, Outside};
use larmorx_nc_core::volume::{Extrapolation, Volume, round_f64};
use larmorx_nc_core::vstats;

const PI: f64 = std::f64::consts::PI;

/// `MISCMATHS::round(float x)`: in `double`, half away from zero, truncated as x86-64 does.
#[inline]
pub fn round_f32(x: f32) -> i32 {
    round_f64(f64::from(x))
}

/// `wrap(theta)`: `theta - round(theta / 2pi) * 2pi` in `float`, with `1/(2pi)` and `2pi` as
/// `float` constants.
#[inline]
pub fn wrap(theta: f32) -> f32 {
    const ONEONTWOPI: f32 = (1.0 / (2.0 * PI)) as f32;
    const TWOPI: f32 = (2.0 * PI) as f32;
    let k = round_f32((S32(theta) * ONEONTWOPI).0);
    (S32(theta) - S32(k as f32) * TWOPI).0
}

/// `maximum_position(vec)` of a histogram: the first position (1-based) of the largest value,
/// compared with the largest so far held as a `float`; -1 for an empty vector.
pub fn maximum_position(vec: &[f64]) -> i64 {
    if vec.is_empty() {
        return -1;
    }
    let mut idx = 1i64;
    let mut max = vec[0] as f32;
    for (n, &v) in vec.iter().enumerate().skip(1) {
        if v > f64::from(max) {
            max = v as f32;
            idx = n as i64 + 1;
        }
    }
    idx
}

// ------------------------------------------------------------------------------------------------
// Masks

/// `basic_mask_threshold(absmap)`: `0.1 * (p98 - p2) + p2` of every value (`percentile`: the
/// values sorted by libstdc++'s `std::sort`). With `without_nan` (FSL-269's correction) NaN
/// values are left out of the sort; otherwise a NaN that sends libstdc++'s sort outside the
/// values is `Err(Outside)` (C++ reads memory it does not own).
pub fn basic_mask_threshold(absmap: &Volume, without_nan: bool) -> Result<f32, Outside> {
    let mut hist: Vec<f32> = if without_nan {
        absmap
            .data
            .iter()
            .copied()
            .filter(|v| !v.is_nan())
            .collect()
    } else {
        absmap.data.clone()
    };
    // each percentile sorts the same values: one sort gives both
    let t2 = vstats::percentile(&mut hist, 0.02)?;
    let n = hist.len();
    let t98 = if n == 0 {
        0.0
    } else {
        hist[vstats::percentile_index(n, 0.98)]
    };
    let d = (S32(t98) - t2).0;
    Ok((S64(0.1) * f64::from(d) + f64::from(t2)).0 as f32)
}

/// Whether `absmap`'s values hold a NaN (FSL-269's condition).
pub fn has_nan(v: &Volume) -> bool {
    v.data.iter().any(|x| x.is_nan())
}

/// `make_basic_head_mask(absmap, thresh)`: `binarise(absmap, thresh)`, 1 for the values in
/// `[thresh, max]` (with `without_nan`, FSL-269's correction, the maximum of the values that
/// are not NaN; FSL's maximum is NaN when the first value is).
pub fn make_basic_head_mask(absmap: &Volume, thresh: f32, without_nan: bool) -> Volume {
    if without_nan {
        let mut out = absmap.clone();
        let upper = vstats::extrema_without_nan(&absmap.data).map_or(0.0, |e| e.max);
        newimagefns::binarise_range(&mut out, thresh, upper);
        out
    } else {
        newimagefns::binarise(absmap, thresh)
    }
}

/// `largest_connected_component(mask)`: the 6-connected component with the most voxels (the
/// first of the largest), as 1; all zeros when there is none. The result carries `mask`'s
/// properties with the extrapolation `extraslice` (FSL sets it on the mask before labelling).
pub fn largest_connected_component(mask: &Volume) -> Volume {
    let mut work = mask.clone();
    work.extrap = Extrapolation::Extraslice;
    let dims = [mask.dims[0], mask.dims[1], mask.dims[2]];
    let (labels, _) = connected_components(&mask.data, dims, 6);
    let lmax = labels.iter().copied().max().unwrap_or(0);
    let first = mask.data.first().copied().unwrap_or(0.0);
    if lmax <= 0 && first == 0.0 {
        // mask * 0.0f
        for v in &mut work.data {
            *v = (S32(*v) * 0.0).0;
        }
        return work;
    }
    // label.histogram(max+1, 0, max+1): one bin per label (lmax <= 0 with a nonzero first
    // voxel: one bin, every voxel's)
    let nbins = (lmax.max(0) + 1) as usize;
    let mut hist = vec![0.0f64; nbins];
    if lmax + 1 > 0 {
        for &l in &labels {
            let b = (l.max(0) as usize).min(nbins - 1);
            hist[b] += 1.0;
        }
    }
    hist[0] = 0.0;
    let brainidx = (maximum_position(&hist) - 1) as i32;
    for (v, &l) in work.data.iter_mut().zip(&labels) {
        *v = if l == brainidx { 1.0 } else { 0.0 };
    }
    work
}

/// `make_head_mask(absmap, thresh)`.
pub fn make_head_mask(absmap: &Volume, thresh: f32, without_nan: bool) -> Volume {
    largest_connected_component(&make_basic_head_mask(absmap, thresh, without_nan))
}

/// Slice `z` of a 3D volume as a volume of one slice (`ROI(minx, miny, z, 0, maxx, maxy, z, 0)`:
/// the volume's own extent in x and y, its values read by its `const` access, so a slice beyond
/// it holds its extrapolation (a mask smaller than the phase: FSL-271); the properties copied).
pub fn slice_of<T: larmorx_nc_core::volume::Elem>(v: &Volume<T>, z: i64) -> Volume<T> {
    let (nx, ny) = (v.dims[0], v.dims[1]);
    let n = (nx * ny) as usize;
    let mut s = Volume::<T>::new(nx, ny, 1, 1);
    s.copy_properties(v);
    if (0..v.dims[2]).contains(&z) {
        let at = z as usize * n;
        s.data.copy_from_slice(&v.data[at..at + n]);
    } else {
        for y in 0..ny {
            for x in 0..nx {
                s.data[(y * nx + x) as usize] = v.get(x, y, z);
            }
        }
    }
    s
}

/// Whether voxel `i` of `ph`'s grid is inside `mask` (read by `ph`'s indices).
pub fn inside_at(mask: &Volume, ph: &Volume, i: usize) -> bool {
    let (nx, ny) = (ph.dims[0] as usize, ph.dims[1] as usize);
    let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
    inside(mask, x as i64, y as i64, z as i64)
}

/// `copyROI(slice, ..., z, ...)`: the slice's values into slice `z`.
pub fn set_slice<T: larmorx_nc_core::volume::Elem>(v: &mut Volume<T>, z: i64, s: &Volume<T>) {
    let n = (v.dims[0] * v.dims[1]) as usize;
    let at = z as usize * n;
    v.data[at..at + n].copy_from_slice(&s.data[..n]);
}

/// `make_head_mask2D(absmap, thresh)`: each slice's own mask (its maximum, its largest
/// 4-connected component), with `absmap`'s properties.
pub fn make_head_mask2d(absmap: &Volume, thresh: f32, without_nan: bool) -> Volume {
    let mut mask = absmap.clone();
    mask.data.fill(0.0);
    for z in 0..absmap.dims[2] {
        let aslice = slice_of(absmap, z);
        let mslice = make_head_mask(&aslice, thresh, without_nan);
        set_slice(&mut mask, z, &mslice);
    }
    mask
}

// ------------------------------------------------------------------------------------------------
// Linear ramps

/// The loops' end along each axis: FSL's run to the last index exclusive (FSL-261); the
/// correction includes it.
fn ramp_ends(v: &Volume, fsl: bool) -> [i64; 3] {
    let e = |n: i64| if fsl { n - 1 } else { n };
    [e(v.dims[0]), e(v.dims[1]), e(v.dims[2])]
}

/// `mask(x, y, z) > 0.5` with the mask's extrapolation outside it (FSL's `const` access).
#[inline]
fn inside(mask: &Volume, x: i64, y: i64, z: i64) -> bool {
    f64::from(mask.get(x, y, z)) > 0.5
}

/// `estimate_linear_ramps(ph, mask)`: the mean wrapped forward difference along each axis over
/// the pairs inside the mask. With `fsl` (FSL-261) the voxels of the last column, row and slice
/// are never visited.
pub fn estimate_linear_ramps(ph: &Volume, mask: &Volume, fsl: bool) -> [f64; 3] {
    let mut ramp = [S64(0.0); 3];
    let (mut nx, mut ny, mut nz) = (0i64, 0i64, 0i64);
    let [ex, ey, ez] = ramp_ends(ph, fsl);
    for z in 0..ez {
        for y in 0..ey {
            for x in 0..ex {
                if !inside(mask, x, y, z) {
                    continue;
                }
                let p = ph.value(x, y, z);
                if ph.in_bounds(x + 1, y, z) && inside(mask, x + 1, y, z) {
                    ramp[0] += f64::from(wrap((S32(ph.value(x + 1, y, z)) - p).0));
                    nx += 1;
                }
                if ph.in_bounds(x, y + 1, z) && inside(mask, x, y + 1, z) {
                    ramp[1] += f64::from(wrap((S32(ph.value(x, y + 1, z)) - p).0));
                    ny += 1;
                }
                if ph.in_bounds(x, y, z + 1) && inside(mask, x, y, z + 1) {
                    ramp[2] += f64::from(wrap((S32(ph.value(x, y, z + 1)) - p).0));
                    nz += 1;
                }
            }
        }
    }
    for (r, n) in ramp.iter_mut().zip([nx, ny, nz]) {
        if n > 0 {
            *r /= f64::from(n as f32);
        }
    }
    [ramp[0].0, ramp[1].0, ramp[2].0]
}

/// The centre of the volume along each axis, `(max + min) / 2.0` stored as `float`.
fn centre(v: &Volume) -> [f32; 3] {
    let c = |n: i64| (f64::from((n - 1) as i32) / 2.0) as f32;
    [c(v.dims[0]), c(v.dims[1]), c(v.dims[2])]
}

/// `ramp(1)*(x-xav) + ...`: each term `double * float`.
#[inline]
fn ramp_terms(ramp: &[f64; 3], c: &[f32; 3], x: i64, y: i64, z: i64) -> [S64; 3] {
    let d = |i: i64, a: f32| f64::from((S32(i as f32) - a).0);
    [
        S64(ramp[0]) * d(x, c[0]),
        S64(ramp[1]) * d(y, c[1]),
        S64(ramp[2]) * d(z, c[2]),
    ]
}

/// `remove_linear_ramps(ramp, ph, mask)`: inside the mask, `ph = wrap(ph - ramp·(r - centre))`
/// (in `double`, then `float`). With `fsl`, FSL-261's loops.
pub fn remove_linear_ramps(ramp: &[f64; 3], ph: &mut Volume, mask: &Volume, fsl: bool) {
    let c = centre(ph);
    let [ex, ey, ez] = ramp_ends(ph, fsl);
    for z in 0..ez {
        for y in 0..ey {
            for x in 0..ex {
                if inside(mask, x, y, z) {
                    let [a, b, d] = ramp_terms(ramp, &c, x, y, z);
                    let v = ((S64(f64::from(ph.value(x, y, z))) - a) - b) - d;
                    ph.set(x, y, z, wrap(v.0 as f32));
                }
            }
        }
    }
}

/// `restore_linear_ramps(ramp, uph, mask)`: inside the mask, `uph += ramp·(r - centre)` (the
/// terms summed in `double`, added to the value in `double`, then `float`). With `fsl`,
/// FSL-261's loops.
pub fn restore_linear_ramps(ramp: &[f64; 3], uph: &mut Volume, mask: &Volume, fsl: bool) {
    let c = centre(uph);
    let [ex, ey, ez] = ramp_ends(uph, fsl);
    for z in 0..ez {
        for y in 0..ey {
            for x in 0..ex {
                if inside(mask, x, y, z) {
                    let [a, b, d] = ramp_terms(ramp, &c, x, y, z);
                    let v = S64(f64::from(uph.value(x, y, z))) + ((a + b) + d);
                    uph.set(x, y, z, v.0 as f32);
                }
            }
        }
    }
}

/// Whether FSL-261 changes what the ramps touch: a voxel of the mask in the last column, row or
/// slice (FSL never visits them; one slice: every voxel).
pub fn ramps_skip_voxels(ph: &Volume, mask: &Volume) -> bool {
    let (nx, ny, nz) = (ph.dims[0], ph.dims[1], ph.dims[2]);
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if (x == nx - 1 || y == ny - 1 || z == nz - 1) && inside(mask, x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

// ------------------------------------------------------------------------------------------------
// Labels

/// `find_phase_labels(phasemap, mask, n)`: inside the mask, the phase's bin number
/// `(int)((phase + pi) / binsize) + 1` (clamped to `[1, n]`, `binsize = 2pi/n` as `float`), 0
/// outside; the 6-connected components of equal bins. The labels carry `phasemap`'s properties.
pub fn find_phase_labels(phasemap: &Volume, mask: &Volume, n: i32) -> Volume<i32> {
    let binsize = ((2.0 * PI) / f64::from(n as f32)) as f32;
    let mut phase = phasemap.clone();
    let (nx, ny, nz) = (phase.dims[0], phase.dims[1], phase.dims[2]);
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let i = phase.idx(x, y, z);
                if inside(mask, x, y, z) {
                    let q = (S64(f64::from(phase.data[i])) + PI) / f64::from(binsize);
                    let mut binno = cnum::f64_to_i32(q.0).wrapping_add(1);
                    if binno < 1 {
                        binno = 1;
                    }
                    if binno > n {
                        binno = n;
                    }
                    phase.data[i] = binno as f32;
                } else {
                    phase.data[i] = 0.0;
                }
            }
        }
    }
    let (labels, _) = connected_components(&phase.data, [nx, ny, nz], 6);
    let mut out = Volume::<i32>::copyconvert(&phase);
    out.data = labels;
    out
}

/// `find_phase_labels2D(phasemap, mask, n, unique3Dlabels)`: each slice's labels (4-connected
/// in its plane); with `unique3Dlabels` each slice's numbered on from the previous slice's
/// largest label. FSL takes that largest label from the slice itself, 0 for a slice without
/// labels, so the slices after an empty one would reuse earlier numbers; prelude's masks are one
/// 3D-connected component, so no empty slice lies between labelled ones and this never happens
/// (`docs/findings/fsl-prelude.md`).
pub fn find_phase_labels2d(
    phasemap: &Volume,
    mask: &Volume,
    n: i32,
    unique3dlabels: bool,
) -> Volume<i32> {
    let mut label = Volume::<i32>::copyconvert(phasemap);
    label.data.fill(0);
    let mut max_label = 0i32;
    for z in 0..phasemap.dims[2] {
        let pslice = slice_of(phasemap, z);
        let mslice = slice_of(mask, z);
        let mut lslice = find_phase_labels(&pslice, &mslice, n);
        if unique3dlabels {
            // lsmask = binarise(lslice, 1): [1, lslice.max()]
            let lmax = lslice.extrema().0;
            for l in &mut lslice.data {
                let m = i32::from(*l >= 1 && *l <= lmax);
                *l = l.wrapping_add(max_label).wrapping_mul(m);
            }
            max_label = lslice.extrema().0;
        }
        set_slice(&mut label, z, &lslice);
    }
    label
}

// ------------------------------------------------------------------------------------------------
// The constraints and their merging

/// `sconstraint`: `C` (the merge priority), `N` (the number of voxel pairs), `P` (their summed
/// phase difference), `K` (the local offset estimate), all `float`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraint {
    pub c: f32,
    pub n: f32,
    pub p: f32,
    pub k: f32,
}

/// `calc_constraint(con)`: `K = -P / (2pi N)` (0 when `N` is not positive), `C = N (0.5 -
/// |round(K) - K|)`, each computed in `double` and stored as `float`.
pub fn calc_constraint(con: &mut Constraint) {
    let n = con.n;
    let p = con.p;
    let k = if n > 0.0 {
        (S64(f64::from(-p)) / (S64(2.0 * PI) * f64::from(n))).0 as f32
    } else {
        0.0
    };
    con.k = k;
    let off = (S32(round_f32(k) as f32) - k).0.abs();
    con.c = (S64(f64::from(n)) * (S64(0.5) - f64::from(off))).0 as f32;
}

/// `make_constraints(phasemap, label, conmap)`: for each labelled voxel (z, then y, then x)
/// and its neighbours at x-1, y-1 and z-1 with another nonzero label, the pair (smaller label,
/// larger label) counts one more voxel pair and adds the smaller label's phase minus the
/// larger's; then each constraint's `K` and `C`.
pub fn make_constraints(
    phasemap: &Volume,
    label: &Volume<i32>,
) -> BTreeMap<(i32, i32), Constraint> {
    let mut conmap: BTreeMap<(i32, i32), Constraint> = BTreeMap::new();
    let (nx, ny, nz) = (label.dims[0], label.dims[1], label.dims[2]);
    // label: zeropad; phasemap: its own extrapolation (read only where the label is nonzero)
    let lab = |x: i64, y: i64, z: i64| {
        if label.in_bounds(x, y, z) {
            label.value(x, y, z)
        } else {
            0
        }
    };
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let l1 = label.value(x, y, z);
                if l1 == 0 {
                    continue;
                }
                let p1 = phasemap.get(x, y, z);
                for (dx, dy, dz) in [(-1, 0, 0), (0, -1, 0), (0, 0, -1)] {
                    let l2 = lab(x + dx, y + dy, z + dz);
                    if l2 != 0 && l1 != l2 {
                        let p2 = phasemap.get(x + dx, y + dy, z + dz);
                        let (mut v1, mut v2, mut pp1, mut pp2) = (l1, l2, p1, p2);
                        if v2 < v1 {
                            std::mem::swap(&mut v1, &mut v2);
                            std::mem::swap(&mut pp1, &mut pp2);
                        }
                        let d = (S32(pp1) - pp2).0;
                        conmap
                            .entry((v1, v2))
                            .and_modify(|c| {
                                c.n = (S32(c.n) + 1.0).0;
                                c.p = (S32(c.p) + d).0;
                            })
                            .or_insert(Constraint {
                                c: 0.0,
                                n: 1.0,
                                p: d,
                                k: 0.0,
                            });
                    }
                }
            }
        }
    }
    for c in conmap.values_mut() {
        calc_constraint(c);
    }
    conmap
}

/// `apply_unwrapping(phasemap, label, offsets)`: `phase + 2pi offset` (in `double`, then
/// `float`) where the label is positive, 0 elsewhere.
pub fn apply_unwrapping(phasemap: &Volume, label: &Volume<i32>, offsets: &[f64]) -> Volume {
    let mut uphase = phasemap.clone();
    let lmax = label.extrema().0;
    if (offsets.len() as i64) < i64::from(lmax) {
        return uphase;
    }
    for (u, &lab) in uphase.data.iter_mut().zip(&label.data) {
        if lab > 0 {
            let add = S64(2.0 * PI) * offsets[(lab - 1) as usize];
            *u = (S64(f64::from(*u)) + add).0 as f32;
        } else {
            *u = 0.0;
        }
    }
    uphase
}

/// A constraint's place in the order of [`find_maxc`]: the largest `C` first (C's `>`: `-0` and
/// `0` equal), then the smallest pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank(std::cmp::Reverse<u32>, (i32, i32));

/// An order on the values of `C` that are not NaN agreeing with `<` (`-0` as `0`).
fn c_order(c: f32) -> u32 {
    let c = if c == 0.0 { 0.0f32 } else { c };
    let b = c.to_bits();
    if b & 0x8000_0000 != 0 {
        !b
    } else {
        b | 0x8000_0000
    }
}

/// The constraint map and the indexes [`unwrap`] keeps beside it.
struct Map {
    cons: BTreeMap<(i32, i32), Constraint>,
    /// The constraints whose `C` is not NaN, by [`Rank`].
    ranked: BTreeSet<Rank>,
    /// Each label's neighbours (the other label of each of its constraints).
    adj: Vec<BTreeSet<i32>>,
}

impl Map {
    fn insert(&mut self, key: (i32, i32), con: Constraint) {
        if let Some(old) = self.cons.insert(key, con) {
            self.unrank(key, old.c);
        }
        if !con.c.is_nan() {
            self.ranked
                .insert(Rank(std::cmp::Reverse(c_order(con.c)), key));
        }
        self.adj[key.0 as usize].insert(key.1);
        self.adj[key.1 as usize].insert(key.0);
    }

    fn unrank(&mut self, key: (i32, i32), c: f32) {
        if !c.is_nan() {
            self.ranked
                .remove(&Rank(std::cmp::Reverse(c_order(c)), key));
        }
    }

    fn remove(&mut self, key: (i32, i32)) -> Option<Constraint> {
        let con = self.cons.remove(&key)?;
        self.unrank(key, con.c);
        self.adj[key.0 as usize].remove(&key.1);
        self.adj[key.1 as usize].remove(&key.0);
        Some(con)
    }

    /// `find_maxC(conmap)`: the first constraint (in key order) whose `C` is the largest; FSL
    /// starts from the first constraint's `C` and replaces it only by a larger one, so a NaN
    /// first `C` is never replaced and NaN values elsewhere are passed over.
    fn find_maxc(&self) -> Option<(i32, i32)> {
        let (&first, con) = self.cons.iter().next()?;
        if con.c.is_nan() {
            return Some(first);
        }
        self.ranked.iter().next().map(|r| r.1)
    }
}

/// `getP(it, r, s)`: the constraint's `P` seen from `r` (negated when `r` is the larger label).
#[inline]
fn get_p(con: &Constraint, r: i32, s: i32) -> f32 {
    if r < s { con.p } else { -con.p }
}

/// What [`unwrap`] reports with `-v`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnwrapLog {
    pub lines: Vec<String>,
}

/// The labels' offsets (in multiples of 2pi) and the number of merges, from the constraints.
/// FSL's `unwrap` between `make_constraints` and `apply_unwrapping`.
fn merge_offsets(
    conmap: BTreeMap<(i32, i32), Constraint>,
    nclass: i32,
    verbose: bool,
    log: &mut UnwrapLog,
) -> Vec<f64> {
    let nc = nclass.max(0) as usize;
    // the area with the biggest interface
    let mut q = vec![0.0f32; nc + 1];
    for (&(a, b), c) in &conmap {
        q[a as usize] = (S32(q[a as usize]) + c.n).0;
        q[b as usize] = (S32(q[b as usize]) + c.n).0;
    }
    let mut bigclassnum = 1usize;
    for n in 1..=nc {
        if q[bigclassnum] < q[n] {
            bigclassnum = n;
        }
    }
    let mut dvec = vec![0i32; nc + 1];
    let mut members: Vec<Vec<i32>> = (0..=nc as i32).map(|k| vec![k]).collect();
    let mut map = Map {
        cons: BTreeMap::new(),
        ranked: BTreeSet::new(),
        adj: vec![BTreeSet::new(); nc + 1],
    };
    for (k, c) in conmap {
        map.insert(k, c);
    }
    let mut iterations = 0i64;
    while let Some(key) = map.find_maxc() {
        iterations += 1;
        if iterations % 1000 == 1 && verbose {
            log.lines
                .push(format!("{} constraints left", map.cons.len()));
        }
        let (r, s) = key;
        let con = map.remove(key).expect("present");
        let krs = round_f32(con.k);
        // merge s into r (r < s: the map's keys are ordered pairs)
        let moved = std::mem::take(&mut members[s as usize]);
        for &p in &moved {
            dvec[p as usize] = dvec[p as usize].wrapping_sub(krs);
        }
        members[r as usize].extend(moved);
        let ts: Vec<i32> = map.adj[s as usize].iter().copied().collect();
        for t in ts {
            let kst = (s.min(t), s.max(t));
            let st = *map.cons.get(&kst).expect("neighbour");
            let pst = get_p(&st, s, t);
            let nst = st.n;
            let krt = (r.min(t), r.max(t));
            let (prt, nrt) = match map.remove(krt) {
                Some(rt) => (get_p(&rt, r, t), rt.n),
                None => (0.0, 0.0),
            };
            map.remove(kst);
            let sum = (S32(prt) + pst).0;
            let prt2 = (S64(f64::from(sum)) - ((S64(2.0 * PI) * f64::from(nst)) * f64::from(krs))).0
                as f32;
            let mut newcon = Constraint {
                c: 0.0,
                n: (S32(nrt) + nst).0,
                p: if r < t { prt2 } else { -prt2 },
                k: 0.0,
            };
            calc_constraint(&mut newcon);
            map.insert(krt, newcon);
        }
    }
    if verbose {
        log.lines.push(format!("Did while loop {iterations} times"));
    }
    let big = dvec.get(bigclassnum).copied().unwrap_or(0);
    (1..=nc)
        .map(|n| f64::from((S32(dvec[n] as f32) - big as f32).0))
        .collect()
}

/// `unwrap(phasemap, label, verbose)`: the constraints between labels merged, largest `C`
/// first, into one offset per label; the phase unwrapped by them (0 outside the labels).
pub fn unwrap(phasemap: &Volume, label: &Volume<i32>, verbose: bool) -> (Volume, UnwrapLog) {
    let mut log = UnwrapLog::default();
    let nclass = label.extrema().0;
    if verbose {
        log.lines.push(format!(
            "Calculating starting matrices ({nclass} by {nclass})"
        ));
    }
    let conmap = make_constraints(phasemap, label);
    if verbose {
        log.lines.push("Finished connection_matrices".to_owned());
    }
    let offsets = merge_offsets(conmap, nclass, verbose, &mut log);
    (apply_unwrapping(phasemap, label, &offsets), log)
}

/// `unwrap2D(phasemap, label, verbose)`: each slice unwrapped on its own (its labels), then each
/// slice from the second on moved by the multiple of 2pi nearest to the median of its
/// differences from the slice below over the voxels labelled in both (the upper median: the
/// value at half the count of the sorted differences). `Err(Outside)` when NaN differences
/// send libstdc++'s sort outside them.
pub fn unwrap2d(
    phasemap: &Volume,
    label: &Volume<i32>,
    verbose: bool,
) -> Result<(Volume, UnwrapLog), Outside> {
    let mut log = UnwrapLog::default();
    let mut uphase = phasemap.clone();
    uphase.data.fill(0.0);
    let nz = phasemap.dims[2];
    for z in 0..nz {
        if verbose {
            log.lines.push(format!("SLICE NUMBER {z}"));
        }
        let pslice = slice_of(phasemap, z);
        let lslice = slice_of(label, z);
        let (uslice, l) = unwrap(&pslice, &lslice, verbose);
        log.lines.extend(l.lines);
        set_slice(&mut uphase, z, &uslice);
    }
    let (nx, ny) = (label.dims[0], label.dims[1]);
    let mut diffvec: Vec<f32> = Vec::new();
    for z in 0..nz - 1 {
        diffvec.clear();
        for y in 0..ny {
            for x in 0..nx {
                if label.value(x, y, z) > 0 && label.value(x, y, z + 1) > 0 {
                    diffvec.push((S32(uphase.value(x, y, z + 1)) - uphase.value(x, y, z)).0);
                }
            }
        }
        if !diffvec.is_empty() {
            stdsort::sort_checked(&mut diffvec)?;
            let median_diff = diffvec[diffvec.len() / 2];
            let m = round_f64((S64(f64::from(median_diff)) / (2.0 * PI)).0);
            let shift = S64(f64::from(m) * 2.0) * PI;
            for y in 0..ny {
                for x in 0..nx {
                    if label.value(x, y, z + 1) > 0 {
                        let v = S64(f64::from(uphase.value(x, y, z + 1))) - shift;
                        uphase.set(x, y, z + 1, v.0 as f32);
                    }
                }
            }
        }
    }
    Ok((uphase, log))
}

/// `unwrap` translated statement by statement (FSL's loops over every label for every merge):
/// the reference [`unwrap`] is tested against.
pub fn unwrap_reference(phasemap: &Volume, label: &Volume<i32>) -> Volume {
    let mut conmap = make_constraints(phasemap, label);
    let nclass = label.extrema().0;
    let nc = nclass.max(0) as usize;
    let mut q = vec![0.0f32; nc + 1];
    for (&(a, b), c) in &conmap {
        q[a as usize] = (S32(q[a as usize]) + c.n).0;
        q[b as usize] = (S32(q[b as usize]) + c.n).0;
    }
    let mut bigclassnum = 1usize;
    for n in 1..=nc {
        if q[bigclassnum] < q[n] {
            bigclassnum = n;
        }
    }
    let mut mvec: Vec<i32> = (0..=nc as i32).collect();
    let mut dvec = vec![0i32; nc + 1];
    let find = |m: &BTreeMap<(i32, i32), Constraint>, r: i32, s: i32| {
        let k = (r.min(s), r.max(s));
        m.get(&k).map(|_| k)
    };
    while !conmap.is_empty() {
        // find_maxC
        let mut it = conmap.iter();
        let (&k0, c0) = it.next().unwrap();
        let (mut itm, mut maxc) = (k0, c0.c);
        for (&k, c) in conmap.iter() {
            if c.c > maxc {
                maxc = c.c;
                itm = k;
            }
        }
        let (r, s) = itm;
        let krs = round_f32(conmap[&itm].k);
        conmap.remove(&itm);
        for p in 1..=nc {
            if mvec[p] == s {
                mvec[p] = r;
                dvec[p] = dvec[p].wrapping_sub(krs);
            }
        }
        for t in 1..=nclass {
            if let Some(kst) = find(&conmap, s, t) {
                let st = conmap[&kst];
                let pst = get_p(&st, s, t);
                let nst = st.n;
                let (mut prt, mut nrt) = (0.0f32, 0.0f32);
                if let Some(krt) = find(&conmap, r, t) {
                    let rt = conmap[&krt];
                    prt = get_p(&rt, r, t);
                    nrt = rt.n;
                    conmap.remove(&krt);
                }
                conmap.remove(&kst);
                let sum = (S32(prt) + pst).0;
                let prt2 = (S64(f64::from(sum))
                    - ((S64(2.0 * PI) * f64::from(nst)) * f64::from(krs)))
                .0 as f32;
                let mut newcon = Constraint {
                    c: 0.0,
                    n: (S32(nrt) + nst).0,
                    p: if r < t { prt2 } else { -prt2 },
                    k: 0.0,
                };
                calc_constraint(&mut newcon);
                conmap.insert((r.min(t), r.max(t)), newcon);
            }
        }
    }
    let big = dvec.get(bigclassnum).copied().unwrap_or(0);
    let offsets: Vec<f64> = (1..=nc)
        .map(|n| f64::from((S32(dvec[n] as f32) - big as f32).0))
        .collect();
    apply_unwrapping(phasemap, label, &offsets)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seeded generator of plain arithmetic (no transcendental functions).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
        fn unit(&mut self) -> f32 {
            (self.next() % 1_000_000) as f32 / 1_000_000.0
        }
    }

    fn phase_volume(nx: i64, ny: i64, nz: i64, seed: u64, slope: f32) -> Volume {
        let mut g = Lcg(seed);
        let mut v = Volume::new(nx, ny, nz, 1);
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let t = slope * (x as f32 + 0.7 * y as f32 + 0.3 * z as f32)
                        + 0.4 * (g.unit() - 0.5);
                    v.set(x, y, z, wrap(t));
                }
            }
        }
        v
    }

    #[test]
    fn wrap_follows_float_arithmetic() {
        assert_eq!(wrap(0.0), 0.0);
        assert_eq!(wrap(7.0), 7.0 - (2.0 * PI) as f32);
        assert!(wrap(f32::NAN).is_nan());
        assert_eq!(wrap(f32::INFINITY), f32::INFINITY);
    }

    #[test]
    fn unwrap_matches_fsl_loops() {
        for (seed, slope, dims) in [
            (1u64, 0.9f32, (9i64, 8i64, 5i64)),
            (2, 1.7, (12, 10, 3)),
            (3, 2.9, (7, 7, 7)),
            (4, 0.3, (16, 5, 1)),
        ] {
            let ph = phase_volume(dims.0, dims.1, dims.2, seed, slope);
            let mut mask = ph.clone();
            mask.data.fill(1.0);
            for n in [2, 3, 8] {
                let label = find_phase_labels(&ph, &mask, n);
                let fast = unwrap(&ph, &label, false).0;
                let slow = unwrap_reference(&ph, &label);
                let same = fast
                    .data
                    .iter()
                    .zip(&slow.data)
                    .all(|(a, b)| a.to_bits() == b.to_bits());
                assert!(same, "seed {seed} n {n}");
                let l2 = find_phase_labels2d(&ph, &mask, n, true);
                let fast = unwrap(&ph, &l2, false).0;
                let slow = unwrap_reference(&ph, &l2);
                assert!(
                    fast.data
                        .iter()
                        .zip(&slow.data)
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                );
            }
        }
    }

    /// FSL's numbering after a slice without labels starts again from 1 (unreachable through
    /// prelude, whose masks are 3D-connected: kept as FSL computes it).
    #[test]
    fn an_empty_slice_restarts_the_numbering() {
        let ph = phase_volume(6, 6, 3, 9, 1.1);
        let mut mask = ph.clone();
        for z in 0..3 {
            for y in 0..6 {
                for x in 0..6 {
                    mask.set(x, y, z, if z == 1 { 0.0 } else { 1.0 });
                }
            }
        }
        let l = find_phase_labels2d(&ph, &mask, 8, true);
        assert_eq!(l.value(0, 0, 2), l.value(0, 0, 0));
    }
}
