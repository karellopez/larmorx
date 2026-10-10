// SPDX-License-Identifier: Apache-2.0
//! Connected components, as ITK v5.4.5 labels and orders them:
//! - `ConnectedComponentImageFilter` ([`connected_components`];
//!   `itkConnectedComponentImageFilter.hxx`, `itkScanlineFilterCommon.h`). Non-zero voxels are
//!   foreground; voxels are joined through faces (`FullyConnected` off) or through any voxel
//!   of the `3^D` neighbourhood (on). ITK encodes each row (along the first axis) as runs,
//!   numbers the runs in memory order, links overlapping runs of neighbouring rows to the
//!   smaller number, and gives the components consecutive labels in the order of their
//!   smallest run: **the order of each component's first voxel in memory** (first axis
//!   fastest). The labels do not depend on ITK's threads, and here not on `n_threads`.
//! - `RelabelComponentImageFilter` ([`relabel_components`]; `itkRelabelComponentImageFilter.hxx`):
//!   components sorted by size, largest first, **ties in the order of their labels**;
//!   components smaller than the minimum size (if it is not 0) become background; the others
//!   are numbered 1, 2, ... in that order.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::volume::strides;

/// A run of foreground voxels along the first axis: `[start, end]` in the row.
#[derive(Clone, Copy, Debug)]
struct Run {
    start: u32,
    end: u32,
}

/// The labels of [`connected_components`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Components {
    /// One label per voxel: 0 for background, components numbered from 1.
    pub labels: Vec<u32>,
    /// The number of components.
    pub count: usize,
}

fn find(parent: &mut [usize], mut a: usize) -> usize {
    while parent[a] != a {
        parent[a] = parent[parent[a]];
        a = parent[a];
    }
    a
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra < rb {
        parent[rb] = ra;
    } else if rb < ra {
        parent[ra] = rb;
    }
}

