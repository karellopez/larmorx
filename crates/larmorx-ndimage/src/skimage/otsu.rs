// SPDX-License-Identifier: Apache-2.0 AND BSD-2-Clause AND BSD-3-Clause
//! `skimage.filters.threshold_otsu` (scikit-image 0.26.0, `filters/thresholding.py`, under the
//! 2-clause BSD licence (notice below); `exposure/exposure.py`'s histogram under scikit-image's
//! 3-clause BSD licence, notice in `super`).
//!
//! `skimage/filters/thresholding.py`: Copyright (c) 2009-2015 Board of Regents of the University
//! of Wisconsin-Madison, Broad Institute of MIT and Harvard, and Max Planck Institute of
//! Molecular Cell Biology and Genetics; 2009 Zachary Pincus; 2009 Almar Klein.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS ``AS IS'' AND ANY EXPRESS
//! OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//! HOLDERS OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY,
//! OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//! SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//! THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
//! OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.
//!
//! The histogram's counts are cast to `float32` (`_validate_image_histogram`), so the class
//! weights are `float32` cumulative sums; the class means are cumulative sums of `counts *
//! bin_centers` in the type numpy promotes that product to (`float32` for a `float32` image,
//! `float64` for a `float64` image or the integer centres of an integer image), divided by the
//! weights; the between-class variance is `weight1 * weight2 * (mean1 - mean2)²` with the first
//! product in `float32`; the threshold is the centre of the first bin of largest variance
//! (`np.argmax`: a NaN counts as the largest).

use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::histograms::{self, HistogramError};

/// Why the threshold could not be computed (scikit-image's and numpy's errors).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OtsuError {
    /// numpy's `IndexError` of `image.reshape(-1)[0]` on an empty image.
    #[error("index 0 is out of bounds for axis 0 with size 0")]
    Empty,
    /// numpy's histogram refused (a NaN or an infinity, or too many bins).
    #[error("{0}")]
    Histogram(String),
    /// An integer image whose range needs more bins than larmorx allocates (numpy's `bincount`
    /// would ask for that much memory).
    #[error("the integer range {0} is too large for a histogram of one bin per value")]
    IntegerRange(u128),
    /// numpy's `ValueError` of `np.argmax` on the empty variance of a one-bin histogram.
    #[error("attempt to get argmax of an empty sequence")]
    OneBin,
}

/// The variance of every split and the first index of its maximum, for counts already in
/// `float32` and centres in `T` (the promoted type of `counts * bin_centers`).
fn otsu_index<T: NpFloat>(counts: &[f32], centers: &[T]) -> Result<usize, OtsuError> {
    let n = counts.len();
    if n < 2 {
        return Err(OtsuError::OneBin);
    }
    let mut w1 = Vec::with_capacity(n);
    let mut acc = 0.0f32;
    for &c in counts {
        acc += c;
        w1.push(acc);
    }
    let mut w2 = vec![0.0f32; n];
    let mut acc = 0.0f32;
    for i in (0..n).rev() {
        acc += counts[i];
        w2[i] = acc;
    }
    let cb: Vec<T> = counts
        .iter()
        .zip(centers)
        .map(|(&c, &b)| T::from_f64(f64::from(c)).mul(b))
        .collect();
    let mut mean1 = Vec::with_capacity(n);
    let mut acc = T::ZERO;
    for (i, &v) in cb.iter().enumerate() {
        acc = if i == 0 { v } else { acc.add(v) };
        mean1.push(acc.div(T::from_f64(f64::from(w1[i]))));
    }
    // mean2 = (cumsum(cb[::-1]) / weight2[::-1])[::-1]
    let mut mean2 = vec![T::ZERO; n];
    let mut acc = T::ZERO;
    for (k, i) in (0..n).rev().enumerate() {
        acc = if k == 0 { cb[i] } else { acc.add(cb[i]) };
        mean2[i] = acc.div(T::from_f64(f64::from(w2[i])));
    }
    let mut best = 0usize;
    let mut best_v: Option<T> = None;
    for i in 0..n.saturating_sub(1) {
        let d = mean1[i].sub(mean2[i + 1]);
        let v = T::from_f64(f64::from(w1[i] * w2[i + 1])).mul(d.mul(d));
        match best_v {
            None => {
                best = i;
                best_v = Some(v);
            }
            Some(b) => {
                if b.is_nan() {
                    break;
                }
                if v.is_nan() || v > b {
                    best = i;
                    best_v = Some(v);
                    if v.is_nan() {
                        break;
                    }
                }
            }
        }
    }
    Ok(best)
}

