# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's masks for the confounds: the brain masks' union and the aCompCor tissue masks
(``fmriprep/workflows/bold/confounds.py``: ``_binary_union``, ``_get_zooms``;
``fmriprep/utils/confounds.py``: ``mask2vf``, ``acompcor_masks``; ``fmriprep/interfaces/
confounds.py``: ``aCompCorMasks``).

Ported from fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; the same in 25.2.6), with
nibabel's reading and writing (:func:`larmorx.io.load_nibabel`, :func:`larmorx.io.save_nibabel`),
SciPy's ``binary_dilation`` and ``gaussian_filter`` (:mod:`larmorx.ndimage`), scikit-image's
``ball`` (:func:`larmorx.skimage.ball`) and numpy's ``percentile`` (``numpy_version``'s), bit for
bit:

- :func:`binary_union`: ``_binary_union``: the voxels where either mask, cast to ``int64`` as
  ``np.asanyarray(dataobj, dtype=int)`` casts it (fractions truncated: a value in ``(0, 1)`` is
  outside), is positive, written as ``uint8`` with the first mask's header as
  ``mask_union.nii.gz``.
- :func:`get_zooms`: ``_get_zooms``: the first three voxel sizes of a file's header.
- :func:`mask2vf`: ``mask2vf``: a binary mask made a volume fraction: smoothed by a Gaussian of
  ``sigma = 0.5 · zooms / voxel sizes`` (``float32``), divided by the 99th percentile of its
  positive values and clipped to ``[0, 1]``.
- :func:`acompcor_masks`, :func:`acompcor_masks_node`: ``acompcor_masks`` and the
  ``aCompCorMasks`` interface: the CSF map (the input itself, or ``mask2vf``'s
  ``acompcor_csf.nii.gz`` on the segmentation path ``is_aseg=True``), the WM map and the CSF + WM
  map, both zero within ``ball(3)`` of the grey matter (``GM > 0.05``, or the segmentation's
  ``uint8`` values ``> 0``), written with the GM map's header as ``acompcor_wm.nii.gz`` and
  ``acompcor_wmcsf.nii.gz``.

Corrected by default (``upstream_defects="reproduce"`` gives fMRIPrep's files; each correction
names its ID in a :class:`larmorx.UpstreamDefectWarning`):

- PIPE-74: ``_binary_union`` casts the masks to ``int64`` with numpy's cast, undefined in C for
  NaN, infinities and values beyond ``int64``'s range (x86-64 gives the most negative ``int64``:
  a voxel of ``+inf`` or ``1e19`` is outside the union); larmorx clamps them (the rule for
  undefined conversions), so such a voxel is inside.
