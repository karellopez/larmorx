# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""niworkflows' ``GenerateSamplingReference`` (``_gen_reference``), with the nearest-neighbour
path of nilearn's ``resample_img`` under it.

``_gen_reference`` makes the grid other images are resampled onto: the field of view of
``fixed_image`` on a RAS grid with ``moving_image``'s voxel sizes (rounded to 0.001 mm), the
fixed image's values copied onto it by nearest-neighbour resampling. sdcflows' SyN
preprocessing resamples the anatomical reference, its mask and the SyN prior onto it.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's): ``interfaces/nibabel.py`` (``GenerateSamplingReference``,
``_gen_reference``); and from nilearn 0.14.1 (BSD-3-Clause, the nilearn developers; the
newest) with nilearn 0.11.1's steps (fMRIPrep 25.2.6's image): ``image/resampling.py``
(``resample_img``, ``get_bounds``, ``to_matrix_vector``, ``_resampling_not_needed``,
``_check_resample_img_inputs``, ``_resample_one_img``), ``image/image.py`` (``new_img_like``,
``crop_img``, ``_downcast_from_int64_if_possible``). The arithmetic is numpy's, SciPy's and
nibabel's as nilearn runs it:

- the bounding box's corners are ``np.dot(transform, box)`` of a 4 x 4 and a 4 x 8 matrix,
  which OpenBLAS's Haswell ``dgemm`` adds as one fused multiply-add per term from zero
  (measured: ``validation/investigations/sdcflows-fieldmap-b/relay9/probe_bounds.py``), with
  ``np.linalg.inv`` and the 4 x 4 products of :mod:`larmorx.transforms._linalg`;
- the voxels are SciPy's ``affine_transform`` at order 0 (:func:`larmorx.ndimage.affine_transform`:
  the general matrix's sums in SciPy 1.16's order, or 1.15's with ``scipy_version``, and the
  ``zoom_shift`` path for a diagonal);
- the output header is nilearn's: 0.14 copies the fixed image's header (``copy_header=True``:
  ``scl_slope``, ``scl_inter`` and ``glmax`` set to 0, ``cal_max``/``cal_min`` to the data's
  range), 0.11 makes a new one (``copy_header=False``), and nibabel's constructor sets the new
  affine into it (:func:`larmorx.io.nibabel_header.header_for_image`); nilearn 0.11 copies
  instead of resampling when the grids differ by whole voxels (``force_resample=False``);
- ``_gen_reference`` then sets the qform and sform of the new affine with the sform's code (2
  when it was 1) and writes ``descrip``.

One step is not bit for bit: nilearn inverts the image's affine with ``scipy.linalg.inv``
(LAPACK ``getrf`` and ``getri`` through SciPy's own OpenBLAS), whose last bits differ from
numpy's ``inv`` used here in tiny entries. The inverse only places the sampling points (not the
grid or the header), and a nearest-neighbour value changes only when a point lies within about
1e-12 voxel of a half-voxel boundary; no such case was found (``check_sampling_reference.py``).
"""

from __future__ import annotations

import os
import warnings
from collections.abc import Sequence
from fractions import Fraction
from typing import Any

import numpy as np

from larmorx import _nipype
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import header_for_image, load_nibabel, nibabel_affine, set_xform
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader

__all__ = [
    "NILEARN_NEWEST",
    "BoundingBoxError",
    "generate_sampling_reference",
    "resample_img_nearest",
]

#: The nilearn release ported (the newest); ``"0.11.1"`` gives fMRIPrep 25.2.6's image's steps.
NILEARN_NEWEST = "0.14.1"
#: niworkflows' release, written into the reference's ``descrip``.
NIWORKFLOWS_VERSION = "1.15.4"
SCIPY_NEWEST = "1.18.1"


class BoundingBoxError(ValueError):
    """nilearn's ``BoundingBoxError``: the target grid holds none of the data."""


def _minor(version: str) -> tuple[int, int]:
    parts = str(version).split(".")
    try:
        return int(parts[0]), int("".join(c for c in parts[1] if c.isdigit()))
    except (IndexError, ValueError):
        raise ValueError(f"not a nilearn version: {version!r}") from None


