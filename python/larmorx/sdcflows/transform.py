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
  target (``approx``); ``apply`` unwarps an EPI image or series with it (the shift along the
  phase-encoding axis, SciPy's spline interpolation, the Jacobian;
  ``crates/larmorx-sdcflows/src/unwarp.rs``); ``to_displacements`` gives the ITK displacement
  field.
- :func:`apply_coeffs_field`: sdcflows' ``ApplyCoeffsField`` interface (the coefficients, an
  ITK transform to the data, ``apply``; sdcflows' files, as nibabel writes them).
"""

from __future__ import annotations

import itertools
import os
import warnings
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image
from larmorx.io.nibabel_header import ensure_positive_cosines
from larmorx.io.nibabel_header import header_for_image as _nb_header_for_image
from larmorx.io.nibabel_header import io_orientation as _io_orientation
from larmorx.io.nibabel_header import set_xform as _nb_set_xform
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader
from larmorx.sdcflows.bspline import NOT_ALIGNED
from larmorx.sdcflows.tools import _as_loaded, _header, rescale_affine
from larmorx.transforms._linalg import inv, matmul
from larmorx.transforms.resample import (
    _apply_orientation,
    _inv_ornt_aff,
    _ornt2axcodes,
    _ornt_transform,
)

__all__ = [
    "ALREADY_FIT",
    "FIELDMAP_INTENT_NAME",
    "HMC_IGNORED",
    "ApplyCoeffsFieldResult",
    "B0FieldTransform",
    "apply_coeffs_field",
    "deoblique_and_zooms",
]

#: The intent name of an evaluated field map.
FIELDMAP_INTENT_NAME = "fieldmap Hz"
#: ``B0FieldTransform.apply``'s warning when the field was evaluated before (``mapped`` set).
ALREADY_FIT = (
    "The fieldmap has been already fit, the user is responsible for "
    "ensuring the parameters of the EPI target are consistent."
)
#: ``B0FieldTransform.apply``'s warning when head-motion transforms are given (and ignored).
HMC_IGNORED = (
    "Head-motion compensating (realignment) transforms are ignored when applying "
    "the unwarp with SDCFlows. This feature will be enabled as soon as unit tests "
    "are implemented for its quality assurance."
)
_PE_DIRECTIONS = ("i", "i-", "j", "j-", "k", "k-")
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
    fields = _nb_set_xform(fields, "sform", affine, int(hdr.sform_code))
    fields = _nb_set_xform(fields, "qform", affine, int(hdr.qform_code))
    disk = hdr.data_dtype
    out = _nb_header_for_image(
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
        upstream_defects: UpstreamDefectsName = "fix",
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

        ``upstream_defects="fix"`` (the default) corrects three defects of sdcflows' field,
        with an :class:`~larmorx.UpstreamDefectWarning` when they change it: positions beyond
        the control grid get the cubic B-spline's weights (SDC-01); a target rotated relative to
        the control grids without the approximation (no transform, or ``approx=False``) is
        evaluated at every voxel's own position (SDC-02; sdcflows warns that the grids are not
        aligned and multiplies per-axis weights); and the approximation's grid covers, and is
        resampled at, the target where ``xfm_data2fmap`` puts it (SDC-10; sdcflows builds and
        samples it at the target's coordinates as given, so the transform is ignored).
        ``"reproduce"`` gives sdcflows' field bit for bit.
        """
        _defects.check(upstream_defects)
        target = _as_loaded(target_reference)
        thdr = _header(target)
        approx = bool(approx) and xfm_data2fmap is not None
        xfm = np.eye(4) if xfm_data2fmap is None else np.asarray(xfm_data2fmap, dtype=np.float64)
        proj_affine = matmul(xfm, np.asarray(target.affine))
        proj_affine = _positive_cosines_affine(proj_affine, target.shape)
        proj_shape = tuple(int(n) for n in target.shape[:3])
        levels = self._levels()
        approx = approx and not _aligned(levels[-1].affine, target.affine)
        sample_affine = np.asarray(target.affine, dtype=np.float64)
        notices: list[Any] = []
        if approx:
            covered = target
            moved = not np.array_equal(xfm, np.eye(4))
            if moved:
                # SDC-10: sdcflows covers (and samples) the target as given; the field is
                # wanted where the transform puts it
                notices.append(
                    (
                        "SDC-10",
                        "sdcflows",
                        upstream_defects,
                        "the approximation's grid is built around, and resampled at, the "
                        "target where xfm_data2fmap puts it (sdcflows uses the target's own "
                        "coordinates, ignoring the transform)",
                    )
                )
                if upstream_defects == "fix":
                    sample_affine = matmul(xfm, np.asarray(target.affine))
                    covered = Image(
                        np.lib.stride_tricks.as_strided(
                            np.zeros(1, np.float32), tuple(target.shape[:3]), (0, 0, 0)
                        ),
                        sample_affine,
                    )
            grid = deoblique_and_zooms(levels[-1], covered)
            proj_affine = np.asarray(grid.affine)
            proj_shape = tuple(int(n) for n in grid.shape[:3])
        if upstream_defects == "reproduce":
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
            upstream_defects,
        )
        values, level_notices = values
        _defects.warn([*level_notices, *notices], stacklevel=2)
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
        mhdr = _nb_header_for_image(
            list(proj_shape), "float32", np.ascontiguousarray(proj_affine), fields, 1
        )
        mapped = Image(fmap, proj_affine, NiftiHeader.from_dict(mhdr))
        if approx:
            mapped = _resample_onto(mapped, target, thdr, n_threads, sample_affine)
        self.mapped = mapped
        return True

    def apply(
        self,
        moving: Any,
        pe_dir: str | Sequence[str],
        ro_time: float | Sequence[float],
        xfms: Sequence[Any] | None = None,
        jacobian: bool = True,
        xfm_data2fmap: Any = None,
        approx: bool = True,
        order: int = 3,
        mode: str = "constant",
        cval: float = 0.0,
        prefilter: bool = True,
        output_dtype: Any = None,
        num_threads: int | None = None,
        allow_negative: bool = False,
        *,
        n_threads: int = 1,
        upstream_defects: UpstreamDefectsName = "fix",
    ) -> Image:
        """Unwarp ``moving`` (a 3D image or a 4D series: path, image or nibabel image) with the
        field: sdcflows' ``B0FieldTransform.apply``.

        The image's axes are first made positive (``ensure_positive_cosines``); unless the
        field was evaluated before (``mapped``; sdcflows' warning :data:`ALREADY_FIT`), it is
        evaluated on that grid (:meth:`fit` with ``xfm_data2fmap`` and ``approx``). Every
        volume is then sampled at its voxel moved along the phase-encoding axis by
        ``field (Hz) × readout time (s)`` voxels (``pe_dir``: ``i``, ``j``, ``k`` with an
        optional ``-``, one for every volume or one for all; ``ro_time`` likewise; the shift is
        reversed when the axis runs to L, P or I, or with ``-``), with SciPy's
        ``map_coordinates`` (``order``, ``mode``, ``cval``, ``prefilter``), times
        ``1 + d(shift)/d(pe axis)`` with ``jacobian``. Values below 0 become ``cval`` unless
        ``allow_negative``. Head-motion transforms (``xfms``) are ignored with sdcflows'
        warning (:data:`HMC_IGNORED`).

        The result is float32 in memory, always 4D (a 3D input gives one volume), back in the
        input's orientation, with the input's header and ``output_dtype`` (by default the
        input's stored type) as its data type: :func:`larmorx.io.save_nibabel` writes
        it as sdcflows does (scaled into an integer type). ``num_threads`` (sdcflows' bound
        on the volumes resampled at once) is accepted and does not change the result;
        ``n_threads`` sets the threads (0 = all).

        ``upstream_defects``: two inputs on which sdcflows fails are unwarped by default
        (``"fix"``, with a warning) and fail as in sdcflows with ``"reproduce"``: a readout
        time given as an integer or a numpy float32 (a ``TypeError``, SDC-05), and a series
        read from a file whose axes already run to R, A and S (an ``AttributeError``,
        SDC-08). The field itself is :meth:`fit`'s, with its corrections (SDC-01, SDC-02,
        SDC-10) or sdcflows' field.
        """
        _defects.check(upstream_defects)
        lazy = _is_proxy(moving)
        src = _as_loaded(moving)
        hdr = _header(src)
        data = np.asanyarray(src.data)
        if data.ndim < 3:
            raise ValueError("Data array has fewer dimensions than orientation")
        pc_data, pc_affine, axcodes = ensure_positive_cosines(data, np.asarray(src.affine))
        pc_affine = np.asarray(pc_affine, dtype=np.float64)
        pc_header = hdr
        if pc_data is not data:
            pc_header = _rebuilt_header(hdr, pc_data.shape, pc_affine, _disk_dtype(hdr, data))
        deferred: list[Any] = []
        if self.mapped is not None:
            warnings.warn(ALREADY_FIT, UserWarning, stacklevel=2)
        else:
            # The field's D13 notices are named once the series is unwarped (a series that
            # cannot be unwarped fails alike in both modes, with nothing corrected).
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                self.fit(
                    Image(pc_data, pc_affine, pc_header),
                    xfm_data2fmap=xfm_data2fmap,
                    approx=approx,
                    n_threads=n_threads,
                    upstream_defects=upstream_defects,
                )
            for w in caught:
                if issubclass(w.category, _defects.UpstreamDefectWarning):
                    deferred.append(w.message)
                else:
                    warnings.warn_explicit(w.message, w.category, w.filename, w.lineno)
        newshape = pc_data.shape[:3] + tuple(d for d in pc_data.shape[3:] if d > 1)
        vols = pc_data.reshape(newshape)
        n_volumes = vols.shape[3] if vols.ndim == 4 else 1
        out_dtype = np.dtype(output_dtype or _disk_dtype(pc_header, pc_data))
        pe_list: Any = [pe_dir] if isinstance(pe_dir, str) else pe_dir
        ro_list, notices = _readout_list(ro_time, n_volumes, upstream_defects)
        if n_volumes > 1 and len(pe_list) == 1:
            pe_list = list(pe_list) * n_volumes
        if n_volumes > 1 and len(ro_list) == 1:
            ro_list = list(ro_list) * n_volumes
        _defects.warn(notices)
        pe_info = []
        for vol_pe_dir, vol_ro_time in zip(pe_list, ro_list, strict=False):
            if np.result_type(np.float32, vol_ro_time) not in (np.float32, np.float64):
                raise TypeError(f"a readout time of type {_scalar_name(vol_ro_time)} is not ported")
            pe_axis = "ijk".index(vol_pe_dir[0])
            flip = (axcodes[pe_axis] in "LPI") ^ vol_pe_dir.endswith("-")
            pe_info.append((pe_axis, -vol_ro_time if flip else vol_ro_time))
        if xfms is not None:
            warnings.warn(HMC_IGNORED, UserWarning, stacklevel=2)
        if lazy and pc_data is data and vols.ndim > 3:
            # sdcflows hands nibabel's ArrayProxy (a file's data, not reoriented) to rollaxis
            if upstream_defects == "reproduce":
                raise AttributeError("'ArrayProxy' object has no attribute 'transpose'")
            detail = (
                "a series read from a file with positive axes is unwarped; sdcflows raises "
                "an AttributeError"
            )
            _defects.warn([("SDC-08", "sdcflows", "fix", detail)])
        series = vols if vols.ndim > 3 else vols[..., np.newaxis]
        n_iter = series.shape[-1]
        if len(pe_info) < n_iter:
            raise IndexError("list index out of range")
        if series.ndim > 4:
            raise RuntimeError("invalid shape for coordinate array")
        assert self.mapped is not None
        fmap = np.ascontiguousarray(np.asarray(self.mapped.data, dtype=np.float32))
        resampled = np.empty(series.shape, dtype=np.float32, order="F")
        for t in range(n_iter):
            volume = _interpolable(series[..., t])
            pe_axis, ro = pe_info[t]
            values, vnotices = _core.sdcflows_unwarp_volume(
                volume,
                fmap,
                int(pe_axis),
                float(ro),
                bool(np.result_type(np.float32, ro) == np.float64),
                bool(jacobian),
                int(order),
                mode,
                float(cval),
                bool(prefilter),
                int(n_threads),
                upstream_defects,
            )
            _defects.warn(vnotices)
            resampled[..., t] = values
        if not allow_negative:
            resampled[resampled < 0] = cval
        for message in deferred:
            warnings.warn(message, stacklevel=2)
        moved_header = _rebuilt_header(pc_header, resampled.shape, pc_affine, out_dtype)
        return _reoriented(Image(resampled, pc_affine, moved_header), axcodes)

    def to_displacements(self, ro_time: float, pe_dir: str, itk_format: bool = True) -> Image:
        """The field as an ITK/ANTs displacement field (sdcflows' ``to_displacements``,
        :func:`larmorx.sdcflows.fmap_to_disp` of ``mapped``)."""
        from larmorx.sdcflows.fmap import fmap_to_disp

        if self.mapped is None:
            raise AttributeError("'NoneType' object has no attribute 'get_fdata'")
        return fmap_to_disp(self.mapped, ro_time, pe_dir, itk_format=itk_format)


