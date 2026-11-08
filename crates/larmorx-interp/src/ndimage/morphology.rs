// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy 1.18.1's binary morphology and labelling with the default structure (the cross of
//! `generate_binary_structure(3, 1)`: a voxel and its six face neighbours), as nilearn's mask
//! post-processing calls them (`scipy/ndimage/_morphology.py`, `src/ni_morphology.c`,
//! `_measurements.py`, `src/_ni_label.pyx`; BSD-3-Clause, the notice in NOTICE).
//!
//! - [`binary_erosion`]: a voxel stays set when it and its six neighbours are set; outside the
//!   array counts as unset (`border_value=0`).
//! - [`binary_dilation`]: a voxel becomes set when it or a neighbour is set; outside counts as
//!   unset (SciPy inverts the border value for its inverted erosion).
//! - `iterations` applies the operation that many times (SciPy's coordinate-list algorithm for
//!   `iterations > 1` gives the same result).
//! - [`label`]: face-connected components numbered 1, 2, ... in the order of their first voxel
//!   in C order (the last axis fastest), as SciPy's line scan with its merge table (each region
//!   keeps its smallest provisional label, provisional labels growing along the scan) numbers
//!   them.
//!
//! Arrays are Fortran-ordered `[n0, n1, n2]` (axis 0 fastest), as in larmorx.

fn neighbours(shape: [usize; 3], i: usize) -> impl Iterator<Item = Option<usize>> {
    let [n0, n1, n2] = shape;
    let (x, y, z) = (i % n0, (i / n0) % n1, i / (n0 * n1));
    let s1 = n0;
    let s2 = n0 * n1;
    [
        (x > 0).then(|| i - 1),
        (x + 1 < n0).then(|| i + 1),
        (y > 0).then(|| i - s1),
        (y + 1 < n1).then(|| i + s1),
        (z > 0).then(|| i - s2),
        (z + 1 < n2).then(|| i + s2),
    ]
    .into_iter()
}

fn erode_once(m: &[bool], shape: [usize; 3]) -> Vec<bool> {
    (0..m.len())
        .map(|i| m[i] && neighbours(shape, i).all(|n| n.is_some_and(|j| m[j])))
        .collect()
}

fn dilate_once(m: &[bool], shape: [usize; 3]) -> Vec<bool> {
    (0..m.len())
        .map(|i| m[i] || neighbours(shape, i).any(|n| n.is_some_and(|j| m[j])))
        .collect()
}

/// `scipy.ndimage.binary_erosion(m, iterations=iterations)` (`iterations >= 1`).
pub fn binary_erosion(m: &[bool], shape: [usize; 3], iterations: usize) -> Vec<bool> {
    assert_eq!(m.len(), shape.iter().product::<usize>());
    let mut cur = m.to_vec();
    for _ in 0..iterations.max(1) {
        cur = erode_once(&cur, shape);
    }
    cur
}

/// `scipy.ndimage.binary_dilation(m, iterations=iterations)` (`iterations >= 1`).
pub fn binary_dilation(m: &[bool], shape: [usize; 3], iterations: usize) -> Vec<bool> {
    assert_eq!(m.len(), shape.iter().product::<usize>());
    let mut cur = m.to_vec();
    for _ in 0..iterations.max(1) {
        cur = dilate_once(&cur, shape);
    }
    cur
}

/// `scipy.ndimage.label(m)`: the labels (0 for unset voxels) and their number.
pub fn label(m: &[bool], shape: [usize; 3]) -> (Vec<u32>, u32) {
    let [n0, n1, n2] = shape;
    assert_eq!(m.len(), n0 * n1 * n2);
    let mut labels = vec![0u32; m.len()];
    let mut next = 0u32;
    let mut stack = Vec::new();
    // C order: axis 2 fastest, axis 0 slowest.
    for x in 0..n0 {
        for y in 0..n1 {
            for z in 0..n2 {
                let i = x + n0 * (y + n1 * z);
                if !m[i] || labels[i] != 0 {
                    continue;
                }
                next += 1;
                labels[i] = next;
                stack.push(i);
                while let Some(v) = stack.pop() {
                    for j in neighbours(shape, v).flatten() {
                        if m[j] && labels[j] == 0 {
                            labels[j] = next;
                            stack.push(j);
                        }
                    }
                }
            }
        }
    }
    (labels, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morphology_of_a_block() {
        let shape = [5, 5, 5];
        let mut m = vec![false; 125];
        for x in 1..4 {
            for y in 1..4 {
                for z in 1..4 {
                    m[x + 5 * (y + 5 * z)] = true;
                }
            }
        }
        let e = binary_erosion(&m, shape, 1);
        assert_eq!(e.iter().filter(|&&b| b).count(), 1);
        let d = binary_dilation(&e, shape, 1);
        assert_eq!(d.iter().filter(|&&b| b).count(), 7);
        // the whole array set: the border erodes (outside is unset)
        let full = vec![true; 27];
        assert_eq!(
            binary_erosion(&full, [3, 3, 3], 1)
                .iter()
                .filter(|&&b| b)
                .count(),
            1
        );
    }

    #[test]
    fn labels_follow_c_order() {
        // two voxels: (0, 0, 1) comes first in C order, (1, 0, 0) second
        let shape = [2, 1, 2];
        let mut m = vec![false; 4];
        m[1] = true; // x = 1, z = 0
        m[2] = true; // x = 0, z = 1
        let (l, n) = label(&m, shape);
        assert_eq!(n, 2);
        assert_eq!(l[2], 1);
        assert_eq!(l[1], 2);
    }
}
