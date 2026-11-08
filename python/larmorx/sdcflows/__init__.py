# SPDX-License-Identifier: Apache-2.0
"""sdcflows' field-map estimation, ported: ``lx.sdcflows``.

A port of sdcflows 2.17.1 (Apache-2.0, the NiPreps developers), bit-identical to its Python
with the libraries it calls (SciPy, scikit-learn, numpy on OpenBLAS's Haswell kernels), in
Rust (``larmorx-sdcflows``):

- :func:`bspline_grid`, :func:`grid_bspline_weights`, :func:`collocation_matrix`: the B-spline
  control grid of a field map and the tensor-product cubic B-spline weights of a sampling grid.
- :func:`ridge_sparse_cg`: scikit-learn's ridge regression of a sparse float32 matrix (the
  ``sparse_cg`` solver, with SciPy's conjugate gradients), the fit sdcflows' ``BSplineApprox``
  runs.
- :func:`bspline_approx`: ``BSplineApprox``, the B-spline approximation of a field map (its
  smooth field, coefficients, fitting error and extrapolation; sdcflows' files and headers).
- :func:`resample_to_zooms`, :func:`rescale_affine`: sdcflows' resampling of an image to new
  voxel sizes, with nibabel's ``rescale_affine``.
- :func:`topup_coeff_reorient`, :func:`transform_coefficients`, :func:`reorient_pedir`:
  ``TOPUPCoeffReorient`` (TOPUP's coefficients placed in space), ``TransformCoefficients``
  (coefficients moved by a rigid transform) and the phase-encoding direction of a reoriented
  image.
- :class:`B0FieldTransform`, :func:`deoblique_and_zooms`: a field map given by B-spline
  coefficients evaluated on a target grid (``fit``), and the fine aligned grid it uses for a
  rotated target.
"""

from larmorx.sdcflows.approx import BSplineApproxResult, bspline_approx
from larmorx.sdcflows.bspline import (
    DEFAULT_HF_ZOOMS_MM,
    DEFAULT_LF_ZOOMS_MM,
    DEFAULT_ZOOMS_MM,
    NOT_ALIGNED,
    CsrMatrix,
    bspline_grid,
    collocation_matrix,
    grid_bspline_weights,
)
from larmorx.sdcflows.coeffs import (
    CoefficientsResult,
    reorient_pedir,
    topup_coeff_reorient,
    transform_coefficients,
)
from larmorx.sdcflows.ridge import RidgeResult, ridge_sparse_cg
from larmorx.sdcflows.tools import resample_to_zooms, rescale_affine
from larmorx.sdcflows.transform import B0FieldTransform, deoblique_and_zooms

__all__ = [
    "DEFAULT_HF_ZOOMS_MM",
    "DEFAULT_LF_ZOOMS_MM",
    "DEFAULT_ZOOMS_MM",
    "NOT_ALIGNED",
    "B0FieldTransform",
    "BSplineApproxResult",
    "CoefficientsResult",
    "CsrMatrix",
    "RidgeResult",
    "bspline_approx",
    "bspline_grid",
    "collocation_matrix",
    "deoblique_and_zooms",
    "grid_bspline_weights",
    "reorient_pedir",
    "resample_to_zooms",
    "rescale_affine",
    "ridge_sparse_cg",
    "topup_coeff_reorient",
    "transform_coefficients",
]