def _is_proxy(moving: Any) -> bool:
    """Whether sdcflows would hold ``moving``'s data as nibabel's ``ArrayProxy`` (a path, or a
    nibabel image read from a file)."""
    if isinstance(moving, (str, bytes, os.PathLike)):
        return True
    dataobj = getattr(moving, "dataobj", None)
    if dataobj is None or isinstance(moving, Image):
        return False
    return not isinstance(dataobj, np.ndarray) and hasattr(dataobj, "slope")


def _disk_dtype(hdr: NiftiHeader, data: np.ndarray) -> np.dtype:
    """The header's stored type (``header.get_data_dtype()``), or the array's without one."""
    disk = hdr.data_dtype
    return np.dtype(disk) if disk is not None else np.asarray(data).dtype


def _rebuilt_header(hdr: NiftiHeader, shape: Sequence[int], affine: Any, dtype: Any) -> NiftiHeader:
    """The header nibabel's image constructor makes from ``hdr`` for an array of ``shape``
    placed by ``affine`` (``update_header``), with ``dtype`` stored."""
    out = _nb_header_for_image(
        [int(n) for n in shape],
        np.dtype(dtype).name,
        np.ascontiguousarray(affine, dtype=np.float64),
        hdr.to_dict(),
        None,
    )
    return NiftiHeader.from_dict(out)


