// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! nilearn 0.14.1's `masking.compute_epi_mask(epi_img)` with its defaults (`lower_cutoff=0.2`,
//! `upper_cutoff=0.85`, `connected=True`, `opening=2`, `exclude_zeros=False`,
//! `ensure_finite=True`), as tedana's `t2smap` computes a mask when none is given
//! (`nilearn/masking.py`, `nilearn/image/image.py` `compute_mean` and `smooth_array`,
//! `nilearn/_utils/ndimage.py` `largest_connected_component`, `nilearn/_utils/niimg.py`
//! `ensure_finite_data`; BSD-3-Clause, "Copyright (c) The nilearn developers", the notice in
//! NOTICE), on the SciPy 1.18.1 filters and morphology of [`crate::ndimage`].
//!
//! The steps: the temporal mean of a 4D image (numpy's `mean(axis=-1)`, in the image's type:
//! `float32`, or `float64` for integers and scaled data; the order of its sum follows the
//! array's memory layout, see [`Layout`]); the smoothing `smooth=1` (a Gaussian of FWHM 1 voxel
//! along each axis with `affine=np.eye(4)`: `sigma = 1 / sqrt(8 ln 2)`, in place, non-finite
//! values set to 0 first and NaNs put back after); the non-finite values set to 0; the sorted
//! values' largest gap between the 20 % and 85 % ranks, the threshold half way across it
//! (`0.5 * (a + b)` in the image's type); the voxels `>= threshold`; two erosions, the largest
//! face-connected component (the earliest in C order among equals), four dilations, two
//! erosions. An empty mask after the first erosions is nilearn's `MaskWarning` ("Computed an
//! empty mask"), returned in [`EpiMask::empty`].

use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::reduce::mean_axis;

use crate::ndimage::filters::gaussian_filter1d;
use crate::ndimage::morphology::{binary_dilation, binary_erosion, label};

/// The values `np.asanyarray(img.dataobj)` gives for the image.
#[derive(Clone, Copy, Debug)]
pub enum EpiData<'a> {
    /// float32 data without scaling.
    F32(&'a [f32]),
    /// float64 data, integers (converted exactly as numpy converts them) or scaled data.
    F64(&'a [f64]),
}

/// The memory layout of the image's array, which sets the order of the temporal mean's sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Fortran order (axis 0 fastest): an image read from a file. The volumes are the slowest
    /// axis, summed in order (pairwise for a single-voxel image).
    Fortran,
    /// C order (the last axis fastest): an array made by `get_fdata()` of a sliced image, as
    /// tedana's reference of z-concatenated data. The volumes are summed pairwise.
    C,
}

/// numpy's float64 `exp` and `log` for the smoothing kernel and its width.
#[derive(Clone, Copy, Debug)]
pub struct Maths {
    pub exp: fn(f64) -> f64,
    pub log: fn(f64) -> f64,
}

/// The mask: Fortran-ordered over the first three axes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpiMask {
    pub mask: Vec<bool>,
    /// nilearn's `MaskWarning: Computed an empty mask. Are you sure that input data are EPI
    /// images not detrended. ` (the mask is then empty).
    pub empty: bool,
}

/// Where nilearn raises.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EpiMaskError {
    /// `delta.argmax()` of an empty array (too few voxels between the cut-offs): numpy's
    /// `ValueError: attempt to get argmax of an empty sequence`.
    ArgmaxEmpty,
    /// `compute_mean`'s `ValueError` for an image that is neither 3D nor 4D.
    Dimensions(usize),
}

impl EpiMaskError {
    /// The Python exception: (class, message).
    pub fn python(&self) -> (&'static str, String) {
        match self {
            EpiMaskError::ArgmaxEmpty => (
                "ValueError",
                "attempt to get argmax of an empty sequence".to_owned(),
            ),
            EpiMaskError::Dimensions(n) => (
                "ValueError",
                format!("Computation expects 3D or 4D images, but {n} dimensions were given"),
            ),
        }
    }
}

fn c_to_fortran<T: Copy>(c: &[T], shape: [usize; 3]) -> Vec<T> {
    let [n0, n1, n2] = shape;
    let mut f = Vec::with_capacity(c.len());
    for z in 0..n2 {
        for y in 0..n1 {
            for x in 0..n0 {
                f.push(c[(x * n1 + y) * n2 + z]);
            }
        }
    }
    f
}

