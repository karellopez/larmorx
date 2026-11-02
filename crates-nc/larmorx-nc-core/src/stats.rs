// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (newimage.h: maskedIterator;
// newimage.cc: calculateSums, sum, mean, percentile, calc_percentiles, percentile_vec,
// threshold, binarise). Copyright (C) 1999-2018 University of Oxford (Mark Jenkinson, Matthew
// Webster et al.). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// originals. Distributed under the FSL Licence (see LICENSE).
//! newimage's statistics and thresholds that FAST uses: sums and means over a mask in FSL's
//! blocks, percentiles, `threshold` and `binarise`.
//!
//! A mask is a `volume<float>` read as newimage's `maskedIterator` reads it: a voxel counts when
//! its mask value is above `maskThreshold()` (0.5), the mask cycled over a volume with more
//! time points; an empty mask (no values) means every voxel.

use crate::volume::{FslError, Volume};

/// newimage's `maskDelimiter`.
pub const MASK_THRESHOLD: f64 = 0.5;

/// `calculateSums(values, mask)`'s sum: the masked values, as `double`, added in blocks of
/// `nlim + 1` values (`nlim = max((long) sqrt(n), 10000)`, `n` the volume's number of values),
/// each block's sum then added to the total; and whether no value was in the mask (FSL then
/// prints `ERROR:: Empty mask image` on standard error and the sum is 0).
pub fn masked_sum(values: &[f32], mask: Option<&[f32]>) -> (f64, bool) {
    masked_sum_by(values.len(), |k| values[k], mask)
}

/// [`masked_sum`] of the `len` values `value(k)` (a volume computed voxel by voxel, such as
/// the product of two volumes, summed without storing it).
pub fn masked_sum_by(
    len: usize,
    value: impl Fn(usize) -> f32,
    mask: Option<&[f32]>,
) -> (f64, bool) {
    let nlim = ((len as f64).sqrt() as i64).max(10000) as usize;
    let (mut total, mut sum) = (0f64, 0f64);
    let (mut n, mut nn) = (0usize, 0usize);
    let mut add = |v: f32| {
        sum += f64::from(v);
        n += 1;
        if n > nlim {
            nn += 1;
            total += sum;
            sum = 0.0;
            n = 0;
        }
    };
    match mask.filter(|m| !m.is_empty()) {
        None => (0..len).for_each(|k| add(value(k))),
        Some(m) => {
            for k in 0..len {
                if f64::from(m[k % m.len()]) > MASK_THRESHOLD {
                    add(value(k));
                }
            }
        }
    }
    let empty = n + nn == 0;
    (total + sum, empty)
}

/// The number of masked voxels (`maskedIterator`'s count).
pub fn masked_count(len: usize, mask: Option<&[f32]>) -> usize {
    match mask.filter(|m| !m.is_empty()) {
        None => len,
        Some(m) => (0..len)
            .filter(|&k| f64::from(m[k % m.len()]) > MASK_THRESHOLD)
            .count(),
    }
}

/// The message `calculateSums` prints for an empty mask.
pub const EMPTY_MASK: &str = "ERROR:: Empty mask image";

impl Volume {
    /// `sum(mask)` (or `sum()` with `None`), and whether the mask was empty ([`masked_sum`]).
    /// `Err` is the mask of another size (`maskedIterator`'s `runtime_error`).
    pub fn sum_masked(&self, mask: Option<&Volume>) -> Result<(f64, bool), FslError> {
        check_mask(self, mask, "calculateSums: ")?;
        Ok(masked_sum(&self.data, mask.map(|m| m.data.as_slice())))
    }

    /// `mean(mask)`: `sum(mask) / max(n, 1)`, `n` the masked voxels; and whether the mask was
    /// empty (`sum`'s message).
    pub fn mean_masked(&self, mask: &Volume) -> Result<(f64, bool), FslError> {
        check_mask(self, Some(mask), "mean: ")?;
        let count = masked_count(self.data.len(), Some(&mask.data));
        let (s, empty) = self.sum_masked(Some(mask))?;
        Ok((s / (count as f64).max(1.0), empty))
    }

