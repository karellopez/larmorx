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

__all__ = [
    "CompCorResult",
    "ComputeDvarsResult",
    "DvarsResult",
    "acompcor",
    "combine_mask_files",
    "compcor",
    "compute_dvars",
    "compute_dvars_node",
    "compute_noise_components",
    "compute_tstd",
    "cosine_drift",
    "cosine_filter",
    "fallback_svd",
    "full_rank",
    "regress_poly",
    "tcompcor",
]
