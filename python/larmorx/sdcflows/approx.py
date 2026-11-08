# SPDX-License-Identifier: Apache-2.0
"""sdcflows' ``BSplineApprox``: the B-spline approximation of a field map.

Ported from sdcflows 2.17.1 (``sdcflows/interfaces/bspline.py``, Apache-2.0, the NiPreps
developers), bit for bit with the libraries it calls: the ridge fit of scikit-learn (1.9.1 and
1.6.1) with SciPy's conjugate gradients, numpy's median and mean, SciPy's mode
(``crates/larmorx-sdcflows/src/approx.rs``), nitransforms' and SciPy's resampling of a field map
finer than ``zooms_min`` (:func:`~larmorx.sdcflows.resample_to_zooms`), and the files and
headers nibabel writes.
"""

from __future__ import annotations

import logging
import os
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image
from larmorx.io.nifti import NiftiHeader
from larmorx.sdcflows.bspline import DEFAULT_HF_ZOOMS_MM
from larmorx.sdcflows.tools import _as_loaded, _header, resample_to_zooms

__all__ = ["BSplineApproxResult", "bspline_approx"]

#: The logger of sdcflows' messages (sdcflows writes them to ``nipype.interface``).
LOGGER = logging.getLogger("larmorx.sdcflows")

_LEVELS = {"info": logging.INFO, "debug": logging.DEBUG}


@dataclass(frozen=True)
class BSplineApproxResult:
    """What ``BSplineApprox`` computes and writes.

    - ``field``: the smooth field on the field map's grid, 0 outside the mask (``out_field``).
    - ``coefficients``: the B-spline coefficients of every control grid (``out_coeff``), each
      an image on its control grid.
    - ``error``: ``(data - field) * mask`` with the input's header (``out_error``).
    - ``extrapolated``: the field with its extrapolation outside the mask
      (``out_extrapolated``); the ``field`` itself when the mask holds every voxel; ``None``
      without ``extrapolate``.
    - ``intercept``: ``out_intercept`` (0.0: the fit has no intercept).
    - ``center``: the value subtracted before the fit (``recenter``; 0.0 without one).
    - ``n_inside``: voxels inside the mask on the grid of the fit; ``n_iter``: the conjugate-
      gradient iterations.
    - ``out_field``, ``out_coeff``, ``out_error``, ``out_extrapolated``: the files written
      (with ``out_dir``), sdcflows' names; ``None`` (empty) otherwise.
    """

    field: Image
    coefficients: tuple[Image, ...]
    error: Image
    extrapolated: Image | None
    intercept: float
    center: float
    n_inside: int
    n_iter: int
    out_field: Path | None = None
    out_coeff: tuple[Path, ...] = ()
    out_error: Path | None = None
    out_extrapolated: Path | None = None


def _split_filename(fname: str) -> tuple[str, str, str]:
    """nipype's ``split_filename``: directory, stem and extension (``.nii.gz`` kept whole)."""
    pth = os.path.dirname(fname)
    base = os.path.basename(fname)
    for special in (".nii.gz", ".tar.gz", ".niml.dset"):
        if len(base) > len(special) and base[-len(special) :].lower() == special.lower():
            return pth, base[: -len(special)], base[-len(special) :]
    stem, ext = os.path.splitext(base)
    return pth, stem, ext


def _fname_presuffix(fname: str, suffix: str, newpath: str | os.PathLike[str]) -> str:
    """nipype's ``fname_presuffix(fname, suffix=suffix, newpath=newpath)``: the file name of
    ``fname`` with ``suffix`` before its extension, in ``newpath`` (made absolute)."""
    _, stem, ext = _split_filename(fname)
    return os.path.join(os.path.abspath(os.fspath(newpath)), stem + suffix + ext)


def _renamed(path: str, old: str, new: str) -> str:
    """sdcflows' ``out_name.replace(old, new)``, applied to the file name only (sdcflows applies
    it to the whole path, which matters only when a directory name contains ``old``)."""
    head, tail = os.path.split(path)
    return os.path.join(head, tail.replace(old, new))


def _spacings(bs_spacing: Any) -> list[tuple[float, float, float]]:
    """``bs_spacing`` as nipype's ``InputMultiObject`` of 3-tuples takes it: one tuple or a
    list of them."""
    items = list(bs_spacing)
    if len(items) == 3 and all(np.ndim(v) == 0 for v in items):
        items = [tuple(items)]
    out = []
    for s in items:
        t = tuple(float(v) for v in s)
        if len(t) != 3:
            raise ValueError("bs_spacing must hold 3-tuples of spacings (mm)")
        out.append(t)
    return out


def _recenter(recenter: Any) -> str:
    if isinstance(recenter, str):
        if recenter not in ("median", "mode", "mean"):
            raise ValueError(
                f"recenter must be 'median', 'mode', 'mean' or False, not {recenter!r}"
            )
        return recenter
    if recenter is not True and not isinstance(recenter, (str, bytes)) and recenter == False:  # noqa: E712
        return "none"
    raise ValueError(f"recenter must be 'median', 'mode', 'mean' or False, not {recenter!r}")


