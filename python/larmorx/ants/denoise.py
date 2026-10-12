# SPDX-License-Identifier: Apache-2.0
"""Adaptive non-local means denoising as ANTs' ``DenoiseImage`` does it (ANTs v2.6.5 on the
``AdaptiveNonLocalMeansDenoisingImageFilter`` of ITK v5.4.5's remote module
AdaptiveDenoising)."""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, warn
from larmorx.ants._filters import memory_image
from larmorx.image import Image, as_image

__all__ = ["DenoiseResult", "denoise_image"]


@dataclass(frozen=True)
class DenoiseResult:
    """The denoised image and the noise removed from it (``image - denoised``, as ANTs
    computes it: the noise is estimated at the filter's resolution and resampled onto the
    image's grid, then subtracted)."""

    denoised: Image
    noise: Image


def _source(image: Any) -> tuple[Any, Any, int]:
    """What the binding takes (a path or an in-memory tuple), the NIfTI header, and the
    number of dimensions ITK reads."""
    if isinstance(image, (str, os.PathLike)):
        from larmorx.io import read_header

        header = read_header(image)
        return os.fspath(image), header, header.itk_geometry.ndim
    img = as_image(image)
    return memory_image(img), img.header, np.asarray(img.data).ndim


def _image(out: tuple, header: Any) -> Image:
    data, affine, _descrip, _spacing = out
    return Image(np.asfortranarray(data), affine, header)


def _radius(value: int | Sequence[int], name: str) -> list[int]:
    values = [int(value)] if isinstance(value, (int, np.integer)) else [int(v) for v in value]
    if not values or any(v < 0 for v in values):
        raise ValueError(f"{name} must be one non-negative integer or one per axis, not {value!r}")
    return values


def denoise_image(
    image: Any,
    noise_model: Literal["Rician", "Gaussian"] = "Rician",
    mask: Any = None,
    shrink_factor: int = 1,
    patch_radius: int | Sequence[int] = 1,
    search_radius: int | Sequence[int] = 2,
    *,
    dimension: int | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> DenoiseResult:
    """``DenoiseImage -d D -i image -n model [-x mask] -s shrink -p patch -r search
    -o [denoised,noise]``: adaptive non-local means (Manjón et al., JMRI 2010).

    ``image`` and ``mask`` are paths (read as ANTs reads them: the image as float, the mask as
    ``unsigned char``, in ``dimension`` dimensions, by default the image's) or in-memory images
    (a :class:`~larmorx.Image`, a nibabel image or ``(array, affine)``).

    - ``noise_model``: ``"Rician"`` (the default here, and what sMRIPrep uses: the squared
      values are averaged and a Rician bias removed) or ``"Gaussian"`` (the command line's
      default). Case does not matter.
    - ``mask``: only voxels where the mask is not 0 are denoised as patch centres. It must be
      on the grid the filter runs on: the image's, or with ``shrink_factor`` above 1 the
      subsampled image's (ITK rejects a full-resolution mask then, and so does larmorx). Its
      values are cast to ``unsigned char`` as C++ does (0.5 becomes 0).
    - ``shrink_factor``: the filter runs on the image subsampled by this factor along every
      axis (``ShrinkImageFilter``); the noise is then resampled linearly onto the image's grid.
    - ``patch_radius``, ``search_radius``: one value for every axis or one per axis.

    Returns the denoised image and the noise, with the input's affine and header. The result
    does not depend on ``n_threads``: it is the one ANTs computes with a single thread (ANTs'
    own result changes with its thread count). larmorx's ``expf`` and ``exp`` are correctly
    rounded; glibc's are not always, and where ANTs' run met such a value its result differs
    from larmorx's by a few float units (``docs/findings/ants-denoise.md``).

    At a high signal-to-noise ratio ITK's Rician correction factor overflows and the bias
    becomes 0 (ANTS-43); larmorx computes it without the overflow. Mask values outside
    ``unsigned char``'s range are clamped (ANTS-06; ANTs on x86-64 wraps them).
    ``upstream_defects="reproduce"`` gives ANTs' result.
    """
    check(upstream_defects)
    model = str(noise_model).lower()
    if model not in ("rician", "gaussian"):
        raise ValueError(f'noise_model must be "Rician" or "Gaussian", not {noise_model!r}')
    patch = _radius(patch_radius, "patch_radius")
    search = _radius(search_radius, "search_radius")
    source, header, ndim = _source(image)
    dim = int(dimension) if dimension is not None else ndim
    for name, values in (("patch_radius", patch), ("search_radius", search)):
        if len(values) not in (1, dim):
            raise ValueError(f"{name} needs one value or {dim}, not {len(values)}")
    mask_source = None if mask is None else _source(mask)[0]
    denoised, noise, notices = _core.ants_denoise_image(
        source,
        dim,
        mask_source,
        model == "rician",
        int(shrink_factor),
        patch,
        search,
        n_threads,
        upstream_defects,
    )
    warn(notices)
    if dim != ndim:
        # Read with another number of dimensions (a 4D file as 3D: its first volume).
        header = None
    return DenoiseResult(denoised=_image(denoised, header), noise=_image(noise, header))
