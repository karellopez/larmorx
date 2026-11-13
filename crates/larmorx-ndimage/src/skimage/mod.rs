// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! scikit-image's pieces under sdcflows' `brain_masker`, ported from scikit-image 0.26.0 (the
//! same arithmetic in 0.25.2, fMRIPrep 25.2.6's): `skimage.morphology.ball`
//! (`morphology/footprints.py`), `skimage.filters.threshold_otsu` (`filters/thresholding.py`,
//! with `skimage.exposure.histogram` over numpy's histogram), and
//! `skimage.segmentation.random_walker` (`segmentation/random_walker_segmentation.py`, with
//! SciPy's sparse matrices and conjugate gradients under it: [`random_walker`]).
//!
//! scikit-image (its `LICENSE.txt`, "Files: *"): Copyright (c) 2009-2022, the scikit-image team.
//! All rights reserved. `filters/thresholding.py` is under the 2-clause BSD licence: its notice
//! is in [`otsu`].
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of skimage nor the names of its contributors may be used to endorse or
//!    promote products derived from this software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES,
//! INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
//! PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
//! INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
//! LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
//! BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//! STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
//! USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//!
//! SciPy's and numpy's notices: in the crate's root and in `larmorx_core::numpy`.

pub mod otsu;
pub mod random_walker;

pub use otsu::{OtsuError, threshold_otsu, threshold_otsu_hist, threshold_otsu_int};
pub use random_walker::{
    Mode, Numbering, Prepared, Preprocessed, RandomWalker, RwError, RwOutput, RwWarning, Spacing,
    SparseOrder, preprocess, random_walker,
};

use crate::Footprint;

/// `skimage.morphology.ball(radius, strict_radius=strict_radius)` (no decomposition) as a
/// footprint: `n = int(abs(2 r + 1))` points per axis from `-r` to `r` (numpy's `mgrid` with a
/// complex step: `k * step - r`, `step = 2 r / (n - 1)` in `float64`), true where `x² + y² + z²
/// <= r²` (`(r + 0.5)²` with `strict_radius=False`), the sum in `float64` as `X**2 + Y**2 +
/// Z**2` with `X` the last axis. A negative radius is scikit-image's too: a radius in `(-1, 0)`
/// gives an empty footprint (no points), `-1` one false element, `-2` the 3 × 3 × 3 cross of
/// radius 1 (the points run from `-r` down to `r`).
pub fn ball(radius: f64, strict_radius: bool) -> Footprint {
    // numpy's `ogrid[-r:r:num * 1j]` takes `int(abs(num))` points, so a negative radius
    // gives points from `-r` (positive) down to `r` (`ball(-2)` is 3 x 3 x 3).
    let num = 2.0 * radius + 1.0;
    let n = num.abs() as usize;
    let coords: Vec<f64> = (0..n)
        .map(|k| {
            let step = if n != 1 {
                (radius - (-radius)) / (n - 1) as f64
            } else {
                1.0
            };
            k as f64 * step + -radius
        })
        .collect();
    let r = if strict_radius { radius } else { radius + 0.5 };
    let rr = r * r;
    let mut mask = Vec::with_capacity(n * n * n);
    for &z in &coords {
        for &y in &coords {
            for &x in &coords {
                mask.push(x * x + y * y + z * z <= rr);
            }
        }
    }
    Footprint {
        shape: vec![n, n, n],
        mask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balls() {
        let b1 = ball(1.0, true);
        assert_eq!(b1.shape, vec![3, 3, 3]);
        assert_eq!(b1.count(), 7);
        assert_eq!(ball(3.0, true).count(), 123);
        assert_eq!(ball(5.0, true).count(), 515);
        assert_eq!(ball(0.0, true).count(), 1);
        // skimage's negative radii (numpy's `int(abs(2r + 1))` points from -r down to r).
        assert_eq!(ball(-1.0, true).shape, vec![1, 1, 1]);
        assert_eq!(ball(-1.0, true).count(), 0);
        assert_eq!(ball(-2.0, true).shape, vec![3, 3, 3]);
        assert_eq!(ball(-2.0, true).count(), 7);
        assert_eq!(ball(-0.5, true).shape, vec![0, 0, 0]);
        assert_eq!(ball(1.0, false).count(), 19);
    }
}
