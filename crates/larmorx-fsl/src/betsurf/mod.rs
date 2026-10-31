// SPDX-License-Identifier: Apache-2.0
//! The inner skull, outer skull and scalp surfaces of FSL's `betsurf` (`specs/bet.md` §6; M.
//! Jenkinson, M. Pechaud, S. Smith, "BET2: MR-based estimation of brain, skull and scalp
//! surfaces", OHBM 2005): intensity profiles cast from the brain's surface along its normals
//! vote for the three boundaries in counting images; each surface is a mesh deformed onto its
//! smoothed votes, the outer ones held outside the inner ones by their masks.
//!
//! The steps, each a function here (the command line, `cli::betsurf`, runs them in FSL's order
//! with its messages and outputs between them):
//! - [`robust_maximum`] and [`clamp`]: the T1 limited to 1.1 times its 98th percentile inside
//!   the brain surface (§6.3);
//! - [`WorkGrid`]: the padded grid the votes and the deformation live on (§6.4);
//! - [`votes`]: the profiles and their estimates (§6.4, `profile`);
//! - [`smooth`]: the separable Gaussian of the votes (§6.5);
//! - [`deform`]: one phase of the deformation (§6.5);
//! - [`surface_mask`]: a surface drawn as 1 and filled from its centroid (§6.5, §6.6).
//!
//! Geometry is in millimetres along the voxel axes of the T1 in its radiological frame (§3.2),
//! as the brain's mesh bet2 writes; on the working grid the same, shifted by the padding.
//! Every vertex's step reads only the previous step's positions and every profile is cast on
//! its own, so both run in parallel with the same result at any thread count.

pub mod meshfile;
pub mod profile;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use rayon::prelude::*;

use crate::bet::mesh::{Mesh, V3, add, dist, dot, scale, sub};
use crate::bet::raster::{self, Rounding};
use crate::bet::skull::trilinear;
use crate::bet::{Vol, surface::c_int};
use profile::{Estimate, Profile};

/// The 4 x 4 matrix's first three rows (FLIRT's, from the T1's FSL coordinates in mm to
/// MNI152's), the only ones betsurf uses.
pub type Matrix = [[f64; 4]; 3];

/// Reads the matrix file's first 12 numbers, row by row, as C++'s `>>` reads them; with fewer
/// the missing ones are 0 and the count read is returned too (FSL-53).
pub fn read_matrix(text: &str) -> (Matrix, usize) {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    let mut m = [[0.0; 4]; 3];
    let mut read = 0;
    for (k, w) in words.iter().take(12).enumerate() {
        match parse_cxx_double(w) {
            Some((v, whole)) => {
                m[k / 4][k % 4] = v;
                read += 1;
                if !whole {
                    // the rest of this word fails the next read: nothing more is set
                    break;
                }
            }
            None => break,
        }
    }
    (m, read)
}

/// C++'s `>> double` on a word: the value of its longest numeric prefix and whether that was
/// the whole word; `None` when no prefix reads.
fn parse_cxx_double(w: &str) -> Option<(f64, bool)> {
    let r = meshfile::read(&format!("x\n1 0\n{w} 0 0 0\n"));
    let _ = r;
    // a small grammar of its own: sign, digits, fraction, exponent
    let b = w.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let d0 = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut n = i - d0;
    if i < b.len() && b[i] == b'.' {
        let f = i + 1;
        let mut j = f;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        n += j - f;
        i = j;
    }
    if n == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let e = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == e {
            return None;
        }
        i = j;
    }
    let v: f64 = w[..i].parse().ok()?;
    Some((v, i == b.len()))
}

/// Why betsurf cannot go on.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// A defect whose correction is a refusal (the notice says why).
    Defect(Notice),
}

