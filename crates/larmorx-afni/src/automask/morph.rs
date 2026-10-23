// SPDX-License-Identifier: Apache-2.0
//! The mask operations of 3dAutomask (`specs/3dAutomask.md` §5.3 to §5.6, §6.1, §6.3, §7):
//! peeling and restoring, filling holes, erosion and dilation, the exterior clip and the depth.
//!
//! Masks are `u8` grids, 0 or 1, x fastest. Every operation decides each voxel from the mask
//! as it was before the pass (except where the spec says otherwise), so the voxels of a pass
//! are independent: they are decided in parallel over z planes, and the changes applied after.
//! Two of the spec's sequential procedures are computed by equivalent direct methods with the
//! same result: the exterior clip's sweeps, repeated until one adds nothing, reach every
//! candidate voxel connected to the outside through candidates, whatever the sweep order (a
//! breadth-first search finds the same set); and the depth, the number of erosions that remove
//! a voxel, is its distance in erosion steps from the clear voxels (a breadth-first search in
//! the erosion's neighbourhood, one layer per erosion).

use rayon::prelude::*;

/// A voxel's neighbourhood: its 6 faces, its faces and 12 edges (18), or all 26 (with its 8
/// corners). `-NN1`, `-NN2`, `-NN3`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Neighbourhood {
    Faces,
    #[default]
    Edges,
    Corners,
}

impl Neighbourhood {
    /// `-NN<level>`.
    pub fn from_level(level: u8) -> Option<Neighbourhood> {
        match level {
            1 => Some(Neighbourhood::Faces),
            2 => Some(Neighbourhood::Edges),
            3 => Some(Neighbourhood::Corners),
            _ => None,
        }
    }

    /// The offsets `(di, dj, dk)` of the neighbourhood.
    pub fn offsets(self) -> Vec<[i64; 3]> {
        let mut out = Vec::new();
        for dk in -1i64..=1 {
            for dj in -1i64..=1 {
                for di in -1i64..=1 {
                    let n = di.abs() + dj.abs() + dk.abs();
                    let keep = match self {
                        Neighbourhood::Faces => n == 1,
                        Neighbourhood::Edges => n == 1 || n == 2,
                        Neighbourhood::Corners => n >= 1,
                    };
                    if keep {
                        out.push([di, dj, dk]);
                    }
                }
            }
        }
        out
    }
}

/// The 8 neighbours of a pixel in its plane (spec §7).
pub fn plane_offsets() -> Vec<[i64; 3]> {
    let mut out = Vec::new();
    for dj in -1i64..=1 {
        for di in -1i64..=1 {
            if di != 0 || dj != 0 {
                out.push([di, dj, 0]);
            }
        }
    }
    out
}

/// A set of neighbour offsets in a grid: linear offsets for interior voxels, and the rule for
/// voxels at the grid's edge, whose missing neighbours are replaced by the voxel at the
/// clamped index along each axis (planar duplication: a face neighbour beyond the edge is the
/// voxel itself).
pub struct Stencil {
    dims: [usize; 3],
    offsets: Vec<[i64; 3]>,
    linear: Vec<isize>,
    /// The half-width of the stencil along each axis (1, or 0 for a plane stencil).
    reach: [usize; 3],
}

impl Stencil {
    pub fn new(dims: [usize; 3], offsets: Vec<[i64; 3]>) -> Stencil {
        let [nx, ny, _] = dims;
        let linear = offsets
            .iter()
            .map(|o| (o[0] + nx as i64 * (o[1] + ny as i64 * o[2])) as isize)
            .collect();
        let mut reach = [0usize; 3];
        for o in &offsets {
            for a in 0..3 {
                reach[a] = reach[a].max(o[a].unsigned_abs() as usize);
            }
        }
        Stencil {
            dims,
            offsets,
            linear,
            reach,
        }
    }

    #[inline]
    fn interior(&self, i: usize, j: usize, k: usize) -> bool {
        let [nx, ny, nz] = self.dims;
        let r = self.reach;
        i >= r[0] && i + r[0] < nx && j >= r[1] && j + r[1] < ny && k >= r[2] && k + r[2] < nz
    }

