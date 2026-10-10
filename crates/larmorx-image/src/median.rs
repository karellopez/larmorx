// SPDX-License-Identifier: Apache-2.0
//! Median filtering, as ITK v5.4.5's `MedianImageFilter` computes it: the median of the
//! `∏(2·radius + 1)` voxels of a box around each voxel, the nearest edge voxel standing in for
//! those outside the image (`ZeroFluxNeumannBoundaryCondition`). The box has an odd number of
//! voxels, so the median is one of them (`std::nth_element` at the middle).
//!
//! The median *value* does not depend on how the selection is done, with two exceptions where
//! ITK's result depends on the order `std::nth_element` happens to leave: `-0.0` and `+0.0`
//! compare equal in ITK, and NaN is unordered. larmorx orders with `f32::total_cmp`
//! (`-0.0 < +0.0`, NaN last). ANTs never sees a NaN from a NIfTI file (the reader turns them
//! into 0).

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::volume::{VolumeRef, strides};

/// `MedianImageFilter<float, float>` with a box of `radius` voxels on each axis.
pub fn median(
    input: VolumeRef<'_, f32>,
    radius: &[usize],
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = input.dim();
    if radius.len() != d {
        return Err(FilterError::invalid(format!(
            "{} radii for a {d}-dimensional image",
            radius.len()
        )));
    }
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let size = input.size;
    let stride = strides(size);
    // The box's offsets, first axis fastest.
    let mut offsets: Vec<Vec<isize>> = vec![Vec::new()];
    for &r in radius {
        let r = r as isize;
        offsets = (-r..=r)
            .flat_map(|o| {
                offsets.iter().map(move |prefix| {
                    let mut v = prefix.clone();
                    v.push(o);
                    v
                })
            })
            .collect();
    }
    // (The order of the offsets does not change the median.)
    let k = offsets.len();
    let nx = size[0];
    // Floats as integers in the order of `f32::total_cmp`, which select faster.
    let key = |v: f32| {
        let b = v.to_bits();
        if b >> 31 == 1 { !b } else { b | 0x8000_0000 }
    };
    let value = |k: u32| f32::from_bits(if k >> 31 == 1 { k & 0x7fff_ffff } else { !k });
    let mut out = vec![0.0f32; input.len()];
    parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx).enumerate().for_each_init(
            || (vec![0usize; k], vec![0u32; k]),
            |(bases, values), (row, out_row)| {
                // The row's coordinates on the other axes.
                let mut coord = vec![0usize; d];
                let mut rest = row;
                for a in 1..d {
                    coord[a] = rest % size[a];
                    rest /= size[a];
                }
                for (base, off) in bases.iter_mut().zip(&offsets) {
                    let mut b = 0usize;
                    for a in 1..d {
                        let c = (coord[a] as isize + off[a]).clamp(0, size[a] as isize - 1);
                        b += c as usize * stride[a];
                    }
                    *base = b;
                }
                for (x, o) in out_row.iter_mut().enumerate() {
                    for ((v, &base), off) in values.iter_mut().zip(bases.iter()).zip(&offsets) {
                        let xi = (x as isize + off[0]).clamp(0, nx as isize - 1) as usize;
                        *v = key(input.data[base + xi]);
                    }
                    let (_, m, _) = values.select_nth_unstable(k / 2);
                    *o = value(*m);
                }
            },
        );
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute(data: &[f32], size: &[usize], radius: &[usize]) -> Vec<f32> {
        let d = size.len();
        let stride = strides(size);
        (0..data.len())
            .map(|i| {
                let coord: Vec<isize> = (0..d)
                    .map(|a| ((i / stride[a]) % size[a]) as isize)
                    .collect();
                let mut vals = Vec::new();
                let mut off = vec![0isize; d];
                let total: usize = radius.iter().map(|r| 2 * r + 1).product();
                for mut t in 0..total {
                    for a in 0..d {
                        let w = 2 * radius[a] + 1;
                        off[a] = (t % w) as isize - radius[a] as isize;
                        t /= w;
                    }
                    let idx: usize = (0..d)
                        .map(|a| {
                            (coord[a] + off[a]).clamp(0, size[a] as isize - 1) as usize * stride[a]
                        })
                        .sum();
                    vals.push(data[idx]);
                }
                vals.sort_by(f32::total_cmp);
                vals[vals.len() / 2]
            })
            .collect()
    }

    #[test]
    fn matches_a_brute_force_median() {
        for (size, radius) in [
            (vec![9usize, 7, 4], vec![1usize, 1, 1]),
            (vec![6, 5], vec![2, 1]),
            (vec![5, 4, 3, 3], vec![1, 0, 1, 1]),
        ] {
            let len: usize = size.iter().product();
            let data: Vec<f32> = (0..len).map(|i| ((i * 7919) % 101) as f32 - 20.0).collect();
            let spacing = vec![1.0; size.len()];
            let v = VolumeRef::new(&data, &size, &spacing);
            let expected = brute(&data, &size, &radius);
            for threads in [1, 3] {
                assert_eq!(median(v, &radius, threads).unwrap(), expected, "{size:?}");
            }
        }
    }
}
