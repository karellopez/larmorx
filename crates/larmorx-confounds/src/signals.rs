// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! niworkflows' `SignalExtraction` (`niworkflows/interfaces/images.py`, niworkflows 1.15.4, the
//! same in 1.15.2 and 1.15.3; Apache-2.0, the NiPreps developers): the mean of the BOLD series
//! over each region, `data[mask, :].mean(axis=0)` of nibabel's `get_fdata()`, with numpy's
//! summation order (numpy, BSD-3-Clause: the notice in `larmorx_core::numpy`).
//!
//! **What numpy computes.** `data` is nibabel's Fortran-ordered `float64` array `(X, Y, Z, T)`
//! (a `float32` file's values exactly; larmorx reads `float32` series as they are stored and
//! widens each value, which is exact). Boolean indexing gives a C-ordered `(N, T)` copy whose
//! rows are the voxels in the C order of the 3-D mask (the last axis fastest:
//! `validation/investigations/confounds/probe_reduction_layouts.py`), and `mean(axis=0)` sums
//! that copy's outer axis in index order, `((0 + x_0) + x_1) + ...` per volume, then divides by
//! the count in `float64` ([`larmorx_core::numpy::reduce`]). With one volume (`T = 1`) the
//! summed axis is the innermost one and numpy sums it pairwise (in chunks of 8192 before numpy
//! 2.3: [`SumOrder`]). An empty mask gives NaN (`0 / 0`, x86-64's default NaN), as numpy's
//! "Mean of empty slice".
//!
//! larmorx never makes the `(N, T)` copy: each volume's sum reads the voxels where they are, in
//! the mask's C order, and the volumes are summed on `n_threads` threads (each volume's
//! additions in the same order: the result does not depend on the thread count).

use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::reduce::{SumOrder, add_reduce_ordered};
use larmorx_core::parallel::{ThreadPoolError, with_threads};
use rayon::prelude::*;

/// A series' values in memory: the stored `float32` or `float64` values, with the offset of
/// each element given by the caller ([`signal_means`]).
#[derive(Clone, Copy, Debug)]
pub enum Series<'a> {
    /// A `float32` series (`get_fdata()` widens it exactly).
    F32(&'a [f32]),
    /// A `float64` series.
    F64(&'a [f64]),
}

impl Series<'_> {
    #[inline(always)]
    fn at(&self, i: usize) -> f64 {
        match self {
            Series::F32(a) => a[i].to_f64(),
            Series::F64(a) => a[i],
        }
    }

    fn len(&self) -> usize {
        match self {
            Series::F32(a) => a.len(),
            Series::F64(a) => a.len(),
        }
    }
}

/// The element offsets of a mask's voxels in the mask's C order (the last axis fastest), for a
/// series whose first three axes have the element strides `strides` (non-negative) and the
/// shape `mask_shape`; `mask` is in C order of that shape.
pub fn voxel_offsets(mask: &[bool], mask_shape: [usize; 3], strides: [usize; 3]) -> Vec<usize> {
    let [nx, ny, nz] = mask_shape;
    assert_eq!(mask.len(), nx * ny * nz, "mask length and shape disagree");
    let mut out = Vec::new();
    let mut c = 0usize;
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                if mask[c] {
                    out.push(i * strides[0] + j * strides[1] + k * strides[2]);
                }
                c += 1;
            }
        }
    }
    out
}

/// `data[mask, :].mean(axis=0)` for the voxels at `offsets` (in the mask's C order) of a series
/// with `nt` volumes `t_stride` elements apart: one mean per volume, as numpy release `order`
/// computes it (module documentation). `nt == 0` gives no value.
pub fn signal_means(
    data: Series<'_>,
    offsets: &[usize],
    t_stride: usize,
    nt: usize,
    order: SumOrder,
    n_threads: usize,
) -> Result<Vec<f64>, ThreadPoolError> {
    if nt > 0 {
        let last = offsets.iter().max().map_or(0, |&o| o + (nt - 1) * t_stride);
        assert!(
            offsets.is_empty() || last < data.len(),
            "offsets beyond the series"
        );
    }
    let count = offsets.len() as f64;
    if nt == 1 {
        let values: Vec<f64> = offsets.iter().map(|&o| data.at(o)).collect();
        return Ok(vec![add_reduce_ordered(&values, order).div(count)]);
    }
    with_threads(n_threads, || {
        (0..nt)
            .into_par_iter()
            .map(|t| {
                let base = t * t_stride;
                let mut s = 0.0f64;
                for &o in offsets {
                    s = NpFloat::add(s, data.at(base + o));
                }
                s.div(count)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn means_follow_numpy_order_and_ignore_threads() {
        // A (2, 2, 2, 3) Fortran-ordered series: element (i, j, k, t) at i + 2j + 4k + 8t.
        let data: Vec<f64> = (0..24)
            .map(|v| 0.1 * v as f64 + 1e16 * ((v % 3) as f64))
            .collect();
        let mask = [true, false, true, true, false, false, true, true];
        let offs = voxel_offsets(&mask, [2, 2, 2], [1, 2, 4]);
        // C order (i, j, k): (0,0,0) (0,1,0) (0,1,1) (1,1,0) (1,1,1).
        assert_eq!(offs, vec![0, 2, 6, 3, 7]);
        let one = signal_means(Series::F64(&data), &offs, 8, 3, SumOrder::Pairwise, 1).unwrap();
        let four = signal_means(Series::F64(&data), &offs, 8, 3, SumOrder::Pairwise, 4).unwrap();
        assert_eq!(one, four);
        for (t, m) in one.iter().enumerate() {
            let mut s = 0.0f64;
            for &o in &offs {
                s += data[o + 8 * t];
            }
            assert_eq!(m.to_bits(), (s / 5.0).to_bits());
        }
    }

    #[test]
    fn an_empty_mask_is_nan_and_one_volume_is_pairwise() {
        let data = vec![1.0f32; 8];
        let m = signal_means(Series::F32(&data), &[], 1, 8, SumOrder::Pairwise, 2).unwrap();
        assert!(m.iter().all(|v| v.to_bits() == 0xfff8_0000_0000_0000));
        let big: Vec<f64> = (0..20_000).map(|v| 1.0 + 1e-7 * v as f64).collect();
        let offs: Vec<usize> = (0..big.len()).collect();
        let p = signal_means(Series::F64(&big), &offs, 1, 1, SumOrder::Pairwise, 1).unwrap();
        let b = signal_means(Series::F64(&big), &offs, 1, 1, SumOrder::Buffered, 1).unwrap();
        assert_eq!(
            p[0].to_bits(),
            (add_reduce_ordered(&big, SumOrder::Pairwise) / 20_000.0).to_bits()
        );
        assert_eq!(
            b[0].to_bits(),
            (add_reduce_ordered(&big, SumOrder::Buffered) / 20_000.0).to_bits()
        );
        assert!(
            signal_means(Series::F64(&big), &offs, 1, 0, SumOrder::Pairwise, 1)
                .unwrap()
                .is_empty()
        );
    }
}
