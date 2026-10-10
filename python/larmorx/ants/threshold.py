# SPDX-License-Identifier: Apache-2.0
"""``ThresholdImage`` and ``MultiplyImages``."""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.ants._filters import float_input, mask_input, result
from larmorx.image import Image

__all__ = ["OtsuResult", "multiply_images", "otsu_threshold", "threshold_image"]


def threshold_image(
    image: Any,
    lower: float,
    upper: float,
    inside: float = 1.0,
    outside: float = 0.0,
    *,
    n_threads: int = 1,
) -> Image:
    """``ThresholdImage d in out lower upper inside outside``: ``inside`` where
    ``lower <= v <= upper`` (bounds compared in float32), else ``outside``. A float32
    image, as ANTs writes it.

    fMRIPrep binarises a probability map with ``threshold_image(prob, 0.5, 1.0)``.
    ``lower > upper`` raises ``ValueError`` (ANTs aborts).
    """
    a = float_input(image, n_threads)
    data = _core.ants_threshold_image(
        np.asfortranarray(a.data),
        float(lower),
        float(upper),
        float(inside),
        float(outside),
        n_threads,
    )
    return result(data, a)


@dataclass(frozen=True)
class OtsuResult:
    """Otsu labels and thresholds."""

    #: Labels: ``0..n`` without a mask; ``1..n+1`` inside the mask and 0 outside with one.
    image: Image
    #: The thresholds (upper bounds of the chosen histogram bins).
    thresholds: tuple[float, ...]


def otsu_threshold(
    image: Any, n_thresholds: int = 1, *, mask: Any = None, n_threads: int = 1
) -> OtsuResult:
    """``ThresholdImage d in out Otsu n [mask]``: ``n`` thresholds that maximise the
    between-class variance.

    Without a mask this is ITK's ``OtsuMultipleThresholdsImageFilter`` on a 128-bin
    histogram: voxels are labelled ``0..n``. With a mask (its voxels equal to 1 after
    conversion to ``int``) it is ANTs' own variant: a 200-bin histogram of the masked voxels,
    labels ``1..n+1`` inside the mask, 0 outside, and the output takes the mask's geometry.
    """
    a = float_input(image, n_threads)
    like = a
    m = None
    if mask is not None:
        m = np.asfortranarray(mask_input(mask, a.data.shape, n_threads))
        if not isinstance(mask, np.ndarray):
            like = float_input(mask, n_threads)  # the output takes the mask's geometry
    labels, thresholds = _core.ants_otsu_threshold(
        np.asfortranarray(a.data), int(n_thresholds), m, n_threads
    )
    return OtsuResult(result(labels, like), tuple(float(t) for t in thresholds))


def multiply_images(image: Any, other: Any, *, n_threads: int = 1) -> Image:
    """``MultiplyImages d image other out``: the voxel-wise product in float32, with
    ``other`` a number or an image (read at ``image``'s voxel indices; its geometry is
    ignored, as in ANTs).

    Unlike the command line, a path that does not exist is an error here (ANTs multiplies by
    ``atof(path)``, usually 0).
    """
    a = float_input(image, n_threads)
    if isinstance(other, (int, float, np.integer, np.floating)):
        b: Any = float(other)
    else:
        if isinstance(other, (str, os.PathLike)) and not os.path.exists(other):
            raise FileNotFoundError(f"{os.fspath(other)}: no such file")
        b = np.asfortranarray(float_input(other, n_threads).data)
    data, _result, _count = _core.ants_arithmetic("m", np.asfortranarray(a.data), b, n_threads)
    return result(data, a)
