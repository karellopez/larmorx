# SPDX-License-Identifier: Apache-2.0
"""Mathematical morphology and mask operations as ANTs does them: ImageMath's ``MD``, ``ME``,
``MO``, ``MC`` (binary), ``GD``, ``GE``, ``GO``, ``GC`` (grayscale), ``FillHoles`` and
``PadImage``.

The structuring element is ITK's ``BinaryBallStructuringElement``: the voxels whose centre
lies within a sphere of diameter ``2·radius + 1`` voxels around the centre voxel (offsets
with ``Σ o² ≤ radius·(radius + 1)``), in index space and in all of the image's dimensions (a
4D image along time too). Every function takes an image (path, :class:`~larmorx.Image`,
nibabel image or ``(array, affine)``), returns float32 voxels with the input's affine and
header (``pad_image`` moves the origin), and does not depend on ``n_threads``.
"""

from __future__ import annotations

from typing import Any

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, warn
from larmorx.ants._filters import float_input, result
from larmorx.ants.resample import _result, _source
from larmorx.image import Image

__all__ = [
    "fill_holes",
    "grayscale_close",
    "grayscale_dilate",
    "grayscale_erode",
    "grayscale_open",
    "morphological_close",
    "morphological_dilate",
    "morphological_erode",
    "morphological_open",
    "morphology",
    "pad_image",
]

_OPERATIONS = ("MD", "ME", "MO", "MC", "GD", "GE", "GO", "GC")


def _radius(radius: int) -> int:
    r = int(radius)
    if r != radius or r < 0:
        raise ValueError(f"the radius is a number of voxels (0 or more), not {radius!r}")
    return r


def morphology(
    image: Any,
    operation: str,
    radius: int = 1,
    *,
    value: float = 1.0,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``ImageMath d out <operation> in radius value``: ANTs' ``ants::Morphological``.

    ``operation`` is ImageMath's name: ``"MD"``, ``"ME"``, ``"MO"``, ``"MC"`` (binary, with
    ``value`` as the foreground) or ``"GD"``, ``"GE"``, ``"GO"``, ``"GC"`` (grayscale;
    ``value`` is ignored). ``radius`` is in voxels (the command line truncates a fractional
    radius). ``ME``, and ``MO`` with the foreground 0, are corrected (ANTS-17, see
    :func:`morphological_erode` and :func:`morphological_open`);
    ``upstream_defects="reproduce"`` gives ANTs' results.
    """
    if operation not in _OPERATIONS:
        raise ValueError(f"unknown operation {operation!r}; choose from {_OPERATIONS}")
    check(upstream_defects)
    a = float_input(image, n_threads)
    data, notices = _core.ants_morphology(
        np.asfortranarray(a.data),
        operation,
        _radius(radius),
        float(value),
        n_threads,
        upstream_defects,
    )
    warn(notices)
    return result(data, a)


def morphological_dilate(
    image: Any, radius: int = 1, *, value: float = 1.0, n_threads: int = 1
) -> Image:
    """``ImageMath d out MD in radius value``: ITK's ``BinaryDilateImageFilter``. Voxels
    within the ball of a voxel equal to ``value`` become ``value``; the others keep their
    value."""
    return morphology(image, "MD", radius, value=value, n_threads=n_threads)


def morphological_erode(
    image: Any,
    radius: int = 1,
    *,
    value: float = 1.0,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``ImageMath d out ME in radius value``: ITK's ``BinaryErodeImageFilter`` (the image
    border does not erode), as a 0/1 mask: 1 where the voxel is still ``value`` after the
    erosion, 0 elsewhere. ANTs thresholds the eroded image at 0.5 instead, so voxels that are
    not ``value`` but above 0.5 (a label 2 in a mask of 1s) stay 1 and a foreground of 0.5 or
    less is lost (ANTS-17; ``upstream_defects="reproduce"``). On a 0/1 mask with ``value`` 1
    both agree."""
    return morphology(
        image, "ME", radius, value=value, n_threads=n_threads, upstream_defects=upstream_defects
    )