/// The brain surface's mask on the T1's grid (§6.3): the surface drawn as 1 into 0s and filled
/// with 1 from its centroid. A centroid outside the image (FSL-43): the nearest voxel inside
/// (default) or FSL's fill by the linear index (`reproduce`); not a number: no fill.
pub fn surface_mask(
    mesh: &Mesh,
    points: &[V3],
    shape: [usize; 3],
    voxel: [f64; 3],
    rounding: Rounding,
    base: Vec<i32>,
    start_from: Option<V3>,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Vec<i32> {
    let mut grid = base;
    raster::draw(mesh, points, shape, voxel, rounding, |k| grid[k] = 1);
    let centroid = start_from.unwrap_or_else(|| centroid(points));
    let Some(start) = raster::voxel_of(centroid, voxel) else {
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_43,
                mode,
                "a surface's centroid is not a number: its mask is the drawn surface, not filled",
            ),
        );
        return grid;
    };
    let inside = (0..3).all(|a| start[a] >= 0 && (start[a] as usize) < shape[a]);
    if inside {
        fill_i32(&mut grid, shape, start.map(|x| x as usize));
    } else if mode.fix() {
        let s = [0, 1, 2].map(|a| start[a].clamp(0, shape[a] as i64 - 1) as usize);
        fill_i32(&mut grid, shape, s);
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_43,
                mode,
                format!(
                    "a mask's fill starts at the voxel inside the image nearest the surface's centroid ({start:?}, outside the image); FSL starts outside the volume"
                ),
            ),
        );
    } else {
        let beyond = fill_i32_by_index(&mut grid, shape, start);
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_43,
                mode,
                format!(
                    "a mask's fill starts outside the image at voxel {start:?}, and FSL's fill reaches places by their linear index ({beyond} reads beyond the volume, taken as not 0)"
                ),
            ),
        );
    }
    grid
}

/// The vertices' centroid, `(Σ p)·(1/V)` (§4.6).
pub fn centroid(points: &[V3]) -> V3 {
    let mut c = [0.0; 3];
    for p in points {
        c = add(c, *p);
    }
    scale(c, 1.0 / points.len() as f64)
}

/// Fills with 1, from `start` (set whatever it holds), the 6-connected voxels holding 0.
fn fill_i32(grid: &mut [i32], shape: [usize; 3], start: [usize; 3]) {
    let [nx, ny, nz] = shape;
    let slice = nx * ny;
    let s = (start[2] * ny + start[1]) * nx + start[0];
    grid[s] = 1;
    let mut stack = vec![s as u32];
    while let Some(p) = stack.pop() {
        let p = p as usize;
        let x = p % nx;
        let y = (p / nx) % ny;
        let z = p / slice;
        let mut visit = |q: usize, stack: &mut Vec<u32>| {
            if grid[q] == 0 {
                grid[q] = 1;
                stack.push(q as u32);
            }
        };
        if x > 0 {
            visit(p - 1, &mut stack);
        }
        if x + 1 < nx {
            visit(p + 1, &mut stack);
        }
        if y > 0 {
            visit(p - nx, &mut stack);
        }
        if y + 1 < ny {
            visit(p + nx, &mut stack);
        }
        if z > 0 {
            visit(p - slice, &mut stack);
        }
        if z + 1 < nz {
            visit(p + slice, &mut stack);
        }
    }
}

/// FSL's fill from a start outside the image (FSL-43 reproduced, as bet2's, §4.6): point by
/// point by the linear index, nothing written beyond the volume and every read there "not 0".
/// Returns the reads beyond the volume.
fn fill_i32_by_index(grid: &mut [i32], shape: [usize; 3], start: [i64; 3]) -> usize {
    let [nx, ny, nz] = shape.map(|n| n as i64);
    let total = grid.len() as i64;
    let place = |p: [i64; 3]| (p[2] * ny + p[1]) * nx + p[0];
    let mut beyond = 0usize;
    let mut stack = vec![start];
    while let Some(p) = stack.pop() {
        let at = place(p);
        if (0..total).contains(&at) {
            grid[at as usize] = 1;
        }
        let candidates = [
            ([p[0] - 1, p[1], p[2]], p[0] > 0),
            ([p[0], p[1] - 1, p[2]], p[1] > 0),
            ([p[0], p[1], p[2] - 1], p[2] > 0),
            ([p[0] + 1, p[1], p[2]], p[0] + 1 < nx),
            ([p[0], p[1] + 1, p[2]], p[1] + 1 < ny),
            ([p[0], p[1], p[2] + 1], p[2] + 1 < nz),
        ];
        for (q, within) in candidates {
            if !within {
                continue;
            }
            let at = place(q);
            if (0..total).contains(&at) {
                if grid[at as usize] == 0 {
                    stack.push(q);
                }
            } else {
                beyond += 1;
            }
        }
    }
    beyond
}