    /// The number of set voxels among the neighbours of `(i, j, k)` (index `p`).
    #[inline]
    pub fn count(&self, mask: &[u8], p: usize, i: usize, j: usize, k: usize) -> u32 {
        if self.interior(i, j, k) {
            self.linear
                .iter()
                .map(|&d| u32::from(mask[(p as isize + d) as usize]))
                .sum()
        } else {
            let [nx, ny, nz] = self.dims;
            let c = |x: usize, d: i64, n: usize| (x as i64 + d).clamp(0, n as i64 - 1) as usize;
            self.offsets
                .iter()
                .map(|o| {
                    let (a, b, cc) = (c(i, o[0], nx), c(j, o[1], ny), c(k, o[2], nz));
                    u32::from(mask[a + nx * (b + ny * cc)])
                })
                .sum()
        }
    }
}

/// The voxels of each z plane, in parallel, for which `pick(p, i, j, k)` holds: the indices
/// in storage order.
fn select(
    dims: [usize; 3],
    planes: std::ops::Range<usize>,
    pick: impl Fn(usize, usize, usize, usize) -> bool + Sync,
) -> Vec<usize> {
    let [nx, ny, _] = dims;
    planes
        .into_par_iter()
        .flat_map_iter(|k| {
            let mut out = Vec::new();
            for j in 0..ny {
                for i in 0..nx {
                    let p = i + nx * (j + ny * k);
                    if pick(p, i, j, k) {
                        out.push(p);
                    }
                }
            }
            out
        })
        .collect()
}

/// The z planes holding a set voxel, widened by `pad` planes on each side (the range the
/// operations need to look at).
fn active_planes(mask: &[u8], dims: [usize; 3], pad: usize) -> std::ops::Range<usize> {
    let plane = dims[0] * dims[1];
    if plane == 0 {
        return 0..0;
    }
    let first = mask.chunks(plane).position(|c| c.iter().any(|&b| b != 0));
    let last = mask.chunks(plane).rposition(|c| c.iter().any(|&b| b != 0));
    match (first, last) {
        (Some(a), Some(b)) => a.saturating_sub(pad)..(b + 1 + pad).min(dims[2]),
        _ => 0..0,
    }
}

/// Peels `peels` layers off `mask` and restores them (spec §5.3): a set voxel with fewer than
/// `threshold` set neighbours (of `stencil`) is peeled; then, from the last layer back, a
/// peeled voxel still clear comes back when it has more than 0 set neighbours (the last layer)
/// or more than 1 (the others). Nothing happens on a grid of fewer than 27 voxels (3D) or 9
/// pixels (a plane).
pub fn peel_and_restore(mask: &mut [u8], stencil: &Stencil, peels: usize, threshold: u32) {
    let dims = stencil.dims;
    let min_voxels = if stencil.reach[2] == 0 { 9 } else { 27 };
    if mask.len() < min_voxels || peels == 0 {
        return;
    }
    let mut layers: Vec<Vec<usize>> = Vec::with_capacity(peels);
    for _ in 0..peels {
        let planes = active_planes(mask, dims, 0);
        let peeled = {
            let m = &*mask;
            select(dims, planes, |p, i, j, k| {
                m[p] != 0 && stencil.count(m, p, i, j, k) < threshold
            })
        };
        for &p in &peeled {
            mask[p] = 0;
        }
        layers.push(peeled);
    }
    let [nx, ny, _] = dims;
    for l in (0..peels).rev() {
        let r = if l + 1 == peels { 0 } else { 1 };
        let restored: Vec<usize> = {
            let m = &*mask;
            layers[l..]
                .par_iter()
                .flat_map_iter(|layer| {
                    layer.iter().copied().filter(move |&p| {
                        let (i, j, k) = (p % nx, (p / nx) % ny, p / (nx * ny));
                        m[p] == 0 && stencil.count(m, p, i, j, k) > r
                    })
                })
                .collect()
        };
        for p in restored {
            mask[p] = 1;
        }
    }
}