    /// `percentile(pvalue, mask)` (`None`: `percentile(pvalue)`): the values in the mask
    /// (above 0.5) sorted with `std::sort` (libstdc++'s moves: [`crate::stdsort`]), the one at
    /// `(unsigned int) ((float) n * pvalue)` (at most `n - 1`); 0 for no values. `Err` is FSL's
    /// exception for a `pvalue` outside `[0, 1]` or a mask of another size. The flag says that
    /// a NaN would have taken libstdc++'s sort outside the vector (the NaN values then last).
    pub fn percentile_masked(
        &self,
        pvalue: f32,
        mask: Option<&Volume>,
    ) -> Result<(f32, bool), FslError> {
        if pvalue > 1.0 || pvalue < 0.0 {
            return Err(FslError::imthrow(
                "Percentiles must be in the range [0.0,1.0]",
                4,
            ));
        }
        let mut hist: Vec<f32> = match mask.filter(|m| !m.data.is_empty()) {
            None => self.data.clone(),
            Some(m) => {
                if !same_size_subset(self, m) {
                    return Err(FslError::imthrow(
                        "mask and vol have different sizes in calc_percentiles",
                        3,
                    ));
                }
                let n = m.data.len();
                self.data
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| f64::from(m.data[k % n]) > MASK_THRESHOLD)
                    .map(|(_, &v)| v)
                    .collect()
            }
        };
        let numbins = hist.len() as u32;
        if numbins == 0 {
            return Ok((0.0, false));
        }
        let outside = crate::stdsort::sort_f32_checked(&mut hist).is_err();
        if outside {
            hist.sort_by(|a, b| match (a.is_nan(), b.is_nan()) {
                (false, false) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                (x, y) => x.cmp(&y),
            });
        }
        let mut p = crate::cnum::f32_to_u32(numbins as f32 * pvalue);
        if p >= numbins {
            p = numbins - 1;
        }
        Ok((hist[p as usize], outside))
    }

    /// `threshold(lower, upper, inclusive)`: values outside `[lower, upper]` set to 0.
    pub fn threshold_inclusive(&mut self, lower: f32, upper: f32) {
        for v in &mut self.data {
            if !(*v >= lower && *v <= upper) {
                *v = 0.0;
            }
        }
    }

    /// `binarise(lower, upper, exclusive)`: 1 inside `(lower, upper)`, 0 elsewhere.
    pub fn binarise_exclusive(&mut self, lower: f32, upper: f32) {
        for v in &mut self.data {
            *v = if *v > lower && *v < upper { 1.0 } else { 0.0 };
        }
    }
}

/// `samesize(vol, mask, SUBSET)`: the mask's first three dimensions are the volume's, and its
/// time points at most the volume's.
fn same_size_subset(v: &Volume, m: &Volume) -> bool {
    v.dims[..3] == m.dims[..3] && m.dims[3] <= v.dims[3]
}

fn check_mask(v: &Volume, mask: Option<&Volume>, source: &str) -> Result<(), FslError> {
    match mask {
        Some(m) if !m.data.is_empty() && !same_size_subset(v, m) => Err(FslError {
            printed: Vec::new(),
            type_name: "std::runtime_error",
            what: format!("{source}maskedIterator: mask and volume must be the same size"),
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_and_masks() {
        let v: Vec<f32> = (0..30000).map(|k| (k % 7) as f32 * 0.1).collect();
        let (s, empty) = masked_sum(&v, None);
        assert!(!empty);
        let direct: f64 = v.iter().map(|&x| f64::from(x)).sum();
        assert!((s - direct).abs() < 1e-6);
        let m: Vec<f32> = (0..30000)
            .map(|k| if k % 2 == 0 { 1.0 } else { 0.5 })
            .collect();
        assert_eq!(masked_count(v.len(), Some(&m)), 15000);
        let none = vec![0f32; 30000];
        assert!(masked_sum(&v, Some(&none)).1);
    }

    #[test]
    fn percentiles() {
        let mut vol = Volume::new(4, 1, 1, 1);
        vol.data = vec![3.0, -1.0, 2.0, 0.5];
        assert_eq!(vol.percentile_masked(0.0, None).unwrap().0, -1.0);
        assert_eq!(vol.percentile_masked(0.5, None).unwrap().0, 2.0);
        assert_eq!(vol.percentile_masked(1.0, None).unwrap().0, 3.0);
        let mut mask = Volume::new(4, 1, 1, 1);
        mask.data = vec![1.0, 0.0, 1.0, 1.0];
        assert_eq!(vol.percentile_masked(0.25, Some(&mask)).unwrap().0, 0.5);
        assert!(vol.percentile_masked(1.5, None).is_err());
    }
}