def morphological_open(
    image: Any,
    radius: int = 1,
    *,
    value: float = 1.0,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``ImageMath d out MO in radius value``: ITK's ``BinaryMorphologicalOpeningImageFilter``
    (erosion, the removed voxels set to 0, then dilation). With ``value`` 0 the removed voxels
    become 1 (for a 0/1 mask, the opening of its zeros); ITK sets them to 0, the foreground
    itself, so ANTs' ``MO r 0`` only dilates the zeros (ANTS-17;
    ``upstream_defects="reproduce"``)."""
    return morphology(
        image, "MO", radius, value=value, n_threads=n_threads, upstream_defects=upstream_defects
    )


def morphological_close(
    image: Any, radius: int = 1, *, value: float = 1.0, n_threads: int = 1
) -> Image:
    """``ImageMath d out MC in radius value``: ITK's ``BinaryMorphologicalClosingImageFilter``
    (with ``SafeBorder``: the image is padded by ``radius`` before dilating and eroding, so
    objects near the border close as if the image went on). Voxels that do not end as
    ``value`` keep their input value."""
    return morphology(image, "MC", radius, value=value, n_threads=n_threads)


def grayscale_dilate(image: Any, radius: int = 1, *, n_threads: int = 1) -> Image:
    """``ImageMath d out GD in radius``: the maximum over the ball."""
    return morphology(image, "GD", radius, n_threads=n_threads)


def grayscale_erode(image: Any, radius: int = 1, *, n_threads: int = 1) -> Image:
    """``ImageMath d out GE in radius``: the minimum over the ball."""
    return morphology(image, "GE", radius, n_threads=n_threads)


def grayscale_open(image: Any, radius: int = 1, *, n_threads: int = 1) -> Image:
    """``ImageMath d out GO in radius``: erosion then dilation, on the image padded by
    ``radius`` voxels of the float maximum (ITK's ``SafeBorder``)."""
    return morphology(image, "GO", radius, n_threads=n_threads)


def grayscale_close(image: Any, radius: int = 1, *, n_threads: int = 1) -> Image:
    """``ImageMath d out GC in radius``: dilation then erosion, on the image padded by
    ``radius`` voxels of the float minimum (ITK's ``SafeBorder``)."""
    return morphology(image, "GC", radius, n_threads=n_threads)


def fill_holes(
    image: Any,
    hole_param: float = 2.0,
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``ImageMath d out FillHoles in hole_param``: the background regions enclosed by the
    object (voxels in ``[0.5, 1e9]``) set to 1; the other voxels keep their value.

    The background is split into face-connected regions; the largest is the outside, the
    others are holes. With ``hole_param`` 2 (ANTs' default, as sMRIPrep calls it) every hole
    is filled; with ``hole_param ≤ 1`` a hole is filled when the share of its neighbouring
    voxels that belong to the object exceeds ``hole_param`` (1 means "enclosed by the object
    only"); between 1 and 2 every hole, above 2 none. Neighbours outside the image are not
    counted; ANTs reads them at other rows of the image or outside its memory (ANTS-19), which
    ``upstream_defects="reproduce"`` gives where defined (0 beyond the image's memory).
    """
    check(upstream_defects)
    a = float_input(image, n_threads)
    data, notices = _core.ants_fill_holes(
        np.asfortranarray(a.data), float(hole_param), n_threads, upstream_defects
    )
    warn(notices)
    return result(data, a)


def pad_image(
    image: Any,
    pad: float,
    value: float = 0.0,
    *,
    dimension: int | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """``ImageMath d out PadImage in pad value``: ``int(pad)`` voxels of ``value`` added on
    both sides of every axis (removed if ``pad`` is negative), the origin moved so that the
    voxels keep their positions in space. A 4D image is padded along time too. ``dimension``
    defaults to the image's.

    A fractional ``pad`` is truncated, the same number of voxels on each side. ANTs grows
    every axis by ``int(2·pad)`` but shifts the image by ``int(|pad|)``: the extra voxel goes
    at the end, and a negative fractional pad also moves the voxels by one in space (ANTS-22;
    ``upstream_defects="reproduce"``).
    """
    check(upstream_defects)
    source, header, shape, _spacing = _source(image)
    dim = int(dimension) if dimension is not None else len(shape)
    (data, affine, _descrip, spacing), notices = _core.ants_pad_image(
        source, dim, float(pad), float(value), n_threads, upstream_defects
    )
    warn(notices)
    return _result(data, affine, header, spacing)
