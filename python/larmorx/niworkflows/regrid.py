# SPDX-License-Identifier: Apache-2.0 AND MIT
"""niworkflows' ``RegridToZooms`` (``interfaces/images.py`` and ``interfaces/nibabel.py``) and
``utils/images.py``'s ``resample_by_spacing`` (niworkflows 1.15.4, the same in 1.15.2,
fMRIPrep 25.2.6's), with nibabel's (5.4.2, MIT) ``from_matvec``, ``get_zooms``, ``set_zooms``,
``set_qform`` and ``set_sform`` and SciPy's ``gaussian_filter`` and ``map_coordinates``.

``RegridToZooms`` resamples an image onto a grid of new voxel sizes covering the same field of
view: fMRIPrep's brain extraction (``res_tmpl``, ``res_target``: 4 mm, ``smooth=True``),
sdcflows' SyN estimation (``zooms_epi``) and its debug sampling reference (``regrid_anat``:
2 mm, ``smooth=True``). The arithmetic is numpy's, nibabel's and SciPy's, as niworkflows runs it:

- the voxel sizes are the header's ``pixdim`` (``float32``); the cardinal matrix ``card`` is a
  ``float32`` diagonal with the centred translation rounded to ``float32``; the extent and
  ``np.diag(zooms).dot(new_size)`` are ``dgemv`` products with one non-zero term each (exact
  per entry, from zero);
- ``np.linalg.inv(card)`` of the ``float32`` matrix is numpy's ``float64`` inverse (``dgesv``)
  cast to ``float32``: numpy computes linear algebra on single-precision input in double;
- the sampling points ``inv(card).dot(new_card.dot(grid))`` are ``dgemm`` products
  (:func:`larmorx.transforms._linalg.affine_points`); the values are SciPy's
  ``map_coordinates`` (order 3 by default, ``mode='constant'``, prefiltered), on the file's
  ``dataobj`` or, with ``smooth``, on ``gaussian_filter(get_fdata(), sigma)``; ``clip`` clips
  them to the input's range;
- the header is the input's with the new zooms, the qform and sform carried by
  ``new_affine.dot(inv(affine).dot(xform))`` (numpy's 4 x 4 products and inverse) with their
  codes (both set to ``new_affine`` with code 1 when neither is set), then nibabel's constructor
  (``Nifti1Image(resampled, new_affine, hdr)``) and writer (the header's stored type, scaled).

Upstream defect: PIPE-35 (``smooth=True``'s sigma ``max(0, (old / new - 1) / 2)`` voxels is
positive only when the grid gets finer, so a coarser grid, the anti-aliasing case, is never
smoothed: fMRIPrep's 1 mm images regridded to 4 mm are not smoothed at all; rule D, to be
measured with the anatomical brain extraction's port: both modes are niworkflows').
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx.image import Image
from larmorx.io.nibabel_header import _get_qform, header_for_image, load_nibabel, set_xform
from larmorx.io.nibabel_writer import HeaderDataError, save_nibabel
from larmorx.io.nifti import NiftiHeader

__all__ = ["RegridToZoomsResult", "regrid_to_zooms", "resample_by_spacing"]

SCIPY_NEWEST = "1.18.1"
NUMPY_NEWEST = "2.5.4"


@dataclass(frozen=True)
class RegridToZoomsResult:
    """``RegridToZooms``' output: the regridded image (``<in_file>_regrid`` in ``out_dir``)."""

    out_file: str


def _from_matvec(matrix: np.ndarray, vector: Any = None) -> np.ndarray:
    """nibabel's ``from_matvec``: the matrix's type, the translation cast into it."""
    matrix = np.asarray(matrix)
    nin, nout = matrix.shape
    t = np.zeros((nin + 1, nout + 1), matrix.dtype)
    t[0:nin, 0:nout] = matrix
    t[nin, nout] = 1.0
    if vector is not None:
        t[0:nin, nout] = vector
    return t


def _diag_dot(diag: np.ndarray, vector: Any) -> np.ndarray:
    """``np.diag(d).dot(v)`` for a float matrix: BLAS ``dgemv``, whose one non-zero product
    per row is added to zeros (an exact zero is +0); numpy's own for integer matrices."""
    d = np.asarray(diag)
    v = np.asarray(vector)
    if np.result_type(d, v).kind in "iub":
        return np.diag(d).dot(v)
    return d.astype(np.float64) * v.astype(np.float64) + 0.0


def _card_inverse(card: np.ndarray) -> np.ndarray:
    """``np.linalg.inv`` of niworkflows' ``float32`` cardinal matrix: numpy inverts a
    ``float32`` matrix in ``float64`` (``_commonType``: LAPACK ``dgesv`` on the converted
    matrix, :func:`larmorx.transforms._linalg.inv`) and casts the inverse to ``float32``. A zero
    voxel size is numpy's "Singular matrix"; voxel sizes that are not normal ``float32`` numbers
    make nibabel's ``set_qform`` refuse the resulting affine, and larmorx raises its
    ``HeaderDataError`` here (the matrix it prints is not niworkflows')."""
    from larmorx.transforms._linalg import inv

    d = np.diag(card)[:3]
    tiny = np.finfo(np.float32).tiny
    if np.all(d != 0) and (
        not np.isfinite(card).all() or np.any(np.abs(d.astype(np.float64)) < tiny)
    ):
        raise HeaderDataError(f"Could not decompose affine:\n{np.full((4, 4), np.nan)}")
    return inv(card.astype(np.float64)).astype(np.float32)