def _axcodes2ornt(axcodes: Sequence[str | None]) -> np.ndarray:
    """nibabel's ``axcodes2ornt`` with the default labels (L/R, P/A, I/S)."""
    labels = list(zip("LPI", "RAS", strict=True))
    ornt = np.ones((len(axcodes), 2), dtype=np.int8) * np.nan
    for code_idx, code in enumerate(axcodes):
        for label_idx, codes in enumerate(labels):
            if code is None:
                continue
            if code in codes:
                ornt[code_idx, :] = [label_idx, -1 if code == codes[0] else 1]
                break
    return ornt


def _reoriented(img: Image, target_axcodes: Sequence[str | None]) -> Image:
    """niworkflows' ``reorient_image(img, target_ornt)``: ``as_reoriented`` from the image's
    axis codes to ``target_axcodes``."""
    img_axcodes = _ornt2axcodes(_io_orientation(np.asarray(img.affine)))
    xfm = _ornt_transform(_axcodes2ornt(img_axcodes), _axcodes2ornt(target_axcodes))
    if np.array_equal(xfm, _IDENTITY_ORNT):
        return img
    data = _apply_orientation(img.data, xfm)
    affine = matmul(np.asarray(img.affine), _inv_ornt_aff(xfm, img.shape))
    assert img.header is not None
    return Image(
        data, affine, _rebuilt_header(img.header, data.shape, affine, img.header.data_dtype)
    )