fn mean_image<T: NpFloat>(data: &[T], shape: &[usize], layout: Layout) -> Vec<T> {
    let s3 = [shape[0], shape[1], shape[2]];
    let nxyz: usize = s3.iter().product();
    let mean = if shape.len() == 4 {
        let t = shape[3];
        match layout {
            Layout::Fortran => mean_axis(data, [1, t, nxyz]),
            Layout::C => mean_axis(data, [nxyz, t, 1]),
        }
    } else {
        data.to_vec()
    };
    match layout {
        Layout::Fortran => mean,
        Layout::C => c_to_fortran(&mean, s3),
    }
}

fn ensure_finite<T: NpFloat>(a: &mut [T]) {
    for v in a.iter_mut() {
        if !v.to_f64().is_finite() {
            *v = T::ZERO;
        }
    }
}

fn mask_from_mean<T: NpFloat>(
    mut mean: Vec<T>,
    s3: [usize; 3],
    maths: Maths,
) -> Result<EpiMask, EpiMaskError> {
    // compute_mean's smoothing
    let nan_mask: Vec<bool> = mean.iter().map(|v| v.is_nan()).collect();
    ensure_finite(&mut mean);
    let ratio = (8.0 * (maths.log)(2.0)).sqrt();
    let sigma = 1.0 / (ratio * 1.0);
    for axis in 0..3 {
        if sigma > 0.0 {
            gaussian_filter1d(&mut mean, &s3, axis, sigma, maths.exp);
        }
    }
    for (v, &nan) in mean.iter_mut().zip(&nan_mask) {
        if nan {
            *v = T::NAN;
        }
    }
    // compute_epi_mask
    ensure_finite(&mut mean);
    let mut sorted = mean.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite values"));
    let n = sorted.len();
    let lower = (0.2 * n as f64).floor() as usize;
    let upper = ((0.85 * n as f64).floor() as usize).min(n.saturating_sub(1));
    if upper <= lower {
        return Err(EpiMaskError::ArgmaxEmpty);
    }
    let mut ia = 0;
    let mut best = None::<T>;
    for i in 0..upper - lower {
        let d = sorted[lower + 1 + i].sub(sorted[lower + i]);
        if best.is_none_or(|b| d > b) {
            best = Some(d);
            ia = i;
        }
    }
    let half = T::from_f64(0.5);
    let threshold = half.mul(sorted[ia + lower].add(sorted[ia + lower + 1]));
    let mask: Vec<bool> = mean.iter().map(|&v| v >= threshold).collect();
    // _post_process_mask(opening=2, connected=True)
    let mut mask = binary_erosion(&mask, s3, 2);
    let any = mask.iter().any(|&b| b);
    if any {
        let (labels, count) = label(&mask, s3);
        if count > 1 {
            let mut sizes = vec![0usize; count as usize + 1];
            for &l in &labels {
                sizes[l as usize] += 1;
            }
            sizes[0] = 0;
            let mut keep = 0;
            for (l, &s) in sizes.iter().enumerate() {
                if s > sizes[keep] {
                    keep = l;
                }
            }
            mask = labels.iter().map(|&l| l as usize == keep).collect();
        }
    }
    let mask = binary_dilation(&mask, s3, 4);
    let mask = binary_erosion(&mask, s3, 2);
    Ok(EpiMask { mask, empty: !any })
}

/// `compute_epi_mask(img)`: `data` holds the image of `shape` (three or four axes) in `layout`.
pub fn compute_epi_mask(
    data: EpiData<'_>,
    shape: &[usize],
    layout: Layout,
    maths: Maths,
) -> Result<EpiMask, EpiMaskError> {
    if shape.len() != 3 && shape.len() != 4 {
        return Err(EpiMaskError::Dimensions(shape.len()));
    }
    let s3 = [shape[0], shape[1], shape[2]];
    match data {
        EpiData::F32(d) => mask_from_mean(mean_image(d, shape, layout), s3, maths),
        EpiData::F64(d) => mask_from_mean(mean_image(d, shape, layout), s3, maths),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bright_block_is_found() {
        let shape = [12, 12, 12, 2];
        let mut d = vec![1.0f32; 12 * 12 * 12 * 2];
        for t in 0..2 {
            for z in 2..10 {
                for y in 2..10 {
                    for x in 2..10 {
                        d[x + 12 * (y + 12 * (z + 12 * t))] = 100.0;
                    }
                }
            }
        }
        let m = compute_epi_mask(
            EpiData::F32(&d),
            &shape,
            Layout::Fortran,
            Maths {
                exp: f64::exp,
                log: f64::ln,
            },
        )
        .unwrap();
        assert!(!m.empty);
        assert!(m.mask[6 + 12 * (6 + 12 * 6)]);
        assert!(!m.mask[0]);
    }
}
