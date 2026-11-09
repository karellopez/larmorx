# SPDX-License-Identifier: Apache-2.0
"""sdcflows' masks (``sdcflows/utils/tools.py``'s ``brain_masker``; ``interfaces/brainmask.py``'s
``BrainExtraction``, ``BinaryDilation`` and ``Union``; ``utils/epimanip.py``'s ``epi_mask``),
ported from sdcflows 2.17.1 (the same in 2.15.1) with scikit-image's, SciPy's and numpy's
arithmetic under them.

``brain_masker``'s result depends on two library releases (``docs/findings/sdcflows-estimation.md``):
numpy's ``percentile`` of a Python-number ``q`` (numpy 2.2/2.3 compute it in ``float32``, numpy
2.5 in ``float64``: ``numpy_version``) and the order of SciPy's sparse rows under the random walker
(SciPy 1.15 or 1.16 onwards: ``scipy_version``). The defaults are the newest releases; fMRIPrep
25.2.6's image has numpy 2.2.6 and SciPy 1.15.2.
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image, as_image
from larmorx.io.nibabel_header import load_nibabel
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.sdcflows.approx import _fname_presuffix

__all__ = [
    "BrainMaskerResult",
    "EpiMaskResult",
    "binary_dilation",
    "brain_extraction",
    "brain_masker",
    "epi_mask",
    "union",
]

#: The newest numpy and SciPy releases (the default arithmetic).
NUMPY_NEWEST = "2.5.4"
SCIPY_NEWEST = "1.18.1"


@dataclass(frozen=True)
class BrainMaskerResult:
    """``brain_masker``'s three outputs: the input zeroed outside the mask (its header and type),
    the brain's probability (``float32`` on disk) and the mask (``uint8``); the paths when
    written."""

    brain: Image
    probseg: Image
    mask: Image
    out_brain: str | None
    out_probseg: str | None
    out_mask: str | None
    warnings: tuple[str, ...]


def _load(obj: Any, dtype: Any = None) -> Image:
    if isinstance(obj, (str, os.PathLike)):
        return load_nibabel(obj, dtype=dtype)
    img = as_image(obj)
    if dtype is not None:
        img = img.with_data(np.asanyarray(img.data, dtype=dtype))
    return img


def _with_dtype(img: Image, data: np.ndarray, dtype: Any) -> Image:
    """``hdr = img.header.copy(); hdr.set_data_dtype(dtype)``; the image of ``data`` (the type
    is given to :func:`larmorx.io.save_nibabel` when written)."""
    return Image(
        np.asarray(data, dtype=dtype) if data.dtype != dtype else data, img.affine, img.header
    )


def brain_masker(
    in_file: Any,
    out_file: str | os.PathLike[str] | None = None,
    padding: int = 5,
    *,
    write: bool = True,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> BrainMaskerResult:
    """sdcflows' ``brain_masker(in_file, out_file, padding)``: a quick brain mask by grey-level
    morphology (the inverted image's grey closing with ``ball(1)``, a ``ball(3)`` median, Otsu's
    threshold, binary opening, the largest component, a ``ball(5)`` closing) and scikit-image's
    random walker between the eroded mask and the padded image's border.

    Written as sdcflows writes them (``write``): the mask at ``out_file`` (default
    ``brain_mask.nii.gz`` in the working directory), the probability as ``<name>_probseg`` and the
    masked input as ``<name>_brainmasked`` (``_mask.`` dropped from the name).

    Corrected by default (``upstream_defects='reproduce'`` gives sdcflows' results): an image
    whose rough mask has no component (sdcflows takes the whole image) and an image whose eroded
    mask is empty (sdcflows takes the background's probability as the brain's) are errors."""
    _defects.check(upstream_defects)
    img = _load(in_file, np.float32)
    data = np.ascontiguousarray(img.data, dtype=np.float32)
    if data.ndim != 3:
        raise RuntimeError("structure array has incorrect shape")
    zooms = img.header.zooms[:3] if img.header is not None else img.voxel_sizes
    zooms = [float(np.float32(z)) for z in zooms]
    prob, mask, _th, _iters, warns = _core.sdcflows_brain_masker(
        data,
        zooms,
        int(padding),
        str(numpy_version),
        str(scipy_version),
        upstream_defects == "reproduce",
        int(n_threads),
    )
    import warnings

    for w in warns:
        warnings.warn(w, stacklevel=2)
    raw = _load(in_file) if isinstance(in_file, (str, os.PathLike)) else as_image(in_file)
    brain_data = np.array(np.asanyarray(raw.data), copy=True)
    brain_data[prob < 0.5] = 0
    mask_img = _with_dtype(img, mask, np.uint8)
    prob_img = _with_dtype(img, prob, np.float32)
    brain_img = Image(brain_data, raw.affine, raw.header)
    out_mask = out_probseg = out_brain = None
    if write:
        out_mask = str(Path(out_file or "brain_mask.nii.gz").absolute())
        base = out_mask.replace("_mask.", ".")
        out_probseg = re.sub(r"\.nii(\.gz)$", r"_probseg.nii\1", base)
        out_brain = re.sub(r"\.nii(\.gz)$", r"_brainmasked.nii\1", base)
        save_nibabel(mask_img, out_mask, dtype=np.uint8, n_threads=n_threads)
        save_nibabel(prob_img, out_probseg, dtype=np.float32, n_threads=n_threads)
        save_nibabel(brain_img, out_brain, n_threads=n_threads)
    return BrainMaskerResult(
        brain_img, prob_img, mask_img, out_brain, out_probseg, out_mask, tuple(warns)
    )


def brain_extraction(
    in_file: str | os.PathLike[str],
    *,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> BrainMaskerResult:
    """sdcflows' ``BrainExtraction``: :func:`brain_masker` with the mask named
    ``<in_file>_mask`` in ``out_dir`` (the working directory by default), so the outputs are
    ``<name>_mask``, ``<name>_probseg`` and ``<name>_brainmasked``."""
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
    out = _fname_presuffix(os.fspath(in_file), "_mask", out_dir or os.getcwd())
    return brain_masker(
        in_file,
        out,
        numpy_version=numpy_version,
        scipy_version=scipy_version,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )


def binary_dilation(
    in_file: Any,
    radius: float = 3,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    n_threads: int = 1,
) -> tuple[Image, str | None]:
    """sdcflows' ``BinaryDilation`` (``_dilate``): ``binary_dilation(dataobj > 0, ball(radius))``
    as ``uint8``, written as ``<name>_dil`` in ``out_dir`` when the input is a file (and
    ``out_dir`` is given, or the working directory)."""
    from larmorx import ndimage, skimage

    img = _load(in_file)
    dil = ndimage.binary_dilation(
        np.asanyarray(img.data) > 0, skimage.ball(radius), n_threads=n_threads
    )
    out = _with_dtype(img, dil.astype(np.uint8), np.uint8)
    path = None
    if isinstance(in_file, (str, os.PathLike)):
        if out_dir is not None:
            os.makedirs(out_dir, exist_ok=True)
        path = _fname_presuffix(os.fspath(in_file), "_dil", out_dir or os.getcwd())
        save_nibabel(out, path, dtype=np.uint8, n_threads=n_threads)
    return out, path


def union(
    in1: Any,
    in2: Any,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    n_threads: int = 1,
) -> tuple[Image, str | None]:
    """sdcflows' ``Union`` (``_union``): ``(dataobj1 + dataobj2) > 0`` (numpy's sum in the
    stored types: unsigned integers wrap) as ``uint8`` with the first image's header, written as
    ``<name1>_union``."""
    a = _load(in1)
    b = _load(in2)
    data = (np.asanyarray(a.data) + np.asanyarray(b.data)) > 0
    out = _with_dtype(a, data.astype(np.uint8), np.uint8)
    path = None
    if isinstance(in1, (str, os.PathLike)):
        if out_dir is not None:
            os.makedirs(out_dir, exist_ok=True)
        path = _fname_presuffix(os.fspath(in1), "_union", out_dir or os.getcwd())
        save_nibabel(out, path, dtype=np.uint8, n_threads=n_threads)
    return out, path


@dataclass(frozen=True)
class EpiMaskResult:
    """``epi_mask``'s mask (``uint8``, the input's header and affine), the voxel its object
    marker was put at (the rounded centre of mass) and the path when written."""

    mask: Image
    centre: tuple[int, ...]
    out_file: str | None


def _x86_int64(v: float) -> int:
    """numpy's cast of a float64 to int64 on x86-64 (``cvttsd2si``): NaN, infinities and values
    out of range give ``-2**63``."""
    if not np.isfinite(v) or v >= 2.0**63 or v < -(2.0**63):
        return -(2**63)
    return int(v)


def _uint16_levels(wshed: np.ndarray, upstream_defects: str) -> np.ndarray:
    """``np.round(1e3 * wshed / wshed.max()).astype(np.uint16)`` in float32. A flat residual
    (``0 / 0``) or one with a NaN or an infinity gives NaN (or infinity, where ``1e3 * wshed``
    overflows), whose conversion is undefined (numpy on x86-64: 0). SDC-28: clamped to the
    type's range (NaN to 0) with a warning; ``reproduce``: x86-64's 0."""
    levels = np.round(1e3 * wshed / wshed.max())
    bad = ~np.isfinite(levels)
    if bad.any():
        n = int(bad.sum())
        if upstream_defects == "reproduce":
            levels = np.where(bad, np.float32(0), levels)
            detail = f"{n} levels NaN or infinite, whose uint16 conversion is undefined: x86-64's 0"
        else:
            levels = np.where(np.isnan(levels), np.float32(0), levels)
            levels = np.clip(levels, np.float32(0), np.float32(65535))
            detail = (
                f"{n} levels NaN or infinite, whose uint16 conversion is undefined: clamped to "
                "the range (NaN to 0)"
            )
        _defects.warn([("SDC-28", "sdcflows epi_mask", upstream_defects, detail)], stacklevel=3)
    return levels.astype(np.uint16)


def epi_mask(
    in_file: Any,
    out_file: str | os.PathLike[str] | None = None,
    *,
    write: bool = True,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> EpiMaskResult:
    """sdcflows' ``epi_mask(in_file, out_file)`` (``utils/epimanip.py``; no workflow calls it):
    a quick mask of an EPI by grey-level morphology and a watershed. The image (float32) is
    opened with the non-flat ``ball(3)`` structure and closed with ``ball(2)``; the closed image,
    less its 30th percentile, is clipped to ``[0, p90]`` of its positive values; the watershed of
    the opened image less the closed one's ``ball(5)`` grey erosion (scaled to ``0..1000`` in
    ``uint16``) grows the object marker at the clipped image's rounded centre of mass against a
    background marker at voxel ``[0, 0, -1]``; the object, dilated with ``ball(2)``, is the mask
    (``uint8``, the input's header), written at ``out_file`` (default ``mask.nii.gz`` in the
    working directory) when ``write``.

    numpy's release changes the percentiles (numpy 2.2 and 2.3 compute them in float32, 2.5 in
    float64) and the centre of mass's sums (pairwise from 2.3, in chunks of 8192 before):
    ``numpy_version``, the newest release by default. Corrected by default
    (``upstream_defects="reproduce"`` gives sdcflows' mask): SDC-28, undefined ``uint16``
    conversions of a flat or non-finite residual (clamped, NaN to 0, with a warning), and the
    defects of SciPy's ``watershed_ift`` (:func:`larmorx.ndimage.watershed_ift`: neighbours
    across the image's edges, a queue that keeps an element alone in it). An image that is not
    3-D raises SciPy's ``RuntimeError``; one with no positive value left after its 30th
    percentile (a flat image) numpy's ``IndexError``, as sdcflows."""
    from larmorx import ndimage, skimage

    _defects.check(upstream_defects)
    img = _load(in_file, np.float32)
    data = np.asanyarray(img.data)
    # First open to blur out the skull around the brain
    opened = ndimage.grey_opening(data, structure=skimage.ball(3), n_threads=n_threads)
    # Second, close large vessels and the ventricles
    closed = ndimage.grey_closing(opened, structure=skimage.ball(2), n_threads=n_threads)
    # Window filter on percentile 30
    closed -= np.float32(
        _core.numpy_percentile_weak(np.ascontiguousarray(closed), 30.0, numpy_version)
    )
    # Window filter on percentile 90 of data
    maxnorm = np.float32(
        _core.numpy_percentile_weak(np.ascontiguousarray(closed[closed > 0]), 90.0, numpy_version)
    )
    closed = _core.numpy_clip_f32(np.ascontiguousarray(closed), 0.0, float(maxnorm))
    # Calculate index of center of masses
    com = np.round(ndimage.center_of_mass(closed, numpy_version=numpy_version))
    cm = tuple(_x86_int64(float(c)) for c in com)
    # Erode the picture of the brain by a lot
    eroded = ndimage.grey_erosion(closed, structure=skimage.ball(5), n_threads=n_threads)
    # Calculate the residual
    wshed = opened - eroded
    wshed -= wshed.min()
    wshed = _uint16_levels(wshed, upstream_defects)
    markers = np.zeros_like(wshed, dtype=int)
    markers[cm] = 2
    markers[0, 0, -1] = -1
    # Run watershed
    labels = ndimage.watershed_ift(wshed, markers, upstream_defects=upstream_defects)
    mask = ndimage.binary_dilation(labels == 2, skimage.ball(2), n_threads=n_threads)
    mask_img = _with_dtype(img, mask.astype(np.uint8), np.uint8)
    path = None
    if write:
        path = str(Path(out_file if out_file is not None else "mask.nii.gz").absolute())
        save_nibabel(mask_img, path, dtype=np.uint8, n_threads=n_threads)
    return EpiMaskResult(mask_img, tuple(int(c) for c in cm), path)
