# SPDX-License-Identifier: Apache-2.0
"""Connected components, distance maps and label values as ANTs computes them: ImageMath's
``GetLargestComponent``, ``D``, ``MaurerDistance``, ``ThresholdAtMean`` and
``ReplaceVoxelValue``.

Every function takes an image (path, :class:`~larmorx.Image`, nibabel image or
``(array, affine)``) in 2, 3 or 4 dimensions (a 4D image is one 4D volume: components and
distances run across time too, with the time step as its spacing), returns float32 voxels
with the input's affine and header, and does not depend on ``n_threads``.
"""

from __future__ import annotations

from typing import Any

import numpy as np

from larmorx import _core
from larmorx.ants._filters import float_input, result
from larmorx.image import Image

__all__ = [
    "distance_map",
    "largest_component",
    "maurer_distance",
    "replace_voxel_value",
    "threshold_at_mean",
]


def largest_component(image: Any, min_size: int = 50, *, n_threads: int = 1) -> Image:
    """``ImageMath d out GetLargestComponent in min_size``: 1 in the largest face-connected
    component of the voxels in ``[0.25, 1e9]``, 0 elsewhere.

    Components smaller than ``min_size`` voxels are dropped first (0 keeps every component).
    As in ANTs, every component of the largest size is kept (ties keep all of them), and **if
    no component is left, every voxel becomes 1**.
    """
    if int(min_size) != min_size or min_size < 0:
        raise ValueError(f"min_size is a number of voxels (0 or more), not {min_size!r}")
    a = float_input(image, n_threads)
    data = _core.ants_largest_component(np.asfortranarray(a.data), int(min_size), n_threads)
    return result(data, a)


def distance_map(image: Any) -> Image:
    """``ImageMath d out D in``: ITK's Danielsson distance map, the distance in millimetres
    from every voxel to the nearest non-zero voxel (0 on them).

    Danielsson's propagation is not exactly Euclidean (it can miss the nearest voxel by a
    fraction of a voxel); larmorx replays ITK's sweeps in the same order, so it gives the
    same values. It runs on one thread, as ITK's does.
    """
    a = float_input(image)
    data = _core.ants_distance_map(np.asfortranarray(a.data), list(a.spacing))
    return result(data, a)


def maurer_distance(image: Any, foreground: float = 1.0, *, n_threads: int = 1) -> Image:
    """``ImageMath d out MaurerDistance in foreground``: ITK's signed Maurer distance map of
    the voxels equal to ``foreground``: the distance in millimetres to the object's inner
    contour (its voxels that touch the background), negative inside the object (``-0.0`` on
    the contour). Without object or background voxels, every voxel is about ``±1.84e19``."""
    a = float_input(image, n_threads)
    data = _core.ants_maurer_distance(
        np.asfortranarray(a.data), list(a.spacing), float(foreground), n_threads
    )
    return result(data, a)


def threshold_at_mean(image: Any, fraction: float = 1.0, *, n_threads: int = 1) -> Image:
    """``ImageMath d out ThresholdAtMean in fraction``: 1 where the value is at least
    ``mean · fraction`` (and at most the maximum), else 0. The mean is a float sum in image
    order, as in ANTs. A fraction above ``max / mean`` raises ``ValueError`` (ANTs aborts)."""
    a = float_input(image, n_threads)
    data = _core.ants_threshold_at_mean(np.asfortranarray(a.data), float(fraction), n_threads)
    return result(data, a)


def replace_voxel_value(
    image: Any, low: float, high: float, value: float, *, n_threads: int = 1
) -> Image:
    """``ImageMath d out ReplaceVoxelValue in low high value``: voxels with
    ``low ≤ v ≤ high`` set to ``value``."""
    a = float_input(image, n_threads)
    data = _core.ants_replace_voxel_value(
        np.asfortranarray(a.data), float(low), float(high), float(value), n_threads
    )
    return result(data, a)
