// SPDX-License-Identifier: Apache-2.0
//! Box-neighbourhood statistics of `float` images: ITK's `MeanImageFilter` (ITK v5.4.5) and
//! the `VarianceImageFilter` of ITK's remote module AdaptiveDenoising (commit `24825c8d`, the
//! one ITK v5.4.5 pins), as `AdaptiveNonLocalMeansDenoisingImageFilter` runs them.
//!
//! Both visit the `(2r + 1)^D` neighbours of each voxel in ITK's neighbourhood order (the first
//! axis fastest, each offset from `−r` to `r`), take the nearest edge voxel outside the image
//! (`ZeroFluxNeumannBoundaryCondition`), and sum in double (`NumericTraits<float>::RealType`):
//! - mean: `float(Σ v / n)`;
//! - variance: `float((Σ v² − (Σ v)² / n) / (n − 1))`, each `v²` computed in double.
//!
//! Each voxel is independent, so the result does not depend on the number of threads.

use rayon::prelude::*;

use crate::FilterError;
use crate::volume::{VolumeRef, strides};

/// A box neighbourhood of radius `radius` in an image of `size`: the offsets in ITK's order
/// and their distance in memory.
pub(crate) struct BoxNeighborhood {
    /// Per offset, the step along each axis.
    pub offsets: Vec<Vec<isize>>,
    /// Per offset, the step in memory (Fortran order).
    pub linear: Vec<isize>,
}

impl BoxNeighborhood {
    /// The `∏ (2 r_k + 1)` offsets, the first axis fastest, each from `−r_k` to `r_k`
    /// (`Neighborhood::ComputeNeighborhoodOffsetTable`).
    pub fn new(radius: &[usize], size: &[usize]) -> Self {
        let d = radius.len();
        let strides = strides(size);
        let count: usize = radius.iter().map(|&r| 2 * r + 1).product();
        let mut offsets = Vec::with_capacity(count);
        let mut o: Vec<isize> = radius.iter().map(|&r| -(r as isize)).collect();
        for _ in 0..count {
            offsets.push(o.clone());
            for j in 0..d {
                o[j] += 1;
                if o[j] > radius[j] as isize {
                    o[j] = -(radius[j] as isize);
                } else {
                    break;
                }
            }
        }
        let linear = offsets
            .iter()
            .map(|o| o.iter().zip(&strides).map(|(&a, &s)| a * s as isize).sum())
            .collect();
        BoxNeighborhood { offsets, linear }
    }

    pub fn len(&self) -> usize {
        self.offsets.len()
    }
}

/// `reduce` of the neighbours of every voxel (in neighbourhood order), row by row in
/// parallel, with the neighbours of the box clamped to the image
/// (`ZeroFluxNeumannBoundaryCondition`).
fn box_filter(
    input: VolumeRef<'_, f32>,
    radius: &[usize],
    n_threads: usize,
    reduce: impl Fn(&[f32]) -> f32 + Sync,
) -> Result<Vec<f32>, FilterError> {
    let d = input.dim();
    if radius.len() != d {
        return Err(FilterError::invalid(format!(
            "a radius of {} values for a {d}-dimensional image",
            radius.len()
        )));
    }
    let size = input.size;
    let len = input.len();
    let mut out = vec![0.0f32; len];
    if len == 0 {
        return Ok(out);
    }
    let nb = BoxNeighborhood::new(radius, size);
    let st = strides(size);
    let nx = size[0];
    let data = input.data;
    larmorx_core::parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx).enumerate().for_each(|(row, line)| {
            let mut coords = vec![0usize; d];
            let mut rest = row;
            for k in 1..d {
                coords[k] = rest % size[k];
                rest /= size[k];
            }
            let row_inside =
                (1..d).all(|k| coords[k] >= radius[k] && coords[k] + radius[k] < size[k]);
            let base = row * nx;
            let mut values = vec![0.0f32; nb.len()];
            for (x, o) in line.iter_mut().enumerate() {
                coords[0] = x;
                if row_inside && x >= radius[0] && x + radius[0] < nx {
                    let centre = (base + x) as isize;
                    for (v, &l) in values.iter_mut().zip(&nb.linear) {
                        *v = data[(centre + l) as usize];
                    }
                } else {
                    for (v, off) in values.iter_mut().zip(&nb.offsets) {
                        let mut index = 0usize;
                        for k in 0..d {
                            let c = (coords[k] as isize + off[k]).clamp(0, size[k] as isize - 1);
                            index += c as usize * st[k];
                        }
                        *v = data[index];
                    }
                }
                *o = reduce(&values);
            }
        });
    })?;
    Ok(out)
}

