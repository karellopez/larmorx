# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``interfaces/nibabel.py`` (niworkflows 1.15.4, the same in 1.15.2):
``IntensityClip``, ``Binarize``, ``MergeSeries`` and ``ReorientImage`` (``reorient_file``,
``reorient_image``),
with nibabel's (5.4.2, MIT) ``axcodes2ornt``, ``aff2axcodes`` and ``ornt_transform``.

numpy's ``percentile`` of a Python-number ``q`` differs between numpy 2.2/2.3 (``float32``
arithmetic) and 2.5 (``float64``): ``numpy_version`` picks the release (the default the newest;
fMRIPrep 25.2.6's image has numpy 2.2.6).

Corrected by default (``upstream_defects='reproduce'`` gives niworkflows' behaviour): PIPE-33,
``IntensityClip`` of an image constant between its percentiles divides 0 by 0 (a NaN image,
converted to ``int16``/``uint8`` by an undefined cast): a ``ValueError`` (rule B).
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image, as_image
from larmorx.io.nibabel_header import load_nibabel
from larmorx.io.nibabel_writer import save_nibabel

__all__ = [
    "BinarizeResult",
    "IntensityClipResult",
    "aff2axcodes",
    "axcodes2ornt",
    "binarize",
    "intensity_clip",
    "merge_series",
    "ornt_transform",
    "reorient_file",
    "reorient_image",
]

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
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> IntensityClipResult:
    """niworkflows' ``IntensityClip``: the image clipped at the ``p_min`` and ``p_max``
    percentiles of its ``ball(3)`` median-filtered version (its positive values with
    ``nonnegative``), shifted to 0, divided by its maximum, inverted with ``invert``, and for
    ``dtype`` ``'int16'`` or ``'uint8'`` rounded from ``255 x``; ``cal_max`` set to the maximum.
    Written as ``clipped.nii.gz`` in ``out_dir`` when given.

    PIPE-33 (rule B; ``upstream_defects="reproduce"`` gives niworkflows' result): when the
    clipped image is constant (both percentiles equal, or a constant image), niworkflows divides
    0 by 0: a NaN image, which ``int16`` and ``uint8`` take by an undefined conversion (0 on
    x86-64; here x86-64's 0 and default NaN on every platform); by default a ``ValueError``."""
    from larmorx import ndimage, skimage

    _defects.check(upstream_defects)
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
    if data.max() == 0:  # PIPE-33: 0 / 0
        if upstream_defects == "fix":
            raise ValueError(
                f"IntensityClip: the image clipped at its percentiles {float(a_min)!r} and "
                f"{float(a_max)!r} is constant, and niworkflows divides 0 by 0 (a NaN image"
                f"{'' if dtype == 'float32' else ', converted to ' + dtype + ' as the CPU does'}"
                "; PIPE-33, upstream_defects='reproduce' gives it)"
            )
        _defects.warn(
            [
                (
                    "PIPE-33",
                    "niworkflows IntensityClip",
                    "reproduce",
                    "a constant clipped image divided by its maximum, 0: NaN (x86-64's "
                    f"default NaN{'' if dtype == 'float32' else ', converted to ' + dtype + ': 0'})",
                )
            ],
            stacklevel=2,
        )
        nan = np.array([0xFFC00000], dtype=np.uint32).view(np.float32)[0]
        data = np.full(data.shape, nan, dtype=np.float32)
        if dtype in ("uint8", "int16"):
            data = np.zeros(data.shape, dtype=dtype)
    else:
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


@dataclass(frozen=True)
class BinarizeResult:
    """``Binarize``'s files: the image with every value at or below the threshold set to 0
    (``out_file``: ``<in>_masked``) and the mask of the values above it (``out_mask``:
    ``<in>_mask``, ``uint8``)."""

    out_file: str
    out_mask: str


def binarize(
    in_file: str | os.PathLike[str],
    thresh_low: float,
    *,
    out_dir: str | os.PathLike[str] = ".",
    n_threads: int = 1,
) -> BinarizeResult:
    """niworkflows' ``Binarize``: ``get_fdata()`` of ``in_file`` (``float64``) above
    ``thresh_low`` (a NaN is not above), the rest set to 0, written with the input's header in
    its stored type as ``<in>_masked``; the mask written as ``uint8`` with the same header as
    ``<in>_mask`` (both in ``out_dir``, nibabel's writer)."""
    from larmorx import _nipype

    in_file = os.fspath(in_file)
    img = load_nibabel(in_file, n_threads=n_threads)
    os.makedirs(out_dir, exist_ok=True)
    out_file = _nipype.fname_presuffix(in_file, suffix="_masked", newpath=out_dir)
    out_mask = _nipype.fname_presuffix(in_file, suffix="_mask", newpath=out_dir)
    data = np.array(load_nibabel(in_file, dtype=np.float64, n_threads=n_threads).data, copy=True)
    mask = data > float(thresh_low)
    data[~mask] = 0.0
    save_nibabel(Image(data, img.affine, img.header), out_file, n_threads=n_threads)
    save_nibabel(
        Image(mask.astype("uint8"), img.affine, img.header),
        out_mask,
        dtype=np.uint8,
        n_threads=n_threads,
    )
    return BinarizeResult(out_file, out_mask)


def axcodes2ornt(axcodes: Sequence[str | None]) -> np.ndarray:
    """nibabel's ``axcodes2ornt(axcodes)`` with its default labels (L/R, P/A, I/S): one row
    ``(axis, flip)`` per code, ``NaN`` for ``None``; ``ValueError`` for another code."""
    labels = list(zip("LPI", "RAS", strict=True))
    allowed: list[str | None] = [None, "L", "R", "P", "A", "I", "S"]
    if not set(axcodes).issubset(allowed):
        raise ValueError(f"Not all axis codes {list(axcodes)} in label set {allowed}")
    ornt = np.ones((len(axcodes), 2), dtype=np.int8) * np.nan
    for code_idx, code in enumerate(axcodes):
        for label_idx, codes in enumerate(labels):
            if code is None:
                continue
            if code in codes:
                ornt[code_idx, :] = [label_idx, -1 if code == codes[0] else 1]
                break
    return ornt


def aff2axcodes(affine: Any) -> tuple[str | None, ...]:
    """nibabel's ``aff2axcodes(affine)``: the axis codes of ``io_orientation(affine)``."""
    from larmorx.transforms.resample import _io_orientation, _ornt2axcodes

    return tuple(_ornt2axcodes(_io_orientation(np.asarray(affine, dtype=np.float64))))


def ornt_transform(start: Any, end: Any) -> np.ndarray:
    """nibabel's ``ornt_transform(start_ornt, end_ornt)``: the orientation taking ``start`` to
    ``end`` (``ValueError`` for different shapes or an axis ``start`` lacks)."""
    start = np.asarray(start)
    end = np.asarray(end)
    if start.shape != end.shape:
        raise ValueError("The orientations must have the same shape")
    if start.shape[1] != 2:
        raise ValueError(f"Invalid shape for an orientation: {start.shape}")
    result = np.empty_like(start)
    for end_in_idx, (end_out_idx, end_flip) in enumerate(end):
        for start_in_idx, (start_out_idx, start_flip) in enumerate(start):
            if end_out_idx == start_out_idx:
                result[start_in_idx, :] = [end_in_idx, 1 if start_flip == end_flip else -1]
                break
        else:
            raise ValueError(f"Unable to find out axis {end_out_idx} in start_ornt")
    return result


def reorient_image(img: Image, target_ornt: Sequence[str | None]) -> Image:
    """niworkflows' ``reorient_image(img, target_ornt)``: ``as_reoriented`` from the image's
    axis codes to ``target_ornt`` (axis codes, e.g. ``"LAS"``)."""
    from larmorx.niworkflows.images import as_reoriented

    in_ornt = axcodes2ornt(aff2axcodes(img.affine))
    out_ornt = axcodes2ornt(target_ornt)
    return as_reoriented(img, ornt_transform(in_ornt, out_ornt))


def reorient_file(
    in_file: str | os.PathLike[str],
    *,
    target_file: str | os.PathLike[str] | None = None,
    target_ornt: str | None = None,
    newpath: str | os.PathLike[str] | None = None,
    n_threads: int = 1,
) -> str:
    """niworkflows' ``reorient_file`` (``ReorientImage``): ``in_file`` reoriented to the axis
    codes of ``target_file`` (or to ``target_ornt``) and written as ``reoriented.nii.gz`` in
    ``newpath`` (the working directory by default); ``TypeError`` without a target."""
    img = load_nibabel(in_file)
    if not target_file and not target_ornt:
        raise TypeError("No target orientation or file is specified.")
    if target_file:
        target_ornt = aff2axcodes(load_nibabel(target_file).affine)  # type: ignore[assignment]
    reoriented = reorient_image(img, target_ornt)  # type: ignore[arg-type]
    folder = Path(os.fspath(newpath or os.getcwd()))
    folder.mkdir(parents=True, exist_ok=True)
    out_file = str((folder / "reoriented.nii.gz").absolute())
    save_nibabel(reoriented, out_file, n_threads=n_threads)
    return out_file


def merge_series(
    in_files: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    allow_4D: bool = True,
    affine_tolerance: float | None = None,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    n_threads: int = 1,
) -> str | None:
    """niworkflows' ``MergeSeries``: 3D images (and, with ``allow_4D``, the volumes of 4D ones),
    each squeezed, merged into one 4D image (nibabel's ``concat_images``; with
    ``affine_tolerance``, the affines need only agree with the first within it, ``np.allclose``
    with that ``atol``, and the first's is kept) and written as ``<first>_merged``.
    ``ValueError`` for another number of dimensions, affines beyond the tolerance, or (without
    one) affines that are not identical. The path (``None`` without ``out_dir``)."""
    from larmorx.niworkflows.images import _concat, _squeezed, _workdir, four_to_three
    from larmorx.sdcflows.approx import _fname_presuffix

    files = [
        os.fspath(f) for f in (in_files if isinstance(in_files, (list, tuple)) else [in_files])
    ]
    nii_list: list[Image] = []
    aff0 = None
    for f in files:
        filenii = _squeezed(load_nibabel(f))
        if affine_tolerance:
            if aff0 is None:
                aff0 = filenii.affine
            elif not np.allclose(aff0, filenii.affine, atol=affine_tolerance):
                raise ValueError(
                    f"Difference in affines greater than allowed tolerance {affine_tolerance}"
                )
        ndim = np.asarray(filenii.data).ndim
        if ndim == 3:
            nii_list.append(filenii)
        elif allow_4D and ndim == 4:
            nii_list += four_to_three(filenii)
        else:
            raise ValueError(f"Input image has an incorrect number of dimensions ({ndim}).")
    img_4d = _concat(nii_list, check_affines=not bool(affine_tolerance))
    with _workdir(out_dir) as (cwd, keep):
        out_file = _fname_presuffix(files[0], "_merged", cwd)
        save_nibabel(img_4d, out_file, n_threads=n_threads)
        return out_file if keep else None
