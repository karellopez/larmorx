# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's confound estimation, ported from nipype 1.12.0 (``algorithms/confounds.py``),
niworkflows 1.15.4 and fMRIPrep 25.2.7 with the numpy, SciPy and nitime arithmetic under them,
bit for bit (the confounds' part fills it step by step).

- :func:`compute_dvars`, :func:`compute_dvars_node`: nipype's ``compute_dvars`` (the
  standardised, plain and voxel-wise standardised DVARS, with nitime's AR(1) estimate) and the
  ``ComputeDVARS`` interface's files.
"""

from larmorx.confounds.dvars import (
    ComputeDvarsResult,
    DvarsResult,
    compute_dvars,
    compute_dvars_node,
)

__all__ = [
    "ComputeDvarsResult",
    "DvarsResult",
    "compute_dvars",
    "compute_dvars_node",
]
