# SPDX-License-Identifier: Apache-2.0
"""Helpers shared by the wrappers of ANTs' image programs (ImageMath, ThresholdImage, ...).

ANTs programs read their images as ``float`` (``itk::Image<float, D>``), and masks often as
``int``. The wrappers do the same: a path is read as ITK 5.4.5 reads it (geometry and
scaling, as for :func:`larmorx.ants.apply_transforms`), and every input is converted the way
a C++ ``static_cast`` converts it. Results keep the input's affine and header.

Spatial filters also need ITK's **spacing** of every axis: for a path, the spacing ITK reads
(the time spacing of a 4D image in seconds); for an in-memory image, the lengths of the
affine's columns, and the header's repetition time for a fourth axis (1 without a header),
as :func:`larmorx.ants.image_math` places in-memory images.

**Adding a wrapper for a new filter group:** write ``python/larmorx/ants/<group>.py`` with
one function per filter that calls :func:`float_input` (and :func:`mask_input`), the
group's ``_core.ants_*`` binding, and :func:`result`; export it from
``larmorx/ants/__init__.py``; add the binding's stub to ``larmorx/_core.pyi``.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image

__all__ = [
    "FloatInput",
    "extra_spacing",
    "float_input",
    "mask_input",
    "memory_image",
    "result",
]

#: Seconds per NIfTI time unit, as ITK's reader scales a fourth axis.
_TIME_SCALE = {"msec": 1e-3, "usec": 1e-6}


@dataclass(frozen=True)
class FloatInput:
    """An input image as ANTs holds it: float32 voxels, the RAS+ affine, the NIfTI header, and
    ITK's spacing of every axis (mm; seconds for a fourth axis)."""

    data: np.ndarray
    affine: np.ndarray
    header: Any = None
    spacing: tuple[float, ...] = ()


def extra_spacing(header: Any, ndim: int) -> list[float] | None:
    """The spacing ITK reads for the axes after the third (``pixdim[4:]``, the time step in
    seconds), from a NIfTI header; ``None`` without a header or a fourth axis."""
    if header is None or ndim < 4:
        return None
    scale = _TIME_SCALE.get(getattr(header, "time_unit", "sec"), 1.0)
    out = [float(header.pixdim[4]) * scale]
    out += [float(header.pixdim[i]) for i in range(5, min(ndim, 7) + 1)]
    return [v if np.isfinite(v) and v > 0 else 1.0 for v in out]


def _memory_spacing(affine: np.ndarray, header: Any, ndim: int) -> tuple[float, ...]:
    rzs = np.asarray(affine, dtype=np.float64)[:3, :3]
    spatial = [float(v) for v in np.sqrt(np.sum(rzs * rzs, axis=0))]
    extra = extra_spacing(header, ndim) or []
    extra += [1.0] * (ndim - 3 - len(extra))
    return tuple((spatial + extra)[:ndim])


def _raw(image: Any, n_threads: int) -> tuple[np.ndarray, np.ndarray, Any, tuple[float, ...]]:
    """The voxels as ITK's reader produces them (for a path) or as given, with the affine,
    header and ITK spacing."""
    if isinstance(image, (str, os.PathLike)):
        data, info = _core.itk_read_image(os.fspath(image), n_threads)
        from larmorx.io import read_header

        spacing = tuple(float(s) for s in info["spacing"])
        return data, np.asarray(info["ras_affine"]), read_header(image), spacing
    img = as_image(image)
    data = np.asarray(img.data)
    if data.dtype.kind == "c":
        raise ValueError("complex images are not supported")
    return data, img.affine, img.header, _memory_spacing(img.affine, img.header, data.ndim)


def float_input(image: Any, n_threads: int = 1) -> FloatInput:
    """``image`` (a path, :class:`~larmorx.Image`, nibabel image or ``(array, affine)``) as a
    float32 image. A path is read as ITK reads it."""
    data, affine, header, spacing = _raw(image, n_threads)
    if data.dtype != np.float32:
        data = data.astype(np.float32)
    return FloatInput(data, affine, header, spacing)


def mask_input(mask: Any, shape: tuple[int, ...], n_threads: int = 1) -> np.ndarray:
    """A mask's voxels as float64 (the bindings convert them to the pixel type ANTs reads the
    mask as); its shape must match the image's. An array is taken as is."""
    data = mask if isinstance(mask, np.ndarray) else _raw(mask, n_threads)[0]
    data = np.asarray(data, dtype=np.float64)
    if data.shape != tuple(shape):
        raise ValueError(f"the mask shape {data.shape} differs from the image shape {tuple(shape)}")
    return data


def memory_image(image: Any) -> tuple[np.ndarray, np.ndarray, bytes | None, list[float] | None]:
    """An in-memory image as the bindings place it (``(data, affine, descrip, extra
    spacing)``): float32 or float64 voxels, the RAS+ affine, the header's ``descrip``, and the
    header's time step for a fourth axis."""
    img = as_image(image)
    data = np.asarray(img.data)
    if data.dtype not in (np.float32, np.float64):
        data = data.astype(np.float64)
    descrip = None if img.header is None else img.header.descrip.split(b"\x00", 1)[0][:79]
    return (
        data,
        np.asarray(img.affine, dtype=np.float64),
        descrip,
        extra_spacing(img.header, data.ndim),
    )


def result(data: np.ndarray, like: FloatInput) -> Image:
    """An output with the input's affine and header."""
    return Image(np.asfortranarray(data), like.affine, like.header)
