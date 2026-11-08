# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``NonsteadyStatesDetector`` and nipype's ``is_outlier`` under it.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's): ``interfaces/bold.py``; and nipype 1.12.0 (Apache-2.0, the nipype
developers; every environment's): ``algorithms/confounds.py`` (``is_outlier``). The arithmetic
is numpy's as they run it: the series read as nibabel's ``get_fdata(dtype='float32')``, its
first ``n_volumes`` volumes clipped to ``[0, p99.8]`` (numpy ``numpy_version``'s percentile),
each volume's mean summed in numpy's order (:func:`larmorx._core.numpy_add_reduce` over the
volume's values in memory order, divided by the count in ``float64``, cast to ``float32``), then
``is_outlier``'s modified z-scores in ``float32`` (numpy's medians:
:func:`larmorx._core.numpy_median`).

Corrected by default (``upstream_defects="reproduce"`` gives nipype's result):

- PIPE-23: ``is_outlier`` divides by the median absolute deviation, which is 0 when more than
  half of the volumes' means are equal (a constant signal, a synthetic or padded series): the
  volumes at the median get ``0/0 = NaN``, which fails the ``<= 3.5`` test, so they count as
  non-steady and every leading volume is discarded (``n_dummy`` the series' length). larmorx
  gives the volumes at the median a z-score of 0 (they are not outliers); the others, away from
  a zero spread, stay infinite outliers.
"""

from __future__ import annotations

import os
import warnings
from dataclasses import dataclass

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.io.nibabel_header import load_nibabel

__all__ = ["NonsteadyStatesResult", "is_outlier", "nonsteady_states_detector"]

NUMPY_NEWEST = "2.5.4"


def is_outlier(
    points: np.ndarray,
    thresh: float = 3.5,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> int:
    """nipype's ``is_outlier(points, thresh)``: the number of leading observations whose
    modified z-score (``0.6745 |x - median| / MAD``) is above ``thresh`` (one value per row for a
    2-D ``points``: the Euclidean distance to the median row). Returns an ``int``. PIPE-23: see
    the module."""
    _defects.check(upstream_defects)
    points = np.asarray(points)
    if points.ndim == 1:
        points = points[:, None]
    if points.ndim != 2:
        raise ValueError("points must be one or two dimensional")
    median = np.stack(
        [
            np.asarray(_core.numpy_median(np.ascontiguousarray(points[:, j]), axis=None))
            for j in range(points.shape[1])
        ]
    ).astype(points.dtype)
    sq = (points - median) ** 2
    diff = sq[:, 0].copy() if sq.shape[1] == 1 else np.sum(sq, axis=-1)
    diff = np.sqrt(diff)
    mad = np.asarray(_core.numpy_median(np.ascontiguousarray(diff), axis=None)).astype(diff.dtype)
    if upstream_defects == "reproduce" or mad != 0:
        z = 0.6745 * diff / mad
    else:
        with np.errstate(divide="ignore", invalid="ignore"):
            z = 0.6745 * diff / mad
        nan_z = z.copy()
        z = np.where(diff == 0, np.zeros_like(z), z)
        before, after = _count(nan_z, thresh), _count(z, thresh)
        if before != after:
            _defects.warn(
                [
                    (
                        "PIPE-23",
                        "nipype is_outlier",
                        upstream_defects,
                        f"the median absolute deviation is 0: the {int(np.sum(diff == 0))} "
                        f"values at the median are not outliers ({after} leading outliers; "
                        f"nipype's 0/0 makes {before})",
                    )
                ]
            )
    return _count(z, thresh)


def _count(z: np.ndarray, thresh: float) -> int:
    n = 0
    for v in z:
        if v <= thresh:
            break
        n += 1
    return n


@dataclass(frozen=True)
class NonsteadyStatesResult:
    """``NonsteadyStatesDetector``'s outputs: ``t_mask`` (``True`` for the volumes to average:
    the non-steady states, or the last ``zero_dummy_masked`` of the first ``n_volumes`` when
    fewer than two were found) and ``n_dummy`` (the number of non-steady states)."""

    t_mask: tuple[bool, ...]
    n_dummy: int


def _volume_means(data: np.ndarray, numpy_version: str) -> np.ndarray:
    """``np.mean(data, axis=(0, 1, 2))`` of a ``float32`` series whose fourth axis is the
    outermost in memory (nibabel's Fortran order): each volume's values added in memory order
    as numpy ``numpy_version`` adds a contiguous block, divided by the count (an ``intp``: in
    ``float64``) and cast to ``float32``."""
    count = np.float64(int(np.prod(data.shape[:3])))
    sums = []
    for t in range(data.shape[3]):
        vol = data[..., t]
        block = vol.ravel(order="F") if vol.flags.f_contiguous else vol.ravel(order="K")
        sums.append(_core.numpy_add_reduce(np.ascontiguousarray(block), numpy_version))
    return (np.asarray(sums, dtype=np.float32).astype(np.float64) / count).astype(np.float32)


def nonsteady_states_detector(
    in_file: str | os.PathLike[str],
    nonnegative: bool = True,
    n_volumes: int = 40,
    zero_dummy_masked: int = 20,
    *,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> NonsteadyStatesResult:
    """niworkflows' ``NonsteadyStatesDetector``: the initial non-steady states of a BOLD series
    (nipype's ``is_outlier`` of the volumes' mean signals, over the first ``n_volumes``, 10 to
    200, after clipping to ``[0, p99.8]``, or ``[p0.2, p99.8]`` without ``nonnegative``). A 3D
    image (or one volume): ``t_mask = (True,)``, ``n_dummy = 1``. ``zero_dummy_masked`` (2 to
    40) volumes are averaged when fewer than two non-steady states are found."""
    _defects.check(upstream_defects)
    if not 10 <= int(n_volumes) <= 200:
        raise ValueError(f"n_volumes must be in [10, 200], not {n_volumes}")
    if not 2 <= int(zero_dummy_masked) <= 40:
        raise ValueError(f"zero_dummy_masked must be in [2, 40], not {zero_dummy_masked}")
    img = load_nibabel(in_file, dtype=np.float32, n_threads=n_threads)
    data = np.asarray(img.data)
    ntotal = data.shape[-1] if data.ndim == 4 else 1
    if ntotal == 1:
        return NonsteadyStatesResult((True,), 1)
    data = data[..., : int(n_volumes)]
    flat = np.ascontiguousarray(data.ravel(order="K"))
    hi = np.float32(_core.numpy_percentile_weak(flat, 99.8, numpy_version))
    lo = 0.0 if nonnegative else np.float32(_core.numpy_percentile_weak(flat, 0.2, numpy_version))
    data = np.clip(data, a_min=lo, a_max=hi)
    means = _volume_means(data, numpy_version)
    with warnings.catch_warnings():
        if upstream_defects == "fix":
            warnings.simplefilter("ignore", RuntimeWarning)
        n_dummy = is_outlier(means, upstream_defects=upstream_defects)
    start, stop = 0, n_dummy
    if stop < 2:
        stop = min(ntotal, int(n_volumes))
        start = max(0, stop - int(zero_dummy_masked))
    t_mask = np.zeros((ntotal,), dtype=bool)
    t_mask[start:stop] = True
    return NonsteadyStatesResult(tuple(bool(v) for v in t_mask), int(n_dummy))
