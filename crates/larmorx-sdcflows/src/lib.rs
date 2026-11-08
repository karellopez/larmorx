// SPDX-License-Identifier: Apache-2.0
//! sdcflows' field-map estimation (part A of the field maps), ported from sdcflows 2.17.1
//! (Apache-2.0, the NiPreps developers) with the arithmetic of the libraries it calls: SciPy's
//! B-spline evaluation, sparse products and conjugate gradients, scikit-learn's ridge solver,
//! numpy's reductions and OpenBLAS's dot products (BSD-3-Clause; notices in the modules that
//! follow them and in `PROVENANCE.md`).
//!
//! The results are those of sdcflows' Python bit for bit (CLAUDE.md rule 8: the source is the
//! specification), and the same on every platform: the operation order of each upstream
//! routine is kept, and numpy's BLAS kernels are those of x86-64 with FMA (OpenBLAS's Haswell
//! kernels, `docs/findings/numpy-blas.md`).
//!
//! - [`bspline`]: the B-spline control grid (`bspline_grid`) and the tensor-product cubic
//!   B-spline weights of a sampling grid (`grid_bspline_weights`), applied as the collocation
//!   matrix without storing it ([`bspline::Collocation`]), or stored as scipy's CSR matrix.
//! - [`approx`]: `BSplineApprox`, the B-spline approximation of a field map (centring, the
//!   ridge fit, the field, the error and the extrapolation).
//! - [`blas`]: numpy's float32 dot product and norm over OpenBLAS's Haswell `sdot` kernel.
//! - [`ridge`]: scikit-learn's ridge regression on a sparse float32 matrix (`sparse_cg`, both
//!   of its systems) with SciPy's conjugate gradients, on the collocation operator or a stored
//!   CSR matrix.
//! - [`npy`]: the numpy element rules this needs (float floor division, `allclose`, `cross`).
//!
//! This crate is the **port** (a replica of a permissive upstream, in the main package). The
//! clean-room original of the same programs, planned later (the user, 2026-11-06), lives in
//! another crate.
#![forbid(unsafe_code)]

pub mod approx;
pub mod blas;
pub mod bspline;
pub mod npy;
pub mod ridge;

/// An sdcflows function cannot run on its inputs (the Python original raises).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SdcError {
    /// Arguments sdcflows (or numpy under it) rejects; the message is the original's.
    #[error("{0}")]
    Invalid(String),
    /// `numpy.linalg.inv` of a singular matrix.
    #[error("Singular matrix")]
    Singular,
    /// What sdcflows raises as `RuntimeError` (a failed spline fit); the message is the
    /// original's.
    #[error("{0}")]
    Runtime(String),
    /// The thread pool could not start.
    #[error("{0}")]
    Threads(String),
}

impl From<larmorx_core::parallel::ThreadPoolError> for SdcError {
    fn from(e: larmorx_core::parallel::ThreadPoolError) -> Self {
        SdcError::Threads(e.to_string())
    }
}
