// SPDX-License-Identifier: Apache-2.0
//! numpy's float32 dot product and vector norm in the operation order of OpenBLAS's Haswell
//! `sdot` kernel, under SciPy's conjugate gradients (scikit-learn's ridge solver). The kernels
//! moved to [`larmorx_core::numpy::dot`] (with `ddot`) when scikit-image's random walker needed
//! them too; this module keeps the names part A's code and bindings use.

pub use larmorx_core::numpy::dot::{dot_f32, norm_f32, sdot};
