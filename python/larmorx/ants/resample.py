# SPDX-License-Identifier: Apache-2.0
"""Resampling onto a new spacing or size, as ANTs' ``ResampleImageBySpacing`` and
``ResampleImage`` do (ITK's ``ResampleImageFilter`` with an identity transform: the output
keeps the input's origin and direction)."""

from __future__ import annotations

import os
from collections.abc import Sequence
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.ants._filters import extra_spacing, memory_image
from larmorx.image import Image, as_image

__all__ = ["PIXEL_TYPES", "resample_image", "resample_image_by_spacing"]

#: ``ResampleImage``'s pixel types: its numbers and the names :func:`resample_image` takes.
PIXEL_TYPES = ("char", "uchar", "short", "ushort", "int", "uint", "float", "double")

_TIME_SCALE = {"msec": 1e-3, "usec": 1e-6}


def _source(image: Any) -> tuple[Any, Any, tuple[int, ...], tuple[float, ...]]:
    """What the bindings take (a path or an in-memory tuple), the NIfTI header, and ITK's
    size and spacing of the image."""
    if isinstance(image, (str, os.PathLike)):
        from larmorx.io import read_header

        header = read_header(image)
        geometry = header.itk_geometry
        return os.fspath(image), header, tuple(geometry.size), tuple(geometry.spacing)
    img = as_image(image)
    data = np.asarray(img.data)
    rzs = np.asarray(img.affine, dtype=np.float64)[:3, :3]
    spacing = [float(v) for v in np.sqrt(np.sum(rzs * rzs, axis=0))][: data.ndim]
    spacing += extra_spacing(img.header, data.ndim) or [1.0] * (data.ndim - len(spacing))
    return memory_image(img), img.header, data.shape, tuple(spacing[: data.ndim])


def _result(data: np.ndarray, affine: np.ndarray, header: Any, spacing: Sequence[float]) -> Image:
    """An output with the input's header, its time step updated for a 4D image."""
    if header is not None and len(spacing) >= 4:
        pixdim = list(header.pixdim)
        pixdim[4] = float(spacing[3]) / _TIME_SCALE.get(header.time_unit, 1.0)
        header = header.replace(pixdim=tuple(pixdim))
    return Image(np.asfortranarray(data), affine, header)


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
    source, header, _size, _spacing = _source(image)
    data, affine, _descrip, out_spacing = _core.ants_resample_image_by_spacing(
        source, [float(s) for s in spacing], bool(smooth), int(add_voxels), bool(nearest), n_threads
    )
    return _result(data, affine, header, out_spacing)


def _values(value: float | Sequence[float] | None) -> list[float] | None:
    if value is None:
        return None
    if isinstance(value, (int, float, np.integer, np.floating)):
        return [float(value)]
    return [float(v) for v in value]


def resample_image(
    image: Any,
    spacing: float | Sequence[float] | None = None,
    *,
    size: int | Sequence[int] | None = None,
    interpolation: str = "linear",
    sigma: float | Sequence[float] | None = None,
    alpha: float = 1.0,
    window: str = "hamming",
    order: int = 3,
    pixel_type: str = "float",
    dimension: int | None = None,
    n_threads: int = 1,
) -> Image:
    """``ResampleImage d in out MxNxO [size?] [interpolation] [pixeltype]``: the image resampled
    onto a grid with the same origin and direction and a new ``spacing`` or ``size`` (one value
    or one per axis), without smoothing.

    By ``size``, the spacing becomes ``old · (old size - 1) / (size - 1)``, so the first and
    last voxel centres stay in place; by ``spacing``, the size becomes
    ``int(old · old size / spacing + 0.5)``.

    ``interpolation``: ``"linear"`` (default), ``"nearest"``, ``"gaussian"`` (``sigma`` in mm,
    default the input spacing, and ``alpha``), ``"sinc"`` (``window``: ``"hamming"``,
    ``"cosine"``, ``"welch"``, ``"lanczos"``, ``"blackman"``; outside the image the nearest
    edge voxel, as ResampleImage's ITK default) or ``"bspline"`` (``order`` 0 to 5). Gaussian,
    sinc and B-spline interpolation need a 3D image.

    ``pixel_type`` (one of :data:`PIXEL_TYPES`) is the type the image is read as and written
    in: values are clamped to its range and truncated toward zero for integer types, as ITK
    casts them. ``dimension`` defaults to the image's.

    Unlike the command line, every window and B-spline order is reachable here (ANTs reads the
    window, the order and the pixel type from the same argument).
    """
    source, header, shape, in_spacing = _source(image)
    dim = int(dimension) if dimension is not None else len(shape)
    if pixel_type not in PIXEL_TYPES:
        raise ValueError(f"unknown pixel type {pixel_type!r}; choose from {PIXEL_TYPES}")
    if (spacing is None) == (size is None):
        raise ValueError("give either spacing or size")
    sizes = None
    if size is not None:
        sizes = [int(size)] if isinstance(size, (int, np.integer)) else [int(n) for n in size]
    sig = _values(sigma)
    if interpolation == "gaussian" and sig is None:
        sig = list(in_spacing[:3])
    data, affine, _descrip, out_spacing = _core.ants_resample_image(
        source,
        dim,
        _values(spacing),
        sizes,
        interpolation,
        sig,
        float(alpha),
        window,
        int(order),
        pixel_type,
        n_threads,
    )
    return _result(data, affine, header, out_spacing)
