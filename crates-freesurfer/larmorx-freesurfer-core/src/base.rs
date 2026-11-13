// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The small helpers of FreeSurfer 8.2.0's `include/base.h` (`fs.core`, queue item 3.1-22):
//! `squaref` and `squared`, one rounding each (`x*x` in its type).

/// `squaref(x)`: `x*x` in float.
pub fn squaref(x: f32) -> f32 {
    x * x
}

/// `squared(x)`: `x*x` in double.
pub fn squared(x: f64) -> f64 {
    x * x
}