/// `threshold_otsu(image, nbins)` of a float image (its values in any order: the histogram does
/// not depend on it): the image's own value when every value equals the first; otherwise the
/// centre of the chosen bin of numpy's `histogram(image, nbins)`, `(edges[i] + edges[i + 1]) / 2`
/// in `T`.
pub fn threshold_otsu<T: NpFloat>(image: &[T], nbins: usize) -> Result<T, OtsuError> {
    let Some(&first) = image.first() else {
        return Err(OtsuError::Empty);
    };
    if image.iter().all(|&x| x == first) {
        return Ok(first);
    }
    let (counts, edges) = histograms::histogram(image, nbins).map_err(|e| match e {
        HistogramError::NotFinite => {
            let (lo, hi) = histograms::min_max(image);
            OtsuError::Histogram(format!(
                "autodetected range of [{}, {}] is not finite",
                py_float(lo.to_f64()),
                py_float(hi.to_f64())
            ))
        }
        other => OtsuError::Histogram(other.to_string()),
    })?;
    let centers: Vec<T> = edges
        .windows(2)
        .map(|w| w[0].add(w[1]).div(T::from_f64(2.0)))
        .collect();
    let counts: Vec<f32> = counts.iter().map(|&c| c as f32).collect();
    Ok(centers[otsu_index(&counts, &centers)?])
}

/// `threshold_otsu(image)` of an integer image (scikit-image's `bincount` histogram: one bin per
/// value from the minimum to the maximum, `nbins` ignored; the arithmetic in `float64`).
pub fn threshold_otsu_int(image: &[i64]) -> Result<i64, OtsuError> {
    let Some(&first) = image.first() else {
        return Err(OtsuError::Empty);
    };
    if image.iter().all(|&x| x == first) {
        return Ok(first);
    }
    let lo = *image.iter().min().expect("not empty");
    let hi = *image.iter().max().expect("not empty");
    let range = (i128::from(hi) - i128::from(lo)) as u128 + 1;
    if range > 1 << 31 {
        return Err(OtsuError::IntegerRange(range));
    }
    let mut counts = vec![0u64; range as usize];
    for &v in image {
        counts[(i128::from(v) - i128::from(lo)) as usize] += 1;
    }
    let centers: Vec<f64> = (0..range as i64).map(|k| (lo + k) as f64).collect();
    let counts: Vec<f32> = counts.iter().map(|&c| c as f32).collect();
    Ok(lo + otsu_index(&counts, &centers)? as i64)
}

/// `threshold_otsu(hist=(counts, bin_centers))` (or `hist=counts`, the centres
/// `arange(counts.size)`): the counts as given (already cast to `float32` by the caller), the
/// zero counts at both ends trimmed when the first or the last count is zero; returns the index
/// of the chosen centre in the given centres (the caller reads the centre in its own type).
/// `T` is the type numpy promotes `float32 * bin_centers` to.
pub fn threshold_otsu_hist<T: NpFloat>(counts: &[f32], centers: &[T]) -> Result<usize, OtsuError> {
    let n = counts.len().min(centers.len());
    let (mut start, mut end) = (0usize, n);
    if n > 0 && (counts[0] == 0.0 || counts[n - 1] == 0.0) {
        // cond = counts > 0; start = argmax(cond); end = size - argmax(cond[::-1])
        start = counts[..n].iter().position(|&c| c > 0.0).unwrap_or(0);
        end = n - counts[..n].iter().rev().position(|&c| c > 0.0).unwrap_or(0);
    }
    Ok(start + otsu_index(&counts[start..end], &centers[start..end])?)
}

/// Python's `str` of a float (for numpy's message): `nan`, `inf`, `-inf`, or the shortest
/// representation.
fn py_float(x: f64) -> String {
    if x.is_nan() {
        "nan".into()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        format!("{x:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_levels_split_between_them() {
        let mut img = vec![0.0f32; 50];
        img.extend(vec![1.0f32; 50]);
        let t = threshold_otsu(&img, 256).unwrap();
        assert!(t > 0.0 && t < 1.0);
        assert_eq!(threshold_otsu(&[2.5f64, 2.5], 256), Ok(2.5));
        assert_eq!(threshold_otsu::<f32>(&[], 256), Err(OtsuError::Empty));
        assert_eq!(threshold_otsu_int(&[0, 0, 9, 9]), Ok(0));
    }
}