def _mask_of(mask_img: Image, data_shape: tuple[int, ...]) -> np.ndarray:
    """``np.asanyarray(masknii.dataobj) > 1e-4`` (nibabel's scaled values, numpy's comparison
    rules), checked against the field map's grid as ``data[mask]`` checks it."""
    mask = np.asanyarray(mask_img.data) > 1e-4
    if mask.shape != tuple(data_shape):
        np.zeros(data_shape, dtype=np.float32)[mask]  # numpy's IndexError
        raise IndexError(
            f"the mask's shape {mask.shape} does not match the field map's {tuple(data_shape)}"
        )
    return mask


def _flat_c(a: np.ndarray, dtype: Any) -> np.ndarray:
    return np.ascontiguousarray(np.asarray(a, dtype=dtype).reshape(-1))


def _written_header(
    template: NiftiHeader, shape: tuple[int, ...], dtype: str, affine: Any, **fields: Any
) -> NiftiHeader:
    """The header nibabel writes for ``Nifti1Image(data, affine, template)`` with ``dtype``
    stored, after ``fields`` were set on it."""
    d = template.to_dict()
    d.update(fields)
    out = _core.nifti_header_for_image(
        list(shape), dtype, np.ascontiguousarray(affine, dtype=np.float64), d, None
    )
    return NiftiHeader.from_dict(out)