/// Fills small holes once with side `s` (spec §5.4): every clear voxel at least `s` voxels
/// (per axis, `min((n - 1) / 2, s)`) from the grid's edges that has a set voxel within that
/// many steps on both sides along x, y or z is set. The tests use the mask before the pass.
/// Returns the number of voxels filled.
pub fn fill_once(mask: &mut [u8], dims: [usize; 3], s: usize) -> usize {
    let [nx, ny, nz] = dims;
    let side = [
        ((nx.max(1) - 1) / 2).min(s),
        ((ny.max(1) - 1) / 2).min(s),
        ((nz.max(1) - 1) / 2).min(s),
    ];
    if side == [0, 0, 0] {
        return 0;
    }
    let [sx, sy, sz] = side;
    let planes = active_planes(mask, dims, 0);
    let (k0, k1) = (planes.start.max(sz), planes.end.min(nz - sz));
    if k0 >= k1 {
        return 0;
    }
    let plane = nx * ny;
    let m = &*mask;
    let filled: Vec<usize> = (k0..k1)
        .into_par_iter()
        .flat_map_iter(|k| {
            let mut out = Vec::new();
            for j in sy..ny - sy {
                for i in sx..nx - sx {
                    let p = i + nx * (j + ny * k);
                    if m[p] != 0 {
                        continue;
                    }
                    let both = |stride: usize, s: usize| {
                        s > 0
                            && (1..=s).any(|a| m[p - a * stride] != 0)
                            && (1..=s).any(|a| m[p + a * stride] != 0)
                    };
                    if both(1, sx) || both(nx, sy) || both(plane, sz) {
                        out.push(p);
                    }
                }
            }
            out
        })
        .collect();
    for &p in &filled {
        mask[p] = 1;
    }
    filled.len()
}

/// Fills with side `s` until a pass fills nothing; the number of voxels filled.
pub fn fill_until_done(mask: &mut [u8], dims: [usize; 3], s: usize) -> usize {
    let mut total = 0;
    loop {
        let n = fill_once(mask, dims, s);
        if n == 0 {
            return total;
        }
        total += n;
    }
}

/// An erosion (spec §6.3): every set voxel on the grid's border is cleared, and every other
/// set voxel with a clear neighbour of `nn`. Returns the number cleared.
pub fn erode(mask: &mut [u8], dims: [usize; 3], nn: Neighbourhood) -> usize {
    let [nx, ny, nz] = dims;
    let stencil = Stencil::new(dims, nn.offsets());
    let n_offsets = stencil.offsets.len() as u32;
    let planes = active_planes(mask, dims, 0);
    let cleared = {
        let m = &*mask;
        select(dims, planes, |p, i, j, k| {
            m[p] != 0
                && (i == 0
                    || j == 0
                    || k == 0
                    || i + 1 == nx
                    || j + 1 == ny
                    || k + 1 == nz
                    || stencil.count(m, p, i, j, k) < n_offsets)
        })
    };
    for &p in &cleared {
        mask[p] = 0;
    }
    cleared.len()
}

/// A dilation (spec §6.1): every clear voxel with at least 3 set neighbours of `nn` (edge
/// voxels duplicated) is set. Returns the number set.
pub fn dilate(mask: &mut [u8], dims: [usize; 3], nn: Neighbourhood) -> usize {
    let stencil = Stencil::new(dims, nn.offsets());
    let planes = active_planes(mask, dims, 1);
    let set = {
        let m = &*mask;
        select(dims, planes, |p, i, j, k| {
            m[p] == 0 && stencil.count(m, p, i, j, k) >= 3
        })
    };
    for &p in &set {
        mask[p] = 1;
    }
    set.len()
}

/// The exterior clip (spec §5.6, step 3) on the mask of the outside `outside`: every interior
/// voxel (at least one voxel from each edge) not in it, whose value is below `level` or above
/// `9999.9 * level`, joins it when it touches it face to face, repeated until nothing joins.
/// Returns the number of voxels added.
pub fn exterior_clip(outside: &mut [u8], v: &[f32], dims: [usize; 3], level: f32) -> usize {
    let [nx, ny, nz] = dims;
    if nx < 3 || ny < 3 || nz < 3 {
        return 0;
    }
    let top = (9999.9 * f64::from(level)) as f32;
    let plane = nx * ny;
    let interior = |p: usize| {
        let (i, j, k) = (p % nx, (p / nx) % ny, p / plane);
        i >= 1 && i + 1 < nx && j >= 1 && j + 1 < ny && k >= 1 && k + 1 < nz
    };
    let candidate = |o: &[u8], p: usize| o[p] == 0 && (v[p] < level || v[p] > top) && interior(p);
    let faces = [
        1isize,
        -1,
        nx as isize,
        -(nx as isize),
        plane as isize,
        -(plane as isize),
    ];
    let touches = |o: &[u8], p: usize| faces.iter().any(|&d| o[(p as isize + d) as usize] != 0);
    let mut queue: Vec<usize> = {
        let o = &*outside;
        select(dims, 1..nz - 1, |p, _, _, _| {
            candidate(o, p) && touches(o, p)
        })
    };
    for &p in &queue {
        outside[p] = 1;
    }
    let mut added = queue.len();
    while let Some(p) = queue.pop() {
        for &d in &faces {
            let q = (p as isize + d) as usize;
            if candidate(outside, q) {
                outside[q] = 1;
                added += 1;
                queue.push(q);
            }
        }
    }
    added
}