/// The T1's robust maximum (§6.3): 1.1 times the value of rank `ceil(0.98 N)` of its `N`
/// values inside the brain surface's mask. `None` when the mask has no voxel (FSL-58), or
/// holds a NaN (FSL-55) and `mode` is the default.
pub fn robust_maximum(
    t1: &Vol,
    mask: &[i32],
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<f64, Refusal> {
    let values: Vec<f32> = t1
        .data
        .iter()
        .zip(mask)
        .filter(|(_, m)| **m == 1)
        .map(|(v, _)| *v)
        .collect();
    if values.is_empty() {
        let n = Notice::new(
            &defects::FSL_58,
            mode,
            match mode {
                UpstreamDefects::Fix => {
                    "no voxel of the T1 lies inside the brain surface: refused (FSL reads the 98th percentile from an empty list)"
                }
                UpstreamDefects::Reproduce => {
                    "no voxel of the T1 lies inside the brain surface: the 98th percentile taken as 0 (FSL reads it from an empty list)"
                }
            },
        );
        if mode.fix() {
            return Err(Refusal::Defect(n));
        }
        notices.push(n);
        return Ok(0.0);
    }
    let rank = (0.98 * values.len() as f64).ceil() as usize;
    let k = rank.clamp(1, values.len()) - 1;
    let nan = values.iter().any(|v| v.is_nan());
    let v = if nan {
        let n = Notice::new(
            &defects::FSL_55,
            mode,
            match mode {
                UpstreamDefects::Fix => {
                    "the T1 holds a NaN inside the brain surface: refused (its 98th percentile is undefined in FSL)"
                }
                UpstreamDefects::Reproduce => {
                    "the T1 holds a NaN inside the brain surface: the 98th percentile is libstdc++'s selection's, as FSL's"
                }
            },
        );
        if mode.fix() {
            return Err(Refusal::Defect(n));
        }
        notices.push(n);
        let mut v = values;
        introselect(&mut v, k);
        v[k]
    } else {
        let mut v = values;
        let (_, m, _) = v.select_nth_unstable_by(k, f32::total_cmp);
        *m
    };
    Ok(1.1 * f64::from(v))
}

/// The T1's values above `thr` set to `thr` (as float).
pub fn clamp(t1: &mut Vol, thr: f64) {
    let t = thr as f32;
    t1.data.par_iter_mut().with_min_len(1 << 16).for_each(|v| {
        if f64::from(*v) > thr {
            *v = t;
        }
    });
}

/// The selection of libstdc++'s `std::nth_element` (introselect: the SGI STL's algorithm with
/// median-of-three pivots and an unguarded partition, insertion sort below four elements, heap
/// selection when the depth runs out), comparing with `<` as C++ does, so that NaN values end
/// where it leaves them (FSL-55 reproduced). Bounds are kept: a scan never leaves the slice.
pub fn introselect(v: &mut [f32], nth: usize) {
    let less = |a: f32, b: f32| a < b;
    let n = v.len();
    if n == 0 || nth >= n {
        return;
    }
    let (mut first, mut last) = (0usize, n);
    let mut depth = 2 * (usize::BITS - 1 - n.leading_zeros()) as i64;
    while last - first > 3 {
        if depth == 0 {
            heap_select(&mut v[first..last], nth - first + 1, &less);
            v.swap(first, nth);
            return;
        }
        depth -= 1;
        let mid = first + (last - first) / 2;
        move_median_to_first(v, first, first + 1, mid, last - 1, &less);
        let cut = unguarded_partition(v, first + 1, last, first, &less);
        if cut <= nth {
            first = cut;
        } else {
            last = cut;
        }
    }
    // insertion sort of [first, last)
    for i in first + 1..last {
        let x = v[i];
        if less(x, v[first]) {
            v.copy_within(first..i, first + 1);
            v[first] = x;
        } else {
            let mut j = i;
            while j > first && less(x, v[j - 1]) {
                v[j] = v[j - 1];
                j -= 1;
            }
            v[j] = x;
        }
    }
}

fn move_median_to_first(
    v: &mut [f32],
    result: usize,
    a: usize,
    b: usize,
    c: usize,
    less: &impl Fn(f32, f32) -> bool,
) {
    let pick = if less(v[a], v[b]) {
        if less(v[b], v[c]) {
            b
        } else if less(v[a], v[c]) {
            c
        } else {
            a
        }
    } else if less(v[a], v[c]) {
        a
    } else if less(v[b], v[c]) {
        c
    } else {
        b
    };
    v.swap(result, pick);
}

fn unguarded_partition(
    v: &mut [f32],
    mut first: usize,
    mut last: usize,
    pivot: usize,
    less: &impl Fn(f32, f32) -> bool,
) -> usize {
    loop {
        while first < v.len() && less(v[first], v[pivot]) {
            first += 1;
        }
        last -= 1;
        while last > 0 && less(v[pivot], v[last]) {
            last -= 1;
        }
        if !(first < last) {
            return first;
        }
        v.swap(first, last);
        first += 1;
    }
}

/// `std::partial_sort`'s heap selection: the `k` smallest of `v` in a max-heap at its front.
fn heap_select(v: &mut [f32], k: usize, less: &impl Fn(f32, f32) -> bool) {
    fn sift(v: &mut [f32], mut hole: usize, len: usize, x: f32, less: &impl Fn(f32, f32) -> bool) {
        let top = hole;
        let mut child = hole;
        while child < (len.saturating_sub(1)) / 2 {
            child = 2 * (child + 1);
            if less(v[child], v[child - 1]) {
                child -= 1;
            }
            v[hole] = v[child];
            hole = child;
        }
        if len % 2 == 0 && len >= 2 && child == (len - 2) / 2 {
            child = 2 * (child + 1);
            v[hole] = v[child - 1];
            hole = child - 1;
        }
        // push x up from hole towards top
        let mut parent = if hole > top { (hole - 1) / 2 } else { top };
        while hole > top && less(v[parent], x) {
            v[hole] = v[parent];
            hole = parent;
            parent = if hole > 0 { (hole - 1) / 2 } else { 0 };
        }
        v[hole] = x;
    }
    if k < 2 {
        return;
    }
    // make_heap of [0, k)
    let mut parent = (k - 2) / 2;
    loop {
        let x = v[parent];
        sift(v, parent, k, x, less);
        if parent == 0 {
            break;
        }
        parent -= 1;
    }
    for i in k..v.len() {
        if less(v[i], v[0]) {
            let x = v[i];
            v[i] = v[0];
            sift(v, 0, k, x, less);
        }
    }
}

/// The padded working grid (§6.4): the T1's grid with the mesh at least 3 voxels from every
/// face, plus 2 voxels (1 with `--t1only`) on each side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkGrid {
    /// Voxels added below each axis's first voxel.
    pub pad: [usize; 3],
    pub shape: [usize; 3],
    pub voxel: [f64; 3],
}

