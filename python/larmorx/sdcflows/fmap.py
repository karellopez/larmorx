# SPDX-License-Identifier: Apache-2.0 AND MIT
"""sdcflows' conversions between a field map and a displacement field.

Ported from sdcflows 2.17.1 (``sdcflows/transform.py``: ``fmap_to_disp``, ``disp_to_fmap``;
``sdcflows/interfaces/fmap.py``: ``DisplacementsField2Fieldmap``; Apache-2.0, the NiPreps
developers) with nibabel's (5.4.2, MIT) ``apply_affine`` and ``set_intent``.

- :func:`fmap_to_disp`: a field map (Hz) as the displacement field (mm, ITK's LPS vectors by
  default) its shift along the phase-encoding axis makes for a readout time.
- :func:`disp_to_fmap`: the inverse: a displacement field as a field map in Hz, on the EPI's
  phase-encoding axis.
- :func:`displacements_field_to_fieldmap`: ``DisplacementsField2Fieldmap`` (files, the
  optional median removal).

The element arithmetic is numpy's own (the readout time's type decides float32 or float64, as
in sdcflows); the products with the affine are numpy's BLAS ones (OpenBLAS's Haswell
kernels, the same bits on every platform; ``crates/larmorx-sdcflows/src/tools.rs``) and the
inverse is numpy's ``inv`` (``lx.transforms``).
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader
from larmorx.sdcflows.tools import _as_loaded
from larmorx.sdcflows.transform import _apply_affine
from larmorx.transforms._linalg import inv

__all__ = [
    "DISPLACEMENTS_INTENT_NAME",
    "HZ_INTENT_NAME",
    "DisplacementsResult",
    "disp_to_fmap",
    "displacements_field_to_fieldmap",
    "fmap_to_disp",
]

#: The intent name of ``fmap_to_disp``'s displacement field ('vector').
DISPLACEMENTS_INTENT_NAME = "SDC"
#: The intent name of ``disp_to_fmap``'s field map ('estimate').
HZ_INTENT_NAME = "Delta_B0 [Hz]"
_INTENT_ESTIMATE = 1001
_INTENT_VECTOR = 1007
_XYZT_MM = 2
_PE_DIRECTIONS = ("j-", "j", "i", "i-", "k", "k-")


def _new_header(
    data: np.ndarray, affine: Any, intent: int, name: str, **fields: Any
) -> NiftiHeader:
    """The header of ``nb.Nifti1Image(data, affine)`` after ``set_intent(intent, name=name)``,
    ``set_xyzt_units('mm')`` and ``fields``."""
    base = _core.nifti_header_for_image(
        list(data.shape), data.dtype.name, np.ascontiguousarray(affine, dtype=np.float64), None, 1
    )
    base.update(
        intent_code=intent,
        intent_p1=0.0,
        intent_p2=0.0,
        intent_p3=0.0,
        intent_name=name.encode()[:16],
        xyzt_units=_XYZT_MM,
        **fields,
    )
    out = _core.nifti_header_for_image(
        list(data.shape), data.dtype.name, np.ascontiguousarray(affine, dtype=np.float64), base, 1
    )
    return NiftiHeader.from_dict(out)


def fmap_to_disp(fmap_nii: Any, ro_time: Any, pe_dir: str, itk_format: bool = True) -> Image:
    """sdcflows' ``fmap_to_disp``: the displacement field (float64, shape ``(X, Y, Z, 1, 3)``,
    the intent 'vector' named "SDC", millimetres) of a field map in Hz, for the total readout
    time ``ro_time`` (s) along ``pe_dir`` (``i``, ``j``, ``k``, with ``-`` for the reversed
    direction). The vectors are the voxel shifts through the field map's affine (without its
    translation), in ITK's LPS convention with ``itk_format`` (x and y negated)."""
    img = _as_loaded(fmap_nii)
    vsm = np.asarray(img.data, dtype=np.float64).copy() * (
        -ro_time if pe_dir.endswith("-") else ro_time
    )
    fieldshape = (*vsm.shape[:3], 1, 3)
    pe_axis = "ijk".index(pe_dir[0])
    ijk_deltas = np.zeros((vsm.size, 3), dtype="float32")
    ijk_deltas[:, pe_axis] = vsm.reshape(-1)
    aff = np.array(img.affine, dtype=np.float64)
    aff[:3, 3] = 0
    xyz_deltas = _apply_affine(aff, ijk_deltas)
    if itk_format:
        xyz_deltas[..., (0, 1)] *= -1.0
    field = xyz_deltas.reshape(fieldshape)
    return Image(
        field, img.affine, _new_header(field, img.affine, _INTENT_VECTOR, DISPLACEMENTS_INTENT_NAME)
    )


