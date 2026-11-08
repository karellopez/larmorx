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

- :func:`brain_masker`, :func:`brain_extraction`, :func:`binary_dilation`, :func:`union`:
  sdcflows' masks (``brain_masker``, ``BrainExtraction``, ``BinaryDilation``, ``Union``), with
  scikit-image's random walker (:mod:`larmorx.skimage`).
- :func:`get_trt`, :func:`get_readout_time`, :func:`select_pe_volumes`, :func:`sort_pe_blips`,
  :func:`uniform_grid`, :func:`pad_slices`, :func:`reorient_image_and_metadata`,
  :func:`select_topup_config`, :func:`topup_config_file`, :func:`topup_node`,
  :func:`topup_mats_to_world`, :func:`topup_workflow`: the PEPOLAR estimation
  (``init_topup_wf`` in sdcflows 2.17.1's and 2.15's wirings, FSL's ``topup`` through ``lx.fsl``).
- :func:`brainextraction_workflow`, :func:`magnitude_workflow`, :func:`fmap_workflow`:
  ``init_brainextraction_wf``, ``init_magnitude_wf`` and ``init_fmap_wf`` (phase difference
  and direct field maps) as plain functions, each node through larmorx's ports.
- :func:`coeff2epi_workflow`, :func:`registration_settings`: ``init_coeff2epi_wf`` (the field
  map's reference registered to the target EPI with ANTs) and sdcflows' ANTs settings files.
- :func:`syn_preprocessing_workflow`, :func:`deoblique`, :func:`reoblique`, :func:`set_dtype`,
  :func:`merge_meta`, :func:`obliquity`, :func:`fmap_atlas`, :func:`fmap_atlas_affine`: the
  fieldmap-less (SyN) preprocessing (``init_syn_preprocessing_wf``) and its helpers.

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
from larmorx.sdcflows.masks import (
    BrainMaskerResult,
    binary_dilation,
    brain_extraction,
    brain_masker,
    union,
)
from larmorx.sdcflows.pepolar import (
    GetReadoutTimeResult,
    SelectPEVolumesResult,
    SortPEBlipsResult,
    TopupNodeResult,
    TopupWorkflowResult,
    UniformGridResult,
    get_readout_time,
    get_trt,
    pad_slices,
    reorient_image_and_metadata,
    select_pe_volumes,
    select_topup_config,
    sort_pe_blips,
    topup_config_file,
    topup_mats_to_world,
    topup_node,
    topup_workflow,
    uniform_grid,
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
from larmorx.sdcflows.registration import (
    Coeff2EpiResult,
    coeff2epi_workflow,
    registration_settings,
)
from larmorx.sdcflows.ridge import RidgeResult, ridge_sparse_cg
from larmorx.sdcflows.syn import (
    SynPreprocessingResult,
    deoblique,
    fmap_atlas,
    fmap_atlas_affine,
    merge_meta,
    obliquity,
    reoblique,
    set_dtype,
    syn_preprocessing_workflow,
)
from larmorx.sdcflows.tools import resample_to_zooms, rescale_affine
from larmorx.sdcflows.transform import (
    ApplyCoeffsFieldResult,
    B0FieldTransform,
    apply_coeffs_field,
    deoblique_and_zooms,
)
from larmorx.sdcflows.workflows import (
    BrainExtractionWorkflowResult,
    FmapWorkflowResult,
    MagnitudeWorkflowResult,
    brainextraction_workflow,
    fmap_workflow,
    magnitude_workflow,
)

__all__ = [
    "DEFAULT_HF_ZOOMS_MM",
    "DEFAULT_LF_ZOOMS_MM",
    "DEFAULT_ZOOMS_MM",
    "NOT_ALIGNED",
    "ApplyCoeffsFieldResult",
    "B0FieldTransform",
    "BSplineApproxResult",
    "BrainExtractionWorkflowResult",
    "BrainMaskerResult",
    "CheckRegisterResult",
    "Coeff2EpiResult",
    "CoefficientsResult",
    "CsrMatrix",
    "DisplacementsResult",
    "FmapWorkflowResult",
    "GetReadoutTimeResult",
    "MagnitudeWorkflowResult",
    "PhaseResult",
    "PhdiffWorkflowResult",
    "RidgeResult",
    "SelectPEVolumesResult",
    "SortPEBlipsResult",
    "SynPreprocessingResult",
    "TopupNodeResult",
    "TopupWorkflowResult",
    "UniformGridResult",
    "apply_coeffs_field",
    "au2rads",
    "binary_dilation",
    "brain_extraction",
    "brain_masker",
    "brainextraction_workflow",
    "bspline_approx",
    "bspline_grid",
    "check_b0_units",
    "check_register",
    "coeff2epi_workflow",
    "collocation_matrix",
    "delta_te",
    "deoblique",
    "deoblique_and_zooms",
    "disp_to_fmap",
    "displacements_field_to_fieldmap",
    "fmap_atlas",
    "fmap_atlas_affine",
    "fmap_to_disp",
    "fmap_workflow",
    "get_readout_time",
    "get_trt",
    "grid_bspline_weights",
    "magnitude_workflow",
    "merge_meta",
    "obliquity",
    "pad_slices",
    "phdiff2fmap",
    "phdiff_workflow",
    "registration_settings",
    "reoblique",
    "reorient_image_and_metadata",
    "reorient_pedir",
    "resample_to_zooms",
    "rescale_affine",
    "ridge_sparse_cg",
    "select_pe_volumes",
    "select_topup_config",
    "set_dtype",
    "sort_pe_blips",
    "subtract_phases",
    "syn_preprocessing_workflow",
    "topup_coeff_reorient",
    "topup_config_file",
    "topup_mats_to_world",
    "topup_node",
    "topup_workflow",
    "transform_coefficients",
    "uniform_grid",
    "union",
]
