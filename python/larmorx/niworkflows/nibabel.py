# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``interfaces/nibabel.py`` (niworkflows 1.15.4): ``IntensityClip``.

numpy's ``percentile`` of a Python-number ``q`` differs between numpy 2.2/2.3 (``float32``
arithmetic) and 2.5 (``float64``): ``numpy_version`` picks the release (the default the newest;
fMRIPrep 25.2.6's image has numpy 2.2.6).
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image
from larmorx.io.nibabel_header import load_nibabel
from larmorx.io.nibabel_writer import save_nibabel

__all__ = ["IntensityClipResult", "intensity_clip"]

#: The newest numpy release (the default arithmetic of the percentiles).
NUMPY_NEWEST = "2.5.4"


@dataclass(frozen=True)
class IntensityClipResult:
    """The clipped image and, when written, its path (``clipped.nii.gz``)."""

    image: Image
    out_file: str | None


def _squeezed(in_file: Any) -> tuple[Image, np.ndarray]:
    """``nb.squeeze_image(nb.load(in_file))`` and its ``get_fdata(dtype='float32')``: trailing
    axes of length 1 after the third dropped (then the data is the scaled array, cast to
    ``float32``); otherwise the file's data read as ``float32``."""
    img = load_nibabel(in_file) if isinstance(in_file, (str, os.PathLike)) else as_image(in_file)
    shape = img.data.shape
    slen = len(shape)
    if slen >= 4:
        for b in shape[3:][::-1]:
            if b == 1:
                slen -= 1
            else:
                break
    if slen != len(shape):
        data = np.asanyarray(img.data).reshape(shape[:slen])
        return img.replace(data=data), np.asanyarray(data, dtype=np.float32)
    if isinstance(in_file, (str, os.PathLike)):
        f32 = load_nibabel(in_file, dtype=np.float32).data
    else:
        f32 = np.asanyarray(img.data, dtype=np.float32)
    return img, f32


def intensity_clip(
    in_file: Any,
    p_min: float = 35.0,
    p_max: float = 99.98,
    nonnegative: bool = True,
    dtype: str = "int16",
    invert: bool = False,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> IntensityClipResult:
    """niworkflows' ``IntensityClip``: the image clipped at the ``p_min`` and ``p_max``
    percentiles of its ``ball(3)`` median-filtered version (its positive values with
    ``nonnegative``), shifted to 0, divided by its maximum, inverted with ``invert``, and for
    ``dtype`` ``'int16'`` or ``'uint8'`` rounded from ``255 x``; ``cal_max`` set to the maximum.
    Written as ``clipped.nii.gz`` in ``out_dir`` when given."""
    from larmorx import ndimage, skimage

    if dtype not in ("int16", "float32", "uint8"):
        raise ValueError(f"dtype must be 'int16', 'float32' or 'uint8', not {dtype!r}")
    img, data = _squeezed(in_file)
    if data.ndim != 3:
        raise RuntimeError(f"<{in_file}> is not a 3D file.")
    data = np.array(data, dtype=np.float32, copy=True)
    denoised = ndimage.median_filter(data, footprint=skimage.ball(3), n_threads=n_threads)
    sel = denoised[denoised > 0] if nonnegative else denoised
    a_min = np.float32(_core.numpy_percentile_weak(sel, float(p_min), numpy_version))
    a_max = np.float32(_core.numpy_percentile_weak(sel, float(p_max), numpy_version))
    data = _core.numpy_clip_f32(np.ascontiguousarray(data), a_min, a_max)
    data -= data.min()
    data /= data.max()
    if invert:
        data = 1.0 - data
    if dtype in ("uint8", "int16"):
        data = np.round(255 * data).astype(dtype)
    out = Image(data, img.affine, img.header)
    path = None
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
        path = str((Path(out_dir) / "clipped.nii.gz").absolute())
        hdr = img.header
        if hdr is not None:
            out = out.replace(header=hdr.replace(cal_max=float(np.float32(data.max()))))
        save_nibabel(out, path, dtype=dtype, n_threads=n_threads)
    return IntensityClipResult(out, path)
