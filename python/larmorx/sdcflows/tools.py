# SPDX-License-Identifier: Apache-2.0 AND MIT
"""sdcflows' grid helpers (``sdcflows/utils/tools.py``, sdcflows 2.17.1; Apache-2.0).

- :func:`resample_to_zooms`: an image resampled onto a grid with new voxel sizes (cubic
  B-splines), as ``BSplineApprox`` resamples a field map finer than ``zooms_min``.
- :func:`rescale_affine`: nibabel's ``rescale_affine`` (nibabel 5.4.2, MIT), the affine of a
  grid with new voxel sizes and the same centre voxel position.

The arithmetic is sdcflows' with nibabel's and nitransforms' (25.1.0, MIT) products in numpy's
BLAS order (OpenBLAS's Haswell kernels, the same bits on every platform;
``crates/larmorx-sdcflows/src/tools.rs``) and SciPy's ``map_coordinates`` (``lx.ndimage``).
"""

from __future__ import annotations

import os
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader

__all__ = ["resample_to_zooms", "rescale_affine"]


def _as_loaded(obj: Any) -> Image:
    """An image as sdcflows' ``nb.load`` gives it: a path is read with nibabel's ``dataobj``
    values (the stored type, or float64 when the header scales the data)."""
    if isinstance(obj, (str, os.PathLike)):
        from larmorx.io.nifti import load

        return load(obj)
    return as_image(obj)


def _header(img: Image) -> NiftiHeader:
    """The image's header, or the one nibabel makes for ``Nifti1Image(data, affine)``."""
    if img.header is not None:
        return img.header
    fields = _core.nifti_header_for_image(
        list(img.shape), np.asarray(img.data).dtype.name, np.asarray(img.affine), None, None
    )
    return NiftiHeader.from_dict(fields)


def rescale_affine(affine: Any, shape: Any, zooms: Any, new_shape: Any | None = None) -> np.ndarray:
    """nibabel's ``rescale_affine(affine, shape, zooms, new_shape)`` for a 4×4 affine: the
    columns of ``affine[:3, :3]`` scaled to ``zooms`` and the translation that keeps the voxel
    ``(shape - 1) // 2`` where the new grid's voxel ``(new_shape - 1) // 2`` is (``new_shape``
    defaults to ``shape``)."""
    a = np.ascontiguousarray(affine, dtype=np.float64)
    if a.shape != (4, 4):
        raise ValueError("rescale_affine is ported for 4x4 affines")
    shape3 = [int(n) for n in np.asarray(shape)]
    ns = np.array(new_shape if new_shape is not None else shape)
    z = np.broadcast_to(np.asarray(zooms), (3,))
    return np.asarray(
        _core.sdcflows_rescale_affine(
            a,
            (shape3[0], shape3[1], shape3[2]),
            tuple(float(v) for v in z),
            tuple(float(v) for v in np.broadcast_to(ns, (3,))),
        )
    )


def resample_to_zooms(
    in_file: Any,
    zooms: Any,
    order: int = 3,
    prefilter: bool = True,
    *,
    n_threads: int = 1,
) -> Image:
    """sdcflows' ``resample_to_zooms(in_file, zooms, order=3, prefilter=True)``: the image on a
    grid with voxel sizes ``zooms``, ``np.rint(shape * zooms_before / zooms)`` voxels, centred
    on the same voxel (:func:`rescale_affine`), resampled by nitransforms' identity transform
    with SciPy's spline interpolation (``order``, ``mode="constant"``, ``cval=0``,
    ``prefilter``).

    ``in_file`` is a NIfTI path (read with nibabel's ``dataobj`` values: the stored type, or
    float64 when the header scales the data), an :class:`~larmorx.Image`, a nibabel image or an
    ``(array, affine)`` pair; 3D, or 4D with one volume. The result keeps the input's data type
    (nitransforms' default: integers are rounded as SciPy rounds them) and has the header
    sdcflows gives it: the input's, with the new grid in the sform and qform (their codes
    kept) and the input's stored type.
    """
    img = _as_loaded(in_file)
    data = np.asarray(img.data)
    hdr = _header(img)
    singleton_4d = data.ndim == 4 and data.shape[-1] == 1
    if data.ndim > 3 and not singleton_4d:
        raise ValueError(
            "resample_to_zooms is ported for 3D images (and 4D images with one volume), "
            "as sdcflows uses it"
        )
    if data.ndim < 3:
        raise ValueError("the image must have at least 3 dimensions")
    vol = data.reshape(data.shape[:3]) if singleton_4d else data
    shape3 = tuple(int(n) for n in data.shape[:3])
    # new_shape = np.rint(np.array(in_file.shape[:3]) * pre_zooms / zooms)
    zooms_arr = np.array(zooms)
    pre_zooms = np.array([np.float32(z) for z in hdr.zooms[:3]], dtype=np.float32)
    new_shape = np.rint(np.array(shape3) * pre_zooms / zooms_arr)
    affine = rescale_affine(img.affine, shape3, zooms_arr, new_shape)
    int_shape = new_shape.astype(int)
    if np.any(int_shape < 0):
        raise ValueError("negative dimensions are not allowed")
    new3 = tuple(int(n) for n in int_shape)
    # The reference's header: the input's, the new grid in both forms (their codes kept).
    fields = hdr.to_dict()
    fields = _core.nifti_set_xform(fields, "sform", affine, int(hdr.sform_code))
    fields = _core.nifti_set_xform(fields, "qform", affine, int(hdr.qform_code))
    disk = hdr.data_dtype
    disk_name = (disk if disk is not None else data.dtype).name
    ref = _core.nifti_header_for_image(list(new3), disk_name, affine, fields, None)
    out_shape = (*new3, 1) if singleton_4d else new3
    out = _core.nifti_header_for_image(list(out_shape), disk_name, affine, ref, None)
    out["scl_slope"], out["scl_inter"] = hdr.scl_slope, hdr.scl_inter
    n = int(np.prod(new3))
    if n == 0:
        np.empty(0).reshape((-1, *new3))  # nitransforms' reshape: numpy's ValueError
    if n == 1:
        raise AssertionError(
            "nitransforms 25.1.0 cannot resample onto a single-voxel grid (its "
            "`assert targets.ndim == 2`)"
        )
    try:
        coords = _core.sdcflows_affine_coordinates(
            new3,
            np.ascontiguousarray(affine),
            np.ascontiguousarray(img.affine),
            None,
            int(n_threads),
        )
    except ValueError as err:
        if str(err) == "Singular matrix":
            raise np.linalg.LinAlgError("Singular matrix") from None
        raise
    from larmorx.ndimage import map_coordinates

    values = map_coordinates(
        vol,
        coords,
        order=order,
        mode="constant",
        cval=0.0,
        prefilter=prefilter,
        n_threads=n_threads,
    )
    result = np.asarray(values).reshape(new3, order="F")
    if singleton_4d:
        result = result[..., None]
    return Image(result, affine, NiftiHeader.from_dict(out))
