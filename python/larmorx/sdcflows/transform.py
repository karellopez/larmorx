# SPDX-License-Identifier: Apache-2.0 AND MIT
"""sdcflows' ``B0FieldTransform``: a field map given by B-spline coefficients, evaluated on a
target grid.

Ported from sdcflows 2.17.1 (``sdcflows/transform.py``, ``sdcflows/utils/tools.py``;
Apache-2.0, the NiPreps developers) with nibabel's (5.4.2, MIT) orientation and affine
arithmetic, numpy's 4×4 products in OpenBLAS's Haswell order (``lx.transforms``), SciPy's
sparse product of the collocation matrix (without storing it,
``crates/larmorx-sdcflows/src/bspline.rs``) and nitransforms' (25.1.0, MIT) resampling.

- :func:`deoblique_and_zooms`: a grid aligned with one image that covers another, rotated one,
  at a finer resolution.
- :class:`B0FieldTransform`: ``fit`` evaluates the field (Hz) on a target grid, directly or,
  for a target rotated relative to the coefficients, on a fine aligned grid resampled onto the
  target (``approx``).
"""

from __future__ import annotations

import itertools
import warnings
from collections.abc import Sequence
from dataclasses import dataclass, field
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.image import Image
from larmorx.io.nifti import NiftiHeader
from larmorx.sdcflows.bspline import NOT_ALIGNED
from larmorx.sdcflows.tools import _as_loaded, _header, rescale_affine
from larmorx.transforms._linalg import inv, matmul
from larmorx.transforms.resample import _inv_ornt_aff, _io_orientation, _ornt_transform

__all__ = ["FIELDMAP_INTENT_NAME", "B0FieldTransform", "deoblique_and_zooms"]

#: The intent name of an evaluated field map.
FIELDMAP_INTENT_NAME = "fieldmap Hz"
_INTENT_ESTIMATE = 1001
_IDENTITY_ORNT = [[0, 1], [1, 1], [2, 1]]


def _apply_affine(aff: Any, pts: Any) -> np.ndarray:
    """nibabel's ``apply_affine(aff, pts)`` for ``(n, 3)`` points or one point."""
    p = np.asarray(pts, dtype=np.float64)
    one = p.ndim == 1
    out = np.asarray(
        _core.sdcflows_apply_affine(
            np.ascontiguousarray(aff, dtype=np.float64), np.ascontiguousarray(p.reshape(-1, 3))
        )
    )
    return out[0] if one else out


def _aligned(ctrl_affine: Any, target_affine: Any) -> bool:
    """sdcflows' test that two grids are not rotated relative to each other:
    ``allclose(norm(cross(ctrl[:3, :3].T, target[:3, :3].T), axis=1), 0, atol=1e-3)``."""
    a = np.asarray(ctrl_affine)[:-1, :-1].T
    b = np.asarray(target_affine)[:-1, :-1].T
    return bool(np.allclose(np.linalg.norm(np.cross(a, b), axis=1), 0, atol=1e-3))


def _positive_cosines_affine(affine: np.ndarray, shape: Sequence[int]) -> np.ndarray:
    """The affine sdcflows' ``ensure_positive_cosines`` gives an image (``as_reoriented`` with
    the axes kept and their directions made positive)."""
    in_ornt = _io_orientation(np.asarray(affine))
    out_ornt = in_ornt.copy()
    out_ornt[:, 1] = 1
    xfm = _ornt_transform(in_ornt, out_ornt)
    if np.array_equal(xfm, _IDENTITY_ORNT):
        return np.asarray(affine)
    return matmul(affine, _inv_ornt_aff(xfm, shape))


def deoblique_and_zooms(
    in_reference: Any,
    oblique: Any,
    factor: int = 4,
    padding: int = 1,
    factor_tol: float = 1e-4,
) -> Image:
    """sdcflows' ``deoblique_and_zooms(in_reference, oblique, factor=4, padding=1,
    factor_tol=1e-4)``: a grid with ``in_reference``'s orientation that covers every corner of
    ``oblique`` plus ``padding`` voxels, ``factor`` times finer (unless ``factor`` is within
    ``factor_tol`` of 1). The result is an image of zeros (float64, nibabel's
    ``strided_scalar``, which takes no memory) with ``in_reference``'s header, the grid in its
    sform and qform (codes kept)."""
    ref = _as_loaded(in_reference)
    obl = _as_loaded(oblique)
    hdr = _header(ref)
    affine = np.array(ref.affine, dtype=np.float64)
    ref_shape = np.array(ref.shape[:3])
    ref_zooms = np.array([np.float32(z) for z in hdr.zooms[:3]], dtype=np.float32)
    corners = np.array(list(itertools.product((0, 1), repeat=3))) * (np.array(obl.shape[:3]) - 1)
    extent_ijk = _apply_affine(matmul(inv(affine), np.asarray(obl.affine)), corners)
    underflow = np.clip(extent_ijk.min(0) - padding, None, 0).astype(int)
    overflow = np.ceil(np.clip(extent_ijk.max(0) + padding + 1 - ref_shape, 0, None)).astype(int)
    if np.any(underflow < 0) or np.any(overflow > 0):
        ref_shape += overflow - underflow
        affine[:-1, -1] = _apply_affine(affine, underflow)
    if abs(1.0 - factor) > factor_tol:
        new_shape = np.rint(ref_shape * factor)
        affine = rescale_affine(affine, ref_shape, ref_zooms / factor, new_shape)
        ref_shape = new_shape
    shape = tuple(int(n) for n in ref_shape.astype(int))
    fields = hdr.to_dict()
    fields = _core.nifti_set_xform(fields, "sform", affine, int(hdr.sform_code))
    fields = _core.nifti_set_xform(fields, "qform", affine, int(hdr.qform_code))
    disk = hdr.data_dtype
    out = _core.nifti_header_for_image(
        list(shape),
        (disk if disk is not None else np.dtype(np.float64)).name,
        affine,
        fields,
        None,
    )
    # nibabel's strided_scalar: float64 zeros that take no memory (the header keeps its type)
    zeros = np.lib.stride_tricks.as_strided(np.zeros(1), shape, (0, 0, 0), writeable=False)
    return Image(zeros, affine, NiftiHeader.from_dict(out))


