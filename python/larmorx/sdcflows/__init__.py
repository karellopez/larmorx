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
- :func:`unwarp_workflow`, :func:`unwarp_num_threads`: ``init_unwarp_wf``, an EPI dataset
  unwarped with a field map's coefficients, merged, averaged and brain-extracted.
- :func:`flatten`, :func:`sorted_pe`, :func:`qwarp2ants`, :func:`qwarp_workflow`: the Python
  steps of the PEPOLAR estimation with AFNI's ``3dQwarp`` (``init_3dQwarp_wf``, which waits for
  its programs: ``NotImplementedError``).
- :func:`brainextraction_workflow`, :func:`magnitude_workflow`, :func:`fmap_workflow`:
  ``init_brainextraction_wf``, ``init_magnitude_wf`` and ``init_fmap_wf`` (phase difference
  and direct field maps) as plain functions, each node through larmorx's ports.
- :func:`coeff2epi_workflow`, :func:`registration_settings`: ``init_coeff2epi_wf`` (the field
  map's reference registered to the target EPI with ANTs) and sdcflows' ANTs settings files.
- :func:`syn_preprocessing_workflow`, :func:`deoblique`, :func:`reoblique`, :func:`set_dtype`,
  :func:`merge_meta`, :func:`obliquity`, :func:`fmap_atlas`, :func:`fmap_atlas_affine`: the
  fieldmap-less (SyN) preprocessing (``init_syn_preprocessing_wf``) and its helpers.
- :func:`syn_sdc_workflow`, :func:`warp_dir`, :func:`mm2vox`, :func:`adjust_zooms`,
  :func:`norm_lap`: the fieldmap-less (SyN) estimation (``init_syn_sdc_wf``) and its helpers.
- :class:`FieldmapFile`, :class:`FieldmapEstimation`, :class:`EstimatorType`,
  :class:`FieldmapRegistry` (:data:`REGISTRY`), :func:`get_identifier`, :func:`clear_registry`,
  :func:`find_estimators`, :func:`find_anatomical_estimators`: the estimators of a BIDS dataset
  (``fieldmaps.py``, ``utils/bimap.py``, ``utils/wrangler.py``), on a
  :class:`larmorx.bids.BIDSLayout`.
- :func:`estimator_workflow` (:meth:`FieldmapEstimation.get_workflow`),
  :func:`build_workflows`, :func:`fmap_preproc_workflow`, :func:`fmap_derivatives_workflow`:
  each estimator's workflow, ``init_fmap_preproc_wf``'s graph built (its checks), a subject's
  estimators run with their derivatives (``init_fmap_preproc_wf``) and the derivatives
  (``init_fmap_derivatives_wf``, with niworkflows' ``DerivativesDataSink``).

sdcflows' files are written as nibabel writes them (the header's data type, scaled into an
integer type): :func:`larmorx.io.save_nibabel`.
"""

from larmorx.sdcflows.approx import BSplineApproxResult, bspline_approx
from larmorx.sdcflows.base import (
    EPI_LIKE_SUFFIXES,
    EstimatorRun,
    EstimatorWorkflow,
    FmapPreprocResult,
    build_workflows,
    check_readout,
    estimator_workflow,
    fmap_preproc_workflow,
    workflow_options,
)
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
from larmorx.sdcflows.correction import (
    MEM_PER_THREAD,
    UnwarpWorkflowResult,
    unwarp_num_threads,
    unwarp_workflow,
)
from larmorx.sdcflows.estimators import (
    MODALITIES,
    REGISTRY,
    SDCFLOWS_VERSIONS,
    EstimatorRegistry,
    EstimatorType,
    FieldmapEstimation,
    FieldmapFile,
    FieldmapRegistry,
    ImageFileError,
    MetadataError,
    bidict,
    clear_registry,
    get_identifier,
    relative_to_root,
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
from larmorx.sdcflows.outputs import (
    SDCFLOWS_OUT_PATH_BASE,
    FmapDerivativesResult,
    fmap_derivatives_workflow,
    gendesc,
    getname,
    getsourcetype,
    selectintent,
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
from larmorx.sdcflows.qwarp import (
    QWARP_ARGS,
    FlattenResult,
    flatten,
    qwarp2ants,
    qwarp_workflow,
    sorted_pe,
)
from larmorx.sdcflows.registration import (
    Coeff2EpiResult,
    coeff2epi_workflow,
    registration_settings,
)
from larmorx.sdcflows.ridge import RidgeResult, ridge_sparse_cg
from larmorx.sdcflows.syn import (
    SynPreprocessingResult,
    SynSdcResult,
    adjust_zooms,
    deoblique,
    fmap_atlas,
    fmap_atlas_affine,
    merge_meta,
    mm2vox,
    norm_lap,
    obliquity,
    reoblique,
    set_dtype,
    syn_preprocessing_workflow,
    syn_sdc_workflow,
    warp_dir,
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
from larmorx.sdcflows.wrangler import find_anatomical_estimators, find_estimators

__all__ = [
    "DEFAULT_HF_ZOOMS_MM",
    "DEFAULT_LF_ZOOMS_MM",
    "DEFAULT_ZOOMS_MM",
    "EPI_LIKE_SUFFIXES",
    "MEM_PER_THREAD",
    "MODALITIES",
    "NOT_ALIGNED",
    "QWARP_ARGS",
    "REGISTRY",
    "SDCFLOWS_OUT_PATH_BASE",
    "SDCFLOWS_VERSIONS",
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
    "EstimatorRegistry",
    "EstimatorRun",
    "EstimatorType",
    "EstimatorWorkflow",
    "FieldmapEstimation",
    "FieldmapFile",
    "FieldmapRegistry",
    "FlattenResult",
    "FmapDerivativesResult",
    "FmapPreprocResult",
    "FmapWorkflowResult",
    "GetReadoutTimeResult",
    "ImageFileError",
    "MagnitudeWorkflowResult",
    "MetadataError",
    "PhaseResult",
    "PhdiffWorkflowResult",
    "RidgeResult",
    "SelectPEVolumesResult",
    "SortPEBlipsResult",
    "SynPreprocessingResult",
    "SynSdcResult",
    "TopupNodeResult",
    "TopupWorkflowResult",
    "UniformGridResult",
    "UnwarpWorkflowResult",
    "adjust_zooms",
    "apply_coeffs_field",
    "au2rads",
    "bidict",
    "binary_dilation",
    "brain_extraction",
    "brain_masker",
    "brainextraction_workflow",
    "bspline_approx",
    "bspline_grid",
    "build_workflows",
    "check_b0_units",
    "check_readout",
    "check_register",
    "clear_registry",
    "coeff2epi_workflow",
    "collocation_matrix",
    "delta_te",
    "deoblique",
    "deoblique_and_zooms",
    "disp_to_fmap",
    "displacements_field_to_fieldmap",
    "estimator_workflow",
    "find_anatomical_estimators",
    "find_estimators",
    "flatten",
    "fmap_atlas",
    "fmap_atlas_affine",
    "fmap_derivatives_workflow",
    "fmap_preproc_workflow",
    "fmap_to_disp",
    "fmap_workflow",
    "gendesc",
    "get_identifier",
    "get_readout_time",
    "get_trt",
    "getname",
    "getsourcetype",
    "grid_bspline_weights",
    "magnitude_workflow",
    "merge_meta",
    "mm2vox",
    "norm_lap",
    "obliquity",
    "pad_slices",
    "phdiff2fmap",
    "phdiff_workflow",
    "qwarp2ants",
    "qwarp_workflow",
    "registration_settings",
    "relative_to_root",
    "reoblique",
    "reorient_image_and_metadata",
    "reorient_pedir",
    "resample_to_zooms",
    "rescale_affine",
    "ridge_sparse_cg",
    "select_pe_volumes",
    "select_topup_config",
    "selectintent",
    "set_dtype",
    "sort_pe_blips",
    "sorted_pe",
    "subtract_phases",
    "syn_preprocessing_workflow",
    "syn_sdc_workflow",
    "topup_coeff_reorient",
    "topup_config_file",
    "topup_mats_to_world",
    "topup_node",
    "topup_workflow",
    "transform_coefficients",
    "uniform_grid",
    "union",
    "unwarp_num_threads",
    "unwarp_workflow",
    "warp_dir",
    "workflow_options",
]