def resample_by_spacing(
    in_file: str | os.PathLike[str] | Image,
    zooms: Any,
    order: int = 3,
    clip: bool = True,
    smooth: bool | float | Any = False,
    *,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> Image:
    """niworkflows' ``resample_by_spacing(in_file, zooms, order, clip, smooth)``: the image on a
    grid of voxel sizes ``zooms`` covering its field of view (see the module). ``smooth`` is
    ``True`` (sigma ``max(0, (old / new - 1) / 2)`` voxels per axis, PIPE-35), a sigma, or
    ``False``. ``in_file`` is a file or an image read as nibabel reads it
    (:func:`larmorx.io.load_nibabel`)."""
    from larmorx.ndimage import gaussian_filter, map_coordinates
    from larmorx.niworkflows.bids import _set_zooms, _zooms
    from larmorx.transforms._linalg import affine_points, inv, matmul

    img = in_file if isinstance(in_file, Image) else load_nibabel(Path(in_file))
    hdr = img.header
    if hdr is None:
        raise ValueError("resample_by_spacing needs a NIfTI header")
    fields = hdr.to_dict()
    scode = int(hdr.sform_code)
    qcode = int(hdr.qform_code)
    sform = np.eye(4)
    sform[0, :] = np.asarray(hdr.srow_x, dtype=np.float32)
    sform[1, :] = np.asarray(hdr.srow_y, dtype=np.float32)
    sform[2, :] = np.asarray(hdr.srow_z, dtype=np.float32)
    zooms = np.array(zooms)
    pre_zooms = np.array(_zooms(fields)[:3])
    shape = tuple(int(n) for n in np.asarray(img.data).shape)
    card = _from_matvec(np.diag(pre_zooms))
    extent = _diag_dot(np.diag(card[:3, :3]), np.array(shape[:3]))
    card[:3, 3] = -0.5 * extent
    icard = _card_inverse(card).astype(np.float64)
    new_size = np.ceil(extent / zooms).astype(int)
    offset = (extent - _diag_dot(zooms, new_size)) * 0.5
    new_card = _from_matvec(np.diag(zooms), card[:3, 3] + offset)
    new_grid = np.array(
        np.meshgrid(
            np.arange(new_size[0]),
            np.arange(new_size[1]),
            np.arange(new_size[2]),
            indexing="ij",
        )
    ).reshape((3, -1))
    pts = np.ascontiguousarray(new_grid.T, dtype=np.float64)
    pts = affine_points(new_card.astype(np.float64), pts, n_threads=n_threads)
    ijk = affine_points(icard, pts, n_threads=n_threads).T
    if smooth:
        if smooth is True:
            smooth = np.maximum(0, (pre_zooms / zooms - 1) / 2)
        data = gaussian_filter(
            np.asarray(img.data, dtype=np.float64),
            smooth,
            numpy_version=numpy_version,
            n_threads=n_threads,
        )
    else:
        data = np.asarray(img.data)
    resampled = map_coordinates(
        data, ijk, order=order, mode="constant", cval=0, prefilter=True, n_threads=n_threads
    ).reshape(tuple(int(n) for n in new_size))
    if clip:
        resampled = np.clip(resampled, a_min=data.min(), a_max=data.max())
    _set_zooms(fields, zooms)
    affine = np.asarray(img.affine, dtype=np.float64)
    rot = matmul(affine, icard)
    new_affine = matmul(rot, new_card.astype(np.float64))
    if qcode != 0:
        qform = _get_qform(hdr)
        fields = set_xform(fields, "qform", matmul(new_affine, matmul(inv(affine), qform)), qcode)
    if scode != 0:
        fields = set_xform(fields, "sform", matmul(new_affine, matmul(inv(affine), sform)), scode)
    if (scode, qcode) == (0, 0):
        fields = set_xform(fields, "qform", new_affine, 1)
        fields = set_xform(fields, "sform", new_affine, 1)
    dtype = hdr.data_dtype if hdr.data_dtype is not None else resampled.dtype
    out_fields = header_for_image(
        [int(n) for n in resampled.shape], np.dtype(dtype).name, new_affine, fields, None
    )
    return Image(resampled, new_affine, NiftiHeader.from_dict(out_fields))


def regrid_to_zooms(
    in_file: str | os.PathLike[str],
    zooms: Any,
    *,
    order: int = 3,
    clip: bool = True,
    smooth: bool | float = False,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str = NUMPY_NEWEST,
    n_threads: int = 1,
) -> RegridToZoomsResult:
    """niworkflows' ``RegridToZooms``: :func:`resample_by_spacing` of ``in_file`` written as
    ``<in_file>_regrid`` in ``out_dir`` (the working directory by default) by nibabel's
    writer. ``zooms`` are three voxel sizes (nipype's ``traits.Float``: numbers as floats)."""
    from larmorx.niworkflows.images import _fname_presuffix

    z = tuple(float(v) for v in zooms)
    if len(z) != 3:
        raise ValueError(f"zooms must be three voxel sizes, not {zooms!r}")
    out_dir = os.fspath(out_dir or os.getcwd())
    os.makedirs(out_dir, exist_ok=True)
    out = _fname_presuffix(os.fspath(in_file), "_regrid", out_dir)
    img = resample_by_spacing(
        in_file,
        z,
        order=int(order),
        clip=bool(clip),
        smooth=smooth,
        numpy_version=numpy_version,
        n_threads=n_threads,
    )
    save_nibabel(img, out)
    return RegridToZoomsResult(out_file=out)