impl WorkGrid {
    pub fn new(points: &[V3], t1: &Vol, t1only: bool) -> WorkGrid {
        let extra = if t1only { 1 } else { 2 };
        let mut pad = [0usize; 3];
        let mut shape = t1.shape;
        for a in 0..3 {
            let d = t1.voxel[a];
            let lo = points.iter().map(|p| p[a]).fold(f64::INFINITY, f64::min);
            let hi = points
                .iter()
                .map(|p| p[a])
                .fold(f64::NEG_INFINITY, f64::max);
            let count = |x: f64| -> usize {
                if x.is_finite() && x > 0.0 {
                    x.min(1e6) as usize
                } else {
                    0
                }
            };
            let below = count((3.0 - lo / d).ceil()) + extra;
            let above = count((3.0 - t1.shape[a] as f64 + hi / d).ceil()) + extra;
            pad[a] = below;
            shape[a] = t1.shape[a] + below + above;
        }
        WorkGrid {
            pad,
            shape,
            voxel: t1.voxel,
        }
    }

    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The padding in mm along each axis.
    pub fn offset(&self) -> V3 {
        [0, 1, 2].map(|a| self.pad[a] as f64 * self.voxel[a])
    }

    /// Points of the T1's grid moved into the working grid.
    pub fn into_grid(&self, points: &[V3]) -> Vec<V3> {
        let o = self.offset();
        points.iter().map(|p| add(*p, o)).collect()
    }

    /// Points of the working grid moved back to the T1's grid.
    pub fn out_of_grid(&self, points: &[V3]) -> Vec<V3> {
        let o = self.offset();
        points.iter().map(|p| sub(*p, o)).collect()
    }
}

/// The three counting images and what casting the profiles counted.
#[derive(Clone, Debug, Default)]
pub struct Votes {
    pub inner: Vec<f32>,
    pub outer: Vec<f32>,
    pub scalp: Vec<f32>,
    /// The profiles cast.
    pub profiles: usize,
    /// The profiles dropped (counting for nothing).
    pub dropped: usize,
    /// The profiles of which only the scalp counts.
    pub scalp_only: usize,
    /// The profiles whose searches read past their last sample (FSL-51).
    pub overruns: usize,
    /// A profile on which FSL ends the program (FSL-50 reproduced).
    pub stop: bool,
    /// Probe dump (temporary).
    pub dump: Vec<String>,
}

