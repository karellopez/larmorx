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
"""

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
from larmorx.sdcflows.ridge import RidgeResult, ridge_sparse_cg

__all__ = [
    "DEFAULT_HF_ZOOMS_MM",
    "DEFAULT_LF_ZOOMS_MM",
    "DEFAULT_ZOOMS_MM",
    "NOT_ALIGNED",
    "CsrMatrix",
    "RidgeResult",
    "bspline_grid",
    "collocation_matrix",
    "grid_bspline_weights",
    "ridge_sparse_cg",
]