def bspline_approx(
    in_data: Any,
    in_mask: Any = None,
    bs_spacing: Sequence[Sequence[float]] = (DEFAULT_HF_ZOOMS_MM,),
    ridge_alpha: float = 1e-4,
    recenter: str | bool = "median",
    extrapolate: bool = True,
    zooms_min: float | Sequence[float] = 1.0,
    debug: bool = False,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    name: str | None = None,
    n_threads: int = 1,
) -> BSplineApproxResult:
    """sdcflows' ``BSplineApprox``: approximate a field map with tensor-product cubic
    B-splines, which smooths it and extrapolates it beyond the brain mask.

    Parameters
    ----------
    in_data
        The field map (Hz): a NIfTI path, an :class:`~larmorx.Image`, a nibabel image or an
        ``(array, affine)`` pair, 3D. A path is read as nibabel reads it
        (``get_fdata(dtype="float32")``).
    in_mask
        The brain mask on the same grid (voxels whose value, scaled by the header, is above
        1e-4); every voxel without one.
    bs_spacing
        The control-point spacings (mm), one 3-tuple per control grid; sdcflows' default is
        the high-frequency grid ``(16, 16, 10)``, the one fMRIPrep uses.
    ridge_alpha
        The ridge regularisation (scikit-learn's ``Ridge(alpha)``, no intercept).
    recenter
        Subtract the ``"median"`` (default), ``"mode"`` or ``"mean"`` of the field inside the
        mask before the fit, or nothing (``False``).
    extrapolate
        Also evaluate the fit outside the mask (``extrapolated``).
    zooms_min
        The smallest voxel size the fit runs at (a number or a 3-tuple, mm; 0 never resizes): a
        finer field map (and its mask) is resampled to ``max(zooms, zooms_min)`` first, the
        coefficients fitted there and evaluated on the original grid.
    debug
        Accepted as sdcflows accepts it; sdcflows' ``BSplineApprox`` does not use it.
    out_dir
        Write sdcflows' files there: ``<name>_field``, ``<name>_coeff000`` (one per grid),
        ``<name>_error`` and ``<name>_extra`` (the field's file when the mask holds every
        voxel), with ``in_data``'s extension.
    name
        The file name the outputs are named after when ``in_data`` is not a path.
    n_threads
        Worker threads (0 = all logical CPUs); the result does not depend on it.

    The log records sdcflows writes (``nipype.interface``) go to the ``larmorx.sdcflows``
    logger, with sdcflows' texts. A fit whose largest coefficient reaches 1e4 raises
    sdcflows' ``RuntimeError``.
    """
    del debug  # sdcflows' interface declares it and never reads it
    is_path = isinstance(in_data, (str, os.PathLike))
    in_name = os.fspath(in_data) if is_path else (name or "fieldmap.nii.gz")
    fmap = _as_loaded(in_data)
    mask_img = None if in_mask is None else _as_loaded(in_mask)
    hdr = _header(fmap)
    zooms = tuple(np.float32(z) for z in hdr.zooms)
    if fmap.ndim < 3:
        raise ValueError("the field map must have at least 3 dimensions")
    shape3 = tuple(int(n) for n in fmap.shape[:3])
    affine = np.ascontiguousarray(fmap.affine, dtype=np.float64)
    spacings = _spacings(bs_spacing)
    rec = _recenter(recenter)
    # The control grids come first (from the original grid); a header with four zooms fails
    # here, as in sdcflows.
    for sp in spacings:
        _core.sdcflows_bspline_grid(shape3, affine, [float(z) for z in zooms], sp)

    need_resize = bool(np.any(np.array(zooms) < zooms_min))
    resampled = None
    if need_resize:
        target_zooms = np.maximum(zooms, zooms_min)
        LOGGER.info(
            "Resampling image with resolution exceeding 'zooms_min' "
            f"({'x'.join(str(s) for s in zooms)} → "
            f"{'x'.join(str(s) for s in target_zooms)})."
        )
        rf = resample_to_zooms(fmap, target_zooms, n_threads=n_threads)
        rm = (
            None
            if mask_img is None
            else resample_to_zooms(mask_img, target_zooms, n_threads=n_threads)
        )
        rdata = np.asarray(rf.data, dtype=np.float32)
        rmask = None if rm is None else _mask_of(rm, rdata.shape)
        resampled = (
            _flat_c(rdata, np.float32),
            tuple(int(n) for n in rf.shape[:3]),
            np.ascontiguousarray(rf.affine, dtype=np.float64),
            None if rmask is None else _flat_c(rmask, np.bool_),
        )
    data = np.asarray(fmap.data, dtype=np.float32)
    mask = None if mask_img is None else _mask_of(mask_img, data.shape)
    try:
        out = _core.sdcflows_bspline_approx(
            _flat_c(data, np.float32),
            shape3,
            affine,
            [float(z) for z in zooms],
            spacings,
            None if mask is None else _flat_c(mask, np.bool_),
            float(ridge_alpha),
            rec,
            bool(extrapolate),
            in_name,
            resampled,
            int(n_threads),
        )
    except (RuntimeError, ValueError) as err:
        # the records sdcflows writes before it raises
        for level, message in getattr(err, "log", ()):
            LOGGER.log(_LEVELS[level], message)
        raise
    for level, message in out["log"]:
        LOGGER.log(_LEVELS[level], message)

    shape = tuple(int(n) for n in fmap.shape)
    data_shape = data.shape
    # out_field: the input's header, float32, cal_max = max |field|, cal_min = -cal_max
    field = np.asarray(out["field"]).reshape(shape3).reshape(data_shape)
    cal = np.float32(np.abs(field).max())
    field_img = Image(
        field,
        affine,
        _written_header(hdr, shape, "float32", affine, cal_max=float(cal), cal_min=float(-cal)),
    )
    # out_coeff: each control grid's new image (nibabel's header for Nifti1Image(c, affine))
    version = hdr.version
    coefficients = []
    for (gshape, gaffine), coef in zip(out["grids"], out["coefficients"], strict=True):
        c = np.asarray(coef, dtype=np.float32).reshape(tuple(gshape))
        fields = _core.nifti_header_for_image(
            list(c.shape), "float32", np.ascontiguousarray(gaffine), None, version
        )
        coefficients.append(Image(c, np.asarray(gaffine), NiftiHeader.from_dict(fields)))
    # out_error: (data - field) * mask with the input's header as is (its stored type),
    # cal_min = 0, cal_max = max(error)
    error = np.asarray(out["error"]).reshape(shape3).reshape(data_shape)
    stored = hdr.data_dtype
    err_dtype = "float32"
    if stored is not None and stored.kind == "f":
        err_dtype = stored.name
    error = error.astype(err_dtype, copy=False)
    error_img = Image(
        error,
        affine,
        _written_header(hdr, shape, err_dtype, affine, cal_min=0.0, cal_max=float(np.max(error))),
    )
    extrapolated_img = None
    if out["extrapolated"] is not None:
        if mask is None or bool(np.all(mask)):
            extrapolated_img = field_img
        else:
            extra = np.asarray(out["extrapolated"]).reshape(shape3).reshape(data_shape)
            extrapolated_img = Image(extra, affine, _written_header(hdr, shape, "float32", affine))

    files: dict[str, Any] = {}
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
        out_name = _fname_presuffix(in_name, "_field", out_dir)
        field_img.save(out_name)
        coeff_paths = []
        for i, c in enumerate(coefficients):
            p = _renamed(out_name, "_field.", f"_coeff{i:03}.")
            c.save(p)
            coeff_paths.append(Path(p))
        err_path = _renamed(out_name, "_field.", "_error.")
        error_img.save(err_path)
        files = {
            "out_field": Path(out_name),
            "out_coeff": tuple(coeff_paths),
            "out_error": Path(err_path),
        }
        if extrapolated_img is not None:
            if extrapolated_img is field_img:
                files["out_extrapolated"] = Path(out_name)
            else:
                p = _renamed(out_name, "_field.", "_extra.")
                extrapolated_img.save(p)
                files["out_extrapolated"] = Path(p)
    return BSplineApproxResult(
        field=field_img,
        coefficients=tuple(coefficients),
        error=error_img,
        extrapolated=extrapolated_img,
        intercept=0.0,
        center=float(out["center"]),
        n_inside=int(out["n_inside"]),
        n_iter=int(out["n_iter"]),
        **files,
    )
