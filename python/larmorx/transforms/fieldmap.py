# SPDX-License-Identifier: Apache-2.0 AND MIT
"""fMRIPrep's field-map steps of the BOLD workflows: ``lx.transforms.reconstruct_fieldmap``
and ``lx.transforms.distortion_parameters``.

Ported from fMRIPrep 25.2.7 (``fmriprep/interfaces/resampling.py``: ``ReconstructFieldmap``,
``reconstruct_fieldmap``, ``as_affine``, ``aligned`` and ``DistortionParameters``; the same in
25.2.6, the oracle runs'; Apache-2.0, the NiPreps developers), with sdcflows 2.17.1's
``grid_bspline_weights`` and ``ensure_positive_cosines`` (the same in 2.15.1; part A's
collocation, ``crates/larmorx-sdcflows/src/bspline.rs``), nitransforms 25.1.0's chain
(``TransformChain.asaffine``) and resampling (``resampling.apply``; MIT) and nibabel 5.4.2's
orientation and header arithmetic (MIT).

- :func:`reconstruct_fieldmap`: the field map (Hz) given by B-spline coefficients, on a target
  grid. When the transforms (target RAS+ to field-map RAS+) collapse to one affine and the
  target projected through it is aligned with the coefficients' grid, the B-spline sum is
  evaluated at the projected target's voxels directly; otherwise on the field map's reference
  grid and resampled onto the target through the transforms (cubic, nitransforms' resampling).
  The header: intent "estimate" named "fieldmap Hz", float32, ``cal_max``/``cal_min``.
- :func:`reconstruct_fieldmap_node`: the ``ReconstructFieldmap`` interface (files in, the
  ``<coefficients>rec`` file out).
- :func:`distortion_parameters`: ``DistortionParameters``, the total readout time and the
  phase-encoding direction of a BOLD run from its metadata.

Corrected by default (``upstream_defects="reproduce"`` gives fMRIPrep's result bit for bit):

- PIPE-21: the direct evaluation returns the field on the target projected into the field
  map's space and made positive (``ensure_positive_cosines``): its affine is the projected one,
  and for a target with an axis against its world axis (LAS, LPS, ...) its voxels run the other
  way along that axis, while fMRIPrep's ``ResampleSeries`` reads the field voxel for voxel on
  the target's grid (a mirrored field). larmorx returns the same values on the target's grid:
  its voxel order and its affine.

Kept and documented (rule C): PIPE-20, ``DistortionParameters`` turns a missing readout time
or phase-encoding direction (``KeyError``, ``ValueError``) into undefined outputs, and the
BOLD run is then resampled without the field map although one is assigned to it (fMRIPrep's
documented design: "downstream interfaces should be prepared to handle this"); larmorx gives
``None`` as fMRIPrep and warns.

nitransforms: Copyright (c) 2021 The NiPy developers; nibabel: Copyright (c) 2009-2019 Matthew
Brett and the NiBabel developers. MIT License; the notices are in the NOTICE file of larmorx.
"""

from __future__ import annotations

import os
import warnings
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image, as_image
from larmorx.io.nibabel_header import header_for_image, io_orientation, load_nibabel
from larmorx.io.nibabel_writer import save_nibabel
from larmorx.io.nifti import NiftiHeader
from larmorx.transforms._linalg import inv, matmul
from larmorx.transforms.chain import (
    Affine,
    AffineSeries,
    DenseField,
    Transform,
    TransformChain,
    _rust_steps,
    load_transforms,
)
from larmorx.transforms.resample import _apply_orientation, _inv_ornt_aff, _ornt_transform

__all__ = [
    "FIELDMAP_INTENT_NAME",
    "NOT_ALIGNED_REFERENCE",
    "DistortionParametersResult",
    "ReconstructFieldmapResult",
    "aligned",
    "as_affine",
    "distortion_parameters",
    "reconstruct_fieldmap",
    "reconstruct_fieldmap_node",
]

PathLike = str | os.PathLike[str]

#: The intent name of a reconstructed field map.
FIELDMAP_INTENT_NAME = "fieldmap Hz"
#: ``reconstruct_fieldmap``'s error when the field map's reference is rotated relative to the
#: coefficients' grid.
NOT_ALIGNED_REFERENCE = "Reference passed is not aligned with spline grids"
#: sdcflows' ``grid_bspline_weights`` warning (part A).
_NOT_ALIGNED = "Image's and B-Spline's grids are not aligned."
_INTENT_ESTIMATE = 1001
_IDENTITY_ORNT = [[0, 1], [1, 1], [2, 1]]
_PE_DIRECTIONS = ("i", "i-", "j", "j-", "k", "k-")