/// Whether a profile from the point `x` (mm, T1's grid) along `n` passes near the eyes (§6.4):
/// one of `x + k n` (k = 0, 5, 10) mapped by the matrix and moved by `(-88, -128, -74)` above
/// the plane `0.0117213 y - 0.021433355 z - 0.6 = 0`, and none in the nose's region
/// (`y > 62`, `|x| < 16`).
pub fn near_eyes(x: V3, n: V3, m: &Matrix) -> bool {
    let mut above = false;
    for k in [0.0, 5.0, 10.0] {
        let p = add(x, scale(n, k));
        let q = [0, 1, 2].map(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3]);
        let (qx, qy, qz) = (q[0] - 88.0, q[1] - 128.0, q[2] - 74.0);
        if qy > 62.0 && qx.abs() < 16.0 {
            return false;
        }
        if 0.0117213 * qy - 0.021433355 * qz - 0.6 > 0.0 {
            above = true;
        }
    }
    above
}

/// Casts every profile (§6.4) and counts its estimates on the working grid. `t1` is the
/// clamped T1; `t2` the T2 on the T1's grid (FSL-48: sampled at the T1's voxel coordinates
/// whatever its grid). `drop_overruns`: a profile whose search reads past its last sample is
/// dropped (FSL-51's default) instead of read as "nothing there"; `fsl50`: a profile with an
/// empty search range ends the run (FSL-50 reproduced) instead of being dropped.
#[allow(clippy::too_many_arguments)]
pub fn votes(
    mesh: &Mesh,
    points: &[V3],
    t1: &Vol,
    t2: Option<&Vol>,
    matrix: &Matrix,
    grid: &WorkGrid,
    drop_overruns: bool,
    fsl50: bool,
    variant: u32,
) -> Votes {
    let shape = t1.shape;
    let d = t1.voxel;
    let [nx, ny, nz] = shape;
    // the brain surface drawn as 1 on the T1's grid
    let mut surface = vec![false; nx * ny * nz];
    raster::draw(mesh, points, shape, d, Rounding::Floor, |k| {
        surface[k] = true
    });
    let is_surface = |i: i64, j: i64, k: i64| -> bool {
        i >= 0
            && j >= 0
            && k >= 0
            && (i as usize) < nx
            && (j as usize) < ny
            && (k as usize) < nz
            && surface[(k as usize * ny + j as usize) * nx + i as usize]
    };
    let normals = mesh.triangle_normals(points);
    let start = if t2.is_some() { -30.0 } else { -3.0 };
    let [gx, gy, _] = grid.shape;
    let dump_mode = std::env::var("LX_BETSURF_DUMP").unwrap_or_default();
    let dump = !dump_mode.is_empty();
    struct Cast {
        hits: Vec<(u8, usize)>,
        profiles: usize,
        dropped: usize,
        scalp_only: usize,
        overruns: usize,
        stop: bool,
        dump: Vec<String>,
    }
    let casts: Vec<Cast> = (0..points.len())
        .into_par_iter()
        .with_min_len(16)
        .map(|v| {
            let mut c = Cast {
                hits: Vec::new(),
                profiles: 0,
                dropped: 0,
                scalp_only: 0,
                overruns: 0,
                stop: false,
                dump: Vec::new(),
            };
            let p = points[v];
            let n = mesh.vertex_normal(v, &normals);
            let nu = [n[0] / d[0], n[1] / d[1], n[2] / d[2]];
            let nb = mesh.neighbours.row(v);
            let mut far = 0.0f64;
            for &q in nb {
                let e = dist(p, points[q as usize]);
                if e > far {
                    far = e;
                }
            }
            let h = (far / 2.0).ceil();
            let m = [p[0] / d[0], p[1] / d[1], p[2] / d[2]];
            let lo = [0, 1, 2].map(|a| (m[a] - h / d[a]).floor());
            let hi = [0, 1, 2].map(|a| (m[a] + h / d[a]).floor());
            if !(0..3).all(|a| lo[a].is_finite() && hi[a].is_finite() && hi[a] - lo[a] < 1e6) {
                return c;
            }
            let (lo, hi) = (lo.map(|x| x as i64), hi.map(|x| x as i64));
            for k in lo[2]..=hi[2] {
                for j in lo[1]..=hi[1] {
                    for i in lo[0]..=hi[0] {
                        if !is_surface(i, j, k) {
                            continue;
                        }
                        let x = [i as f64 * d[0], j as f64 * d[1], k as f64 * d[2]];
                        let dp = dist(x, p);
                        let mut nearest = f64::INFINITY;
                        for &q in nb {
                            let e = dist(x, points[q as usize]);
                            if e < nearest {
                                nearest = e;
                            }
                        }
                        if !(nearest >= dp) {
                            continue;
                        }
                        c.profiles += 1;
                        let o = [i as f64, j as f64, k as f64];
                        let eyes = near_eyes(x, n, matrix);
                        let end = if eyes { 100.0 } else { 60.0 };
                        let sample = |vol: &Vol| -> Vec<f32> {
                            let mut out = Vec::with_capacity(((end - start) / 0.5) as usize);
                            let mut s = start;
                            let mut acc = [o[0] + s * nu[0], o[1] + s * nu[1], o[2] + s * nu[2]];
                            while s < end {
                                let q = if variant & 1024 != 0 {
                                    [0, 1, 2].map(|a| (x[a] + s * n[a]) / d[a])
                                } else if variant & 2048 != 0 {
                                    [0, 1, 2].map(|a| o[a] + (s * n[a]) / d[a])
                                } else if variant & 4096 != 0 {
                                    let q = acc;
                                    acc = [0, 1, 2].map(|a| acc[a] + 0.5 * nu[a]);
                                    q
                                } else {
                                    [o[0] + s * nu[0], o[1] + s * nu[1], o[2] + s * nu[2]]
                                };
                                out.push(trilinear(vol, q));
                                s += 0.5;
                            }
                            out
                        };
                        let mut p1 = Profile::new(start, sample(t1));
                        p1.variant = variant;

                        let record = dump.then(|| {
                            format!(
                                "{{\"v\": {v}, \"o\": [{i}, {j}, {k}], \"eyes\": {eyes}, \"n\": {:?}, \"start\": {start}, \"t1\": {:?}, \"t2\": {:?}",
                                n,
                                p1.values,
                                t2.map(&sample).unwrap_or_default()
                            )
                        });
                        let est = match t2 {
                            Some(t2v) => {
                                let mut p2 = Profile::new(start, sample(t2v));
                                p2.variant = variant;
                                let e = if eyes {
                                    profile::t1t2_eyes(&mut p1, &mut p2)
                                } else {
                                    profile::t1t2(&mut p1, &mut p2, fsl50)
                                };
                                p1.overrun |= p2.overrun;
                                e
                            }
                            None => {
                                if eyes {
                                    profile::t1only_eyes(&mut p1)
                                } else {
                                    profile::t1only(&mut p1)
                                }
                            }
                        };
                        if let Some(r) = record {
                            let keep = match dump_mode.as_str() {
                                "overrun" => p1.overrun,
                                "eyes" => eyes,
                                _ => true,
                            };
                            if keep {
                                c.dump.push(format!(
                                    "{r}, \"over\": {}, \"est\": \"{est:?}\"}}",
                                    p1.overrun
                                ));
                            }
                        }
                        if p1.overrun {
                            c.overruns += 1;
                            if drop_overruns {
                                c.dropped += 1;
                                continue;
                            }
                        }
                        let mut put = |image: u8, e: f64| {
                            let mut idx = [0usize; 3];
                            for a in 0..3 {
                                let x = if variant & 8192 != 0 {
                                    ((x[a] + e * n[a]) / d[a] + 0.5).floor()
                                } else {
                                    (o[a] + e * nu[a] + 0.5).floor()
                                };
                                let x = x + grid.pad[a] as f64;
                                if !(x >= 0.0 && x < grid.shape[a] as f64) {
                                    return;
                                }
                                idx[a] = x as usize;
                            }
                            c.hits.push((image, (idx[2] * gy + idx[1]) * gx + idx[0]));
                        };
                        match est {
                            Estimate::Dropped => c.dropped += 1,
                            Estimate::Stop => {
                                c.stop = true;
                                return c;
                            }
                            Estimate::Scalp(s) => {
                                c.scalp_only += 1;
                                put(2, s)
                            }
                            Estimate::Kept {
                                inner,
                                outer,
                                scalp,
                            } => {
                                put(0, inner);
                                put(1, outer);
                                put(2, scalp);
                            }
                        }
                    }
                }
            }
            c
        })
        .collect();
    let len = grid.len();
    let mut out = Votes {
        inner: vec![0.0; len],
        outer: vec![0.0; len],
        scalp: vec![0.0; len],
        ..Votes::default()
    };
    for c in casts {
        out.profiles += c.profiles;
        out.dropped += c.dropped;
        out.scalp_only += c.scalp_only;
        for line in &c.dump {
            out.dump.push(line.clone());
        }
        out.overruns += c.overruns;
        if c.stop {
            out.stop = true;
            return out;
        }
        for (image, k) in c.hits {
            match image {
                0 => out.inner[k] += 1.0,
                1 => out.outer[k] += 1.0,
                _ => out.scalp[k] += 1.0,
            }
        }
    }
    let _ = nz;
    out
}

