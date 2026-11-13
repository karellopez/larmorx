# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``RobustACompCor`` and ``RobustTCompCor``: nipype's ``ACompCor`` and ``TCompCor``,
which fMRIPrep runs through them.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's): ``interfaces/patches.py``. The interfaces' files are
:func:`larmorx.confounds.acompcor`'s and :func:`larmorx.confounds.tcompcor`'s, bit for bit.

PIPE-48 (kept and documented, no switch): niworkflows reruns the interface when it raises
``numpy.linalg.LinAlgError`` (a workaround for a random failure of a BLAS library,
fMRIPrep issues 776 and 940), up to ten more times, sleeping ``randint(10 k - 6, 10 k)`` seconds
before the ``k``-th retry (about 8 to 9 minutes in all), then raises the last error. larmorx's
computation is deterministic, so a rerun gives the same error: the error is raised at once, the
error niworkflows raises after its retries. Results are never affected. With fMRIPrep's
``failure_mode='NaN'`` the decomposition's errors are caught by ``compute_noise_components`` and
never reach the retry; a non-finite voxel ends in a ``ValueError``, which niworkflows does not
retry either.
"""

from __future__ import annotations

from typing import Any

from larmorx.confounds.compcor import CompCorResult, acompcor, tcompcor

__all__ = ["robust_acompcor", "robust_tcompcor"]


def robust_acompcor(realigned_file: Any, mask_files: Any = None, **kwargs: Any) -> CompCorResult:
    """niworkflows' ``RobustACompCor``: :func:`larmorx.confounds.acompcor` with the same inputs
    (its keyword arguments). PIPE-48: no retry (see the module)."""
    return acompcor(realigned_file, mask_files, **kwargs)


def robust_tcompcor(realigned_file: Any, mask_files: Any = None, **kwargs: Any) -> CompCorResult:
    """niworkflows' ``RobustTCompCor``: :func:`larmorx.confounds.tcompcor` with the same inputs
    (its keyword arguments). PIPE-48: no retry (see the module)."""
    return tcompcor(realigned_file, mask_files, **kwargs)