# ------------------------------------------------------------------------------------------
# as_affine, aligned


def aligned(aff1: Any, aff2: Any) -> bool:
    """fMRIPrep's ``aligned``: whether two grids are not rotated relative to each other,
    ``allclose(norm(cross(aff1[:3, :3].T, aff2[:3, :3].T), axis=1), 0, atol=1e-3)`` (the
    columns scaled by the voxel sizes, so the tolerance is in mm², and opposite axes count as
    aligned)."""
    a = np.asarray(aff1, dtype=np.float64)[:-1, :-1].T
    b = np.asarray(aff2, dtype=np.float64)[:-1, :-1].T
    return bool(np.allclose(np.linalg.norm(np.cross(a, b), axis=1), 0, atol=1e-3))


def as_affine(xfm: Transform) -> Affine | None:
    """fMRIPrep's ``as_affine``: the transform as one affine (an :class:`Affine` as it is, a
    chain of affines combined as nitransforms' ``TransformChain.asaffine`` combines them, each
    product numpy's), or ``None`` when it holds anything else (a displacement field, a nested
    chain)."""
    if isinstance(xfm, Affine):
        return xfm
    if isinstance(xfm, TransformChain) and all(isinstance(x, Affine) for x in xfm):
        steps = list(xfm.transforms)
        retval = steps[0]
        for x in steps[1:]:
            # nitransforms' ``xfm @ retval``: Affine(xfm.matrix.dot(retval.matrix))
            retval = Affine(matmul(x.matrix, retval.matrix))
        return retval
    return None


# ------------------------------------------------------------------------------------------
# reconstruct_fieldmap


def _positive(affine: np.ndarray, shape: Sequence[int]) -> tuple[np.ndarray, np.ndarray]:
    """sdcflows' ``ensure_positive_cosines`` of an image of ``shape`` and ``affine``: the
    orientation transform (axes kept, directions made positive) and the new affine
    (``affine.dot(inv_ornt_aff(...))``, nibabel's ``as_reoriented``)."""
    in_ornt = io_orientation(np.asarray(affine, dtype=np.float64))
    out_ornt = in_ornt.copy()
    out_ornt[:, 1] = 1
    xfm = _ornt_transform(in_ornt, out_ornt)
    if np.array_equal(xfm, _IDENTITY_ORNT):
        return xfm, np.asarray(affine, dtype=np.float64)
    return xfm, matmul(np.asarray(affine, dtype=np.float64), _inv_ornt_aff(xfm, shape))


def _loaded(obj: Any) -> Image:
    """An image as ``nb.load`` gives it (a path through :func:`larmorx.io.load_nibabel`)."""
    if isinstance(obj, (str, os.PathLike)):
        return load_nibabel(obj)
    return as_image(obj)


def _header_fields(img: Image) -> dict | None:
    return None if img.header is None else img.header.to_dict()


def _finish_header(
    shape: Sequence[int], affine: np.ndarray, template: dict | None, data: np.ndarray
) -> NiftiHeader:
    """The header fMRIPrep gives the field map: the image constructor's header for the data
    (``template`` copied, or a fresh one), then ``set_intent('estimate', name='fieldmap Hz')``,
    ``set_data_dtype('float32')`` and ``cal_max = max(|min|, max)``, ``cal_min = -cal_max``
    (float32, as the header stores them)."""
    affine = np.ascontiguousarray(affine, dtype=np.float64)
    fields = header_for_image(list(shape), data.dtype.name, affine, template, None)
    cal = max((abs(data.min()), data.max()))
    cal32 = np.float32(cal)
    fields.update(
        intent_code=_INTENT_ESTIMATE,
        intent_p1=0.0,
        intent_p2=0.0,
        intent_p3=0.0,
        intent_name=FIELDMAP_INTENT_NAME.encode(),
        cal_max=float(cal32),
        cal_min=float(-cal32),
    )
    return NiftiHeader.from_dict(header_for_image(list(shape), "float32", affine, fields, None))