/// The separable Gaussian of σ = `sigma` mm (§6.5): per axis σ in voxels `sigma/d`, the kernel
/// of half-width `2 trunc(σ − 0.001) + 3` voxels and weights `exp(−j²/(2σ²))` normalised to sum
/// 1, outside the grid 0; along x, then y, then z.
pub fn smooth(data: &[f32], shape: [usize; 3], voxel: [f64; 3], sigma: f64) -> Vec<f32> {
    let mut cur = data.to_vec();
    let [nx, ny, nz] = shape;
    let strides = [1, nx, nx * ny];
    for a in 0..3 {
        let s = sigma / voxel[a];
        let half = 2 * (s - 0.001).trunc().max(0.0) as i64 + 3;
        let mut w: Vec<f64> = (-half..=half)
            .map(|j| larmorx_core::math::exp(-((j * j) as f64) / (2.0 * s * s)))
            .collect();
        let total: f64 = w.iter().sum();
        for x in &mut w {
            *x /= total;
        }
        let n = shape[a];
        let stride = strides[a];
        let src = cur;
        let mut dst = vec![0.0f32; src.len()];
        // each line along the axis on its own
        let lines: Vec<usize> = (0..src.len()).filter(|&k| (k / stride) % n == 0).collect();
        let results: Vec<(usize, Vec<f32>)> = lines
            .par_iter()
            .with_min_len(64)
            .map(|&base| {
                let line: Vec<f32> = (0..n).map(|i| src[base + i * stride]).collect();
                let out: Vec<f32> = (0..n as i64)
                    .map(|i| {
                        let mut acc = 0.0f64;
                        for (t, &wt) in w.iter().enumerate() {
                            let j = i + t as i64 - half;
                            if j >= 0 && j < n as i64 {
                                acc += wt * f64::from(line[j as usize]);
                            }
                        }
                        acc as f32
                    })
                    .collect();
                (base, out)
            })
            .collect();
        for (base, out) in results {
            for (i, v) in out.into_iter().enumerate() {
                dst[base + i * stride] = v;
            }
        }
        cur = dst;
        let _ = (ny, nz);
    }
    cur
}

