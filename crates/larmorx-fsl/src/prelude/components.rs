// SPDX-License-Identifier: Apache-2.0
//! Connected components on a voxel grid (`specs/prelude.md` §4.4, §4.6): a union-find over the
//! voxel indices whose roots stay at each component's smallest index (its first voxel in the
//! radiological raster order, x fastest), so that numbering components by their first voxel is
//! one raster scan, and the tie between components of equal size falls on the first one, as
//! the spec asks. Any method that gives these components is allowed (§10); the in-plane unions
//! run in parallel over the slices, the unions across slices after them.

use rayon::prelude::*;

/// No component (a voxel outside the set).
pub const NONE: u32 = u32::MAX;

/// The root of `i` with path halving (every parent index at most its child's).
#[inline]
fn find(parent: &mut [u32], mut i: u32) -> u32 {
    loop {
        let p = parent[i as usize];
        if p == i {
            return i;
        }
        let g = parent[p as usize];
        parent[i as usize] = g;
        i = g;
    }
}

/// Joins the components of `a` and `b`, the larger root attached under the smaller.
#[inline]
fn union(parent: &mut [u32], a: u32, b: u32) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra < rb {
        parent[rb as usize] = ra;
    } else if rb < ra {
        parent[ra as usize] = rb;
    }
}

/// Labels the components of the voxels `member` selects, two voxels joined when they are face
/// neighbours (x and y; z too when `three_d`) for which `joined(p, q)` holds (`p` and `q` members).
/// Fills `parent` (one entry per voxel) with each member's root, the smallest index of its
/// component, and [`NONE`] for the others.
pub fn label(
    shape: [usize; 3],
    member: impl Fn(usize) -> bool + Sync,
    joined: impl Fn(usize, usize) -> bool + Sync,
    three_d: bool,
    parent: &mut [u32],
) {
    let [nx, ny, nz] = shape;
    let plane = nx * ny;
    debug_assert_eq!(parent.len(), plane * nz);
    if plane == 0 {
        return;
    }
    // each slice on its own (its entries name voxels of the same slice only)
    parent
        .par_chunks_mut(plane)
        .enumerate()
        .for_each(|(z, sl)| {
            let base = z * plane;
            for y in 0..ny {
                for x in 0..nx {
                    let j = y * nx + x;
                    let i = base + j;
                    if !member(i) {
                        sl[j] = NONE;
                        continue;
                    }
                    sl[j] = j as u32;
                    if x > 0 && sl[j - 1] != NONE && joined(i, i - 1) {
                        union(sl, j as u32, (j - 1) as u32);
                    }
                    if y > 0 && sl[j - nx] != NONE && joined(i, i - nx) {
                        union(sl, j as u32, (j - nx) as u32);
                    }
                }
            }
            // local indices back to global ones, every entry its root
            for j in 0..plane {
                let p = sl[j];
                if p == NONE {
                    continue;
                }
                let r = if p as usize == j { p } else { sl[p as usize] };
                sl[j] = r;
            }
            for v in sl.iter_mut() {
                if *v != NONE {
                    *v = (*v as usize + base) as u32;
                }
            }
        });
    if !three_d || nz < 2 {
        return;
    }
    for z in 1..nz {
        let base = z * plane;
        for j in 0..plane {
            let i = base + j;
            if parent[i] != NONE && parent[i - plane] != NONE && joined(i, i - plane) {
                union(parent, i as u32, (i - plane) as u32);
            }
        }
    }
    // every entry its root (parents precede their children)
    for i in 0..parent.len() {
        let p = parent[i];
        if p != NONE && p as usize != i {
            parent[i] = parent[p as usize];
        }
    }
}

/// The largest component of a labelling ([`label`]'s roots), among equal sizes the one whose
/// first voxel comes first: its root, `None` when there is no member. Within `range` (a slice's
/// voxels in the slices mode, else every voxel).
pub fn largest(parent: &[u32], range: std::ops::Range<usize>, counts: &mut [u32]) -> Option<u32> {
    for &p in &parent[range.clone()] {
        if p != NONE {
            counts[p as usize] = 0;
        }
    }
    for &p in &parent[range.clone()] {
        if p != NONE {
            counts[p as usize] += 1;
        }
    }
    let mut best: Option<(u32, u32)> = None;
    for i in range {
        if parent[i] == i as u32 {
            let c = counts[i];
            if best.is_none_or(|(_, bc)| c > bc) {
                best = Some((i as u32, c));
            }
        }
    }
    best.map(|(r, _)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_by_first_voxel() {
        // 4 x 3 x 2: two components in slice 0 joined through slice 1
        let shape = [4, 3, 2];
        let on = [
            1, 0, 1, 0, //
            0, 0, 1, 0, //
            0, 0, 0, 0, //
            1, 1, 1, 0, //
            0, 0, 0, 0, //
            0, 0, 0, 1, //
        ];
        let mut parent = vec![0u32; 24];
        label(shape, |i| on[i] == 1, |_, _| true, true, &mut parent);
        assert_eq!(parent[0], 0);
        assert_eq!(parent[2], 0);
        assert_eq!(parent[6], 0);
        assert_eq!(parent[14], 0);
        assert_eq!(parent[23], 23);
        assert_eq!(parent[1], NONE);
        let mut counts = vec![0u32; 24];
        assert_eq!(largest(&parent, 0..24, &mut counts), Some(0));
        let mut p2 = vec![0u32; 24];
        label(shape, |i| on[i] == 1, |_, _| true, false, &mut p2);
        assert_eq!(p2[2], 2);
        assert_eq!(p2[14], 12);
        assert_eq!(largest(&p2, 0..12, &mut counts), Some(2));
        // equal sizes: the first one
        assert_eq!(largest(&p2, 12..24, &mut counts), Some(12));
        let one = [1, 0, 1, 0];
        let mut p3 = vec![0u32; 4];
        label([4, 1, 1], |i| one[i] == 1, |_, _| true, true, &mut p3);
        assert_eq!(largest(&p3, 0..4, &mut counts), Some(0));
    }
}
