# SPDX-License-Identifier: Apache-2.0
"""nipype's ``NonSteadyStateDetector``: the non-steady-state volumes at the start of a series.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers; every environment's):
``algorithms/confounds.py`` (``NonSteadyStateDetector``, ``is_outlier``), with numpy's
arithmetic under it, bit for bit: the first 50 volumes as nibabel's ``dataobj[:, :, :, :50]``
gives them (the file's type, or float64 when the header scales the data; only those volumes are
read, as nibabel reads them), averaged over the first, second and third axes in turn (numpy
``numpy_version``'s ``mean(axis=0)`` of a Fortran-ordered array: float32 data in float32, the
integer types in float64), then nipype's ``is_outlier`` of that global signal
(:func:`larmorx.niworkflows.is_outlier`).

niworkflows' ``NonsteadyStatesDetector`` (fMRIPrep's), which clips the series first and averages
other volumes, is :func:`larmorx.niworkflows.nonsteady_states_detector`.

Corrected by default (``upstream_defects="reproduce"`` gives nipype's result): PIPE-23, in
``is_outlier``: a median absolute deviation of 0 (more than half of the volumes' means equal)
counts every leading volume as non-steady; larmorx gives the volumes at the median a z-score of
0.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName

__all__ = ["NonSteadyStateResult", "nonsteady_state_detector"]

NUMPY_NEWEST = "2.5.4"

#: nipype's number of volumes read (``dataobj[:, :, :, :50]``).
VOLUMES = 50


@dataclass(frozen=True)
class NonSteadyStateResult:
    """``NonSteadyStateDetector``'s output, ``n_volumes_to_discard``, with the global signal it
    came from (the mean of each of the first 50 volumes, in nipype's type)."""

    n_volumes_to_discard: int
    global_signal: np.ndarray


def _first_volumes(path: str, n_threads: int) -> np.ndarray:
    """nibabel's ``nb.load(path).dataobj[:, :, :, :50]`` (Fortran order)."""
    from larmorx.io.nifti import read_header

    if len(read_header(path).shape) < 4:
        # nibabel's canonical_slicers indexes the image's shape at the fourth slicer.
        raise IndexError("tuple index out of range")
    return _core.nifti_read(path, "auto", n_threads, volumes=VOLUMES)[0]


def _mean_axis0(data: np.ndarray, numpy_version: str) -> np.ndarray:
    """``data.mean(axis=0)`` of a Fortran-ordered array as numpy ``numpy_version`` computes it:
    float32 and float64 in their own type, the integer types in float64 (numpy's buffered cast
    is exact and keeps the order: each volume is cast and averaged on its own, so no float64 copy
    of the series is held); the result Fortran-ordered."""
    if data.dtype in (np.float32, np.float64):
        return _core.numpy_reduce(np.asfortranarray(data), "mean", 0, str(numpy_version))
    if data.dtype.kind == "c":
        raise NotImplementedError("complex series are not ported")
    if data.dtype.kind not in "iu":
        raise NotImplementedError(f"{data.dtype} series are not ported")
    shape = data.shape
    volumes = np.asfortranarray(data).reshape((*shape[:3], -1), order="F")
    out = np.empty(shape[1:3] + volumes.shape[3:], dtype=np.float64, order="F")
    for m in range(volumes.shape[3]):
        out[:, :, m] = _core.numpy_reduce(
            volumes[..., m].astype(np.float64, order="F"), "mean", 0, str(numpy_version)
        )
    return out.reshape(shape[1:], order="F")


def nonsteady_state_detector(
    in_file: str | os.PathLike[str],
    *,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> NonSteadyStateResult:
    """nipype's ``NonSteadyStateDetector``: the number of non-steady-state volumes at the
    beginning of the scan ``in_file`` (a 4-D NIfTI file): nipype's ``is_outlier`` of the mean
    signal of each of the first 50 volumes. A file with fewer than four axes raises nibabel's
    ``IndexError``; with more than four, each volume's means form a row of ``is_outlier``'s
    points. PIPE-23: see the module."""
    from larmorx.niworkflows.bold import is_outlier

    _defects.check(upstream_defects)
    path = os.fspath(in_file)
    if not os.path.isfile(path):
        raise ValueError(
            "The trait 'in_file' of a NonSteadyStateDetectorInputSpec instance is an existing "
            f"file name, but the path '{path}' does not exist."
        )
    data: Any = _first_volumes(path, n_threads)
    signal = _mean_axis0(data, numpy_version)
    del data
    signal = _core.numpy_reduce(np.asfortranarray(signal), "mean", 0, str(numpy_version))
    signal = _core.numpy_reduce(np.asfortranarray(signal), "mean", 0, str(numpy_version))
    signal = np.asarray(signal)
    n = is_outlier(signal, upstream_defects=upstream_defects)
    return NonSteadyStateResult(n_volumes_to_discard=int(n), global_signal=signal)