def _scalar_name(x: Any) -> str:
    t = type(x)
    return t.__name__ if t.__module__ == "builtins" else f"{t.__module__}.{t.__name__}"


def _readout_list(
    ro_time: Any, n_volumes: int, upstream_defects: str
) -> tuple[list[Any], list[tuple[str, str, str, str]]]:
    """``ro_time`` as the list sdcflows zips with the phase-encoding directions: a Python
    float is wrapped; another number fails in sdcflows (SDC-05) and is wrapped by larmorx."""
    if isinstance(ro_time, float):
        return [ro_time], []
    if isinstance(ro_time, (int, np.number)) and not isinstance(ro_time, bool):
        name = _scalar_name(ro_time)
        if upstream_defects == "reproduce":
            if n_volumes > 1:
                raise TypeError(f"object of type '{name}' has no len()")
            raise TypeError(f"'{name}' object is not iterable")
        np.result_type(np.float32, ro_time)  # numpy's own TypeError for an unusable type
        detail = f"a readout time of type {name} is used; sdcflows raises a TypeError"
        return [ro_time], [("SDC-05", "sdcflows", "fix", detail)]
    return list(ro_time), []


def _interpolable(volume: np.ndarray) -> np.ndarray:
    """The volume as ``map_coordinates`` reads it (``lx.ndimage``'s rule: other types, bool
    and non-native byte orders as float64)."""
    from larmorx.ndimage import _INPUT_TYPES

    if volume.dtype.kind == "c":
        raise TypeError("complex input is not supported by larmorx")
    if volume.dtype.type not in _INPUT_TYPES or not volume.dtype.isnative:
        return volume.astype(np.float64)
    return volume


