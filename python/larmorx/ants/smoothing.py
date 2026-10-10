# SPDX-License-Identifier: Apache-2.0
"""Gaussian filtering as ANTs does it: ``SmoothImage``, ``ResampleImageBySpacing`` and
ImageMath's ``G``, ``Laplacian``, ``Grad`` and ``UnsharpMask``.

Every function takes an image (path, :class:`~larmorx.Image`, nibabel image or
``(array, affine)``) in 2, 3 or 4 dimensions and filters it in all of them, as ITK does (a 4D
image is smoothed along time too, with the time step as its spacing). Results are float32
and do not depend on ``n_threads``.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.ants._filters import float_input, memory_image, result
from larmorx.image import Image

__all__ = [
    "discrete_gaussian",
    "gradient_magnitude",
    "laplacian",
    "resample_image_by_spacing",
    "smooth_image",
    "unsharp_mask",
]


def _floats(value: float | Sequence[float]) -> list[float]:
    if isinstance(value, (int, float, np.integer, np.floating)):
        return [float(value)]
    return [float(v) for v in value]


def smooth_image(
    image: Any,
    sigma: float | Sequence[float],
    *,
    sigma_in_physical_units: bool = False,
    median: bool = False,
    n_threads: int = 1,
) -> Image:
    """``SmoothImage d in sigma out [physical] [median]``: Gaussian smoothing with ITK's
    ``SmoothingRecursiveGaussianImageFilter``.

    ``sigma`` is one value or one per axis, in voxels (multiplied by the spacing) unless
    ``sigma_in_physical_units``. With ``median=True`` the image is median-filtered instead,
    ``sigma`` being the radius of the box in voxels (truncated to an integer).
    """
    a = float_input(image, n_threads)
    data = _core.ants_smooth_image(
        np.asfortranarray(a.data),
        list(a.spacing),
        _floats(sigma),
        bool(sigma_in_physical_units),
        bool(median),
        n_threads,
    )
    return result(data, a)


def discrete_gaussian(image: Any, sigma: float | Sequence[float], *, n_threads: int = 1) -> Image:
    """``ImageMath d out G in sigma``: ITK's ``DiscreteGaussianImageFilter`` with variance
    ``sigma²`` in physical units (one sigma or one per axis), maximum error 0.01 and kernels
    of at most 32 voxels on each side."""
    a = float_input(image, n_threads)
    data = _core.ants_discrete_gaussian(
        np.asfortranarray(a.data), list(a.spacing), _floats(sigma), n_threads
    )
    return result(data, a)


def laplacian(
    image: Any, sigma: float = 1.0, *, normalize: bool = False, n_threads: int = 1
) -> Image:
    """``ImageMath d out Laplacian in sigma``: ITK's ``LaplacianRecursiveGaussianImageFilter``
    (sigma in physical units; a sigma ≤ 0 becomes 0.5), rescaled to ``[0, 1]`` if
    ``normalize``.

    On the command line ANTs reads the normalize flag from the sigma argument itself
    (``std::stoi("1.5")`` is 1), so fMRIPrep's ``Laplacian in 1.5 1`` is
    ``laplacian(img, 1.5, normalize=True)``; here the two are independent.
    """
    a = float_input(image, n_threads)
    data = _core.ants_laplacian(
        np.asfortranarray(a.data), list(a.spacing), float(sigma), bool(normalize), n_threads
    )
    return result(data, a)


def gradient_magnitude(
    image: Any, sigma: float = 1.0, *, normalize: bool = False, n_threads: int = 1
) -> Image:
    """``ImageMath d out Grad in sigma normalize``: ITK's
    ``GradientMagnitudeRecursiveGaussianImageFilter`` (sigma in physical units; a sigma ≤ 0
    becomes 0.5), rescaled to ``[0, 1]`` if ``normalize``."""
    a = float_input(image, n_threads)
    data = _core.ants_gradient_magnitude(
        np.asfortranarray(a.data), list(a.spacing), float(sigma), bool(normalize), n_threads
    )
    return result(data, a)


def unsharp_mask(
    image: Any,
    amount: float = 0.5,
    radius: float = 1.0,
    threshold: float = 0.0,
    *,
    radius_in_physical_units: bool = False,
    n_threads: int = 1,
) -> Image:
    """``ImageMath d out UnsharpMask in amount radius threshold physical``: ITK's
    ``UnsharpMaskImageFilter`` in float. The image is smoothed with a Gaussian of sigma
    ``radius`` (voxels unless ``radius_in_physical_units``), and each voxel moves away from
    the smoothed value by ``amount`` times the difference beyond ``threshold``."""
    a = float_input(image, n_threads)
    data = _core.ants_unsharp_mask(
        np.asfortranarray(a.data),
        list(a.spacing),
        float(amount),
        float(radius),
        float(threshold),
        bool(radius_in_physical_units),
        n_threads,
    )
    return result(data, a)


def resample_image_by_spacing(
    image: Any,
    spacing: Sequence[float],
    *,
    smooth: bool = True,
    add_voxels: int = 0,
    nearest: bool = False,
    n_threads: int = 1,
) -> Image:
    """``ResampleImageBySpacing d in out sx sy [sz [st]] smooth addvox nn``: the image resampled
    onto a grid with the same origin and direction and the new ``spacing`` (one value per
    axis; for a 4D image the last is the time step in seconds).

    The output has ``int(size · old spacing / new spacing + add_voxels)`` voxels on each axis.
    With ``smooth`` (as in ANTs, the default), each axis is first smoothed with a recursive
    Gaussian of sigma ``new/old - 1`` where that is positive (ITK reads it in millimetres:
    ``(new/old - 1) / old`` voxels). Interpolation is linear, or nearest-neighbour with
    ``nearest``. Output voxels beyond the input take the input's value at index ``(1, 1, …)``.

    Unlike the command line, ``nearest`` works in 2D too (there ANTs reads it from the
    ``addvox`` argument).
    """
    spacing = [float(s) for s in spacing]
    if isinstance(image, (str, os.PathLike)):
        from larmorx.io import read_header

        source: Any = os.fspath(image)
        header = read_header(image)
    else:
        source = memory_image(image)
        header = getattr(image, "header", None)
    data, affine, _descrip, out_spacing = _core.ants_resample_image_by_spacing(
        source, spacing, bool(smooth), int(add_voxels), bool(nearest), n_threads
    )
    if header is not None and len(out_spacing) >= 4:
        scale = {"msec": 1e-3, "usec": 1e-6}.get(header.time_unit, 1.0)
        pixdim = list(header.pixdim)
        pixdim[4] = float(out_spacing[3]) / scale
        header = header.replace(pixdim=tuple(pixdim))
    return Image(np.asfortranarray(data), affine, header)