/// The depth (spec §6.3): for every voxel of `mask`, the number of erosions of `nn` that
/// remove it (0 outside), and the number of voxels each erosion removes.
pub fn depth(mask: &[u8], dims: [usize; 3], nn: Neighbourhood) -> (Vec<i16>, Vec<usize>) {
    let [nx, ny, nz] = dims;
    let stencil = Stencil::new(dims, nn.offsets());
    let n_offsets = stencil.offsets.len() as u32;
    let mut depth = vec![0i16; mask.len()];
    let planes = active_planes(mask, dims, 0);
    // Layer 1: the voxels the first erosion removes.
    let mut layer = select(dims, planes, |p, i, j, k| {
        mask[p] != 0
            && (i == 0
                || j == 0
                || k == 0
                || i + 1 == nx
                || j + 1 == ny
                || k + 1 == nz
                || stencil.count(mask, p, i, j, k) < n_offsets)
    });
    let mut counts = Vec::new();
    let mut d: i16 = 1;
    // The voxels not yet in a layer.
    let mut left: Vec<u8> = mask.to_vec();
    while !layer.is_empty() {
        counts.push(layer.len());
        for &p in &layer {
            depth[p] = d;
            left[p] = 0;
        }
        // The next erosion removes the voxels left that touch this layer.
        let mut next = Vec::new();
        for &p in &layer {
            let (i, j, k) = (p % nx, (p / nx) % ny, p / (nx * ny));
            for o in &stencil.offsets {
                let (a, b, c) = (i as i64 + o[0], j as i64 + o[1], k as i64 + o[2]);
                if a < 0 || b < 0 || c < 0 || a >= nx as i64 || b >= ny as i64 || c >= nz as i64 {
                    continue;
                }
                let q = a as usize + nx * (b as usize + ny * c as usize);
                if left[q] != 0 {
                    left[q] = 0;
                    next.push(q);
                }
            }
        }
        layer = next;
        d = d.saturating_add(1);
    }
    (depth, counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automask::cluster::tests::random_mask;

    fn at(dims: [usize; 3], i: i64, j: i64, k: i64) -> Option<usize> {
        let [nx, ny, nz] = dims;
        (i >= 0 && j >= 0 && k >= 0 && (i as usize) < nx && (j as usize) < ny && (k as usize) < nz)
            .then(|| i as usize + nx * (j as usize + ny * k as usize))
    }

    /// The count of set neighbours by the spec's words: each missing neighbour replaced by the
    /// voxel at the clamped index along each axis.
    fn count_by_hand(m: &[u8], dims: [usize; 3], p: usize, offsets: &[[i64; 3]]) -> u32 {
        let [nx, ny, nz] = dims;
        let (i, j, k) = (
            (p % nx) as i64,
            ((p / nx) % ny) as i64,
            (p / (nx * ny)) as i64,
        );
        offsets
            .iter()
            .map(|o| {
                let c = |x: i64, n: usize| x.clamp(0, n as i64 - 1);
                let q = at(dims, c(i + o[0], nx), c(j + o[1], ny), c(k + o[2], nz)).unwrap();
                u32::from(m[q])
            })
            .sum()
    }

    #[test]
    fn neighbourhoods() {
        assert_eq!(Neighbourhood::Faces.offsets().len(), 6);
        assert_eq!(Neighbourhood::Edges.offsets().len(), 18);
        assert_eq!(Neighbourhood::Corners.offsets().len(), 26);
        assert_eq!(plane_offsets().len(), 8);
        let dims = [5, 4, 3];
        let m = random_mask(dims, 50, 3);
        for nn in [
            Neighbourhood::Faces,
            Neighbourhood::Edges,
            Neighbourhood::Corners,
        ] {
            let s = Stencil::new(dims, nn.offsets());
            for p in 0..m.len() {
                let (i, j, k) = (p % 5, (p / 5) % 4, p / 20);
                assert_eq!(
                    s.count(&m, p, i, j, k),
                    count_by_hand(&m, dims, p, &nn.offsets())
                );
            }
        }
    }

    /// Peeling and restoring by the spec's words, voxel by voxel.
    fn peel_by_hand(m: &mut [u8], dims: [usize; 3], offsets: &[[i64; 3]], p: usize, t: u32) {
        let mut mark = vec![0usize; m.len()];
        for l in 1..=p {
            let before = m.to_vec();
            for q in 0..m.len() {
                if before[q] != 0 && count_by_hand(&before, dims, q, offsets) < t {
                    mark[q] = l;
                    m[q] = 0;
                }
            }
        }
        for l in (1..=p).rev() {
            let r = if l == p { 0 } else { 1 };
            let before = m.to_vec();
            for q in 0..m.len() {
                if before[q] == 0 && mark[q] >= l && count_by_hand(&before, dims, q, offsets) > r {
                    m[q] = 1;
                }
            }
        }
    }

    #[test]
    fn peeling_against_the_spec() {
        for (seed, dims) in [[9, 8, 7], [6, 6, 6], [12, 5, 4]].into_iter().enumerate() {
            for density in [55, 75, 90] {
                for (p, t) in [(1, 17), (2, 17), (3, 12), (1, 6), (2, 18)] {
                    let mut a = random_mask(dims, density, 11 + seed as u64 * 100 + density);
                    let mut b = a.clone();
                    let offsets = Neighbourhood::Edges.offsets();
                    peel_by_hand(&mut a, dims, &offsets, p, t);
                    peel_and_restore(&mut b, &Stencil::new(dims, offsets), p, t);
                    assert_eq!(a, b, "{dims:?} density {density} peels {p} threshold {t}");
                }
            }
        }
        // In a plane: 8 neighbours, threshold 6, three peels.
        let dims = [11, 9, 1];
        let mut a = random_mask(dims, 70, 5);
        let mut b = a.clone();
        peel_by_hand(&mut a, dims, &plane_offsets(), 3, 6);
        peel_and_restore(&mut b, &Stencil::new(dims, plane_offsets()), 3, 6);
        assert_eq!(a, b);
    }

    fn fill_by_hand(m: &mut [u8], dims: [usize; 3], s: usize) -> usize {
        let [nx, ny, nz] = dims;
        let side = [
            ((nx - 1) / 2).min(s),
            ((ny - 1) / 2).min(s),
            ((nz - 1) / 2).min(s),
        ];
        if side == [0, 0, 0] {
            return 0;
        }
        let before = m.to_vec();
        let mut n = 0;
        for k in side[2]..nz - side[2] {
            for j in side[1]..ny - side[1] {
                for i in side[0]..nx - side[0] {
                    let p = at(dims, i as i64, j as i64, k as i64).unwrap();
                    if before[p] != 0 {
                        continue;
                    }
                    let mut fill = false;
                    for (a, &sa) in side.iter().enumerate() {
                        if sa == 0 {
                            continue;
                        }
                        let get = |d: i64| {
                            let mut c = [i as i64, j as i64, k as i64];
                            c[a] += d;
                            before[at(dims, c[0], c[1], c[2]).unwrap()] != 0
                        };
                        let sa = sa as i64;
                        if (1..=sa).any(|d| get(-d)) && (1..=sa).any(get) {
                            fill = true;
                        }
                    }
                    if fill {
                        m[p] = 1;
                        n += 1;
                    }
                }
            }
        }
        n
    }

    #[test]
    fn filling_against_the_spec() {
        for dims in [[9, 8, 7], [5, 5, 2], [16, 3, 9], [7, 7, 1]] {
            for density in [10, 30, 50] {
                for s in [1, 2, 3, 5] {
                    let mut a = random_mask(dims, density, density + s as u64);
                    let mut b = a.clone();
                    assert_eq!(fill_by_hand(&mut a, dims, s), fill_once(&mut b, dims, s));
                    assert_eq!(a, b, "{dims:?} density {density} side {s}");
                }
            }
        }
    }

    #[test]
    fn erosion_and_dilation_against_the_spec() {
        for dims in [[8, 7, 6], [5, 5, 5], [9, 4, 3]] {
            for density in [40, 70, 90] {
                for nn in [
                    Neighbourhood::Faces,
                    Neighbourhood::Edges,
                    Neighbourhood::Corners,
                ] {
                    let m = random_mask(dims, density, density * 7);
                    let offsets = nn.offsets();
                    // Erosion: border voxels, and voxels with a clear neighbour, cleared.
                    let [nx, ny, nz] = dims;
                    let mut expected = m.clone();
                    for p in 0..m.len() {
                        let (i, j, k) = (p % nx, (p / nx) % ny, p / (nx * ny));
                        let border =
                            i == 0 || j == 0 || k == 0 || i + 1 == nx || j + 1 == ny || k + 1 == nz;
                        if m[p] != 0
                            && (border
                                || offsets.iter().any(|o| {
                                    m[at(dims, i as i64 + o[0], j as i64 + o[1], k as i64 + o[2])
                                        .unwrap()]
                                        == 0
                                }))
                        {
                            expected[p] = 0;
                        }
                    }
                    let mut e = m.clone();
                    let n = erode(&mut e, dims, nn);
                    assert_eq!(e, expected);
                    assert_eq!(n, m.iter().zip(&e).filter(|(a, b)| a != b).count());
                    // Dilation: clear voxels with at least 3 set neighbours (clamped).
                    let mut expected = m.clone();
                    for p in 0..m.len() {
                        if m[p] == 0 && count_by_hand(&m, dims, p, &offsets) >= 3 {
                            expected[p] = 1;
                        }
                    }
                    let mut g = m.clone();
                    dilate(&mut g, dims, nn);
                    assert_eq!(g, expected);
                }
            }
        }
    }

    #[test]
    fn depth_is_the_number_of_erosions() {
        for dims in [[10, 9, 8], [6, 6, 6]] {
            for density in [70, 95] {
                for nn in [
                    Neighbourhood::Faces,
                    Neighbourhood::Edges,
                    Neighbourhood::Corners,
                ] {
                    let m = random_mask(dims, density, density + 1);
                    let (d, counts) = depth(&m, dims, nn);
                    let mut e = m.clone();
                    let mut expected = vec![0i16; m.len()];
                    let mut expected_counts = Vec::new();
                    let mut level = 1;
                    while e.iter().any(|&b| b != 0) {
                        let before = e.clone();
                        let n = erode(&mut e, dims, nn);
                        expected_counts.push(n);
                        for p in 0..m.len() {
                            if before[p] != 0 && e[p] == 0 {
                                expected[p] = level;
                            }
                        }
                        level += 1;
                    }
                    assert_eq!(d, expected, "{dims:?} {density} {nn:?}");
                    assert_eq!(counts, expected_counts);
                }
            }
        }
    }

    #[test]
    fn exterior_clip_against_repeated_sweeps() {
        let dims = [10, 9, 8];
        let [nx, ny, nz] = dims;
        for seed in 0..6u64 {
            let outside = random_mask(dims, 15 + seed * 5, seed + 40);
            let v: Vec<f32> = random_mask(dims, 50, seed + 90)
                .iter()
                .map(|&b| if b != 0 { 5.0 } else { 50.0 })
                .collect();
            let level = 10.0;
            // The spec's sweeps, in storage order, in place, until one adds nothing.
            let mut expected = outside.clone();
            let mut total = 0;
            loop {
                let mut added = 0;
                for k in 1..nz - 1 {
                    for j in 1..ny - 1 {
                        for i in 1..nx - 1 {
                            let p = at(dims, i as i64, j as i64, k as i64).unwrap();
                            let t = (9999.9 * f64::from(level)) as f32;
                            if expected[p] == 0 && (v[p] < level || v[p] > t) {
                                let faces =
                                    [p - 1, p + 1, p - nx, p + nx, p - nx * ny, p + nx * ny];
                                if faces.iter().any(|&q| expected[q] != 0) {
                                    expected[p] = 1;
                                    added += 1;
                                }
                            }
                        }
                    }
                }
                total += added;
                if added == 0 {
                    break;
                }
            }
            let mut got = outside.clone();
            assert_eq!(exterior_clip(&mut got, &v, dims, level), total);
            assert_eq!(got, expected);
        }
    }
}
