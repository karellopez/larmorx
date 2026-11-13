# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's confound estimation, ported from nipype 1.12.0 (``algorithms/confounds.py``),
niworkflows 1.15.4 and fMRIPrep 25.2.7 with the numpy, SciPy and nitime arithmetic under them,
bit for bit (the confounds' part fills it step by step).

- :func:`compute_dvars`, :func:`compute_dvars_node`: nipype's ``compute_dvars`` (the
  standardised, plain and voxel-wise standardised DVARS, with nitime's AR(1) estimate) and the
  ``ComputeDVARS`` interface's files.
- :func:`compcor`, :func:`acompcor`, :func:`tcompcor`: nipype's ``CompCor``, ``ACompCor`` and
  ``TCompCor`` interfaces' files; :func:`compute_noise_components` and the functions under it
  (:func:`regress_poly`, :func:`cosine_filter`, :func:`cosine_drift`, :func:`full_rank`,
  :func:`compute_tstd`, :func:`fallback_svd`, :func:`combine_mask_files`).
- :func:`tsnr`: nipype's ``TSNR`` (the tSNR, mean and standard deviation maps, the detrended
  series).
- :func:`nonsteady_state_detector`: nipype's ``NonSteadyStateDetector`` (``is_outlier`` of the
  first 50 volumes' mean signal; niworkflows' version, fMRIPrep's, is
  :func:`larmorx.niworkflows.nonsteady_states_detector`).
- :func:`framewise_displacement`, :func:`normalize_mc_params`: nipype's
  ``FramewiseDisplacement`` and the motion parameters' conversion to SPM's order and units.
- :func:`fmriprep_framewise_displacement`, :func:`filter_dropped`, :func:`rename_acompcor`,
  :func:`gather_confounds`: fMRIPrep's own ``FramewiseDisplacement``, ``FilterDropped``,
  ``RenameACompCor`` and ``GatherConfounds`` (the tables read and written as pandas reads and
  writes them: PD-01).
- :func:`fsl_motion_params`, :func:`fsl_rms_deviation`: fMRIPrep's ``FSLMotionParams`` (25.2.7's
  and 25.2.6's forms) and ``FSLRMSDeviation``: the motion parameters and the root mean square
  deviation of the head-motion transforms (nitransforms' ITK reading and FSL conversion, SciPy's
  ``Rotation``).
- :func:`binary_union`, :func:`acompcor_masks`, :func:`acompcor_masks_node`, :func:`mask2vf`,
  :func:`get_zooms`: fMRIPrep's masks: the brain masks' union (``_binary_union``) and the
  aCompCor tissue masks (``acompcor_masks``, ``aCompCorMasks``, ``mask2vf``; PIPE-74 to
  PIPE-76). niworkflows' ``BinaryDilation``, ``BinarySubtraction`` and ``SignalExtraction``,
  which make the crown mask and the tissue signals, are :mod:`larmorx.niworkflows.morphology`
  and :func:`larmorx.niworkflows.images.signal_extraction` (PIPE-77, PIPE-78).
"""

from larmorx.confounds.compcor import (
    CompCorResult,
    acompcor,
    combine_mask_files,
    compcor,
    compute_noise_components,
    compute_tstd,
    cosine_drift,
    cosine_filter,
    fallback_svd,
    full_rank,
    regress_poly,
    tcompcor,
)
from larmorx.confounds.dvars import (
    ComputeDvarsResult,
    DvarsResult,
    compute_dvars,
    compute_dvars_node,
)
from larmorx.confounds.fmriprep import (
    GatherConfoundsResult,
    RenameACompCorResult,
    filter_dropped,
    fmriprep_framewise_displacement,
    gather_confounds,
    rename_acompcor,
)
from larmorx.confounds.framewise import (
    FramewiseDisplacementResult,
    framewise_displacement,
    normalize_mc_params,
)
from larmorx.confounds.masks import (
    AcompcorMasksResult,
    acompcor_masks,
    acompcor_masks_node,
    binary_union,
    get_zooms,
    mask2vf,
)
from larmorx.confounds.motion import (
    MotionParamsResult,
    RmsDeviationResult,
    fsl_motion_params,
    fsl_rms_deviation,
)
from larmorx.confounds.nonsteady import NonSteadyStateResult, nonsteady_state_detector
from larmorx.confounds.tsnr import TsnrResult, tsnr

__all__ = [
    "AcompcorMasksResult",
    "CompCorResult",
    "ComputeDvarsResult",
    "DvarsResult",
    "FramewiseDisplacementResult",
    "GatherConfoundsResult",
    "MotionParamsResult",
    "NonSteadyStateResult",
    "RenameACompCorResult",
    "RmsDeviationResult",
    "TsnrResult",
    "acompcor",
    "acompcor_masks",
    "acompcor_masks_node",
    "binary_union",
    "combine_mask_files",
    "compcor",
    "compute_dvars",
    "compute_dvars_node",
    "compute_noise_components",
    "compute_tstd",
    "cosine_drift",
    "cosine_filter",
    "fallback_svd",
    "filter_dropped",
    "fmriprep_framewise_displacement",
    "framewise_displacement",
    "fsl_motion_params",
    "fsl_rms_deviation",
    "full_rank",
    "gather_confounds",
    "get_zooms",
    "mask2vf",
    "nonsteady_state_detector",
    "normalize_mc_params",
    "regress_poly",
    "rename_acompcor",
    "tcompcor",
    "tsnr",
]
