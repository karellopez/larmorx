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
  rotated target; an EPI image or series unwarped with it (``apply``); its displacement field
  (``to_displacements``).
- :func:`apply_coeffs_field`: ``ApplyCoeffsField``, the unwarping of a file with coefficient
  files and an ITK transform.
- :func:`fmap_to_disp`, :func:`disp_to_fmap`, :func:`displacements_field_to_fieldmap`: a field
  map as a displacement field and back (``DisplacementsField2Fieldmap``).
- :func:`au2rads`, :func:`subtract_phases`, :func:`delta_te`, :func:`phdiff2fmap`,
  :func:`check_b0_units`, :func:`check_register`, :func:`phdiff_workflow`: the
  phase-difference preparation (``PhaseMap2rads``, ``SubtractPhases``, ``Phasediff2Fieldmap``,
  ``CheckB0Units``, ``CheckRegister``, ``init_phdiff_wf`` with PRELUDE through ``lx.fsl``).

sdcflows' files are written as nibabel writes them (the header's data type, scaled into an
integer type): :func:`larmorx.io.save_nibabel`.
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
from larmorx.sdcflows.fmap import (
    DisplacementsResult,
    disp_to_fmap,
    displacements_field_to_fieldmap,
    fmap_to_disp,
)
from larmorx.sdcflows.phase import (
    CheckRegisterResult,
    PhaseResult,
    PhdiffWorkflowResult,
    au2rads,
    check_b0_units,
    check_register,
    delta_te,
    phdiff2fmap,
    phdiff_workflow,
    subtract_phases,
)
from larmorx.sdcflows.ridge import RidgeResult, ridge_sparse_cg
from larmorx.sdcflows.tools import resample_to_zooms, rescale_affine
from larmorx.sdcflows.transform import (
    ApplyCoeffsFieldResult,
    B0FieldTransform,
    apply_coeffs_field,
    deoblique_and_zooms,
)

__all__ = [
    "DEFAULT_HF_ZOOMS_MM",
    "DEFAULT_LF_ZOOMS_MM",
    "DEFAULT_ZOOMS_MM",
    "NOT_ALIGNED",
    "ApplyCoeffsFieldResult",
    "B0FieldTransform",
    "BSplineApproxResult",
    "CheckRegisterResult",
    "CoefficientsResult",
    "CsrMatrix",
    "DisplacementsResult",
    "PhaseResult",
    "PhdiffWorkflowResult",
    "RidgeResult",
    "apply_coeffs_field",
    "au2rads",
    "bspline_approx",
    "bspline_grid",
    "check_b0_units",
    "check_register",
    "collocation_matrix",
    "delta_te",
    "deoblique_and_zooms",
    "disp_to_fmap",
    "displacements_field_to_fieldmap",
    "fmap_to_disp",
    "grid_bspline_weights",
    "phdiff2fmap",
    "phdiff_workflow",
    "reorient_pedir",
    "resample_to_zooms",
    "rescale_affine",
    "ridge_sparse_cg",
    "subtract_phases",
    "topup_coeff_reorient",
    "transform_coefficients",
]
