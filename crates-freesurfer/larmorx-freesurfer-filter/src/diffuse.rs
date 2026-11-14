// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/diffuse.cpp`'s global Sobel coefficient tables (queue item 3.1-42): `sy` and `sx`, the
//! 3 x 3 derivative kernels (rows of three, the y one differencing the rows, the x one the
//! columns) that FreeSurfer's 2-D image code reads.

/// `sy`: the y derivative's weights, row by row.
pub const SY: [f32; 9] = [-0.25, -0.5, -0.25, 0.0, 0.0, 0.0, 0.25, 0.5, 0.25];

/// `sx`: the x derivative's weights, row by row.
pub const SX: [f32; 9] = [-0.25, 0.0, 0.25, -0.5, 0.0, 0.5, -0.25, 0.0, 0.25];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_transposes() {
        for r in 0..3 {
            for c in 0..3 {
                assert_eq!(SX[3 * r + c].to_bits(), SY[3 * c + r].to_bits());
            }
        }
    }
}