- PIPE-75: ``_binary_union`` broadcasts masks of different shapes (``(X, Y, Z) | (X, Y, 1)``
  takes the second mask's one slice for every slice) without a word; larmorx raises
  ``ValueError``.
- PIPE-76: ``acompcor_masks`` with ``is_aseg=True`` casts the GM map to ``uint8`` with numpy's
  cast: integers wrap (``-1`` is 255, inside; 256 is 0, outside) and floats out of
  ``(-2**31, 2**31)`` or NaN are undefined (x86-64: 0, outside; a float in range wraps as its
  ``int32``); larmorx clamps the values to ``[0, 255]`` first, so the GM is the voxels ``>= 1``.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.confounds.fmriprep import _existing, _out_dir

__all__ = [
    "AcompcorMasksResult",
    "acompcor_masks",
    "acompcor_masks_node",
    "binary_union",
    "get_zooms",
    "mask2vf",
]

#: The newest numpy release (its ``percentile`` for ``mask2vf``; the Gaussian's ``exp``).
NUMPY_NEWEST = "2.5.4"
#: ``2**63``: the first float beyond ``int64``'s range.
_TWO63 = 2.0**63


@dataclass(frozen=True)
class AcompcorMasksResult:
    """``aCompCorMasks``' output: ``out_masks``, the CSF, WM and combined maps' paths."""

    out_masks: list[str]


def _load(path: Any, dtype: Any = None, n_threads: int = 1):
    from larmorx.io.nibabel_header import load_nibabel

    p = os.fspath(path)
    if not os.path.exists(p):
        raise FileNotFoundError(f"No such file or no access: '{p}'")
    return load_nibabel(p, dtype=dtype, n_threads=n_threads)


def _save(
    data: np.ndarray,
    affine: Any,
    header: Any,
    path: str,
    dtype: Any,
    n_threads: int,
    version: int | None = None,
) -> None:
    """nibabel's ``img.to_filename(path)`` of ``Image(data, affine, header)`` in ``dtype`` (the
    header's when ``None``); ``version=1`` for ``nb.Nifti1Image`` of any header (a NIfTI-2 one
    converted), ``None`` for the header's own class (``img.__class__``)."""
    from larmorx.image import Image
    from larmorx.io.nibabel_writer import save_nibabel

    save_nibabel(
        Image(data, affine, header), path, dtype=dtype, version=version, n_threads=n_threads
    )


def _positive_int64(data: np.ndarray, mode: str, what: str) -> tuple[np.ndarray, int]:
    """``np.asanyarray(dataobj, dtype=int) > 0`` of ``dataobj``'s values ``data``: integer types
    other than ``uint64`` exactly; ``uint64``, floating and complex values through ``float64``
    (numpy's ``promote_types(dtype, int64)``; complex: the real part, numpy's
    ``ComplexWarning``), whose ``int64`` conversion truncates and is undefined from ``2**63``
    on: ``reproduce`` x86-64's (the most negative ``int64``: outside), the default clamped
    (inside; PIPE-74). ``(mask, number of voxels the two modes put differently)``."""
    if data.dtype.kind == "b" or (data.dtype.kind in "iu" and data.dtype != np.uint64):
        return data > 0, 0
    f = np.asarray(data).astype(np.float64)
    inside = f >= 1.0
    beyond = f >= _TWO63
    n = int(np.count_nonzero(beyond))
    if n == 0:
        return inside, 0
    if mode == "reproduce":
        inside &= ~beyond
        detail = (
            f"{n} voxels of {what} at or above 2**63 (an infinity included), whose int64 "
            "conversion is undefined: x86-64's most negative int64, outside the union"
        )
    else:
        detail = (
            f"{n} voxels of {what} at or above 2**63 (an infinity included), whose int64 "
            "conversion is undefined: clamped to int64's range, inside the union"
        )
    _defects.warn([("PIPE-74", "fMRIPrep _binary_union", mode, detail)], stacklevel=4)
    return inside, n


def binary_union(
    mask1: os.PathLike[str] | str,
    mask2: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """fMRIPrep's ``_binary_union(mask1, mask2)``: the voxels where either mask's values, cast to
    ``int64`` (truncated toward zero: fractions below 1 are outside; PIPE-74 for NaN, infinities
    and values beyond ``int64``), are positive, written as ``uint8`` with ``mask1``'s header and
    affine (``mask2``'s affine is not compared) to ``mask_union.nii.gz`` in ``out_dir``. Masks of
    different shapes raise ``ValueError`` (PIPE-75; ``reproduce``: numpy's broadcasting, or its
    ``ValueError``). Its path."""
    mode = _defects.check(upstream_defects)
    img = _load(mask1, n_threads=n_threads)
    one, _ = _positive_int64(np.asanyarray(img.data), mode, os.fspath(mask1))
    two, _ = _positive_int64(
        np.asanyarray(_load(mask2, n_threads=n_threads).data), mode, os.fspath(mask2)
    )
    if one.shape != two.shape:
        if mode == "fix":
            raise ValueError(
                f"the masks' shapes {one.shape} and {two.shape} differ (PIPE-75: fMRIPrep's "
                "_binary_union broadcasts them; upstream_defects='reproduce' does too)"
            )
        union = one | two  # numpy's broadcasting, or its ValueError
        _defects.warn(
            [
                (
                    "PIPE-75",
                    "fMRIPrep _binary_union",
                    mode,
                    f"the masks' shapes {one.shape} and {two.shape} were broadcast to "
                    f"{union.shape}",
                )
            ],
            stacklevel=2,
        )
    else:
        union = one | two
    out = os.path.join(_out_dir(out_dir), "mask_union.nii.gz")
    _save(union, img.affine, img.header, out, np.uint8, n_threads)
    return out


def get_zooms(in_file: os.PathLike[str] | str) -> tuple[float, ...]:
    """fMRIPrep's ``_get_zooms(in_file)``: ``tuple(nb.load(in_file).header.get_zooms()[:3])``,
    the header's first three voxel sizes after nibabel's fixes (``(1.0,)`` for an image without
    dimensions; fewer than three for one of fewer axes), as Python floats of their ``float32``
    values."""
    from larmorx.confounds.motion import _reference

    p = os.fspath(in_file)
    if not os.path.exists(p):
        raise FileNotFoundError(f"No such file or no access: '{p}'")
    _header, _shape, zooms, _affine = _reference(p)
    return tuple(float(z) for z in zooms[:3])


def _zooms3(zooms: Any, imgzooms: np.ndarray) -> np.ndarray:
    """``np.array(zooms, dtype=float)`` (the image's own when ``None``)."""
    return imgzooms if zooms is None else np.array(zooms, dtype=float)


def mask2vf(
    in_file: os.PathLike[str] | str,
    zooms: Sequence[float] | None = None,
    out_file: os.PathLike[str] | str | None = None,
    *,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> np.ndarray | str:
    """fMRIPrep's ``mask2vf(in_file, zooms, out_file)``: ``get_fdata(dtype=np.float32)`` of
    ``in_file`` smoothed by SciPy's ``gaussian_filter`` with ``sigma = 0.5 · zooms / voxel
    sizes`` (the image's own voxel sizes when ``zooms`` is ``None``: ``sigma`` 0.5), divided by
    the 99th percentile of its positive values (numpy ``numpy_version``'s ``percentile``: an
    image without a positive value raises numpy's ``IndexError``) and clipped to ``[0, 1]``.
    The ``float32`` array, or, with ``out_file``, the path it is written to (``float32``,
    ``in_file``'s header and affine)."""
    from larmorx import ndimage

    img = _load(in_file, dtype=np.float32, n_threads=n_threads)
    nd = int(img.header.dim[0])
    pix = np.asarray(img.header.pixdim[1 : nd + 1], dtype=np.float32) if nd else np.ones(1)
    imgzooms = np.array(pix[:3], dtype=float)
    sigma = 0.5 * (_zooms3(zooms, imgzooms) / imgzooms)
    data = ndimage.gaussian_filter(
        np.asarray(img.data), sigma=sigma, numpy_version=numpy_version, n_threads=n_threads
    )
    positive = np.ascontiguousarray(data[data > 0])
    if positive.size == 0:
        raise IndexError("index -1 is out of bounds for axis 0 with size 0")
    max_data = np.float32(_core.numpy_percentile_weak(positive, 99.0, numpy_version))
    data = _core.numpy_clip_f32(np.ascontiguousarray(data / max_data), 0.0, 1.0)
    if out_file is None:
        return data
    out = os.fspath(out_file)
    _save(data.astype(np.float32), img.affine, img.header, out, np.float32, n_threads, 1)
    return out


def _gm_uint8(data: np.ndarray, mode: str, what: str) -> np.ndarray:
    """``np.asanyarray(dataobj, np.uint8) > 0`` (PIPE-76). numpy casts integers modulo 256 and
    floats through x86-64's ``int32`` conversion (``cvttss2si``/``cvttsd2si``: truncated, its
    low byte; NaN and values out of ``(-2**31 - 1, 2**31)`` give 0); the default clamps the
    values to ``[0, 255]`` first (the voxels ``>= 1``)."""
    if data.dtype.kind == "b" or data.dtype == np.uint8:
        return data > 0
    if data.dtype.kind in "iu":
        fixed = data >= 1
        orig = (data.astype(np.uint8)) > 0  # numpy's modular conversion: defined
    else:
        f = np.asarray(data).astype(np.float64)
        fixed = f >= 1.0
        with np.errstate(invalid="ignore"):
            valid = (f > -(2.0**31) - 1.0) & (f < 2.0**31)
            t = np.where(valid, np.trunc(np.where(valid, f, 0.0)), 0.0).astype(np.int64)
        orig = (t & 0xFF) > 0
    n = int(np.count_nonzero(fixed != orig))
    if n:
        if mode == "reproduce":
            detail = (
                f"{n} voxels of {what} whose uint8 conversion wraps or is undefined (negative, "
                "above 255, NaN or infinite) were cast as numpy casts them on x86-64"
            )
        else:
            detail = (
                f"{n} voxels of {what} whose uint8 conversion wraps or is undefined (negative, "
                "above 255, NaN or infinite) were clamped to [0, 255] first"
            )
        _defects.warn([("PIPE-76", "fMRIPrep acompcor_masks", mode, detail)], stacklevel=4)
    return orig if mode == "reproduce" else fixed


def acompcor_masks(
    in_files: Sequence[os.PathLike[str] | str],
    is_aseg: bool = False,
    zooms: Sequence[float] | None = None,
    *,
    out_dir: os.PathLike[str] | str = ".",
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> list[str]:
    """fMRIPrep's ``acompcor_masks(in_files, is_aseg, zooms)``: from the GM, WM and CSF maps
    ``in_files`` (in that order; later ones ignored), the CSF map (``in_files[2]`` itself, or
    with ``is_aseg`` :func:`mask2vf`'s ``acompcor_csf.nii.gz``), the WM map (``get_fdata()``, or
    with ``is_aseg`` :func:`mask2vf`'s array) and their sum, both set to 0 within SciPy's
    ``binary_dilation`` by ``ball(3)`` of the grey matter (``GM > 0.05``, or with ``is_aseg``
    the GM map's ``uint8`` values ``> 0``: PIPE-76), the two written as nibabel's
    ``Nifti1Image(data, gm.affine, gm.header)`` (the GM map's stored type and nibabel's
    scaling) to ``acompcor_wm.nii.gz`` and ``acompcor_wmcsf.nii.gz`` in ``out_dir``.
    ``zooms`` (the BOLD series' voxel sizes) only set ``mask2vf``'s smoothing. The three
    paths."""
    from larmorx import ndimage, skimage

    mode = _defects.check(upstream_defects)
    files = list(in_files)
    csf_file = os.fspath(files[2])  # BIDS labeling (CSF=2; last of list)
    gm_path, wm_path = os.fspath(files[0]), os.fspath(files[1])
    d = _out_dir(out_dir)
    gm_vf = _load(gm_path, n_threads=n_threads)
    for p in (wm_path, csf_file):
        if not os.path.exists(p):
            raise FileNotFoundError(f"No such file or no access: '{p}'")
    if not is_aseg:
        gm_data = np.asarray(_load(gm_path, dtype=np.float64, n_threads=n_threads).data) > 0.05
        wm_data = np.array(_load(wm_path, dtype=np.float64, n_threads=n_threads).data)
        csf_data = np.asarray(_load(csf_file, dtype=np.float64, n_threads=n_threads).data)
    else:
        csf_file = mask2vf(
            csf_file,
            zooms=zooms,
            out_file=os.path.join(d, "acompcor_csf.nii.gz"),
            numpy_version=numpy_version,
            n_threads=n_threads,
        )
        csf_data = np.asarray(_load(csf_file, dtype=np.float64, n_threads=n_threads).data)
        wm_data = np.array(
            mask2vf(wm_path, zooms=zooms, numpy_version=numpy_version, n_threads=n_threads)
        )
        gm_data = _gm_uint8(np.asanyarray(gm_vf.data), mode, gm_path)

    # Dilate the GM mask
    gm_data = ndimage.binary_dilation(gm_data, structure=skimage.ball(3), n_threads=n_threads)

    wm_file = os.path.join(d, "acompcor_wm.nii.gz")
    combined_file = os.path.join(d, "acompcor_wmcsf.nii.gz")

    # Prepare WM mask
    wm_data[gm_data] = 0  # Make sure voxel does not contain GM
    _save(wm_data, gm_vf.affine, gm_vf.header, wm_file, None, n_threads, 1)

    # Prepare combined CSF+WM mask
    comb_data = csf_data + wm_data
    comb_data[gm_data] = 0  # Make sure voxel does not contain GM
    _save(comb_data, gm_vf.affine, gm_vf.header, combined_file, None, n_threads, 1)
    return [csf_file, wm_file, combined_file]


def acompcor_masks_node(
    in_vfs: Sequence[os.PathLike[str] | str] | os.PathLike[str] | str,
    bold_zooms: Sequence[float],
    *,
    is_aseg: bool = False,
    out_dir: os.PathLike[str] | str = ".",
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> AcompcorMasksResult:
    """fMRIPrep's ``aCompCorMasks`` interface: :func:`acompcor_masks` of ``in_vfs`` (existing
    files; one path is a list of one) with ``bold_zooms`` (three numbers, mandatory) and
    ``is_aseg``, as nipype's traits check them (``ValueError`` with traits' text)."""
    spec = "_aCompCorMasksInputSpec"
    files = [in_vfs] if isinstance(in_vfs, (str, os.PathLike)) else list(in_vfs)
    files = [_existing(spec, "in_vfs", f) for f in files]
    zooms = tuple(bold_zooms)
    if len(zooms) != 3 or not all(
        isinstance(z, (int, float, np.floating, np.integer)) and not isinstance(z, bool)
        for z in zooms
    ):
        raise ValueError(
            f"The 'bold_zooms' trait of an {spec} instance must be a tuple of the form: (a "
            f"float, a float, a float), but a value of {tuple(bold_zooms)!r} "
            f"{type(bold_zooms)!r} was specified."
        )
    masks = acompcor_masks(
        files,
        bool(is_aseg),
        tuple(float(z) for z in zooms),
        out_dir=out_dir,
        numpy_version=numpy_version,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    return AcompcorMasksResult(masks)
