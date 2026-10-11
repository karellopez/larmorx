# SPDX-License-Identifier: Apache-2.0
"""``3dTshift``: slice-timing correction (clean-room re-implementation of AFNI 25.2.09, or its
bit-exact replica from ``larmorx-gpl``, run as a separate program)."""

from __future__ import annotations

import math
import os
import tempfile
import warnings
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _core, _replica
from larmorx._replica import ImplementationName
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

#: The 3dTshift options of each method and restore mode (for the replica's command line).
_METHOD_OPTIONS = {m: "-Fourier" if m == "fourier" else f"-{m}" for m in METHODS}
_RESTORE_OPTIONS = {"trend": [], "none": ["-rlt"], "intercept": ["-rlt+"]}


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
    implementation: ImplementationName = "auto",
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
    implementation
        ``"auto"`` (default): the bit-exact replica when the ``larmorx-gpl`` package is
        installed (``pip install "larmorx[exact]"``), else the clean-room original.
        ``"replica"`` or ``"original"`` force one. The replica runs as a separate program on
        temporary NIfTI files (``docs/licensing.md``). The two differ only in the last bits of
        some values (``docs/validation/afni-tshift.md``).

    Returns
    -------
    Image
        The shifted image with the header ``3dTshift`` writes (``pixdim[4]`` = TR,
        ``toffset`` = ``tzero``, no slice timing). Data are in AFNI's storage type; when
        the output has a brick factor (``scl_slope``), the values are scaled to float64 as
        :func:`larmorx.load` would return them. :attr:`~larmorx.Image.implementation` says
        which implementation ran, with its package and version.

    Raises
    ------
    ValueError
        Invalid arguments. The checks that need the image (the number of slice times, their
        range, ``slice``, ``ignore``) are made by the implementation that runs: the replica
        reports them as a :class:`~larmorx.ReplicaError`.
    larmorx.ReplicaNotFoundError
        ``implementation="replica"`` without ``larmorx-gpl`` installed.
    larmorx.ReplicaError
        The replica was found but failed. larmorx does not fall back to the original then.
    """
    _check(slice_times, tr, tzero, slice, ignore, method, restore, detrend)
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
    selected = _replica.select("afni", "3dTshift", implementation)
    if selected is not None:
        return _tshift_replica(selected, image, times, options)
    if isinstance(image, (str, os.PathLike)):
        data, fields, messages = _core.afni_tshift_file(os.fspath(image), times, **options)
        header = NiftiHeader.from_dict(fields)
        data = _scaled(data, header)
    else:
        array, fields_in = _in_memory(image, tr)
        data, fields, messages = _core.afni_tshift_array(array, fields_in, times, **options)
        header = NiftiHeader.from_dict(fields)
    for message in messages:
        warnings.warn(message, stacklevel=2)
    return Image(data, header.affine, header, implementation=_replica.ORIGINAL)


def _check(
    slice_times: Any,
    tr: float | None,
    tzero: float | None,
    slice: int | None,
    ignore: int,
    method: str,
    restore: str,
    detrend: bool,
) -> None:
    """The checks that do not need the image, made for either implementation."""
    if method not in METHODS:
        raise ValueError(f"unknown method {method!r}; use one of {', '.join(METHODS)}")
    if restore not in _RESTORE_OPTIONS:
        raise ValueError(f"restore must be 'trend', 'none' or 'intercept', not {restore!r}")
    if not detrend and restore != "trend":
        raise ValueError("detrend=False restores the mean: restore must be 'trend'")
    if isinstance(slice_times, str):
        if slice_times not in PATTERNS[:-2] and slice_times.lower() not in ("zero", "simult"):
            raise ValueError(
                f"unknown slice pattern {slice_times!r}; use one of {', '.join(PATTERNS)}"
            )
    elif not all(math.isfinite(float(t)) and float(t) >= 0 for t in slice_times):
        raise ValueError("slice times must be finite and non-negative")
    if tr is not None and not (math.isfinite(tr) and tr > 0):
        raise ValueError(f"the TR must be positive, not {tr}")
    if tzero is not None and not (math.isfinite(tzero) and tzero >= 0):
        raise ValueError(f"tzero must be a non-negative number, not {tzero}")
    if slice is not None and int(slice) < 0:
        raise ValueError(f"slice must be non-negative, not {slice}")
    if int(ignore) < 0:
        raise ValueError(f"ignore must be non-negative, not {ignore}")


def _in_memory(image: Any, tr: float | None) -> tuple[np.ndarray, dict[str, Any]]:
    """An in-memory image as 3dTshift reads it: a 4D uint8, int16 or float32 array (other
    types become float32), and the header fields it would be written with."""
    img = as_image(image)
    array = np.asarray(img.data)
    if array.ndim != 4:
        raise ValueError(f"tshift needs a 4D (x, y, z, t) image, not {array.ndim}D")
    if array.dtype not in (np.uint8, np.int16, np.float32) or not array.dtype.isnative:
        array = array.astype(np.float32)
    if tr is None and img.header is None:
        raise ValueError("tr is required for an image without a NIfTI header")
    template = None if img.header is None else img.header.to_dict()
    fields = _core.nifti_header_for_image(
        list(array.shape), array.dtype.name, img.affine, template, None
    )
    return array, fields


def _tshift_replica(
    selected: _replica.Selected,
    image: Any,
    times: str | list[float],
    options: dict[str, Any],
) -> Image:
    """``larmorx-gpl afni 3dTshift`` on files in a temporary directory, with the options
    ``lx.afni.tshift`` has (they mean what the same options mean on the command line)."""
    implementation = selected.implementation()
    n_threads = options["n_threads"]
    with tempfile.TemporaryDirectory(prefix="larmorx-tshift-", ignore_cleanup_errors=True) as tmp:
        if isinstance(image, (str, os.PathLike)):
            # Absolute, so that it is never read as an option and does not depend on `cwd`.
            source = os.path.abspath(os.fspath(image))
        else:
            array, fields = _in_memory(image, options["tr"])
            # The data as they are: no scaling (the array path ignores the header's scaling).
            fields.update(scl_slope=1.0, scl_inter=0.0)
            source = os.path.join(tmp, "input.nii")
            _core.nifti_write(source, array, fields, 0, n_threads)
        args = _replica_args(times, options)
        # Relative names in `tmp`: AFNI refuses a -prefix with blanks or non-ASCII bytes, which
        # temporary directories can have (Windows user names).
        if not isinstance(times, str):
            Path(tmp, "slice_times.1D").write_text(
                "\t".join(repr(t) for t in times) + "\n", encoding="utf-8"
            )
        process = _replica.run(
            selected, [*args, "-prefix", "out.nii", source], cwd=tmp, n_threads=n_threads
        )
        out = os.path.join(tmp, "out.nii")
        data = _core.nifti_read(out, "raw", n_threads)[0]
        header = NiftiHeader.from_dict(_core.nifti_read_header(out, False))
    for line in process.stderr.splitlines():
        if line.startswith("*+ WARNING: "):
            warnings.warn(line.removeprefix("*+ WARNING: "), stacklevel=3)
    return Image(_scaled(data, header), header.affine, header, implementation=implementation)


def _replica_args(times: str | list[float], options: dict[str, Any]) -> list[str]:
    """3dTshift's options for :func:`tshift`'s arguments (before ``-prefix`` and the input)."""
    method = _METHOD_OPTIONS[options["method"]]
    if options["detrend"]:
        args = [method, *_RESTORE_OPTIONS[options["restore"]]]
    elif options["method"] == "fourier":
        # 3dTshift refuses -no_detrend while the method is Fourier, but not Fourier after it.
        args = ["-linear", "-no_detrend", method]
    else:
        args = [method, "-no_detrend"]
    args += ["-ignore", str(options["ignore"])]
    if options["tr"] is not None:
        args += ["-TR", f"{options['tr']!r}s"]
    if options["tzero"] is not None:
        args += ["-tzero", repr(options["tzero"])]
    if options["slice"] is not None:
        args += ["-slice", str(int(options["slice"]))]
    pattern = times if isinstance(times, str) else "@slice_times.1D"
    return [*args, "-tpattern", pattern]


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