/// `ConnectedComponentImageFilter` on an image of `size` whose foreground voxels are those
/// with `foreground[i]`: labels in the order of each component's first voxel in memory.
pub fn connected_components(
    foreground: &[bool],
    size: &[usize],
    fully_connected: bool,
    n_threads: usize,
) -> Result<Components, FilterError> {
    let n: usize = size.iter().product();
    if foreground.len() != n {
        return Err(FilterError::SizeMismatch {
            what: "foreground",
            actual: foreground.len(),
            expected: n,
        });
    }
    if n == 0 {
        return Ok(Components {
            labels: Vec::new(),
            count: 0,
        });
    }
    let nx = size[0];
    if nx > u32::MAX as usize {
        return Err(FilterError::invalid("rows longer than 2^32 voxels"));
    }
    let n_rows = n / nx;
    // 1. The runs of every row.
    let rows: Vec<Vec<Run>> = parallel::with_threads(n_threads, || {
        foreground
            .par_chunks(nx)
            .with_min_len(64)
            .map(|row| {
                let mut runs = Vec::new();
                let mut x = 0;
                while x < nx {
                    if row[x] {
                        let start = x;
                        while x < nx && row[x] {
                            x += 1;
                        }
                        runs.push(Run {
                            start: start as u32,
                            end: (x - 1) as u32,
                        });
                    } else {
                        x += 1;
                    }
                }
                runs
            })
            .collect()
    })?;
    let mut first_run = Vec::with_capacity(n_rows + 1);
    let mut total = 0usize;
    for r in &rows {
        first_run.push(total);
        total += r.len();
    }
    first_run.push(total);
    // 2. Link runs of neighbouring rows: the previous rows of the neighbourhood on the other
    // axes (faces only, or every combination of -1, 0, +1).
    let row_size = &size[1..];
    let row_strides = strides(row_size);
    let d = row_size.len();
    let mut neighbours: Vec<Vec<isize>> = Vec::new();
    let mut o = vec![-1isize; d];
    if d > 0 {
        loop {
            let nonzero = o.iter().filter(|&&v| v != 0).count();
            let linear: isize = o
                .iter()
                .zip(&row_strides)
                .map(|(&v, &s)| v * s as isize)
                .sum();
            if linear < 0 && (fully_connected || nonzero == 1) {
                neighbours.push(o.clone());
            }
            let mut a = 0;
            while a < d && o[a] == 1 {
                o[a] = -1;
                a += 1;
            }
            if a == d {
                break;
            }
            o[a] += 1;
        }
    }
    let reach = u32::from(fully_connected);
    let mut parent: Vec<usize> = (0..total).collect();
    let mut coord = vec![0usize; d];
    for (row, runs) in rows.iter().enumerate() {
        if !runs.is_empty() {
            for offset in &neighbours {
                let mut other = 0usize;
                let mut inside = true;
                for a in 0..d {
                    let c = coord[a] as isize + offset[a];
                    if c < 0 || c >= row_size[a] as isize {
                        inside = false;
                        break;
                    }
                    other += c as usize * row_strides[a];
                }
                if !inside || rows[other].is_empty() {
                    continue;
                }
                let theirs = &rows[other];
                let (mut i, mut j) = (0, 0);
                while i < runs.len() && j < theirs.len() {
                    let (a, b) = (runs[i], theirs[j]);
                    if a.start <= b.end + reach && b.start <= a.end + reach {
                        union(&mut parent, first_run[row] + i, first_run[other] + j);
                    }
                    if a.end < b.end {
                        i += 1;
                    } else {
                        j += 1;
                    }
                }
            }
        }
        // Next row's coordinates.
        for a in 0..d {
            coord[a] += 1;
            if coord[a] < row_size[a] {
                break;
            }
            coord[a] = 0;
        }
    }
    // 3. Consecutive labels in the order of each component's first run.
    let mut label_of_root = vec![0u32; total];
    let mut run_label = vec![0u32; total];
    let mut count = 0usize;
    for (run, label) in run_label.iter_mut().enumerate() {
        let root = find(&mut parent, run);
        if label_of_root[root] == 0 {
            count += 1;
            label_of_root[root] = u32::try_from(count)
                .map_err(|_| FilterError::invalid("more than 2^32 - 1 components"))?;
        }
        *label = label_of_root[root];
    }
    let mut labels = vec![0u32; n];
    parallel::with_threads(n_threads, || {
        labels
            .par_chunks_mut(nx)
            .enumerate()
            .with_min_len(64)
            .for_each(|(row, out)| {
                for (k, run) in rows[row].iter().enumerate() {
                    let l = run_label[first_run[row] + k];
                    out[run.start as usize..=run.end as usize].fill(l);
                }
            });
    })?;
    Ok(Components { labels, count })
}

/// The result of [`relabel_components`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relabeling {
    /// The new label of each old label (`map[0] = 0`; removed components map to 0).
    pub map: Vec<u32>,
    /// The size in voxels of each kept component, by new label (`sizes[k - 1]` for label `k`).
    pub sizes: Vec<u64>,
    /// The number of components before removing the small ones.
    pub original_count: usize,
}

/// The number of voxels of each label `0..=count` (deterministic integer sums).
pub fn label_sizes(
    labels: &[u32],
    count: usize,
    n_threads: usize,
) -> Result<Vec<u64>, FilterError> {
    const CHUNK: usize = 1 << 18;
    let partial: Vec<Vec<u64>> = parallel::with_threads(n_threads, || {
        labels
            .par_chunks(CHUNK)
            .map(|chunk| {
                let mut h = vec![0u64; count + 1];
                for &l in chunk {
                    h[l as usize] += 1;
                }
                h
            })
            .collect()
    })?;
    let mut sizes = vec![0u64; count + 1];
    for h in partial {
        for (s, v) in sizes.iter_mut().zip(h) {
            *s += v;
        }
    }
    Ok(sizes)
}