def _fma(a: float, b: float, c: float) -> float:
    """``a * b + c`` rounded once (an exact fused multiply-add)."""
    return float(Fraction(float(a)) * Fraction(float(b)) + Fraction(float(c)))


def _get_bounds(shape: Sequence[int], affine: np.ndarray) -> list[tuple[float, float]]:
    """nilearn's ``get_bounds``: the world bounds of the corner voxels' centres, the corners
    ``np.dot(affine, box)`` as OpenBLAS's Haswell ``dgemm`` computes them (one fused
    multiply-add per term, from zero)."""
    a, b, c = (int(n) - 1 for n in shape)
    box = np.array(
        [
            [0.0, 0, 0, 1],
            [a, 0, 0, 1],
            [0, b, 0, 1],
            [0, 0, c, 1],
            [a, b, 0, 1],
            [a, 0, c, 1],
            [0, b, c, 1],
            [a, b, c, 1],
        ]
    ).T
    out = np.empty((3, 8))
    for i in range(3):
        for j in range(8):
            acc = 0.0
            for k in range(4):
                acc = _fma(affine[i, k], box[k, j], acc)
            out[i, j] = acc
    return list(zip(out.min(axis=-1), out.max(axis=-1), strict=False))


def _diag_dot(matrix: np.ndarray, vector: Sequence[float]) -> np.ndarray:
    """``matrix.dot(vector)`` for a diagonal 3 x 3 ``matrix`` (BLAS ``dgemv``: every other
    product is zero, so each entry is its one product, from zero)."""
    return np.array([0.0 + float(matrix[i, i]) * float(vector[i]) for i in range(3)])


def _downcast_from_int64_if_possible(data: np.ndarray) -> np.ndarray:
    if data.dtype not in (np.int64, np.uint64):
        return data
    info = np.iinfo(np.int32)
    if info.min <= np.min(data) and info.max >= np.max(data):
        warnings.warn(
            "Data array used to create a new image contains 64-bit ints. This is likely due to "
            "creating the array with numpy and passing `int` as the `dtype`. Many tools such as "
            "FSL and SPM cannot deal with int64 in Nifti images, so for compatibility the data "
            "has been converted to int32.",
            stacklevel=4,
        )
        return data.astype("int32")
    warnings.warn(
        "Data array used to create a new image contains 64-bit ints, and some values too large "
        "to store in 32-bit ints. The resulting image thus contains 64-bit ints, which may cause "
        "some compatibility issues with some other tools or an error when saving the image to a "
        "nifti file.",
        stacklevel=4,
    )
    return data


def _new_img_like(ref: Image, data: np.ndarray, affine: np.ndarray, copy_header: bool) -> Image:
    """nilearn's ``new_img_like(ref, data, affine, copy_header)`` (a NIfTI-1 image) with the
    header nibabel's constructor makes."""
    if data.dtype == np.bool_:
        data = data.astype(np.uint8)
    data = _downcast_from_int64_if_possible(data)
    template = None
    if copy_header and ref.header is not None:
        template = ref.header.to_dict()
        template["scl_slope"] = 0.0
        template["scl_inter"] = 0.0
        template["glmax"] = 0
        template["cal_max"] = float(np.float32(np.max(data))) if data.size > 0 else 0.0
        template["cal_min"] = float(np.float32(np.min(data))) if data.size > 0 else 0.0
        dtype = np.dtype(ref.header.data_dtype)
    else:
        dtype = data.dtype
        if dtype in (np.dtype("int64"), np.dtype("uint64")):
            raise ValueError(
                f"Image data has type {dtype}, which may cause incompatibilities with other "
                "tools. To use this type, pass an explicit header or dtype argument to "
                "Nifti1Image()."
            )
    fields = header_for_image(
        [int(n) for n in data.shape],
        dtype.name,
        np.ascontiguousarray(affine, dtype=np.float64),
        template,
        None,
    )
    return Image(data, np.asarray(affine, dtype=np.float64), NiftiHeader.from_dict(fields))


