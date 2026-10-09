# SPDX-License-Identifier: Apache-2.0
"""``3dTshift``: slice-timing correction (clean-room re-implementation of AFNI 25.2.09)."""

from __future__ import annotations

import math
import os
import warnings
from collections.abc import Sequence
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader

__all__ = ["METHODS", "PATTERNS", "fmriprep_slice_timing", "tshift"]

#: Interpolation methods (AFNI's ``-Fourier``, ``-linear``, ..., ``-wsinc9``).
METHODS = ("fourier", "linear", "cubic", "quintic", "heptic", "wsinc5", "wsinc9")

#: Named slice patterns (AFNI's ``-tpattern``); ``zero`` and ``simult`` ignore case.
PATTERNS = (
    "alt+z",
    "altplus",
    "alt+z2",
    "alt-z",
    "altminus",
    "alt-z2",
    "seq+z",
    "seqplus",
    "seq-z",
    "seqminus",
    "zero",
    "simult",
)

Method = Literal["fourier", "linear", "cubic", "quintic", "heptic", "wsinc5", "wsinc9"]
RestoreName = Literal["trend", "none", "intercept"]


def _scaled(data: np.ndarray, header: NiftiHeader) -> np.ndarray:
    """What ``lx.load`` returns for data stored with ``header``'s ``scl_slope``."""
    slope = header.scl_slope
    if slope == 0 or not math.isfinite(slope) or slope == 1:
        return data
    return data.astype(np.float64) * slope


def tshift(
    image: Any,
    *,
    slice_times: Sequence[float] | str,
    tr: float | None = None,
    tzero: float | None = None,
    slice: int | None = None,
    ignore: int = 0,
    method: Method = "fourier",
    restore: RestoreName = "trend",
    detrend: bool = True,
    n_threads: int = 1,
) -> Image:
    """Align every slice of a 4D image to one time origin (AFNI's ``3dTshift``).

    Each voxel's time series is detrended, shifted so that its slice refers to ``tzero``,
    clipped to its own range and given its trend back, exactly as AFNI 25.2.09 does.

    Parameters
    ----------
    image
        A path to a 4D NIfTI file, read with AFNI's rules (see ``docs/api/afni-tshift.md``),
        or an in-memory image (:class:`~larmorx.Image`, nibabel image, ``(array, affine)``).
        In-memory ``uint8``, ``int16`` and ``float32`` data keep their type; other types are
        converted to ``float32``.
    slice_times
        The acquisition time of each slice (third axis) within the TR, in seconds, like BIDS
        ``SliceTiming`` (reverse it first if ``SliceEncodingDirection`` ends in ``-``); or a
        pattern name from :data:`PATTERNS` (``-tpattern``). Values are rounded to float32
        and must lie in ``[0, TR]``.
    tr
        Repetition time in seconds (``-TR``). By default the header's, converted from ms or
        µs (1 s if it is not positive). Required for images without a NIfTI header.
    tzero
        The common time origin (``-tzero``). By default the mean slice time.
    slice
        Align to the time of this slice instead (``-slice``; wins over ``tzero``).
    ignore
        Leading time points left out of the detrending and shifting (``-ignore``).
    method
        One of :data:`METHODS` (default ``fourier``, as AFNI).
    restore
        What is added back after shifting: ``trend`` (default; then the series is clipped to
        its input range), ``none`` (``-rlt``) or ``intercept`` (``-rlt+``, the fitted
        trend's value at the first time point used).
    detrend
        ``False`` is ``-no_detrend``: the mean instead of the trend is removed and restored,
        with AFNI 25.2.09's behaviour reproduced (a warning says so): within each slice, voxels
        are taken in pairs in index order, and only the first of each pair has its mean
        removed; the second is shifted as raw values. Needs ``restore="trend"``.
    n_threads
        Worker threads (0 = all). The result does not depend on it.

    Returns
    -------
    Image
        The shifted image with the header ``3dTshift`` writes (``pixdim[4]`` = TR,
        ``toffset`` = ``tzero``, no slice timing). Data are in AFNI's storage type; when
        the output has a brick factor (``scl_slope``), the values are scaled to float64 as
        :func:`larmorx.load` would return them.
    """
    if method not in METHODS:
        raise ValueError(f"unknown method {method!r}; use one of {', '.join(METHODS)}")
    if not detrend:
        warnings.warn(
            "detrend=False reproduces AFNI 25.2.09's -no_detrend: only the first voxel of each "
            "pair (in index order within a slice) has its mean removed and restored; the second "
            "is shifted as raw values"
            + (
                "; AFNI also calls it dangerous with Fourier interpolation"
                if method == "fourier"
                else ""
            ),
            stacklevel=2,
        )
    times: str | list[float] = (
        slice_times if isinstance(slice_times, str) else [float(t) for t in slice_times]
    )
    options = dict(
        tr=None if tr is None else float(tr),
        tzero=None if tzero is None else float(tzero),
        slice=slice,
        ignore=int(ignore),
        method=method,
        restore=restore,
        detrend=bool(detrend),
        n_threads=int(n_threads),
    )
    if isinstance(image, (str, os.PathLike)):
        data, fields, messages = _core.afni_tshift_file(os.fspath(image), times, **options)
        header = NiftiHeader.from_dict(fields)
        data = _scaled(data, header)
    else:
        img = as_image(image)
        array = np.asarray(img.data)
        if array.ndim != 4:
            raise ValueError(f"tshift needs a 4D (x, y, z, t) image, not {array.ndim}D")
        if array.dtype not in (np.uint8, np.int16, np.float32) or not array.dtype.isnative:
            array = array.astype(np.float32)
        if tr is None and img.header is None:
            raise ValueError("tr is required for an image without a NIfTI header")
        template = None if img.header is None else img.header.to_dict()
        fields_in = _core.nifti_header_for_image(
            list(array.shape), array.dtype.name, img.affine, template, None
        )
        data, fields, messages = _core.afni_tshift_array(array, fields_in, times, **options)
        header = NiftiHeader.from_dict(fields)
    for message in messages:
        warnings.warn(message, stacklevel=2)
    return Image(data, header.affine, header)


def fmriprep_slice_timing(
    slice_timing: Sequence[float], slice_encoding_direction: str | None = None
) -> tuple[list[float], float]:
    """The slice times and ``tzero`` fMRIPrep gives ``3dTshift`` for a BIDS run.

    fMRIPrep writes ``SliceTiming`` (reversed when ``SliceEncodingDirection`` ends in ``-``)
    to the ``-tpattern`` file and uses the middle of the acquisition,
    ``round(min + 0.5 * (max - min), 3)``, as ``-tzero``. Use the result as
    ``tshift(img, slice_times=times, tzero=t0)``.
    """
    times = [float(t) for t in slice_timing]
    if slice_encoding_direction and slice_encoding_direction.endswith("-"):
        times = times[::-1]
    t0 = round(min(times) + 0.5 * (max(times) - min(times)), 3)
    return times, t0
