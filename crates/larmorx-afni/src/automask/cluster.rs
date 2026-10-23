// SPDX-License-Identifier: Apache-2.0
//! Connected clusters of a mask (`specs/3dAutomask.md` §5.2 and §7).
//!
//! The method is run-based labelling: each row (x fastest) is cut into runs of set voxels,
//! and a run is joined (union-find) with the runs of the previous row and the previous plane
//! that touch it face to face. Runs are numbered in storage order and a union always keeps the
//! smaller number as the root, so a cluster's root is the run holding its first voxel in
//! storage order: the spec's tie rule (the largest cluster, the one whose first voxel comes
//! first when several have the largest size) is then a comparison of root numbers. The work is
//! linear in the number of runs and rows, and touches the mask once to cut the runs and once
//! to clear the dropped clusters.

/// A run of set voxels: `len` voxels from storage index `start`, in row `row`.
#[derive(Clone, Copy, Debug)]
struct Run {
    start: usize,
    len: u32,
    /// x of the first voxel.
    x0: u32,
}

/// The runs of a mask, row by row.
struct Runs {
    runs: Vec<Run>,
    /// `rows[r]..rows[r + 1]` are the runs of row `r` (`r = j + ny * k`).
    rows: Vec<usize>,
}

fn runs_of(mask: &[u8], nx: usize, nrows: usize) -> Runs {
    let mut runs = Vec::new();
    let mut rows = Vec::with_capacity(nrows + 1);
    for r in 0..nrows {
        rows.push(runs.len());
        let row = &mask[r * nx..(r + 1) * nx];
        let mut i = 0;
        while i < nx {
            if row[i] != 0 {
                let x0 = i;
                while i < nx && row[i] != 0 {
                    i += 1;
                }
                runs.push(Run {
                    start: r * nx + x0,
                    len: (i - x0) as u32,
                    x0: x0 as u32,
                });
            } else {
                i += 1;
            }
        }
    }
    rows.push(runs.len());
    Runs { runs, rows }
}

fn find(parent: &mut [u32], mut a: u32) -> u32 {
    let mut root = a;
    while parent[root as usize] != root {
        root = parent[root as usize];
    }
    while parent[a as usize] != root {
        let next = parent[a as usize];
        parent[a as usize] = root;
        a = next;
    }
    root
}

fn union(parent: &mut [u32], a: u32, b: u32) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
        parent[hi as usize] = lo;
    }
}

/// Joins the runs of rows `a` and `b` that share an x position (face neighbours).
fn join_rows(runs: &Runs, parent: &mut [u32], a: usize, b: usize) {
    let (mut p, pe) = (runs.rows[a], runs.rows[a + 1]);
    let (mut q, qe) = (runs.rows[b], runs.rows[b + 1]);
    while p < pe && q < qe {
        let (ra, rb) = (&runs.runs[p], &runs.runs[q]);
        let (a0, a1) = (ra.x0, ra.x0 + ra.len);
        let (b0, b1) = (rb.x0, rb.x0 + rb.len);
        if a0 < b1 && b0 < a1 {
            union(parent, p as u32, q as u32);
        }
        if a1 <= b1 {
            p += 1;
        } else {
            q += 1;
        }
    }
}

/// The clusters of `mask` (a grid of `dims`, x fastest), face-connected (6 neighbours in 3D,
/// 4 in a plane): for every run, its cluster's root run, and per root run the cluster's size.
struct Clusters {
    runs: Runs,
    /// The root run of each run.
    root: Vec<u32>,
    /// The size of the cluster of each root run (0 for runs that are not roots).
    size: Vec<usize>,
}

fn clusters(mask: &[u8], dims: [usize; 3]) -> Clusters {
    let [nx, ny, nz] = dims;
    let runs = runs_of(mask, nx, ny * nz);
    let n = runs.runs.len();
    assert!(u32::try_from(n).is_ok(), "too many runs");
    let mut parent: Vec<u32> = (0..n as u32).collect();
    for k in 0..nz {
        for j in 0..ny {
            let r = j + ny * k;
            if j > 0 {
                join_rows(&runs, &mut parent, r - 1, r);
            }
            if k > 0 {
                join_rows(&runs, &mut parent, r - ny, r);
            }
        }
    }
    let mut root = vec![0u32; n];
    let mut size = vec![0usize; n];
    for (i, (slot, run)) in root.iter_mut().zip(&runs.runs).enumerate() {
        let r = find(&mut parent, i as u32);
        *slot = r;
        size[r as usize] += run.len as usize;
    }
    Clusters { runs, root, size }
}

/// Keeps only the largest face-connected cluster of `mask` (6 neighbours; spec §5.2): when
/// several have the largest size, the one whose first voxel comes first in storage order.
/// Returns its size (0 for an empty mask).
pub fn keep_largest(mask: &mut [u8], dims: [usize; 3]) -> usize {
    let c = clusters(mask, dims);
    // Roots in increasing order: the first root of the largest size wins.
    let mut best: Option<(usize, u32)> = None;
    for (r, &s) in c.size.iter().enumerate() {
        if s > 0 && best.is_none_or(|(bs, _)| s > bs) {
            best = Some((s, r as u32));
        }
    }
    let Some((size, keep)) = best else {
        return 0;
    };
    for (run, &r) in c.runs.runs.iter().zip(&c.root) {
        if r != keep {
            mask[run.start..run.start + run.len as usize].fill(0);
        }
    }
    size
}