def _resample_onto(
    mapped: Image, target: Image, thdr: NiftiHeader, n_threads: int, sample_affine: Any = None
) -> Image:
    """nitransforms' ``Affine(reference=zeros on target's grid).apply(mapped)``: the field
    resampled onto the target's grid (cubic, ``constant`` 0, prefiltered), with the target's
    header (3D, float32). ``sample_affine``: where the target's voxels are sampled (SDC-10
    corrected: the transform applied); by default the target's own affine."""
    from larmorx.ndimage import map_coordinates

    shape3 = tuple(int(n) for n in target.shape[:3])
    disk = thdr.data_dtype
    tname = (disk if disk is not None else np.asarray(target.data).dtype).name
    ref = _nb_header_for_image(
        list(shape3), tname, np.ascontiguousarray(target.affine), thdr.to_dict(), 1
    )
    coords = _core.sdcflows_affine_coordinates(
        shape3,
        np.ascontiguousarray(target.affine if sample_affine is None else sample_affine),
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
    out = _nb_header_for_image(list(shape3), "float32", np.ascontiguousarray(target.affine), ref, 1)
    return Image(
        np.asarray(values).reshape(shape3, order="F"), target.affine, NiftiHeader.from_dict(out)
    )


@dataclass(frozen=True)
class ApplyCoeffsFieldResult:
    """What ``ApplyCoeffsField`` computes and writes: the unwarped data (``corrected``,
    ``out_corrected``: ``<in_data>_sdc``) and the field on its grid (``field``,
    ``out_field``: ``<in_data>_field``); the paths with ``out_dir``, else ``None``."""

    corrected: Image
    field: Image
    out_corrected: Path | None = None
    out_field: Path | None = None


def _itk_affine(path: Any) -> np.ndarray:
    """``nitransforms.linear.Affine.from_filename(path, fmt='itk').matrix``."""
    from larmorx.transforms.chain import Affine, TransformFileError, load_itk_linear

    loaded = load_itk_linear(path)
    if not isinstance(loaded, Affine):
        raise TransformFileError("More than one linear transform found.")
    return _normalised(np.asarray(loaded.matrix, dtype=np.float64))


def _normalised(m: np.ndarray) -> np.ndarray:
    """nitransforms' ``Affine(matrix)``: the last row checked and set to ``(0, 0, 0, 1)``."""
    m = np.array(m, dtype=np.float64)
    if not np.allclose(m[3, :], (0, 0, 0, 1)):
        raise ValueError(
            f"The last row of a homogeneus matrix should be (0, 0, 0, 1), got {m[3, :]}."
        )
    m[3, :] = (0, 0, 0, 1)
    return m


def _first(x: Any) -> Any:
    return x[0] if isinstance(x, (list, tuple)) else x


def apply_coeffs_field(
    in_data: Any,
    in_coeff: Any,
    ro_time: Any,
    pe_dir: Any,
    *,
    fmap2data_xfm: Any = None,
    data2fmap_xfm: Any = None,
    in_xfms: Sequence[Any] | None = None,
    jacobian: bool = False,
    num_threads: int | None = None,
    approx: bool = True,
    out_dir: str | os.PathLike[str] | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ApplyCoeffsFieldResult:
    """sdcflows' ``ApplyCoeffsField``: unwarp ``in_data`` with the field of the B-spline
    coefficients ``in_coeff`` (one path or image, or a list).

    ``data2fmap_xfm`` (or ``fmap2data_xfm``, its inverse; exclusive) is an ITK linear
    transform file (the first of a list) that maps the data's coordinates into the field
    map's; ``ro_time`` and ``pe_dir`` (``i``, ``i-``, ``j``, ``j-``, ``k``, ``k-``) are one value
    or one per volume; ``jacobian`` is off by default here (sdcflows' interface default);
    ``in_xfms`` (head-motion matrices) are ignored with sdcflows' warning. The field is
    evaluated on the data's grid (``approx``: on a fine aligned grid, then resampled, when the
    data are rotated relative to the coefficients) and applied by
    :meth:`B0FieldTransform.apply`. With ``out_dir`` the corrected data and the field are
    written there as ``<in_data>_sdc`` and ``<in_data>_field`` (nibabel's writer:
    :func:`larmorx.io.save_nibabel`).
    """
    if fmap2data_xfm is not None and data2fmap_xfm is not None:
        raise ValueError("fmap2data_xfm and data2fmap_xfm are mutually exclusive")
    pes = list(pe_dir) if isinstance(pe_dir, (list, tuple)) else [pe_dir]
    for p in pes:
        if p not in _PE_DIRECTIONS:
            raise ValueError(f"pe_dir must be one of {_PE_DIRECTIONS}, not {p!r}")
    ros = [float(r) for r in (ro_time if isinstance(ro_time, (list, tuple)) else [ro_time])]
    xfm = None
    if data2fmap_xfm is not None:
        xfm = _itk_affine(_first(data2fmap_xfm))
    elif fmap2data_xfm is not None:
        xfm = _normalised(inv(_itk_affine(_first(fmap2data_xfm))))
    coeffs = list(in_coeff) if isinstance(in_coeff, (list, tuple)) else [in_coeff]
    unwarp = B0FieldTransform(coeffs=[_as_loaded(c) for c in coeffs])
    corrected = unwarp.apply(
        in_data,
        pes,
        ros,
        jacobian=jacobian,
        xfms=list(in_xfms) if in_xfms else None,
        xfm_data2fmap=xfm,
        approx=approx,
        num_threads=num_threads or None,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    assert unwarp.mapped is not None
    paths: dict[str, Path] = {}
    if out_dir is not None:
        from larmorx.sdcflows.approx import _fname_presuffix

        os.makedirs(out_dir, exist_ok=True)
        name = os.fspath(in_data) if isinstance(in_data, (str, os.PathLike)) else "data.nii.gz"
        paths["out_field"] = Path(_fname_presuffix(name, "_field", out_dir))
        paths["out_corrected"] = Path(_fname_presuffix(name, "_sdc", out_dir))
        save_nibabel(corrected, paths["out_corrected"], n_threads=n_threads)
        save_nibabel(unwarp.mapped, paths["out_field"], n_threads=n_threads)
    return ApplyCoeffsFieldResult(corrected, unwarp.mapped, **paths)