/// `RelabelComponentImageFilter` on `labels` (0 background, components `1..=count`): sizes
/// in voxels, the order by decreasing size (ties by increasing label), and the components
/// smaller than `minimum_size` removed when `minimum_size > 0`.
pub fn relabel_components(
    labels: &[u32],
    count: usize,
    minimum_size: u64,
    n_threads: usize,
) -> Result<Relabeling, FilterError> {
    if let Some(&l) = labels.iter().find(|&&l| l as usize > count) {
        return Err(FilterError::invalid(format!(
            "label {l} is above the component count {count}"
        )));
    }
    let all = label_sizes(labels, count, n_threads)?;
    // The labels present (ITK counts only labels that occur).
    let mut present: Vec<(u32, u64)> = (1..=count)
        .filter(|&l| all[l] > 0)
        .map(|l| (l as u32, all[l]))
        .collect();
    present.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut map = vec![0u32; count + 1];
    let mut sizes = Vec::new();
    for &(label, size) in &present {
        if minimum_size > 0 && size < minimum_size {
            continue;
        }
        sizes.push(size);
        map[label as usize] = sizes.len() as u32;
    }
    Ok(Relabeling {
        map,
        sizes,
        original_count: present.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flood fill in memory order, the definition.
    fn brute(fg: &[bool], size: &[usize], full: bool) -> (Vec<u32>, usize) {
        let st = strides(size);
        let d = size.len();
        let mut labels = vec![0u32; fg.len()];
        let mut count = 0;
        for start in 0..fg.len() {
            if !fg[start] || labels[start] != 0 {
                continue;
            }
            count += 1;
            labels[start] = count as u32;
            let mut stack = vec![start];
            while let Some(i) = stack.pop() {
                let c: Vec<isize> = (0..d).map(|a| ((i / st[a]) % size[a]) as isize).collect();
                let total = 3usize.pow(d as u32);
                for mut t in 0..total {
                    let mut o = vec![0isize; d];
                    for v in o.iter_mut() {
                        *v = (t % 3) as isize - 1;
                        t /= 3;
                    }
                    let nz = o.iter().filter(|&&v| v != 0).count();
                    if nz == 0 || (!full && nz > 1) {
                        continue;
                    }
                    let mut j = 0usize;
                    let mut ok = true;
                    for a in 0..d {
                        let p = c[a] + o[a];
                        if p < 0 || p >= size[a] as isize {
                            ok = false;
                            break;
                        }
                        j += p as usize * st[a];
                    }
                    if ok && fg[j] && labels[j] == 0 {
                        labels[j] = count as u32;
                        stack.push(j);
                    }
                }
            }
        }
        (labels, count)
    }

    #[test]
    fn components_are_numbered_in_memory_order() {
        for size in [
            vec![17usize, 13, 9],
            vec![23, 19],
            vec![7, 6, 5, 4],
            vec![40],
        ] {
            let n: usize = size.iter().product();
            for density in [3usize, 5, 8] {
                let fg: Vec<bool> = (0..n).map(|i| (i * 7919 + i / 3) % density == 0).collect();
                for full in [false, true] {
                    let (expected, count) = brute(&fg, &size, full);
                    for threads in [1, 4] {
                        let got = connected_components(&fg, &size, full, threads).unwrap();
                        assert_eq!(got.count, count, "{size:?} full {full}");
                        assert_eq!(got.labels, expected, "{size:?} full {full}");
                    }
                }
            }
        }
    }

    #[test]
    fn relabeling_orders_by_size_then_label() {
        // Sizes: label 1 → 2, 2 → 3, 3 → 2, 4 → 1, 5 → 3.
        let labels = [1, 1, 2, 2, 2, 3, 3, 4, 5, 5, 5, 0, 0];
        let r = relabel_components(&labels, 5, 0, 2).unwrap();
        assert_eq!(r.map, vec![0, 3, 1, 4, 5, 2]);
        assert_eq!(r.sizes, vec![3, 3, 2, 2, 1]);
        let r = relabel_components(&labels, 5, 2, 2).unwrap();
        assert_eq!(r.map, vec![0, 3, 1, 4, 0, 2]);
        assert_eq!(r.sizes, vec![3, 3, 2, 2]);
        assert_eq!(r.original_count, 5);
    }
}