/// One phase of betsurf's deformation (§6.5): `steps` moves of every vertex at once on the
/// counting image `image` (working grid), at speed `speed`, held outside `mask` when given.
pub fn deform(
    mesh: &Mesh,
    points: &mut Vec<V3>,
    image: &Vol,
    mask: Option<&Vol>,
    steps: usize,
    speed: f64,
) {
    let rmin = 3.33;
    let rmax = f64::from(10.0f32);
    let e = (1.0 / rmin + 1.0 / rmax) / 2.0;
    let f = 6.0 / (1.0 / rmin - 1.0 / rmax);
    let d = image.voxel;
    let at = |vol: &Vol, q: V3| -> f64 {
        f64::from(trilinear(vol, [q[0] / d[0], q[1] / d[1], q[2] / d[2]]))
    };
    let mut next = vec![[0.0; 3]; points.len()];
    let mut outside = vec![false; points.len()];
    let mut l = 0.0;
    for i in 0..steps {
        if i % 50 == 0 {
            l = mesh.mean_neighbour_distance(points);
        }
        let normals = mesh.triangle_normals(points);
        let dk: u32 = std::env::var("LX_DEFORM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if i % 100 == 0 {
            let pts = &*points;
            outside
                .par_iter_mut()
                .with_min_len(64)
                .enumerate()
                .for_each(|(v, o)| {
                    let p = pts[v];
                    let n = mesh.vertex_normal(v, &normals);
                    let mut out = true;
                    let mut prev: Option<f64> = None;
                    let mut t = if dk & 1 != 0 {
                        1.0
                    } else if dk & 2 != 0 {
                        0.0
                    } else {
                        -1.0
                    };
                    while t <= 149.0 {
                        let x = at(image, add(p, scale(n, t)));
                        if x > 0.1 {
                            out = false;
                            break;
                        }
                        if let Some(pv) = prev
                            && (if dk & 8 != 0 {
                                f64::from(100.0f32 * (x as f32 - pv as f32))
                            } else {
                                100.0 * (x - pv)
                            }) > 0.1
                            && dk & 16 == 0
                        {
                            out = false;
                            break;
                        }
                        prev = Some(x);
                        t += 2.0;
                    }
                    if dk & 4 != 0 {
                        *o |= out;
                    } else {
                        *o = out;
                    }
                });
        }
        let l2 = l * l;
        let pts = &*points;
        let marks = &outside;
        next.par_iter_mut()
            .with_min_len(64)
            .enumerate()
            .for_each(|(v, out)| {
                let p = pts[v];
                let n = mesh.vertex_normal(v, &normals);
                let m = mesh.neighbour_mean(v, pts);
                let s = sub(m, p);
                let sigma = dot(s, n);
                let s_n = scale(n, sigma);
                let s_t = sub(s, s_n);
                let u1 = scale(s_t, 0.5);
                let rinv = (2.0 * dot(s_n, n).abs()) / l2;
                let f2 = (1.0 + larmorx_core::math::tanh_glibc(f * (rinv - e))) * 0.5;
                let u2 = scale(s_n, f2);
                let mval = mask.map(|mk| at(mk, p));
                let f3 = if !marks[v] {
                    let up = at(image, add(p, n));
                    let down = at(image, sub(p, n));
                    let g = 100.0 * (up - down);
                    let mut f3 = g.clamp(-1.0, 1.0);
                    if (0.0..0.1).contains(&g) && up < 0.1 {
                        f3 = speed;
                    }
                    if let Some(mv) = mval
                        && mv > 0.0
                    {
                        f3 = (mv / 2.0).max(0.1).max(f3);
                    }
                    f3
                } else {
                    match mval {
                        Some(mv) if mv > 0.0 => (mv / 2.0).max(0.1),
                        _ => 0.0,
                    }
                };
                let u3 = scale(n, 0.05 * f3);
                *out = add(p, add(add(u1, u2), u3));
            });
        std::mem::swap(points, &mut next);
    }
}

