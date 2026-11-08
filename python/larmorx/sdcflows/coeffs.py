# SPDX-License-Identifier: Apache-2.0
"""sdcflows' B-spline coefficient files: TOPUP's coefficients placed in space, coefficients
moved by a rigid transform, and the phase-encoding direction of a reoriented image.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers): ``TOPUPCoeffReorient``
(``_fix_topup_fieldcoeff``) and ``TransformCoefficients`` (``sdcflows/interfaces/bspline.py``,
``_move_coeff`` in ``sdcflows/transform.py``) and ``reorient_pedir``
(``sdcflows/utils/tools.py``), with nibabel's orientation and affine arithmetic and numpy's
4×4 products and inverses in OpenBLAS's Haswell order (``lx.transforms``), so the files are
sdcflows' bit for bit on every platform.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image
from larmorx.io.nifti import NiftiHeader
from larmorx.sdcflows.approx import _fname_presuffix
from larmorx.sdcflows.tools import _as_loaded, _header
from larmorx.transforms._linalg import inv, matmul
from larmorx.transforms.resample import _inv_ornt_aff, _io_orientation, _ornt_transform

__all__ = [
    "COEFF_INTENT_NAME",
    "PE_DIRS_TOPUP",
    "CoefficientsResult",
    "reorient_pedir",
    "topup_coeff_reorient",
    "transform_coefficients",
]

#: The phase-encoding directions ``TOPUPCoeffReorient`` accepts.
PE_DIRS_TOPUP = tuple(a + s for a in "ijkxyz" for s in ("", "-"))
#: The intent name sdcflows gives TOPUP's coefficients (stored as its first 16 bytes).
COEFF_INTENT_NAME = "B-Spline coefficients"
_INTENT_ESTIMATE = 1001
_RAS: list[list[int]] = [[0, 1], [1, 1], [2, 1]]


@dataclass(frozen=True)
class CoefficientsResult:
    """The coefficient images, in input order, and the files written (with ``out_dir``)."""

    coefficients: tuple[Image, ...]
    out_coeff: tuple[Path, ...] = ()


def reorient_pedir(pe_dir: str, source_ornt: Any, target_ornt: Any = None) -> str:
    """sdcflows' ``reorient_pedir(pe_dir, source_ornt, target_ornt=None)``: the
    phase-encoding direction (``i``, ``j-``, ... or ``x``, ``y-``, ...) after the data array is
    reoriented from ``source_ornt`` to ``target_ornt`` (nibabel orientations; RAS when
    ``None``). An orientation transform can be passed as ``source_ornt`` (applied) or as
    ``target_ornt`` (inverted)."""
    src = np.asarray(_RAS if source_ornt is None else source_ornt)
    dst = np.asarray(_RAS if target_ornt is None else target_ornt)
    if src.shape != dst.shape:
        raise ValueError("The orientations must have the same shape")
    if src.shape[1] != 2:
        raise ValueError(f"Invalid shape for an orientation: {src.shape}")
    xfm = _ornt_transform(src, dst).astype(int)
    directions = "ijk" if pe_dir[0] in "ijk" else "xyz"
    source_axis = directions.index(pe_dir[0])
    source_flip = pe_dir[1:] == "-"
    axis_xfm = xfm[source_axis, :]
    target_axis = directions[axis_xfm[0]]
    target_flip = source_flip ^ (axis_xfm[1] == -1)
    return f"{target_axis}-" if target_flip else target_axis


def _cal_max(data: np.ndarray) -> Any:
    """sdcflows' ``max((abs(data.min()), data.max()))`` (Python's ``max`` of two numpy
    scalars: the first when neither is larger)."""
    return max((abs(np.asanyarray(data).min()), np.asanyarray(data).max()))


def _coefficient_header(
    hdr: NiftiHeader, data: np.ndarray, newaff: np.ndarray, *, intent: bool
) -> NiftiHeader:
    """The coefficients' header sdcflows writes: the input's with ``newaff`` in the qform and
    the sform (code 1), ``cal_max = max(|min|, max)``, ``cal_min = -cal_max``, and (TOPUP's)
    the intent "estimate" named "B-Spline coefficients"."""
    fields = hdr.to_dict()
    fields = _core.nifti_set_xform(fields, "qform", newaff, 1)
    fields = _core.nifti_set_xform(fields, "sform", newaff, 1)
    cal = np.float32(_cal_max(data))
    fields["cal_max"] = float(cal)
    fields["cal_min"] = float(-cal)
    if intent:
        fields.update(
            intent_code=_INTENT_ESTIMATE,
            intent_p1=0.0,
            intent_p2=0.0,
            intent_p3=0.0,
            # numpy's 16-byte field truncates it: "B-Spline coeffic"
            intent_name=COEFF_INTENT_NAME.encode()[:16],
        )
    disk = hdr.data_dtype
    name = (disk if disk is not None else data.dtype).name
    out = _core.nifti_header_for_image(
        list(data.shape), name, np.ascontiguousarray(newaff), fields, None
    )
    out["scl_slope"], out["scl_inter"] = hdr.scl_slope, hdr.scl_inter
    return NiftiHeader.from_dict(out)


def _apply_affine(aff: np.ndarray, point: np.ndarray) -> np.ndarray:
    """nibabel's ``apply_affine(aff, point)`` of one point."""
    pts = np.ascontiguousarray(np.asarray(point, dtype=np.float64).reshape(1, 3))
    return np.asarray(_core.sdcflows_apply_affine(np.ascontiguousarray(aff), pts))[0]


def _write(images: list[Image], names: list[str], suffix: str, out_dir: Any) -> tuple[Path, ...]:
    if out_dir is None:
        return ()
    os.makedirs(out_dir, exist_ok=True)
    paths = []
    for img, name in zip(images, names, strict=True):
        p = _fname_presuffix(name, suffix, out_dir)
        img.save(p)
        paths.append(Path(p))
    return tuple(paths)


def _names(items: Sequence[Any], default: str) -> list[str]:
    return [
        os.fspath(c) if isinstance(c, (str, os.PathLike)) else f"{default}{i:03}.nii.gz"
        for i, c in enumerate(items)
    ]


def _listify(x: Any) -> list[Any]:
    return list(x) if isinstance(x, (list, tuple)) else [x]


def topup_coeff_reorient(
    in_coeff: Any,
    fmap_ref: Any,
    pe_dir: str,
    *,
    out_dir: str | os.PathLike[str] | None = None,
) -> CoefficientsResult:
    """sdcflows' ``TOPUPCoeffReorient``: TOPUP's B-spline coefficients (``--fout``'s
    ``fieldcoef`` files) placed in the space of the field-map reference.

    TOPUP stores its coefficients flipped left-right (FSL's internal LAS order) with the
    control-point spacings as voxel sizes. For each file this flips the first axis, negates
    the coefficients when the phase-encoding direction of the reference reoriented to RAS is
    the first axis (``i``), checks that the grid is TOPUP's for the reference (``shape //
    spacing + 3`` where the spacing is above 1), and gives the coefficients an affine with the
    reference's (RAS) directions and the control-point spacing, centred on the reference
    (qform and sform code 1), with ``cal_max``/``cal_min`` and the intent "estimate"
    ("B-Spline coefficients"). ``ValueError`` when the shape is not TOPUP's.

    ``in_coeff``: one or more coefficient files (or images); ``fmap_ref``: the reference
    TOPUP estimated the field on; ``pe_dir``: its phase-encoding direction (``i``, ``j-``, ...
    or ``x``, ``y-``, ...). With ``out_dir``, the files are written there as
    ``<coeff>_fixed``.
    """
    if pe_dir not in PE_DIRS_TOPUP:
        raise ValueError(f"pe_dir must be one of {PE_DIRS_TOPUP}, not {pe_dir!r}")
    coeffs = _listify(in_coeff)
    ref = _as_loaded(fmap_ref)
    # nb.io_orientation, as_reoriented (only the affine and the shape are used)
    ref_ornt = _io_orientation(np.asarray(ref.affine))
    if np.array_equal(ref_ornt, _RAS):
        ras_affine = np.asarray(ref.affine)
        ras_shape = np.array(ref.shape[:3])
    else:
        ras_affine = matmul(ref.affine, _inv_ornt_aff(ref_ornt, ref.shape))
        order = np.argsort(ref_ornt[:, 0])
        ras_shape = np.array([ref.shape[int(a)] for a in order])
    coeff_pe_dir = reorient_pedir(pe_dir, ref_ornt)
    reverse_pe = -1.0 if coeff_pe_dir[0] == "i" else 1.0
    out = []
    for c in coeffs:
        img = _as_loaded(c)
        hdr = _header(img)
        data = reverse_pe * np.flip(np.asanyarray(img.data), axis=0)
        coeff_shape = np.array(data.shape[:3])
        factors = np.array([np.float32(z) for z in hdr.zooms[:3]], dtype=np.float32)
        exp_shape = ras_shape // factors + 3 * (factors > 1)
        if not np.all(coeff_shape == exp_shape):
            raise ValueError(
                f"Shape of coefficients file {coeff_shape} does not meet the "
                f"expected shape {exp_shape} (toupup factors are {factors})."
            )
        newaff = np.eye(4)
        newaff[:3, :3] = ras_affine[:3, :3] * factors
        c_ref = _apply_affine(ras_affine, 0.5 * (ras_shape - 1))
        c_coeff = _apply_affine(newaff, 0.5 * (coeff_shape - 1))
        newaff[:3, 3] = c_ref - c_coeff
        out.append(Image(data, newaff, _coefficient_header(hdr, data, newaff, intent=True)))
    files = _write(out, _names(coeffs, "coefficients"), "_fixed", out_dir)
    return CoefficientsResult(tuple(out), files)


def transform_coefficients(
    in_coeff: Any,
    fmap_ref: Any,
    transform: Any,
    fmap_target: Any = None,
    *,
    out_dir: str | os.PathLike[str] | None = None,
    n_threads: int = 1,
) -> CoefficientsResult:
    """sdcflows' ``TransformCoefficients``: B-spline coefficient files moved to another space
    by a rigid-body transform (``_move_coeff``).

    ``transform`` is an ITK linear transform file with one transform (``.txt``, ``.tfm`` or
    ``.mat``; ANTs' rigid registration of the field-map reference) or a 4×4 RAS+ matrix (as
    nitransforms' ``to_ras()`` gives it). Each coefficient image keeps its data and gets the
    affine ``inv(inv(coeff.affine) @ inv(xfm))`` (qform and sform code 1) with
    ``cal_max``/``cal_min``. With ``fmap_target`` (sdcflows' debug mode) the field-map
    reference resampled through the inverse transform onto ``fmap_target``'s grid is written
    as ``debug_fmapref.nii.gz`` in ``out_dir`` (the current directory without one, where
    sdcflows writes it).
    With ``out_dir``, the coefficients are written as ``<coeff>_space-target``.
    """
    from larmorx.transforms.chain import Affine, TransformFileError, load_itk_linear

    if isinstance(transform, (str, os.PathLike)):
        loaded = load_itk_linear(transform)
        if not isinstance(loaded, Affine):
            raise TransformFileError("More than one linear transform found.")
        matrix = np.asarray(loaded.matrix, dtype=np.float64)
    else:
        matrix = np.asarray(transform, dtype=np.float64)
    if matrix.shape != (4, 4):
        raise ValueError("the transform must be a 4x4 matrix")
    # nitransforms' Affine: the last row normalised, the inverse with numpy
    m = matrix.copy()
    if not np.allclose(m[3, :], (0, 0, 0, 1)):
        raise ValueError(
            f"The last row of a homogeneus matrix should be (0, 0, 0, 1), got {m[3, :]}."
        )
    m[3, :] = (0, 0, 0, 1)
    inverse = inv(m)  # (~xfm).matrix
    coeffs = _listify(in_coeff)
    if fmap_target is not None:
        debug_dir = out_dir if out_dir is not None else os.getcwd()
        _debug_fmapref(fmap_ref, fmap_target, inverse, debug_dir, n_threads)
    out = []
    for c in coeffs:
        img = _as_loaded(c)
        hdr = _header(img)
        newaff = inv(matmul(inv(np.asarray(img.affine)), inverse))
        data = np.asanyarray(img.data)
        out.append(Image(data, newaff, _coefficient_header(hdr, data, newaff, intent=False)))
    files = _write(out, _names(coeffs, "coefficients"), "_space-target", out_dir)
    return CoefficientsResult(tuple(out), files)


def _debug_fmapref(
    fmap_ref: Any, fmap_target: Any, inverse: np.ndarray, out_dir: Any, n_threads: int
) -> None:
    """sdcflows' debug output: ``(~xfm).apply(fmap_ref, reference=fmap_target)`` with the
    target's affine in both forms (code 1), as ``debug_fmapref.nii.gz``."""
    from larmorx.ndimage import map_coordinates

    ref = _as_loaded(fmap_ref)
    target = _as_loaded(fmap_target)
    vol = np.asanyarray(ref.data)
    if vol.ndim == 4 and vol.shape[-1] == 1:
        vol = vol.reshape(vol.shape[:3])
    tshape = tuple(int(n) for n in target.shape[:3])
    coords = _core.sdcflows_affine_coordinates(
        tshape,
        np.ascontiguousarray(target.affine),
        np.ascontiguousarray(ref.affine),
        np.ascontiguousarray(inverse),
        int(n_threads),
    )
    values = map_coordinates(vol, coords, order=3, mode="constant", cval=0.0, n_threads=n_threads)
    moved = np.asarray(values).reshape(tshape, order="F")
    thdr = _header(target)
    disk = _header(ref).data_dtype
    fields = _core.nifti_header_for_image(
        list(tshape),
        (disk if disk is not None else vol.dtype).name,
        np.ascontiguousarray(target.affine),
        thdr.to_dict(),
        None,
    )
    fields = _core.nifti_set_xform(fields, "qform", np.ascontiguousarray(target.affine), 1)
    fields = _core.nifti_set_xform(fields, "sform", np.ascontiguousarray(target.affine), 1)
    os.makedirs(out_dir, exist_ok=True)
    Image(moved, target.affine, NiftiHeader.from_dict(fields)).save(
        os.path.join(os.fspath(out_dir), "debug_fmapref.nii.gz")
    )