/// The sizes of the face-connected clusters of `mask`, and the size of the largest.
pub fn largest_size(mask: &[u8], dims: [usize; 3]) -> usize {
    clusters(mask, dims).size.iter().copied().max().unwrap_or(0)
}

/// Keeps every face-connected cluster of `mask` with at least `min_size` voxels (spec §7's
/// clusters of a plane: 4 neighbours when `nz` is 1).
pub fn keep_at_least(mask: &mut [u8], dims: [usize; 3], min_size: usize) {
    let c = clusters(mask, dims);
    for (run, &r) in c.runs.runs.iter().zip(&c.root) {
        if c.size[r as usize] < min_size {
            mask[run.start..run.start + run.len as usize].fill(0);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A plain flood fill: the clusters in order of their first voxel, as (first, size), and
    /// each voxel's cluster number.
    pub(crate) fn flood(mask: &[u8], dims: [usize; 3]) -> (Vec<(usize, usize)>, Vec<usize>) {
        let [nx, ny, nz] = dims;
        let mut label = vec![usize::MAX; mask.len()];
        let mut found = Vec::new();
        for start in 0..mask.len() {
            if mask[start] == 0 || label[start] != usize::MAX {
                continue;
            }
            let id = found.len();
            let mut stack = vec![start];
            label[start] = id;
            let mut size = 0;
            while let Some(p) = stack.pop() {
                size += 1;
                let (i, j, k) = (p % nx, (p / nx) % ny, p / (nx * ny));
                let mut nb = Vec::new();
                if i > 0 {
                    nb.push(p - 1);
                }
                if i + 1 < nx {
                    nb.push(p + 1);
                }
                if j > 0 {
                    nb.push(p - nx);
                }
                if j + 1 < ny {
                    nb.push(p + nx);
                }
                if k > 0 {
                    nb.push(p - nx * ny);
                }
                if k + 1 < nz {
                    nb.push(p + nx * ny);
                }
                for q in nb {
                    if mask[q] != 0 && label[q] == usize::MAX {
                        label[q] = id;
                        stack.push(q);
                    }
                }
            }
            found.push((start, size));
        }
        (found, label)
    }

    pub(crate) fn random_mask(dims: [usize; 3], density: u64, seed: u64) -> Vec<u8> {
        let mut s = seed | 1;
        (0..dims.iter().product::<usize>())
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                u8::from(s % 100 < density)
            })
            .collect()
    }

    #[test]
    fn largest_against_flood_fill() {
        for (t, dims) in [[7, 5, 3], [16, 9, 6], [1, 12, 7], [13, 1, 1], [9, 9, 1]]
            .into_iter()
            .enumerate()
        {
            for density in [20, 45, 60, 85] {
                let mut m = random_mask(dims, density, 77 + t as u64 * 1000 + density);
                let (found, label) = flood(&m, dims);
                let mut best: Option<(usize, usize)> = None;
                for (id, &(_, s)) in found.iter().enumerate() {
                    if best.is_none_or(|(bs, _)| s > bs) {
                        best = Some((s, id));
                    }
                }
                let expected: Vec<u8> = label
                    .iter()
                    .map(|&l| u8::from(best.is_some_and(|(_, id)| l == id)))
                    .collect();
                let size = keep_largest(&mut m, dims);
                assert_eq!(size, best.map_or(0, |b| b.0));
                assert_eq!(m, expected, "{dims:?} {density}");
            }
        }
    }

    #[test]
    fn ties_keep_the_first_cluster() {
        // Two clusters of 2 voxels: the first in storage order is kept.
        let dims = [5, 1, 1];
        let mut m = vec![1, 1, 0, 1, 1];
        assert_eq!(keep_largest(&mut m, dims), 2);
        assert_eq!(m, [1, 1, 0, 0, 0]);
        // A U shape whose two arms join in a later row: one cluster.
        let dims = [3, 3, 1];
        let mut m = vec![1, 0, 1, 1, 0, 1, 1, 1, 1];
        assert_eq!(keep_largest(&mut m, dims), 7);
        let mut e = vec![0u8; 4];
        assert_eq!(keep_largest(&mut e, [2, 2, 1]), 0);
    }

    #[test]
    fn keep_clusters_of_a_size() {
        let dims = [9, 9, 1];
        for density in [30, 50, 70] {
            let mut m = random_mask(dims, density, density);
            let (found, label) = flood(&m, dims);
            let expected: Vec<u8> = label
                .iter()
                .map(|&l| u8::from(l != usize::MAX && found[l].1 >= 3))
                .collect();
            keep_at_least(&mut m, dims, 3);
            assert_eq!(m, expected);
        }
    }
}