def _resampled(
    chain: Transform,
    data: np.ndarray,
    data_affine: np.ndarray,
    shape: tuple[int, int, int],
    target_affine: np.ndarray,
    n_threads: int,
) -> np.ndarray:
    """nitransforms' ``resampling.apply(chain, Nifti1Image(data, data_affine),
    reference=target)``: the target's voxel coordinates (``ImageGrid.ndcoords``) through the
    chain and the data's ``ras2vox`` (each step rounding its points to float32 first, as
    nitransforms does), then SciPy's cubic ``map_coordinates`` (``constant`` 0, prefiltered;
    the output in the data's type)."""
    from larmorx.ndimage import map_coordinates

    if isinstance(chain, AffineSeries) or (
        isinstance(chain, TransformChain)
        and any(isinstance(s, AffineSeries) for s in chain.steps())
    ):
        raise ValueError(
            "a series of affines (one per volume) cannot map a field map onto a target grid"
        )
    steps = TransformChain((chain,)).steps()
    rust = [
        *_rust_steps(steps),
        ("affine", np.ascontiguousarray(inv(np.asarray(data_affine, dtype=np.float64)))),
    ]
    grid = _core.nitransforms_ndcoords(list(shape), np.ascontiguousarray(target_affine))
    points = np.ascontiguousarray(np.asarray(grid).reshape(-1, 3, order="F"))
    mapped = np.asarray(_core.nitransforms_map(points, rust, n_threads=int(n_threads)))
    coords = np.stack([mapped[:, c].reshape(shape, order="F") for c in range(3)])
    return map_coordinates(
        data, coords, order=3, mode="constant", cval=0.0, prefilter=True, n_threads=n_threads
    )


