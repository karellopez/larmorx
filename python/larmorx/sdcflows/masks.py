# SPDX-License-Identifier: Apache-2.0
"""sdcflows' masks (``sdcflows/utils/tools.py``'s ``brain_masker``; ``interfaces/brainmask.py``'s
``BrainExtraction``, ``BinaryDilation`` and ``Union``), ported from sdcflows 2.17.1 (the same in
2.15.1) with scikit-image's, SciPy's and numpy's arithmetic under them.

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
    "binary_dilation",
    "brain_extraction",
    "brain_masker",
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