@dataclass
class B0FieldTransform:
    """sdcflows' ``B0FieldTransform``: the field given by the B-spline coefficients of one or
    more control grids (``coeffs``: paths or images, as ``BSplineApprox`` and
    ``TOPUPCoeffReorient`` write them), and the field evaluated on a target grid by
    :meth:`fit` (``mapped``, Hz)."""

    coeffs: Any = None
    mapped: Image | None = field(default=None, init=False)

    def _levels(self) -> list[Image]:
        items = self.coeffs if isinstance(self.coeffs, (list, tuple)) else [self.coeffs]
        return [_as_loaded(c) for c in items]

    def fit(
        self,
        target_reference: Any,
        xfm_data2fmap: Any = None,
        approx: bool = True,
        *,
        n_threads: int = 1,
    ) -> bool:
        """Evaluate the field on ``target_reference``'s grid (``mapped``).

        ``xfm_data2fmap`` (a 4×4 RAS+ matrix) maps the target's coordinates into the field
        map's space; the target grid is projected through it and its axes made positive
        (``ensure_positive_cosines``). The field is the coefficients' B-spline sum at every
        voxel of that grid. With ``approx`` (the default) and a transform given and the target
        rotated relative to the last control grid, the field is evaluated instead on a grid
        aligned with the control grid, 4 times finer and covering the target
        (:func:`deoblique_and_zooms`), and resampled onto the target with cubic B-splines.

        ``mapped`` gets the target's header: the intent "estimate" named "fieldmap Hz",
        float32, ``cal_max``/``cal_min`` (not after the approximation's resampling, which
        gives the target's header as nitransforms makes it). Returns ``True``, as sdcflows.
        """
        target = _as_loaded(target_reference)
        thdr = _header(target)
        approx = bool(approx) and xfm_data2fmap is not None
        xfm = np.eye(4) if xfm_data2fmap is None else np.asarray(xfm_data2fmap, dtype=np.float64)
        proj_affine = matmul(xfm, np.asarray(target.affine))
        proj_affine = _positive_cosines_affine(proj_affine, target.shape)
        proj_shape = tuple(int(n) for n in target.shape[:3])
        levels = self._levels()
        approx = approx and not _aligned(levels[-1].affine, target.affine)
        if approx:
            grid = deoblique_and_zooms(levels[-1], target)
            proj_affine = np.asarray(grid.affine)
            proj_shape = tuple(int(n) for n in grid.shape[:3])
        for lv in levels:
            if not _aligned(lv.affine, proj_affine):
                warnings.warn(NOT_ALIGNED, UserWarning, stacklevel=2)
        coefficients = np.hstack(
            [np.asarray(lv.data, dtype=np.float32).reshape(-1) for lv in levels]
        )
        values = _core.sdcflows_collocation_apply(
            proj_shape,
            np.ascontiguousarray(proj_affine),
            [
                (tuple(int(n) for n in lv.shape[:3]), np.ascontiguousarray(lv.affine))
                for lv in levels
            ],
            np.ascontiguousarray(coefficients),
            False,
            None,
            int(n_threads),
        )
        fmap = np.reshape(np.asarray(values), proj_shape)
        cal = max((abs(fmap.min()), fmap.max()))
        fields = thdr.to_dict()
        fields.update(
            intent_code=_INTENT_ESTIMATE,
            intent_p1=0.0,
            intent_p2=0.0,
            intent_p3=0.0,
            intent_name=FIELDMAP_INTENT_NAME.encode(),
            cal_max=float(np.float32(cal)),
            cal_min=float(-np.float32(cal)),
        )
        mhdr = _core.nifti_header_for_image(
            list(proj_shape), "float32", np.ascontiguousarray(proj_affine), fields, 1
        )
        mapped = Image(fmap, proj_affine, NiftiHeader.from_dict(mhdr))
        if approx:
            mapped = _resample_onto(mapped, target, thdr, n_threads)
        self.mapped = mapped
        return True


def _resample_onto(mapped: Image, target: Image, thdr: NiftiHeader, n_threads: int) -> Image:
    """nitransforms' ``Affine(reference=zeros on target's grid).apply(mapped)``: the field
    resampled onto the target's grid (cubic, ``constant`` 0, prefiltered), with the target's
    header (3D, float32)."""
    from larmorx.ndimage import map_coordinates

    shape3 = tuple(int(n) for n in target.shape[:3])
    disk = thdr.data_dtype
    tname = (disk if disk is not None else np.asarray(target.data).dtype).name
    ref = _core.nifti_header_for_image(
        list(shape3), tname, np.ascontiguousarray(target.affine), thdr.to_dict(), 1
    )
    coords = _core.sdcflows_affine_coordinates(
        shape3,
        np.ascontiguousarray(target.affine),
        np.ascontiguousarray(mapped.affine),
        None,
        int(n_threads),
    )
    values = map_coordinates(
        mapped.data,
        coords,
        order=3,
        mode="constant",
        cval=0.0,
        prefilter=True,
        n_threads=n_threads,
    )
    out = _core.nifti_header_for_image(
        list(shape3), "float32", np.ascontiguousarray(target.affine), ref, 1
    )
    return Image(
        np.asarray(values).reshape(shape3, order="F"), target.affine, NiftiHeader.from_dict(out)
    )