/// ITK's `MeanImageFilter<float, float>` with a box of `radius`: the mean of the neighbours in
/// double, rounded to float.
pub fn mean_filter(
    input: VolumeRef<'_, f32>,
    radius: &[usize],
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let n = radius.iter().map(|&r| 2 * r + 1).product::<usize>() as f64;
    box_filter(input, radius, n_threads, |values| {
        let mut sum = 0.0f64;
        for &v in values {
            sum += f64::from(v);
        }
        (sum / n) as f32
    })
}

/// AdaptiveDenoising's `VarianceImageFilter<float, float>` with a box of `radius`: the sample
/// variance of the neighbours, `(Σ v² − (Σ v)² / n) / (n − 1)` in double, rounded to float.
pub fn variance_filter(
    input: VolumeRef<'_, f32>,
    radius: &[usize],
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let n = radius.iter().map(|&r| 2 * r + 1).product::<usize>() as f64;
    box_filter(input, radius, n_threads, |values| {
        let mut sum = 0.0f64;
        let mut sum_of_squares = 0.0f64;
        for &v in values {
            let v = f64::from(v);
            sum += v;
            sum_of_squares += v * v;
        }
        ((sum_of_squares - sum * sum / n) / (n - 1.0)) as f32
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_follow_itk_order() {
        let nb = BoxNeighborhood::new(&[1, 1], &[5, 4]);
        assert_eq!(nb.len(), 9);
        assert_eq!(nb.offsets[0], [-1, -1]);
        assert_eq!(nb.offsets[1], [0, -1]);
        assert_eq!(nb.offsets[3], [-1, 0]);
        assert_eq!(nb.offsets[8], [1, 1]);
        assert_eq!(nb.linear, [-6, -5, -4, -1, 0, 1, 4, 5, 6]);
        let nb = BoxNeighborhood::new(&[2, 0, 1], &[7, 3, 2]);
        assert_eq!(nb.len(), 15);
        assert_eq!(nb.offsets[5], [-2, 0, 0]);
    }

    /// A direct transcription of the definitions, voxel by voxel with clamped coordinates.
    fn reference(data: &[f32], size: &[usize], r: usize, variance: bool) -> Vec<f32> {
        let (nx, ny, nz) = (size[0] as isize, size[1] as isize, size[2] as isize);
        let at = |x: isize, y: isize, z: isize| {
            let c = |v: isize, n: isize| v.clamp(0, n - 1) as usize;
            data[c(x, nx) + size[0] * (c(y, ny) + size[1] * c(z, nz))]
        };
        let r = r as isize;
        let n = ((2 * r + 1) * (2 * r + 1) * (2 * r + 1)) as f64;
        let mut out = Vec::new();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let (mut s, mut q) = (0.0f64, 0.0f64);
                    for dz in -r..=r {
                        for dy in -r..=r {
                            for dx in -r..=r {
                                let v = f64::from(at(x + dx, y + dy, z + dz));
                                s += v;
                                q += v * v;
                            }
                        }
                    }
                    out.push(if variance {
                        ((q - s * s / n) / (n - 1.0)) as f32
                    } else {
                        (s / n) as f32
                    });
                }
            }
        }
        out
    }

    #[test]
    fn mean_and_variance_match_their_definitions() {
        let size = [9usize, 6, 5];
        let spacing = [1.0, 1.0, 2.0];
        let data: Vec<f32> = (0..270)
            .map(|i| ((i * 37) % 23) as f32 * 0.37 + 1.0)
            .collect();
        let v = VolumeRef::new(&data, &size, &spacing);
        for r in [1usize, 2] {
            let m = mean_filter(v, &[r; 3], 1).unwrap();
            assert_eq!(m, reference(&data, &size, r, false));
            assert_eq!(m, mean_filter(v, &[r; 3], 3).unwrap());
            let var = variance_filter(v, &[r; 3], 1).unwrap();
            assert_eq!(var, reference(&data, &size, r, true));
            assert_eq!(var, variance_filter(v, &[r; 3], 4).unwrap());
        }
    }
}