def _cal_max(data: np.ndarray) -> Any:
    return max((abs(np.asanyarray(data).min()), np.asanyarray(data).max()))


def disp_to_fmap(
    xyz_nii: Any, epi_nii: Any, ro_time: Any, pe_dir: str, itk_format: bool = True
) -> Image:
    """sdcflows' ``disp_to_fmap``: the field map in Hz (the intent 'estimate' named
    "Delta_B0 [Hz]", ``cal_max = max(|min|, max)``, ``cal_min = -cal_max``) of a displacement
    field (ITK's LPS vectors with ``itk_format``), on the EPI's phase-encoding axis ``pe_dir``
    for its readout time ``ro_time``: the displacements in the EPI's voxels
    (``inv(epi.affine)``) along that axis, divided by the signed readout time. A readout time
    of 0 gives infinities and NaN, as in sdcflows."""
    xyz = _as_loaded(xyz_nii)
    epi = _as_loaded(epi_nii)
    # get_fdata(dtype='float32'), copied: the vectors are negated in place below
    xyz_deltas = np.squeeze(np.array(xyz.data, dtype=np.float32)).reshape((-1, 3))
    if itk_format:
        xyz_deltas[:, (0, 1)] *= -1
    inv_aff = inv(np.asarray(epi.affine, dtype=np.float64))
    lin = np.zeros((4, 4))
    lin[:3, :3] = inv_aff[:3, :3]
    ijk_deltas = _apply_affine(lin, xyz_deltas).astype("float32")
    pe_axis = "ijk".index(pe_dir[0])
    vsm = ijk_deltas[:, pe_axis].reshape(xyz.shape[:3])
    scale_factor = -ro_time if pe_dir.endswith("-") else ro_time
    with np.errstate(divide="ignore", invalid="ignore"):
        data = vsm / scale_factor
    cal = _cal_max(data)
    hdr = _new_header(
        data,
        xyz.affine,
        _INTENT_ESTIMATE,
        HZ_INTENT_NAME,
        cal_max=float(np.float32(cal)),
        cal_min=float(-np.float32(cal)),
    )
    return Image(data, xyz.affine, hdr)


@dataclass(frozen=True)
class DisplacementsResult:
    """``DisplacementsField2Fieldmap``'s field map (``fieldmap``) and its file
    (``out_file``: ``<transform>_Hz``, with ``out_dir``; else ``None``)."""

    fieldmap: Image
    out_file: Path | None = None


def displacements_field_to_fieldmap(
    transform: Any,
    epi: Any,
    ro_time: float,
    pe_dir: str,
    demean: bool = False,
    itk_transform: bool = True,
    *,
    out_dir: str | os.PathLike[str] | None = None,
) -> DisplacementsResult:
    """sdcflows' ``DisplacementsField2Fieldmap``: :func:`disp_to_fmap` of a displacement field
    (``itk_transform``: ITK's vectors) for the EPI ``epi``; with ``demean`` the field's median
    is subtracted (the header's ``cal_max``/``cal_min`` are those before, as in sdcflows).
    ``ro_time`` is a float (sdcflows' trait), ``pe_dir`` one of ``j-``, ``j``, ``i``, ``i-``,
    ``k``, ``k-``. With ``out_dir`` the field map is written as ``<transform>_Hz``."""
    if pe_dir not in _PE_DIRECTIONS:
        raise ValueError(f"pe_dir must be one of {_PE_DIRECTIONS}, not {pe_dir!r}")
    fmap = disp_to_fmap(
        transform, epi, ro_time=float(ro_time), pe_dir=pe_dir, itk_format=itk_transform
    )
    if demean:
        data = np.array(fmap.data)
        data -= np.median(data)
        fmap = Image(data.astype("float32"), fmap.affine, fmap.header)
    out_file = None
    if out_dir is not None:
        from larmorx.sdcflows.approx import _fname_presuffix

        os.makedirs(out_dir, exist_ok=True)
        name = (
            os.fspath(transform)
            if isinstance(transform, (str, os.PathLike))
            else "transform.nii.gz"
        )
        out_file = Path(_fname_presuffix(name, "_Hz", out_dir))
        save_nibabel(fmap, out_file)
    return DisplacementsResult(fmap, out_file)