def reconstruct_fieldmap(
    coefficients: Sequence[Any] | Any,
    fmap_reference: Any,
    target: Any,
    transforms: Transform | Sequence[PathLike] | None = None,
    inverse: bool | Sequence[bool] = False,
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """The field map given by B-spline coefficients, on ``target``'s grid: fMRIPrep's
    ``reconstruct_fieldmap``.

    Parameters
    ----------
    coefficients
        The B-spline coefficient images (paths or images), one per level, as sdcflows writes
        them; their affines place the knots.
    fmap_reference
        The field map's reference image (the grid the field is evaluated on before it is
        resampled, when it cannot be evaluated on the target directly).
    target
        The image whose grid the field is wanted on.
    transforms
        Target RAS+ to field-map RAS+: a loaded chain (:func:`load_transforms`) or transform
        files in fMRIPrep's order with ``inverse`` (one flag, or one per file). ``None`` (or
        no files) is the identity.
    n_threads
        Worker threads (0 = all). The result does not depend on it.
    upstream_defects
        ``"fix"`` (default) returns the directly evaluated field on the target's grid
        (PIPE-21, see the module); ``"reproduce"`` gives fMRIPrep's image bit for bit. sdcflows'
        own B-spline defects (SDC-01, SDC-02) follow the same switch.

    Returns
    -------
    Image
        The field (Hz): float32 values (float64 where SciPy's sparse product upcasts) and the
        header fMRIPrep writes (stored as float32).
    """
    _defects.check(upstream_defects)
    levels = [
        _loaded(c)
        for c in (coefficients if isinstance(coefficients, (list, tuple)) else [coefficients])
    ]
    if not levels:
        raise IndexError("list index out of range")
    tgt = _loaded(target)
    fref = _loaded(fmap_reference)
    if isinstance(transforms, (Affine, AffineSeries, DenseField, TransformChain)):
        chain: Transform = transforms
    else:
        chain = load_transforms(list(transforms or []), inverse, upstream_defects=upstream_defects)

    t_affine = np.asarray(tgt.affine, dtype=np.float64)
    t_shape = tuple(int(n) for n in tgt.shape[:3])
    last = np.asarray(levels[-1].affine, dtype=np.float64)
    direct = False
    affine_xfm = as_affine(chain)
    projected = None
    if affine_xfm is not None:
        projected = matmul(np.asarray(affine_xfm.matrix, dtype=np.float64), t_affine)
        direct = aligned(projected, last)

    notices: list[Any] = []
    if direct:
        assert projected is not None
        ornt, ref_affine = _positive(projected, t_shape)
        ref_shape = t_shape
    else:
        from larmorx.niworkflows.images import as_closest_canonical
        from larmorx.niworkflows.nibabel import aff2axcodes

        # fMRIPrep's "hack": a field map reference rotated relative to RAS coefficients
        if aff2axcodes(last) == ("R", "A", "S") != aff2axcodes(fref.affine):
            fref = as_closest_canonical(fref)
        if not aligned(fref.affine, last):
            raise ValueError(NOT_ALIGNED_REFERENCE)
        ref_shape = tuple(int(n) for n in fref.shape[:3])
        ornt, ref_affine = _positive(np.asarray(fref.affine, dtype=np.float64), ref_shape)

    # Tensor-product B-spline weights of every level (sdcflows' grid_bspline_weights)
    if upstream_defects == "reproduce":
        for lv in levels:
            if not aligned(lv.affine, ref_affine):
                warnings.warn(_NOT_ALIGNED, UserWarning, stacklevel=2)
    coeffs = np.hstack([np.asarray(lv.data, dtype=np.float32).reshape(-1) for lv in levels])
    values, level_notices = _core.sdcflows_collocation_apply(
        ref_shape,
        np.ascontiguousarray(ref_affine),
        [
            (tuple(int(n) for n in lv.shape[:3]), np.ascontiguousarray(lv.affine, dtype=np.float64))
            for lv in levels
        ],
        np.ascontiguousarray(coeffs),
        False,
        None,
        int(n_threads),
        upstream_defects,
    )
    notices.extend(level_notices)
    field = np.reshape(np.asarray(values), ref_shape)

    if direct:
        moved = not np.array_equal(ornt, _IDENTITY_ORNT)
        if (moved or not np.array_equal(ref_affine, t_affine)) and upstream_defects == "fix":
            # PIPE-21: the field on the target's own grid (voxel order and affine)
            if moved:
                field = np.ascontiguousarray(_apply_orientation(field, ornt))
            notices.append(
                (
                    "PIPE-21",
                    "fMRIPrep ReconstructFieldmap",
                    upstream_defects,
                    "the directly evaluated field is returned on the target's grid (fMRIPrep "
                    "returns it on the target projected into the field map's space"
                    + (" with its axes made positive, mirrored along them" if moved else "")
                    + ")",
                )
            )
            ref_affine = t_affine
        elif moved or not np.array_equal(ref_affine, t_affine):
            notices.append(
                (
                    "PIPE-21",
                    "fMRIPrep ReconstructFieldmap",
                    upstream_defects,
                    "the directly evaluated field is on the target projected into the field "
                    "map's space"
                    + (" with its axes made positive (mirrored along them)" if moved else "")
                    + ", as fMRIPrep returns it",
                )
            )
        header = _finish_header(ref_shape, ref_affine, None, field)
        _defects.warn(notices, stacklevel=2)
        return Image(field, ref_affine, header)

    data = _resampled(chain, field, ref_affine, t_shape, t_affine, n_threads)
    header = _finish_header(t_shape, t_affine, _header_fields(tgt), data)
    _defects.warn(notices, stacklevel=2)
    return Image(data, t_affine, header)


@dataclass(frozen=True)
class ReconstructFieldmapResult:
    """``ReconstructFieldmap``'s output: the field (``fieldmap``) and its file (``out_file``,
    ``<last coefficients>rec``; ``None`` without ``out_dir``)."""

    fieldmap: Image
    out_file: Path | None = None


def reconstruct_fieldmap_node(
    in_coeffs: Sequence[PathLike] | PathLike,
    target_ref_file: PathLike,
    fmap_ref_file: PathLike,
    transforms: Sequence[PathLike] | PathLike,
    inverse: bool | Sequence[bool] = (False,),
    *,
    out_dir: PathLike | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> ReconstructFieldmapResult:
    """fMRIPrep's ``ReconstructFieldmap`` interface: :func:`reconstruct_fieldmap` of the
    coefficient files onto ``target_ref_file`` through ``transforms`` (files, fMRIPrep's
    order; ``inverse`` one flag or one per file, fMRIPrep's default ``[False]``), written as
    nibabel writes it to ``<out_dir>/<last coefficient file's stem>rec<ext>``."""
    from larmorx._nipype import ensure_list, fname_presuffix

    coeffs = ensure_list(in_coeffs) or []
    xfms = ensure_list(transforms) or []
    for name, files in (
        ("in_coeffs", coeffs),
        ("target_ref_file", [target_ref_file]),
        ("fmap_ref_file", [fmap_ref_file]),
        ("transforms", xfms),
    ):
        for f in files:
            if not os.path.isfile(os.fspath(f)):
                raise FileNotFoundError(f"{name}: no such file '{os.fspath(f)}'")
    if not coeffs or not xfms:
        raise ValueError("ReconstructFieldmap requires in_coeffs and transforms")
    flags = [bool(inverse)] if isinstance(inverse, bool) else [bool(i) for i in inverse]
    img = reconstruct_fieldmap(
        [os.fspath(c) for c in coeffs],
        os.fspath(fmap_ref_file),
        os.fspath(target_ref_file),
        [os.fspath(x) for x in xfms],
        flags,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    out = None
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
        out = Path(fname_presuffix(os.fspath(coeffs[-1]), suffix="rec", newpath=out_dir))
        save_nibabel(img, out, n_threads=n_threads)
    return ReconstructFieldmapResult(img, out)


# ------------------------------------------------------------------------------------------
# DistortionParameters


@dataclass(frozen=True)
class DistortionParametersResult:
    """``DistortionParameters``' outputs: the total readout time (s) and the phase-encoding
    direction (``None`` where fMRIPrep leaves them undefined)."""

    readout_time: float | None
    pe_direction: str | None


def _fallback(value: Any) -> float | str | None:
    """The ``fallback`` trait: ``None``, ``'estimated'`` or a float (an int becomes one)."""
    if value is None or (isinstance(value, str) and value == "estimated"):
        return value
    if isinstance(value, (bool, int, float, np.integer, np.floating)) and not isinstance(
        value, str
    ):
        return float(value)
    raise TypeError(
        "The 'fallback' trait of a DistortionParametersInputSpec instance must be None or "
        f"'estimated' or a float, but a value of {value!r} {type(value)} was specified."
    )


def distortion_parameters(
    metadata: Mapping[str, Any],
    in_file: Any = None,
    fallback: float | str | None = None,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> DistortionParametersResult:
    """fMRIPrep's ``DistortionParameters``: the BOLD run's total readout time
    (:func:`larmorx.sdcflows.get_trt` of ``metadata``, with ``in_file`` for the number of
    phase-encoding steps; ``fallback='estimated'`` uses the estimated values, a float is the
    fallback readout time) and its ``PhaseEncodingDirection``.

    A ``KeyError`` or ``ValueError`` on the way (no readout time can be found, no
    phase-encoding direction) leaves the outputs from there on ``None``, as fMRIPrep leaves
    them undefined (PIPE-20, kept: the run is then resampled without its field map); larmorx
    warns when it happens (``upstream_defects="fix"``). Other errors (``in_file`` needed but
    not given) are raised, as in fMRIPrep."""
    from larmorx.sdcflows.pepolar import _float_trait, get_trt

    _defects.check(upstream_defects)
    fb = _fallback(fallback)
    results: dict[str, Any] = {}
    try:
        results["readout_time"] = get_trt(
            metadata,
            in_file or None,
            use_estimate=fb == "estimated",
            fallback=fb if isinstance(fb, float) else None,
        )
        results["pe_direction"] = metadata["PhaseEncodingDirection"]
    except (KeyError, ValueError) as err:
        if upstream_defects == "fix":
            what = "readout time" if "readout_time" not in results else "phase-encoding direction"
            warnings.warn(
                f"PIPE-20 (fMRIPrep DistortionParameters): the {what} cannot be found "
                f"({type(err).__name__}: {err}); the outputs from there on are None, as fMRIPrep "
                "leaves them undefined, and a resampling with them applies no field map "
                "(fMRIPrep's --fallback-total-readout-time gives a readout time)",
                UserWarning,
                stacklevel=2,
            )
    ro = (
        _float_trait(results["readout_time"], "readout_time") if "readout_time" in results else None
    )
    pe = results.get("pe_direction")
    if pe is not None and pe not in _PE_DIRECTIONS:
        raise TypeError(
            "The 'pe_direction' trait of a DistortionParametersOutputSpec instance must be "
            + " or ".join(repr(p) for p in _PE_DIRECTIONS)
            + f", but a value of {pe!r} {type(pe)} was specified."
        )
    return DistortionParametersResult(ro, pe)