/// A new sphere for the scalp (§6.5, phase 5): the icosahedron subdivided four times, centred
/// on `centre` with radius `radius`.
pub fn sphere(centre: V3, radius: f64) -> (Mesh, Vec<V3>) {
    let m = Mesh::sphere(4);
    let pts = m
        .points
        .iter()
        .map(|p| add(centre, scale(sub(add(*p, centre), centre), radius)))
        .collect();
    (m, pts)
}

/// The mean distance of the vertices from their centroid.
pub fn mean_radius(points: &[V3], c: V3) -> f64 {
    let mut s = 0.0;
    for p in points {
        s += dist(*p, c);
    }
    s / points.len() as f64
}

/// A mesh's retessellation (each triangle into four at its edges' midpoints) with its points.
pub fn retessellate(mesh: &Mesh, points: &[V3]) -> (Mesh, Vec<V3>) {
    let moved = Mesh::new(points.to_vec(), mesh.triangles.clone());
    let r = moved.retessellated();
    let pts = r.points.clone();
    (r, pts)
}

/// Integer conversion of a smoothed image (truncation; values of 1 or more are the fill's
/// barriers, §6.5).
pub fn truncated(data: &[f32]) -> Vec<i32> {
    data.par_iter()
        .with_min_len(1 << 16)
        .map(|&v| {
            c_int(f64::from(v)).map_or(i32::MIN, |x| {
                x.clamp(i32::MIN as i64, i32::MAX as i64) as i32
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn introselect_is_a_selection() {
        let mut s: u64 = 7;
        for n in [1usize, 2, 3, 4, 5, 17, 100, 1000] {
            let v: Vec<f32> = (0..n)
                .map(|_| {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ((s >> 40) % 50) as f32
                })
                .collect();
            for k in [0, n / 2, n - 1] {
                let mut a = v.clone();
                introselect(&mut a, k);
                let mut b = v.clone();
                b.sort_by(f32::total_cmp);
                assert_eq!(a[k], b[k], "n {n} k {k}");
            }
        }
    }

    #[test]
    fn matrix_reading() {
        let (m, n) = read_matrix("1 0 0 -8\n0 1 0 4\n0 0 1 -12\n0 0 0 1\n");
        assert_eq!(n, 12);
        assert_eq!(m[2], [0.0, 0.0, 1.0, -12.0]);
        let (m, n) = read_matrix("1 0 0 -8\n0 1\n");
        assert_eq!(n, 6);
        assert_eq!(m[1], [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn smoothing_keeps_the_sum_inside() {
        let shape = [21, 21, 21];
        let mut v = vec![0.0f32; 21 * 21 * 21];
        v[(10 * 21 + 10) * 21 + 10] = 1.0;
        let s = smooth(&v, shape, [1.0; 3], 3.0);
        let total: f64 = s.iter().map(|&x| f64::from(x)).sum();
        assert!((total - 1.0).abs() < 1e-5, "{total}");
    }
}
