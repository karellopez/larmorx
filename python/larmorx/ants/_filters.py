# SPDX-License-Identifier: Apache-2.0
"""Helpers shared by the wrappers of ANTs' image programs (ImageMath, ThresholdImage, ...).

ANTs programs read their images as ``float`` (``itk::Image<float, D>``), and masks often as
``int``. The wrappers do the same: a path is read as ITK 5.4.5 reads it (geometry and
scaling, as for :func:`larmorx.ants.apply_transforms`), and every input is converted the way
a C++ ``static_cast`` converts it. Results keep the input's affine and header.

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

__all__ = ["FloatInput", "float_input", "mask_input", "result"]


@dataclass(frozen=True)
class FloatInput:
    """An input image as ANTs holds it: float32 voxels, the RAS+ affine, the NIfTI header."""

    data: np.ndarray
    affine: np.ndarray
    header: Any = None


def _raw(image: Any, n_threads: int) -> tuple[np.ndarray, np.ndarray, Any]:
    """The voxels as ITK's reader produces them (for a path) or as given, with the affine
    and header."""
    if isinstance(image, (str, os.PathLike)):
        data, info = _core.itk_read_image(os.fspath(image), n_threads)
        from larmorx.io import read_header

        return data, np.asarray(info["ras_affine"]), read_header(image)
    img = as_image(image)
    data = np.asarray(img.data)
    if data.dtype.kind == "c":
        raise ValueError("complex images are not supported")
    return data, img.affine, img.header


def float_input(image: Any, n_threads: int = 1) -> FloatInput:
    """``image`` (a path, :class:`~larmorx.Image`, nibabel image or ``(array, affine)``) as a
    float32 image. A path is read as ITK reads it."""
    data, affine, header = _raw(image, n_threads)
    if data.dtype != np.float32:
        data = data.astype(np.float32)
    return FloatInput(data, affine, header)


def mask_input(mask: Any, shape: tuple[int, ...], n_threads: int = 1) -> np.ndarray:
    """A mask's voxels as float64 (the bindings convert them to the pixel type ANTs reads the
    mask as); its shape must match the image's. An array is taken as is."""
    data = mask if isinstance(mask, np.ndarray) else _raw(mask, n_threads)[0]
    data = np.asarray(data, dtype=np.float64)
    if data.shape != tuple(shape):
        raise ValueError(f"the mask shape {data.shape} differs from the image shape {tuple(shape)}")
    return data


def result(data: np.ndarray, like: FloatInput) -> Image:
    """An output with the input's affine and header."""
    return Image(np.asfortranarray(data), like.affine, like.header)