def _crop_offsets(data: np.ndarray, rtol: float = 1e-8) -> tuple[slice, ...]:
    """nilearn 0.11's ``crop_img(img, rtol, pad=False, return_offset=True)``'s slices."""
    infinity_norm = max(-data.min(), data.max())
    passes = np.logical_or(data < -rtol * infinity_norm, data > rtol * infinity_norm)
    if data.ndim == 4:
        passes = np.any(passes, axis=-1)
    coords = np.array(np.where(passes))
    if coords.shape[1] == 0:
        start, end = [0, 0, 0], list(data.shape)
    else:
        start = coords.min(axis=1)
        end = coords.max(axis=1) + 1
    return tuple(slice(s, e) for s, e in zip(start, end, strict=False))[:3]


def resample_img_nearest(
    img: Image,
    target_affine: Any,
    target_shape: Sequence[int] | None = None,
    *,
    copy: bool = False,
    nilearn_version: str = NILEARN_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """nilearn's ``resample_img(img, target_affine, target_shape, interpolation='nearest')``
    for an image read as nibabel reads it (:func:`larmorx.io.load_nibabel`): the data on the
    grid of ``target_affine`` (3 x 3: the grid's origin placed at the data's bounding box, its
    shape the box's; 4 x 4 with ``target_shape``), with nilearn's header (see the module).
    ``img`` itself when no resampling is needed. ``copy`` is nilearn's (``False`` for an image
    given as a file name); it only decides whether the image itself is returned."""
    old = _minor(nilearn_version) < (0, 13)
    if old and target_shape is not None and target_affine is None:
        raise ValueError("If target_shape is specified, target_affine should be specified too.")
    if target_shape is not None:
        if len(target_shape) != 3:
            raise ValueError(
                "The shape specified should be the shape of the 3D grid, and thus of length 3. "
                f"{target_shape} was specified."
            )
        if target_affine is None:
            raise ValueError("If target_shape is specified, target_affine should be specified too.")
        if not old and np.shape(target_affine) == (3, 3):
            raise ValueError(
                "Given target shape without anchor vector: "
                "'target_affine' shape should be (4, 4) and not (3, 3)"
            )
    hdr = img.header
    if hdr is not None and int(hdr.sform_code) == 0:
        warnings.warn(
            "The provided image has no sform in its header. Please check the provided file. "
            "Results may not be as expected.",
            stacklevel=2,
        )
    affine = np.asarray(img.affine, dtype=np.float64)
    shape = tuple(int(n) for n in np.asarray(img.data).shape)
    if target_affine is None and target_shape is None:
        return img
    target_affine = np.asarray(target_affine)
    if (
        target_affine.shape == affine.shape
        and np.allclose(target_affine, affine)
        and np.array_equal(np.asarray(target_shape), shape)
    ):
        return img
    if np.all(np.array(target_shape) == shape[:3]) and np.allclose(target_affine, affine):
        return img
    data = np.asanyarray(img.data)
    from larmorx.transforms._linalg import inv, matmul

    if target_affine.shape == (3, 3):
        missing_offset = True
        tmp = np.eye(4)
        tmp[:3, :3] = target_affine
        target_affine = tmp
    else:
        missing_offset = False
        target_affine = np.array(target_affine, dtype=np.float64, copy=True)
    transform_affine = matmul(inv(target_affine), affine)
    (xmin, xmax), (ymin, ymax), (zmin, zmax) = _get_bounds(data.shape[:3], transform_affine)
    if missing_offset:
        target_affine[:3, 3] = _diag_dot(target_affine[:3, :3], [xmin, ymin, zmin])
        (xmin, xmax), (ymin, ymax), (zmin, zmax) = (
            (0.0, xmax - xmin),
            (0.0, ymax - ymin),
            (0.0, zmax - zmin),
        )
    if target_shape is None:
        target_shape = (
            int(np.ceil(xmax)) + 1,
            int(np.ceil(ymax)) + 1,
            int(np.ceil(zmax)) + 1,
        )
    if xmax < 0 or ymax < 0 or zmax < 0:
        raise BoundingBoxError(
            "The field of view given by the target affine does not contain any of the data"
        )
    if np.all(target_affine == affine):
        transform_affine = np.eye(4)
    else:
        # nilearn: np.dot(scipy.linalg.inv(affine), target_affine) (see the module)
        transform_affine = matmul(inv(affine), target_affine)
    A = transform_affine[:3, :3]
    b = transform_affine[:3, 3]
    target_shape = tuple(int(n) for n in target_shape)
    other_shape = list(data.shape[3:])
    out = np.zeros(list(target_shape) + other_shape, order="F", dtype=data.dtype.newbyteorder("="))
    if old and np.all(np.eye(3) == A) and all(bt == np.round(bt) for bt in b):
        # nilearn 0.11 (force_resample=False): the cropped data placed by whole voxels
        offsets = _crop_offsets(data)
        cropped = data[offsets]
        indices = [
            (int(off.start - dim_b), int(off.stop - dim_b))
            for off, dim_b in zip(offsets[:3], b[:3], strict=False)
        ]
        slices = tuple(
            slice(np.max((0, index[0])), np.min((dimsize, index[1])))
            for dimsize, index in zip(out.shape, indices, strict=False)
        )
        subset = tuple(slice(0, s.stop - s.start) for s in slices)
        out[slices] = cropped[subset]
    else:
        from larmorx.ndimage import affine_transform

        matrix: np.ndarray = np.diag(A) if np.all(np.diag(np.diag(A)) == A) else A
        for ind in np.ndindex(*other_shape):
            vol = data[(slice(None),) * 3 + ind]
            finite = vol.dtype.kind in "iu" or bool(np.isfinite(vol).all())
            if not finite:
                warnings.warn(
                    "NaNs or infinite values are present in the data passed to resample. This "
                    "is a bad thing as they make resampling ill-defined and much slower.",
                    RuntimeWarning,
                    stacklevel=2,
                )
            res = affine_transform(
                vol,
                matrix,
                offset=b,
                output_shape=target_shape,
                output=out.dtype,
                order=0,
                mode="constant",
                cval=0.0,
                scipy_version=scipy_version,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
            )
            if not finite:
                # nilearn extrapolates the non-finite voxels, resamples, then sets NaN where the
                # (nearest) resampled mask of non-finite voxels is set
                bad = affine_transform(
                    (~np.isfinite(vol)).astype(np.uint8),
                    matrix,
                    offset=b,
                    output_shape=target_shape,
                    output=np.uint8,
                    order=0,
                    scipy_version=scipy_version,
                    n_threads=n_threads,
                )
                res = np.where(bad != 0, np.nan, res).astype(out.dtype)
            out[(slice(None),) * 3 + ind] = res
    vmin = min(np.nanmin(data), 0)
    vmax = max(np.nanmax(data), 0)
    out.clip(vmin, vmax, out=out)
    return _new_img_like(img, out, target_affine, copy_header=not old)


def _canonical_zooms(path: str) -> np.ndarray:
    """``nb.as_closest_canonical(nb.load(path)).header.get_zooms()[:3]`` without reading the
    voxels: the zooms nibabel's constructor sets from the reoriented affine (the header's own
    when the image is already closest to RAS)."""
    from larmorx.io.nibabel_header import io_orientation
    from larmorx.io.nifti import read_header
    from larmorx.niworkflows.images import _IDENTITY_ORNT
    from larmorx.transforms._linalg import matmul
    from larmorx.transforms.resample import _inv_ornt_aff

    hdr = read_header(path)
    affine = nibabel_affine(hdr)
    ornt = io_orientation(affine)
    if np.array_equal(ornt, _IDENTITY_ORNT):
        return np.asarray(hdr.pixdim[1:4], dtype=np.float32)
    shape = tuple(int(n) for n in hdr.shape)
    new_aff = matmul(affine, _inv_ornt_aff(ornt, shape))
    new_shape = list(shape)
    for i, (ax, _) in enumerate(ornt[: len(shape)]):
        if not np.isnan(ax):
            new_shape[int(ax)] = shape[i]
    fields = header_for_image(new_shape, np.dtype(hdr.data_dtype).name, new_aff, hdr.to_dict())
    return np.asarray(NiftiHeader.from_dict(fields).pixdim[1:4], dtype=np.float32)


def generate_sampling_reference(
    fixed_image: str | os.PathLike[str],
    moving_image: str | os.PathLike[str],
    *,
    xform_code: int | None = None,
    fov_mask: str | os.PathLike[str] | None = None,
    keep_native: bool = True,
    target_resolution: Sequence[float] | None = None,
    out_dir: str | os.PathLike[str] = ".",
    niworkflows_version: str = NIWORKFLOWS_VERSION,
    nilearn_version: str = NILEARN_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``GenerateSamplingReference``: the field of view of ``fixed_image`` on a RAS
    grid of ``moving_image``'s voxel sizes (after nibabel's ``as_closest_canonical``; or
    ``target_resolution``), rounded to 0.001 mm, the fixed image's values resampled onto it
    (nearest neighbour: :func:`resample_img_nearest`), written as ``<fixed>_reference`` in
    ``out_dir`` with the qform and sform of the grid (code: the sform's, 2 for 1;
    ``xform_code`` forces 2 or 4) and ``descrip`` ``reference image generated by
    GenerateSamplingReference (niworkflows v<version>).``. With ``fov_mask`` (a mask on the
    fixed image's grid) the grid is cropped to the mask's bounding box plus two voxels. Without
    ``keep_native`` the fixed image itself. The path of the reference."""
    if xform_code not in (None, 2, 4):
        raise ValueError(f"xform_code must be None, 2 or 4, not {xform_code!r}")
    if not keep_native:
        return os.fspath(fixed_image)
    message = f"GenerateSamplingReference (niworkflows v{niworkflows_version})"
    fixed_image = os.fspath(fixed_image)
    os.makedirs(out_dir, exist_ok=True)
    out_file = _nipype.fname_presuffix(fixed_image, suffix="_reference", newpath=out_dir)
    if target_resolution is not None:
        new_zooms = np.round(np.asarray(target_resolution), 3)
    else:
        new_zooms = np.round(_canonical_zooms(os.fspath(moving_image)), 3)
    new_affine = np.diag(new_zooms)
    kw: dict[str, Any] = {
        "nilearn_version": nilearn_version,
        "scipy_version": scipy_version,
        "n_threads": n_threads,
        "upstream_defects": upstream_defects,
    }
    fixed = load_nibabel(fixed_image, n_threads=n_threads)
    resampled = resample_img_nearest(fixed, new_affine, **kw)
    if fov_mask is not None:
        masknii = load_nibabel(fov_mask, n_threads=n_threads)
        if tuple(masknii.shape[:3]) != tuple(fixed.shape[:3]):
            raise RuntimeError("Fixed image and mask do not have the same dimensions.")
        if not np.allclose(masknii.affine, fixed.affine, atol=1e-5):
            raise RuntimeError("Fixed image and mask have different affines")
        masknii = resample_img_nearest(masknii, new_affine, copy=True, **kw)
        res_shape = np.array(masknii.shape[:3])
        bbox = np.argwhere(np.asanyarray(masknii.data) > 0)
        new_origin = np.clip(bbox.min(0) - 2, a_min=0, a_max=None)
        new_end = np.clip(bbox.max(0) + 2, a_min=0, a_max=res_shape - 1)
        new_affine_4 = np.array(resampled.affine, dtype=np.float64, copy=True)
        new_affine_4[:3, 3] = _diag_dot(new_affine_4[:3, :3], new_origin) + new_affine_4[:3, 3]
        new_shape = new_end - new_origin + 1
        resampled = resample_img_nearest(fixed, new_affine_4, target_shape=new_shape.tolist(), **kw)
    xform = np.asarray(resampled.affine, dtype=np.float64)
    hdr = resampled.header
    assert hdr is not None
    fields = hdr.to_dict()
    qform_code, sform_code = int(fields["qform_code"]), int(fields["sform_code"])
    code = sform_code if sform_code > 0 else qform_code
    if code == 1:
        code = 2
    if xform_code is not None:
        code = xform_code
    fields = set_xform(fields, "qform", xform, int(code))
    fields = set_xform(fields, "sform", xform, int(code))
    text = ("reference image generated by %s." % message).encode()  # noqa: UP031
    size = len(fields["descrip"])
    fields["descrip"] = text[:size].ljust(size, b"\0")
    header = NiftiHeader.from_dict(fields)
    # nilearn 0.13 onwards copies the header with scl_slope = scl_inter = 0, which nibabel
    # writes unscaled; a new header (0.11) or the loaded image (nothing resampled) has them
    # unset (NaN, as nibabel loads them): scaled as usual
    scale = resampled is fixed or _minor(nilearn_version) < (0, 13)
    save_nibabel(Image(resampled.data, xform, header), out_file, scale=scale, n_threads=n_threads)
    return out_file
